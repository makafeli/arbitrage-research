"""One-shot exact-source preparation; no provider, database, secret or ref operations."""
from pathlib import Path
import json
import os
import subprocess

BASE = 'bc8a2106b1ce412f61cda8c78f37581534da342f'
BRANCH = 'feat/ARB-018-producer-capture-source'
REPO = 'repos/makafeli/arbitrage-research'
ALLOWED = {
    'migrations/0009_session_ingestion_sources.sql',
    'crates/arb-storage/src/capture_source.rs',
    'crates/arb-control/src/capture_source.rs',
    'apps/research-worker/src/capture_source.rs',
    'crates/arb-storage/tests/support/capture_source_cases.rs',
    'apps/research-worker/tests/support/capture_source_process.rs',
    'crates/arb-storage/src/lib.rs', 'crates/arb-control/src/lib.rs',
    'apps/research-worker/src/main.rs', 'crates/arb-domain/src/decision.rs',
    'crates/arb-storage/src/decisions.rs', 'crates/arb-storage/tests/decisions.rs',
    'apps/research-worker/tests/controlled_capture.rs', 'docs/CAPTURE-CONTINUITY.md',
    'GOAL.md', 'planning/implementation-progress.json',
}

def command(*args):
    return subprocess.check_output(args, text=True, timeout=90).strip()

def api(path, data=None):
    args = ['gh', 'api', REPO + path]
    if data is None:
        return json.loads(command(*args))
    result = subprocess.run(args + ['--method', 'POST', '--input', '-'],
        input=json.dumps(data), text=True, capture_output=True, timeout=30, check=True)
    return json.loads(result.stdout)

def edit(path, old, new):
    file = Path(path)
    value = file.read_text()
    assert value.count(old) == 1, f'exact-source edit failed: {path}'
    file.write_text(value.replace(old, new))

assert os.environ['GITHUB_REPOSITORY'] == 'makafeli/arbitrage-research'
assert os.environ['GITHUB_ACTOR'] == 'makafeli'
assert os.environ['GITHUB_REF'] == 'refs/heads/' + BRANCH
assert command('git', 'rev-parse', 'HEAD') == os.environ['GITHUB_SHA']
assert command('git', 'rev-parse', 'HEAD^') == BASE
assert api('/git/ref/heads/main')['object']['sha'] == BASE
assert api('/git/ref/heads/' + BRANCH)['object']['sha'] == os.environ['GITHUB_SHA']

edit('crates/arb-storage/src/lib.rs', 'mod ingestion;\npub use ingestion::*;',
    'mod ingestion;\npub use ingestion::*;\nmod capture_source;\npub use capture_source::*;')
edit('crates/arb-control/src/lib.rs', 'use arb_storage::{Store, StoreError, WorkerClaim, WorkerUpdate};',
    'mod capture_source;\nuse arb_storage::{Store, StoreError, WorkerClaim, WorkerUpdate};')
edit('crates/arb-storage/src/capture_source.rs',
    'if lifecycle(&session)?.state() != arb_domain::State::Stopped {',
    'if session.try_get::<String, _>("observed_state")? != "STOPPED" || lifecycle(&session)?.allows_evaluation() {')
edit('crates/arb-domain/src/decision.rs', 'pub const DECISION_SCHEMA_VERSION:',
    '/// Maximum source set for a newly published worker decision; legacy readers stay compatible.\npub const MAX_PUBLICATION_CAPTURE_REFS: usize = 8;\npub const DECISION_SCHEMA_VERSION:')
edit('crates/arb-domain/src/decision.rs', 'impl DecisionTrace {', '''impl DecisionTrace {
    /// Validate newly admitted worker output. Historical schema validation and
    /// exact retries remain compatible; missing-data diagnostics may have no inputs.
    pub fn validate_for_publication(&self) -> Result<(), DecisionError> {
        if self.capture_refs.len() > MAX_PUBLICATION_CAPTURE_REFS {
            return Err(invalid("capture_refs", "new publication supports at most eight capture references"));
        }
        self.validate()
    }
''')
edit('crates/arb-storage/src/decisions.rs',
    'trace\n                .validate()\n                .map_err(|_| StoreError::InvalidInput("invalid sealed decision trace"))?;',
    'trace\n                .validate_for_publication()\n                .map_err(|_| StoreError::InvalidInput("invalid sealed decision trace"))?;')
edit('crates/arb-storage/src/decisions.rs',
    '.fetch_one(&mut **tx).await?;\n            result.push(decision_record(&row)?);',
    '.fetch_one(&mut **tx).await.map_err(crate::capture_source::continuity_error)?;\n            result.push(decision_record(&row)?);')
edit('apps/research-worker/src/main.rs', 'mod pipeline_metrics;', 'mod capture_source;\nmod pipeline_metrics;')
edit('apps/research-worker/src/main.rs', 'async fn run() -> Result<(), AnyError> {\n    let metrics_enabled',
    'async fn run() -> Result<(), AnyError> {\n    let ingestion_stream = capture_source::setting()?;\n    let metrics_enabled')
edit('apps/research-worker/src/main.rs', '    let worker = ControlWorker::claim(',
    '    let bound_source = capture_source::configured(&plan, ingestion_stream.as_deref())?;\n    let worker = ControlWorker::claim(')
edit('apps/research-worker/src/main.rs', '    worker.complete_recovery().await?;',
    '    worker.complete_recovery().await?;\n    worker.configure_capture_ingestion_source(bound_source.as_ref()).await?;')
edit('apps/research-worker/src/main.rs',
    '"stale or fenced capture"\n                | "capture generation is fenced"',
    '"stale or fenced capture"\n                | "stale or fenced capture binding"\n                | "capture generation is fenced"')
edit('apps/research-worker/src/main.rs',
    '                            scheduler.set_gate(network, worker.generation().await.ok())?;',
    '''                            if all_admitted {
                                if let Some(source) = &bound_source {
                                    let work = generation.expect("admitted capture has generation");
                                    match capture_source::bind_batch(&worker, work, &plan, source, &batch).await {
                                        Ok(()) => {},
                                        Err(error) if is_generation_fence(&error) => all_admitted = false,
                                        Err(error) => {
                                            finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::InputValidationFailed)).await?;
                                            return Err(error.into());
                                        }
                                    }
                                }
                            }
                            scheduler.set_gate(network, worker.generation().await.ok())?;''')
for path, module in [
    ('crates/arb-storage/tests/decisions.rs', 'capture_source_cases'),
    ('apps/research-worker/tests/controlled_capture.rs', 'capture_source_process'),
]:
    file = Path(path)
    assert module not in file.read_text()
    with file.open('a') as handle:
        handle.write(f'\n#[path = "support/{module}.rs"]\nmod {module};\n')
edit('docs/CAPTURE-CONTINUITY.md',
    'standalone capture worker does not yet create these associations automatically.',
    'research worker creates these associations automatically when an explicit source is configured, as described below.')
edit('docs/CAPTURE-CONTINUITY.md',
    'Once a session contains an association, the existing `decision_traces` insertion',
    'Once a session declares a required source or contains an association, the existing `decision_traces` insertion')
edit('docs/CAPTURE-CONTINUITY.md',
    'after a halt. Sessions without associations keep their old recording behavior,',
    'after a halt. Sessions without a required source or associations keep their old recording behavior,')
with Path('docs/CAPTURE-CONTINUITY.md').open('a') as handle:
    handle.write('''
## Explicit research-worker producer integration

Set `ARB_BASE_INGESTION_STREAM` to an already initialized Base stream owned by the
same operator. This is an internal identifier, not a URL, secret or trading switch.
The application does not create a stream, issue extra RPC requests or deploy a
worker merely because this code is present. The configured pool order, complete
typed registry digest, ABI source revision and recorded/fixture origin must match
the ingestion binding. The digest uses the same serialized `Vec<PoolRegistry>` as
`base-ingest`, not the distinct outer runtime registry-document hash.

After recovery to STOPPED and before the first provider request, the worker persists
an immutable per-session source requirement. A later restart cannot omit it, change
the stream or substitute another registry/origin. New quotes are guarded even before
the first capture association. Existing untracked decision history cannot be promoted
by turning the requirement on retrospectively; create a new research session instead.

Each admitted complete capture batch must have one exact finalized block number,
hash, parent hash and timestamp, plus the authorized per-pool registry and capture
provenance. The adapter validates the original responses, and `write_bundle` commits
the corresponding manifest/object digests before admission. All dependencies are
then registered atomically under the same worker generation and source locks.
The source must already retain the exact matching batch checkpoint: neither the
nearest height nor a similarly timed or differently hashed block is substituted.
An ingestion lag therefore produces a typed refusal, not optimistic publication or
an unbounded catch-up attempt. The independent ingestion process still needs its
own scheduling and resource budget.

After a source HALT, new publication and current re-consumption fail while historical
payloads remain readable and their separate validity becomes INVALIDATED. The worker
retains a typed collection failure when source binding fails. This does not provide
automatic recovery from terminal HALT, source rotation, Solana rollback, live trading
or automatic virtual settlement.

New worker publication uses `DecisionTrace::validate_for_publication` with at most
eight references, matching the bounded acquisition contract. Existing schema-level
`validate`, sealing, reads and exact retries retain the legacy representation so
stored 9..64-reference non-route diagnostics do not become corrupt after deployment.
Zero-input missing-data diagnostics remain valid. The number of decisions per batch
is still independently bounded at 64; it is not the number of captures per decision.

Tests use actual PostgreSQL and actual research-worker processes with the existing
synthetic HTTP provider. They cover automatic matching, same-height/wrong-hash refusal,
post-HALT exclusion, sticky source configuration, atomic rejected batches and the
publication-reference boundary. CI run/commit results are recorded on the PR and #32;
source code or this list alone is not a passed test or provider qualification.
''')
edit('GOAL.md', '## Current delivery checkpoint\n', '''## Current delivery checkpoint

### Producer/source integration, 17 September 2026

Continue existing #32/#30 from main `bc8a2106`. The research worker now has an explicit,
per-session persistent Base ingestion source requirement and automatic exact-context
capture associations. New publication is bounded independently from legacy history
validation. See [capture continuity](docs/CAPTURE-CONTINUITY.md). Actual test, review,
merge and deployment state must be read from the current PR and native issue. No new
task, owner account change, provider purchase, worker activation or live trade is implied.
The older checkpoints below retain their historical source-specific claims.
''')
file = Path('planning/implementation-progress.json')
progress = json.loads(file.read_text())
progress['updated_on'] = '2026-09-17'
for ticket in progress['tickets']:
    if ticket['id'] in ('ARB-016', 'ARB-018'):
        ticket['evidence'].append('Explicit producer/source integration: docs/CAPTURE-CONTINUITY.md; source configuration is persisted before acquisition, exact finalized registry/context matching registers capture dependencies atomically, and new publication uses the eight-reference boundary without changing historical readers. Actual CI/review results and remaining original gates are recorded in the current PR and #32.')
file.write_text(json.dumps(progress, indent=2, ensure_ascii=False) + '\n')
Path('.github/workflows/prepare-capture-source.yml').unlink()
Path('scripts/prepare_capture_source.py').unlink()
subprocess.run(['cargo', 'fmt', '--all'], check=True, timeout=90)
subprocess.run(['git', 'diff', '--check', BASE], check=True, timeout=30)
changed = set(command('git', 'diff', '--name-only', BASE).splitlines())
assert changed == ALLOWED, f'unexpected source paths: {changed ^ ALLOWED}'
entries = []
for path in sorted(ALLOWED):
    content = Path(path).read_text()
    assert len(content.encode()) <= 2_000_000
    blob = api('/git/blobs', {'content': content, 'encoding': 'utf-8'})
    entries.append({'path': path, 'mode': '100644', 'type': 'blob', 'sha': blob['sha']})
assert api('/git/ref/heads/main')['object']['sha'] == BASE
assert api('/git/ref/heads/' + BRANCH)['object']['sha'] == os.environ['GITHUB_SHA']
base_tree = api('/git/commits/' + BASE)['tree']['sha']
tree = api('/git/trees', {'base_tree': base_tree, 'tree': entries})['sha']
report = {'prepared_tree': tree, 'preparation_source': os.environ['GITHUB_SHA'],
    'base_commit': BASE, 'paths': sorted(ALLOWED), 'tests_run': False,
    'refs_changed_by_workflow': False, 'preparation_files_in_result': False}
Path(os.environ['GITHUB_STEP_SUMMARY']).write_text(json.dumps(report, indent=2) + '\n')
print('PREPARED_SOURCE_RESULT=' + json.dumps(report))
