#!/usr/bin/python3 -I
"""One-shot scoped metadata inspection. Never initializes state or starts a worker."""
from __future__ import annotations

import json
import os
import re
import resource
import subprocess
import sys
import tempfile
import urllib.parse

MAX_OUTPUT = 128 * 1024
REQUIRED_SETTINGS = (
    'ARB_BASE_RPC_URL', 'ARB_WORKER_CONFIG', 'ARB_POOL_REGISTRY',
    'ARB_SESSION_ID', 'ARB_BASE_INGESTION_STREAM', 'ARB_BASE_MANAGED_INGESTION',
)
# Fixed SELECT only, with a transaction-level read-only fence and independent limits.
# No account tables, stored configuration bodies, free-text reasons or endpoints.
SQL = r"""
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SELECT json_build_object(
 'schema_version', 1,
 'read_only', current_setting('transaction_read_only') = 'on',
 'configuration_count', (SELECT count(*)::text FROM public.configuration_snapshots WHERE operator_id=:'operator'),
 'enabled_base_profile_count', (SELECT count(*)::text FROM public.configuration_snapshots
  WHERE operator_id=:'operator' AND snapshot#>>'{networks,base,enabled}'='true'),
 'session_count', (SELECT count(*)::text FROM public.research_sessions
  WHERE operator_id=:'operator' AND network_id='base-mainnet'),
 'stream_count', (SELECT count(*)::text FROM public.ingestion_streams
  WHERE operator_id=:'operator' AND binding->>'network_id'='base-mainnet'),
 'sessions', COALESCE((SELECT json_agg(s) FROM (
  SELECT session_id, mode, observed_state, configuration_digest,
   desired_revision::text, applied_revision::text,
   (lease_until IS NOT NULL AND lease_until > statement_timestamp()) AS lease_active
  FROM public.research_sessions WHERE operator_id=:'operator' AND network_id='base-mainnet'
  ORDER BY created_at DESC,session_id LIMIT 20
 ) s), '[]'::json),
 'streams', COALESCE((SELECT json_agg(s) FROM (
  SELECT stream_id, state, revision::text, checkpoint->>'number' AS checkpoint_number,
   binding->>'registry_digest' AS registry_digest
  FROM public.ingestion_streams WHERE operator_id=:'operator' AND binding->>'network_id'='base-mainnet'
  ORDER BY created_at DESC,stream_id LIMIT 20
 ) s), '[]'::json)
);
ROLLBACK;
"""


class ReadinessError(Exception):
    """Fixed diagnostic only; resolved environment values never enter the report."""


def require(condition: bool, reason: str = 'DATABASE_METADATA_REJECTED') -> None:
    if not condition:
        raise ReadinessError(reason)


def scope(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.:-]{1,128}', value) is not None


def count(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r'0|[1-9][0-9]{0,19}', value) is not None


def digest(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', value) is not None


def validate(value: object) -> dict:
    require(isinstance(value, dict) and set(value) == {
        'schema_version', 'read_only', 'configuration_count', 'enabled_base_profile_count',
        'session_count', 'stream_count', 'sessions', 'streams',
    })
    require(type(value['schema_version']) is int and value['schema_version'] == 1
            and value['read_only'] is True)
    for key in ('configuration_count', 'enabled_base_profile_count', 'session_count', 'stream_count'):
        require(count(value[key]))
    require(int(value['enabled_base_profile_count']) <= int(value['configuration_count']))
    for key, total in [('sessions', 'session_count'), ('streams', 'stream_count')]:
        require(isinstance(value[key], list) and len(value[key]) == min(20, int(value[total])))
    for row in value['sessions']:
        require(isinstance(row, dict) and set(row) == {
            'session_id', 'mode', 'observed_state', 'configuration_digest',
            'desired_revision', 'applied_revision', 'lease_active',
        })
        require(scope(row['session_id']) and digest(row['configuration_digest']))
        require(isinstance(row['mode'], str) and row['mode'] in {'OBSERVE', 'PAPER', 'REPLAY'})
        require(isinstance(row['observed_state'], str) and row['observed_state'] in {
            'RECOVERING', 'STOPPED', 'RUNNING', 'PAUSING', 'PAUSED', 'DRAINING', 'FAULTED',
        })
        require(count(row['desired_revision']) and count(row['applied_revision'])
                and int(row['applied_revision']) <= int(row['desired_revision'])
                and type(row['lease_active']) is bool)
    for row in value['streams']:
        require(isinstance(row, dict) and set(row) == {
            'stream_id', 'state', 'revision', 'checkpoint_number', 'registry_digest',
        })
        require(scope(row['stream_id']) and digest(row['registry_digest'])
                and isinstance(row['state'], str) and row['state'] in {'ACTIVE', 'HALTED'}
                and count(row['revision']) and count(row['checkpoint_number']))
    require(len({row['session_id'] for row in value['sessions']}) == len(value['sessions']))
    require(len({row['stream_id'] for row in value['streams']}) == len(value['streams']))
    return value


def child_limits() -> None:
    # Bound diagnostic output even if a corrupted database contains huge identifiers.
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_OUTPUT, MAX_OUTPUT))


def inspect(environment: dict[str, str], runner=subprocess.run) -> dict:
    operator, database = environment.get('ARB_OPERATOR_ID'), environment.get('ARB_DATABASE_URL', '')
    require(scope(operator), 'OPERATOR_SETTING_MISSING_OR_INVALID')
    try:
        parsed = urllib.parse.urlsplit(database)
        require(parsed.scheme in {'postgres', 'postgresql'} and bool(parsed.hostname)
                and bool(parsed.username) and parsed.path.startswith('/') and len(parsed.path) > 1
                and not parsed.fragment,
                'DATABASE_SETTING_MISSING_OR_INVALID')
        options = urllib.parse.parse_qs(parsed.query, strict_parsing=True, keep_blank_values=True)
        require(set(options).issubset({'sslmode'})
                and all(len(v) == 1 for v in options.values()), 'DATABASE_SETTING_MISSING_OR_INVALID')
        sslmode = options.get('sslmode', ['require'])[0]
        require(sslmode in {'require', 'verify-ca', 'verify-full'},
                'DATABASE_SETTING_MISSING_OR_INVALID')
        port = str(parsed.port or 5432)
        user = urllib.parse.unquote(parsed.username)
        password = urllib.parse.unquote(parsed.password or '')
        dbname = urllib.parse.unquote(parsed.path[1:])
        require(all('\x00' not in v and '\n' not in v and '\r' not in v
                    for v in (user, password, dbname)), 'DATABASE_SETTING_MISSING_OR_INVALID')
    except ValueError:
        raise ReadinessError('DATABASE_SETTING_MISSING_OR_INVALID') from None
    # No psqlrc, prompting, inherited libpq overrides or credentials in argv/logs.
    env = {'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent', 'LC_ALL': 'C',
           'PGDATABASE': dbname, 'PGHOST': parsed.hostname, 'PGPORT': port, 'PGUSER': user,
           'PGPASSWORD': password, 'PGSSLMODE': sslmode,
           'PGCONNECT_TIMEOUT': '5', 'PGPASSFILE': '/dev/null',
           'PGOPTIONS': '-c default_transaction_read_only=on -c statement_timeout=5000 -c lock_timeout=1000'}
    try:
        with tempfile.TemporaryFile() as output:
            result = runner(['/usr/bin/psql', '-X', '-w', '-qAt', '-v', 'ON_ERROR_STOP=1',
                             '-v', 'operator=' + operator], input=SQL.encode(),
                            stdout=output, stderr=subprocess.DEVNULL, env=env,
                            timeout=15, check=False, preexec_fn=child_limits)
            require(result.returncode == 0, 'DATABASE_READ_UNAVAILABLE')
            require(output.tell() <= MAX_OUTPUT, 'DATABASE_RESPONSE_LIMIT')
            output.seek(0)
            data = output.read(MAX_OUTPUT + 1)
        require(len(data) <= MAX_OUTPUT, 'DATABASE_RESPONSE_LIMIT')
        state = validate(json.loads(data))
    except (OSError, subprocess.SubprocessError, ValueError, RecursionError):
        raise ReadinessError('DATABASE_READ_UNAVAILABLE') from None
    missing = [name for name in REQUIRED_SETTINGS if not environment.get(name)]
    # Presence is not validation. Matching config/source, real reads and control
    # acceptance remain necessary even when all settings are supplied.
    return {'status': 'WORKER_READINESS_INSPECTED', 'database_read_only': True,
            'provider_requests': 0, 'worker_started': False, 'execution_authorized': False,
            'runtime_qualified': False, 'missing_settings': missing, 'state': state}


def main(args: list[str] | None = None) -> int:
    try:
        require(not (sys.argv[1:] if args is None else args), 'READINESS_ARGUMENTS_REJECTED')
        print(json.dumps(inspect(dict(os.environ)), sort_keys=True))
        return 0
    except ReadinessError as error:
        print(json.dumps({'status': 'WORKER_READINESS_UNAVAILABLE', 'reason': str(error),
                          'worker_started': False, 'execution_authorized': False}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
