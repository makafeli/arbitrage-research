#!/bin/sh
# Explicit finite profile preparation. This never starts the research loop.
set -eu
test "$#" -eq 0 || { echo 'BASE_PREPARATION_ARGUMENTS_REJECTED' >&2; exit 2; }
worker-volume-check
exec python3 -I /app/scripts/worker_prepare_base.py --prepare
