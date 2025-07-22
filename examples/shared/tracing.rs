use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Setup tracing based on environment or explicit configuration
/// 
/// Priority (first match wins):
/// 1. If `force_level` provided - use it
/// 2. If RUST_LOG_DISABLE=1 - disable tracing completely
/// 3. If RUST_LOG set - use environment configuration
/// 4. Otherwise - no tracing (silent)
/// 
/// # Arguments
/// * `force_level` - Optional explicit level ("debug", "info", "warn", "error")
/// 
/// # Examples
/// ```bash
/// # Environment-driven (standard approach)
/// RUST_LOG=debug cargo run --example hello_world
/// 
/// # Force disable even if RUST_LOG is set
/// RUST_LOG_DISABLE=1 cargo run --example hello_world
/// 
/// # Silent by default
/// cargo run --example hello_world
/// ```
pub fn setup(force_level: Option<&str>) {
    // Load .env files first to make RUST_LOG available
    load_env_files();
    
    // Check for explicit disable flag
    if std::env::var("RUST_LOG_DISABLE").is_ok() {
        return; // No tracing
    }
    
    // Determine the filter to use
    let filter = if let Some(level) = force_level {
        // Explicit level provided
        tracing_subscriber::EnvFilter::new(level)
    } else if std::env::var("RUST_LOG").is_ok() {
        // Use environment configuration
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into())
    } else {
        // No configuration - stay silent
        return;
    };
    
    // Initialize tracing with consistent formatting
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .compact(),
        )
        .init();
}

/// Setup tracing with specific level (convenience function)
/// 
/// Equivalent to `setup(Some(level))`
/// 
/// # Arguments
/// * `level` - Tracing level: "trace", "debug", "info", "warn", "error"
pub fn setup_with_level(level: &str) {
    setup(Some(level));
}

/// Load .env files with explicit paths
/// 
/// Loads configuration from .env files in the same order as config module.
/// This ensures consistency between tracing and config loading.
fn load_env_files() {
    dotenv::from_filename("examples/.env").ok();
    if std::path::Path::new("examples/.env.local").exists() {
        dotenv::from_filename("examples/.env.local").ok();
    }
}
