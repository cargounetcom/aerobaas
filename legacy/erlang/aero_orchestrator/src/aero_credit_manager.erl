-module(aero_credit_manager).
-behaviour(gen_server).

-export([start_link/0, consume/3, get_balance/1, top_up/2, deactivate/1]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2, code_change/3]).

-define(TABLE, aero_credits).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    ets:new(?TABLE, [named_table, set, public, {read_concurrency, true}]),
    {ok, #state{}}.

consume(KeyId, Amount, Limit) ->
    gen_server:call(?MODULE, {consume, KeyId, Amount, Limit}).

get_balance(KeyId) ->
    case ets:lookup(?TABLE, KeyId) of
        [{KeyId, Balance, _Active}] -> {ok, Balance};
        [] -> {ok, unlimited}
    end.

top_up(KeyId, Amount) ->
    gen_server:cast(?MODULE, {top_up, KeyId, Amount}).

deactivate(KeyId) ->
    gen_server:cast(?MODULE, {deactivate, KeyId}).

handle_call({consume, KeyId, Amount, Limit}, _From, State) ->
    Result = case ets:lookup(?TABLE, KeyId) of
        [{KeyId, Balance, true}] ->
            NewBalance = Balance - Amount,
            case NewBalance =< 0 of
                true ->
                    ets:insert(?TABLE, {KeyId, 0, false}),
                    {error, credits_exhausted};
                false ->
                    ets:insert(?TABLE, {KeyId, NewBalance, true}),
                    {ok, NewBalance}
            end;
        [{KeyId, _Balance, false}] ->
            {error, key_deactivated};
        [] ->
            ets:insert(?TABLE, {KeyId, Limit - Amount, true}),
            {ok, Limit - Amount}
    end,
    {reply, Result, State};

handle_call(_Request, _From, State) ->
    {reply, ok, State}.

handle_cast({top_up, KeyId, Amount}, State) ->
    case ets:lookup(?TABLE, KeyId) of
        [{KeyId, Balance, _Active}] ->
            ets:insert(?TABLE, {KeyId, Balance + Amount, true});
        [] ->
            ets:insert(?TABLE, {KeyId, Amount, true})
    end,
    {noreply, State};

handle_cast({deactivate, KeyId}, State) ->
    case ets:lookup(?TABLE, KeyId) of
        [{KeyId, Balance, _}] ->
            ets:insert(?TABLE, {KeyId, Balance, false});
        [] ->
            ok
    end,
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

-record(state, {}).
