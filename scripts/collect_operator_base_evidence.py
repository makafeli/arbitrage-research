#!/usr/bin/env python3
"""Collect a redacted Base identity summary using an existing authorized endpoint."""
from __future__ import annotations

import argparse
import ipaddress
import json
import os
from pathlib import Path
from urllib.parse import urlsplit

import inspect_pool_candidates as observer

SECRET_NAME = 'ARB_BASE_RPC_URL'
BASE_METHODS = {'eth_chainId', 'eth_getBlockByNumber', 'eth_getCode', 'eth_call'}
SUMMARY_FIELDS = {
    'network', 'status', 'reason', 'started_at_utc', 'finished_at_utc',
    'context', 'factory', 'assets', 'pools', 'canonical_recheck_passed',
}
REQUEST_FIELDS = {'request', 'started_at_utc', 'elapsed_ms', 'outcome',
                  'http_status', 'response_sha256'}


class AccessError(ValueError):
    """Fixed preflight reason, never a credential value or upstream message."""


def endpoint_from_environment() -> str:
    value = os.environ.get(SECRET_NAME)
    if not value:
        raise AccessError('AUTHORIZED_BASE_ENDPOINT_MISSING')
    try:
        if len(value) > 8192 or any(ord(c) <= 32 or ord(c) == 127 for c in value):
            raise ValueError
        url = urlsplit(value)
        host = url.hostname or ''
        if (url.scheme != 'https' or not host or url.username is not None
                or url.password is not None or url.fragment or url.port not in (None, 443)
                or '\\' in value or host.endswith('.')
                or host.lower() == 'localhost'
                or host.lower().endswith(('.localhost', '.local', '.internal'))):
            raise ValueError
        try:
            address = ipaddress.ip_address(host)
        except ValueError:
            # This is operator-supplied trusted configuration, not an arbitrary
            # public URL endpoint. Network egress/DNS policy remains external.
            if '.' not in host or not all(c.isascii() and (c.isalnum() or c in '.-') for c in host):
                raise ValueError
        else:
            if not address.is_global:
                raise ValueError
    except (ValueError, TypeError):
        raise AccessError('INVALID_AUTHORIZED_BASE_ENDPOINT') from None
    return value


class OperatorBaseRpc(observer.PublicRpc):
    def __init__(self, endpoint: str):
        super().__init__('base-mainnet')
        self.endpoint = endpoint

    def call(self, method: str, params: list):
        if method not in BASE_METHODS:
            raise observer.ObservationError('METHOD_NOT_READ_ONLY_BASE')
        try:
            return super().call(method, params)
        finally:
            # Private-provider raw payloads may echo request credentials. Do
            # not retain or export them. Exact response hashes remain available.
            if self.calls:
                self.calls[-1].pop('response_base64', None)


def collect(endpoint: str) -> dict:
    rpc = OperatorBaseRpc(endpoint)
    observed = observer.observe('base-mainnet', rpc)
    result = {key: value for key, value in observed.items() if key in SUMMARY_FIELDS}
    result['requests'] = [
        {key: value for key, value in row.items() if key in REQUEST_FIELDS}
        for row in rpc.calls
    ]
    result.update(
        schema_version=1,
        kind='OPERATOR_BASE_IDENTITY_SUMMARY',
        endpoint_reference=SECRET_NAME,
        raw_response_bytes_exported=False,
        provider_identity_attested=False,
        production_qualified=False,
        runtime_activation=False,
        execution_authorized=False,
        limitation='Single-provider observation; hashes alone cannot reconstruct raw responses.',
    )
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--collect', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    if not args.collect:
        print(json.dumps({'status': 'NOT_RUN', 'secret_reference': SECRET_NAME,
                          'network_calls': 0}))
        return 0
    try:
        endpoint = endpoint_from_environment()
        if args.output is None:
            raise AccessError('NEW_OUTPUT_FILE_REQUIRED')
        # Reserve output before any network operation. Existing files or links
        # fail without contacting the provider or overwriting earlier evidence.
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'w', encoding='utf-8') as stream:
            result = collect(endpoint)
            json.dump(result, stream, indent=2, ensure_ascii=True, allow_nan=False)
            stream.write('\n')
        print(json.dumps({'status': result['status'],
                          'requests': len(result['requests']),
                          'production_qualified': False}))
        return 0 if result['status'] == 'OBSERVATIONS_COLLECTED' else 2
    except AccessError as exc:
        print(json.dumps({'status': 'START_BLOCKED', 'reason': str(exc)}))
    except (OSError, ValueError, TypeError, RecursionError):
        print(json.dumps({'status': 'COLLECTION_FAILED', 'reason': 'INPUT_OUTPUT_OR_OBSERVATION_ERROR'}))
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
