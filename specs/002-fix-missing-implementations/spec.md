# Feature Specification: Fix Missing Implementations

**Feature Branch**: `002-fix-missing-implementations`  
**Created**: 2025-01-27  
**Status**: Draft  
**Input**: Missing Implementations Report from `MISSING_IMPLEMENTATIONS.md`

## User Scenarios & Testing *(mandatory)*

### User Story 1 - File-Based Storage Support for POST /api/v1/workspaces (Priority: P1)

Users need to create workspaces via the standardized `/api/v1/workspaces` endpoint even when using file-based storage (development/testing mode without PostgreSQL).

**Why this priority**: This is a HIGH priority bug that prevents workspace creation in development/testing environments. The endpoint exists but returns `501 NOT_IMPLEMENTED` for file-based storage, blocking basic functionality.

**Independent Test**: Can be fully tested by making POST requests to `/api/v1/workspaces` endpoint with valid authentication tokens when `DATABASE_URL` is not set. The request should create a workspace using file-based storage (ModelService) and return workspace details.

**Acceptance Scenarios**:

1. **Given** a user is authenticated with a valid JWT token and file-based storage is active (no DATABASE_URL), **When** they send POST request to `/api/v1/workspaces` with workspace name and type, **Then** the system creates a new workspace using ModelService and returns workspace details
2. **Given** a user is authenticated with PostgreSQL storage, **When** they send POST request to `/api/v1/workspaces`, **Then** the system continues to work as before (no regression)
3. **Given** a user sends invalid workspace data, **When** they POST to `/api/v1/workspaces` with file-based storage, **Then** the system returns 400 Bad Request with error details (same validation as PostgreSQL mode)

---

### User Story 2 - Cross-Domain Relationships PostgreSQL Backend (Priority: P2)

Users need cross-domain relationship references to work properly in PostgreSQL mode. Currently, these endpoints fall back to file-based storage even when PostgreSQL is available.

**Why this priority**: This is a MEDIUM priority enhancement that improves feature parity between storage backends. Cross-domain relationships work in file-based mode but not fully in PostgreSQL mode.

**Independent Test**: Can be fully tested by making GET and DELETE requests to `/workspace/domains/{domain}/cross-domain/relationships` endpoints with PostgreSQL storage active. The requests should use PostgreSQL storage backend instead of falling back to file-based storage.

**Acceptance Scenarios**:

1. **Given** PostgreSQL storage is active and a domain exists, **When** a user requests GET `/workspace/domains/{domain}/cross-domain/relationships`, **Then** the system retrieves cross-domain references from PostgreSQL database
2. **Given** PostgreSQL storage is active, **When** a user requests DELETE `/workspace/domains/{domain}/cross-domain/relationships/{relationship_id}`, **Then** the system removes the reference from PostgreSQL database
3. **Given** file-based storage is active, **When** a user requests cross-domain relationship endpoints, **Then** the system continues to work as before (no regression)

---

### User Story 3 - Health Check Endpoint Documentation (Priority: P3)

Developers and API consumers need documentation for health check endpoints to understand how to monitor API availability.

**Why this priority**: This is a LOW priority documentation task. The endpoints work correctly but are not documented, making them difficult to discover.

**Independent Test**: Can be verified by checking that `LLM.txt` and `README.md` include documentation for `/health` and `/api/v1/health` endpoints.

**Acceptance Scenarios**:

1. **Given** a developer reads `LLM.txt`, **When** they look for health check endpoints, **Then** they find documentation for `/health` and `/api/v1/health` endpoints
2. **Given** a developer reads `README.md`, **When** they look for API endpoints, **Then** they find health check endpoints documented in the API Endpoints section

---

### User Story 4 - Integration Test Contracts (Priority: P2)

Developers need automated contract tests to verify API responses match expected contracts and catch regressions.

**Why this priority**: This is a MEDIUM priority quality improvement. Contract tests ensure API consistency and help catch breaking changes early.

**Independent Test**: Can be verified by running `cargo test` and confirming that contract tests pass (no longer marked `#[ignore]`).

**Acceptance Scenarios**:

1. **Given** contract tests are implemented, **When** `cargo test` is run, **Then** all contract tests execute and verify API response formats
2. **Given** an API response format changes, **When** contract tests run, **Then** they fail and alert developers to the contract violation

---

### User Story 5 - Liquibase Format Parsing (Priority: P3)

Users need Liquibase-formatted SQL files to parse correctly without falling back to standard SQL parsing.

**Why this priority**: This is a LOW priority enhancement. A fallback exists, but full Liquibase support would improve parsing accuracy.

**Independent Test**: Can be tested by importing a Liquibase-formatted SQL file and verifying it parses correctly without fallback warnings.

**Acceptance Scenarios**:

1. **Given** a Liquibase-formatted SQL file, **When** a user imports it via POST `/import/sql/text`, **Then** the system parses it using Liquibase format parser (no fallback warning)
2. **Given** a standard SQL file, **When** a user imports it, **Then** the system continues to parse it correctly (no regression)

---

## Functional Requirements

### FR-001: File-Based Workspace Creation
- **Requirement**: POST `/api/v1/workspaces` must support file-based storage mode
- **Implementation**: Use ModelService to create workspace files similar to legacy `/workspace/create` endpoint
- **Validation**: Workspace name and type validation must match PostgreSQL mode
- **Response**: Return same response format as PostgreSQL mode

### FR-002: Cross-Domain Relationships PostgreSQL Storage
- **Requirement**: Cross-domain relationship endpoints must use PostgreSQL storage when available
- **Implementation**: Enhance existing PostgreSQL storage methods for cross-domain references
- **Validation**: Ensure data consistency between file-based and PostgreSQL modes
- **Response**: Return same response format regardless of storage backend

### FR-003: Health Check Documentation
- **Requirement**: Health check endpoints must be documented in `LLM.txt` and `README.md`
- **Implementation**: Add endpoint descriptions, request/response examples, and usage notes
- **Validation**: Documentation must be accurate and match actual endpoint behavior

### FR-004: Contract Test Implementation
- **Requirement**: All contract tests must be implemented and no longer marked `#[ignore]`
- **Implementation**: Write tests for each endpoint contract, verify response formats match expected structure
- **Validation**: Tests must pass and verify API contract compliance

### FR-005: Liquibase Format Parser
- **Requirement**: Liquibase format parsing must be fully implemented
- **Implementation**: Complete Liquibase parser or document limitation clearly
- **Validation**: Liquibase files parse correctly or clear error message explains limitation

---

## Key Entities

### Workspace (File-Based)
- **Attributes**: name, type, email, workspace_path
- **Storage**: File system via ModelService
- **Relationships**: Owned by user (email)

### Cross-Domain Reference (PostgreSQL)
- **Attributes**: id, target_domain_id, source_domain_id, table_id, display_alias, position, notes
- **Storage**: PostgreSQL `cross_domain_refs` table
- **Relationships**: References domains and tables

### Health Check Response
- **Attributes**: status, service, version (optional)
- **Storage**: N/A (runtime information)
- **Relationships**: N/A

---

## Success Criteria

### SC-001: File-Based Workspace Creation
- ✅ POST `/api/v1/workspaces` returns 200 OK with workspace details when using file-based storage
- ✅ Workspace files are created correctly in file system
- ✅ No regression in PostgreSQL mode

### SC-002: Cross-Domain Relationships PostgreSQL
- ✅ GET `/workspace/domains/{domain}/cross-domain/relationships` uses PostgreSQL storage when available
- ✅ DELETE `/workspace/domains/{domain}/cross-domain/relationships/{relationship_id}` uses PostgreSQL storage when available
- ✅ No regression in file-based mode

### SC-003: Health Check Documentation
- ✅ `/health` and `/api/v1/health` endpoints documented in `LLM.txt`
- ✅ Health check endpoints documented in `README.md` API section

### SC-004: Contract Tests
- ✅ All 14+ contract tests implemented and passing
- ✅ No tests marked `#[ignore]` unless explicitly required
- ✅ Tests verify API response formats match contracts

### SC-005: Liquibase Format Parser
- ✅ Liquibase format parsing fully implemented OR limitation clearly documented
- ✅ No fallback warnings for valid Liquibase files (if implemented)

---

## Out of Scope

- Complete rewrite of file-based storage backend (intentional design)
- New API endpoints (only fixing existing ones)
- Performance optimizations (unless required for functionality)
- Breaking changes to existing API contracts

---

## Dependencies

- Existing ModelService for file-based workspace operations
- Existing PostgreSQL storage backend for cross-domain references
- Existing health check endpoint implementation
- Existing contract test structure

---

## Assumptions

- File-based storage is primarily for development/testing
- PostgreSQL is preferred for production
- Backward compatibility must be maintained
- Existing API contracts are correct and should be verified

