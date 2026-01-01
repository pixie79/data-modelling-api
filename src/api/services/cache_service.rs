//! SQLite cache service for UI performance.

use crate::models::{DataModel, Relationship, Table};
use anyhow::{Context, Result};
use rusqlite::{Connection, Row, params};
use serde_json;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

/// SQLite cache for model data.
#[allow(dead_code)]
pub struct CacheService {
    conn: Connection,
}

#[allow(dead_code)]
impl CacheService {
    /// Create a new cache service instance.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open cache database: {:?}", db_path))?;

        let service = Self { conn };
        service.init_db()?;

        Ok(service)
    }

    /// Initialize database schema.
    fn init_db(&self) -> Result<()> {
        // Tables cache
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tables_cache (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                columns_json TEXT NOT NULL,
                database_type TEXT,
                catalog_name TEXT,
                schema_name TEXT,
                medallion_layer TEXT,
                scd_pattern TEXT,
                data_vault_classification TEXT,
                odcl_metadata_json TEXT,
                position_json TEXT,
                yaml_file_path TEXT,
                drawio_cell_id TEXT,
                created_at TIMESTAMP,
                updated_at TIMESTAMP,
                last_synced_at TIMESTAMP
            )",
            [],
        )?;

        // Relationships cache
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS relationships_cache (
                id TEXT PRIMARY KEY,
                source_table_id TEXT NOT NULL,
                target_table_id TEXT NOT NULL,
                cardinality TEXT,
                foreign_key_details_json TEXT,
                etl_job_metadata_json TEXT,
                relationship_type TEXT,
                drawio_edge_id TEXT,
                visual_metadata_json TEXT,
                created_at TIMESTAMP,
                updated_at TIMESTAMP,
                last_synced_at TIMESTAMP,
                FOREIGN KEY (source_table_id) REFERENCES tables_cache(id),
                FOREIGN KEY (target_table_id) REFERENCES tables_cache(id)
            )",
            [],
        )?;

        // Models cache
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS models_cache (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                git_directory_path TEXT NOT NULL UNIQUE,
                control_file_path TEXT NOT NULL,
                diagram_file_path TEXT,
                created_at TIMESTAMP,
                updated_at TIMESTAMP,
                last_synced_at TIMESTAMP
            )",
            [],
        )?;

        // DrawIO cache table for additional DrawIO-specific data
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS drawio_cache (
                id TEXT PRIMARY KEY,
                table_id TEXT,
                relationship_id TEXT,
                drawio_cell_id TEXT,
                drawio_edge_id TEXT,
                visual_metadata_json TEXT,
                last_updated TIMESTAMP,
                FOREIGN KEY (table_id) REFERENCES tables_cache(id),
                FOREIGN KEY (relationship_id) REFERENCES relationships_cache(id)
            )",
            [],
        )?;

        info!("Cache database initialized");
        Ok(())
    }

    /// Synchronize table cache from table data.
    pub fn sync_table_from_yaml(&self, table: &Table) -> Result<()> {
        let columns_json =
            serde_json::to_string(&table.columns).context("Failed to serialize columns to JSON")?;

        let odcl_metadata_json = serde_json::to_string(&table.odcl_metadata)
            .context("Failed to serialize ODCL metadata to JSON")?;

        let position_json = table
            .position
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("Failed to serialize position to JSON")?;

        let now = chrono::Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO tables_cache (
                id, name, columns_json, database_type, catalog_name, schema_name,
                medallion_layer, scd_pattern, data_vault_classification, odcl_metadata_json,
                position_json, yaml_file_path, drawio_cell_id, created_at, updated_at, last_synced_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                table.id.to_string(),
                table.name,
                columns_json,
                table.database_type.as_ref().map(|dt| format!("{:?}", dt)),
                table.catalog_name,
                table.schema_name,
                table.medallion_layers.first().map(|l| format!("{:?}", l)),
                table.scd_pattern.as_ref().map(|p| format!("{:?}", p)),
                table.data_vault_classification.as_ref().map(|c| format!("{:?}", c)),
                odcl_metadata_json,
                position_json,
                table.yaml_file_path,
                table.drawio_cell_id,
                table.created_at.to_rfc3339(),
                table.updated_at.to_rfc3339(),
                now,
            ],
        )?;

        Ok(())
    }

    /// Synchronize relationship cache from relationship data.
    pub fn sync_relationship_from_yaml(&self, relationship: &Relationship) -> Result<()> {
        let foreign_key_details_json = relationship
            .foreign_key_details
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("Failed to serialize foreign key details to JSON")?;

        let etl_job_metadata_json = relationship
            .etl_job_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("Failed to serialize ETL job metadata to JSON")?;

        let visual_metadata_json = relationship
            .visual_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("Failed to serialize visual metadata to JSON")?;

        let now = chrono::Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO relationships_cache (
                id, source_table_id, target_table_id, cardinality,
                foreign_key_details_json, etl_job_metadata_json, relationship_type,
                drawio_edge_id, visual_metadata_json, created_at, updated_at, last_synced_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                relationship.id.to_string(),
                relationship.source_table_id.to_string(),
                relationship.target_table_id.to_string(),
                relationship
                    .cardinality
                    .as_ref()
                    .map(|c| format!("{:?}", c)),
                foreign_key_details_json,
                etl_job_metadata_json,
                relationship
                    .relationship_type
                    .as_ref()
                    .map(|rt| format!("{:?}", rt)),
                relationship.drawio_edge_id,
                visual_metadata_json,
                relationship.created_at.to_rfc3339(),
                relationship.updated_at.to_rfc3339(),
                now,
            ],
        )?;

        Ok(())
    }

    /// Synchronize entire model from YAML files.
    pub fn sync_model_from_yaml(&self, model: &DataModel) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Sync model metadata
        self.conn.execute(
            "INSERT OR REPLACE INTO models_cache (
                id, name, description, git_directory_path, control_file_path,
                diagram_file_path, created_at, updated_at, last_synced_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                model.id.to_string(),
                model.name,
                model.description,
                model.git_directory_path,
                model.control_file_path,
                model.diagram_file_path,
                model.created_at.to_rfc3339(),
                model.updated_at.to_rfc3339(),
                now,
            ],
        )?;

        // Sync all tables
        for table in &model.tables {
            if let Err(e) = self.sync_table_from_yaml(table) {
                warn!("Failed to sync table {} to cache: {}", table.name, e);
            }
        }

        // Sync all relationships
        for relationship in &model.relationships {
            if let Err(e) = self.sync_relationship_from_yaml(relationship) {
                warn!(
                    "Failed to sync relationship {} to cache: {}",
                    relationship.id, e
                );
            }
        }

        Ok(())
    }

    /// Retrieve table data from cache.
    pub fn get_table_from_cache(&self, table_id: &str) -> Result<Option<Table>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tables_cache WHERE id = ?1")?;

        let mut rows = stmt.query_map(params![table_id], |row| self.row_to_table(row))?;

        if let Some(row_result) = rows.next() {
            row_result
                .map(Some)
                .context("Failed to parse table from cache")
        } else {
            Ok(None)
        }
    }

    /// Retrieve relationship data from cache.
    pub fn get_relationship_from_cache(
        &self,
        relationship_id: &str,
    ) -> Result<Option<Relationship>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM relationships_cache WHERE id = ?1")?;

        let mut rows = stmt.query_map(params![relationship_id], |row| {
            self.row_to_relationship(row)
        })?;

        if let Some(row_result) = rows.next() {
            row_result
                .map(Some)
                .context("Failed to parse relationship from cache")
        } else {
            Ok(None)
        }
    }

    /// Get all tables from cache.
    pub fn get_all_tables_from_cache(&self) -> Result<Vec<Table>> {
        let mut stmt = self.conn.prepare("SELECT * FROM tables_cache")?;
        let rows = stmt.query_map([], |row| self.row_to_table(row))?;

        let mut tables = Vec::new();
        for row_result in rows {
            match row_result {
                Ok(table) => tables.push(table),
                Err(e) => {
                    warn!("Failed to parse table from cache: {}", e);
                }
            }
        }

        Ok(tables)
    }

    /// Get all relationships from cache.
    pub fn get_all_relationships_from_cache(&self) -> Result<Vec<Relationship>> {
        let mut stmt = self.conn.prepare("SELECT * FROM relationships_cache")?;
        let rows = stmt.query_map([], |row| self.row_to_relationship(row))?;

        let mut relationships = Vec::new();
        for row_result in rows {
            match row_result {
                Ok(relationship) => relationships.push(relationship),
                Err(e) => {
                    warn!("Failed to parse relationship from cache: {}", e);
                }
            }
        }

        Ok(relationships)
    }

    /// Get model metadata from cache.
    pub fn get_model_metadata(&self) -> Result<Option<(String, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, git_directory_path FROM models_cache LIMIT 1")?;

        let mut rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("name")?,
                row.get::<_, String>("git_directory_path")?,
            ))
        })?;

        if let Some(row_result) = rows.next() {
            row_result
                .map(Some)
                .context("Failed to parse model metadata from cache")
        } else {
            Ok(None)
        }
    }

    /// Get counts of tables and relationships in cache.
    pub fn get_cache_counts(&self) -> Result<(usize, usize)> {
        let tables_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM tables_cache", [], |row| row.get(0))?;
        let relationships_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM relationships_cache", [], |row| {
                    row.get(0)
                })?;
        Ok((tables_count as usize, relationships_count as usize))
    }

    /// Clear all cache data.
    pub fn clear_cache(&self) -> Result<()> {
        self.conn.execute("DELETE FROM tables_cache", [])?;
        self.conn.execute("DELETE FROM relationships_cache", [])?;
        self.conn.execute("DELETE FROM models_cache", [])?;
        self.conn.execute("DELETE FROM drawio_cache", [])?;
        info!("Cache cleared");
        Ok(())
    }

    /// Convert database row to Table.
    fn row_to_table(&self, row: &Row) -> rusqlite::Result<Table> {
        use crate::models::Column;
        use crate::models::Position;

        let id_str: String = row.get("id")?;
        let id = Uuid::parse_str(&id_str).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text)
        })?;

        let columns_json: String = row.get("columns_json")?;
        let columns: Vec<Column> = serde_json::from_str(&columns_json).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(
                0,
                "columns_json".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        let position_json: Option<String> = row.get("position_json")?;
        let position = position_json
            .as_ref()
            .and_then(|json| serde_json::from_str::<Position>(json).ok());

        let odcl_metadata_json: String = row.get("odcl_metadata_json")?;
        let odcl_metadata: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(&odcl_metadata_json).unwrap_or_default();

        let created_at_str: String = row.get("created_at")?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let updated_at_str: String = row.get("updated_at")?;
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        Ok(Table {
            id,
            name: row.get("name")?,
            columns,
            database_type: row
                .get::<_, Option<String>>("database_type")?
                .and_then(|s| match s.as_str() {
                    "Postgres" => Some(crate::models::enums::DatabaseType::Postgres),
                    "Mysql" => Some(crate::models::enums::DatabaseType::Mysql),
                    "SqlServer" => Some(crate::models::enums::DatabaseType::SqlServer),
                    "DatabricksDelta" => Some(crate::models::enums::DatabaseType::DatabricksDelta),
                    "AwsGlue" => Some(crate::models::enums::DatabaseType::AwsGlue),
                    _ => None,
                }),
            catalog_name: row.get::<_, Option<String>>("catalog_name")?,
            schema_name: row.get::<_, Option<String>>("schema_name")?,
            medallion_layers: row
                .get::<_, Option<String>>("medallion_layers")?
                .map(|s| {
                    s.split(',')
                        .filter_map(|l| match l.trim() {
                            "Bronze" => Some(crate::models::enums::MedallionLayer::Bronze),
                            "Silver" => Some(crate::models::enums::MedallionLayer::Silver),
                            "Gold" => Some(crate::models::enums::MedallionLayer::Gold),
                            "Operational" => {
                                Some(crate::models::enums::MedallionLayer::Operational)
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            scd_pattern: row.get::<_, Option<String>>("scd_pattern")?.and_then(|s| {
                match s.as_str() {
                    "Type1" => Some(crate::models::enums::SCDPattern::Type1),
                    "Type2" => Some(crate::models::enums::SCDPattern::Type2),
                    _ => None,
                }
            }),
            data_vault_classification: row
                .get::<_, Option<String>>("data_vault_classification")?
                .and_then(|s| match s.as_str() {
                    "Hub" => Some(crate::models::enums::DataVaultClassification::Hub),
                    "Link" => Some(crate::models::enums::DataVaultClassification::Link),
                    "Satellite" => Some(crate::models::enums::DataVaultClassification::Satellite),
                    _ => None,
                }),
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata,
            position,
            yaml_file_path: row.get("yaml_file_path")?,
            drawio_cell_id: row.get("drawio_cell_id")?,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at,
            updated_at,
        })
    }

    /// Convert database row to Relationship.
    fn row_to_relationship(&self, row: &Row) -> rusqlite::Result<Relationship> {
        use crate::models::enums::{Cardinality, RelationshipType};
        use crate::models::relationship::{ETLJobMetadata, ForeignKeyDetails, VisualMetadata};

        let id_str: String = row.get("id")?;
        let id = Uuid::parse_str(&id_str).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text)
        })?;

        let source_table_id_str: String = row.get("source_table_id")?;
        let source_table_id = Uuid::parse_str(&source_table_id_str).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(
                0,
                "source_table_id".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        let target_table_id_str: String = row.get("target_table_id")?;
        let target_table_id = Uuid::parse_str(&target_table_id_str).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(
                0,
                "target_table_id".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        let cardinality_str: Option<String> = row.get("cardinality")?;
        let cardinality = cardinality_str.as_ref().and_then(|s| match s.as_str() {
            "OneToOne" => Some(Cardinality::OneToOne),
            "OneToMany" => Some(Cardinality::OneToMany),
            "ManyToOne" => Some(Cardinality::ManyToOne),
            "ManyToMany" => Some(Cardinality::ManyToMany),
            _ => None,
        });

        let relationship_type_str: Option<String> = row.get("relationship_type")?;
        let relationship_type = relationship_type_str
            .as_ref()
            .and_then(|s| match s.as_str() {
                "DataFlow" => Some(RelationshipType::DataFlow),
                "Dependency" => Some(RelationshipType::Dependency),
                "ForeignKey" => Some(RelationshipType::ForeignKey),
                "EtlTransformation" => Some(RelationshipType::EtlTransformation),
                _ => None,
            });

        let foreign_key_details_json: Option<String> = row.get("foreign_key_details_json")?;
        let foreign_key_details = foreign_key_details_json
            .as_ref()
            .and_then(|json| serde_json::from_str::<ForeignKeyDetails>(json).ok());

        let etl_job_metadata_json: Option<String> = row.get("etl_job_metadata_json")?;
        let etl_job_metadata = etl_job_metadata_json
            .as_ref()
            .and_then(|json| serde_json::from_str::<ETLJobMetadata>(json).ok());

        let visual_metadata_json: Option<String> = row.get("visual_metadata_json")?;
        let visual_metadata = visual_metadata_json
            .as_ref()
            .and_then(|json| serde_json::from_str::<VisualMetadata>(json).ok());

        let created_at_str: String = row.get("created_at")?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let updated_at_str: String = row.get("updated_at")?;
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        Ok(Relationship {
            id,
            source_table_id,
            target_table_id,
            cardinality,
            source_optional: None, // Not stored in cache yet
            target_optional: None, // Not stored in cache yet
            foreign_key_details,
            etl_job_metadata,
            relationship_type,
            notes: None, // Notes not stored in cache, loaded from YAML
            visual_metadata,
            drawio_edge_id: row.get("drawio_edge_id")?,
            created_at,
            updated_at,
        })
    }
}
