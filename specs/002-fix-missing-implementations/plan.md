# Implementation Plan: Fix Missing Implementations

**Branch**: `002-fix-missing-implementations` | **Date**: 2025-01-27 | **Spec**: [spec.md](./spec.md)
**Input**: Missing Implementations Report from `MISSING_IMPLEMENTATIONS.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Fix five categories of missing implementations to improve feature parity, documentation, and test coverage. The implementation will add file-based storage support for POST `/api/v1/workspaces`, complete PostgreSQL backend for cross-domain relationships, document health check endpoints, implement contract tests, and enhance Liquibase format parsing.

## Technical Context

**Language/Version**: Rust edition 2024, stable toolchain  
**Primary Dependencies**: Axum 0.8 (web framework), sqlx 0.8 (database), utoipa 4.2 (OpenAPI), tokio 1.0 (async runtime)  
**Storage**: PostgreSQL 15+ (via sqlx) with file-based fallback for development  
**Testing**: cargo test with integration tests in `tests/integration/` and unit tests in `tests/unit/`  
**Target Platform**: Linux server (API backend)  
**Project Type**: Single REST API project  
**Performance Goals**: API endpoints respond within 1-3 seconds per success criteria  
**Constraints**: Must maintain backward compatibility with existing endpoints, 95% test coverage required, all code must pass clippy and fmt checks  
**Scale/Scope**: Multiple bug fixes and enhancements across 5 user stories, affecting storage backends, documentation, and tests

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Required Compliance Checks** (from `.specify/memory/constitution.md`):

- ✅ **Formatting**: All code MUST pass `cargo fmt --all -- --check`
- ✅ **Linting**: All code MUST pass `cargo clippy --all-targets --all-features -- -D warnings`
- ✅ **Test Coverage**: Minimum 95% test coverage MUST be maintained
- ✅ **Security Audit**: All dependencies MUST pass `cargo audit`
- ✅ **Pre-commit**: All changes MUST pass pre-commit hooks

**Constitution Violations**: Document any violations below with justification (see Complexity Tracking section).

## Project Structure

### Documentation (this feature)

```text
specs/002-fix-missing-implementations/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── api/
│   ├── routes/
│   │   ├── workspace.rs      # Add file-based storage support for POST /api/v1/workspaces
│   │   └── mod.rs            # No changes needed
│   ├── storage/
│   │   ├── postgres.rs       # Enhance cross-domain relationship storage methods
│   │   └── traits.rs         # No changes needed (methods already exist)
│   └── services/
│       ├── sql_parser.rs     # Complete Liquibase format parsing
│       └── odcs_parser.rs    # Complete Liquibase format parsing
│   └── main.rs              # Health check endpoints (already implemented)

tests/
├── integration/
│   └── test_api_contracts.rs # Implement all contract tests (remove #[ignore])

docs/
├── LLM.txt                  # Add health check endpoint documentation
└── README.md                # Add health check endpoint documentation
```

**Structure Decision**: Single project structure. Enhancements will be added to existing modules. No new modules or major structural changes required.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations. All constitution requirements can be met with standard implementation patterns.

## Phase 0: Research (Complete)

**Status**: ✅ Complete

Research completed on existing patterns and implementation approach.

**Key Decisions**:

### User Story 1: File-Based Workspace Creation
- Use existing `ModelService` and `create_workspace_for_email()` pattern from legacy `/workspace/create` endpoint
- Maintain same validation logic as PostgreSQL mode
- Return same response format for consistency

### User Story 2: Cross-Domain Relationships PostgreSQL
- Existing PostgreSQL storage methods (`get_cross_domain_refs`, `add_cross_domain_ref`, etc.) already exist
- Need to verify they're being called correctly in workspace routes
- May need to enhance error handling or add missing query logic

### User Story 3: Health Check Documentation
- Endpoints already implemented in `src/api/main.rs`
- Need to add to `LLM.txt` and `README.md` API documentation sections
- Include request/response examples

### User Story 4: Contract Tests
- Test structure already exists in `tests/integration/test_api_contracts.rs`
- Need to implement actual test logic for each endpoint
- Remove `#[ignore]` attributes once tests are implemented

### User Story 5: Liquibase Format Parsing
- Current implementation falls back to standard SQL parsing
- Need to evaluate: implement full Liquibase support OR document limitation clearly
- Decision: Document limitation for now (LOW priority), can enhance later if needed

## Phase 1: Design & Contracts (Complete)

**Status**: ✅ Complete

Design artifacts generated:

- **Data Model**: See existing data models in `src/api/storage/traits.rs` and `src/api/models/`
- **API Contracts**: See existing contracts in `openapi.json` and endpoint documentation
- **Quickstart**: See `QUICKSTART.md` for general testing guide

**Constitution Check (Post-Design)**:
- ✅ All enhancements follow existing patterns (no new architectural patterns)
- ✅ Test coverage can be maintained at 95% with integration tests
- ✅ No new dependencies required
- ✅ Code formatting and linting requirements can be met
- ✅ Security audit requirements can be met (no new dependencies)

## Implementation Phases

### Phase 1: Setup & Verification
- Verify existing code structure
- Review existing ModelService patterns
- Review PostgreSQL storage methods for cross-domain refs

### Phase 2: User Story 1 - File-Based Workspace Creation
- Implement file-based storage support in `create_workspace_v1()`
- Add tests for file-based workspace creation
- Verify no regression in PostgreSQL mode

### Phase 3: User Story 2 - Cross-Domain Relationships PostgreSQL
- Review and enhance PostgreSQL storage methods
- Update workspace routes to use PostgreSQL storage when available
- Add tests for PostgreSQL cross-domain relationships

### Phase 4: User Story 3 - Health Check Documentation
- Add health check endpoints to `LLM.txt`
- Add health check endpoints to `README.md`
- Verify documentation accuracy

### Phase 5: User Story 4 - Contract Tests
- Implement all contract tests in `test_api_contracts.rs`
- Remove `#[ignore]` attributes
- Verify tests pass and catch contract violations

### Phase 6: User Story 5 - Liquibase Format Parsing
- Document Liquibase limitation clearly
- Add TODO comments for future enhancement
- Verify fallback behavior works correctly

### Phase 7: Polish & Validation
- Run `cargo fmt --all`
- Run `cargo clippy --all-targets --all-features -- -D warnings`
- Run `cargo test` and verify all tests pass
- Verify 95% test coverage maintained
- Run `cargo audit` to verify security compliance

## Next Steps

Ready for `/speckit.tasks` to break down implementation into specific tasks.

