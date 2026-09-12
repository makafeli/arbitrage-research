#!/usr/bin/env python3
"""Small read-only provider probe. No retries, secrets, wallets or broadcast calls.
Outputs an evidence file, not a production registry or a market capture bundle.
"""
import concurrent.futures
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import time
import urllib.error
import urllib.request

MAX_BYTES = 2 * 1024 * 1024
MAX_REQUESTS = 5
TIMEOUT_SECONDS = 10
ENDPOINTS = {"base": "https://mainnet.base.org", "solana": "https://api.mainnet.solana.com"}
FACTORY = "0x33128a8fc17869897dce68ed026d694621f6fdfd"
BASE_WETH = "0x4200000000000000000000000000000000000006"
BASE_USDC = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
WHIRLPOOL = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
SOL_USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
WSOL = "So11111111111111111111111111111111111111112"
ALLOWED = {"eth_chainId", "eth_getBlockByNumber", "eth_getCode", "eth_call", "getGenesisHash", "getMultipleAccounts", "getProgramAccounts"}

class NoRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def rpc(chain, method, params, sequence):
    assert method in ALLOWED and sequence < MAX_REQUESTS
    request = {"jsonrpc": "2.0", "id": sequence, "method": method, "params": params}
    started = datetime.now(timezone.utc).isoformat()
    tick = time.monotonic()
    result = {"request": request, "started_at_utc": started}
    try:
        req = urllib.request.Request(ENDPOINTS[chain], data=json.dumps(request).encode(), headers={"Content-Type": "application/json"}, method="POST")
        with urllib.request.build_opener(NoRedirects()).open(req, timeout=TIMEOUT_SECONDS) as response:
            body = response.read(MAX_BYTES + 1)
            result["http_status"] = response.status
        if len(body) > MAX_BYTES:
            raise ValueError("byte-quota")
        parsed = json.loads(body)
        result["response_sha256"] = "sha256:" + hashlib.sha256(body).hexdigest()
        result["response_body"] = body.decode("utf-8")
        if parsed.get("id") != sequence or parsed.get("jsonrpc") != "2.0" or "result" not in parsed or "error" in parsed:
            result["outcome"] = "rpc-rejected-or-invalid-response"
        else:
            result["outcome"] = "success"
            result["result"] = parsed["result"]
    except urllib.error.HTTPError as error:
        result.update(outcome="http-rejected", http_status=error.code)
    except Exception as error:
        result.update(outcome="transport-or-decode-failed", error_type=type(error).__name__)
    result["elapsed_ms"] = round((time.monotonic() - tick) * 1000, 3)
    return result


def main():
    report = {"schema_version": 1, "purpose": "bounded-public-read-only-qualification-probe", "started_at_utc": datetime.now(timezone.utc).isoformat(), "budget": {"paid_spend": 0, "max_requests_per_chain": MAX_REQUESTS, "max_response_bytes": MAX_BYTES, "request_timeout_seconds": TIMEOUT_SECONDS, "retries": 0}, "providers": {chain: {"public_documented_endpoint": endpoint, "calls": [], "production_qualified": False, "eligible_pools": []} for chain, endpoint in ENDPOINTS.items()}}
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        futures = {chain: executor.submit(rpc, chain, method, [], 0) for chain, method in [("base", "eth_chainId"), ("solana", "getGenesisHash")]}
        for chain, future in futures.items():
            report["providers"][chain]["calls"].append(future.result())
    if any(p["calls"][0]["outcome"] != "success" for p in report["providers"].values()):
        report["stopped_reason"] = "Initial public RPC request failed or was blocked; no alternate endpoints, retries or follow-up calls attempted."
    elif report["providers"]["base"]["calls"][0]["result"] != "0x2105":
        report["stopped_reason"] = "Base returned unexpected chain identity."
    else:
        base = report["providers"]["base"]["calls"]
        base.append(rpc("base", "eth_getBlockByNumber", ["finalized", False], len(base)))
        if base[-1]["outcome"] == "success" and isinstance(base[-1]["result"], dict) and isinstance(base[-1]["result"].get("hash"), str):
            context = {"blockHash": base[-1]["result"]["hash"], "requireCanonical": True}
            base.append(rpc("base", "eth_getCode", [FACTORY, context], len(base)))
            if base[-1]["outcome"] == "success":
                for fee in [500, 3000]:
                    calldata = "0x1698ee82" + BASE_WETH[2:].rjust(64, "0") + BASE_USDC[2:].rjust(64, "0") + hex(fee)[2:].rjust(64, "0")
                    base.append(rpc("base", "eth_call", [{"to": FACTORY, "data": calldata}, context], len(base)))
                    if base[-1]["outcome"] != "success":
                        break
        sol = report["providers"]["solana"]["calls"]
        sol.append(rpc("solana", "getMultipleAccounts", [[WHIRLPOOL, WSOL, SOL_USDC], {"encoding": "base64", "commitment": "finalized"}], len(sol)))
        if sol[-1]["outcome"] == "success":
            filters = [{"dataSize": 653}, {"memcmp": {"offset": 101, "bytes": WSOL}}, {"memcmp": {"offset": 181, "bytes": SOL_USDC}}]
            sol.append(rpc("solana", "getProgramAccounts", [WHIRLPOOL, {"encoding": "base64", "commitment": "finalized", "withContext": True, "dataSlice": {"offset": 0, "length": 245}, "filters": filters}], len(sol)))
        report["stopped_reason"] = "Bounded sampling completed. Successful pings or pool addresses do not qualify production providers or executable pools."
    report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    stamp = datetime.fromisoformat(report["started_at_utc"]).strftime("%Y-%m-%dT%H-%M-%S-%fZ")
    path = Path(__file__).with_name(f"public-rpc-probe-{stamp}.json")
    with path.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"path": str(path), "providers": {chain: {"calls": len(p["calls"]), "outcomes": [c["outcome"] for c in p["calls"]]} for chain, p in report["providers"].items()}, "stopped_reason": report["stopped_reason"]}))

if __name__ == "__main__":
    main()
