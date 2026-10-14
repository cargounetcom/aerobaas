-module(aero_orchestrator_app).
-behaviour(application).

-export([start/2, stop/1]).

start(_StartType, _StartArgs) ->
    {ok, _Pid} = aero_orchestrator_sup:start_link(),
    aero_logger:info("AeroBaaS Orchestrator started"),
    {ok, self()}.

stop(_State) ->
    aero_logger:info("AeroBaaS Orchestrator stopped"),
    ok.
