# Data Model: Fix Missing API Endpoints and Email Selection Support

**Date**: 2025-01-27  
**Feature**: Fix Missing API Endpoints and Email Selection Support

## Entities

### Workspace

Represents a user's workspace containing domains and data models.

**Attributes**:
- `id` (UUID): Unique workspace identifier
- `owner_id` (UUID): User ID of workspace owner (from authenticated session)
- `email` (String): Email address associated with workspace
- `name` (String, optional): Workspace name (for new POST endpoint)
- `type` (String, optional): Workspace type - "personal" or "organization" (for new POST endpoint)
- `created_at` (DateTime): Timestamp when workspace was created
- `updated_at` (DateTime): Timestamp when workspace was last updated

**Relationships**:
- Belongs to User (via `owner_id`)
- Has many Domains (via workspace_id foreign key)

**Validation Rules**:
- `email` must be valid email format
- `name` must be provided when creating workspace (POST)
- `name` must be unique per `email` address (workspace names are scoped to the user's email - same user can have duplicate names with different email aliases)
- `type` must be "personal" or "organization" when provided
- `type` can change over time (workspace can switch between personal and organization modes)
- `owner_id` must match authenticated user's user_id

**Storage**:
- Stored in `workspaces` table (PostgreSQL)
- File-based mode uses synthetic workspace info derived from email

### User

Represents an authenticated user from GitHub OAuth.

**Attributes**:
- `user_id` (UUID): Unique user identifier
- `github_id` (u64): GitHub user ID
- `github_username` (String): GitHub username
- `email` (String): Primary or selected email address
- `selected_email` (String, optional): Email selected by user during OAuth flow
- `emails` (Vec<GitHubEmail>): List of all verified GitHub emails

**Relationships**:
- Has many Workspaces (via owner_id)
- Has one active Session

**Validation Rules**:
- `email` must be in user's verified GitHub emails list
- `selected_email` must be verified if provided

**Storage**:
- User information stored in session (in-memory or PostgreSQL `sessions` table)
- Not stored as separate entity - derived from OAuth session

### OAuth Session

Represents an active authentication session.

**Attributes**:
- `session_id` (UUID): Unique session identifier
- `user_id` (UUID): User ID associated with session
- `github_id` (u64): GitHub user ID
- `github_username` (String): GitHub username
- `github_access_token` (String): GitHub OAuth access token
- `emails` (Vec<GitHubEmail>): List of verified GitHub emails
- `selected_email` (String, optional): Email selected by user
- `created_at` (DateTime): Session creation timestamp
- `last_activity` (DateTime): Last activity timestamp
- `revoked_at` (DateTime, optional): Session revocation timestamp
- `expires_at` (DateTime): Session expiration timestamp

**Relationships**:
- Belongs to User (via user_id)

**State Transitions**:
- Created → Active (on successful OAuth exchange)
- Active → Revoked (on logout or token revocation)
- Active → Expired (after expires_at timestamp)

**Storage**:
- In-memory HashMap for file-based mode
- PostgreSQL `sessions` table for database mode

### GitHubEmail

Represents a GitHub email address.

**Attributes**:
- `email` (String): Email address
- `verified` (bool): Whether email is verified
- `primary` (bool): Whether email is primary
- `visibility` (String, optional): Email visibility setting

**Usage**:
- Used in OAuth session to track available emails
- Used in exchange response when `select_email=true`

## API Request/Response Models

### CreateWorkspaceRequest

**Fields**:
- `name` (String, required): Workspace name
- `type` (String, required): Workspace type - "personal" or "organization"

### WorkspaceResponse

**Fields**:
- `id` (UUID): Workspace identifier
- `name` (String): Workspace name
- `type` (String): Workspace type
- `email` (String): Associated email
- `created_at` (DateTime): Creation timestamp

### WorkspacesListResponse

**Fields**:
- `workspaces` (Vec<WorkspaceResponse>): List of workspaces

### UserInfoResponse

**Fields**:
- `id` (String): User identifier (as string)
- `name` (String): User name (github_username)
- `email` (String): User email (selected_email or primary email)

### ExchangeAuthCodeRequest (Enhanced)

**Fields**:
- `code` (String, required): OAuth authorization code
- `email` (String, optional): Selected email (required when `select_email=true`)

### ExchangeAuthCodeResponse

**Fields**:
- `access_token` (String): JWT access token
- `refresh_token` (String): JWT refresh token
- `access_token_expires_at` (i64): Access token expiration timestamp
- `refresh_token_expires_at` (i64): Refresh token expiration timestamp
- `token_type` (String): Token type ("Bearer")
- `emails` (Vec<GitHubEmail>): Available emails (when `select_email=true`)
- `select_email` (bool): Whether email selection is required

## Data Flow

### Workspace Creation Flow

1. User sends POST `/api/v1/workspaces` with name and type
2. System extracts user context from JWT token
3. System creates workspace with owner_id = user_context.user_id
4. System returns workspace details

### Workspace Listing Flow

1. User sends GET `/api/v1/workspaces`
2. System extracts user context from JWT token
3. System queries workspaces filtered by owner_id
4. System returns list of workspaces

### User Info Flow

1. User sends GET `/api/v1/auth/me`
2. System extracts JWT token and validates
3. System looks up session to get user information
4. System returns user ID, name, and email

### Email Selection Flow

1. OAuth callback includes `select_email=true`
2. Exchange endpoint returns emails and `select_email=true` flag
3. Frontend displays email selection UI
4. User selects email and sends exchange request with email parameter
5. System validates email is in verified emails list
6. System completes authentication with selected email

