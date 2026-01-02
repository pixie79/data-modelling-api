# Tasks: Fix Missing API Endpoints and Email Selection Support

**Input**: Design documents from `/specs/001-fix-missing-endpoints/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are included to ensure bug fixes work correctly and maintain 95% test coverage requirement.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `tests/` at repository root
- Paths use existing project structure

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify project structure and prepare for implementation

- [x] T001 Verify existing project structure matches plan.md requirements
- [x] T002 [P] Verify Rust toolchain and dependencies are available
- [x] T003 [P] Review existing route registration patterns in src/api/routes/mod.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure verification - all infrastructure already exists, just verify

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Verify authentication middleware (AuthContext) works correctly
- [x] T005 [P] Verify workspace storage backend methods exist and work
- [x] T006 [P] Verify session store provides user information needed
- [x] T007 Verify JWT service can extract user context from tokens

**Checkpoint**: Foundation verified - user story implementation can now begin

---

## Phase 3: User Story 1 - Workspace Management (Priority: P1) 🎯 MVP

**Goal**: Add GET and POST `/api/v1/workspaces` endpoints to enable workspace listing and creation

**Independent Test**: Can be fully tested by making GET and POST requests to `/api/v1/workspaces` endpoint with valid authentication tokens. The GET request should return a list of workspaces for the authenticated user, and POST request should create a new workspace and return workspace details.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T008 [P] [US1] Integration test for GET /api/v1/workspaces endpoint in tests/integration/test_api_endpoints.rs
- [x] T009 [P] [US1] Integration test for POST /api/v1/workspaces endpoint in tests/integration/test_api_endpoints.rs
- [x] T010 [P] [US1] Integration test for workspace listing with empty result in tests/integration/test_api_endpoints.rs
- [x] T011 [P] [US1] Integration test for workspace creation validation errors in tests/integration/test_api_endpoints.rs
- [x] T011a [P] [US1] Integration test for duplicate workspace name per email (409 Conflict) in tests/integration/test_api_endpoints.rs
- [x] T011b [P] [US1] Integration test for duplicate workspace name with different email alias (should succeed) in tests/integration/test_api_endpoints.rs

### Implementation for User Story 1

- [x] T012 [P] [US1] Add CreateWorkspaceRequest struct in src/api/routes/workspace.rs
- [x] T013 [P] [US1] Add WorkspaceResponse struct in src/api/routes/workspace.rs
- [x] T014 [P] [US1] Add WorkspacesListResponse struct in src/api/routes/workspace.rs
- [x] T015 [US1] Implement GET /api/v1/workspaces handler function in src/api/routes/workspace.rs
- [x] T016 [US1] Implement POST /api/v1/workspaces handler function in src/api/routes/workspace.rs
- [x] T017 [US1] Add workspace validation logic (name and type validation) in src/api/routes/workspace.rs
- [x] T017a [US1] Add uniqueness check for workspace name per user email address (return 409 Conflict if duplicate name exists for same email) in src/api/routes/workspace.rs
- [x] T018 [US1] Add OpenAPI documentation for GET /api/v1/workspaces using utoipa::path macro in src/api/routes/workspace.rs
- [x] T019 [US1] Add OpenAPI documentation for POST /api/v1/workspaces using utoipa::path macro in src/api/routes/workspace.rs
- [x] T020 [US1] Register GET /api/v1/workspaces route in workspace_router() function in src/api/routes/workspace.rs
- [x] T021 [US1] Register POST /api/v1/workspaces route in workspace_router() function in src/api/routes/workspace.rs
- [x] T022 [US1] Add helper function to filter workspaces by owner_id in src/api/routes/workspace.rs
- [x] T023 [US1] Add error handling for workspace creation failures in src/api/routes/workspace.rs

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently. GET and POST /api/v1/workspaces endpoints should work with authentication.

---

## Phase 4: User Story 2 - User Information Display (Priority: P2)

**Goal**: Add GET `/api/v1/auth/me` endpoint to return current authenticated user information

**Independent Test**: Can be fully tested by making a GET request to `/api/v1/auth/me` with a valid JWT token. The response should include user ID, name, and email address from the authenticated session.

### Tests for User Story 2

- [ ] T024 [P] [US2] Integration test for GET /api/v1/auth/me endpoint in tests/integration/test_api_endpoints.rs
- [ ] T025 [P] [US2] Integration test for GET /api/v1/auth/me with invalid token in tests/integration/test_api_endpoints.rs
- [ ] T026 [P] [US2] Integration test for GET /api/v1/auth/me with expired session in tests/integration/test_api_endpoints.rs

### Implementation for User Story 2

- [x] T027 [P] [US2] Add UserInfoResponse struct in src/api/routes/auth.rs
- [x] T028 [US2] Implement GET /api/v1/auth/me handler function in src/api/routes/auth.rs
- [x] T029 [US2] Add logic to extract user info from session store in src/api/routes/auth.rs
- [x] T030 [US2] Add OpenAPI documentation for GET /api/v1/auth/me using utoipa::path macro in src/api/routes/auth.rs
- [x] T031 [US2] Register GET /api/v1/auth/me route in auth_router() function in src/api/routes/auth.rs
- [x] T032 [US2] Add error handling for missing or invalid sessions in src/api/routes/auth.rs

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently. GET /api/v1/auth/me endpoint should return user information.

---

## Phase 5: User Story 3 - Email Selection During OAuth (Priority: P2)

**Goal**: Enhance POST `/api/v1/auth/exchange` endpoint to properly support email selection when `select_email=true` is specified

**Independent Test**: Can be fully tested by initiating OAuth flow with `select_email=true` parameter, verifying that the exchange endpoint returns available emails, and then allowing email selection before completing authentication.

### Tests for User Story 3

- [ ] T033 [P] [US3] Integration test for exchange with select_email=true and multiple emails in tests/integration/test_api_endpoints.rs
- [ ] T034 [P] [US3] Integration test for exchange requiring email selection in tests/integration/test_api_endpoints.rs
- [ ] T035 [P] [US3] Integration test for exchange with invalid email selection in tests/integration/test_api_endpoints.rs
- [ ] T036 [P] [US3] Integration test for exchange auto-selecting email when select_email=false in tests/integration/test_api_endpoints.rs
- [ ] T037 [P] [US3] Integration test for exchange with single email (auto-select) in tests/integration/test_api_endpoints.rs

### Implementation for User Story 3

- [x] T038 [P] [US3] Add email field to ExchangeAuthCodeRequest struct in src/api/routes/auth.rs
- [x] T039 [US3] Update exchange_auth_code handler to check select_email flag in src/api/routes/auth.rs
- [x] T040 [US3] Add validation logic to require email when select_email=true and multiple emails exist in src/api/routes/auth.rs
- [x] T041 [US3] Add validation logic to verify selected email is in verified emails list in src/api/routes/auth.rs
- [x] T042 [US3] Update exchange response to return emails and select_email flag correctly in src/api/routes/auth.rs
- [x] T043 [US3] Add logic to auto-select primary email when select_email=false or single email exists in src/api/routes/auth.rs
- [x] T044 [US3] Update OpenAPI documentation for POST /api/v1/auth/exchange endpoint in src/api/routes/auth.rs
- [x] T045 [US3] Add error handling for email validation failures in src/api/routes/auth.rs

**Checkpoint**: At this point, all user stories should be independently functional. Email selection flow should work correctly when select_email=true.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, documentation, and quality checks

- [ ] T046 [P] Run cargo fmt --all to ensure code formatting compliance
- [ ] T047 [P] Run cargo clippy --all-targets --all-features -- -D warnings to ensure linting compliance
- [ ] T048 [P] Run cargo test --all-features to verify all tests pass
- [ ] T049 [P] Verify test coverage meets 95% threshold using cargo tarpaulin
- [ ] T050 [P] Run cargo audit to verify security compliance
- [ ] T051 [P] Update OpenAPI spec generation to include new endpoints
- [ ] T052 Verify all endpoints match API contract specifications in contracts/api-contracts.md
- [ ] T053 Run quickstart.md validation tests from quickstart.md
- [ ] T054 Verify backward compatibility with existing endpoints
- [ ] T055 [P] Update CHANGELOG.md with new endpoints

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories, independently testable
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - Enhances existing exchange endpoint, independently testable

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Request/Response structs before handler functions
- Handler functions before route registration
- Core implementation before error handling
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Request/Response structs within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members
- All Polish tasks marked [P] can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Integration test for GET /api/v1/workspaces endpoint in tests/integration/test_api_endpoints.rs"
Task: "Integration test for POST /api/v1/workspaces endpoint in tests/integration/test_api_endpoints.rs"
Task: "Integration test for workspace listing with empty result in tests/integration/test_api_endpoints.rs"
Task: "Integration test for workspace creation validation errors in tests/integration/test_api_endpoints.rs"

# Launch all struct definitions for User Story 1 together:
Task: "Add CreateWorkspaceRequest struct in src/api/routes/workspace.rs"
Task: "Add WorkspaceResponse struct in src/api/routes/workspace.rs"
Task: "Add WorkspacesListResponse struct in src/api/routes/workspace.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (Workspace endpoints)
   - Developer B: User Story 2 (Auth me endpoint)
   - Developer C: User Story 3 (Email selection)
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- All code must pass cargo fmt, cargo clippy, and maintain 95% test coverage
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence

