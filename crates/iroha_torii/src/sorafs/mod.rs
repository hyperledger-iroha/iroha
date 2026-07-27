//! SoraFS-related helpers exposed by Torii.

pub mod admission;
pub mod alias_cache;
#[cfg(feature = "app_api")]
pub mod api;
pub mod blinded;
pub mod concurrency;
#[cfg(feature = "app_api")]
pub mod delegated_routing;
pub mod discovery;
#[cfg(feature = "app_api")]
pub(crate) mod evidence_viewer_api;
#[cfg(feature = "app_api")]
pub(crate) mod evidence_viewer_runtime;
pub mod gateway;
#[cfg(feature = "app_api")]
pub(crate) mod gateway_compliance_api;
pub mod gc;
#[cfg(feature = "app_api")]
pub(crate) mod hedging_billing_api;
pub mod hosts;
pub mod limits;
#[cfg(feature = "app_api")]
pub mod moderation_runtime;
#[cfg(feature = "app_api")]
pub(crate) mod orderbook_runtime;
pub(crate) mod orderbook_worker;
#[cfg(feature = "app_api")]
pub mod pop_api;
pub mod por;
#[cfg(feature = "app_api")]
pub mod potr_signing;
pub mod quota;
#[cfg(feature = "app_api")]
pub mod registry;
#[cfg(feature = "app_api")]
pub(crate) mod reserve_api;
#[cfg(feature = "app_api")]
pub(crate) mod reserve_runtime;
pub(crate) mod reserve_worker;
pub mod site;
pub mod token;

pub use admission::{
    AdmissionCheckError, AdmissionRegistry, AdmissionRegistryError, AdmissionRegistryUpdateError,
};
#[cfg(feature = "app_api")]
pub(crate) use alias_cache::evaluate_cache_decision;
pub use alias_cache::{
    AliasCacheEnforcement, AliasCachePolicy, AliasCachePolicyExt, AliasCachePolicyHttpExt,
    AliasProofError, AliasProofEvaluation, AliasProofEvaluationExt, AliasProofState, CacheDecision,
    CacheDecisionOutcome, GovernanceAssessment, SuccessorAssessment, decode_alias_proof,
    enforcement_from_config, policy_from_config, unix_now_secs,
};
pub use blinded::{
    BLINDED_CID_LEN, BlindedCidResolver, ResolveError as BlindedResolveError, SaltSchedule,
    SaltScheduleError,
};
pub(crate) use concurrency::{StreamTokenConcurrencyPermit, StreamTokenConcurrencyTracker};
pub use discovery::{
    ProviderAdvertCache, ReplayCheckpointError, capability_name, parse_capability_name,
};
#[cfg(feature = "app_api")]
pub use gc::GcSweeperRuntime;
pub use hosts::{HostMappingInput, HostMappingSummary};
pub use limits::{
    QuotaExceeded, SorafsAction, SorafsQuotaConfig, SorafsQuotaEnforcer, SorafsQuotaWindow,
};
#[cfg(feature = "app_api")]
pub use por::{
    DrandHttpRandomnessProvider, PorAutomationError, PorCoordinatorRuntime, PorStorage,
    RandomnessProvider, VerifiedVrfProvider, VrfError, VrfProvider,
};
pub use por::{PorCoordinator, PorCoordinatorError, PorStatusExportV1, PorStatusFilter};
#[cfg(feature = "app_api")]
pub use potr_signing::{
    PotrAdmissionMaterialResolverV1, PotrAdmissionReaderError, PotrAdmissionReaderV1,
    PotrAdmissionRegistryResolverV1, PotrAdmissionSnapshotV1,
    PotrFinalizedAdmissionReaderConfigError, PotrFinalizedAdmissionReaderV1,
    PotrFinalizedPolicySnapshotV1, PotrFinalizedPolicySourceV1, PotrGatewaySignerV1,
    PotrProviderSignerV1, PotrRuntimeProviderBindingV1, PotrRuntimeProviderQualificationV1,
    PotrRuntimeReaderBindingsV1, PotrRuntimeSignerConfigError, PotrRuntimeSignerRolesV1,
    PotrRuntimeSignersV1, PotrSignerServiceError, PotrStateFinalizedPolicySourceV1,
};
pub(crate) use quota::{StreamTokenQuotaError, StreamTokenQuotaTracker};
#[cfg(feature = "app_api")]
pub(crate) use registry::{
    CapacitySnapshot, RegistryDeclaration, RegistryError, RegistryFeeLedgerEntry, collect_snapshot,
};
pub use sorafs_manifest::{
    capacity::ReplicationOrderV1,
    provider_advert::{EndpointKind, TransportProtocol},
};
#[cfg(feature = "app_api")]
pub use sorafs_node::{
    PotrAdmissionPolicyBindingError, PotrAdmissionPolicyBindingV1, PotrAdmissionPolicyProgressError,
};
pub(crate) use token::{
    MAX_CLIENT_ID_BYTES, MAX_NONCE_BYTES, MAX_STREAM_TOKEN_BASE64_BYTES, MAX_TOKEN_FUTURE_SKEW_SECS,
};
pub use token::{
    StreamTokenHeaderError, StreamTokenIssuer, StreamTokenIssuerError, StreamTokenRuntimeSigner,
    StreamTokenSigningError, TokenOverrides, decode_token_base64, encode_token_base64,
};
