<!--
  Sync Impact Report:
  
  Version change: 1.0.0 (initial creation)
  
  Principles added:
  - I. Code Formatting (cargo fmt)
  - II. Code Linting (cargo clippy)
  - III. Test Coverage (95% minimum)
  - IV. Security Audit (cargo audit)
  - V. Pre-commit Validation
  
  Sections added:
  - Code Quality Standards
  - Development Workflow
  
  Templates requiring updates:
  - ✅ plan-template.md (Constitution Check section exists)
  - ✅ spec-template.md (no changes needed)
  - ✅ tasks-template.md (no changes needed)
  
  Follow-up TODOs: None
-->

# Data Modelling API Constitution

## Core Principles

### I. Code Formatting (NON-NEGOTIABLE)
All Rust code MUST be formatted using `cargo fmt --all`. The CI pipeline enforces formatting checks via `cargo fmt --all -- --check`. Pre-commit hooks MUST run formatting validation before allowing commits. Code that fails formatting checks MUST NOT be merged.

**Rationale**: Consistent formatting improves readability, reduces diff noise, and enables automated tooling. It eliminates style debates and ensures all contributors follow the same standards.

### II. Code Linting (NON-NEGOTIABLE)
All Rust code MUST pass `cargo clippy --all-targets --all-features -- -D warnings` with zero warnings. The CI pipeline enforces clippy checks. Pre-commit hooks MUST run clippy validation before allowing commits. Code that fails clippy checks MUST NOT be merged.

**Rationale**: Clippy catches common Rust mistakes, enforces idiomatic patterns, and improves code quality. Denying warnings ensures all issues are addressed proactively.

### III. Test Coverage (NON-NEGOTIABLE)
The codebase MUST maintain a minimum of 95% test coverage. All new code MUST include corresponding tests. Test coverage MUST be measured using `cargo tarpaulin` or equivalent tooling. CI pipelines MUST enforce coverage thresholds and fail builds that fall below 95%.

**Rationale**: High test coverage ensures reliability, prevents regressions, and enables confident refactoring. It documents expected behavior and catches bugs early in development.

### IV. Security Audit (NON-NEGOTIABLE)
All dependencies MUST pass `cargo audit` checks. Known acceptable advisories MUST be documented in `cargo-audit.toml` with explicit justification. The CI pipeline MUST run security audits on every pull request. New security vulnerabilities MUST be addressed before merge, unless explicitly documented and approved.

**Rationale**: Security vulnerabilities in dependencies pose risks to the entire application. Regular audits ensure known vulnerabilities are identified and addressed promptly.

### V. Pre-commit Validation
All code changes MUST pass pre-commit hooks before being committed. Pre-commit hooks MUST validate formatting, linting, and basic tests. Developers MUST install pre-commit hooks using `pre-commit install`. Commits that bypass pre-commit hooks MUST be caught by CI and rejected.

**Rationale**: Pre-commit hooks catch issues early, reducing CI failures and ensuring consistent code quality. They provide fast feedback loops for developers.

## Code Quality Standards

### Rust Edition and Toolchain
- MUST use Rust edition 2024 (as specified in Cargo.toml)
- MUST use stable Rust toolchain
- MUST use `rustfmt` and `clippy` components from stable toolchain

### Testing Standards
- Unit tests MUST be placed in `tests/unit/` directory
- Integration tests MUST be placed in `tests/integration/` directory
- Tests MUST be run with `cargo test --all-features`
- Tests MUST run sequentially for integration tests: `cargo test -- --test-threads=1`
- Test coverage MUST be measured and reported in CI

### Security Standards
- Security audit configuration MUST be maintained in `cargo-audit.toml`
- All ignored advisories MUST include justification comments
- New dependencies MUST be audited before addition
- Critical security vulnerabilities MUST be addressed within 24 hours

## Development Workflow

### Pre-commit Requirements
1. Run `cargo fmt --all` to format code
2. Run `cargo clippy --all-targets --all-features -- -D warnings` to check linting
3. Run `cargo audit` to check for security vulnerabilities
4. Run `cargo test --all-features` to ensure tests pass
5. Verify test coverage meets 95% threshold

### CI Pipeline Requirements
The CI pipeline MUST enforce:
1. Formatting check: `cargo fmt --all -- --check`
2. Linting check: `cargo clippy --all-targets --all-features -- -D warnings`
3. Security audit: `cargo audit` (via rustsec/audit-check action)
4. Test execution: `cargo test --all-features`
5. Test coverage: Minimum 95% coverage threshold
6. Build verification: `cargo build --release --all-features`

### Pull Request Requirements
All pull requests MUST:
- Pass all CI checks (formatting, linting, audit, tests, coverage)
- Include tests for new functionality
- Maintain or improve test coverage
- Document any new dependencies or security considerations
- Follow the project's commit message conventions

### Code Review Requirements
Code reviewers MUST verify:
- Code passes formatting and linting checks
- Tests are included for new functionality
- Test coverage requirements are met
- Security audit passes
- Code follows Rust best practices and idioms

## Governance

This constitution supersedes all other development practices and guidelines. All code contributions MUST comply with these principles.

### Amendment Procedure
1. Proposed amendments MUST be documented with rationale
2. Amendments require review and approval from project maintainers
3. Version MUST be incremented according to semantic versioning:
   - **MAJOR**: Backward incompatible governance/principle removals or redefinitions
   - **MINOR**: New principle/section added or materially expanded guidance
   - **PATCH**: Clarifications, wording, typo fixes, non-semantic refinements
4. Amendments MUST be reflected in all dependent templates and documentation

### Compliance Review
- All pull requests MUST verify compliance with constitution principles
- CI failures MUST be resolved before merge approval
- Regular audits SHOULD be conducted to ensure ongoing compliance
- Violations MUST be documented and addressed promptly

### Enforcement
- CI pipeline MUST enforce all non-negotiable principles
- Pre-commit hooks MUST enforce formatting and linting
- Code reviews MUST verify compliance
- Non-compliant code MUST NOT be merged

**Version**: 1.0.0 | **Ratified**: 2025-01-27 | **Last Amended**: 2025-01-27
