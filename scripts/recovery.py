#!/usr/bin/env python3
"""Quiesced research backup and isolated restore. Never starts workers or drops databases."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import time


class RecoveryError(ValueError):
    """A failed precondition or verification. No raw database diagnostics are exposed."""


def require(ok, message):
    if not ok:
        raise RecoveryError(message)


@dataclass(frozen=True)
class Limits:
    files: int = 50000
    bytes: int = 10 * 1024**3
    seconds: int = 300

    def __post_init__(self):
        require(all(type(v) is int and v > 0 for v in (self.files, self.bytes, self.seconds)), 'Invalid limits')
        require(self.files <= 1000000 and self.bytes <= 1024**4 and self.seconds <= 3600, 'Limits exceed safety ceiling')


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'Duplicate JSON key')
        result[key] = value
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def relative(path):
    require(isinstance(path, str) and 0 < len(path) <= 2048, 'Invalid artifact path')
    require(len(path.split('/')) <= 64, 'Artifact nesting bound exceeded')
    require(all(re.fullmatch(r'[A-Za-z0-9_.-]+', p) and p not in {'.', '..'} for p in path.split('/')), 'Unsafe artifact path')
    return path


def root_path(path):
    path = Path(path).absolute()
    require(path.resolve(strict=True) == path and path.is_dir(), 'Root and its parents must be real directories')
    return path


def new_directory(path):
    path = Path(path).absolute()
    root_path(path.parent)
    path.mkdir(mode=0o700)  # exclusive: never reuse or replace an existing destination
    return path


@contextmanager
def source_file(root, path):
    """Open every component relative to no-follow directory descriptors (POSIX only)."""
    relative(path)
    fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        parts = path.split('/')
        for part in parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd); fd = child
        file_fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        with os.fdopen(file_fd, 'rb') as source:
            before = os.fstat(source.fileno())
            require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1, 'Only single-link regular files are supported')
            yield source
            after = os.fstat(source.fileno())
            current = os.stat(parts[-1], dir_fd=fd, follow_symlinks=False)
            fields = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns, s.st_nlink)
            require(fields(before) == fields(after) == fields(current), 'File changed during read')
    finally:
        os.close(fd)


def file_record(root, path, limit, destination=None):
    h = hashlib.sha256(); size = 0
    with source_file(root, path) as source:
        target = None
        try:
            if destination is not None:
                target = os.fdopen(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb')
            while chunk := source.read(1024 * 1024):
                size += len(chunk); require(size <= limit, 'Artifact byte bound exceeded')
                h.update(chunk)
                if target is not None:
                    target.write(chunk)
            if target is not None:
                target.flush(); os.fsync(target.fileno())
        finally:
            if target is not None:
                target.close()
    return {'path': path, 'bytes': size, 'sha256': h.hexdigest()}


def inventory(root, limits, omit=()):
    root = root_path(root); paths = []; entries = 0
    def walk(fd, prefix):
        nonlocal entries
        with os.scandir(fd) as stream:
            names = []
            for entry in stream:
                entries += 1; require(entries <= limits.files * 4 + 32, 'Directory entry bound exceeded')
                names.append(entry.name)
        for name in sorted(names):
            path = relative(prefix + name)
            info = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                try: walk(child, path + '/')
                finally: os.close(child)
            else:
                require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, 'Links and special files are forbidden')
                if path not in omit:
                    paths.append(path); require(len(paths) <= limits.files, 'Artifact count bound exceeded')
    fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try: walk(fd, '')
    finally: os.close(fd)
    records = []; total = 0
    for path in sorted(paths):
        record = file_record(root, path, limits.bytes - total)
        total += record['bytes']; records.append(record)
    return records


def write_private(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600), 'wb') as output:
        output.write(data); output.flush(); os.fsync(output.fileno())


def read_json(root, path):
    with source_file(root, path) as source:
        data = source.read(16 * 1024**2 + 1)
    require(len(data) <= 16 * 1024**2, 'Metadata byte bound exceeded')
    return json.loads(data, object_pairs_hook=unique), data


def command(args, limits, *, capture=False, readonly=False):
    """Stream/hash bounded stdout with a deadline; never print connection strings or stderr."""
    env = dict(os.environ)
    env['LC_ALL'] = 'C'
    env['PGCONNECT_TIMEOUT'] = '10'
    env['PGOPTIONS'] = '-c timezone=UTC -c datestyle=ISO,YMD -c extra_float_digits=3 -c search_path=pg_catalog -c statement_timeout=' + str(limits.seconds * 1000)
    if readonly: env['PGOPTIONS'] += ' -c default_transaction_read_only=on'
    h = hashlib.sha256(); size = rows = 0; chunks = []; started = time.monotonic()
    with subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL,
                          env=env, start_new_session=True) as process:
        selector = selectors.DefaultSelector()
        try:
            selector.register(process.stdout, selectors.EVENT_READ)
            while selector.get_map():
                require(time.monotonic() - started < limits.seconds, 'Command deadline exceeded')
                for key, _ in selector.select(0.1):
                    data = os.read(key.fileobj.fileno(), 65536)
                    if not data:
                        selector.unregister(key.fileobj); continue
                    size += len(data); rows += data.count(b'\n'); h.update(data)
                    require(size <= (min(limits.bytes, 16 * 1024**2) if capture else limits.bytes), 'Command output bound exceeded')
                    if capture: chunks.append(data)
            require(process.wait(timeout=max(0.1, limits.seconds - (time.monotonic() - started))) == 0, 'PostgreSQL command failed; inspect privately')
        finally:
            selector.close()
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL); process.wait()
    return b''.join(chunks) if capture else {'rows': rows, 'bytes': size, 'sha256': h.hexdigest()}


def database_name(name):
    require(isinstance(name, str) and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]{0,62}', name), 'Use a literal database name, not a URL or connection string')
    return name


def sql(database, statement, limits, *, capture=True):
    return command(['psql', '-X', '-qAt', '-w', '-v', 'ON_ERROR_STOP=1', '--dbname', database_name(database), '-c', statement],
                   limits, capture=capture, readonly=True)


def ident(value):
    return '"' + value.replace('"', '""') + '"'


def db_evidence(database, limits):
    relations = json.loads(sql(database, "SELECT coalesce(json_agg(x ORDER BY schema,name),'[]'::json) FROM (SELECT n.nspname AS schema,c.relname AS name,c.relkind AS kind FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' AND c.relkind IN ('r','p','S','m') AND NOT c.relispartition) x", limits))
    require(len(relations) <= 1024, 'Database relation bound exceeded')
    records = []
    for relation in relations:
        table = ident(relation['schema']) + '.' + ident(relation['name'])
        expr = 'row_to_json(t)::text'
        if relation['kind'] == 'S':
            statement = f'COPY (SELECT last_value,is_called FROM {table}) TO STDOUT'
        else:
            statement = f'COPY (SELECT {expr} FROM {table} t ORDER BY {expr} COLLATE "C") TO STDOUT'
        records.append({**relation, **sql(database, statement, limits, capture=False)})
    return {'relations': records}


def copy_records(source, target, records, limits):
    for record in records:
        path = relative(record['path']); dest = target / path
        dest.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        require(file_record(source, path, limits.bytes, dest) == record, 'Copied content differs from inventory')


def backup(database, captures, output, source_commit, quiesced, limits=Limits()):
    database_name(database)
    require(quiesced is True, 'Stop all API/worker/maintenance writers and acknowledge quiescence')
    require(re.fullmatch('[0-9a-f]{40}', source_commit or ''), 'Exact source commit required')
    captures = root_path(captures); output = Path(output).absolute()
    require(not output.is_relative_to(captures), 'Backup destination must be outside capture source')
    version = int(sql(database, 'SHOW server_version_num', limits)) // 10000
    require(version >= 15, 'PostgreSQL 15 or newer required')
    before_files = inventory(captures, limits); before_db = db_evidence(database, limits)
    output = new_directory(output)
    # Incomplete output is retained without a final manifest, never passed off as a backup.
    command(['pg_dump', '-w', '--format=custom', '--dbname', database, '--file', str(output / 'database.dump')], limits, readonly=True)
    os.chmod(output / 'database.dump', 0o600)
    require((output / 'database.dump').stat().st_size <= limits.bytes, 'Dump byte bound exceeded')
    with source_file(output, 'database.dump') as dump:
        require(dump.read(5) == b'PGDMP', 'Dump is not a custom PostgreSQL archive')
    copy_records(captures, new_directory(output / 'captures'), before_files, limits)
    require(before_db == db_evidence(database, limits), 'Database changed during quiesced backup')
    require(before_files == inventory(captures, limits), 'Capture set changed during quiesced backup')
    write_private(output / 'database-evidence.json', canonical(before_db))
    records = inventory(output, limits)
    manifest = {'schema_version': 1, 'kind': 'ARBITRAGE_QUIESCED_BACKUP', 'source_commit': source_commit,
                'source_database': database, 'postgres_major': version, 'created_at': datetime.now(timezone.utc).isoformat(),
                'quiescence': 'OPERATOR_CONFIRMED_BEFORE_AFTER_EQUAL', 'capture_validation': 'FILE_BYTES_ONLY', 'files': records}
    raw = canonical(manifest); require(len(raw) <= 16 * 1024**2, 'Manifest too large')
    write_private(output / 'manifest.json', raw)
    return {'status': 'BACKUP_COMPLETE', 'manifest_sha256': digest(raw), 'files': len(records), 'source_commit': source_commit}


def verify(bundle, expected, limits=Limits()):
    require(re.fullmatch('[0-9a-f]{64}', expected or ''), 'Externally retained manifest SHA-256 required')
    bundle = root_path(bundle); manifest, raw = read_json(bundle, 'manifest.json')
    require(digest(raw) == expected, 'Manifest digest mismatch')
    require(isinstance(manifest, dict) and set(manifest) == {'schema_version', 'kind', 'source_commit', 'source_database', 'postgres_major', 'created_at', 'quiescence', 'capture_validation', 'files'}, 'Unexpected manifest fields')
    require(isinstance(manifest['created_at'], str) and datetime.fromisoformat(manifest['created_at']).utcoffset() is not None, 'Invalid backup timestamp')
    require(type(manifest.get('schema_version')) is int and manifest['schema_version'] == 1
            and manifest.get('kind') == 'ARBITRAGE_QUIESCED_BACKUP', 'Unsupported manifest')
    require(re.fullmatch('[0-9a-f]{40}', manifest.get('source_commit', '')), 'Invalid source commit')
    database_name(manifest.get('source_database'))
    require(type(manifest.get('postgres_major')) is int and manifest['postgres_major'] >= 15, 'Invalid database version')
    require(manifest.get('quiescence') == 'OPERATOR_CONFIRMED_BEFORE_AFTER_EQUAL' and manifest.get('capture_validation') == 'FILE_BYTES_ONLY', 'Unsupported evidence claims')
    records = manifest.get('files'); require(isinstance(records, list) and 2 <= len(records) <= limits.files, 'Invalid artifact list')
    names = []
    for record in records:
        require(isinstance(record, dict) and set(record) == {'path', 'bytes', 'sha256'}, 'Invalid file record')
        path = relative(record['path'])
        require(path in {'database.dump', 'database-evidence.json'} or path.startswith('captures/'), 'Unexpected artifact namespace')
        require(type(record['bytes']) is int and 0 <= record['bytes'] <= limits.bytes and re.fullmatch('[0-9a-f]{64}', record['sha256']), 'Invalid artifact size/digest')
        names.append(path)
    require(names == sorted(set(names)) and {'database.dump', 'database-evidence.json'} <= set(names), 'Duplicate, unsorted or missing artifact identities')
    require(sum(r['bytes'] for r in records) <= limits.bytes, 'Bundle byte bound exceeded')
    require(inventory(bundle, limits, omit=('manifest.json',)) == records, 'Missing, extra or altered bundle files')
    with source_file(bundle, 'database.dump') as dump:
        require(dump.read(5) == b'PGDMP', 'Not a PostgreSQL custom archive')
    evidence, _ = read_json(bundle, 'database-evidence.json')
    require(isinstance(evidence, dict) and set(evidence) == {'relations'} and isinstance(evidence['relations'], list) and len(evidence['relations']) <= 1024, 'Invalid database evidence')
    identities = []
    for row in evidence['relations']:
        require(isinstance(row, dict) and set(row) == {'schema', 'name', 'kind', 'rows', 'bytes', 'sha256'}, 'Invalid relation evidence')
        require(all(isinstance(row[k], str) and row[k] for k in ('schema', 'name')) and row['kind'] in ('r', 'p', 'S', 'm'), 'Invalid relation identity')
        require(all(type(row[k]) is int and row[k] >= 0 for k in ('rows', 'bytes')) and re.fullmatch('[0-9a-f]{64}', row['sha256']), 'Invalid relation fingerprint')
        identities.append((row['schema'], row['name']))
    require(len(identities) == len(set(identities)), 'Duplicate relation evidence')
    return manifest, evidence


def restore(bundle, expected, database, confirm_target, destination, trusted_source, limits=Limits()):
    database_name(database)
    require(re.fullmatch(r'arb_restore_[a-z0-9_]+', database) and database == confirm_target, 'Explicit isolated arb_restore_* target confirmation required')
    require(trusted_source is True, 'Restoring executes source database code; trusted source acknowledgement required')
    manifest, evidence = verify(bundle, expected, limits)  # before the first database command
    require(database != manifest['source_database'], 'Source database cannot be the restore target')
    destination = Path(destination).absolute(); root_path(destination.parent)
    require(not destination.exists() and not destination.is_symlink(), 'Capture destination must not exist')
    require(not destination.is_relative_to(Path(bundle).absolute()), 'Restore destination cannot be inside the bundle')
    require(int(sql(database, 'SHOW server_version_num', limits)) // 10000 == manifest['postgres_major'], 'Restore drill requires the same PostgreSQL major')
    nonempty = "SELECT (SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema') + (SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema') + (SELECT count(*) FROM pg_extension WHERE extname <> 'plpgsql') + (SELECT count(*) FROM pg_namespace WHERE nspname !~ '^pg_' AND nspname NOT IN ('information_schema','public')) + (SELECT count(*) FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema') + (SELECT count(*) FROM pg_event_trigger)"
    require(int(sql(database, nonempty, limits)) == 0, 'Restore database must be empty and isolated')
    destination = new_directory(destination)
    capture_records = [{**r, 'path': r['path'][9:]} for r in manifest['files'] if r['path'].startswith('captures/')]
    copy_records(root_path(Path(bundle) / 'captures'), destination, capture_records, limits)
    # Reverify before executing the archive; callers must keep the bundle private and immutable.
    verify(bundle, expected, limits)
    command(['pg_restore', '-w', '--exit-on-error', '--single-transaction', '--no-owner', '--no-acl', '--dbname', database, str(Path(bundle) / 'database.dump')], limits)
    require(db_evidence(database, limits) == evidence, 'Restored database evidence differs; target remains quarantined')
    require(inventory(destination, limits) == capture_records, 'Restored captures differ; target remains quarantined')
    return {'status': 'ISOLATED_RESTORE_VERIFIED', 'manifest_sha256': expected, 'relations': len(evidence['relations']),
            'capture_files': len(capture_records), 'workers_started': 0, 'production_accepted': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('backup', 'verify', 'restore'))
    parser.add_argument('--database'); parser.add_argument('--captures'); parser.add_argument('--output')
    parser.add_argument('--source-commit'); parser.add_argument('--quiesced', action='store_true')
    parser.add_argument('--bundle'); parser.add_argument('--manifest-sha256'); parser.add_argument('--confirm-target')
    parser.add_argument('--trusted-source', action='store_true')
    parser.add_argument('--max-files', type=int, default=50000); parser.add_argument('--max-bytes', type=int, default=10 * 1024**3)
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    try:
        require(os.name == 'posix', 'POSIX environment required')
        limits = Limits(args.max_files, args.max_bytes, args.timeout)
        if args.operation == 'backup':
            require(args.database and args.captures and args.output, 'Backup database, captures and output are required')
            result = backup(args.database, args.captures, args.output, args.source_commit, args.quiesced, limits)
        else:
            require(args.bundle and args.manifest_sha256, 'Bundle and trusted manifest digest required')
            if args.operation == 'verify':
                manifest, _ = verify(args.bundle, args.manifest_sha256, limits)
                result = {'status': 'BUNDLE_BYTES_VERIFIED', 'files': len(manifest['files']), 'raw_capture_semantics': 'NOT_VALIDATED'}
            else:
                require(args.database and args.captures, 'Restore target database and new captures path required')
                result = restore(args.bundle, args.manifest_sha256, args.database, args.confirm_target, args.captures, args.trusted_source, limits)
        print(json.dumps(result)); return 0
    except (RecoveryError, OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
        # Do not echo paths, SQL, connection details, credentials or source data.
        print(json.dumps({'status': 'RECOVERY_FAILED', 'message': 'Precondition, command or verification failed. Keep partial output/target quarantined; consult the recovery runbook.'})); return 2


if __name__ == '__main__':
    raise SystemExit(main())
