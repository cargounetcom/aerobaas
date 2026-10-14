use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::WorkspaceSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: WorkspacePlan,
    pub status: WorkspaceStatus,
    pub settings: WorkspaceSettings,
    pub member_count: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePlan {
    Free,
    Pro,
    Enterprise,
    AirGapped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Active,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    pub max_members: u32,
    pub max_functions: u32,
    pub max_rps: u32,
    pub ai_enabled: bool,
    pub hazmat_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub slug: String,
    pub plan: Option<WorkspacePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub plan: Option<WorkspacePlan>,
    pub settings: Option<WorkspaceSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub status: MembershipStatus,
    pub invited_by: Option<Uuid>,
    pub joined_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Manager,
    User,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::User => "user",
        }
    }

    pub fn can_manage_members(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn can_invite(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Manager)
    }

    pub fn can_manage_workspace(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn can_deploy(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Manager | Self::User)
    }

    pub fn can_manage_keys(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Manager)
    }

    pub fn can_access_inspector(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Manager)
    }

    pub fn level(&self) -> u8 {
        match self {
            Role::Owner => 4,
            Role::Admin => 3,
            Role::Manager => 2,
            Role::User => 1,
        }
    }

    pub fn implies(&self, other: &Role) -> bool {
        self.level() >= other.level()
    }

    pub fn parse(s: &str) -> Option<Role> {
        match s.trim().to_ascii_lowercase().as_str() {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            "manager" => Some(Role::Manager),
            "user" => Some(Role::User),
            _ => None,
        }
    }

    /// Same matrix as the SovereignBaaS admin: owners invite anyone, admins up to admin,
    /// managers only users, users nobody.
    pub fn can_invite_role(&self, target: &Role) -> bool {
        match self {
            Role::Owner => true,
            Role::Admin => target.level() <= Role::Admin.level(),
            Role::Manager => *target == Role::User,
            Role::User => false,
        }
    }

    pub fn can_change_role(&self, current: &Role, new: &Role) -> bool {
        match self {
            Role::Owner => true,
            Role::Admin => {
                current.level() < Role::Admin.level() && new.level() <= Role::Admin.level()
            }
            _ => false,
        }
    }

    pub fn can_remove(&self, target: &Role) -> bool {
        match self {
            Role::Owner => true,
            Role::Admin => target.level() < Role::Admin.level(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MembershipStatus {
    Pending,
    Active,
    Revoked,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRequest {
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    pub invite_id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
    pub role: Role,
    pub invite_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptInviteRequest {
    pub invite_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMembersResponse {
    pub members: Vec<Membership>,
    pub total: usize,
}

impl From<&Workspace> for WorkspaceSummary {
    fn from(w: &Workspace) -> Self {
        Self {
            id: w.id,
            name: w.name.clone(),
            slug: w.slug.clone(),
            plan: format!("{:?}", w.plan).to_lowercase(),
        }
    }
}
