// SPDX-License-Identifier: MIT

use std::{env, path::PathBuf, process};

use eltk_collector::{CollectorScanResult, collect_claude_records};
use eltk_core::{MeteredUsage, ScanConfig};
use serde_json::json;

fn main() {
    match run(env::args().skip(1)) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{}", error.message);
            process::exit(error.exit_code);
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<String, CliError> {
    match parse_command(args)? {
        Command::Help => Ok(help_text(env!("CARGO_PKG_VERSION"))),
        Command::Version => Ok(version_text(env!("CARGO_PKG_VERSION"))),
        Command::ScanHelp => Ok(scan_help_text()),
        Command::Scan(options) => run_scan(options),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Help,
    Version,
    ScanHelp,
    Scan(ScanOptions),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ScanOptions {
    roots: Vec<PathBuf>,
    include_excluded: bool,
    output_format: ScanOutputFormat,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ScanOutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliError {
    message: String,
    exit_code: i32,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }
}

fn parse_command(args: impl IntoIterator<Item = String>) -> Result<Command, CliError> {
    let mut args = args.into_iter();

    let Some(arg) = args.next() else {
        return Ok(Command::Help);
    };

    match arg.as_str() {
        "--help" | "-h" => Ok(Command::Help),
        "--version" | "-V" | "version" => Ok(Command::Version),
        "scan" => parse_scan_args(args),
        _ => Err(CliError::usage(format!(
            "unknown argument: {arg}\ntry 'eltk --help'"
        ))),
    }
}

fn parse_scan_args(args: impl IntoIterator<Item = String>) -> Result<Command, CliError> {
    let mut roots = Vec::new();
    let mut include_excluded = false;
    let mut output_format = ScanOutputFormat::Text;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Command::ScanHelp),
            "--include-excluded" => include_excluded = true,
            "--json" => output_format = ScanOutputFormat::Json,
            "--root" | "-r" => {
                let Some(root) = args.next() else {
                    return Err(CliError::usage(
                        "missing path after --root\ntry 'eltk scan --help'",
                    ));
                };
                roots.push(PathBuf::from(root));
            }
            _ => {
                if let Some(root) = arg.strip_prefix("--root=") {
                    roots.push(PathBuf::from(root));
                } else {
                    return Err(CliError::usage(format!(
                        "unknown scan argument: {arg}\ntry 'eltk scan --help'"
                    )));
                }
            }
        }
    }

    Ok(Command::Scan(ScanOptions {
        roots,
        include_excluded,
        output_format,
    }))
}

fn run_scan(options: ScanOptions) -> Result<String, CliError> {
    collect_claude_records(&ScanConfig {
        roots: options.roots,
    })
    .map(|result| match options.output_format {
        ScanOutputFormat::Text => format_scan_report(&result, options.include_excluded),
        ScanOutputFormat::Json => format_scan_json_report(&result, options.include_excluded),
    })
    .map_err(|error| CliError::runtime(format!("scan failed: {error}")))
}

fn version_text(version: &str) -> String {
    format!("{} {version}", eltk_core::CLI_NAME)
}

fn help_text(version: &str) -> String {
    format!(
        "{program} {version}\n\n{product} token usage tracker.\n\nUsage: {program} [OPTIONS] [COMMAND]\n\nCommands:\n  scan          Scan Claude Code usage records\n  version       Print version\n\nOptions:\n  -V, --version Print version\n  -h, --help    Print help",
        program = eltk_core::CLI_NAME,
        product = eltk_core::PRODUCT_NAME
    )
}

fn scan_help_text() -> String {
    format!(
        "{program} scan\n\nScan Claude Code usage records.\n\nUsage: {program} scan [OPTIONS]\n\nOptions:\n  -r, --root <PATH>    Scan a Claude config root or projects directory\n      --include-excluded Include synthetic/API-error rows in displayed totals\n      --json           Print machine-readable JSON\n  -h, --help           Print help",
        program = eltk_core::CLI_NAME
    )
}

fn format_scan_report(result: &CollectorScanResult, include_excluded: bool) -> String {
    let summary = ScanReportSummary::from_result(result, include_excluded);
    let mut output = format!(
        "Claude Code scan\n\
         sources discovered: {}\n\
         sources scanned: {}\n\
         records seen: {}\n\
         records emitted: {}\n\
         records after merge: {}\n\
         warnings: {}\n\
         total token scope: {}\n\
         total tokens: {}\n\
         bucket tokens: {}\n\
         input tokens: {}\n\
         output tokens: {}\n\
         cache read input tokens: {}\n\
         cache creation input tokens: {}\n\
         reasoning output tokens: {}\n\
         extra reported tokens: {}\n\
         server tool requests: {}\n\
         excluded records: {}\n\
         excluded total tokens: {}\n\
         excluded bucket tokens: {}\n\
         synthetic records: {}\n\
         synthetic total tokens: {}\n\
         api error records: {}\n\
         api error total tokens: {}\n\
         observed total tokens: {}\n\
         observed bucket tokens: {}",
        result.stats.sources_discovered,
        result.stats.sources_scanned,
        result.stats.records_seen,
        result.stats.records_emitted,
        result.stats.records_after_merge,
        result.stats.warnings,
        summary.total_scope,
        summary.displayed.total_tokens,
        summary.displayed.bucket_tokens,
        summary.displayed.input_tokens,
        summary.displayed.output_tokens,
        summary.displayed.cache_read_input_tokens,
        summary.displayed.cache_creation_input_tokens,
        summary.displayed.reasoning_output_tokens,
        summary.displayed.extra_total_tokens,
        summary.displayed.server_tool_requests,
        result.stats.excluded_usage.records(),
        summary.excluded.total_tokens,
        summary.excluded.bucket_tokens,
        result.stats.excluded_usage.synthetic_records,
        summary.synthetic.total_tokens,
        result.stats.excluded_usage.api_error_records,
        summary.api_error.total_tokens,
        summary.observed.total_tokens,
        summary.observed.bucket_tokens
    );

    if !result.source_errors.is_empty() {
        output.push_str("\nsource errors:");
        for error in &result.source_errors {
            output.push_str(&format!(
                "\n  {}: {}",
                error.source.path.display(),
                error.message
            ));
        }
    }

    output
}

fn format_scan_json_report(result: &CollectorScanResult, include_excluded: bool) -> String {
    let summary = ScanReportSummary::from_result(result, include_excluded);
    let source_errors = result
        .source_errors
        .iter()
        .map(|error| {
            json!({
                "source": error.source.path.to_string_lossy(),
                "message": error.message,
            })
        })
        .collect::<Vec<_>>();
    let report = json!({
        "format": "eltk_scan_v1",
        "agent": "claude_code",
        "include_excluded": include_excluded,
        "total_scope": summary.total_scope,
        "stats": {
            "sources_discovered": result.stats.sources_discovered,
            "sources_scanned": result.stats.sources_scanned,
            "records_seen": result.stats.records_seen,
            "records_emitted": result.stats.records_emitted,
            "records_after_merge": result.stats.records_after_merge,
            "warnings": result.stats.warnings,
            "excluded_records": result.stats.excluded_usage.records(),
            "synthetic_records": result.stats.excluded_usage.synthetic_records,
            "api_error_records": result.stats.excluded_usage.api_error_records,
        },
        "totals": {
            "displayed": scan_totals_json(summary.displayed),
            "included": scan_totals_json(summary.included),
            "excluded": scan_totals_json(summary.excluded),
            "synthetic": scan_totals_json(summary.synthetic),
            "api_error": scan_totals_json(summary.api_error),
            "observed": scan_totals_json(summary.observed),
        },
        "source_errors": source_errors,
    });

    serde_json::to_string_pretty(&report).expect("scan report json is serializable")
}

fn scan_totals_json(totals: ScanTotals) -> serde_json::Value {
    json!({
        "total_tokens": totals.total_tokens,
        "bucket_tokens": totals.bucket_tokens,
        "input_tokens": totals.input_tokens,
        "output_tokens": totals.output_tokens,
        "cache_read_input_tokens": totals.cache_read_input_tokens,
        "cache_creation_input_tokens": totals.cache_creation_input_tokens,
        "reasoning_output_tokens": totals.reasoning_output_tokens,
        "extra_reported_tokens": totals.extra_total_tokens,
        "server_tool_requests": totals.server_tool_requests,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScanReportSummary {
    included: ScanTotals,
    excluded: ScanTotals,
    synthetic: ScanTotals,
    api_error: ScanTotals,
    observed: ScanTotals,
    displayed: ScanTotals,
    total_scope: &'static str,
}

impl ScanReportSummary {
    fn from_result(result: &CollectorScanResult, include_excluded: bool) -> Self {
        let included = ScanTotals::from_result(result);
        let synthetic =
            ScanTotals::from_metered_usage(&result.stats.excluded_usage.synthetic_usage);
        let api_error =
            ScanTotals::from_metered_usage(&result.stats.excluded_usage.api_error_usage);
        let excluded = synthetic.saturating_add(&api_error);
        let observed = included.saturating_add(&excluded);
        let displayed = if include_excluded { observed } else { included };
        let total_scope = if include_excluded {
            "included + excluded rows"
        } else {
            "included records"
        };

        Self {
            included,
            excluded,
            synthetic,
            api_error,
            observed,
            displayed,
            total_scope,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScanTotals {
    total_tokens: u64,
    bucket_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_input_tokens: u64,
    cache_creation_input_tokens: u64,
    reasoning_output_tokens: u64,
    extra_total_tokens: u64,
    server_tool_requests: u64,
}

impl ScanTotals {
    fn from_result(result: &CollectorScanResult) -> Self {
        let mut totals = Self::default();

        for record in &result.records {
            totals.saturating_add_usage(&record.usage);
        }

        totals
    }

    fn from_metered_usage(usage: &MeteredUsage) -> Self {
        let mut totals = Self::default();
        totals.saturating_add_usage(usage);
        totals
    }

    fn saturating_add(&self, other: &Self) -> Self {
        Self {
            total_tokens: self.total_tokens.saturating_add(other.total_tokens),
            bucket_tokens: self.bucket_tokens.saturating_add(other.bucket_tokens),
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            output_tokens: self.output_tokens.saturating_add(other.output_tokens),
            cache_read_input_tokens: self
                .cache_read_input_tokens
                .saturating_add(other.cache_read_input_tokens),
            cache_creation_input_tokens: self
                .cache_creation_input_tokens
                .saturating_add(other.cache_creation_input_tokens),
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .saturating_add(other.reasoning_output_tokens),
            extra_total_tokens: self
                .extra_total_tokens
                .saturating_add(other.extra_total_tokens),
            server_tool_requests: self
                .server_tool_requests
                .saturating_add(other.server_tool_requests),
        }
    }

    fn saturating_add_usage(&mut self, usage: &MeteredUsage) {
        self.total_tokens = self.total_tokens.saturating_add(usage.known_total_tokens());
        self.bucket_tokens = self
            .bucket_tokens
            .saturating_add(usage.bucket_total_tokens());
        self.input_tokens = self.input_tokens.saturating_add(usage.tokens.input_tokens);
        self.output_tokens = self
            .output_tokens
            .saturating_add(usage.tokens.output_tokens);
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(usage.tokens.cache_read_input_tokens);
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(usage.tokens.cache_creation_input_tokens);
        self.reasoning_output_tokens = self
            .reasoning_output_tokens
            .saturating_add(usage.tokens.reasoning_output_tokens);
        self.extra_total_tokens = self
            .extra_total_tokens
            .saturating_add(usage.extra_total_tokens);
        self.server_tool_requests = self
            .server_tool_requests
            .saturating_add(usage.server_tools.total_requests());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eltk_collector::{CollectScanStats, CollectSourceError};
    use eltk_core::{
        CostInfo, DedupIdentity, MeteredUsage, RecordQuality, ScanExcludedUsageStats,
        ServerToolUsage, SourceKind, SourceProvenance, Timestamp, TokenUsage, UsageActor,
        UsageContext, UsageRecord, UsageRecordParts, UsageSource,
    };
    use std::collections::BTreeMap;

    #[test]
    fn formats_version_text() {
        assert_eq!(version_text("0.0.0"), "eltk 0.0.0");
    }

    #[test]
    fn help_lists_supported_flags() {
        let help = help_text("0.0.0");
        assert!(help.starts_with("eltk 0.0.0"));
        assert!(help.contains("scan"));
        assert!(help.contains("version"));
        assert!(help.contains("-V, --version"));
        assert!(help.contains("-h, --help"));
    }

    #[test]
    fn scan_help_lists_root_option() {
        let help = scan_help_text();

        assert!(help.contains("eltk scan"));
        assert!(help.contains("-r, --root <PATH>"));
        assert!(help.contains("--include-excluded"));
        assert!(help.contains("--json"));
    }

    #[test]
    fn parses_scan_roots() {
        let command = parse_command([
            "scan".to_owned(),
            "--root".to_owned(),
            "/tmp/claude-a".to_owned(),
            "-r".to_owned(),
            "/tmp/claude-b".to_owned(),
            "--root=/tmp/claude-c".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Scan(ScanOptions {
                roots: vec![
                    PathBuf::from("/tmp/claude-a"),
                    PathBuf::from("/tmp/claude-b"),
                    PathBuf::from("/tmp/claude-c"),
                ],
                include_excluded: false,
                output_format: ScanOutputFormat::Text,
            })
        );
    }

    #[test]
    fn parses_scan_include_excluded() {
        let command = parse_command(["scan".to_owned(), "--include-excluded".to_owned()]).unwrap();

        assert_eq!(
            command,
            Command::Scan(ScanOptions {
                roots: Vec::new(),
                include_excluded: true,
                output_format: ScanOutputFormat::Text,
            })
        );
    }

    #[test]
    fn parses_scan_json_output() {
        let command = parse_command(["scan".to_owned(), "--json".to_owned()]).unwrap();

        assert_eq!(
            command,
            Command::Scan(ScanOptions {
                roots: Vec::new(),
                include_excluded: false,
                output_format: ScanOutputFormat::Json,
            })
        );
    }

    #[test]
    fn rejects_scan_root_without_path() {
        let error = parse_command(["scan".to_owned(), "--root".to_owned()]).unwrap_err();

        assert_eq!(error.exit_code, 2);
        assert!(error.message.contains("missing path after --root"));
    }

    #[test]
    fn formats_scan_report_with_totals_and_source_errors() {
        let result = CollectorScanResult {
            sources: vec![UsageSource::new(SourceKind::ClaudeCodeJsonl, "ok.jsonl")],
            records: vec![test_record()],
            source_errors: vec![CollectSourceError {
                source: UsageSource::new(SourceKind::ClaudeCodeJsonl, "bad.jsonl"),
                message: "permission denied".to_owned(),
            }],
            stats: CollectScanStats {
                sources_discovered: 2,
                sources_scanned: 2,
                records_seen: 4,
                records_emitted: 1,
                records_after_merge: 1,
                warnings: 1,
                excluded_usage: ScanExcludedUsageStats {
                    synthetic_records: 1,
                    synthetic_usage: MeteredUsage {
                        tokens: TokenUsage {
                            input_tokens: 2,
                            output_tokens: 3,
                            ..TokenUsage::default()
                        },
                        ..MeteredUsage::default()
                    },
                    api_error_records: 1,
                    api_error_usage: MeteredUsage {
                        tokens: TokenUsage {
                            input_tokens: 4,
                            output_tokens: 5,
                            ..TokenUsage::default()
                        },
                        ..MeteredUsage::default()
                    },
                },
            },
        };

        let report = format_scan_report(&result, false);

        assert!(report.contains("sources discovered: 2"));
        assert!(report.contains("records after merge: 1"));
        assert!(report.contains("warnings: 1"));
        assert!(report.contains("total token scope: included records"));
        assert!(report.contains("total tokens: 25"));
        assert!(report.contains("bucket tokens: 22"));
        assert!(report.contains("input tokens: 10"));
        assert!(report.contains("output tokens: 5"));
        assert!(report.contains("cache read input tokens: 3"));
        assert!(report.contains("cache creation input tokens: 4"));
        assert!(report.contains("reasoning output tokens: 2"));
        assert!(report.contains("extra reported tokens: 1"));
        assert!(report.contains("server tool requests: 3"));
        assert!(report.contains("excluded records: 2"));
        assert!(report.contains("excluded total tokens: 14"));
        assert!(report.contains("excluded bucket tokens: 14"));
        assert!(report.contains("synthetic records: 1"));
        assert!(report.contains("synthetic total tokens: 5"));
        assert!(report.contains("api error records: 1"));
        assert!(report.contains("api error total tokens: 9"));
        assert!(report.contains("observed total tokens: 39"));
        assert!(report.contains("observed bucket tokens: 36"));
        assert!(report.contains("source errors:\n  bad.jsonl: permission denied"));
    }

    #[test]
    fn format_scan_report_can_include_excluded_usage_in_displayed_totals() {
        let result = CollectorScanResult {
            records: vec![test_record()],
            stats: CollectScanStats {
                excluded_usage: ScanExcludedUsageStats {
                    api_error_records: 1,
                    api_error_usage: MeteredUsage {
                        tokens: TokenUsage {
                            input_tokens: 4,
                            output_tokens: 5,
                            ..TokenUsage::default()
                        },
                        ..MeteredUsage::default()
                    },
                    ..ScanExcludedUsageStats::default()
                },
                ..CollectScanStats::default()
            },
            ..CollectorScanResult::default()
        };

        let report = format_scan_report(&result, true);

        assert!(report.contains("total token scope: included + excluded rows"));
        assert!(report.contains("total tokens: 34"));
        assert!(report.contains("bucket tokens: 31"));
        assert!(report.contains("input tokens: 14"));
        assert!(report.contains("output tokens: 10"));
    }

    #[test]
    fn formats_scan_json_report() {
        let result = CollectorScanResult {
            sources: vec![UsageSource::new(SourceKind::ClaudeCodeJsonl, "ok.jsonl")],
            records: vec![test_record()],
            source_errors: vec![CollectSourceError {
                source: UsageSource::new(SourceKind::ClaudeCodeJsonl, "bad.jsonl"),
                message: "permission denied".to_owned(),
            }],
            stats: CollectScanStats {
                sources_discovered: 2,
                sources_scanned: 2,
                records_seen: 4,
                records_emitted: 1,
                records_after_merge: 1,
                warnings: 1,
                excluded_usage: ScanExcludedUsageStats {
                    synthetic_records: 1,
                    synthetic_usage: MeteredUsage {
                        tokens: TokenUsage {
                            input_tokens: 2,
                            output_tokens: 3,
                            ..TokenUsage::default()
                        },
                        ..MeteredUsage::default()
                    },
                    api_error_records: 1,
                    api_error_usage: MeteredUsage {
                        tokens: TokenUsage {
                            input_tokens: 4,
                            output_tokens: 5,
                            ..TokenUsage::default()
                        },
                        ..MeteredUsage::default()
                    },
                },
            },
        };

        let report = format_scan_json_report(&result, false);
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();

        assert_eq!(value["format"], "eltk_scan_v1");
        assert_eq!(value["agent"], "claude_code");
        assert_eq!(value["include_excluded"], false);
        assert_eq!(value["total_scope"], "included records");
        assert_eq!(value["stats"]["records_after_merge"], 1);
        assert_eq!(value["stats"]["excluded_records"], 2);
        assert_eq!(value["totals"]["displayed"]["total_tokens"], 25);
        assert_eq!(value["totals"]["displayed"]["bucket_tokens"], 22);
        assert_eq!(value["totals"]["observed"]["total_tokens"], 39);
        assert_eq!(value["totals"]["observed"]["bucket_tokens"], 36);
        assert_eq!(value["totals"]["synthetic"]["total_tokens"], 5);
        assert_eq!(value["totals"]["api_error"]["total_tokens"], 9);
        assert_eq!(value["source_errors"][0]["source"], "bad.jsonl");
        assert_eq!(value["source_errors"][0]["message"], "permission denied");
    }

    #[test]
    fn scan_json_matches_default_golden_fixture() {
        let output = run_scan_json_fixture(false);
        let expected = read_json_fixture("scan-json/expected-default.json");

        assert_eq!(output, expected);
    }

    #[test]
    fn scan_json_matches_include_excluded_golden_fixture() {
        let output = run_scan_json_fixture(true);
        let expected = read_json_fixture("scan-json/expected-include-excluded.json");

        assert_eq!(output, expected);
    }

    #[cfg(unix)]
    #[test]
    fn formats_scan_json_report_with_non_utf8_source_error_path() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let path = PathBuf::from(OsString::from_vec(b"bad-\xFF.jsonl".to_vec()));
        let result = CollectorScanResult {
            source_errors: vec![CollectSourceError {
                source: UsageSource::new(SourceKind::ClaudeCodeJsonl, path),
                message: "permission denied".to_owned(),
            }],
            ..CollectorScanResult::default()
        };

        let report = format_scan_json_report(&result, false);
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();

        assert_eq!(value["source_errors"][0]["source"], "bad-\u{FFFD}.jsonl");
        assert_eq!(value["source_errors"][0]["message"], "permission denied");
    }

    #[test]
    fn scan_totals_saturate_on_overflow() {
        let result = CollectorScanResult {
            records: vec![
                test_record_with_usage(TokenUsage {
                    input_tokens: u64::MAX,
                    output_tokens: 1,
                    ..TokenUsage::default()
                }),
                test_record_with_usage(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    ..TokenUsage::default()
                }),
            ],
            ..CollectorScanResult::default()
        };

        let totals = ScanTotals::from_result(&result);

        assert_eq!(totals.total_tokens, u64::MAX);
        assert_eq!(totals.input_tokens, u64::MAX);
        assert_eq!(totals.output_tokens, 2);
    }

    fn run_scan_json_fixture(include_excluded: bool) -> serde_json::Value {
        let root = fixture_path("scan-json/claude");
        let mut args = vec![
            "scan".to_owned(),
            "--root".to_owned(),
            root.to_string_lossy().into_owned(),
            "--json".to_owned(),
        ];
        if include_excluded {
            args.push("--include-excluded".to_owned());
        }

        let output = run(args).unwrap();
        serde_json::from_str(&output).unwrap()
    }

    fn read_json_fixture(relative_path: &str) -> serde_json::Value {
        let path = fixture_path(relative_path);
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    fn fixture_path(relative_path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(relative_path)
    }

    fn test_record() -> UsageRecord {
        test_record_with_usage(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_input_tokens: 3,
            cache_creation_input_tokens: 4,
            reasoning_output_tokens: 2,
            ..TokenUsage::default()
        })
    }

    fn test_record_with_usage(tokens: TokenUsage) -> UsageRecord {
        let mut other_tools = BTreeMap::new();
        other_tools.insert("mcp_tool_requests".to_owned(), 2);

        UsageRecord::from_parts(UsageRecordParts {
            dedup: DedupIdentity::claude_message_request("msg-test", "req-test"),
            observed_at: Timestamp::from("2026-06-13T00:00:00Z"),
            source: SourceProvenance::new(
                SourceKind::ClaudeCodeJsonl,
                "source-path-hash",
                "claude-jsonl-v1",
                Timestamp::from("2026-06-13T00:00:01Z"),
            ),
            actor: UsageActor::claude_code("claude-sonnet-4-20250514"),
            context: UsageContext {
                message_id: Some("msg-test".to_owned()),
                request_id: Some("req-test".to_owned()),
                ..UsageContext::default()
            },
            usage: MeteredUsage {
                tokens,
                server_tools: ServerToolUsage {
                    web_search_requests: 1,
                    other: other_tools,
                },
                extra_total_tokens: 1,
                ..MeteredUsage::default()
            },
            cost: CostInfo::unknown(),
            quality: RecordQuality::default(),
        })
    }
}
