//! AI service for assisted import and error resolution.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tracing::warn;

/// AI service for parsing, mapping, and error resolution.
pub struct AIService {
    client: Option<Client>,
    api_key: Option<String>,
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIResponse {
    pub corrected_sql: Option<String>,
    pub corrected_yaml: Option<String>,
    pub explanation: String,
    pub confidence: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIErrorResolution {
    #[serde(rename = "type")]
    pub error_type: String,
    pub corrected_sql: Option<String>,
    pub corrected_yaml: Option<String>,
    pub explanation: String,
    pub confidence: String,
}

impl AIService {
    /// Create a new AI service instance.
    pub fn new() -> Self {
        let api_key = env::var("OPENAI_API_KEY").ok();
        let model = env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

        let client = if api_key.is_some() {
            Some(Client::new())
        } else {
            warn!("OpenAI API key not configured");
            None
        };

        Self {
            client,
            api_key,
            model,
        }
    }

    /// Use AI to resolve SQL syntax errors.
    pub async fn resolve_sql_errors(
        &self,
        sql_content: &str,
        error_message: &str,
    ) -> Result<Vec<AIErrorResolution>> {
        if self.client.is_none() || self.api_key.is_none() {
            return Ok(Vec::new());
        }

        let prompt = format!(
            r#"You are a SQL expert. Fix the following SQL syntax error.

SQL Statement:
{sql_content}

Error Message:
{error_message}

Provide the corrected SQL statement and explain what was wrong. Return your response as JSON with this format:
{{
    "corrected_sql": "the fixed SQL statement",
    "explanation": "brief explanation of what was fixed",
    "confidence": "high|medium|low"
}}"#
        );

        let response = self
            .call_ai(
                &prompt,
                "You are a SQL expert assistant. Always return valid JSON.",
            )
            .await?;

        let ai_response: AIResponse =
            serde_json::from_str(&response).context("Failed to parse AI response")?;

        Ok(vec![AIErrorResolution {
            error_type: "sql_fix".to_string(),
            corrected_sql: ai_response.corrected_sql,
            corrected_yaml: None,
            explanation: ai_response.explanation,
            confidence: ai_response.confidence,
        }])
    }

    /// Use AI to resolve ODCL validation errors.
    pub async fn resolve_odcl_errors(
        &self,
        yaml_content: &str,
        errors: &[String],
    ) -> Result<Vec<AIErrorResolution>> {
        if self.client.is_none() || self.api_key.is_none() {
            return Ok(Vec::new());
        }

        let errors_str = errors.join("; ");

        let prompt = format!(
            r#"You are an Open Data Contract (ODCL) expert. Fix the following ODCL YAML validation errors.

YAML Content:
{yaml_content}

Validation Errors:
{errors_str}

Provide the corrected YAML and explain what was wrong. Return your response as JSON with this format:
{{
    "corrected_yaml": "the fixed YAML content",
    "explanation": "brief explanation of what was fixed",
    "confidence": "high|medium|low"
}}"#
        );

        let response = self
            .call_ai(
                &prompt,
                "You are an ODCL expert assistant. Always return valid JSON.",
            )
            .await?;

        let ai_response: AIResponse =
            serde_json::from_str(&response).context("Failed to parse AI response")?;

        Ok(vec![AIErrorResolution {
            error_type: "odcl_fix".to_string(),
            corrected_sql: None,
            corrected_yaml: ai_response.corrected_yaml,
            explanation: ai_response.explanation,
            confidence: ai_response.confidence,
        }])
    }

    /// Use AI to suggest relationships between tables.
    #[allow(dead_code)]
    pub async fn suggest_relationships(
        &self,
        tables: &[crate::models::Table],
    ) -> Result<Vec<serde_json::Value>> {
        if self.client.is_none() || self.api_key.is_none() {
            return Ok(Vec::new());
        }

        let tables_json =
            serde_json::to_string(tables).context("Failed to serialize tables to JSON")?;

        let prompt = format!(
            r#"You are a data modeling expert. Analyze the following tables and suggest relationships between them.

Tables:
{tables_json}

For each suggested relationship, return JSON with this format:
{{
    "source_table_id": "uuid",
    "target_table_id": "uuid",
    "cardinality": "OneToOne|OneToMany|ManyToOne|ManyToMany",
    "relationship_type": "DataFlow|Dependency|ForeignKey|EtlTransformation",
    "explanation": "why this relationship makes sense",
    "confidence": "high|medium|low"
}}

Return an array of suggested relationships."#
        );

        let response = self
            .call_ai(
                &prompt,
                "You are a data modeling expert. Always return valid JSON array.",
            )
            .await?;

        let suggestions: Vec<serde_json::Value> =
            serde_json::from_str(&response).context("Failed to parse AI suggestions")?;

        Ok(suggestions)
    }

    /// Call AI API with prompt.
    async fn call_ai(&self, prompt: &str, system_message: &str) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AI client not initialized"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("API key not configured"))?;

        let url = env::var("AI_SERVICE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());

        let request_body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_message},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3,
            "response_format": {"type": "json_object"}
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to AI service")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "AI service returned error {}: {}",
                status,
                error_text
            ));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse AI service response")?;

        let content = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid AI response format"))?;

        Ok(content.to_string())
    }
}

impl Default for AIService {
    fn default() -> Self {
        Self::new()
    }
}
