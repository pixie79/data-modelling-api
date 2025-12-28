//! ODCL to ODCS converter service.
//!
//! This module handles conversion from legacy ODCL (Data Contract Specification) format
//! to ODCS v3.1.0 format. ODCL is End of Life, but we support importing ODCL files
//! and converting them to ODCS v3.1.0 for internal storage and processing.
//!
//! All ODCL imports are automatically converted to ODCS v3.1.0 format.

use anyhow::Result;
use serde_json::Value as JsonValue;

/// Converter for transforming ODCL (Data Contract Specification) format to ODCS v3.1.0 format.
pub struct ODCLConverter;

impl ODCLConverter {
    /// Convert ODCL (Data Contract Specification) format to ODCS v3.1.0 format.
    ///
    /// This function takes ODCL YAML data and converts it to ODCS v3.1.0 format.
    /// The conversion preserves all data while transforming the structure to match
    /// ODCS v3.1.0 schema requirements.
    ///
    /// # Arguments
    ///
    /// * `odcl_data` - JSON Value representing parsed ODCL YAML data
    ///
    /// # Returns
    ///
    /// Returns a JSON Value in ODCS v3.1.0 format, or an error if conversion fails.
    ///
    /// # Conversion Mapping
    ///
    /// - `dataContractSpecification` → stored in `customProperties` (for reference)
    /// - `id` → `id` (direct mapping)
    /// - `info.title` → `name` (direct mapping)
    /// - `info.version` → `version` (direct mapping)
    /// - `info.status` → `status` (direct mapping)
    /// - `info.description` → `description.purpose` (nested in description object)
    /// - `info.owner` → stored in `customProperties` (no direct ODCS equivalent)
    /// - `info.contact` → stored in `customProperties` (no direct ODCS equivalent)
    /// - `models` → `schema` (convert models to schema array)
    /// - `models[].fields` → `schema[].properties` (convert fields to properties array)
    /// - `definitions` → merge into schema (resolve $ref and merge)
    /// - `servers` → `servers` (convert to ODCS servers array format)
    /// - `tags` → `tags` (direct mapping)
    /// - `servicelevels` → `servicelevels` (direct mapping if present)
    /// - `links` → `links` (direct mapping if present)
    /// - `terms` → stored in `customProperties` (ODCS doesn't have direct equivalent)
    #[allow(dead_code)] // Used in tests
    pub fn convert_to_odcs_v3_1_0(odcl_data: &JsonValue) -> Result<JsonValue> {
        let mut odcs = serde_json::Map::new();

        // Required ODCS v3.1.0 fields
        odcs.insert(
            "apiVersion".to_string(),
            JsonValue::String("v3.1.0".to_string()),
        );
        odcs.insert(
            "kind".to_string(),
            JsonValue::String("DataContract".to_string()),
        );

        // Direct mappings
        if let Some(id) = odcl_data.get("id") {
            odcs.insert("id".to_string(), id.clone());
        }

        // Extract info section
        let info = odcl_data.get("info").and_then(|v| v.as_object());

        // Map info.title to name
        if let Some(title) = info.and_then(|i| i.get("title")) {
            odcs.insert("name".to_string(), title.clone());
        }

        // Map info.version to version
        if let Some(version) = info.and_then(|i| i.get("version")) {
            odcs.insert("version".to_string(), version.clone());
        }

        // Map info.status to status
        if let Some(status) = info.and_then(|i| i.get("status")) {
            odcs.insert("status".to_string(), status.clone());
        }

        // Map info.description to description.purpose
        if let Some(desc) = info.and_then(|i| i.get("description")) {
            let mut description_obj = serde_json::Map::new();
            description_obj.insert("purpose".to_string(), desc.clone());
            odcs.insert(
                "description".to_string(),
                JsonValue::Object(description_obj),
            );
        }

        // Build customProperties for ODCL-specific fields
        let mut custom_props = serde_json::Map::new();

        // Store dataContractSpecification in customProperties
        if let Some(spec) = odcl_data.get("dataContractSpecification") {
            custom_props.insert("dataContractSpecification".to_string(), spec.clone());
        }

        // Store info.owner in customProperties
        if let Some(owner) = info.and_then(|i| i.get("owner")) {
            custom_props.insert("odcl_owner".to_string(), owner.clone());
        }

        // Store info.contact in customProperties
        if let Some(contact) = info.and_then(|i| i.get("contact")) {
            custom_props.insert("odcl_contact".to_string(), contact.clone());
        }

        // Store terms in customProperties
        if let Some(terms) = odcl_data.get("terms") {
            custom_props.insert("odcl_terms".to_string(), terms.clone());
        }

        // Add customProperties if not empty
        if !custom_props.is_empty() {
            odcs.insert(
                "customProperties".to_string(),
                JsonValue::Object(custom_props),
            );
        }

        // Convert models to schema array
        if let Some(models) = odcl_data.get("models").and_then(|v| v.as_object()) {
            let mut schema_array = Vec::new();

            for (model_name, model_data) in models {
                let mut schema_obj = serde_json::Map::new();
                schema_obj.insert("name".to_string(), JsonValue::String(model_name.clone()));

                // Convert model description
                if let Some(desc) = model_data.get("description") {
                    schema_obj.insert("description".to_string(), desc.clone());
                }

                // Convert fields to properties
                if let Some(fields) = model_data.get("fields").and_then(|v| v.as_object()) {
                    let mut properties = serde_json::Map::new();

                    for (field_name, field_data) in fields {
                        let property =
                            Self::convert_field_to_property(field_name, field_data, odcl_data)?;
                        properties.insert(field_name.clone(), property);
                    }

                    schema_obj.insert("properties".to_string(), JsonValue::Object(properties));
                }

                schema_array.push(JsonValue::Object(schema_obj));
            }

            odcs.insert("schema".to_string(), JsonValue::Array(schema_array));
        }

        // Direct mappings for optional fields
        if let Some(tags) = odcl_data.get("tags") {
            odcs.insert("tags".to_string(), tags.clone());
        }

        if let Some(servicelevels) = odcl_data.get("servicelevels") {
            odcs.insert("servicelevels".to_string(), servicelevels.clone());
        }

        if let Some(links) = odcl_data.get("links") {
            odcs.insert("links".to_string(), links.clone());
        }

        // Convert servers format
        if let Some(servers) = odcl_data.get("servers") {
            let converted_servers = Self::convert_servers_format(servers)?;
            odcs.insert("servers".to_string(), converted_servers);
        }

        // Copy any other top-level fields that might be ODCS-compatible
        // (domain, dataProduct, tenant, pricing, team, roles, infrastructure)
        for (key, value) in odcl_data.as_object().unwrap_or(&serde_json::Map::new()) {
            if !matches!(
                key.as_str(),
                "dataContractSpecification"
                    | "id"
                    | "info"
                    | "models"
                    | "definitions"
                    | "servers"
                    | "tags"
                    | "servicelevels"
                    | "links"
                    | "terms"
            ) {
                // These are ODCS-compatible fields, copy them directly
                if matches!(
                    key.as_str(),
                    "domain"
                        | "dataProduct"
                        | "tenant"
                        | "pricing"
                        | "team"
                        | "roles"
                        | "infrastructure"
                ) {
                    odcs.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(JsonValue::Object(odcs))
    }

    /// Convert an ODCL field to an ODCS property.
    #[allow(dead_code)] // Used internally
    fn convert_field_to_property(
        _field_name: &str,
        field_data: &JsonValue,
        odcl_data: &JsonValue,
    ) -> Result<JsonValue> {
        let mut property = serde_json::Map::new();

        // Handle $ref to definitions
        if let Some(ref_str) = field_data.get("$ref").and_then(|v| v.as_str()) {
            // Resolve the reference
            if let Some(resolved) = Self::resolve_ref(ref_str, odcl_data) {
                // Merge resolved definition into property
                if let Some(resolved_obj) = resolved.as_object() {
                    for (key, value) in resolved_obj {
                        // Don't override existing fields in field_data
                        if !field_data
                            .as_object()
                            .map(|o| o.contains_key(key))
                            .unwrap_or(false)
                        {
                            // Convert type if present
                            if key == "type" {
                                property.insert(key.clone(), Self::convert_type(value)?);
                            } else {
                                property.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }
            }
        }

        // Copy all field properties
        if let Some(field_obj) = field_data.as_object() {
            for (key, value) in field_obj {
                // Skip $ref as it's already resolved
                if key == "$ref" {
                    continue;
                }

                // Convert ODCL-specific field names to ODCS equivalents
                match key.as_str() {
                    "required" => {
                        // ODCS uses "required" as boolean, same as ODCL
                        property.insert("required".to_string(), value.clone());
                    }
                    "type" => {
                        // Map type, converting ODCL types to ODCS types if needed
                        property.insert("type".to_string(), Self::convert_type(value)?);
                    }
                    _ => {
                        // Copy other fields as-is (description, quality, etc.)
                        property.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        Ok(JsonValue::Object(property))
    }

    /// Convert ODCL type to ODCS type.
    /// Most types are the same, but we normalize them.
    #[allow(dead_code)] // Used internally
    fn convert_type(type_value: &JsonValue) -> Result<JsonValue> {
        if let Some(type_str) = type_value.as_str() {
            // Normalize type to uppercase for consistency
            let normalized = type_str.to_uppercase();
            Ok(JsonValue::String(normalized))
        } else {
            // If it's not a string, return as-is
            Ok(type_value.clone())
        }
    }

    /// Resolve a $ref reference like '#/definitions/CustomerStatus'.
    #[allow(dead_code)] // Used internally
    fn resolve_ref<'a>(ref_str: &str, data: &'a JsonValue) -> Option<&'a JsonValue> {
        if !ref_str.starts_with("#/") {
            return None;
        }

        // Remove the leading '#/'
        let path = &ref_str[2..];
        let parts: Vec<&str> = path.split('/').collect();

        // Navigate through the data structure
        let mut current = data;
        for part in parts {
            current = current.get(part)?;
        }

        if current.is_object() {
            Some(current)
        } else {
            None
        }
    }

    /// Convert servers format from ODCL to ODCS.
    /// ODCL servers can be:
    /// - Object: { "server_name": { "type": "...", ... } }
    /// - Array: [ { "server": "...", "type": "...", ... } ]
    ///   ODCS servers is always an array: [ { "name": "...", "type": "...", "url": "...", ... } ]
    #[allow(dead_code)] // Used internally
    fn convert_servers_format(servers: &JsonValue) -> Result<JsonValue> {
        match servers {
            JsonValue::Array(arr) => {
                // Already an array, convert each server object
                let mut converted = Vec::new();
                for server in arr {
                    if let Some(server_obj) = server.as_object() {
                        let mut odcs_server = serde_json::Map::new();

                        // Map common fields
                        for (key, value) in server_obj {
                            match key.as_str() {
                                "server" => {
                                    // ODCL uses "server", ODCS uses "name"
                                    odcs_server.insert("name".to_string(), value.clone());
                                }
                                _ => {
                                    // Copy other fields as-is (type, url, description, environment)
                                    odcs_server.insert(key.clone(), value.clone());
                                }
                            }
                        }

                        // If no name field, try to use type or generate one
                        if !odcs_server.contains_key("name") {
                            if let Some(server_type) =
                                odcs_server.get("type").and_then(|v| v.as_str())
                            {
                                odcs_server.insert(
                                    "name".to_string(),
                                    JsonValue::String(format!("{}_server", server_type)),
                                );
                            } else {
                                odcs_server.insert(
                                    "name".to_string(),
                                    JsonValue::String("server".to_string()),
                                );
                            }
                        }

                        converted.push(JsonValue::Object(odcs_server));
                    } else {
                        // Invalid server entry, skip
                        continue;
                    }
                }
                Ok(JsonValue::Array(converted))
            }
            JsonValue::Object(obj) => {
                // Object format: convert to array
                let mut converted = Vec::new();
                for (server_name, server_data) in obj {
                    if let Some(server_obj) = server_data.as_object() {
                        let mut odcs_server = serde_json::Map::new();
                        odcs_server
                            .insert("name".to_string(), JsonValue::String(server_name.clone()));

                        // Copy all fields from server object
                        for (key, value) in server_obj {
                            odcs_server.insert(key.clone(), value.clone());
                        }

                        converted.push(JsonValue::Object(odcs_server));
                    }
                }
                Ok(JsonValue::Array(converted))
            }
            _ => {
                // Invalid format, return empty array
                Ok(JsonValue::Array(Vec::new()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_odcl_conversion() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0",
                "status": "active",
                "description": "Test description",
                "owner": "Test Team",
                "contact": {
                    "name": "Test Contact",
                    "email": "test@example.com"
                }
            },
            "models": {
                "Customer": {
                    "description": "Customer model",
                    "fields": {
                        "id": {
                            "type": "integer",
                            "required": true,
                            "description": "Customer ID"
                        },
                        "name": {
                            "type": "string",
                            "required": true,
                            "description": "Customer name"
                        }
                    }
                }
            },
            "tags": ["customer", "test"],
            "servicelevels": {
                "availability": {
                    "description": "99.9% uptime",
                    "percentage": "99.9%"
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        // Check required fields
        assert_eq!(result["apiVersion"], "v3.1.0");
        assert_eq!(result["kind"], "DataContract");
        assert_eq!(result["id"], "test-contract");
        assert_eq!(result["name"], "Test Contract");
        assert_eq!(result["version"], "1.0.0");
        assert_eq!(result["status"], "active");

        // Check description.purpose
        assert_eq!(result["description"]["purpose"], "Test description");

        // Check customProperties
        assert_eq!(
            result["customProperties"]["dataContractSpecification"],
            "1.2.1"
        );
        assert_eq!(result["customProperties"]["odcl_owner"], "Test Team");
        assert_eq!(
            result["customProperties"]["odcl_contact"]["name"],
            "Test Contact"
        );

        // Check schema
        assert!(result["schema"].is_array());
        assert_eq!(result["schema"][0]["name"], "Customer");
        assert_eq!(result["schema"][0]["description"], "Customer model");
        assert!(result["schema"][0]["properties"].is_object());
        assert_eq!(result["schema"][0]["properties"]["id"]["type"], "INTEGER");
        assert_eq!(result["schema"][0]["properties"]["id"]["required"], true);

        // Check tags
        assert_eq!(result["tags"], serde_json::json!(["customer", "test"]));

        // Check servicelevels
        assert_eq!(
            result["servicelevels"]["availability"]["description"],
            "99.9% uptime"
        );
    }

    #[test]
    fn test_servers_conversion_array_format() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "servers": [
                {
                    "server": "production",
                    "type": "postgres",
                    "url": "postgresql://localhost:5432/db"
                }
            ]
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert!(result["servers"].is_array());
        assert_eq!(result["servers"][0]["name"], "production");
        assert_eq!(result["servers"][0]["type"], "postgres");
        assert_eq!(
            result["servers"][0]["url"],
            "postgresql://localhost:5432/db"
        );
    }

    #[test]
    fn test_servers_conversion_object_format() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "servers": {
                "production": {
                    "type": "postgres",
                    "url": "postgresql://localhost:5432/db",
                    "environment": "production"
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert!(result["servers"].is_array());
        assert_eq!(result["servers"][0]["name"], "production");
        assert_eq!(result["servers"][0]["type"], "postgres");
        assert_eq!(
            result["servers"][0]["url"],
            "postgresql://localhost:5432/db"
        );
        assert_eq!(result["servers"][0]["environment"], "production");
    }

    #[test]
    fn test_definitions_ref_resolution() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "models": {
                "Customer": {
                    "fields": {
                        "status": {
                            "$ref": "#/definitions/CustomerStatus",
                            "required": true
                        }
                    }
                }
            },
            "definitions": {
                "CustomerStatus": {
                    "type": "string",
                    "enum": ["ACTIVE", "INACTIVE", "SUSPENDED"],
                    "description": "Customer status"
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        let status_prop = &result["schema"][0]["properties"]["status"];
        // $ref should be resolved and merged
        assert_eq!(status_prop["type"], "STRING");
        assert_eq!(status_prop["required"], true);
        assert_eq!(status_prop["description"], "Customer status");
        assert!(status_prop["enum"].is_array());
        assert_eq!(
            status_prop["enum"],
            serde_json::json!(["ACTIVE", "INACTIVE", "SUSPENDED"])
        );
        // $ref should not be present after resolution
        assert!(!status_prop.as_object().unwrap().contains_key("$ref"));
    }

    #[test]
    fn test_multiple_models() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "models": {
                "Customer": {
                    "fields": {
                        "id": {
                            "type": "integer",
                            "required": true
                        }
                    }
                },
                "Order": {
                    "fields": {
                        "order_id": {
                            "type": "long",
                            "required": true
                        }
                    }
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert_eq!(result["schema"].as_array().unwrap().len(), 2);
        assert_eq!(result["schema"][0]["name"], "Customer");
        assert_eq!(result["schema"][1]["name"], "Order");
    }

    #[test]
    fn test_terms_in_custom_properties() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "terms": {
                "usage": "Internal use only",
                "legal": "Subject to company policy"
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert_eq!(
            result["customProperties"]["odcl_terms"]["usage"],
            "Internal use only"
        );
        assert_eq!(
            result["customProperties"]["odcl_terms"]["legal"],
            "Subject to company policy"
        );
    }

    #[test]
    fn test_odcs_compatible_fields_preserved() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "domain": "ecommerce",
            "dataProduct": "customer-analytics",
            "tenant": "acme-corp",
            "pricing": {
                "model": "free"
            },
            "team": [
                {
                    "name": "John Doe",
                    "email": "john@example.com"
                }
            ],
            "roles": {
                "viewer": {
                    "description": "Can view data",
                    "permissions": ["read"]
                }
            },
            "infrastructure": {
                "cluster": "production-cluster"
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert_eq!(result["domain"], "ecommerce");
        assert_eq!(result["dataProduct"], "customer-analytics");
        assert_eq!(result["tenant"], "acme-corp");
        assert_eq!(result["pricing"]["model"], "free");
        assert_eq!(result["team"][0]["name"], "John Doe");
        assert_eq!(result["roles"]["viewer"]["description"], "Can view data");
        assert_eq!(result["infrastructure"]["cluster"], "production-cluster");
    }

    #[test]
    fn test_minimal_odcl() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "minimal-contract",
            "info": {
                "title": "Minimal Contract"
            },
            "models": {
                "Table1": {
                    "fields": {}
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert_eq!(result["apiVersion"], "v3.1.0");
        assert_eq!(result["kind"], "DataContract");
        assert_eq!(result["id"], "minimal-contract");
        assert_eq!(result["name"], "Minimal Contract");
        assert!(result["schema"].is_array());
        assert_eq!(result["schema"][0]["name"], "Table1");
    }

    #[test]
    fn test_links_preserved() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "links": {
                "githubRepo": "https://github.com/example/repo",
                "documentation": "https://docs.example.com"
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        assert_eq!(
            result["links"]["githubRepo"],
            "https://github.com/example/repo"
        );
        assert_eq!(result["links"]["documentation"], "https://docs.example.com");
    }

    #[test]
    fn test_quality_rules_preserved() {
        let odcl_data = serde_json::json!({
            "dataContractSpecification": "1.2.1",
            "id": "test-contract",
            "info": {
                "title": "Test Contract",
                "version": "1.0.0"
            },
            "models": {
                "Customer": {
                    "fields": {
                        "email": {
                            "type": "string",
                            "quality": [
                                {
                                    "type": "custom",
                                    "engine": "great-expectations",
                                    "description": "Email validation",
                                    "implementation": {
                                        "expectation_type": "expect_column_values_to_match_regex",
                                        "kwargs": {
                                            "regex": "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let result = ODCLConverter::convert_to_odcs_v3_1_0(&odcl_data).unwrap();

        let email_prop = &result["schema"][0]["properties"]["email"];
        assert!(email_prop["quality"].is_array());
        assert_eq!(email_prop["quality"][0]["type"], "custom");
        assert_eq!(email_prop["quality"][0]["engine"], "great-expectations");
    }
}
