#!/bin/sh
# PostToolUse (Edit|Write): format Rust files the way CI checks them (cargo fmt --all -- --check).
# Edition matches [workspace.package] in Cargo.toml.
f=$(jq -r '.tool_input.file_path // empty')
case "$f" in
  *.rs) command -v rustfmt >/dev/null && rustfmt --edition 2024 "$f" 2>&1 | head -5 ;;
esac
exit 0
