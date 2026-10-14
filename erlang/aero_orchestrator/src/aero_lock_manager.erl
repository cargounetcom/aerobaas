-module(aero_lock_manager).
-behaviour(gen_server).

-export([start_link/0, acquire/2, release/2, acquire/3, is_locked/1]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2, code_change/3]).

-define(TABLE, aero_locks).
-define(DEFAULT_TIMEOUT, 30000).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    ets:new(?TABLE, [named_table, set, public]),
    {ok, #state{}}.

acquire(Resource, AgentId) ->
    acquire(Resource, AgentId, ?DEFAULT_TIMEOUT).

acquire(Resource, AgentId, Timeout) ->
    gen_server:call(?MODULE, {acquire, Resource, AgentId, Timeout}, Timeout + 1000).

release(Resource, AgentId) ->
    gen_server:call(?MODULE, {release, Resource, AgentId}).

is_locked(Resource) ->
    case ets:lookup(?TABLE, Resource) of
        [{Resource, _AgentId, _Expiry}] -> true;
        [] -> false
    end.

handle_call({acquire, Resource, AgentId, Timeout}, _From, State) ->
    Now = erlang:system_time(millisecond),
    Expiry = Now + Timeout,
    Result = case ets:lookup(?TABLE, Resource) of
        [{Resource, HolderAgent, ExpiryTime}] when ExpiryTime > Now ->
            {error, {locked, HolderAgent}};
        [{Resource, _Holder, _ExpiredExpiry}] ->
            ets:insert(?TABLE, {Resource, AgentId, Expiry}),
            {ok, acquired};
        [] ->
            ets:insert(?TABLE, {Resource, AgentId, Expiry}),
            {ok, acquired}
    end,
    {reply, Result, State};

handle_call({release, Resource, AgentId}, _From, State) ->
    case ets:lookup(?TABLE, Resource) of
        [{Resource, AgentId, _}] ->
            ets:delete(?TABLE, Resource),
            {reply, ok, State};
        [{Resource, OtherAgent, _}] ->
            {reply, {error, {not_owner, OtherAgent}}, State};
        [] ->
            {reply, {error, not_locked}, State}
    end;

handle_call(_Request, _From, State) ->
    {reply, ok, State}.

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
