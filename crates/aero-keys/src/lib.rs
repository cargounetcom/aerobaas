use std::sync::Arc;

use aero_config::AeroConfig;
use aero_crypto::{generate_api_key, hash_api_key, key_prefix};
use aero_db::DbClient;
use aero_protocol::keys::{
    ApiKey, CreateKeyRequest, CreateKeyResponse, Scope, VerifyKeyRequest, VerifyKeyResponse,
};
use aero_protocol::{AeroError, AeroResult};
use uuid::Uuid;

pub struct KeyManager {
    db: Arc<DbClient>,
    #[allow(dead_code)] // wired up in a later phase (see ROADMAP.md)
    config: Arc<AeroConfig>,
}

impl KeyManager {
    pub fn new(db: Arc<DbClient>, config: Arc<AeroConfig>) -> Self {
        Self { db, config }
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> AeroResult<CreateKeyResponse> {
        let raw_key = generate_api_key();
        let key_hash = hash_api_key(&raw_key);
        let prefix = key_prefix(&raw_key);
        let key_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        let credits_limit = req.credits_limit.unwrap_or(1_000_000);

        let api_key = ApiKey {
            id: key_id,
            project_id,
            key_prefix: prefix.clone(),
            key_hash,
            name: req.name,
            scopes: req.scopes.clone(),
            allowed_function_ids: req.allowed_function_ids.unwrap_or_default(),
            allowed_database_ids: req.allowed_database_ids.unwrap_or_default(),
            credits: credits_limit,
            credits_limit,
            enabled: true,
            created_at: chrono::Utc::now(),
            expires_at: req.expires_at,
        };

        self.db.store_key(api_key).await?;

        tracing::info!(key_id = %key_id, "API key created");

        Ok(CreateKeyResponse {
            id: key_id,
            key: raw_key,
            key_prefix: prefix,
            scopes: req.scopes,
            credits_limit,
            expires_at: req.expires_at,
        })
    }

    pub async fn verify_key(&self, req: VerifyKeyRequest) -> AeroResult<VerifyKeyResponse> {
        let key_hash = hash_api_key(&req.key);
        let api_key = self.db.get_key_by_hash(&key_hash).await?;

        match api_key {
            Some(k) if k.enabled => {
                let credits_remaining = if k.credits > 0 { Some(k.credits) } else { None };
                if k.credits <= 0 {
                    return Ok(VerifyKeyResponse {
                        valid: false,
                        project_id: None,
                        scopes: vec![],
                        credits_remaining: None,
                    });
                }
                Ok(VerifyKeyResponse {
                    valid: true,
                    project_id: Some(k.project_id),
                    scopes: k.scopes,
                    credits_remaining,
                })
            }
            _ => Ok(VerifyKeyResponse {
                valid: false,
                project_id: None,
                scopes: vec![],
                credits_remaining: None,
            }),
        }
    }

    pub async fn check_scope(&self, key: &ApiKey, required: &Scope) -> AeroResult<()> {
        if key.scopes.iter().any(|s| s.implies(required)) {
            Ok(())
        } else {
            Err(AeroError::Forbidden(required.as_str().to_string()))
        }
    }

    pub async fn consume_credits(&self, key_id: Uuid, amount: i64) -> AeroResult<()> {
        self.db.decrement_credits(key_id, amount).await?;
        Ok(())
    }

    pub async fn list_keys(&self) -> AeroResult<Vec<ApiKey>> {
        self.db.list_keys().await
    }

    pub async fn get_key(&self, id: Uuid) -> AeroResult<ApiKey> {
        self.db
            .get_key(id)
            .await?
            .ok_or_else(|| AeroError::NotFound(format!("Key {id} not found")))
    }

    pub async fn delete_key(&self, id: Uuid) -> AeroResult<()> {
        self.db.delete_key(id).await
    }
}
