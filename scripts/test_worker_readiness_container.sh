#!/usr/bin/env bash
# Actual read-only PostgreSQL inspection in disposable private containers only.
set -euo pipefail
umask 077
image="${1:-arb-worker:test}"
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
docker image inspect "$image" >/dev/null
network=$(docker network create --label arb.readiness-test=true "arb-readiness-$(basename "$work")")
volume=$(docker volume create --label arb.readiness-test=true)
database=$(docker run -d --rm --network "$network" --network-alias readiness-db \
    -e POSTGRES_PASSWORD=disposable-readiness-admin "$pg")
# TCP on purpose: the image init starts a socket-only temporary server; a socket
# pg_isready passes during init and the next psql hits "database system is shutting down".
for attempt in $(seq 1 40); do
    if docker exec "$database" pg_isready -h 127.0.0.1 -U postgres >/dev/null; then break; fi
    sleep 1
done
docker exec "$database" pg_isready -h 127.0.0.1 -U postgres >/dev/null
for migration in migrations/[0-9]*.sql; do
    docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 < "$migration"
done
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'SQL'
CREATE ROLE metadata_reader LOGIN PASSWORD 'disposable-readiness-only';
GRANT CONNECT ON DATABASE postgres TO metadata_reader;
GRANT USAGE ON SCHEMA public TO metadata_reader;
GRANT SELECT ON public.configuration_snapshots,public.research_sessions,public.ingestion_streams TO metadata_reader;
INSERT INTO configuration_snapshots(operator_id,configuration_digest,snapshot) VALUES
 ('operator','sha256:'||repeat('a',64),'{"mode":"OBSERVE","networks":{"base":{"enabled":true}},"private_sentinel":"NOT_FOR_LOGS"}'),
 ('other','sha256:'||repeat('b',64),'{"mode":"OBSERVE"}');
INSERT INTO research_sessions(session_id,operator_id,network_id,mode,configuration_digest,experiment_id,strategy_ids,observed_state,lifecycle) VALUES
 ('ci-session-1','operator','base-mainnet','OBSERVE','sha256:'||repeat('a',64),'ci-only','[]','STOPPED','{}'),
 ('ci-other','other','base-mainnet','OBSERVE','sha256:'||repeat('b',64),'ci-only','[]','STOPPED','{}');
INSERT INTO ingestion_streams(operator_id,stream_id,binding,initial_checkpoint,checkpoint) VALUES
 ('operator','ci-stream-1',jsonb_build_object('network_id','base-mainnet','registry_digest','sha256:'||repeat('c',64)),'{"number":123}','{"number":123}');
SQL
snapshot() {
    docker exec "$database" psql -XqAt -U postgres -v ON_ERROR_STOP=1 -c \
      "SELECT md5(COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY session_id)::text FROM research_sessions t),'') || COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY stream_id)::text FROM ingestion_streams t),''));"
}
snapshot > "$work/before"
# The production default must refuse an otherwise reachable non-TLS server.
status=0
docker run --rm --pull=never --network "$network" --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m \
    --mount "type=volume,source=$volume,target=/data" \
    -e ARB_OPERATOR_ID=operator \
    -e ARB_DATABASE_URL=postgres://metadata_reader:disposable-readiness-only@readiness-db/postgres \
    "$image" worker-readiness-check > "$work/no-tls.jsonl" 2> "$work/no-tls.err" || status=$?
test "$status" -eq 2
grep -F 'DATABASE_READ_UNAVAILABLE' "$work/no-tls.err" >/dev/null
! grep -F 'WORKER_READINESS_INSPECTED' "$work/no-tls.jsonl" >/dev/null

# Ephemeral self-signed TLS proves encryption, not authenticated server identity.
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=readiness-db' \
    -keyout "$work/server.key" -out "$work/server.crt" >/dev/null 2>&1
docker cp "$work/server.key" "$database:/tmp/readiness.key"
docker cp "$work/server.crt" "$database:/tmp/readiness.crt"
docker exec "$database" sh -ceu '
    chown postgres:postgres /tmp/readiness.key /tmp/readiness.crt
    chmod 600 /tmp/readiness.key
'
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'TLS_SQL'
ALTER SYSTEM SET ssl_cert_file = '/tmp/readiness.crt';
ALTER SYSTEM SET ssl_key_file = '/tmp/readiness.key';
ALTER SYSTEM SET ssl = 'on';
SELECT pg_reload_conf();
TLS_SQL
# Verify the same SELECT-only role can actually establish an encrypted connection.
tls_ready=false
for attempt in $(seq 1 20); do
    if docker exec -e PGPASSWORD=disposable-readiness-only -e PGSSLMODE=require \
       "$database" psql -XqAt -h readiness-db -U metadata_reader -d postgres \
       -v ON_ERROR_STOP=1 -c 'SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()' \
       > "$work/tls" 2>/dev/null && grep -Fx t "$work/tls" >/dev/null; then
        tls_ready=true; break
    fi
    sleep 1
done
test "$tls_ready" = true

docker run --rm --pull=never --network "$network" --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m \
    --mount "type=volume,source=$volume,target=/data" \
    -e ARB_OPERATOR_ID=operator \
    -e ARB_DATABASE_URL=postgres://metadata_reader:disposable-readiness-only@readiness-db/postgres \
    "$image" worker-readiness-check > "$work/readiness.jsonl"
snapshot > "$work/after"
cmp "$work/before" "$work/after"
python3 - "$work/readiness.jsonl" <<'PY'
import json,sys
text=open(sys.argv[1],encoding='utf-8').read()
rows=[json.loads(line) for line in text.splitlines()]
assert len(rows)==3
assert rows[1]['status']=='VOLUME_CHECK_PASSED'
v=rows[2]
assert v['status']=='WORKER_READINESS_INSPECTED'
assert v['database_read_only'] is True and v['runtime_qualified'] is False
assert v['worker_started'] is False and v['execution_authorized'] is False
assert v['provider_requests']==0
s=v['state']
assert s['configuration_count']==s['enabled_base_profile_count']==s['session_count']==s['stream_count']=='1'
assert s['sessions'][0]['session_id']=='ci-session-1'
assert s['streams'][0]['stream_id']=='ci-stream-1' and s['streams'][0]['revision']=='0'
assert 'NOT_FOR_LOGS' not in text and 'disposable-readiness' not in text and 'ci-other' not in text
assert 'ARB_BASE_RPC_URL' in v['missing_settings']
PY
printf '%s\n' 'Readiness PostgreSQL inspection passed with required TLS and a SELECT-only role; plaintext refused; state unchanged; no provider requests.'
