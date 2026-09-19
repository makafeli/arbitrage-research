#!/bin/sh
# Finite mounted-volume and read-only metadata checks, never the market worker.
set -eu
test "$#" -eq 0 || { echo 'READINESS_ARGUMENTS_REJECTED' >&2; exit 2; }
# The pre-deploy CI gate is not honoured by Railway on a non-zero exit (#58), so
# every Railway container repeats it here and stops before any inspection.
if [ -n "${RAILWAY_ENVIRONMENT_ID:-}" ]; then
    python3 -I /usr/local/lib/arb/worker_ci_gate.py
fi
worker-volume-check
exec python3 -I /usr/local/lib/arb/worker_readiness.py
