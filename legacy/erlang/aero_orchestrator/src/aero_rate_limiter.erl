-module(aero_rate_limiter).
-behaviour(gen_server).

-export([start_link/0, check_rate/2, consume/3, get_balance/1, reset/1]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2, code_change/3]).

-define(DEFAULT_RATE, 5000).
-define(TABLE, aero_rate_table).

-record(state, {}).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    ets:new(?TABLE, [named_table, set, public, {read_concurrency, true}]),
    {ok, #state{}}.

check_rate(Key, _Limit) ->
    case ets:lookup(?TABLE, Key) of
        [{Key, Balance, _LastReset}] when Balance > 0 ->
            {ok, Balance};
        [{_Key, 0, _}] ->
            {error, rate_limited};
        [] ->
            {ok, unlimited}
    end.

consume(Key, Amount, Limit) ->
    gen_server:call(?MODULE, {consume, Key, Amount, Limit}).

get_balance(Key) ->
    case ets:lookup(?TABLE, Key) of
        [{Key, Balance, _}] -> {ok, Balance};
        [] -> {ok, unlimited}
    end.

reset(Key) ->
    gen_server:cast(?MODULE, {reset, Key}).

handle_call({consume, Key, Amount, Limit}, _From, State) ->
    Result = case ets:lookup(?TABLE, Key) of
        [{Key, Balance, LastReset}] ->
            Now = erlang:system_time(second),
            NewBalance = case Now - LastReset > 60 of
                true -> Limit - Amount;
                false -> Balance - Amount
            end,
            ets:insert(?TABLE, {Key, NewBalance, Now}),
            case NewBalance > 0 of
                true -> {ok, NewBalance};
                false -> {error, rate_limited}
            end;
        [] ->
            Now = erlang:system_time(second),
            ets:insert(?TABLE, {Key, Limit - Amount, Now}),
            {ok, Limit - Amount}
    end,
    {reply, Result, State};

handle_call(_Request, _From, State) ->
    {reply, ok, State}.

handle_cast({reset, Key}, State) ->
    ets:delete(?TABLE, Key),
    {noreply, State};

handle_cast(_Msg, State) ->
    {noreply, State}.

handle_info(_Info, State) ->
    {noreply, State}.

terminate(_Reason, _State) ->
    ets:delete(?TABLE),
    ok.

code_change(_OldVsn, State, _Extra) ->
    {ok, State}.
