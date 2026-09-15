//! Typed account-alias error details within the canonical Torii error envelope.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// A deterministic alias-planning rejection (HTTP 400, 403, or 409).
pub const ALIAS_SETUP_REJECTED_CODE: &str = "alias_setup_rejected";
/// A plan awaiting committed state (HTTP 503).
pub const ALIAS_SETUP_PENDING_CODE: &str = "alias_setup_pending";
/// An exact canonical alias did not resolve (HTTP 404).
pub const ACCOUNT_ALIAS_NOT_FOUND_CODE: &str = "account_alias_not_found";
/// The exact account lookup was absent on all successfully queried routes (HTTP 404).
pub const ACCOUNT_ALIASES_BY_ACCOUNT_NOT_FOUND_CODE: &str = "account_aliases_by_account_not_found";
/// Bound for a complete account-alias absence error envelope.
pub const ACCOUNT_ALIAS_ABSENCE_MAX_BYTES: usize = 4096;

/// Authoritative absence for one canonical account alias, not a finality proof.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii_shared::aliases::AccountAliasNotFoundV1")]
#[norito(deny_unknown_fields)]
pub struct AccountAliasNotFoundV1 {
    /// Exact canonical requested alias.
    pub alias: String,
}

/// Authoritative absence for an account lookup with exact scope filters.
/// Null filters mean unfiltered; they are required so an omitted selector is not accepted.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii_shared::aliases::AccountAliasesByAccountNotFoundV1")]
#[norito(deny_unknown_fields)]
pub struct AccountAliasesByAccountNotFoundV1 {
    /// Exact canonical I105 account literal.
    pub account_id: String,
    /// Exact canonical dataspace filter, or null for all dataspaces.
    #[norito(required)]
    pub dataspace: Option<String>,
    /// Exact canonical domain filter, or null for all domains.
    #[norito(required)]
    pub domain: Option<String>,
}
impl AccountAliasesByAccountNotFoundV1 {
    /// Compare every selector field. The caller separately checks HTTP status and error code.
    #[must_use]
    pub fn matches_selector(
        &self,
        account_id: &str,
        dataspace: Option<&str>,
        domain: Option<&str>,
    ) -> bool {
        self.account_id == account_id
            && self.dataspace.as_deref() == dataspace
            && self.domain.as_deref() == domain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorDetails, ErrorEnvelope};
    #[test]
    fn alias_error_details_roundtrip_and_reject_unknown_fields() {
        let alias = AccountAliasNotFoundV1 {
            alias: "admin@dpn".into(),
        };
        let account = AccountAliasesByAccountNotFoundV1 {
            account_id: "exact-public-account-selector".into(),
            dataspace: Some("dpn".into()),
            domain: None,
        };
        for details in [
            ErrorDetails {
                account_alias_not_found: Some(alias.clone()),
                ..Default::default()
            },
            ErrorDetails {
                account_aliases_by_account_not_found: Some(account.clone()),
                ..Default::default()
            },
            ErrorDetails {
                alias_setup_report: Some(iroha_data_model::alias_setup::AliasSetupReportV1::new(
                    iroha_data_model::alias_setup::AliasSetupStatusV1::Blocked,
                    vec![],
                )),
                ..Default::default()
            },
        ] {
            assert!(!details.is_empty());
            let envelope =
                ErrorEnvelope::new("test_code", "typed error details").with_details(details);
            let json = norito::json::to_vec(&envelope).unwrap();
            let decoded: ErrorEnvelope = norito::json::from_slice(&json).unwrap();
            assert_eq!(norito::json::to_vec(&decoded).unwrap(), json);
            let native = norito::to_bytes(&envelope).unwrap();
            let decoded: ErrorEnvelope = norito::decode_from_bytes(&native).unwrap();
            assert_eq!(norito::json::to_vec(&decoded).unwrap(), json);
        }
        assert!(account.matches_selector("exact-public-account-selector", Some("dpn"), None));
        assert!(!account.matches_selector("other", Some("dpn"), None));
        assert!(!account.matches_selector("exact-public-account-selector", None, None));
        for malformed in [
            r#"{"alias":"admin@dpn","extra":true}"#,
            r#"{"alias":"admin@dpn","alias":"other@dpn"}"#,
        ] {
            assert!(norito::json::from_str::<AccountAliasNotFoundV1>(malformed).is_err());
        }
        for malformed in [
            r#"{"account_id":"a","dataspace":null}"#,
            r#"{"account_id":"a","domain":null}"#,
            r#"{"account_id":"a","dataspace":null,"domain":null,"extra":true}"#,
        ] {
            assert!(
                norito::json::from_str::<AccountAliasesByAccountNotFoundV1>(malformed).is_err()
            );
        }
        // The embedded native report is recursively closed, including tagged enums.
        use iroha_data_model::alias_setup::*;
        let report = AliasSetupReportV1::new(
            AliasSetupStatusV1::Blocked,
            vec![AliasSetupDiagnosticV1 {
                phase: AliasSetupValidationPhaseV1::Planning,
                code: "alias.plan.conflict".into(),
                severity: AliasSetupSeverityV1::Error,
                resource: None,
                config_path: None,
                expected: None,
                actual: None,
                remediation: "request a corrected plan".into(),
            }],
        );
        let envelope = ErrorEnvelope::new(ALIAS_SETUP_REJECTED_CODE, "plan rejected").with_details(
            ErrorDetails {
                alias_setup_report: Some(report),
                ..Default::default()
            },
        );
        let json = norito::json::to_vec(&envelope).unwrap();
        for level in 0..5 {
            let mut value: norito::json::Value = norito::json::from_slice(&json).unwrap();
            let report = value
                .as_object_mut()
                .unwrap()
                .get_mut("details")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .get_mut("alias_setup_report")
                .unwrap();
            let target = match level {
                0 => report,
                1 => report.as_object_mut().unwrap().get_mut("status").unwrap(),
                _ => {
                    let diagnostic = &mut report
                        .as_object_mut()
                        .unwrap()
                        .get_mut("diagnostics")
                        .unwrap()
                        .as_array_mut()
                        .unwrap()[0];
                    match level {
                        2 => diagnostic,
                        3 => diagnostic
                            .as_object_mut()
                            .unwrap()
                            .get_mut("phase")
                            .unwrap(),
                        _ => diagnostic
                            .as_object_mut()
                            .unwrap()
                            .get_mut("severity")
                            .unwrap(),
                    }
                }
            };
            target
                .as_object_mut()
                .unwrap()
                .insert("unexpected".into(), norito::json::Value::Bool(true));
            assert!(
                norito::json::from_slice::<ErrorEnvelope>(&norito::json::to_vec(&value).unwrap())
                    .is_err(),
                "unknown report field at level {level}"
            );
        }
    }
}
