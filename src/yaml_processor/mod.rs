// YAML processor module for fast YAML operations
// Note: This is a legacy module kept for compatibility
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: Option<bool>,
    pub primary_key: Option<bool>,
    pub foreign_key: Option<HashMap<String, String>>,
    pub constraints: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub database_type: Option<String>,
    pub medallion_layer: Option<String>,
    pub scd_pattern: Option<String>,
    pub data_vault_classification: Option<String>,
    pub odcl_metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Process ODCL YAML content
#[allow(dead_code)]
pub fn process_yaml(yaml_content: &str) -> Result<Table, String> {
    // Parse YAML using yaml-rust
    let docs = yaml_rust::YamlLoader::load_from_str(yaml_content)
        .map_err(|e| format!("YAML parsing error: {}", e))?;

    if docs.is_empty() {
        return Err("Empty YAML document".to_string());
    }

    let doc = &docs[0];

    // Extract table name
    let name = doc["name"]
        .as_str()
        .ok_or("Missing required field: name")?
        .to_string();

    // Extract columns
    let columns_vec = doc["columns"]
        .as_vec()
        .ok_or("Missing required field: columns")?;

    let mut columns = Vec::new();
    for col_doc in columns_vec {
        let col = Column {
            name: col_doc["name"]
                .as_str()
                .ok_or("Column missing name")?
                .to_string(),
            data_type: col_doc["data_type"]
                .as_str()
                .ok_or("Column missing data_type")?
                .to_string(),
            nullable: col_doc["nullable"].as_bool(),
            primary_key: col_doc["primary_key"].as_bool(),
            foreign_key: None, // Would need to parse if present
            constraints: col_doc["constraints"].as_vec().map(|v| {
                v.iter()
                    .filter_map(|s| s.as_str())
                    .map(|s| s.to_string())
                    .collect()
            }),
        };
        columns.push(col);
    }

    // Extract optional metadata
    let database_type = doc["database_type"].as_str().map(|s| s.to_string());
    let medallion_layer = doc["medallion_layer"].as_str().map(|s| s.to_string());
    let scd_pattern = doc["scd_pattern"].as_str().map(|s| s.to_string());
    let data_vault_classification = doc["data_vault_classification"]
        .as_str()
        .map(|s| s.to_string());

    // Extract odcl_metadata
    let odcl_metadata = if doc["odcl_metadata"].is_badvalue() {
        None
    } else {
        // Convert YAML hash to HashMap
        let mut metadata = HashMap::new();
        if let Some(hash) = doc["odcl_metadata"].as_hash() {
            for (k, v) in hash {
                if let Some(key) = k.as_str() {
                    // Convert YAML value to JSON value (simplified)
                    if let Some(s) = v.as_str() {
                        metadata.insert(key.to_string(), serde_json::Value::String(s.to_string()));
                    } else if let Some(i) = v.as_i64() {
                        metadata.insert(key.to_string(), serde_json::Value::Number(i.into()));
                    } else if let Some(b) = v.as_bool() {
                        metadata.insert(key.to_string(), serde_json::Value::Bool(b));
                    }
                }
            }
        }
        Some(metadata)
    };

    Ok(Table {
        name,
        columns,
        database_type,
        medallion_layer,
        scd_pattern,
        data_vault_classification,
        odcl_metadata,
    })
}

/// Validate ODCL YAML structure
pub fn validate_odcl(yaml_content: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let docs = match yaml_rust::YamlLoader::load_from_str(yaml_content) {
        Ok(docs) => docs,
        Err(e) => {
            errors.push(format!("Invalid YAML: {}", e));
            return Err(errors);
        }
    };

    if docs.is_empty() {
        errors.push("Empty YAML document".to_string());
        return Err(errors);
    }

    let doc = &docs[0];

    // Check required fields
    if doc["name"].is_badvalue() {
        errors.push("Missing required field: name".to_string());
    }

    if doc["columns"].is_badvalue() {
        errors.push("Missing required field: columns".to_string());
    } else if let Some(cols) = doc["columns"].as_vec() {
        if cols.is_empty() {
            errors.push("Columns array cannot be empty".to_string());
        }

        // Validate each column
        for (i, col) in cols.iter().enumerate() {
            if col["name"].is_badvalue() {
                errors.push(format!("Column {} missing 'name' field", i));
            }
            if col["data_type"].is_badvalue() {
                errors.push(format!("Column {} missing 'data_type' field", i));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_simple_yaml() {
        let yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
    nullable: false
    primary_key: true
  - name: name
    data_type: VARCHAR(255)
"#;
        let result = process_yaml(yaml);
        assert!(result.is_ok());
        let table = result.unwrap();
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 2);
    }

    #[test]
    fn test_validate_odcl() {
        let yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
"#;
        let result = validate_odcl(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_odcl_missing_fields() {
        let yaml = "name: users";
        let result = validate_odcl(yaml);
        assert!(result.is_err());
    }
}
