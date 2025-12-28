#[cfg(test)]
mod tests {
    use data_modelling_api::api::models::{Column, Table, MedallionLayer, DatabaseType};

    #[test]
    fn test_table_creation() {
        let columns = vec![
            Column::new("id".to_string(), "INTEGER".to_string()),
            Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        ];
        let table = Table::new("users".to_string(), columns.clone());

        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].name, "id");
        assert_eq!(table.columns[1].name, "name");
    }

    #[test]
    fn test_table_serialization() {
        let columns = vec![Column::new("id".to_string(), "INTEGER".to_string())];
        let mut table = Table::new("users".to_string(), columns);
        table.medallion_layers.push(MedallionLayer::Bronze);
        table.database_type = Some(DatabaseType::Postgres);

        let json = serde_json::to_string(&table).unwrap();
        assert!(json.contains("users"));
        assert!(json.contains("bronze"));
    }

    #[test]
    fn test_table_deserialization() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "users",
            "columns": [
                {"name": "id", "data_type": "INTEGER", "nullable": true, "primary_key": false, "secondary_key": false, "constraints": [], "description": "", "errors": [], "quality": [], "enum_values": [], "column_order": 0}
            ],
            "medallion_layers": ["bronze"],
            "tags": [],
            "odcl_metadata": {},
            "quality": [],
            "errors": [],
            "created_at": "2025-11-30T00:00:00Z",
            "updated_at": "2025-11-30T00:00:00Z"
        }"#;

        let table: Table = serde_json::from_str(json).unwrap();
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 1);
        assert_eq!(table.medallion_layers.len(), 1);
    }

    #[test]
    fn test_table_unique_key() {
        let columns = vec![Column::new("id".to_string(), "INTEGER".to_string())];
        let mut table = Table::new("users".to_string(), columns);
        table.database_type = Some(DatabaseType::Postgres);
        table.catalog_name = Some("my_catalog".to_string());
        table.schema_name = Some("public".to_string());

        let key = table.get_unique_key();
        assert!(key.0.is_some());
        assert_eq!(key.1, "users");
        assert_eq!(key.2, Some("my_catalog".to_string()));
        assert_eq!(key.3, Some("public".to_string()));
    }
}
