# Decision source continuity in the dashboard

Existing tasks: #32 (ARB-018) and #51 (ARB-037). Source baseline: PR141 / `b3867784`.
This is a read-only diagnostic component, not a market worker deployment or a paper settlement engine.

## Operator view

In Paper trading, open Opportunities, select a decision session and choose **Inspect evidence**.
The **Source continuity** panel independently retrieves the current stored source projection.
**Refresh source status** repeats that bounded read; it does not reacquire market data.
The original decision, captured prices, historical chain-time assessment and selected-trace export remain unchanged.

- **No known source invalidation** means only that the linked stream has no recorded terminal invalidation at the database read snapshot.
- **Source invalidated** includes fixed public reasons from matching linked streams. This does not imply that all historical blocks were reorganized.
- **Source continuity not tracked** means the links are absent or incomplete; legacy data is not retrospectively approved.
- **Source continuity unverifiable** includes unsupported chains and contradictory evidence.

A failed or unauthorized refresh must never produce a healthy state. The last received status, when retained, is explicitly labelled as stale and retains its original check timestamp. A newly selected observation cannot borrow another observation's status. The panel does not poll in the background. Opening the inspector or explicitly refreshing performs a read.

## API contract

`GET /v1/sessions/{session_id}/decisions/{observation_id}/continuity` is protected by the existing account/session middleware, read-capacity limits and request deadline. It accepts no query parameters. Missing or cross-operator/session resources return the same not-found response. Storage failures are redacted. All responses retain the existing no-store and request-correlation headers.

The additive `1.0.0` response contains `assessment_kind=CONTINUITY_ONLY`, `authorizes_execution=false`, exact trace/session/observation identities, network, the database statement timestamp, policy version, status, decimal-string capture/bound counts (0..64 for retained history), and at most four distinct public invalidation reasons. The projection and reasons are read in one SQL statement. This is an MVCC read snapshot, not a lock-held permission or freshness guarantee.

No network endpoint, raw provider message, account secret, capture file path, signing or broadcast interface is exposed. Existing transaction-bound publication/consumption gates remain mandatory for actual decisions. A display response is never passed back as authority.

## Verification and limits

Added tests cover real PostgreSQL read-back across source HALT with immutable history, owner/session scope, missing and zero-input records, strict browser contracts, authenticated HTTP routing, public error handling, visible invalidation, failed-refresh retention and mismatched source responses. Database and browser fixtures are synthetic. Passing counts, exact source and CI runs must be read from the associated pull request rather than inferred from this document.

The remaining original task requirements include provider/freshness/protocol qualification, Solana rollback support, whole dependency acceptance and hosted worker/settlement integration. This component does not close those tasks, change credentials, introduce Demo navigation or enable Real trading.
