#!/usr/bin/env python3
"""Mandatory disposable PostgreSQL drill. Missing CI service is failure, never a skipped pass."""
import json
import os
from pathlib import Path
import sys
import tempfile
import uuid

import recovery as r

ROOT = Path(__file__).resolve().parents[1]
LIMITS = r.Limits(bytes=256 * 1024**2, seconds=120)


def execute(database, statement=None, file=None):
    args = ['psql', '-X', '-qAt', '-w', '-v', 'ON_ERROR_STOP=1', '--dbname', database,
            '-c', 'SET search_path=public,pg_catalog;']
    args += ['-f', str(file)] if file else ['-c', statement]
    return r.command(args, LIMITS, capture=True)


def refusal(call):
    try:
        call()
    except (r.RecoveryError, OSError):
        return
    raise AssertionError('Expected refusal was not enforced')


def main():
    # This script only creates disposable databases in the named CI service.
    # It contains no DROP DATABASE and is not an operator production drill.
    r.require(os.environ.get('ARB_RECOVERY_INTEGRATION') == 'disposable-ci-service'
              and os.environ.get('GITHUB_ACTIONS') == 'true'
              and os.environ.get('PGHOST') == 'postgres', 'Disposable CI service acknowledgement required')
    suffix = uuid.uuid4().hex[:12]
    source, target = 'arb_recovery_source_' + suffix, 'arb_restore_ci_' + suffix
    execute('postgres', 'CREATE DATABASE ' + source + ' TEMPLATE template0')
    execute('postgres', 'CREATE DATABASE ' + target + ' TEMPLATE template0')
    migrations = sorted((ROOT / 'migrations').glob('*.sql'))
    r.require(len(migrations) >= 4, 'Real project migration set missing')
    for migration in migrations:
        execute(source, file=migration)
    # Storage fixtures deliberately do not claim valid runtime configuration,
    # real market origin, complete paper-event DTOs or replayable market captures.
    execute(source, """
INSERT INTO configuration_snapshots(operator_id,configuration_digest,snapshot)
VALUES ('fixture-operator','sha256:storage-fixture','{"origin":"SYNTHETIC_STORAGE_FIXTURE"}');
INSERT INTO research_sessions(session_id,operator_id,network_id,mode,configuration_digest,experiment_id,strategy_ids,observed_state,desired_revision,lifecycle)
VALUES ('fixture-session','fixture-operator','base-mainnet','PAPER','sha256:storage-fixture','fixture-experiment','[]','STOPPED',1,'{"storage_fixture":true}');
INSERT INTO control_commands(command_id,operator_id,session_id,idempotency_key,payload_digest,action,revision,status,outstanding_attempts)
VALUES ('fixture-command','fixture-operator','fixture-session','fixture-key','sha256:fixture','START',1,'PENDING',0);
INSERT INTO control_audit_events(session_id,operator_id,event_kind,command_id,detail)
VALUES ('fixture-session','fixture-operator','STORAGE_FIXTURE','fixture-command','{"amount":"999999999999999999999999","note":"line one\\nline two"}');
INSERT INTO research_attempts(attempt_id,session_id,generation,admitted_worker_epoch,status)
VALUES ('fixture-attempt','fixture-session',0,1,'OUTSTANDING');
INSERT INTO capture_admissions(session_id,capture_id,attempt_id,manifest_digest,artifact_path,generation)
VALUES ('fixture-session','fixture-capture','fixture-attempt','sha256:fixture-manifest','fixture-capture',0);
INSERT INTO paper_runs(run_id,operator_id,session_id,configuration_digest,network_id,initial_balances,journal_bytes)
VALUES ('fixture-run','fixture-operator','fixture-session','sha256:storage-fixture','base-mainnet','[{"amount":"999999999999999999999999","origin":"SYNTHETIC_STORAGE_FIXTURE"}]',64);
INSERT INTO paper_journal(event_id,run_id,sequence,command_id,payload_digest,payload)
VALUES ('fixture-event','fixture-run',0,'fixture-journal-command','sha256:fixture-event','{"amount":"999999999999999999999999","origin":"SYNTHETIC_STORAGE_FIXTURE"}');
CREATE TABLE recovery_text_fixture (id bigint GENERATED ALWAYS AS IDENTITY, note text, exact_amount numeric(78,0));
INSERT INTO recovery_text_fixture(note,exact_amount) VALUES (E'quote " slash \\\\ newline\\nend',999999999999999999999999);
""")
    initial = r.db_evidence(source, LIMITS)
    r.require(len(initial['relations']) >= 12, 'Real migration relations not inventoried')
    r.require(any(row['kind'] == 'S' and row['rows'] == 1 for row in initial['relations']), 'Sequence evidence missing')
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve(); captures = root / 'captures'; captures.mkdir()
        (captures / 'fixture-capture').mkdir()
        raw = b'{"origin":"SYNTHETIC_STORAGE_FIXTURE","expires_at":"2000-01-01","not_a_market_capture":true}\n'
        (captures / 'fixture-capture/manifest.json').write_bytes(raw)
        (captures / 'fixture-capture/state.bin').write_bytes(bytes(range(256)) * 1024)
        bundle = root / 'backup'; restored = root / 'restored'
        source_commit = os.environ.get('GITHUB_SHA', '')
        backup = r.backup(source, captures, bundle, source_commit, True, LIMITS)
        sha = backup['manifest_sha256']; r.verify(bundle, sha, LIMITS)
        # Tamper detection must leave the target empty and no destination created.
        artifact = bundle / 'captures/fixture-capture/state.bin'; original = artifact.read_bytes()
        artifact.write_bytes(b'corrupt')
        refusal(lambda: r.restore(bundle, sha, target, target, restored, True, LIMITS))
        r.require(r.db_evidence(target, LIMITS) == {'relations': []} and not restored.exists(), 'Failed verification mutated target')
        artifact.write_bytes(original)
        result = r.restore(bundle, sha, target, target, restored, True, LIMITS)
        r.require(r.db_evidence(source, LIMITS) == initial == r.db_evidence(target, LIMITS), 'Source/restore contents or sequence states differ')
        r.require((restored / 'fixture-capture/manifest.json').read_bytes() == raw, 'Capture expiry/content changed')
        r.require(execute(target, "SELECT status FROM control_commands WHERE command_id='fixture-command'").strip() == b'PENDING', 'Pending command was applied or lost')
        r.require(execute(target, "SELECT payload->>'amount' FROM paper_journal WHERE event_id='fixture-event'").strip() == b'999999999999999999999999', 'Ledger precision lost')
        r.require(execute(target, "SELECT observed_state FROM research_sessions WHERE session_id='fixture-session'").strip() == b'STOPPED', 'Restore altered persisted worker state')
        # Restored triggers and uniqueness still protect audit and idempotency.
        refusal(lambda: execute(target, "UPDATE control_audit_events SET detail='{}'"))
        refusal(lambda: execute(target, "INSERT INTO control_commands SELECT * FROM control_commands"))
        r.require(r.db_evidence(target, LIMITS) == initial, 'Constraint refusal changed restored history')
        refusal(lambda: r.restore(bundle, sha, target, target, root / 'second', True, LIMITS))
        refusal(lambda: r.restore(bundle, sha, 'production', 'production', root / 'unsafe', True, LIMITS))
        r.require(not (root / 'second').exists() and not (root / 'unsafe').exists(), 'Rejected restore created output')
        # The actual command-line entrypoint also validates this same bundle.
        cli = json.loads(r.command([sys.executable, str(ROOT / 'scripts/recovery.py'), 'verify', '--bundle', str(bundle), '--manifest-sha256', sha], LIMITS, capture=True))
        r.require(cli['status'] == 'BUNDLE_BYTES_VERIFIED', 'CLI verification failed')
        print(json.dumps({**result, 'migration_files': len(migrations), 'pending_command': 'PRESERVED',
                          'exact_ledger_amount': 'PRESERVED', 'audit_and_uniqueness': 'ENFORCED',
                          'source_unchanged': True, 'capture_expiry_unchanged': True, 'evidence_origin': 'SYNTHETIC_STORAGE_FIXTURE'}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
