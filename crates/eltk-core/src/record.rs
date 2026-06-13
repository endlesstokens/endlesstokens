// SPDX-License-Identifier: MIT

use crate::{
    context::{UsageActor, UsageContext},
    cost::CostInfo,
    identity::{DedupIdentity, UsageRecordId},
    quality::RecordQuality,
    source::SourceProvenance,
    time::Timestamp,
    usage::MeteredUsage,
};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct UsageRecord {
    pub id: UsageRecordId,
    pub dedup: DedupIdentity,
    pub observed_at: Timestamp,
    pub source: SourceProvenance,
    pub actor: UsageActor,
    pub context: UsageContext,
    pub usage: MeteredUsage,
    pub cost: CostInfo,
    pub quality: RecordQuality,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct UsageRecordParts {
    pub dedup: DedupIdentity,
    pub observed_at: Timestamp,
    pub source: SourceProvenance,
    pub actor: UsageActor,
    pub context: UsageContext,
    pub usage: MeteredUsage,
    pub cost: CostInfo,
    pub quality: RecordQuality,
}

impl UsageRecord {
    pub fn from_parts(parts: UsageRecordParts) -> Self {
        let id = UsageRecordId::from_dedup_identity(&parts.dedup);
        Self {
            id,
            dedup: parts.dedup,
            observed_at: parts.observed_at,
            source: parts.source,
            actor: parts.actor,
            context: parts.context,
            usage: parts.usage,
            cost: parts.cost,
            quality: parts.quality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{AgentId, DedupScope, ProviderId, StableUsageKey},
        source::SourceKind,
    };

    #[test]
    fn usage_record_id_is_derived_from_dedup_identity() {
        let dedup = DedupIdentity::new(
            DedupScope::Global,
            AgentId::claude_code(),
            Some(ProviderId::anthropic()),
            StableUsageKey::ClaudeMessageRequest {
                message_id: "msg_123".to_owned(),
                request_id: "req_456".to_owned(),
            },
        );

        let record = UsageRecord::from_parts(UsageRecordParts {
            dedup: dedup.clone(),
            observed_at: Timestamp::from("2026-06-12T00:00:00Z"),
            source: SourceProvenance::new(
                SourceKind::ClaudeCodeJsonl,
                "source-path-hash",
                "claude-jsonl-v1",
                Timestamp::from("2026-06-12T00:00:01Z"),
            ),
            actor: UsageActor::claude_code("claude-sonnet-4-20250514"),
            context: UsageContext {
                session_id: Some("session_123".to_owned()),
                message_id: Some("msg_123".to_owned()),
                request_id: Some("req_456".to_owned()),
                ..UsageContext::default()
            },
            usage: MeteredUsage::default(),
            cost: CostInfo::unknown(),
            quality: RecordQuality::default(),
        });

        assert_eq!(record.id, UsageRecordId::from_dedup_identity(&dedup));
    }
}
