#!/bin/sh
# Requires worker-entrypoint; neither action issues a research START command.
set -eu
exec python3 -I /usr/local/lib/arb/worker_launch_base.py "$@"
