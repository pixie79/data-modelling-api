//! JWT Service for token generation and validation.
//!
//! Provides time-scoped JWT tokens for API authentication.
//! - Access tokens: Short-lived (15 minutes) for API requests
//! - Refresh tokens: Longer-lived (7 days) for obtaining new access tokens

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user email)
    pub sub: String,
    /// GitHub user ID
    pub github_id: u64,
    /// GitHub username
    pub github_username: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Token type: "access" or "refresh"
    pub token_type: TokenType,
    /// Session ID (for tracking/revocation)
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
}

/// Token pair returned after authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: i64,
    pub refresh_token_expires_at: i64,
    pub token_type: String,
}

/// JWT Service configuration
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_duration: Duration,
    refresh_token_duration: Duration,
}

impl JwtService {
    /// Create a new JWT service with the given secret
    ///
    /// # Arguments
    /// * `secret` - The secret key for signing tokens (should be at least 32 bytes)
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_token_duration: Duration::minutes(15),
            refresh_token_duration: Duration::days(7),
        }
    }

    /// Create a new JWT service from environment variables.
    ///
    /// In production (APP_ENV != "development"), this will panic if JWT_SECRET is not set.
    /// In development, falls back to an insecure default secret with a warning.
    ///
    /// # Panics
    /// Panics in production if JWT_SECRET environment variable is not set.
    pub fn from_env() -> Self {
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
        let is_development = app_env.to_lowercase() == "development";

        let secret = match std::env::var("JWT_SECRET") {
            Ok(s) => s,
            Err(_) => {
                if is_development {
                    warn!(
                        "JWT_SECRET not set! Using default secret for development. DO NOT USE IN PRODUCTION!"
                    );
                    "dev-secret-do-not-use-in-production-change-me-now".to_string()
                } else {
                    panic!(
                        "CRITICAL: JWT_SECRET environment variable is required in production. Set APP_ENV=development to use default secret."
                    );
                }
            }
        };

        if secret.len() < 32 {
            if is_development {
                warn!("JWT_SECRET is less than 32 characters. Consider using a longer secret.");
            } else {
                panic!("CRITICAL: JWT_SECRET must be at least 32 characters in production.");
            }
        }

        Self::new(&secret)
    }

    /// Try to create a JWT service from environment variables.
    ///
    /// Returns an error if JWT_SECRET is not set or is too short in production.
    /// This is the safer alternative to `from_env()` for graceful error handling.
    #[allow(dead_code)]
    pub fn try_from_env() -> Result<Self, String> {
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
        let is_development = app_env.to_lowercase() == "development";

        let secret = match std::env::var("JWT_SECRET") {
            Ok(s) => s,
            Err(_) => {
                if is_development {
                    warn!(
                        "JWT_SECRET not set! Using default secret for development. DO NOT USE IN PRODUCTION!"
                    );
                    "dev-secret-do-not-use-in-production-change-me-now".to_string()
                } else {
                    return Err(
                        "JWT_SECRET environment variable is required in production".to_string()
                    );
                }
            }
        };

        if secret.len() < 32 {
            if is_development {
                warn!("JWT_SECRET is less than 32 characters. Consider using a longer secret.");
            } else {
                return Err("JWT_SECRET must be at least 32 characters in production".to_string());
            }
        }

        Ok(Self::new(&secret))
    }

    /// Generate a token pair (access + refresh) for a user
    pub fn generate_token_pair(
        &self,
        email: &str,
        github_id: u64,
        github_username: &str,
        session_id: &str,
    ) -> Result<TokenPair, String> {
        let now = Utc::now();

        // Access token
        let access_exp = now + self.access_token_duration;
        let access_claims = Claims {
            sub: email.to_string(),
            github_id,
            github_username: github_username.to_string(),
            exp: access_exp.timestamp(),
            iat: now.timestamp(),
            token_type: TokenType::Access,
            session_id: session_id.to_string(),
        };

        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)
            .map_err(|e| format!("Failed to encode access token: {}", e))?;

        // Refresh token
        let refresh_exp = now + self.refresh_token_duration;
        let refresh_claims = Claims {
            sub: email.to_string(),
            github_id,
            github_username: github_username.to_string(),
            exp: refresh_exp.timestamp(),
            iat: now.timestamp(),
            token_type: TokenType::Refresh,
            session_id: session_id.to_string(),
        };

        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)
            .map_err(|e| format!("Failed to encode refresh token: {}", e))?;

        info!(
            "Generated token pair for user {} (session: {}), access expires: {}, refresh expires: {}",
            email, session_id, access_exp, refresh_exp
        );

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_token_expires_at: access_exp.timestamp(),
            refresh_token_expires_at: refresh_exp.timestamp(),
            token_type: "Bearer".to_string(),
        })
    }

    /// Validate an access token and return the claims
    pub fn validate_access_token(&self, token: &str) -> Result<Claims, String> {
        let token_data = self.decode_token(token)?;

        if token_data.claims.token_type != TokenType::Access {
            return Err("Invalid token type: expected access token".to_string());
        }

        Ok(token_data.claims)
    }

    /// Validate a refresh token and return the claims
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims, String> {
        let token_data = self.decode_token(token)?;

        if token_data.claims.token_type != TokenType::Refresh {
            return Err("Invalid token type: expected refresh token".to_string());
        }

        Ok(token_data.claims)
    }

    /// Decode and validate a token (checks signature and expiration)
    fn decode_token(&self, token: &str) -> Result<TokenData<Claims>, String> {
        let mut validation = Validation::default();
        validation.validate_exp = true;

        decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => "Token has expired".to_string(),
            jsonwebtoken::errors::ErrorKind::InvalidToken => "Invalid token format".to_string(),
            jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                "Invalid token signature".to_string()
            }
            _ => format!("Token validation failed: {}", e),
        })
    }

    /// Generate a new access token from a valid refresh token
    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenPair, String> {
        let claims = self.validate_refresh_token(refresh_token)?;

        // Generate new token pair with same session ID
        self.generate_token_pair(
            &claims.sub,
            claims.github_id,
            &claims.github_username,
            &claims.session_id,
        )
    }

    /// Extract bearer token from Authorization header
    pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
        if auth_header.starts_with("Bearer ") {
            auth_header.strip_prefix("Bearer ")
        } else {
            None
        }
    }
}

/// Shared JWT service for use across the application
pub type SharedJwtService = Arc<JwtService>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_validation() {
        let service = JwtService::new("test-secret-key-at-least-32-chars");

        let token_pair = service
            .generate_token_pair("test@example.com", 12345, "testuser", "session-123")
            .unwrap();

        // Validate access token
        let claims = service
            .validate_access_token(&token_pair.access_token)
            .unwrap();
        assert_eq!(claims.sub, "test@example.com");
        assert_eq!(claims.github_id, 12345);
        assert_eq!(claims.github_username, "testuser");
        assert_eq!(claims.token_type, TokenType::Access);

        // Validate refresh token
        let refresh_claims = service
            .validate_refresh_token(&token_pair.refresh_token)
            .unwrap();
        assert_eq!(refresh_claims.sub, "test@example.com");
        assert_eq!(refresh_claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_wrong_token_type() {
        let service = JwtService::new("test-secret-key-at-least-32-chars");

        let token_pair = service
            .generate_token_pair("test@example.com", 12345, "testuser", "session-123")
            .unwrap();

        // Try to validate access token as refresh token
        let result = service.validate_refresh_token(&token_pair.access_token);
        assert!(result.is_err());

        // Try to validate refresh token as access token
        let result = service.validate_access_token(&token_pair.refresh_token);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_token() {
        let service = JwtService::new("test-secret-key-at-least-32-chars");

        let result = service.validate_access_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_refresh() {
        let service = JwtService::new("test-secret-key-at-least-32-chars");

        let original_pair = service
            .generate_token_pair("test@example.com", 12345, "testuser", "session-123")
            .unwrap();

        let new_pair = service
            .refresh_access_token(&original_pair.refresh_token)
            .unwrap();

        // New access token should be valid
        let claims = service
            .validate_access_token(&new_pair.access_token)
            .unwrap();
        assert_eq!(claims.sub, "test@example.com");
        assert_eq!(claims.session_id, "session-123");
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(
            JwtService::extract_bearer_token("Bearer abc123"),
            Some("abc123")
        );
        assert_eq!(JwtService::extract_bearer_token("bearer abc123"), None);
        assert_eq!(JwtService::extract_bearer_token("abc123"), None);
    }
}
