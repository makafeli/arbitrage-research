#!/usr/bin/python3 -I
"""Launch the existing anchored Base session. Never implicitly initialize or START."""
from __future__ import annotations

import fcntl
import json
import os
from pathlib import Path
import re
import resource
import signal
import ssl
import stat
import subprocess
import sys
import tempfile
import urllib.parse
import uuid

ROOT = Path('/data/runtime/base-v1')
STREAM = 'railway-base-profile-v1'
MAX_OUTPUT = 65536
MAX_PROFILE = 1048576
ACTIONS = {'--start', '--initialize-and-start'}
GENERATION_PATTERN = re.compile(r'[1-9][0-9]?')


class LaunchError(Exception):
    """Only fixed reason codes are exposed; never child diagnostics or secrets."""


def require(ok: bool, code: str) -> None:
    if not ok:
        raise LaunchError(code)


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'DUPLICATE_PROFILE_KEY')
        result[key] = value
    return result


def read_json(path: Path):
    # Validate the opened descriptor, not a separate lstat/read path pair.
    # NONBLOCK prevents a replaced FIFO from hanging; the read stays bounded even
    # if a regular file grows after fstat. Parent directories remain trusted.
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except OSError as error:
        raise LaunchError('PROFILE_FILE_REJECTED') from error
    try:
        info = os.fstat(descriptor)
        require(stat.S_ISREG(info.st_mode) and info.st_size <= MAX_PROFILE,
                'PROFILE_FILE_REJECTED')
        with os.fdopen(descriptor, 'rb', closefd=False) as source:
            data = source.read(MAX_PROFILE + 1)
        require(len(data) <= MAX_PROFILE, 'PROFILE_FILE_REJECTED')
    finally:
        os.close(descriptor)
    return json.loads(data, object_pairs_hook=pairs)


def child_limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_OUTPUT, MAX_OUTPUT))


def call(argv: list[str], env: dict[str, str]) -> tuple[int, bytes, bytes]:
    # The child sees only selected service settings. No shell or inherited proxies.
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as error:
        with subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=output, stderr=error,
                              env=env, preexec_fn=child_limits) as process:
            try:
                process.wait(timeout=80)
            except BaseException:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                raise
            output.seek(0)
            error.seek(0)
            out, err = output.read(MAX_OUTPUT + 1), error.read(MAX_OUTPUT + 1)
            require(len(out) <= MAX_OUTPUT and len(err) <= MAX_OUTPUT, 'LAUNCH_RESPONSE_LIMIT')
            return process.returncode, out, err


def generation(source: dict[str, str]) -> int:
    # Absent or '1' is today's single stream, byte-identical below. A generation
    # succeeds a HALTED source (#193); it never touches or resets the old one.
    raw = source.get('ARB_BASE_GENERATION')
    if raw is None:
        return 1
    require(GENERATION_PATTERN.fullmatch(raw) is not None, 'GENERATION_REJECTED')
    return int(raw)


def stream_id(value: int) -> str:
    return STREAM if value == 1 else f'{STREAM}.g{value}'


def environment(source: dict[str, str]) -> dict[str, str]:
    value = generation(source)
    stream = stream_id(value)
    operator = source.get('ARB_OPERATOR_ID', '')
    require(re.fullmatch(r'[A-Za-z0-9_.:-]{1,128}', operator) is not None, 'OPERATOR_REQUIRED')
    anchor = source.get('ARB_BASE_PROFILE_DIGEST', '')
    require(re.fullmatch(r'sha256:[0-9a-f]{64}', anchor) is not None, 'PROFILE_ANCHOR_REQUIRED')
    endpoint = source.get('ARB_BASE_RPC_URL', '')
    parsed = urllib.parse.urlsplit(endpoint)
    require(parsed.scheme == 'https' and bool(parsed.hostname) and not parsed.fragment
            and parsed.username is None and not any(c.isspace() for c in endpoint), 'BASE_ENDPOINT_REQUIRED')
    database = urllib.parse.urlsplit(source.get('ARB_DATABASE_URL', ''))
    require(database.scheme in {'postgres', 'postgresql'} and bool(database.hostname)
            and bool(database.username) and len(database.path) > 1 and not database.fragment,
            'DATABASE_SETTING_REJECTED')
    query = urllib.parse.parse_qs(database.query, keep_blank_values=True, strict_parsing=True)
    require(set(query) <= {'sslmode'} and (not query or query['sslmode'] == ['verify-full']),
            'DATABASE_TLS_SETTING_REJECTED')
    # Trust comes from the authenticated deployment configuration, never TOFU or
    # an unauthenticated connection to the database. SQLx accepts PEM in this env.
    ca = source.get('ARB_DATABASE_CA_PEM', '')
    require(len(ca) <= 16384 and re.fullmatch(
        r'-----BEGIN CERTIFICATE-----\n[A-Za-z0-9+/=\r\n]+-----END CERTIFICATE-----[ \t\r\n]*', ca
    ) is not None, 'DATABASE_CA_REQUIRED')
    try:
        ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT).load_verify_locations(cadata=ca)
    except (ssl.SSLError, ValueError) as error:
        raise LaunchError('DATABASE_CA_REJECTED') from error
    # Both Rust processes must authenticate the chain AND the unchanged hostname.
    database = urllib.parse.urlunsplit(database._replace(query='sslmode=verify-full'))
    pace = source.get('ARB_RPC_MIN_INTERVAL_MS', '')
    require(pace.isascii() and pace.isdigit() and str(int(pace)) == pace
            and 75 <= int(pace) <= 1000, 'RPC_PACING_REQUIRED')
    result = {'PATH': '/usr/local/bin:/usr/bin:/bin', 'HOME': '/home/arb', 'LC_ALL': 'C',
              'ARB_OPERATOR_ID': operator, 'ARB_INGEST_OPERATOR_ID': operator,
              'ARB_DATABASE_URL': database, 'ARB_INGEST_DATABASE_URL': database,
              'PGSSLROOTCERT': ca,
              'ARB_BASE_PROFILE_DIGEST': anchor, 'ARB_BASE_RPC_URL': endpoint,
              'ARB_RPC_MIN_INTERVAL_MS': pace, 'ARB_INGEST_STREAM_ID': stream,
              'ARB_BASE_INGESTION_STREAM': stream, 'ARB_BASE_MANAGED_INGESTION': 'true'}
    # Absent/'1' stays byte-identical: no ARB_BASE_GENERATION key at all.
    if value != 1:
        result['ARB_BASE_GENERATION'] = str(value)
    return result


def prepare(source: dict[str, str], action: str, root: Path = ROOT, runner=call) -> dict[str, str]:
    require(action in ACTIONS, 'LAUNCH_ARGUMENTS_REJECTED')
    require(stat.S_ISDIR(root.lstat().st_mode), 'PROFILE_DIRECTORY_REJECTED')
    env = environment(source)
    # Generation 1's session was registered before this launcher existed, so it
    # never registers here (byte-identical). A generation >= 2 successor has no
    # prior registration: its one-time --initialize-and-start deploy must
    # register the new session first (refused unless every existing
    # base-mainnet session is STOPPED/FAULTED; idempotent on retry) before any
    # source mutation. --start never registers, for any generation.
    if action == '--initialize-and-start' and 'ARB_BASE_GENERATION' in env:
        code, _, _ = runner(['/usr/local/bin/worker-session', '--register', str(root)], env)
        require(code == 0, 'GENERATION_SESSION_REGISTRATION_REFUSED')
    # The Rust session checker validates configuration+registry against the external
    # digest and reads the registration. Registration happens only in the
    # generation >= 2 branch above; --status never registers.
    code, out, _ = runner(['/usr/local/bin/worker-session', '--status', str(root)], env)
    require(code == 0, 'REGISTERED_SESSION_REQUIRED')
    value = json.loads(out, object_pairs_hook=pairs)
    session = value.get('session', {})
    require(value.get('status') == 'BASE_SESSION_REGISTERED'
            and value.get('registration_requested') is False
            and session.get('configuration_digest') == env['ARB_BASE_PROFILE_DIGEST']
            and session.get('network_id') == 'base-mainnet' and session.get('mode') == 'OBSERVE'
            and session.get('execution_authorized') is False, 'SESSION_PROFILE_MISMATCH')
    sid = session.get('session_id', '')
    require(isinstance(sid, str) and str(uuid.UUID(sid)) == sid, 'SESSION_ID_REJECTED')
    registry, ingestion = read_json(root/'registry.json'), read_json(root/'ingestion-registry.json')
    require(isinstance(registry, dict) and isinstance(ingestion, list)
            and len(ingestion) == 2 and registry.get('pools') == ingestion, 'INGESTION_REGISTRY_MISMATCH')
    env.update(ARB_SESSION_ID=sid, ARB_WORKER_CONFIG=str(root/'configuration.toml'),
               ARB_POOL_REGISTRY=str(root/'registry.json'),
               ARB_INGEST_REGISTRY_FILE=str(root/'ingestion-registry.json'))
    if action == '--initialize-and-start':
        # Existing base-ingest checks absence BEFORE any RPC and never resets a source.
        code, _, _ = runner(['/usr/local/bin/base-ingest', '--initialize'], env)
        require(code == 0, 'SOURCE_INITIALIZATION_REFUSED_OR_UNCERTAIN')
    code, out, _ = runner(['/usr/local/bin/base-ingest', '--status'], env)
    require(code == 0, 'EXISTING_SOURCE_REQUIRED')
    cursor = json.loads(out, object_pairs_hook=pairs)
    require(cursor.get('status') == 'STATUS' and cursor.get('state') == 'ACTIVE'
            and cursor.get('dataset_origin') == 'RECORDED_LIVE'
            and cursor.get('execution_authorized') is False, 'ACTIVE_RECORDED_SOURCE_REQUIRED')
    # The research worker rechecks the exact typed binding before claiming the
    # session and before each collection. No status here grants quote readiness.
    return env


def cancelled(_signum, _frame):
    raise LaunchError('LAUNCH_CANCELLED')


def main(args: list[str] | None = None) -> int:
    args = sys.argv[1:] if args is None else args
    if not args or args == ['--check']:
        print(json.dumps({'status': 'NOT_STARTED', 'provider_requests': 0,
                          'database_requests': 0, 'execution_authorized': False}))
        return 0
    lock = None
    try:
        require(len(args) == 1 and args[0] in ACTIONS, 'LAUNCH_ARGUMENTS_REJECTED')
        require(os.getuid() == 10001 and os.getgid() == 10001, 'UNPRIVILEGED_WORKER_REQUIRED')
        parent = ROOT.parent.lstat()
        require(stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.getuid()
                and stat.S_IMODE(parent.st_mode) == 0o700, 'PRIVATE_RUNTIME_REQUIRED')
        lock = os.open(ROOT.parent/'.base-launch.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        info = os.fstat(lock)
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
                and stat.S_IMODE(info.st_mode) == 0o600, 'LAUNCH_LOCK_REJECTED')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        signal.signal(signal.SIGINT, cancelled)
        signal.signal(signal.SIGTERM, cancelled)
        env = prepare(dict(os.environ), args[0])
        os.set_inheritable(lock, True)
        print(json.dumps({'status': 'BASE_WORKER_EXECUTING', 'session_id': env['ARB_SESSION_ID'],
                          'source_id': env['ARB_BASE_INGESTION_STREAM'], 'start_command_issued': False,
                          'execution_authorized': False}), flush=True)
        # Replace PID1; existing worker now owns signals, recovery and commands.
        os.execve('/usr/local/bin/research-worker', ['research-worker'], env)
    except LaunchError as error:
        reason = str(error)
    except (OSError, ValueError, TypeError, AttributeError, RecursionError, subprocess.SubprocessError):
        reason = 'BASE_LAUNCH_FAILED'
    finally:
        if lock is not None:
            os.close(lock)
    print(json.dumps({'status': 'BASE_LAUNCH_BLOCKED', 'reason': reason,
                      'execution_authorized': False}), file=sys.stderr)
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
