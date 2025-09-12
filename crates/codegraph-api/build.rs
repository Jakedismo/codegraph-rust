use std::env;

fn main() {
    // Emit rerun-if-changed directives for build script dependencies
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_LEAK_DETECT");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_OPENAPI_UI");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_GRAPHQL");
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_HTTP2");

    // Platform-specific configurations
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Configure platform-specific features
    configure_platform(&target_os, &target_arch);

    // Check and emit feature configurations
    configure_features();

    // Check for minimum required dependencies
    check_dependencies();

    // Set up compile-time optimizations
    configure_optimizations();
}

fn configure_platform(target_os: &str, target_arch: &str) {
    // Platform-specific configurations
    match target_os {
        "linux" => {
            println!("cargo::rustc-cfg=platform_linux");
            // Linux-specific optimizations
            if target_arch == "x86_64" {
                println!("cargo::rustc-cfg=linux_x64_optimized");
            }
        }
        "macos" => {
            println!("cargo::rustc-cfg=platform_macos");
            // macOS-specific configurations
            if target_arch == "aarch64" {
                println!("cargo::rustc-cfg=macos_arm64");
            }
        }
        "windows" => {
            println!("cargo::rustc-cfg=platform_windows");
            // Windows-specific configurations
            println!("cargo::rustc-link-arg=/STACK:4194304"); // 4MB stack for Windows
        }
        _ => {
            println!("cargo::warning=Unknown target OS: {}", target_os);
        }
    }

    // Architecture-specific configurations
    match target_arch {
        "x86_64" => {
            println!("cargo::rustc-cfg=arch_x64");
            println!("cargo::rustc-cfg=simd_available");
        }
        "aarch64" => {
            println!("cargo::rustc-cfg=arch_arm64");
            println!("cargo::rustc-cfg=simd_available");
        }
        "wasm32" => {
            println!("cargo::rustc-cfg=arch_wasm");
            println!("cargo::warning=WASM target detected - some features may be limited");
        }
        _ => {}
    }
}

fn configure_features() {
    // Check for feature flags and emit corresponding cfg values
    println!("cargo::rustc-check-cfg=cfg(has_leak_detect)");
    println!("cargo::rustc-check-cfg=cfg(has_openapi_ui)");
    println!("cargo::rustc-check-cfg=cfg(has_graphql)");
    println!("cargo::rustc-check-cfg=cfg(has_http2)");
    println!("cargo::rustc-check-cfg=cfg(platform_linux)");
    println!("cargo::rustc-check-cfg=cfg(platform_macos)");
    println!("cargo::rustc-check-cfg=cfg(platform_windows)");
    println!("cargo::rustc-check-cfg=cfg(arch_x64)");
    println!("cargo::rustc-check-cfg=cfg(arch_arm64)");
    println!("cargo::rustc-check-cfg=cfg(arch_wasm)");
    println!("cargo::rustc-check-cfg=cfg(simd_available)");
    println!("cargo::rustc-check-cfg=cfg(linux_x64_optimized)");
    println!("cargo::rustc-check-cfg=cfg(macos_arm64)");
    println!("cargo::rustc-check-cfg=cfg(optimized_build)");
    println!("cargo::rustc-check-cfg=cfg(has_all_features)");

    // Feature detection
    if env::var("CARGO_FEATURE_LEAK_DETECT").is_ok() {
        println!("cargo::rustc-cfg=has_leak_detect");
        println!("cargo::warning=Memory leak detection enabled - performance may be impacted");
    }

    if env::var("CARGO_FEATURE_OPENAPI_UI").is_ok() {
        println!("cargo::rustc-cfg=has_openapi_ui");
    }

    if env::var("CARGO_FEATURE_GRAPHQL").is_ok() {
        println!("cargo::rustc-cfg=has_graphql");
    }

    if env::var("CARGO_FEATURE_HTTP2").is_ok() {
        println!("cargo::rustc-cfg=has_http2");
    }

    // Check if all features are enabled
    let all_features = env::var("CARGO_FEATURE_LEAK_DETECT").is_ok()
        && env::var("CARGO_FEATURE_OPENAPI_UI").is_ok()
        && env::var("CARGO_FEATURE_GRAPHQL").is_ok()
        && env::var("CARGO_FEATURE_HTTP2").is_ok();

    if all_features {
        println!("cargo::rustc-cfg=has_all_features");
    }
}

fn check_dependencies() {
    // Check for required dependencies and their versions
    // This helps catch dependency issues at compile time

    // Check if we're building with workspace dependencies
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let workspace_root = std::path::Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent());

        if let Some(root) = workspace_root {
            if root.join("Cargo.toml").exists() {
                println!("cargo::rustc-cfg=workspace_build");
            }
        }
    }
}

fn configure_optimizations() {
    // Configure build optimizations based on profile
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    match profile.as_str() {
        "release" => {
            println!("cargo::rustc-cfg=optimized_build");
            // Enable release-specific optimizations
            println!("cargo::rustc-link-arg=-s"); // Strip symbols in release
        }
        "bench" => {
            println!("cargo::rustc-cfg=optimized_build");
            println!("cargo::rustc-cfg=bench_build");
        }
        _ => {
            // Debug build - no special optimizations
        }
    }

    // Set up linker optimizations for specific platforms
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if profile == "release" {
        match target_os.as_str() {
            "linux" => {
                // Linux linker optimizations
                println!("cargo::rustc-link-arg=-Wl,--gc-sections");
                println!("cargo::rustc-link-arg=-Wl,--as-needed");
            }
            "macos" => {
                // macOS linker optimizations
                println!("cargo::rustc-link-arg=-Wl,-dead_strip");
            }
            _ => {}
        }
    }
}
