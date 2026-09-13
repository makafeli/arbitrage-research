#!/usr/bin/env python3
"""Read-only, bounded local capture dependency audit; never replay or market evidence."""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import Any, Iterator

MAX_MANIFEST = 1024**2
MAX_BUNDLE = 64 * 1024**2
U64_MAX = 2**64 - 1
NETWORKS = {'base-mainnet', 'solana-mainnet'}
MANIFEST_FIELDS = {
    'schema_version', 'capture_id', 'origin', 'network', 'provider_alias',
    'adapter_version', 'adapter_source_commit', 'build_digest', 'config_digest',
    'created_at_ms', 'raw_expires_at_ms', 'context', 'first_sequence', 'last_sequence',
    'required_inputs', 'missing_inputs', 'coherent', 'complete_for_quote', 'objects',
}


class AuditError(ValueError):
    """A fixed public reason code, never a path, raw value or OS exception."""


def require(condition: bool, reason: str) -> None:
    """Reject untrusted input without disclosing its contents."""
    if not condition:
        raise AuditError(reason)


def label(value: Any) -> bool:
    """Use the flat, bounded ASCII identity accepted by arb-capture."""
    return isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,100}', value) is not None and value not in {'.', '..'}


def valid_digest(value: Any) -> bool:
    """Recognize the repository's lower-case SHA-256 identity."""
    return isinstance(value, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', value) is not None


def digest(data: bytes) -> str:
    """Hash exact bytes, without canonicalizing or modifying the source."""
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def uint(value: Any) -> bool:
    """Reject booleans, floats, negative numbers and values outside Rust u64."""
    return type(value) is int and 0 <= value <= U64_MAX


def decode(data: bytes) -> Any:
    """Reject duplicate keys, nonfinite numbers and malformed/deep JSON."""
    def unique(pairs: list) -> dict:
        result = {}
        for key, value in pairs:
            require(key not in result, 'DUPLICATE_JSON_KEY')
            result[key] = value
        return result

    def nonfinite(_value: str) -> None:
        raise AuditError('INVALID_JSON')

    depth = 0
    quoted = escaped = False
    for byte in data:
        if quoted:
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                quoted = False
        elif byte == 34:
            quoted = True
        elif byte in (91, 123):
            depth += 1
            require(depth <= 64, 'JSON_NESTING_LIMIT')
        elif byte in (93, 125):
            depth -= 1
    try:
        return json.loads(data.decode('utf-8'), object_pairs_hook=unique, parse_constant=nonfinite)
    except (UnicodeError, ValueError, RecursionError) as error:
        if isinstance(error, AuditError):
            raise
        raise AuditError('INVALID_JSON') from None


@dataclass
class Budget:
    """Global read budget; metadata and repeated dependencies consume it too."""
    maximum: int = 256 * 1024**2
    used: int = 0

    def __post_init__(self) -> None:
        require(type(self.maximum) is int and 0 < self.maximum <= 1024**4, 'INVALID_BYTE_BUDGET')

    def consume(self, count: int) -> None:
        """Refuse before reading beyond the caller's total budget."""
        require(self.used + count <= self.maximum, 'AUDIT_BYTE_LIMIT')
        self.used += count


def metadata(info: os.stat_result) -> tuple:
    """Detect identity, content metadata and link changes without trusting filenames."""
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_nlink)


@contextmanager
def directory(path: str | Path, parent_fd: int | None = None) -> Iterator[int]:
    """Retain a no-follow directory descriptor and detect replacement during use."""
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
    try:
        before = os.fstat(fd)
        yield fd
        after = os.fstat(fd)
        current = os.stat(path, dir_fd=parent_fd, follow_symlinks=False)
        require(metadata(before) == metadata(after) == metadata(current), 'DIRECTORY_CHANGED')
    finally:
        os.close(fd)


def read_file(parent_fd: int, name: str, maximum: int, budget: Budget, *, retain: bool = False) -> tuple[int, str, bytes]:
    """Hash a single-link regular file through its descriptor, with bounded memory."""
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent_fd)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1, 'UNSAFE_FILE')
        require(before.st_size <= maximum, 'FILE_SIZE_LIMIT')
        budget.consume(before.st_size)
        hasher = hashlib.sha256()
        chunks = []
        size = 0
        while size < before.st_size:
            chunk = source.read(min(65536, before.st_size - size))
            require(bool(chunk), 'FILE_CHANGED')
            size += len(chunk)
            hasher.update(chunk)
            if retain:
                chunks.append(chunk)
        current = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        require(metadata(before) == metadata(os.fstat(source.fileno())) == metadata(current), 'FILE_CHANGED')
    return size, 'sha256:' + hasher.hexdigest(), b''.join(chunks)


def validate_request(value: Any) -> dict:
    """Validate every requested identity before looking at the capture root."""
    require(isinstance(value, dict) and set(value) == {'schema_version', 'network', 'config_digest', 'captures'}, 'INVALID_REQUEST')
    require(type(value['schema_version']) is int and value['schema_version'] == 1, 'UNSUPPORTED_REQUEST_SCHEMA')
    require(isinstance(value['network'], str) and value['network'] in NETWORKS and valid_digest(value['config_digest']), 'INVALID_REQUEST_CONTEXT')
    refs = value['captures']
    require(isinstance(refs, list) and len(refs) <= 1000, 'REQUEST_COUNT_LIMIT')
    seen = set()
    for ref in refs:
        require(isinstance(ref, dict) and set(ref) == {'capture_id', 'manifest_digest'}, 'INVALID_REFERENCE')
        require(label(ref['capture_id']) and valid_digest(ref['manifest_digest']), 'INVALID_REFERENCE')
        key = (ref['capture_id'], ref['manifest_digest'])
        require(key not in seen, 'DUPLICATE_REFERENCE')
        seen.add(key)
    return value


def validate_manifest(value: Any) -> dict:
    """Check the current Rust v1 storage contract, not adapter/protocol semantics."""
    require(isinstance(value, dict), 'INVALID_MANIFEST')
    require(type(value.get('schema_version')) is int, 'INVALID_MANIFEST')
    require(value['schema_version'] == 1, 'UNSUPPORTED_CAPTURE_SCHEMA')
    require(set(value) == MANIFEST_FIELDS, 'INVALID_MANIFEST_FIELDS')
    require(all(label(value[key]) for key in ('capture_id', 'provider_alias', 'adapter_version')), 'INVALID_PROVENANCE')
    require(isinstance(value['adapter_source_commit'], str) and re.fullmatch(r'[0-9a-fA-F]{40}', value['adapter_source_commit']) is not None, 'INVALID_PROVENANCE')
    require(valid_digest(value['build_digest']) and valid_digest(value['config_digest']), 'INVALID_PROVENANCE')
    require(value['origin'] in ('synthetic', 'manually-constructed', 'recorded-live'), 'INVALID_ORIGIN')
    require(all(uint(value[key]) for key in ('created_at_ms', 'first_sequence', 'last_sequence')), 'INVALID_ORDERING')
    expires = value['raw_expires_at_ms']
    require(value['created_at_ms'] > 0 and value['first_sequence'] <= value['last_sequence'], 'INVALID_ORDERING')
    require(expires is None or (uint(expires) and expires > value['created_at_ms']), 'INVALID_RETENTION')
    context = value['context']
    require(isinstance(context, dict), 'INVALID_CONTEXT')
    if value['network'] == 'base-mainnet':
        require(set(context) == {'kind', 'block_number', 'block_hash', 'parent_hash', 'block_timestamp_seconds', 'finality'}, 'INVALID_CONTEXT')
        require(context['kind'] == 'Evm' and uint(context['block_number']) and uint(context['block_timestamp_seconds']), 'INVALID_CONTEXT')
        require(all(isinstance(context[key], str) and re.fullmatch(r'0x[0-9a-fA-F]{64}', context[key]) for key in ('block_hash', 'parent_hash')), 'INVALID_CONTEXT')
        require(context['finality'] in ('safe', 'finalized'), 'INVALID_CONTEXT')
    elif value['network'] == 'solana-mainnet':
        require(set(context) == {'kind', 'slot', 'genesis_hash', 'commitment', 'account_context'}, 'INVALID_CONTEXT')
        require(context['kind'] == 'Solana' and uint(context['slot']) and isinstance(context['genesis_hash'], str) and bool(context['genesis_hash']), 'INVALID_CONTEXT')
        require(context['commitment'] == 'finalized' and context['account_context'] == 'single-getMultipleAccounts-response', 'INVALID_CONTEXT')
    else:
        raise AuditError('INVALID_NETWORK')
    required, missing = value['required_inputs'], value['missing_inputs']
    require(isinstance(required, list) and isinstance(missing, list) and bool(required), 'INVALID_COMPLETENESS')
    require(all(isinstance(item, str) for item in required + missing), 'INVALID_COMPLETENESS')
    require(len(set(required)) == len(required) and len(set(missing)) == len(missing) and set(missing) <= set(required), 'INVALID_COMPLETENESS')
    require(type(value['coherent']) is bool and type(value['complete_for_quote']) is bool, 'INVALID_COMPLETENESS')
    require(not value['complete_for_quote'] or (value['coherent'] and not missing), 'INVALID_COMPLETENESS')
    objects = value['objects']
    require(isinstance(objects, list) and 1 <= len(objects) <= 4096, 'INVALID_OBJECT_COUNT')
    names = set()
    total = 0
    for obj in objects:
        require(isinstance(obj, dict) and set(obj) == {'name', 'sha256', 'bytes', 'content_type'}, 'INVALID_OBJECT')
        name = obj['name']
        require(label(name) and name not in {'manifest.json', 'INCOMPLETE'} and not name.endswith('.tmp') and name not in names, 'INVALID_OBJECT_NAME')
        require(valid_digest(obj['sha256']) and uint(obj['bytes']) and obj['bytes'] > 0 and obj['content_type'] == 'application/json', 'INVALID_OBJECT')
        names.add(name)
        total += obj['bytes']
    require(total <= MAX_BUNDLE, 'BUNDLE_SIZE_LIMIT')
    return value


def empty_row(ref: dict) -> dict:
    """Keep unknown evidence explicit until its digest and shape are verified."""
    return {**ref, 'raw_artifact_status': 'NOT_ASSESSED', 'reason': None,
            'expiration_status': 'UNKNOWN', 'capture_time_status': 'UNKNOWN', 'origin': 'UNKNOWN',
            'created_at_ms': None, 'raw_expires_at_ms': None,
            'quote_inputs_declared_complete': None, 'verified_objects': 0,
            'missing_objects': 0, 'corrupt_objects': 0,
            'replay_status': 'NOT_ASSESSED', 'market_performance_eligible': False}


def audit_one(root_fd: int, ref: dict, request: dict, now_ms: int, budget: Budget) -> dict:
    """Report one declared reference even if another reference is invalid or absent."""
    row = empty_row(ref)
    try:
        with directory(ref['capture_id'], root_fd) as fd:
            # Fixed-size directory inventory; never traverse unknown subdirectories.
            names = set()
            with os.scandir(fd) as entries:
                for entry in entries:
                    require(len(names) < 4098, 'DIRECTORY_ENTRY_LIMIT')
                    names.add(entry.name)
            if 'INCOMPLETE' in names:
                row.update(raw_artifact_status='INCOMPLETE', reason='INCOMPLETE_MARKER')
            elif 'manifest.json' not in names:
                row.update(raw_artifact_status='INCOMPLETE', reason='MANIFEST_MISSING')
            else:
                _, checksum, raw = read_file(fd, 'manifest.json', MAX_MANIFEST, budget, retain=True)
                require(checksum == ref['manifest_digest'], 'MANIFEST_DIGEST_MISMATCH')
                manifest = validate_manifest(decode(raw))
                require(manifest['capture_id'] == ref['capture_id'], 'CAPTURE_ID_MISMATCH')
                require(manifest['network'] == request['network'], 'NETWORK_MISMATCH')
                require(manifest['config_digest'] == request['config_digest'], 'CONFIGURATION_MISMATCH')
                expires = manifest['raw_expires_at_ms']
                row.update(origin=manifest['origin'], created_at_ms=str(manifest['created_at_ms']),
                           capture_time_status='AFTER_AUDIT' if manifest['created_at_ms'] > now_ms else 'AT_OR_BEFORE_AUDIT',
                           raw_expires_at_ms=None if expires is None else str(expires),
                           expiration_status='NOT_DECLARED' if expires is None else ('EXPIRED' if now_ms >= expires else 'NOT_EXPIRED'),
                           quote_inputs_declared_complete=manifest['complete_for_quote'])
                expected_names = {'manifest.json'} | {obj['name'] for obj in manifest['objects']}
                require(not names - expected_names, 'UNEXPECTED_BUNDLE_ENTRY')
                for obj in manifest['objects']:
                    try:
                        size, checksum, _ = read_file(fd, obj['name'], obj['bytes'], budget)
                        require(size == obj['bytes'] and checksum == obj['sha256'], 'OBJECT_DIGEST_MISMATCH')
                        row['verified_objects'] += 1
                    except FileNotFoundError:
                        row['missing_objects'] += 1
                    except AuditError as error:
                        if str(error) == 'AUDIT_BYTE_LIMIT':
                            raise
                        row['corrupt_objects'] += 1
                if row['corrupt_objects']:
                    row.update(raw_artifact_status='CORRUPT', reason='OBJECT_INTEGRITY_FAILED')
                elif row['missing_objects']:
                    row.update(raw_artifact_status='MISSING', reason='OBJECT_MISSING')
                else:
                    row.update(raw_artifact_status='AVAILABLE', reason='DECLARED_BYTES_VERIFIED')
    except FileNotFoundError:
        row.update(raw_artifact_status='MISSING', reason='CAPTURE_DIRECTORY_OR_INPUT_MISSING')
    except AuditError as error:
        reason = str(error)
        status = 'UNSUPPORTED' if reason == 'UNSUPPORTED_CAPTURE_SCHEMA' else ('LIMIT_EXCEEDED' if reason == 'AUDIT_BYTE_LIMIT' else 'CORRUPT')
        row.update(raw_artifact_status=status, reason=reason)
    except OSError:
        row.update(raw_artifact_status='UNREADABLE', reason='FILESYSTEM_REFUSAL')
    return row


def audit(root: str | Path, request: dict, now_ms: int, *, max_bytes: int = 256 * 1024**2) -> dict:
    """Perform a point-in-time inventory, without modifying files or historical evidence."""
    validate_request(request)
    require(uint(now_ms) and now_ms > 0, 'INVALID_AUDIT_TIME')
    require(os.name == 'posix', 'POSIX_REQUIRED')
    root = Path(root).absolute()
    require(root.resolve(strict=True) == root, 'ROOT_SYMLINK_FORBIDDEN')
    budget = Budget(max_bytes)
    rows = []
    with directory(root) as root_fd:
        for ref in request['captures']:
            rows.append(audit_one(root_fd, ref, request, now_ms, budget))
    unavailable = sum(row['raw_artifact_status'] != 'AVAILABLE' or row['expiration_status'] == 'EXPIRED' or row['capture_time_status'] == 'AFTER_AUDIT' or row['quote_inputs_declared_complete'] is not True for row in rows)
    limited = any(row['raw_artifact_status'] in {'LIMIT_EXCEEDED', 'UNREADABLE'} for row in rows)
    return {'schema_version': 1, 'kind': 'CAPTURE_DEPENDENCY_AUDIT', 'checked_at_ms': str(now_ms),
            'network': request['network'], 'config_digest': request['config_digest'],
            'request_sha256': digest(json.dumps(request, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()),
            'status': 'INCOMPLETE' if limited else ('COMPLETE_WITH_GAPS' if unavailable or not rows else 'COMPLETE'),
            'references_requested': len(request['captures']), 'references_reported': len(rows),
            'raw_status_counts': dict(sorted(Counter(row['raw_artifact_status'] for row in rows).items())),
            'bytes_read_budgeted': str(budget.used), 'max_bytes': str(budget.maximum),
            'source_authenticity': 'CALLER_RETAINED_DIGEST_NOT_INDEPENDENTLY_AUTHENTICATED',
            'filesystem_snapshot': 'NON_ATOMIC_KEEP_ROOT_QUIESCED',
            'execution_authorized': False, 'dependencies': rows}


def main() -> int:
    """Emit a bounded redacted JSON report; use exits 0=complete, 1=gaps, 2=failure."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True)
    parser.add_argument('--request', required=True)
    parser.add_argument('--now-ms', required=True, type=int)
    parser.add_argument('--max-bytes', type=int, default=256 * 1024**2)
    args = parser.parse_args()
    try:
        path = Path(args.request).absolute()
        require(path.parent.resolve(strict=True) == path.parent, 'REQUEST_PARENT_SYMLINK_FORBIDDEN')
        with directory(path.parent) as fd:
            _, _, raw = read_file(fd, path.name, MAX_MANIFEST, Budget(MAX_MANIFEST), retain=True)
        report = audit(args.root, decode(raw), args.now_ms, max_bytes=args.max_bytes)
        print(json.dumps(report, sort_keys=True, ensure_ascii=True))
        return {'COMPLETE': 0, 'COMPLETE_WITH_GAPS': 1, 'INCOMPLETE': 2}[report['status']]
    except (AuditError, OSError, ValueError, TypeError, KeyError, RecursionError):
        print(json.dumps({'status': 'AUDIT_FAILED', 'reason': 'INVALID_REQUEST_ROOT_OR_UNSTABLE_SOURCE',
                          'execution_authorized': False}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
