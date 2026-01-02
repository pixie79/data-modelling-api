# SQLX Metadata Update Required

## Issue

The new workspace queries added in this PR require SQLX offline metadata to be regenerated.

## Affected Queries

The following new queries in `src/api/storage/postgres.rs` need metadata:
1. `get_workspace_by_email` - Line 30 (SELECT with name, type columns)
2. `get_workspaces_by_owner` - Line 457 (SELECT with name, type columns)  
3. `create_workspace_with_details` - Line 503 (INSERT with name, type columns)
4. `workspace_name_exists` - Line 532 (SELECT COUNT with name column)

## Solution

Run the following to regenerate SQLX metadata:

```bash
# Ensure DATABASE_URL is set and migrations are applied
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling
cargo sqlx migrate run

# Generate offline metadata
cargo sqlx prepare -- --all-features

# Commit the updated .sqlx/ directory
git add .sqlx/
git commit -m "chore: Regenerate SQLX metadata for new workspace queries"
```

## CI Impact

CI builds will fail until SQLX metadata is regenerated because `SQLX_OFFLINE=true` is set in `.github/workflows/ci.yml` and `.github/workflows/publish.yml`.

## Temporary Workaround

If you need to test locally without a database, you can temporarily unset `SQLX_OFFLINE`:
```bash
unset SQLX_OFFLINE
cargo check
```

However, CI will still require the metadata files to be committed.

