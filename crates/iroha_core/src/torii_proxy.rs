//! Peer-to-peer proxy envelopes for Torii ingress routing.
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{NetworkId, transaction::TransactionEntrypoint};
use iroha_model_base::peer::PeerId;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use norito::codec::{Decode, Encode};
use std::fmt;
/// Schema version for deadline-bound Torii proxy requests.
pub const TORII_PROXY_REQUEST_VERSION_V1: u16 = 1;
/// Maximum inner body admitted by a first-release Torii proxy request.
pub const TORII_PROXY_REQUEST_MAX_INNER_BODY_BYTES_V1: usize = 64_000_000;
/// Source-coupled allowance for the signed proxy request envelope.
pub const TORII_PROXY_REQUEST_FRAME_OVERHEAD_BYTES_V1: usize = 8 * 1024 * 1024;
/// Maximum encoded `ToriiProxyRequestV1`/HTTP body before relay framing.
pub const TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1: usize =
    TORII_PROXY_REQUEST_MAX_INNER_BODY_BYTES_V1 + TORII_PROXY_REQUEST_FRAME_OVERHEAD_BYTES_V1;
/// Maximum enum/length framing around one proxy request or response inside `NetworkMessage`.
pub const TORII_PROXY_NETWORK_MESSAGE_OVERHEAD_BYTES_V1: usize = 64 * 1024;
/// Maximum P2P relay framing above the bounded `NetworkMessage` carrier.
pub const TORII_PROXY_REQUEST_RELAY_OVERHEAD_BYTES_V1: usize = 1024 * 1024;
/// Maximum complete encoded first-release Torii proxy frame admitted from P2P.
pub const TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1: usize =
    TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1 + TORII_PROXY_REQUEST_RELAY_OVERHEAD_BYTES_V1;
/// Maximum cumulative allocation while decoding one first-release proxy request.
///
/// The submitted transaction is reconstructed through nested owned proxy,
/// entrypoint, executable, and instruction containers. Keep that graph under
/// an explicit eightfold cap while the independent 64 MB body and 73 MB frame
/// limits continue to bound attacker-controlled source bytes.
pub const TORII_PROXY_REQUEST_MAX_DECODE_ALLOCATED_BYTES_V1: usize =
    TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1.saturating_mul(8);
/// Maximum encoded proxy-response body plus its bounded HTTP header envelope.
pub const TORII_PROXY_RESPONSE_MAX_ENCODED_BYTES_V1: usize =
    TORII_PROXY_REQUEST_MAX_INNER_BODY_BYTES_V1 + TORII_PROXY_REQUEST_FRAME_OVERHEAD_BYTES_V1;
/// Maximum complete first-release proxy-response frame admitted from P2P.
pub const TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1: usize =
    TORII_PROXY_RESPONSE_MAX_ENCODED_BYTES_V1 + TORII_PROXY_REQUEST_RELAY_OVERHEAD_BYTES_V1;
/// Maximum cumulative allocation while decoding one first-release proxy response.
pub const TORII_PROXY_RESPONSE_MAX_DECODE_ALLOCATED_BYTES_V1: usize =
    TORII_PROXY_RESPONSE_MAX_ENCODED_BYTES_V1;
/// Schema version for peer-to-peer Torii proxy responses.
pub const TORII_PROXY_RESPONSE_VERSION_V1: u16 = 1;








#[cfg(test)]
thread_local! {
    static QUEUE_PLAN_AUTHENTICATION_OBSERVER:
        std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
}


/// Stable lane/dataspace assignment determined at ingress.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiRouteHintV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyResponseFormatV1")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyResponseFormatV1 {
    /// Serialize the response body as Norito.
    Norito,
    /// Serialize the response body as JSON.
    Json,
}
/// Supported read endpoints forwarded over the Torii control plane.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiReadEndpointV1")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadEndpointV1 {
    /// `GET /v1/accounts/{account_id}`
    AccountGet,
    /// `GET /v1/explorer/accounts/{account_id}`
    ExplorerAccountDetail,
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
    /// `POST /v1/transactions/query`
    TransactionsQuery,
    /// `GET /v1/pipeline/transactions/status`
    PipelineTransactionStatusGet,
    /// `GET /v1/proofs/{id}`
    ProofRecordGet,
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
    /// `GET /v1/nexus/public-lanes/{lane_id}/validators`
    NexusPublicLaneValidators,
    /// `GET /v1/nexus/public-lanes/{lane_id}/stake`
    NexusPublicLaneStake,
    /// `GET /v1/nexus/public-lanes/{lane_id}/rewards/pending`
    NexusPublicLaneRewards,
    /// `GET /v1/nexus/dataspaces/accounts/{literal}/summary`
    NexusDataspacesAccountSummary,
    /// `GET /v1/space-directory/uaids/{uaid}`
    SpaceDirectoryBindingsGet,
    /// `GET /v1/space-directory/uaids/{uaid}/manifests`
    SpaceDirectoryManifestsGet,
    /// `GET /v1/rwas`
    RwasList,
    /// `POST /v1/rwas/query`
    RwasQuery,
    /// `POST /v1/aliases/resolve`
    AliasResolve,
    /// `POST /v1/aliases/resolve-index`
    AliasResolveIndex,
    /// `POST /v1/aliases/by-account`
    AliasLookupByAccount,
    /// `GET /v1/explorer/asset-definitions/{id}`
    ExplorerAssetDefinitionDetail,
    /// `GET /v1/explorer/asset-definitions/{id}/econometrics`
    ExplorerAssetDefinitionEconometrics,
    /// `GET /v1/explorer/asset-definitions/{id}/snapshot`
    ExplorerAssetDefinitionSnapshot,
    /// `POST /v1/contracts/aliases/resolve`
    ContractAliasResolve,
    /// `GET /v1/contracts/state`
    ContractStateGet,
    /// `POST /v1/contracts/view`
    ContractViewPost,
    /// `POST /v1/contracts/view/batch`
    ContractViewBatchPost,
    /// `GET /v1/accounts/{account_id}/history`
    AccountHistoryGet,
    /// `GET /v1/internal/accounts/{account_id}`
    InternalAccountGet,
    /// `GET /v1/internal/accounts/{account_id}/transactions/{entrypoint_hash}`
    InternalAccountTransactionGet,
    /// `GET /v1/internal/accounts/{account_id}/assets/{asset_definition_id}?scope=...`
    InternalAccountAssetGet,
    /// `POST /v1/contracts/deployment-state`
    ContractDeploymentState,
    /// `POST /v1/accounts/onboarding/current-state`
    AccountOnboardingCurrentState,
}
/// Canonical routed read executed on an authoritative Torii peer.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiReadProxyRequestV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiReadProxyRequestV1 {
    /// Supported read endpoint identifier.
    pub endpoint: ToriiReadEndpointV1,
    /// Complete caller-visible route scope authorized by the ingress node.
    pub route_scope: ToriiFanoutRouteScopeV1,
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
/// Route set Nexus should recompute for a coordinated fanout request.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiFanoutRouteScopeV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiFanoutRouteScopeV1 {
    /// Fan out across all configured dataspace routes.
    AllDataspaces,
    /// Fan out across the dataspaces that may own the target account.
    TargetAccount {
        /// Canonical target account id literal.
        account_id: String,
        /// Canonical signed caller whose full visible scope authorizes row disclosure.
        caller_account_id: Option<String>,
    },
    /// Fan out across public routes plus caller-visible private dataspaces.
    VisibleAccount {
        /// Optional canonical caller account id literal.
        caller_account_id: Option<String>,
    },
}
/// Merge behavior requested for an App API read fanout.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiReadFanoutMergeV1")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadFanoutMergeV1 {
    /// Merge JSON list-style responses.
    List,
    /// Merge JSON singleton responses.
    Singleton,
    /// Merge account-detail responses while preserving the requested response format.
    Account,
    /// Merge account-history responses with global ordering and pagination.
    AccountHistory,
    /// Merge account portfolio responses.
    Portfolio,
    /// Merge dataspace account summary responses.
    DataspaceSummary,
    /// Merge space-directory bindings responses.
    SpaceDirectoryBindings,
    /// Merge space-directory manifest responses.
    SpaceDirectoryManifests {
        /// Client pagination offset to apply after merged deduplication.
        page_offset: u64,
        /// Client pagination limit to apply after merged deduplication.
        page_limit: Option<u64>,
    },
}
/// App API read fanout coordinated by the Nexus/default route.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiReadFanoutProxyRequestV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiReadFanoutProxyRequestV1 {
    /// Supported read endpoint identifier.
    pub endpoint: ToriiReadEndpointV1,
    /// Route scope that Nexus must recompute from its local catalog/world.
    pub route_scope: ToriiFanoutRouteScopeV1,
    /// Merge behavior for the endpoint response.
    pub merge: ToriiReadFanoutMergeV1,
    /// String path arguments in endpoint-specific order.
    pub path_args: Vec<String>,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// Raw JSON body for POST-style read endpoints.
    pub body: Vec<u8>,
    /// Response encoding negotiated by the ingress node.
    pub response_format: ToriiProxyResponseFormatV1,
}
/// Hosted HTTP request forwarded to a peer that may own a healthy Inrou target.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiHostedHttpProxyRequestV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiHostedHttpProxyRequestV1 {
    /// Soracloud service name already resolved from the public route.
    pub service_name: String,
    /// Exact service revision selected by the ingress node.
    pub service_version: String,
    /// Exact authoritative replica slot selected by the ingress node.
    pub replica_slot: u16,
    /// Request path relative to the admitted public route prefix.
    pub request_path: String,
    /// Original client HTTP method.
    pub method: String,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// End-to-end application headers preserved after ingress removes platform and hop-by-hop
    /// metadata.
    pub headers: Vec<ToriiProxyHeaderV1>,
    /// Raw request body bytes.
    pub body: Vec<u8>,
    /// Original client IP address when known, used for deterministic canary selection.
    pub remote_ip: Option<String>,
}
/// Canonical first-release Torii request body forwarded over the P2P control plane.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::derive::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyRequestKindV1")]
pub enum ToriiProxyRequestKindV1 {
    /// Submit a signed transaction to the authoritative lane validator.
    #[codec(index = 0)]
    SubmitTransaction {
        /// Original transaction entrypoint from the client.
        transaction: TransactionEntrypoint,
        /// Full routing plan resolved by the ingress node.
        expected_plan: ToriiRoutingPlanHintV1,
        /// Durability boundary the route-owning peer must satisfy before acknowledging.
        admission: ToriiProxyTransactionAdmissionV1,
        /// Exact shared journal binding required for a durable admission.
        ///
        /// This must be present for `QueuePlanSynced` and is revalidated by
        /// every forwarding and admitting authority.
        admission_binding: Option<QueuePlanAdmissionBindingV1>,
    },
    /// Execute a signed query on the authoritative lane validator.
    #[codec(index = 1)]
    SignedQuery {
        /// Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Exhaust a client-signed query on one exact authoritative route.
    #[codec(index = 2)]
    SignedQueryRouteScan {
        /// Original versioned Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a client-signed query fanout coordinated by the Nexus/default route.
    #[codec(index = 3)]
    SignedQueryFanout {
        /// Original versioned Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a routed Torii read endpoint on the authoritative peer.
    #[codec(index = 4)]
    Read(ToriiReadProxyRequestV1),
    /// Execute an App API read fanout coordinated by the Nexus/default route.
    #[codec(index = 5)]
    ReadFanout(ToriiReadFanoutProxyRequestV1),
    /// Proxy a Soracloud public hosted-HTTP request to a peer with a local healthy Inrou target.
    #[codec(index = 6)]
    HostedHttp(ToriiHostedHttpProxyRequestV1),
}
/// First-release P2P Torii proxy request sent from ingress to an authoritative peer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyRequestV1")]
pub struct ToriiProxyRequestV1 {
    /// Version of the proxy request envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Authenticated absolute execution deadline in Unix epoch milliseconds.
    ///
    /// Every proxy hop preserves this value. Receivers reject expired or
    /// excessive horizons before executing the request, and reserve the final
    /// portion of the budget for returning the bounded response.
    pub deadline_unix_ms: u64,
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyHeaderV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyHeaderV1 {
    /// Lower- or mixed-case header name as received from the responder.
    pub name: String,
    /// Raw header value bytes.
    pub value: Vec<u8>,
}
/// Serialized HTTP response sent back to the ingress node.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyHttpResponseV1")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::torii_proxy::ToriiProxyResponseV1")]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyResponseV1 {
    /// Version of the proxy response envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Serialized HTTP response from the authoritative peer.
    pub response: ToriiProxyHttpResponseV1,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};
    fn torii_read_endpoint_wire_index(endpoint: ToriiReadEndpointV1) -> u32 {
        let encoded = norito::codec::Encode::encode(&endpoint);
        assert_eq!(
            encoded.len(),
            4,
            "ToriiReadEndpointV1 should encode as a u32 variant index"
        );
        u32::from_le_bytes(encoded.try_into().expect("four-byte variant index"))
    }
    #[test]
    fn torii_proxy_v1_envelope_roundtrips_exact_deadline_bound_request() {
        let request = ToriiProxyRequestV1 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V1,
            request_id: Hash::new(b"torii-proxy-v1-roundtrip"),
            deadline_unix_ms: 1_900_000_000_000,
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
                endpoint: ToriiReadEndpointV1::AccountsList,
                route_scope: ToriiFanoutRouteScopeV1::AllDataspaces,
                expected_route: ToriiRouteHintV1 {
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(9),
                },
                path_args: Vec::new(),
                query_string: None,
                body: Vec::new(),
                response_format: ToriiProxyResponseFormatV1::Json,
            }),
        };
        let encoded = norito::to_bytes(&request).expect("encode V1 Torii proxy request");
        let decoded = norito::decode_from_bytes::<ToriiProxyRequestV1>(&encoded)
            .expect("decode V1 Torii proxy request");
        assert_eq!(decoded, request);
    }
    #[test]
    fn torii_read_endpoint_wire_indexes_match_first_release_schema() {
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountTransactionsGet),
            5
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountTransactionsQuery),
            6
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountHistoryGet),
            40
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountGet),
            41
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountTransactionGet),
            42
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountAssetGet),
            43
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::ContractDeploymentState),
            44
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountOnboardingCurrentState),
            45
        );
    }
}
