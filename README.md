# Data Modelling API

A REST API for data modeling, schema management, and collaboration built with Rust and Axum.

## Features

- **Workspace & Domain Management**: Organize data models into workspaces and domains
- **Table & Relationship CRUD**: Full CRUD operations for tables and relationships
- **Multi-format Import**: Import from SQL, ODCS, JSON Schema, Avro, Protobuf, DrawIO
- **Multi-format Export**: Export to various formats including ODCS v3.1.0
- **Git Synchronization**: Version control integration via Git repositories
- **Real-time Collaboration**: Shared editing sessions with presence tracking
- **GitHub OAuth**: Secure authentication via GitHub
- **PostgreSQL & File Storage**: Flexible storage backends
- **OpenAPI Documentation**: Auto-generated API documentation
- **Audit Trail**: Complete audit history of all changes

## Quick Start

### Prerequisites

- Rust 1.75 or later
- PostgreSQL 15+ (optional, for database-backed storage)
- Docker & Docker Compose (optional)

### Local Development

1. Clone the repository:
```bash
git clone https://github.com/pixie79/data-modelling-api.git
cd data-modelling-api
```

2. Set environment variables:
```bash
export WORKSPACE_DATA=/tmp/workspace_data
export JWT_SECRET=your-secret-key-change-in-production
export GITHUB_CLIENT_ID=your-github-client-id
export GITHUB_CLIENT_SECRET=your-github-client-secret
export FRONTEND_URL=http://localhost:8080
```

3. (Optional) Set up PostgreSQL:
```bash
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling
```

4. Run migrations (if using PostgreSQL):
```bash
sqlx migrate run
```

5. Run the API:
```bash
cargo run --bin api
```

The API will be available at `http://localhost:8081`

### Docker Deployment

1. Build and run with Docker Compose:
```bash
docker-compose up -d
```

2. The API will be available at `http://localhost:8081`

## API Documentation

### OpenAPI Specification

The OpenAPI specification is available at:
- `http://localhost:8081/api/v1/openapi.json`

### Health Check

The API provides health check endpoints to monitor service availability:

- `GET /health`: Basic health check endpoint
- `GET /api/v1/health`: API versioned health check endpoint

Both endpoints return `200 OK` if the service is running. These endpoints are useful for:
- Load balancer health checks
- Monitoring and alerting systems
- Container orchestration (Kubernetes liveness/readiness probes)

Example:
```bash
curl http://localhost:8081/health
curl http://localhost:8081/api/v1/health
```

### Authentication

1. Initiate GitHub OAuth:
```bash
curl "http://localhost:8081/api/v1/auth/github/login?redirect_uri=http://localhost:8080/callback"
```

2. After OAuth callback, use the returned JWT token:
```bash
curl -H "Authorization: Bearer <token>" http://localhost:8081/api/v1/workspace/info
```

## Configuration

### Environment Variables

#### Required
- `WORKSPACE_DATA`: Path to workspace data directory
- `JWT_SECRET`: Secret key for JWT signing
- `GITHUB_CLIENT_ID`: GitHub OAuth client ID
- `GITHUB_CLIENT_SECRET`: GitHub OAuth client secret

#### Optional
- `DATABASE_URL`: PostgreSQL connection string (default: file-based storage)
- `FRONTEND_URL`: Frontend URL for OAuth redirects (default: http://localhost:8080)
- `REDIRECT_URI_WHITELIST`: Comma-separated allowed redirect URIs
- `ENFORCE_HTTPS_REDIRECT`: Enforce HTTPS for redirects (true/false)
- `OTEL_SERVICE_NAME`: OpenTelemetry service name
- `OTEL_EXPORTER_OTLP_ENDPOINT`: OpenTelemetry endpoint URL

## Storage Backends

### PostgreSQL (Recommended for Production)

Set `DATABASE_URL` environment variable to enable PostgreSQL storage:
```bash
export DATABASE_URL=postgresql://user:password@localhost:5432/dbname
```

Migrations are automatically run on startup.

### File-based (Development/Testing)

If `DATABASE_URL` is not set, the API uses file-based storage in the `WORKSPACE_DATA` directory.

## Development

### SQLx Offline Mode

This project uses sqlx's offline mode to avoid requiring a database connection during compilation. The `.sqlx` directory contains pre-generated query metadata.

**⚠️ Important:** Pre-commit hooks require either:
1. A database connection (set `DATABASE_URL`), OR
2. Generated `.sqlx` metadata files (see below)

**First-time setup (requires database):**
```bash
# Set up database connection
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/data_modelling

# Run migrations
cargo sqlx migrate run

# Generate offline metadata
./scripts/prepare-sqlx.sh
# Or manually: cargo sqlx prepare -- --all-features

# Commit the .sqlx directory to git
git add .sqlx
git commit -m "Add sqlx offline metadata"
```

**Normal development (no database required after metadata is generated):**
```bash
# Build with offline mode (automatic if .sqlx exists)
cargo build

# Or explicitly set it
export SQLX_OFFLINE=true
cargo build
```

**If you don't have database access yet:**
Pre-commit will fail until `.sqlx` metadata is generated. You can:
- Skip pre-commit temporarily: `git commit --no-verify`
- Or set up a local PostgreSQL instance and generate metadata
- Or wait for someone else to commit the `.sqlx` directory

### Running Tests

```bash
# Run all tests sequentially (recommended for integration tests)
cargo test -- --test-threads=1

# Run specific test
cargo test --test test_name
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy --all-features

# Check for security vulnerabilities
cargo audit
```

### Pre-commit Hooks

Install pre-commit hooks:
```bash
pre-commit install
```

## Project Structure

```
├── src/
│   ├── api/              # API implementation
│   │   ├── routes/       # Route handlers
│   │   ├── services/     # Business logic
│   │   ├── storage/      # Storage backends
│   │   └── middleware/   # Middleware
│   ├── export/           # Format exporters
│   └── lib.rs            # Library root
├── migrations/           # Database migrations
├── tests/               # Test suites
└── Cargo.toml           # Dependencies
```

## Dependencies

### Published Crates
- `data-modelling-sdk = "1.0.2"` - Shared types and Git operations

### Key Dependencies
- `axum = "0.7"` - Web framework
- `sqlx = "0.8"` - Database toolkit
- `utoipa = "5.0"` - OpenAPI generation
- `tokio = "1.0"` - Async runtime

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please ensure:
- Code is formatted with `cargo fmt`
- Code passes `cargo clippy`
- Tests pass
- Security audit passes (`cargo audit`)

## Support

For issues and questions, please open an issue on GitHub.
