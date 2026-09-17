#!/bin/sh
set -eu
if [ ! -d /data ] || [ -L /data ]; then
    echo 'Worker requires its persistent volume at /data' >&2
    exit 2
fi
# Both subdirectories are private to one worker; never follow links during root setup.
for directory in /data/captures /data/runtime; do
    if [ -L "$directory" ] || { [ -e "$directory" ] && [ ! -d "$directory" ]; }; then
        echo 'Worker capture path must be a real directory, not a link or file' >&2
        exit 2
    fi
done
if [ "$(id -u)" = 0 ]; then
    install -d -o 10001 -g 10001 -m 700 /data/captures /data/runtime
    exec gosu 10001:10001 "$@"
fi
test -w /data/captures || { echo 'Worker capture directory is not writable' >&2; exit 2; }
test -w /data/runtime || { echo 'Worker runtime directory is not writable' >&2; exit 2; }
exec "$@"
