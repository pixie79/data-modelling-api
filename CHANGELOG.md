# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/pixie79/data-modelling-api/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/pixie79/data-modelling-api/releases/tag/v1.0.0
