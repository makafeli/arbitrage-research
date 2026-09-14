#!/usr/bin/env python3
"""Reproduce the reviewed, point-in-time ARB-003 identity inventory offline."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

import inspect_pool_candidates as observer

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = 'docs/registries/initial-identities.json'
# These are hashes of reviewed projections, not attestations of provider truth.
INPUTS = {
    'base': ('docs/qualification-fixtures/base-identity-2026-09-14.json',
             'e35abda8d1f51d513d6bcdec4013d0acf9c545156c50ca37742d2eb2df4ee763'),
    'solana': ('docs/qualification-fixtures/solana-identity-2026-09-14.json',
               '6029d6e0ddcf68e50914e0a6b78ffcccb89750cb2486f2a9b0c918a8f46de007'),
}
SOURCES = [
    'https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments',
    'https://developers.circle.com/stablecoins/usdc-contract-addresses',
    'https://github.com/orca-so/whirlpools',
    'https://solana.com/docs/tokens/basics/sync-native',
    'https://docs.anza.xyz/clusters/available',
    'https://github.com/circlefin/stablecoin-evm',
]


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def amount(value: object) -> str:
    require(isinstance(value, str) and re.fullmatch(r'0|[1-9][0-9]{0,77}', value) is not None,
            'INVALID_EXACT_AMOUNT')
    require(int(value) < 2**256, 'AMOUNT_OVERFLOW')
    return value


def identity(network: str, address: str) -> dict:
    if network == 'base-mainnet':
        require(observer.evm_key(address) == address, 'NONCANONICAL_ADDRESS')
    elif network == 'solana-mainnet':
        observer.b58_key(address)
    else:
        raise ValueError('UNSUPPORTED_NETWORK')
    return {'network_id': network, 'address': address}


def fingerprint(value: str) -> str:
    require(isinstance(value, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', value) is not None,
            'INVALID_CODE_FINGERPRINT')
    return value


def assemble(base: dict, solana: dict) -> dict:
    """Transform checked report projections; never authorize a live registry."""
    chains = []
    for name, envelope in [('base', base), ('solana', solana)]:
        require(envelope['kind'] == 'REVIEWED_REPORT_PROJECTION_NOT_RAW_CAPTURE', 'INVALID_EVIDENCE_KIND')
        report = envelope['observation']
        network = name + '-mainnet'
        require(report['network'] == network and report['status'] == 'OBSERVATIONS_COLLECTED',
                'MISSING_NETWORK_OBSERVATIONS')
        for field in ('runtime_activation', 'execution_authorized', 'production_qualified'):
            require(report[field] is False, 'UNSUPPORTED_AUTHORIZATION_CLAIM')
        expected = [(observer.WETH, 18), (observer.BASE_USDC, 6)] if name == 'base' else [
            (observer.WSOL, 9), (observer.SOL_USDC, 6)]
        require(len(report['assets']) == 2, 'UNEXPECTED_ASSET_COUNT')
        assets = []
        for row, (address, decimals) in zip(report['assets'], expected):
            require(row['network'] == network and row['address'] == address
                    and type(row['decimals']) is int and row['decimals'] == decimals,
                    'ASSET_IDENTITY_OR_DECIMALS_MISMATCH')
            item = {'identity': identity(network, address), 'decimals': decimals}
            if name == 'base':
                item['observed_runtime_sha256'] = fingerprint(row['runtime_sha256'])
                item['transfer_behavior_qualified'] = False
            else:
                require(row['token_program'] == observer.TOKEN and row['extensions_supported'] is False,
                        'UNSUPPORTED_TOKEN_PROGRAM')
                item.update(token_program=observer.TOKEN, observed_account_sha256=fingerprint(row['account_sha256']))
                for authority in ('mint_authority', 'freeze_authority'):
                    value = row[authority]
                    if value is not None:
                        identity(network, value)
                    item[authority] = value
            assets.append(item)
        if name == 'base':
            require(report['canonical_recheck_passed'] is True, 'BASE_CONTEXT_NOT_RECHECKED')
            context = observer.base_header(report['context'])
            require(report['factory']['address'] == observer.FACTORY, 'WRONG_FACTORY')
            venue = {'protocol': 'uniswap-v3', 'identity': identity(network, observer.FACTORY),
                     'observed_runtime_sha256': fingerprint(report['factory']['runtime_sha256']),
                     'observed_factory_owner': identity(network, report['factory']['owner'])}
            chain_identity = {'chain_id': 8453}
        else:
            require(report['genesis_hash'] == observer.GENESIS, 'WRONG_GENESIS')
            require(int(amount(report['verified_account_slot'])) >= int(amount(report['discovery_slot'])) > 0,
                    'REGRESSING_ACCOUNT_CONTEXT')
            context = {'discovery_slot': report['discovery_slot'], 'account_batch_slot': report['verified_account_slot']}
            program = report['program']
            require(program['address'] == observer.WHIRLPOOL, 'WRONG_WHIRLPOOL_PROGRAM')
            require(program['source_build_equivalence_verified'] is False, 'UNSUPPORTED_BUILD_CLAIM')
            venue = {'protocol': 'orca-whirlpool-static-fee', 'identity': identity(network, program['address']),
                     'program_data': identity(network, program['program_data']),
                     'observed_program_data_sha256': fingerprint(program['program_data_sha256']),
                     'observed_elf_sha256': fingerprint(program['elf_sha256']),
                     'last_upgrade_slot': amount(program['last_upgrade_slot']),
                     'upgrade_authority': identity(network, program['upgrade_authority']) if program['upgrade_authority'] else None}
            chain_identity = {'genesis_hash': observer.GENESIS}
        require(isinstance(report['pools'], list) and len(report['pools']) <= 8, 'POOL_COUNT_LIMIT')
        pools, seen = [], set()
        for row in report['pools']:
            pool_id = identity(network, row['address'])
            require(row['address'] not in seen and row['address'] not in [a for a, _ in expected], 'DUPLICATE_POOL')
            seen.add(row['address'])
            pair = (row['token0'], row['token1']) if name == 'base' else (row['mint_a'], row['mint_b'])
            require(pair == tuple(a for a, _ in expected), 'POOL_PAIR_MISMATCH')
            require(row['status'] == 'IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED'
                    and int(amount(row['liquidity'])) > 0, 'MISSING_ACTIVE_LIQUIDITY')
            require(row['amount_specific_quote_qualified'] is False, 'UNSUPPORTED_QUOTE_CLAIM')
            fee = row['fee_millionths'] if name == 'base' else row['fee_rate_millionths']
            spacing = row['tick_spacing']
            require(type(fee) is int and 0 < fee < 1_000_000
                    and type(spacing) is int and 0 < spacing <= 32767, 'INVALID_POOL_PARAMETERS')
            item = {'identity': pool_id, 'assets': [identity(network, address) for address in pair],
                    'fee_millionths': fee, 'tick_spacing': spacing,
                    'active_liquidity_at_observation': row['liquidity'],
                    'liquidity_suitability': 'NONZERO_AT_OBSERVATION_NOT_SIZE_SPECIFIC',
                    'amount_specific_quote_qualified': False}
            if name == 'base':
                require((fee, spacing) in {(500, 10), (3000, 60)}, 'UNEXPECTED_BASE_FEE_TIER')
                item['observed_runtime_sha256'] = fingerprint(row['runtime_sha256'])
            else:
                require(row['static_fee_supported'] is True and row['fee_tier_index'] == spacing,
                        'ADAPTIVE_FEE_UNSUPPORTED')
                require(row['tick_arrays_qualified'] is False, 'UNSUPPORTED_TICK_CLAIM')
                item['whirlpools_config'] = identity(network, row['whirlpools_config'])
                item['observed_account_sha256'] = fingerprint(row['account_sha256'])
                item['observed_config_sha256'] = fingerprint(row['config_sha256'])
                item['vaults'] = []
                for suffix, token in zip(('a', 'b'), pair):
                    state = row[f'vault_{suffix}_state']
                    require(state['frozen'] is False and state['delegate'] is None
                            and state['close_authority'] is None, 'UNSUPPORTED_VAULT_AUTHORITY')
                    item['vaults'].append({'identity': identity(network, row[f'vault_{suffix}']),
                                          'mint': identity(network, token), 'owner': pool_id,
                                          'balance_minor_at_observation': amount(state['balance_minor']),
                                          'observed_account_sha256': fingerprint(state['account_sha256'])})
            pools.append(item)
        chains.append({'network_id': network, 'chain_identity': chain_identity,
                       'observed_from_utc': report['started_at_utc'], 'observed_until_utc': report['finished_at_utc'],
                       'context': context, 'venue': venue, 'assets': assets, 'pools': pools,
                       'two_distinct_identity_candidates_observed': len(pools) >= 2,
                       'two_pool_experiments_authorized': False,
                       'source_evidence': INPUTS[name][0], 'source_provenance': envelope['provenance']})
    return {'schema_version': 1, 'kind': 'ARB003_IDENTITY_INVENTORY_NOT_RUNTIME_REGISTRY',
            'review_date': '2026-09-14', 'scope': 'INITIAL_NETWORK_ASSET_POOL_IDENTITIES_ONLY',
            'runtime_activation': False, 'execution_authorized': False, 'production_qualified': False,
            'same_venue_routes_permitted_in_scope': True,
            'unsupported_scope': ['Sushi', 'Raydium', 'Token-2022/extensions', 'adaptive fees',
                                  'fee-on-transfer/rebasing tokens', 'additional networks/assets'],
            'risk_notes': ['USDC issuer mint/freeze/blacklist and proxy upgrade controls are not disabled by identity acceptance.',
                           'Whirlpool is upgradeable; observed code fingerprints are not reproducible-build attestations.',
                           'Pool state/liquidity and authorities are historical observations, not perpetual eligibility.',
                           'Wrapping/unwrapping, complete tick coverage, costs and atomic simulation require later gates.'],
            'primary_references_reviewed': SOURCES, 'chains': chains}


def encode(value: dict) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode()


def build(root: Path = ROOT) -> dict:
    reports = []
    for path, expected_hash in INPUTS.values():
        with (root / path).open('rb') as stream:
            raw = stream.read(128 * 1024 + 1)
        require(len(raw) <= 128 * 1024 and hashlib.sha256(raw).hexdigest() == expected_hash,
                'REVIEWED_EVIDENCE_DIGEST_MISMATCH')
        reports.append(observer.strict_json(raw))
    return assemble(*reports)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Compare the committed inventory with reviewed evidence')
    args = parser.parse_args()
    try:
        result = encode(build())
        if args.check:
            require((ROOT / OUTPUT).read_bytes() == result, 'INVENTORY_NOT_REPRODUCIBLE')
            print(json.dumps({'status': 'IDENTITY_INVENTORY_CONSISTENT', 'runtime_activation': False}))
        else:
            print(result.decode(), end='')
        return 0
    except (OSError, ValueError, TypeError, KeyError, RecursionError):
        print(json.dumps({'status': 'INVENTORY_CHECK_FAILED', 'runtime_activation': False}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
