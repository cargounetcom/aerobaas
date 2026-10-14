use std::sync::Arc;

use aero_protocol::tenancy::*;
use aero_protocol::{AeroError, AeroResult};
use uuid::Uuid;

use crate::store::TenancyStore;

pub struct TenancyService {
    store: Arc<TenancyStore>,
}

impl TenancyService {
    pub fn new(store: Arc<TenancyStore>) -> Self {
        Self { store }
    }

    pub async fn create_workspace(
        &self,
        owner_id: Uuid,
        req: CreateWorkspaceRequest,
    ) -> AeroResult<Workspace> {
        let now = chrono::Utc::now();
        let ws_id = Uuid::new_v4();

        let workspace = Workspace {
            id: ws_id,
            owner_id,
            name: req.name,
            slug: req.slug,
            plan: req.plan.unwrap_or(WorkspacePlan::Free),
            status: WorkspaceStatus::Active,
            settings: WorkspaceSettings {
                max_members: 10,
                max_functions: 50,
                max_rps: 1000,
                ai_enabled: true,
                hazmat_required: true,
            },
            member_count: 1,
            created_at: now,
            updated_at: now,
        };

        let membership = Membership {
            id: Uuid::new_v4(),
            workspace_id: ws_id,
            user_id: owner_id,
            role: Role::Owner,
            status: MembershipStatus::Active,
            invited_by: None,
            joined_at: Some(now),
            created_at: now,
        };

        self.store
            .create_workspace(workspace.clone(), membership)
            .await?;

        tracing::info!(workspace_id = %ws_id, owner_id = %owner_id, "Workspace created");

        Ok(workspace)
    }

    pub async fn get_workspace(&self, id: Uuid) -> AeroResult<Workspace> {
        self.store
            .get_workspace(id)
            .await?
            .ok_or_else(|| AeroError::NotFound("Workspace not found".into()))
    }

    pub async fn list_workspaces(&self, user_id: Uuid) -> AeroResult<Vec<Workspace>> {
        self.store.list_workspaces_for_user(user_id).await
    }

    pub async fn update_workspace(
        &self,
        id: Uuid,
        req: UpdateWorkspaceRequest,
    ) -> AeroResult<Workspace> {
        self.store.update_workspace(id, req).await
    }

    pub async fn delete_workspace(&self, id: Uuid) -> AeroResult<()> {
        self.store.delete_workspace(id).await
    }

    pub async fn invite_member(
        &self,
        workspace_id: Uuid,
        invited_by: Uuid,
        req: InviteRequest,
    ) -> AeroResult<InviteResponse> {
        let inviter_membership = self
            .store
            .get_membership(workspace_id, invited_by)
            .await?
            .ok_or_else(|| AeroError::Forbidden("Not a member of this workspace".into()))?;

        if !inviter_membership.role.can_invite() {
            return Err(AeroError::Forbidden("Insufficient role to invite".into()));
        }

        let invite_id = Uuid::new_v4();
        let token = format!("inv_{}", uuid::Uuid::new_v4().simple());
        let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

        let record = crate::store::InviteRecord {
            invite_id,
            workspace_id,
            email: req.email.clone(),
            role: req.role.clone(),
            token: token.clone(),
            expires_at,
        };

        self.store.create_invite(record).await?;

        tracing::info!(
            workspace_id = %workspace_id,
            invite_id = %invite_id,
            email = %req.email,
            role = req.role.as_str(),
            "Member invited"
        );

        Ok(InviteResponse {
            invite_id,
            workspace_id,
            email: req.email,
            role: req.role,
            invite_token: token,
            expires_at,
        })
    }

    pub async fn accept_invite(&self, token: &str, user_id: Uuid) -> AeroResult<Membership> {
        self.store.accept_invite(token, user_id).await
    }

    pub async fn list_members(&self, workspace_id: Uuid) -> AeroResult<ListMembersResponse> {
        let members = self.store.list_members(workspace_id).await?;
        let total = members.len();
        Ok(ListMembersResponse { members, total })
    }

    pub async fn update_member_role(
        &self,
        workspace_id: Uuid,
        requester_id: Uuid,
        target_user_id: Uuid,
        new_role: Role,
    ) -> AeroResult<()> {
        let requester = self
            .store
            .get_membership(workspace_id, requester_id)
            .await?
            .ok_or_else(|| AeroError::Forbidden("Not a member".into()))?;

        if !requester.role.can_manage_members() {
            return Err(AeroError::Forbidden("Insufficient role".into()));
        }

        if new_role == Role::Owner {
            return Err(AeroError::Forbidden("Cannot assign Owner role".into()));
        }

        self.store
            .update_member_role(workspace_id, target_user_id, new_role)
            .await
    }

    pub async fn remove_member(
        &self,
        workspace_id: Uuid,
        requester_id: Uuid,
        target_user_id: Uuid,
    ) -> AeroResult<()> {
        let requester = self
            .store
            .get_membership(workspace_id, requester_id)
            .await?
            .ok_or_else(|| AeroError::Forbidden("Not a member".into()))?;

        if !requester.role.can_manage_members() && requester_id != target_user_id {
            return Err(AeroError::Forbidden("Insufficient role".into()));
        }

        self.store.remove_member(workspace_id, target_user_id).await
    }

    pub async fn check_permission(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        required_role: Role,
    ) -> AeroResult<()> {
        let membership = self
            .store
            .get_membership(workspace_id, user_id)
            .await?
            .ok_or_else(|| AeroError::Forbidden("Not a member of this workspace".into()))?;

        if membership.role.implies(&required_role) {
            Ok(())
        } else {
            Err(AeroError::Forbidden(format!(
                "Requires {} role or above",
                required_role.as_str()
            )))
        }
    }
}
