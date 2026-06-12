#!/bin/bash
# Purge a TEE secure storage entry (and optionally its SQLite row).
# Used for: TEE orphans (SQLite row already deleted) and test keys.
#
# Usage:
#   bash purge-tee-entry.sh <uuid>               # purge one entry
#   bash purge-tee-entry.sh --list-test-keys      # list test/gap key candidates
#   bash purge-tee-entry.sh --purge-all-test-keys # purge all test keys (prompts for confirm)
#
# Requires: KMS_ADMIN_TOKEN env var   OR   service running with no API key (open mode)
# Endpoint: POST /admin/purge-key  (added in v0.20.0)
set -eo pipefail

HOST="${KMS_HOST:-localhost:3000}"
BASE="http://$HOST"
DB="${KMS_DB_PATH:-/root/AirAccount/kms.db}"
TOKEN="${KMS_ADMIN_TOKEN:-}"
AUTH_HDR=""
[ -n "$TOKEN" ] && AUTH_HDR="-H Authorization: Bearer $TOKEN"

usage() {
    echo "Usage: $0 <uuid>"
    echo "       $0 --list-test-keys"
    echo "       $0 --purge-all-test-keys"
    echo ""
    echo "Env vars:"
    echo "  KMS_HOST        (default: localhost:3000)"
    echo "  KMS_ADMIN_TOKEN (required in production)"
    echo "  KMS_DB_PATH     (default: /root/AirAccount/kms.db)"
    exit 1
}

list_test_keys() {
    echo "=== Test/Gap Key Candidates in SQLite ==="
    sqlite3 "$DB" "
    SELECT key_id, description, status,
           substr(passkey_pubkey,1,20) as pk_prefix,
           datetime(created_at) as created
    FROM wallets
    WHERE passkey_pubkey LIKE '04000000%'
       OR passkey_pubkey LIKE '0x04000000%'
       OR description LIKE '%test%'
       OR description LIKE '%gap%'
       OR description LIKE '%smoke%'
       OR description LIKE '%dev%'
    ORDER BY created_at
    " 2>/dev/null || echo "(sqlite3 not available or DB not found)"
    echo ""
    echo "=== All wallets (for reference) ==="
    sqlite3 "$DB" "
    SELECT key_id, description, status, substr(passkey_pubkey,1,12) as pk_prefix,
           datetime(created_at) as created
    FROM wallets ORDER BY created_at
    " 2>/dev/null
}

purge_by_uuid() {
    local uuid="$1"
    echo "Purging UUID: $uuid"

    # Check if in SQLite
    IN_DB=$(sqlite3 "$DB" "SELECT COUNT(*) FROM wallets WHERE key_id='$uuid'" 2>/dev/null || echo "?")
    echo "  SQLite row: $IN_DB"

    # Call admin purge endpoint
    RESP=$(curl -sf --max-time 10 -X POST "$BASE/admin/purge-key" \
        -H "Content-Type: application/json" \
        ${AUTH_HDR:+-H "$AUTH_HDR"} \
        -d "{\"key_id\":\"$uuid\",\"reason\":\"manual-purge\"}" 2>&1)
    CODE=$?
    echo "  API response: $RESP"

    if [ $CODE -eq 0 ]; then
        echo "  ✓ Purge successful"
    else
        echo "  ✗ API call failed (code $CODE)"
        # Fall back: try direct SQLite delete
        echo "  Attempting direct SQLite delete..."
        sqlite3 "$DB" "DELETE FROM wallets WHERE key_id='$uuid'" 2>/dev/null && \
            echo "  ✓ SQLite row deleted" || echo "  (no row or sqlite3 unavailable)"
    fi
}

case "${1:-}" in
    --list-test-keys)
        list_test_keys
        ;;
    --purge-all-test-keys)
        echo "WARNING: This will purge all test/gap keys from SQLite and TEE."
        read -p "Type 'yes' to continue: " confirm
        [ "$confirm" != "yes" ] && { echo "Aborted."; exit 0; }
        UUIDS=$(sqlite3 "$DB" "
            SELECT key_id FROM wallets
            WHERE passkey_pubkey LIKE '04000000%'
               OR passkey_pubkey LIKE '0x04000000%'
               OR description LIKE '%test%'
               OR description LIKE '%gap%'
               OR description LIKE '%smoke%'
        " 2>/dev/null)
        for uuid in $UUIDS; do
            purge_by_uuid "$uuid"
        done
        echo "Done."
        ;;
    "")
        usage
        ;;
    *)
        # Validate UUID format
        if echo "$1" | grep -qE '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'; then
            purge_by_uuid "$1"
        else
            echo "ERROR: '$1' is not a valid UUID"
            usage
        fi
        ;;
esac
