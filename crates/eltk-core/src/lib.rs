// SPDX-License-Identifier: MIT

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
