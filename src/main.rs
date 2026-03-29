//! Alexandria Nexus — Bibliography and knowledge engine for Philosophie.ch

use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Required environment configuration.
///
/// All fields are mandatory — the app won't start with missing config.
/// DATABASE_URL is the connection string for the app.
/// DB-specific vars (POSTGRES_USER, etc.) are for docker-compose only.
struct Config {
    database_url: String,
    host: String,
    port: String,
    allowed_origins: String,
    /// Optional: seed an admin API key on startup
    seed_api_key: Option<String>,
    /// Required when seed_api_key is set
    seed_api_key_name: Option<String>,
}

impl Config {
    /// Parse all required environment variables at once.
    /// Reports ALL missing variables in a single error, not one at a time.
    fn from_env() -> Result<Self, String> {
        let required = ["DATABASE_URL", "HOST", "PORT", "ALLOWED_ORIGINS"];

        let mut missing: Vec<&str> = Vec::new();
        for var in &required {
            if std::env::var(var).is_err() {
                missing.push(var);
            }
        }

        // Check conditional requirement: SEED_API_KEY_NAME when SEED_API_KEY is set
        let seed_key = std::env::var("SEED_API_KEY").ok().filter(|s| !s.is_empty());
        let seed_name = std::env::var("SEED_API_KEY_NAME")
            .ok()
            .filter(|s| !s.is_empty());

        if seed_key.is_some() && seed_name.is_none() {
            missing.push("SEED_API_KEY_NAME (required when SEED_API_KEY is set)");
        }

        if !missing.is_empty() {
            return Err(format!(
                "Missing required environment variables:\n{}",
                missing
                    .iter()
                    .map(|v| format!("  - {v}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        Ok(Self {
            database_url: std::env::var("DATABASE_URL").expect("validated above"),
            host: std::env::var("HOST").expect("validated above"),
            port: std::env::var("PORT").expect("validated above"),
            allowed_origins: std::env::var("ALLOWED_ORIGINS").expect("validated above"),
            seed_api_key: seed_key,
            seed_api_key_name: seed_name,
        })
    }

    fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn cors(&self) -> hexforge::CorsConfig {
        let origins: Vec<&str> = self.allowed_origins.split(',').map(|s| s.trim()).collect();
        hexforge::CorsConfig::origins(origins)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tracing (RUST_LOG is the only variable with a fallback — it's dev tooling)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "alexandria_nexus=debug,hexforge=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse config — fails with ALL missing vars at once
    let config = Config::from_env().map_err(|e| {
        eprintln!("{e}");
        e
    })?;

    // Database
    let pool = hexforge::DatabaseConfig::new(&config.database_url)
        .max_connections(50)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .max_lifetime(Duration::from_secs(1800))
        .idle_timeout(Duration::from_secs(600))
        .stream_timeout(Duration::from_secs(300))
        .transaction_timeout(Duration::from_secs(30))
        .connect()
        .await?;

    tracing::info!("Connected to database");

    // Migrations
    tracing::info!("Running database migrations...");
    hexforge::migrate!("./migrations").run(pool.pool()).await?;
    tracing::info!("Migrations complete");

    // Seed API key (optional)
    if let Some(ref seed_key) = config.seed_api_key {
        let seed_name = config.seed_api_key_name.as_ref().expect("validated above");
        let key_hash = alexandria_nexus::auth::hash_api_key(seed_key);
        hexforge::db_exports::query(
            r#"
            INSERT INTO api_keys (key_hash, name, permission)
            VALUES ($1, $2, 'admin'::permission_level)
            ON CONFLICT (key_hash) DO NOTHING
            "#,
        )
        .bind(&key_hash)
        .bind(seed_name)
        .execute(pool.pool())
        .await?;
        tracing::info!("Seeded API key: {}", seed_name);
    }

    // Build and serve
    let app = alexandria_nexus::build_app(pool, config.cors());

    hexforge::serve(app, &config.addr()).await?;

    Ok(())
}
