#!/usr/bin/env bash
# Run only against a locally built image and a newly allocated disposable volume.
set -euo pipefail
umask 077
image="${1:-arb-worker:test}"
docker image inspect "$image" >/dev/null
work="$(mktemp -d)"
volume=""
cleanup() {
    if [[ -n "$volume" ]]; then docker volume rm "$volume" >/dev/null; fi
    rm -rf -- "$work"
}
trap cleanup EXIT
volume="$(docker volume create --label arb.worker-package-test=true)"
run=(docker run --rm --pull=never --network none --read-only
    --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m
    --mount "type=volume,source=$volume,target=/data")
expect_failure() {
    local expected="$1"; shift
    local status=0
    "$@" >"$work/refusal.log" 2>&1 || status=$?
    test "$status" -eq 2
    grep -F -- "$expected" "$work/refusal.log" >/dev/null
}
verify_inert() {
    python3 - "$1" <<'PYJSON'
import json, sys
with open(sys.argv[1], encoding='utf-8') as stream:
    value = json.load(stream)
assert value == {
    'status': 'NOT_STARTED', 'provider_requests': 0,
    'actions': ['--migrate', '--initialize', '--run', '--follow', '--status', '--rotate'],
    'execution_authorized': False,
}, value
PYJSON
}

# A container without /data must not silently record into ephemeral image storage.
expect_failure 'Worker requires its persistent volume at /data' \
    docker run --rm --pull=never --network none --read-only "$image" base-ingest --check

# Test the same finite command used for the hosted storage preflight.
"${run[@]}" "$image" worker-volume-check > "$work/preflight.jsonl"
python3 - "$work/preflight.jsonl" <<'PYJSON'
import json, sys
with open(sys.argv[1], encoding='utf-8') as stream:
    records = [json.loads(line) for line in stream]
assert len(records) == 2
assert records[0]['status'] == 'NOT_STARTED'
assert records[0]['provider_requests'] == 0
assert records[0]['execution_authorized'] is False
assert records[1] == {
    'status': 'VOLUME_CHECK_PASSED', 'worker_started': False,
    'provider_requests': 0, 'database_requests': 0, 'execution_authorized': False,
    'uid': 10001, 'gid': 10001, 'capture_directory_mode': '0700',
}
PYJSON

# An already unprivileged deployment can use the same prepared volume.
"${run[@]}" --user 10001:10001 "$image" base-ingest --check > "$work/nonroot.json"
verify_inert "$work/nonroot.json"

# The real binary must reject missing initialization settings without a network.
expect_failure 'REQUIRED_SETTING_MISSING' "${run[@]}" "$image" base-ingest --initialize

# Malformed volume contents are refused before root follows a link or touches a file.
"${run[@]}" --entrypoint sh "$image" -ceu 'rmdir /data/captures; ln -s /tmp /data/captures'
expect_failure 'Worker capture path must be a real directory' \
    "${run[@]}" "$image" base-ingest --check
"${run[@]}" --entrypoint sh "$image" -ceu 'rm /data/captures; touch /data/captures'
expect_failure 'Worker capture path must be a real directory' \
    "${run[@]}" "$image" base-ingest --check
# Deployment guards refuse missing source or database metadata without networking.
expect_failure 'CI_SOURCE_REJECTED' docker run --rm --pull=never --network none --read-only \
    --entrypoint python3 "$image" -I /usr/local/lib/arb/worker_ci_gate.py
expect_failure 'OPERATOR_SETTING_MISSING_OR_INVALID' docker run --rm --pull=never --network none --read-only \
    --entrypoint python3 "$image" -I /usr/local/lib/arb/worker_readiness.py
printf '%s\n' 'Worker package: eight container scenarios passed; no network, database, provider or trade.'
