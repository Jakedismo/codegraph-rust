use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use tower_http::add_extension::AddExtensionLayer;
use tower::ServiceBuilder;
use codegraph_core::security::{AuthContext, JwtManager, ApiKeyManager, SecurityLogger, SecurityEvent, AuthorizationEngine};
use uuid::Uuid;
use std::net::IpAddr;

/// Security headers middleware
pub async fn security_headers_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;
    
    let headers = response.headers_mut();
    
    // Strict Transport Security (HSTS)
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );
    
    // Content Security Policy
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' 'unsafe-eval'; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: https:; \
             font-src 'self' https:; \
             connect-src 'self'; \
             media-src 'none'; \
             object-src 'none'; \
             child-src 'none'; \
             worker-src 'none'; \
             frame-ancestors 'none'; \
             base-uri 'self'; \
             form-action 'self'"
        ),
    );
    
    // X-Frame-Options
    headers.insert(
        "X-Frame-Options".parse().unwrap(),
        HeaderValue::from_static("DENY"),
    );
    
    // X-Content-Type-Options
    headers.insert(
        "X-Content-Type-Options".parse().unwrap(),
        HeaderValue::from_static("nosniff"),
    );
    
    // X-XSS-Protection
    headers.insert(
        "X-XSS-Protection".parse().unwrap(),
        HeaderValue::from_static("1; mode=block"),
    );
    
    // Referrer-Policy
    headers.insert(
        "Referrer-Policy".parse().unwrap(),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    
    // X-Permitted-Cross-Domain-Policies
    headers.insert(
        "X-Permitted-Cross-Domain-Policies".parse().unwrap(),
        HeaderValue::from_static("none"),
    );
    
    // Cache-Control for sensitive endpoints
    if let Some(path) = response.extensions().get::<String>() {
        if path.contains("admin") || path.contains("auth") {
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store, no-cache, must-revalidate, private"),
            );
        }
    }
    
    Ok(response)
}

/// Authentication middleware with security logging
pub async fn auth_middleware(
    State(state): State<Arc<crate::AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let client_ip = extract_client_ip(&req);
    let path = req.uri().path().to_string();
    
    // Check for API key authentication first
    if let Some(api_key) = req.headers().get("X-API-KEY").and_then(|v| v.to_str().ok()) {
        return api_key_auth(api_key, &mut req, next, &state, &client_ip).await;
    }

    // Then check for JWT Bearer token
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
        match state.jwt_manager.validate_token(&token) {
            Ok(auth_context) => {
                // Log successful authentication
                SecurityLogger::log_event(SecurityEvent::AuthenticationSuccess {
                    user_id: auth_context.user_id,
                    ip_address: client_ip,
                    method: "JWT".to_string(),
                });
                
                // Check token expiry
                if auth_context.expires_at <= chrono::Utc::now() {
                    SecurityLogger::log_event(SecurityEvent::AuthenticationFailure {
                        username: auth_context.username.clone(),
                        ip_address: client_ip,
                        reason: "Token expired".to_string(),
                    });
                    return Err(StatusCode::UNAUTHORIZED);
                }
                
                req.extensions_mut().insert(auth_context);
                req.extensions_mut().insert(path); // For cache control headers
                Ok(next.run(req).await)
            }
            Err(e) => {
                SecurityLogger::log_event(SecurityEvent::AuthenticationFailure {
                    username: "unknown".to_string(),
                    ip_address: client_ip,
                    reason: format!("Invalid JWT: {:?}", e),
                });
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    } else {
        // Allow unauthenticated access to public endpoints
        let public_endpoints = ["/health", "/metrics", "/docs", "/graphiql"];
        if public_endpoints.iter().any(|&endpoint| path.starts_with(endpoint)) {
            Ok(next.run(req).await)
        } else {
            SecurityLogger::log_event(SecurityEvent::AuthenticationFailure {
                username: "anonymous".to_string(),
                ip_address: client_ip,
                reason: "No authentication provided".to_string(),
            });
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

async fn api_key_auth(
    api_key: &str,
    req: &mut Request,
    next: Next,
    state: &crate::AppState,
    client_ip: &str,
) -> Result<Response, StatusCode> {
    match state.api_key_manager.validate_key(api_key) {
        Some(key_info) => {
            let auth_context = AuthContext {
                user_id: key_info.id,
                username: format!("api-key-{}", key_info.name),
                permissions: key_info.permissions,
                roles: vec!["api-key".to_string()],
                organization_id: None,
                project_access: vec![],
                session_id: key_info.id.to_string(),
                issued_at: key_info.created_at,
                expires_at: key_info.expires_at.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(365)),
            };
            
            SecurityLogger::log_event(SecurityEvent::AuthenticationSuccess {
                user_id: auth_context.user_id,
                ip_address: client_ip.to_string(),
                method: "API_KEY".to_string(),
            });
            
            req.extensions_mut().insert(auth_context);
            Ok(next.run(req).await)
        }
        None => {
            SecurityLogger::log_event(SecurityEvent::AuthenticationFailure {
                username: "api-key-unknown".to_string(),
                ip_address: client_ip.to_string(),
                reason: "Invalid API key".to_string(),
            });
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Rate limiting middleware
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    RateLimiter, Quota,
};
use std::{collections::HashMap, num::NonZeroU32, sync::Mutex};

pub struct RateLimitManager {
    limiters: Mutex<HashMap<String, Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>,
}

impl RateLimitManager {
    pub fn new() -> Self {
        Self {
            limiters: Mutex::new(HashMap::new()),
        }
    }
    
    fn get_limiter(&self, tier: &str) -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let mut limiters = self.limiters.lock().unwrap();
        
        if let Some(limiter) = limiters.get(tier) {
            return limiter.clone();
        }
        
        let quota = match tier {
            "anonymous" => Quota::per_minute(NonZeroU32::new(60).unwrap()),
            "user" => Quota::per_minute(NonZeroU32::new(1000).unwrap()),
            "premium" => Quota::per_minute(NonZeroU32::new(5000).unwrap()),
            "admin" => Quota::per_minute(NonZeroU32::new(10000).unwrap()),
            _ => Quota::per_minute(NonZeroU32::new(60).unwrap()),
        };
        
        let limiter = Arc::new(RateLimiter::direct(quota));
        limiters.insert(tier.to_string(), limiter.clone());
        limiter
    }
    
    pub fn check_rate_limit(&self, tier: &str) -> Result<(), StatusCode> {
        let limiter = self.get_limiter(tier);
        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => Err(StatusCode::TOO_MANY_REQUESTS),
        }
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<crate::AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_context = req.extensions().get::<AuthContext>();
    let client_ip = extract_client_ip(&req);
    
    let user_tier = auth_context
        .map(|ctx| {
            if ctx.permissions.contains(&codegraph_core::security::Permission::ADMIN_SYSTEM) {
                "admin"
            } else if ctx.roles.contains(&"premium".to_string()) {
                "premium"
            } else {
                "user"
            }
        })
        .unwrap_or("anonymous");
    
    match state.rate_limiter.check_rate_limit(user_tier) {
        Ok(_) => Ok(next.run(req).await),
        Err(status) => {
            SecurityLogger::log_event(SecurityEvent::SuspiciousActivity {
                user_id: auth_context.map(|ctx| ctx.user_id),
                ip_address: client_ip,
                description: format!("Rate limit exceeded for tier: {}", user_tier),
            });
            
            // Add rate limit headers
            let mut response = Response::new(axum::body::Body::from("Rate limit exceeded"));
            *response.status_mut() = status;
            
            response.headers_mut().insert(
                "X-RateLimit-Limit",
                HeaderValue::from_str(&format!("{}", get_limit_for_tier(user_tier))).unwrap(),
            );
            response.headers_mut().insert(
                "X-RateLimit-Remaining", 
                HeaderValue::from_static("0"),
            );
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_static("60"),
            );
            
            Ok(response)
        }
    }
}

fn get_limit_for_tier(tier: &str) -> u32 {
    match tier {
        "anonymous" => 60,
        "user" => 1000,
        "premium" => 5000,
        "admin" => 10000,
        _ => 60,
    }
}

/// Permission checking middleware
pub async fn require_permission(
    required_permissions: Vec<codegraph_core::security::Permission>
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>> {
    move |req: Request, next: Next| {
        let required_perms = required_permissions.clone();
        Box::pin(async move {
            let auth_context = req.extensions().get::<AuthContext>();
            let client_ip = extract_client_ip(&req);
            
            match auth_context {
                Some(ctx) => {
                    if AuthorizationEngine::has_permission(ctx, &required_perms) {
                        // Log admin access for sensitive operations
                        if required_perms.contains(&codegraph_core::security::Permission::ADMIN_SYSTEM) {
                            SecurityLogger::log_event(SecurityEvent::AdminAccess {
                                user_id: ctx.user_id,
                                action: req.method().to_string(),
                                resource: req.uri().path().to_string(),
                            });
                        }
                        Ok(next.run(req).await)
                    } else {
                        SecurityLogger::log_event(SecurityEvent::PermissionDenied {
                            user_id: ctx.user_id,
                            resource: req.uri().path().to_string(),
                            required_permission: format!("{:?}", required_perms),
                        });
                        Err(StatusCode::FORBIDDEN)
                    }
                }
                None => {
                    SecurityLogger::log_event(SecurityEvent::PermissionDenied {
                        user_id: Uuid::nil(),
                        resource: req.uri().path().to_string(),
                        required_permission: format!("{:?}", required_perms),
                    });
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        })
    }
}

/// Extract client IP address from request
fn extract_client_ip(req: &Request) -> String {
    // Check X-Forwarded-For header (behind reverse proxy)
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip) = forwarded_str.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    
    // Check X-Real-IP header
    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    
    // Fallback to connection info (may not be available in all cases)
    "unknown".to_string()
}

/// CORS configuration for security
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any) // In production, specify allowed origins
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            "X-API-KEY".parse().unwrap(),
            "X-Request-ID".parse().unwrap(),
        ])
        .expose_headers([
            "X-RateLimit-Limit".parse().unwrap(),
            "X-RateLimit-Remaining".parse().unwrap(),
            "X-Request-ID".parse().unwrap(),
        ])
        .max_age(std::time::Duration::from_secs(86400))
        .allow_credentials(true)
}

/// Security middleware stack builder
pub fn security_middleware_stack() -> ServiceBuilder<
    tower::layer::util::Stack<
        tower::layer::util::Stack<
            axum::middleware::FromFnLayer<
                fn(Request, Next) -> impl std::future::Future<Output = Result<Response, StatusCode>>
            >,
            tower_http::cors::CorsLayer
        >,
        axum::middleware::FromFnLayer<
            fn(Request, Next) -> impl std::future::Future<Output = Result<Response, StatusCode>>
        >
    >
> {
    ServiceBuilder::new()
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(cors_layer())
        .layer(axum::middleware::from_fn(rate_limit_middleware))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use axum::body::Body;
    
    #[test]
    fn test_rate_limit_manager() {
        let manager = RateLimitManager::new();
        
        // Should allow first request
        assert_eq!(manager.check_rate_limit("user"), Ok(()));
        
        // Test different tiers have different limits
        assert_eq!(get_limit_for_tier("anonymous"), 60);
        assert_eq!(get_limit_for_tier("user"), 1000);
        assert_eq!(get_limit_for_tier("premium"), 5000);
        assert_eq!(get_limit_for_tier("admin"), 10000);
    }
    
    #[test]
    fn test_client_ip_extraction() {
        let mut req = Request::builder()
            .uri("/test")
            .header("X-Forwarded-For", "192.168.1.1, 10.0.0.1")
            .body(Body::empty())
            .unwrap();
            
        assert_eq!(extract_client_ip(&req), "192.168.1.1");
        
        let mut req = Request::builder()
            .uri("/test")
            .header("X-Real-IP", "192.168.1.2")
            .body(Body::empty())
            .unwrap();
            
        assert_eq!(extract_client_ip(&req), "192.168.1.2");
    }
}