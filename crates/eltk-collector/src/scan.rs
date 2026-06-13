// SPDX-License-Identifier: MIT

use eltk_adapters::ClaudeCodeAdapter;
use eltk_core::{
    AdapterResult, ScanConfig, ScanExcludedUsageStats, ScanSourceStats, UsageAdapter, UsageRecord,
    UsageSource,
};

use crate::dedup::merge_claude_records;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CollectorScanResult {
    pub sources: Vec<UsageSource>,
    pub records: Vec<UsageRecord>,
    pub source_errors: Vec<CollectSourceError>,
    pub stats: CollectScanStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectSourceError {
    pub source: UsageSource,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct CollectScanStats {
    pub sources_discovered: u64,
    pub sources_scanned: u64,
    pub records_seen: u64,
    pub records_emitted: u64,
    pub records_after_merge: u64,
    pub warnings: u64,
    pub excluded_usage: ScanExcludedUsageStats,
}

impl CollectScanStats {
    fn add_source_stats(&mut self, source_stats: ScanSourceStats) {
        self.sources_scanned += 1;
        self.records_seen += source_stats.records_seen;
        self.records_emitted += source_stats.records_emitted;
        self.warnings += source_stats.warnings;
        self.excluded_usage
            .saturating_add_assign(&source_stats.excluded_usage);
    }
}

pub fn collect_claude_records(config: &ScanConfig) -> AdapterResult<CollectorScanResult> {
    let adapter = ClaudeCodeAdapter::new();
    collect_records_with_adapter(&adapter, config, merge_claude_records)
}

fn collect_records_with_adapter<A, M>(
    adapter: &A,
    config: &ScanConfig,
    merge_records: M,
) -> AdapterResult<CollectorScanResult>
where
    A: UsageAdapter,
    M: FnOnce(Vec<UsageRecord>) -> Vec<UsageRecord>,
{
    let sources = adapter.discover(config)?;
    let mut stats = CollectScanStats {
        sources_discovered: sources.len() as u64,
        ..CollectScanStats::default()
    };
    let mut records = Vec::new();
    let mut source_errors = Vec::new();

    for source in &sources {
        match adapter.scan_source(source, &mut records) {
            Ok(source_stats) => stats.add_source_stats(source_stats),
            Err(error) => {
                stats.sources_scanned += 1;
                stats.warnings += 1;
                source_errors.push(CollectSourceError {
                    source: source.clone(),
                    message: error.to_string(),
                });
            }
        }
    }

    records = merge_records(records);
    stats.records_after_merge = records.len() as u64;

    Ok(CollectorScanResult {
        sources,
        records,
        source_errors,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eltk_core::{AdapterError, SourceKind, UsageRecordSink};
    use std::{
        fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn collects_and_merges_claude_records_from_configured_root() {
        let root = TestRoot::new("collects-and-merges");
        root.write_jsonl(
            "projects/example/session.jsonl",
            [
                streamed_record("msg-1", "req-1", 10, 1),
                streamed_record("msg-1", "req-1", 25, 2),
                streamed_record("msg-1", "req-1", 20, 3),
                sidechain_record("msg-1", "req-sidechain", 50, 4),
                streamed_record("msg-2", "req-2", 7, 5),
            ]
            .join("\n"),
        );

        let result = collect_claude_records(&ScanConfig {
            roots: vec![root.path().to_owned()],
        })
        .unwrap();

        assert_eq!(result.stats.sources_discovered, 1);
        assert_eq!(result.stats.sources_scanned, 1);
        assert_eq!(result.stats.records_seen, 5);
        assert_eq!(result.stats.records_emitted, 5);
        assert_eq!(result.stats.records_after_merge, 2);
        assert_eq!(result.records.len(), 2);
        assert_eq!(
            result
                .records
                .iter()
                .find(|record| record.context.message_id.as_deref() == Some("msg-1"))
                .unwrap()
                .usage
                .known_total_tokens(),
            25
        );
        assert!(result.records.iter().any(|record| {
            record.context.message_id.as_deref() == Some("msg-2")
                && record.usage.known_total_tokens() == 7
        }));
    }

    #[test]
    fn aggregates_scan_stats_across_sources() {
        let root = TestRoot::new("aggregates-stats");
        root.write_jsonl(
            "projects/example/a.jsonl",
            [
                streamed_record("msg-a", "req-a", 10, 1),
                user_record(),
                synthetic_record(2, 2),
            ]
            .join("\n"),
        );
        root.write_jsonl(
            "projects/example/b.jsonl",
            [
                streamed_record("msg-b", "req-b", 20, 1),
                api_error_record(3, 2),
            ]
            .join("\n"),
        );

        let result = collect_claude_records(&ScanConfig {
            roots: vec![root.path().to_owned()],
        })
        .unwrap();

        assert_eq!(result.stats.sources_discovered, 2);
        assert_eq!(result.stats.sources_scanned, 2);
        assert_eq!(result.stats.records_seen, 5);
        assert_eq!(result.stats.records_emitted, 2);
        assert_eq!(result.stats.records_after_merge, 2);
        assert_eq!(result.stats.excluded_usage.synthetic_records, 1);
        assert_eq!(
            result
                .stats
                .excluded_usage
                .synthetic_usage
                .tokens
                .input_tokens,
            2
        );
        assert_eq!(result.stats.excluded_usage.api_error_records, 1);
        assert_eq!(
            result
                .stats
                .excluded_usage
                .api_error_usage
                .tokens
                .input_tokens,
            3
        );
        assert_eq!(result.records.len(), 2);
    }

    #[test]
    fn keeps_records_from_other_sources_when_one_source_fails() {
        let adapter = FakeAdapter {
            sources: vec![
                UsageSource::new(SourceKind::ClaudeCodeJsonl, "ok.jsonl"),
                UsageSource::new(SourceKind::ClaudeCodeJsonl, "bad.jsonl"),
            ],
        };

        let result =
            collect_records_with_adapter(&adapter, &ScanConfig::default(), |records| records)
                .unwrap();

        assert_eq!(result.records.len(), 1);
        assert_eq!(
            result.records[0].context.message_id.as_deref(),
            Some("msg-ok")
        );
        assert_eq!(result.stats.sources_discovered, 2);
        assert_eq!(result.stats.sources_scanned, 2);
        assert_eq!(result.stats.records_seen, 1);
        assert_eq!(result.stats.records_emitted, 1);
        assert_eq!(result.stats.records_after_merge, 1);
        assert_eq!(result.stats.warnings, 1);
        assert_eq!(result.source_errors.len(), 1);
        assert!(result.source_errors[0].source.path.ends_with("bad.jsonl"));
        assert_eq!(result.source_errors[0].message, "failed source");
    }

    #[derive(Clone, Debug)]
    struct FakeAdapter {
        sources: Vec<UsageSource>,
    }

    impl UsageAdapter for FakeAdapter {
        fn id(&self) -> eltk_core::AgentId {
            eltk_core::AgentId::claude_code()
        }

        fn discover(&self, _config: &ScanConfig) -> AdapterResult<Vec<UsageSource>> {
            Ok(self.sources.clone())
        }

        fn scan_source(
            &self,
            source: &UsageSource,
            sink: &mut dyn UsageRecordSink,
        ) -> AdapterResult<ScanSourceStats> {
            if source.path.ends_with("bad.jsonl") {
                return Err(AdapterError::new("failed source"));
            }

            sink.push(test_record("msg-ok", "req-ok", 1));
            Ok(ScanSourceStats {
                records_seen: 1,
                records_emitted: 1,
                warnings: 0,
                ..ScanSourceStats::default()
            })
        }
    }

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir()
                .join(format!("eltk-collector-{name}-{}-{unique}", process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_jsonl(&self, relative_path: &str, content: String) {
            let path = self.path.join(relative_path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn streamed_record(
        message_id: &str,
        request_id: &str,
        input_tokens: u64,
        minute: u64,
    ) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-06-13T00:{minute:02}:00Z","sessionId":"session-main","cwd":"/workspace/example","requestId":"{request_id}","message":{{"id":"{message_id}","model":"claude-sonnet-4-20250514","usage":{{"input_tokens":{input_tokens},"output_tokens":0}}}}}}"#
        )
    }

    fn sidechain_record(
        message_id: &str,
        request_id: &str,
        input_tokens: u64,
        minute: u64,
    ) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-06-13T00:{minute:02}:00Z","sessionId":"session-sidechain","cwd":"/workspace/example","requestId":"{request_id}","isSidechain":true,"message":{{"id":"{message_id}","model":"claude-sonnet-4-20250514","usage":{{"input_tokens":{input_tokens},"output_tokens":0}}}}}}"#
        )
    }

    fn user_record() -> String {
        r#"{"type":"user","timestamp":"2026-06-13T00:00:00Z","sessionId":"session-main","message":{"role":"user","content":"sanitized"}}"#.to_owned()
    }

    fn synthetic_record(input_tokens: u64, minute: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-06-13T00:{minute:02}:00Z","sessionId":"session-main","requestId":"req-synthetic","message":{{"id":"msg-synthetic","model":"<synthetic>","usage":{{"input_tokens":{input_tokens},"output_tokens":0}}}}}}"#
        )
    }

    fn api_error_record(input_tokens: u64, minute: u64) -> String {
        format!(
            r#"{{"timestamp":"2026-06-13T00:{minute:02}:00Z","sessionId":"session-main","isApiErrorMessage":true,"message":{{"content":[{{"text":"sanitized error"}}],"usage":{{"input_tokens":{input_tokens},"output_tokens":0}}}}}}"#
        )
    }

    fn test_record(message_id: &str, request_id: &str, input_tokens: u64) -> UsageRecord {
        use eltk_core::{
            CostInfo, DedupIdentity, MeteredUsage, RecordQuality, SourceProvenance, Timestamp,
            TokenUsage, UsageActor, UsageContext, UsageRecordParts,
        };

        UsageRecord::from_parts(UsageRecordParts {
            dedup: DedupIdentity::claude_message_request(message_id, request_id),
            observed_at: Timestamp::from("2026-06-13T00:00:00Z"),
            source: SourceProvenance::new(
                SourceKind::ClaudeCodeJsonl,
                "fake-source",
                "fake-parser",
                Timestamp::from("2026-06-13T00:00:01Z"),
            ),
            actor: UsageActor::claude_code("claude-test"),
            context: UsageContext {
                message_id: Some(message_id.to_owned()),
                request_id: Some(request_id.to_owned()),
                ..UsageContext::default()
            },
            usage: MeteredUsage {
                tokens: TokenUsage {
                    input_tokens,
                    ..TokenUsage::default()
                },
                ..MeteredUsage::default()
            },
            cost: CostInfo::unknown(),
            quality: RecordQuality::default(),
        })
    }
}
