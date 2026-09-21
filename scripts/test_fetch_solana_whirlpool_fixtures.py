#!/usr/bin/env python3
"""Offline checks for fetch_solana_whirlpool_fixtures.py against the committed
mainnet fixture: PDA derivation, Whirlpool field offsets and file hashes must
reproduce what was recorded. No network."""
import base64
import hashlib
import importlib.util
import json
import pathlib
import struct
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

    def test_elf_len_takes_the_farthest_file_backed_end(self):
        # Synthetic ELF64 header: 1 program header, 3 section headers. The NOBITS section
        # claims the largest range and must be ignored; the PROGBITS section wins.
        e_phoff, e_shoff = 64, 120
        hdr = bytearray(b"\x7fELF\x02\x01\x01" + b"\0" * 57)
        struct.pack_into("<QQ", hdr, 32, e_phoff, e_shoff)
        struct.pack_into("<HHHH", hdr, 54, 56, 1, 64, 3)
        prog = bytearray(56)
        struct.pack_into("<QQQQ", prog, 8, 400, 0, 0, 50)  # segment file range 400..450
        sect = bytearray(64 * 3)
        struct.pack_into("<I", sect, 64 + 4, 8)  # NOBITS: ignored
        struct.pack_into("<QQ", sect, 64 + 24, 9_000, 9_000)
        struct.pack_into("<I", sect, 128 + 4, 1)  # PROGBITS at 500..600
        struct.pack_into("<QQ", sect, 128 + 24, 500, 100)
        self.assertEqual(fetch.elf_len(bytes(hdr + prog + sect)), 600)
        # Without file-backed ranges the section table's own end (120 + 3 * 64) wins.
        struct.pack_into("<QQ", sect, 128 + 24, 0, 0)
        struct.pack_into("<QQQQ", prog, 8, 0, 0, 0, 0)
        self.assertEqual(fetch.elf_len(bytes(hdr + prog + sect)), 312)
        with self.assertRaises(AssertionError):
            fetch.elf_len(b"\x7fELF\x02\x02" + bytes(58))  # big-endian

    def test_hashes_match_the_identity_registry(self):
        registry = json.loads((ROOT / "docs/registries/initial-identities.json").read_text())
        chain = next(c for c in registry["chains"] if c["network_id"] == "solana-mainnet")
        venue = chain["venue"]
        prov = self.fixture["provenance"]
        self.assertEqual(venue["program_data"]["address"], prov["programdata"])
        self.assertEqual("sha256:" + prov["programdata_sha256"], venue["observed_program_data_sha256"])
        self.assertEqual("sha256:" + prov["elf_sha256"], venue["observed_elf_sha256"])
        self.assertEqual(str(prov["programdata_last_deployed_slot"]), venue["last_upgrade_slot"])
        self.assertEqual(set(self.fixture["pools"]), {p["identity"]["address"] for p in chain["pools"]})

    def test_tick_array_records_belong_to_their_pool_and_start(self):
        # Replays the fetch-time self-check offline: swapping two records' bytes keeps
        # every hash consistent but breaks the start index / pool binding.
        disc = hashlib.sha256(b"account:TickArray").digest()[:8]
        for pool, info in self.fixture["pools"].items():
            for start, addr in zip(info["tick_array_starts"], info["tick_arrays"]):
                rec = self.accounts[addr]
                if not rec["exists"]:
                    continue
                data = base64.b64decode(rec["data_base64"])
                self.assertEqual(rec["owner"], fetch.WHIRLPOOL_PROGRAM)
                self.assertEqual(len(data), fetch.TICK_ARRAY_LEN)
                self.assertEqual(data[:8], disc)
                self.assertEqual(int.from_bytes(data[8:12], "little", signed=True), start)
                self.assertEqual(fetch.b58encode(data[-32:]), pool)

    def test_so_file_is_the_complete_elf_image(self):
        # A loader reads the section-header table by offset; a file cut short by even
        # one trailing zero byte fails with "Offset or value is out of bounds".
        elf = (FIXTURES / "whirlpool-program.so").read_bytes()
        prov = self.fixture["provenance"]
        self.assertEqual(fetch.elf_len(elf), len(elf))
        padded = elf + b"\0" * prov["padding_stripped_bytes"]
        self.assertEqual(hashlib.sha256(padded).hexdigest(), prov["elf_sha256"])


if __name__ == "__main__":
    unittest.main()
