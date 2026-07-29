//! Embeddable Iroha daemon launcher and runtime-provider injection surface.
//!
//! The stock binaries and deployment-owned launchers share this exact
//! implementation. External launchers can provide runtime-only adapters
//! through [`IrohaRuntimeProviderRegistryV1`] without copying daemon startup
//! logic or exposing provider credentials through `iroha_config`.

include!("main.rs");
