#!/usr/bin/env python3
"""Historical Base WETH/USDC pool-fee gap study; research-only, not P&L or qualification evidence.

Answers: how often, how large and how long price gaps between the Uniswap V3
WETH/USDC pools on Base were over a historical window (gaps per hour, size
distribution in bps, persistence in blocks).

Does not answer: whether a gap would have been won (competition, inclusion,
latency), what gas or slippage would have cost, or any profit figure.
"""
from __future__ import annotations

import argparse
import csv
from decimal import Decimal, getcontext
import itertools
import json
import os
from pathlib import Path
from statistics import median
import time
import urllib.error
import urllib.request

getcontext().prec = 60

FACTORY = '0x33128a8fc17869897dce68ed026d694621f6fdfd'
WETH = '0x4200000000000000000000000000000000000006'
USDC = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913'
FEES = (100, 500, 3000, 10000)
WETH_DECIMALS = 18
USDC_DECIMALS = 6
SWAP_TOPIC0 = '0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67'
GET_POOL_SELECTOR = '0x1698ee82'
DEFAULT_RPC = 'https://mainnet.base.org'
BLOCK_TIME_SECONDS = 2
BLOCKS_PER_HOUR = 3600 // BLOCK_TIME_SECONDS
MIN_POLITE_INTERVAL_MS = 250
Q96 = 2 ** 96
RANGE_ERROR_HINTS = ('block range', 'query returned more', 'limit exceeded',
                      'exceeds', 'too large', 'more than', 'response size')
CAVEAT = ('Pool fees only; no gas, slippage, competition or inclusion is modelled. '
          'This does not indicate whether any gap would have been won, what execution '
          'would have cost, or any profit figure. Candidate frequency only, not '
          'acceptance evidence for any ARB ticket.')


class StudyError(ValueError):
    """Fixed non-sensitive reason; provider responses are never echoed verbatim."""


def require(ok, reason):
    if not ok:
        raise StudyError(reason)


class RpcCallError(Exception):
    """A JSON-RPC application error (distinct from transport failures)."""

    def __init__(self, error):
        self.code = error.get('code') if isinstance(error, dict) else None
        self.message = str(error.get('message', '')) if isinstance(error, dict) else str(error)
        super().__init__(self.message)


def is_range_error(message: str) -> bool:
    lower = str(message).lower()
    return any(hint in lower for hint in RANGE_ERROR_HINTS)


class Rpc:
    """Public JSON-RPC client: polite pacing, bounded retries with backoff.

    Application-level RPC errors (the 'error' field) are raised as
    RpcCallError and are the caller's responsibility (e.g. adaptive range
    halving); only transport failures are retried here.
    """

    def __init__(self, endpoint: str, min_interval_ms: int = MIN_POLITE_INTERVAL_MS,
                 max_retries: int = 5, timeout: float = 20.0):
        require(min_interval_ms >= MIN_POLITE_INTERVAL_MS, 'PACING_TOO_AGGRESSIVE')
        self.endpoint = endpoint
        self.min_interval = min_interval_ms / 1000
        self.max_retries = max_retries
        self.timeout = timeout
        self._last_call = None

    def _pace(self):
        if self._last_call is not None:
            elapsed = time.monotonic() - self._last_call
            if elapsed < self.min_interval:
                time.sleep(self.min_interval - elapsed)

    def call(self, method: str, params: list):
        request = {'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}
        body = json.dumps(request).encode()
        backoff = 0.5
        last_error = None
        for attempt in range(self.max_retries):
            self._pace()
            self._last_call = time.monotonic()
            try:
                req = urllib.request.Request(
                    self.endpoint, data=body,
                    headers={'Content-Type': 'application/json'}, method='POST')
                with urllib.request.urlopen(req, timeout=self.timeout) as response:
                    raw = response.read()
                parsed = json.loads(raw)
                require(isinstance(parsed, dict), 'MALFORMED_RPC_RESPONSE')
                if 'error' in parsed:
                    raise RpcCallError(parsed['error'])
                require('result' in parsed, 'MALFORMED_RPC_RESPONSE')
                return parsed['result']
            except RpcCallError:
                raise
            except (urllib.error.URLError, OSError, ValueError) as exc:
                last_error = exc
                if attempt == self.max_retries - 1:
                    raise StudyError('RPC_TRANSPORT_EXHAUSTED') from exc
                time.sleep(backoff)
                backoff *= 2
        raise StudyError('RPC_TRANSPORT_EXHAUSTED') from last_error


# --- ABI helpers -----------------------------------------------------------

def encode_get_pool(token_a: str, token_b: str, fee: int) -> str:
    return GET_POOL_SELECTOR + token_a[2:].zfill(64) + token_b[2:].zfill(64) + f'{fee:064x}'


def decode_address(word_hex: str) -> str:
    require(isinstance(word_hex, str) and word_hex.startswith('0x'), 'INVALID_ABI_WORD')
    data = bytes.fromhex(word_hex[2:].zfill(64))
    require(len(data) == 32 and data[:12] == bytes(12), 'INVALID_ABI_ADDRESS')
    return '0x' + data[12:].hex()


def decode_sqrt_price_x96(log_data: str) -> int:
    require(isinstance(log_data, str) and log_data.startswith('0x'), 'INVALID_SWAP_LOG_DATA')
    data = bytes.fromhex(log_data[2:])
    require(len(data) == 160, 'INVALID_SWAP_LOG_DATA')
    return int.from_bytes(data[64:96], 'big')


# --- Price math (Decimal only, no floats) -----------------------------------

def sqrt_price_to_usdc_per_weth(sqrt_price_x96: int) -> Decimal:
    require(isinstance(sqrt_price_x96, int) and 0 < sqrt_price_x96 < (1 << 160), 'INVALID_SQRT_PRICE')
    ratio = Decimal(sqrt_price_x96) / Decimal(Q96)
    return ratio * ratio * (Decimal(10) ** (WETH_DECIMALS - USDC_DECIMALS))


def fee_bps(fee_ppm: int) -> Decimal:
    return Decimal(fee_ppm) / Decimal(100)


def gross_gap_bps(price_a: Decimal, price_b: Decimal) -> Decimal:
    require(price_a > 0 and price_b > 0, 'INVALID_PRICE')
    return abs(price_a - price_b) / min(price_a, price_b) * Decimal(10000)


def net_gap_bps(price_a: Decimal, price_b: Decimal, fee_a_ppm: int, fee_b_ppm: int) -> Decimal:
    return gross_gap_bps(price_a, price_b) - fee_bps(fee_a_ppm) - fee_bps(fee_b_ppm)


# --- Pool discovery ----------------------------------------------------------

def discover_pools(rpc) -> dict:
    """fee_ppm -> lowercase pool address, for pools that exist (non-zero)."""
    pools = {}
    for fee in FEES:
        data = encode_get_pool(WETH, USDC, fee)
        result = rpc.call('eth_call', [{'to': FACTORY, 'data': data}, 'latest'])
        address = decode_address(result)
        if address == '0x' + '0' * 40:
            continue
        pools[fee] = address.lower()
    return pools


# --- Block-range resolution ---------------------------------------------------

def get_head_block(rpc):
    block = rpc.call('eth_getBlockByNumber', ['latest', False])
    require(isinstance(block, dict), 'MISSING_BLOCK_HEADER')
    return int(block['number'], 16), int(block['timestamp'], 16)


def estimate_from_block(rpc, head_number: int, head_timestamp: int, days: int) -> int:
    target_timestamp = head_timestamp - days * 86400
    estimate = max(0, head_number - (days * 86400) // BLOCK_TIME_SECONDS)
    for _ in range(6):
        block = rpc.call('eth_getBlockByNumber', [hex(estimate), False])
        require(isinstance(block, dict), 'MISSING_BLOCK_HEADER')
        timestamp = int(block['timestamp'], 16)
        diff_seconds = target_timestamp - timestamp
        if abs(diff_seconds) <= BLOCK_TIME_SECONDS:
            break
        estimate = max(0, estimate + diff_seconds // BLOCK_TIME_SECONDS)
    return estimate


def resolve_block_range(rpc, from_block, to_block, days):
    if from_block is not None and to_block is not None:
        require(from_block <= to_block, 'INVALID_BLOCK_RANGE')
        return from_block, to_block
    head_number, head_timestamp = get_head_block(rpc)
    resolved_to = to_block if to_block is not None else head_number
    resolved_from = (from_block if from_block is not None
                      else estimate_from_block(rpc, head_number, head_timestamp, days))
    require(resolved_from <= resolved_to, 'INVALID_BLOCK_RANGE')
    return resolved_from, resolved_to


# --- Checkpointed, adaptive log collection ------------------------------------

def checkpoint_path(out_dir: Path, pool: str) -> Path:
    return out_dir / f'checkpoint-{pool}.json'


def load_checkpoint(out_dir: Path, pool: str, from_block: int) -> int:
    path = checkpoint_path(out_dir, pool)
    if not path.exists():
        return from_block
    data = json.loads(path.read_text())
    require(data.get('pool') == pool, 'CHECKPOINT_POOL_MISMATCH')
    return data['next_block']


def save_checkpoint(out_dir: Path, pool: str, next_block: int) -> None:
    checkpoint_path(out_dir, pool).write_text(json.dumps({'pool': pool, 'next_block': next_block}))


def fetch_logs(rpc, address: str, from_block: int, to_block: int):
    return rpc.call('eth_getLogs', [{
        'address': address,
        'topics': [SWAP_TOPIC0],
        'fromBlock': hex(from_block),
        'toBlock': hex(to_block),
    }])


def collect_pool_swaps(rpc, pool: str, from_block: int, to_block: int, out_dir: Path,
                        initial_span: int, min_span: int = 1, sink=None) -> int:
    """Fetch Swap logs for one pool over [from_block, to_block], with adaptive
    range halving on provider range/result-limit errors and a resumable
    per-pool checkpoint under out_dir. Returns the number of logs fetched
    this run (already-checkpointed ranges are skipped, not counted)."""
    next_block = load_checkpoint(out_dir, pool, from_block)
    span = initial_span
    fetched = 0
    while next_block <= to_block:
        end = min(next_block + span - 1, to_block)
        try:
            logs = fetch_logs(rpc, pool, next_block, end)
        except RpcCallError as exc:
            if span > min_span and is_range_error(exc.message):
                span = max(min_span, span // 2)
                continue
            raise
        for log in logs:
            fetched += 1
            if sink is not None:
                sink(log)
        next_block = end + 1
        save_checkpoint(out_dir, pool, next_block)
    return fetched


# --- Gap analysis --------------------------------------------------------------

def build_price_events(pool_logs: dict) -> list:
    """pool_logs: pool address -> list of raw eth_getLogs rows.
    Returns a list of (block, log_index, pool, price) sorted by (block, log_index)."""
    events = []
    for pool, rows in pool_logs.items():
        for log in rows:
            block = int(log['blockNumber'], 16)
            log_index = int(log['logIndex'], 16)
            price = sqrt_price_to_usdc_per_weth(decode_sqrt_price_x96(log['data']))
            events.append((block, log_index, pool, price))
    events.sort(key=lambda e: (e[0], e[1]))
    return events


def pair_runs(events: list, pools: list) -> dict:
    """Carry each pool's latest price forward and split pairwise runs whenever
    either pool's price changes. Returns pair -> list of open (start_block,
    price_a, price_b) tuples still needing to be closed by close_runs, mixed
    with already-closed (start, end, price_a, price_b) tuples."""
    latest = {}
    open_start = {}
    open_prices = {}
    pairs = list(itertools.combinations(sorted(pools), 2))
    runs = {pair: [] for pair in pairs}
    for block, group in itertools.groupby(events, key=lambda e: e[0]):
        changed = set()
        for _, _, pool, price in group:
            latest[pool] = price
            changed.add(pool)
        for pair in pairs:
            a, b = pair
            if a not in latest or b not in latest:
                continue
            if pair in open_start:
                if a in changed or b in changed:
                    runs[pair].append((open_start[pair], block - 1, *open_prices[pair]))
                    open_start[pair] = block
                    open_prices[pair] = (latest[a], latest[b])
            else:
                open_start[pair] = block
                open_prices[pair] = (latest[a], latest[b])
    return runs, open_start, open_prices


def close_runs(runs: dict, open_start: dict, open_prices: dict, to_block: int) -> dict:
    for pair, start in open_start.items():
        runs[pair].append((start, to_block, *open_prices[pair]))
    return runs


def weighted_percentile(values_with_weights: list, percentile: int):
    if not values_with_weights:
        return None
    total_weight = sum(weight for _, weight in values_with_weights)
    ordered = sorted(values_with_weights, key=lambda vw: vw[0])
    threshold = Decimal(percentile) / Decimal(100) * Decimal(total_weight)
    cumulative = Decimal(0)
    for value, weight in ordered:
        cumulative += weight
        if cumulative >= threshold:
            return value
    return ordered[-1][0]


def summarize_pair(pair_run_list: list, fee_a_ppm: int, fee_b_ppm: int, from_block: int) -> dict:
    per_hour = {}
    positive_values_weights = []
    episodes = []
    episode_blocks = 0
    in_episode = False
    for start, end, price_a, price_b in sorted(pair_run_list):
        net = net_gap_bps(price_a, price_b, fee_a_ppm, fee_b_ppm)
        length = end - start + 1
        block = start
        while block <= end:
            hour = (block - from_block) // BLOCKS_PER_HOUR
            hour_last_block = from_block + (hour + 1) * BLOCKS_PER_HOUR - 1
            seg_end = min(end, hour_last_block)
            seg_len = seg_end - block + 1
            bucket = per_hour.setdefault(hour, {'gt0': 0, 'gt10': 0, 'gt25': 0})
            if net > 0:
                bucket['gt0'] += seg_len
            if net > 10:
                bucket['gt10'] += seg_len
            if net > 25:
                bucket['gt25'] += seg_len
            block = seg_end + 1
        if net > 0:
            positive_values_weights.append((net, length))
            episode_blocks += length
            in_episode = True
        else:
            if in_episode:
                episodes.append(episode_blocks)
            in_episode = False
            episode_blocks = 0
    if in_episode:
        episodes.append(episode_blocks)
    hours_covered = max(1, len(per_hour))
    return {
        'hours_covered': hours_covered,
        'avg_blocks_per_hour_gt0': sum(b['gt0'] for b in per_hour.values()) / hours_covered,
        'avg_blocks_per_hour_gt10': sum(b['gt10'] for b in per_hour.values()) / hours_covered,
        'avg_blocks_per_hour_gt25': sum(b['gt25'] for b in per_hour.values()) / hours_covered,
        'p50_bps': weighted_percentile(positive_values_weights, 50),
        'p90_bps': weighted_percentile(positive_values_weights, 90),
        'max_bps': max((v for v, _ in positive_values_weights), default=None),
        'persistence_p50_blocks': median(episodes) if episodes else None,
        'persistence_max_blocks': max(episodes) if episodes else None,
        'episode_count': len(episodes),
    }


# --- Output --------------------------------------------------------------------

def write_pools_json(path: Path, pools: dict, from_block: int, to_block: int) -> None:
    payload = {
        'factory': FACTORY, 'weth': WETH, 'usdc': USDC,
        'from_block': from_block, 'to_block': to_block,
        'pools': [{'fee_ppm': fee, 'address': addr} for fee, addr in sorted(pools.items())],
    }
    path.write_text(json.dumps(payload, indent=2) + '\n')


CSV_PRICE_QUANT = Decimal('0.000001')
CSV_BPS_QUANT = Decimal('0.0001')


def write_gaps_csv(path: Path, runs: dict, fee_by_pool: dict) -> None:
    with path.open('w', newline='') as handle:
        writer = csv.writer(handle)
        writer.writerow(['pool_a', 'pool_b', 'fee_a_ppm', 'fee_b_ppm', 'start_block', 'end_block',
                          'price_a_usdc_per_weth', 'price_b_usdc_per_weth', 'gross_gap_bps', 'net_gap_bps'])
        for (pool_a, pool_b), pair_run_list in sorted(runs.items()):
            for start, end, price_a, price_b in sorted(pair_run_list):
                gross = gross_gap_bps(price_a, price_b)
                net = net_gap_bps(price_a, price_b, fee_by_pool[pool_a], fee_by_pool[pool_b])
                writer.writerow([pool_a, pool_b, fee_by_pool[pool_a], fee_by_pool[pool_b], start, end,
                                  price_a.quantize(CSV_PRICE_QUANT), price_b.quantize(CSV_PRICE_QUANT),
                                  gross.quantize(CSV_BPS_QUANT), net.quantize(CSV_BPS_QUANT)])


def fmt(value) -> str:
    return '-' if value is None else f'{value:.2f}'


def write_summary_md(path: Path, summary: dict, pools: dict, from_block: int, to_block: int) -> None:
    lines = [
        '# Historical Base WETH/USDC gap study', '',
        f'Blocks {from_block}-{to_block} (Base mainnet). Pools found: ' +
        ', '.join(f'{fee / 100:g}bps={addr}' for fee, addr in sorted(pools.items())) + '.', '',
        f'**Caveat:** {CAVEAT}', '',
        '| Pair | Hours | Avg blocks/h net>0bps | Avg blocks/h net>10bps | Avg blocks/h net>25bps | '
        'p50 bps | p90 bps | max bps | persistence p50 (blocks) | persistence max (blocks) | episodes |',
        '|---|---|---|---|---|---|---|---|---|---|---|',
    ]
    for (pool_a, pool_b), row in sorted(summary.items()):
        lines.append('| {}.../{}... | {} | {:.2f} | {:.2f} | {:.2f} | {} | {} | {} | {} | {} | {} |'.format(
            pool_a[:10], pool_b[:10], row['hours_covered'],
            row['avg_blocks_per_hour_gt0'], row['avg_blocks_per_hour_gt10'], row['avg_blocks_per_hour_gt25'],
            fmt(row['p50_bps']), fmt(row['p90_bps']), fmt(row['max_bps']),
            row['persistence_p50_blocks'] or '-', row['persistence_max_blocks'] or '-', row['episode_count']))
    if not summary:
        lines.append('')
        lines.append('_No pool pair had swap events in this window; nothing to compare._')
    path.write_text('\n'.join(lines) + '\n')


# --- CLI -------------------------------------------------------------------------

def run(rpc, from_block: int, to_block: int, out_dir: Path, initial_span: int) -> dict:
    out_dir.mkdir(parents=True, exist_ok=True)
    pools = discover_pools(rpc)
    write_pools_json(out_dir / 'pools.json', pools, from_block, to_block)
    require(len(pools) >= 2, 'INSUFFICIENT_POOLS_FOR_COMPARISON')

    pool_logs = {}
    for fee, address in pools.items():
        rows = []
        collect_pool_swaps(rpc, address, from_block, to_block, out_dir, initial_span, sink=rows.append)
        pool_logs[address] = rows

    events = build_price_events(pool_logs)
    runs, open_start, open_prices = pair_runs(events, list(pools.values()))
    close_runs(runs, open_start, open_prices, to_block)

    fee_by_pool = {addr: fee for fee, addr in pools.items()}
    summary = {pair: summarize_pair(pair_run_list, fee_by_pool[pair[0]], fee_by_pool[pair[1]], from_block)
               for pair, pair_run_list in runs.items() if pair_run_list}

    write_gaps_csv(out_dir / 'gaps.csv', runs, fee_by_pool)
    write_summary_md(out_dir / 'summary.md', summary, pools, from_block, to_block)
    return {'from_block': from_block, 'to_block': to_block, 'pools_found': len(pools),
            'pairs_analyzed': len(summary), 'out_dir': str(out_dir)}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--collect', action='store_true',
                         help='Actually contact the RPC and run the study; default is a no-op.')
    parser.add_argument('--from-block', type=int, default=None)
    parser.add_argument('--to-block', type=int, default=None)
    parser.add_argument('--days', type=int, default=14)
    parser.add_argument('--out', type=Path, default=None)
    parser.add_argument('--min-interval-ms', type=int, default=MIN_POLITE_INTERVAL_MS)
    parser.add_argument('--initial-span', type=int, default=2000)
    args = parser.parse_args(argv)

    if not args.collect:
        print(json.dumps({'status': 'NOT_RUN', 'rpc_env_var': 'ARB_STUDY_RPC_URL'}))
        return 0

    rpc_url = os.environ.get('ARB_STUDY_RPC_URL', DEFAULT_RPC)
    rpc = Rpc(rpc_url, min_interval_ms=args.min_interval_ms)
    from_block, to_block = resolve_block_range(rpc, args.from_block, args.to_block, args.days)
    out_dir = args.out or Path('history-study') / f'{from_block}-{to_block}'
    result = run(rpc, from_block, to_block, out_dir, args.initial_span)
    print(json.dumps(result))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
