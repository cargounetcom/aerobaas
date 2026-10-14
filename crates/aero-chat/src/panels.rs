use aero_protocol::chat::*;

pub struct PanelManager;

impl PanelManager {
    pub fn build_comfort_panel(
        user_name: Option<&str>,
        conversations: &[ConversationSummary],
        stats: ComfortStats,
    ) -> ComfortPanel {
        let greeting = match user_name {
            Some(name) => format!("Welcome back, {name}"),
            None => "Welcome to AeroBaaS".into(),
        };

        let quick_actions = vec![
            QuickAction {
                id: "new_chat".into(),
                label: "New Chat".into(),
                icon: "message-square".into(),
                action_type: QuickActionType::NewChat,
            },
            QuickAction {
                id: "deploy".into(),
                label: "Deploy Function".into(),
                icon: "rocket".into(),
                action_type: QuickActionType::DeployFunction,
            },
            QuickAction {
                id: "scan".into(),
                label: "Scan Dependencies".into(),
                icon: "shield".into(),
                action_type: QuickActionType::ScanDependencies,
            },
            QuickAction {
                id: "create_key".into(),
                label: "Create API Key".into(),
                icon: "key".into(),
                action_type: QuickActionType::CreateKey,
            },
            QuickAction {
                id: "inspector".into(),
                label: "View Inspector".into(),
                icon: "activity".into(),
                action_type: QuickActionType::ViewInspector,
            },
            QuickAction {
                id: "switch_ws".into(),
                label: "Switch Workspace".into(),
                icon: "building".into(),
                action_type: QuickActionType::SwitchWorkspace,
            },
        ];

        let recent: Vec<ConversationSummary> = conversations.iter().take(5).cloned().collect();

        ComfortPanel {
            id: "comfort-main".into(),
            greeting,
            user_name: user_name.map(|s| s.to_string()),
            mood: Mood::Calm,
            ambient_color: "#1a1b2e".into(),
            quick_actions,
            recent_conversations: recent,
            stats,
        }
    }

    pub fn default_panels() -> Vec<Panel> {
        vec![
            Panel {
                id: "chat".into(),
                panel_type: PanelType::Chat,
                title: "Chat".into(),
                config: serde_json::json!({}),
                position: 0,
                visible: true,
            },
            Panel {
                id: "code".into(),
                panel_type: PanelType::CodePreview,
                title: "Code Preview".into(),
                config: serde_json::json!({}),
                position: 1,
                visible: true,
            },
            Panel {
                id: "tokens".into(),
                panel_type: PanelType::TokenUsage,
                title: "Token Usage".into(),
                config: serde_json::json!({}),
                position: 2,
                visible: true,
            },
            Panel {
                id: "latency".into(),
                panel_type: PanelType::LatencyGraph,
                title: "Latency".into(),
                config: serde_json::json!({}),
                position: 3,
                visible: false,
            },
            Panel {
                id: "functions".into(),
                panel_type: PanelType::FunctionList,
                title: "Functions".into(),
                config: serde_json::json!({}),
                position: 4,
                visible: false,
            },
            Panel {
                id: "comfort".into(),
                panel_type: PanelType::Comfort,
                title: "Comfort".into(),
                config: serde_json::json!({}),
                position: 5,
                visible: true,
            },
        ]
    }

    pub fn build_dashboard_layout() -> PanelConfig {
        PanelConfig {
            layout: PanelLayout::Dashboard,
            theme: PanelTheme::Dark,
            comfort_mode: true,
            show_token_count: true,
            show_latency: true,
            streaming: true,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            sidebar_collapsed: false,
            panel_order: vec![
                "chat".into(),
                "code".into(),
                "tokens".into(),
                "latency".into(),
                "comfort".into(),
            ],
        }
    }

    pub fn build_split_layout() -> PanelConfig {
        PanelConfig {
            layout: PanelLayout::Split,
            theme: PanelTheme::Midnight,
            comfort_mode: true,
            show_token_count: true,
            show_latency: false,
            streaming: true,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            sidebar_collapsed: false,
            panel_order: vec!["chat".into(), "code".into()],
        }
    }

    pub fn build_triple_layout() -> PanelConfig {
        PanelConfig {
            layout: PanelLayout::Triple,
            theme: PanelTheme::Aurora,
            comfort_mode: true,
            show_token_count: true,
            show_latency: true,
            streaming: true,
            max_tokens: Some(8192),
            temperature: Some(0.8),
            sidebar_collapsed: false,
            panel_order: vec!["chat".into(), "code".into(), "tokens".into()],
        }
    }

    pub fn get_theme_colors(theme: &PanelTheme) -> ThemeColors {
        match theme {
            PanelTheme::Light => ThemeColors {
                bg: "#ffffff".into(),
                surface: "#f8f9fa".into(),
                text: "#1a1b2e".into(),
                accent: "#6366f1".into(),
                border: "#e5e7eb".into(),
            },
            PanelTheme::Dark => ThemeColors {
                bg: "#0f0f1a".into(),
                surface: "#1a1b2e".into(),
                text: "#e4e4e7".into(),
                accent: "#818cf8".into(),
                border: "#27272a".into(),
            },
            PanelTheme::Midnight => ThemeColors {
                bg: "#0a0a0f".into(),
                surface: "#12121a".into(),
                text: "#c0c0d0".into(),
                accent: "#a78bfa".into(),
                border: "#1e1e2e".into(),
            },
            PanelTheme::Aurora => ThemeColors {
                bg: "#0d0d1a".into(),
                surface: "#16162e".into(),
                text: "#e0e0f0".into(),
                accent: "#22d3ee".into(),
                border: "#2a2a4a".into(),
            },
            PanelTheme::Sunset => ThemeColors {
                bg: "#1a0f0a".into(),
                surface: "#2a1a14".into(),
                text: "#f0e0d0".into(),
                accent: "#f59e0b".into(),
                border: "#3a2a24".into(),
            },
            PanelTheme::Ocean => ThemeColors {
                bg: "#0a1a1f".into(),
                surface: "#142a30".into(),
                text: "#d0e0e8".into(),
                accent: "#06b6d4".into(),
                border: "#1e3a42".into(),
            },
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ThemeColors {
    pub bg: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
    pub border: String,
}
