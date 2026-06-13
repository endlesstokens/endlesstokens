// SPDX-License-Identifier: MIT

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RecordQuality {
    pub identity_confidence: IdentityConfidence,
    pub timestamp_confidence: TimestampConfidence,
    pub cost_confidence: CostConfidence,
    pub warnings: Vec<RecordWarning>,
}

impl Default for RecordQuality {
    fn default() -> Self {
        Self {
            identity_confidence: IdentityConfidence::Strong,
            timestamp_confidence: TimestampConfidence::Source,
            cost_confidence: CostConfidence::Unknown,
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IdentityConfidence {
    Strong,
    Weak,
    Fallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TimestampConfidence {
    Source,
    Inferred,
    FileMetadata,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CostConfidence {
    Reported,
    Calculated,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RecordWarning {
    MissingMessageId,
    MissingRequestId,
    SourceLocalIdentity,
    FallbackTimestamp,
    InferredModel,
    CalculatedCost,
    SourceTotalMismatch { reported: u64, computed: u64 },
    Other(String),
}
