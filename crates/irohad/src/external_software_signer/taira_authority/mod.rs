//! Authenticated, purpose-separated authorities for the Taira release corridor.
//!
//! This module deliberately uses a protocol namespace distinct from the SoraFS
//! external-signer protocol.  Each service process owns exactly one role, one
//! encrypted Ed25519 key, one administrator-issued run ledger, one consume-once
//! replay ledger, and one predecessor-bound audit chain.

mod protocol;
mod sandbox;
mod service;
mod store;
mod transport;

pub use protocol::{
    TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1, TairaAuthorityArtifactManifestEntryV1,
    TairaAuthorityInstallationV1, TairaAuthorityPublicBindingV1, TairaAuthorityRoleV1,
    validate_taira_authority_installations_v1, validate_taira_authority_registry_v1,
};
pub use service::{TairaAuthorityErrorV1, TairaAuthorityProvisioningV1, TairaAuthorityServiceV1};
pub use transport::{
    TairaAuthorityClientV1, TairaAuthorityEndpointPolicyV1, TairaAuthorityServerV1,
};

/// Run the standalone native authority command-line interface.
pub fn run_cli() -> Result<(), &'static str> {
    crate::external_software_signer::taira_authority::transport::run_cli()
}

#[cfg(test)]
mod tests;
