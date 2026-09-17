#!/bin/sh
set -eu
# Refuse indirection before root creates or changes the dedicated capture directory.
if [ ! -d /data ] || [ -L /data ]; then
    echo 'Worker requires its persistent volume at /data' >&2
    exit 2
fi
if [ -L /data/captures ] || { [ -e /data/captures ] && [ ! -d /data/captures ]; }; then
    echo 'Worker capture path must be a real directory, not a link or file' >&2
    exit 2
fi
# Only prepare the dedicated directory, then drop privileges. No migrations or START.
if [ "$(id -u)" = 0 ]; then
    install -d -o 10001 -g 10001 -m 700 /data/captures
    exec gosu 10001:10001 "$@"
fi
test -w /data/captures || { echo 'Worker capture directory is not writable' >&2; exit 2; }
exec "$@"
