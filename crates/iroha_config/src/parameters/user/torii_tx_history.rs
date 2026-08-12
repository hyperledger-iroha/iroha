// User-facing transaction-history visibility and authentication configuration.

/// Transaction-history visibility/auth configuration for Torii app API endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiTxHistory {
    /// Optional dataspace-keyed mandatory-alias policy file.
    pub mandatory_aliases_path: Option<PathBuf>,
    /// Maximum bytes accepted from the mandatory-alias policy file.
    #[config(default = "defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES")]
    pub mandatory_aliases_max_file_bytes: usize,
    /// Optional asset-definition restriction applied to visible-history endpoints.
    pub allowed_asset_definition_id: Option<String>,
    /// Optional JWT bearer verification configuration.
    pub jwt: Option<ToriiTxHistoryJwt>,
}

impl ToriiTxHistory {
    fn parse(self) -> actual::ToriiTxHistory {
        if !(1..=defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1)
            .contains(&self.mandatory_aliases_max_file_bytes)
        {
            panic!(
                "torii.tx_history.mandatory_aliases_max_file_bytes must be between 1 and {}",
                defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1
            );
        }
        self.mandatory_aliases_max_file_bytes
            .checked_mul(defaults::torii::tx_history::MANDATORY_ALIASES_MEMORY_PHASE_UNITS)
            .and_then(|bytes| {
                bytes.checked_add(
                    defaults::torii::tx_history::MANDATORY_ALIASES_NORMALIZATION_TRANSIENT_BYTES,
                )
            })
            .unwrap_or_else(|| {
                panic!(
                    "torii.tx_history.mandatory_aliases_max_file_bytes does not fit the startup memory envelope"
                )
            });
        let allowed_asset_definition_id = self.allowed_asset_definition_id.map(|value| {
            parse_asset_definition_selector_literal(
                "torii.tx_history.allowed_asset_definition_id",
                &value,
            )
        });
        actual::ToriiTxHistory {
            mandatory_aliases_path: self.mandatory_aliases_path,
            mandatory_aliases_max_file_bytes: self.mandatory_aliases_max_file_bytes,
            allowed_asset_definition_id,
            jwt: self.jwt.map(ToriiTxHistoryJwt::parse),
        }
    }
}

/// JWT bearer verification inputs for transaction-history endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct ToriiTxHistoryJwt {
    /// Expected JWT algorithm label (for example `RS256` or `HS256`).
    pub algorithm: String,
    /// Shared-secret material used for HMAC JWT algorithms.
    pub secret: Option<String>,
    /// PEM-encoded public key used for asymmetric JWT algorithms.
    pub public_key_pem: Option<String>,
    /// Optional issuer constraint.
    pub issuer: Option<String>,
    /// Optional audience constraint.
    pub audience: Option<String>,
}

impl ToriiTxHistoryJwt {
    fn parse(self) -> actual::ToriiTxHistoryJwt {
        let algorithm = self.algorithm.trim().to_ascii_uppercase();
        if algorithm.is_empty() {
            panic!("torii.tx_history.jwt.algorithm must not be empty");
        }
        match algorithm.as_str() {
            "HS256" | "HS384" | "HS512" => {
                let secret = self.secret.filter(|value| !value.trim().is_empty());
                if secret.is_none() {
                    panic!("torii.tx_history.jwt.secret must be set for HMAC JWT algorithms");
                }
                actual::ToriiTxHistoryJwt {
                    algorithm,
                    secret,
                    public_key_pem: None,
                    issuer: self.issuer.filter(|value| !value.trim().is_empty()),
                    audience: self.audience.filter(|value| !value.trim().is_empty()),
                }
            }
            "RS256" | "RS384" | "RS512" | "PS256" | "PS384" | "PS512" | "ES256" | "ES384"
            | "EDDSA" => {
                let public_key_pem = self.public_key_pem.filter(|value| !value.trim().is_empty());
                if public_key_pem.is_none() {
                    panic!(
                        "torii.tx_history.jwt.public_key_pem must be set for asymmetric JWT algorithms"
                    );
                }
                actual::ToriiTxHistoryJwt {
                    algorithm,
                    secret: None,
                    public_key_pem,
                    issuer: self.issuer.filter(|value| !value.trim().is_empty()),
                    audience: self.audience.filter(|value| !value.trim().is_empty()),
                }
            }
            other => panic!(
                "invalid torii.tx_history.jwt.algorithm `{other}`; expected HS256/384/512, RS256/384/512, PS256/384/512, ES256/384, or EdDSA"
            ),
        }
    }
}

#[cfg(test)]
mod torii_tx_history_tests {
    use super::*;

    #[test]
    fn torii_tx_history_parse_accepts_asset_alias_selector() {
        let parsed = ToriiTxHistory {
            mandatory_aliases_path: None,
            mandatory_aliases_max_file_bytes:
                defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES,
            allowed_asset_definition_id: Some("xor#universal".to_owned()),
            jwt: None,
        }
        .parse();

        assert_eq!(
            parsed.allowed_asset_definition_id.as_deref(),
            Some("xor#universal")
        );
        assert_eq!(
            parsed.mandatory_aliases_max_file_bytes,
            defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES
        );
    }

    #[test]
    fn torii_tx_history_parse_rejects_invalid_asset_selector() {
        let panic = std::panic::catch_unwind(|| {
            ToriiTxHistory {
                mandatory_aliases_path: None,
                mandatory_aliases_max_file_bytes:
                    defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES,
                allowed_asset_definition_id: Some("not a selector".to_owned()),
                jwt: None,
            }
            .parse();
        });

        assert!(panic.is_err(), "expected invalid selector to panic");
    }

    #[test]
    fn torii_tx_history_parse_rejects_invalid_alias_policy_memory_geometry() {
        for maximum in [
            0,
            defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1 + 1,
            usize::MAX,
        ] {
            let panic = std::panic::catch_unwind(|| {
                ToriiTxHistory {
                    mandatory_aliases_path: None,
                    mandatory_aliases_max_file_bytes: maximum,
                    allowed_asset_definition_id: None,
                    jwt: None,
                }
                .parse();
            });

            assert!(
                panic.is_err(),
                "expected invalid maximum {maximum} to panic"
            );
        }
    }
}
