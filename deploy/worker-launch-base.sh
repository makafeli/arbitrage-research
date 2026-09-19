#!/bin/sh
# Requires worker-entrypoint; neither action issues a research START command.
set -eu
# The pre-deploy CI gate is not honoured by Railway on a non-zero exit (#58), so
# every Railway container repeats it here and stops before the launcher runs.
if [ -n "${RAILWAY_ENVIRONMENT_ID:-}" ]; then
    python3 -I /usr/local/lib/arb/worker_ci_gate.py
fi
exec python3 -I /usr/local/lib/arb/worker_launch_base.py "$@"
