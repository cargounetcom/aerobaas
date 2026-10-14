use std::collections::HashMap;

use aero_config::DatabaseConfig;
use aero_protocol::functions::Function;
use aero_protocol::keys::ApiKey;
use aero_protocol::{AeroError, AeroResult};
use parking_lot::RwLock;
use uuid::Uuid;

pub struct DbClient {
    #[allow(dead_code)] // read by the PostgreSQL backend (Phase 1)
    config: DatabaseConfig,
    keys: RwLock<HashMap<Uuid, ApiKey>>,
    key_hashes: RwLock<HashMap<String, Uuid>>,
    functions: RwLock<HashMap<Uuid, Function>>,
    collections: RwLock<HashMap<String, serde_json::Value>>,
}

impl DbClient {
    pub async fn new(config: &DatabaseConfig) -> anyhow::Result<Self> {
        tracing::info!(url = %config.url, namespace = %config.namespace, "Initializing database client");

        Ok(Self {
            config: config.clone(),
            keys: RwLock::new(HashMap::new()),
            key_hashes: RwLock::new(HashMap::new()),
            functions: RwLock::new(HashMap::new()),
            collections: RwLock::new(HashMap::new()),
        })
    }

    pub async fn store_key(&self, key: ApiKey) -> AeroResult<()> {
        let id = key.id;
        let hash = key.key_hash.clone();
        self.keys.write().insert(id, key);
        self.key_hashes.write().insert(hash, id);
        Ok(())
    }

    pub async fn get_key_by_hash(&self, hash: &str) -> AeroResult<Option<ApiKey>> {
        let id = self.key_hashes.read().get(hash).copied();
        Ok(id.and_then(|id| self.keys.read().get(&id).cloned()))
    }

    pub async fn get_key(&self, id: Uuid) -> AeroResult<Option<ApiKey>> {
        Ok(self.keys.read().get(&id).cloned())
    }

    pub async fn list_keys(&self) -> AeroResult<Vec<ApiKey>> {
        Ok(self.keys.read().values().cloned().collect())
    }

    pub async fn delete_key(&self, id: Uuid) -> AeroResult<()> {
        self.keys.write().remove(&id);
        self.key_hashes.write().retain(|_, v| *v != id);
        Ok(())
    }

    pub async fn decrement_credits(&self, key_id: Uuid, amount: i64) -> AeroResult<()> {
        let mut keys = self.keys.write();
        if let Some(key) = keys.get_mut(&key_id) {
            key.credits -= amount;
            if key.credits <= 0 {
                key.enabled = false;
                tracing::warn!(key_id = %key_id, "Key disabled — credits exhausted");
            }
        }
        Ok(())
    }

    pub async fn store_function(&self, func: Function) -> AeroResult<()> {
        self.functions.write().insert(func.id, func);
        Ok(())
    }

    pub async fn get_function(&self, id: Uuid) -> AeroResult<Function> {
        self.functions
            .read()
            .get(&id)
            .cloned()
            .ok_or_else(|| AeroError::NotFound(format!("Function {id} not found")))
    }

    pub async fn list_functions(&self) -> AeroResult<Vec<Function>> {
        Ok(self.functions.read().values().cloned().collect())
    }

    pub async fn delete_function(&self, id: Uuid) -> AeroResult<()> {
        self.functions.write().remove(&id);
        Ok(())
    }

    pub async fn create_collection(&self, name: &str, schema: serde_json::Value) -> AeroResult<()> {
        self.collections.write().insert(name.to_string(), schema);
        tracing::info!(collection = name, "Collection created");
        Ok(())
    }

    pub async fn get_schema(&self) -> AeroResult<serde_json::Value> {
        let collections = self.collections.read();
        Ok(serde_json::json!({
            "collections": collections.iter().map(|(k, v)| {
                serde_json::json!({ "name": k, "schema": v })
            }).collect::<Vec<_>>()
        }))
    }
}
