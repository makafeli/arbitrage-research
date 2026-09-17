#!/bin/sh
# Explicit one-shot deployment check. Never connects to PostgreSQL or a provider.
set -eu
if [ "$#" -ne 0 ]; then
    echo 'VOLUME_CHECK_INVALID_ARGUMENTS' >&2
    exit 2
fi
if [ "$(id -u)" != 10001 ] || [ "$(id -g)" != 10001 ]; then
    echo 'VOLUME_CHECK_REQUIRES_UNPRIVILEGED_WORKER' >&2
    exit 2
fi
if [ -L /data ] || [ -L /data/captures ] || [ ! -d /data/captures ]; then
    echo 'VOLUME_CHECK_INVALID_CAPTURE_DIRECTORY' >&2
    exit 2
fi
test "$(stat -c %u /data/captures)" = 10001
test "$(stat -c %g /data/captures)" = 10001
test "$(stat -c %a /data/captures)" = 700
test -s /app/config/research.example.toml
umask 077
probe=$(mktemp /data/captures/.deployment-check.XXXXXX)
trap 'rm -f -- "$probe"' EXIT
printf %s worker-volume-check > "$probe"
sync -f "$probe"
test "$(cat "$probe")" = worker-volume-check
rm -- "$probe"
trap - EXIT
base-ingest --check
printf '%s\n' '{"status":"VOLUME_CHECK_PASSED","worker_started":false,"provider_requests":0,"database_requests":0,"execution_authorized":false,"uid":10001,"gid":10001,"capture_directory_mode":"0700"}'
