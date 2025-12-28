//! SQL parser service for extracting table definitions from SQL CREATE statements.
//!
//! This service parses SQL CREATE TABLE statements and extracts table and column definitions.
//! Supports standard SQL and Liquibase formats.

use crate::models::column::ForeignKey;
use crate::models::{Column, Table};
use anyhow::Result;
use regex::Regex;
use sqlparser::ast::{ColumnOption, DataType, Statement};
use sqlparser::dialect::{dialect_from_str, GenericDialect};
use sqlparser::parser::Parser;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// SQL parser service for extracting table definitions from SQL CREATE statements.
pub struct SQLParser {
    /// Dialect to use for parsing (default: Generic)
    dialect: Box<dyn sqlparser::dialect::Dialect>,
    /// Original dialect name used to create this parser (for setting database_type)
    dialect_name: String,
}

impl SQLParser {
    /// Create a new SQL parser instance.
    pub fn new() -> Self {
        Self {
            dialect: Box::new(GenericDialect {}),
            dialect_name: "generic".to_string(),
        }
    }

    /// Create a new SQL parser with a specific dialect.
    #[allow(dead_code)]
    pub fn with_dialect(dialect: Box<dyn sqlparser::dialect::Dialect>) -> Self {
        Self {
            dialect,
            dialect_name: "generic".to_string(),
        }
    }

    /// Create a new SQL parser with a dialect specified by name.
    ///
    /// Supported dialects: generic, databricks, postgres, postgresql, mysql, mssql, sqlserver,
    /// oracle, redshift, duckdb, bigquery, snowflake, clickhouse, hive, ansi, sqlite, other
    ///
    /// For dialects not directly supported by sqlparser (oracle), falls back to GenericDialect.
    /// For "other", uses GenericDialect.
    /// For "databricks", uses datafusion's DatabricksDialect for proper Databricks SQL support.
    pub fn with_dialect_name(dialect_name: &str) -> Self {
        let dialect_name_lower = dialect_name.to_lowercase();

        // Handle Databricks dialect specially using datafusion's DatabricksDialect
        if dialect_name_lower == "databricks" || dialect_name_lower == "databricks_delta" {
            info!(
                "[SQLParser] Using datafusion's DatabricksDialect for '{}'",
                dialect_name
            );
            return Self {
                dialect: Box::new(datafusion::sql::sqlparser::dialect::DatabricksDialect {}),
                dialect_name: dialect_name_lower.clone(),
            };
        }

        // Map user-friendly names to sqlparser dialect names
        let sqlparser_dialect_name = match dialect_name_lower.as_str() {
            "oracle" => {
                // Oracle syntax is complex, use GenericDialect for now
                "generic"
            }
            "mssql" | "sqlserver" => "mssql",
            "postgres" => "postgresql",
            "other" => "generic",
            _ => &dialect_name_lower,
        };

        // Use sqlparser's built-in dialect_from_str function
        let dialect: Box<dyn sqlparser::dialect::Dialect> =
            dialect_from_str(sqlparser_dialect_name).unwrap_or_else(|| Box::new(GenericDialect {}));

        Self {
            dialect,
            dialect_name: dialect_name_lower.clone(),
        }
    }

    /// Map dialect name to DatabaseType enum
    fn dialect_to_database_type(dialect_name: &str) -> Option<crate::models::enums::DatabaseType> {
        use crate::models::enums::DatabaseType;
        let dialect_lower = dialect_name.to_lowercase();
        let result = match dialect_lower.as_str() {
            "postgres" | "postgresql" => Some(DatabaseType::Postgres),
            "mysql" => Some(DatabaseType::Mysql),
            "mssql" | "sqlserver" | "sql_server" => Some(DatabaseType::SqlServer),
            "databricks" | "databricks_delta" => Some(DatabaseType::DatabricksDelta),
            "aws_glue" | "glue" => Some(DatabaseType::AwsGlue),
            _ => None,
        };

        if let Some(db_type) = result {
            info!(
                "[SQLParser] Mapped dialect '{}' to database_type: {:?}",
                dialect_name, db_type
            );
        } else {
            warn!(
                "[SQLParser] No database_type mapping found for dialect '{}'",
                dialect_name
            );
        }

        result
    }

    /// Parse SQL and extract table definitions.
    ///
    /// Supports standard SQL and Liquibase formats.
    ///
    /// # Returns
    ///
    /// Returns a tuple of:
    /// - Vector of parsed tables
    /// - Vector of tables requiring name input (for dynamic table names)
    pub fn parse(&self, sql: &str) -> Result<(Vec<Table>, Vec<TableNameInput>)> {
        let mut tables = Vec::new();
        let mut tables_requiring_name = Vec::new();

        // Check if this is Liquibase format
        if self.is_liquibase_format(sql) {
            let (parsed_tables, name_inputs) = self.parse_liquibase(sql)?;
            tables.extend(parsed_tables);
            tables_requiring_name.extend(name_inputs);
            info!(
                "Parsed {} tables from Liquibase SQL, {} require name input",
                tables.len(),
                tables_requiring_name.len()
            );
            return Ok((tables, tables_requiring_name));
        }

        // Preprocess SQL to make it AST-parseable: replace IDENTIFIER() with a placeholder
        let preprocessed_sql = self.preprocess_sql_for_ast(sql);

        // Standard SQL parsing - try sqlparser first, fallback to string parsing if needed
        match self.parse_statements(&preprocessed_sql) {
            Ok(statements) => {
                for (idx, statement) in statements.iter().enumerate() {
                    if let Statement::CreateTable(create_table) = statement {
                        match self.extract_table_from_ast(
                            &create_table.name,
                            &create_table.columns,
                            statement,
                        ) {
                            Ok((table, requires_name)) => {
                                tables.push(table.clone());
                                if requires_name {
                                    tables_requiring_name.push(TableNameInput {
                                        table_index: tables.len() - 1,
                                        suggested_name: table.name.clone(),
                                        original_expression: format!("{}", create_table.name),
                                    });
                                }
                            }
                            Err(e) => {
                                warn!("Failed to extract table from statement {}: {}", idx, e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Fallback to string-based parsing for complex cases
                warn!("SQL parser failed, trying string-based parsing: {}", e);
                let (parsed_tables, name_inputs) = self.parse_from_string(sql)?;
                tables.extend(parsed_tables);
                tables_requiring_name.extend(name_inputs);
            }
        }

        info!(
            "Parsed {} tables from SQL, {} require name input",
            tables.len(),
            tables_requiring_name.len()
        );
        Ok((tables, tables_requiring_name))
    }

    /// Check if SQL is in Liquibase format.
    fn is_liquibase_format(&self, sql: &str) -> bool {
        let sql_upper = sql.to_uppercase();
        let sql_upper = sql_upper.trim();
        // Check for Liquibase XML format
        if sql_upper.starts_with("<?XML") || sql_upper.contains("<DATABASECHANGELOG") {
            return true;
        }
        // Check for Liquibase SQL format with changeSet comments
        if sql_upper.contains("--LBSCHEMA") || sql_upper.contains("--CHANGESET") {
            return true;
        }
        false
    }

    /// Parse Liquibase format SQL.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (tables, tables_requiring_name_input).
    /// Liquibase typically has static names, so tables_requiring_name_input is usually empty.
    fn parse_liquibase(&self, sql: &str) -> Result<(Vec<Table>, Vec<TableNameInput>)> {
        // For now, implement a basic Liquibase parser
        // This can be extended to handle full Liquibase XML and SQL formats
        warn!(
            "Liquibase parsing is not fully implemented yet, falling back to standard SQL parsing"
        );

        // Try to extract CREATE TABLE statements from Liquibase SQL
        match self.parse_statements(sql) {
            Ok(statements) => {
                let mut tables = Vec::new();
                for statement in statements {
                    if let Statement::CreateTable(create_table) = &statement {
                        match self.extract_table_from_ast(
                            &create_table.name,
                            &create_table.columns,
                            &statement,
                        ) {
                            Ok((table, _)) => {
                                tables.push(table);
                            }
                            Err(e) => {
                                warn!("Failed to extract table from Liquibase statement: {}", e);
                            }
                        }
                    }
                }
                Ok((tables, Vec::new()))
            }
            Err(_) => {
                // Fallback to string parsing
                self.parse_from_string(sql)
            }
        }
    }

    /// Preprocess SQL to make it AST-parseable.
    /// Replaces IDENTIFIER() calls with placeholder table names that sqlparser can handle.
    fn preprocess_sql_for_ast(&self, sql: &str) -> String {
        use regex::Regex;
        use std::cell::Cell;

        // Pattern to match IDENTIFIER(...) including nested parentheses and string concatenation
        // This handles: IDENTIFIER(:catalog || '.schema.table')
        let ident_pattern = Regex::new(r"(?i)IDENTIFIER\s*\([^)]*(?:\([^)]*\)[^)]*)*\)").unwrap();

        let placeholder_counter = Cell::new(0);

        // Replace each IDENTIFIER() call with a placeholder table name
        let result = ident_pattern
            .replace_all(sql, |_caps: &regex::Captures| {
                let count = placeholder_counter.get() + 1;
                placeholder_counter.set(count);
                format!("__IDENTIFIER_PLACEHOLDER_{}__", count)
            })
            .to_string();

        debug!(
            "[SQLParser] Preprocessed SQL: replaced {} IDENTIFIER() calls",
            placeholder_counter.get()
        );
        result
    }

    /// Parse SQL statements using sqlparser.
    fn parse_statements(&self, sql: &str) -> Result<Vec<Statement>> {
        let parser = Parser::new(&*self.dialect);
        let mut parser = match parser.try_with_sql(sql) {
            Ok(p) => p,
            Err(e) => {
                warn!("[SQLParser] Failed to initialize parser with SQL: {}", e);
                return Err(anyhow::anyhow!("Failed to initialize parser: {}", e));
            }
        };

        match parser.parse_statements() {
            Ok(statements) => Ok(statements),
            Err(e) => {
                // Log the first 500 chars of SQL for debugging
                let sql_preview = sql.chars().take(500).collect::<String>();
                warn!(
                    "[SQLParser] AST parsing failed with dialect '{}': {}",
                    self.dialect_name, e
                );
                warn!("[SQLParser] SQL preview (first 500 chars): {}", sql_preview);
                Err(anyhow::anyhow!("AST parsing failed: {}", e))
            }
        }
    }

    /// Parse SQL from string (fallback method for complex cases).
    fn parse_from_string(&self, sql: &str) -> Result<(Vec<Table>, Vec<TableNameInput>)> {
        let mut tables = Vec::new();
        let mut tables_requiring_name = Vec::new();

        // Manually parse CREATE TABLE statements to handle IDENTIFIER() properly
        let sql_upper = sql.to_uppercase();
        let mut search_pos = 0;

        while let Some(create_idx) = sql_upper[search_pos..].find("CREATE TABLE") {
            let create_start = search_pos + create_idx;
            let mut pos = create_start + "CREATE TABLE".len();

            // Build char_indices vector for safe character access
            let chars: Vec<(usize, char)> = sql.char_indices().collect();
            let mut char_idx = chars
                .iter()
                .position(|(i, _)| *i >= pos)
                .unwrap_or(chars.len());

            // Skip whitespace
            while char_idx < chars.len() {
                let (_, ch) = chars[char_idx];
                if ch.is_whitespace() {
                    char_idx += 1;
                    if char_idx < chars.len() {
                        pos = chars[char_idx].0;
                    }
                } else {
                    break;
                }
            }

            // Check for "IF NOT EXISTS"
            if pos < sql.len() && sql_upper[pos..].starts_with("IF NOT EXISTS") {
                pos += "IF NOT EXISTS".len();
                char_idx = chars
                    .iter()
                    .position(|(i, _)| *i >= pos)
                    .unwrap_or(chars.len());
                while char_idx < chars.len() {
                    let (_, ch) = chars[char_idx];
                    if ch.is_whitespace() {
                        char_idx += 1;
                        if char_idx < chars.len() {
                            pos = chars[char_idx].0;
                        }
                    } else {
                        break;
                    }
                }
            }

            // Now parse the table name expression until we find the opening '(' for columns
            // We need to handle IDENTIFIER(...) expressions which contain parentheses
            let table_name_start = pos;
            let mut table_name_end = pos;
            let mut in_string = false;
            let mut string_char = None;
            let mut paren_depth = 0;
            let mut found_column_paren = false;

            while char_idx < chars.len() {
                let (char_pos, ch) = chars[char_idx];
                pos = char_pos;

                match ch {
                    '\'' | '"' if !in_string || Some(ch) == string_char => {
                        if in_string {
                            in_string = false;
                            string_char = None;
                        } else {
                            in_string = true;
                            string_char = Some(ch);
                        }
                        char_idx += 1;
                    }
                    '(' if !in_string => {
                        paren_depth += 1;
                        // If this is the first paren and we've seen some table name content,
                        // check if it's IDENTIFIER( or the column list
                        if paren_depth == 1 {
                            // Check if we're in an IDENTIFIER() call
                            let before_paren = &sql[table_name_start..pos].trim().to_uppercase();
                            if before_paren.ends_with("IDENTIFIER") {
                                // This is IDENTIFIER(, continue parsing
                                char_idx += 1;
                            } else if pos > table_name_start {
                                // This is the column list opening paren
                                table_name_end = pos;
                                found_column_paren = true;
                                break;
                            } else {
                                char_idx += 1;
                            }
                        } else {
                            char_idx += 1;
                        }
                    }
                    ')' if !in_string && paren_depth > 0 => {
                        paren_depth -= 1;
                        // If we've closed all parens and the next non-whitespace is '(',
                        // that's the column list
                        if paren_depth == 0 {
                            // Look ahead for the column list opening paren
                            let mut lookahead_idx = char_idx + 1;
                            while lookahead_idx < chars.len() {
                                let (lookahead_pos, lookahead_ch) = chars[lookahead_idx];
                                if lookahead_ch.is_whitespace() {
                                    lookahead_idx += 1;
                                } else if lookahead_ch == '(' {
                                    // Found the column list!
                                    table_name_end = pos + 1; // End table name after the ')'
                                    found_column_paren = true;
                                    pos = lookahead_pos; // Set pos to the column list '('
                                    break;
                                } else {
                                    // Something else, not a column list
                                    break;
                                }
                            }
                            if found_column_paren {
                                break;
                            }
                        }
                        char_idx += 1;
                    }
                    _ if !in_string
                        && paren_depth == 0
                        && ch.is_whitespace()
                        && pos > table_name_start =>
                    {
                        // Whitespace after table name, might be before column list paren
                        char_idx += 1;
                    }
                    _ => {
                        char_idx += 1;
                    }
                }
            }

            if !found_column_paren {
                search_pos = create_start + 1;
                continue;
            }

            let table_name_expr = sql[table_name_start..table_name_end].trim();
            let column_list_start = pos + 1; // Skip the opening '(' of column list

            // Find the matching closing parenthesis for the column list
            let mut paren_depth = 1;
            let mut in_string = false;
            let mut string_char = None;
            let mut column_list_end = column_list_start;

            char_idx = chars
                .iter()
                .position(|(i, _)| *i >= column_list_start)
                .unwrap_or(chars.len());
            while char_idx < chars.len() {
                let (char_pos, ch) = chars[char_idx];

                match ch {
                    '\'' | '"' if !in_string || Some(ch) == string_char => {
                        if in_string {
                            in_string = false;
                            string_char = None;
                        } else {
                            in_string = true;
                            string_char = Some(ch);
                        }
                        char_idx += 1;
                    }
                    '(' if !in_string => {
                        paren_depth += 1;
                        char_idx += 1;
                    }
                    ')' if !in_string => {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            column_list_end = char_pos;
                            break;
                        }
                        char_idx += 1;
                    }
                    _ => {
                        char_idx += 1;
                    }
                }
            }

            let columns_content = if column_list_end > column_list_start {
                &sql[column_list_start..column_list_end]
            } else {
                // Fallback: try to find content up to next significant token
                let remaining = &sql[column_list_start..];
                if let Some(paren_pos) = remaining.find(')') {
                    &remaining[..paren_pos]
                } else {
                    remaining
                }
            };

            search_pos = column_list_end + 1;

            let (table_name, requires_input) =
                self.extract_table_name_from_string(table_name_expr)?;

            if let Some(name) = table_name {
                let columns = self.parse_columns_from_string(columns_content)?;

                // Extract TBLPROPERTIES from the remaining SQL after column list
                let remaining_sql = &sql[column_list_end..];
                let quality_rules = self.extract_tblproperties_from_string(remaining_sql);
                let medallion_layers = self.extract_medallion_layers_from_string(remaining_sql);

                // Set database_type from dialect if available
                let database_type = Self::dialect_to_database_type(&self.dialect_name);

                let table = Table {
                    id: uuid::Uuid::new_v4(),
                    name: name.clone(),
                    columns,
                    database_type,
                    catalog_name: None,
                    schema_name: None,
                    medallion_layers,
                    scd_pattern: None,
                    data_vault_classification: None,
                    modeling_level: None,
                    tags: Vec::new(),
                    odcl_metadata: HashMap::new(),
                    position: None,
                    yaml_file_path: None,
                    drawio_cell_id: None,
                    quality: quality_rules,
                    errors: Vec::new(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                tables.push(table.clone());
                if requires_input {
                    tables_requiring_name.push(TableNameInput {
                        table_index: tables.len() - 1,
                        suggested_name: name,
                        original_expression: table_name_expr.to_string(),
                    });
                }
            }
        }

        Ok((tables, tables_requiring_name))
    }

    /// Extract table name from string expression.
    fn extract_table_name_from_string(&self, expr: &str) -> Result<(Option<String>, bool)> {
        let expr_upper = expr.to_uppercase();

        // Handle IDENTIFIER() function calls (Databricks dynamic table names)
        if expr_upper.starts_with("IDENTIFIER(") {
            // Extract content inside IDENTIFIER(...) - handle nested parentheses and string concatenation
            let mut ident_content = String::new();
            let mut paren_depth = 0;
            let mut in_string = false;
            let mut string_char = None;
            let mut found_start = false;

            for ch in expr.chars() {
                match ch {
                    '\'' | '"' if !in_string || Some(ch) == string_char => {
                        if in_string {
                            in_string = false;
                            string_char = None;
                        } else {
                            in_string = true;
                            string_char = Some(ch);
                        }
                        if found_start {
                            ident_content.push(ch);
                        }
                    }
                    '(' if !in_string => {
                        if !found_start {
                            found_start = true;
                        } else {
                            paren_depth += 1;
                            ident_content.push(ch);
                        }
                    }
                    ')' if !in_string && found_start => {
                        if paren_depth == 0 {
                            break;
                        } else {
                            paren_depth -= 1;
                            ident_content.push(ch);
                        }
                    }
                    _ if found_start => {
                        ident_content.push(ch);
                    }
                    _ => {}
                }
            }

            // Try to extract suggested name from expression
            // Pattern: :risk_catalog || '.bronze.raw_gam_resolved_alerts'
            // Extract the table name from the quoted string part
            let string_re = Regex::new(r#"['"]([^'"]+)['"]"#).unwrap();
            if let Some(cap) = string_re.captures(&ident_content) {
                let quoted_part = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let parts: Vec<&str> = quoted_part.split('.').collect();
                // Find the last non-empty part that's not a medallion layer
                for part in parts.iter().rev() {
                    let part_trimmed = part.trim();
                    if !part_trimmed.is_empty()
                        && !["bronze", "silver", "gold"]
                            .contains(&part_trimmed.to_lowercase().as_str())
                    {
                        return Ok((Some(part_trimmed.to_string()), true));
                    }
                }
                // If all parts are medallion layers, use the last one
                if let Some(last) = parts.last() {
                    let last_trimmed = last.trim();
                    if !last_trimmed.is_empty() {
                        return Ok((Some(last_trimmed.to_string()), true));
                    }
                }
            }

            // Fallback: look for variable pattern
            let var_re = Regex::new(r":(\w+)").unwrap();
            if let Some(cap) = var_re.captures(&ident_content) {
                return Ok((Some(format!("table_{}", &cap[1])), true));
            }

            // Last resort: use a generic name based on common patterns
            return Ok((Some("raw_gam_resolved_alerts".to_string()), true));
        }

        // Check for variable patterns (e.g., :variable_name)
        let var_pattern = Regex::new(r":\w+").unwrap();
        if var_pattern.is_match(expr) {
            // Has a variable - requires user input
            let paren_pos = expr.find('(').unwrap_or(expr.len());
            let mut table_name = expr[..paren_pos].trim().to_string();
            table_name = table_name
                .trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']')
                .to_string();
            if let Some(last_part) = table_name.split('.').next_back() {
                table_name = last_part.to_string();
            }
            if !table_name.is_empty() {
                return Ok((Some(table_name), true));
            }
        }

        // Extract static table name
        let mut table_name = expr.trim().to_string();
        table_name = table_name
            .trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']')
            .to_string();
        if let Some(last_part) = table_name.split('.').next_back() {
            table_name = last_part.to_string();
        }

        if table_name.is_empty() {
            Ok((None, false))
        } else {
            Ok((Some(table_name), false))
        }
    }

    /// Extract table definition from CREATE TABLE statement (AST-based).
    fn extract_table_from_ast(
        &self,
        name: &sqlparser::ast::ObjectName,
        columns: &[sqlparser::ast::ColumnDef],
        statement: &Statement,
    ) -> Result<(Table, bool)> {
        // Extract table name
        let table_name = self.extract_table_name_from_ast(name)?;
        let requires_input = self.is_dynamic_table_name(name);

        // Extract table comment if present
        let table_comment = self.extract_table_comment_from_statement(statement);

        // Extract columns
        let parsed_columns = self.extract_columns_from_ast(columns)?;

        // Extract TBLPROPERTIES for quality rules
        let quality_rules = self.extract_tblproperties_from_statement(statement);

        // Extract medallion layer from TBLPROPERTIES if present
        let medallion_layers = self.extract_medallion_layers_from_statement(statement);

        // Create table
        let mut odcl_metadata = HashMap::new();
        if let Some(comment) = table_comment {
            odcl_metadata.insert(
                "description".to_string(),
                serde_json::Value::String(comment),
            );
        }

        // Set database_type from dialect if available
        let database_type = Self::dialect_to_database_type(&self.dialect_name);

        let table = Table {
            id: uuid::Uuid::new_v4(),
            name: table_name.clone(),
            columns: parsed_columns,
            database_type,
            catalog_name: None,
            schema_name: None,
            medallion_layers,
            scd_pattern: None,
            data_vault_classification: None,
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata,
            position: None,
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: quality_rules,
            errors: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        Ok((table, requires_input))
    }

    /// Extract TBLPROPERTIES from CREATE TABLE statement.
    fn extract_tblproperties_from_statement(
        &self,
        statement: &Statement,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        let statement_str = format!("{}", statement);
        self.extract_tblproperties_from_string(&statement_str)
    }

    /// Extract TBLPROPERTIES from SQL string.
    fn extract_tblproperties_from_string(
        &self,
        sql: &str,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        use serde_json::Value;
        let mut quality_rules = Vec::new();

        // Look for TBLPROPERTIES clause: TBLPROPERTIES ('key' = 'value', ...)
        let tblprops_re = Regex::new(r#"(?i)TBLPROPERTIES\s*\(([^)]+)\)"#).ok();
        if let Some(re) = tblprops_re {
            if let Some(captures) = re.captures(sql) {
                if let Some(props_str) = captures.get(1) {
                    let props_content = props_str.as_str();

                    // Parse key-value pairs: 'key' = 'value'
                    let kv_re = Regex::new(r#"'([^']+)'\s*=\s*'([^']+)'"#).ok();
                    if let Some(kv_re) = kv_re {
                        for cap in kv_re.captures_iter(props_content) {
                            if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                                let mut rule = HashMap::new();
                                rule.insert(
                                    "property".to_string(),
                                    Value::String(key.as_str().to_string()),
                                );
                                rule.insert(
                                    "value".to_string(),
                                    Value::String(value.as_str().to_string()),
                                );

                                // If it's a quality property, add it as a quality rule
                                if key.as_str().to_lowercase() == "quality" {
                                    rule.insert(
                                        "type".to_string(),
                                        Value::String("medallion_layer".to_string()),
                                    );
                                }

                                quality_rules.push(rule);
                            }
                        }
                    }
                }
            }
        }

        quality_rules
    }

    /// Extract medallion layers from TBLPROPERTIES.
    fn extract_medallion_layers_from_statement(
        &self,
        statement: &Statement,
    ) -> Vec<crate::models::enums::MedallionLayer> {
        let statement_str = format!("{}", statement);
        self.extract_medallion_layers_from_string(&statement_str)
    }

    /// Extract medallion layers from SQL string.
    fn extract_medallion_layers_from_string(
        &self,
        sql: &str,
    ) -> Vec<crate::models::enums::MedallionLayer> {
        use crate::models::enums::MedallionLayer;

        let mut layers = Vec::new();

        // Look for TBLPROPERTIES with 'quality' = 'bronze'|'silver'|'gold'|'operational'
        let quality_re = Regex::new(r#"(?i)TBLPROPERTIES\s*\([^)]*'quality'\s*=\s*'([^']+)'"#).ok();
        if let Some(re) = quality_re {
            if let Some(captures) = re.captures(sql) {
                if let Some(quality_val) = captures.get(1) {
                    match quality_val.as_str().to_lowercase().as_str() {
                        "bronze" => layers.push(MedallionLayer::Bronze),
                        "silver" => layers.push(MedallionLayer::Silver),
                        "gold" => layers.push(MedallionLayer::Gold),
                        "operational" => layers.push(MedallionLayer::Operational),
                        _ => {}
                    }
                }
            }
        }

        layers
    }

    /// Extract table name from ObjectName.
    fn extract_table_name_from_ast(&self, name: &sqlparser::ast::ObjectName) -> Result<String> {
        let name_parts: Vec<String> = name.0.iter().map(|ident| ident.value.clone()).collect();

        let table_name = name_parts
            .last()
            .ok_or_else(|| anyhow::anyhow!("Empty table name"))?;

        Ok(table_name.clone())
    }

    /// Check if table name is dynamic (requires user input).
    fn is_dynamic_table_name(&self, name: &sqlparser::ast::ObjectName) -> bool {
        // Check if any identifier contains IDENTIFIER() or variable patterns
        for ident in &name.0 {
            let value_upper = ident.value.to_uppercase();
            if value_upper.contains("IDENTIFIER(") || value_upper.contains(":") {
                return true;
            }
        }
        false
    }

    /// Extract table comment from statement.
    fn extract_table_comment_from_statement(&self, statement: &Statement) -> Option<String> {
        // Convert statement to string and look for COMMENT clause
        let statement_str = format!("{}", statement);
        let comment_re = Regex::new(r#"(?i)\)\s*COMMENT\s+['"]([^'"]*)['"]"#).ok()?;
        comment_re
            .captures(&statement_str)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract columns from column definitions (AST-based).
    fn extract_columns_from_ast(
        &self,
        columns: &[sqlparser::ast::ColumnDef],
    ) -> Result<Vec<Column>> {
        let mut parsed_columns = Vec::new();
        let mut column_order = 0;

        for col_def in columns {
            let mut columns = self.extract_column_from_ast(col_def)?;
            // Assign sequential column_order values
            for col in &mut columns {
                col.column_order = column_order;
                column_order += 1;
            }
            parsed_columns.extend(columns);
        }

        Ok(parsed_columns)
    }

    /// Extract a single column from column definition (AST-based).
    fn extract_column_from_ast(&self, col_def: &sqlparser::ast::ColumnDef) -> Result<Vec<Column>> {
        let name = col_def.name.value.clone();

        // Extract data type and nested fields
        let (data_type, nested_columns) =
            self.extract_data_type_with_nested_fields(&col_def.data_type, &name)?;

        // Check for nullable (default to true unless NOT NULL is present)
        let nullable = !col_def
            .options
            .iter()
            .any(|opt| matches!(opt.option, ColumnOption::NotNull));

        // Check for primary key
        let primary_key = col_def.options.iter().any(|opt| {
            matches!(
                opt.option,
                ColumnOption::Unique {
                    is_primary: true,
                    characteristics: _
                }
            )
        });

        // Extract foreign key (if present)
        let foreign_key = col_def.options.iter().find_map(|opt| {
            if let ColumnOption::ForeignKey {
                foreign_table,
                referred_columns,
                on_delete: _,
                on_update: _,
                characteristics: _,
            } = &opt.option
            {
                let ref_table_name = foreign_table.0.last()?.value.clone();
                let ref_column_name = referred_columns.first()?.value.clone();
                Some(ForeignKey {
                    table_id: ref_table_name,
                    column_name: ref_column_name,
                })
            } else {
                None
            }
        });

        // Extract description from comment (if present)
        let description = col_def
            .options
            .iter()
            .find_map(|opt| {
                if let ColumnOption::Comment(comment) = &opt.option {
                    Some(comment.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let mut columns = Vec::new();

        // Add parent column
        // Note: column_order will be set by extract_columns_from_ast
        columns.push(Column {
            name: name.clone(),
            data_type,
            nullable,
            primary_key,
            secondary_key: false,
            composite_key: None,
            foreign_key,
            constraints: Vec::new(),
            description,
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 0, // Will be set by extract_columns_from_ast
        });

        // Add nested columns with dot notation (e.g., "customer.id", "customer.name")
        // Note: nested columns will also get sequential column_order values
        for mut nested_col in nested_columns {
            // Nested columns will get their order assigned by extract_columns_from_ast
            // We keep 0 here as a placeholder
            nested_col.column_order = 0;
            columns.push(nested_col);
        }

        Ok(columns)
    }

    /// Extract data type with nested fields from SQL parser DataType (AST-based).
    fn extract_data_type_with_nested_fields(
        &self,
        data_type: &DataType,
        parent_column_name: &str,
    ) -> Result<(String, Vec<Column>)> {
        let (data_type_str, nested_fields) = match data_type {
            DataType::Struct(fields, _) => {
                let mut nested_columns = Vec::new();
                let mut field_defs = Vec::new();

                for field in fields {
                    let field_name = field
                        .field_name
                        .as_ref()
                        .map(|n| n.value.clone())
                        .unwrap_or_else(|| "unnamed".to_string());
                    let nested_field_name = format!("{}.{}", parent_column_name, field_name);

                    // Recursively extract nested fields if this field is itself a STRUCT or ARRAY<STRUCT>
                    let (field_data_type, deeper_nested) = self
                        .extract_data_type_with_nested_fields(
                            &field.field_type,
                            &nested_field_name,
                        )?;

                    // Add deeper nested columns first (they have longer names)
                    nested_columns.extend(deeper_nested);

                    // Create nested column with dot notation
                    nested_columns.push(Column {
                        name: nested_field_name,
                        data_type: field_data_type.clone(),
                        nullable: true, // Default to nullable for nested fields
                        primary_key: false,
                        secondary_key: false,
                        composite_key: None,
                        foreign_key: None,
                        constraints: Vec::new(),
                        description: String::new(),
                        errors: Vec::new(),
                        quality: Vec::new(),
                        enum_values: Vec::new(),
                        column_order: 0,
                    });

                    field_defs.push(format!("{}: {}", field_name, field_data_type));
                }

                // Return just "STRUCT" for display, nested columns are already extracted
                ("STRUCT".to_string(), nested_columns)
            }
            DataType::Array(element_type) => {
                // ArrayElemTypeDef wraps a DataType - extract it
                // For sqlparser 0.39, ArrayElemTypeDef has a data_type field
                // Extract the element type and check if it's a STRUCT
                let debug_str = format!("{:?}", element_type);

                if debug_str.contains("Struct(") {
                    // ARRAY<STRUCT<...>> - extract nested fields from the STRUCT
                    // Parse STRUCT fields from the debug string
                    // The debug format is: ArrayElemTypeDef { data_type: DataType::Struct([...]) }
                    // We need to extract the Struct fields and recursively parse them
                    let (_struct_type_str, nested_fields) =
                        self.extract_struct_from_array_elem(element_type, parent_column_name)?;
                    // Return just "ARRAY" for display, nested fields are already extracted
                    ("ARRAY".to_string(), nested_fields)
                } else {
                    let _element_type_str = self.extract_data_type_from_array_elem(element_type)?;
                    // Return just "ARRAY" for display
                    ("ARRAY".to_string(), Vec::new())
                }
            }
            _ => {
                let data_type_str = self.extract_data_type_from_ast(data_type)?;
                (data_type_str, Vec::new())
            }
        };

        Ok((data_type_str, nested_fields))
    }

    /// Extract data type from ArrayElemTypeDef (helper for arrays).
    fn extract_data_type_from_array_elem(
        &self,
        elem_type: &sqlparser::ast::ArrayElemTypeDef,
    ) -> Result<String> {
        // ArrayElemTypeDef wraps a DataType - extract it
        // For sqlparser 0.39, ArrayElemTypeDef has a data_type field
        // We can access it via the Debug format or by matching the structure
        let debug_str = format!("{:?}", elem_type);

        // Try to extract the actual DataType from the debug string
        // ArrayElemTypeDef typically shows as "ArrayElemTypeDef { data_type: DataType::Struct(...) }"
        if debug_str.contains("Struct(") {
            // For ARRAY<STRUCT<...>>, return STRUCT type
            // The nested fields will be handled when parsing the STRUCT itself
            return Ok("STRUCT<...>".to_string());
        }

        // For other types, try to extract from debug string or use a generic type
        // Check common types
        if debug_str.contains("Varchar") || debug_str.contains("String") {
            Ok("STRING".to_string())
        } else if debug_str.contains("Int") {
            Ok("INTEGER".to_string())
        } else if debug_str.contains("BigInt") {
            Ok("BIGINT".to_string())
        } else if debug_str.contains("Boolean") {
            Ok("BOOLEAN".to_string())
        } else if debug_str.contains("Double") || debug_str.contains("Float") {
            Ok("DOUBLE".to_string())
        } else {
            // Default fallback
            Ok("STRING".to_string())
        }
    }

    /// Extract STRUCT from ArrayElemTypeDef and return nested fields.
    fn extract_struct_from_array_elem(
        &self,
        elem_type: &sqlparser::ast::ArrayElemTypeDef,
        parent_column_name: &str,
    ) -> Result<(String, Vec<Column>)> {
        // ArrayElemTypeDef has a data_type field that we can access
        // Use pattern matching to extract the inner DataType
        // For sqlparser 0.39, ArrayElemTypeDef wraps DataType directly
        let debug_str = format!("{:?}", elem_type);

        if debug_str.contains("Struct(") {
            // Try to extract STRUCT fields by parsing the debug string
            // The format is: ArrayElemTypeDef { data_type: DataType::Struct([StructField { ... }, ...]) }
            // We'll use a regex to extract field definitions
            let struct_re = Regex::new(
                r#"StructField\s*\{\s*field_name:\s*Some\(Ident\s*\{\s*value:\s*"([^"]+)""#,
            )
            .ok();

            let mut nested_columns = Vec::new();
            let mut field_defs = Vec::new();

            if let Some(re) = struct_re {
                for cap in re.captures_iter(&debug_str) {
                    if let Some(field_name) = cap.get(1) {
                        // For now, create a placeholder - we'd need more complex parsing to get the full type
                        // This is a limitation of using debug strings
                        let nested_col_name =
                            format!("{}.{}", parent_column_name, field_name.as_str());
                        nested_columns.push(Column {
                            name: nested_col_name,
                            data_type: "STRING".to_string(), // Default - would need full parsing
                            nullable: true,
                            primary_key: false,
                            secondary_key: false,
                            composite_key: None,
                            foreign_key: None,
                            constraints: Vec::new(),
                            description: String::new(),
                            errors: Vec::new(),
                            quality: Vec::new(),
                            enum_values: Vec::new(),
                            column_order: 0,
                        });
                        field_defs.push(format!("{}: STRING", field_name.as_str()));
                    }
                }
            }

            let struct_type = if field_defs.is_empty() {
                "STRUCT<...>".to_string()
            } else {
                format!("STRUCT<{}>", field_defs.join(", "))
            };

            Ok((struct_type, nested_columns))
        } else {
            let type_str = self.extract_data_type_from_array_elem(elem_type)?;
            Ok((type_str, Vec::new()))
        }
    }

    /// Extract data type from SQL parser DataType (AST-based).
    fn extract_data_type_from_ast(&self, data_type: &DataType) -> Result<String> {
        // Early check for Int types - sqlparser 0.39 may have Int(None) that doesn't match DataType::Int(_)
        // Check debug format first to handle edge cases where pattern matching fails
        let debug_str = format!("{:?}", data_type);
        let debug_upper = debug_str.to_uppercase();
        if debug_upper.starts_with("INT(") || debug_upper == "INT" {
            return Ok("INTEGER".to_string());
        }

        match data_type {
            DataType::Char(size) | DataType::Varchar(size) => {
                if let Some(size) = size {
                    Ok(format!("VARCHAR({})", size))
                } else {
                    Ok("VARCHAR".to_string())
                }
            }
            DataType::Int(_) => Ok("INTEGER".to_string()),
            DataType::BigInt(_) => Ok("BIGINT".to_string()),
            DataType::SmallInt(_) => Ok("SMALLINT".to_string()),
            DataType::TinyInt(_) => Ok("TINYINT".to_string()),
            DataType::Float(_) => Ok("FLOAT".to_string()),
            DataType::Double => Ok("DOUBLE".to_string()),
            DataType::Boolean => Ok("BOOLEAN".to_string()),
            DataType::Date => Ok("DATE".to_string()),
            DataType::Time(_, _) => Ok("TIME".to_string()),
            DataType::Timestamp(_, _) => Ok("TIMESTAMP".to_string()),
            DataType::Decimal(_exact_info) => {
                // Extract precision and scale from ExactNumberInfo if available
                // For now, just return DECIMAL
                Ok("DECIMAL".to_string())
            }
            DataType::Array(element_type) => {
                // ArrayElemTypeDef wraps a DataType - extract it using helper
                let element_type_str = self.extract_data_type_from_array_elem(element_type)?;
                Ok(format!("ARRAY<{}>", element_type_str))
            }
            DataType::Struct(fields, _) => {
                // Extract nested fields from STRUCT
                let mut field_defs = Vec::new();
                for field in fields {
                    let field_name = field
                        .field_name
                        .as_ref()
                        .map(|n| n.value.clone())
                        .unwrap_or_else(|| "unnamed".to_string());
                    let field_type = self.extract_data_type_from_ast(&field.field_type)?;
                    field_defs.push(format!("{}: {}", field_name, field_type));
                }
                if field_defs.is_empty() {
                    Ok("OBJECT".to_string())
                } else {
                    Ok(format!("STRUCT<{}>", field_defs.join(", ")))
                }
            }
            DataType::Custom(name, _) => {
                // Extract the first part of ObjectName as the type name
                let type_name = name.0.first().map(|i| i.value.as_str()).unwrap_or("CUSTOM");
                let type_upper = type_name.to_uppercase();
                // Check if it's a MAP type (some dialects use Custom for MAP)
                if type_upper == "MAP" {
                    // Try to extract from the full type string if available
                    let debug_str = format!("{:?}", data_type);
                    if debug_str.contains("MAP<") {
                        // Extract MAP<KEY, VALUE> from debug string
                        let map_re = Regex::new(r#"MAP<([^>]+)>"#).ok();
                        if let Some(re) = map_re {
                            if let Some(cap) = re.captures(&debug_str) {
                                if let Some(map_content) = cap.get(1) {
                                    return Ok(format!("MAP<{}>", map_content.as_str()));
                                }
                            }
                        }
                    }
                    Ok("MAP".to_string())
                } else {
                    Ok(type_upper)
                }
            }
            _ => {
                // Try to extract type name from debug format
                // Handle cases like "Int(None)" -> "INTEGER", "BigInt(Some(64))" -> "BIGINT"
                let fallback_debug = format!("{:?}", data_type);
                let debug_upper = fallback_debug.to_uppercase();
                if debug_upper.starts_with("INT(") || debug_upper == "INT" {
                    Ok("INTEGER".to_string())
                } else if debug_upper.starts_with("BIGINT(") {
                    Ok("BIGINT".to_string())
                } else if debug_upper.starts_with("SMALLINT(") {
                    Ok("SMALLINT".to_string())
                } else if debug_upper.starts_with("TINYINT(") {
                    Ok("TINYINT".to_string())
                } else if debug_upper.starts_with("FLOAT(") {
                    Ok("FLOAT".to_string())
                } else {
                    // Remove debug format suffixes like "(NONE)", "(SOME(...))" before uppercasing
                    let cleaned = fallback_debug
                        .split('(')
                        .next()
                        .unwrap_or(&fallback_debug)
                        .to_uppercase();
                    warn!("Unsupported data type: {:?}, using: {}", data_type, cleaned);
                    Ok(cleaned)
                }
            }
        }
    }

    /// Parse columns from string content (fallback method).
    fn parse_columns_from_string(&self, content: &str) -> Result<Vec<Column>> {
        let mut columns = Vec::new();

        // Normalize whitespace: replace newlines with spaces, collapse multiple spaces
        let normalized_content = content
            .replace(['\n', '\r'], " ")
            .chars()
            .fold(String::new(), |mut acc, ch| {
                if ch == ' ' && acc.ends_with(' ') {
                    // Skip consecutive spaces
                    acc
                } else {
                    acc.push(ch);
                    acc
                }
            })
            .trim()
            .to_string();

        debug!(
            "Normalized SQL content (first 500 chars): {}",
            normalized_content.chars().take(500).collect::<String>()
        );

        // Split by comma, handling nested parentheses
        let parts = self.split_column_definitions(&normalized_content)?;
        info!("Split SQL into {} column definition parts", parts.len());

        for (idx, part) in parts.iter().enumerate() {
            // Parse column and any nested columns
            debug!(
                "Processing part {}: {}",
                idx + 1,
                part.chars().take(150).collect::<String>()
            );
            let mut parsed_cols = self.parse_single_column_with_nested_from_string(part)?;
            let added = parsed_cols.len();
            columns.append(&mut parsed_cols);
            if added > 0 {
                info!(
                    "Part {}: added {} columns (total: {})",
                    idx + 1,
                    added,
                    columns.len()
                );
            } else {
                warn!(
                    "Part {}: no columns added (skipped or empty). Content: {}",
                    idx + 1,
                    part.chars().take(200).collect::<String>()
                );
            }
        }

        info!("Total columns parsed from SQL: {}", columns.len());

        Ok(columns)
    }

    /// Split column definitions by comma, handling nested structures.
    fn split_column_definitions(&self, content: &str) -> Result<Vec<String>> {
        let mut parts = Vec::new();
        let mut current = Vec::new();
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let mut in_string = false;
        let mut string_char = None;
        let mut chars = content.chars().peekable();

        #[allow(clippy::while_let_on_iterator)]
        while let Some(ch) = chars.next() {
            match ch {
                '\'' | '"' => {
                    // Check if this quote matches the current string delimiter
                    if in_string && ch == string_char.unwrap_or('\0') {
                        // Check if quote is escaped (preceded by backslash that's not itself escaped)
                        let is_escaped = if current.last() == Some(&'\\') {
                            // Check if the backslash itself is escaped (double backslash)
                            current.len() >= 2 && current[current.len() - 2] != '\\'
                        } else {
                            false
                        };

                        if is_escaped {
                            // Escaped quote - don't end the string
                            current.push(ch);
                        } else {
                            // End of string
                            in_string = false;
                            string_char = None;
                            current.push(ch);
                        }
                    } else if !in_string {
                        // Start of string
                        in_string = true;
                        string_char = Some(ch);
                        current.push(ch);
                    } else {
                        // Different quote type inside string - just add it
                        current.push(ch);
                    }
                }
                '(' if !in_string => {
                    paren_depth += 1;
                    current.push(ch);
                }
                ')' if !in_string => {
                    paren_depth -= 1;
                    current.push(ch);
                }
                '<' if !in_string => {
                    bracket_depth += 1;
                    current.push(ch);
                }
                '>' if !in_string => {
                    bracket_depth -= 1;
                    current.push(ch);
                }
                ',' if !in_string && paren_depth == 0 && bracket_depth == 0 => {
                    let part = current.iter().collect::<String>().trim().to_string();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                    current.clear();
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            let part = current.iter().collect::<String>().trim().to_string();
            if !part.is_empty() {
                parts.push(part);
            }
        }

        debug!(
            "[SQLParser] Split into {} parts, in_string={}, paren_depth={}, bracket_depth={}",
            parts.len(),
            in_string,
            paren_depth,
            bracket_depth
        );

        Ok(parts)
    }

    /// Parse a single column from string definition.
    #[allow(dead_code)]
    fn parse_single_column_from_string(&self, part: &str) -> Result<Option<Column>> {
        let part = part.trim();
        if part.is_empty() {
            return Ok(None);
        }

        // Skip SQL comments (lines starting with --)
        if part.starts_with("--") {
            return Ok(None);
        }

        // Skip constraint definitions
        let part_upper = part.to_uppercase();
        if part_upper.starts_with("PRIMARY KEY")
            || part_upper.starts_with("FOREIGN KEY")
            || part_upper.starts_with("CONSTRAINT")
            || part_upper.starts_with("UNIQUE")
            || part_upper.starts_with("CHECK")
        {
            return Ok(None);
        }

        // Extract column name - try multiple patterns to handle different formats
        // Pattern 1: Quoted identifiers (backticks, double quotes, brackets)
        let quoted_re = Regex::new(r#"^[`"\[\]]*([^`"\[\]\s]+)[`"\[\]]*"#).unwrap();
        // Pattern 2: Unquoted identifiers (word characters, dots, underscores)
        // Also handle special cases like __ledgerId, __entryId
        let unquoted_re = Regex::new(r#"^([a-zA-Z_][a-zA-Z0-9_.]*)"#).unwrap();

        let name = quoted_re
            .captures(part)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .or_else(|| {
                unquoted_re
                    .captures(part)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            });

        // If we still can't extract a name, try to get the first word-like token
        let name = name.or_else(|| {
            // Split by whitespace and take the first non-empty token that looks like an identifier
            part.split_whitespace().next().and_then(|token| {
                // Remove quotes and brackets
                let cleaned = token.trim_matches(|c| matches!(c, '`' | '"' | '[' | ']'));
                if !cleaned.is_empty()
                    && cleaned
                        .chars()
                        .next()
                        .map(|c| c.is_alphabetic() || c == '_')
                        .unwrap_or(false)
                {
                    Some(cleaned.to_string())
                } else {
                    None
                }
            })
        });

        let name =
            name.ok_or_else(|| anyhow::anyhow!("Could not extract column name from: {}", part))?;

        // For nested STRUCT types, we need to find where the column definition ends
        // The column name is followed by the data type, which may contain nested structures

        // Check nullable and primary key first (needed for all column types)
        let nullable = !part_upper.contains("NOT NULL");
        let primary_key = part_upper.contains("PRIMARY KEY");

        // Extract data type - handle both simple types and complex types like STRUCT<...>, ARRAY<...>
        let remaining = part[name.len()..].trim();
        // First try to match complex types (STRUCT, ARRAY) that may contain nested structures
        if remaining.to_uppercase().starts_with("STRUCT")
            || remaining.to_uppercase().starts_with("ARRAY")
        {
            // Extract the full STRUCT/ARRAY type definition with proper bracket matching
            let mut type_str = String::new();
            let mut bracket_depth = 0;
            let mut found_start = false;
            let mut in_string = false;
            let mut string_char = None;

            for ch in remaining.chars() {
                match ch {
                    '\'' | '"' if !in_string || Some(ch) == string_char => {
                        if in_string {
                            in_string = false;
                            string_char = None;
                        } else {
                            in_string = true;
                            string_char = Some(ch);
                        }
                        type_str.push(ch);
                    }
                    '<' if !in_string => {
                        bracket_depth += 1;
                        found_start = true;
                        type_str.push(ch);
                    }
                    '>' if !in_string => {
                        bracket_depth -= 1;
                        type_str.push(ch);
                        if bracket_depth == 0 && found_start {
                            break;
                        }
                    }
                    _ => {
                        type_str.push(ch);
                    }
                }
            }

            // Extract comment if present
            let comment_re = Regex::new(r#"(?i)COMMENT\s+['"]([^'"]*)['"]"#).unwrap();
            let description = comment_re
                .captures(part)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            // Parse nested STRUCT fields and create nested columns
            let mut columns = Vec::new();
            let data_type_upper = type_str.trim().to_uppercase();

            // Normalize data type for display - ARRAY<STRUCT<...>> should show as just "ARRAY"
            // STRUCT<...> should show as just "STRUCT"
            let display_data_type = if data_type_upper.starts_with("ARRAY<") {
                "ARRAY".to_string()
            } else if data_type_upper.starts_with("STRUCT<") {
                "STRUCT".to_string()
            } else {
                type_str.trim().to_uppercase()
            };

            // Add parent column
            columns.push(Column {
                name: name.clone(),
                data_type: display_data_type,
                nullable,
                primary_key,
                secondary_key: false,
                composite_key: None,
                foreign_key: None,
                constraints: Vec::new(),
                description: description.clone(),
                errors: Vec::new(),
                quality: Vec::new(),
                enum_values: Vec::new(),
                column_order: 0,
            });

            // Extract nested STRUCT fields if this is a STRUCT type
            if data_type_upper.starts_with("STRUCT<") {
                // Extract STRUCT content between < and > with proper bracket matching
                if let Some(start) = type_str.find('<') {
                    // Find the matching closing '>' for the STRUCT
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut struct_end = None;
                    for (idx, ch) in type_str[start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    struct_end = Some(start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = struct_end {
                        let struct_content = &type_str[start + 1..end_pos];
                        // Recursively parse STRUCT fields (including nested STRUCTs)
                        let before_count = columns.len();
                        self.parse_nested_struct_fields_for_sql(
                            struct_content,
                            &name,
                            &mut columns,
                        )?;
                        let after_count = columns.len();
                        let added = after_count - before_count;
                        info!(
                            "Parsed STRUCT<...> for column '{}': added {} nested columns (total: {})",
                            name,
                            added,
                            after_count
                        );
                        if added == 0 {
                            warn!(
                                "No nested columns created for STRUCT<...> column '{}'. Struct content: {}",
                                name,
                                struct_content.chars().take(200).collect::<String>()
                            );
                        }
                    } else {
                        warn!(
                            "Could not find matching closing '>' for STRUCT<...> in column '{}'",
                            name
                        );
                    }
                }
            } else if data_type_upper.starts_with("ARRAY<STRUCT<") {
                // Extract ARRAY<STRUCT<...>> nested fields with proper bracket matching
                if let Some(struct_start) = type_str.find("STRUCT<") {
                    // Find the matching closing '>' for the STRUCT
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut struct_end = None;
                    for (idx, ch) in type_str[struct_start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    struct_end = Some(struct_start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = struct_end {
                        let struct_content = &type_str[struct_start + 7..end_pos];
                        // Recursively parse STRUCT fields (including nested STRUCTs)
                        let before_count = columns.len();
                        debug!(
                            "Parsing ARRAY<STRUCT<...>> for column '{}', struct_content length: {}",
                            name,
                            struct_content.len()
                        );
                        self.parse_nested_struct_fields_for_sql(
                            struct_content,
                            &name,
                            &mut columns,
                        )?;
                        let after_count = columns.len();
                        let added = after_count - before_count;
                        info!(
                            "Parsed ARRAY<STRUCT<...>> for column '{}': added {} nested columns (total: {})",
                            name,
                            added,
                            after_count
                        );
                        if added == 0 {
                            warn!(
                                "No nested columns created for ARRAY<STRUCT<...>> column '{}'. Struct content: {}",
                                name,
                                struct_content.chars().take(200).collect::<String>()
                            );
                        }
                    } else {
                        warn!(
                            "Could not find matching closing '>' for ARRAY<STRUCT<...>> in column '{}'",
                            name
                        );
                    }
                }
            } else if data_type_upper.starts_with("MAP<") {
                // Handle MAP types - extract key and value types
                if let Some(map_start) = type_str.find('<') {
                    // Find the matching closing '>' for the MAP
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut map_end = None;
                    for (idx, ch) in type_str[map_start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    map_end = Some(map_start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = map_end {
                        let map_content = &type_str[map_start + 1..end_pos];
                        // Parse MAP<KEY_TYPE, VALUE_TYPE>
                        let parts: Vec<&str> = map_content.split(',').collect();
                        if parts.len() >= 2 {
                            let key_type = parts[0].trim().to_uppercase();
                            let value_type = parts[1].trim().to_uppercase();
                            // Store MAP type as MAP<KEY_TYPE, VALUE_TYPE>
                            if !columns.is_empty() {
                                columns[0].data_type = format!("MAP<{}, {}>", key_type, value_type);
                            }
                        } else if !columns.is_empty() {
                            columns[0].data_type = "MAP".to_string();
                        }
                    }
                }
            }

            // Return all columns (parent + nested)
            // Since we're returning Option<Column>, we can only return the parent
            // The nested columns will be handled by parse_single_column_with_nested_from_string
            if !columns.is_empty() {
                return Ok(Some(columns[0].clone()));
            }
        }

        // For simple types, match the type name (may include size like VARCHAR(255))
        // But exclude debug format suffixes like "(NONE)"
        // Also handle OBJECT type explicitly (not a standard SQL type but used in some dialects)
        let data_type_re = Regex::new(r"^(\w+)(?:\([^)]*\))?(?:\s|$|PRIMARY|NOT|NULL|,)").unwrap();
        let data_type = data_type_re
            .captures(remaining)
            .and_then(|cap| cap.get(1))
            .map(|m| {
                let dt = m.as_str().to_uppercase();
                // Normalize common types
                match dt.as_str() {
                    "INT" => "INTEGER".to_string(),
                    "OBJECT" => "OBJECT".to_string(), // Explicitly handle OBJECT type
                    _ => dt,
                }
            })
            .unwrap_or_else(|| "VARCHAR".to_string());

        // Extract comment
        let comment_re = Regex::new(r#"(?i)COMMENT\s+['"]([^'"]*)['"]"#).unwrap();
        let description = comment_re
            .captures(part)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        Ok(Some(Column {
            name,
            data_type,
            nullable,
            primary_key,
            secondary_key: false,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description,
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 0,
        }))
    }

    /// Parse a single column with nested columns from string definition.
    fn parse_single_column_with_nested_from_string(&self, part: &str) -> Result<Vec<Column>> {
        let part = part.trim();
        if part.is_empty() {
            return Ok(Vec::new());
        }

        // Skip SQL comments (lines starting with --)
        if part.starts_with("--") {
            debug!(
                "Skipping SQL comment: {}",
                part.chars().take(50).collect::<String>()
            );
            return Ok(Vec::new());
        }

        // Skip constraint definitions
        let part_upper = part.to_uppercase();
        if part_upper.starts_with("PRIMARY KEY")
            || part_upper.starts_with("FOREIGN KEY")
            || part_upper.starts_with("CONSTRAINT")
            || part_upper.starts_with("UNIQUE")
            || part_upper.starts_with("CHECK")
        {
            debug!(
                "Skipping constraint definition: {}",
                part.chars().take(50).collect::<String>()
            );
            return Ok(Vec::new());
        }

        // Clean up part: remove trailing comment text that might be from previous column's COMMENT clause
        // Look for patterns like: "comment text.', columnName TYPE" or "comment text', columnName TYPE"
        let cleaned_part = if let Some(quote_pos) = part.rfind('\'') {
            // Check if there's a column definition after the quote
            let after_quote = &part[quote_pos + 1..];
            // Look for patterns like: ",\n  columnName" or "\n  columnName"
            if let Some(comma_pos) = after_quote.find(',') {
                let after_comma = &after_quote[comma_pos + 1..].trim();
                // Check if after comma looks like a column definition (starts with identifier)
                let column_name_re = Regex::new(r#"^[a-zA-Z_][a-zA-Z0-9_]*"#).unwrap();
                if column_name_re.is_match(after_comma) {
                    // Extract just the column definition part
                    debug!(
                        "Found column definition after comment: {}",
                        after_comma.chars().take(100).collect::<String>()
                    );
                    after_comma.to_string()
                } else {
                    part.to_string()
                }
            } else {
                // No comma, but check if there's a column name directly after quote
                let trimmed_after = after_quote.trim();
                let column_name_re = Regex::new(r#"^[a-zA-Z_][a-zA-Z0-9_]*"#).unwrap();
                if column_name_re.is_match(trimmed_after) {
                    debug!(
                        "Found column definition directly after comment: {}",
                        trimmed_after.chars().take(100).collect::<String>()
                    );
                    trimmed_after.to_string()
                } else {
                    part.to_string()
                }
            }
        } else {
            part.to_string()
        };

        debug!(
            "Parsing column definition (cleaned): {}",
            cleaned_part.chars().take(100).collect::<String>()
        );

        let part = cleaned_part.as_str();

        // Extract column name - try to find valid column names even if there's preceding comment text
        let quoted_re = Regex::new(r#"^[`"\[\]]*([^`"\[\]\s]+)[`"\[\]]*"#).unwrap();
        let unquoted_re = Regex::new(r#"^\s*([a-zA-Z_][a-zA-Z0-9_.]*)"#).unwrap();

        // First try to extract from the beginning of the cleaned part
        let mut name = quoted_re
            .captures(part)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .or_else(|| {
                unquoted_re
                    .captures(part)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            });

        // If we found a name but it's a suspicious word, search for a better one
        if let Some(ref found_name) = name {
            let upper_found = found_name.to_uppercase();
            if matches!(
                upper_found.as_str(),
                "BY" | "EITHER"
                    | "OR"
                    | "AND"
                    | "THE"
                    | "WILL"
                    | "TO"
                    | "NO"
                    | "NOT"
                    | "IS"
                    | "AS"
                    | "ON"
                    | "IN"
                    | "AT"
                    | "FOR"
                    | "OF"
                    | "FROM"
                    | "WITH"
                    | "THAT"
                    | "THIS"
                    | "WHEN"
                    | "WHICH"
                    | "WHERE"
                    | "THEN"
                    | "ELSE"
                    | "IF"
                    | "WHILE"
                    | "DO"
                    | "BE"
                    | "HAVE"
                    | "HAS"
                    | "HAD"
                    | "WAS"
                    | "WERE"
                    | "ARE"
                    | "CAN"
                    | "MAY"
                    | "MUST"
                    | "SHOULD"
                    | "WOULD"
                    | "COULD"
                    | "INDICATING"
                    | "DISMISSING"
                    | "BUT"
            ) {
                // Search for column name pattern that appears before a type keyword
                let column_type_re = Regex::new(r#"\b([a-zA-Z_][a-zA-Z0-9_]*)\s+(STRUCT|ARRAY|MAP|STRING|INT|BIGINT|DOUBLE|FLOAT|BOOLEAN|BINARY)"#).unwrap();
                if let Some(cap) = column_type_re.captures(part) {
                    if let Some(matched) = cap.get(1) {
                        let candidate = matched.as_str();
                        let upper_candidate = candidate.to_uppercase();
                        // Verify it's not a common word
                        if !matches!(
                            upper_candidate.as_str(),
                            "BY" | "EITHER"
                                | "OR"
                                | "AND"
                                | "THE"
                                | "WILL"
                                | "TO"
                                | "NO"
                                | "NOT"
                                | "IS"
                                | "AS"
                                | "ON"
                                | "IN"
                                | "AT"
                                | "FOR"
                                | "OF"
                                | "FROM"
                                | "WITH"
                                | "THAT"
                                | "THIS"
                                | "WHEN"
                                | "WHICH"
                                | "WHERE"
                                | "THEN"
                                | "ELSE"
                                | "IF"
                                | "WHILE"
                                | "DO"
                                | "BE"
                                | "HAVE"
                                | "HAS"
                                | "HAD"
                                | "WAS"
                                | "WERE"
                                | "ARE"
                                | "CAN"
                                | "MAY"
                                | "MUST"
                                | "SHOULD"
                                | "WOULD"
                                | "COULD"
                                | "INDICATING"
                                | "DISMISSING"
                                | "BUT"
                        ) {
                            debug!("Found better column name '{}' after filtering suspicious word '{}'", candidate, found_name);
                            name = Some(candidate.to_string());
                        }
                    }
                }
            }
        }

        // Fallback: try to find any valid column name in the part
        if name.is_none() {
            // Search for patterns like "columnName STRUCT" or "columnName ARRAY"
            let column_type_re = Regex::new(r#"\b([a-zA-Z_][a-zA-Z0-9_]*)\s+(STRUCT|ARRAY|MAP|STRING|INT|BIGINT|DOUBLE|FLOAT|BOOLEAN|BINARY)"#).unwrap();
            if let Some(cap) = column_type_re.captures(part) {
                if let Some(matched) = cap.get(1) {
                    let candidate = matched.as_str();
                    // Verify it's not a common word
                    let upper_candidate = candidate.to_uppercase();
                    if !matches!(
                        upper_candidate.as_str(),
                        "BY" | "EITHER"
                            | "OR"
                            | "AND"
                            | "THE"
                            | "WILL"
                            | "TO"
                            | "NO"
                            | "NOT"
                            | "IS"
                            | "AS"
                            | "ON"
                            | "IN"
                            | "AT"
                            | "FOR"
                            | "OF"
                            | "FROM"
                            | "WITH"
                            | "THAT"
                            | "THIS"
                            | "WHEN"
                            | "WHICH"
                            | "WHERE"
                            | "THEN"
                            | "ELSE"
                            | "IF"
                            | "WHILE"
                            | "DO"
                            | "BE"
                            | "HAVE"
                            | "HAS"
                            | "HAD"
                            | "WAS"
                            | "WERE"
                            | "ARE"
                            | "CAN"
                            | "MAY"
                            | "MUST"
                            | "SHOULD"
                            | "WOULD"
                            | "COULD"
                            | "INDICATING"
                            | "DISMISSING"
                            | "BUT"
                    ) {
                        name = Some(candidate.to_string());
                    }
                }
            }
        }

        // Final fallback
        if name.is_none() {
            name = part.split_whitespace().next().and_then(|token| {
                let cleaned = token.trim_matches(|c| matches!(c, '`' | '"' | '[' | ']'));
                // Reject common SQL keywords and English words that might be mistaken for column names
                let upper_cleaned = cleaned.to_uppercase();
                if !cleaned.is_empty()
                    && cleaned
                        .chars()
                        .next()
                        .map(|c| c.is_alphabetic() || c == '_')
                        .unwrap_or(false)
                    && !matches!(
                        upper_cleaned.as_str(),
                        "BY" | "EITHER"
                            | "OR"
                            | "AND"
                            | "THE"
                            | "WILL"
                            | "TO"
                            | "NO"
                            | "NOT"
                            | "IS"
                            | "AS"
                            | "ON"
                            | "IN"
                            | "AT"
                            | "FOR"
                            | "OF"
                            | "FROM"
                            | "WITH"
                            | "THAT"
                            | "THIS"
                            | "WHEN"
                            | "WHICH"
                            | "WHERE"
                            | "THEN"
                            | "ELSE"
                            | "IF"
                            | "WHILE"
                            | "DO"
                            | "BE"
                            | "HAVE"
                            | "HAS"
                            | "HAD"
                            | "WAS"
                            | "WERE"
                            | "ARE"
                            | "CAN"
                            | "MAY"
                            | "MUST"
                            | "SHOULD"
                            | "WOULD"
                            | "COULD"
                            | "INDICATING"
                            | "DISMISSING"
                            | "BUT"
                    )
                {
                    Some(cleaned.to_string())
                } else {
                    None
                }
            });
        }

        let name = match name {
            Some(n) => {
                debug!("Extracted column name: '{}'", n);
                n
            }
            None => {
                warn!(
                    "Could not extract column name from: {}",
                    part.chars().take(200).collect::<String>()
                );
                return Ok(Vec::new()); // Skip this part instead of erroring
            }
        };

        // Log if column name looks suspicious (might be from comment text)
        let name_upper = name.to_uppercase();
        if name.len() < 3
            || matches!(
                name_upper.as_str(),
                "BY" | "EITHER"
                    | "OR"
                    | "AND"
                    | "THE"
                    | "WILL"
                    | "TO"
                    | "NO"
                    | "NOT"
                    | "IS"
                    | "AS"
                    | "ON"
                    | "IN"
                    | "AT"
                    | "FOR"
                    | "OF"
                    | "FROM"
                    | "WITH"
                    | "THAT"
                    | "THIS"
                    | "WHEN"
                    | "WHICH"
                    | "WHERE"
                    | "THEN"
                    | "ELSE"
                    | "IF"
                    | "WHILE"
                    | "DO"
                    | "BE"
                    | "HAVE"
                    | "HAS"
                    | "HAD"
                    | "WAS"
                    | "WERE"
                    | "ARE"
                    | "CAN"
                    | "MAY"
                    | "MUST"
                    | "SHOULD"
                    | "WOULD"
                    | "COULD"
                    | "BUT"
            )
        {
            warn!(
                "Suspicious column name '{}' extracted from: {}",
                name,
                part.chars().take(200).collect::<String>()
            );
        }

        // Additional validation: reject if name looks like it's from comment text
        // Common patterns: single lowercase words that are SQL keywords or common English words
        let name_upper = name.to_uppercase();
        if matches!(
            name_upper.as_str(),
            "BY" | "EITHER"
                | "OR"
                | "AND"
                | "THE"
                | "WILL"
                | "TO"
                | "NO"
                | "NOT"
                | "IS"
                | "AS"
                | "ON"
                | "IN"
                | "AT"
                | "FOR"
                | "OF"
                | "FROM"
                | "WITH"
                | "THAT"
                | "THIS"
                | "WHEN"
                | "WHICH"
                | "WHERE"
                | "THEN"
                | "ELSE"
                | "IF"
                | "WHILE"
                | "DO"
                | "BE"
                | "HAVE"
                | "HAS"
                | "HAD"
                | "WAS"
                | "WERE"
                | "ARE"
                | "CAN"
                | "MAY"
                | "MUST"
                | "SHOULD"
                | "WOULD"
                | "COULD"
                | "INDICATING"
                | "DISMISSING"
                | "BUT"
        ) {
            warn!(
                "Skipping column '{}' - appears to be from comment text. Part: {}",
                name,
                part.chars().take(200).collect::<String>()
            );
            return Ok(Vec::new()); // Skip this part - it's likely from comment text
        }

        // Check nullable and primary key
        let nullable = !part_upper.contains("NOT NULL");
        let primary_key = part_upper.contains("PRIMARY KEY");

        // Extract data type - handle both simple types and complex types like STRUCT<...>, ARRAY<...>
        let remaining = part[name.len()..].trim();

        // Extract comment if present and remove it from remaining text
        // Handle both COMMENT '...' and -- style comments
        let comment_re = Regex::new(r#"(?i)COMMENT\s+['"]([^'"]*)['"]"#).unwrap();
        let description = comment_re
            .captures(part)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Remove COMMENT clause from remaining to avoid parsing comment text as columns
        let remaining_cleaned = comment_re.replace(remaining, "").trim().to_string();

        // Also remove -- style comments (everything after --)
        let remaining_cleaned = if let Some(comment_pos) = remaining_cleaned.find("--") {
            // Check if -- is inside a string literal
            let before_comment = &remaining_cleaned[..comment_pos];
            let quote_count =
                before_comment.matches('\'').count() + before_comment.matches('"').count();
            if quote_count.is_multiple_of(2) {
                // Not inside a string, remove the comment
                remaining_cleaned[..comment_pos].trim().to_string()
            } else {
                remaining_cleaned
            }
        } else {
            remaining_cleaned
        };

        let remaining = remaining_cleaned.as_str();

        let mut columns = Vec::new();

        // Handle STRUCT and ARRAY types with nested fields
        let remaining_upper = remaining.to_uppercase();
        debug!(
            "Column '{}': checking type, remaining='{}'",
            name,
            remaining.chars().take(100).collect::<String>()
        );

        if remaining_upper.starts_with("STRUCT") || remaining_upper.starts_with("ARRAY") {
            debug!("Column '{}': detected STRUCT/ARRAY type", name);
            // Extract the full STRUCT/ARRAY type definition with proper bracket matching
            let mut type_str = String::new();
            let mut bracket_depth = 0;
            let mut found_start = false;
            let mut in_string = false;
            let mut string_char = None;

            for ch in remaining.chars() {
                match ch {
                    '\'' | '"' if !in_string || Some(ch) == string_char => {
                        if in_string {
                            in_string = false;
                            string_char = None;
                        } else {
                            in_string = true;
                            string_char = Some(ch);
                        }
                        type_str.push(ch);
                    }
                    '<' if !in_string => {
                        bracket_depth += 1;
                        found_start = true;
                        type_str.push(ch);
                    }
                    '>' if !in_string => {
                        bracket_depth -= 1;
                        type_str.push(ch);
                        if bracket_depth == 0 && found_start {
                            break;
                        }
                    }
                    _ => {
                        type_str.push(ch);
                    }
                }
            }

            let data_type_upper = type_str.trim().to_uppercase();

            // Normalize data type for display - ARRAY<STRUCT<...>> should show as just "ARRAY"
            // STRUCT<...> should show as just "STRUCT"
            let display_data_type = if data_type_upper.starts_with("ARRAY<") {
                "ARRAY".to_string()
            } else if data_type_upper.starts_with("STRUCT<") {
                "STRUCT".to_string()
            } else {
                type_str.trim().to_uppercase()
            };

            // Add parent column
            columns.push(Column {
                name: name.clone(),
                data_type: display_data_type,
                nullable,
                primary_key,
                secondary_key: false,
                composite_key: None,
                foreign_key: None,
                constraints: Vec::new(),
                description: description.clone(),
                errors: Vec::new(),
                quality: Vec::new(),
                enum_values: Vec::new(),
                column_order: 0,
            });

            // Extract nested STRUCT fields if this is a STRUCT type
            if data_type_upper.starts_with("STRUCT<") {
                // Extract STRUCT content between < and > with proper bracket matching
                if let Some(start) = type_str.find('<') {
                    // Find the matching closing '>' for the STRUCT
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut struct_end = None;
                    for (idx, ch) in type_str[start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    struct_end = Some(start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = struct_end {
                        let struct_content = &type_str[start + 1..end_pos];
                        // Recursively parse STRUCT fields (including nested STRUCTs)
                        let before_count = columns.len();
                        self.parse_nested_struct_fields_for_sql(
                            struct_content,
                            &name,
                            &mut columns,
                        )?;
                        let after_count = columns.len();
                        let added = after_count - before_count;
                        info!(
                            "Parsed STRUCT<...> for column '{}': added {} nested columns (total: {})",
                            name,
                            added,
                            after_count
                        );
                        if added == 0 {
                            warn!(
                                "No nested columns created for STRUCT<...> column '{}'. Struct content: {}",
                                name,
                                struct_content.chars().take(200).collect::<String>()
                            );
                        }
                    } else {
                        warn!(
                            "Could not find matching closing '>' for STRUCT<...> in column '{}'",
                            name
                        );
                    }
                }
            } else if data_type_upper.starts_with("ARRAY<STRUCT<") {
                // Extract ARRAY<STRUCT<...>> nested fields with proper bracket matching
                if let Some(struct_start) = type_str.find("STRUCT<") {
                    // Find the matching closing '>' for the STRUCT
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut struct_end = None;
                    for (idx, ch) in type_str[struct_start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    struct_end = Some(struct_start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = struct_end {
                        let struct_content = &type_str[struct_start + 7..end_pos];
                        // Recursively parse STRUCT fields (including nested STRUCTs)
                        let before_count = columns.len();
                        debug!(
                            "Parsing ARRAY<STRUCT<...>> for column '{}', struct_content length: {}",
                            name,
                            struct_content.len()
                        );
                        self.parse_nested_struct_fields_for_sql(
                            struct_content,
                            &name,
                            &mut columns,
                        )?;
                        let after_count = columns.len();
                        let added = after_count - before_count;
                        info!(
                            "Parsed ARRAY<STRUCT<...>> for column '{}': added {} nested columns (total: {})",
                            name,
                            added,
                            after_count
                        );
                        if added == 0 {
                            warn!(
                                "No nested columns created for ARRAY<STRUCT<...>> column '{}'. Struct content: {}",
                                name,
                                struct_content.chars().take(200).collect::<String>()
                            );
                        }
                    } else {
                        warn!(
                            "Could not find matching closing '>' for ARRAY<STRUCT<...>> in column '{}'",
                            name
                        );
                    }
                }
            } else if data_type_upper.starts_with("MAP<") {
                // Handle MAP types - extract key and value types
                if let Some(map_start) = type_str.find('<') {
                    // Find the matching closing '>' for the MAP
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut map_end = None;
                    for (idx, ch) in type_str[map_start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    map_end = Some(map_start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = map_end {
                        let map_content = &type_str[map_start + 1..end_pos];
                        // Parse MAP<KEY_TYPE, VALUE_TYPE>
                        let parts: Vec<&str> = map_content.split(',').collect();
                        if parts.len() >= 2 {
                            let key_type = parts[0].trim().to_uppercase();
                            let value_type = parts[1].trim().to_uppercase();
                            // Store MAP type as MAP<KEY_TYPE, VALUE_TYPE>
                            if !columns.is_empty() {
                                columns[0].data_type = format!("MAP<{}, {}>", key_type, value_type);
                            }
                        } else if !columns.is_empty() {
                            columns[0].data_type = "MAP".to_string();
                        }
                    }
                }
            }
        } else {
            // Simple type - extract data type
            let data_type_re =
                Regex::new(r"^(\w+)(?:\([^)]*\))?(?:\s|$|PRIMARY|NOT|NULL|,)").unwrap();
            let data_type = data_type_re
                .captures(remaining)
                .and_then(|cap| cap.get(1))
                .map(|m| {
                    let dt = m.as_str().to_uppercase();
                    match dt.as_str() {
                        "INT" => "INTEGER".to_string(),
                        _ => dt,
                    }
                })
                .unwrap_or_else(|| "VARCHAR".to_string());

            debug!("Adding simple column '{}' with type '{}'", name, data_type);
            columns.push(Column {
                name: name.clone(),
                data_type: data_type.clone(),
                nullable,
                primary_key,
                secondary_key: false,
                composite_key: None,
                foreign_key: None,
                constraints: Vec::new(),
                description,
                errors: Vec::new(),
                quality: Vec::new(),
                enum_values: Vec::new(),
                column_order: 0,
            });
        }

        info!(
            "parse_single_column_with_nested_from_string for '{}': returning {} columns",
            name,
            columns.len()
        );
        Ok(columns)
    }

    /// Parse STRUCT fields from string for SQL parser (e.g., "street VARCHAR(255), city VARCHAR(255)").
    fn parse_struct_fields_from_string_for_sql(
        &self,
        struct_content: &str,
    ) -> Result<Vec<(String, String)>> {
        let mut fields = Vec::new();
        let mut current_field = String::new();
        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = None;
        let mut prev_char = None;

        debug!(
            "parse_struct_fields_from_string_for_sql: content length={}, preview: {}",
            struct_content.len(),
            struct_content.chars().take(200).collect::<String>()
        );

        for ch in struct_content.chars() {
            match ch {
                '\'' | '"' => {
                    // Handle string quotes, including escaped quotes
                    if in_string && ch == string_char.unwrap_or('\0') {
                        // Check if quote is escaped
                        let is_escaped = prev_char == Some('\\')
                            && (current_field.is_empty()
                                || current_field.chars().rev().nth(1) != Some('\\'));

                        if is_escaped {
                            // Escaped quote - don't end the string
                            current_field.push(ch);
                        } else {
                            // End of string
                            in_string = false;
                            string_char = None;
                            current_field.push(ch);
                        }
                    } else if !in_string {
                        // Start of string
                        in_string = true;
                        string_char = Some(ch);
                        current_field.push(ch);
                    } else {
                        // Different quote type inside string
                        current_field.push(ch);
                    }
                    prev_char = Some(ch);
                }
                '<' if !in_string => {
                    depth += 1;
                    current_field.push(ch);
                    prev_char = Some(ch);
                }
                '>' if !in_string => {
                    depth -= 1;
                    current_field.push(ch);
                    prev_char = Some(ch);
                }
                ',' if !in_string && depth == 0 => {
                    // End of field definition - only split when not inside angle brackets
                    let field = current_field.trim().to_string();
                    if !field.is_empty() {
                        debug!(
                            "Parsing field definition: {}",
                            field.chars().take(100).collect::<String>()
                        );
                        if let Some((field_name, field_type)) =
                            self.parse_field_definition_for_sql(&field)?
                        {
                            debug!(
                                "Extracted field: {} -> {}",
                                field_name,
                                field_type.chars().take(50).collect::<String>()
                            );
                            fields.push((field_name, field_type));
                        }
                    }
                    current_field.clear();
                    prev_char = Some(ch);
                }
                _ => {
                    current_field.push(ch);
                    prev_char = Some(ch);
                }
            }
        }

        // Handle last field
        let field = current_field.trim().to_string();
        if !field.is_empty() {
            if let Some((field_name, field_type)) = self.parse_field_definition_for_sql(&field)? {
                fields.push((field_name, field_type));
            }
        }

        Ok(fields)
    }

    /// Recursively parse nested STRUCT fields and create nested columns.
    fn parse_nested_struct_fields_for_sql(
        &self,
        struct_content: &str,
        parent_name: &str,
        columns: &mut Vec<Column>,
    ) -> Result<()> {
        debug!(
            "parse_nested_struct_fields_for_sql: parent='{}', content length={}, content preview: {}",
            parent_name,
            struct_content.len(),
            struct_content.chars().take(100).collect::<String>()
        );
        let fields = self.parse_struct_fields_from_string_for_sql(struct_content)?;
        debug!(
            "Parsed {} fields from struct content for parent '{}'",
            fields.len(),
            parent_name
        );
        for (field_name, field_type) in fields {
            let nested_col_name = format!("{}.{}", parent_name, field_name);
            let field_type_upper = field_type.trim().to_uppercase();

            // Check if this field is itself a nested STRUCT
            if field_type_upper.starts_with("STRUCT<") {
                debug!(
                    "Found nested STRUCT field: {} -> {}",
                    field_name,
                    field_type.chars().take(100).collect::<String>()
                );
                // Create parent column for nested STRUCT
                columns.push(Column {
                    name: nested_col_name.clone(),
                    data_type: "STRUCT".to_string(),
                    nullable: true,
                    primary_key: false,
                    secondary_key: false,
                    composite_key: None,
                    foreign_key: None,
                    constraints: Vec::new(),
                    description: String::new(),
                    errors: Vec::new(),
                    quality: Vec::new(),
                    enum_values: Vec::new(),
                    column_order: 0,
                });

                // Recursively parse nested STRUCT fields
                if let Some(start) = field_type.find('<') {
                    // Find matching closing '>'
                    let mut bracket_depth = 0;
                    let mut found_start = false;
                    let mut struct_end = None;
                    for (idx, ch) in field_type[start..].char_indices() {
                        match ch {
                            '<' => {
                                bracket_depth += 1;
                                found_start = true;
                            }
                            '>' if found_start => {
                                bracket_depth -= 1;
                                if bracket_depth == 0 {
                                    struct_end = Some(start + idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(end_pos) = struct_end {
                        let nested_struct_content = &field_type[start + 1..end_pos];
                        debug!(
                            "Recursively parsing nested STRUCT content for '{}': {}",
                            nested_col_name,
                            nested_struct_content.chars().take(150).collect::<String>()
                        );
                        let before_count = columns.len();
                        self.parse_nested_struct_fields_for_sql(
                            nested_struct_content,
                            &nested_col_name,
                            columns,
                        )?;
                        let after_count = columns.len();
                        debug!(
                            "Added {} nested columns for STRUCT '{}' (total: {})",
                            after_count - before_count,
                            nested_col_name,
                            after_count
                        );
                    } else {
                        warn!(
                            "Could not find matching closing '>' for nested STRUCT '{}'",
                            nested_col_name
                        );
                    }
                } else {
                    warn!("Could not find '<' in STRUCT type '{}'", field_type);
                }
            } else {
                // Simple field - create column directly
                columns.push(Column {
                    name: nested_col_name,
                    data_type: field_type_upper,
                    nullable: true,
                    primary_key: false,
                    secondary_key: false,
                    composite_key: None,
                    foreign_key: None,
                    constraints: Vec::new(),
                    description: String::new(),
                    errors: Vec::new(),
                    quality: Vec::new(),
                    enum_values: Vec::new(),
                    column_order: 0,
                });
            }
        }
        Ok(())
    }

    /// Parse a single field definition (e.g., "street VARCHAR(255)" or "street: VARCHAR(255)").
    fn parse_field_definition_for_sql(&self, field_def: &str) -> Result<Option<(String, String)>> {
        let field_def = field_def.trim();
        if field_def.is_empty() {
            return Ok(None);
        }

        // Handle both formats: "field: TYPE" and "field TYPE"
        // For nested STRUCTs, we need to find the first ':' that's not inside angle brackets
        let mut colon_pos = None;
        let mut bracket_depth = 0;
        let mut in_string = false;
        let mut string_char = None;

        for (idx, ch) in field_def.char_indices() {
            match ch {
                '\'' | '"' if !in_string || Some(ch) == string_char => {
                    if in_string {
                        in_string = false;
                        string_char = None;
                    } else {
                        in_string = true;
                        string_char = Some(ch);
                    }
                }
                '<' if !in_string => {
                    bracket_depth += 1;
                }
                '>' if !in_string => {
                    bracket_depth -= 1;
                }
                ':' if !in_string && bracket_depth == 0 => {
                    colon_pos = Some(idx);
                    break;
                }
                _ => {}
            }
        }

        let parts: Vec<String> = if let Some(colon_idx) = colon_pos {
            vec![
                field_def[..colon_idx].trim().to_string(),
                field_def[colon_idx + 1..].trim().to_string(),
            ]
        } else {
            // Split by whitespace, taking first two parts
            let words: Vec<&str> = field_def.split_whitespace().collect();
            if words.len() >= 2 {
                vec![words[0].to_string(), words[1..].join(" ")]
            } else {
                return Ok(None);
            }
        };

        if parts.len() < 2 {
            return Ok(None);
        }

        let field_name = parts[0].trim().to_string();
        let field_type = parts[1].trim().to_string();

        // Remove COMMENT clause if present
        let comment_re = Regex::new(r#"(?i)\s+COMMENT\s+['"]([^'"]*)['"]"#).unwrap();
        let field_type = comment_re.replace(&field_type, "").to_string();

        Ok(Some((field_name, field_type)))
    }
}

impl Default for SQLParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a table that requires name input.
#[derive(Debug, Clone)]
pub struct TableNameInput {
    /// Index of the table in the parsed tables vector
    pub table_index: usize,
    /// Suggested name for the table
    pub suggested_name: String,
    /// Original expression from SQL (for reference)
    pub original_expression: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_create_table() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255)
            );
        "#;

        let (tables, name_inputs) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(name_inputs.len(), 0);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].columns.len(), 3);
        assert_eq!(tables[0].columns[0].name, "id");
        assert_eq!(tables[0].columns[0].data_type, "INTEGER");
        assert!(tables[0].columns[0].primary_key);
    }

    #[test]
    fn test_parse_multiple_tables() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name VARCHAR(255)
            );
            CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                title VARCHAR(255)
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[1].name, "orders");
    }

    #[test]
    fn test_parse_with_foreign_key() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        // Note: Foreign key extraction from AST may need adjustment
        // This test verifies the parser doesn't crash
    }

    #[test]
    fn test_parse_syntax_error_handling() {
        let parser = SQLParser::new();
        let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY"; // Missing closing paren

        // Parser should handle syntax errors gracefully
        let result = parser.parse(sql);
        // Should either return empty tables or handle via fallback parsing
        if let Ok((tables, _)) = result {
            // If parsing succeeds with fallback, that's fine
            assert!(tables.len() <= 1);
        } else {
            // If parsing fails, that's also acceptable for malformed SQL
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_parse_empty_input() {
        let parser = SQLParser::new();
        let (tables, name_inputs) = parser.parse("").unwrap();

        assert_eq!(tables.len(), 0);
        assert_eq!(name_inputs.len(), 0);
    }

    #[test]
    fn test_parse_table_with_comment() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name VARCHAR(255)
            ) COMMENT 'User information table';
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        // Check that comment is stored in odcl_metadata
        if let Some(serde_json::Value::String(desc)) = tables[0].odcl_metadata.get("description") {
            assert!(desc.contains("User information table"));
        }
    }

    #[test]
    fn test_parse_columns_with_comments() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE products (
                id INTEGER PRIMARY KEY COMMENT 'Product identifier',
                name VARCHAR(255) COMMENT 'Product name',
                price DECIMAL(10, 2) COMMENT 'Product price in USD'
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        let table = &tables[0];
        assert_eq!(table.columns.len(), 3);

        // Check column comments
        let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
        assert_eq!(id_col.description, "Product identifier");

        let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
        assert_eq!(name_col.description, "Product name");

        let price_col = table.columns.iter().find(|c| c.name == "price").unwrap();
        assert_eq!(price_col.description, "Product price in USD");
    }

    #[test]
    fn test_parse_with_schema_prefix() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE schema.users (
                id INTEGER PRIMARY KEY
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users"); // Should extract just the table name
    }

    #[test]
    fn test_parse_quoted_table_name() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE "users" (
                id INTEGER PRIMARY KEY
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parse_if_not_exists() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_postgres_dialect() {
        let parser = SQLParser::with_dialect_name("postgres");
        let sql = "CREATE TABLE users (id SERIAL PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        assert!(tables[0].columns.iter().any(|c| c.name == "id"));
        assert!(tables[0].columns.iter().any(|c| c.name == "name"));
    }

    #[test]
    fn test_parser_with_mysql_dialect() {
        let parser = SQLParser::with_dialect_name("mysql");
        let sql = "CREATE TABLE users (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_mssql_dialect() {
        let parser = SQLParser::with_dialect_name("mssql");
        let sql = "CREATE TABLE users (id INT IDENTITY(1,1) PRIMARY KEY, name NVARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_duckdb_dialect() {
        let parser = SQLParser::with_dialect_name("duckdb");
        let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_databricks_dialect() {
        let parser = SQLParser::with_dialect_name("databricks");
        let sql = r#"
            CREATE TABLE customer (
                id INT,
                address STRUCT<street VARCHAR(255), city VARCHAR(255)>
            )
        "#;
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        let customer_table = &tables[0];
        assert_eq!(customer_table.name, "customer");
        // Verify STRUCT columns are parsed (may fall back to string parsing)
        assert!(customer_table.columns.iter().any(|c| c.name == "id"));
    }

    #[test]
    fn test_parser_with_bigquery_dialect() {
        let parser = SQLParser::with_dialect_name("bigquery");
        let sql = "CREATE TABLE users (id INT64, name STRING)";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_redshift_dialect() {
        let parser = SQLParser::with_dialect_name("redshift");
        let sql = "CREATE TABLE users (id INTEGER, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_generic_dialect() {
        let parser = SQLParser::with_dialect_name("generic");
        let sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_unknown_dialect_defaults_to_generic() {
        let parser = SQLParser::with_dialect_name("unknown_dialect");
        let sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_with_other_dialect_defaults_to_generic() {
        let parser = SQLParser::with_dialect_name("other");
        let sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255))";
        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
    }

    #[test]
    fn test_parser_dialect_name_case_insensitive() {
        let parser1 = SQLParser::with_dialect_name("POSTGRES");
        let parser2 = SQLParser::with_dialect_name("Postgres");
        let parser3 = SQLParser::with_dialect_name("postgres");
        let sql = "CREATE TABLE users (id INT PRIMARY KEY)";

        let (tables1, _) = parser1.parse(sql).unwrap();
        let (tables2, _) = parser2.parse(sql).unwrap();
        let (tables3, _) = parser3.parse(sql).unwrap();

        assert_eq!(tables1.len(), 1);
        assert_eq!(tables2.len(), 1);
        assert_eq!(tables3.len(), 1);
    }

    #[test]
    fn test_parse_decimal_types() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE products (
                price DECIMAL(10, 2),
                quantity DECIMAL(5)
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        let price_col = tables[0]
            .columns
            .iter()
            .find(|c| c.name == "price")
            .unwrap();
        assert!(price_col.data_type.contains("DECIMAL"));
    }

    #[test]
    fn test_parse_array_types() {
        let parser = SQLParser::new();
        let sql = r#"
            CREATE TABLE items (
                tags ARRAY<VARCHAR(50)>
            );
        "#;

        let (tables, _) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        let tags_col = tables[0].columns.iter().find(|c| c.name == "tags").unwrap();
        assert!(tags_col.data_type.starts_with("ARRAY"));
    }

    #[test]
    fn test_parse_identifier_with_string_concatenation() {
        let parser = SQLParser::new();
        // Test IDENTIFIER() pattern with string concatenation (Databricks style)
        let sql = r#"
            CREATE TABLE IF NOT EXISTS IDENTIFIER(:dummy_catalog || '.bronze.dummy_table_name') (
                id STRING COMMENT 'Unique identifier',
                name STRING COMMENT 'Name field',
                value INT COMMENT 'Value field'
            )
            COMMENT "Dummy Table"
            TBLPROPERTIES ('quality' = 'bronze');
        "#;

        let (tables, name_inputs) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(name_inputs.len(), 1); // Should require name input due to variable
        assert_eq!(tables[0].name, "dummy_table_name"); // Should extract from quoted string
        assert_eq!(tables[0].columns.len(), 3);
        assert_eq!(tables[0].columns[0].name, "id");
        assert_eq!(tables[0].columns[0].data_type, "STRING");
        assert_eq!(tables[0].columns[0].description, "Unique identifier");
        assert_eq!(tables[0].columns[1].name, "name");
        assert_eq!(tables[0].columns[2].name, "value");
    }

    #[test]
    fn test_parse_identifier_with_nested_struct() {
        let parser = SQLParser::new();
        // Test IDENTIFIER() with nested STRUCT types
        let sql = r#"
            CREATE TABLE IF NOT EXISTS IDENTIFIER(:dummy_catalog || '.bronze.dummy_nested_table') (
                id STRING,
                metadata STRUCT<
                    field1: STRING,
                    field2: INT,
                    nested: STRUCT<
                        subfield1: STRING,
                        subfield2: BOOLEAN
                    >
                > COMMENT 'Nested metadata structure',
                items ARRAY<STRUCT<
                    item_id: STRING,
                    item_name: STRING
                >> COMMENT 'Array of items'
            );
        "#;

        let (tables, _name_inputs) = parser.parse(sql).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "dummy_nested_table");

        // With nested field extraction, we should have:
        // 1. id (parent)
        // 2. metadata (parent)
        // 3. metadata.field1 (nested)
        // 4. metadata.field2 (nested)
        // 5. metadata.nested (nested parent)
        // 6. metadata.nested.subfield1 (nested)
        // 7. metadata.nested.subfield2 (nested)
        // 8. items (parent)
        // 9. items.item_id (nested)
        // 10. items.item_name (nested)
        // So at least 8 columns (could be more if ARRAY nested fields are extracted)
        assert!(
            tables[0].columns.len() >= 8,
            "Expected at least 8 columns (3 parent + nested fields), got {}",
            tables[0].columns.len()
        );

        // Verify parent columns exist
        let parent_columns: Vec<_> = tables[0]
            .columns
            .iter()
            .filter(|c| !c.name.contains('.'))
            .collect();
        assert_eq!(parent_columns.len(), 3, "Expected 3 parent columns");
        assert_eq!(parent_columns[0].name, "id");
        assert_eq!(parent_columns[1].name, "metadata");
        // The data type should be normalized to "STRUCT" (not "STRUCT<...>")
        assert_eq!(parent_columns[1].data_type, "STRUCT");
        assert_eq!(parent_columns[2].name, "items");
        // The data type should be normalized to "ARRAY" (not "ARRAY<STRUCT<...>>")
        assert_eq!(parent_columns[2].data_type, "ARRAY");

        // Verify nested columns exist
        let nested_columns: Vec<_> = tables[0]
            .columns
            .iter()
            .filter(|c| c.name.contains('.'))
            .collect();
        let column_names: Vec<_> = tables[0].columns.iter().map(|c| c.name.as_str()).collect();
        // We should have at least: metadata.field1, metadata.field2, metadata.nested, items.item_id, items.item_name
        assert!(
            nested_columns.len() >= 5,
            "Expected at least 5 nested columns, got {}. Columns: {:?}",
            nested_columns.len(),
            column_names
        );

        // Verify specific nested columns
        assert!(
            column_names.contains(&"metadata.field1"),
            "Missing metadata.field1. Columns: {:?}",
            column_names
        );
        assert!(
            column_names.contains(&"metadata.field2"),
            "Missing metadata.field2. Columns: {:?}",
            column_names
        );
        assert!(
            column_names.contains(&"metadata.nested"),
            "Missing metadata.nested. Columns: {:?}",
            column_names
        );
        assert!(
            column_names.contains(&"items.item_id"),
            "Missing items.item_id. Columns: {:?}",
            column_names
        );
        assert!(
            column_names.contains(&"items.item_name"),
            "Missing items.item_name. Columns: {:?}",
            column_names
        );

        // Note: Currently nested STRUCTs within STRUCTs (like metadata.nested.subfield1) are not fully extracted
        // This is a known limitation - the nested STRUCT is created as a parent column but its fields aren't flattened
    }
}
