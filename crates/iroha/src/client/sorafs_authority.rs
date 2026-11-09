//! Authenticated exact reads of the additive SoraFS Parliament proposal host.

use super::*;
use iroha_data_model::governance::types::ProposalContentId;
use iroha_torii_shared::sorafs_authority_api::{
    SorafsAuthorityHeadReadV1, SorafsAuthorityLeaseReadV1, SorafsAuthorityProposalReadV1,
};

impl Client {
    /// Read one exact SoraFS authority proposal from the pinned network.
    ///
    /// This authenticated observation does not replace a certified execution proof.
    /// # Errors
    /// Transport, HTTP, JSON, network or exact proposal identity verification failed.
    pub fn get_sorafs_authority_proposal(
        &self,
        id: ProposalContentId,
    ) -> Result<SorafsAuthorityProposalReadV1> {
        let path = torii_routes::runtime_governance::GOV_SORAFS_AUTHORITY_PROPOSAL
            .path()
            .replace("{id}", &id.to_hex());
        let response = self.send_builder(
            self.account_signed_get_request(join_torii_url(&self.torii_url, &path))?
                .header("Accept", APPLICATION_JSON)
                .max_response_bytes(PARLIAMENT_JSON_RESPONSE_MAX_BYTES),
        )?;
        Self::ensure_response_status(
            &response,
            StatusCode::OK,
            "Failed to read SoraFS authority proposal",
            " ",
        )?;
        let result: SorafsAuthorityProposalReadV1 = norito::json::from_slice(response.body())?;
        if !result.matches(&self.network_id, id) {
            return Err(eyre!("SoraFS authority proposal response binding mismatch"));
        }
        Ok(result)
    }

    /// Read one exact authority subject and its last successful receipt.
    ///
    /// Revoked heads remain present, preserving their monotonic revision.
    /// # Errors
    /// Transport, HTTP, JSON, network or exact subject identity verification failed.
    pub fn get_sorafs_authority_head(
        &self,
        subject: [u8; 32],
    ) -> Result<SorafsAuthorityHeadReadV1> {
        let path = torii_routes::runtime_governance::GOV_SORAFS_AUTHORITY_HEAD
            .path()
            .replace("{id}", &hex::encode(subject));
        let response = self.send_builder(
            self.account_signed_get_request(join_torii_url(&self.torii_url, &path))?
                .header("Accept", APPLICATION_JSON)
                .max_response_bytes(PARLIAMENT_JSON_RESPONSE_MAX_BYTES),
        )?;
        Self::ensure_response_status(
            &response,
            StatusCode::OK,
            "Failed to read SoraFS authority head",
            " ",
        )?;
        let result: SorafsAuthorityHeadReadV1 = norito::json::from_slice(response.body())?;
        if !result.matches(&self.network_id, &subject) {
            return Err(eyre!("SoraFS authority head response binding mismatch"));
        }
        Ok(result)
    }

    /// Read one exact lease plan, its separate authorizations and actual effects.
    ///
    /// This authenticated observation does not replace certified execution proofs.
    /// # Errors
    /// Transport, HTTP, JSON, network or exact plan identity verification failed.
    pub fn get_sorafs_authority_lease(&self, plan: [u8; 32]) -> Result<SorafsAuthorityLeaseReadV1> {
        let path = torii_routes::runtime_governance::GOV_SORAFS_AUTHORITY_LEASE
            .path()
            .replace("{id}", &hex::encode(plan));
        let response = self.send_builder(
            self.account_signed_get_request(join_torii_url(&self.torii_url, &path))?
                .header("Accept", APPLICATION_JSON)
                .max_response_bytes(PARLIAMENT_JSON_RESPONSE_MAX_BYTES),
        )?;
        Self::ensure_response_status(
            &response,
            StatusCode::OK,
            "Failed to read SoraFS authority lease",
            " ",
        )?;
        let result: SorafsAuthorityLeaseReadV1 = norito::json::from_slice(response.body())?;
        if !result.matches(&self.network_id, &plan) {
            return Err(eyre!("SoraFS authority lease response binding mismatch"));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::evidence_http_tests::{
        SnapshotStore, base_url, client_with_base_url, respond_with, with_mock_http,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn exact_authority_reads_bind_request_network_and_subject() {
        let client = client_with_base_url(base_url());
        let id = ProposalContentId::new([1; 32]);
        for substitute in [false, true] {
            let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
            let response = SorafsAuthorityProposalReadV1 {
                network: client.network_id,
                proposal_content_id: if substitute {
                    ProposalContentId::new([2; 32])
                } else {
                    id
                },
                current_height: "50".into(),
                record: None,
            };
            with_mock_http(
                respond_with(
                    &snapshots,
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", APPLICATION_JSON)
                        .body(norito::json::to_vec(&response).unwrap())
                        .unwrap(),
                ),
                |transport| {
                    let client = client.clone().with_test_http_transport(transport.clone());
                    assert_eq!(
                        client.get_sorafs_authority_proposal(id).is_err(),
                        substitute
                    );
                },
            );
            let response = SorafsAuthorityHeadReadV1 {
                network: client.network_id,
                subject_id: hex::encode(if substitute { [2; 32] } else { [1; 32] }),
                current_height: "50".into(),
                head: None,
            };
            with_mock_http(
                respond_with(
                    &snapshots,
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", APPLICATION_JSON)
                        .body(norito::json::to_vec(&response).unwrap())
                        .unwrap(),
                ),
                |transport| {
                    let client = client.clone().with_test_http_transport(transport.clone());
                    assert_eq!(
                        client.get_sorafs_authority_head([1; 32]).is_err(),
                        substitute
                    );
                },
            );
            let response = SorafsAuthorityLeaseReadV1 {
                network: client.network_id,
                plan_id: hex::encode(if substitute { [2; 32] } else { [1; 32] }),
                current_height: "50".into(),
                lease: None,
            };
            with_mock_http(
                respond_with(
                    &snapshots,
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", APPLICATION_JSON)
                        .body(norito::json::to_vec(&response).unwrap())
                        .unwrap(),
                ),
                |transport| {
                    let client = client.clone().with_test_http_transport(transport.clone());
                    assert_eq!(
                        client.get_sorafs_authority_lease([1; 32]).is_err(),
                        substitute
                    );
                },
            );
        }
    }
}
