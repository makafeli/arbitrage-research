#!/usr/bin/env bash
# Isolated real PostgreSQL + shipped Rust verifier. Synthetic keys/data only.
set -euo pipefail
umask 077
image=${1:-arb-worker:test}
pg='postgres:17.11@sha256:67f41722b7a8cbdb868a44a4995c846eddfdc2973bccb291ce937dce88ad5675'
work=$(mktemp -d); network=''; database=''; volume=''
cleanup() {
    [[ -z "$database" ]] || docker rm -f "$database" >/dev/null
    [[ -z "$volume" ]] || docker volume rm "$volume" >/dev/null
    [[ -z "$network" ]] || docker network rm "$network" >/dev/null
    rm -rf -- "$work"
}
trap cleanup EXIT
network=$(docker network create --internal "arb-leaf-$(basename "$work")")
volume=$(docker volume create)
database=$(docker run -d --rm --network "$network" --network-alias session-db \
    -e POSTGRES_PASSWORD=disposable-leaf-only "$pg")
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
python3 - "$work" <<'PY'
import runpy,shutil,sys
from pathlib import Path
cls=runpy.run_path('scripts/test_postgres_leaf_repair.py')['RepairTests']
cls.setUpClass()
try: shutil.copytree(cls.seed.name,Path(sys.argv[1])/'certs')
finally: cls.tearDownClass()
PY
docker cp "$work/certs" "$database:/tmp/certs"
docker cp scripts/postgres_leaf_repair.sh "$database:/tmp/postgres_leaf_repair.sh"
docker exec "$database" sh -ceu 'chown -R postgres:postgres /tmp/certs; chmod 700 /tmp/certs; chmod 755 /tmp/postgres_leaf_repair.sh'
root_pin=$(openssl x509 -in "$work/certs/root.crt" -noout -fingerprint -sha256 | cut -d= -f2 | tr -d ':' | tr A-F a-f)
original_pin=$(openssl x509 -in "$work/certs/server.crt" -noout -fingerprint -sha256 | cut -d= -f2 | tr -d ':' | tr A-F a-f)
docker exec -i "$database" psql -Xq -U postgres -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE leaf_maintenance_sentinel (id integer PRIMARY KEY, payload text NOT NULL);
INSERT INTO leaf_maintenance_sentinel VALUES (1,'preserve-me'), (2,'also-preserve-me');
ALTER SYSTEM SET ssl_cert_file='/tmp/certs/server.crt';
ALTER SYSTEM SET ssl_key_file='/tmp/certs/server.key';
ALTER SYSTEM SET ssl='on';
SELECT pg_reload_conf();
SQL
# Exact runtime profile shape, with explicit synthetic observations.
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
' >/dev/null
anchor=$(docker run --rm --pull=never --network none --read-only --user 10001:10001 \
    --mount "type=volume,source=$volume,target=/data,readonly" --entrypoint python3 "$image" \
    -I -c 'import json;print(json.load(open("/data/runtime/base-v1/profile.json"))["configuration_digest"])')
run=(docker run --rm --pull=never --network "$network" --read-only
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m
    --mount "type=volume,source=$volume,target=/data"
    -e "ARB_BASE_PROFILE_DIGEST=$anchor" -e ARB_OPERATOR_ID=operator
    -e "PGSSLROOTCERT=$(cat "$work/certs/root.crt")"
    -e ARB_DATABASE_URL=postgres://postgres:disposable-leaf-only@session-db/postgres)
expect_status() {
    local reason=$1 code=0
    "${run[@]}" "$image" worker-session --status /data/runtime/base-v1 >"$work/status.out" 2>"$work/status.err" || code=$?
    if [[ "$code" != 2 ]] || ! grep -F "$reason" "$work/status.err" >/dev/null; then
        printf 'Rust verification expectation failed: %s\n' "$reason" >&2; return 1
    fi
    printf 'Rust verification result: %s\n' "$reason"
}
wait_peer() {
    local expected=$1 actual=''
    for attempt in $(seq 1 40); do
        actual=$(docker exec "$database" sh -ceu '
            timeout 3 openssl s_client -starttls postgres -connect session-db:5432 -servername session-db </dev/null 2>/dev/null |
            openssl x509 -noout -fingerprint -sha256 | cut -d= -f2 | tr -d ":" | tr A-F a-f
        ' 2>/dev/null) || actual=''
        if [[ "$actual" == "$expected" ]]; then return 0; fi
        sleep 0.2
    done
    printf '%s\n' 'PostgreSQL did not serve the expected synthetic leaf.' >&2; return 1
}
manifest() {
    docker exec "$database" sh -ceu 'sha256sum /tmp/certs/root.crt /tmp/certs/root.key /tmp/certs/server.key /tmp/certs/root.srl'
    docker exec "$database" psql -XqAt -U postgres -v ON_ERROR_STOP=1 \
        -c "SELECT pg_postmaster_start_time(); SELECT id,payload FROM leaf_maintenance_sentinel ORDER BY id; SELECT count(*) FROM research_sessions; SELECT count(*) FROM ingestion_streams;"
}
wait_peer "$original_pin"
expect_status DATABASE_UNAVAILABLE
manifest > "$work/before"
docker exec --user postgres "$database" bash /tmp/postgres_leaf_repair.sh --prepare /tmp/certs "$root_pin" "$original_pin" session-db
# Preparation cannot change the active certificate.
wait_peer "$original_pin"
docker exec --user postgres "$database" bash /tmp/postgres_leaf_repair.sh --apply /tmp/certs "$root_pin" "$original_pin" session-db
next_pin=$(docker exec "$database" cat /tmp/certs/.arb-leaf-repair/next.sha256)
# Reload only: same postmaster and same database contents must survive.
docker exec --user postgres "$database" sh -ceu 'pg_ctl reload -D "$PGDATA"'
wait_peer "$next_pin"
# This code is emitted only after a verified database read; no session is created.
expect_status BASE_SESSION_NOT_REGISTERED
manifest > "$work/after"
cmp "$work/before" "$work/after"
docker exec --user postgres "$database" bash /tmp/postgres_leaf_repair.sh --rollback /tmp/certs "$root_pin" "$original_pin" session-db
docker exec --user postgres "$database" sh -ceu 'pg_ctl reload -D "$PGDATA"'
wait_peer "$original_pin"
expect_status DATABASE_UNAVAILABLE
manifest > "$work/rollback"
cmp "$work/before" "$work/rollback"
printf '%s\n' 'Leaf maintenance passed: prepare/apply/reload/Rust verify/rollback; same postmaster, keys, CA, data, and zero sessions/streams.'
