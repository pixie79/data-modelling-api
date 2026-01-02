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

### Step 1: Start PostgreSQL Database

You need a PostgreSQL database running. The easiest way is using Docker Compose:

```bash
# Start PostgreSQL using docker-compose
docker-compose up -d postgres

# Wait a few seconds for PostgreSQL to be ready
sleep 5

# Verify PostgreSQL is running
docker-compose ps
```

Alternatively, if you have PostgreSQL installed locally, ensure it's running on port 5432.

### Step 2: Set Database URL and Run Migrations

```bash
# Set DATABASE_URL (matches docker-compose.yml default)
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling

# Run migrations (this includes the new workspace name/type migration)
cargo sqlx migrate run
```

### Step 3: Generate SQLX Offline Metadata

```bash
# Generate offline metadata for all queries
cargo sqlx prepare -- --all-features

# Verify .sqlx/ directory was updated
ls -la .sqlx/ | wc -l  # Should show updated file count
```

### Step 4: Commit the Updated Metadata

```bash
# Add the updated .sqlx/ directory
git add .sqlx/

# Commit the changes
git commit -m "chore: Regenerate SQLX metadata for new workspace queries"

# Push to branch
git push
```

### Alternative: Use the Prepare Script

You can also use the provided script:

```bash
# Ensure PostgreSQL is running (via docker-compose or locally)
docker-compose up -d postgres

# Set DATABASE_URL
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling

# Run the prepare script
./scripts/prepare-sqlx.sh

# Commit and push
git add .sqlx/
git commit -m "chore: Regenerate SQLX metadata for new workspace queries"
git push
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

