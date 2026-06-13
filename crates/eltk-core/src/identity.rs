// SPDX-License-Identifier: MIT

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UsageRecordId(String);

impl UsageRecordId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn from_dedup_identity(dedup: &DedupIdentity) -> Self {
        stable_id_from_bytes(dedup.canonical_string().as_bytes())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UsageRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for UsageRecordId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for UsageRecordId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn claude_code() -> Self {
        Self::new("claude_code")
    }

    pub fn codex() -> Self {
        Self::new("codex")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AgentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AgentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn anthropic() -> Self {
        Self::new("anthropic")
    }

    pub fn openai() -> Self {
        Self::new("openai")
    }

    pub fn vertex_ai() -> Self {
        Self::new("vertex_ai")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ProviderId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DedupIdentity {
    pub scope: DedupScope,
    pub agent: AgentId,
    pub provider: Option<ProviderId>,
    pub stable_key: StableUsageKey,
}

impl DedupIdentity {
    pub fn new(
        scope: DedupScope,
        agent: AgentId,
        provider: Option<ProviderId>,
        stable_key: StableUsageKey,
    ) -> Self {
        Self {
            scope,
            agent,
            provider,
            stable_key,
        }
    }

    pub fn claude_message_request(
        message_id: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Self {
        Self::new(
            DedupScope::Global,
            AgentId::claude_code(),
            Some(ProviderId::anthropic()),
            StableUsageKey::ClaudeMessageRequest {
                message_id: message_id.into(),
                request_id: request_id.into(),
            },
        )
    }

    pub fn canonical_string(&self) -> String {
        let mut canonical = String::from("usage-record-id:v1");
        append_field(&mut canonical, "scope", self.scope.as_str());
        append_field(&mut canonical, "agent", self.agent.as_str());
        append_field(
            &mut canonical,
            "provider",
            self.provider.as_ref().map(ProviderId::as_str).unwrap_or(""),
        );
        self.stable_key.append_canonical(&mut canonical);
        canonical
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DedupScope {
    Global,
    Machine,
    SourceFile,
}

impl DedupScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Machine => "machine",
            Self::SourceFile => "source_file",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum StableUsageKey {
    ClaudeMessageRequest {
        message_id: String,
        request_id: String,
    },
    ClaudeMessageOnly {
        message_id: String,
    },
    CodexTokenTotal {
        upstream_session_id: String,
        model: String,
        input_tokens: u64,
        cached_input_tokens: u64,
        output_tokens: u64,
        reasoning_output_tokens: u64,
    },
    AdapterEventUid {
        event_uid: String,
    },
    SourceFallback {
        source_fingerprint: String,
        byte_offset: Option<u64>,
        line_number: Option<u64>,
        line_hash: String,
    },
}

impl StableUsageKey {
    fn append_canonical(&self, canonical: &mut String) {
        match self {
            Self::ClaudeMessageRequest {
                message_id,
                request_id,
            } => {
                append_field(canonical, "kind", "claude_message_request");
                append_field(canonical, "message_id", message_id);
                append_field(canonical, "request_id", request_id);
            }
            Self::ClaudeMessageOnly { message_id } => {
                append_field(canonical, "kind", "claude_message_only");
                append_field(canonical, "message_id", message_id);
            }
            Self::CodexTokenTotal {
                upstream_session_id,
                model,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
            } => {
                append_field(canonical, "kind", "codex_token_total");
                append_field(canonical, "upstream_session_id", upstream_session_id);
                append_field(canonical, "model", model);
                append_u64(canonical, "input_tokens", *input_tokens);
                append_u64(canonical, "cached_input_tokens", *cached_input_tokens);
                append_u64(canonical, "output_tokens", *output_tokens);
                append_u64(
                    canonical,
                    "reasoning_output_tokens",
                    *reasoning_output_tokens,
                );
            }
            Self::AdapterEventUid { event_uid } => {
                append_field(canonical, "kind", "adapter_event_uid");
                append_field(canonical, "event_uid", event_uid);
            }
            Self::SourceFallback {
                source_fingerprint,
                byte_offset,
                line_number,
                line_hash,
            } => {
                append_field(canonical, "kind", "source_fallback");
                append_field(canonical, "source_fingerprint", source_fingerprint);
                append_optional_u64(canonical, "byte_offset", *byte_offset);
                append_optional_u64(canonical, "line_number", *line_number);
                append_field(canonical, "line_hash", line_hash);
            }
        }
    }
}

fn append_field(canonical: &mut String, name: &str, value: &str) {
    canonical.push('|');
    canonical.push_str(name);
    canonical.push('=');
    canonical.push_str(&value.len().to_string());
    canonical.push(':');
    canonical.push_str(value);
}

fn append_u64(canonical: &mut String, name: &str, value: u64) {
    append_field(canonical, name, &value.to_string());
}

fn append_optional_u64(canonical: &mut String, name: &str, value: Option<u64>) {
    match value {
        Some(value) => append_u64(canonical, name, value),
        None => append_field(canonical, name, ""),
    }
}

fn stable_id_from_bytes(bytes: &[u8]) -> UsageRecordId {
    const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    UsageRecordId::new(format!("ur_{hash:032x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_stable_record_id_from_claude_global_identity() {
        let identity = DedupIdentity::claude_message_request("msg_123", "req_456");

        let first = UsageRecordId::from_dedup_identity(&identity);
        let second = UsageRecordId::from_dedup_identity(&identity);

        assert_eq!(first, second);
        assert_eq!(first.as_str().len(), 35);
        assert!(first.as_str().starts_with("ur_"));
    }

    #[test]
    fn distinct_stable_keys_have_distinct_record_ids() {
        let first = UsageRecordId::from_dedup_identity(&DedupIdentity::claude_message_request(
            "msg_123", "req_456",
        ));
        let second = UsageRecordId::from_dedup_identity(&DedupIdentity::claude_message_request(
            "msg_123", "req_789",
        ));

        assert_ne!(first, second);
    }

    #[test]
    fn fallback_identity_is_source_scoped() {
        let identity = DedupIdentity::new(
            DedupScope::SourceFile,
            AgentId::claude_code(),
            Some(ProviderId::anthropic()),
            StableUsageKey::SourceFallback {
                source_fingerprint: "path-hash".to_owned(),
                byte_offset: Some(42),
                line_number: Some(7),
                line_hash: "line-hash".to_owned(),
            },
        );

        assert_eq!(identity.scope, DedupScope::SourceFile);
        assert!(identity.canonical_string().contains("source_fallback"));
    }
}
