use std::sync::Arc;

use aero_protocol::chat::*;
use aero_protocol::AeroResult;
use parking_lot::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

pub struct ChatStore {
    conversations: RwLock<HashMap<Uuid, Conversation>>,
    messages: RwLock<HashMap<Uuid, Vec<Message>>>,
}

impl ChatStore {
    pub fn new() -> Self {
        Self {
            conversations: RwLock::new(HashMap::new()),
            messages: RwLock::new(HashMap::new()),
        }
    }

    pub async fn store_conversation(&self, conv: Conversation) -> AeroResult<()> {
        self.conversations.write().insert(conv.id, conv);
        Ok(())
    }

    pub async fn get_conversation(&self, id: Uuid) -> AeroResult<Option<Conversation>> {
        Ok(self.conversations.read().get(&id).cloned())
    }

    pub async fn list_conversations(&self, user_id: Uuid) -> AeroResult<Vec<ConversationSummary>> {
        let convs = self.conversations.read();
        let mut summaries: Vec<ConversationSummary> = convs
            .values()
            .filter(|c| c.user_id == user_id)
            .map(|c| {
                let preview = self
                    .messages
                    .read()
                    .get(&c.id)
                    .and_then(|msgs| msgs.last())
                    .map(|m| m.content.chars().take(100).collect())
                    .unwrap_or_default();
                ConversationSummary {
                    id: c.id,
                    title: c.title.clone(),
                    preview,
                    message_count: c.message_count,
                    updated_at: c.updated_at,
                }
            })
            .collect();
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    pub async fn delete_conversation(&self, id: Uuid) -> AeroResult<()> {
        self.conversations.write().remove(&id);
        self.messages.write().remove(&id);
        Ok(())
    }

    pub async fn store_message(&self, msg: Message) -> AeroResult<()> {
        let mut messages = self.messages.write();
        messages.entry(msg.conversation_id).or_default().push(msg.clone());

        let mut convs = self.conversations.write();
        if let Some(conv) = convs.get_mut(&msg.conversation_id) {
            conv.message_count += 1;
            conv.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    pub async fn get_messages(&self, conversation_id: Uuid) -> AeroResult<Vec<Message>> {
        Ok(self
            .messages
            .read()
            .get(&conversation_id)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn update_conversation_stats(
        &self,
        conversation_id: Uuid,
        tokens_added: u64,
    ) -> AeroResult<()> {
        let mut convs = self.conversations.write();
        if let Some(conv) = convs.get_mut(&conversation_id) {
            conv.tokens_used += tokens_added;
            conv.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    pub async fn update_panel_config(
        &self,
        conversation_id: Uuid,
        config: PanelConfig,
    ) -> AeroResult<Conversation> {
        let mut convs = self.conversations.write();
        let conv = convs
            .get_mut(&conversation_id)
            .ok_or_else(|| aero_protocol::AeroError::NotFound("Conversation not found".into()))?;
        conv.panel_config = config;
        conv.updated_at = chrono::Utc::now();
        Ok(conv.clone())
    }
}

impl Default for ChatStore {
    fn default() -> Self {
        Self::new()
    }
}
