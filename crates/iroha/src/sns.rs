//! Rust client helpers for the Sora Name Service registrar routes.

use std::{
    thread,
    time::{Duration, Instant},
};

use eyre::Result;

use crate::{
    client::{Client, ResponseReport, join_torii_url},
    data_model::sns::{
        ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID,
        FreezeNameRequestV1, GovernanceHookV1, NameRecordV1, RegisterNameRequestV1,
        RegisterNameResponseV1, RenewNameRequestV1, SuffixId, SuffixPolicyV1,
        TransferNameRequestV1, UpdateControllersRequestV1,
    },
    http::{Method as HttpMethod, RequestBuilder, Response, StatusCode},
};

const APPLICATION_JSON: &str = "application/json";
const COMMITTED_NAME_READ_TIMEOUT: Duration = Duration::from_secs(30);
const COMMITTED_NAME_READ_INTERVAL: Duration = Duration::from_millis(250);

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

fn retryable_name_lookup_status(status: StatusCode) -> bool {
    status == StatusCode::NOT_FOUND
}

impl<'a> SnsApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Submit a consensus transaction to register a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn register(&self, payload: &RegisterNameRequestV1) -> Result<RegisterNameResponseV1> {
        let namespace = SnsNamespacePath::from_suffix_id(payload.selector.suffix_id)?;
        let literal = payload.selector.normalized_label().to_owned();
        self.client
            .submit_blocking(crate::data_model::isi::sns::RegisterSnsName::new(
                payload.clone(),
            ))?;
        let name_record = self.get_committed_name(namespace, &literal)?;
        Ok(RegisterNameResponseV1 { name_record })
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
            .send()?;
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
            .send()
    }

    fn decode_name_response(response: &Response<Vec<u8>>) -> Result<NameRecordV1> {
        ensure_status(
            response,
            StatusCode::OK,
            "unexpected SNS registration lookup response",
        )?;
        Ok(norito::json::from_slice(response.body())?)
    }

    fn get_committed_name(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
    ) -> Result<NameRecordV1> {
        let deadline = Instant::now() + COMMITTED_NAME_READ_TIMEOUT;
        let mut last_response = self.get_name_response(namespace, literal)?;

        while retryable_name_lookup_status(last_response.status()) && Instant::now() < deadline {
            thread::sleep(COMMITTED_NAME_READ_INTERVAL);
            last_response = self.get_name_response(namespace, literal)?;
        }

        Self::decode_name_response(&last_response)
    }

    /// Submit a consensus transaction to renew a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn renew(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
        payload: &RenewNameRequestV1,
    ) -> Result<NameRecordV1> {
        self.client
            .submit_blocking(crate::data_model::isi::sns::RenewSnsName::new(
                namespace.suffix_id(),
                literal,
                payload.clone(),
            ))?;
        self.get_committed_name(namespace, literal)
    }

    /// Submit a consensus transaction to transfer a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn transfer(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
        payload: &TransferNameRequestV1,
    ) -> Result<NameRecordV1> {
        self.client
            .submit_blocking(crate::data_model::isi::sns::TransferSnsName::new(
                namespace.suffix_id(),
                literal,
                payload.clone(),
            ))?;
        self.get_committed_name(namespace, literal)
    }

    /// Submit a consensus transaction to replace name controllers.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn update_controllers(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
        payload: &UpdateControllersRequestV1,
    ) -> Result<NameRecordV1> {
        self.client
            .submit_blocking(crate::data_model::isi::sns::UpdateSnsNameControllers::new(
                namespace.suffix_id(),
                literal,
                payload.clone(),
            ))?;
        self.get_committed_name(namespace, literal)
    }

    /// Submit a consensus transaction to freeze a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn freeze(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
        payload: &FreezeNameRequestV1,
    ) -> Result<NameRecordV1> {
        self.client
            .submit_blocking(crate::data_model::isi::sns::FreezeSnsName::new(
                namespace.suffix_id(),
                literal,
                payload.clone(),
            ))?;
        self.get_committed_name(namespace, literal)
    }

    /// Submit a consensus transaction to unfreeze a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction is rejected or the committed record cannot be fetched.
    pub fn unfreeze(
        &self,
        namespace: SnsNamespacePath,
        literal: &str,
        payload: &GovernanceHookV1,
    ) -> Result<NameRecordV1> {
        self.client
            .submit_blocking(crate::data_model::isi::sns::UnfreezeSnsName::new(
                namespace.suffix_id(),
                literal,
                payload.clone(),
            ))?;
        self.get_committed_name(namespace, literal)
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
    fn name_lookup_retry_is_limited_to_not_found() {
        assert!(retryable_name_lookup_status(StatusCode::NOT_FOUND));
        assert!(!retryable_name_lookup_status(StatusCode::BAD_REQUEST));
        assert!(!retryable_name_lookup_status(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
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
}
