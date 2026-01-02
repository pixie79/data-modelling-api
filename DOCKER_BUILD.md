# Docker Build Instructions

## Prerequisites

Before building the Docker image, you must generate SQLX offline metadata for all database queries:

```bash
# 1. Start PostgreSQL database
docker-compose up -d postgres

# 2. Set database URL
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling

# 3. Run migrations
cargo sqlx migrate run

# 4. Generate SQLX offline metadata
cargo sqlx prepare -- --all-features

# 5. Now you can build the Docker image
docker build -t data-modelling-api .
```

## Why This Is Required

The project uses SQLX compile-time query verification. The `.sqlx` directory contains pre-computed query metadata that allows SQLX to verify queries without a live database connection during build.

**Note**: The `.sqlx` directory must be committed to version control so that Docker builds work in CI/CD environments.

## Building Without SQLX Metadata (Not Recommended)

If you need to build without SQLX metadata (e.g., for development), you can modify the Dockerfile to skip SQLX checks, but this is **not recommended** for production builds as it bypasses query validation.

