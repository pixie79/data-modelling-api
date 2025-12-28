#[cfg(test)]
mod tests {
    use data_modelling_api::api::models::{Relationship, Cardinality, RelationshipType};
    use uuid::Uuid;

    #[test]
    fn test_relationship_creation() {
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let relationship = Relationship::new(source_id, target_id);

        assert_eq!(relationship.source_table_id, source_id);
        assert_eq!(relationship.target_table_id, target_id);
    }

    #[test]
    fn test_relationship_serialization() {
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let mut relationship = Relationship::new(source_id, target_id);
        relationship.cardinality = Some(Cardinality::OneToMany);
        relationship.relationship_type = Some(RelationshipType::ForeignKey);

        let json = serde_json::to_string(&relationship).unwrap();
        assert!(json.contains("One-to-Many") || json.contains("OneToMany"));
    }

    #[test]
    fn test_relationship_deserialization() {
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let json = format!(r#"{{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source_table_id": "{}",
            "target_table_id": "{}",
            "cardinality": "One-to-Many",
            "created_at": "2025-11-30T00:00:00Z",
            "updated_at": "2025-11-30T00:00:00Z"
        }}"#, source_id, target_id);

        let relationship: Relationship = serde_json::from_str(&json).unwrap();
        assert_eq!(relationship.source_table_id, source_id);
        assert_eq!(relationship.target_table_id, target_id);
        assert_eq!(relationship.cardinality, Some(Cardinality::OneToMany));
    }
}
