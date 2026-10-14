//! Workspaces, memberships and invitations (PostgreSQL). Role rules match the SovereignBaaS admin.

use std::sync::Arc;

use aero_db::{db_err, dec, enc, is_unique_violation, DbClient};
use aero_protocol::tenancy::*;
use aero_protocol::{AeroError, AeroResult};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

pub struct TenancyService {
    db: Arc<DbClient>,
    invite_ttl_hours: i64,
}

/// A member with the user's name and email.
#[derive(Debug, Clone, Serialize)]
pub struct MemberInfo {
    #[serde(flatten)]
    pub membership: Membership,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InviteInfo {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub email: String,
    pub role: Role,
    pub invited_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// open | accepted | revoked | expired
    pub status: &'static str,
}

fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn new_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

fn validate_ws_name(name: &str) -> AeroResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 {
        return Err(AeroError::BadRequest(
            "Workspace name must be 1–100 characters".into(),
        ));
    }
    Ok(n.to_string())
}

fn validate_slug(slug: &str) -> AeroResult<String> {
    let s = slug.trim().to_lowercase();
    let ok = (3..=48).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-');
    if ok {
        Ok(s)
    } else {
        Err(AeroError::BadRequest(
            "Slug must be 3–48 characters: lowercase letters, digits and dashes".into(),
        ))
    }
}

fn membership_from_row(r: &PgRow) -> AeroResult<Membership> {
    Ok(Membership {
        id: r.get("id"),
        workspace_id: r.get("workspace_id"),
        user_id: r.get("user_id"),
        role: dec(r.get::<&str, _>("role"))?,
        status: MembershipStatus::Active,
        invited_by: r.get("invited_by"),
        joined_at: Some(r.get("joined_at")),
        created_at: r.get("created_at"),
    })
}

fn workspace_from_row(r: &PgRow) -> AeroResult<Workspace> {
    let settings: sqlx::types::Json<WorkspaceSettings> = r.get("settings");
    Ok(Workspace {
        id: r.get("id"),
        owner_id: r.get("owner_id"),
        name: r.get("name"),
        slug: r.get("slug"),
        plan: dec(r.get::<&str, _>("plan"))?,
        status: dec(r.get::<&str, _>("status"))?,
        settings: settings.0,
        member_count: r.get::<i64, _>("member_count") as u32,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

const WS_SELECT: &str = "SELECT w.id, w.owner_id, w.name, w.slug, w.plan, w.status, w.settings, w.created_at, w.updated_at,
    (SELECT COUNT(*) FROM memberships c WHERE c.workspace_id = w.id) AS member_count FROM workspaces w";
const M_COLS: &str = "id, workspace_id, user_id, role, invited_by, joined_at, created_at";

impl TenancyService {
    pub fn new(db: Arc<DbClient>, invite_ttl_hours: i64) -> Self {
        Self {
            db,
            invite_ttl_hours: invite_ttl_hours.max(1),
        }
    }

    fn pool(&self) -> &sqlx::PgPool {
        self.db.pool()
    }

    pub async fn create_workspace(
        &self,
        owner_id: Uuid,
        req: CreateWorkspaceRequest,
    ) -> AeroResult<Workspace> {
        let name = validate_ws_name(&req.name)?;
        let slug = validate_slug(&req.slug)?;
        if req.plan.as_ref().is_some_and(|p| *p != WorkspacePlan::Free) {
            return Err(AeroError::Forbidden(
                "Paid plans are set by the licence, not by the API".into(),
            ));
        }
        let now = chrono::Utc::now();
        let id = Uuid::new_v4();
        let settings = WorkspaceSettings {
            max_members: 10,
            max_functions: 50,
            max_rps: 1000,
            ai_enabled: true,
            hazmat_required: true,
        };
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        let r = sqlx::query(
            "INSERT INTO workspaces (id, owner_id, name, slug, plan, status, settings, created_at, updated_at)
             VALUES ($1,$2,$3,$4,'free','active',$5,$6,$6)",
        )
        .bind(id)
        .bind(owner_id)
        .bind(&name)
        .bind(&slug)
        .bind(sqlx::types::Json(&settings))
        .bind(now)
        .execute(&mut *tx)
        .await;
        if let Err(e) = r {
            if is_unique_violation(&e) {
                return Err(AeroError::Conflict(format!("The slug `{slug}` is taken")));
            }
            return Err(db_err(e));
        }
        sqlx::query(
            "INSERT INTO memberships (id, workspace_id, user_id, role, joined_at, created_at) VALUES ($1,$2,$3,'owner',$4,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(owner_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        tracing::info!(workspace_id = %id, owner_id = %owner_id, "Workspace created");
        self.get_workspace(id).await
    }

    pub async fn get_workspace(&self, id: Uuid) -> AeroResult<Workspace> {
        let r = sqlx::query(&format!("{WS_SELECT} WHERE w.id = $1"))
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?
            .ok_or_else(|| AeroError::NotFound("Workspace not found".into()))?;
        workspace_from_row(&r)
    }

    /// Workspaces the user belongs to, with the user's role in each.
    pub async fn list_workspaces(&self, user_id: Uuid) -> AeroResult<Vec<(Workspace, Role)>> {
        let rows = sqlx::query(
            "SELECT w.id, w.owner_id, w.name, w.slug, w.plan, w.status, w.settings, w.created_at, w.updated_at,
                (SELECT COUNT(*) FROM memberships c WHERE c.workspace_id = w.id) AS member_count, me.role AS my_role
             FROM workspaces w JOIN memberships me ON me.workspace_id = w.id AND me.user_id = $1
             ORDER BY me.joined_at ASC, w.created_at ASC",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        rows.iter()
            .map(|r| Ok((workspace_from_row(r)?, dec(r.get::<&str, _>("my_role"))?)))
            .collect()
    }

    /// The caller's membership. Errors with 403 for non-members (and unknown workspaces,
    /// so outsiders can't probe which ids exist).
    pub async fn access(&self, workspace_id: Uuid, user_id: Uuid) -> AeroResult<Membership> {
        self.membership_of(workspace_id, user_id)
            .await?
            .ok_or_else(|| AeroError::Forbidden("You are not a member of this workspace".into()))
    }

    pub async fn membership_of(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AeroResult<Option<Membership>> {
        let r = sqlx::query(&format!(
            "SELECT {M_COLS} FROM memberships WHERE workspace_id = $1 AND user_id = $2"
        ))
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        r.as_ref().map(membership_from_row).transpose()
    }

    async fn membership_by_id(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<Membership> {
        let r = sqlx::query(&format!(
            "SELECT {M_COLS} FROM memberships WHERE workspace_id = $1 AND id = $2"
        ))
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or_else(|| AeroError::NotFound("Member not found".into()))?;
        membership_from_row(&r)
    }

    pub async fn update_workspace(
        &self,
        actor: &Membership,
        req: UpdateWorkspaceRequest,
    ) -> AeroResult<Workspace> {
        if !actor.role.can_manage_workspace() {
            return Err(AeroError::Forbidden("Requires Admin role or higher".into()));
        }
        if req.plan.is_some() || req.settings.is_some() {
            return Err(AeroError::Forbidden(
                "Plan and limits are set by the licence, not by the API".into(),
            ));
        }
        if let Some(name) = req.name {
            let name = validate_ws_name(&name)?;
            sqlx::query("UPDATE workspaces SET name = $2, updated_at = now() WHERE id = $1")
                .bind(actor.workspace_id)
                .bind(name)
                .execute(self.pool())
                .await
                .map_err(db_err)?;
        }
        self.get_workspace(actor.workspace_id).await
    }

    /// Owners only. Removes the workspace with its members, invites, keys, functions,
    /// request log and audit log.
    pub async fn delete_workspace(&self, actor: &Membership) -> AeroResult<()> {
        if actor.role != Role::Owner {
            return Err(AeroError::Forbidden(
                "Only an owner can delete a workspace".into(),
            ));
        }
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        for sql in [
            "DELETE FROM requests WHERE workspace_id = $1",
            "DELETE FROM audit_events WHERE workspace_id = $1",
            "DELETE FROM workspaces WHERE id = $1",
        ] {
            sqlx::query(sql)
                .bind(actor.workspace_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    pub async fn list_members(&self, workspace_id: Uuid) -> AeroResult<Vec<MemberInfo>> {
        let rows = sqlx::query(
            "SELECT m.id, m.workspace_id, m.user_id, m.role, m.invited_by, m.joined_at, m.created_at, u.name, u.email
             FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        let mut out = rows
            .iter()
            .map(|r| {
                Ok(MemberInfo {
                    membership: membership_from_row(r)?,
                    name: r.get("name"),
                    email: r.get("email"),
                })
            })
            .collect::<AeroResult<Vec<_>>>()?;
        out.sort_by(|a, b| {
            b.membership
                .role
                .level()
                .cmp(&a.membership.role.level())
                .then(a.name.cmp(&b.name))
        });
        Ok(out)
    }

    async fn owner_count(&self, workspace_id: Uuid) -> AeroResult<i64> {
        let r = sqlx::query(
            "SELECT COUNT(*) AS n FROM memberships WHERE workspace_id = $1 AND role = 'owner'",
        )
        .bind(workspace_id)
        .fetch_one(self.pool())
        .await
        .map_err(db_err)?;
        Ok(r.get("n"))
    }

    pub async fn membership_for_user(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> AeroResult<Membership> {
        self.membership_of(workspace_id, user_id)
            .await?
            .ok_or_else(|| AeroError::NotFound("Member not found".into()))
    }

    pub async fn change_role(
        &self,
        actor: &Membership,
        membership_id: Uuid,
        new_role: Role,
    ) -> AeroResult<Membership> {
        let target = self
            .membership_by_id(actor.workspace_id, membership_id)
            .await?;
        if !actor.role.can_change_role(&target.role, &new_role) {
            return Err(AeroError::Forbidden(format!(
                "A {} cannot change a {} to {}",
                actor.role.as_str(),
                target.role.as_str(),
                new_role.as_str()
            )));
        }
        if target.role == Role::Owner
            && new_role != Role::Owner
            && self.owner_count(actor.workspace_id).await? <= 1
        {
            return Err(AeroError::Conflict(
                "A workspace must keep at least one owner".into(),
            ));
        }
        sqlx::query("UPDATE memberships SET role = $2 WHERE id = $1")
            .bind(membership_id)
            .bind(enc(&new_role))
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(Membership {
            role: new_role,
            ..target
        })
    }

    pub async fn remove_member(
        &self,
        actor: &Membership,
        membership_id: Uuid,
    ) -> AeroResult<Membership> {
        let target = self
            .membership_by_id(actor.workspace_id, membership_id)
            .await?;
        let leaving_self = target.user_id == actor.user_id;
        if !leaving_self && !actor.role.can_remove(&target.role) {
            return Err(AeroError::Forbidden(format!(
                "A {} cannot remove a {}",
                actor.role.as_str(),
                target.role.as_str()
            )));
        }
        if target.role == Role::Owner && self.owner_count(actor.workspace_id).await? <= 1 {
            return Err(AeroError::Conflict(
                "A workspace must keep at least one owner".into(),
            ));
        }
        sqlx::query("DELETE FROM memberships WHERE id = $1")
            .bind(membership_id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(target)
    }

    // ------------------------------------------------------------------ invites

    pub async fn invite_member(
        &self,
        actor: &Membership,
        req: InviteRequest,
    ) -> AeroResult<InviteResponse> {
        if !actor.role.can_invite_role(&req.role) {
            return Err(AeroError::Forbidden(format!(
                "A {} cannot invite a {}",
                actor.role.as_str(),
                req.role.as_str()
            )));
        }
        let email = aero_validate_email(&req.email)?;
        let ws = actor.workspace_id;
        let already = sqlx::query(
            "SELECT 1 FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.workspace_id = $1 AND u.email = $2",
        )
        .bind(ws)
        .bind(&email)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        if already.is_some() {
            return Err(AeroError::Conflict(
                "This person is already a member".into(),
            ));
        }
        let token = new_token();
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::hours(self.invite_ttl_hours);
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        // One open invite per email: revoke older ones.
        sqlx::query(
            "UPDATE invites SET revoked_at = $3 WHERE workspace_id = $1 AND email = $2 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(ws)
        .bind(&email)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "INSERT INTO invites (id, workspace_id, email, role, token_hash, invited_by, created_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(id)
        .bind(ws)
        .bind(&email)
        .bind(enc(&req.role))
        .bind(sha256_hex(&token))
        .bind(actor.user_id)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        tracing::info!(workspace_id = %ws, invite_id = %id, role = req.role.as_str(), "Member invited");
        Ok(InviteResponse {
            invite_id: id,
            workspace_id: ws,
            email,
            role: req.role,
            invite_token: token,
            expires_at,
        })
    }

    fn invite_from_row(r: &PgRow) -> AeroResult<InviteInfo> {
        let accepted: Option<chrono::DateTime<chrono::Utc>> = r.get("accepted_at");
        let revoked: Option<chrono::DateTime<chrono::Utc>> = r.get("revoked_at");
        let expires_at: chrono::DateTime<chrono::Utc> = r.get("expires_at");
        let status = if revoked.is_some() {
            "revoked"
        } else if accepted.is_some() {
            "accepted"
        } else if expires_at < chrono::Utc::now() {
            "expired"
        } else {
            "open"
        };
        Ok(InviteInfo {
            id: r.get("id"),
            workspace_id: r.get("workspace_id"),
            workspace_name: r.get("workspace_name"),
            email: r.get("email"),
            role: dec(r.get::<&str, _>("role"))?,
            invited_by: r.get("invited_by"),
            created_at: r.get("created_at"),
            expires_at,
            status,
        })
    }

    const INV_SELECT: &'static str = "SELECT i.id, i.workspace_id, w.name AS workspace_name, i.email, i.role, i.invited_by,
        i.created_at, i.expires_at, i.accepted_at, i.revoked_at FROM invites i JOIN workspaces w ON w.id = i.workspace_id";

    /// Open invites (Manager role or higher).
    pub async fn list_invites(&self, actor: &Membership) -> AeroResult<Vec<InviteInfo>> {
        if !actor.role.implies(&Role::Manager) {
            return Err(AeroError::Forbidden(
                "Requires Manager role or higher".into(),
            ));
        }
        let rows = sqlx::query(&format!(
            "{} WHERE i.workspace_id = $1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > now()
             ORDER BY i.created_at DESC",
            Self::INV_SELECT
        ))
        .bind(actor.workspace_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        rows.iter().map(Self::invite_from_row).collect()
    }

    pub async fn revoke_invite(&self, actor: &Membership, id: Uuid) -> AeroResult<()> {
        let r = sqlx::query(&format!(
            "{} WHERE i.id = $1 AND i.workspace_id = $2",
            Self::INV_SELECT
        ))
        .bind(id)
        .bind(actor.workspace_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or_else(|| AeroError::NotFound("Invite not found".into()))?;
        let inv = Self::invite_from_row(&r)?;
        if !(inv.invited_by == actor.user_id || actor.role.can_invite_role(&inv.role)) {
            return Err(AeroError::Forbidden("You cannot revoke this invite".into()));
        }
        sqlx::query("UPDATE invites SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Public: what an invite token is for.
    pub async fn invite_info(&self, token: &str) -> AeroResult<InviteInfo> {
        let r = sqlx::query(&format!("{} WHERE i.token_hash = $1", Self::INV_SELECT))
            .bind(sha256_hex(token))
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?
            .ok_or_else(|| AeroError::NotFound("Invite not found".into()))?;
        Self::invite_from_row(&r)
    }

    /// The signed-in user joins the workspace. The invite must be open and addressed to
    /// the user's email.
    pub async fn accept_invite(
        &self,
        token: &str,
        user_id: Uuid,
        user_email: &str,
    ) -> AeroResult<(Membership, InviteInfo)> {
        let inv = self.invite_info(token).await?;
        if inv.status != "open" {
            return Err(AeroError::Conflict(format!(
                "This invite is {}",
                inv.status
            )));
        }
        if !inv.email.eq_ignore_ascii_case(user_email) {
            return Err(AeroError::Forbidden(format!(
                "This invite is for {}. Sign in with that email to accept it.",
                inv.email
            )));
        }
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO memberships (id, workspace_id, user_id, role, invited_by, joined_at, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$6) ON CONFLICT (workspace_id, user_id) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(inv.workspace_id)
        .bind(user_id)
        .bind(enc(&inv.role))
        .bind(inv.invited_by)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query("UPDATE invites SET accepted_at = $2, accepted_by = $3 WHERE id = $1")
            .bind(inv.id)
            .bind(now)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        let m = self.membership_for_user(inv.workspace_id, user_id).await?;
        Ok((m, inv))
    }
}

fn aero_validate_email(e: &str) -> AeroResult<String> {
    let e = e.trim().to_lowercase();
    let ok = e.len() <= 254
        && !e.contains(char::is_whitespace)
        && e.split_once('@')
            .map(|(l, d)| {
                !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')
            })
            .unwrap_or(false);
    if ok {
        Ok(e)
    } else {
        Err(AeroError::BadRequest("Enter a valid email address".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_rules() {
        assert_eq!(validate_slug(" Acme-1 ").unwrap(), "acme-1");
        for bad in ["ab", "-abc", "abc-", "a_b_c", "ünï"] {
            assert!(validate_slug(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn role_matrix() {
        use Role::*;
        assert!(Owner.can_invite_role(&Owner));
        assert!(Admin.can_invite_role(&Admin) && !Admin.can_invite_role(&Owner));
        assert!(Manager.can_invite_role(&User) && !Manager.can_invite_role(&Manager));
        assert!(!User.can_invite_role(&User));
        assert!(Admin.can_change_role(&Manager, &Admin));
        assert!(!Admin.can_change_role(&Admin, &User));
        assert!(!Admin.can_change_role(&User, &Owner));
        assert!(Admin.can_remove(&Manager) && !Admin.can_remove(&Admin));
    }
}
