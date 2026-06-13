// SPDX-License-Identifier: MIT

//! Core EndlessTokens usage model.
//!
//! This crate owns the normalized records that adapters, stores, reports, and
//! remote ingest build on. It intentionally avoids parser and storage
//! dependencies so every boundary crate can share the same event shape.

pub mod adapter;
pub mod context;
pub mod cost;
pub mod identity;
pub mod quality;
pub mod record;
pub mod source;
pub mod time;
pub mod usage;

pub use adapter::{
    AdapterError, AdapterResult, ScanConfig, ScanExcludedUsageStats, ScanSourceStats, UsageAdapter,
    UsageRecordSink, UsageSource,
};
pub use context::{ModelVariant, UsageActor, UsageContext};
pub use cost::{CostInfo, CostSource, UsdNanos};
pub use identity::{AgentId, DedupIdentity, DedupScope, ProviderId, StableUsageKey, UsageRecordId};
pub use quality::{
    CostConfidence, IdentityConfidence, RecordQuality, RecordWarning, TimestampConfidence,
};
pub use record::{UsageRecord, UsageRecordParts};
pub use source::{SourceKind, SourceProvenance};
pub use time::{CalendarDate, Timestamp};
pub use usage::{MeteredUsage, ServerToolUsage, TokenUsage};

pub const PRODUCT_NAME: &str = "EndlessTokens";
pub const CLI_NAME: &str = "eltk";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_product_identifiers() {
        assert_eq!(PRODUCT_NAME, "EndlessTokens");
        assert_eq!(CLI_NAME, "eltk");
    }
}
