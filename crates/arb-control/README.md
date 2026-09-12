# arb-control

`ControlWorker` coordinates one research session with PostgreSQL intent and an exclusive local admission gate. It claims a chain-specific lease/epoch, starts RECOVERING, completes recovery only with no unresolved research attempts, then waits STOPPED for a fresh operator command. Polling never means the API itself acknowledged a command.

Queue evaluation work with a `WorkGeneration` obtained when RUNNING. Admit completed results through the same worker; pause/stop increments the generation and stale results are rejected even after resume. Command acknowledgement and admission share an async mutex held through database commit. Database errors close the gate and latch recovery-required; subsequent polling cannot silently reopen it. Lease expiry closes admission on the next generation/admission operation. Reconciliation remains available when paused or draining. No signing or network transaction submission exists.

The standalone `evm-worker` and `solana-worker` one-shot capture commands remain independent tools. The [research-worker runtime](../../apps/research-worker/README.md) integrates their capture adapters with this coordinator for OBSERVE-only sessions. API controls apply to registered coordinator instances; every executable is not implicitly controlled.

Durable research attempt IDs support deterministic lifecycle/recovery tests; terminal resolution is idempotent per session/attempt and conflicting evidence is rejected. They are not signed transaction identities or a paper ledger. Positive resolution evidence is required by API shape, but actual protocol settlement evidence and financial accounting belong to the execution/ledger tickets.

Cancellation safety: the gate closes and latches recovery-required before every database mutation await. A cancelled future cannot reopen it; only a successfully committed operation may restore the previous admission state. A held database row-lock test drops a polling future and verifies the latch.

Readiness loss during RUNNING records FAULTED and increments the work generation. A later healthy poll cannot reopen a faulted session. START/RESUME remain PENDING while not ready; PAUSE/STOP still apply their fence regardless of readiness.
