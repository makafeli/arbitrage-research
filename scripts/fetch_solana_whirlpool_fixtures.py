#!/usr/bin/env python3
"""One-time, read-only fetch of real Orca Whirlpool state for the offline
litesvm harness (ARB-029, issue #43).

Records the Whirlpool program ELF plus every account two whirlpools need for
a `swap` in either direction (pool, vaults, mints, five tick arrays around the
current tick, oracle PDA) from ONE finalized getMultipleAccounts response, and
writes them with provenance (host, genesis hash, context slot, sha256).

The RPC URL is an argument only; nothing here signs, sends or writes on-chain.
Standard library only.

Usage:
    fetch_solana_whirlpool_fixtures.py <rpc_url> <whirlpool> <whirlpool> [--out DIR]
"""
from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import json
import pathlib
import struct
import sys
import urllib.parse
import urllib.request

MAINNET_GENESIS = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
WHIRLPOOL_PROGRAM = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
TICK_ARRAY_SIZE = 88
TICK_ARRAY_LEN = 8 + 4 + TICK_ARRAY_SIZE * 113 + 32  # 9988
WHIRLPOOL_LEN = 653
PROGRAMDATA_HEADER = 4 + 8 + 1 + 32  # variant, slot, Option<upgrade authority>
ALPHABET = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
P = 2**255 - 19
D = (-121665 * pow(121666, P - 2, P)) % P


def b58decode(s: str) -> bytes:
    n = 0
    for c in s.encode():
        n = n * 58 + ALPHABET.index(c)
    raw = n.to_bytes(32, "big")
    if b58encode(raw) != s:
        raise ValueError(f"not a 32-byte base58 key: {s}")
    return raw


def b58encode(raw: bytes) -> str:
    n = int.from_bytes(raw, "big")
    out = bytearray()
    while n:
        n, r = divmod(n, 58)
        out.append(ALPHABET[r])
    out.extend(ALPHABET[0:1] * (len(raw) - len(raw.lstrip(b"\0"))))
    return out[::-1].decode()


def on_curve(raw: bytes) -> bool:
    y = int.from_bytes(raw, "little")
    sign, y = y >> 255, y & ((1 << 255) - 1)
    if y >= P:
        return False
    x2 = (y * y - 1) * pow(D * y * y + 1, P - 2, P) % P
    x = pow(x2, (P + 3) // 8, P)
    if (x * x - x2) % P:
        x = x * pow(2, (P - 1) // 4, P) % P
    if (x * x - x2) % P:
        return False
    return not (x == 0 and sign == 1)


def find_pda(seeds: list[bytes], program: bytes) -> bytes:
    for bump in range(255, -1, -1):
        h = hashlib.sha256(b"".join(seeds) + bytes([bump]) + program + b"ProgramDerivedAddress").digest()
        if not on_curve(h):
            return h
    raise ValueError("no PDA bump")


class Rpc:
    def __init__(self, url: str):
        self.url, self.seq = url, 0

    def call(self, method: str, params: list) -> dict:
        self.seq += 1
        body = json.dumps({"jsonrpc": "2.0", "id": self.seq, "method": method, "params": params}).encode()
        req = urllib.request.Request(self.url, body, {"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            reply = json.loads(resp.read())
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']}")
        return reply["result"]


def elf_len(data: bytes) -> int:
    """Byte length of the ELF64 image declared by its own headers: the end of the
    program-header table, the section-header table and every file-backed segment
    and section. Trailing zero bytes inside that range belong to the file."""
    assert data[:5] == b"\x7fELF\x02", "not an ELF64 image"
    e_phoff, e_shoff = struct.unpack_from("<QQ", data, 32)
    e_phentsize, e_phnum, e_shentsize, e_shnum = struct.unpack_from("<HHHH", data, 54)
    end = max(e_phoff + e_phentsize * e_phnum, e_shoff + e_shentsize * e_shnum)
    for i in range(e_phnum):
        p_offset, _, _, p_filesz = struct.unpack_from("<QQQQ", data, e_phoff + i * e_phentsize + 8)
        end = max(end, p_offset + p_filesz)
    for i in range(e_shnum):
        sh_type = struct.unpack_from("<I", data, e_shoff + i * e_shentsize + 4)[0]
        sh_offset, sh_size = struct.unpack_from("<QQ", data, e_shoff + i * e_shentsize + 24)
        if sh_type != 8:  # SHT_NOBITS occupies no file bytes
            end = max(end, sh_offset + sh_size)
    return end


def account_record(pubkey: str, value: dict | None) -> dict:
    if value is None:
        return {"pubkey": pubkey, "exists": False}
    data = base64.b64decode(value["data"][0])
    return {
        "pubkey": pubkey,
        "exists": True,
        "owner": value["owner"],
        "lamports": value["lamports"],
        "executable": value["executable"],
        "rent_epoch": value["rentEpoch"],
        "data_len": len(data),
        "data_sha256": hashlib.sha256(data).hexdigest(),
        "data_base64": value["data"][0],
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("rpc_url")
    ap.add_argument("whirlpools", nargs=2)
    ap.add_argument("--out", default="crates/arb-solana-harness/tests/fixtures/mainnet")
    args = ap.parse_args(argv)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    rpc = Rpc(args.rpc_url)
    program = b58decode(WHIRLPOOL_PROGRAM)

    genesis = rpc.call("getGenesisHash", [])
    if genesis != MAINNET_GENESIS:
        raise SystemExit(f"genesis mismatch: {genesis}")

    # Program account -> programdata address -> ELF.
    prog = rpc.call("getAccountInfo", [WHIRLPOOL_PROGRAM, {"encoding": "base64"}])["value"]
    prog_data = base64.b64decode(prog["data"][0])
    assert prog["executable"] and prog_data[:4] == b"\x02\x00\x00\x00", "not an upgradeable program account"
    programdata = b58encode(prog_data[4:36])
    pd = rpc.call("getAccountInfo", [programdata, {"encoding": "base64", "commitment": "finalized"}])
    pd_bytes = base64.b64decode(pd["value"]["data"][0])
    assert pd_bytes[:4] == b"\x03\x00\x00\x00", "not a programdata account"
    # Repo convention (inspect_pool_candidates.py): elf_sha256 covers everything after
    # the 45-byte header, padding included. The stored .so drops the zero padding that
    # follows the ELF image; the image length comes from the ELF headers, not from
    # rstrip, because the section-header table can legitimately end in zero bytes.
    elf_padded = pd_bytes[PROGRAMDATA_HEADER:]
    elf = elf_padded[: elf_len(elf_padded)]
    assert elf_padded[len(elf) :].count(0) == len(elf_padded) - len(elf), "non-zero bytes after the ELF image"
    (out / "whirlpool-program.so").write_bytes(elf)

    # Pool accounts (one pass to learn tick spacing / current tick / mints / vaults).
    pools = {}
    for pool in args.whirlpools:
        acc = rpc.call("getAccountInfo", [pool, {"encoding": "base64"}])["value"]
        data = base64.b64decode(acc["data"][0])
        assert acc["owner"] == WHIRLPOOL_PROGRAM and len(data) == WHIRLPOOL_LEN, pool
        spacing = int.from_bytes(data[41:43], "little")
        tick = int.from_bytes(data[81:85], "little", signed=True)
        span = spacing * TICK_ARRAY_SIZE
        start = (tick // span) * span
        starts = [start + i * span for i in range(-2, 3)]
        key = b58decode(pool)
        pools[pool] = {
            "tick_spacing": spacing,
            "tick_current_index": tick,
            "mint_a": b58encode(data[101:133]),
            "vault_a": b58encode(data[133:165]),
            "mint_b": b58encode(data[181:213]),
            "vault_b": b58encode(data[213:245]),
            "tick_array_starts": starts,
            "tick_arrays": [
                b58encode(find_pda([b"tick_array", key, str(s).encode()], program)) for s in starts
            ],
            "oracle": b58encode(find_pda([b"oracle", key], program)),
        }
    mints = {pools[p][m] for p in pools for m in ("mint_a", "mint_b")}
    if len(mints) != 2:
        raise SystemExit(f"the two pools must share both tokens, got mints {sorted(mints)}")

    addresses: list[str] = []
    for pool, info in pools.items():
        for a in [pool, info["vault_a"], info["vault_b"], info["mint_a"], info["mint_b"], *info["tick_arrays"], info["oracle"]]:
            if a not in addresses:
                addresses.append(a)
    batch = rpc.call(
        "getMultipleAccounts", [addresses, {"encoding": "base64", "commitment": "finalized"}]
    )
    records = {a: account_record(a, v) for a, v in zip(addresses, batch["value"])}

    # Self-check: every fetched tick array belongs to its pool at the expected start index,
    # which proves the PDA derivation above.
    tick_disc = hashlib.sha256(b"account:TickArray").digest()[:8]
    for pool, info in pools.items():
        for start, addr in zip(info["tick_array_starts"], info["tick_arrays"]):
            rec = records[addr]
            if not rec["exists"]:
                continue
            data = base64.b64decode(rec["data_base64"])
            assert rec["owner"] == WHIRLPOOL_PROGRAM and len(data) == TICK_ARRAY_LEN, addr
            assert data[:8] == tick_disc, addr
            assert int.from_bytes(data[8:12], "little", signed=True) == start, addr
            assert b58encode(data[-32:]) == pool, addr
        for a in (pool, info["vault_a"], info["vault_b"], info["mint_a"], info["mint_b"]):
            assert records[a]["exists"], f"required account missing: {a}"

    host = urllib.parse.urlsplit(args.rpc_url).hostname
    fixture = {
        "schema_version": 1,
        "purpose": "ARB-029 offline litesvm fixtures: real mainnet Whirlpool state, research-only, never a deployment",
        "provenance": {
            "fetched_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
            "rpc_host": host,
            "genesis_hash": genesis,
            "accounts_context_slot": batch["context"]["slot"],
            "program": WHIRLPOOL_PROGRAM,
            "programdata": programdata,
            "programdata_context_slot": pd["context"]["slot"],
            "programdata_last_deployed_slot": int.from_bytes(pd_bytes[4:12], "little"),
            "programdata_sha256": hashlib.sha256(pd_bytes).hexdigest(),
            "elf_sha256": hashlib.sha256(elf_padded).hexdigest(),
            "so_file_sha256": hashlib.sha256(elf).hexdigest(),
            "so_file_len": len(elf),
            "padding_stripped_bytes": len(elf_padded) - len(elf),
            "fetch_script": "scripts/fetch_solana_whirlpool_fixtures.py",
        },
        "pools": pools,
        "accounts": [records[a] for a in addresses],
    }
    (out / "accounts.json").write_text(json.dumps(fixture, indent=2) + "\n")
    missing = [a for a in addresses if not records[a]["exists"]]
    print(json.dumps({"slot": batch["context"]["slot"], "accounts": len(addresses), "missing": missing, "so_file_len": len(elf)}))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
