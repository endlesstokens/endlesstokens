// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use eltk_core::{
    AdapterError, AdapterResult, AgentId, CostConfidence, CostInfo, CostSource, DedupIdentity,
    DedupScope, IdentityConfidence, MeteredUsage, ProviderId, RecordQuality, RecordWarning,
    ScanConfig, ScanSourceStats, ServerToolUsage, SourceKind, SourceProvenance, StableUsageKey,
    Timestamp, TimestampConfidence, TokenUsage, UsageActor, UsageAdapter, UsageContext,
    UsageRecord, UsageRecordParts, UsageRecordSink, UsageSource, UsdNanos,
};
use serde_json::Value;

pub const CLAUDE_CODE_PARSER_VERSION: &str = "claude-code-jsonl-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeAdapter {
    parser_version: String,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self {
            parser_version: CLAUDE_CODE_PARSER_VERSION.to_owned(),
        }
    }

    pub fn with_parser_version(parser_version: impl Into<String>) -> Self {
        Self {
            parser_version: parser_version.into(),
        }
    }

    pub fn parse_jsonl_line(
        &self,
        line: &str,
        source: &UsageSource,
        line_number: u64,
        byte_offset: u64,
        byte_len: u64,
        ingested_at: Timestamp,
    ) -> AdapterResult<Option<UsageRecord>> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(line)
            .map_err(|error| AdapterError::new(format!("invalid Claude JSONL line: {error}")))?;
        let Some(object) = value.as_object() else {
            return Ok(None);
        };

        let Some(message) = object.get("message").and_then(Value::as_object) else {
            return Ok(None);
        };
        if !is_assistant_usage_row(object, message) {
            return Ok(None);
        }

        if object
            .get("isApiErrorMessage")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(None);
        }
        let model = message.get("model").and_then(Value::as_str);
        if model == Some("<synthetic>") {
            return Ok(None);
        }

        let Some(usage) = message.get("usage").and_then(Value::as_object) else {
            return Ok(None);
        };

        let message_id = message.get("id").and_then(Value::as_str).map(str::to_owned);
        let request_id = object
            .get("requestId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let source_path_hash = stable_hash(source.path.to_string_lossy().as_bytes());
        let line_hash = stable_hash(line.as_bytes());

        let mut warnings = Vec::new();
        let (dedup, identity_confidence) = match (&message_id, &request_id) {
            (Some(message_id), Some(request_id)) => (
                DedupIdentity::claude_message_request(message_id, request_id),
                IdentityConfidence::Strong,
            ),
            (Some(message_id), None) => {
                warnings.push(RecordWarning::MissingRequestId);
                (
                    DedupIdentity::new(
                        DedupScope::Global,
                        AgentId::claude_code(),
                        Some(ProviderId::anthropic()),
                        StableUsageKey::ClaudeMessageOnly {
                            message_id: message_id.clone(),
                        },
                    ),
                    IdentityConfidence::Weak,
                )
            }
            (None, Some(_)) => {
                warnings.push(RecordWarning::MissingMessageId);
                warnings.push(RecordWarning::SourceLocalIdentity);
                (
                    fallback_identity(&source_path_hash, byte_offset, line_number, &line_hash),
                    IdentityConfidence::Fallback,
                )
            }
            (None, None) => {
                warnings.push(RecordWarning::MissingMessageId);
                warnings.push(RecordWarning::MissingRequestId);
                warnings.push(RecordWarning::SourceLocalIdentity);
                (
                    fallback_identity(&source_path_hash, byte_offset, line_number, &line_hash),
                    IdentityConfidence::Fallback,
                )
            }
        };

        let (observed_at, timestamp_confidence) =
            if let Some(timestamp) = object.get("timestamp").and_then(Value::as_str) {
                (Timestamp::new(timestamp), TimestampConfidence::Source)
            } else {
                warnings.push(RecordWarning::FallbackTimestamp);
                (ingested_at.clone(), TimestampConfidence::Inferred)
            };

        let usage = metered_usage_from_value(usage, &mut warnings);
        let cost = cost_from_value(&value);
        let cost_confidence = match cost.source {
            CostSource::Reported => CostConfidence::Reported,
            CostSource::Calculated => CostConfidence::Calculated,
            CostSource::Unknown => CostConfidence::Unknown,
        };

        let mut source_provenance = SourceProvenance::new(
            SourceKind::ClaudeCodeJsonl,
            source_path_hash,
            self.parser_version.clone(),
            ingested_at,
        );
        source_provenance.source_path = Some(source.path.clone());
        source_provenance.root_kind.clone_from(&source.root_kind);
        source_provenance.byte_offset = Some(byte_offset);
        source_provenance.byte_len = Some(byte_len);
        source_provenance.line_number = Some(line_number);
        source_provenance.line_hash = Some(line_hash);

        Ok(Some(UsageRecord::from_parts(UsageRecordParts {
            dedup,
            observed_at,
            source: source_provenance,
            actor: actor_from_model(model),
            context: UsageContext {
                session_id: object
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                parent_session_id: object
                    .get("parentSessionId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                message_id,
                request_id,
                turn_id: None,
                cwd: object.get("cwd").and_then(Value::as_str).map(str::to_owned),
                project: None,
                git_branch: object
                    .get("gitBranch")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                version: object
                    .get("version")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                agent_name: None,
                is_sidechain: object
                    .get("isSidechain")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                is_api_error_message: false,
                is_synthetic: false,
            },
            usage,
            cost,
            quality: RecordQuality {
                identity_confidence,
                timestamp_confidence,
                cost_confidence,
                warnings,
            },
        })))
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageAdapter for ClaudeCodeAdapter {
    fn id(&self) -> AgentId {
        AgentId::claude_code()
    }

    fn discover(&self, config: &ScanConfig) -> AdapterResult<Vec<UsageSource>> {
        let mut sources = Vec::new();
        for root in claude_roots(config) {
            discover_jsonl_sources(&root, &mut sources)?;
        }
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(sources)
    }

    fn scan_source(
        &self,
        source: &UsageSource,
        sink: &mut dyn UsageRecordSink,
    ) -> AdapterResult<ScanSourceStats> {
        let file = fs::File::open(&source.path)
            .map_err(|error| AdapterError::new(format!("failed to open Claude source: {error}")))?;
        let mut reader = BufReader::new(file);
        let mut stats = ScanSourceStats::default();
        let mut byte_offset = 0;
        let ingested_at = current_ingested_at();

        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).map_err(|error| {
                AdapterError::new(format!("failed to read Claude source line: {error}"))
            })?;
            if bytes_read == 0 {
                break;
            }

            stats.records_seen += 1;
            match self.parse_jsonl_line(
                &line,
                source,
                stats.records_seen,
                byte_offset,
                bytes_read as u64,
                ingested_at.clone(),
            ) {
                Ok(Some(record)) => {
                    stats.records_emitted += 1;
                    stats.warnings += record.quality.warnings.len() as u64;
                    sink.push(record);
                }
                Ok(None) => {}
                Err(_) => {
                    stats.warnings += 1;
                }
            }
            byte_offset += bytes_read as u64;
        }

        Ok(stats)
    }
}

fn is_assistant_usage_row(
    object: &serde_json::Map<String, Value>,
    message: &serde_json::Map<String, Value>,
) -> bool {
    match object.get("type").and_then(Value::as_str) {
        Some("assistant") => true,
        Some(_) => false,
        None => message.get("role").and_then(Value::as_str) == Some("assistant"),
    }
}

fn metered_usage_from_value(
    usage: &serde_json::Map<String, Value>,
    warnings: &mut Vec<RecordWarning>,
) -> MeteredUsage {
    let cache_creation = usage.get("cache_creation").and_then(Value::as_object);
    let mut tokens = TokenUsage::with_cache_creation_ttl(
        u64_field(usage, "input_tokens"),
        u64_field(usage, "output_tokens"),
        u64_field(usage, "cache_read_input_tokens"),
        u64_field(usage, "cache_creation_input_tokens"),
        cache_creation
            .map(|value| u64_field(value, "ephemeral_5m_input_tokens"))
            .unwrap_or_default(),
        cache_creation
            .map(|value| u64_field(value, "ephemeral_1h_input_tokens"))
            .unwrap_or_default(),
        u64_field(usage, "reasoning_output_tokens"),
    );
    tokens.reconcile_cache_creation_from_ttl();

    let computed_total_tokens = tokens.total_tokens();
    let reported_total_tokens = usage.get("total_tokens").and_then(Value::as_u64);
    if let Some(reported) = reported_total_tokens
        && reported != computed_total_tokens
    {
        warnings.push(RecordWarning::SourceTotalMismatch {
            reported,
            computed: computed_total_tokens,
        });
    }

    let extra_total_tokens = reported_total_tokens
        .map(|total| total.saturating_sub(computed_total_tokens))
        .unwrap_or_default();
    let server_tool_use = usage.get("server_tool_use").and_then(Value::as_object);

    MeteredUsage {
        tokens,
        server_tools: server_tool_usage_from_value(server_tool_use),
        reported_total_tokens,
        extra_total_tokens,
    }
}

fn server_tool_usage_from_value(
    server_tool_use: Option<&serde_json::Map<String, Value>>,
) -> ServerToolUsage {
    let Some(server_tool_use) = server_tool_use else {
        return ServerToolUsage::default();
    };

    let other = server_tool_use
        .iter()
        .filter_map(|(name, value)| {
            if name == "web_search_requests" {
                return None;
            }
            value.as_u64().map(|count| (name.clone(), count))
        })
        .collect::<BTreeMap<_, _>>();

    ServerToolUsage {
        web_search_requests: u64_field(server_tool_use, "web_search_requests"),
        other,
    }
}

fn cost_from_value(value: &Value) -> CostInfo {
    value
        .get("costUSD")
        .and_then(Value::as_f64)
        .map(|cost| {
            CostInfo::reported(UsdNanos::new(
                (cost * UsdNanos::NANOS_PER_USD as f64).round() as i64,
            ))
        })
        .unwrap_or_else(CostInfo::unknown)
}

fn actor_from_model(model: Option<&str>) -> UsageActor {
    let mut actor = UsageActor::new(AgentId::claude_code(), "Claude Code");
    actor.provider = Some(ProviderId::anthropic());
    actor.model_raw = model.map(str::to_owned);
    actor.model_normalized = model.map(str::to_owned);
    actor
}

fn fallback_identity(
    source_fingerprint: &str,
    byte_offset: u64,
    line_number: u64,
    line_hash: &str,
) -> DedupIdentity {
    DedupIdentity::new(
        DedupScope::SourceFile,
        AgentId::claude_code(),
        Some(ProviderId::anthropic()),
        StableUsageKey::SourceFallback {
            source_fingerprint: source_fingerprint.to_owned(),
            byte_offset: Some(byte_offset),
            line_number: Some(line_number),
            line_hash: line_hash.to_owned(),
        },
    )
}

fn u64_field(values: &serde_json::Map<String, Value>, name: &str) -> u64 {
    values.get(name).and_then(Value::as_u64).unwrap_or_default()
}

fn claude_roots(config: &ScanConfig) -> Vec<PathBuf> {
    if !config.roots.is_empty() {
        return config.roots.clone();
    }

    if let Some(config_dirs) = env::var_os("CLAUDE_CONFIG_DIR") {
        let roots = config_dirs
            .to_string_lossy()
            .split(',')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if !roots.is_empty() {
            return roots;
        }
    }

    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };

    vec![home.join(".claude"), home.join(".config").join("claude")]
}

fn discover_jsonl_sources(root: &Path, sources: &mut Vec<UsageSource>) -> AdapterResult<()> {
    if root.is_file() {
        if is_jsonl(root) {
            sources.push(UsageSource::new(SourceKind::ClaudeCodeJsonl, root));
        }
        return Ok(());
    }

    let scan_root = if root.file_name().and_then(|name| name.to_str()) == Some("projects") {
        root.to_owned()
    } else {
        let projects = root.join("projects");
        if projects.is_dir() {
            projects
        } else {
            root.to_owned()
        }
    };

    walk_jsonl_sources(&scan_root, sources)
}

fn walk_jsonl_sources(root: &Path, sources: &mut Vec<UsageSource>) -> AdapterResult<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(AdapterError::new(format!(
                "failed to read Claude source directory: {error}"
            )));
        }
    };

    for entry in entries {
        let entry = entry.map_err(|error| {
            AdapterError::new(format!("failed to read Claude directory entry: {error}"))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            AdapterError::new(format!("failed to read Claude entry type: {error}"))
        })?;
        let path = entry.path();

        if file_type.is_dir() {
            walk_jsonl_sources(&path, sources)?;
        } else if file_type.is_file() && is_jsonl(&path) {
            sources.push(UsageSource::new(SourceKind::ClaudeCodeJsonl, path));
        }
    }

    Ok(())
}

fn is_jsonl(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
}

fn current_ingested_at() -> Timestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    unix_seconds_to_utc_timestamp(seconds)
}

fn unix_seconds_to_utc_timestamp(seconds: u64) -> Timestamp {
    let seconds = seconds.min(i64::MAX as u64);
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    Timestamp::new(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let shifted_days = days + 719_468;
    let era = if shifted_days >= 0 {
        shifted_days
    } else {
        shifted_days - 146_096
    } / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };

    (year, month as u64, day as u64)
}

fn stable_hash(bytes: &[u8]) -> String {
    const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_seconds_as_utc_iso_timestamp() {
        assert_eq!(
            unix_seconds_to_utc_timestamp(0).as_str(),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            unix_seconds_to_utc_timestamp(951_782_400).as_str(),
            "2000-02-29T00:00:00Z"
        );
    }
}
