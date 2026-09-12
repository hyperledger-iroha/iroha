//! Embeddable Iroha daemon launcher and runtime-provider injection surface.
//!
//! The stock binaries and deployment-owned launchers share this exact implementation. External
//! launchers can provide runtime-only adapters through [`IrohaRuntimeProviderRegistryV1`] without
//! copying daemon startup logic or exposing provider credentials through `iroha_config`.
/// Authenticated external software signer service and broker adapters.
#[cfg(feature = "daemon")]
pub mod external_software_signer;
#[cfg(all(feature = "daemon", unix))]
mod runtime_credential;
/// Opaque hardware operations fenced by independently authenticated custody and completion.
#[cfg(feature = "daemon")]
pub mod signer_operation;
use iroha_model_base::peer::PeerId;
#[cfg(all(feature = "daemon", unix))]
pub use runtime_credential::RuntimeCredentialErrorV1;
include!("main.rs");

#[cfg(all(test, feature = "daemon"))]
mod frame_test_support;
