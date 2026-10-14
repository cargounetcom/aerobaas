-module(aero_agent_sup).
-behaviour(supervisor).

-export([start_link/0, init/1, start_agent/2, stop_agent/1]).

start_link() ->
    supervisor:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    SupFlags = #{
        strategy => one_for_one,
        intensity => 1000,
        period => 10
    },
    {ok, {SupFlags, []}}.

start_agent(AgentId, Config) ->
    ChildSpec = #{
        id => {agent, AgentId},
        start => {aero_agent, start_link, [AgentId, Config]},
        type => worker
    },
    supervisor:start_child(?MODULE, ChildSpec).

stop_agent(AgentId) ->
    supervisor:terminate_child(?MODULE, {agent, AgentId}),
    supervisor:delete_child(?MODULE, {agent, AgentId}).
