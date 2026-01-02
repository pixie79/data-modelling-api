# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- **refactor(api)**: Domain-scoped resource migration plan created
  - Comprehensive migration plan for moving all resources to domain-scoped endpoints
  - Import, export, git sync, and data-flow diagram endpoints to be migrated
  - See `specs/003-domain-scoped-migration/plan.md` for full details
  - Migration will ensure consistency with domain-scoped storage model

### Changed

- **refactor(data-flow)**: Data-flow diagrams now stored at domain level
  - Data-flow diagrams are now stored as `data-flow.yaml` in each domain folder
  - Updated folder structure: `workspace-folder/domain-canvas-1/data-flow.yaml`
  - Diagrams are now associated with their domain (domain_id is set)
  - Aligns with domain-scoped relationships and the concept that data-flow diagrams link to conceptual tables
  - Note: API endpoints remain workspace-level (`/api/v1/workspaces/{workspace_id}/data-flow-diagrams`) for now; offline mode uses domain-scoped storage

## [1.1.2] - 2026-01-02

### Fixed

- **fix(auth)**: OAuth exchange code can now be re-used for email selection
  - Fixed API design issue where session code could only be exchanged once
  - Exchange code is now only removed after successful token generation
  - If email selection is needed, code can be re-exchanged with email parameter
  - Added support for `/auth/select-email` to accept exchange code as alternative to Bearer token
  - This allows email selection when initial exchange returns empty tokens with `select_email=true`
  - Fixes issue where multi-email users couldn't complete authentication flow

- **fix(migration)**: Fixed data-flow diagrams migration foreign key reference
  - Removed non-existent `users` table foreign key constraint
  - Updated to use `user_id` UUID directly (consistent with other tables like `audit_entries`)

- **fix(docker)**: Fixed Docker build for Rust 2024 edition
  - Updated Dockerfile to use `rustlang/rust:nightly-slim` base image
  - Added proper handling of SQLX offline metadata in Docker build
  - Created `DOCKER_BUILD.md` documentation for build prerequisites

## [1.1.1] - 2026-01-02

### Fixed

- **fix(auth)**: Database session support for `/api/v1/auth/select-email` endpoint
  - Endpoint now correctly checks PostgreSQL-backed sessions before falling back to in-memory
  - Added `update_selected_email` method to `DbSessionStore` for database session updates
  - Resolves 401 errors when using PostgreSQL storage backend

- **fix(auth)**: Database session support for authentication helpers
  - `ensure_workspace_loaded_with_session_id` now checks database sessions
  - `get_session_email` now validates database sessions first
  - `AuthContext` extractor now supports database-backed sessions
  - All authentication endpoints now work correctly with PostgreSQL storage

- **fix(openapi)**: Consistent OpenAPI path specifications
  - Fixed `/api/v1/workspaces` and `/api/v1/auth/me` paths to use relative paths
  - All endpoints now correctly report `/api/v1/` prefix in OpenAPI spec
  - Paths are now consistent with router mounting at `/api/v1`

- **fix(domain)**: Updated URLs and emails to use opendatamodelling.com domain
  - API support email updated to mark@opendatamodelling.com
  - Production server URL updated to api.opendatamodelling.com
  - Author email in Cargo.toml updated to mark@opendatamodelling.com

## [1.1.0] - 2025-01-27

### Added

- **feat(workspaces)**: File-based storage support for POST /api/v1/workspaces
  - Workspace creation now works in file-based mode (when DATABASE_URL is not set)
  - Workspace name tracking via `.workspaces.json` files per user
  - GET /api/v1/workspaces now reads from file-based storage
  - Maintains backward compatibility with PostgreSQL mode

- **feat(tests)**: Comprehensive contract test suite
  - Implemented 15+ contract tests for API endpoint verification
  - Tests verify response formats match expected contracts
  - Health check, tables, relationships, import, and filter endpoints covered

- **feat(docs)**: Enhanced API documentation
  - Added health check endpoint documentation to LLM.txt and README.md
  - Documented Liquibase format parsing limitations with clear TODO notes
  - Improved error messages for better user guidance

### Fixed

- **fix(workspaces)**: Fixed POST /api/v1/workspaces returning 501 NOT_IMPLEMENTED in file-based mode
  - Now properly creates workspaces using ModelService
  - Validates workspace name uniqueness per email address
  - Returns consistent response format regardless of storage backend

- **fix(docs)**: Fixed missing health check endpoint documentation
  - Added `/health` and `/api/v1/health` endpoints to LLM.txt
  - Enhanced README.md with health check usage examples and monitoring guidance

- **fix(comments)**: Clarified cross-domain relationships implementation status
  - Updated comments to explain relationship refs vs table refs distinction
  - Documented intentional file-based storage for relationship references

### Changed

- **refactor(workspaces)**: Improved file-based workspace management
  - Uses JSON metadata files for workspace tracking
  - Better error handling and validation
  - Consistent behavior between PostgreSQL and file-based modes

## [1.0.1] - 2025-01-01

### Fixed

- **fix(sqlx)**: Regenerated SQLX offline metadata to include missing queries
  - Added missing query metadata for `collaboration_participants` INSERT query
  - Added missing query metadata for `sessions` SELECT query
  - Fixes GitHub Actions publish workflow failures when `SQLX_OFFLINE=true`

## [1.0.0] - 2024-12-31

### Added

#### Core Features

- **feat(api)**: Comprehensive REST API built with Axum framework
  - Workspace and domain management endpoints
  - Table and relationship CRUD operations
  - Multi-format import/export (SQL, ODCS, JSON Schema, Avro, Protobuf, DrawIO)
  - GitHub OAuth authentication with JWT tokens
  - PostgreSQL and file-based storage backends
  - Real-time collaboration features with WebSocket support
  - Git synchronization support for version control
  - OpenAPI specification generation with utoipa
  - Comprehensive audit trail for all changes

#### Authentication & Authorization

- **feat(auth)**: GitHub OAuth integration
  - Web and desktop authentication flows
  - JWT token generation and refresh
  - Session management with PostgreSQL or in-memory storage
  - Support for `redirect_uri` query parameter for multi-instance frontend support
  - Secure redirect URI validation with whitelisting

#### Storage Backends

- **feat(storage)**: Multiple storage backends
  - PostgreSQL backend with sqlx migrations
  - File-based storage for development/testing
  - Session store with automatic cleanup
  - Audit trail storage

#### Import/Export

- **feat(import)**: Multi-format import support
  - SQL DDL parsing (PostgreSQL, MySQL, SQL Server)
  - ODCS v3.1.0 YAML format
  - JSON Schema with nested object support
  - Avro schema import
  - Protobuf schema import
  - DrawIO XML import

- **feat(export)**: Multi-format export support
  - SQL DDL generation
  - ODCS v3.1.0 YAML format
  - JSON Schema export
  - Avro schema export
  - DrawIO XML export with visual layout

#### Collaboration

- **feat(collaboration)**: Real-time collaboration features
  - Shared sessions with permission management
  - Participant presence tracking
  - Access request workflow
  - WebSocket updates for real-time changes

#### Git Integration

- **feat(git)**: Git synchronization
  - Repository cloning and initialization
  - Domain export to Git repositories
  - Commit and push operations
  - Pull and conflict detection
  - Conflict resolution support

#### Documentation

- **feat(docs)**: Comprehensive documentation
  - OpenAPI specification with full endpoint annotations
  - Postman collection covering all 78+ endpoints
  - README with quick start guide
  - Environment variables documentation
  - Database schema documentation
  - Git sync documentation
  - Collaboration features documentation

#### Testing

- **feat(tests)**: Comprehensive test suite
  - Integration tests for all route handlers
  - Unit tests for core services
  - Test isolation with unique temporary directories
  - Sequential test execution support

#### CI/CD

- **feat(ci)**: GitHub Actions workflows
  - CI workflow for linting, formatting, and testing
  - Publish workflow for crates.io releases
  - Pre-commit hooks for code quality
  - Cargo audit for security vulnerabilities

#### Docker

- **feat(docker)**: Docker deployment
  - Multi-stage Dockerfile for production builds
  - Docker Compose setup with PostgreSQL
  - Health check endpoints
  - Environment variable configuration

### Changed

- **refactor(api)**: Refactored Git operations to use SDK
  - Removed direct `git2` dependency from API
  - All Git operations now go through `data-modelling-sdk`
  - Improved error handling and test coverage

- **refactor(storage)**: Improved database connection handling
  - Added connection pool validation with `test_before_acquire`
  - Enhanced logging for database connection status
  - Better error messages for connection failures

- **refactor(tests)**: Improved test isolation
  - Tests use unique temporary directories
  - Unique domain names and emails per test
  - Sequential test execution support

### Fixed

- **fix(auth)**: Fixed OAuth redirect_uri parameter support
  - Added `redirect_uri` query parameter to `/api/v1/auth/github/login`
  - Stored redirect_uri in OAuth state for callback redirection
  - Added security validation for redirect URIs
  - Support for multiple frontend instances

- **fix(openapi)**: Fixed OpenAPI specification generation
  - Added all endpoint annotations with utoipa
  - Fixed component initialization to prevent panics
  - Added proper security scheme definitions

- **fix(tests)**: Fixed test failures and isolation issues
  - Fixed table route test assertions
  - Improved domain ownership verification
  - Fixed test data serialization

- **fix(clippy)**: Fixed all clippy warnings
  - Fixed collapsible_if warnings
  - Fixed type_complexity warnings
  - Fixed ptr_arg warnings
  - Fixed manual_strip warnings
  - Fixed wildcard_in_or_patterns warnings
  - Fixed should_implement_trait warnings
  - Fixed unnecessary_unwrap warnings

- **fix(fmt)**: Fixed all formatting issues
  - Consistent import ordering
  - Proper multi-line formatting
  - Fixed test code formatting

### Security

- **security(auth)**: Enhanced redirect URI validation
  - Scheme validation (http/https only)
  - Localhost and whitelist support
  - Optional HTTPS enforcement
  - Protection against open redirect vulnerabilities

[Unreleased]: https://github.com/pixie79/data-modelling-api/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/pixie79/data-modelling-api/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/pixie79/data-modelling-api/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/pixie79/data-modelling-api/releases/tag/v1.0.0
