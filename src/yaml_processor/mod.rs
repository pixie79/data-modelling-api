//! YAML processing utilities.
//!
//! Provides YAML validation and processing functions.
//! Uses SDK functionality where possible.

/// Validate ODCL YAML content
/// Basic validation - checks for valid YAML structure
pub fn validate_odcl(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Parse YAML to validate structure
    serde_yaml::from_str::<serde_yaml::Value>(content)?;
    Ok(())
}
