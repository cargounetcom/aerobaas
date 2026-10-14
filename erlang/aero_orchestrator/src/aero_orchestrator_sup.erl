-module(aero_orchestrator_sup).
-behaviour(supervisor).

-export([start_link/0, init/1]).

start_link() ->
    supervisor:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    SupFlags = #{
        strategy => one_for_one,
        intensity => 10,
        period => 60
    },

    AgentSup = #{
        id => aero_agent_sup,
        start => {aero_agent_sup, start_link, []},
        type => supervisor
    },

    RateLimiter = #{
        id => aero_rate_limiter,
        start => {aero_rate_limiter, start_link, []},
        type => worker
    },

    LockManager = #{
        id => aero_lock_manager,
        start => {aero_lock_manager, start_link, []},
        type => worker
    },

    CreditManager = #{
        id => aero_credit_manager,
        start => {aero_credit_manager, start_link, []},
        type => worker
    },

    {ok, {SupFlags, [AgentSup, RateLimiter, LockManager, CreditManager]}}.
