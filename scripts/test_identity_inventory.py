"""Offline integrity tests. Mutations are fixtures, not additional chain observations."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import build_identity_inventory as inventory


class IdentityInventoryTests(unittest.TestCase):
    def setUp(self):
        self.reports = [json.loads((inventory.ROOT / path).read_bytes()) for path, _ in inventory.INPUTS.values()]

    def test_committed_inventory_reproduces_exactly(self):
        self.assertEqual(inventory.encode(inventory.build()), (inventory.ROOT / inventory.OUTPUT).read_bytes())

    def test_four_assets_and_four_pools_have_network_qualified_identity(self):
        result = inventory.build()
        for chain in result['chains']:
            self.assertEqual(len(chain['assets']), 2)
            self.assertEqual(len(chain['pools']), 2)
            self.assertTrue(chain['two_distinct_identity_candidates_observed'])
            self.assertFalse(chain['two_pool_experiments_authorized'])
            assets = [a['identity'] for a in chain['assets']]
            for pool in chain['pools']:
                self.assertEqual(pool['identity']['network_id'], chain['network_id'])
                self.assertEqual(pool['assets'], assets)
                self.assertFalse(pool['amount_specific_quote_qualified'])

    def test_identity_acceptance_does_not_authorize_execution(self):
        result = inventory.build()
        for flag in ['runtime_activation', 'execution_authorized', 'production_qualified']:
            self.assertIs(result[flag], False)
        self.assertTrue(result['same_venue_routes_permitted_in_scope'])
        self.assertIn('Sushi', result['unsupported_scope'])
        self.assertIn('Raydium', result['unsupported_scope'])

    def test_source_projection_modification_is_detected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for path, _ in inventory.INPUTS.values():
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(inventory.ROOT / path, root / path)
            path = root / inventory.INPUTS['base'][0]
            path.write_bytes(path.read_bytes() + b' ')
            with self.assertRaisesRegex(ValueError, 'REVIEWED_EVIDENCE_DIGEST_MISMATCH'):
                inventory.build(root)

    def test_fixture_and_wrong_chain_identities_are_rejected(self):
        for index in (0, 1):
            for change in ('fixture', 'wrong-network'):
                reports = copy.deepcopy(self.reports)
                asset = reports[index]['observation']['assets'][0]
                asset['address' if change == 'fixture' else 'network'] = 'fixture:wrong-identity'
                with self.subTest(index=index, change=change), self.assertRaises(ValueError):
                    inventory.assemble(*reports)
        for network in ['base-mainnet', 'solana-mainnet', 'unknown']:
            with self.assertRaises(ValueError):
                inventory.identity(network, 'fixture:pool')

    def test_duplicate_pool_and_wrong_pair_are_rejected(self):
        for index in (0, 1):
            reports = copy.deepcopy(self.reports)
            pools = reports[index]['observation']['pools']
            pools[1]['address'] = pools[0]['address']
            with self.assertRaisesRegex(ValueError, 'DUPLICATE_POOL'):
                inventory.assemble(*reports)
            reports = copy.deepcopy(self.reports)
            reports[index]['observation']['pools'][0]['token0' if index == 0 else 'mint_a'] = 'fixture:token'
            with self.assertRaisesRegex(ValueError, 'POOL_PAIR_MISMATCH'):
                inventory.assemble(*reports)

    def test_absence_stays_explicit_without_fabricated_second_pool(self):
        for index in (0, 1):
            reports = copy.deepcopy(self.reports)
            reports[index]['observation']['pools'] = reports[index]['observation']['pools'][:1]
            chain = inventory.assemble(*reports)['chains'][index]
            self.assertEqual(len(chain['pools']), 1)
            self.assertFalse(chain['two_distinct_identity_candidates_observed'])
            self.assertFalse(chain['two_pool_experiments_authorized'])

    def test_zero_liquidity_unknown_amount_or_number_is_not_eligible(self):
        for amount in ['0', None, 123, '01', '-1', '9' * 79]:
            reports = copy.deepcopy(self.reports)
            reports[0]['observation']['pools'][0]['liquidity'] = amount
            with self.subTest(amount=amount), self.assertRaises(ValueError):
                inventory.assemble(*reports)

    def test_unsupported_context_and_runtime_claims_fail_closed(self):
        changes = [(0, 'canonical_recheck_passed', False), (1, 'verified_account_slot', '1'),
                   (1, 'genesis_hash', 'fixture:genesis'), (0, 'runtime_activation', True)]
        for index, field, value in changes:
            reports = copy.deepcopy(self.reports)
            reports[index]['observation'][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                inventory.assemble(*reports)

    def test_decimals_fees_code_fingerprints_are_not_silently_coerced(self):
        reports = copy.deepcopy(self.reports)
        reports[0]['observation']['assets'][0]['decimals'] = '18'
        with self.assertRaises(ValueError):
            inventory.assemble(*reports)
        for field, value in [('fee_millionths', True), ('tick_spacing', -1), ('runtime_sha256', 'fixture:hash')]:
            reports = copy.deepcopy(self.reports)
            reports[0]['observation']['pools'][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                inventory.assemble(*reports)

    def test_authority_and_adaptive_fee_failures_are_not_eligible(self):
        for field, value in [('static_fee_supported', False), ('amount_specific_quote_qualified', True)]:
            reports = copy.deepcopy(self.reports)
            reports[1]['observation']['pools'][0][field] = value
            with self.assertRaises(ValueError):
                inventory.assemble(*reports)
        for field, value in [('frozen', True), ('delegate', 'fixture:delegate'), ('close_authority', 'fixture:authority')]:
            reports = copy.deepcopy(self.reports)
            reports[1]['observation']['pools'][0]['vault_a_state'][field] = value
            with self.assertRaises(ValueError):
                inventory.assemble(*reports)

    def test_exact_amounts_and_evidence_are_preserved_without_mutation(self):
        before = copy.deepcopy(self.reports)
        result = inventory.assemble(*self.reports)
        self.assertEqual(self.reports, before)
        self.assertEqual(result['chains'][0]['pools'][1]['active_liquidity_at_observation'], '30803223731147198624')
        self.assertEqual(result['chains'][0]['source_provenance']['workflow_run'], 34892533716)
        self.assertEqual(result['chains'][1]['source_provenance']['workflow_run'], 34839690433)
        self.assertNotEqual(result['chains'][0]['observed_until_utc'], result['chains'][1]['observed_until_utc'])

    def test_real_check_command_has_no_write(self):
        target = inventory.ROOT / inventory.OUTPUT
        before = target.read_bytes()
        run = subprocess.run([sys.executable, inventory.__file__, '--check'], capture_output=True, text=True, timeout=10)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(json.loads(run.stdout)['status'], 'IDENTITY_INVENTORY_CONSISTENT')
        self.assertEqual(target.read_bytes(), before)


if __name__ == '__main__':
    unittest.main()
