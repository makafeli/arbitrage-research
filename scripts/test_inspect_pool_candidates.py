"""Synthetic fixtures test the observer's contracts, not mainnet qualification."""
import base64
from email.message import Message
from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.response

import inspect_pool_candidates as p


def account(data, owner=p.WHIRLPOOL, executable=False):
    return {'owner': owner, 'executable': executable, 'data': [base64.b64encode(data).decode(), 'base64']}


def mint_bytes(decimals=6):
    b = bytearray(82); b[44] = decimals; b[45] = 1
    return b


def pool_bytes():
    b = bytearray(653)
    b[:8] = hashlib.sha256(b'account:Whirlpool').digest()[:8]
    b[8:40] = bytes([9])*32
    b[41:43] = (64).to_bytes(2, 'little'); b[43:45] = b[41:43]
    b[45:47] = (3000).to_bytes(2, 'little')
    b[49:65] = (2**90).to_bytes(16, 'little'); b[65:81] = (2**64).to_bytes(16, 'little')
    b[101:133] = p.b58_key(p.WSOL); b[181:213] = p.b58_key(p.SOL_USDC)
    b[133:165] = bytes([3])*32; b[213:245] = bytes([4])*32
    return b


def word(n):
    return '0x' + int(n).to_bytes(32, 'big', signed=n < 0).hex()


class FakeBase:
    def __init__(self, canonical=True):
        self.calls = []; self.canonical = canonical

    def call(self, method, params):
        self.calls.append({'request': {'method': method, 'params': params}})
        if method == 'eth_chainId': return '0x2105'
        if method == 'eth_getBlockByNumber':
            return {'hash': '0x' + ('ab' if params[0] == 'finalized' or self.canonical else 'cd')*32,
                    'number': '0x123', 'timestamp': '0x456', 'parentHash': '0x' + 'ef'*32}
        if method == 'eth_getCode': return '0x60006000'
        target = params[0]['to']; selector = params[0]['data'][:10]
        a = '0x' + '11'*20; b = '0x' + '22'*20
        values = {'0x8da5cb5b': word(1), '0x313ce567': word(18 if target == p.WETH else 6),
                  '0xc45a0155': word(int(p.FACTORY, 16)), '0x0dfe1681': word(int(p.WETH, 16)),
                  '0xd21220a7': word(int(p.BASE_USDC, 16)),
                  '0xddca3f43': word(500 if target == a else 3000),
                  '0xd0c93a7c': word(10 if target == a else 60), '0x1a686502': word(2**100),
                  '0x3850c7bd': '0x' + ''.join(word(v)[2:] for v in (2**96, -1000, 1, 2, 2, 0, 1))}
        if selector == '0x1698ee82': return word(int(a if int(params[0]['data'][-64:], 16) == 500 else b, 16))
        return values[selector]


class ObservationTests(unittest.TestCase):
    def test_default_cli_has_no_network_side_effect(self):
        with patch.object(p, 'PublicRpc') as rpc, redirect_stdout(io.StringIO()) as output:
            self.assertEqual(p.main([]), 0)
        rpc.assert_not_called(); self.assertEqual(json.loads(output.getvalue())['status'], 'NOT_RUN')

    def test_real_default_command(self):
        run = subprocess.run([sys.executable, p.__file__], capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 0); self.assertEqual(json.loads(run.stdout)['status'], 'NOT_RUN')

    def test_base58_round_trips_preserve_leading_zeroes(self):
        for b in (bytes([0])*2 + bytes([1])*30, bytes([255])*32, p.b58_key(p.WSOL), p.b58_key(p.GENESIS)):
            self.assertEqual(p.b58_key(p.b58_encode(b)), b)
        for invalid in ('fixture:pool', '', '1'*32, '0'*44, ['not-a-string']):
            with self.assertRaises(p.ObservationError): p.b58_key(invalid)

    def test_abi_lengths_and_overflow_are_rejected(self):
        self.assertEqual(p.uint_word(word(2**100)), 2**100)
        for value in ('0x123', '0x', '0xzz', None):
            with self.assertRaises(p.ObservationError): p.uint_word(value)
        with self.assertRaises(p.ObservationError): p.uint_word(word(256), 8)
        with self.assertRaises(p.ObservationError): p.address_word(word(2**160))
        with self.assertRaises(p.ObservationError): p.evm_key('fixture:pool')

    def test_pool_exact_units_and_supported_pair(self):
        result = p.whirlpool(account(pool_bytes()))
        self.assertEqual(result['liquidity'], str(2**90)); self.assertTrue(result['static_fee_supported'])
        self.assertEqual(result['mint_a'], p.WSOL); self.assertEqual(result['mint_b'], p.SOL_USDC)

    def test_pool_rejects_layout_owner_pair_and_price(self):
        for mutate in (lambda b: b.__setitem__(0, 0), lambda b: b.__setitem__(slice(101,133), bytes(32)),
                       lambda b: b.__setitem__(slice(65,81), bytes(16)), lambda b: b.pop()):
            b = pool_bytes(); mutate(b)
            with self.assertRaises(p.ObservationError): p.whirlpool(account(b))
        with self.assertRaises(p.ObservationError): p.whirlpool(account(pool_bytes(), p.TOKEN))

    def test_adaptive_fee_is_visible_not_supported(self):
        b = pool_bytes(); b[43:45] = (123).to_bytes(2, 'little')
        self.assertFalse(p.whirlpool(account(b))['static_fee_supported'])

    def test_mint_rejects_extensions_decimals_and_flags(self):
        self.assertEqual(p.mint(account(mint_bytes(), p.TOKEN), 6)['decimals'], 6)
        for raw, owner in ((mint_bytes()+b'extension',p.TOKEN), (mint_bytes(9),p.TOKEN), (mint_bytes(),p.WHIRLPOOL)):
            with self.assertRaises(p.ObservationError): p.mint(account(raw, owner), 6)

    def test_vault_frozen_delegate_wrong_owner_and_large_balance(self):
        pool = p.b58_encode(bytes([2])*32); b = bytearray(165)
        b[:32] = p.b58_key(p.WSOL); b[32:64] = p.b58_key(pool)
        b[64:72] = (2**60).to_bytes(8, 'little'); b[108] = 1
        self.assertEqual(p.vault(account(b,p.TOKEN),p.WSOL,pool)['balance_minor'],str(2**60))
        for pos in (0, 32, 72, 108, 129):
            bad = b.copy(); bad[pos] ^= 1
            with self.assertRaises(p.ObservationError): p.vault(account(bad,p.TOKEN),p.WSOL,pool)

    def test_account_encoding_boolean_and_context_fail_closed(self):
        a = account(b'ab'); a['executable'] = 0
        with self.assertRaises(p.ObservationError): p.account_bytes(a,p.WHIRLPOOL)
        for v in ({}, {'context':{'slot':True},'value':[]}, {'context':{'slot':0},'value':[]}):
            with self.assertRaises(p.ObservationError): p.contextual(v)

    def test_duplicate_and_nonfinite_json_rejected(self):
        for raw in ('{"a":1,"a":2}', 'NaN', 'Infinity'):
            with self.assertRaises(p.ObservationError): p.strict_json(raw)

    def test_base_all_reads_pinned_and_no_activation(self):
        rpc = FakeBase(); r = p.observe('base-mainnet', rpc)
        self.assertEqual(r['status'], 'OBSERVATIONS_COLLECTED')
        self.assertEqual(len(r['pools']), 2); self.assertTrue(r['canonical_recheck_passed'])
        self.assertFalse(r['runtime_activation']); self.assertFalse(r['production_qualified'])
        for call in rpc.calls:
            req = call['request']
            if req['method'] in ('eth_call','eth_getCode'):
                self.assertEqual(req['params'][-1], {'blockHash':'0x'+'ab'*32, 'requireCanonical':True})

    def test_changed_finalized_anchor_blocks_report(self):
        r = p.observe('base-mainnet', FakeBase(False))
        self.assertEqual(r['status'],'BLOCKED'); self.assertEqual(r['reason'],'CANONICAL_BLOCK_CHANGED')
        self.assertFalse(r['production_qualified'])

    def test_matching_hash_with_changed_header_blocks_report(self):
        for field, changed in (('number', '0x124'), ('timestamp', '0x457'),
                               ('parentHash', '0x' + 'cd'*32)):
            with self.subTest(field=field):
                rpc = FakeBase(); original = rpc.call
                def call(method, params):
                    response = original(method, params)
                    if method == 'eth_getBlockByNumber' and params[0] != 'finalized':
                        response[field] = changed
                    return response
                rpc.call = call
                result = p.observe('base-mainnet', rpc)
                self.assertEqual(result['status'], 'BLOCKED')
                self.assertEqual(result['reason'], 'CANONICAL_BLOCK_CHANGED')
                self.assertNotIn('canonical_recheck_passed', result)

    def test_invalid_initial_or_final_header_never_passes_canonical_check(self):
        invalid_fields = [('hash', None), ('parentHash', None), ('parentHash', '0x12'),
                          ('number', None), ('number', '0x+123'), ('number', '0x0123'),
                          ('number', '0x10000000000000000'), ('timestamp', None),
                          ('timestamp', '0x+456'), ('timestamp', '0x0456'),
                          ('timestamp', '0x10000000000000000')]
        for phase in ('initial', 'final'):
            for field, invalid in invalid_fields:
                with self.subTest(phase=phase, field=field, invalid=invalid):
                    rpc = FakeBase(); original = rpc.call
                    def call(method, params):
                        response = original(method, params)
                        if method == 'eth_getBlockByNumber' and ((params[0] == 'finalized') == (phase == 'initial')):
                            if invalid is None:
                                response.pop(field)
                            else:
                                response[field] = invalid
                        return response
                    rpc.call = call
                    result = p.observe('base-mainnet', rpc)
                    self.assertEqual(result['status'], 'BLOCKED')
                    self.assertNotIn('canonical_recheck_passed', result)
                    if phase == 'initial':
                        self.assertEqual(len(rpc.calls), 2)

    def test_unrelated_header_fields_do_not_change_context_identity(self):
        rpc = FakeBase(); original = rpc.call
        def call(method, params):
            response = original(method, params)
            if method == 'eth_getBlockByNumber' and params[0] != 'finalized':
                response['transactions'] = []
            return response
        rpc.call = call
        result = p.observe('base-mainnet', rpc)
        self.assertEqual(result['status'], 'OBSERVATIONS_COLLECTED')
        self.assertTrue(result['canonical_recheck_passed'])

    def test_wrong_solana_identity_stops_before_accounts(self):
        rpc = FakeBase()
        def wrong(method,params):
            rpc.calls.append({'request':{'method':method}}); return None
        rpc.call = wrong
        r = p.observe('solana-mainnet',rpc)
        self.assertEqual(r['reason'],'WRONG_SOLANA_GENESIS'); self.assertEqual(len(rpc.calls),1)

    def test_http_refusal_has_no_retry_or_error_body_leak(self):
        rpc = p.PublicRpc('base-mainnet')
        error = urllib.error.HTTPError(p.BASE_URL,403,'private-message',{},io.BytesIO(b'private-body'))
        with patch.object(p.urllib.request,'build_opener') as opener, patch.object(p.time,'sleep'):
            opener.return_value.open.side_effect = error
            r = p.observe('base-mainnet',rpc)
            self.assertEqual(r['reason'],'HTTP_REFUSED')
            self.assertEqual(opener.return_value.open.call_count,1)
            with self.assertRaises(p.ObservationError): rpc.call('eth_chainId',[])
        self.assertNotIn('private',json.dumps(r))

    def test_real_redirect_handler_refuses_disallowed_location(self):
        requests = []
        class StubHttps(p.urllib.request.HTTPSHandler):
            def https_open(self, request):
                requests.append(request.full_url)
                headers = Message()
                headers['Location'] = 'https://disallowed.invalid/private-target'
                response = urllib.response.addinfourl(io.BytesIO(b'private-body'), headers,
                                                      request.full_url, 302)
                response.msg = 'Found'
                return response
        opener = p.urllib.request.build_opener(p.NoRedirects(), StubHttps())
        with patch.object(p.urllib.request, 'build_opener', return_value=opener), patch.object(p.time, 'sleep'):
            report = p.observe('base-mainnet')
        self.assertEqual(requests, [p.BASE_URL])
        self.assertEqual(report['reason'], 'HTTP_REFUSED')
        self.assertEqual(report['requests'][0]['http_status'], 302)
        self.assertNotIn('private', json.dumps(report))
        self.assertNotIn('disallowed', json.dumps(report))

    def test_rpc_envelope_id_boolean_and_byte_budget(self):
        for raw in (b'{"jsonrpc":"2.0","id":false,"result":"0x2105"}',b'{}',b'x'*9):
            rpc = p.PublicRpc('base-mainnet')
            with patch.object(p.urllib.request,'build_opener') as opener, patch.object(p.time,'sleep'), \
                 patch.object(p,'MAX_RESPONSE',8 if len(raw)==9 else p.MAX_RESPONSE):
                response = opener.return_value.open.return_value.__enter__.return_value
                response.status = 200; response.read.return_value = raw
                with self.assertRaises(p.ObservationError): rpc.call('eth_chainId',[])
            self.assertTrue(rpc.stopped)

    def test_large_program_account_is_bounded_after_base64_decoding(self):
        raw = bytes(9 * 1024 * 1024)
        self.assertEqual(len(p.account_bytes(account(raw,p.LOADER),p.LOADER)),len(raw))
        with patch.object(p,'MAX_ACCOUNT',8):
            with self.assertRaisesRegex(p.ObservationError,'ACCOUNT_BYTE_LIMIT'):
                p.account_bytes(account(b'123456789',p.LOADER),p.LOADER)

    def test_solana_only_collection_does_not_retry_base(self):
        with tempfile.TemporaryDirectory() as root, patch.object(p,'PublicRpc') as client:
            client.return_value.calls = []
            with patch.object(p,'observe',return_value={'network':'solana-mainnet','status':'BLOCKED'}), redirect_stdout(io.StringIO()):
                p.main(['--collect','--network','solana-mainnet','--output',str(Path(root)/'new')])
            client.assert_called_once_with('solana-mainnet')
            self.assertFalse((Path(root)/'new/base-mainnet-transcript.json').exists())
            self.assertTrue((Path(root)/'new/solana-mainnet-transcript.json').exists())

    def test_forbidden_method_and_call_budget_precede_network(self):
        rpc = p.PublicRpc('base-mainnet')
        with patch.object(p.urllib.request,'build_opener') as opener:
            with self.assertRaises(p.ObservationError): rpc.call('eth_sendRawTransaction',[])
            rpc.calls = [{}]*p.MAX_REQUESTS
            with self.assertRaises(p.ObservationError): rpc.call('eth_chainId',[])
        opener.assert_not_called()


if __name__ == '__main__':
    unittest.main()
