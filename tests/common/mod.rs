//! Common test infrastructure for integration tests.
//!
//! Each test spawns its own PostgreSQL container via testcontainers,
//! runs migrations, seeds an admin API key, and starts the app on a random port.

#![allow(dead_code)]

use std::net::SocketAddr;

use reqwest::{Client, Response};
use sqlx::postgres::PgPoolOptions;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

use alexandria_nexus::auth::hash_api_key;
use alexandria_nexus::build_app;
use hexforge::{CorsConfig, DatabasePool};

/// Test API keys
const TEST_ADMIN_KEY: &str = "test-admin-key-12345";

/// A self-contained test application with its own Postgres database.
pub struct TestApp {
    pub addr: SocketAddr,
    pub client: Client,
    pub api_key: String,
    _db: ContainerAsync<Postgres>,
}

impl TestApp {
    /// Spawn a fresh test application with permissive CORS (suitable for most tests).
    pub async fn spawn() -> Self {
        Self::spawn_with_cors(CorsConfig::permissive()).await
    }

    /// Spawn a fresh test application with a specific CORS configuration.
    pub async fn spawn_with_cors(cors: CorsConfig) -> Self {
        // 1. Start testcontainer Postgres
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let host = container
            .get_host()
            .await
            .expect("Failed to get container host");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get container port");

        let database_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

        // 2. Connect and run migrations
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // 3. Seed admin API key
        let key_hash = hash_api_key(TEST_ADMIN_KEY);
        sqlx::query(
            r#"INSERT INTO api_keys (key_hash, name, permission)
               VALUES ($1, 'Test Admin Key', 'admin'::permission_level)
               ON CONFLICT (key_hash) DO NOTHING"#,
        )
        .bind(&key_hash)
        .execute(&pool)
        .await
        .expect("Failed to seed admin API key");

        // 4. Build app
        let db_pool: DatabasePool = pool.into();
        let app = build_app(db_pool, cors, 100);

        // 5. Bind to random port and spawn
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to random port");
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Brief delay to let server start accepting connections
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // 6. Build client
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(0)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            addr,
            client,
            api_key: TEST_ADMIN_KEY.to_string(),
            _db: container,
        }
    }

    /// Build the full URL for a path.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// GET request with authentication.
    pub async fn get(&self, path: &str) -> Response {
        self.client
            .get(self.url(path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .expect("Failed to send GET request")
    }

    /// POST request with JSON body and authentication.
    pub async fn post_json<T: serde::Serialize>(&self, path: &str, body: &T) -> Response {
        self.client
            .post(self.url(path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(body)
            .send()
            .await
            .expect("Failed to send POST request")
    }

    /// PUT request with JSON body and authentication.
    pub async fn put_json<T: serde::Serialize>(&self, path: &str, body: &T) -> Response {
        self.client
            .put(self.url(path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(body)
            .send()
            .await
            .expect("Failed to send PUT request")
    }

    /// DELETE request with authentication.
    pub async fn delete(&self, path: &str) -> Response {
        self.client
            .delete(self.url(path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .expect("Failed to send DELETE request")
    }

    /// GET request without authentication.
    pub async fn get_no_auth(&self, path: &str) -> Response {
        self.client
            .get(self.url(path))
            .send()
            .await
            .expect("Failed to send unauthenticated GET request")
    }

    /// POST request without authentication.
    pub async fn post_json_no_auth<T: serde::Serialize>(&self, path: &str, body: &T) -> Response {
        self.client
            .post(self.url(path))
            .json(body)
            .send()
            .await
            .expect("Failed to send unauthenticated POST request")
    }

    /// GET request with a custom bearer token.
    pub async fn get_with_token(&self, path: &str, token: &str) -> Response {
        self.client
            .get(self.url(path))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("Failed to send GET request with custom token")
    }
}

/// Generate a unique suffix for test data using UUID.
pub fn unique_suffix() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}
