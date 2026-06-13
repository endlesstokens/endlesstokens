// SPDX-License-Identifier: MIT

use std::{env, path::PathBuf, process};

use eltk_collector::{CollectorScanResult, collect_claude_records};
use eltk_core::ScanConfig;

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
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Command::ScanHelp),
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

    Ok(Command::Scan(ScanOptions { roots }))
}

fn run_scan(options: ScanOptions) -> Result<String, CliError> {
    collect_claude_records(&ScanConfig {
        roots: options.roots,
    })
    .map(|result| format_scan_report(&result))
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
        "{program} scan\n\nScan Claude Code usage records.\n\nUsage: {program} scan [OPTIONS]\n\nOptions:\n  -r, --root <PATH> Scan a Claude config root or projects directory\n  -h, --help        Print help",
        program = eltk_core::CLI_NAME
    )
}

fn format_scan_report(result: &CollectorScanResult) -> String {
    let totals = ScanTotals::from_result(result);
    let mut output = format!(
        "Claude Code scan\n\
         sources discovered: {}\n\
         sources scanned: {}\n\
         records seen: {}\n\
         records emitted: {}\n\
         records after merge: {}\n\
         warnings: {}\n\
         total tokens: {}\n\
         input tokens: {}\n\
         output tokens: {}\n\
         cache read input tokens: {}\n\
         cache creation input tokens: {}\n\
         reasoning output tokens: {}\n\
         extra reported tokens: {}\n\
         server tool requests: {}",
        result.stats.sources_discovered,
        result.stats.sources_scanned,
        result.stats.records_seen,
        result.stats.records_emitted,
        result.stats.records_after_merge,
        result.stats.warnings,
        totals.total_tokens,
        totals.input_tokens,
        totals.output_tokens,
        totals.cache_read_input_tokens,
        totals.cache_creation_input_tokens,
        totals.reasoning_output_tokens,
        totals.extra_total_tokens,
        totals.server_tool_requests
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScanTotals {
    total_tokens: u64,
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
            totals.total_tokens = totals
                .total_tokens
                .saturating_add(record.usage.known_total_tokens());
            totals.input_tokens = totals
                .input_tokens
                .saturating_add(record.usage.tokens.input_tokens);
            totals.output_tokens = totals
                .output_tokens
                .saturating_add(record.usage.tokens.output_tokens);
            totals.cache_read_input_tokens = totals
                .cache_read_input_tokens
                .saturating_add(record.usage.tokens.cache_read_input_tokens);
            totals.cache_creation_input_tokens = totals
                .cache_creation_input_tokens
                .saturating_add(record.usage.tokens.cache_creation_input_tokens);
            totals.reasoning_output_tokens = totals
                .reasoning_output_tokens
                .saturating_add(record.usage.tokens.reasoning_output_tokens);
            totals.extra_total_tokens = totals
                .extra_total_tokens
                .saturating_add(record.usage.extra_total_tokens);
            totals.server_tool_requests = totals
                .server_tool_requests
                .saturating_add(record.usage.server_tools.total_requests());
        }

        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eltk_collector::{CollectScanStats, CollectSourceError};
    use eltk_core::{
        CostInfo, DedupIdentity, MeteredUsage, RecordQuality, ServerToolUsage, SourceKind,
        SourceProvenance, Timestamp, TokenUsage, UsageActor, UsageContext, UsageRecord,
        UsageRecordParts, UsageSource,
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
            },
        };

        let report = format_scan_report(&result);

        assert!(report.contains("sources discovered: 2"));
        assert!(report.contains("records after merge: 1"));
        assert!(report.contains("warnings: 1"));
        assert!(report.contains("total tokens: 25"));
        assert!(report.contains("input tokens: 10"));
        assert!(report.contains("output tokens: 5"));
        assert!(report.contains("cache read input tokens: 3"));
        assert!(report.contains("cache creation input tokens: 4"));
        assert!(report.contains("reasoning output tokens: 2"));
        assert!(report.contains("extra reported tokens: 1"));
        assert!(report.contains("server tool requests: 3"));
        assert!(report.contains("source errors:\n  bad.jsonl: permission denied"));
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
