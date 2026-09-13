#!/usr/bin/env python3
"""Bind a local capture audit to a dashboard-generated frozen-export request."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import capture_audit as captures

REQUEST_KIND = 'FROZEN_EXPORT_CAPTURE_AUDIT_REQUEST'
REPORT_KIND = 'FROZEN_EXPORT_CAPTURE_AUDIT'


def canonical_digest(value: dict) -> str:
    """Hash the strict ASCII/string/integer envelope shared with the browser."""
    return captures.digest(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                      ensure_ascii=True, allow_nan=False).encode())


def validate_request(value: Any) -> dict:
    """Validate the complete binding before any access to the capture volume."""
    captures.require(isinstance(value, dict) and set(value) == {
        'schema_version', 'kind', 'source_export', 'capture_request'}, 'INVALID_EXPORT_REQUEST')
    captures.require(type(value['schema_version']) is int and value['schema_version'] == 1
                     and value['kind'] == REQUEST_KIND, 'UNSUPPORTED_EXPORT_REQUEST')
    source = value['source_export']
    captures.require(isinstance(source, dict) and set(source) == {
        'schema_version', 'export_id', 'content_sha256', 'session_id'}, 'INVALID_EXPORT_BINDING')
    captures.require(source['schema_version'] == '1.1.0'
                     and captures.label(source['export_id']) and captures.label(source['session_id'])
                     and captures.valid_digest(source['content_sha256']), 'INVALID_EXPORT_BINDING')
    captures.validate_request(value['capture_request'])
    return value


def audit_export(root: str | Path, request: dict, now_ms: int,
                 *, max_bytes: int = 256 * 1024**2) -> dict:
    """Preserve every requested reference and echo only the validated source binding."""
    validate_request(request)
    report = captures.audit(root, request['capture_request'], now_ms, max_bytes=max_bytes)
    return {'schema_version': 1, 'kind': REPORT_KIND,
            'source_export': dict(request['source_export']),
            'request_sha256': canonical_digest(request), 'audit': report}


def main() -> int:
    """Use the existing bounded descriptor reader; never print raw failures or paths."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True)
    parser.add_argument('--request', required=True)
    parser.add_argument('--now-ms', required=True, type=int)
    parser.add_argument('--max-bytes', type=int, default=256 * 1024**2)
    args = parser.parse_args()
    try:
        path = Path(args.request).absolute()
        captures.require(path.parent.resolve(strict=True) == path.parent,
                         'REQUEST_PARENT_SYMLINK_FORBIDDEN')
        with captures.directory(path.parent) as fd:
            _, _, raw = captures.read_file(fd, path.name, captures.MAX_MANIFEST,
                                          captures.Budget(captures.MAX_MANIFEST), retain=True)
        report = audit_export(args.root, captures.decode(raw), args.now_ms, max_bytes=args.max_bytes)
        print(json.dumps(report, sort_keys=True, ensure_ascii=True))
        return {'COMPLETE': 0, 'COMPLETE_WITH_GAPS': 1, 'INCOMPLETE': 2}[report['audit']['status']]
    except (captures.AuditError, OSError, ValueError, TypeError, KeyError, RecursionError):
        print(json.dumps({'status': 'EXPORT_AUDIT_FAILED',
                          'reason': 'INVALID_REQUEST_ROOT_OR_UNSTABLE_SOURCE',
                          'execution_authorized': False}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
