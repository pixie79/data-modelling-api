# Feature Specification: Fix Missing API Endpoints and Email Selection Support

**Feature Branch**: `001-fix-missing-endpoints`  
**Created**: 2025-01-27  
**Status**: Draft  
**Input**: User description: "- fix bug report - https://github.com/pixie79/data-modelling-api/issues/3"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Workspace Management (Priority: P1)

Users need to create and list workspaces through standardized API endpoints. Currently, workspace creation and listing functionality exists but uses non-standard endpoint paths that don't match the API contract expected by the frontend.

**Why this priority**: This is a HIGH priority bug that blocks core workspace management functionality. Users cannot create or list workspaces in online mode, which prevents basic application usage.

**Independent Test**: Can be fully tested by making GET and POST requests to `/api/v1/workspaces` endpoint with valid authentication tokens. The GET request should return a list of workspaces for the authenticated user, and POST request should create a new workspace and return workspace details.

**Acceptance Scenarios**:

1. **Given** a user is authenticated with a valid JWT token, **When** they send GET request to `/api/v1/workspaces`, **Then** the system returns a list of workspaces associated with their account
2. **Given** a user is authenticated with a valid JWT token, **When** they send POST request to `/api/v1/workspaces` with workspace name and type, **Then** the system creates a new workspace and returns workspace details
3. **Given** an unauthenticated user, **When** they attempt to access `/api/v1/workspaces`, **Then** the system returns 401 Unauthorized
4. **Given** a user sends invalid workspace data (missing name or type), **When** they POST to `/api/v1/workspaces`, **Then** the system returns 400 Bad Request with error details

---

### User Story 2 - User Information Display (Priority: P2)

Users need to retrieve their own profile information after authentication. The frontend expects a standardized endpoint to fetch current user details for display in the UI.

**Why this priority**: This is a MEDIUM priority bug that affects user experience. While the frontend has a workaround (creating a default user object), users should see their actual information from GitHub after login.

**Independent Test**: Can be fully tested by making a GET request to `/api/v1/auth/me` with a valid JWT token. The response should include user ID, name, and email address from the authenticated session.

**Acceptance Scenarios**:

1. **Given** a user is authenticated with a valid JWT token, **When** they send GET request to `/api/v1/auth/me`, **Then** the system returns their user information including ID, name, and email
2. **Given** an unauthenticated user, **When** they attempt to access `/api/v1/auth/me`, **Then** the system returns 401 Unauthorized
3. **Given** a user's session has expired, **When** they attempt to access `/api/v1/auth/me`, **Then** the system returns 401 Unauthorized

---

### User Story 3 - Email Selection During OAuth (Priority: P2)

Users with multiple GitHub email addresses need to select which email to use for their session when `select_email=true` is specified in the OAuth callback. Currently, the system auto-selects an email without user input.

**Why this priority**: This is a MEDIUM priority bug that affects users with multiple GitHub email aliases. Users should have control over which email address is associated with their session, especially when they have multiple verified emails.

**Independent Test**: Can be fully tested by initiating OAuth flow with `select_email=true` parameter, verifying that the exchange endpoint returns available emails, and then allowing email selection before completing authentication.

**Acceptance Scenarios**:

1. **Given** OAuth callback includes `select_email=true` parameter, **When** the user exchanges the auth code, **Then** the system returns available emails and requires email selection before completing authentication
2. **Given** a user has multiple verified GitHub emails, **When** they exchange auth code with `select_email=true`, **Then** the system returns all verified emails in the response
3. **Given** a user attempts to exchange auth code without selecting email when `select_email=true`, **When** they POST to `/api/v1/auth/exchange` without email parameter, **Then** the system returns 400 Bad Request indicating email selection is required
4. **Given** a user selects an email that is not in their verified GitHub emails, **When** they attempt to complete authentication, **Then** the system returns 400 Bad Request with validation error
5. **Given** OAuth callback does NOT include `select_email=true`, **When** the user exchanges the auth code, **Then** the system auto-selects the primary email and completes authentication normally

---

### Edge Cases

- What happens when a user has no workspaces? (GET `/api/v1/workspaces` should return empty array)
- What happens when workspace creation fails due to storage errors? (System should return 500 Internal Server Error)
- What happens when a user attempts to create a workspace with duplicate name for the same email? (System should return 409 Conflict - workspace names must be unique per user email address. However, the same user can have duplicate workspace names when using different email aliases, and a workspace can switch between "personal" and "organization" types over time)
- What happens when JWT token is malformed? (System should return 401 Unauthorized)
- What happens when user's GitHub session is revoked but JWT is still valid? (System should return 401 Unauthorized)
- What happens when OAuth code expires before exchange? (System should return 400 Bad Request)
- What happens when user has no verified GitHub emails? (System should return error or use primary email)
- What happens when `select_email=true` but user only has one email? (System should auto-select that email)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide GET `/api/v1/workspaces` endpoint that returns a list of workspaces for the authenticated user
- **FR-002**: System MUST provide POST `/api/v1/workspaces` endpoint that creates a new workspace with name and type
- **FR-003**: System MUST require JWT authentication for all `/api/v1/workspaces` endpoints
- **FR-004**: System MUST validate workspace creation requests (name and type must be provided)
- **FR-004a**: System MUST enforce workspace name uniqueness per user email address (workspace names must be unique within the scope of the authenticated user's email, but the same user can have duplicate names when using different email aliases)
- **FR-005**: System MUST provide GET `/api/v1/auth/me` endpoint that returns current authenticated user information
- **FR-006**: System MUST require JWT authentication for `/api/v1/auth/me` endpoint
- **FR-007**: System MUST return user ID, name, and email in `/api/v1/auth/me` response
- **FR-008**: System MUST support email selection when `select_email=true` is present in OAuth callback
- **FR-009**: System MUST return available emails in exchange response when `select_email=true`
- **FR-010**: System MUST require email parameter in exchange request when `select_email=true` and email selection is required
- **FR-011**: System MUST validate that selected email is in user's verified GitHub emails
- **FR-012**: System MUST auto-select primary email when `select_email=true` is not present or when user has only one email

### Key Entities *(include if feature involves data)*

- **Workspace**: Represents a user's workspace containing domains and data models. Key attributes: workspace_id, name, type (personal/organization), user_id, created_at
- **User**: Represents an authenticated user from GitHub OAuth. Key attributes: user_id, github_id, github_username, email, selected_email
- **OAuth Session**: Represents an active authentication session. Key attributes: session_id, user_id, github_id, emails (list), selected_email, expires_at

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can successfully list their workspaces via GET `/api/v1/workspaces` endpoint within 2 seconds of request
- **SC-002**: Users can successfully create a new workspace via POST `/api/v1/workspaces` endpoint within 3 seconds of request
- **SC-003**: Users can retrieve their profile information via GET `/api/v1/auth/me` endpoint within 1 second of request
- **SC-004**: Users with multiple GitHub emails can select their preferred email during OAuth flow when `select_email=true` is specified
- **SC-005**: 100% of API contract endpoints match frontend expectations (no 404 errors for expected endpoints)
- **SC-006**: All authentication-protected endpoints properly reject unauthenticated requests with 401 status code
- **SC-007**: Email selection flow completes successfully for users with multiple verified GitHub emails within 5 seconds
