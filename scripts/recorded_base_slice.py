#!/usr/bin/env python3
"""Explicit, disposable Base observation/control/replay experiment. Never trades."""
from __future__ import annotations

import argparse
import hashlib
import http.cookiejar
import json
import os
import platform
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import uuid

import collect_operator_base_evidence as operator_access

ROOT = Path(__file__).resolve().parents[1]
NETWORK = 'base-mainnet'
USDC = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913'
WETH = '0x4200000000000000000000000000000000000006'
MAX_FILE = 64 * 1024 * 1024
MAX_API_CALLS = 500
TIME_LIMIT = 240
RPC_MIN_INTERVAL_MS = 75


class SliceError(ValueError):
    """Fixed diagnostic; never include credentials or arbitrary provider text."""


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise SliceError(reason)


def sha(data: bytes) -> str:
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def save(path: Path, data: object) -> None:
    with path.open('x', encoding='utf-8') as stream:
        json.dump(data, stream, indent=2, ensure_ascii=True, allow_nan=False)
        stream.write('\n')


def safe_database(value: str) -> None:
    """Only the named disposable loopback database is allowed for this harness."""
    try:
        url = urllib.parse.urlsplit(value)
        require(url.scheme in ('postgres', 'postgresql') and url.hostname == '127.0.0.1'
                and url.port == 5432 and url.path == '/arb_recorded_slice'
                and not url.query and not url.fragment,
                'DISPOSABLE_DATABASE_REQUIRED')
    except (ValueError, TypeError):
        raise SliceError('DISPOSABLE_DATABASE_REQUIRED') from None


def runtime_registry(report: dict, inventory: dict) -> dict:
    """Recheck identities, then scope a bounded window; do not attest eligibility."""
    require(report.get('network') == NETWORK and report.get('status') == 'OBSERVATIONS_COLLECTED'
            and report.get('canonical_recheck_passed') is True, 'BASE_IDENTITY_INCOMPLETE')
    chain = next(c for c in inventory['chains'] if c['network_id'] == NETWORK)
    factory = chain['venue']
    require(report['factory']['address'] == factory['identity']['address']
            and report['factory']['runtime_sha256'] == factory['observed_runtime_sha256'],
            'REVIEWED_FACTORY_CHANGED')
    expected_assets = {a['identity']['address']: a for a in chain['assets']}
    require(len(report['assets']) == 2 and {a['address'] for a in report['assets']} == set(expected_assets),
            'ASSET_SET_CHANGED')
    for asset in report['assets']:
        expected = expected_assets[asset['address']]
        require(asset['decimals'] == expected['decimals']
                and asset['runtime_sha256'] == expected['observed_runtime_sha256'], 'REVIEWED_ASSET_CHANGED')
    expected_pools = {p['identity']['address']: p for p in chain['pools']}
    require(len(report['pools']) == 2 and {p['address'] for p in report['pools']} == set(expected_pools),
            'POOL_SET_CHANGED')
    pools = []
    for pool in report['pools']:
        expected = expected_pools[pool['address']]
        require(pool['status'] == 'IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED'
                and pool['token0'] == WETH and pool['token1'] == USDC
                and pool['fee_millionths'] == expected['fee_millionths']
                and pool['tick_spacing'] == expected['tick_spacing']
                and pool['runtime_sha256'] == expected['observed_runtime_sha256'], 'REVIEWED_POOL_CHANGED')
        require(type(pool['tick']) is int and -887272 <= pool['tick'] <= 887272,
                'INVALID_TICK_POSITION')
        # Only the actual current bitmap word. Quotes leaving it must reject;
        # missing ticks are never extrapolated and the window is not expanded.
        word = pool['tick'] // pool['tick_spacing'] // 256
        pools.append(dict(schema_version=1, pool=pool['address'], token0=WETH, token1=USDC,
                          fee=pool['fee_millionths'], tick_spacing=pool['tick_spacing'],
                          pool_runtime_sha256=pool['runtime_sha256'],
                          factory_runtime_sha256=report['factory']['runtime_sha256'],
                          qualification_reference='EPIC01_IDENTITIES;ISOLATED_CANDIDATE_ONLY',
                          bitmap_word_min=word, bitmap_word_max=word))
    return dict(schema_version=1, network_id=NETWORK, pools=pools)


def config_text(registry: bytes, capture_root: Path) -> str:
    source = (ROOT / 'config/research.example.toml').read_text()
    start = source.index('[networks.base]'); end = source.index('[networks.solana]')
    pools = json.loads(registry)['pools']
    section = '\n'.join([
        '[networks.base]', 'registry_id = "base-mainnet"', 'expected_evm_chain_id = 8453',
        'enabled = true', 'candidate_venue = "uniswap-v3"',
        'verified_pool_ids = ' + json.dumps([NETWORK + ':' + p['pool'] for p in pools]),
        'verified_asset_ids = ' + json.dumps([NETWORK + ':' + WETH, NETWORK + ':' + USDC]),
        'rpc_secret_reference = "env:ARB_BASE_RPC_URL"',
        'registry_qualification_digest = ' + json.dumps(sha(registry)),
        'starting_asset_id = ' + json.dumps(NETWORK + ':' + USDC), '', '',
    ])
    source = source[:start] + section + source[end:]
    for old, new in [('mode = "PAPER"', 'mode = "OBSERVE"'),
                     ('trade_sizes_minor = []', 'trade_sizes_minor = ["1000000"]'),
                     ('database_secret_reference = "UNCONFIGURED"', 'database_secret_reference = "env:TEST_DATABASE_URL"'),
                     ('capture_quota_bytes = 10737418240', 'capture_quota_bytes = 134217728'),
                     ('capture_directory = "./data/captures"', 'capture_directory = ' + json.dumps(str(capture_root)))]:
        require(source.count(old) == 1, 'CONFIG_TEMPLATE_CHANGED')
        source = source.replace(old, new)
    source += '\n[simulation]\ndelay_scenarios_ms = [0]\nfee_buffer_bps = 1000\nmaximum_state_age_ms = 60000\n'
    # A declared research freshness envelope, not current-chain or SLA evidence.
    tomllib.loads(source)
    return source


class LocalAPI:
    def __init__(self, secret: str):
        self.secret = secret
        self.csrf = ''
        self.calls = 0
        self.headers: list[dict] = []
        self.client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def call(self, path: str, data: dict | None = None, key: str | None = None) -> dict:
        require(path.startswith('/v1/') or path == '/healthz', 'INVALID_LOCAL_API_PATH')
        self.calls += 1
        require(self.calls <= MAX_API_CALLS, 'API_REQUEST_LIMIT')
        headers = {'Origin': 'http://127.0.0.1:5173', 'Content-Type': 'application/json'}
        if self.csrf: headers['X-CSRF-Token'] = self.csrf
        if key: headers['Idempotency-Key'] = key
        request = urllib.request.Request('http://127.0.0.1:8080' + path,
                                         data=json.dumps(data).encode() if data is not None else None,
                                         headers=headers)
        try:
            with self.client.open(request, timeout=4) as response:
                raw = response.read(8 * 1024 * 1024 + 1)
                require(len(raw) <= 8 * 1024 * 1024, 'API_RESPONSE_LIMIT')
                self.headers.append({'path': path, 'status': response.status,
                                     'request_id': response.headers.get('x-request-id'),
                                     'server_timing': response.headers.get('server-timing')})
                return json.loads(raw)
        except urllib.error.HTTPError as error:
            raise SliceError('LOCAL_API_HTTP_' + str(error.code)) from None
        except (OSError, json.JSONDecodeError):
            raise SliceError('LOCAL_API_UNAVAILABLE') from None

    def login(self) -> None:
        result = self.call('/v1/auth/login', {'operator_secret': self.secret})
        self.csrf = result['csrf_token']


def clean_env() -> dict[str, str]:
    return {k: v for k, v in os.environ.items()
            if not (k.startswith(('ARB_', 'TEST_', 'GH_', 'GITHUB_TOKEN')) or k in ('DATABASE_URL', 'PGPASSWORD'))}


def wait_for(check, timeout: float, reason: str):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value: return value
        time.sleep(1)
    raise SliceError(reason)


def projection(trace: dict) -> dict:
    keys = ('configuration_digest', 'calculation_version', 'network_id', 'dataset_origin',
            'capture_refs', 'route', 'amount_in_minor', 'result', 'diagnostics')
    require(trace.get('dataset_origin') == 'RECORDED_LIVE', 'NONRECORDED_DECISION')
    require(trace.get('mode') in ('OBSERVE', 'PAPER'), 'NONRESEARCH_DECISION')
    return {key: trace[key] for key in keys}


def capture_batch_key(trace: dict) -> tuple[str, tuple[tuple[str, str, str], ...]]:
    """Identify a source batch independent of route traversal order.

    Only batch selection is unordered. The comparison projection still retains
    the original ordered capture references, route legs, amounts and results.
    """
    context_fields = ('session_id', 'experiment_id', 'generation',
                      'configuration_digest', 'calculation_version', 'strategy_id',
                      'network_id', 'mode', 'dataset_origin', 'observed_at_unix_ms')
    try:
        refs = trace['capture_refs']
        require(isinstance(refs, list) and len(refs) == 2,
                'TWO_DISTINCT_CAPTURE_REFERENCES_REQUIRED')
        identity = tuple((ref['capture_id'], ref['manifest_digest'], ref['snapshot_id'])
                         for ref in refs)
        require(all(isinstance(value, str) and value for ref in identity for value in ref)
                and len({ref[0] for ref in identity}) == 2,
                'TWO_DISTINCT_CAPTURE_REFERENCES_REQUIRED')
        context = json.dumps({key: trace[key] for key in context_fields},
                             sort_keys=True, allow_nan=False)
        return context, tuple(sorted(identity))
    except (KeyError, TypeError):
        raise SliceError('INVALID_CAPTURE_BATCH_IDENTITY') from None


def batch_traces(rows: list[dict], reference: dict) -> list[dict]:
    """Select both directions from the exact session/configuration/source batch."""
    key = capture_batch_key(reference)
    return [row['trace'] for row in rows if capture_batch_key(row['trace']) == key]


def compare_replay(expected: list[dict], replayed: dict) -> int:
    """Compare the full ordered decision projections, retaining multiplicity."""
    require(replayed['network_requests'] == 0
            and replayed['dataset_origin'] == 'RECORDED_LIVE', 'REPLAY_ORIGIN_CHANGED')
    actual = replayed['decisions']
    require(bool(expected) and all(capture_batch_key(t) == capture_batch_key(expected[0])
                                  for t in expected + actual), 'REPLAY_BATCH_MISMATCH')
    canonical = lambda traces: sorted(json.dumps(projection(t), sort_keys=True)
                                      for t in traces)
    require(canonical(expected) == canonical(actual), 'REPLAY_DECISION_MISMATCH')
    return len(actual)


def private_process(args: list[str], env: dict[str, str], log: Path, children: list) -> subprocess.Popen:
    stream = log.open('xb')
    try:
        child = subprocess.Popen(args, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                 stdout=stream, stderr=stream, start_new_session=True)
    except BaseException:
        stream.close()
        raise
    children.append((child, stream))
    return child


def stop_process(child: subprocess.Popen) -> None:
    if child.poll() is None:
        os.killpg(child.pid, signal.SIGTERM)
        try: child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=4)


def exercise(root: Path, endpoint: str, database: str) -> dict:
    started = time.monotonic(); children: list = []
    result = {'schema_version': 1, 'kind': 'RECORDED_BASE_SLICE', 'status': 'INCOMPLETE',
              'execution_authorized': False, 'full_transaction_simulation': False,
              'whole_epic_accepted': False, 'upstream_prerequisites_accepted': False,
              'rpc_min_request_interval_ms': RPC_MIN_INTERVAL_MS,
              'host': {'runner': os.environ.get('RUNNER_NAME'), 'os': platform.system(),
                       'architecture': platform.machine(), 'logical_cpus': os.cpu_count(),
                       'build_profile': 'debug', 'production_capacity_verified': False}}
    work = root / 'private'; work.mkdir(mode=0o700)
    evidence = root / 'evidence'; evidence.mkdir(mode=0o700)
    session = None; api = None
    secret = secrets.token_urlsafe(40)
    try:
        report = operator_access.collect(endpoint)
        save(evidence / 'identity-observation.json', report)
        registry = json.dumps(runtime_registry(report, json.loads((ROOT/'docs/registries/initial-identities.json').read_text())), separators=(',', ':')).encode()
        registry_path = work/'registry.json'; registry_path.write_bytes(registry)
        captures = evidence/'captures'; captures.mkdir()
        configuration = config_text(registry, captures)
        config_path = work/'configuration.toml'; config_path.write_text(configuration)
        base = clean_env()
        api_env = {**base, 'ARB_DATABASE_URL': database, 'ARB_OPERATOR_SECRET': secret,
                   'ARB_PUBLIC_ORIGIN': 'http://127.0.0.1:5173', 'ARB_ALLOW_INSECURE_LOOPBACK': 'true',
                   'ARB_API_BIND_IP': '127.0.0.1', 'ARB_API_PORT': '8080', 'ARB_CONFIG_FILES': str(config_path)}
        api_child = private_process([str(ROOT/'target/debug/control-api')], api_env, work/'api.log', children)
        api = LocalAPI(secret)
        def ready_api():
            require(api_child.poll() is None, 'API_PROCESS_EXITED')
            try: return api.call('/healthz')
            except SliceError: return None
        wait_for(ready_api, 15, 'API_START_TIMEOUT'); api.login()
        capabilities = api.call('/v1/capabilities')
        require(capabilities['live_execution'] is False, 'LIVE_CAPABILITY_FORBIDDEN')
        choices = capabilities['registered_configurations']; require(len(choices) == 1, 'CONFIG_COUNT_CHANGED')
        choice = choices[0]
        session = api.call('/v1/sessions', {'network_id': NETWORK, 'mode': 'OBSERVE',
                           'configuration_digest': choice['configuration_digest'],
                           'experiment_id': 'recorded-base-' + str(uuid.uuid4()),
                           'strategy_ids': choice['strategy_ids']}, str(uuid.uuid4()))
        sid = session['session_id']; session_path = '/v1/sessions/' + sid
        result['session_id'] = sid
        result['configuration_digest'] = session['configuration_digest']
        worker_env = {**base, 'ARB_WORKER_CONFIG': str(config_path), 'ARB_POOL_REGISTRY': str(registry_path),
                      'ARB_OPERATOR_ID': 'operator', 'ARB_SESSION_ID': sid, 'TEST_DATABASE_URL': database,
                      'ARB_BASE_RPC_URL': endpoint, 'ARB_STAGE_METRICS_STDERR': '1',
                      'ARB_RPC_MIN_INTERVAL_MS': str(RPC_MIN_INTERVAL_MS)}
        worker = private_process([str(ROOT/'target/debug/research-worker')], worker_env, work/'worker.log', children)
        def checked_attempts():
            require(worker.poll() is None, 'WORKER_PROCESS_EXITED')
            attempts = api.call(session_path + '/collection-attempts?limit=100')['items']
            require(len(attempts) <= 6, 'COLLECTION_ATTEMPT_LIMIT')
            require(not any(a['outcome'] in ('ACQUISITION_FAILED', 'EVALUATION_FAILED',
                        'DEADLINE_EXCEEDED', 'WORKER_CANCELLED') for a in attempts),
                    'COLLECTION_FAILED_NO_AUTOMATIC_RETRY')
            return attempts
        def ready_worker():
            attempts = checked_attempts()
            return next((a for a in attempts if a['outcome'] == 'READINESS_COMPLETED'), None)
        result['readiness'] = wait_for(ready_worker, 70, 'READINESS_TIMEOUT')
        def command(action: str) -> dict:
            current = api.call(session_path)
            before = time.monotonic()
            receipt = api.call(session_path + '/commands', {'action': action, 'expected_revision': current['desired_revision'],
                               'reason': 'bounded recorded observation acceptance'}, str(uuid.uuid4()))
            require(receipt['status'] == 'PENDING', 'COMMAND_ACCEPTANCE_NOT_PENDING')
            def applied():
                update = api.call('/v1/commands/' + receipt['command_id'])
                require(update['status'] in ('PENDING', 'APPLIED'), 'COMMAND_REJECTED')
                return update if update['status'] == 'APPLIED' else None
            ack = wait_for(applied, 10, 'COMMAND_ACK_TIMEOUT')
            return {'accepted': receipt, 'applied': ack, 'elapsed_ms': round((time.monotonic()-before)*1000, 3)}
        result['start'] = command('START')
        def decisions():
            checked_attempts()
            return api.call('/v1/decisions?session_id=' + sid + '&limit=100')['items']
        rows = wait_for(decisions, 75, 'RECORDED_DECISION_TIMEOUT')
        result['stop'] = command('STOP')
        stopped = api.call(session_path)
        require(stopped['observed_state'] == 'STOPPED' and not stopped['execution_authorized'], 'STOP_NOT_EFFECTIVE')
        result['stopped'] = stopped
        rows = api.call('/v1/decisions?session_id=' + sid + '&limit=100')['items']
        save(evidence/'decisions.json', rows)
        save(evidence/'collection-attempts.json', api.call(session_path + '/collection-attempts?limit=100'))
        save(evidence/'coverage.json', api.call(session_path + '/collection-coverage'))
        # Freeze the worker before inspecting its raw files. API remains running.
        stop_process(worker)
        first = rows[0]['trace']; projection(first)
        same_batch = batch_traces(rows, first)
        require(len(first['capture_refs']) == 2, 'TWO_REAL_CAPTURE_REFERENCES_REQUIRED')
        request = dict(schema_version=1, session_id=sid, experiment_id=first['experiment_id'],
                       strategy_id=first['strategy_id'], network_id=NETWORK, generation=int(first['generation']),
                       observed_at_unix_ms=first['observed_at_unix_ms'], input_age_ms=first['input_age_ms'],
                       captures=[{'path': str(captures/ref['capture_id']), 'manifest_digest': ref['manifest_digest']}
                                 for ref in first['capture_refs']])
        replay_request = work/'replay-request.json'; save(replay_request, request)
        replay = subprocess.run([str(ROOT/'target/debug/replay'), '--evaluate-captures', str(replay_request)],
                                env=base, capture_output=True, timeout=25, check=False)
        require(replay.returncode == 0, 'RECORDED_REPLAY_FAILED')
        replayed = json.loads(replay.stdout)
        # Preserve the bounded replay output even when comparison fails, so
        # a later investigation does not need another provider collection.
        save(evidence/'replay.json', replayed)
        matched = compare_replay(same_batch, replayed)
        # Wait for the original 15-second lease rather than changing database rows.
        time.sleep(16)
        restarted = private_process([str(ROOT/'target/debug/research-worker')], worker_env, work/'restarted-worker.log', children)
        def recovered():
            require(restarted.poll() is None, 'RESTART_PROCESS_EXITED')
            state = api.call(session_path)
            require(state['observed_state'] not in ('RUNNING', 'PAUSED'), 'RESTART_IMPLICITLY_STARTED')
            return state if state['observed_state'] == 'STOPPED' else None
        # Observe the replacement process announcing actual recovery, not stale DB state.
        wait_for(lambda: '"event":"worker-ready"' in (work/'restarted-worker.log').read_text(), 12, 'RESTART_READY_TIMEOUT')
        result['restart'] = wait_for(recovered, 10, 'RESTART_STATE_TIMEOUT')
        require(all(result['restart'][key] == stopped[key] for key in
                    ('session_id', 'network_id', 'mode', 'configuration_digest')),
                'RESTART_CONFIGURATION_CHANGED')
        stop_process(restarted)
        before_browser = time.monotonic()
        browser_env = {**base, 'ARB_SLICE_BROWSER_SECRET': secret, 'ARB_SLICE_SESSION': sid,
                       'ARB_SLICE_EVIDENCE_DIR': str(evidence)}
        vite = private_process(['node', str(ROOT/'apps/web/node_modules/vite/bin/vite.js'), str(ROOT/'apps/web'),
                                '--host', '127.0.0.1', '--port', '5173'], base, work/'web-root.log', children)
        completed = subprocess.run(['node', str(ROOT/'apps/web/scripts/recorded-slice-browser.mjs')],
                                   env=browser_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=40)
        require(completed.returncode == 0, 'RECORDED_BROWSER_FAILED')
        result.update(status='RECORDED_ROUTE_CONTROL_REPLAY_VERIFIED',
                      source_sha=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                      worker_build_digest=sha((ROOT/'target/debug/research-worker').read_bytes()),
                      api_build_digest=sha((ROOT/'target/debug/control-api').read_bytes()),
                      replay_build_digest=sha((ROOT/'target/debug/replay').read_bytes()),
                      matched_decisions=matched, browser_elapsed_ms=round((time.monotonic()-before_browser)*1000,3))
    except (SliceError, operator_access.AccessError) as exc:
        result.update(status='BLOCKED', reason=str(exc))
    except (OSError, ValueError, KeyError, StopIteration, subprocess.SubprocessError):
        result.update(status='BLOCKED', reason='EXPERIMENT_INPUT_PROCESS_OR_OUTPUT_FAILURE')
    finally:
        for child, stream in reversed(children):
            stop_process(child); stream.close()
        result['elapsed_ms'] = round((time.monotonic()-started)*1000,3)
        if api is not None: save(evidence/'api-timing.json', api.headers)
        # Preserve fixed redacted application diagnostics and operational metrics.
        for name in ('worker.log', 'restarted-worker.log', 'api.log'):
            path = work/name
            if path.is_file():
                raw = path.read_bytes()
                require(len(raw) <= MAX_FILE and endpoint.encode() not in raw and secret.encode() not in raw,
                        'PRIVATE_LOG_EXPORT_REFUSED')
                (evidence/name).write_bytes(raw)
        save(evidence/'result.json', result)
        validate_export(evidence, [endpoint, secret])
    return result


def validate_export(root: Path, secrets_to_check: list[str]) -> None:
    """Export only bounded regular files, after checking known credential bytes."""
    needles = {value.encode() for value in secrets_to_check if value}
    for value in secrets_to_check:
        url = urllib.parse.urlsplit(value)
        if url.scheme == 'https':
            needles.update(part.encode() for part in url.path.split('/') if len(part) >= 12)
            needles.update(v.encode() for _, v in urllib.parse.parse_qsl(url.query) if len(v) >= 8)
    # A failed revalidation must not leave a stale upload authorization.
    marker = root/'EXPORT_READY'
    if marker.exists() or marker.is_symlink():
        marker.unlink()
    total = 0
    hashes = []
    for path in sorted(root.rglob('*')):
        require(not path.is_symlink(), 'EVIDENCE_SYMLINK_REFUSED')
        if path.is_dir(): continue
        require(path.is_file() and path.stat().st_size <= MAX_FILE, 'EVIDENCE_FILE_LIMIT')
        total += path.stat().st_size
        require(total <= 256 * 1024 * 1024, 'EVIDENCE_TOTAL_LIMIT')
        raw = path.read_bytes()
        require(all(value not in raw for value in needles), 'CREDENTIAL_IN_EVIDENCE')
        if path.name not in ('SHA256SUMS', 'EXPORT_READY'):
            hashes.append(hashlib.sha256(raw).hexdigest() + '  ' + path.relative_to(root).as_posix() + '\n')
    (root/'SHA256SUMS').write_text(''.join(hashes))
    (root/'EXPORT_READY').write_text('BOUNDED_CREDENTIAL_SCAN_PASSED\n')


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    if not args.run:
        print(json.dumps({'status':'NOT_RUN','network_calls':0,'production_changes':False}))
        return 0
    try:
        endpoint = operator_access.endpoint_from_environment()
        database = os.environ.get('TEST_DATABASE_URL', ''); safe_database(database)
        require(args.output is not None, 'NEW_OUTPUT_DIRECTORY_REQUIRED')
        for name in ('control-api', 'research-worker', 'replay'):
            require((ROOT/'target/debug'/name).is_file(), 'REQUIRED_BINARY_MISSING')
        # Never attach this harness to an already-running local application.
        for port in (8080, 5173):
            with socket.socket() as probe:
                try: probe.bind(('127.0.0.1', port))
                except OSError: raise SliceError('LOCAL_PORT_ALREADY_IN_USE') from None
        args.output.mkdir(mode=0o700, parents=False, exist_ok=False)
        def deadline(_signum, _frame):
            raise SliceError('EXPERIMENT_DEADLINE')
        previous = signal.signal(signal.SIGALRM, deadline)
        signal.alarm(TIME_LIMIT)
        try:
            result = exercise(args.output.resolve(), endpoint, database)
        finally:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, previous)
        validate_export(args.output.resolve()/'evidence', [endpoint])
        print(json.dumps({'status':result['status'],'reason':result.get('reason'),'whole_epic_accepted':False}))
        return 0 if result['status'] == 'RECORDED_ROUTE_CONTROL_REPLAY_VERIFIED' else 2
    except (SliceError, operator_access.AccessError) as exc:
        print(json.dumps({'status':'START_BLOCKED','reason':str(exc)}))
    except (OSError, ValueError, TypeError):
        print(json.dumps({'status':'START_BLOCKED','reason':'INVALID_LOCAL_INPUT'}))
    return 2


if __name__ == '__main__':
    raise SystemExit(main())
