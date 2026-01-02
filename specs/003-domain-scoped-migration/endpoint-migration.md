# Endpoint Migration Summary: Domain-Scoped Resources

## Overview

This document provides a quick reference for the endpoint migration from workspace-level/implicit-domain to domain-scoped endpoints.

## Endpoint Comparison

### Import Endpoints

| Current Endpoint | New Endpoint | Status |
|-----------------|--------------|--------|
| `POST /import/odcl` | `POST /workspace/domains/{domain}/import/odcl` | ⚠️ To Migrate |
| `POST /import/odcl/text` | `POST /workspace/domains/{domain}/import/odcl/text` | ⚠️ To Migrate |
| `POST /import/sql` | `POST /workspace/domains/{domain}/import/sql` | ⚠️ To Migrate |
| `POST /import/sql/text` | `POST /workspace/domains/{domain}/import/sql/text` | ⚠️ To Migrate |
| `POST /import/avro` | `POST /workspace/domains/{domain}/import/avro` | ⚠️ To Migrate |
| `POST /import/json-schema` | `POST /workspace/domains/{domain}/import/json-schema` | ⚠️ To Migrate |
| `POST /import/protobuf` | `POST /workspace/domains/{domain}/import/protobuf` | ⚠️ To Migrate |

### Export Endpoints

| Current Endpoint | New Endpoint | Status |
|-----------------|--------------|--------|
| `GET /models/export/{format}` | `GET /workspace/domains/{domain}/export/{format}` | ⚠️ To Migrate |
| `GET /models/export/all` | `GET /workspace/domains/{domain}/export/all` | ⚠️ To Migrate |

**Formats**: `json_schema`, `avro`, `protobuf`, `sql`, `odcl`, `png`

### Git Sync Endpoints

| Current Endpoint | New Endpoint | Status |
|-----------------|--------------|--------|
| `GET /git/config?domain={domain}` | `GET /workspace/domains/{domain}/git/config` | ⚠️ To Migrate |
| `POST /git/config?domain={domain}` | `POST /workspace/domains/{domain}/git/config` | ⚠️ To Migrate |
| `POST /git/init?domain={domain}` | `POST /workspace/domains/{domain}/git/init` | ⚠️ To Migrate |
| `POST /git/clone?domain={domain}` | `POST /workspace/domains/{domain}/git/clone` | ⚠️ To Migrate |
| `GET /git/status?domain={domain}` | `GET /workspace/domains/{domain}/git/status` | ⚠️ To Migrate |
| `POST /git/export?domain={domain}` | `POST /workspace/domains/{domain}/git/export` | ⚠️ To Migrate |
| `POST /git/commit?domain={domain}` | `POST /workspace/domains/{domain}/git/commit` | ⚠️ To Migrate |
| `POST /git/push?domain={domain}` | `POST /workspace/domains/{domain}/git/push` | ⚠️ To Migrate |
| `POST /git/pull?domain={domain}` | `POST /workspace/domains/{domain}/git/pull` | ⚠️ To Migrate |
| `GET /git/conflicts?domain={domain}` | `GET /workspace/domains/{domain}/git/conflicts` | ⚠️ To Migrate |
| `POST /git/conflicts/resolve?domain={domain}` | `POST /workspace/domains/{domain}/git/conflicts/resolve` | ⚠️ To Migrate |

### Data-Flow Diagrams (New)

| Endpoint | Description | Status |
|----------|-------------|--------|
| `GET /workspace/domains/{domain}/data-flow-diagrams` | List all data-flow diagrams for a domain | 🆕 To Implement |
| `POST /workspace/domains/{domain}/data-flow-diagrams` | Create a new data-flow diagram | 🆕 To Implement |
| `GET /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}` | Get a specific data-flow diagram | 🆕 To Implement |
| `PUT /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}` | Update a data-flow diagram | 🆕 To Implement |
| `DELETE /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}` | Delete a data-flow diagram | 🆕 To Implement |

## Already Domain-Scoped (No Changes)

### Tables
- ✅ `GET /workspace/domains/{domain}/tables`
- ✅ `POST /workspace/domains/{domain}/tables`
- ✅ `GET /workspace/domains/{domain}/tables/{table_id}`
- ✅ `PUT /workspace/domains/{domain}/tables/{table_id}`
- ✅ `DELETE /workspace/domains/{domain}/tables/{table_id}`

### Relationships
- ✅ `GET /workspace/domains/{domain}/relationships`
- ✅ `POST /workspace/domains/{domain}/relationships`
- ✅ `GET /workspace/domains/{domain}/relationships/{relationship_id}`
- ✅ `PUT /workspace/domains/{domain}/relationships/{relationship_id}`
- ✅ `DELETE /workspace/domains/{domain}/relationships/{relationship_id}`

### Cross-Domain References
- ✅ `GET /workspace/domains/{domain}/cross-domain`
- ✅ `GET /workspace/domains/{domain}/cross-domain/tables`
- ✅ `POST /workspace/domains/{domain}/cross-domain/tables`
- ✅ `PUT /workspace/domains/{domain}/cross-domain/tables/{table_id}`
- ✅ `DELETE /workspace/domains/{domain}/cross-domain/tables/{table_id}`
- ✅ `GET /workspace/domains/{domain}/cross-domain/relationships`
- ✅ `DELETE /workspace/domains/{domain}/cross-domain/relationships/{relationship_id}`
- ✅ `POST /workspace/domains/{domain}/cross-domain/sync`

### Canvas
- ✅ `GET /workspace/domains/{domain}/canvas`

## Workspace-Level Endpoints (Correctly Scoped, No Changes)

### Workspace Management
- ✅ `GET /api/v1/workspaces`
- ✅ `POST /api/v1/workspaces`
- ✅ `GET /workspace/info`
- ✅ `GET /workspace/profiles`

### Domain Management
- ✅ `GET /workspace/domains`
- ✅ `POST /workspace/domains`
- ✅ `GET /workspace/domains/{domain}`
- ✅ `PUT /workspace/domains/{domain}`
- ✅ `DELETE /workspace/domains/{domain}`
- ✅ `POST /workspace/load-domain`

## Migration Impact

### Breaking Changes
- **Import endpoints**: Domain must be specified in path (currently implicit)
- **Export endpoints**: Domain must be specified in path (currently implicit)
- **Git sync endpoints**: Domain moved from query to path parameter

### Client Migration Required
Clients using import/export/git sync endpoints will need to:
1. Update endpoint URLs to include domain path parameter
2. Remove domain query parameter (for git sync)
3. Ensure domain is loaded before calling endpoints (or rely on `ensure_domain_loaded()`)

### Backward Compatibility
- Option 1: Keep legacy endpoints with deprecation warnings (recommended)
- Option 2: Remove legacy endpoints immediately (cleaner but breaking)

## Example Migration

### Before (Import)
```http
POST /import/odcl
Authorization: Bearer {token}
Content-Type: multipart/form-data

[file data]
```

### After (Import)
```http
POST /workspace/domains/conceptual/import/odcl
Authorization: Bearer {token}
Content-Type: multipart/form-data

[file data]
```

### Before (Git Sync)
```http
POST /git/commit?domain=conceptual
Authorization: Bearer {token}
Content-Type: application/json

{
  "message": "Update tables"
}
```

### After (Git Sync)
```http
POST /workspace/domains/conceptual/git/commit
Authorization: Bearer {token}
Content-Type: application/json

{
  "message": "Update tables"
}
```

## Benefits

1. **Consistency**: All domain resources follow the same URL pattern
2. **Clarity**: Domain scope is explicit in the URL
3. **RESTful**: Better alignment with REST principles
4. **Maintainability**: Easier to understand and maintain codebase
5. **Scalability**: Easier to add domain-scoped features in the future

