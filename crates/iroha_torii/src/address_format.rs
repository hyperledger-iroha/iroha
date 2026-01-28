use iroha_data_model::{account::AccountId, query::error::QueryExecutionFail};

use crate::Result;

/// Preferred textual encoding for account identifiers in Torii responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressFormatPreference {
    /// IH58 literal (`{ih58}`).
    Ih58,
    /// Compressed `sora` literal.
    Compressed,
}

impl AddressFormatPreference {
    /// Parse an address_format query parameter into an [`AddressFormatPreference`].
    pub fn from_param(value: Option<&str>) -> Result<Self> {
        let Some(raw) = value else {
            return Ok(Self::Ih58);
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Self::Ih58);
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "ih58" | "ih-b32" | "canonical" => Ok(Self::Ih58),
            "compressed" | "sora" => Ok(Self::Compressed),
            other => Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
                    format!("address_format `{other}` must be `ih58` or `compressed`"),
                )),
            )),
        }
    }

    /// Render an [`AccountId`] according to the preference.
    pub fn display_literal(self, account_id: &AccountId) -> String {
        match self {
            Self::Ih58 => account_id.to_string(),
            Self::Compressed => account_id
                .to_account_address()
                .and_then(|address| address.to_compressed_sora())
                .map(|compressed| compressed)
                .unwrap_or_else(|err| {
                    iroha_logger::error!(
                        %err,
                        "failed to encode account id `{}` as compressed literal; falling back to IH58",
                        account_id
                    );
                    account_id.to_string()
                }),
        }
    }

    /// Render a canonical literal string (IH58) according to the preference.
    pub fn display_from_literal(self, literal: &str) -> String {
        #[cfg(test)]
        crate::ensure_test_domain_selector_resolver();

        match self {
            Self::Ih58 => literal.to_string(),
            Self::Compressed => {
                let parsed = match AccountId::parse(literal) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        iroha_logger::warn!(
                            %literal,
                            %err,
                            "failed to parse account literal while applying address_format; returning original literal"
                        );
                        return literal.to_string();
                    }
                };
                match parsed.account_id().to_account_address() {
                    Ok(address) => match address.to_compressed_sora() {
                        Ok(compressed) => compressed,
                        Err(err) => {
                            iroha_logger::warn!(
                                %literal,
                                %err,
                                "failed to encode compressed literal; returning original literal"
                            );
                            literal.to_string()
                        }
                    },
                    Err(err) => {
                        iroha_logger::warn!(
                            %literal,
                            %err,
                            "failed to construct account address while applying address_format; returning original literal"
                        );
                        literal.to_string()
                    }
                }
            }
        }
    }

    /// Stable label used for telemetry counters.
    pub fn metric_label(self) -> &'static str {
        match self {
            Self::Ih58 => "ih58",
            Self::Compressed => "compressed",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use iroha_data_model::account::{
        account_domain_selector_resolver, clear_account_domain_selector_resolver,
        set_account_domain_selector_resolver,
    };
    use iroha_test_samples::ALICE_ID;

    use super::*;

    static RESOLVER_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn from_param_defaults_and_accepts_aliases() {
        assert_eq!(
            AddressFormatPreference::from_param(None).expect("no param defaults to IH58"),
            AddressFormatPreference::Ih58
        );
        assert_eq!(
            AddressFormatPreference::from_param(Some(" ih-b32 "))
                .expect("alias ih-b32 should map to IH58"),
            AddressFormatPreference::Ih58
        );
        assert_eq!(
            AddressFormatPreference::from_param(Some("sora"))
                .expect("sora alias should map to compressed"),
            AddressFormatPreference::Compressed
        );
        assert!(
            AddressFormatPreference::from_param(Some("binary")).is_err(),
            "unknown formats must be rejected"
        );
    }

    #[test]
    fn display_literal_and_from_literal_round_trip() {
        let canonical_literal = ALICE_ID.to_string();
        let compressed_literal = ALICE_ID
            .to_account_address()
            .and_then(|addr| addr.to_compressed_sora())
            .map(|compressed| compressed)
            .expect("compressed literal should encode");

        assert_eq!(
            AddressFormatPreference::Ih58.display_literal(&ALICE_ID),
            canonical_literal
        );
        assert_eq!(
            AddressFormatPreference::Compressed.display_literal(&ALICE_ID),
            compressed_literal
        );

        assert_eq!(
            AddressFormatPreference::Ih58.display_from_literal(&canonical_literal),
            canonical_literal
        );
        assert_eq!(
            AddressFormatPreference::Compressed.display_from_literal(&canonical_literal),
            compressed_literal
        );
    }

    #[test]
    fn display_from_literal_falls_back_on_invalid_input() {
        let invalid_literal = "not-an-account@wonderland";
        assert_eq!(
            AddressFormatPreference::Compressed.display_from_literal(invalid_literal),
            invalid_literal
        );
    }

    #[test]
    fn ensure_test_domain_selector_resolver_enables_parsing() {
        let _lock = RESOLVER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = account_domain_selector_resolver();
        clear_account_domain_selector_resolver();

        crate::ensure_test_domain_selector_resolver();

        let literal = ALICE_ID.to_string();
        let parsed = AccountId::parse(&literal).expect("resolver should parse IH58 literal");
        assert_eq!(parsed.account_id(), &*ALICE_ID);

        clear_account_domain_selector_resolver();
        crate::ensure_test_domain_selector_resolver();

        let reparsed = AccountId::parse(&literal).expect("resolver reinstalls after clear");
        assert_eq!(reparsed.account_id(), &*ALICE_ID);

        clear_account_domain_selector_resolver();
        if let Some(resolver) = previous {
            set_account_domain_selector_resolver(resolver);
        }
    }
}
