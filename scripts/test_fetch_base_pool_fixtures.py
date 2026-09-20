#!/usr/bin/env python3
"""Offline checks for fetch_base_pool_fixtures.py against the committed Base
fixture: keccak-256, storage-slot derivation, slot0 decoding and hashes must
reproduce what was recorded. No network."""
import hashlib
import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "contracts/base-guard/test/fixtures/mainnet/pools.json"
spec = importlib.util.spec_from_file_location("fetch", ROOT / "scripts/fetch_base_pool_fixtures.py")
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class FixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads(FIXTURE.read_text())

    def test_keccak_vectors(self):
        self.assertEqual(fetch.keccak256(b"").hex(), "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
        self.assertEqual(fetch.keccak256(b"abc").hex(), "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45")
        # Two blocks (> 136 bytes) exercise the absorb loop: keccak256 of 136 'a' + one more block.
        self.assertEqual(len(fetch.keccak256(b"a" * 200)), 32)
        self.assertNotEqual(fetch.keccak256(b"a" * 200), fetch.keccak256(b"a" * 199))

    def test_slot_derivation_reproduces_recorded_storage_keys(self):
        for pool in self.fixture["pools"]:
            storage = pool["storage"]
            spacing = pool["immutables"]["tickSpacing"]
            for wp_str in pool["tick_bitmap_words"]:
                self.assertIn(hex(fetch.mapping_slot(int(wp_str), fetch.SLOT_TICK_BITMAP)), storage)
            for tick in pool["initialized_ticks"]:
                self.assertEqual(tick % spacing, 0)
                base = fetch.mapping_slot(tick, fetch.SLOT_TICKS)
                for i in range(4):
                    self.assertIn(hex(base + i), storage, f"tick {tick} slot {i}")
                # liquidityGross (low 128 bits) is non-zero for an initialized tick
                self.assertNotEqual(int(storage[hex(base)], 16) & ((1 << 128) - 1), 0, tick)
            for index in pool["observation_indices"]:
                self.assertIn(hex(fetch.SLOT_OBSERVATIONS + index), storage)

    def test_bitmap_words_enumerate_exactly_the_recorded_ticks(self):
        for pool in self.fixture["pools"]:
            spacing = pool["immutables"]["tickSpacing"]
            ticks = sorted(
                (int(wp) * 256 + bit) * spacing
                for wp, bits in pool["tick_bitmap_words"].items()
                for bit in range(256)
                if int(bits, 16) >> bit & 1
            )
            self.assertEqual(ticks, pool["initialized_ticks"])
            compressed = pool["slot0"]["tick"] // spacing
            self.assertIn(str(compressed >> 8), pool["tick_bitmap_words"])

    def test_slot0_and_liquidity_decode_from_recorded_values(self):
        for pool in self.fixture["pools"]:
            storage = pool["storage"]
            self.assertEqual(fetch.decode_slot0(int(storage["0x0"], 16)), pool["slot0"])
            self.assertEqual(str(int(storage["0x4"], 16)), pool["liquidity"])
            self.assertTrue(pool["slot0"]["unlocked"])

    def test_code_hash_and_provenance(self):
        prov = self.fixture["provenance"]
        self.assertEqual(prov["chain_id"], fetch.BASE_CHAIN_ID)
        self.assertEqual(len(prov["block_hash"]), 66)
        for pool in self.fixture["pools"]:
            code = bytes.fromhex(pool["code"][2:])
            self.assertGreater(len(code), 1000)
            self.assertEqual(hashlib.sha256(code).hexdigest(), pool["code_sha256"])
            self.assertLess(pool["immutables"]["token0"], pool["immutables"]["token1"])


if __name__ == "__main__":
    unittest.main()
