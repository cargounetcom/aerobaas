use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub panel_config: PanelConfig,
    pub message_count: u32,
    pub tokens_used: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PanelConfig {
    pub layout: PanelLayout,
    pub theme: PanelTheme,
    pub comfort_mode: bool,
    pub show_token_count: bool,
    pub show_latency: bool,
    pub streaming: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub sidebar_collapsed: bool,
    pub panel_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PanelLayout {
    Single,
    #[default]
    Split,
    Triple,
    Dashboard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PanelTheme {
    Light,
    #[default]
    Dark,
    Midnight,
    Aurora,
    Sunset,
    Ocean,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub tokens: u32,
    pub model: Option<String>,
    pub latency_ms: Option<u64>,
    pub attachments: Vec<Attachment>,
    pub panel_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub url: Option<String>,
    pub base64_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    pub title: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub panel_config: Option<PanelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub conversation_id: Uuid,
    pub content: String,
    pub attachments: Option<Vec<Attachment>>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message: Message,
    pub assistant_message: Message,
    pub conversation: Conversation,
    pub tokens_used: u64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChunk {
    pub message_id: Uuid,
    pub delta: String,
    pub done: bool,
    pub tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Panel {
    pub id: String,
    pub panel_type: PanelType,
    pub title: String,
    pub config: serde_json::Value,
    pub position: u32,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PanelType {
    Chat,
    CodePreview,
    TokenUsage,
    LatencyGraph,
    FunctionList,
    DbSchema,
    DeploymentLog,
    Inspector,
    Settings,
    Comfort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfortPanel {
    pub id: String,
    pub greeting: String,
    pub user_name: Option<String>,
    pub mood: Mood,
    pub ambient_color: String,
    pub quick_actions: Vec<QuickAction>,
    pub recent_conversations: Vec<ConversationSummary>,
    pub stats: ComfortStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Calm,
    Energetic,
    Focused,
    Creative,
    Relaxed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickAction {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub action_type: QuickActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuickActionType {
    NewChat,
    DeployFunction,
    ScanDependencies,
    CreateKey,
    ViewInspector,
    SwitchWorkspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub preview: String,
    pub message_count: u32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfortStats {
    pub total_conversations: u32,
    pub total_tokens: u64,
    pub total_deployments: u32,
    pub avg_latency_ms: f64,
    pub uptime_hours: f64,
}
