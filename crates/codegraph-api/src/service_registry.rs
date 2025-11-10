use crate::{ApiError, ApiResult};
use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceRegistration {
    pub service_id: String,
    pub service_name: String,
    pub version: String,
    pub address: String,
    pub port: u16,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub health_check_url: Option<String>,
    pub ttl_seconds: Option<u64>,
    pub registered_at: u64,
    pub last_heartbeat: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceRegistrationRequest {
    pub service_name: String,
    pub version: String,
    pub address: String,
    pub port: u16,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub health_check_url: Option<String>,
    pub ttl_seconds: Option<u64>,
}

#[derive(Serialize, Debug)]
pub struct ServiceRegistrationResponse {
    pub service_id: String,
    pub message: String,
    pub expires_at: Option<u64>,
}

#[derive(Serialize, Debug)]
pub struct ServiceDiscoveryResponse {
    pub services: Vec<ServiceRegistration>,
    pub total: usize,
}

#[derive(Deserialize)]
pub struct ServiceQuery {
    pub service_name: Option<String>,
    pub tag: Option<String>,
    pub healthy: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub service_id: String,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub success: bool,
    pub message: String,
    pub next_heartbeat_in: Option<u64>,
}

pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<String, ServiceRegistration>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        let registry = Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start background cleanup task
        let services_clone = registry.services.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                Self::cleanup_expired_services(&services_clone).await;
            }
        });

        registry
    }

    async fn cleanup_expired_services(
        services: &Arc<RwLock<HashMap<String, ServiceRegistration>>>,
    ) {
        let now = current_timestamp();
        let mut services_guard = services.write().await;

        let expired_services: Vec<String> = services_guard
            .iter()
            .filter_map(|(id, service)| {
                if let Some(ttl) = service.ttl_seconds {
                    if now > service.last_heartbeat + ttl {
                        Some(id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for service_id in expired_services {
            services_guard.remove(&service_id);
            tracing::info!("Removed expired service: {}", service_id);
        }
    }

    pub async fn register_service(
        &self,
        request: ServiceRegistrationRequest,
    ) -> ApiResult<ServiceRegistrationResponse> {
        let service_id = format!(
            "{}-{}-{}",
            request.service_name, request.address, request.port
        );

        let now = current_timestamp();
        let expires_at = request.ttl_seconds.map(|ttl| now + ttl);

        let registration = ServiceRegistration {
            service_id: service_id.clone(),
            service_name: request.service_name,
            version: request.version,
            address: request.address,
            port: request.port,
            tags: request.tags.unwrap_or_default(),
            metadata: request.metadata.unwrap_or_default(),
            health_check_url: request.health_check_url,
            ttl_seconds: request.ttl_seconds,
            registered_at: now,
            last_heartbeat: now,
        };

        let address = registration.address.clone();
        let port = registration.port;

        let mut services = self.services.write().await;
        services.insert(service_id.clone(), registration);

        tracing::info!("Registered service: {} ({}:{})", service_id, address, port);

        Ok(ServiceRegistrationResponse {
            service_id,
            message: "Service registered successfully".to_string(),
            expires_at,
        })
    }

    pub async fn heartbeat(&self, service_id: &str) -> ApiResult<HeartbeatResponse> {
        let mut services = self.services.write().await;

        if let Some(service) = services.get_mut(service_id) {
            service.last_heartbeat = current_timestamp();

            let next_heartbeat_in = service.ttl_seconds.map(|ttl| ttl / 2);

            Ok(HeartbeatResponse {
                success: true,
                message: "Heartbeat recorded".to_string(),
                next_heartbeat_in,
            })
        } else {
            Err(ApiError::NotFound(format!(
                "Service {} not found",
                service_id
            )))
        }
    }

    pub async fn deregister_service(&self, service_id: &str) -> ApiResult<()> {
        let mut services = self.services.write().await;

        if services.remove(service_id).is_some() {
            tracing::info!("Deregistered service: {}", service_id);
            Ok(())
        } else {
            Err(ApiError::NotFound(format!(
                "Service {} not found",
                service_id
            )))
        }
    }

    pub async fn discover_services(
        &self,
        query: ServiceQuery,
    ) -> ApiResult<ServiceDiscoveryResponse> {
        let services = self.services.read().await;

        let mut filtered_services: Vec<ServiceRegistration> = services
            .values()
            .filter(|service| {
                // Filter by service name
                if let Some(ref name) = query.service_name {
                    if service.service_name != *name {
                        return false;
                    }
                }

                // Filter by tag
                if let Some(ref tag) = query.tag {
                    if !service.tags.contains(tag) {
                        return false;
                    }
                }

                // Filter by health (simplified - would need actual health checks)
                if let Some(healthy) = query.healthy {
                    if healthy && service.health_check_url.is_none() {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by registration time (newest first)
        filtered_services.sort_by(|a, b| b.registered_at.cmp(&a.registered_at));

        // Apply limit
        if let Some(limit) = query.limit {
            filtered_services.truncate(limit);
        }

        let total = filtered_services.len();

        Ok(ServiceDiscoveryResponse {
            services: filtered_services,
            total,
        })
    }

    pub async fn get_service(&self, service_id: &str) -> ApiResult<ServiceRegistration> {
        let services = self.services.read().await;

        services
            .get(service_id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound(format!("Service {} not found", service_id)))
    }

    pub async fn list_all_services(&self) -> Vec<ServiceRegistration> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// HTTP Handlers that work with AppState
use crate::AppState;
use axum::extract::{Path, State};

pub async fn register_service_handler(
    State(state): State<AppState>,
    Json(request): Json<ServiceRegistrationRequest>,
) -> ApiResult<Json<ServiceRegistrationResponse>> {
    let response = state.service_registry.register_service(request).await?;
    Ok(Json(response))
}

pub async fn heartbeat_handler(
    State(state): State<AppState>,
    Json(request): Json<HeartbeatRequest>,
) -> ApiResult<Json<HeartbeatResponse>> {
    let response = state
        .service_registry
        .heartbeat(&request.service_id)
        .await?;
    Ok(Json(response))
}

pub async fn deregister_service_handler(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    state
        .service_registry
        .deregister_service(&service_id)
        .await?;
    Ok(Json(serde_json::json!({
        "message": format!("Service {} deregistered successfully", service_id)
    })))
}

pub async fn discover_services_handler(
    State(state): State<AppState>,
    Query(query): Query<ServiceQuery>,
) -> ApiResult<Json<ServiceDiscoveryResponse>> {
    let response = state.service_registry.discover_services(query).await?;
    Ok(Json(response))
}

pub async fn get_service_handler(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
) -> ApiResult<Json<ServiceRegistration>> {
    let service = state.service_registry.get_service(&service_id).await?;
    Ok(Json(service))
}

pub async fn list_services_handler(
    State(state): State<AppState>,
) -> Json<ServiceDiscoveryResponse> {
    let services = state.service_registry.list_all_services().await;
    let total = services.len();

    Json(ServiceDiscoveryResponse { services, total })
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_registration() {
        let registry = ServiceRegistry::new();

        let request = ServiceRegistrationRequest {
            service_name: "test-service".to_string(),
            version: "1.0.0".to_string(),
            address: "127.0.0.1".to_string(),
            port: 8080,
            tags: Some(vec!["http".to_string(), "api".to_string()]),
            metadata: None,
            health_check_url: Some("http://127.0.0.1:8080/health".to_string()),
            ttl_seconds: Some(60),
        };

        let response = registry.register_service(request).await.unwrap();
        assert!(!response.service_id.is_empty());
        assert!(response.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_service_discovery() {
        let registry = ServiceRegistry::new();

        // Register a test service
        let request = ServiceRegistrationRequest {
            service_name: "test-service".to_string(),
            version: "1.0.0".to_string(),
            address: "127.0.0.1".to_string(),
            port: 8080,
            tags: Some(vec!["http".to_string()]),
            metadata: None,
            health_check_url: None,
            ttl_seconds: None,
        };

        registry.register_service(request).await.unwrap();

        // Discover services
        let query = ServiceQuery {
            service_name: Some("test-service".to_string()),
            tag: None,
            healthy: None,
            limit: None,
        };

        let response = registry.discover_services(query).await.unwrap();
        assert_eq!(response.services.len(), 1);
        assert_eq!(response.services[0].service_name, "test-service");
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let registry = ServiceRegistry::new();

        // Register a service with TTL
        let request = ServiceRegistrationRequest {
            service_name: "test-service".to_string(),
            version: "1.0.0".to_string(),
            address: "127.0.0.1".to_string(),
            port: 8080,
            tags: None,
            metadata: None,
            health_check_url: None,
            ttl_seconds: Some(60),
        };

        let reg_response = registry.register_service(request).await.unwrap();

        // Send heartbeat
        let heartbeat_response = registry.heartbeat(&reg_response.service_id).await.unwrap();
        assert!(heartbeat_response.success);
        assert!(heartbeat_response.next_heartbeat_in.is_some());
    }
}
