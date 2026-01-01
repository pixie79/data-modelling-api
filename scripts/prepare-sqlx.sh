#!/bin/bash
# Script to prepare sqlx offline metadata files
# This requires a running PostgreSQL database with migrations applied

set -e

echo "Preparing sqlx offline metadata files..."
echo "This requires DATABASE_URL to be set and migrations to be applied."

if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL environment variable is not set"
    echo "Example: export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling"
    exit 1
fi

# Check if sqlx-cli is installed
if ! cargo sqlx --version &> /dev/null 2>&1; then
    echo "Installing sqlx-cli..."
    cargo install sqlx-cli --features postgres
fi

# Run migrations first
echo "Running migrations..."
cargo sqlx migrate run

# Prepare offline metadata
echo "Generating .sqlx metadata files..."
cargo sqlx prepare -- --all-features

echo "Done! The .sqlx directory contains query metadata for offline compilation."
echo "You can now build without a database connection using: SQLX_OFFLINE=true cargo build"
