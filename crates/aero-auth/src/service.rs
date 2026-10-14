//! Accounts and sessions: registration, login (Argon2id), JWT access/refresh tokens,
//! workspace switching. Users, workspaces and memberships live in PostgreSQL.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aero_db::{db_err, dec, enc, is_unique_violation, DbClient};
use aero_protocol::auth::*;
use aero_protocol::tenancy::*;
use aero_protocol::{AeroError, AeroResult};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use parking_lot::Mutex;
use sqlx::Row;
use uuid::Uuid;

use crate::jwt::JwtService;

const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
/// Failed logins allowed from one client IP (across all emails) per window.
const MAX_FAILURES_PER_IP: u32 = 50;

pub struct AuthService {
    jwt: JwtService,
    db: Arc<DbClient>,
    max_failures: u32,
    failures: Mutex<HashMap<String, (Instant, u32)>>,
    /// Hash of a random password, verified for unknown emails so timing doesn't reveal accounts.
    dummy_hash: String,
}

pub fn validate_email(email: &str) -> AeroResult<String> {
    let e = email.trim().to_lowercase();
    let ok = e.len() <= 254
        && !e.contains(char::is_whitespace)
        && e.split_once('@')
            .map(|(local, domain)| {
                !local.is_empty()
                    && domain.contains('.')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            })
            .unwrap_or(false);
    if ok {
        Ok(e)
    } else {
        Err(AeroError::BadRequest("Enter a valid email address".into()))
    }
}

fn validate_name(name: &str) -> AeroResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 {
        return Err(AeroError::BadRequest(
            "Name must be 1–100 characters".into(),
        ));
    }
    Ok(n.to_string())
}

fn validate_password(p: &str) -> AeroResult<()> {
    let len = p.chars().count();
    if len < 8 {
        return Err(AeroError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }
    if len > 256 {
        return Err(AeroError::BadRequest(
            "Password must be at most 256 characters".into(),
        ));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> AeroResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AeroError::Internal(format!("password hashing failed: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn default_settings() -> WorkspaceSettings {
    WorkspaceSettings {
        max_members: 5,
        max_functions: 10,
        max_rps: 100,
        ai_enabled: true,
        hazmat_required: true,
    }
}

impl AuthService {
    pub fn new(db: Arc<DbClient>, jwt_secret: &str, max_failures: u32) -> Self {
        let random: String = Uuid::new_v4().to_string();
        Self {
            jwt: JwtService::new(jwt_secret),
            db,
            max_failures: max_failures.max(1),
            failures: Mutex::new(HashMap::new()),
            dummy_hash: hash_password(&random).unwrap_or_default(),
        }
    }

    fn issue(
        &self,
        user_id: Uuid,
        email: &str,
        workspace: Option<(Uuid, &Role)>,
    ) -> AeroResult<(String, String)> {
        self.jwt
            .issue_pair(
                user_id,
                email,
                workspace.map(|w| w.0),
                workspace.map(|w| w.1.as_str()),
                &[],
            )
            .map_err(|e| AeroError::Internal(e.to_string()))
    }

    pub async fn register(&self, req: RegisterRequest) -> AeroResult<AuthResponse> {
        let email = validate_email(&req.email)?;
        let name = validate_name(&req.name)?;
        validate_password(&req.password)?;
        let password_hash = hash_password(&req.password)?;

        let user_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let slug = format!("ws-{}", &user_id.simple().to_string()[..12]);
        let ws_name = format!("{name}'s Workspace");
        let settings = default_settings();

        let mut tx = self.db.pool().begin().await.map_err(db_err)?;
        let inserted = sqlx::query(
            "INSERT INTO users (id, email, name, password_hash, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$5)",
        )
        .bind(user_id)
        .bind(&email)
        .bind(&name)
        .bind(&password_hash)
        .bind(now)
        .execute(&mut *tx)
        .await;
        if let Err(e) = inserted {
            if is_unique_violation(&e) {
                return Err(AeroError::Conflict(
                    "This email is already registered".into(),
                ));
            }
            return Err(db_err(e));
        }
        sqlx::query(
            "INSERT INTO workspaces (id, owner_id, name, slug, plan, status, settings, created_at, updated_at)
             VALUES ($1,$2,$3,$4,'free','active',$5,$6,$6)",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(&ws_name)
        .bind(&slug)
        .bind(sqlx::types::Json(&settings))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "INSERT INTO memberships (id, workspace_id, user_id, role, joined_at, created_at) VALUES ($1,$2,$3,'owner',$4,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;

        tracing::info!(user_id = %user_id, workspace_id = %workspace_id, "User registered");

        let (token, refresh_token) =
            self.issue(user_id, &email, Some((workspace_id, &Role::Owner)))?;
        Ok(AuthResponse {
            token,
            refresh_token,
            user: UserPublic {
                id: user_id,
                email,
                name,
                avatar_url: None,
                email_verified: false,
            },
            default_workspace: Some(WorkspaceSummary {
                id: workspace_id,
                name: ws_name,
                slug,
                plan: "free".into(),
            }),
        })
    }

    fn over_limit(&self, key: &str, limit: u32) -> bool {
        let mut m = self.failures.lock();
        match m.get(key) {
            Some((t, n)) if t.elapsed() < FAILURE_WINDOW => *n >= limit,
            Some(_) => {
                m.remove(key);
                false
            }
            None => false,
        }
    }

    fn note_failure(&self, key: String) {
        let mut m = self.failures.lock();
        if m.len() > 100_000 {
            m.retain(|_, (t, _)| t.elapsed() < FAILURE_WINDOW);
        }
        let e = m.entry(key).or_insert((Instant::now(), 0));
        if e.0.elapsed() >= FAILURE_WINDOW {
            *e = (Instant::now(), 0);
        }
        e.1 += 1;
    }

    /// Log in. `client_ip` (when known) adds a per-IP limit on top of the per-email limit.
    pub async fn login(
        &self,
        req: LoginRequest,
        client_ip: Option<&str>,
    ) -> AeroResult<AuthResponse> {
        let email = req.email.trim().to_lowercase();
        let email_key = format!("email:{email}");
        let ip_key = client_ip.map(|ip| format!("ip:{ip}"));
        if self.over_limit(&email_key, self.max_failures)
            || ip_key
                .as_deref()
                .is_some_and(|k| self.over_limit(k, MAX_FAILURES_PER_IP))
        {
            return Err(AeroError::TooManyRequests(
                "Too many failed sign-in attempts. Try again in 15 minutes.".into(),
            ));
        }

        let row = sqlx::query("SELECT id, email, name, password_hash, avatar_url, email_verified, status FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(self.db.pool())
            .await
            .map_err(db_err)?;

        let ok = match &row {
            Some(r) => verify_password(&req.password, r.get::<&str, _>("password_hash")),
            None => {
                let _ = verify_password(&req.password, &self.dummy_hash);
                false
            }
        };
        if !ok {
            self.note_failure(email_key);
            if let Some(k) = ip_key {
                self.note_failure(k);
            }
            return Err(AeroError::Unauthorized("Invalid email or password".into()));
        }
        let r = row.expect("checked above");
        if r.get::<&str, _>("status") != "active" {
            return Err(AeroError::Unauthorized("Account suspended".into()));
        }
        self.failures.lock().remove(&email_key);

        let user = UserPublic {
            id: r.get("id"),
            email: r.get("email"),
            name: r.get("name"),
            avatar_url: r.get("avatar_url"),
            email_verified: r.get("email_verified"),
        };

        // Default workspace: the one the user owns first, else the earliest joined.
        let ws = sqlx::query(
            "SELECT w.id, w.name, w.slug, w.plan, m.role FROM memberships m JOIN workspaces w ON w.id = m.workspace_id
             WHERE m.user_id = $1 ORDER BY (m.role = 'owner') DESC, m.joined_at ASC LIMIT 1",
        )
        .bind(user.id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(db_err)?;
        let (workspace, role) = match ws {
            Some(w) => (
                Some(WorkspaceSummary {
                    id: w.get("id"),
                    name: w.get("name"),
                    slug: w.get("slug"),
                    plan: w.get("plan"),
                }),
                Some(dec::<Role>(w.get::<&str, _>("role"))?),
            ),
            None => (None, None),
        };
        let (token, refresh_token) = self.issue(
            user.id,
            &user.email,
            workspace
                .as_ref()
                .zip(role.as_ref())
                .map(|(w, r)| (w.id, r)),
        )?;
        tracing::info!(user_id = %user.id, "User logged in");
        Ok(AuthResponse {
            token,
            refresh_token,
            user,
            default_workspace: workspace,
        })
    }

    /// Current role of a user in a workspace (None when not a member).
    async fn role_in(&self, workspace_id: Uuid, user_id: Uuid) -> AeroResult<Option<Role>> {
        let row =
            sqlx::query("SELECT role FROM memberships WHERE workspace_id = $1 AND user_id = $2")
                .bind(workspace_id)
                .bind(user_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(db_err)?;
        row.map(|r| dec::<Role>(r.get::<&str, _>("role")))
            .transpose()
    }

    pub async fn refresh(&self, req: RefreshTokenRequest) -> AeroResult<AuthResponse> {
        let claims = self
            .jwt
            .verify(&req.refresh_token)
            .map_err(|e| AeroError::Unauthorized(e.to_string()))?;
        if claims.typ != "refresh" {
            return Err(AeroError::Unauthorized("Not a refresh token".into()));
        }
        let user = self
            .get_user(claims.sub)
            .await
            .map_err(|_| AeroError::Unauthorized("User not found".into()))?;
        // Re-read the role: it may have changed (or the user may have left) since the token was issued.
        let ws = match claims.workspace_id {
            Some(w) => self.role_in(w, user.id).await?.map(|r| (w, r)),
            None => None,
        };
        let (token, refresh_token) =
            self.issue(user.id, &user.email, ws.as_ref().map(|(w, r)| (*w, r)))?;
        Ok(AuthResponse {
            token,
            refresh_token,
            user,
            default_workspace: None,
        })
    }

    pub async fn switch_workspace(
        &self,
        user_id: Uuid,
        req: SwitchWorkspaceRequest,
    ) -> AeroResult<SwitchWorkspaceResponse> {
        let user = self.get_user(user_id).await?;
        let row = sqlx::query(
            "SELECT w.name, w.slug, w.plan, m.role FROM memberships m JOIN workspaces w ON w.id = m.workspace_id
             WHERE m.workspace_id = $1 AND m.user_id = $2",
        )
        .bind(req.workspace_id)
        .bind(user_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(db_err)?
        .ok_or_else(|| AeroError::Forbidden("You are not a member of this workspace".into()))?;
        let role: Role = dec(row.get::<&str, _>("role"))?;
        let (token, _) = self.issue(user_id, &user.email, Some((req.workspace_id, &role)))?;
        Ok(SwitchWorkspaceResponse {
            token,
            workspace: WorkspaceSummary {
                id: req.workspace_id,
                name: row.get("name"),
                slug: row.get("slug"),
                plan: row.get("plan"),
            },
            role: enc(&role),
        })
    }

    /// Verify an access token (refresh tokens are rejected).
    pub fn verify_token(&self, token: &str) -> AeroResult<JwtClaims> {
        let claims = self
            .jwt
            .verify(token)
            .map_err(|e| AeroError::Unauthorized(e.to_string()))?;
        if claims.typ != "access" {
            return Err(AeroError::Unauthorized("Not an access token".into()));
        }
        Ok(claims)
    }

    pub async fn get_user(&self, user_id: Uuid) -> AeroResult<UserPublic> {
        let r = sqlx::query("SELECT id, email, name, avatar_url, email_verified FROM users WHERE id = $1 AND status = 'active'")
            .bind(user_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(db_err)?
            .ok_or_else(|| AeroError::NotFound("User not found".into()))?;
        Ok(UserPublic {
            id: r.get("id"),
            email: r.get("email"),
            name: r.get("name"),
            avatar_url: r.get("avatar_url"),
            email_verified: r.get("email_verified"),
        })
    }

    /// The earliest registered user (platform admin when `AERO_PLATFORM_ADMINS` is empty).
    pub async fn first_user_id(&self) -> AeroResult<Option<Uuid>> {
        let r = sqlx::query("SELECT id FROM users ORDER BY created_at ASC, id ASC LIMIT 1")
            .fetch_optional(self.db.pool())
            .await
            .map_err(db_err)?;
        Ok(r.map(|r| r.get("id")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_roundtrip() {
        let h = hash_password("correct horse").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
        assert!(!verify_password("x", "not-a-hash"));
    }

    #[test]
    fn email_validation() {
        assert_eq!(validate_email(" A@X.io ").unwrap(), "a@x.io");
        for bad in ["", "a", "@x.io", "a@", "a@x", "a b@x.io", "a@.io"] {
            assert!(validate_email(bad).is_err(), "{bad}");
        }
    }
}
