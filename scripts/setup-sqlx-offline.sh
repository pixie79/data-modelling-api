#!/bin/bash
# Helper script to set up sqlx offline mode
# This checks if .sqlx metadata exists and sets SQLX_OFFLINE accordingly

set -e

if [ -d .sqlx ] && [ "$(ls -A .sqlx 2>/dev/null)" ]; then
    echo "✓ .sqlx metadata found - using offline mode"
    export SQLX_OFFLINE=true
else
    echo "⚠ .sqlx metadata not found - sqlx will try to connect to database"
    echo "  To generate metadata, run: ./scripts/prepare-sqlx.sh"
    unset SQLX_OFFLINE
fi
