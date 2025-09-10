use secrecy::{ExposeSecret, Secret, Zeroize};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng}};
use dashmap::DashMap;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Token generation failed: {0}")]
    TokenGeneration(String),
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("Cryptographic operation failed: {0}")]
    CryptographicFailure(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    READ_CODE,
    READ_GRAPH,
    READ_METRICS,
    READ_ANALYSIS,
    WRITE_ANNOTATIONS,
    MANAGE_CACHE,
    ADMIN_SYSTEM,
    SUBSCRIBE_UPDATES,
    MANAGE_API_KEYS,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::READ_CODE => write!(f, "read:code"),
            Permission::READ_GRAPH => write!(f, "read:graph"),
            Permission::READ_METRICS => write!(f, "read:metrics"),
            Permission::READ_ANALYSIS => write!(f, "read:analysis"),
            Permission::WRITE_ANNOTATIONS => write!(f, "write:annotations"),
            Permission::MANAGE_CACHE => write!(f, "manage:cache"),
            Permission::ADMIN_SYSTEM => write!(f, "admin:system"),
            Permission::SUBSCRIBE_UPDATES => write!(f, "subscribe:updates"),
            Permission::MANAGE_API_KEYS => write!(f, "manage:api_keys"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub username: String,
    pub permissions: Vec<Permission>,
    pub roles: Vec<String>,
    pub organization_id: Option<String>,
    pub project_access: Vec<String>,
    pub session_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: Secret<String>,
    pub jwt_expiry_hours: u64,
    pub api_key_prefix: String,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self, SecurityError> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| SecurityError::KeyDerivation("JWT_SECRET environment variable not found".to_string()))?;
            
        // Validate JWT secret strength
        if jwt_secret.len() < 32 {
            return Err(SecurityError::KeyDerivation("JWT_SECRET must be at least 32 characters".to_string()));
        }
        
        Ok(Self {
            jwt_secret: Secret::new(jwt_secret),
            jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
            api_key_prefix: std::env::var("API_KEY_PREFIX")
                .unwrap_or_else(|_| "cgk".to_string()),
        })
    }
    
    pub fn jwt_secret(&self) -> &str {
        self.jwt_secret.expose_secret()
    }
}

pub struct ApiKeyManager {
    keys: DashMap<String, ApiKey>,
    argon2: Argon2<'static>,
}

impl ApiKeyManager {
    pub fn new() -> Self {
        Self {
            keys: DashMap::new(),
            argon2: Argon2::default(),
        }
    }
    
    pub fn create_key(
        &self,
        name: String,
        permissions: Vec<Permission>,
        created_by: Uuid,
        prefix: &str,
    ) -> Result<(String, Uuid), SecurityError> {
        // Generate cryptographically secure key
        let key_value = self.generate_secure_key(prefix)?;
        let key_hash = self.hash_api_key(&key_value)?;
        
        let api_key = ApiKey {
            id: Uuid::new_v4(),
            key_hash: key_hash.clone(),
            name,
            permissions,
            created_at: Utc::now(),
            expires_at: None,
            last_used: None,
            is_active: true,
            created_by,
        };
        
        self.keys.insert(key_hash, api_key.clone());
        Ok((key_value, api_key.id))
    }
    
    pub fn validate_key(&self, key: &str) -> Option<ApiKey> {
        let key_hash = self.hash_api_key(key).ok()?;
        
        self.keys.get_mut(&key_hash).and_then(|mut entry| {
            let api_key = entry.value_mut();
            
            // Check if key is active and not expired
            if !api_key.is_active {
                return None;
            }
            
            if let Some(expires_at) = api_key.expires_at {
                if expires_at <= Utc::now() {
                    return None;
                }
            }
            
            // Update last used timestamp
            api_key.last_used = Some(Utc::now());
            Some(api_key.clone())
        })
    }
    
    pub fn revoke_key(&self, key_id: &Uuid) -> bool {
        for mut entry in self.keys.iter_mut() {
            if entry.value().id == *key_id {
                entry.value_mut().is_active = false;
                return true;
            }
        }
        false
    }
    
    pub fn list_keys_for_user(&self, user_id: &Uuid) -> Vec<ApiKey> {
        self.keys
            .iter()
            .filter(|entry| entry.value().created_by == *user_id)
            .map(|entry| {
                let mut key = entry.value().clone();
                // Don't expose the hash
                key.key_hash = "***".to_string();
                key
            })
            .collect()
    }
    
    fn generate_secure_key(&self, prefix: &str) -> Result<String, SecurityError> {
        use base64::{Engine as _, engine::general_purpose};
        let mut key_bytes = [0u8; 32];
        getrandom::getrandom(&mut key_bytes)
            .map_err(|e| SecurityError::TokenGeneration(e.to_string()))?;
        
        Ok(format!("{}_{}", prefix, general_purpose::URL_SAFE_NO_PAD.encode(&key_bytes)))
    }
    
    fn hash_api_key(&self, key: &str) -> Result<String, SecurityError> {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = self.argon2
            .hash_password(key.as_bytes(), &salt)
            .map_err(|e| SecurityError::CryptographicFailure(e.to_string()))?;
        
        Ok(password_hash.to_string())
    }
    
    fn verify_api_key(&self, key: &str, hash: &str) -> Result<bool, SecurityError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| SecurityError::CryptographicFailure(e.to_string()))?;
        
        match self.argon2.verify_password(key.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Secure JWT token manager
pub struct JwtManager {
    config: AuthConfig,
}

impl JwtManager {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }
    
    pub fn create_token(&self, auth_context: &AuthContext) -> Result<String, SecurityError> {
        use jsonwebtoken::{encode, Header, EncodingKey, Algorithm};
        use serde_json::json;
        
        let claims = json!({
            "sub": auth_context.user_id.to_string(),
            "username": auth_context.username,
            "permissions": auth_context.permissions,
            "roles": auth_context.roles,
            "org": auth_context.organization_id,
            "projects": auth_context.project_access,
            "session_id": auth_context.session_id,
            "iat": auth_context.issued_at.timestamp(),
            "exp": auth_context.expires_at.timestamp(),
        });
        
        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.config.jwt_secret().as_bytes());
        
        encode(&header, &claims, &key)
            .map_err(|e| SecurityError::TokenGeneration(e.to_string()))
    }
    
    pub fn validate_token(&self, token: &str) -> Result<AuthContext, SecurityError> {
        use jsonwebtoken::{decode, Validation, DecodingKey, Algorithm};
        use serde_json::Value;
        
        let key = DecodingKey::from_secret(self.config.jwt_secret().as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        
        let token_data = decode::<Value>(token, &key, &validation)
            .map_err(|e| SecurityError::InvalidCredentials)?;
        
        let claims = &token_data.claims;
        
        Ok(AuthContext {
            user_id: Uuid::parse_str(claims["sub"].as_str().unwrap_or(""))
                .map_err(|_| SecurityError::InvalidCredentials)?,
            username: claims["username"].as_str().unwrap_or("").to_string(),
            permissions: serde_json::from_value(claims["permissions"].clone())
                .unwrap_or_default(),
            roles: serde_json::from_value(claims["roles"].clone())
                .unwrap_or_default(),
            organization_id: claims["org"].as_str().map(|s| s.to_string()),
            project_access: serde_json::from_value(claims["projects"].clone())
                .unwrap_or_default(),
            session_id: claims["session_id"].as_str().unwrap_or("").to_string(),
            issued_at: DateTime::from_timestamp(claims["iat"].as_i64().unwrap_or(0), 0)
                .unwrap_or_default(),
            expires_at: DateTime::from_timestamp(claims["exp"].as_i64().unwrap_or(0), 0)
                .unwrap_or_default(),
        })
    }
}

/// Authorization engine for permission checks
pub struct AuthorizationEngine;

impl AuthorizationEngine {
    pub fn has_permission(context: &AuthContext, required: &[Permission]) -> bool {
        // Admin system permission grants all other permissions
        if context.permissions.contains(&Permission::ADMIN_SYSTEM) {
            return true;
        }
        
        // Check specific permissions
        required.iter().all(|perm| context.permissions.contains(perm))
    }
    
    pub fn has_role(context: &AuthContext, required_roles: &[&str]) -> bool {
        required_roles.iter().any(|role| context.roles.contains(&role.to_string()))
    }
    
    pub fn can_access_project(context: &AuthContext, project_id: &str) -> bool {
        // Admin can access any project
        if context.permissions.contains(&Permission::ADMIN_SYSTEM) {
            return true;
        }
        
        // Check project-specific access
        context.project_access.contains(&project_id.to_string())
    }
}

/// Security event logging
#[derive(Debug, Clone)]
pub enum SecurityEvent {
    AuthenticationFailure {
        username: String,
        ip_address: String,
        reason: String,
    },
    AuthenticationSuccess {
        user_id: Uuid,
        ip_address: String,
        method: String,
    },
    PermissionDenied {
        user_id: Uuid,
        resource: String,
        required_permission: String,
    },
    AdminAccess {
        user_id: Uuid,
        action: String,
        resource: String,
    },
    ApiKeyCreated {
        key_id: Uuid,
        created_by: Uuid,
        permissions: Vec<Permission>,
    },
    ApiKeyRevoked {
        key_id: Uuid,
        revoked_by: Uuid,
    },
    SuspiciousActivity {
        user_id: Option<Uuid>,
        ip_address: String,
        description: String,
    },
}

pub struct SecurityLogger;

impl SecurityLogger {
    pub fn log_event(event: SecurityEvent) {
        use tracing::{warn, info, error};
        
        match event {
            SecurityEvent::AuthenticationFailure { username, ip_address, reason } => {
                warn!(
                    username = %username,
                    ip_address = %ip_address,
                    reason = %reason,
                    "Authentication failure"
                );
            }
            SecurityEvent::AuthenticationSuccess { user_id, ip_address, method } => {
                info!(
                    user_id = %user_id,
                    ip_address = %ip_address,
                    method = %method,
                    "Authentication success"
                );
            }
            SecurityEvent::PermissionDenied { user_id, resource, required_permission } => {
                warn!(
                    user_id = %user_id,
                    resource = %resource,
                    required_permission = %required_permission,
                    "Permission denied"
                );
            }
            SecurityEvent::AdminAccess { user_id, action, resource } => {
                info!(
                    user_id = %user_id,
                    action = %action,
                    resource = %resource,
                    "Admin access"
                );
            }
            SecurityEvent::ApiKeyCreated { key_id, created_by, permissions } => {
                info!(
                    key_id = %key_id,
                    created_by = %created_by,
                    permissions = ?permissions,
                    "API key created"
                );
            }
            SecurityEvent::ApiKeyRevoked { key_id, revoked_by } => {
                info!(
                    key_id = %key_id,
                    revoked_by = %revoked_by,
                    "API key revoked"
                );
            }
            SecurityEvent::SuspiciousActivity { user_id, ip_address, description } => {
                error!(
                    user_id = ?user_id,
                    ip_address = %ip_address,
                    description = %description,
                    "Suspicious activity detected"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_key_generation() {
        let manager = ApiKeyManager::new();
        let user_id = Uuid::new_v4();
        
        let result = manager.create_key(
            "test-key".to_string(),
            vec![Permission::READ_CODE],
            user_id,
            "test"
        );
        
        assert!(result.is_ok());
        let (key, key_id) = result.unwrap();
        assert!(key.starts_with("test_"));
        assert!(key.len() > 40); // Should be long enough
    }
    
    #[test]
    fn test_permission_hierarchy() {
        let mut context = AuthContext {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            permissions: vec![Permission::ADMIN_SYSTEM],
            roles: vec![],
            organization_id: None,
            project_access: vec![],
            session_id: "session".to_string(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        
        // Admin should have all permissions
        assert!(AuthorizationEngine::has_permission(&context, &[Permission::READ_CODE]));
        assert!(AuthorizationEngine::has_permission(&context, &[Permission::MANAGE_CACHE]));
        
        // Non-admin should only have specific permissions
        context.permissions = vec![Permission::READ_CODE];
        assert!(AuthorizationEngine::has_permission(&context, &[Permission::READ_CODE]));
        assert!(!AuthorizationEngine::has_permission(&context, &[Permission::ADMIN_SYSTEM]));
    }
}