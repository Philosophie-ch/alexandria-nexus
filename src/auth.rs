//! API key authentication for the bibliography API.
//!
//! Implements hexforge's TokenValidator trait for API key authentication.
//! API keys are stored as SHA-256 hashes in the database.

use std::collections::HashMap;

use hexforge::{
    DatabasePool,
    async_exports::async_trait,
    contracts::{AuthContext, Permission, TokenValidator},
};
use sha2::{Digest, Sha256};

/// Hash an API key using SHA-256, returning a hex-encoded digest.
///
/// This is used both when storing keys (seed/insert) and when validating
/// incoming tokens (lookup). The plaintext key is never stored in the database.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// API key record with only the fields we need.
#[derive(Debug, Clone)]
struct ApiKeyRecord {
    id: i64,
    permission: String,
}

/// API key validator that checks tokens against the database.
#[derive(Debug, Clone)]
pub struct ApiKeyValidator {
    pool: DatabasePool,
}

impl ApiKeyValidator {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenValidator for ApiKeyValidator {
    async fn validate(&self, token: &str) -> Option<AuthContext> {
        let key_hash = hash_api_key(token);

        let row: Option<(i64, String)> = hexforge::db_exports::query_as(
            "SELECT id, permission::text FROM api_keys WHERE key_hash = $1 AND revoked_at IS NULL",
        )
        .bind(&key_hash)
        .fetch_optional(self.pool.pool())
        .await
        .ok()?;

        row.map(|(id, permission)| {
            let record = ApiKeyRecord { id, permission };
            AuthContext {
                permission: match record.permission.as_str() {
                    "admin" => Permission::Admin,
                    "write" => Permission::Write,
                    "read" => Permission::Read,
                    _ => Permission::Public,
                },
                token_id: Some(record.id.to_string()),
                claims: HashMap::new(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mapping() {
        let map_permission = |s: &str| -> Permission {
            match s {
                "admin" => Permission::Admin,
                "write" => Permission::Write,
                "read" => Permission::Read,
                _ => Permission::Public,
            }
        };

        assert!(matches!(map_permission("admin"), Permission::Admin));
        assert!(matches!(map_permission("write"), Permission::Write));
        assert!(matches!(map_permission("read"), Permission::Read));
        assert!(matches!(map_permission("public"), Permission::Public));
        assert!(matches!(map_permission("unknown"), Permission::Public));
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let hash1 = hash_api_key("test-key");
        let hash2 = hash_api_key("test-key");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_api_key_different_inputs() {
        let hash1 = hash_api_key("key-a");
        let hash2 = hash_api_key("key-b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_api_key_is_hex() {
        let hash = hash_api_key("test-key");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
