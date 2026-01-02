# API Contracts: Fix Missing API Endpoints

**Date**: 2025-01-27  
**Feature**: Fix Missing API Endpoints and Email Selection Support

## Endpoint: GET /api/v1/workspaces

List all workspaces for the authenticated user.

### Request

**Method**: `GET`  
**Path**: `/api/v1/workspaces`  
**Headers**:
- `Authorization: Bearer <jwt_token>` (required)

### Response

**Status**: `200 OK`

**Body**:
```json
{
  "workspaces": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "My Workspace",
      "type": "personal",
      "email": "user@example.com",
      "created_at": "2025-01-27T10:00:00Z"
    }
  ]
}
```

**Status**: `401 Unauthorized` (if token missing or invalid)

**Status**: `500 Internal Server Error` (if database error)

---

## Endpoint: POST /api/v1/workspaces

Create a new workspace for the authenticated user.

### Request

**Method**: `POST`  
**Path**: `/api/v1/workspaces`  
**Headers**:
- `Authorization: Bearer <jwt_token>` (required)
- `Content-Type: application/json`

**Body**:
```json
{
  "name": "My Workspace",
  "type": "personal"
}
```

**Validation**:
- `name` (required): String, non-empty, must be unique per user email address (workspace names are scoped to the authenticated user's email - duplicate names allowed with different email aliases)
- `type` (required): String, must be "personal" or "organization"

### Response

**Status**: `200 OK`

**Body**:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My Workspace",
  "type": "personal",
  "email": "user@example.com",
  "created_at": "2025-01-27T10:00:00Z"
}
```

**Status**: `400 Bad Request` (if validation fails - missing name/type or invalid format)

**Status**: `401 Unauthorized` (if token missing or invalid)

**Status**: `409 Conflict` (if workspace name already exists for the authenticated user's email address)

**Status**: `500 Internal Server Error` (if database error)

---

## Endpoint: GET /api/v1/auth/me

Get current authenticated user information.

### Request

**Method**: `GET`  
**Path**: `/api/v1/auth/me`  
**Headers**:
- `Authorization: Bearer <jwt_token>` (required)

### Response

**Status**: `200 OK`

**Body**:
```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "github_username",
    "email": "user@example.com"
  }
}
```

**Status**: `401 Unauthorized` (if token missing, invalid, or expired)

---

## Endpoint: POST /api/v1/auth/exchange (Enhanced)

Exchange OAuth authorization code for JWT tokens. Enhanced to support email selection.

### Request

**Method**: `POST`  
**Path**: `/api/v1/auth/exchange`  
**Headers**:
- `Content-Type: application/json`

**Body** (when `select_email=true` and email selection required):
```json
{
  "code": "authorization_code",
  "email": "selected@example.com"
}
```

**Body** (when `select_email=false` or auto-select):
```json
{
  "code": "authorization_code"
}
```

**Validation**:
- `code` (required): String, non-empty, must be valid exchange code
- `email` (optional): String, required when `select_email=true` and multiple emails exist, must be in verified emails list

### Response

**Status**: `200 OK`

**Body** (when `select_email=true`):
```json
{
  "access_token": "jwt_access_token",
  "refresh_token": "jwt_refresh_token",
  "access_token_expires_at": 1706356800,
  "refresh_token_expires_at": 1706961600,
  "token_type": "Bearer",
  "emails": [
    {
      "email": "user@example.com",
      "verified": true,
      "primary": true,
      "visibility": "public"
    },
    {
      "email": "user2@example.com",
      "verified": true,
      "primary": false,
      "visibility": "private"
    }
  ],
  "select_email": true
}
```

**Body** (when `select_email=false`):
```json
{
  "access_token": "jwt_access_token",
  "refresh_token": "jwt_refresh_token",
  "access_token_expires_at": 1706356800,
  "refresh_token_expires_at": 1706961600,
  "token_type": "Bearer",
  "emails": [],
  "select_email": false
}
```

**Status**: `400 Bad Request` (if code invalid, expired, or email validation fails)

**Status**: `400 Bad Request` (if `select_email=true` and email not provided when multiple emails exist)

---

## Error Response Format

All error responses follow this format:

```json
{
  "error": "Error message description"
}
```

**Status Codes**:
- `400 Bad Request`: Invalid request data or validation failure
- `401 Unauthorized`: Missing or invalid authentication token
- `404 Not Found`: Resource not found (not applicable for these endpoints)
- `500 Internal Server Error`: Server error

