// User-facing transaction-history visibility and authentication configuration.
/// Transaction-history visibility/auth configuration for Torii app API endpoints.
#[derive(Debug, ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
    fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::ToriiTxHistory {
        if !(1..=defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1)
            .contains(&self.mandatory_aliases_max_file_bytes)
        {
            emit_torii_config_error(
                emitter,
                format!(
                    "torii.tx_history.mandatory_aliases_max_file_bytes must be between 1 and {}",
                    defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1
                ),
            );
        }
        if self
            .mandatory_aliases_max_file_bytes
            .checked_mul(defaults::torii::tx_history::MANDATORY_ALIASES_MEMORY_PHASE_UNITS)
            .and_then(|bytes| {
                bytes.checked_add(
                    defaults::torii::tx_history::MANDATORY_ALIASES_NORMALIZATION_TRANSIENT_BYTES,
                )
            })
            .is_none()
        {
            emit_torii_config_error(
                emitter,
                "torii.tx_history.mandatory_aliases_max_file_bytes does not fit the startup memory envelope",
            );
        }
        let allowed_asset_definition_id = self
            .allowed_asset_definition_id
            .and_then(|value| parse_tx_history_asset_selector(value, emitter));
        actual::ToriiTxHistory {
            mandatory_aliases_path: self.mandatory_aliases_path,
            mandatory_aliases_max_file_bytes: self.mandatory_aliases_max_file_bytes,
            allowed_asset_definition_id,
            jwt: self.jwt.and_then(|jwt| jwt.parse(emitter)),
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
/// JWT bearer verification inputs for transaction-history endpoints.
#[derive(ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
impl Debug for ToriiTxHistoryJwt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToriiTxHistoryJwt")
            .field("algorithm", &self.algorithm)
            .field(
                "secret",
                &self
                    .secret
                    .as_ref()
                    .map(|_| "[REDACTED transaction-history JWT secret]"),
            )
            .field(
                "public_key_pem",
                &self
                    .public_key_pem
                    .as_ref()
                    .map(|_| "[REDACTED transaction-history JWT key]"),
            )
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .finish()
    }
}
impl ToriiTxHistoryJwt {
    fn parse(self, emitter: &mut Emitter<ParseError>) -> Option<actual::ToriiTxHistoryJwt> {
        let algorithm = self.algorithm;
        let issuer = parse_tx_history_jwt_constraint(
            "torii.tx_history.jwt.issuer",
            self.issuer,
            emitter,
        );
        let audience = parse_tx_history_jwt_constraint(
            "torii.tx_history.jwt.audience",
            self.audience,
            emitter,
        );
        let mut valid = true;
        let (secret, public_key_pem) = match algorithm.as_str() {
            "HS256" | "HS384" | "HS512" => {
                if self.public_key_pem.is_some() {
                    emit_torii_config_error(
                        emitter,
                        "torii.tx_history.jwt.public_key_pem must be absent for HMAC JWT algorithms",
                    );
                    valid = false;
                }
                match self.secret {
                    Some(secret) if !secret.trim().is_empty() => (Some(secret), None),
                    _ => {
                        emit_torii_config_error(
                            emitter,
                            "torii.tx_history.jwt.secret must be set to non-whitespace material for HMAC JWT algorithms",
                        );
                        valid = false;
                        (None, None)
                    }
                }
            }
            "RS256" | "RS384" | "RS512" | "PS256" | "PS384" | "PS512" | "ES256" | "ES384"
            | "EdDSA" => {
                if self.secret.is_some() {
                    emit_torii_config_error(
                        emitter,
                        "torii.tx_history.jwt.secret must be absent for asymmetric JWT algorithms",
                    );
                    valid = false;
                }
                match self.public_key_pem {
                    Some(public_key_pem) if !public_key_pem.trim().is_empty() => {
                        (None, Some(public_key_pem))
                    }
                    _ => {
                        emit_torii_config_error(
                            emitter,
                            "torii.tx_history.jwt.public_key_pem must be set to non-whitespace material for asymmetric JWT algorithms",
                        );
                        valid = false;
                        (None, None)
                    }
                }
            }
            other => {
                emit_torii_config_error(
                    emitter,
                    format!(
                        "invalid torii.tx_history.jwt.algorithm `{other}`; expected an exact HS256/384/512, RS256/384/512, PS256/384/512, ES256/384, or EdDSA label"
                    ),
                );
                return None;
            }
        };
        if !valid {
            return None;
        }
        Some(actual::ToriiTxHistoryJwt {
            algorithm,
            secret,
            public_key_pem,
            issuer,
            audience,
        })
    }
}
fn parse_tx_history_jwt_constraint(
    field: &str,
    value: Option<String>,
    emitter: &mut Emitter<ParseError>,
) -> Option<String> {
    if let Some(value) = value.as_ref()
        && (value.is_empty() || value.trim() != value)
    {
        emit_torii_config_error(
            emitter,
            format!("{field} must be non-empty and must not contain surrounding whitespace"),
        );
        return None;
    }
    value
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
            mandatory_aliases_path: None,
            mandatory_aliases_max_file_bytes:
                defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES,
            allowed_asset_definition_id: Some("xor#universal".to_owned()),
            jwt: None,
        }
        .parse(&mut emitter);
        emitter
            .into_result()
            .expect("canonical history configuration must be accepted");
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
        for selector in ["not a selector", " xor#universal", "xor#universal "] {
            let report = parse_error(ToriiTxHistory {
                mandatory_aliases_path: None,
                mandatory_aliases_max_file_bytes:
                    defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES,
                allowed_asset_definition_id: Some(selector.to_owned()),
                jwt: None,
            });
            assert!(
                report.contains("allowed_asset_definition_id"),
                "unexpected error for {selector:?}: {report}"
            );
        }
    }
    #[test]
    fn torii_tx_history_parse_rejects_invalid_alias_policy_memory_geometry() {
        for maximum in [
            0,
            defaults::torii::tx_history::MANDATORY_ALIASES_MAX_FILE_BYTES_V1 + 1,
            usize::MAX,
        ] {
            let report = parse_error(ToriiTxHistory {
                mandatory_aliases_path: None,
                mandatory_aliases_max_file_bytes: maximum,
                allowed_asset_definition_id: None,
                jwt: None,
            });
            assert!(
                report.contains("mandatory_aliases_max_file_bytes"),
                "unexpected error for invalid maximum {maximum}: {report}"
            );
        }
    }
    #[test]
    fn torii_tx_history_jwt_rejects_noncanonical_algorithm_labels() {
        for algorithm in [" hs256", "hs256", "HS256 ", "EDDSA"] {
            let jwt = ToriiTxHistoryJwt {
                algorithm: algorithm.to_owned(),
                secret: Some("secret".to_owned()),
                public_key_pem: Some("public key".to_owned()),
                issuer: None,
                audience: None,
            };
            let mut emitter = Emitter::new();
            assert!(jwt.parse(&mut emitter).is_none());
            let report = format!(
                "{:?}",
                emitter
                    .into_result()
                    .expect_err("noncanonical algorithm must fail closed")
            );
            assert!(report.contains("jwt.algorithm"), "{algorithm:?}: {report}");
        }
    }

    #[test]
    fn torii_tx_history_jwt_rejects_contradictory_or_empty_inputs() {
        let cases = [
            ToriiTxHistoryJwt {
                algorithm: "HS256".to_owned(),
                secret: Some("secret".to_owned()),
                public_key_pem: Some("public key".to_owned()),
                issuer: None,
                audience: None,
            },
            ToriiTxHistoryJwt {
                algorithm: "RS256".to_owned(),
                secret: Some("secret".to_owned()),
                public_key_pem: Some("public key".to_owned()),
                issuer: None,
                audience: None,
            },
            ToriiTxHistoryJwt {
                algorithm: "HS256".to_owned(),
                secret: Some("   ".to_owned()),
                public_key_pem: None,
                issuer: None,
                audience: None,
            },
            ToriiTxHistoryJwt {
                algorithm: "EdDSA".to_owned(),
                secret: None,
                public_key_pem: Some(String::new()),
                issuer: None,
                audience: None,
            },
        ];
        for jwt in cases {
            let mut emitter = Emitter::new();
            assert!(jwt.parse(&mut emitter).is_none());
            emitter
                .into_result()
                .expect_err("contradictory or empty JWT inputs must fail closed");
        }
    }

    #[test]
    fn torii_tx_history_jwt_rejects_noncanonical_claim_constraints() {
        for (issuer, audience) in [
            (Some(String::new()), None),
            (Some(" issuer".to_owned()), None),
            (None, Some("audience ".to_owned())),
        ] {
            let jwt = ToriiTxHistoryJwt {
                algorithm: "HS256".to_owned(),
                secret: Some("secret".to_owned()),
                public_key_pem: None,
                issuer,
                audience,
            };
            let mut emitter = Emitter::new();
            let _ = jwt.parse(&mut emitter);
            emitter
                .into_result()
                .expect_err("noncanonical claim constraint must fail closed");
        }
    }

    #[test]
    fn torii_tx_history_jwt_debug_redacts_key_material() {
        let jwt = ToriiTxHistoryJwt {
            algorithm: "HS256".to_owned(),
            secret: Some("do-not-log-this-secret".to_owned()),
            public_key_pem: Some("do-not-log-this-key".to_owned()),
            issuer: Some("issuer".to_owned()),
            audience: Some("audience".to_owned()),
        };
        let debug = format!("{jwt:?}");
        assert!(debug.contains("REDACTED"), "{debug}");
        assert!(!debug.contains("do-not-log-this-secret"), "{debug}");
        assert!(!debug.contains("do-not-log-this-key"), "{debug}");
    }
}
