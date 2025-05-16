//! WASM runtime contol and monitoring utilities.

/// Allocate more fuel in the current runtime.
///
/// # Errors
///
/// - If execution on Iroha side failed
#[cfg(not(test))]
pub fn add_fuel(fuel: u64) {
    unsafe { iroha_smart_contract_utils::encode_and_execute(&fuel, host::add_fuel) }
}

/// Consume some fuel in the current runtime.
///
/// # Errors
///
/// - If execution on Iroha side failed
#[cfg(not(test))]
pub fn consume_fuel(fuel: u64) {
    unsafe { iroha_smart_contract_utils::encode_and_execute(&fuel, host::consume_fuel) }
}

#[cfg(not(test))]
mod host {
    #[link(wasm_import_module = "iroha")]
    extern "C" {
        pub(super) fn add_fuel(ptr: *const u8, len: usize);
    }

    #[link(wasm_import_module = "iroha")]
    extern "C" {
        pub(super) fn consume_fuel(ptr: *const u8, len: usize);
    }
}
