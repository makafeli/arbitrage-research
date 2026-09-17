#!/usr/bin/env bash
# Exercise the shipped profile generator and Rust parser using synthetic inputs only.
set -euo pipefail
image="${1:-arb-worker:test}"
volume="$(docker volume create --label arb.profile-test=true)"
trap 'docker volume rm "$volume" >/dev/null' EXIT
# All code under /app comes from the built image; only the test harness is mounted.
docker run --rm --pull=never --network none --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=8m \
  --mount "type=volume,source=$volume,target=/data" \
  --mount "type=bind,source=$PWD/scripts/test_worker_prepare_base.py,target=/tests/test_worker_prepare_base.py,readonly" \
  "$image" python3 -I -c \
  'import runpy,sys;sys.path.insert(0,"/app/scripts");sys.argv=["test_worker_prepare_base.py","--container"];runpy.run_path("/tests/test_worker_prepare_base.py",run_name="__main__")'
