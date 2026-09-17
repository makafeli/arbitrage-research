#!/bin/sh
# Optional operator-supplied, secret-free Base profile; preserve normal API arguments.
set -eu
present=0
for name in ARB_BASE_PROFILE_TOML ARB_BASE_PROFILE_REGISTRY ARB_BASE_PROFILE_DIGEST; do
    case "$name" in
        ARB_BASE_PROFILE_TOML) value=${ARB_BASE_PROFILE_TOML-} ;;
        ARB_BASE_PROFILE_REGISTRY) value=${ARB_BASE_PROFILE_REGISTRY-} ;;
        ARB_BASE_PROFILE_DIGEST) value=${ARB_BASE_PROFILE_DIGEST-} ;;
    esac
    if [ -n "$value" ]; then present=$((present + 1)); fi
done
if [ "$present" -eq 0 ]; then
    if [ "${1-}" = --check-profile ]; then
        printf '%s\n' '{"status":"API_PROFILE_NOT_CONFIGURED","execution_authorized":false}'
        exit 0
    fi
    exec control-api "$@"
fi
if [ "$present" -ne 3 ]; then
    echo 'API_BASE_PROFILE_INCOMPLETE' >&2
    exit 2
fi
if ! printf '%s\n' "$ARB_BASE_PROFILE_DIGEST" | grep -Eq '^sha256:[0-9a-f]{64}$'; then
    echo 'API_BASE_PROFILE_ANCHOR_REJECTED' >&2
    exit 2
fi
umask 077
directory=$(mktemp -d /tmp/arb-api-base.XXXXXX)
cleanup() {
    rm -f -- "$directory/configuration.toml" "$directory/registry.json" "$directory/check.json"
    rmdir -- "$directory"
}
trap cleanup EXIT
printf '%s' "$ARB_BASE_PROFILE_TOML" > "$directory/configuration.toml"
printf '%s' "$ARB_BASE_PROFILE_REGISTRY" > "$directory/registry.json"
worker-profile-check --check "$directory/configuration.toml" "$directory/registry.json" > "$directory/check.json"
if ! grep -F "\"configuration_digest\":\"$ARB_BASE_PROFILE_DIGEST\"" "$directory/check.json" >/dev/null; then
    echo 'API_BASE_PROFILE_ANCHOR_MISMATCH' >&2
    exit 2
fi
if [ "${1-}" = --check-profile ]; then
    test "$#" -eq 1 || { echo 'API_PROFILE_ARGUMENTS_REJECTED' >&2; exit 2; }
    cat "$directory/check.json"
    exit 0
fi
ARB_CONFIG_FILES="${ARB_CONFIG_FILES:-/app/config/research.example.toml},$directory/configuration.toml"
export ARB_CONFIG_FILES
unset ARB_BASE_PROFILE_TOML ARB_BASE_PROFILE_REGISTRY
# The private ephemeral profile must remain readable for the API process lifetime.
trap - EXIT
exec control-api "$@"
