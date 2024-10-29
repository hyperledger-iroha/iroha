//! Iroha Trigger Rust SDK
#![no_std]
#![allow(unsafe_code)]

pub use iroha_smart_contract as smart_contract;
pub use iroha_smart_contract_utils::debug;
pub use iroha_trigger_derive::main;
pub use smart_contract::{data_model, stub_getrandom};

pub mod log {
    //! WASM logging utilities
    pub use iroha_smart_contract_utils::{debug, error, event, info, log::*, trace, warn};
}

/// Get context for smart contract `main()` entrypoint.
///
/// # Safety
///
/// It's safe to call this function as long as it's safe to construct, from the given
/// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
#[cfg(not(test))]
pub unsafe fn decode_trigger_context(
    context: *const u8,
) -> data_model::smart_contract::payloads::TriggerContext {
    // Safety: ownership of the returned result is transferred into `_decode_from_raw`
    iroha_smart_contract_utils::decode_with_length_prefix_from_raw(context)
}

pub mod prelude {
    //! Common imports used by triggers

    pub use iroha_smart_contract::prelude::*;
    pub use iroha_smart_contract_utils::debug::DebugUnwrapExt;
    pub use iroha_trigger_derive::main;

    pub use crate::data_model::smart_contract::payloads::TriggerContext as Context;
}
