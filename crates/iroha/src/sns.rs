//! Rust client helpers for the Sora Name Service registrar routes.
use crate::{
    client::{Client, ResponseReport, join_torii_url},
    data_model::sns::{
        ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID, NameRecordV1,
        NameSelectorV1, SuffixId, SuffixPolicyV1,
    },
    http::{Method as HttpMethod, RequestBuilder, Response, StatusCode},
};
use eyre::Result;
use iroha_torii_shared::sns::{SNS_REGISTRATION_NOT_FOUND_MAX_BYTES, SnsRegistrationNotFoundV1};
const APPLICATION_JSON: &str = "application/json";
fn ensure_status(
    response: &Response<Vec<u8>>,
    expected: StatusCode,
    context: &str,
) -> eyre::Result<()> {
    if response.status() == expected {
        return Ok(());
    }
    let message = format!("{context}; expected HTTP status {expected}");
    let report = match ResponseReport::with_msg(message, response) {
        Ok(report) | Err(report) => report.0,
    };
    Err(report)
}
/// Typed helper exposed by [`Client::sns()`].
pub struct SnsApi<'a> {
    client: &'a Client,
}
/// Namespace selector used by the ledger-backed SNS HTTP API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnsNamespacePath {
    /// Full account-alias keys.
    AccountAlias,
    /// Domain-name literals.
    Domain,
    /// Dataspace-alias literals.
    Dataspace,
}
impl SnsNamespacePath {
    /// Stable Torii path segment for this namespace.
    #[must_use]
    pub const fn as_path(self) -> &'static str {
        match self {
            Self::AccountAlias => "account-alias",
            Self::Domain => "domain",
            Self::Dataspace => "dataspace",
        }
    }
    /// Resolve the namespace from the fixed on-chain suffix id.
    ///
    /// # Errors
    ///
    /// Returns an error when the suffix id does not map to one of the fixed
    /// ledger-backed SNS namespaces.
    pub fn from_suffix_id(suffix_id: SuffixId) -> eyre::Result<Self> {
        match suffix_id {
            ACCOUNT_ALIAS_SUFFIX_ID => Ok(Self::AccountAlias),
            DOMAIN_NAME_SUFFIX_ID => Ok(Self::Domain),
            DATASPACE_ALIAS_SUFFIX_ID => Ok(Self::Dataspace),
            other => Err(eyre::eyre!("unsupported SNS namespace suffix id `{other}`")),
        }
    }
    /// Fixed on-chain suffix id for this namespace.
    #[must_use]
    pub const fn suffix_id(self) -> SuffixId {
        match self {
            Self::AccountAlias => ACCOUNT_ALIAS_SUFFIX_ID,
            Self::Domain => DOMAIN_NAME_SUFFIX_ID,
            Self::Dataspace => DATASPACE_ALIAS_SUFFIX_ID,
        }
    }
}
fn name_path(namespace: SnsNamespacePath, literal: &str) -> String {
    format!("v1/sns/names/{}/{literal}", namespace.as_path())
}
fn name_selector(namespace: SnsNamespacePath, literal: &str) -> Result<NameSelectorV1> {
    match namespace {
        SnsNamespacePath::AccountAlias => {
            let alias: crate::data_model::alias_setup::AccountAliasName = literal.parse()?;
            Ok(NameSelectorV1 {
                version: NameSelectorV1::VERSION,
                suffix_id: namespace.suffix_id(),
                label: alias.canonical_text(),
            })
        }
        SnsNamespacePath::Domain => {
            let domain = iroha_model_base::domain::DomainId::parse_fully_qualified(literal.trim())?;
            Ok(NameSelectorV1::new(
                namespace.suffix_id(),
                domain.to_string(),
            )?)
        }
        SnsNamespacePath::Dataspace => Ok(NameSelectorV1::new(namespace.suffix_id(), literal)?),
    }
}
impl<'a> SnsApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }
    /// GET `/v1/sns/policies/{suffix_id}`.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching or decoding the policy fails.
    pub fn get_policy(&self, suffix_id: u16) -> Result<SuffixPolicyV1> {
        let url = join_torii_url(
            &self.client.torii_url,
            &format!("v1/sns/policies/{suffix_id}"),
        );
        let response = self
            .client
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send_blocking()?;
        ensure_status(&response, StatusCode::OK, "unexpected SNS policy response")?;
        Ok(norito::json::from_slice(response.body())?)
    }
    /// GET `/v1/sns/names/{namespace}/{literal}`.
    ///
    /// # Errors
    ///
    /// Returns an error if the registration lookup or decoding fails.
    pub fn get_name(&self, namespace: SnsNamespacePath, literal: &str) -> Result<NameRecordV1> {
        let response = self.get_name_response(namespace, literal)?;
        Self::decode_name_response(&response)
    }
    /// Fetch a registration, returning `None` only for its typed authoritative absence.
    ///
    /// The response must identify the exact requested canonical selector. This
    /// unsigned HTTP observation is not a cryptographic proof of ledger state.
    ///
    /// # Errors
    ///
    /// Invalid selectors, transport failures, generic HTTP 404 responses, policy
    /// failures, malformed responses, and responses for another name remain errors.
    pub fn get_name_optional(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
    ) -> Result<Option<NameRecordV1>> {
        let selector = name_selector(namespace, literal)?;
        let response = self.get_name_response(namespace, literal)?;
        Self::decode_optional_name_response(&response, &selector)
    }
    fn decode_optional_name_response(
        response: &Response<Vec<u8>>,
        selector: &NameSelectorV1,
    ) -> Result<Option<NameRecordV1>> {
        if response.status() == StatusCode::NOT_FOUND {
            let content_type = response
                .headers()
                .get("Content-Type")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next());
            eyre::ensure!(
                content_type
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case(APPLICATION_JSON)),
                "SNS absence requires the typed JSON registration response"
            );
            eyre::ensure!(
                response.body().len() <= SNS_REGISTRATION_NOT_FOUND_MAX_BYTES,
                "SNS missing-registration response exceeds its bound"
            );
            let absence: SnsRegistrationNotFoundV1 = norito::json::from_slice(response.body())?;
            eyre::ensure!(
                absence.matches_selector(selector),
                "SNS missing-registration response differs from the requested selector"
            );
            return Ok(None);
        }
        let record = Self::decode_name_response(response)?;
        eyre::ensure!(
            record.selector == *selector,
            "SNS registration response differs from the requested selector"
        );
        Ok(Some(record))
    }
    fn get_name_response(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
    ) -> Result<Response<Vec<u8>>> {
        let path = name_path(namespace, literal);
        let url = join_torii_url(&self.client.torii_url, &path);
        self.client
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send_blocking()
    }
    fn decode_name_response(response: &Response<Vec<u8>>) -> Result<NameRecordV1> {
        ensure_status(
            response,
            StatusCode::OK,
            "unexpected SNS registration lookup response",
        )?;
        Ok(norito::json::from_slice(response.body())?)
    }
}
impl Client {
    /// Access the SNS registrar helper.
    pub fn sns(&self) -> SnsApi<'_> {
        SnsApi::new(self)
    }
}
#[cfg(test)]
mod tests {
    //! SNS client helper tests.
    use super::*;
    fn response_with_status(status: StatusCode, body: &[u8]) -> Response<Vec<u8>> {
        Response::builder()
            .status(status)
            .body(body.to_vec())
            .expect("response build")
    }
    #[test]
    fn ensure_status_accepts_expected_status_code() {
        let response = response_with_status(StatusCode::OK, br#"{"ok":true}"#);
        ensure_status(&response, StatusCode::OK, "status check").expect("status must pass");
    }
    #[test]
    fn ensure_status_reports_text_body_when_status_mismatches() {
        let response = response_with_status(StatusCode::BAD_REQUEST, b"invalid JSON body");
        let err = ensure_status(&response, StatusCode::CREATED, "register")
            .expect_err("mismatched status must fail");
        let message = err.to_string();
        assert!(
            message.contains("register"),
            "expected context in error message, got: {message}"
        );
        assert!(
            message.contains("invalid JSON body"),
            "expected response body in error message, got: {message}"
        );
    }
    #[test]
    fn namespace_path_maps_to_fixed_suffix_id() {
        assert_eq!(
            SnsNamespacePath::AccountAlias.suffix_id(),
            ACCOUNT_ALIAS_SUFFIX_ID
        );
        assert_eq!(SnsNamespacePath::Domain.suffix_id(), DOMAIN_NAME_SUFFIX_ID);
        assert_eq!(
            SnsNamespacePath::Dataspace.suffix_id(),
            DATASPACE_ALIAS_SUFFIX_ID
        );
    }
    #[test]
    fn optional_name_http_absence_requires_exact_typed_json() {
        let selector = name_selector(SnsNamespacePath::Dataspace, "dpn").expect("selector");
        let body = norito::json::to_vec(&SnsRegistrationNotFoundV1::new(
            selector.suffix_id,
            selector.label.clone(),
        ))
        .expect("absence");
        let response = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", APPLICATION_JSON)
            .body(body.clone())
            .expect("response");
        assert!(
            SnsApi::decode_optional_name_response(&response, &selector)
                .expect("typed absence")
                .is_none()
        );
        assert!(
            SnsApi::decode_name_response(&response).is_err(),
            "mandatory lookup must still fail"
        );
        for (status, media, bytes) in [
            (StatusCode::NOT_FOUND, "text/plain", body.clone()),
            (
                StatusCode::NOT_FOUND,
                APPLICATION_JSON,
                b"registration `dpn` not found".to_vec(),
            ),
            (StatusCode::BAD_REQUEST, APPLICATION_JSON, body.clone()),
            (StatusCode::UNAUTHORIZED, APPLICATION_JSON, body.clone()),
            (StatusCode::FORBIDDEN, APPLICATION_JSON, body.clone()),
            (StatusCode::INTERNAL_SERVER_ERROR, APPLICATION_JSON, body),
            (
                StatusCode::NOT_FOUND,
                APPLICATION_JSON,
                vec![b' '; SNS_REGISTRATION_NOT_FOUND_MAX_BYTES + 1],
            ),
        ] {
            let response = Response::builder()
                .status(status)
                .header("Content-Type", media)
                .body(bytes)
                .expect("response");
            assert!(
                SnsApi::decode_optional_name_response(&response, &selector).is_err(),
                "{status} {media}"
            );
        }
        for invalid in [
            r#"{"code":"other","suffix_id":4099,"label":"dpn"}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4097,"label":"dpn"}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"other"}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"dpn","extra":0}"#,
            r#"{"code":"sns.registration_not_found","suffix_id":4099,"label":"dpn","label":"dpn"}"#,
        ] {
            let response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", APPLICATION_JSON)
                .body(invalid.as_bytes().to_vec())
                .expect("response");
            assert!(
                SnsApi::decode_optional_name_response(&response, &selector).is_err(),
                "{invalid}"
            );
        }
    }
    #[test]
    fn optional_name_http_success_binds_canonical_namespace_and_record() {
        for (namespace, literal) in [
            (SnsNamespacePath::Dataspace, "dpn"),
            (SnsNamespacePath::Domain, "bank.dpn"),
            (SnsNamespacePath::AccountAlias, "admin@dpn"),
        ] {
            let selector = name_selector(namespace, literal).expect("selector");
            let record = NameRecordV1::new(
                selector.clone(),
                iroha_test_samples::ALICE_ID.clone(),
                Vec::new(),
                0,
                0,
                100,
                200,
                300,
                iroha_model_base::metadata::Metadata::default(),
            );
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", APPLICATION_JSON)
                .body(norito::json::to_vec(&record).expect("record"))
                .expect("response");
            assert_eq!(
                SnsApi::decode_optional_name_response(&response, &selector).expect("record"),
                Some(record)
            );
            let changed = NameSelectorV1 {
                label: "other".to_owned(),
                ..selector
            };
            assert!(SnsApi::decode_optional_name_response(&response, &changed).is_err());
        }
    }
}
