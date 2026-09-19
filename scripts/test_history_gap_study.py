"""Synthetic fixtures test the study's math and mechanics; no network access."""
from contextlib import redirect_stdout
from decimal import Decimal
import io
import json
from pathlib import Path
import tempfile
import unittest

import history_gap_study as h

POOL_A = '0x' + 'aa' * 20
POOL_B = '0x' + 'bb' * 20
POOL_C = '0x' + 'cc' * 20


def swap_log(block, log_index, sqrt_price_x96):
    data = (0).to_bytes(32, 'big') + (0).to_bytes(32, 'big') + \
        sqrt_price_x96.to_bytes(32, 'big') + (0).to_bytes(32, 'big') + (0).to_bytes(32, 'big', signed=False)
    return {'blockNumber': hex(block), 'logIndex': hex(log_index), 'data': '0x' + data.hex()}


def sqrt_price_for(usdc_per_weth: Decimal) -> int:
    """Inverse of sqrt_price_to_usdc_per_weth, for building test fixtures."""
    raw_ratio_squared = usdc_per_weth / (Decimal(10) ** (h.WETH_DECIMALS - h.USDC_DECIMALS))
    ratio = raw_ratio_squared.sqrt()
    return int(ratio * Decimal(h.Q96))


class FakeFactoryRpc:
    """Fake eth_call responses for factory.getPool(WETH, USDC, fee)."""

    def __init__(self, pools_by_fee):
        self.pools_by_fee = pools_by_fee
        self.calls = []

    def call(self, method, params):
        self.calls.append((method, params))
        assert method == 'eth_call'
        data = params[0]['data']
        fee = int(data[-64:], 16)
        address = self.pools_by_fee.get(fee, '0x' + '00' * 20)
        return '0x' + '00' * 12 + bytes.fromhex(address[2:]).hex()


class FakeLogsRpc:
    """Fake eth_getLogs that enforces a maximum block span, like a strict provider."""

    def __init__(self, max_span, logs_by_block):
        self.max_span = max_span
        self.logs_by_block = logs_by_block
        self.requested_ranges = []
        self.accepted_ranges = []

    def call(self, method, params):
        assert method == 'eth_getLogs'
        request = params[0]
        from_block = int(request['fromBlock'], 16)
        to_block = int(request['toBlock'], 16)
        self.requested_ranges.append((from_block, to_block))
        if to_block - from_block + 1 > self.max_span:
            raise h.RpcCallError({'code': -32005, 'message': 'query returned more than 10000 results'})
        self.accepted_ranges.append((from_block, to_block))
        rows = []
        for block in range(from_block, to_block + 1):
            rows.extend(self.logs_by_block.get(block, []))
        return rows


class PriceMathTests(unittest.TestCase):
    def test_round_trips_known_price_within_integer_truncation_tolerance(self):
        for target in (Decimal('2500.123456'), Decimal('1'), Decimal('1000000'), Decimal('0.0001')):
            sqrt_price_x96 = sqrt_price_for(target)
            computed = h.sqrt_price_to_usdc_per_weth(sqrt_price_x96)
            relative_error = abs(computed - target) / target
            self.assertLess(relative_error, Decimal('1e-9'))

    def test_unit_ratio_gives_scale_adjustment_only(self):
        # sqrtPriceX96 == Q96 means raw ratio == 1; human price is 10**(18-6).
        self.assertEqual(h.sqrt_price_to_usdc_per_weth(h.Q96), Decimal(10) ** 12)

    def test_invalid_sqrt_price_rejected(self):
        for bad in (0, -1, 1 << 160):
            with self.assertRaises(h.StudyError):
                h.sqrt_price_to_usdc_per_weth(bad)

    def test_decode_sqrt_price_from_swap_log_layout(self):
        log = swap_log(100, 0, 12345)
        self.assertEqual(h.decode_sqrt_price_x96(log['data']), 12345)

    def test_decode_rejects_wrong_length_data(self):
        with self.assertRaises(h.StudyError):
            h.decode_sqrt_price_x96('0x1234')


class GapMathTests(unittest.TestCase):
    def test_gross_gap_is_symmetric_and_zero_when_equal(self):
        self.assertEqual(h.gross_gap_bps(Decimal(2000), Decimal(2000)), 0)
        a = h.gross_gap_bps(Decimal(2000), Decimal(2010))
        b = h.gross_gap_bps(Decimal(2010), Decimal(2000))
        self.assertEqual(a, b)
        self.assertAlmostEqual(float(a), float((Decimal(10) / Decimal(2000)) * 10000), places=6)

    def test_gross_gap_rejects_non_positive_price(self):
        with self.assertRaises(h.StudyError):
            h.gross_gap_bps(Decimal(0), Decimal(1))

    def test_fee_bps_converts_ppm_to_bps(self):
        self.assertEqual(h.fee_bps(500), Decimal(5))
        self.assertEqual(h.fee_bps(3000), Decimal(30))

    def test_net_gap_subtracts_both_pool_fees(self):
        gross = h.gross_gap_bps(Decimal(2000), Decimal(2010))
        net = h.net_gap_bps(Decimal(2000), Decimal(2010), 500, 3000)
        self.assertEqual(net, gross - Decimal(5) - Decimal(30))

    def test_weighted_percentile_picks_expected_bucket(self):
        values = [(Decimal(10), 1), (Decimal(20), 1), (Decimal(30), 8)]
        self.assertEqual(h.weighted_percentile(values, 50), Decimal(30))
        self.assertEqual(h.weighted_percentile([], 50), None)


class CarryForwardTests(unittest.TestCase):
    def test_price_carried_forward_until_next_swap_in_either_pool(self):
        # Pool A swaps at block 10 and 30; pool B swaps at block 20.
        # The A/B price pair should therefore change (split into a new run)
        # at blocks 20 and 30, carrying forward between events.
        events = [
            (10, 0, POOL_A, Decimal(2000)),
            (20, 0, POOL_B, Decimal(2010)),
            (30, 0, POOL_A, Decimal(2020)),
        ]
        runs, open_start, open_prices = h.pair_runs(events, [POOL_A, POOL_B])
        pair = tuple(sorted([POOL_A, POOL_B]))
        h.close_runs(runs, open_start, open_prices, to_block=40)
        self.assertEqual(runs[pair], [
            (20, 29, Decimal(2000), Decimal(2010)),
            (30, 40, Decimal(2020), Decimal(2010)),
        ])

    def test_pair_with_only_one_priced_pool_has_no_runs(self):
        events = [(10, 0, POOL_A, Decimal(2000))]
        runs, open_start, open_prices = h.pair_runs(events, [POOL_A, POOL_B])
        h.close_runs(runs, open_start, open_prices, to_block=20)
        pair = tuple(sorted([POOL_A, POOL_B]))
        self.assertEqual(runs[pair], [])

    def test_three_pools_produce_all_pairwise_combinations(self):
        events = [
            (10, 0, POOL_A, Decimal(2000)),
            (10, 1, POOL_B, Decimal(2005)),
            (10, 2, POOL_C, Decimal(1995)),
        ]
        runs, open_start, open_prices = h.pair_runs(events, [POOL_A, POOL_B, POOL_C])
        h.close_runs(runs, open_start, open_prices, to_block=50)
        self.assertEqual(set(runs.keys()), {
            tuple(sorted([POOL_A, POOL_B])),
            tuple(sorted([POOL_A, POOL_C])),
            tuple(sorted([POOL_B, POOL_C])),
        })
        for pair_runs_list in runs.values():
            self.assertEqual(len(pair_runs_list), 1)

    def test_summarize_pair_reports_persistence_and_percentiles(self):
        # net gap starts positive (episode), then drops to zero-or-below, ending the episode.
        pair_run_list = [
            (0, 899, Decimal(2000), Decimal(2050)),   # net well above 0 for 900 blocks
            (900, 1799, Decimal(2000), Decimal(2000)),  # net == 0, episode ends
        ]
        summary = h.summarize_pair(pair_run_list, 500, 500, from_block=0)
        self.assertEqual(summary['episode_count'], 1)
        self.assertEqual(summary['persistence_max_blocks'], 900)
        self.assertIsNotNone(summary['p50_bps'])
        self.assertGreater(summary['avg_blocks_per_hour_gt0'], 0)


class RangeHalvingTests(unittest.TestCase):
    def test_halves_span_until_provider_accepts_and_still_collects_all_logs(self):
        logs_by_block = {5: [swap_log(5, 0, 12345)], 15: [swap_log(15, 0, 54321)]}
        rpc = FakeLogsRpc(max_span=8, logs_by_block=logs_by_block)
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            collected = []
            fetched = h.collect_pool_swaps(rpc, POOL_A, 0, 19, out_dir, initial_span=20,
                                            sink=collected.append)
            self.assertEqual(fetched, 2)
            self.assertEqual(len(collected), 2)
            # Every accepted request must have respected the provider's span limit,
            # and at least one request must have been rejected and halved.
            self.assertGreater(len(rpc.requested_ranges), len(rpc.accepted_ranges))
            for from_block, to_block in rpc.accepted_ranges:
                self.assertLessEqual(to_block - from_block + 1, 8)
            checkpoint = json.loads(h.checkpoint_path(out_dir, POOL_A).read_text())
            self.assertEqual(checkpoint['next_block'], 20)

    def test_non_range_error_is_not_halved_away(self):
        class AlwaysFails:
            def call(self, method, params):
                raise h.RpcCallError({'code': -32000, 'message': 'execution reverted'})

        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(h.RpcCallError):
                h.collect_pool_swaps(AlwaysFails(), POOL_A, 0, 9, Path(tmp), initial_span=10)


class CheckpointResumeTests(unittest.TestCase):
    def test_second_run_resumes_from_checkpoint_and_never_refetches(self):
        logs_by_block = {2: [swap_log(2, 0, 111)], 12: [swap_log(12, 0, 222)]}
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            rpc1 = FakeLogsRpc(max_span=1000, logs_by_block=logs_by_block)
            first_pass = []
            h.collect_pool_swaps(rpc1, POOL_A, 0, 9, out_dir, initial_span=1000, sink=first_pass.append)
            self.assertEqual(len(first_pass), 1)
            checkpoint = json.loads(h.checkpoint_path(out_dir, POOL_A).read_text())
            self.assertEqual(checkpoint['next_block'], 10)

            class RefusesOldRange:
                def __init__(self):
                    self.requested_ranges = []

                def call(self, method, params):
                    request = params[0]
                    from_block = int(request['fromBlock'], 16)
                    self.requested_ranges.append(from_block)
                    assert from_block >= 10, 'must not refetch a checkpointed range'
                    to_block = int(request['toBlock'], 16)
                    rows = []
                    for block in range(from_block, to_block + 1):
                        rows.extend(logs_by_block.get(block, []))
                    return rows

            rpc2 = RefusesOldRange()
            second_pass = []
            h.collect_pool_swaps(rpc2, POOL_A, 0, 19, out_dir, initial_span=1000, sink=second_pass.append)
            self.assertEqual(len(second_pass), 1)
            self.assertEqual(rpc2.requested_ranges, [10])

    def test_checkpoint_pool_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp)
            h.checkpoint_path(out_dir, POOL_A).write_text(json.dumps({'pool': POOL_B, 'next_block': 5}))
            with self.assertRaises(h.StudyError):
                h.load_checkpoint(out_dir, POOL_A, 0)


class PoolDiscoveryTests(unittest.TestCase):
    def test_skips_zero_address_and_records_found_pools(self):
        rpc = FakeFactoryRpc({500: POOL_A, 3000: POOL_B})
        pools = h.discover_pools(rpc)
        self.assertEqual(pools, {500: POOL_A.lower(), 3000: POOL_B.lower()})
        self.assertEqual(len(rpc.calls), len(h.FEES))


class BlockRangeTests(unittest.TestCase):
    def test_explicit_range_is_used_as_is(self):
        self.assertEqual(h.resolve_block_range(None, 10, 20, days=14), (10, 20))

    def test_explicit_range_rejects_inverted_bounds(self):
        with self.assertRaises(h.StudyError):
            h.resolve_block_range(None, 20, 10, days=14)


class CliSafetyTests(unittest.TestCase):
    def test_default_cli_has_no_network_side_effect(self):
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(h.main([]), 0)
        self.assertEqual(json.loads(output.getvalue())['status'], 'NOT_RUN')

    def test_pacing_floor_is_enforced(self):
        with self.assertRaises(h.StudyError):
            h.Rpc('https://example.invalid', min_interval_ms=1)


if __name__ == '__main__':
    unittest.main()
