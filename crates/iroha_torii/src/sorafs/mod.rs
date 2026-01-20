//! SoraFS-related helpers exposed by Torii.

pub mod admission;
pub mod alias_cache;
pub mod api;
pub mod blinded;
pub mod concurrency;
pub mod discovery;
pub mod gc;
pub mod gateway;
pub mod hosts;
pub mod limits;
pub mod pin;
pub mod por;
pub mod quota;
pub mod repair;
pub mod registry;
pub mod token;

pub use admission::{AdmissionCheckError, AdmissionRegistry, AdmissionRegistryError};
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
pub use discovery::{ProviderAdvertCache, capability_name, parse_capability_name};
pub use hosts::{HostMappingInput, HostMappingSummary};
pub use limits::{
    QuotaExceeded, SorafsAction, SorafsQuotaConfig, SorafsQuotaEnforcer, SorafsQuotaWindow,
};
pub use pin::{PinAuthError, PinPolicyError, PinSubmissionPolicy};
#[cfg(feature = "app_api")]
pub use por::{
    DeterministicRandomnessProvider, EmptyVrfProvider, FilesystemGovernancePublisher,
    GovernancePublisher, PorAutomationError, PorCoordinatorRuntime, PorStorage, RandomnessProvider,
    VrfProvider,
};
#[cfg(feature = "app_api")]
pub use gc::GcSweeperRuntime;
#[cfg(feature = "app_api")]
pub use repair::RepairWorkerRuntime;
pub use por::{PorCoordinator, PorCoordinatorError, PorStatusExportV1, PorStatusFilter};
pub(crate) use quota::{StreamTokenQuotaExceeded, StreamTokenQuotaTracker};
pub(crate) use registry::{
    CapacitySnapshot, RegistryDeclaration, RegistryError, RegistryFeeLedgerEntry, collect_snapshot,
};
pub use token::{
    StreamTokenHeaderError, StreamTokenIssuer, StreamTokenIssuerError, TokenOverrides,
    decode_token_base64, encode_token_base64,
};
