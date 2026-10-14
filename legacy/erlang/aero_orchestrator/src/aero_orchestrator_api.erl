-module(aero_orchestrator_api).
-export([
    start_agent/2,
    stop_agent/1,
    execute_task/2,
    acquire_lock/2,
    release_lock/2,
    consume_credits/3,
    check_rate/2,
    get_agent_state/1
]).

start_agent(AgentId, Config) ->
    aero_agent_sup:start_agent(AgentId, Config).

stop_agent(AgentId) ->
    aero_agent_sup:stop_agent(AgentId).

execute_task(AgentPid, Task) ->
    aero_agent:execute(AgentPid, Task).

acquire_lock(Resource, AgentId) ->
    aero_lock_manager:acquire(Resource, AgentId).

release_lock(Resource, AgentId) ->
    aero_lock_manager:release(Resource, AgentId).

consume_credits(KeyId, Amount, Limit) ->
    aero_credit_manager:consume(KeyId, Amount, Limit).

check_rate(Key, Limit) ->
    aero_rate_limiter:check_rate(Key, Limit).

get_agent_state(AgentPid) ->
    aero_agent:get_state(AgentPid).
