//! Embeddable Iroha daemon launcher and runtime-provider injection surface.
//!
//! The stock binaries and deployment-owned launchers share this exact implementation. External
//! launchers can provide runtime-only adapters through [`IrohaRuntimeProviderRegistryV1`] without
//! copying daemon startup logic or exposing provider credentials through `iroha_config`.
/// Authenticated external software signer service and broker adapters.
#[cfg(feature = "daemon")]
pub mod external_software_signer;
include!("main.rs");
