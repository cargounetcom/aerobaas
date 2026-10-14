-module(aero_logger).
-export([info/1, info/2, warn/1, warn/2, error/1, error/2]).

info(Msg) -> info(Msg, []).
info(Fmt, Args) -> log(info, Fmt, Args).

warn(Msg) -> warn(Msg, []).
warn(Fmt, Args) -> log(warn, Fmt, Args).

error(Msg) -> error(Msg, []).
error(Fmt, Args) -> log(error, Fmt, Args).

log(Level, Fmt, Args) ->
    Timestamp = calendar:now_to_local_time(erlang:now()),
    Formatted = io_lib:format(Fmt, Args),
    io:format("[~p] [~p] ~s~n", [Timestamp, Level, Formatted]).
