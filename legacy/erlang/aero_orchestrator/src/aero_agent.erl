-module(aero_agent).
-behaviour(gen_server).

-export([start_link/2, execute/2, get_state/1, stop/1]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2, code_change/3]).

-record(state, {
    id :: binary(),
    workspace_id :: binary(),
    status = idle :: idle | running | waiting,
    current_task :: term() | undefined,
    tasks_completed = 0 :: non_neg_integer(),
    created_at :: erlang:timestamp(),
    last_active :: erlang:timestamp()
}).

start_link(AgentId, Config) ->
    gen_server:start_link(?MODULE, {AgentId, Config}, []).

execute(Pid, Task) ->
    gen_server:call(Pid, {execute, Task}, 300000).

get_state(Pid) ->
    gen_server:call(Pid, get_state).

stop(Pid) ->
    gen_server:stop(Pid).

init({AgentId, Config}) ->
    WorkspaceId = maps:get(workspace_id, Config, <<"default">>),
    Now = erlang:timestamp(),
    {ok, #state{
        id = AgentId,
        workspace_id = WorkspaceId,
        created_at = Now,
        last_active = Now
    }}.

handle_call({execute, Task}, _From, State) ->
    NewState = State#state{
        status = running,
        current_task = Task,
        last_active = erlang:timestamp()
    },
    Result = execute_task(Task, NewState),
    Reply = {ok, Result},
    UpdatedState = NewState#state{
        status = idle,
        current_task = undefined,
        tasks_completed = NewState#state.tasks_completed + 1
    },
    {reply, Reply, UpdatedState};

handle_call(get_state, _From, State) ->
    {reply, {ok, State}, State}.

handle_cast(_Msg, State) ->
    {noreply, State}.

handle_info(timeout, State) ->
    {stop, normal, State};

handle_info(_Info, State) ->
    {noreply, State}.

terminate(_Reason, _State) ->
    ok.

code_change(_OldVsn, State, _Extra) ->
    {ok, State}.

execute_task(_Task, _State) ->
    {completed, erlang:system_time(millisecond)}.
