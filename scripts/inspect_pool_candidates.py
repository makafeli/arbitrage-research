#!/usr/bin/env python3
"""Bounded public identity/state observations for ARB-003; no trading or activation."""
from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import time
import urllib.error
import urllib.request

BASE_URL = 'https://base-rpc.publicnode.com'
SOLANA_URL = 'https://api.mainnet.solana.com'
FACTORY = '0x33128a8fc17869897dce68ed026d694621f6fdfd'
WETH = '0x4200000000000000000000000000000000000006'
BASE_USDC = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913'
WHIRLPOOL = 'whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc'
TOKEN = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'
LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111'
GENESIS = '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d'
WSOL = 'So11111111111111111111111111111111111111112'
SOL_USDC = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
MAX_RESPONSE = 8 * 1024 * 1024
MAX_TOTAL = 24 * 1024 * 1024
MAX_REQUESTS = 48
METHODS = {'eth_chainId', 'eth_getBlockByNumber', 'eth_getCode', 'eth_call',
           'getGenesisHash', 'getMultipleAccounts', 'getProgramAccounts'}


class ObservationError(ValueError):
    """Fixed non-sensitive reason; input and endpoint responses are never echoed."""


def require(ok, reason):
    if not ok:
        raise ObservationError(reason)


def sha(raw: bytes) -> str:
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def unique_object(pairs):
    result = {}
    for k, v in pairs:
        require(k not in result, 'DUPLICATE_JSON_KEY')
        result[k] = v
    return result


def strict_json(raw):
    def invalid(_):
        raise ObservationError('NONFINITE_JSON')
    return json.loads(raw, object_pairs_hook=unique_object, parse_constant=invalid)


class NoRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class PublicRpc:
    def __init__(self, network: str):
        require(network in ('base-mainnet', 'solana-mainnet'), 'UNSUPPORTED_NETWORK')
        self.network = network
        self.endpoint = BASE_URL if network == 'base-mainnet' else SOLANA_URL
        self.calls = []
        self.total_bytes = 0
        self.started = time.monotonic()
        self.stopped = False

    def call(self, method: str, params: list):
        require(not self.stopped, 'PROVIDER_ALREADY_STOPPED')
        require(method in METHODS, 'METHOD_NOT_READ_ONLY')
        require(len(self.calls) < MAX_REQUESTS and self.total_bytes < MAX_TOTAL
                and time.monotonic() - self.started < 240, 'REQUEST_BUDGET_EXHAUSTED')
        request = {'jsonrpc': '2.0', 'id': len(self.calls), 'method': method, 'params': params}
        row = {'request': request, 'started_at_utc': utc(), 'outcome': 'NOT_COMPLETED'}
        self.calls.append(row)
        tick = time.monotonic()
        try:
            req = urllib.request.Request(self.endpoint, data=json.dumps(request).encode(),
                                         headers={'Content-Type': 'application/json'}, method='POST')
            with urllib.request.build_opener(NoRedirects()).open(req, timeout=10) as response:
                row['http_status'] = response.status
                cap = min(MAX_RESPONSE, MAX_TOTAL - self.total_bytes)
                raw = response.read(cap + 1)
            self.total_bytes += len(raw)
            require(len(raw) <= cap, 'RESPONSE_BYTE_LIMIT')
            # Retain exact successful and JSON-RPC-error bytes in the temporary
            # artifact. HTTP error bodies are deliberately never read.
            row['response_sha256'] = sha(raw)
            row['response_base64'] = base64.b64encode(raw).decode()
            parsed = strict_json(raw)
            require(isinstance(parsed, dict) and parsed.get('jsonrpc') == '2.0'
                    and type(parsed.get('id')) is int and parsed['id'] == request['id']
                    and 'result' in parsed and 'error' not in parsed, 'RPC_REJECTED_OR_MALFORMED')
            row['outcome'] = 'RPC_RESPONSE'
            return parsed['result']
        except urllib.error.HTTPError as exc:
            row['http_status'] = exc.code
            row['outcome'] = 'HTTP_REFUSED'
            self.stopped = True
            raise ObservationError('HTTP_REFUSED') from None
        except ObservationError as exc:
            row['outcome'] = str(exc)
            self.stopped = True
            raise
        except (OSError, ValueError, TypeError, RecursionError):
            row['outcome'] = 'TRANSPORT_OR_DECODE_FAILED'
            self.stopped = True
            raise ObservationError('TRANSPORT_OR_DECODE_FAILED') from None
        finally:
            row['elapsed_ms'] = round((time.monotonic() - tick) * 1000, 3)
            time.sleep(0.4)


def hex_data(value, length=None):
    require(isinstance(value, str) and len(value) <= MAX_RESPONSE * 2 + 2
            and re.fullmatch(r'0x(?:[0-9a-fA-F]{2})*', value) is not None, 'INVALID_EVM_BYTES')
    data = bytes.fromhex(value[2:])
    require(length is None or len(data) == length, 'INVALID_EVM_LENGTH')
    return data


def uint_word(value, bits=256):
    n = int.from_bytes(hex_data(value, 32), 'big')
    require(n < 1 << bits, 'ABI_INTEGER_OVERFLOW')
    return n


def address_word(value):
    data = hex_data(value, 32)
    require(data[:12] == bytes(12), 'INVALID_ABI_ADDRESS')
    return '0x' + data[12:].hex()


def evm_key(value):
    data = hex_data(value, 20)
    require(any(data), 'ZERO_EVM_ADDRESS')
    return value.lower()


def b58_encode(data: bytes) -> str:
    n = int.from_bytes(data, 'big')
    text = ''
    while n:
        n, r = divmod(n, 58)
        text = B58[r] + text
    return '1' * (len(data) - len(data.lstrip(b'\0'))) + text


def b58_key(value) -> bytes:
    require(isinstance(value, str) and 32 <= len(value) <= 44
            and all(c in B58 for c in value), 'INVALID_SOLANA_KEY')
    n = 0
    for c in value:
        n = n * 58 + B58.index(c)
    raw = bytes(len(value) - len(value.lstrip('1'))) + n.to_bytes((n.bit_length() + 7) // 8, 'big')
    require(len(raw) == 32 and any(raw) and b58_encode(raw) == value, 'INVALID_SOLANA_KEY')
    return raw


def account_bytes(account, owner, executable=False, length=None):
    require(isinstance(account, dict) and account.get('owner') == owner
            and account.get('executable') is executable, 'ACCOUNT_OWNER_OR_EXECUTABLE_MISMATCH')
    value = account.get('data')
    require(isinstance(value, list) and len(value) == 2 and value[1] == 'base64'
            and isinstance(value[0], str) and len(value[0]) <= MAX_RESPONSE, 'INVALID_ACCOUNT_ENCODING')
    try:
        data = base64.b64decode(value[0], validate=True)
    except ValueError:
        raise ObservationError('INVALID_ACCOUNT_ENCODING') from None
    require(base64.b64encode(data).decode() == value[0], 'NONCANONICAL_BASE64')
    require(length is None or len(data) == length, 'UNSUPPORTED_ACCOUNT_LAYOUT')
    return data


def integer(data, start, size, signed=False):
    return int.from_bytes(data[start:start + size], 'little', signed=signed)


def mint(account, decimals):
    data = account_bytes(account, TOKEN, length=82)
    require(data[44] == decimals and data[45] == 1
            and integer(data, 0, 4) in (0, 1) and integer(data, 46, 4) in (0, 1), 'UNSUPPORTED_MINT')
    return {'decimals': decimals, 'supply_minor': str(integer(data, 36, 8)),
            'mint_authority': b58_encode(data[4:36]) if integer(data, 0, 4) else None,
            'freeze_authority': b58_encode(data[50:82]) if integer(data, 46, 4) else None,
            'token_program': TOKEN, 'extensions_supported': False, 'account_sha256': sha(data)}


def whirlpool(account):
    data = account_bytes(account, WHIRLPOOL, length=653)
    require(data[:8] == hashlib.sha256(b'account:Whirlpool').digest()[:8], 'INVALID_POOL_DISCRIMINATOR')
    spacing, fee_tier = integer(data, 41, 2), integer(data, 43, 2)
    tick, price = integer(data, 81, 4, True), integer(data, 65, 16)
    require(spacing > 0 and -443636 <= tick <= 443636 and price > 0, 'INVALID_POOL_STATE')
    ma, mb = b58_encode(data[101:133]), b58_encode(data[181:213])
    require((ma, mb) == (WSOL, SOL_USDC), 'UNEXPECTED_POOL_PAIR')
    return {'whirlpools_config': b58_encode(data[8:40]), 'tick_spacing': spacing,
            'fee_tier_index': fee_tier, 'fee_rate_millionths': integer(data, 45, 2),
            'protocol_fee_rate': integer(data, 47, 2), 'liquidity': str(integer(data, 49, 16)),
            'sqrt_price_x64': str(price), 'tick_current_index': tick,
            'mint_a': ma, 'mint_b': mb, 'vault_a': b58_encode(data[133:165]),
            'vault_b': b58_encode(data[213:245]), 'account_sha256': sha(data),
            'static_fee_supported': spacing == fee_tier}


def vault(account, mint_id, pool):
    data = account_bytes(account, TOKEN, length=165)
    require(b58_encode(data[:32]) == mint_id and b58_encode(data[32:64]) == pool
            and data[108] == 1 and integer(data, 72, 4) == 0
            and integer(data, 129, 4) == 0 and integer(data, 109, 4) in (0, 1), 'UNSUPPORTED_VAULT')
    return {'balance_minor': str(integer(data, 64, 8)), 'account_sha256': sha(data),
            'delegate': None, 'close_authority': None, 'frozen': False}


def inspect_base(rpc, result):
    require(rpc.call('eth_chainId', []) == '0x2105', 'WRONG_BASE_CHAIN')
    anchor = rpc.call('eth_getBlockByNumber', ['finalized', False])
    require(isinstance(anchor, dict), 'MISSING_FINALIZED_BLOCK')
    hex_data(anchor.get('hash'), 32)
    require(isinstance(anchor.get('number'), str)
            and re.fullmatch(r'0x(?:0|[1-9a-f][0-9a-f]*)', anchor['number']) is not None, 'INVALID_BLOCK_NUMBER')
    context = {'blockHash': anchor['hash'], 'requireCanonical': True}
    result['context'] = {k: anchor.get(k) for k in ('hash', 'number', 'timestamp', 'parentHash')}

    def call(address, selector):
        return rpc.call('eth_call', [{'to': evm_key(address), 'data': selector}, context])

    def code(address):
        raw = hex_data(rpc.call('eth_getCode', [evm_key(address), context]))
        require(bool(raw), 'CONTRACT_CODE_MISSING')
        return sha(raw)

    result['factory'] = {'address': FACTORY, 'runtime_sha256': code(FACTORY),
                         'owner': address_word(call(FACTORY, '0x8da5cb5b'))}
    result['assets'] = []
    for address, decimals in ((WETH, 18), (BASE_USDC, 6)):
        require(uint_word(call(address, '0x313ce567'), 8) == decimals, 'TOKEN_DECIMALS_MISMATCH')
        result['assets'].append({'network': 'base-mainnet', 'address': address, 'decimals': decimals,
                                 'runtime_sha256': code(address),
                                 'behavior_review': 'CANONICAL_ADDRESS_NOT_TRANSFER_SIMULATION'})
    result['pools'] = []
    for fee, spacing in ((500, 10), (3000, 60)):
        query = '0x1698ee82' + WETH[2:].zfill(64) + BASE_USDC[2:].zfill(64) + f'{fee:064x}'
        pool = address_word(call(FACTORY, query))
        row = {'address': pool, 'fee_millionths': fee, 'status': 'INCOMPLETE'}
        result['pools'].append(row)
        if pool == '0x' + '0' * 40:
            row['status'] = 'NO_POOL_AT_FEE_TIER'
            continue
        require(address_word(call(pool, '0xc45a0155')) == FACTORY, 'POOL_FACTORY_MISMATCH')
        require(address_word(call(pool, '0x0dfe1681')) == WETH
                and address_word(call(pool, '0xd21220a7')) == BASE_USDC, 'POOL_TOKEN_MISMATCH')
        require(uint_word(call(pool, '0xddca3f43'), 24) == fee
                and uint_word(call(pool, '0xd0c93a7c'), 24) == spacing, 'POOL_FEE_OR_SPACING_MISMATCH')
        slot = hex_data(call(pool, '0x3850c7bd'), 224)
        price, tick = int.from_bytes(slot[:32], 'big'), int.from_bytes(slot[32:64], 'big', signed=True)
        require(0 < price < 1 << 160 and -887272 <= tick <= 887272
                and all(int.from_bytes(slot[i*32:(i+1)*32], 'big') < 1 << 16 for i in (2, 3, 4))
                and int.from_bytes(slot[160:192], 'big') < 256
                and int.from_bytes(slot[192:224], 'big') == 1, 'INVALID_OR_LOCKED_SLOT0')
        liquidity = uint_word(call(pool, '0x1a686502'), 128)
        row.update(token0=WETH, token1=BASE_USDC, tick_spacing=spacing, tick=tick,
                   sqrt_price_x96=str(price), liquidity=str(liquidity), runtime_sha256=code(pool),
                   status='IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED' if liquidity else 'ZERO_ACTIVE_LIQUIDITY',
                   amount_specific_quote_qualified=False)
    canonical = rpc.call('eth_getBlockByNumber', [anchor['number'], False])
    require(isinstance(canonical, dict) and canonical.get('hash') == anchor['hash'], 'CANONICAL_BLOCK_CHANGED')
    result['canonical_recheck_passed'] = True


def contextual(value):
    require(isinstance(value, dict) and isinstance(value.get('context'), dict)
            and type(value['context'].get('slot')) is int and value['context']['slot'] > 0
            and isinstance(value.get('value'), list), 'INVALID_ACCOUNT_CONTEXT')
    return value['context']['slot'], value['value']


def inspect_solana(rpc, result):
    require(rpc.call('getGenesisHash', []) == GENESIS, 'WRONG_SOLANA_GENESIS')
    result['genesis_hash'] = GENESIS
    _, identity = contextual(rpc.call('getMultipleAccounts', [[WHIRLPOOL, WSOL, SOL_USDC],
                                              {'encoding': 'base64', 'commitment': 'finalized'}]))
    require(len(identity) == 3, 'MISSING_IDENTITY_ACCOUNTS')
    program = account_bytes(identity[0], LOADER, executable=True, length=36)
    require(program[:4] == bytes([2, 0, 0, 0]), 'INVALID_PROGRAM_LINK')
    program_data = b58_encode(program[4:])
    b58_key(program_data)
    result['assets'] = [dict(network='solana-mainnet', address=a, **mint(v, d))
                        for a, v, d in ((WSOL, identity[1], 9), (SOL_USDC, identity[2], 6))]
    filters = [{'dataSize': 653}, {'memcmp': {'offset': 101, 'bytes': WSOL}},
               {'memcmp': {'offset': 181, 'bytes': SOL_USDC}}]
    slot, discovered = contextual(rpc.call('getProgramAccounts', [WHIRLPOOL, {
        'encoding': 'base64', 'commitment': 'finalized', 'withContext': True, 'filters': filters}]))
    require(len(discovered) <= 256, 'POOL_DISCOVERY_LIMIT')
    result.update(discovery_slot=str(slot), pools=[])
    seen = set()
    for entry in discovered:
        require(isinstance(entry, dict), 'INVALID_POOL_ENTRY')
        address = entry.get('pubkey')
        b58_key(address)
        require(address not in seen, 'DUPLICATE_POOL')
        seen.add(address)
        pool = whirlpool(entry.get('account'))
        pool.update(address=address, status='ADAPTIVE_FEE_UNSUPPORTED' if not pool['static_fee_supported']
                    else 'ZERO_ACTIVE_LIQUIDITY' if not int(pool['liquidity']) else 'CANDIDATE')
        result['pools'].append(pool)
    # Liquidity is used only to choose two bounded inspection subjects. This
    # does not rank their profitability or imply a whole-market census.
    selected = sorted((p for p in result['pools'] if p['status'] == 'CANDIDATE'),
                      key=lambda p: (-int(p['liquidity']), p['address']))[:2]
    addresses = [WHIRLPOOL, program_data, WSOL, SOL_USDC]
    for pool in selected:
        addresses.extend([pool['address'], pool['vault_a'], pool['vault_b'], pool['whirlpools_config']])
    addresses = list(dict.fromkeys(addresses))
    final_slot, values = contextual(rpc.call('getMultipleAccounts', [addresses,
                                       {'encoding': 'base64', 'commitment': 'finalized'}]))
    require(len(values) == len(addresses), 'ACCOUNT_BATCH_LENGTH_MISMATCH')
    accounts = dict(zip(addresses, values))
    require(account_bytes(accounts[WHIRLPOOL], LOADER, True, 36) == program, 'PROGRAM_LINK_CHANGED')
    binary = account_bytes(accounts[program_data], LOADER)
    require(len(binary) >= 49 and binary[:4] == bytes([3, 0, 0, 0])
            and binary[12] in (0, 1) and binary[45:49] == b'\x7fELF', 'INVALID_PROGRAM_DATA')
    result['program'] = {'address': WHIRLPOOL, 'program_data': program_data,
                         'program_data_sha256': sha(binary), 'elf_sha256': sha(binary[45:]),
                         'last_upgrade_slot': str(integer(binary, 4, 8)),
                         'upgrade_authority': b58_encode(binary[13:45]) if binary[12] else None,
                         'source_build_equivalence_verified': False}
    result['verified_account_slot'] = str(final_slot)
    result['assets'] = [dict(network='solana-mainnet', address=a, **mint(accounts[a], d))
                        for a, d in ((WSOL, 9), (SOL_USDC, 6))]
    for pool in selected:
        fresh = whirlpool(accounts[pool['address']])
        require(all(fresh[k] == pool[k] for k in ('mint_a', 'mint_b', 'vault_a', 'vault_b',
                                                 'whirlpools_config', 'tick_spacing', 'fee_tier_index')),
                'POOL_IDENTITY_CHANGED')
        config = account_bytes(accounts[pool['whirlpools_config']], WHIRLPOOL)
        require(len(config) >= 106 and config[:8] == hashlib.sha256(b'account:WhirlpoolsConfig').digest()[:8],
                'UNSUPPORTED_WHIRLPOOLS_CONFIG')
        pool.update(fresh)
        pool['vault_a_state'] = vault(accounts[pool['vault_a']], WSOL, pool['address'])
        pool['vault_b_state'] = vault(accounts[pool['vault_b']], SOL_USDC, pool['address'])
        pool['config_sha256'] = sha(config)
        pool['status'] = ('IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED' if int(fresh['liquidity'])
                          else 'ZERO_ACTIVE_LIQUIDITY')
        pool['amount_specific_quote_qualified'] = False
        pool['tick_arrays_qualified'] = False


def observe(network, client=None):
    rpc = client if client is not None else PublicRpc(network)
    result = {'network': network, 'endpoint': BASE_URL if network == 'base-mainnet' else SOLANA_URL,
              'started_at_utc': utc(), 'status': 'INCOMPLETE', 'runtime_activation': False,
              'execution_authorized': False, 'production_qualified': False}
    try:
        (inspect_base if network == 'base-mainnet' else inspect_solana)(rpc, result)
        result['status'] = 'OBSERVATIONS_COLLECTED'
    except ObservationError as exc:
        result.update(status='BLOCKED', reason=str(exc))
    except (ValueError, TypeError, KeyError, IndexError, RecursionError):
        result.update(status='BLOCKED', reason='INVALID_PROVIDER_STATE')
    result['finished_at_utc'] = utc()
    result['requests'] = [{k: v for k, v in c.items() if k != 'response_base64'} for c in rpc.calls]
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--collect', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    if not args.collect:
        print(json.dumps({'status': 'NOT_RUN', 'max_requests_per_network': MAX_REQUESTS}))
        return 0
    if args.output is None:
        parser.error('--output must identify a new directory')
    args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
    report = {'schema_version': 1, 'kind': 'PUBLIC_REGISTRY_OBSERVATIONS_NOT_ACTIVATION', 'networks': []}
    for network in ('base-mainnet', 'solana-mainnet'):
        client = PublicRpc(network)
        report['networks'].append(observe(network, client))
        with (args.output / (network + '-transcript.json')).open('x') as f:
            json.dump(client.calls, f, ensure_ascii=True)
            f.write('\n')
    with (args.output / 'report.json').open('x') as f:
        json.dump(report, f, indent=2, ensure_ascii=True)
        f.write('\n')
    print(json.dumps({n['network']: n['status'] for n in report['networks']}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
