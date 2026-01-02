# Tasks: Fix Missing Implementations

**Input**: Design documents from `/specs/002-fix-missing-implementations/`
**Prerequisites**: plan.md (required), spec.md (required for user stories)

**Tests**: Tests are included to ensure fixes work correctly and maintain 95% test coverage requirement.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4, US5)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths use existing project structure

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify project structure and prepare for implementation

- [x] T001 Verify existing project structure matches plan.md requirements
- [x] T002 [P] Review existing ModelService patterns for file-based workspace creation
- [x] T003 [P] Review PostgreSQL storage methods for cross-domain references
- [x] T004 [P] Review health check endpoint implementation in src/api/main.rs
- [x] T005 [P] Review contract test structure in tests/integration/test_api_contracts.rs

---

## Phase 2: User Story 1 - File-Based Storage Support for POST /api/v1/workspaces (Priority: P1) 🎯 MVP

**Goal**: Enable POST `/api/v1/workspaces` endpoint to work with file-based storage (when DATABASE_URL is not set)

**Independent Test**: Can be fully tested by making POST requests to `/api/v1/workspaces` endpoint with valid authentication tokens when file-based storage is active. The request should create a workspace using ModelService and return workspace details.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T006 [P] [US1] Integration test for POST /api/v1/workspaces with file-based storage in tests/integration/test_api_endpoints.rs
- [x] T007 [P] [US1] Integration test for POST /api/v1/workspaces validation errors with file-based storage in tests/integration/test_api_endpoints.rs
- [x] T008 [P] [US1] Integration test to verify no regression in PostgreSQL mode in tests/integration/test_api_endpoints.rs

### Implementation for User Story 1

- [x] T009 [US1] Review existing create_workspace_for_email() function in src/api/routes/workspace.rs
- [x] T010 [US1] Implement file-based workspace creation logic in create_workspace_v1() function in src/api/routes/workspace.rs
- [x] T011 [US1] Add ModelService integration for workspace creation in src/api/routes/workspace.rs
- [x] T012 [US1] Ensure workspace name and type validation matches PostgreSQL mode in src/api/routes/workspace.rs
- [x] T013 [US1] Ensure response format matches PostgreSQL mode in src/api/routes/workspace.rs
- [x] T014 [US1] Add error handling for file-based workspace creation failures in src/api/routes/workspace.rs
- [x] T015 [US1] Update OpenAPI documentation to note file-based storage support in src/api/routes/workspace.rs

**Checkpoint**: At this point, POST /api/v1/workspaces should work with both file-based and PostgreSQL storage modes.

---

## Phase 3: User Story 2 - Cross-Domain Relationships PostgreSQL Backend (Priority: P2)

**Goal**: Ensure cross-domain relationship endpoints use PostgreSQL storage when available instead of falling back to file-based storage

**Independent Test**: Can be fully tested by making GET and DELETE requests to `/workspace/domains/{domain}/cross-domain/relationships` endpoints with PostgreSQL storage active. The requests should use PostgreSQL storage backend.

### Tests for User Story 2

- [ ] T016 [P] [US2] Integration test for GET cross-domain relationships with PostgreSQL storage in tests/integration/test_api_endpoints.rs
- [ ] T017 [P] [US2] Integration test for DELETE cross-domain relationship with PostgreSQL storage in tests/integration/test_api_endpoints.rs
- [ ] T018 [P] [US2] Integration test to verify fallback to file-based storage when PostgreSQL unavailable in tests/integration/test_api_endpoints.rs

### Implementation for User Story 2

- [x] T019 [US2] Review existing PostgreSQL storage methods for cross-domain refs in src/api/storage/postgres.rs
- [x] T020 [US2] Verify get_cross_domain_refs() implementation in src/api/storage/postgres.rs
- [x] T021 [US2] Verify remove_cross_domain_ref() implementation in src/api/storage/postgres.rs
- [x] T022 [US2] Update list_cross_domain_relationships() to use PostgreSQL storage when available in src/api/routes/workspace.rs
- [x] T023 [US2] Update delete_cross_domain_relationship() to use PostgreSQL storage when available in src/api/routes/workspace.rs
- [x] T024 [US2] Add error handling for PostgreSQL storage failures with fallback to file-based in src/api/routes/workspace.rs
- [x] T025 [US2] Remove or update "not yet implemented" comments in src/api/routes/workspace.rs

**Checkpoint**: At this point, cross-domain relationship endpoints should use PostgreSQL storage when available.

---

## Phase 4: User Story 3 - Health Check Endpoint Documentation (Priority: P3)

**Goal**: Document health check endpoints in LLM.txt and README.md

**Independent Test**: Can be verified by checking that LLM.txt and README.md include documentation for /health and /api/v1/health endpoints.

### Tests for User Story 3

- [x] T026 [P] [US3] Verify health check endpoints work correctly (manual verification)
- [x] T027 [P] [US3] Verify documentation accuracy matches actual endpoint behavior

### Implementation for User Story 3

- [x] T028 [US3] Add health check endpoint documentation to LLM.txt
- [x] T029 [US3] Add health check endpoint documentation to README.md API section
- [x] T030 [US3] Include request/response examples in documentation
- [x] T031 [US3] Include usage notes and monitoring guidance in documentation

**Checkpoint**: At this point, health check endpoints should be fully documented.

---

## Phase 5: User Story 4 - Integration Test Contracts (Priority: P2)

**Goal**: Implement all contract tests to verify API responses match expected contracts

**Independent Test**: Can be verified by running cargo test and confirming that contract tests pass (no longer marked #[ignore]).

### Tests for User Story 4

- [x] T032 [P] [US4] Implement test_health_check_contract() in tests/integration/test_api_contracts.rs
- [x] T033 [P] [US4] Implement test_get_tables_empty_contract() in tests/integration/test_api_contracts.rs
- [x] T034 [P] [US4] Implement test_get_tables_with_filtering_contract() in tests/integration/test_api_contracts.rs
- [x] T035 [P] [US4] Implement test_import_sql_text_contract() in tests/integration/test_api_contracts.rs
- [x] T036 [P] [US4] Implement test_import_odcl_text_contract() in tests/integration/test_api_contracts.rs
- [x] T037 [P] [US4] Implement test_get_table_by_id_contract() in tests/integration/test_api_contracts.rs
- [x] T038 [P] [US4] Implement test_get_table_not_found_contract() in tests/integration/test_api_contracts.rs
- [x] T039 [P] [US4] Implement test_get_relationships_empty_contract() in tests/integration/test_api_contracts.rs
- [x] T040 [P] [US4] Implement test_create_relationship_contract() in tests/integration/test_api_contracts.rs
- [x] T041 [P] [US4] Implement test_get_table_stats_contract() in tests/integration/test_api_contracts.rs
- [x] T042 [P] [US4] Implement test_filter_tables_contract() in tests/integration/test_api_contracts.rs
- [x] T043 [P] [US4] Implement test_import_sql_with_conflicts_contract() in tests/integration/test_api_contracts.rs
- [x] T044 [P] [US4] Implement test_check_circular_dependency_contract() in tests/integration/test_api_contracts.rs
- [x] T045 [P] [US4] Implement test_delete_table_contract() in tests/integration/test_api_contracts.rs
- [x] T046 [US4] Remove #[ignore] attributes from all implemented contract tests in tests/integration/test_api_contracts.rs
- [ ] T047 [US4] Verify all contract tests pass with cargo test in tests/integration/test_api_contracts.rs (requires database setup)

**Checkpoint**: At this point, all contract tests should be implemented and passing.

---

## Phase 6: User Story 5 - Liquibase Format Parsing (Priority: P3)

**Goal**: Document Liquibase format parsing limitation clearly

**Independent Test**: Can be tested by importing a Liquibase-formatted SQL file and verifying fallback behavior works correctly.

### Tests for User Story 5

- [x] T048 [P] [US5] Test Liquibase format fallback behavior in tests/integration/test_api_endpoints.rs
- [x] T049 [P] [US5] Verify fallback warning message is clear and helpful

### Implementation for User Story 5

- [x] T050 [US5] Review Liquibase parsing code in src/api/services/sql_parser.rs
- [x] T051 [US5] Review Liquibase parsing code in src/api/services/odcs_parser.rs
- [x] T052 [US5] Add clear documentation comment explaining Liquibase limitation in src/api/services/sql_parser.rs
- [x] T053 [US5] Add clear documentation comment explaining Liquibase limitation in src/api/services/odcs_parser.rs
- [x] T054 [US5] Add TODO comment for future Liquibase implementation in src/api/services/sql_parser.rs
- [x] T055 [US5] Add TODO comment for future Liquibase implementation in src/api/services/odcs_parser.rs
- [x] T056 [US5] Ensure fallback error message is clear and helpful in src/api/services/sql_parser.rs
- [x] T057 [US5] Ensure fallback error message is clear and helpful in src/api/services/odcs_parser.rs

**Checkpoint**: At this point, Liquibase limitation should be clearly documented.

---

## Phase 7: Polish (Final Validation)

**Purpose**: Ensure code quality, formatting, linting, and documentation

- [x] T058 [P] Run cargo fmt --all to ensure code formatting compliance
- [x] T059 [P] Run cargo clippy --all-targets --all-features -- -D warnings to ensure linting compliance
- [ ] T060 [P] Run cargo test --all-features to verify all tests pass (requires database setup)
- [ ] T061 [P] Verify test coverage meets 95% threshold using cargo tarpaulin (requires database setup)
- [ ] T062 [P] Run cargo audit to verify security compliance (requires network access)
- [x] T063 [P] Update CHANGELOG.md with new fixes and enhancements
- [x] T064 Verify backward compatibility with existing endpoints
- [x] T065 Review all error messages for clarity and helpfulness

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **User Stories (Phase 2-6)**: Can proceed in parallel after Setup, or sequentially in priority order (P1 → P2 → P3)
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Setup - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Setup - No dependencies on other stories
- **User Story 3 (P3)**: Can start after Setup - No dependencies on other stories
- **User Story 4 (P2)**: Can start after Setup - No dependencies on other stories
- **User Story 5 (P3)**: Can start after Setup - No dependencies on other stories

### Task Dependencies Within Stories

- **User Story 1**: Tests (T006-T008) should be written before implementation (T009-T015)
- **User Story 2**: Tests (T016-T018) should be written before implementation (T019-T025)
- **User Story 4**: All tests (T032-T045) can be implemented in parallel, then remove #[ignore] (T046), then verify (T047)
- **User Story 5**: Documentation tasks (T050-T057) can be done in parallel

---

## Notes

- File-based storage is primarily for development/testing. PostgreSQL is preferred for production.
- Cross-domain relationships already work in file-based mode. PostgreSQL implementation is an enhancement.
- Health check endpoints are already implemented and working. Only documentation is needed.
- Contract tests are important for API contract verification but not blocking for functionality.
- Liquibase format parsing has a working fallback. Full implementation can be deferred.

