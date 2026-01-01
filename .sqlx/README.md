# SQLx Offline Metadata

This directory contains pre-generated query metadata for sqlx's offline mode.

## Generating Metadata

To generate these files (requires a database connection):

```bash
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling
./scripts/prepare-sqlx.sh
```

Or manually:
```bash
cargo sqlx migrate run
cargo sqlx prepare -- --all-features
```

## Using Offline Mode

Once the `.sqlx` directory exists, you can build without a database:

```bash
export SQLX_OFFLINE=true
cargo build
```

Or set it permanently:
```bash
echo 'export SQLX_OFFLINE=true' >> ~/.bashrc  # or ~/.zshrc
```

## CI/CD

The CI workflows automatically set `SQLX_OFFLINE=true`, so they don't require a database connection.

## Committing Metadata

The `.sqlx` directory should be committed to git so all developers can build without setting up a database.
