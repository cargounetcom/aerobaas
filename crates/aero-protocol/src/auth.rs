use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub avatar_url: Option<String>,
    pub email_verified: bool,
    pub status: UserStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: UserPublic,
    pub default_workspace: Option<WorkspaceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub email_verified: bool,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            avatar_url: u.avatar_url,
            email_verified: u.email_verified,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: Uuid,
    pub email: String,
    pub workspace_id: Option<Uuid>,
    pub role: Option<String>,
    pub scopes: Vec<String>,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchWorkspaceRequest {
    pub workspace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchWorkspaceResponse {
    pub token: String,
    pub workspace: WorkspaceSummary,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: String,
}
