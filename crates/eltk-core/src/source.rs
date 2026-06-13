// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use crate::time::Timestamp;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SourceProvenance {
    pub source_kind: SourceKind,
    pub source_path: Option<PathBuf>,
    pub source_path_hash: String,
    pub root_kind: Option<String>,
    pub byte_offset: Option<u64>,
    pub byte_len: Option<u64>,
    pub line_number: Option<u64>,
    pub line_hash: Option<String>,
    pub parser_version: String,
    pub ingested_at: Timestamp,
}

impl SourceProvenance {
    pub fn new(
        source_kind: SourceKind,
        source_path_hash: impl Into<String>,
        parser_version: impl Into<String>,
        ingested_at: Timestamp,
    ) -> Self {
        Self {
            source_kind,
            source_path: None,
            source_path_hash: source_path_hash.into(),
            root_kind: None,
            byte_offset: None,
            byte_len: None,
            line_number: None,
            line_hash: None,
            parser_version: parser_version.into(),
            ingested_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum SourceKind {
    ClaudeCodeJsonl,
    CodexJsonl,
    Other(String),
}
