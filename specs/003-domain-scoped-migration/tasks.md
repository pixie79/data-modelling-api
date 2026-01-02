# Tasks: Domain-Scoped Resource Migration

**Input**: Design documents from `/specs/003-domain-scoped-migration/`
**Prerequisites**: plan.md (required), endpoint-migration.md (reference)

**Tests**: Integration tests are included to verify endpoint migrations work correctly.

**Organization**: Tasks are grouped by migration phase (user story) to enable independent implementation and testing of each phase.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which migration phase this task belongs to (US1=Import, US2=Export, US3=Git Sync, US4=Data-Flow Diagrams)
- Include exact file paths in descriptions

## Path Conventions

- **API Routes**: `src/api/routes/`
- **Storage**: `src/api/storage/`
- **Models**: `src/api/models/`
- **Tests**: `tests/integration/`
- **Migrations**: `migrations/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and preparation for migration

- [X] T001 Create feature branch `003-domain-scoped-migration` from main
- [X] T002 [P] Review current import router structure in `src/api/routes/import.rs`
- [X] T003 [P] Review current export router structure in `src/api/routes/models.rs`
- [X] T004 [P] Review current git sync router structure in `src/api/routes/git_sync.rs`
- [X] T005 [P] Review `ensure_domain_loaded()` helper function in `src/api/routes/workspace.rs`
- [X] T006 Document current endpoint usage patterns for backward compatibility planning

**Checkpoint**: Understanding of current structure complete - migration can begin

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY migration phase can be implemented

**⚠️ CRITICAL**: No migration work can begin until this phase is complete

- [X] T007 Create `DomainPath` extractor helper if not exists in `src/api/routes/workspace.rs`
- [X] T008 [P] Verify `ensure_domain_loaded()` function supports both PostgreSQL and file-based storage in `src/api/routes/workspace.rs`
- [X] T009 [P] Create helper function `domain_import_router()` stub in `src/api/routes/import.rs`
- [X] T010 [P] Create helper function `domain_export_router()` stub in `src/api/routes/models.rs`
- [X] T011 [P] Create helper function `domain_git_router()` stub in `src/api/routes/git_sync.rs`
- [X] T012 Update workspace router structure to support nested domain-scoped routes in `src/api/routes/workspace.rs`

**Checkpoint**: Foundation ready - migration phases can now begin in parallel

---

## Phase 3: User Story 1 - Import Endpoints Migration (Priority: P1) 🎯 MVP

**Goal**: Migrate all import endpoints from `/import/*` to `/workspace/domains/{domain}/import/*` with domain path parameter

**Independent Test**: Import a file via new domain-scoped endpoint and verify it's stored in the correct domain folder

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T013 [P] [US1] Integration test for domain-scoped ODCL import in `tests/integration/test_domain_import.rs`
- [ ] T014 [P] [US1] Integration test for domain-scoped SQL import in `tests/integration/test_domain_import.rs`
- [ ] T015 [P] [US1] Integration test for domain-scoped JSON Schema import in `tests/integration/test_domain_import.rs`
- [ ] T016 [P] [US1] Integration test for domain-scoped Avro import in `tests/integration/test_domain_import.rs`
- [ ] T017 [P] [US1] Integration test for domain-scoped Protobuf import in `tests/integration/test_domain_import.rs`

### Implementation for User Story 1

- [X] T018 [US1] Implement `domain_import_router()` function accepting domain path parameter in `src/api/routes/import.rs`
- [X] T019 [US1] Update `import_odcl()` handler to use `ensure_domain_loaded()` instead of current model in `src/api/routes/import.rs`
- [X] T020 [US1] Update `import_odcl_text()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T021 [US1] Update `import_sql()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T022 [US1] Update `import_sql_text()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T023 [US1] Update `import_avro()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T024 [US1] Update `import_json_schema()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T025 [US1] Update `import_protobuf()` handler to use `ensure_domain_loaded()` in `src/api/routes/import.rs`
- [X] T026 [US1] Update OpenAPI path annotations for all import endpoints in `src/api/routes/import.rs`
- [X] T027 [US1] Nest import router under `/workspace/domains/{domain}/import` in `src/api/routes/workspace.rs`
- [ ] T028 [US1] Add backward compatibility layer (optional deprecation) for legacy `/import/*` endpoints in `src/api/routes/mod.rs`

**Checkpoint**: At this point, all import endpoints should work via domain-scoped paths and be independently testable

---

## Phase 4: User Story 2 - Export Endpoints Migration (Priority: P2)

**Goal**: Migrate export endpoints from `/models/export/*` to `/workspace/domains/{domain}/export/*` with domain path parameter

**Independent Test**: Export a domain's model via new domain-scoped endpoint and verify correct format output

### Tests for User Story 2

- [ ] T029 [P] [US2] Integration test for domain-scoped JSON Schema export in `tests/integration/test_domain_export.rs`
- [ ] T030 [P] [US2] Integration test for domain-scoped Avro export in `tests/integration/test_domain_export.rs`
- [ ] T031 [P] [US2] Integration test for domain-scoped Protobuf export in `tests/integration/test_domain_export.rs`
- [ ] T032 [P] [US2] Integration test for domain-scoped SQL export in `tests/integration/test_domain_export.rs`
- [ ] T033 [P] [US2] Integration test for domain-scoped ODCL export in `tests/integration/test_domain_export.rs`
- [ ] T034 [P] [US2] Integration test for domain-scoped PNG export in `tests/integration/test_domain_export.rs`
- [ ] T035 [P] [US2] Integration test for domain-scoped export all (ZIP) in `tests/integration/test_domain_export.rs`

### Implementation for User Story 2

- [ ] T036 [US2] Implement `domain_export_router()` function accepting domain path parameter in `src/api/routes/models.rs`
- [ ] T037 [US2] Update `export_format()` handler to use `ensure_domain_loaded()` instead of current model in `src/api/routes/models.rs`
- [ ] T038 [US2] Update `export_all()` handler to use `ensure_domain_loaded()` in `src/api/routes/models.rs`
- [ ] T039 [US2] Update OpenAPI path annotations for export endpoints in `src/api/routes/models.rs`
- [ ] T040 [US2] Nest export router under `/workspace/domains/{domain}/export` in `src/api/routes/workspace.rs`
- [ ] T041 [US2] Add backward compatibility layer (optional deprecation) for legacy `/models/export/*` endpoints in `src/api/routes/mod.rs`

**Checkpoint**: At this point, all export endpoints should work via domain-scoped paths and be independently testable

---

## Phase 5: User Story 3 - Git Sync Endpoints Migration (Priority: P3)

**Goal**: Migrate git sync endpoints from `/git/*?domain={domain}` to `/workspace/domains/{domain}/git/*` with domain path parameter

**Independent Test**: Perform git operations (init, commit, push) via new domain-scoped endpoints and verify they operate on correct domain folder

### Tests for User Story 3

- [ ] T042 [P] [US3] Integration test for domain-scoped git config in `tests/integration/test_domain_git_sync.rs`
- [ ] T043 [P] [US3] Integration test for domain-scoped git init in `tests/integration/test_domain_git_sync.rs`
- [ ] T044 [P] [US3] Integration test for domain-scoped git commit in `tests/integration/test_domain_git_sync.rs`
- [ ] T045 [P] [US3] Integration test for domain-scoped git push in `tests/integration/test_domain_git_sync.rs`
- [ ] T046 [P] [US3] Integration test for domain-scoped git pull in `tests/integration/test_domain_git_sync.rs`
- [ ] T047 [P] [US3] Integration test for domain-scoped git status in `tests/integration/test_domain_git_sync.rs`

### Implementation for User Story 3

- [ ] T048 [US3] Implement `domain_git_router()` function accepting domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T049 [US3] Update `get_sync_config()` handler to use domain path parameter instead of query in `src/api/routes/git_sync.rs`
- [ ] T050 [US3] Update `update_sync_config()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T051 [US3] Update `init_repository()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T052 [US3] Update `clone_repository()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T053 [US3] Update `get_sync_status()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T054 [US3] Update `export_domain()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T055 [US3] Update `commit_changes()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T056 [US3] Update `push_changes()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T057 [US3] Update `pull_changes()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T058 [US3] Update `list_conflicts()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T059 [US3] Update `resolve_conflict()` handler to use domain path parameter in `src/api/routes/git_sync.rs`
- [ ] T060 [US3] Remove `DomainPath` query parameter struct and update to path parameter in `src/api/routes/git_sync.rs`
- [ ] T061 [US3] Update OpenAPI path annotations for all git sync endpoints in `src/api/routes/git_sync.rs`
- [ ] T062 [US3] Nest git sync router under `/workspace/domains/{domain}/git` in `src/api/routes/workspace.rs`
- [ ] T063 [US3] Add backward compatibility layer (optional deprecation) for legacy `/git/*?domain={domain}` endpoints in `src/api/routes/mod.rs`

**Checkpoint**: At this point, all git sync endpoints should work via domain-scoped paths and be independently testable

---

## Phase 6: User Story 4 - Data-Flow Diagrams Implementation (Priority: P4)

**Goal**: Implement domain-scoped data-flow diagram CRUD endpoints matching offline storage structure

**Independent Test**: Create, read, update, and delete data-flow diagrams via domain-scoped endpoints and verify storage in both PostgreSQL and file-based modes

### Tests for User Story 4

- [ ] T064 [P] [US4] Integration test for creating data-flow diagram in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T065 [P] [US4] Integration test for listing data-flow diagrams in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T066 [P] [US4] Integration test for getting data-flow diagram by ID in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T067 [P] [US4] Integration test for updating data-flow diagram in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T068 [P] [US4] Integration test for deleting data-flow diagram in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T069 [P] [US4] Integration test for file-based storage (data-flow.yaml) in `tests/integration/test_data_flow_diagrams.rs`
- [ ] T070 [P] [US4] Integration test for PostgreSQL storage in `tests/integration/test_data_flow_diagrams.rs`

### Implementation for User Story 4

- [ ] T071 [US4] Create `DataFlowDiagram` model struct in `src/api/models/data_flow_diagram.rs`
- [ ] T072 [US4] Add `get_data_flow_diagrams()` method to `StorageBackend` trait in `src/api/storage/traits.rs`
- [ ] T073 [US4] Add `create_data_flow_diagram()` method to `StorageBackend` trait in `src/api/storage/traits.rs`
- [ ] T074 [US4] Add `get_data_flow_diagram()` method to `StorageBackend` trait in `src/api/storage/traits.rs`
- [ ] T075 [US4] Add `update_data_flow_diagram()` method to `StorageBackend` trait in `src/api/storage/traits.rs`
- [ ] T076 [US4] Add `delete_data_flow_diagram()` method to `StorageBackend` trait in `src/api/storage/traits.rs`
- [ ] T077 [US4] Create database migration for `data_flow_diagrams` table in `migrations/[timestamp]_create_data_flow_diagrams.sql`
- [ ] T078 [US4] Implement PostgreSQL storage methods for data-flow diagrams in `src/api/storage/postgres.rs`
- [ ] T079 [US4] Implement file-based storage methods (read/write data-flow.yaml) in `src/api/storage/file.rs`
- [ ] T080 [US4] Create data-flow diagram router with CRUD handlers in `src/api/routes/data_flow.rs`
- [ ] T081 [US4] Implement `list_data_flow_diagrams()` handler in `src/api/routes/data_flow.rs`
- [ ] T082 [US4] Implement `create_data_flow_diagram()` handler in `src/api/routes/data_flow.rs`
- [ ] T083 [US4] Implement `get_data_flow_diagram()` handler in `src/api/routes/data_flow.rs`
- [ ] T084 [US4] Implement `update_data_flow_diagram()` handler in `src/api/routes/data_flow.rs`
- [ ] T085 [US4] Implement `delete_data_flow_diagram()` handler in `src/api/routes/data_flow.rs`
- [ ] T086 [US4] Add OpenAPI documentation for all data-flow diagram endpoints in `src/api/routes/data_flow.rs`
- [ ] T087 [US4] Nest data-flow diagram router under `/workspace/domains/{domain}/data-flow-diagrams` in `src/api/routes/workspace.rs`
- [ ] T088 [US4] Register data-flow routes module in `src/api/routes/mod.rs`

**Checkpoint**: At this point, data-flow diagrams should be fully functional via domain-scoped endpoints in both storage modes

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final improvements, documentation, and validation

- [ ] T089 [P] Update OpenAPI specification to reflect all new domain-scoped endpoints in `src/api/openapi.rs`
- [ ] T090 [P] Update API documentation in `LLM.txt` with new endpoint paths
- [ ] T091 [P] Update API documentation in `README.md` with migration notes
- [ ] T092 [P] Create migration guide for API consumers in `docs/api-migration-guide.md`
- [ ] T093 [P] Add deprecation warnings to legacy endpoint responses (if backward compatibility implemented)
- [ ] T094 Run all integration tests and verify all endpoints work correctly
- [ ] T095 Verify backward compatibility endpoints still function (if implemented)
- [ ] T096 Update CHANGELOG.md with migration details
- [ ] T097 Code review and refactoring for consistency
- [ ] T098 Performance testing for domain lookup operations
- [ ] T099 Security review for domain-scoped access control
- [ ] T100 Update SQLX offline metadata if database schema changed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all migration phases
- **User Stories (Phase 3-6)**: All depend on Foundational phase completion
  - Migration phases can proceed in parallel (if staffed)
  - Or sequentially in priority order (US1 → US2 → US3 → US4)
- **Polish (Phase 7)**: Depends on all desired migration phases being complete

### User Story Dependencies

- **User Story 1 (P1) - Import Migration**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2) - Export Migration**: Can start after Foundational (Phase 2) - Independent of other stories
- **User Story 3 (P3) - Git Sync Migration**: Can start after Foundational (Phase 2) - Independent of other stories
- **User Story 4 (P4) - Data-Flow Diagrams**: Can start after Foundational (Phase 2) - Independent of other stories

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Router structure before handler updates
- Handler updates before OpenAPI documentation
- Implementation complete before integration into workspace router
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members
- Storage backend implementations (PostgreSQL vs file-based) can be done in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Integration test for domain-scoped ODCL import"
Task: "Integration test for domain-scoped SQL import"
Task: "Integration test for domain-scoped JSON Schema import"
Task: "Integration test for domain-scoped Avro import"
Task: "Integration test for domain-scoped Protobuf import"

# Launch handler updates together (after router structure):
Task: "Update import_odcl() handler"
Task: "Update import_sql() handler"
Task: "Update import_avro() handler"
```

---

## Parallel Example: User Story 4

```bash
# Launch storage implementations in parallel:
Task: "Implement PostgreSQL storage methods for data-flow diagrams"
Task: "Implement file-based storage methods (read/write data-flow.yaml)"

# Launch handler implementations in parallel:
Task: "Implement list_data_flow_diagrams() handler"
Task: "Implement create_data_flow_diagram() handler"
Task: "Implement get_data_flow_diagram() handler"
Task: "Implement update_data_flow_diagram() handler"
Task: "Implement delete_data_flow_diagram() handler"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all migrations)
3. Complete Phase 3: User Story 1 (Import Migration)
4. **STOP and VALIDATE**: Test import endpoints independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 (Import) → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 (Export) → Test independently → Deploy/Demo
4. Add User Story 3 (Git Sync) → Test independently → Deploy/Demo
5. Add User Story 4 (Data-Flow Diagrams) → Test independently → Deploy/Demo
6. Each phase adds value without breaking previous phases

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (Import Migration)
   - Developer B: User Story 2 (Export Migration)
   - Developer C: User Story 3 (Git Sync Migration)
   - Developer D: User Story 4 (Data-Flow Diagrams)
3. Phases complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific migration phase for traceability
- Each migration phase should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate phase independently
- Avoid: vague tasks, same file conflicts, cross-phase dependencies that break independence
- Backward compatibility is optional but recommended for smooth migration
- All domain-scoped endpoints should use `ensure_domain_loaded()` helper for consistency

---

## Summary

- **Total Tasks**: 100
- **Tasks per Phase**:
  - Phase 1 (Setup): 6 tasks
  - Phase 2 (Foundational): 6 tasks
  - Phase 3 (US1 - Import): 16 tasks
  - Phase 4 (US2 - Export): 12 tasks
  - Phase 5 (US3 - Git Sync): 22 tasks
  - Phase 6 (US4 - Data-Flow Diagrams): 25 tasks
  - Phase 7 (Polish): 12 tasks
- **Parallel Opportunities**: High - most phases can run independently after foundational phase
- **Independent Test Criteria**: Each phase has clear test criteria for independent validation
- **Suggested MVP Scope**: Phases 1-3 (Setup + Foundational + Import Migration)
- **Format Validation**: ✅ All tasks follow checklist format with ID, labels, and file paths

