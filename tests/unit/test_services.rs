//! Unit tests for services

#[test]
fn test_odcl_converter_stub() {
    // ODCL converter is a stub that returns errors
    // This is expected behavior as ODCL is legacy format
    use crate::api::services::odcl_converter::ODCLConverter;

    let result = ODCLConverter::convert_to_odcs("test");
    assert!(result.is_err());

    let result = ODCLConverter::convert_to_odcl("test");
    assert!(result.is_err());
}

#[test]
fn test_rate_limiter_creation() {
    use crate::api::middleware::rate_limit::create_rate_limiter;

    let limiter = create_rate_limiter();
    assert!(!limiter.is_none());
}
