//! Isolated authenticated software signing for SoraFS runtime roles.
//!
//! The service keeps private keys outside node configuration and ledger state.
//! A deployment supplies a 256-bit wrapping key through an inherited descriptor
//! or a systemd credential. The process decrypts one role-bound key envelope in
//! memory, serves only that role on peer-credential-authenticated Unix sockets,
//! and persists a payload-free predecessor-bound audit chain before reporting a
//! mutating operation as successful.
//!
//! The broker adapter implements the existing native transaction signer traits,
//! so replacing this software backend with a future HSM adapter does not change
//! Torii or runtime-provider registry interfaces.
#[cfg(unix)]
mod adapter;
mod envelope;
#[cfg(unix)]
mod journal;
#[allow(dead_code)]
mod privacy_governance;
mod protocol;
#[cfg(unix)]
mod runtime_adapters;
#[cfg(unix)]
mod runtime_backends;
#[cfg(unix)]
mod service;
#[cfg(unix)]
mod typed_payload;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use adapter::{
    ExternalSoftwareSignerAdapterErrorV1, ExternalSoftwareSignerNativeAdapterV1,
    ExternalSoftwareSignerNativeBackendsV1,
};
pub use envelope::{
    SoftwareSignerEnvelopeErrorV1, SoftwareSignerKeyEnvelopeV1, SoftwareSignerWrappingKeyV1,
};
#[cfg(unix)]
pub use journal::SoftwareSignerJournalErrorV1;
pub use protocol::{
    ExternalSignerBackendV1, SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1, SoftwareSignerKeyAlgorithmV1,
    SoftwareSignerLiveProvenanceV1, SoftwareSignerPublicBindingV1, SoftwareSignerPurposeBindingV1,
    SoftwareSignerRoleV1, SoftwareSignerValueParseErrorV1,
};
#[cfg(unix)]
pub use runtime_adapters::{
    ExternalSoftwareSignerBillingStatementAdapterV1, ExternalSoftwareSignerEvidenceViewerAdapterV1,
    ExternalSoftwareSignerGovernanceDagAdapterV1, ExternalSoftwareSignerPopIssuerAdapterV1,
    ExternalSoftwareSignerPopRegistryV1, ExternalSoftwareSignerPotrGatewayAdapterV1,
    ExternalSoftwareSignerPotrProviderAdapterV1, ExternalSoftwareSignerStreamTokenAdapterV1,
};
#[cfg(unix)]
pub use runtime_backends::ExternalSoftwareSignerBackendsV1;
#[cfg(unix)]
pub use service::{
    SoftwareSignerAdminErrorV1, SoftwareSignerErrorV1, SoftwareSignerProvisioningV1,
    SoftwareSignerServiceV1,
};
#[cfg(unix)]
pub use typed_payload::SoftwareSignerPurposeV1;
#[cfg(unix)]
pub use unix::{
    ExternalSoftwareSignerClientErrorV1, SoftwareSignerAdministratorClientV1,
    SoftwareSignerClientV1, SoftwareSignerCredentialErrorV1, SoftwareSignerEndpointPolicyV1,
    SoftwareSignerRotationRequestV1, SoftwareSignerServerErrorV1, SoftwareSignerServerV1,
    SoftwareSignerSignatureReceiptV1, load_software_signer_wrapping_key_from_credential_v1,
    load_software_signer_wrapping_key_from_fd_v1,
};
#[cfg(all(test, unix))]
mod runtime_adapter_tests;
#[cfg(all(test, unix))]
mod tests;
