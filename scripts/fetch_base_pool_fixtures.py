#!/usr/bin/env python3
"""One-time, read-only fetch of real Uniswap v3 pool state on Base for the
offline forge harness (ARB-028, issue #42).

For each pool it records, at ONE finalized block: the runtime code, the
immutables (token0/token1/fee/tickSpacing), storage slots 0-4 (slot0,
feeGrowthGlobal0/1X128, protocolFees, liquidity), the three tickBitmap words
around the current tick, every initialized tick in those words (four slots
each), and the observations a swap reads or writes. Everything is stored as
raw `slot -> value` pairs so a forge test can replay it with `vm.etch` and
`vm.store`. Provenance: host, chain id, block number/hash/timestamp, sha256.

The RPC URL is an argument only; nothing here signs, sends or writes on-chain.
Standard library only (keccak-256 is implemented below because hashlib has
sha3, not keccak).

Usage:
    fetch_base_pool_fixtures.py <rpc_url> <pool> <pool> [--out DIR]
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import sys
import time
import urllib.parse
import urllib.request

BASE_CHAIN_ID = 8453
SLOT_TICKS, SLOT_TICK_BITMAP, SLOT_OBSERVATIONS = 5, 6, 8
SELECTORS = {"token0": "0dfe1681", "token1": "d21220a7", "fee": "ddca3f43", "tickSpacing": "d0c93a7c"}
BITMAP_WINDOW = (-1, 0, 1)

# --- keccak-256 (FIPS 202 permutation, original Keccak padding 0x01) ---------
_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
]
_ROT = [[0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61], [28, 55, 25, 21, 56], [27, 20, 39, 8, 14]]
_M = (1 << 64) - 1


def _rol(v: int, n: int) -> int:
    return ((v << n) | (v >> (64 - n))) & _M


def _keccak_f(a: list[list[int]]) -> None:
    for rc in _RC:
        c = [a[x][0] ^ a[x][1] ^ a[x][2] ^ a[x][3] ^ a[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ _rol(c[(x + 1) % 5], 1) for x in range(5)]
        a[:] = [[a[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                b[y][(2 * x + 3 * y) % 5] = _rol(a[x][y], _ROT[x][y])
        a[:] = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)] for x in range(5)]
        a[0][0] ^= rc


def keccak256(data: bytes) -> bytes:
    rate = 136
    padded = bytearray(data) + b"\x01" + b"\x00" * (rate - 1 - len(data) % rate)
    padded[-1] |= 0x80
    a = [[0] * 5 for _ in range(5)]
    for off in range(0, len(padded), rate):
        for i in range(rate // 8):
            a[i % 5][i // 5] ^= int.from_bytes(padded[off + 8 * i : off + 8 * i + 8], "little")
        _keccak_f(a)
    return b"".join(a[i % 5][i // 5].to_bytes(8, "little") for i in range(4))


assert keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"


# --- helpers ------------------------------------------------------------------
def word(value: int) -> bytes:
    return (value % (1 << 256)).to_bytes(32, "big")


def mapping_slot(key: int, base: int) -> int:
    return int.from_bytes(keccak256(word(key) + word(base)), "big")


def to_int(value: int, bits: int) -> int:
    return value - (1 << bits) if value >= 1 << (bits - 1) else value


def decode_slot0(value: int) -> dict:
    return {
        "sqrt_price_x96": str(value & ((1 << 160) - 1)),
        "tick": to_int((value >> 160) & 0xFFFFFF, 24),
        "observation_index": (value >> 184) & 0xFFFF,
        "observation_cardinality": (value >> 200) & 0xFFFF,
        "observation_cardinality_next": (value >> 216) & 0xFFFF,
        "fee_protocol": (value >> 232) & 0xFF,
        "unlocked": bool((value >> 240) & 1),
    }


class Rpc:
    def __init__(self, url: str):
        self.url, self.seq = url, 0

    def _post(self, payload) -> object:
        req = urllib.request.Request(self.url, json.dumps(payload).encode(), {"Content-Type": "application/json", "User-Agent": "arbitrage-research-fixture-fetch/1"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read())

    def call(self, method: str, params: list):
        self.seq += 1
        reply = self._post({"jsonrpc": "2.0", "id": self.seq, "method": method, "params": params})
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']}")
        return reply["result"]

    def storage(self, address: str, slots: list[int], block: str) -> dict[int, int]:
        out: dict[int, int] = {}
        for i in range(0, len(slots), 10):  # public Base RPC: max 10 calls per batch
            chunk = slots[i : i + 10]
            batch = [
                {"jsonrpc": "2.0", "id": n, "method": "eth_getStorageAt", "params": [address, hex(s), block]}
                for n, s in enumerate(chunk)
            ]
            for attempt in range(6):
                replies = self._post(batch)
                if not isinstance(replies, list):
                    raise RuntimeError(f"batch rejected: {json.dumps(replies)[:300]}")
                by_id = {r["id"]: r for r in replies}
                errors = [r["error"] for r in replies if "error" in r]
                if not errors:
                    break
                if any(e.get("code") != -32016 for e in errors) or attempt == 5:
                    raise RuntimeError(f"eth_getStorageAt: {errors[0]}")
                time.sleep(2 * (attempt + 1))  # public RPC rate limit: back off and retry the chunk
            for n, s in enumerate(chunk):
                out[s] = int(by_id[n]["result"], 16)
            time.sleep(0.25)
        return out


def fetch_pool(rpc: Rpc, pool: str, block: str) -> dict:
    code = rpc.call("eth_getCode", [pool, block])
    if code in ("0x", ""):
        raise SystemExit(f"no code at {pool}")
    imm = {}
    for name, sel in SELECTORS.items():
        raw = rpc.call("eth_call", [{"to": pool, "data": "0x" + sel}, block])
        if not (isinstance(raw, str) and len(raw) == 66 and raw.startswith("0x")):
            raise SystemExit(f"eth_call {name} on {pool}: expected one 32-byte word, got {raw!r}")
        imm[name] = "0x" + raw[-40:] if name.startswith("token") else int(raw, 16)
    spacing = imm["tickSpacing"]
    base_slots = rpc.storage(pool, [0, 1, 2, 3, 4], block)
    slot0 = decode_slot0(base_slots[0])
    compressed = slot0["tick"] // spacing
    word_positions = [(compressed >> 8) + w for w in BITMAP_WINDOW]
    bitmap_slots = {mapping_slot(wp, SLOT_TICK_BITMAP): wp for wp in word_positions}
    bitmap = rpc.storage(pool, list(bitmap_slots), block)
    ticks: list[int] = []
    for slot, wp in bitmap_slots.items():
        bits = bitmap[slot]
        ticks.extend((wp * 256 + bit) * spacing for bit in range(256) if bits >> bit & 1)
    ticks.sort()
    tick_slots = {mapping_slot(t, SLOT_TICKS) + i: (t, i) for t in ticks for i in range(4)}
    card, card_next, idx = (
        slot0["observation_cardinality"],
        slot0["observation_cardinality_next"],
        slot0["observation_index"],
    )
    obs_indices = sorted({idx, (idx + 1) % max(card, 1), (idx + 1) % max(card_next, 1)})
    obs_slots = {SLOT_OBSERVATIONS + i: i for i in obs_indices}
    values = rpc.storage(pool, list(tick_slots) + list(obs_slots), block)
    storage = {**base_slots, **bitmap, **values}
    return {
        "address": pool,
        "code": code,
        "code_sha256": hashlib.sha256(bytes.fromhex(code[2:])).hexdigest(),
        "immutables": imm,
        "slot0": slot0,
        "liquidity": str(base_slots[4]),
        "tick_bitmap_words": {str(wp): hex(bitmap[s]) for s, wp in bitmap_slots.items()},
        "initialized_ticks": ticks,
        "observation_indices": obs_indices,
        "storage": {hex(s): hex(v) for s, v in sorted(storage.items())},
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("rpc_url")
    ap.add_argument("pools", nargs=2)
    ap.add_argument("--out", default="contracts/base-guard/test/fixtures/mainnet")
    args = ap.parse_args(argv)
    rpc = Rpc(args.rpc_url)
    if int(rpc.call("eth_chainId", []), 16) != BASE_CHAIN_ID:
        raise SystemExit("not Base mainnet")
    head = rpc.call("eth_getBlockByNumber", ["finalized", False])
    block = head["number"]
    pools = [fetch_pool(rpc, p.lower(), block) for p in args.pools]
    if {p["immutables"]["token0"] for p in pools} | {p["immutables"]["token1"] for p in pools} != {
        pools[0]["immutables"]["token0"], pools[0]["immutables"]["token1"]
    }:
        raise SystemExit("the two pools must share token0 and token1")
    fixture = {
        "schema_version": 1,
        "purpose": "ARB-028 offline forge fixtures: real Base Uniswap v3 pool state, research-only, never a deployment",
        "provenance": {
            "fetched_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
            "rpc_host": urllib.parse.urlsplit(args.rpc_url).hostname,
            "chain_id": BASE_CHAIN_ID,
            "block_number": int(block, 16),
            "block_hash": head["hash"],
            "block_timestamp": int(head["timestamp"], 16),
            "block_tag": "finalized",
            "fetch_script": "scripts/fetch_base_pool_fixtures.py",
        },
        "pools": pools,
    }
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "pools.json").write_text(json.dumps(fixture, indent=2) + "\n")
    print(json.dumps({
        "block": int(block, 16),
        "pools": [{"address": p["address"], "tick": p["slot0"]["tick"], "ticks": len(p["initialized_ticks"]), "slots": len(p["storage"])} for p in pools],
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
