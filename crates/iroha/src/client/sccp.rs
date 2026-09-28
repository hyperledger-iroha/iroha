//! Public asynchronous SCCP v1 reads on one immutable client context (`specs/sccp.md` §6).
//!
//! Every response is data from one Taira peer; nothing here is trusted. Wallets verify proof
//! bundles and rotation chains locally (`iroha_sccp_wallet::pure`) before using them.

use super::{Client, dispatch, join_torii_url};
use crate::{Error, Result, http::Method};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    bridge::SccpNetworkV1,
    sccp::{outbound::SccpOutboundMessageRecordV1, registry::SccpRouteV1},
};
use iroha_sccp::api::{
    SccpCapabilitiesV1, SccpControlProofBundleV1, SccpMessageProofBundleV1, SccpRosterViewV1,
    SccpRotationChainV1,
};
use iroha_torii_shared::route_catalog::{RouteDescriptor, sccp};
use norito::json::JsonDeserialize;

const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Which attestation a proof bundle uses (`?attestation=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpAttestation {
    /// The subject of the block that holds the leaf (direct mode).
    Own,
    /// The newest attested subject at or above that block (historical mode when later).
    Latest,
    /// The subject at this height.
    At(u64),
}

impl SccpAttestation {
    fn query(self) -> String {
        match self {
            Self::Own => "own".to_owned(),
            Self::Latest => "latest".to_owned(),
            Self::At(height) => height.to_string(),
        }
    }
}

/// Public SCCP read routes.
#[derive(Clone, Copy, Debug)]
pub struct Sccp<'a> {
    client: &'a Client,
}

impl Client {
    /// Access the public SCCP read API through the context's asynchronous transport.
    #[must_use]
    pub const fn sccp(&self) -> Sccp<'_> {
        Sccp { client: self }
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    hex::encode(bytes)
}

/// Fill the `{name}` placeholders of `route`'s path in order.
fn path(route: RouteDescriptor, values: &[&str]) -> String {
    let mut path = route.path().to_owned();
    for value in values {
        if let (Some(start), Some(end)) = (path.find('{'), path.find('}')) {
            path.replace_range(start..=end, value);
        }
    }
    path
}

impl Sccp<'_> {
    async fn get<R: JsonDeserialize>(
        &self,
        route: RouteDescriptor,
        path_and_query: &str,
    ) -> Result<R> {
        let operation = route.stable_route_id();
        let response = dispatch::send(
            self.client,
            operation,
            self.client
                .default_request(
                    Method::GET,
                    join_torii_url(&self.client.torii_url, path_and_query),
                )
                .max_response_bytes(MAX_RESPONSE_BYTES),
            "application/json",
        )
        .await?;
        if response.status() != crate::http::StatusCode::OK {
            return Err(Error::Http {
                operation,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        let _format = ChainDiscriminantGuard::enter(self.client.account_chain_discriminant);
        norito::json::from_slice(response.body()).map_err(|error| Error::Decode {
            operation,
            details: error.to_string(),
        })
    }

    /// Read the Taira identity, parameters and attestation health.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn capabilities(&self) -> Result<SccpCapabilitiesV1> {
        self.get(sccp::CAPABILITIES, sccp::CAPABILITIES.path())
            .await
    }

    /// Read the nonzero per-subject governance revisions (absent subjects are at 0).
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn governance_revisions(
        &self,
    ) -> Result<Vec<iroha_data_model::sccp::governance::SccpGovernanceBaseRevisionV1>> {
        self.get(sccp::GOVERNANCE, sccp::GOVERNANCE.path()).await
    }

    /// Read every open SCCP governance proposal, oldest first.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn governance_proposals(
        &self,
    ) -> Result<Vec<iroha_sccp::api::SccpGovernanceProposalStatusV1>> {
        self.get(
            sccp::GOVERNANCE_PROPOSALS,
            sccp::GOVERNANCE_PROPOSALS.path(),
        )
        .await
    }

    /// Read every installed inbound light client.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn light_clients(
        &self,
    ) -> Result<Vec<iroha_data_model::sccp::light_client::SccpLightClientV1>> {
        self.get(sccp::LIGHT_CLIENTS, sccp::LIGHT_CLIENTS.path())
            .await
    }

    /// Read the stored consensus sets of `network`'s light client.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn light_client_sets(
        &self,
        network: SccpNetworkV1,
    ) -> Result<Vec<iroha_data_model::sccp::light_client::SccpLcConsensusSetV1>> {
        self.get(
            sccp::LIGHT_CLIENT_SETS,
            &path(sccp::LIGHT_CLIENT_SETS, &[network.profile_key()]),
        )
        .await
    }

    /// Read every route with its escrow, stranded balance and revisions.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn registry(&self) -> Result<Vec<SccpRouteV1>> {
        self.get(sccp::REGISTRY, sccp::REGISTRY.path()).await
    }

    /// Read one outbound message record.
    ///
    /// # Errors
    /// Returns structured transport, HTTP (404 when unknown) or decoding errors.
    pub async fn message(&self, message_id: &[u8; 32]) -> Result<SccpOutboundMessageRecordV1> {
        self.get(sccp::MESSAGE, &path(sccp::MESSAGE, &[&hex32(message_id)]))
            .await
    }

    /// Read the proof bundle of one outbound message.
    ///
    /// # Errors
    /// Returns structured transport, HTTP (409 until attested) or decoding errors.
    pub async fn message_proof(
        &self,
        message_id: &[u8; 32],
        attestation: SccpAttestation,
    ) -> Result<SccpMessageProofBundleV1> {
        let path = format!(
            "{}?attestation={}",
            path(sccp::MESSAGE_PROOF, &[&hex32(message_id)]),
            attestation.query()
        );
        self.get(sccp::MESSAGE_PROOF, &path).await
    }

    /// Read the proof bundle of one destination control.
    ///
    /// # Errors
    /// Returns structured transport, HTTP (409 until attested) or decoding errors.
    pub async fn control_proof(
        &self,
        network: SccpNetworkV1,
        revision: u32,
        control_nonce: u64,
        attestation: SccpAttestation,
    ) -> Result<SccpControlProofBundleV1> {
        let path = format!(
            "{}?attestation={}",
            path(
                sccp::CONTROL_PROOF,
                &[
                    network.profile_key(),
                    &revision.to_string(),
                    &control_nonce.to_string()
                ]
            ),
            attestation.query()
        );
        self.get(sccp::CONTROL_PROOF, &path).await
    }

    /// Read the current roster generation.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn current_roster(&self) -> Result<SccpRosterViewV1> {
        self.get(sccp::ROSTER_CURRENT, sccp::ROSTER_CURRENT.path())
            .await
    }

    /// Read one roster generation.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn roster(&self, generation: u64) -> Result<SccpRosterViewV1> {
        self.get(
            sccp::ROSTER,
            &path(sccp::ROSTER, &[&generation.to_string()]),
        )
        .await
    }

    /// Read the catch-up rotation chain from `after_generation`, at most `limit` (≤ 16) steps.
    ///
    /// # Errors
    /// Returns structured transport, HTTP or decoding errors.
    pub async fn rotations(
        &self,
        after_generation: u64,
        limit: usize,
    ) -> Result<SccpRotationChainV1> {
        let path = format!(
            "{}?after_generation={after_generation}&limit={limit}",
            sccp::ROSTER_ROTATIONS.path()
        );
        self.get(sccp::ROSTER_ROTATIONS, &path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_fill_placeholders_in_order() {
        assert_eq!(
            path(sccp::MESSAGE_PROOF, &["ab"]),
            "/v1/sccp/messages/ab/proof"
        );
        assert_eq!(
            path(sccp::CONTROL_PROOF, &["ton-mainnet", "2", "7"]),
            "/v1/sccp/controls/ton-mainnet/2/7/proof"
        );
        assert_eq!(path(sccp::ROSTER, &["4"]), "/v1/sccp/rosters/4");
    }

    #[test]
    fn attestation_queries_are_canonical() {
        assert_eq!(SccpAttestation::Own.query(), "own");
        assert_eq!(SccpAttestation::Latest.query(), "latest");
        assert_eq!(SccpAttestation::At(12).query(), "12");
    }
}
