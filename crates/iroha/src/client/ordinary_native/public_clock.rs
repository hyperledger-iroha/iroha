//! Credential-free actual clock reads; all finality/root/nonce admission stays in the owner.
use super::*;
use crate::http::{Method, TransportRequest};
use crate::http_default::DefaultHttpTransport;
use std::{num::NonZeroU64, time::Instant};

pub(super) enum ClockNodes {
    AccountContext(Box<[Client; 4]>),
    Public(Box<[PublicClockNode; 4]>),
}
pub(super) struct PublicClockNode {
    origin: Url,
    network: NetworkId,
    transport: DefaultHttpTransport,
}
#[derive(Clone, Copy)]
pub(super) enum ClockNode<'a> {
    Account(&'a Client),
    Public(&'a PublicClockNode),
}
pub(super) struct ClockRead<'a> {
    node: ClockNode<'a>,
    deadline: Instant,
}
impl ClockNodes {
    pub(super) fn public(origins: [String; 4], network: NetworkId) -> Result<Self> {
        let mut nodes = Vec::with_capacity(4);
        for origin in origins {
            let url = super::endpoint::require_https_directory_base(&origin)?;
            nodes.push(PublicClockNode {
                origin: url,
                network,
                transport: DefaultHttpTransport::new()?,
            });
        }
        Ok(Self::Public(Box::new(
            nodes
                .try_into()
                .map_err(|_| eyre!("public node count differs"))?,
        )))
    }
    pub(super) fn at(&self, index: usize) -> ClockNode<'_> {
        match self {
            Self::AccountContext(nodes) => ClockNode::Account(&nodes[index]),
            Self::Public(nodes) => ClockNode::Public(&nodes[index]),
        }
    }
}
impl<'a> ClockNode<'a> {
    pub(super) fn with_request_deadline(self, deadline: Instant) -> ClockRead<'a> {
        ClockRead {
            node: self,
            deadline,
        }
    }
}
impl ClockRead<'_> {
    pub(super) fn get_sumeragi_finality_attestation(
        &self,
        height: NonZeroU64,
        nonce: [u8; 32],
        peer: &iroha_model_base::peer::PeerId,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation> {
        match &self.node {
            ClockNode::Account(client) => client
                .with_request_deadline(self.deadline)
                .get_sumeragi_finality_attestation(height, nonce, peer),
            ClockNode::Public(node) => {
                ensure!(nonce != [0; 32], "clock nonce rejected");
                let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
                    .path()
                    .replace("{height}", &height.get().to_string());
                let response = node.read(&path, Some(nonce), self.deadline)?;
                if response.status() != StatusCode::OK {
                    let failure = Client::decode_finality_attestation_failure(
                        &response,
                        height.get(),
                        nonce,
                        peer,
                        node.network,
                    )?;
                    if let Some(progress) = failure.tip_mismatch {
                        return Err(BridgeFinalityAttestationTipMismatch::from_response(
                            progress,
                            height,
                            nonce,
                            peer,
                            node.network,
                        )?
                        .into());
                    }
                    return Err(eyre!("public clock attestation unavailable"));
                }
                let original: iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation =
                    Client::decode_canonical_norito_response(
                        &response,
                        SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                        "ordinary_native.public_clock.finality_attestation.read",
                    )?;
                original.verify()?;
                ensure!(
                    original.body.challenge == nonce
                        && original.body.node_id == *peer
                        && original.body.network_id == node.network
                        && original.body.status.committed_height == height.get(),
                    "public clock request binding differs"
                );
                ensure!(
                    Instant::now() < self.deadline,
                    "public clock deadline expired"
                );
                Ok(original)
            }
        }
    }
    pub(super) fn get_sumeragi_finality_proof(
        &self,
        height: NonZeroU64,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        match &self.node {
            ClockNode::Account(client) => client
                .with_request_deadline(self.deadline)
                .get_sumeragi_finality_proof(height),
            ClockNode::Public(node) => {
                let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
                    .path()
                    .replace("{height}", &height.get().to_string());
                let response = node.read(&path, None, self.deadline)?;
                let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
                    Client::decode_canonical_norito_response(
                        &response,
                        SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                        "ordinary_native.public_clock.finality_proof.read",
                    )?;
                ensure!(
                    proof.height() == height.get(),
                    "public clock proof height differs"
                );
                proof.decode_checked()?;
                ensure!(
                    Instant::now() < self.deadline,
                    "public clock deadline expired"
                );
                Ok(proof)
            }
        }
    }
}
impl PublicClockNode {
    fn read(
        &self,
        path: &str,
        nonce: Option<[u8; 32]>,
        deadline: Instant,
    ) -> Result<Response<Vec<u8>>> {
        crate::blocking::reject_inside_async_runtime()?;
        ensure!(Instant::now() < deadline, "public clock deadline expired");
        let mut headers = vec![(
            http::header::ACCEPT,
            http::header::HeaderValue::from_static(APPLICATION_NORITO),
        )];
        if let Some(nonce) = nonce {
            headers.push((
                http::header::HeaderName::from_static("x-iroha-finality-challenge"),
                http::header::HeaderValue::from_str(&hex::encode(nonce))?,
            ));
        }
        let response = self
            .transport
            .with_deadline(deadline)
            .send_blocking(TransportRequest {
                method: Method::GET,
                url: self.origin.join(path.trim_start_matches('/'))?,
                headers,
                body: Vec::new(),
                timeout: Some(deadline.saturating_duration_since(Instant::now())),
                max_response_bytes: SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                direct_loopback: false,
            })?;
        ensure!(Instant::now() < deadline, "public clock deadline expired");
        Ok(response)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_clock_has_no_wallet_or_credential_constructor() {
        for origin in [
            "http://node.example",
            "https://user@node.example",
            "https://node.example/path",
            "https://node.example?x=1",
        ] {
            let origins = std::array::from_fn(|_| origin.into());
            let network = NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"synthetic no authority")),
            );
            assert!(ClockNodes::public(origins, network).is_err());
        }
    }
}
