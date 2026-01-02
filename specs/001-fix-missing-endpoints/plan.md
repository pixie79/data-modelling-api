# Implementation Plan: Fix Missing API Endpoints and Email Selection Support

**Branch**: `001-fix-missing-endpoints` | **Date**: 2025-01-27 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-fix-missing-endpoints/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Fix three missing API endpoints and email selection support to align with the API contract expected by the frontend. The implementation will add `/api/v1/workspaces` (GET/POST), `/api/v1/auth/me` (GET), and enhance the OAuth exchange flow to properly support email selection when `select_email=true` is specified.

## Technical Context

**Language/Version**: Rust edition 2024, stable toolchain  
**Primary Dependencies**: Axum 0.8 (web framework), sqlx 0.8 (database), utoipa 4.2 (OpenAPI), tokio 1.0 (async runtime)  
**Storage**: PostgreSQL 15+ (via sqlx) with file-based fallback for development  
**Testing**: cargo test with integration tests in `tests/integration/` and unit tests in `tests/unit/`  
**Target Platform**: Linux server (API backend)  
**Project Type**: Single REST API project  
**Performance Goals**: API endpoints respond within 1-3 seconds per success criteria  
**Constraints**: Must maintain backward compatibility with existing endpoints, 95% test coverage required, all code must pass clippy and fmt checks  
**Scale/Scope**: Bug fix affecting 3 endpoints, minimal scope change to existing OAuth flow

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
specs/[###-feature]/
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
│   │   ├── workspace.rs      # Add GET/POST /api/v1/workspaces endpoints
│   │   ├── auth.rs           # Add GET /api/v1/auth/me endpoint, enhance exchange flow
│   │   └── mod.rs            # Register new routes
│   ├── storage/
│   │   ├── postgres.rs       # Workspace storage operations (already exists)
│   │   └── traits.rs         # StorageBackend trait (already exists)
│   └── services/
│       └── jwt_service.rs    # JWT token handling (already exists)

tests/
├── integration/
│   └── test_api_endpoints.rs # Add tests for new endpoints
└── unit/
    └── test_services.rs      # Unit tests for new functionality
```

**Structure Decision**: Single project structure. New endpoints will be added to existing route modules (`workspace.rs` and `auth.rs`). No new modules or major structural changes required.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations. All constitution requirements can be met with standard implementation patterns.

## Phase 0: Research (Complete)

**Status**: ✅ Complete

Research completed on existing patterns and implementation approach. See [research.md](./research.md) for details.

**Key Decisions**:
- Use existing workspace storage methods with user filtering
- Follow existing auth route patterns
- Enhance exchange endpoint rather than creating new endpoints
- Maintain backward compatibility

## Phase 1: Design & Contracts (Complete)

**Status**: ✅ Complete

Design artifacts generated:

- **Data Model**: [data-model.md](./data-model.md) - Entity definitions and relationships
- **API Contracts**: [contracts/api-contracts.md](./contracts/api-contracts.md) - OpenAPI-style endpoint specifications
- **Quickstart**: [quickstart.md](./quickstart.md) - Testing and integration guide
- **Agent Context**: Updated Cursor IDE context with technology stack

**Constitution Check (Post-Design)**:
- ✅ All endpoints follow existing patterns (no new architectural patterns)
- ✅ Test coverage can be maintained at 95% with integration tests
- ✅ No new dependencies required
- ✅ Code formatting and linting requirements can be met
- ✅ Security audit requirements can be met (no new dependencies)

## Next Steps

Ready for `/speckit.tasks` to break down implementation into specific tasks.
