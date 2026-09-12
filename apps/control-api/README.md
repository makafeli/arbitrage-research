# Control API foundation

This process implements **only** a development liveness endpoint:

```sh
cargo run -p control-api
curl http://127.0.0.1:8080/healthz
```

It binds `127.0.0.1:8080`, reports the process version and explicitly reports that
trading and persistence are unavailable. `OK` means that this liveness handler is
responding; it does not mean workers, RPC, simulation or databases are ready.
Ctrl-C stops this HTTP process. Stopping this process is not an implementation of
a future worker's STOP command.

There is no `/v1` application route, authentication, session storage, command
processor, configuration mutation, opportunity feed, key handling or submission.
The OpenAPI document specifies the planned authenticated API. Its `/v1/health`
route is distinct from this minimal unauthenticated local `/healthz` probe. Do not
expose this scaffold as a production control service.

The dashboard runs against its own marked synthetic fixtures. It is not connected
to this process yet. API implementation and integration remain tracked work.
