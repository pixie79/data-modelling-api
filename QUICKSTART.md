# Quick Start Guide

## Setting Up SQLx Offline Mode

To fix pre-commit failures, you need to generate sqlx metadata files. Here are the steps:

### Prerequisites

1. **Install sqlx-cli:**
   ```bash
   cargo install sqlx-cli --features postgres
   ```

2. **Start PostgreSQL:**
   ```bash
   # Option A: Using Docker (if Docker is running)
   docker-compose up -d postgres

   # Option B: Using local PostgreSQL
   # Make sure PostgreSQL is running on localhost:5432
   ```

### Generate Metadata

```bash
# Set DATABASE_URL
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling

# Run the preparation script
./scripts/prepare-sqlx.sh

# Verify metadata was created
ls -la .sqlx/*.json | head -5
```

### Commit Metadata

```bash
# Add the metadata files (they should be committed to git)
git add .sqlx/
git commit -m "Add sqlx offline metadata"
```

### After This

Once `.sqlx` metadata files are committed:
- ✅ Pre-commit will work without a database
- ✅ CI will work without a database
- ✅ All developers can build without a database

### Troubleshooting

**If Docker is not running:**
- Start Docker Desktop, or
- Use a local PostgreSQL installation, or
- Skip pre-commit temporarily: `git commit --no-verify`

**If sqlx-cli is not installed:**
```bash
cargo install sqlx-cli --features postgres
```

**If you can't access a database right now:**
```bash
# Skip pre-commit for this commit
git commit --no-verify -m "Your commit message"
```
