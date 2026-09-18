#!/usr/bin/env bash
# Explicit, local certificate maintenance. Never restart PostgreSQL or handle wallets.
set +x
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
unset OPENSSL_CONF OPENSSL_MODULES OPENSSL_ENGINES RANDFILE
umask 077
fail() { printf '{"status":"BLOCKED","reason":"%s"}\n' "$1" >&2; exit 2; }
if [[ $# -eq 0 ]] || [[ $# -eq 1 && $1 == --check ]]; then
    printf '%s\n' '{"status":"NOT_STARTED","database_requests":0,"provider_requests":0,"files_changed":false}'
    exit 0
fi
[[ $# -eq 5 ]] || fail ARGUMENTS_REJECTED
action=$1; directory=$2; root_pin=$3; original_pin=$4; host=$5
case "$action" in --prepare|--apply|--rollback) ;; *) fail ARGUMENTS_REJECTED;; esac
[[ "$directory" =~ ^/[A-Za-z0-9_./-]+$ && "$directory" != */ && "$directory" != *'/../'* && "$directory" != */.. && "$directory" != *'/./'* ]] || fail DIRECTORY_REJECTED
[[ "$root_pin" =~ ^[0-9a-f]{64}$ && "$original_pin" =~ ^[0-9a-f]{64}$ ]] || fail FINGERPRINT_REQUIRED
[[ ${#host} -le 253 && "$host" =~ ^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$ && "$host" != *..* ]] || fail HOST_REJECTED
uid=$(id -u)
# Directory ancestry is trusted; reject links at every traversed path component.
path=$directory
while [[ "$path" != / ]]; do
    [[ -d "$path" && ! -L "$path" ]] || fail DIRECTORY_REJECTED
    path=$(dirname -- "$path")
done
[[ $(stat -c %u "$directory") == "$uid" ]] || fail CERTIFICATE_OWNER_REQUIRED
permissions=$(stat -c %a "$directory")
(( (8#$permissions & 0022) == 0 )) || fail DIRECTORY_WRITABLE_BY_OTHERS
regular() {
    [[ -f "$1" && ! -L "$1" && $(stat -c %h "$1") == 1 && $(stat -c %s "$1") -le 65536 ]] || fail CERTIFICATE_FILE_REJECTED
}
for name in root.crt server.crt server.key; do regular "$directory/$name"; done
[[ $(stat -c %u "$directory/server.crt") == "$uid" ]] || fail CERTIFICATE_OWNER_REQUIRED
# A separate maintenance lock never replaces Railway's runtime/upgrade locks.
lock=$directory/.arb-leaf-repair.lock
if [[ -e "$lock" || -L "$lock" ]]; then
    regular "$lock"
    [[ $(stat -c %u "$lock") == "$uid" && $(stat -c %a "$lock") == 600 ]] || fail LOCK_REJECTED
fi
exec 3>>"$lock"
flock -n 3 || fail MAINTENANCE_BUSY
fingerprint() { openssl x509 -in "$1" -noout -fingerprint -sha256 2>/dev/null | cut -d= -f2 | tr -d ':' | tr A-F a-f; }
[[ $(fingerprint "$directory/root.crt") == "$root_pin" ]] || fail ROOT_ANCHOR_MISMATCH
active=$(fingerprint "$directory/server.crt")
# Compare public material derived in-process. Private key bytes never leave OpenSSL.
key_public=$(openssl pkey -in "$directory/server.key" -passin pass: -pubout 2>/dev/null) || fail SERVER_KEY_REJECTED
cert_public=$(openssl x509 -in "$directory/server.crt" -pubkey -noout 2>/dev/null) || fail CERTIFICATE_FILE_REJECTED
[[ "$key_public" == "$cert_public" ]] || fail SERVER_KEY_MISMATCH
stage=$directory/.arb-leaf-repair
scratch=''
cleanup() { if [[ -n "$scratch" ]]; then rm -rf -- "$scratch"; fi; }
trap cleanup EXIT
trap 'exit 2' INT TERM HUP
validate_leaf() {
    regular "$1"
    openssl x509 -in "$1" -noout -ext basicConstraints 2>/dev/null | grep -Fq 'CA:FALSE' || fail NOT_SERVER_LEAF
    openssl verify -no-CApath -no-CAstore -CAfile "$directory/root.crt" \
        -purpose sslserver -verify_hostname "$host" "$1" >/dev/null 2>&1 || fail LEAF_VERIFICATION_FAILED
    [[ $(openssl x509 -in "$1" -pubkey -noout 2>/dev/null) == "$key_public" ]] || fail SERVER_KEY_MISMATCH
    openssl x509 -in "$1" -checkend 2592000 -noout >/dev/null 2>&1 || fail LEAF_RENEWAL_REQUIRED
}
if [[ "$action" == --prepare ]]; then
    [[ "$active" == "$original_pin" ]] || fail ORIGINAL_ANCHOR_MISMATCH
    [[ ! -e "$stage" && ! -L "$stage" ]] || fail EXISTING_MAINTENANCE_RECORD
    regular "$directory/root.key"
    # Keep the existing trust anchor and a 90-day leaf inside its validity window.
    openssl x509 -in "$directory/root.crt" -checkend 7776000 -noout >/dev/null 2>&1 || fail ROOT_EXPIRY_REQUIRES_REVIEW
    scratch=$(mktemp -d "$directory/.arb-leaf-staging.XXXXXX")
    cp -- "$directory/server.crt" "$scratch/server.original.crt"
    printf '%s\n' "$root_pin" > "$scratch/root.sha256"
    printf '%s\n' "$original_pin" > "$scratch/original.sha256"
    printf '%s\n' "$host" > "$scratch/hostname"
    openssl req -new -key "$directory/server.key" -passin pass: -subj "/CN=$host" \
        -out "$scratch/server.csr" >/dev/null 2>&1 || fail CSR_FAILED
    cat > "$scratch/leaf.ext" <<CERT
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:localhost,DNS:$host
CERT
    serial=$(openssl rand -hex 16) || fail SERIAL_FAILED
    # Do not mutate root.srl or rotate either private key.
    openssl x509 -req -in "$scratch/server.csr" -CA "$directory/root.crt" \
        -CAkey "$directory/root.key" -passin pass: -set_serial "0x$serial" -days 90 \
        -extfile "$scratch/leaf.ext" -out "$scratch/server.next.crt" >/dev/null 2>&1 || fail LEAF_ISSUANCE_FAILED
    validate_leaf "$scratch/server.next.crt"
    [[ $(fingerprint "$scratch/server.original.crt") == "$original_pin" ]] || fail BACKUP_MISMATCH
    cmp -s "$directory/server.crt" "$scratch/server.original.crt" || fail BACKUP_MISMATCH
    fingerprint "$scratch/server.next.crt" > "$scratch/next.sha256"
    chmod 600 "$scratch"/*
    sync "$scratch"/* "$scratch"
    mv -- "$scratch" "$stage"; scratch=''
    sync "$directory"
    printf '%s\n' '{"status":"LEAF_PREPARED","active_certificate_changed":false,"reload_issued":false,"private_keys_changed":false}'
    exit 0
fi
[[ -d "$stage" && ! -L "$stage" && $(stat -c %u "$stage") == "$uid" && $(stat -c %a "$stage") == 700 ]] || fail MAINTENANCE_RECORD_REQUIRED
for name in root.sha256 original.sha256 hostname next.sha256 server.original.crt server.next.crt; do regular "$stage/$name"; done
[[ $(cat "$stage/root.sha256") == "$root_pin" && $(cat "$stage/original.sha256") == "$original_pin" && $(cat "$stage/hostname") == "$host" ]] || fail MAINTENANCE_ANCHOR_MISMATCH
next_pin=$(cat "$stage/next.sha256")
[[ "$next_pin" =~ ^[0-9a-f]{64}$ && $(fingerprint "$stage/server.next.crt") == "$next_pin" && $(fingerprint "$stage/server.original.crt") == "$original_pin" ]] || fail MAINTENANCE_CERTIFICATE_MISMATCH
if [[ "$action" == --apply ]]; then
    validate_leaf "$stage/server.next.crt"
    [[ "$active" == "$original_pin" || "$active" == "$next_pin" ]] || fail ACTIVE_CERTIFICATE_CHANGED
    target=$stage/server.next.crt; status=LEAF_REPLACED_RELOAD_REQUIRED
else
    # Rollback is explicitly requested and restores the known original weakness.
    # Keep the strict worker stopped; this is recovery, not successful hardening.
    [[ "$active" == "$next_pin" || "$active" == "$original_pin" ]] || fail ACTIVE_CERTIFICATE_CHANGED
    target=$stage/server.original.crt; status=ORIGINAL_RESTORED_RELOAD_REQUIRED
fi
[[ $(openssl x509 -in "$target" -pubkey -noout 2>/dev/null) == "$key_public" ]] || fail SERVER_KEY_MISMATCH
scratch=$(mktemp -d "$directory/.arb-leaf-replacement.XXXXXX")
cp -- "$target" "$scratch/server.crt"
chmod --reference="$directory/server.crt" "$scratch/server.crt"
chgrp --reference="$directory/server.crt" "$scratch/server.crt"
sync "$scratch/server.crt"
mv -f -- "$scratch/server.crt" "$directory/server.crt"
sync "$directory"
printf '{"status":"%s","reload_issued":false,"private_keys_changed":false}\n' "$status"
