use std::sync::Arc;

use aero_ai_engine::AiEngine;
use aero_auth::AuthService;
use aero_chat::{ChatService, ChatStore};
use aero_config::AeroConfig;
use aero_db::DbClient;
use aero_inspector::TraceCollector;
use aero_keys::KeyManager;
use aero_mcp_server::McpServer;
use aero_metrics::{MetricsCollector, MetricsHistoryStore};
use aero_realtime::{AuditLogger, EventBroadcaster};
use aero_remediation::RemediationService;
use aero_skills::SkillRegistry;
use aero_tenancy::{TenancyService, TenancyStore};
use aero_wasm_runtime::WasmRuntime;

use crate::router::build_router;

pub struct AeroApp {
    pub config: Arc<AeroConfig>,
    pub db: Arc<DbClient>,
    pub wasm_runtime: Arc<WasmRuntime>,
    pub ai_engine: Arc<AiEngine>,
    pub key_manager: Arc<KeyManager>,
    pub mcp_server: Arc<McpServer>,
    pub remediation: Arc<RemediationService>,
    pub skill_registry: Arc<SkillRegistry>,
    pub auth: Arc<AuthService>,
    pub tenancy: Arc<TenancyService>,
    pub tenancy_store: Arc<TenancyStore>,
    pub inspector: Arc<TraceCollector>,
    pub chat: Arc<ChatService>,
    pub audit_logger: Arc<AuditLogger>,
    pub event_broadcaster: Arc<EventBroadcaster>,
    pub metrics_collector: Arc<MetricsCollector>,
    pub metrics_history: Arc<MetricsHistoryStore>,
}

impl AeroApp {
    pub async fn new(config: AeroConfig) -> anyhow::Result<Self> {
        let config = Arc::new(config);

        tracing::info!(port = config.server.port, "Initializing AeroBaaS core");

        let db = Arc::new(DbClient::new(&config.database).await?);
        let wasm_runtime = Arc::new(WasmRuntime::new(config.wasm.clone())?);
        let ai_engine = Arc::new(AiEngine::new(config.ai.clone()).await?);
        let key_manager = Arc::new(KeyManager::new(db.clone(), config.clone()));
        let mcp_server = Arc::new(McpServer::new(
            db.clone(),
            ai_engine.clone(),
            wasm_runtime.clone(),
        ));
        let remediation = Arc::new(RemediationService::new(config.clone()));
        let skill_registry = Arc::new(SkillRegistry::new(db.clone()));

        let auth = Arc::new(AuthService::new(db.clone(), &config.security.jwt_secret));
        let tenancy_store = Arc::new(TenancyStore::new());
        let tenancy = Arc::new(TenancyService::new(tenancy_store.clone()));
        let inspector = Arc::new(TraceCollector::new(
            aero_protocol::inspector::InspectorConfig::default(),
        ));
        let chat_store = ChatStore::new();
        let chat = Arc::new(ChatService::new(ai_engine.clone(), chat_store));

        let event_broadcaster = Arc::new(EventBroadcaster::new(4096));
        let audit_logger = Arc::new(AuditLogger::new(event_broadcaster.clone(), 100_000));
        let metrics_collector = Arc::new(MetricsCollector::new());
        let metrics_history = Arc::new(MetricsHistoryStore::new(10_000));

        tracing::info!("AeroBaaS core initialized — all modules loaded (auth, tenancy, inspector, chat, realtime, metrics)");

        Ok(Self {
            config,
            db,
            wasm_runtime,
            ai_engine,
            key_manager,
            mcp_server,
            remediation,
            skill_registry,
            auth,
            tenancy,
            tenancy_store,
            inspector,
            chat,
            audit_logger,
            event_broadcaster,
            metrics_collector,
            metrics_history,
        })
    }

    pub async fn serve(self: std::sync::Arc<Self>) -> anyhow::Result<()> {
        let app = build_router(&self);
        let addr = format!("{}:{}", self.config.server.host, self.config.server.port);
        tracing::info!(addr = %addr, "Starting HTTP server");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
