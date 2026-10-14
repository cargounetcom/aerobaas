use std::sync::Arc;

use aero_db::DbClient;
use aero_protocol::auth::*;
use aero_protocol::tenancy::*;
use aero_protocol::{AeroError, AeroResult};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::jwt::JwtService;

pub struct AuthService {
    jwt: JwtService,
    #[allow(dead_code)] // wired up in a later phase (see ROADMAP.md)
    db: Arc<DbClient>,
    users: RwLock<std::collections::HashMap<Uuid, User>>,
    email_index: RwLock<std::collections::HashMap<String, Uuid>>,
}

impl AuthService {
    pub fn new(db: Arc<DbClient>, jwt_secret: &str) -> Self {
        Self {
            jwt: JwtService::new(jwt_secret),
            db,
            users: RwLock::new(std::collections::HashMap::new()),
            email_index: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub async fn register(&self, req: RegisterRequest) -> AeroResult<AuthResponse> {
        {
            let email_idx = self.email_index.read();
            if email_idx.contains_key(&req.email) {
                return Err(AeroError::Internal("Email already registered".into()));
            }
        }

        let user_id = Uuid::new_v4();
        let password_hash = hash_password(&req.password);
        let now = chrono::Utc::now();

        let user = User {
            id: user_id,
            email: req.email.clone(),
            name: req.name.clone(),
            password_hash,
            avatar_url: None,
            email_verified: false,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
        };

        self.users.write().insert(user_id, user.clone());
        self.email_index.write().insert(req.email.clone(), user_id);

        let workspace_id = Uuid::new_v4();
        let workspace = Workspace {
            id: workspace_id,
            owner_id: user_id,
            name: format!("{}'s Workspace", req.name),
            slug: format!("ws-{}", &user_id.to_string()[..8]),
            plan: WorkspacePlan::Free,
            status: WorkspaceStatus::Active,
            settings: WorkspaceSettings {
                max_members: 5,
                max_functions: 10,
                max_rps: 100,
                ai_enabled: true,
                hazmat_required: true,
            },
            member_count: 1,
            created_at: now,
            updated_at: now,
        };

        let _membership = Membership {
            id: Uuid::new_v4(),
            workspace_id,
            user_id,
            role: Role::Owner,
            status: MembershipStatus::Active,
            invited_by: None,
            joined_at: Some(now),
            created_at: now,
        };

        tracing::info!(user_id = %user_id, workspace_id = %workspace_id, "User registered");

        let (token, refresh_token) = self
            .jwt
            .issue_pair(
                user_id,
                &user.email,
                Some(workspace_id),
                Some(Role::Owner.as_str()),
                &[],
            )
            .map_err(|e| AeroError::Internal(e.to_string()))?;

        Ok(AuthResponse {
            token,
            refresh_token,
            user: user.into(),
            default_workspace: Some(WorkspaceSummary::from(&workspace)),
        })
    }

    pub async fn login(&self, req: LoginRequest) -> AeroResult<AuthResponse> {
        let user_id = {
            let email_idx = self.email_index.read();
            email_idx.get(&req.email).copied()
        };

        let user_id =
            user_id.ok_or_else(|| AeroError::Unauthorized("Invalid credentials".into()))?;

        let user = {
            let users = self.users.read();
            users.get(&user_id).cloned()
        };

        let user = user.ok_or_else(|| AeroError::Unauthorized("Invalid credentials".into()))?;

        if !verify_password(&req.password, &user.password_hash) {
            return Err(AeroError::Unauthorized("Invalid credentials".into()));
        }

        if user.status != UserStatus::Active {
            return Err(AeroError::Unauthorized("Account suspended".into()));
        }

        let workspace_id = Some(Uuid::new_v4());
        let (token, refresh_token) = self
            .jwt
            .issue_pair(
                user_id,
                &user.email,
                workspace_id,
                Some(Role::Owner.as_str()),
                &[],
            )
            .map_err(|e| AeroError::Internal(e.to_string()))?;

        tracing::info!(user_id = %user_id, "User logged in");

        Ok(AuthResponse {
            token,
            refresh_token,
            user: user.into(),
            default_workspace: None,
        })
    }

    pub async fn refresh(&self, req: RefreshTokenRequest) -> AeroResult<AuthResponse> {
        let claims = self
            .jwt
            .verify(&req.refresh_token)
            .map_err(|e| AeroError::Unauthorized(e.to_string()))?;

        let user = {
            let users = self.users.read();
            users.get(&claims.sub).cloned()
        };

        let user = user.ok_or_else(|| AeroError::Unauthorized("User not found".into()))?;

        let (token, refresh_token) = self
            .jwt
            .issue_pair(
                user.id,
                &user.email,
                claims.workspace_id,
                claims.role.as_deref(),
                &claims.scopes,
            )
            .map_err(|e| AeroError::Internal(e.to_string()))?;

        Ok(AuthResponse {
            token,
            refresh_token,
            user: user.into(),
            default_workspace: None,
        })
    }

    pub async fn switch_workspace(
        &self,
        user_id: Uuid,
        req: SwitchWorkspaceRequest,
    ) -> AeroResult<SwitchWorkspaceResponse> {
        let user = {
            let users = self.users.read();
            users.get(&user_id).cloned()
        };

        let user = user.ok_or_else(|| AeroError::Unauthorized("User not found".into()))?;

        let (token, _) = self
            .jwt
            .issue_pair(
                user_id,
                &user.email,
                Some(req.workspace_id),
                Some(Role::User.as_str()),
                &[],
            )
            .map_err(|e| AeroError::Internal(e.to_string()))?;

        Ok(SwitchWorkspaceResponse {
            token,
            workspace: WorkspaceSummary {
                id: req.workspace_id,
                name: "Switched Workspace".into(),
                slug: format!("ws-{}", &req.workspace_id.to_string()[..8]),
                plan: "free".into(),
            },
            role: Role::User.as_str().into(),
        })
    }

    pub fn verify_token(&self, token: &str) -> AeroResult<JwtClaims> {
        self.jwt
            .verify(token)
            .map_err(|e| AeroError::Unauthorized(e.to_string()))
    }

    pub async fn get_user(&self, user_id: Uuid) -> AeroResult<UserPublic> {
        let user = {
            let users = self.users.read();
            users.get(&user_id).cloned()
        };
        let user = user.ok_or_else(|| AeroError::NotFound("User not found".into()))?;
        Ok(user.into())
    }
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(b"aero_salt_2026");
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn verify_password(password: &str, hash: &str) -> bool {
    let computed = hash_password(password);
    computed == hash
}
