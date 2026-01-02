# Migration Plan: Domain-Scoped Resources

## Overview

This plan outlines the migration of all API resources to be domain-scoped, ensuring consistency with the data model where subfolders represent domain canvases and all resources (tables, relationships, data-flow diagrams) are stored per domain.

## Current State Analysis

### ✅ Already Domain-Scoped
- **Tables**: `/workspace/domains/{domain}/tables`
- **Relationships**: `/workspace/domains/{domain}/relationships`
- **Cross-domain References**: `/workspace/domains/{domain}/cross-domain`
- **Canvas**: `/workspace/domains/{domain}/canvas`

### ⚠️ Needs Migration
- **Import Endpoints**: `/import/*` (currently uses implicit domain via loaded model)
- **Export Endpoints**: `/models/export/*` (currently uses implicit domain via loaded model)
- **Git Sync Endpoints**: `/git/*` (uses domain query parameter, not path parameter)
- **Data-Flow Diagrams**: Not yet implemented in API (offline mode stores at domain level)

### ✅ Correctly Workspace-Level (No Change Needed)
- **Workspace Management**: `/api/v1/workspaces` (GET, POST)
- **Domain Management**: `/workspace/domains` (GET, POST)
- **Workspace Info**: `/workspace/info`

## Migration Strategy

### Phase 1: Import Endpoints Migration
**Goal**: Move import endpoints to domain-scoped paths

**Current Endpoints**:
- `POST /import/odcl`
- `POST /import/odcl/text`
- `POST /import/sql`
- `POST /import/sql/text`
- `POST /import/avro`
- `POST /import/json-schema`
- `POST /import/protobuf`

**New Endpoints**:
- `POST /workspace/domains/{domain}/import/odcl`
- `POST /workspace/domains/{domain}/import/odcl/text`
- `POST /workspace/domains/{domain}/import/sql`
- `POST /workspace/domains/{domain}/import/sql/text`
- `POST /workspace/domains/{domain}/import/avro`
- `POST /workspace/domains/{domain}/import/json-schema`
- `POST /workspace/domains/{domain}/import/protobuf`

**Changes Required**:
1. Update `import_router()` to accept domain path parameter
2. Update all import handlers to use `ensure_domain_loaded()` instead of relying on current model
3. Update OpenAPI documentation
4. Add backward compatibility redirects (optional, for deprecation period)

### Phase 2: Export Endpoints Migration
**Goal**: Move export endpoints to domain-scoped paths

**Current Endpoints**:
- `GET /models/export/{format}`
- `GET /models/export/all`

**New Endpoints**:
- `GET /workspace/domains/{domain}/export/{format}`
- `GET /workspace/domains/{domain}/export/all`

**Changes Required**:
1. Update `models_router()` to accept domain path parameter
2. Update export handlers to use `ensure_domain_loaded()` instead of current model
3. Update OpenAPI documentation
4. Add backward compatibility redirects (optional, for deprecation period)

### Phase 3: Git Sync Endpoints Migration
**Goal**: Move git sync endpoints to domain-scoped paths

**Current Endpoints** (using query parameter):
- `GET /git/config?domain={domain}`
- `POST /git/config?domain={domain}`
- `POST /git/init?domain={domain}`
- `POST /git/clone?domain={domain}`
- `GET /git/status?domain={domain}`
- `POST /git/export?domain={domain}`
- `POST /git/commit?domain={domain}`
- `POST /git/push?domain={domain}`
- `POST /git/pull?domain={domain}`
- `GET /git/conflicts?domain={domain}`
- `POST /git/conflicts/resolve?domain={domain}`

**New Endpoints** (using path parameter):
- `GET /workspace/domains/{domain}/git/config`
- `POST /workspace/domains/{domain}/git/config`
- `POST /workspace/domains/{domain}/git/init`
- `POST /workspace/domains/{domain}/git/clone`
- `GET /workspace/domains/{domain}/git/status`
- `POST /workspace/domains/{domain}/git/export`
- `POST /workspace/domains/{domain}/git/commit`
- `POST /workspace/domains/{domain}/git/push`
- `POST /workspace/domains/{domain}/git/pull`
- `GET /workspace/domains/{domain}/git/conflicts`
- `POST /workspace/domains/{domain}/git/conflicts/resolve`

**Changes Required**:
1. Update `git_sync_router()` to be nested under `/workspace/domains/{domain}/git`
2. Update all handlers to use domain path parameter instead of query parameter
3. Update OpenAPI documentation
4. Remove domain query parameter support (or keep as fallback during deprecation)

### Phase 4: Data-Flow Diagrams Implementation
**Goal**: Implement domain-scoped data-flow diagram endpoints

**New Endpoints**:
- `GET /workspace/domains/{domain}/data-flow-diagrams`
- `POST /workspace/domains/{domain}/data-flow-diagrams`
- `GET /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}`
- `PUT /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}`
- `DELETE /workspace/domains/{domain}/data-flow-diagrams/{diagram_id}`

**Changes Required**:
1. Create data-flow diagram models (if not exists)
2. Add storage backend methods for data-flow diagrams
3. Implement CRUD handlers
4. Load/save from `data-flow.yaml` in domain folder for file-based mode
5. Store in database with `domain_id` foreign key for PostgreSQL mode
6. Update OpenAPI documentation

## Technical Implementation Details

### Router Structure Changes

**Current Structure**:
```rust
Router::new()
    .nest("/workspace", workspace::workspace_router())
    .nest("/import", import::import_router())
    .nest("/export", drawio::drawio_router())
    .nest("/models", models::models_router())
    .nest("/git", git_sync::git_sync_router())
```

**New Structure**:
```rust
Router::new()
    .nest("/workspace", workspace::workspace_router()) // Contains domain-scoped routes
    // Legacy endpoints kept for backward compatibility (deprecated)
    .nest("/import", import::import_router_legacy()) // Deprecated
    .nest("/models", models::models_router_legacy()) // Deprecated
    .nest("/git", git_sync::git_sync_router_legacy()) // Deprecated
```

**Workspace Router Structure**:
```rust
workspace_router()
    // Domain CRUD
    .route("/domains", ...)
    // Domain-scoped resources
    .nest("/domains/{domain}", Router::new()
        .nest("/tables", ...)
        .nest("/relationships", ...)
        .nest("/cross-domain", ...)
        .nest("/import", import::domain_import_router()) // NEW
        .nest("/export", models::domain_export_router()) // NEW
        .nest("/git", git_sync::domain_git_router()) // NEW
        .nest("/data-flow-diagrams", data_flow::router()) // NEW
        .route("/canvas", ...)
    )
```

### Storage Backend Changes

**New Methods Required** (for data-flow diagrams):
```rust
// In StorageBackend trait
async fn get_data_flow_diagrams(
    &self,
    domain_id: Uuid,
) -> Result<Vec<DataFlowDiagram>, StorageError>;

async fn create_data_flow_diagram(
    &self,
    domain_id: Uuid,
    diagram: DataFlowDiagram,
    user_context: &UserContext,
) -> Result<DataFlowDiagram, StorageError>;

async fn get_data_flow_diagram(
    &self,
    domain_id: Uuid,
    diagram_id: Uuid,
) -> Result<Option<DataFlowDiagram>, StorageError>;

async fn update_data_flow_diagram(
    &self,
    diagram: DataFlowDiagram,
    expected_version: Option<i32>,
    user_context: &UserContext,
) -> Result<DataFlowDiagram, StorageError>;

async fn delete_data_flow_diagram(
    &self,
    domain_id: Uuid,
    diagram_id: Uuid,
    user_context: &UserContext,
) -> Result<(), StorageError>;
```

**Database Migration Required**:
```sql
CREATE TABLE data_flow_diagrams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    diagram_data JSONB NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(domain_id, name)
);

CREATE INDEX idx_data_flow_diagrams_domain_id ON data_flow_diagrams(domain_id);
```

### File-Based Storage Changes

**Data-Flow Diagram Storage**:
- File path: `{WORKSPACE_DATA}/{email}/{domain}/data-flow.yaml`
- Format: YAML array of diagram objects
- Structure matches offline mode implementation

**Import/Export Changes**:
- Import operations write to domain-specific folder
- Export operations read from domain-specific folder
- Git sync operates on domain-specific folder

## Backward Compatibility Strategy

### Option 1: Deprecation Period (Recommended)
1. Keep legacy endpoints active for 2-3 minor versions
2. Add deprecation warnings in responses
3. Redirect legacy endpoints to new domain-scoped endpoints (if domain can be inferred)
4. Document migration path in API docs

### Option 2: Immediate Removal
1. Remove legacy endpoints immediately
2. Update all clients before release
3. Higher risk but cleaner codebase

**Recommendation**: Use Option 1 with 3-month deprecation period.

## Testing Strategy

### Unit Tests
- Test domain path parameter extraction
- Test `ensure_domain_loaded()` integration
- Test storage backend methods

### Integration Tests
- Test all migrated endpoints with domain path parameter
- Test backward compatibility (if implemented)
- Test file-based and PostgreSQL storage modes
- Test data-flow diagram CRUD operations

### Migration Tests
- Test importing existing data to new domain-scoped structure
- Test exporting from domain-scoped endpoints
- Test git sync with domain path parameter

## Rollout Plan

### Step 1: Preparation (Week 1)
- [ ] Create migration plan document
- [ ] Review with team
- [ ] Create feature branch

### Step 2: Phase 1 - Import Migration (Week 2)
- [ ] Update import router structure
- [ ] Migrate import handlers
- [ ] Update tests
- [ ] Update documentation

### Step 3: Phase 2 - Export Migration (Week 2-3)
- [ ] Update export router structure
- [ ] Migrate export handlers
- [ ] Update tests
- [ ] Update documentation

### Step 4: Phase 3 - Git Sync Migration (Week 3)
- [ ] Update git sync router structure
- [ ] Migrate git sync handlers
- [ ] Update tests
- [ ] Update documentation

### Step 5: Phase 4 - Data-Flow Diagrams (Week 4)
- [ ] Create data-flow diagram models
- [ ] Implement storage backend methods
- [ ] Create database migration
- [ ] Implement CRUD handlers
- [ ] Update tests
- [ ] Update documentation

### Step 6: Testing & Documentation (Week 5)
- [ ] Comprehensive integration testing
- [ ] Update API documentation
- [ ] Update client SDKs (if applicable)
- [ ] Create migration guide for API consumers

### Step 7: Release (Week 6)
- [ ] Merge to main branch
- [ ] Deploy to staging
- [ ] Staging validation
- [ ] Deploy to production
- [ ] Monitor for issues

## Success Criteria

- ✅ All resources accessible via domain-scoped endpoints
- ✅ Legacy endpoints deprecated but functional (if backward compatibility implemented)
- ✅ All tests passing
- ✅ Documentation updated
- ✅ No breaking changes for existing domain-scoped endpoints
- ✅ Data-flow diagrams fully functional in both storage modes

## Risks & Mitigation

### Risk 1: Breaking Changes for Existing Clients
**Mitigation**: Implement backward compatibility layer with deprecation warnings

### Risk 2: Performance Impact from Domain Lookup
**Mitigation**: Cache domain lookups, optimize `ensure_domain_loaded()` function

### Risk 3: Data Migration Complexity
**Mitigation**: Test migration scripts thoroughly, provide rollback procedures

### Risk 4: Git Sync Breaking Existing Workflows
**Mitigation**: Maintain query parameter support during deprecation period

## Future Considerations

- Consider workspace-level aggregations (e.g., export all domains)
- Consider cross-domain operations (already partially implemented)
- Consider bulk operations (import/export multiple domains)
- Consider domain templates/cloning

