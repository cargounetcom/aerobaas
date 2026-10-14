use std::collections::HashMap;

use aero_protocol::tenancy::*;
use aero_protocol::AeroResult;
use parking_lot::RwLock;
use uuid::Uuid;

pub struct TenancyStore {
    workspaces: RwLock<HashMap<Uuid, Workspace>>,
    memberships: RwLock<HashMap<Uuid, Vec<Membership>>>,
    invites: RwLock<HashMap<String, InviteRecord>>,
}

#[derive(Clone)]
pub struct InviteRecord {
    pub invite_id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
    pub role: Role,
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl TenancyStore {
    pub fn new() -> Self {
        Self {
            workspaces: RwLock::new(HashMap::new()),
            memberships: RwLock::new(HashMap::new()),
            invites: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_workspace(&self, ws: Workspace, owner_membership: Membership) -> AeroResult<()> {
        self.workspaces.write().insert(ws.id, ws);
        self.memberships
            .write()
            .entry(owner_membership.workspace_id)
            .or_default()
            .push(owner_membership);
        Ok(())
    }

    pub async fn get_workspace(&self, id: Uuid) -> AeroResult<Option<Workspace>> {
        Ok(self.workspaces.read().get(&id).cloned())
    }

    pub async fn list_workspaces_for_user(&self, user_id: Uuid) -> AeroResult<Vec<Workspace>> {
        let memberships = self.memberships.read();
        let workspaces = self.workspaces.read();
        let ws_ids: Vec<Uuid> = memberships
            .values()
            .flatten()
            .filter(|m| m.user_id == user_id && m.status == MembershipStatus::Active)
            .map(|m| m.workspace_id)
            .collect();
        Ok(ws_ids
            .iter()
            .filter_map(|id| workspaces.get(id).cloned())
            .collect())
    }

    pub async fn update_workspace(&self, id: Uuid, req: UpdateWorkspaceRequest) -> AeroResult<Workspace> {
        let mut workspaces = self.workspaces.write();
        let ws = workspaces
            .get_mut(&id)
            .ok_or_else(|| aero_protocol::AeroError::NotFound("Workspace not found".into()))?;
        if let Some(name) = req.name {
            ws.name = name;
        }
        if let Some(plan) = req.plan {
            ws.plan = plan;
        }
        if let Some(settings) = req.settings {
            ws.settings = settings;
        }
        ws.updated_at = chrono::Utc::now();
        Ok(ws.clone())
    }

    pub async fn delete_workspace(&self, id: Uuid) -> AeroResult<()> {
        self.workspaces.write().remove(&id);
        self.memberships.write().remove(&id);
        Ok(())
    }

    pub async fn list_members(&self, workspace_id: Uuid) -> AeroResult<Vec<Membership>> {
        Ok(self
            .memberships
            .read()
            .get(&workspace_id)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn get_membership(&self, workspace_id: Uuid, user_id: Uuid) -> AeroResult<Option<Membership>> {
        Ok(self
            .memberships
            .read()
            .get(&workspace_id)
            .and_then(|members| {
                members.iter().find(|m| m.user_id == user_id && m.status == MembershipStatus::Active)
            })
            .cloned())
    }

    pub async fn update_member_role(&self, workspace_id: Uuid, user_id: Uuid, role: Role) -> AeroResult<()> {
        let mut memberships = self.memberships.write();
        if let Some(members) = memberships.get_mut(&workspace_id) {
            for m in members {
                if m.user_id == user_id {
                    m.role = role.clone();
                    return Ok(());
                }
            }
        }
        Err(aero_protocol::AeroError::NotFound("Member not found".into()))
    }

    pub async fn remove_member(&self, workspace_id: Uuid, user_id: Uuid) -> AeroResult<()> {
        let mut memberships = self.memberships.write();
        if let Some(members) = memberships.get_mut(&workspace_id) {
            for m in members {
                if m.user_id == user_id {
                    m.status = MembershipStatus::Revoked;
                    return Ok(());
                }
            }
        }
        Err(aero_protocol::AeroError::NotFound("Member not found".into()))
    }

    pub async fn create_invite(&self, record: InviteRecord) -> AeroResult<()> {
        self.invites.write().insert(record.token.clone(), record);
        Ok(())
    }

    pub async fn get_invite_by_token(&self, token: &str) -> AeroResult<Option<InviteRecord>> {
        Ok(self.invites.read().get(token).cloned())
    }

    pub async fn accept_invite(&self, token: &str, user_id: Uuid) -> AeroResult<Membership> {
        let mut invites = self.invites.write();
        let record = invites
            .remove(token)
            .ok_or_else(|| aero_protocol::AeroError::NotFound("Invite not found or expired".into()))?;

        if record.expires_at < chrono::Utc::now() {
            return Err(aero_protocol::AeroError::Unauthorized("Invite expired".into()));
        }

        let membership = Membership {
            id: Uuid::new_v4(),
            workspace_id: record.workspace_id,
            user_id,
            role: record.role.clone(),
            status: MembershipStatus::Active,
            invited_by: None,
            joined_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
        };

        self.memberships
            .write()
            .entry(record.workspace_id)
            .or_default()
            .push(membership.clone());

        Ok(membership)
    }
}

impl Default for TenancyStore {
    fn default() -> Self {
        Self::new()
    }
}
