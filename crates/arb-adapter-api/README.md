# Read-only adapter boundary

`ReadRpc` accepts a closed `ReadMethod` enum containing only six state-query methods. There is no raw method string, signing key, transaction builder or submission method. `HttpReadRpc` requires HTTPS outside loopback, disables redirects, bounds every response to at most 8 MiB, retained response bytes to 64 MiB, requests to at most 4,096, each request to its configured timeout, and the whole capture to 60 seconds. Failed requests consume the request quota. Transport/RPC errors omit endpoint URLs and provider error bodies.

`RpcRecord` preserves the exact successful JSON response body, request parameters and acquisition sequence. `TranscriptRpc` repeats the same calls offline and rejects mismatched parameters, wrong sequences, RPC errors, missing calls and unconsumed inputs. `SnapshotQuality::ineligibility` rejects missing state, unqualified mathematics, incoherence, stale/future timestamps and explicit invalidation. This helper does not discover reorgs or supply an opportunity-evidence grant.

The HTTP client is synchronous and intended for bounded one-shot capture work. A continuous service must isolate it from its command/control runtime. It does not acknowledge dashboard Start/Pause/Stop commands.
