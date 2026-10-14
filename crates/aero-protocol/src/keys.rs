use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub project_id: Uuid,
    pub key_prefix: String,
    pub key_hash: String,
    pub name: String,
    pub scopes: Vec<Scope>,
    pub allowed_function_ids: Vec<Uuid>,
    pub allowed_database_ids: Vec<Uuid>,
    pub credits: i64,
    pub credits_limit: i64,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    FunctionsRead,
    FunctionsWrite,
    FunctionsList,
    ExecutionsWrite,
    ExecutionsRead,
    DatabasesRead,
    DatabasesWrite,
    AiInference,
    SkillsManage,
    RemediationRun,
    McpConnect,
    Admin,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FunctionsRead => "functions.read",
            Self::FunctionsWrite => "functions.write",
            Self::FunctionsList => "functions.list",
            Self::ExecutionsWrite => "executions.write",
            Self::ExecutionsRead => "executions.read",
            Self::DatabasesRead => "databases.read",
            Self::DatabasesWrite => "databases.write",
            Self::AiInference => "ai.inference",
            Self::SkillsManage => "skills.manage",
            Self::RemediationRun => "remediation.run",
            Self::McpConnect => "mcp.connect",
            Self::Admin => "admin",
        }
    }

    pub fn implies(&self, other: &Scope) -> bool {
        if self == &Scope::Admin {
            return true;
        }
        self == other
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub scopes: Vec<Scope>,
    pub allowed_function_ids: Option<Vec<Uuid>>,
    pub allowed_database_ids: Option<Vec<Uuid>>,
    pub credits_limit: Option<i64>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    pub id: Uuid,
    pub key: String,
    pub key_prefix: String,
    pub scopes: Vec<Scope>,
    pub credits_limit: i64,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKeyRequest {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKeyResponse {
    pub valid: bool,
    pub project_id: Option<Uuid>,
    pub scopes: Vec<Scope>,
    pub credits_remaining: Option<i64>,
}
