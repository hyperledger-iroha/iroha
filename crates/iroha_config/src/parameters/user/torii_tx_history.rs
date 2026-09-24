/// Asset restriction for the signed transaction-history feed.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ToriiTxHistory {
    /// A canonical Base58 asset definition id or an on-chain asset alias.
    pub allowed_asset_definition_id: Option<String>,
}
impl ToriiTxHistory {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::ToriiTxHistory {
        actual::ToriiTxHistory {
            allowed_asset_definition_id: self
                .allowed_asset_definition_id
                .and_then(|value| parse_tx_history_asset_selector(value, emitter)),
        }
    }
}
fn parse_tx_history_asset_selector(
    value: String,
    emitter: &mut Emitter<ParseError>,
) -> Option<String> {
    const FIELD: &str = "torii.tx_history.allowed_asset_definition_id";
    if value.trim() != value {
        emit_torii_config_error(
            emitter,
            format!("{FIELD} must not contain surrounding whitespace"),
        );
        return None;
    }
    if let Ok(asset_definition_id) = AssetDefinitionId::parse_address_literal(&value) {
        let canonical = asset_definition_id.canonical_address();
        if canonical == value {
            return Some(value);
        }
        emit_torii_config_error(
            emitter,
            format!("{FIELD} must use the canonical Base58 spelling `{canonical}`"),
        );
        return None;
    }
    match AssetDefinitionAlias::from_str(&value) {
        Ok(_) => Some(value),
        Err(err) => {
            emit_torii_config_error(
                emitter,
                format!(
                    "invalid {FIELD} `{value}`: {err}; expected a canonical Base58 asset definition id or on-chain asset alias literal"
                ),
            );
            None
        }
    }
}
#[cfg(test)]
mod torii_tx_history_tests {
    use super::*;

    fn parse_error(config: ToriiTxHistory) -> String {
        let mut emitter = Emitter::new();
        let _ = config.parse(&mut emitter);
        let error = emitter
            .into_result()
            .expect_err("configuration must be rejected");
        format!("{error:?}")
    }

    #[test]
    fn torii_tx_history_parse_accepts_asset_alias_selector() {
        let mut emitter = Emitter::new();
        let parsed = ToriiTxHistory {
            allowed_asset_definition_id: Some("xor#universal".to_owned()),
        }
        .parse(&mut emitter);
        emitter
            .into_result()
            .expect("canonical history configuration must be accepted");
        assert_eq!(
            parsed.allowed_asset_definition_id.as_deref(),
            Some("xor#universal")
        );
    }

    #[test]
    fn torii_tx_history_parse_rejects_invalid_asset_selector() {
        for selector in ["not a selector", " xor#universal", "xor#universal "] {
            let report = parse_error(ToriiTxHistory {
                allowed_asset_definition_id: Some(selector.to_owned()),
            });
            assert!(
                report.contains("allowed_asset_definition_id"),
                "unexpected error for {selector:?}: {report}"
            );
        }
    }
}
