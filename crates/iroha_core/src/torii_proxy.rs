//! Peer-to-peer proxy envelopes for Torii ingress routing.

use iroha_crypto::Hash;
use iroha_data_model::{
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
    transaction::SignedTransaction,
};
use norito::codec::{Decode, Encode};

/// Schema version for peer-to-peer Torii proxy requests.
pub const TORII_PROXY_REQUEST_VERSION_V1: u16 = 1;
/// Schema version for bounded multi-hop peer-to-peer Torii proxy requests.
pub const TORII_PROXY_REQUEST_VERSION_V2: u16 = 2;
/// Schema version for peer-to-peer Torii proxy responses.
pub const TORII_PROXY_RESPONSE_VERSION_V1: u16 = 1;

/// Stable lane/dataspace assignment determined at ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiRouteHintV1 {
    /// Nexus lane selected for the request.
    pub lane_id: LaneId,
    /// Dataspace selected for the request.
    pub dataspace_id: DataSpaceId,
}

impl From<crate::queue::RoutingDecision> for ToriiRouteHintV1 {
    fn from(value: crate::queue::RoutingDecision) -> Self {
        Self {
            lane_id: value.lane_id,
            dataspace_id: value.dataspace_id,
        }
    }
}

impl From<ToriiRouteHintV1> for crate::queue::RoutingDecision {
    fn from(value: ToriiRouteHintV1) -> Self {
        Self::new(value.lane_id, value.dataspace_id)
    }
}

/// Encoded response format requested by the ingress node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyResponseFormatV1 {
    /// Serialize the response body as Norito.
    Norito,
    /// Serialize the response body as JSON.
    Json,
}

/// Supported read endpoints forwarded over the Torii control plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadEndpointV1 {
    /// `GET /v1/accounts/{account_id}/assets`
    AccountAssetsGet,
    /// `POST /v1/accounts/{account_id}/assets/query`
    AccountAssetsQuery,
    /// `GET /v1/accounts/{account_id}/permissions`
    AccountPermissionsGet,
    /// `GET /v1/accounts/{account_id}/transactions`
    AccountTransactionsGet,
    /// `POST /v1/accounts/{account_id}/transactions/query`
    AccountTransactionsQuery,
    /// `GET /v1/accounts`
    AccountsList,
    /// `POST /v1/accounts/query`
    AccountsQuery,
    /// `GET /v1/accounts/{uaid}/portfolio`
    AccountsPortfolio,
    /// `GET /v1/assets/definitions`
    AssetDefinitionsList,
    /// `GET /v1/assets/definitions/{asset}`
    AssetDefinitionGet,
    /// `POST /v1/assets/definitions/query`
    AssetDefinitionsQuery,
    /// `GET /v1/assets/definitions/{asset}/holders`
    AssetHoldersGet,
    /// `POST /v1/assets/definitions/{asset}/holders/query`
    AssetHoldersQuery,
    /// `GET /v1/domains`
    DomainsList,
    /// `POST /v1/domains/query`
    DomainsQuery,
    /// `GET /v1/nfts`
    NftsList,
    /// `POST /v1/nfts/query`
    NftsQuery,
    /// `GET /v1/nexus/public_lanes/{lane_id}/validators`
    NexusPublicLaneValidators,
    /// `GET /v1/nexus/public_lanes/{lane_id}/stake`
    NexusPublicLaneStake,
    /// `GET /v1/nexus/public_lanes/{lane_id}/rewards/pending`
    NexusPublicLaneRewards,
    /// `GET /v1/nexus/dataspaces/accounts/{literal}/summary`
    NexusDataspacesAccountSummary,
    /// `GET /v1/rwas`
    RwasList,
    /// `POST /v1/rwas/query`
    RwasQuery,
}

/// Canonical routed read executed on an authoritative Torii peer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiReadProxyRequestV1 {
    /// Supported read endpoint identifier.
    pub endpoint: ToriiReadEndpointV1,
    /// Stable route resolved by the ingress node.
    pub expected_route: ToriiRouteHintV1,
    /// String path arguments in endpoint-specific order.
    pub path_args: Vec<String>,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// Raw JSON body for POST-style read endpoints.
    pub body: Vec<u8>,
    /// Response encoding negotiated by the ingress node.
    pub response_format: ToriiProxyResponseFormatV1,
}

/// Canonical Torii request body forwarded over the P2P control plane.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyRequestKindV1 {
    /// Submit a signed transaction to the authoritative lane validator.
    SubmitTransaction {
        /// Original signed transaction from the client.
        transaction: SignedTransaction,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
    },
    /// Execute a signed query on the authoritative lane validator.
    SignedQuery {
        /// Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute an ingress-verified query request on the authoritative peer.
    VerifiedQuery {
        /// Norito-encoded verified query payload forwarded by the ingress node.
        request_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a routed Torii read endpoint on the authoritative peer.
    Read(ToriiReadProxyRequestV1),
}

/// P2P Torii proxy request sent from ingress to an authoritative peer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyRequestV2 {
    /// Version of the proxy request envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Current forwarding depth observed by this hop.
    pub hop_count: u8,
    /// Maximum number of hops allowed before the request is rejected.
    pub max_hops: u8,
    /// Peer ids already traversed by the request to prevent proxy loops.
    pub visited_peer_ids: Vec<PeerId>,
    /// Canonical request to execute on the authoritative peer.
    pub request: ToriiProxyRequestKindV1,
}

/// One HTTP header preserved across the Torii proxy response snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyHeaderV1 {
    /// Lower- or mixed-case header name as received from the responder.
    pub name: String,
    /// Raw header value bytes.
    pub value: Vec<u8>,
}

/// Serialized HTTP response sent back to the ingress node.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyHttpResponseV1 {
    /// HTTP status code returned by the authoritative responder.
    pub status_code: u16,
    /// HTTP headers returned by the authoritative responder.
    pub headers: Vec<ToriiProxyHeaderV1>,
    /// Raw response body bytes returned by the authoritative responder.
    pub body: Vec<u8>,
}

/// P2P Torii proxy response sent from the authoritative peer back to ingress.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyResponseV1 {
    /// Version of the proxy response envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Serialized HTTP response from the authoritative peer.
    pub response: ToriiProxyHttpResponseV1,
}
