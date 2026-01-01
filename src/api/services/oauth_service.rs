//! OAuth service for GitHub authentication.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Clone)]
pub struct OAuthService {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http_client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    #[allow(dead_code)]
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubEmail {
    pub email: String,
    pub verified: bool,
    pub primary: bool,
}

impl OAuthService {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            http_client: reqwest::Client::new(),
        }
    }

    /// Generate GitHub OAuth authorization URL
    #[allow(dead_code)]
    pub fn get_authorize_url(&self) -> Result<String> {
        // Back-compat default: historically this embedded source in state.
        // New callers should generate a random state and call `get_authorize_url_with_state`.
        self.get_authorize_url_with_state("web")
    }

    /// Generate GitHub OAuth authorization URL with an explicit `state` value.
    ///
    /// Security: the caller should pass a cryptographically random, server-validated CSRF token.
    pub fn get_authorize_url_with_state(&self, state: &str) -> Result<String> {
        let url = format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email&state={}",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(state)
        );
        Ok(url)
    }

    /// Deprecated: embeds caller-controlled values into OAuth `state`.
    /// Kept temporarily for compatibility; do not use for new code.
    #[allow(dead_code)]
    pub fn get_authorize_url_with_source(&self, source: &str) -> Result<String> {
        self.get_authorize_url_with_state(source)
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(&self, code: &str) -> Result<String> {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
        ];

        let response = self
            .http_client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .context("Failed to send token request to GitHub")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "GitHub token exchange failed: {}",
                error_text
            ));
        }

        let token_response: GitHubTokenResponse = response
            .json()
            .await
            .context("Failed to parse GitHub token response")?;

        Ok(token_response.access_token)
    }

    /// Fetch user information and emails from GitHub
    pub async fn fetch_user_info(
        &self,
        access_token: &str,
    ) -> Result<(u64, String, Vec<GitHubEmail>)> {
        // Fetch user profile
        let user_response = self
            .http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "modelling-app")
            .send()
            .await
            .context("Failed to fetch user info from GitHub")?;

        if !user_response.status().is_success() {
            let error_text = user_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("GitHub user API failed: {}", error_text));
        }

        let user: GitHubUser = user_response
            .json()
            .await
            .context("Failed to parse GitHub user response")?;

        // Fetch user emails
        let emails_response = self
            .http_client
            .get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "modelling-app")
            .send()
            .await
            .context("Failed to fetch user emails from GitHub")?;

        if !emails_response.status().is_success() {
            warn!("Failed to fetch emails, using user email if available");
            let emails = if let Some(email) = user.email {
                vec![GitHubEmail {
                    email,
                    verified: true,
                    primary: true,
                }]
            } else {
                Vec::new()
            };
            return Ok((user.id, user.login, emails));
        }

        let emails: Vec<GitHubEmail> = emails_response
            .json()
            .await
            .context("Failed to parse GitHub emails response")?;

        info!(
            "Fetched {} emails for GitHub user {}",
            emails.len(),
            user.login
        );

        Ok((user.id, user.login, emails))
    }
}
