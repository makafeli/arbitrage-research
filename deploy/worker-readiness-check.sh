#!/bin/sh
# Finite mounted-volume and read-only metadata checks, never the market worker.
set -eu
test "$#" -eq 0 || { echo 'READINESS_ARGUMENTS_REJECTED' >&2; exit 2; }
worker-volume-check
exec python3 -I /usr/local/lib/arb/worker_readiness.py
