use std::sync::Arc;

use aero_ai_engine::AiEngine;
use aero_protocol::chat::*;
use aero_protocol::AeroResult;
use uuid::Uuid;

use crate::store::ChatStore;

pub struct ChatService {
    ai_engine: Arc<AiEngine>,
    store: ChatStore,
}

impl ChatService {
    pub fn new(ai_engine: Arc<AiEngine>, store: ChatStore) -> Self {
        Self { ai_engine, store }
    }

    pub async fn create_conversation(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        req: CreateConversationRequest,
    ) -> AeroResult<Conversation> {
        let conversation = Conversation {
            id: Uuid::new_v4(),
            workspace_id,
            user_id,
            title: req.title.unwrap_or_else(|| "New Conversation".into()),
            model: req.model.unwrap_or_else(|| "aero-local-slm".into()),
            system_prompt: req.system_prompt,
            panel_config: req.panel_config.unwrap_or_default(),
            message_count: 0,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.store.store_conversation(conversation.clone()).await?;

        tracing::info!(
            conversation_id = %conversation.id,
            title = %conversation.title,
            "Conversation created"
        );

        Ok(conversation)
    }

    pub async fn send_message(
        &self,
        _user_id: Uuid,
        req: SendMessageRequest,
    ) -> AeroResult<SendMessageResponse> {
        let start = std::time::Instant::now();

        let conversation = self
            .store
            .get_conversation(req.conversation_id)
            .await?
            .ok_or_else(|| aero_protocol::AeroError::NotFound("Conversation not found".into()))?;

        let user_message = Message {
            id: Uuid::new_v4(),
            conversation_id: req.conversation_id,
            role: MessageRole::User,
            content: req.content.clone(),
            tokens: estimate_tokens(&req.content),
            model: None,
            latency_ms: None,
            attachments: req.attachments.unwrap_or_default(),
            panel_id: None,
            created_at: chrono::Utc::now(),
        };

        self.store.store_message(user_message.clone()).await?;

        let prompt = if let Some(ref system) = conversation.system_prompt {
            format!(
                "{system}\n\nUser: {content}\nAssistant:",
                content = req.content
            )
        } else {
            req.content.clone()
        };

        let ai_response = self.ai_engine.generate_text(&prompt).await?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let assistant_tokens = estimate_tokens(&ai_response);

        let assistant_message = Message {
            id: Uuid::new_v4(),
            conversation_id: req.conversation_id,
            role: MessageRole::Assistant,
            content: ai_response,
            tokens: assistant_tokens,
            model: Some(conversation.model.clone()),
            latency_ms: Some(latency_ms),
            attachments: vec![],
            panel_id: None,
            created_at: chrono::Utc::now(),
        };

        self.store.store_message(assistant_message.clone()).await?;

        let total_tokens = user_message.tokens as u64 + assistant_tokens as u64;
        self.store
            .update_conversation_stats(req.conversation_id, total_tokens)
            .await?;

        let conversation = self
            .store
            .get_conversation(req.conversation_id)
            .await?
            .unwrap();

        Ok(SendMessageResponse {
            message: user_message,
            assistant_message,
            conversation,
            tokens_used: total_tokens,
            latency_ms,
        })
    }

    pub async fn list_conversations(&self, user_id: Uuid) -> AeroResult<Vec<ConversationSummary>> {
        self.store.list_conversations(user_id).await
    }

    pub async fn get_conversation(&self, id: Uuid) -> AeroResult<Conversation> {
        self.store
            .get_conversation(id)
            .await?
            .ok_or_else(|| aero_protocol::AeroError::NotFound("Conversation not found".into()))
    }

    pub async fn get_messages(&self, conversation_id: Uuid) -> AeroResult<Vec<Message>> {
        self.store.get_messages(conversation_id).await
    }

    pub async fn delete_conversation(&self, id: Uuid) -> AeroResult<()> {
        self.store.delete_conversation(id).await
    }

    pub async fn update_panel_config(
        &self,
        conversation_id: Uuid,
        config: PanelConfig,
    ) -> AeroResult<Conversation> {
        self.store
            .update_panel_config(conversation_id, config)
            .await
    }
}

fn estimate_tokens(text: &str) -> u32 {
    ((text.len() as f32) / 4.0).ceil() as u32
}
