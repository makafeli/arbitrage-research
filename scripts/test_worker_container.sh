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
    'actions': ['--migrate', '--initialize', '--run', '--follow', '--status'],
    'execution_authorized': False,
}, value
PYJSON
}

# A container without /data must not silently record into ephemeral image storage.
expect_failure 'Worker requires its persistent volume at /data' \
    docker run --rm --pull=never --network none --read-only "$image" base-ingest --check

# Entry point creates exactly the private capture directory and drops root.
"${run[@]}" "$image" sh -ceu '
    test "$(id -u)" = 10001
    test "$(id -g)" = 10001
    test "$(stat -c %u /data/captures)" = 10001
    test "$(stat -c %g /data/captures)" = 10001
    test "$(stat -c %a /data/captures)" = 700
    test -s /app/config/research.example.toml
    file=$(mktemp /data/captures/package-check.XXXXXX)
    printf %s worker-volume-check > "$file"
    sync -f "$file"
    test "$(cat "$file")" = worker-volume-check
    rm -- "$file"
    base-ingest --check
' > "$work/root.json"
verify_inert "$work/root.json"

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
printf '%s\n' 'Worker package: six container scenarios passed; no network, database, provider or trade.'
