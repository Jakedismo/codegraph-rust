use async_graphql::{Context, Guard, Result};
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Utc};
use governor::{
    clock::DefaultClock,
    middleware::{RateLimitingMiddleware, StateInformationMiddleware},
    state::{InMemoryState, NotKeyed},
    RateLimiter,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, num::NonZeroU32, sync::Arc};
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    READ_CODE,
    READ_GRAPH,
    READ_METRICS,
    READ_ANALYSIS,
    WRITE_ANNOTATIONS,
    MANAGE_CACHE,
    ADMIN_SYSTEM,
    SUBSCRIBE_UPDATES,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    permissions: Vec<Permission>,
    roles: Vec<String>,
    org: Option<String>,
    projects: Vec<String>,
    session_id: String,
    iat: usize,
    exp: usize,
}

pub struct AuthGuard {
    pub required_permissions: Vec<Permission>,
}

impl Guard for AuthGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let auth_context = ctx.data_opt::<AuthContext>();

        if let Some(auth_context) = auth_context {
            if AuthorizationEngine::has_permission(auth_context, &self.required_permissions) {
                Ok(())
            } else {
                Err("Insufficient permissions".into())
            }
        } else {
            Err("Authentication required".into())
        }
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = req
        .headers()
        .get("X-API-KEY")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(api_key) = api_key {
        return api_key_auth(&api_key, req, next).await;
    }

    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            if auth_value.starts_with("Bearer ") {
                Some(auth_value[7..].to_string())
            } else {
                None
            }
        });

    if let Some(token) = token {
        let decoding_key = DecodingKey::from_secret("your-secret-key".as_ref());
        let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        if let Ok(token_data) = decode::<Claims>(&token, &decoding_key, &validation) {
            let auth_context = AuthContext {
                user_id: Uuid::parse_str(&token_data.claims.sub).unwrap_or_default(),
                username: token_data.claims.username,
                permissions: token_data.claims.permissions,
                roles: token_data.claims.roles,
                organization_id: token_data.claims.org,
                project_access: token_data.claims.projects,
                session_id: token_data.claims.session_id,
                issued_at: DateTime::from_timestamp(token_data.claims.iat as i64, 0)
                    .unwrap_or_default(),
                expires_at: DateTime::from_timestamp(token_data.claims.exp as i64, 0)
                    .unwrap_or_default(),
            };

            let rate_limit_manager = RateLimitManager::new();
            let user_tier = if auth_context.roles.contains(&"premium".to_string()) {
                "premium"
            } else {
                "user"
            };
            rate_limit_manager.check_rate_limit(user_tier, "api_request")?;

            req.extensions_mut().insert(auth_context);
        }
    }

    Ok(next.run(req).await)
}

async fn api_key_auth(api_key: &str, mut req: Request, next: Next) -> Result<Response, StatusCode> {
    // In a real application, you would look up the API key in a database.
    // For this example, we'll use a hardcoded key.
    if api_key == "test-api-key" {
        let auth_context = AuthContext {
            user_id: Uuid::nil(),
            username: "service-account".to_string(),
            permissions: vec![Permission::READ_CODE, Permission::READ_GRAPH],
            roles: vec!["service".to_string()],
            organization_id: None,
            project_access: vec![],
            session_id: "".to_string(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(365),
        };
        req.extensions_mut().insert(auth_context);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

impl ToString for Permission {
    fn to_string(&self) -> String {
        match self {
            Permission::READ_CODE => "read:code".to_string(),
            Permission::READ_GRAPH => "read:graph".to_string(),
            Permission::READ_METRICS => "read:metrics".to_string(),
            Permission::READ_ANALYSIS => "read:analysis".to_string(),
            Permission::WRITE_ANNOTATIONS => "write:annotations".to_string(),
            Permission::MANAGE_CACHE => "manage:cache".to_string(),
            Permission::ADMIN_SYSTEM => "admin:system".to_string(),
            Permission::SUBSCRIBE_UPDATES => "subscribe:updates".to_string(),
        }
    }
}

use governor::clock::QuantaClock;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
    NODE,
    RELATION,
    SUBGRAPH,
    PROJECT,
    REPOSITORY,
}

pub struct RateLimitManager {
    limiters: HashMap<String, Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>>,
}

impl RateLimitManager {
    pub fn new() -> Self {
        let mut limiters = HashMap::new();
        limiters.insert(
            "user".to_string(),
            Arc::new(RateLimiter::direct(governor::Quota::per_hour(
                NonZeroU32::new(1000).unwrap(),
            ))),
        );
        limiters.insert(
            "premium".to_string(),
            Arc::new(RateLimiter::direct(governor::Quota::per_hour(
                NonZeroU32::new(5000).unwrap(),
            ))),
        );
        limiters.insert(
            "complex_query".to_string(),
            Arc::new(RateLimiter::direct(governor::Quota::per_hour(
                NonZeroU32::new(100).unwrap(),
            ))),
        );
        Self { limiters }
    }

    pub fn get_limiter(
        &self,
        user_tier: &str,
        operation: &str,
    ) -> Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>> {
        let limiter_key = if operation.starts_with("subscribe") {
            "subscription"
        } else if user_tier == "premium" {
            "premium"
        } else {
            "user"
        };
        self.limiters.get(limiter_key).unwrap().clone()
    }

    pub fn check_rate_limit(&self, user_tier: &str, operation: &str) -> Result<(), StatusCode> {
        let limiter = self.get_limiter(user_tier, operation);
        if limiter.check().is_err() {
            Err(StatusCode::TOO_MANY_REQUESTS)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiKey {
    pub key: String,
    pub permissions: Vec<Permission>,
}

lazy_static! {
    static ref PERMISSION_HIERARCHY: HashMap<Permission, Vec<Permission>> = {
        let mut m = HashMap::new();
        m.insert(
            Permission::ADMIN_SYSTEM,
            vec![
                Permission::READ_CODE,
                Permission::READ_GRAPH,
                Permission::READ_METRICS,
                Permission::READ_ANALYSIS,
                Permission::WRITE_ANNOTATIONS,
                Permission::MANAGE_CACHE,
                Permission::SUBSCRIBE_UPDATES,
            ],
        );
        m.insert(
            Permission::READ_ANALYSIS,
            vec![
                Permission::READ_CODE,
                Permission::READ_GRAPH,
                Permission::READ_METRICS,
            ],
        );
        m.insert(Permission::READ_GRAPH, vec![Permission::READ_CODE]);
        m
    };
}

pub struct AuthorizationEngine;

impl AuthorizationEngine {
    pub fn has_permission(context: &AuthContext, required: &[Permission]) -> bool {
        required.iter().all(|req| {
            context.permissions.contains(req)
                || context.permissions.iter().any(|p| {
                    PERMISSION_HIERARCHY
                        .get(p)
                        .map_or(false, |perms| perms.contains(req))
                })
        })
    }

    pub fn check_resource_access(
        context: &AuthContext,
        resource_type: &ResourceType,
        resource_id: &str,
        required_permissions: &[Permission],
    ) -> bool {
        if !Self::has_permission(context, required_permissions) {
            return false;
        }

        match resource_type {
            ResourceType::PROJECT => {
                if let Some(org_id) = &context.organization_id {
                    if resource_id.starts_with(&format!("{}:", org_id)) {
                        return context.project_access.contains(&resource_id.to_string());
                    }
                }
                false
            }
            _ => true,
        }
    }
}
