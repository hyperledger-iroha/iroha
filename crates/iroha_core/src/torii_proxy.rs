//! Peer-to-peer proxy envelopes for Torii ingress routing.

use std::fmt;

use iroha_crypto::Hash;
use iroha_data_model::{
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
    transaction::TransactionEntrypoint,
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

/// Role of one route in a Torii transaction routing plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiRouteLegRoleV1 {
    /// Coordinator route for final admission and commit ordering.
    Coordinator,
    /// Dataspace-local participant route.
    Participant,
}

impl From<crate::queue::RouteLegRole> for ToriiRouteLegRoleV1 {
    fn from(value: crate::queue::RouteLegRole) -> Self {
        match value {
            crate::queue::RouteLegRole::Coordinator => Self::Coordinator,
            crate::queue::RouteLegRole::Participant => Self::Participant,
        }
    }
}

impl From<ToriiRouteLegRoleV1> for crate::queue::RouteLegRole {
    fn from(value: ToriiRouteLegRoleV1) -> Self {
        match value {
            ToriiRouteLegRoleV1::Coordinator => Self::Coordinator,
            ToriiRouteLegRoleV1::Participant => Self::Participant,
        }
    }
}

/// One lane/dataspace leg in a Torii transaction routing plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiRouteLegHintV1 {
    /// Lane/dataspace route selected for this leg.
    pub route: ToriiRouteHintV1,
    /// Role assigned to this leg.
    pub role: ToriiRouteLegRoleV1,
}

impl From<crate::queue::RouteLeg> for ToriiRouteLegHintV1 {
    fn from(value: crate::queue::RouteLeg) -> Self {
        Self {
            route: value.route.into(),
            role: value.role.into(),
        }
    }
}

impl From<ToriiRouteLegHintV1> for crate::queue::RouteLeg {
    fn from(value: ToriiRouteLegHintV1) -> Self {
        Self::new(value.route.into(), value.role.into())
    }
}

/// Kind of validation failure in a Torii routing-plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToriiRoutingPlanHintErrorKind {
    /// A coordinator leg was encoded with a non-coordinator role.
    UnexpectedCoordinatorRole,
    /// A participant leg was encoded with a non-participant role.
    UnexpectedParticipantRole,
    /// A Native AMX hint advertised a digest that does not match its route legs.
    NativeAmxPlanDigestMismatch,
}

/// Error returned when a Torii routing-plan hint is not internally canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToriiRoutingPlanHintError {
    kind: ToriiRoutingPlanHintErrorKind,
    leg_index: Option<usize>,
    actual_role: Option<ToriiRouteLegRoleV1>,
    advertised_digest: Option<Hash>,
    computed_digest: Option<Hash>,
}

impl ToriiRoutingPlanHintError {
    /// Construct an error for a malformed coordinator leg role.
    #[must_use]
    pub const fn unexpected_coordinator_role(actual: ToriiRouteLegRoleV1) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::UnexpectedCoordinatorRole,
            leg_index: None,
            actual_role: Some(actual),
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a malformed participant leg role.
    #[must_use]
    pub const fn unexpected_participant_role(index: usize, actual: ToriiRouteLegRoleV1) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::UnexpectedParticipantRole,
            leg_index: Some(index),
            actual_role: Some(actual),
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a Native AMX digest that does not match the route legs.
    #[must_use]
    pub const fn native_amx_plan_digest_mismatch(advertised: Hash, computed: Hash) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::NativeAmxPlanDigestMismatch,
            leg_index: None,
            actual_role: None,
            advertised_digest: Some(advertised),
            computed_digest: Some(computed),
        }
    }

    /// Return the failure kind.
    #[must_use]
    pub const fn kind(&self) -> ToriiRoutingPlanHintErrorKind {
        self.kind
    }

    /// Return the malformed leg role, when this error is role-related.
    #[must_use]
    pub const fn actual_role(&self) -> Option<ToriiRouteLegRoleV1> {
        self.actual_role
    }

    /// Return the malformed participant index, when this error is participant-role related.
    #[must_use]
    pub const fn leg_index(&self) -> Option<usize> {
        self.leg_index
    }

    /// Return the advertised Native AMX plan digest, when this error is digest-related.
    #[must_use]
    pub const fn advertised_digest(&self) -> Option<Hash> {
        self.advertised_digest
    }

    /// Return the recomputed Native AMX plan digest, when this error is digest-related.
    #[must_use]
    pub const fn computed_digest(&self) -> Option<Hash> {
        self.computed_digest
    }
}

impl fmt::Display for ToriiRoutingPlanHintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ToriiRoutingPlanHintErrorKind::UnexpectedCoordinatorRole => match self.actual_role {
                Some(actual) => write!(f, "unexpected coordinator role {actual:?}"),
                None => f.write_str("unexpected coordinator role"),
            },
            ToriiRoutingPlanHintErrorKind::UnexpectedParticipantRole => {
                match (self.leg_index, self.actual_role) {
                    (Some(index), Some(actual)) => {
                        write!(f, "unexpected participant role {actual:?} at index {index}")
                    }
                    _ => f.write_str("unexpected participant role"),
                }
            }
            ToriiRoutingPlanHintErrorKind::NativeAmxPlanDigestMismatch => {
                match (self.advertised_digest, self.computed_digest) {
                    (Some(advertised), Some(computed)) => write!(
                        f,
                        "native AMX plan digest mismatch: advertised {advertised}, computed {computed}"
                    ),
                    _ => f.write_str("native AMX plan digest mismatch"),
                }
            }
        }
    }
}

impl std::error::Error for ToriiRoutingPlanHintError {}

/// Stable full routing plan determined at ingress.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiRoutingPlanHintV1 {
    /// Single coordinator route.
    Single(ToriiRouteLegHintV1),
    /// Native AMX coordinator and participant route set.
    NativeAmx {
        /// Stable digest of the native AMX plan.
        plan_digest: Hash,
        /// Coordinator route for final ordering.
        coordinator: ToriiRouteLegHintV1,
        /// Dataspace-local participant routes.
        participants: Vec<ToriiRouteLegHintV1>,
    },
}

impl ToriiRoutingPlanHintV1 {
    /// Return the coordinator route for peer selection and diagnostics.
    #[must_use]
    pub fn coordinator_route(&self) -> ToriiRouteHintV1 {
        match self {
            Self::Single(leg) => leg.route,
            Self::NativeAmx { coordinator, .. } => coordinator.route,
        }
    }

    /// Convert this hint to a full routing plan after validating redundant wire fields.
    ///
    /// # Errors
    /// Returns an error when leg roles are not canonical or a Native AMX hint's advertised digest
    /// does not match the digest recomputed from its route legs.
    pub fn try_into_routing_plan(
        self,
    ) -> Result<crate::queue::RoutingPlan, ToriiRoutingPlanHintError> {
        match self {
            Self::Single(leg) => {
                if leg.role != ToriiRouteLegRoleV1::Coordinator {
                    return Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                        leg.role,
                    ));
                }
                Ok(crate::queue::RoutingPlan::single(
                    crate::queue::RouteLeg::from(leg).route,
                ))
            }
            Self::NativeAmx {
                plan_digest,
                coordinator,
                participants,
            } => {
                if coordinator.role != ToriiRouteLegRoleV1::Coordinator {
                    return Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                        coordinator.role,
                    ));
                }

                let mut participant_legs = Vec::with_capacity(participants.len());
                for (index, leg) in participants.into_iter().enumerate() {
                    if leg.role != ToriiRouteLegRoleV1::Participant {
                        return Err(ToriiRoutingPlanHintError::unexpected_participant_role(
                            index, leg.role,
                        ));
                    }
                    participant_legs.push(crate::queue::RouteLeg::from(leg));
                }

                let plan = crate::queue::RoutingPlan::native_amx(
                    crate::queue::RouteLeg::from(coordinator).route,
                    participant_legs,
                );
                let computed = plan.digest();
                if computed != plan_digest {
                    return Err(ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
                        plan_digest,
                        computed,
                    ));
                }
                Ok(plan)
            }
        }
    }
}

impl From<crate::queue::RoutingPlan> for ToriiRoutingPlanHintV1 {
    fn from(value: crate::queue::RoutingPlan) -> Self {
        match value {
            crate::queue::RoutingPlan::Single(leg) => Self::Single(leg.into()),
            crate::queue::RoutingPlan::NativeAmx(plan) => Self::NativeAmx {
                plan_digest: plan.plan_digest,
                coordinator: plan.coordinator.into(),
                participants: plan.participants.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<ToriiRoutingPlanHintV1> for crate::queue::RoutingPlan {
    fn from(value: ToriiRoutingPlanHintV1) -> Self {
        match value {
            ToriiRoutingPlanHintV1::Single(leg) => {
                let leg: crate::queue::RouteLeg = leg.into();
                Self::single(leg.route)
            }
            ToriiRoutingPlanHintV1::NativeAmx {
                coordinator,
                participants,
                ..
            } => Self::native_amx(
                crate::queue::RouteLeg::from(coordinator).route,
                participants.into_iter().map(Into::into).collect(),
            ),
        }
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
    /// `GET /v1/nexus/public_lanes/{lane_id}/validators`
    NexusPublicLaneValidators,
    /// `GET /v1/nexus/public_lanes/{lane_id}/stake`
    NexusPublicLaneStake,
    /// `GET /v1/nexus/public_lanes/{lane_id}/rewards/pending`
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
    /// `POST /v1/aliases/resolve_index`
    AliasResolveIndex,
    /// `POST /v1/aliases/by_account`
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
    /// `GET /v1/musubi/packages`
    MusubiPackagesSearch,
    /// `GET /v1/musubi/release`
    MusubiReleaseGet,
    /// `GET /v1/musubi/releases`
    MusubiPackageReleases,
    /// `GET /v1/musubi/versions`
    MusubiPackageVersions,
    /// `GET /v1/musubi/aliases/{alias}`
    MusubiAliasResolve,
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

/// Route set Nexus should recompute for a coordinated fanout request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiFanoutRouteScopeV1 {
    /// Fan out across all configured dataspace routes.
    AllDataspaces,
    /// Fan out across the dataspaces that may own the target account.
    TargetAccount {
        /// Canonical target account id literal.
        account_id: String,
    },
    /// Fan out across public routes plus caller-visible private dataspaces.
    VisibleAccount {
        /// Optional canonical caller account id literal.
        caller_account_id: Option<String>,
    },
}

/// Merge behavior requested for an App API read fanout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadFanoutMergeV1 {
    /// Merge JSON list-style responses.
    List,
    /// Merge JSON singleton responses.
    Singleton,
    /// Merge account-detail responses while preserving the requested response format.
    Account,
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
    /// Original request headers preserved by ingress.
    pub headers: Vec<ToriiProxyHeaderV1>,
    /// Raw request body bytes.
    pub body: Vec<u8>,
    /// Original client IP address when known, used for deterministic canary selection.
    pub remote_ip: Option<String>,
}

/// Canonical Torii request body forwarded over the P2P control plane.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyRequestKindV1 {
    /// Submit a signed transaction to the authoritative lane validator.
    SubmitTransaction {
        /// Original transaction entrypoint from the client.
        transaction: TransactionEntrypoint,
        /// Full routing plan resolved by the ingress node.
        expected_plan: ToriiRoutingPlanHintV1,
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
    /// Execute a verified query fanout coordinated by the Nexus/default route.
    VerifiedQueryFanout {
        /// Norito-encoded verified query payload forwarded by the ingress node.
        request_bytes: Vec<u8>,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a routed Torii read endpoint on the authoritative peer.
    Read(ToriiReadProxyRequestV1),
    /// Execute an App API read fanout coordinated by the Nexus/default route.
    ReadFanout(ToriiReadFanoutProxyRequestV1),
    /// Proxy a Soracloud public hosted-HTTP request to a peer with a local healthy Inrou target.
    HostedHttp(ToriiHostedHttpProxyRequestV1),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};

    #[test]
    fn torii_routing_plan_hint_roundtrips_single_and_native_amx_plans() {
        let single_route = RoutingDecision::new(LaneId::new(4), DataSpaceId::new(9));
        let single_hint = ToriiRoutingPlanHintV1::from(RoutingPlan::single(single_route));

        assert_eq!(
            single_hint.coordinator_route(),
            ToriiRouteHintV1 {
                lane_id: single_route.lane_id,
                dataspace_id: single_route.dataspace_id,
            }
        );
        assert_eq!(
            single_hint
                .clone()
                .try_into_routing_plan()
                .expect("canonical single-route hint should validate"),
            RoutingPlan::single(single_route)
        );
        assert_eq!(
            RoutingPlan::from(single_hint),
            RoutingPlan::single(single_route)
        );

        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let native_plan = RoutingPlan::native_amx(
            coordinator,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
                    RouteLegRole::Coordinator,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7)),
                    RouteLegRole::Coordinator,
                ),
            ],
        );
        let native_hint = ToriiRoutingPlanHintV1::from(native_plan.clone());

        assert_eq!(
            native_hint.coordinator_route(),
            ToriiRouteHintV1 {
                lane_id: coordinator.lane_id,
                dataspace_id: coordinator.dataspace_id,
            }
        );
        let ToriiRoutingPlanHintV1::NativeAmx {
            plan_digest,
            participants,
            ..
        } = &native_hint
        else {
            panic!("expected native AMX routing plan hint");
        };
        assert_eq!(*plan_digest, native_plan.digest());
        assert!(
            participants
                .iter()
                .all(|leg| leg.role == ToriiRouteLegRoleV1::Participant)
        );
        assert_eq!(
            native_hint
                .clone()
                .try_into_routing_plan()
                .expect("canonical native AMX hint should validate"),
            native_plan
        );
        assert_eq!(RoutingPlan::from(native_hint), native_plan);
    }

    #[test]
    fn torii_routing_plan_hint_rejects_forged_digest_and_roles() {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let native_plan = RoutingPlan::native_amx(
            coordinator,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(3), DataSpaceId::new(9)),
                    RouteLegRole::Participant,
                ),
            ],
        );

        let mut forged_digest = ToriiRoutingPlanHintV1::from(native_plan.clone());
        let advertised = Hash::new(b"forged-native-amx-plan-digest");
        let ToriiRoutingPlanHintV1::NativeAmx { plan_digest, .. } = &mut forged_digest else {
            panic!("expected native AMX hint");
        };
        *plan_digest = advertised;
        assert_eq!(
            forged_digest.try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
                advertised,
                native_plan.digest()
            ))
        );

        let wrong_single_role = ToriiRoutingPlanHintV1::Single(ToriiRouteLegHintV1 {
            route: ToriiRouteHintV1::from(coordinator),
            role: ToriiRouteLegRoleV1::Participant,
        });
        assert_eq!(
            wrong_single_role.try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                ToriiRouteLegRoleV1::Participant
            ))
        );

        let mut wrong_participant_role = ToriiRoutingPlanHintV1::from(native_plan);
        let ToriiRoutingPlanHintV1::NativeAmx { participants, .. } = &mut wrong_participant_role
        else {
            panic!("expected native AMX hint");
        };
        participants[1].role = ToriiRouteLegRoleV1::Coordinator;
        assert_eq!(
            wrong_participant_role.try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::unexpected_participant_role(
                1,
                ToriiRouteLegRoleV1::Coordinator
            ))
        );
    }
}
