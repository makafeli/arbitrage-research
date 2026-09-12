#!/bin/sh
set -eu
# Prepare only the dedicated volume directory, then drop privileges.
if [ "$(id -u)" = 0 ]; then
    test -d /data || { echo 'Worker requires its persistent volume at /data' >&2; exit 2; }
    install -d -o 10001 -g 10001 -m 700 /data/captures
    exec gosu 10001:10001 "$@"
fi
test -w /data/captures || { echo 'Worker capture directory is not writable' >&2; exit 2; }
exec "$@"
