// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use eltk_core::{CostSource, UsageRecord, UsageRecordId};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    order: usize,
    record: UsageRecord,
}

pub fn merge_claude_records(records: impl IntoIterator<Item = UsageRecord>) -> Vec<UsageRecord> {
    let exact_merged = merge_exact_record_ids(records);
    merge_claude_message_replays(exact_merged)
}

fn merge_exact_record_ids(records: impl IntoIterator<Item = UsageRecord>) -> Vec<Candidate> {
    let mut merged = Vec::<Candidate>::new();
    let mut by_id = BTreeMap::<UsageRecordId, usize>::new();

    for (order, record) in records.into_iter().enumerate() {
        let candidate = Candidate { order, record };

        if let Some(index) = by_id.get(&candidate.record.id).copied() {
            if should_replace(&candidate, &merged[index]) {
                merged[index] = candidate;
            }
            continue;
        }

        by_id.insert(candidate.record.id.clone(), merged.len());
        merged.push(candidate);
    }

    merged
}

fn merge_claude_message_replays(records: Vec<Candidate>) -> Vec<UsageRecord> {
    let mut merged = Vec::<Candidate>::new();
    let mut by_message_id = BTreeMap::<String, usize>::new();

    for candidate in records {
        let message_id = claude_message_id(&candidate).map(str::to_owned);
        let Some(message_id) = message_id else {
            merged.push(candidate);
            continue;
        };

        if let Some(index) = by_message_id.get(&message_id).copied()
            && should_merge_by_message_id(&candidate, &merged[index])
        {
            if should_replace(&candidate, &merged[index]) {
                merged[index] = candidate;
            }
            continue;
        }

        by_message_id.entry(message_id).or_insert(merged.len());
        merged.push(candidate);
    }

    merged
        .into_iter()
        .map(|candidate| candidate.record)
        .collect()
}

fn claude_message_id(candidate: &Candidate) -> Option<&str> {
    (candidate.record.dedup.agent.as_str() == "claude_code")
        .then_some(candidate.record.context.message_id.as_deref())
        .flatten()
}

fn should_merge_by_message_id(left: &Candidate, right: &Candidate) -> bool {
    left.record.context.is_sidechain
        || right.record.context.is_sidechain
        || left.record.context.request_id.is_none()
        || right.record.context.request_id.is_none()
}

fn should_replace(candidate: &Candidate, existing: &Candidate) -> bool {
    replacement_score(candidate) > replacement_score(existing)
}

fn replacement_score(candidate: &Candidate) -> ReplacementScore {
    // Streamed Claude rows should grow monotonically; prefer the largest known
    // total so a truncated later flush does not undercount.
    ReplacementScore {
        is_parent: !candidate.record.context.is_sidechain,
        has_request_id: candidate.record.context.request_id.is_some(),
        token_total: candidate.record.usage.known_total_tokens(),
        cost_nanos: cost_nanos(&candidate.record),
        source_line: candidate.record.source.line_number.unwrap_or_default(),
        source_offset: candidate.record.source.byte_offset.unwrap_or_default(),
        order: candidate.order,
    }
}

fn cost_nanos(record: &UsageRecord) -> i64 {
    match record.cost.source {
        CostSource::Reported | CostSource::Calculated => record
            .cost
            .amount_nanos_usd
            .map(|amount| amount.as_i64())
            .unwrap_or_default(),
        CostSource::Unknown => 0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ReplacementScore {
    is_parent: bool,
    has_request_id: bool,
    token_total: u64,
    cost_nanos: i64,
    source_line: u64,
    source_offset: u64,
    order: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use eltk_core::{
        AgentId, CostInfo, DedupIdentity, DedupScope, MeteredUsage, ProviderId, RecordQuality,
        SourceKind, SourceProvenance, StableUsageKey, Timestamp, TokenUsage, UsageActor,
        UsageContext, UsageRecordParts, UsdNanos,
    };

    #[test]
    fn keeps_largest_streamed_record_for_same_request() {
        let records = merge_claude_records([
            test_record("msg-1", Some("req-1"), false, 10, 1),
            test_record("msg-1", Some("req-1"), false, 25, 2),
            test_record("msg-1", Some("req-1"), false, 20, 3),
        ]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].usage.known_total_tokens(), 25);
        assert_eq!(records[0].source.line_number, Some(2));
    }

    #[test]
    fn prefers_parent_record_over_sidechain_replay() {
        let records = merge_claude_records([
            test_record("msg-1", Some("req-parent"), false, 10, 1),
            test_record("msg-1", Some("req-sidechain"), true, 50, 2),
        ]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].context.request_id.as_deref(), Some("req-parent"));
        assert!(!records[0].context.is_sidechain);
    }

    #[test]
    fn keeps_unique_sidechain_responses() {
        let records = merge_claude_records([
            test_record("msg-parent", Some("req-parent"), false, 10, 1),
            test_record("msg-sidechain", Some("req-sidechain"), true, 50, 2),
        ]);

        assert_eq!(records.len(), 2);
        assert!(records.iter().any(|record| record.context.is_sidechain));
    }

    #[test]
    fn replaces_message_only_weak_record_with_strong_request_record() {
        let records = merge_claude_records([
            test_record("msg-1", None, false, 50, 1),
            test_record("msg-1", Some("req-1"), false, 10, 2),
        ]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].context.request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn keeps_source_fallback_records_distinct() {
        let records = merge_claude_records([
            fallback_record(10, 1),
            fallback_record(10, 2),
            fallback_record(10, 3),
        ]);

        assert_eq!(records.len(), 3);
    }

    #[test]
    fn collapses_exact_source_fallback_duplicates() {
        let records = merge_claude_records([
            fallback_record_at_line("same-line", 10, 1),
            fallback_record_at_line("same-line", 25, 1),
        ]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].usage.known_total_tokens(), 25);
    }

    fn test_record(
        message_id: &str,
        request_id: Option<&str>,
        is_sidechain: bool,
        token_total: u64,
        line_number: u64,
    ) -> UsageRecord {
        let dedup = match request_id {
            Some(request_id) => DedupIdentity::claude_message_request(message_id, request_id),
            None => DedupIdentity::new(
                DedupScope::Global,
                AgentId::claude_code(),
                Some(ProviderId::anthropic()),
                StableUsageKey::ClaudeMessageOnly {
                    message_id: message_id.to_owned(),
                },
            ),
        };

        UsageRecord::from_parts(UsageRecordParts {
            dedup,
            observed_at: Timestamp::from("2026-06-13T00:00:00Z"),
            source: source(line_number),
            actor: UsageActor::claude_code("claude-test"),
            context: UsageContext {
                message_id: Some(message_id.to_owned()),
                request_id: request_id.map(str::to_owned),
                is_sidechain,
                ..UsageContext::default()
            },
            usage: MeteredUsage {
                tokens: TokenUsage {
                    input_tokens: token_total,
                    ..TokenUsage::default()
                },
                ..MeteredUsage::default()
            },
            cost: CostInfo::reported(UsdNanos::new(token_total as i64)),
            quality: RecordQuality::default(),
        })
    }

    fn fallback_record(token_total: u64, line_number: u64) -> UsageRecord {
        fallback_record_at_line(&format!("line-{line_number}"), token_total, line_number)
    }

    fn fallback_record_at_line(line_hash: &str, token_total: u64, line_number: u64) -> UsageRecord {
        UsageRecord::from_parts(UsageRecordParts {
            dedup: DedupIdentity::new(
                DedupScope::SourceFile,
                AgentId::claude_code(),
                Some(ProviderId::anthropic()),
                StableUsageKey::SourceFallback {
                    source_fingerprint: "source-a".to_owned(),
                    byte_offset: Some(line_number * 10),
                    line_number: Some(line_number),
                    line_hash: line_hash.to_owned(),
                },
            ),
            observed_at: Timestamp::from("2026-06-13T00:00:00Z"),
            source: source(line_number),
            actor: UsageActor::claude_code("claude-test"),
            context: UsageContext::default(),
            usage: MeteredUsage {
                tokens: TokenUsage {
                    input_tokens: token_total,
                    ..TokenUsage::default()
                },
                ..MeteredUsage::default()
            },
            cost: CostInfo::unknown(),
            quality: RecordQuality::default(),
        })
    }

    fn source(line_number: u64) -> SourceProvenance {
        let mut source = SourceProvenance::new(
            SourceKind::ClaudeCodeJsonl,
            "source-a",
            "test-parser",
            Timestamp::from("2026-06-13T00:00:01Z"),
        );
        source.line_number = Some(line_number);
        source.byte_offset = Some(line_number * 10);
        source
    }
}
