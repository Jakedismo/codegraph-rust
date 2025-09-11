use codegraph_api::Server;
use codegraph_core::ConfigManager;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> codegraph_core::Result<()> {
    // Fast path for binary verification across platforms
    // Supports: --version | -V
    {
        let mut args = std::env::args();
        let _ = args.next(); // program name
        if let Some(flag) = args.next() {
            if flag == "--version" || flag == "-V" {
                let name = option_env!("CARGO_PKG_NAME").unwrap_or("codegraph-api");
                let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");
                let arch = option_env!("CARGO_CFG_TARGET_ARCH").unwrap_or("unknown-arch");
                let os = option_env!("CARGO_CFG_TARGET_OS").unwrap_or("unknown-os");
                let env = option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("");
                if env.is_empty() {
                    println!("{name} v{version} ({arch}-{os})");
                } else {
                    println!("{name} v{version} ({arch}-{os}-{env})");
                }
                return Ok(());
            }
        }
    }

    // Ensure leak guard exists for final shutdown reporting when enabled
    let _leak_guard = codegraph_api::leak_guard::LeakGuard::new();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codegraph_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration (env-aware) with hot-reload
    let config = ConfigManager::new_watching(None)?;
    let settings = config.settings().read().await.clone();

    // Bind address configurable via config or env override
    let host = std::env::var("HOST")
        .ok()
        .unwrap_or_else(|| settings.server.host.clone());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(settings.server.port);

    // If HOST is an IP string, construct directly; otherwise try full SocketAddr string
    let addr = SocketAddr::from_str(&format!("{}:{}", host, port))
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], port)));
    let server = Server::new(addr, config).await?;
    server.run().await
}
