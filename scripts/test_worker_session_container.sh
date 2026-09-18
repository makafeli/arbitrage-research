#!/usr/bin/env bash
# Real session registration against an isolated disposable database; never a provider.
set -euo pipefail
umask 077
image=${1:-arb-worker:test}
api_image=${2:-arb-api:test}
pg='postgres:17.11@sha256:67f41722b7a8cbdb868a44a4995c846eddfdc2973bccb291ce937dce88ad5675'
work=$(mktemp -d)
network='' database='' volume=''
cleanup() {
    if [[ -n "$database" ]]; then docker rm -f "$database" >/dev/null; fi
    if [[ -n "$volume" ]]; then docker volume rm "$volume" >/dev/null; fi
    if [[ -n "$network" ]]; then docker network rm "$network" >/dev/null; fi
    rm -rf -- "$work"
}
trap cleanup EXIT
network=$(docker network create --internal --label arb.session-test=true "arb-session-$(basename "$work")")
volume=$(docker volume create --label arb.session-test=true)
database=$(docker run -d --rm --network "$network" --network-alias session-db --network-alias wrong-db \
    -e POSTGRES_PASSWORD=disposable-session-only "$pg")
for attempt in $(seq 1 40); do
    if docker exec "$database" pg_isready -U postgres >/dev/null; then break; fi
    sleep 1
done
docker exec "$database" pg_isready -U postgres >/dev/null
for migration in migrations/[0-9]*.sql; do
    docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 < "$migration"
done
# Prepare the exact production file shape with explicitly synthetic observations.
docker run --rm --pull=never --network none --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m \
    --mount "type=volume,source=$volume,target=/data" \
    --mount "type=bind,source=$PWD/scripts/test_worker_prepare_base.py,target=/tests/sample.py,readonly" \
    "$image" python3 -I -c '
import runpy,sys
from pathlib import Path
sys.path.insert(0,"/app/scripts")
import worker_prepare_base as p
_,report=runpy.run_path("/tests/sample.py")["sample"]()
p.prepare(Path("/data/runtime"),"not-a-provider",lambda _:report)
' > /dev/null
anchor=$(docker run --rm --pull=never --network none --read-only --user 10001:10001 \
    --mount "type=volume,source=$volume,target=/data,readonly" --entrypoint python3 "$image" \
    -I -c 'import json;print(json.load(open("/data/runtime/base-v1/profile.json"))["configuration_digest"])')
# Ephemeral test CA and hostname-bound end-entity certificate, never production trust.
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=isolated-session-ca' \
    -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' \
    -keyout "$work/ca.key" -out "$work/ca.crt" >/dev/null 2>&1
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=wrong-session-ca' \
    -keyout "$work/wrong.key" -out "$work/wrong.crt" >/dev/null 2>&1
openssl req -new -newkey rsa:2048 -nodes -subj '/CN=session-db' \
    -keyout "$work/server.key" -out "$work/server.csr" >/dev/null 2>&1
cat > "$work/server.ext" <<'CERT'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:session-db
CERT
openssl x509 -req -in "$work/server.csr" -CA "$work/ca.crt" -CAkey "$work/ca.key" \
    -CAcreateserial -days 1 -extfile "$work/server.ext" -out "$work/server.crt" >/dev/null 2>&1
run=(docker run --rm --pull=never --network "$network" --read-only
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m
    --mount "type=volume,source=$volume,target=/data"
    -e "ARB_BASE_PROFILE_DIGEST=$anchor"
    -e "PGSSLROOTCERT=$(cat "$work/ca.crt")"
    -e "ARB_DATABASE_CA_PEM=$(cat "$work/ca.crt")"
    -e ARB_DATABASE_URL=postgres://postgres:disposable-session-only@session-db/postgres)
expect_failure() {
    local reason=$1; shift
    local status=0
    "$@" > "$work/refused.out" 2> "$work/refused.err" || status=$?
    if [[ "$status" -ne 2 ]] || ! grep -F "$reason" "$work/refused.err" >/dev/null; then
        # Fixed test labels only; never echo command arguments, PEM or child output.
        printf 'Expected refusal did not match: %s (exit %s)\n' "$reason" "$status" >&2
        return 1
    fi
    printf 'Verified refusal: %s\n' "$reason"
    ! grep -F 'disposable-session-only' "$work/refused.err" >/dev/null
}
wait_server_certificate() {
    local expected actual=''
    expected=$(openssl x509 -in "$1" -noout -fingerprint -sha256)
    # Check the actual peer, not the configured path or a fixed sleep. A failed
    # PostgreSQL reload retains the old certificate and must fail this test.
    for attempt in $(seq 1 20); do
        actual=$(docker exec "$database" sh -ceu '
            timeout 4 openssl s_client -starttls postgres -connect session-db:5432 \
                -servername session-db </dev/null 2>/dev/null \
                | openssl x509 -noout -fingerprint -sha256
        ' 2>/dev/null) || actual=''
        if [[ "$actual" == "$expected" ]]; then return 0; fi
        sleep 0.2
    done
    printf '%s\n' 'PostgreSQL did not serve the expected synthetic certificate.' >&2
    return 1
}
# The registration CLI must refuse a reachable plaintext-only server.
expect_failure DATABASE_UNAVAILABLE "${run[@]}" -e ARB_OPERATOR_ID=operator "$image" \
    worker-session --register /data/runtime/base-v1
docker cp "$work/server.key" "$database:/tmp/session.key"
docker cp "$work/server.crt" "$database:/tmp/session.crt"
docker exec "$database" sh -ceu 'chown postgres:postgres /tmp/session.key /tmp/session.crt; chmod 600 /tmp/session.key'
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'SQL'
ALTER SYSTEM SET ssl_cert_file='/tmp/session.crt';
ALTER SYSTEM SET ssl_key_file='/tmp/session.key';
ALTER SYSTEM SET ssl='on';
SELECT pg_reload_conf();
SQL
tls_ready=false
for attempt in $(seq 1 20); do
    if docker exec -e PGPASSWORD=disposable-session-only -e PGSSLMODE=require "$database" \
       psql -XqAt -h session-db -U postgres -v ON_ERROR_STOP=1 \
       -c 'SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()' > "$work/tls" 2>/dev/null \
       && grep -Fx t "$work/tls" >/dev/null; then tls_ready=true; break; fi
    sleep 1
done
test "$tls_ready" = true
wait_server_certificate "$work/server.crt"
# The shipped Rust session checker must not downgrade verify-full to require.
expect_failure DATABASE_UNAVAILABLE "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e "PGSSLROOTCERT=$(cat "$work/wrong.crt")" "$image" \
    worker-session --status /data/runtime/base-v1
expect_failure DATABASE_UNAVAILABLE "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e ARB_DATABASE_URL=postgres://postgres:disposable-session-only@wrong-db/postgres \
    "$image" worker-session --status /data/runtime/base-v1
# Reproduce the production template's CA:TRUE leaf using synthetic keys only.
# A trusted CA and correct SAN are insufficient: reject a CA used as a server leaf.
sed 's/CA:FALSE/CA:TRUE/' "$work/server.ext" > "$work/ca-leaf.ext"
openssl x509 -req -in "$work/server.csr" -CA "$work/ca.crt" -CAkey "$work/ca.key" \
    -CAcreateserial -days 1 -extfile "$work/ca-leaf.ext" -out "$work/ca-leaf.crt" >/dev/null 2>&1
docker cp "$work/ca-leaf.crt" "$database:/tmp/ca-leaf.crt"
# mktemp/umask produces a root-owned 0600 copy; the server must be able to read it.
docker exec "$database" sh -ceu 'chown postgres:postgres /tmp/ca-leaf.crt; chmod 600 /tmp/ca-leaf.crt'
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'SQL'
ALTER SYSTEM SET ssl_cert_file='/tmp/ca-leaf.crt';
SELECT pg_reload_conf();
SQL
wait_server_certificate "$work/ca-leaf.crt"
expect_failure DATABASE_UNAVAILABLE "${run[@]}" -e ARB_OPERATOR_ID=operator "$image" \
    worker-session --status /data/runtime/base-v1
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'SQL'
ALTER SYSTEM SET ssl_cert_file='/tmp/session.crt';
SELECT pg_reload_conf();
SQL
wait_server_certificate "$work/server.crt"
expect_failure BASE_SESSION_NOT_REGISTERED "${run[@]}" -e ARB_OPERATOR_ID=operator "$image" \
    worker-session --status /data/runtime/base-v1
"${run[@]}" -e ARB_OPERATOR_ID=operator "$image" worker-session --register /data/runtime/base-v1 > "$work/first.json"
"${run[@]}" -e ARB_OPERATOR_ID=operator "$image" worker-session --register /data/runtime/base-v1 > "$work/reused.json"
"${run[@]}" -e ARB_OPERATOR_ID=operator "$image" worker-session --status /data/runtime/base-v1 > "$work/status.json"
# The launcher also authenticates the server before any source initialization.
expect_failure DATABASE_CA_REQUIRED "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e ARB_DATABASE_CA_PEM= -e ARB_BASE_RPC_URL=https://rpc.invalid/unused \
    -e ARB_RPC_MIN_INTERVAL_MS=75 "$image" worker-launch-base --initialize-and-start
expect_failure REGISTERED_SESSION_REQUIRED "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e "ARB_DATABASE_CA_PEM=$(cat "$work/wrong.crt")" \
    -e ARB_BASE_RPC_URL=https://rpc.invalid/unused -e ARB_RPC_MIN_INTERVAL_MS=75 \
    "$image" worker-launch-base --initialize-and-start
expect_failure REGISTERED_SESSION_REQUIRED "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e ARB_DATABASE_URL=postgres://postgres:disposable-session-only@wrong-db/postgres \
    -e ARB_BASE_RPC_URL=https://rpc.invalid/unused -e ARB_RPC_MIN_INTERVAL_MS=75 \
    "$image" worker-launch-base --initialize-and-start
# The shipped launch wrapper never initializes a missing source on normal startup.
# The reserved .invalid endpoint cannot receive requests; the missing-source check
# must happen before the existing worker is executed.
expect_failure EXISTING_SOURCE_REQUIRED "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e ARB_BASE_RPC_URL=https://rpc.invalid/unused -e ARB_RPC_MIN_INTERVAL_MS=75 \
    "$image" worker-launch-base --start
"${run[@]}" -e ARB_OPERATOR_ID=operator "$image" worker-launch-base --check > "$work/launch-inert.json"
python3 - "$work/launch-inert.json" <<'PYLAUNCH'
import json,sys
v=json.load(open(sys.argv[1])); assert v == dict(status='NOT_STARTED', provider_requests=0,
    database_requests=0, execution_authorized=False)
PYLAUNCH
# Same-key first-use races must retain only one session, with no provider endpoint.
"${run[@]}" -e ARB_OPERATOR_ID=race "$image" worker-session --register /data/runtime/base-v1 > "$work/race1.json" &
a=$!
"${run[@]}" -e ARB_OPERATOR_ID=race "$image" worker-session --register /data/runtime/base-v1 > "$work/race2.json" &
b=$!
s1=0; s2=0
wait "$a" || s1=$?
wait "$b" || s2=$?
test "$s1" -eq 0 && test "$s2" -eq 0
expect_failure PROFILE_ANCHOR_MISMATCH "${run[@]}" -e ARB_OPERATOR_ID=operator \
    -e "ARB_BASE_PROFILE_DIGEST=sha256:$(printf '%064d' 0)" "$image" worker-session --register /data/runtime/base-v1
docker exec "$database" psql -XqAt -U postgres -v ON_ERROR_STOP=1 -c \
    "SELECT json_build_object('sessions',(SELECT count(*) FROM research_sessions),'configurations',(SELECT count(*) FROM configuration_snapshots),'commands',(SELECT count(*) FROM control_commands),'streams',(SELECT count(*) FROM ingestion_streams),'audit',(SELECT count(*) FROM control_audit_events));" > "$work/counts.json"
python3 - "$work" "$anchor" <<'PY'
import json,sys
from pathlib import Path
root=Path(sys.argv[1]); read=lambda n:json.loads((root/n).read_text())
a,b,c=read('first.json'),read('reused.json'),read('status.json')
assert a['status']==b['status']==c['status']=='BASE_SESSION_REGISTERED'
assert a['session']==b['session']==c['session']
s=a['session']; assert s['mode']=='OBSERVE' and s['network_id']=='base-mainnet'
assert s['configuration_digest']==sys.argv[2] and s['observed_state']=='RECOVERING'
assert s['desired_revision']==s['applied_revision']=='0'
assert s['execution_authorized'] is False and s['last_heartbeat_at'] is None
assert b['registration_requested'] is False
for f in ('first.json','reused.json','status.json','race1.json','race2.json'):
    v=read(f); assert v['provider_requests']==0 and v['worker_started'] is False
assert read('race1.json')['session']['session_id']==read('race2.json')['session']['session_id']
assert read('counts.json')==dict(sessions=2,configurations=2,commands=0,streams=0,audit=2)
PY
# Validate matching API catalog inputs with the shipped wrapper, still no network.
docker run --rm --pull=never --network none --read-only --user 10001:10001 \
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m \
    --mount "type=volume,source=$volume,target=/inputs,readonly" \
    -e "ARB_BASE_PROFILE_DIGEST=$anchor" --entrypoint sh "$api_image" -ceu '
    ARB_BASE_PROFILE_TOML=$(cat /inputs/runtime/base-v1/configuration.toml)
    ARB_BASE_PROFILE_REGISTRY=$(cat /inputs/runtime/base-v1/registry.json)
    export ARB_BASE_PROFILE_TOML ARB_BASE_PROFILE_REGISTRY
    api-entrypoint --check-profile
' > "$work/api-profile.json"
python3 - "$work/api-profile.json" "$anchor" <<'PY'
import json,sys
v=json.load(open(sys.argv[1])); assert v['status']=='PROFILE_VALIDATED'
assert v['configuration_digest']==sys.argv[2] and v['provider_requests']==v['database_requests']==0
PY
expect_failure API_BASE_PROFILE_INCOMPLETE docker run --rm --pull=never --network none --read-only \
    -e "ARB_BASE_PROFILE_DIGEST=$anchor" "$api_image" --check-profile
docker run --rm --pull=never --network none --read-only "$api_image" --check-profile > "$work/api-inert.json"
grep -F 'API_PROFILE_NOT_CONFIGURED' "$work/api-inert.json" >/dev/null
printf '%s\n' 'Anchored session registration and API profile passed: verified TLS, wrong CA/hostname/CA-leaf refusal, same-key reuse, concurrent registration, wrong anchor refusal, no commands/streams/provider calls.'
