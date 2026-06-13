// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use eltk_adapters::ClaudeCodeAdapter;
use eltk_core::{
    CostSource, DedupScope, IdentityConfidence, RecordWarning, ScanConfig, SourceKind,
    StableUsageKey, UsageAdapter, UsageSource,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude-code")
}

fn fixture_path(name: &str) -> PathBuf {
    fixture_dir().join(name)
}

#[test]
fn discovers_jsonl_sources_from_configured_roots() {
    let adapter = ClaudeCodeAdapter::new();
    let config = ScanConfig {
        roots: vec![fixture_dir()],
    };

    let sources = adapter.discover(&config).unwrap();

    assert_eq!(sources.len(), 2);
    assert!(
        sources
            .iter()
            .all(|source| source.kind == SourceKind::ClaudeCodeJsonl)
    );
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("role-only-records.jsonl"))
    );
    assert!(
        sources
            .iter()
            .any(|source| source.path.ends_with("usage-records.jsonl"))
    );
}

#[test]
fn scans_sanitized_claude_usage_records() {
    let adapter = ClaudeCodeAdapter::new();
    let source = UsageSource::new(
        SourceKind::ClaudeCodeJsonl,
        fixture_path("usage-records.jsonl"),
    );
    let mut records = Vec::new();

    let stats = adapter.scan_source(&source, &mut records).unwrap();

    assert_eq!(stats.records_seen, 5);
    assert_eq!(stats.records_emitted, 2);
    assert_eq!(stats.warnings, 2);
    assert_eq!(stats.excluded_usage.synthetic_records, 1);
    assert_eq!(stats.excluded_usage.synthetic_usage.tokens.input_tokens, 1);
    assert_eq!(stats.excluded_usage.synthetic_usage.tokens.output_tokens, 1);
    assert_eq!(stats.excluded_usage.api_error_records, 1);
    assert_eq!(stats.excluded_usage.api_error_usage.tokens.input_tokens, 1);
    assert_eq!(stats.excluded_usage.api_error_usage.tokens.output_tokens, 1);

    let record = &records[0];
    assert_eq!(record.dedup.scope, DedupScope::Global);
    assert_eq!(record.context.session_id.as_deref(), Some("session-main"));
    assert_eq!(record.context.message_id.as_deref(), Some("msg-main-1"));
    assert_eq!(record.context.request_id.as_deref(), Some("req-main-1"));
    assert_eq!(record.context.cwd.as_deref(), Some("/workspace/example"));
    assert_eq!(record.context.git_branch.as_deref(), Some("main"));
    assert_eq!(record.context.version.as_deref(), Some("1.2.3"));
    assert!(!record.context.is_sidechain);
    assert_eq!(record.actor.agent.as_str(), "claude_code");
    assert_eq!(
        record.actor.provider.as_ref().unwrap().as_str(),
        "anthropic"
    );
    assert_eq!(
        record.actor.model_raw.as_deref(),
        Some("claude-sonnet-4-20250514")
    );
    assert_eq!(record.usage.tokens.input_tokens, 100);
    assert_eq!(record.usage.tokens.output_tokens, 25);
    assert_eq!(record.usage.tokens.cache_read_input_tokens, 40);
    assert_eq!(record.usage.tokens.cache_creation_input_tokens, 12);
    assert_eq!(
        record.usage.tokens.cache_creation_ephemeral_5m_input_tokens,
        5
    );
    assert_eq!(
        record.usage.tokens.cache_creation_ephemeral_1h_input_tokens,
        7
    );
    assert_eq!(record.usage.tokens.reasoning_output_tokens, 3);
    assert_eq!(record.usage.reported_total_tokens, Some(200));
    assert_eq!(record.usage.extra_total_tokens, 20);
    assert_eq!(record.usage.server_tools.web_search_requests, 2);
    assert_eq!(
        record.usage.server_tools.other.get("mcp_tool_requests"),
        Some(&3)
    );
    assert_eq!(record.usage.server_tools.total_requests(), 5);
    assert_eq!(record.cost.source, CostSource::Reported);
    assert_eq!(record.cost.amount_nanos_usd.unwrap().as_i64(), 12_345_000);
    assert!(
        record
            .quality
            .warnings
            .contains(&RecordWarning::SourceTotalMismatch {
                reported: 200,
                computed: 180,
            })
    );
}

#[test]
fn keeps_message_only_records_with_weak_identity() {
    let adapter = ClaudeCodeAdapter::new();
    let source = UsageSource::new(
        SourceKind::ClaudeCodeJsonl,
        fixture_path("usage-records.jsonl"),
    );
    let mut records = Vec::new();

    adapter.scan_source(&source, &mut records).unwrap();

    let record = &records[1];
    assert_eq!(record.quality.identity_confidence, IdentityConfidence::Weak);
    assert!(
        record
            .quality
            .warnings
            .contains(&RecordWarning::MissingRequestId)
    );
    assert!(matches!(
        record.dedup.stable_key,
        StableUsageKey::ClaudeMessageOnly { .. }
    ));
}

#[test]
fn accepts_role_only_assistant_usage_records() {
    let adapter = ClaudeCodeAdapter::new();
    let source = UsageSource::new(
        SourceKind::ClaudeCodeJsonl,
        fixture_path("role-only-records.jsonl"),
    );
    let mut records = Vec::new();

    let stats = adapter.scan_source(&source, &mut records).unwrap();

    assert_eq!(stats.records_seen, 4);
    assert_eq!(stats.records_emitted, 1);
    assert_eq!(stats.warnings, 0);
    assert_eq!(stats.excluded_usage.api_error_records, 1);
    assert_eq!(stats.excluded_usage.api_error_usage.tokens.input_tokens, 2);
    assert_eq!(stats.excluded_usage.api_error_usage.tokens.output_tokens, 3);

    let record = &records[0];
    assert_eq!(
        record.context.session_id.as_deref(),
        Some("session-role-only")
    );
    assert_eq!(
        record.context.message_id.as_deref(),
        Some("msg-role-only-1")
    );
    assert_eq!(
        record.context.request_id.as_deref(),
        Some("req-role-only-1")
    );
    assert_eq!(record.usage.tokens.input_tokens, 100);
    assert_eq!(record.usage.tokens.output_tokens, 50);
    assert_eq!(record.usage.tokens.cache_creation_input_tokens, 25);
    assert_eq!(record.usage.tokens.cache_read_input_tokens, 10);
}
