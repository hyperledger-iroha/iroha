use iroha_data_model::account::AccountId;
/// Render an [`AccountId`] using the canonical Torii literal (I105).
pub fn display_literal(account_id: &AccountId) -> String {
    account_id.to_string()
}
/// Render a canonical literal string for display.
pub fn display_from_literal(literal: &str) -> Option<String> {
    match AccountId::parse_encoded(literal) {
        Ok(account_id) => Some(account_id.to_string()),
        Err(err) => {
            iroha_logger::warn!(
                %literal,
                %err,
                "failed to parse account literal while normalizing display literal; omitting invalid value"
            );
            None
        }
    }
}
/// Stable label used for telemetry counters.
pub const fn metric_label() -> &'static str {
    "i105"
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_test_samples::ALICE_ID;
    use std::sync::{LazyLock, Mutex};
    static RESOLVER_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    #[test]
    fn display_literal_and_from_literal_round_trip() {
        let canonical_literal = ALICE_ID.to_string();
        assert_eq!(display_literal(&ALICE_ID), canonical_literal);
        assert_eq!(
            display_from_literal(&canonical_literal),
            Some(canonical_literal)
        );
    }
    #[test]
    fn display_from_literal_omits_invalid_input() {
        let invalid_literal = "not-an-account@banka.dataspace";
        assert_eq!(display_from_literal(invalid_literal), None);
    }
    #[test]
    fn account_literal_parsing_is_selector_free() {
        let _lock = RESOLVER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let literal = ALICE_ID.to_string();
        let parsed = AccountId::parse_encoded(&literal).expect("i105 literal should parse");
        assert_eq!(parsed.to_string(), literal);
        assert_eq!(parsed.controller(), ALICE_ID.controller());
        let reparsed =
            AccountId::parse_encoded(&literal).expect("repeated hook call should be harmless");
        assert_eq!(reparsed.to_string(), literal);
        assert_eq!(reparsed.controller(), ALICE_ID.controller());
    }
}
