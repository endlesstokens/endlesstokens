// SPDX-License-Identifier: MIT

use std::{error::Error, fmt, path::PathBuf};

use crate::{
    identity::AgentId,
    record::UsageRecord,
    source::{SourceKind, SourceProvenance},
    usage::MeteredUsage,
};

pub type AdapterResult<T> = Result<T, AdapterError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterError {
    message: String,
}

impl AdapterError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AdapterError {}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ScanConfig {
    pub roots: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct UsageSource {
    pub kind: SourceKind,
    pub path: PathBuf,
    pub root_kind: Option<String>,
}

impl UsageSource {
    pub fn new(kind: SourceKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
            root_kind: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ScanSourceStats {
    pub records_seen: u64,
    pub records_emitted: u64,
    pub warnings: u64,
    pub excluded_usage: ScanExcludedUsageStats,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ScanExcludedUsageStats {
    pub synthetic_records: u64,
    pub synthetic_usage: MeteredUsage,
    pub api_error_records: u64,
    pub api_error_usage: MeteredUsage,
}

impl ScanExcludedUsageStats {
    pub fn records(&self) -> u64 {
        self.synthetic_records
            .saturating_add(self.api_error_records)
    }

    pub fn usage(&self) -> MeteredUsage {
        let mut usage = self.synthetic_usage.clone();
        usage.saturating_add_assign(&self.api_error_usage);
        usage
    }

    pub fn add_synthetic(&mut self, usage: &MeteredUsage) {
        self.synthetic_records = self.synthetic_records.saturating_add(1);
        self.synthetic_usage.saturating_add_assign(usage);
    }

    pub fn add_api_error(&mut self, usage: &MeteredUsage) {
        self.api_error_records = self.api_error_records.saturating_add(1);
        self.api_error_usage.saturating_add_assign(usage);
    }

    pub fn saturating_add_assign(&mut self, other: &Self) {
        self.synthetic_records = self
            .synthetic_records
            .saturating_add(other.synthetic_records);
        self.synthetic_usage
            .saturating_add_assign(&other.synthetic_usage);
        self.api_error_records = self
            .api_error_records
            .saturating_add(other.api_error_records);
        self.api_error_usage
            .saturating_add_assign(&other.api_error_usage);
    }
}

pub trait UsageRecordSink {
    fn push(&mut self, record: UsageRecord);
}

impl UsageRecordSink for Vec<UsageRecord> {
    fn push(&mut self, record: UsageRecord) {
        Vec::push(self, record);
    }
}

pub trait UsageAdapter {
    fn id(&self) -> AgentId;

    fn discover(&self, config: &ScanConfig) -> AdapterResult<Vec<UsageSource>>;

    fn scan_source(
        &self,
        source: &UsageSource,
        sink: &mut dyn UsageRecordSink,
    ) -> AdapterResult<ScanSourceStats>;

    fn source_provenance(&self, _source: &UsageSource) -> Option<SourceProvenance> {
        None
    }
}
