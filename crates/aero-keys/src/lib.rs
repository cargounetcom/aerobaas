//! Keys API 2.0: workspace-scoped API keys with scopes, per-key function allow-lists and credits.

use std::sync::Arc;

use aero_config::AeroConfig;
use aero_crypto::{generate_api_key, hash_api_key, key_prefix};
use aero_db::DbClient;
use aero_protocol::keys::{
    ApiKey, CreateKeyRequest, CreateKeyResponse, Scope, VerifyKeyRequest, VerifyKeyResponse,
};
use aero_protocol::{AeroError, AeroResult};
use uuid::Uuid;

/// Credits a new key gets when the request doesn't say.
pub const DEFAULT_CREDITS: i64 = 1_000_000;
const MAX_CREDITS: i64 = 1_000_000_000_000;

pub struct KeyManager {
    db: Arc<DbClient>,
    #[allow(dead_code)] // plan limits arrive with licensing (Phase 5)
    config: Arc<AeroConfig>,
}

fn invalid() -> VerifyKeyResponse {
    VerifyKeyResponse {
        valid: false,
        project_id: None,
        scopes: vec![],
        credits_remaining: None,
    }
}

impl KeyManager {
    pub fn new(db: Arc<DbClient>, config: Arc<AeroConfig>) -> Self {
        Self { db, config }
    }

    /// Create a key in `workspace_id`. The caller's permission is checked by the router.
    pub async fn create_key(
        &self,
        workspace_id: Uuid,
        created_by: Option<Uuid>,
        req: CreateKeyRequest,
    ) -> AeroResult<CreateKeyResponse> {
        let name = req.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(AeroError::BadRequest(
                "Key name must be 1–100 characters".into(),
            ));
        }
        if req.scopes.is_empty() {
            return Err(AeroError::BadRequest(
                "Give the key at least one scope".into(),
            ));
        }
        let credits_limit = req.credits_limit.unwrap_or(DEFAULT_CREDITS);
        if !(1..=MAX_CREDITS).contains(&credits_limit) {
            return Err(AeroError::BadRequest(
                "credits_limit must be positive".into(),
            ));
        }
        if req.expires_at.is_some_and(|t| t <= chrono::Utc::now()) {
            return Err(AeroError::BadRequest(
                "expires_at must be in the future".into(),
            ));
        }
        let allowed_function_ids = req.allowed_function_ids.unwrap_or_default();
        if !allowed_function_ids.is_empty() {
            let found = self
                .db
                .existing_function_ids(workspace_id, &allowed_function_ids)
                .await?;
            if let Some(missing) = allowed_function_ids.iter().find(|id| !found.contains(id)) {
                return Err(AeroError::BadRequest(format!(
                    "Function {missing} is not in this workspace"
                )));
            }
        }
        let mut scopes = req.scopes;
        scopes.dedup();

        let raw_key = generate_api_key();
        let api_key = ApiKey {
            id: Uuid::new_v4(),
            project_id: workspace_id,
            key_prefix: key_prefix(&raw_key),
            key_hash: hash_api_key(&raw_key),
            name,
            scopes: scopes.clone(),
            allowed_function_ids,
            allowed_database_ids: req.allowed_database_ids.unwrap_or_default(),
            credits: credits_limit,
            credits_limit,
            enabled: true,
            created_at: chrono::Utc::now(),
            expires_at: req.expires_at,
        };
        self.db.store_key(&api_key, created_by).await?;
        tracing::info!(key_id = %api_key.id, workspace_id = %workspace_id, "API key created");

        Ok(CreateKeyResponse {
            id: api_key.id,
            key: raw_key,
            key_prefix: api_key.key_prefix,
            scopes,
            credits_limit,
            expires_at: api_key.expires_at,
        })
    }

    fn usable(k: &ApiKey) -> bool {
        k.enabled && k.credits > 0 && k.expires_at.is_none_or(|t| t > chrono::Utc::now())
    }

    /// Public check of a raw key (does not use credits).
    pub async fn verify_key(&self, req: VerifyKeyRequest) -> AeroResult<VerifyKeyResponse> {
        if !req.key.starts_with("aero_") {
            return Ok(invalid());
        }
        match self.db.get_key_by_hash(&hash_api_key(&req.key)).await? {
            Some(k) if Self::usable(&k) => Ok(VerifyKeyResponse {
                valid: true,
                project_id: Some(k.project_id),
                scopes: k.scopes,
                credits_remaining: Some(k.credits),
            }),
            _ => Ok(invalid()),
        }
    }

    /// Authenticate a request made with a raw key and charge `cost` credits for it.
    /// 401 for unknown, disabled or expired keys; 429 when the credits run out.
    pub async fn authenticate(&self, raw_key: &str, cost: i64) -> AeroResult<ApiKey> {
        let unauthorized = || AeroError::Unauthorized("Invalid or expired API key".into());
        let mut key = self
            .db
            .get_key_by_hash(&hash_api_key(raw_key))
            .await?
            .ok_or_else(unauthorized)?;
        if key.expires_at.is_some_and(|t| t <= chrono::Utc::now()) {
            return Err(unauthorized());
        }
        if !key.enabled && key.credits > 0 {
            return Err(unauthorized());
        }
        match self.db.take_credits(key.id, cost).await? {
            Some(remaining) => {
                key.credits = remaining;
                key.enabled = remaining > 0;
                Ok(key)
            }
            None => Err(AeroError::RateLimited),
        }
    }

    pub fn check_scope(key: &ApiKey, required: &Scope) -> AeroResult<()> {
        if key.scopes.iter().any(|s| s.implies(required)) {
            Ok(())
        } else {
            Err(AeroError::Forbidden(format!(
                "this API key lacks the `{}` scope",
                required.as_str()
            )))
        }
    }

    /// Keys with an allow-list may only see/run the functions on it.
    pub fn function_allowed(key: &ApiKey, function_id: Uuid) -> bool {
        key.allowed_function_ids.is_empty() || key.allowed_function_ids.contains(&function_id)
    }

    pub async fn list_keys(&self, workspace_id: Uuid) -> AeroResult<Vec<ApiKey>> {
        self.db.list_keys(workspace_id).await
    }

    pub async fn get_key(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<ApiKey> {
        self.db
            .get_key(workspace_id, id)
            .await?
            .ok_or_else(|| AeroError::NotFound(format!("Key {id} not found")))
    }

    pub async fn delete_key(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<()> {
        if self.db.delete_key(workspace_id, id).await? {
            Ok(())
        } else {
            Err(AeroError::NotFound(format!("Key {id} not found")))
        }
    }
}
