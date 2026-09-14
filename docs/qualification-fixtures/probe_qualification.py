#!/usr/bin/env python3
"""Explicit bounded public-RPC feasibility sampling; never enables a provider."""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import platform
import shutil
import time

import probe_public_rpc as transport

SAMPLES = 3


def distribution(calls: list[dict]) -> dict:
    """Nearest-rank statistics, separating successful responses from failures."""
    def group(rows):
        values = sorted(r['elapsed_ms'] for r in rows)
        return {'n': len(values), **{f'p{p}_ms': values[math.ceil(len(values) * p / 100) - 1]
                if values else None for p in (50, 95, 99)}}
    return {'all_attempts': group(calls),
            'successful_rpc': group([c for c in calls if c['outcome'] == 'success']),
            'failed_attempts': group([c for c in calls if c['outcome'] != 'success']),
            'interpretation': 'SMALL_SAMPLE_NOT_SLA_OR_SUSTAINED_CAPACITY'}


def sample(chain: str, call=transport.rpc) -> list[dict]:
    """The fixed request budget and primary endpoints cannot be overridden."""
    if chain not in ('base', 'solana'):
        raise ValueError('UNSUPPORTED_NETWORK')
    calls = []

    def get(method, params):
        result = call(chain, method, params, len(calls))
        # Keep response hashes/statuses, not arbitrary upstream error strings.
        if result['outcome'] != 'success':
            result.pop('response_body', None)
            result.pop('result', None)
        calls.append(result)
        return result

    identity = get('eth_chainId' if chain == 'base' else 'getGenesisHash', [])
    if identity['outcome'] != 'success':
        return calls
    if chain == 'base':
        if identity['result'] != '0x2105':
            identity['outcome'] = 'unexpected-chain-identity'
            return calls
        block = get('eth_getBlockByNumber', ['finalized', False])
        if block['outcome'] != 'success' or not isinstance(block.get('result'), dict):
            return calls
        block_hash = block['result'].get('hash')
        if not isinstance(block_hash, str) or len(block_hash) != 66 or not block_hash.startswith('0x'):
            return calls
        try:
            bytes.fromhex(block_hash[2:])
        except ValueError:
            return calls
        context = {'blockHash': block_hash, 'requireCanonical': True}
        if get('eth_getCode', [transport.FACTORY, context])['outcome'] != 'success':
            return calls
        for fee in (500, 3000):
            data = ('0x1698ee82' + transport.BASE_WETH[2:].rjust(64, '0')
                    + transport.BASE_USDC[2:].rjust(64, '0') + hex(fee)[2:].rjust(64, '0'))
            get('eth_call', [{'to': transport.FACTORY, 'data': data}, context])
    else:
        accounts = get('getMultipleAccounts', [[transport.WHIRLPOOL, transport.WSOL, transport.SOL_USDC],
                                              {'encoding': 'base64', 'commitment': 'finalized'}])
        if accounts['outcome'] == 'success':
            get('getProgramAccounts', [transport.WHIRLPOOL, {
                'encoding': 'base64', 'commitment': 'finalized', 'withContext': True,
                'dataSlice': {'offset': 0, 'length': 245}, 'filters': [
                    {'dataSize': 653}, {'memcmp': {'offset': 101, 'bytes': transport.WSOL}},
                    {'memcmp': {'offset': 181, 'bytes': transport.SOL_USDC}}]}])
    return calls


def collect() -> dict:
    """Perform at most 24 sequential, read-only requests with no retries."""
    report = {'schema_version': 1, 'purpose': 'ARB-002_FEASIBILITY_NOT_PRODUCTION_QUALIFICATION',
              'started_at_utc': datetime.now(timezone.utc).isoformat(),
              'source_sha': os.environ.get('GITHUB_SHA'),
              'run_id': os.environ.get('GITHUB_RUN_ID'),
              'host': {'os': platform.system(), 'architecture': platform.machine(),
                       'python': platform.python_version(), 'logical_cpus': os.cpu_count(),
                       'disk_total_bytes': str(shutil.disk_usage('.').total)},
              'budget': {'paid_rpc_spend_eur': '0', 'max_requests': 24, 'retries': 0,
                         'response_byte_cap': transport.MAX_BYTES,
                         'per_request_timeout_seconds': transport.TIMEOUT_SECONDS},
              'providers': {}}
    for chain in ('base', 'solana'):
        rounds = []
        for _ in range(SAMPLES):
            rounds.append(sample(chain))
            if any(c.get('http_status') in (403, 429) for c in rounds[-1]):
                break  # Respect access/rate refusal; do not seek another endpoint.
            time.sleep(1)
        calls = [c for run in rounds for c in run]
        report['providers'][chain] = {
            'public_endpoint': transport.ENDPOINTS[chain], 'rounds': rounds,
            'outcomes': dict(Counter(c['outcome'] for c in calls)),
            'latency_by_method': {method: distribution([c for c in calls if c['request']['method'] == method])
                                  for method in sorted({c['request']['method'] for c in calls})},
            'complete_quote_state_verified': False, 'production_qualified': False,
            'decision': 'BLOCKED_PENDING_COMPLETE_STATE_AND_OPERATIONAL_QUALIFICATION',
            'blocking_gaps': ['FULL_AMOUNT_SPECIFIC_TICK_STATE_NOT_QUALIFIED',
                              'RECONNECT_BACKFILL_ARCHIVE_AND_SUSTAINED_QUOTA_NOT_QUALIFIED',
                              'NO_APPROVED_CAMPAIGN_SERVICE_OR_SPENDING'],
            'chain_inactivity_inferred': False}
    report['finished_at_utc'] = datetime.now(timezone.utc).isoformat()
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-public-probe', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.run_public_probe:
        print(json.dumps({'status': 'NOT_RUN', 'max_requests': 24, 'paid_rpc_spend_eur': '0'}))
        return 0
    if args.output is None or args.output.exists():
        parser.error('A new output path is required; existing evidence is never overwritten.')
    report = collect()
    with args.output.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps({c: p['outcomes'] for c, p in report['providers'].items()}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
