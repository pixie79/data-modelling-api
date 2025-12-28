#[cfg(test)]
mod tests {
    use data_modelling_api::api::models::{DataModel, Table, Column, Relationship};
    use uuid::Uuid;

    #[test]
    fn test_data_model_creation() {
        let model = DataModel::new(
            "Test Model".to_string(),
            "/path/to/git".to_string(),
            "relationships.yaml".to_string(),
        );

        assert_eq!(model.name, "Test Model");
        assert_eq!(model.git_directory_path, "/path/to/git");
        assert_eq!(model.tables.len(), 0);
        assert_eq!(model.relationships.len(), 0);
    }

    #[test]
    fn test_get_table_by_id() {
        let mut model = DataModel::new(
            "Test Model".to_string(),
            "/path/to/git".to_string(),
            "relationships.yaml".to_string(),
        );

        let columns = vec![Column::new("id".to_string(), "INTEGER".to_string())];
        let table = Table::new("users".to_string(), columns);
        let table_id = table.id;
        model.tables.push(table);

        let found = model.get_table_by_id(table_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "users");
    }

    #[test]
    fn test_get_table_by_name() {
        let mut model = DataModel::new(
            "Test Model".to_string(),
            "/path/to/git".to_string(),
            "relationships.yaml".to_string(),
        );

        let columns = vec![Column::new("id".to_string(), "INTEGER".to_string())];
        let table = Table::new("users".to_string(), columns);
        model.tables.push(table);

        let found = model.get_table_by_name("users");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "users");
    }

    #[test]
    fn test_get_relationships_for_table() {
        let mut model = DataModel::new(
            "Test Model".to_string(),
            "/path/to/git".to_string(),
            "relationships.yaml".to_string(),
        );

        let table1_id = Uuid::new_v4();
        let table2_id = Uuid::new_v4();

        let relationship = Relationship::new(table1_id, table2_id);
        model.relationships.push(relationship);

        let relationships = model.get_relationships_for_table(table1_id);
        assert_eq!(relationships.len(), 1);
    }

    #[test]
    fn test_data_model_serialization() {
        let model = DataModel::new(
            "Test Model".to_string(),
            "/path/to/git".to_string(),
            "relationships.yaml".to_string(),
        );

        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("Test Model"));
        assert!(json.contains("/path/to/git"));
    }
}
