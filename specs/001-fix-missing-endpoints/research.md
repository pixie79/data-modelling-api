# Research: Fix Missing API Endpoints and Email Selection Support

**Date**: 2025-01-27  
**Feature**: Fix Missing API Endpoints and Email Selection Support

## Research Tasks

### Task 1: Existing Workspace Endpoint Patterns

**Decision**: Use existing workspace route patterns and storage backend methods

**Rationale**: 
- Current implementation has `/workspace/create` and `/workspace/info` endpoints
- Storage backend already has `get_workspaces()` and `create_workspace()` methods
- Need to add new `/api/v1/workspaces` endpoints that wrap existing functionality
- Should filter workspaces by authenticated user (owner_id) for GET endpoint

**Alternatives considered**:
- Creating entirely new storage methods: Rejected - existing methods are sufficient
- Duplicating workspace logic: Rejected - would violate DRY principle

### Task 2: Authentication Endpoint Patterns

**Decision**: Follow existing auth route patterns for `/api/v1/auth/me` endpoint

**Rationale**:
- Existing `/auth/status` endpoint provides similar functionality
- JWT token extraction and validation patterns already established
- Session store provides user information needed for `/auth/me`
- Should return user ID, name (github_username), and email (selected_email or primary email)

**Alternatives considered**:
- Reusing `/auth/status` endpoint: Rejected - frontend expects `/auth/me` with different response format
- Creating new session lookup: Rejected - existing session store is sufficient

### Task 3: OAuth Email Selection Flow

**Decision**: Enhance existing exchange endpoint to require email selection when `select_email=true`

**Rationale**:
- Current `ExchangeAuthCodeResponse` already includes `emails` and `select_email` fields
- Token exchange store (`TokenExchangeEntry`) already tracks `select_email` flag
- Need to validate email selection before completing authentication
- Should return 400 Bad Request if email selection required but not provided

**Alternatives considered**:
- Creating separate email selection endpoint: Rejected - exchange flow should handle it
- Auto-selecting email always: Rejected - violates requirement for user choice when `select_email=true`

### Task 4: Route Registration Pattern

**Decision**: Add new routes to existing router modules and register in `mod.rs`

**Rationale**:
- Routes are nested under `/api/v1` in `main.rs`
- Workspace routes are in `workspace.rs` module
- Auth routes are in `auth.rs` module
- New endpoints should follow existing patterns for consistency

**Alternatives considered**:
- Creating new route modules: Rejected - would fragment related functionality
- Modifying existing endpoints: Rejected - need to maintain backward compatibility

### Task 5: OpenAPI Documentation

**Decision**: Use utoipa macros for OpenAPI documentation

**Rationale**:
- Existing endpoints use `#[utoipa::path]` macros
- OpenAPI spec is auto-generated from route handlers
- New endpoints should follow same pattern for consistency
- Frontend expects endpoints documented in OpenAPI spec

**Alternatives considered**:
- Manual OpenAPI spec updates: Rejected - would be error-prone and inconsistent
- Skipping OpenAPI docs: Rejected - violates API documentation requirements

## Key Findings

1. **Workspace Storage**: PostgreSQL storage backend has `get_workspaces()` method that returns all workspaces. Need to filter by `owner_id` for authenticated user.

2. **Session Information**: Session store contains user information (github_id, github_username, emails, selected_email) that can be used for `/auth/me` endpoint.

3. **Email Selection**: Exchange endpoint already returns emails and `select_email` flag. Need to add validation to require email parameter when `select_email=true` and multiple emails exist.

4. **Route Structure**: All routes are registered in `create_api_router()` function in `routes/mod.rs`. New routes should be added to existing modules.

5. **Testing**: Integration tests exist in `tests/integration/test_api_endpoints.rs`. Should add tests for new endpoints following existing patterns.

## Implementation Notes

- All endpoints require JWT authentication via `AuthContext` extractor
- Error responses should follow existing patterns (401 for unauthorized, 400 for bad request)
- Response formats should match frontend expectations from API contract
- Backward compatibility must be maintained with existing endpoints

