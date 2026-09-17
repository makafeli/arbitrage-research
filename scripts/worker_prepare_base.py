#!/usr/bin/env python3
"""Explicit Base identity observation and immutable runtime profile preparation."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import tomllib

# With python -I, import only the trusted scripts shipped in the same image.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import collect_operator_base_evidence as access
import recorded_base_slice as profile

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = Path('/data/runtime')
CHECKER = '/usr/local/bin/worker-profile-check'
FILES = ('configuration.toml', 'registry.json', 'ingestion-registry.json')
MAX_FILE = 1024 * 1024


class ProfileError(ValueError):
    """Only fixed public reason codes may cross the CLI boundary."""


def require(ok: bool, reason: str) -> None:
    if not ok:
        raise ProfileError(reason)


def sha(data: bytes) -> str:
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def read_file(path: Path) -> bytes:
    require(path.is_file() and not path.is_symlink(), 'PROFILE_FILE_REJECTED')
    with path.open('rb') as stream:
        value = stream.read(MAX_FILE + 1)
    require(len(value) <= MAX_FILE, 'PROFILE_FILE_REJECTED')
    return value


def write_file(path: Path, value: bytes) -> None:
    require(len(value) <= MAX_FILE, 'PROFILE_FILE_REJECTED')
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(value)
        stream.flush()
        os.fsync(stream.fileno())


def render(report: dict, inventory: dict) -> dict[str, bytes]:
    try:
        registry = profile.runtime_registry(report, inventory)
        raw = json.dumps(registry, separators=(',', ':'), allow_nan=False).encode()
        text = profile.config_text(raw, Path('/data/captures'))
        old = 'database_secret_reference = "env:TEST_DATABASE_URL"'
        require(text.count(old) == 1, 'PROFILE_TEMPLATE_CHANGED')
        text = text.replace(old, 'database_secret_reference = "env:ARB_DATABASE_URL"')
        lines = text.splitlines()
        lines[:2] = ['# Base OBSERVE candidate research; generated from a bounded identity observation.',
                     '# No transaction simulation, virtual settlement or live execution is enabled.']
        text = '\n'.join(lines) + '\n'
        parsed = tomllib.loads(text)
        require(parsed['deployment']['mode'] == 'OBSERVE'
                and parsed['storage']['capture_quota_bytes'] == 134217728
                and not parsed['networks']['solana']['enabled']
                and not any(parsed['execution'][k] for k in
                            ('enabled', 'signer_enabled', 'broadcast_enabled', 'flash_loans_enabled')),
                'PROFILE_SCOPE_REJECTED')
        return {'registry.json': raw, 'configuration.toml': text.encode(),
                'ingestion-registry.json': json.dumps(registry['pools'], separators=(',', ':'),
                                                     allow_nan=False).encode()}
    except (KeyError, TypeError, profile.SliceError):
        raise ProfileError('BASE_IDENTITY_VALIDATION_FAILED') from None


def check_profile(directory: Path) -> dict:
    result = subprocess.run([CHECKER, '--check', str(directory/'configuration.toml'),
                             str(directory/'registry.json')],
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, timeout=10, check=False,
                            env={'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent', 'LC_ALL': 'C'})
    require(result.returncode == 0 and len(result.stdout) <= 8192, 'RUST_PROFILE_CHECK_FAILED')
    value = json.loads(result.stdout)
    require(isinstance(value, dict) and value.get('status') == 'PROFILE_VALIDATED'
            and value.get('mode') == 'OBSERVE' and value.get('network_id') == 'base-mainnet'
            and value.get('execution_authorized') is False
            and value.get('registry_digest') == sha(read_file(directory/'registry.json'))
            and isinstance(value.get('configuration_digest'), str)
            and re.fullmatch(r'sha256:[0-9a-f]{64}', value['configuration_digest']) is not None,
            'RUST_PROFILE_CHECK_FAILED')
    return value


def sync_directory(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def prepare(root: Path, endpoint: str, collector=access.collect, checker=check_profile) -> dict:
    info = root.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
            and stat.S_IMODE(info.st_mode) == 0o700, 'PRIVATE_RUNTIME_DIRECTORY_REQUIRED')
    lock = os.open(root/'.base-profile.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock, 'rb') as held:
        require(stat.S_ISREG(os.fstat(held.fileno()).st_mode), 'PROFILE_LOCK_REJECTED')
        try:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ProfileError('PROFILE_PREPARATION_BUSY') from None
        final = root/'base-v1'
        if os.path.lexists(final):
            require(final.is_dir() and not final.is_symlink(), 'EXISTING_PROFILE_REJECTED')
            prior = json.loads(read_file(final/'profile.json'))
            require(isinstance(prior, dict) and prior.get('status') == 'BASE_PROFILE_PREPARED'
                    and isinstance(prior.get('file_digests'), dict), 'EXISTING_PROFILE_REJECTED')
            require(set(prior['file_digests']) == set(FILES), 'EXISTING_PROFILE_REJECTED')
            for name in FILES:
                require(prior['file_digests'][name] == sha(read_file(final/name)),
                        'EXISTING_PROFILE_CHANGED')
            checked = checker(final)
            require(checked['configuration_digest'] == prior.get('configuration_digest'),
                    'EXISTING_PROFILE_CHANGED')
            return {'status': 'BASE_PROFILE_REUSED', 'profile_directory': str(final),
                    'configuration_digest': checked['configuration_digest'],
                    'provider_requests': 0, 'current_rpc_verified': False,
                    'worker_started': False, 'execution_authorized': False}
        attempt = Path(tempfile.mkdtemp(prefix='.base-attempt-', dir=root))
        try:
            report = collector(endpoint)
            inventory = json.loads(read_file(ROOT/'docs/registries/initial-identities.json'))
            files = render(report, inventory)
            for name, value in files.items():
                write_file(attempt/name, value)
            checked = checker(attempt)
            requests = report.get('requests')
            require(isinstance(requests, list) and 0 < len(requests) <= 48,
                    'OBSERVATION_REQUEST_COUNT_REJECTED')
            result = {'status': 'BASE_PROFILE_PREPARED', 'profile_directory': str(final),
                      'network_id': 'base-mainnet', 'mode': 'OBSERVE', 'pool_count': 2,
                      'configuration_digest': checked['configuration_digest'],
                      'file_digests': {name: sha(value) for name, value in files.items()},
                      'provider_requests': len(requests), 'current_rpc_verified': True,
                      'worker_started': False, 'execution_authorized': False,
                      'production_qualified': False, 'database_requests': 0}
            write_file(attempt/'profile.json', json.dumps(result, sort_keys=True).encode())
            sync_directory(attempt)
            os.rename(attempt, final)
            sync_directory(root)
            # Only the constructed, secret-free profile is exported for matching API registration.
            return {**result, 'configuration_toml': files['configuration.toml'].decode(),
                    'registry': json.loads(files['registry.json'])}
        except Exception:
            # Preserve failed preparation evidence; never overwrite a complete profile or retry RPC.
            write_file(attempt/'failure.json', b'{"status":"BASE_PROFILE_FAILED","automatic_retry":false}')
            raise


def alarm(_signal, _frame):
    raise ProfileError('PROFILE_WALL_LIMIT')


def main(args: list[str] | None = None) -> int:
    args = sys.argv[1:] if args is None else args
    if not args:
        print(json.dumps({'status': 'NOT_STARTED', 'provider_requests': 0, 'execution_authorized': False}))
        return 0
    try:
        require(args == ['--prepare'], 'PROFILE_ARGUMENTS_REJECTED')
        require(Path(CHECKER).is_file() and os.access(CHECKER, os.X_OK), 'PROFILE_CHECKER_MISSING')
        endpoint = access.endpoint_from_environment()
        # Do not route credential-bearing RPC URLs through inherited proxy configuration.
        for key in list(os.environ):
            if key.lower() in ('http_proxy', 'https_proxy', 'all_proxy', 'no_proxy'):
                os.environ.pop(key)
        signal.signal(signal.SIGALRM, alarm)
        signal.alarm(300)
        try:
            result = prepare(RUNTIME, endpoint)
        finally:
            signal.alarm(0)
        print(json.dumps(result, sort_keys=True, allow_nan=False))
        return 0
    except ProfileError as error:
        reason = str(error)
    except access.AccessError:
        reason = 'AUTHORIZED_BASE_ENDPOINT_MISSING_OR_INVALID'
    except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError, RecursionError):
        reason = 'BASE_PROFILE_PREPARATION_FAILED'
    print(json.dumps({'status': 'BASE_PROFILE_BLOCKED', 'reason': reason,
                      'worker_started': False, 'execution_authorized': False}), file=sys.stderr)
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
