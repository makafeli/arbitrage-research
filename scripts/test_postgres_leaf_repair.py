#!/usr/bin/env python3
"""Real OpenSSL filesystem tests with generated synthetic keys, never production."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('postgres_leaf_repair.sh').resolve()

def openssl(*args):
    return subprocess.check_output(['openssl', *map(str, args)], stderr=subprocess.DEVNULL)

def digest(path):
    return hashlib.sha256(openssl('x509', '-in', path, '-outform', 'DER')).hexdigest()

class RepairTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.seed = tempfile.TemporaryDirectory()
        d = Path(cls.seed.name)
        openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '365',
                '-subj', '/CN=synthetic-root', '-addext', 'basicConstraints=critical,CA:TRUE',
                '-keyout', d/'root.key', '-out', d/'root.crt')
        openssl('req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=localhost',
                '-keyout', d/'server.key', '-out', d/'server.csr')
        (d/'old.ext').write_text('basicConstraints=critical,CA:TRUE\nsubjectAltName=DNS:localhost,DNS:session-db\n')
        openssl('x509', '-req', '-in', d/'server.csr', '-CA', d/'root.crt', '-CAkey', d/'root.key',
                '-set_serial', '42', '-days', '180', '-extfile', d/'old.ext', '-out', d/'server.crt')
        (d/'root.srl').write_text('unchanged-serial-marker\n')
        cls.root_pin = digest(d/'root.crt')
        cls.original_pin = digest(d/'server.crt')

    @classmethod
    def tearDownClass(cls):
        cls.seed.cleanup()

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)/'certs'
        shutil.copytree(self.seed.name, self.directory)
        self.keep = {n: (self.directory/n).read_bytes() for n in ['root.crt', 'root.key', 'server.key', 'root.srl']}
        self.before = (self.directory/'server.crt').read_bytes()
        self.original_mode = (self.directory/'server.crt').stat().st_mode

    def call(self, action, root=None, original=None, host='session-db', expect=0):
        p = subprocess.run(['bash', str(SCRIPT), action, str(self.directory), root or self.root_pin,
                            original or self.original_pin, host], capture_output=True, text=True, timeout=20)
        self.assertEqual(p.returncode, expect, p.stderr)
        self.assertNotIn('PRIVATE KEY', p.stdout+p.stderr)
        return json.loads(p.stdout if expect == 0 else p.stderr)

    def unchanged_keys(self):
        for name, value in self.keep.items():
            self.assertEqual((self.directory/name).read_bytes(), value, name)

    def test_default_is_inert(self):
        for args in [[], ['--check']]:
            p = subprocess.run(['bash',str(SCRIPT),*args],capture_output=True,text=True,check=True)
            self.assertEqual(json.loads(p.stdout)['status'],'NOT_STARTED')
        self.assertFalse((self.directory/'.arb-leaf-repair').exists())

    def test_prepare_apply_idempotent_rollback_preserves_keys_and_mode(self):
        self.assertEqual(self.call('--prepare')['status'], 'LEAF_PREPARED')
        self.assertEqual((self.directory/'server.crt').read_bytes(),self.before)
        self.unchanged_keys()
        self.assertEqual(self.call('--apply')['status'], 'LEAF_REPLACED_RELOAD_REQUIRED')
        after=(self.directory/'server.crt').read_bytes()
        self.assertNotEqual(after,self.before)
        self.assertEqual((self.directory/'server.crt').stat().st_mode,self.original_mode)
        self.assertIn(b'CA:FALSE',openssl('x509','-in',self.directory/'server.crt','-noout','-text'))
        self.assertIn(b'TLS Web Server Authentication',openssl('x509','-in',self.directory/'server.crt','-noout','-text'))
        self.call('--apply')
        self.assertEqual((self.directory/'server.crt').read_bytes(),after)
        self.assertEqual(self.call('--rollback')['status'], 'ORIGINAL_RESTORED_RELOAD_REQUIRED')
        self.assertEqual((self.directory/'server.crt').read_bytes(),self.before)
        self.call('--rollback')
        self.unchanged_keys()

    def test_wrong_root_pin_refused(self):
        self.assertEqual(self.call('--prepare',root='0'*64,expect=2)['reason'],'ROOT_ANCHOR_MISMATCH')
        self.assertEqual((self.directory/'server.crt').read_bytes(),self.before)

    def test_wrong_original_pin_refused(self):
        self.assertEqual(self.call('--prepare',original='0'*64,expect=2)['reason'],'ORIGINAL_ANCHOR_MISMATCH')

    def test_prepare_does_not_overwrite_existing_record(self):
        self.call('--prepare')
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'EXISTING_MAINTENANCE_RECORD')

    def test_apply_requires_staged_record(self):
        self.assertEqual(self.call('--apply',expect=2)['reason'],'MAINTENANCE_RECORD_REQUIRED')

    def test_changed_candidate_is_rejected(self):
        self.call('--prepare')
        (self.directory/'.arb-leaf-repair/server.next.crt').write_bytes(self.before)
        self.assertEqual(self.call('--apply',expect=2)['reason'],'MAINTENANCE_CERTIFICATE_MISMATCH')

    def test_changed_backup_is_rejected(self):
        self.call('--prepare')
        stage=self.directory/'.arb-leaf-repair'
        (stage/'server.original.crt').write_bytes((stage/'server.next.crt').read_bytes())
        self.assertEqual(self.call('--apply',expect=2)['reason'],'MAINTENANCE_CERTIFICATE_MISMATCH')

    def test_changed_host_is_rejected(self):
        self.call('--prepare')
        self.assertEqual(self.call('--apply',host='other-db',expect=2)['reason'],'MAINTENANCE_ANCHOR_MISMATCH')

    def test_symlink_cert_refused(self):
        cert=self.directory/'server.crt'; cert.rename(self.directory/'old.crt'); cert.symlink_to('old.crt')
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'CERTIFICATE_FILE_REJECTED')

    def test_symlink_directory_refused(self):
        alias=Path(self.tmp.name)/'alias'; alias.symlink_to(self.directory,target_is_directory=True)
        self.directory=alias
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'DIRECTORY_REJECTED')

    def test_world_writable_directory_refused(self):
        self.directory.chmod(0o777)
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'DIRECTORY_WRITABLE_BY_OTHERS')

    def test_changed_server_key_refused(self):
        (self.directory/'server.key').write_bytes((self.directory/'root.key').read_bytes())
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'SERVER_KEY_MISMATCH')

    def test_shell_injection_host_refused(self):
        self.assertEqual(self.call('--prepare',host='db;touch /tmp/never',expect=2)['reason'],'HOST_REJECTED')

    def test_hardlinked_certificate_refused(self):
        os.link(self.directory/'server.crt',self.directory/'linked.crt')
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'CERTIFICATE_FILE_REJECTED')

    def test_oversized_certificate_refused(self):
        (self.directory/'server.crt').write_bytes(b'x'*65537)
        self.assertEqual(self.call('--prepare',expect=2)['reason'],'CERTIFICATE_FILE_REJECTED')

    def test_busy_maintenance_refused(self):
        import fcntl
        path=self.directory/'.arb-leaf-repair.lock'; path.touch(mode=0o600)
        with path.open('r+') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            self.assertEqual(self.call('--prepare',expect=2)['reason'],'MAINTENANCE_BUSY')

    def test_no_database_or_network_commands(self):
        # This assertion describes script scope; the container test checks reload separately.
        text=SCRIPT.read_text()
        for forbidden in ['pg_ctl ', 'psql ', 's_client ', 'curl ', 'wget ', 'systemctl ', 'server.key" >']:
            self.assertNotIn(forbidden,text)

if __name__ == '__main__':
    unittest.main()
