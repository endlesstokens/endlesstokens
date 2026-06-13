// SPDX-License-Identifier: MIT

use crate::time::CalendarDate;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CostInfo {
    pub amount_nanos_usd: Option<UsdNanos>,
    pub source: CostSource,
    pub pricing_model: Option<String>,
    pub pricing_version: Option<String>,
    pub pricing_effective_date: Option<CalendarDate>,
}

impl CostInfo {
    pub fn unknown() -> Self {
        Self {
            amount_nanos_usd: None,
            source: CostSource::Unknown,
            pricing_model: None,
            pricing_version: None,
            pricing_effective_date: None,
        }
    }

    pub fn reported(amount_nanos_usd: UsdNanos) -> Self {
        Self {
            amount_nanos_usd: Some(amount_nanos_usd),
            source: CostSource::Reported,
            pricing_model: None,
            pricing_version: None,
            pricing_effective_date: None,
        }
    }

    pub fn calculated(
        amount_nanos_usd: UsdNanos,
        pricing_model: impl Into<String>,
        pricing_version: impl Into<String>,
        pricing_effective_date: Option<CalendarDate>,
    ) -> Self {
        Self {
            amount_nanos_usd: Some(amount_nanos_usd),
            source: CostSource::Calculated,
            pricing_model: Some(pricing_model.into()),
            pricing_version: Some(pricing_version.into()),
            pricing_effective_date,
        }
    }
}

impl Default for CostInfo {
    fn default() -> Self {
        Self::unknown()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum CostSource {
    Reported,
    Calculated,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UsdNanos(i64);

impl UsdNanos {
    pub const NANOS_PER_MICRO: i64 = 1_000;
    pub const NANOS_PER_USD: i64 = 1_000_000_000;

    pub fn new(value: i64) -> Self {
        Self(value)
    }

    pub fn from_micros(value: i64) -> Self {
        Self(value * Self::NANOS_PER_MICRO)
    }

    pub fn from_whole_usd(value: i64) -> Self {
        Self(value * Self::NANOS_PER_USD)
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }

    pub fn as_micros(self) -> Option<i64> {
        (self.0 % Self::NANOS_PER_MICRO == 0).then_some(self.0 / Self::NANOS_PER_MICRO)
    }
}

impl From<i64> for UsdNanos {
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_micro_conversion_round_trips_when_exact() {
        let cost = UsdNanos::from_micros(12_345);

        assert_eq!(cost.as_i64(), 12_345_000);
        assert_eq!(cost.as_micros(), Some(12_345));
    }

    #[test]
    fn unknown_cost_is_not_zero_cost() {
        let cost = CostInfo::unknown();

        assert_eq!(cost.amount_nanos_usd, None);
        assert_eq!(cost.source, CostSource::Unknown);
    }
}
