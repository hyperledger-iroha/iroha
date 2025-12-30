//! Rust client helpers for the Sora Name Service registrar routes.

use eyre::Result;
use norito::json::Value;
use url::Url;

use crate::{
    client::{Client, join_torii_url},
    data_model::sns::{
        FreezeNameRequestV1, GovernanceHookV1, NameRecordV1, RegisterNameRequestV1,
        RegisterNameResponseV1, RenewNameRequestV1, SuffixPolicyV1, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
    http::{Method as HttpMethod, RequestBuilder},
};

const APPLICATION_JSON: &str = "application/json";

/// Typed helper exposed by [`Client::sns()`].
pub struct SnsApi<'a> {
    client: &'a Client,
}

impl<'a> SnsApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// POST `/v1/sns/registrations` to register a name.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be built, sent, or decoded.
    pub fn register(&self, payload: &RegisterNameRequestV1) -> Result<RegisterNameResponseV1> {
        let url = join_torii_url(&self.client.torii_url, "v1/sns/registrations");
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
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
        Ok(norito::json::from_slice(response.body())?)
    }

    /// GET `/v1/sns/registrations/{selector}`.
    ///
    /// # Errors
    ///
    /// Returns an error if the registration lookup or decoding fails.
    pub fn get_registration(&self, selector: &str) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}");
        let url = join_torii_url(&self.client.torii_url, &path);
        let response = self
            .client
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// POST `/v1/sns/registrations/{selector}/renew`.
    ///
    /// # Errors
    ///
    /// Returns an error if the renewal request or response decoding fails.
    pub fn renew(&self, selector: &str, payload: &RenewNameRequestV1) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}/renew");
        let url = join_torii_url(&self.client.torii_url, &path);
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// POST `/v1/sns/registrations/{selector}/transfer`.
    ///
    /// # Errors
    ///
    /// Returns an error if the transfer request or response decoding fails.
    pub fn transfer(
        &self,
        selector: &str,
        payload: &TransferNameRequestV1,
    ) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}/transfer");
        let url = join_torii_url(&self.client.torii_url, &path);
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// POST `/v1/sns/registrations/{selector}/controllers`.
    ///
    /// # Errors
    ///
    /// Returns an error if the update request or response decoding fails.
    pub fn update_controllers(
        &self,
        selector: &str,
        payload: &UpdateControllersRequestV1,
    ) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}/controllers");
        let url = join_torii_url(&self.client.torii_url, &path);
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// POST `/v1/sns/registrations/{selector}/freeze`.
    ///
    /// # Errors
    ///
    /// Returns an error if the freeze request or response decoding fails.
    pub fn freeze(&self, selector: &str, payload: &FreezeNameRequestV1) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}/freeze");
        let url = join_torii_url(&self.client.torii_url, &path);
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// DELETE `/v1/sns/registrations/{selector}/freeze`.
    ///
    /// # Errors
    ///
    /// Returns an error if the unfreeze request or response decoding fails.
    pub fn unfreeze(&self, selector: &str, payload: &GovernanceHookV1) -> Result<NameRecordV1> {
        let path = format!("v1/sns/registrations/{selector}/freeze");
        let url = join_torii_url(&self.client.torii_url, &path);
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::DELETE, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// POST `/v1/sns/governance/cases` to create an arbitration record.
    ///
    /// Returns the created case envelope as raw JSON so CLI tools can attach it to
    /// transparency artefacts without depending on generated structs.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be built or sent, or if the JSON
    /// response fails to decode.
    pub fn create_case(&self, payload: &Value) -> Result<Value> {
        let url = join_torii_url(&self.client.torii_url, "v1/sns/governance/cases");
        let body = norito::json::to_vec(payload)?;
        let response = self
            .client
            .default_request(HttpMethod::POST, url)
            .header("Content-Type", APPLICATION_JSON)
            .header("Accept", APPLICATION_JSON)
            .body(body)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }

    /// GET `/v1/sns/governance/cases` with optional filters to export disputes.
    ///
    /// # Errors
    ///
    /// Returns an error if the export request cannot be built or sent, or if the
    /// response payload fails JSON decoding.
    pub fn export_cases(&self, query: CaseExportQuery<'_>) -> Result<Value> {
        let mut url = join_torii_url(&self.client.torii_url, "v1/sns/governance/cases");
        query.apply(&mut url);
        let response = self
            .client
            .default_request(HttpMethod::GET, url)
            .header("Accept", APPLICATION_JSON)
            .build()?
            .send()?;
        Ok(norito::json::from_slice(response.body())?)
    }
}

impl Client {
    /// Access the SNS registrar helper.
    pub fn sns(&self) -> SnsApi<'_> {
        SnsApi::new(self)
    }
}

/// Query parameters accepted by [`SnsApi::export_cases`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CaseExportQuery<'a> {
    /// ISO-8601 timestamp; filters cases updated after the instant provided.
    pub since: Option<&'a str>,
    /// Optional status filter (`open`, `decision`, `closed`, etc.).
    pub status: Option<&'a str>,
    /// Optional max number of cases to return.
    pub limit: Option<u32>,
}

impl CaseExportQuery<'_> {
    fn apply(self, url: &mut Url) {
        {
            let mut qp = url.query_pairs_mut();
            if let Some(since) = self.since {
                qp.append_pair("since", since);
            }
            if let Some(status) = self.status {
                qp.append_pair("status", status);
            }
            if let Some(limit) = self.limit {
                qp.append_pair("limit", &limit.to_string());
            }
        }
    }
}
