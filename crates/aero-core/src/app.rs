use std::net::SocketAddr;
use std::sync::Arc;

use aero_ai_engine::AiEngine;
use aero_auth::AuthService;
use aero_chat::{ChatService, ChatStore};
use aero_config::AeroConfig;
use aero_db::DbClient;
use aero_keys::KeyManager;
use aero_metrics::{MetricsCollector, MetricsHistoryStore};
use aero_protocol::AeroResult;
use aero_realtime::EventBroadcaster;
use aero_remediation::RemediationService;
use aero_skills::SkillRegistry;
use aero_tenancy::TenancyService;
use aero_wasm_runtime::WasmRuntime;
use uuid::Uuid;

use crate::router::build_router;

/// Invitations stay open this long.
const INVITE_TTL_HOURS: i64 = 7 * 24;

pub struct AeroApp {
    pub config: Arc<AeroConfig>,
    pub db: Arc<DbClient>,
    pub wasm_runtime: Arc<WasmRuntime>,
    pub ai_engine: Arc<AiEngine>,
    pub key_manager: Arc<KeyManager>,
    pub remediation: Arc<RemediationService>,
    pub skill_registry: Arc<SkillRegistry>,
    pub auth: Arc<AuthService>,
    pub tenancy: Arc<TenancyService>,
    pub chat: Arc<ChatService>,
    pub event_broadcaster: Arc<EventBroadcaster>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub metrics_history: Arc<MetricsHistoryStore>,
}

impl AeroApp {
    /// Connects to PostgreSQL (applying migrations) and builds all services.
    pub async fn new(config: AeroConfig) -> anyhow::Result<Self> {
        let config = Arc::new(config);
        tracing::info!(port = config.server.port, "Initializing AeroBaaS core");

        let db = Arc::new(DbClient::new(&config.database).await?);
        let wasm_runtime = Arc::new(WasmRuntime::new(config.wasm.clone())?);
        let ai_engine = Arc::new(AiEngine::new(config.ai.clone()).await?);
        let key_manager = Arc::new(KeyManager::new(db.clone(), config.clone()));
        let remediation = Arc::new(RemediationService::new(config.clone()));
        let skill_registry = Arc::new(SkillRegistry::new(db.clone()));
        let auth = Arc::new(AuthService::new(
            db.clone(),
            &config.security.jwt_secret,
            config.security.login_max_failures,
        ));
        let tenancy = Arc::new(TenancyService::new(db.clone(), INVITE_TTL_HOURS));
        let chat = Arc::new(ChatService::new(ai_engine.clone(), ChatStore::new()));
        let event_broadcaster = Arc::new(EventBroadcaster::new(4096));
        let metrics_collector = Arc::new(MetricsCollector::new());
        let metrics_history = Arc::new(MetricsHistoryStore::new(10_000));

        Ok(Self {
            config,
            db,
            wasm_runtime,
            ai_engine,
            key_manager,
            remediation,
            skill_registry,
            auth,
            tenancy,
            chat,
            event_broadcaster,
            metrics_collector,
            metrics_history,
        })
    }

    /// Emails in `AERO_PLATFORM_ADMINS`, or the first registered user when that is empty.
    pub async fn is_platform_admin(&self, user_id: Uuid, email: &str) -> AeroResult<bool> {
        let admins = &self.config.security.platform_admins;
        if !admins.is_empty() {
            return Ok(admins.iter().any(|a| a.eq_ignore_ascii_case(email)));
        }
        Ok(self.auth.first_user_id().await? == Some(user_id))
    }

    pub async fn serve(self: Arc<Self>) -> anyhow::Result<()> {
        crate::inspector::spawn_retention(self.clone());
        let app = build_router(&self);
        let addr = format!("{}:{}", self.config.server.host, self.config.server.port);
        tracing::info!(addr = %addr, "Starting HTTP server");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("Shutting down");
}
