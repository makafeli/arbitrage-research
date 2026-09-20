#!/usr/bin/env python3
"""Offline checks for fetch_solana_whirlpool_fixtures.py against the committed
mainnet fixture: PDA derivation, Whirlpool field offsets and file hashes must
reproduce what was recorded. No network."""
import base64
import hashlib
import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "crates/arb-solana-harness/tests/fixtures/mainnet"
spec = importlib.util.spec_from_file_location("fetch", ROOT / "scripts/fetch_solana_whirlpool_fixtures.py")
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class FixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads((FIXTURES / "accounts.json").read_text())
        cls.accounts = {a["pubkey"]: a for a in cls.fixture["accounts"]}

    def test_base58_round_trip_and_on_curve(self):
        for key in (fetch.WHIRLPOOL_PROGRAM, "So11111111111111111111111111111111111111112"):
            self.assertEqual(fetch.b58encode(fetch.b58decode(key)), key)
            self.assertTrue(fetch.on_curve(fetch.b58decode(key)), key)
        with self.assertRaises(ValueError):
            fetch.b58decode("0")

    def test_pda_derivation_reproduces_recorded_tick_arrays_and_oracles(self):
        program = fetch.b58decode(fetch.WHIRLPOOL_PROGRAM)
        for pool, info in self.fixture["pools"].items():
            key = fetch.b58decode(pool)
            for start, addr in zip(info["tick_array_starts"], info["tick_arrays"]):
                pda = fetch.find_pda([b"tick_array", key, str(start).encode()], program)
                self.assertEqual(fetch.b58encode(pda), addr)
                self.assertFalse(fetch.on_curve(pda))
            self.assertEqual(fetch.b58encode(fetch.find_pda([b"oracle", key], program)), info["oracle"])

    def test_pool_fields_match_recorded_account_bytes(self):
        for pool, info in self.fixture["pools"].items():
            data = base64.b64decode(self.accounts[pool]["data_base64"])
            self.assertEqual(len(data), fetch.WHIRLPOOL_LEN)
            self.assertEqual(int.from_bytes(data[41:43], "little"), info["tick_spacing"])
            self.assertEqual(int.from_bytes(data[81:85], "little", signed=True), info["tick_current_index"])
            self.assertEqual(fetch.b58encode(data[101:133]), info["mint_a"])
            self.assertEqual(fetch.b58encode(data[133:165]), info["vault_a"])
            self.assertEqual(fetch.b58encode(data[181:213]), info["mint_b"])
            self.assertEqual(fetch.b58encode(data[213:245]), info["vault_b"])
            span = info["tick_spacing"] * fetch.TICK_ARRAY_SIZE
            self.assertEqual(info["tick_array_starts"][2], (info["tick_current_index"] // span) * span)
            for addr in (info["vault_a"], info["vault_b"]):
                self.assertEqual(self.accounts[addr]["owner"], "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")

    def test_recorded_hashes_and_elf_match(self):
        for rec in self.fixture["accounts"]:
            if rec["exists"]:
                data = base64.b64decode(rec["data_base64"])
                self.assertEqual(len(data), rec["data_len"])
                self.assertEqual(hashlib.sha256(data).hexdigest(), rec["data_sha256"])
        elf = (FIXTURES / "whirlpool-program.so").read_bytes()
        prov = self.fixture["provenance"]
        self.assertEqual(elf[:4], b"\x7fELF")
        self.assertEqual(len(elf), prov["so_file_len"])
        self.assertEqual(hashlib.sha256(elf).hexdigest(), prov["so_file_sha256"])
        self.assertEqual(prov["genesis_hash"], fetch.MAINNET_GENESIS)


if __name__ == "__main__":
    unittest.main()
