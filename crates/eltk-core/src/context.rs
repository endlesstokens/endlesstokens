// SPDX-License-Identifier: MIT

use crate::identity::{AgentId, ProviderId};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct UsageActor {
    pub agent: AgentId,
    pub agent_display_name: String,
    pub provider: Option<ProviderId>,
    pub model_raw: Option<String>,
    pub model_normalized: Option<String>,
    pub model_variant: Option<ModelVariant>,
}

impl UsageActor {
    pub fn new(agent: AgentId, agent_display_name: impl Into<String>) -> Self {
        Self {
            agent,
            agent_display_name: agent_display_name.into(),
            provider: None,
            model_raw: None,
            model_normalized: None,
            model_variant: None,
        }
    }

    pub fn claude_code(model_raw: impl Into<String>) -> Self {
        let model_raw = model_raw.into();
        Self {
            agent: AgentId::claude_code(),
            agent_display_name: "Claude Code".to_owned(),
            provider: Some(ProviderId::anthropic()),
            model_normalized: Some(model_raw.clone()),
            model_raw: Some(model_raw),
            model_variant: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ModelVariant {
    Standard,
    Fast,
    Priority,
    Unknown(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct UsageContext {
    pub session_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub message_id: Option<String>,
    pub request_id: Option<String>,
    pub turn_id: Option<String>,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub git_branch: Option<String>,
    pub version: Option<String>,
    pub agent_name: Option<String>,
    pub is_sidechain: bool,
    pub is_api_error_message: bool,
    pub is_synthetic: bool,
}
