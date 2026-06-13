// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct MeteredUsage {
    pub tokens: TokenUsage,
    pub server_tools: ServerToolUsage,
    pub reported_total_tokens: Option<u64>,
    pub extra_total_tokens: u64,
}

impl MeteredUsage {
    pub fn known_total_tokens(&self) -> u64 {
        self.tokens
            .total_tokens()
            .saturating_add(self.extra_total_tokens)
    }

    pub fn bucket_total_tokens(&self) -> u64 {
        self.tokens.bucket_total_tokens()
    }

    pub fn saturating_add_assign(&mut self, other: &Self) {
        self.tokens.saturating_add_assign(&other.tokens);
        self.server_tools.saturating_add_assign(&other.server_tools);
        self.extra_total_tokens = self
            .extra_total_tokens
            .saturating_add(other.extra_total_tokens);
        self.reported_total_tokens = None;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_creation_ephemeral_5m_input_tokens: u64,
    pub cache_creation_ephemeral_1h_input_tokens: u64,
    pub reasoning_output_tokens: u64,
}

impl TokenUsage {
    pub fn with_cache_creation_ttl(
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
        cache_creation_ephemeral_5m_input_tokens: u64,
        cache_creation_ephemeral_1h_input_tokens: u64,
        reasoning_output_tokens: u64,
    ) -> Self {
        let mut usage = Self {
            input_tokens,
            output_tokens,
            cache_read_input_tokens,
            cache_creation_input_tokens,
            cache_creation_ephemeral_5m_input_tokens,
            cache_creation_ephemeral_1h_input_tokens,
            reasoning_output_tokens,
        };
        usage.reconcile_cache_creation_from_ttl();
        usage
    }

    pub fn total_tokens(&self) -> u64 {
        self.bucket_total_tokens()
            .saturating_add(self.reasoning_output_tokens)
    }

    pub fn bucket_total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_input_tokens)
            .saturating_add(self.cache_creation_input_tokens)
    }

    pub fn saturating_add_assign(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(other.cache_read_input_tokens);
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(other.cache_creation_input_tokens);
        self.cache_creation_ephemeral_5m_input_tokens = self
            .cache_creation_ephemeral_5m_input_tokens
            .saturating_add(other.cache_creation_ephemeral_5m_input_tokens);
        self.cache_creation_ephemeral_1h_input_tokens = self
            .cache_creation_ephemeral_1h_input_tokens
            .saturating_add(other.cache_creation_ephemeral_1h_input_tokens);
        self.reasoning_output_tokens = self
            .reasoning_output_tokens
            .saturating_add(other.reasoning_output_tokens);
    }

    pub fn cache_creation_ttl_total(&self) -> u64 {
        self.cache_creation_ephemeral_5m_input_tokens
            .saturating_add(self.cache_creation_ephemeral_1h_input_tokens)
    }

    pub fn reconcile_cache_creation_from_ttl(&mut self) {
        let ttl_total = self.cache_creation_ttl_total();
        if ttl_total > 0 && self.cache_creation_input_tokens != ttl_total {
            self.cache_creation_input_tokens = ttl_total;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct ServerToolUsage {
    pub web_search_requests: u64,
    pub other: BTreeMap<String, u64>,
}

impl ServerToolUsage {
    pub fn total_requests(&self) -> u64 {
        self.other
            .values()
            .fold(self.web_search_requests, |total, value| {
                total.saturating_add(*value)
            })
    }

    pub fn saturating_add_assign(&mut self, other: &Self) {
        self.web_search_requests = self
            .web_search_requests
            .saturating_add(other.web_search_requests);
        for (name, count) in &other.other {
            let total = self.other.entry(name.clone()).or_default();
            *total = total.saturating_add(*count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_total_sums_all_token_buckets() {
        let usage = TokenUsage {
            input_tokens: 10,
            output_tokens: 20,
            cache_read_input_tokens: 30,
            cache_creation_input_tokens: 40,
            cache_creation_ephemeral_5m_input_tokens: 25,
            cache_creation_ephemeral_1h_input_tokens: 15,
            reasoning_output_tokens: 50,
        };

        assert_eq!(usage.total_tokens(), 150);
        assert_eq!(usage.bucket_total_tokens(), 100);
    }

    #[test]
    fn ttl_cache_creation_total_replaces_inconsistent_flat_total() {
        let usage = TokenUsage::with_cache_creation_ttl(10, 20, 30, 999, 25, 15, 50);

        assert_eq!(usage.cache_creation_input_tokens, 40);
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn metered_usage_adds_extra_reported_unknown_tokens() {
        let usage = MeteredUsage {
            tokens: TokenUsage {
                input_tokens: 1,
                output_tokens: 2,
                ..TokenUsage::default()
            },
            extra_total_tokens: 4,
            ..MeteredUsage::default()
        };

        assert_eq!(usage.known_total_tokens(), 7);
    }

    #[test]
    fn token_total_saturates_on_overflow() {
        let usage = TokenUsage {
            input_tokens: u64::MAX,
            output_tokens: 1,
            ..TokenUsage::default()
        };

        assert_eq!(usage.total_tokens(), u64::MAX);
    }

    #[test]
    fn cache_creation_ttl_total_saturates_on_overflow() {
        let usage = TokenUsage {
            cache_creation_ephemeral_5m_input_tokens: u64::MAX,
            cache_creation_ephemeral_1h_input_tokens: 1,
            ..TokenUsage::default()
        };

        assert_eq!(usage.cache_creation_ttl_total(), u64::MAX);
    }

    #[test]
    fn metered_usage_total_saturates_on_overflow() {
        let usage = MeteredUsage {
            tokens: TokenUsage {
                input_tokens: u64::MAX,
                ..TokenUsage::default()
            },
            extra_total_tokens: 1,
            ..MeteredUsage::default()
        };

        assert_eq!(usage.known_total_tokens(), u64::MAX);
    }

    #[test]
    fn server_tool_total_saturates_on_overflow() {
        let usage = ServerToolUsage {
            web_search_requests: u64::MAX,
            other: BTreeMap::from([("mcp_tool_requests".to_owned(), 1)]),
        };

        assert_eq!(usage.total_requests(), u64::MAX);
    }

    #[test]
    fn metered_usage_add_assign_saturates_totals() {
        let mut usage = MeteredUsage {
            tokens: TokenUsage {
                input_tokens: u64::MAX,
                output_tokens: 1,
                ..TokenUsage::default()
            },
            extra_total_tokens: 1,
            reported_total_tokens: Some(10),
            ..MeteredUsage::default()
        };
        usage.saturating_add_assign(&MeteredUsage {
            tokens: TokenUsage {
                input_tokens: 1,
                output_tokens: 2,
                ..TokenUsage::default()
            },
            extra_total_tokens: 2,
            reported_total_tokens: Some(5),
            ..MeteredUsage::default()
        });

        assert_eq!(usage.tokens.input_tokens, u64::MAX);
        assert_eq!(usage.tokens.output_tokens, 3);
        assert_eq!(usage.extra_total_tokens, 3);
        assert_eq!(usage.reported_total_tokens, None);
    }
}
