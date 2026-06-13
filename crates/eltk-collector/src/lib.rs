// SPDX-License-Identifier: MIT

pub mod dedup;
pub mod scan;

pub use dedup::merge_claude_records;
pub use scan::{CollectScanStats, CollectSourceError, CollectorScanResult, collect_claude_records};

pub const CRATE_NAME: &str = "eltk-collector";
