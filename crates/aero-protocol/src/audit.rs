use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_type: ActorType,
    pub actor_id: Option<Uuid>,
    pub actor_name: String,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: serde_json::Value,
    pub severity: EventSeverity,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    AiAgent,
    System,
    HazmatModule,
    RemediationModule,
    ErlangOrchestrator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Login,
    Logout,
    Register,
    WorkspaceCreated,
    WorkspaceDeleted,
    MemberInvited,
    MemberRemoved,
    MemberRoleChanged,
    KeyCreated,
    KeyDeleted,
    KeyRevoked,
    FunctionDeployed,
    FunctionDeleted,
    FunctionExecuted,
    FunctionBlocked,
    CollectionCreated,
    CollectionDeleted,
    ScanCompleted,
    RemediationApplied,
    RemediationBlocked,
    HazmatViolation,
    HazmatQuarantinePassed,
    AiGenerated,
    AiChatMessage,
    SkillInstalled,
    SkillDeployed,
    WorkspaceSwitched,
    AnomalyDetected,
    CreditsConsumed,
    CreditsExhausted,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Logout => "logout",
            Self::Register => "register",
            Self::WorkspaceCreated => "workspace_created",
            Self::WorkspaceDeleted => "workspace_deleted",
            Self::MemberInvited => "member_invited",
            Self::MemberRemoved => "member_removed",
            Self::MemberRoleChanged => "member_role_changed",
            Self::KeyCreated => "key_created",
            Self::KeyDeleted => "key_deleted",
            Self::KeyRevoked => "key_revoked",
            Self::FunctionDeployed => "function_deployed",
            Self::FunctionDeleted => "function_deleted",
            Self::FunctionExecuted => "function_executed",
            Self::FunctionBlocked => "function_blocked",
            Self::CollectionCreated => "collection_created",
            Self::CollectionDeleted => "collection_deleted",
            Self::ScanCompleted => "scan_completed",
            Self::RemediationApplied => "remediation_applied",
            Self::RemediationBlocked => "remediation_blocked",
            Self::HazmatViolation => "hazmat_violation",
            Self::HazmatQuarantinePassed => "hazmat_quarantine_passed",
            Self::AiGenerated => "ai_generated",
            Self::AiChatMessage => "ai_chat_message",
            Self::SkillInstalled => "skill_installed",
            Self::SkillDeployed => "skill_deployed",
            Self::WorkspaceSwitched => "workspace_switched",
            Self::AnomalyDetected => "anomaly_detected",
            Self::CreditsConsumed => "credits_consumed",
            Self::CreditsExhausted => "credits_exhausted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogQuery {
    pub workspace_id: Option<Uuid>,
    pub actor_type: Option<ActorType>,
    pub action: Option<AuditAction>,
    pub severity: Option<EventSeverity>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogResponse {
    pub events: Vec<AuditEvent>,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeEvent {
    pub event: AuditEvent,
    pub stream_id: String,
}
