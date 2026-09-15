"""Offline harness regressions; these are not evidence of an actual market run."""
import contextlib
import copy
import io
import json
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import recorded_base_slice as s


def inventory_and_report():
    inventory = json.loads((s.ROOT/'docs/registries/initial-identities.json').read_text())
    chain = inventory['chains'][0]
    report = {'network':s.NETWORK, 'status':'OBSERVATIONS_COLLECTED', 'canonical_recheck_passed':True,
              'factory':{'address':chain['venue']['identity']['address'],'runtime_sha256':chain['venue']['observed_runtime_sha256']},
              'assets':[{'address':a['identity']['address'],'decimals':a['decimals'],'runtime_sha256':a['observed_runtime_sha256']} for a in chain['assets']],
              'pools':[{'address':p['identity']['address'],'token0':s.WETH,'token1':s.USDC,
                        'fee_millionths':p['fee_millionths'],'tick_spacing':p['tick_spacing'],
                        'runtime_sha256':p['observed_runtime_sha256'],'tick':-200001,
                        'status':'IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED'} for p in chain['pools']]}
    return inventory,report


class RecordedSliceTests(unittest.TestCase):
    def test_default_never_reads_credentials_or_executes(self):
        with patch.object(s.operator_access,'endpoint_from_environment') as access, patch.object(s,'exercise') as run, contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(s.main([]),0)
        access.assert_not_called();run.assert_not_called()
        self.assertEqual(json.loads(out.getvalue())['network_calls'],0)

    def test_refuses_any_database_except_explicit_disposable_loopback(self):
        s.safe_database('postgres://test:test@127.0.0.1:5432/arb_recorded_slice')
        for url in ('postgres://host/arb_recorded_slice','postgres://127.0.0.1:5432/production',
                    'https://127.0.0.1:5432/arb_recorded_slice', 'postgres://127.0.0.1:5555/arb_recorded_slice',
                    'postgres://127.0.0.1:5432/arb_recorded_slice?host=production',
                    'postgres://127.0.0.1:5432/arb_recorded_slice#other',''):
            with self.assertRaises(s.SliceError):s.safe_database(url)

    def test_registry_retains_exact_identity_and_bounded_floor_division(self):
        inventory,report=inventory_and_report();registry=s.runtime_registry(report,inventory)
        self.assertEqual(len(registry['pools']),2)
        for pool,raw in zip(registry['pools'],report['pools']):
            self.assertEqual(pool['bitmap_word_min'],raw['tick']//raw['tick_spacing']//256)
            self.assertEqual(pool['bitmap_word_min'],pool['bitmap_word_max'])
            self.assertEqual(pool['pool_runtime_sha256'],raw['runtime_sha256'])

    def test_changed_factory_asset_pool_or_empty_report_fail_closed(self):
        inventory,original=inventory_and_report()
        for mutate in (lambda r:r['factory'].update(runtime_sha256='sha256:'+'0'*64),
                       lambda r:r['assets'][0].update(decimals=6),
                       lambda r:r['pools'][0].update(runtime_sha256='sha256:'+'0'*64),
                       lambda r:r['pools'][0].update(token0=s.USDC),
                       lambda r:r.update(pools=[]),lambda r:r.update(status='BLOCKED')):
            report=copy.deepcopy(original);mutate(report)
            with self.assertRaises(s.SliceError):s.runtime_registry(report,inventory)

    def test_duplicate_or_foreign_pools_cannot_be_authorized(self):
        inventory,report=inventory_and_report()
        report['pools'][1]=copy.deepcopy(report['pools'][0])
        with self.assertRaises(s.SliceError):s.runtime_registry(report,inventory)

    def test_config_is_isolated_exact_and_observation_only(self):
        inventory,report=inventory_and_report();raw=json.dumps(s.runtime_registry(report,inventory)).encode()
        config=tomllib.loads(s.config_text(raw,Path('/tmp/only-test-captures')))
        self.assertEqual(config['deployment']['mode'],'OBSERVE')
        self.assertFalse(config['execution']['enabled'])
        self.assertFalse(config['execution']['broadcast_enabled'])
        self.assertFalse(config['networks']['solana']['enabled'])
        self.assertEqual(config['research']['trade_sizes_minor'],['1000000'])
        self.assertEqual(config['networks']['base']['registry_qualification_digest'],s.sha(raw))
        self.assertEqual(config['storage']['database_secret_reference'],'env:TEST_DATABASE_URL')

    def test_replay_comparison_preserves_unknown_costs_and_rejections(self):
        trace=dict(configuration_digest='sha256:test',calculation_version='v1',network_id=s.NETWORK,
                   dataset_origin='RECORDED_LIVE',mode='OBSERVE',capture_refs=[{'capture_id':'one'}],
                   route=[],amount_in_minor=None,result={'status':'DATA_UNAVAILABLE','reason':'missing'},diagnostics=['missing'])
        projected=s.projection(trace)
        self.assertIsNone(projected['amount_in_minor'])
        self.assertEqual(projected['result'],trace['result'])
        trace['dataset_origin']='MANUALLY_CONSTRUCTED'
        with self.assertRaises(s.SliceError):s.projection(trace)

    def test_child_environment_does_not_inherit_provider_or_operator_secrets(self):
        with patch.dict(os.environ,{'ARB_BASE_RPC_URL':'private','ARB_OPERATOR_SECRET':'private','TEST_DATABASE_URL':'private','GH_TOKEN':'private','PATH':'retained'},clear=True):
            self.assertEqual(s.clean_env(),{'PATH':'retained'})

    def test_api_limit_precedes_http_and_only_loopback_paths_are_accepted(self):
        api=s.LocalAPI('never-export')
        with patch.object(api.client,'open') as call:
            with self.assertRaises(s.SliceError):api.call('https://evil.invalid/v1/sessions')
            api.calls=s.MAX_API_CALLS
            with self.assertRaises(s.SliceError):api.call('/v1/sessions')
            call.assert_not_called()

    def test_export_refuses_endpoint_token_query_credentials_or_symlinks(self):
        endpoint='https://authorized.example/v2/PRIVATE_KEY_123456?apikey=querysecretvalue'
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'data.json'
            for text in (endpoint,'PRIVATE_KEY_123456','querysecretvalue'):
                path.write_text(text)
                with self.assertRaises(s.SliceError):s.validate_export(Path(root),[endpoint])
            path.write_text('{}')
            link=Path(root)/'link';link.symlink_to(path)
            with self.assertRaises(s.SliceError):s.validate_export(Path(root),[endpoint])
            link.unlink();s.validate_export(Path(root),[endpoint])
            self.assertTrue((Path(root)/'EXPORT_READY').is_file())
            self.assertIn('data.json',(Path(root)/'SHA256SUMS').read_text())
            path.write_text('PRIVATE_KEY_123456')
            with self.assertRaises(s.SliceError):s.validate_export(Path(root),[endpoint])
            self.assertFalse((Path(root)/'EXPORT_READY').exists())

    def test_workflow_separates_offline_checks_and_explicit_owner_collection(self):
        text=(s.ROOT/'.github/workflows/recorded-base-slice.yml').read_text()
        offline=text.split('  offline:',1)[1].split('  recorded:',1)[0]
        self.assertNotIn('secrets.',offline)
        self.assertIn('python3 scripts/test_recorded_base_slice.py -v',offline)
        self.assertIn("github.event_name == 'workflow_dispatch'",text)
        self.assertIn("github.ref == 'refs/heads/main'",text)
        self.assertIn("github.actor == 'makafeli'",text)
        self.assertIn("github.event.head_commit.message == 'Run recorded Base slice once'",text)
        self.assertIn("github.event_name == 'push'",text)
        self.assertNotIn('pull_request_target:',text)
        self.assertEqual(text.count('${{ secrets.ARB_BASE_RPC_URL }}'),1)
        self.assertIn('contents: read',text)
        self.assertIn("always() && steps.experiment.outputs.export_ready == 'true'",text)
        self.assertIn('exit "$result"',text)
        self.assertNotIn('continue-on-error',text)

    def test_browser_asserts_actual_visible_trace_and_forbids_extra_mutations(self):
        text=(s.ROOT/'apps/web/scripts/recorded-slice-browser.mjs').read_text()
        self.assertIn("getByLabel('Decision session'",text)
        self.assertIn('Inspect decision ${id}',text)
        self.assertIn('toContainText(ref.manifest_digest)',text)
        self.assertIn("path !== '/v1/auth/login'",text)
        self.assertNotIn('page.route(',text)

    def test_missing_binary_prevents_any_provider_call(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(s,'ROOT',Path(directory)), patch.object(s.operator_access,'endpoint_from_environment',return_value='https://authorized.example'), patch.dict(os.environ,{'TEST_DATABASE_URL':'postgres://test:test@127.0.0.1:5432/arb_recorded_slice'}), patch.object(s,'exercise') as run, contextlib.redirect_stdout(io.StringIO()) as out:
            code=s.main(['--run','--output',str(Path(directory)/'out')])
            self.assertEqual(code,2);run.assert_not_called()
            self.assertEqual(json.loads(out.getvalue())['reason'],'REQUIRED_BINARY_MISSING')


class ReplayBatchTests(unittest.TestCase):
    def traces(self):
        first = dict(session_id='session-1', experiment_id='experiment-1', generation='1',
                     configuration_digest='sha256:config', calculation_version='v1',
                     strategy_id='cycle-v1', network_id=s.NETWORK, mode='OBSERVE',
                     dataset_origin='RECORDED_LIVE', observed_at_unix_ms=1000,
                     capture_refs=[dict(capture_id='capture-a', manifest_digest='sha256:a', snapshot_id='sha256:a'),
                                   dict(capture_id='capture-b', manifest_digest='sha256:b', snapshot_id='sha256:b')],
                     route=['pool-a', 'pool-b'], amount_in_minor='1000000',
                     result={'status':'QUOTED', 'gross_delta_minor':'-3389'},
                     diagnostics=['EXTERNAL_COSTS_UNAVAILABLE'])
        second = copy.deepcopy(first)
        second['capture_refs'].reverse(); second['route'].reverse()
        second['result']['gross_delta_minor'] = '-3609'
        return first, second

    def replay(self, traces):
        return dict(network_requests=0, dataset_origin='RECORDED_LIVE', decisions=traces)

    def test_reverse_routes_belong_to_one_batch_without_reordering_either(self):
        first, second = self.traces(); original = copy.deepcopy([first, second])
        # Regression: the preceding equality selected only one expected route.
        self.assertNotEqual(first['capture_refs'], second['capture_refs'])
        rows = [{'trace':first}, {'trace':second}]
        selected = s.batch_traces(rows, first)
        self.assertEqual(len(selected), 2)
        self.assertEqual(s.compare_replay(selected, self.replay([second, first])), 2)
        self.assertEqual([first, second], original)
        self.assertNotEqual(s.projection(first)['route'], s.projection(second)['route'])

    def test_other_session_config_generation_or_acquisition_never_joins_batch(self):
        first, _ = self.traces()
        for key in ('session_id','experiment_id','generation','configuration_digest',
                    'calculation_version','strategy_id','network_id','mode','dataset_origin','observed_at_unix_ms'):
            different = copy.deepcopy(first); different[key] = 'different'
            self.assertEqual(s.batch_traces([{'trace':first},{'trace':different}],first), [first])
            with self.assertRaisesRegex(s.SliceError, 'REPLAY_BATCH_MISMATCH'):
                s.compare_replay([first], self.replay([different]))

    def test_changed_manifest_or_snapshot_is_not_the_same_batch(self):
        first, _ = self.traces()
        for key in ('capture_id','manifest_digest','snapshot_id'):
            different = copy.deepcopy(first); different['capture_refs'][0][key] = 'changed'
            self.assertNotEqual(s.capture_batch_key(first), s.capture_batch_key(different))
            with self.assertRaisesRegex(s.SliceError, 'REPLAY_BATCH_MISMATCH'):
                s.compare_replay([first], self.replay([different]))

    def test_duplicate_missing_and_invalid_capture_references_fail_closed(self):
        first, _ = self.traces()
        variants = [[], [first['capture_refs'][0]], [first['capture_refs'][0]] * 2,
                    [{}, first['capture_refs'][1]], [None, first['capture_refs'][1]]]
        invalid = copy.deepcopy(first['capture_refs']); invalid[0]['snapshot_id'] = None
        variants.append(invalid)
        conflicting = copy.deepcopy(first['capture_refs'])
        conflicting[1]['capture_id'] = conflicting[0]['capture_id']; variants.append(conflicting)
        for refs in variants:
            changed = copy.deepcopy(first); changed['capture_refs'] = refs
            with self.assertRaises(s.SliceError): s.capture_batch_key(changed)

    def test_missing_duplicate_or_mutated_routes_do_not_pass_comparison(self):
        first, second = self.traces()
        for actual in ([], [first], [first, first], [first, second, second]):
            with self.assertRaisesRegex(s.SliceError, 'REPLAY_DECISION_MISMATCH'):
                s.compare_replay([first, second], self.replay(actual))
        for key, value in [('route',['pool-b','pool-a']),('amount_in_minor','1000001'),
                           ('result',{'status':'QUOTED','gross_delta_minor':'0'}),
                           ('diagnostics',[])]:
            changed = copy.deepcopy(first); changed[key] = value
            with self.assertRaisesRegex(s.SliceError, 'REPLAY_DECISION_MISMATCH'):
                s.compare_replay([first, second], self.replay([changed, second]))
        changed = copy.deepcopy(first); changed['capture_refs'].reverse()
        with self.assertRaisesRegex(s.SliceError, 'REPLAY_DECISION_MISMATCH'):
            s.compare_replay([first, second], self.replay([changed, second]))

    def test_network_replay_or_synthetic_substitution_is_never_accepted(self):
        first, second = self.traces()
        for key, value in [('network_requests',1),('dataset_origin','MANUALLY_CONSTRUCTED')]:
            replayed = self.replay([first, second]); replayed[key] = value
            with self.assertRaisesRegex(s.SliceError, 'REPLAY_ORIGIN_CHANGED'):
                s.compare_replay([first, second], replayed)


if __name__ == '__main__':
    unittest.main()
