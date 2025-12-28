// SQL parser module for high-performance SQL parsing
// Note: This is a legacy module kept for compatibility
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

/// Parse SQL CREATE TABLE statements
/// This is a simplified parser - full implementation would use a proper SQL parser library
#[allow(dead_code)]
pub fn parse_sql(sql: &str) -> Result<Vec<Table>, String> {
    let mut tables = Vec::new();

    // Split by semicolons to get individual statements
    let statements: Vec<&str> = sql
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for statement in statements {
        if statement.to_uppercase().starts_with("CREATE TABLE") {
            if let Some(table) = parse_create_table(statement) {
                tables.push(table);
            }
        }
    }

    Ok(tables)
}

#[allow(dead_code)]
fn parse_create_table(statement: &str) -> Option<Table> {
    // Extract table name (simplified - between CREATE TABLE and opening paren)
    let table_name_start = statement.to_uppercase().find("CREATE TABLE")?;
    let after_create = &statement[table_name_start + 12..].trim();

    // Find table name (before opening paren, handle IF NOT EXISTS)
    let after_create_upper = after_create.to_uppercase();
    let skip_if_not_exists = if after_create_upper.starts_with("IF NOT EXISTS") {
        13
    } else {
        0
    };
    let after_keywords = &after_create[skip_if_not_exists..].trim();

    // Find table name (before opening paren)
    let paren_pos = after_keywords.find('(')?;
    let table_name = after_keywords[..paren_pos].trim().to_string();

    // Extract column definitions (between parentheses)
    let start_paren = after_keywords.find('(')?;
    let columns_str = &after_keywords[start_paren + 1..];
    let end_paren = columns_str.rfind(')')?;
    let columns_str = &columns_str[..end_paren];

    let columns = parse_columns(columns_str);

    Some(Table {
        name: table_name,
        columns,
    })
}

#[allow(dead_code)]
fn parse_columns(columns_str: &str) -> Vec<Column> {
    let mut columns = Vec::new();

    // Split by comma, but be careful of nested parentheses
    let parts: Vec<&str> = columns_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for part in parts {
        let part_upper = part.to_uppercase();
        let part_upper_trimmed = part_upper.trim();

        // Skip standalone constraint definitions (not part of column definition)
        // Standalone constraints start with PRIMARY KEY, FOREIGN KEY, or CONSTRAINT
        // and don't have a column name (first word is the constraint keyword)
        let first_word = part_upper_trimmed.split_whitespace().next().unwrap_or("");
        if first_word == "PRIMARY" && part_upper.contains("KEY") && !part.contains("(")
            || (first_word == "FOREIGN" && part_upper.contains("KEY"))
            || first_word == "CONSTRAINT"
        {
            continue;
        }

        // Parse column definition
        // Format: column_name data_type [constraints]
        let words: Vec<&str> = part.split_whitespace().collect();
        if words.len() >= 2 {
            let name = words[0].to_string();
            let data_type = words[1].to_string();
            let nullable = !part.to_uppercase().contains("NOT NULL");
            let primary_key = part.to_uppercase().contains("PRIMARY KEY");

            columns.push(Column {
                name,
                data_type,
                nullable,
                primary_key,
            });
        }
    }

    columns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));";
        let result = parse_sql(sql);

        assert!(result.is_ok());
        let tables = result.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].columns.len(), 2);
    }

    #[test]
    fn test_parse_multiple_tables() {
        let sql = "CREATE TABLE users (id INT); CREATE TABLE orders (id INT);";
        let result = parse_sql(sql);

        assert!(result.is_ok());
        let tables = result.unwrap();
        assert_eq!(tables.len(), 2);
    }
}
