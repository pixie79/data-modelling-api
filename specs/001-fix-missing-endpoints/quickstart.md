# Quickstart: Fix Missing API Endpoints

**Date**: 2025-01-27  
**Feature**: Fix Missing API Endpoints and Email Selection Support

## Overview

This feature adds three missing API endpoints and enhances email selection support:

1. **GET /api/v1/workspaces** - List workspaces for authenticated user
2. **POST /api/v1/workspaces** - Create a new workspace
3. **GET /api/v1/auth/me** - Get current user information
4. **Enhanced POST /api/v1/auth/exchange** - Support email selection when `select_email=true`

## Prerequisites

- Rust 1.75+ with stable toolchain
- PostgreSQL 15+ (optional, for database mode)
- Valid JWT token from OAuth authentication

## Testing the Endpoints

### 1. List Workspaces

```bash
curl -X GET http://localhost:8081/api/v1/workspaces \
  -H "Authorization: Bearer <your_jwt_token>"
```

**Expected Response**:
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

### 2. Create Workspace

```bash
curl -X POST http://localhost:8081/api/v1/workspaces \
  -H "Authorization: Bearer <your_jwt_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My New Workspace",
    "type": "personal"
  }'
```

**Expected Response**:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "My New Workspace",
  "type": "personal",
  "email": "user@example.com",
  "created_at": "2025-01-27T10:00:00Z"
}
```

### 3. Get Current User Info

```bash
curl -X GET http://localhost:8081/api/v1/auth/me \
  -H "Authorization: Bearer <your_jwt_token>"
```

**Expected Response**:
```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "github_username",
    "email": "user@example.com"
  }
}
```

### 4. OAuth Exchange with Email Selection

**Step 1**: Initiate OAuth with `select_email=true`:
```bash
curl "http://localhost:8081/api/v1/auth/github/login?redirect_uri=http://localhost:8080/callback&select_email=true"
```

**Step 2**: After OAuth callback, exchange code (first attempt - returns emails):
```bash
curl -X POST http://localhost:8081/api/v1/auth/exchange \
  -H "Content-Type: application/json" \
  -d '{
    "code": "<authorization_code>"
  }'
```

**Expected Response** (if multiple emails):
```json
{
  "access_token": null,
  "refresh_token": null,
  "emails": [
    {
      "email": "user@example.com",
      "verified": true,
      "primary": true
    },
    {
      "email": "user2@example.com",
      "verified": true,
      "primary": false
    }
  ],
  "select_email": true
}
```

**Step 3**: Exchange with selected email:
```bash
curl -X POST http://localhost:8081/api/v1/auth/exchange \
  -H "Content-Type: application/json" \
  -d '{
    "code": "<authorization_code>",
    "email": "user2@example.com"
  }'
```

**Expected Response**:
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

## Error Scenarios

### Unauthorized Access

```bash
curl -X GET http://localhost:8081/api/v1/workspaces
```

**Response**: `401 Unauthorized`

### Invalid Workspace Data

```bash
curl -X POST http://localhost:8081/api/v1/workspaces \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": ""
  }'
```

**Response**: `400 Bad Request`

### Missing Email Selection

```bash
curl -X POST http://localhost:8081/api/v1/auth/exchange \
  -H "Content-Type: application/json" \
  -d '{
    "code": "<code>"
  }'
```

**Response** (when `select_email=true` and multiple emails): `400 Bad Request` with message indicating email selection required

## Integration with Frontend

The frontend should:

1. Use `GET /api/v1/workspaces` to list workspaces instead of falling back to empty list
2. Use `POST /api/v1/workspaces` to create workspaces
3. Use `GET /api/v1/auth/me` to display user information after login
4. Handle email selection flow when `select_email=true` in OAuth exchange response

## Verification Checklist

- [ ] GET /api/v1/workspaces returns list of workspaces for authenticated user
- [ ] POST /api/v1/workspaces creates new workspace successfully
- [ ] GET /api/v1/auth/me returns user information
- [ ] OAuth exchange with `select_email=true` returns emails list
- [ ] OAuth exchange requires email parameter when `select_email=true` and multiple emails exist
- [ ] All endpoints return 401 for unauthenticated requests
- [ ] All endpoints validate input data correctly
- [ ] Error responses follow consistent format

