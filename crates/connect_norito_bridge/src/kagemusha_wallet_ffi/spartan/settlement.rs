//! The authenticated wallet's retained ledger outcomes and checkpoint-rooted epoch synchronization.
use super::*;
use iroha_core_zk::kagemusha_spartan_native::UnloadSettlement;
use iroha_data_model::{
    kagemusha::KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
    sumeragi_finality::MAX_COMMIT_CERTIFICATE_BYTES_V1,
};

pub(super) fn outcome_bytes(outcome: &UnloadSettlement) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(83);
    bytes.extend_from_slice(&outcome.entrypoint);
    bytes.extend_from_slice(
        &outcome
            .required_height
            .unwrap_or(outcome.height)
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&outcome.block);
    bytes.extend_from_slice(&[
        u8::from(outcome.accepted),
        u8::from(outcome.paid),
        u8::from(outcome.epoch_required.is_some()),
    ]);
    bytes.extend_from_slice(&outcome.epoch_required.unwrap_or_default().to_le_bytes());
    bytes
}

/// Verify checkpoint-rooted finality for the exact account-signed envelope of a released Unload.
/// An empty proof only reads the previous decision from protected custody.
/// # Safety
/// Slot and operation address 32 bytes each; variable inputs address their lengths and output is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_unload_settlement_v1(
    handle: u64,
    slot: *const u8,
    operation: *const u8,
    signed: *const u8,
    signed_len: usize,
    proof: *const u8,
    proof_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::UnloadSettlement {
                    slot: fixed32(slot)?,
                    operation: fixed32(operation)?,
                    signed: exports::input(signed, signed_len, 64 * 1024)?,
                    proof: exports::input(
                        proof,
                        proof_len,
                        KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
                    )?,
                },
            )
        })
    }
}

/// Verify finality for the account-signed activation or read its retained outcome with an empty proof.
///
/// # Safety
/// Slot addresses 32 bytes, variable inputs address their stated lengths, and output is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_activation_settlement_v1(
    handle: u64,
    slot: *const u8,
    signed: *const u8,
    signed_len: usize,
    proof: *const u8,
    proof_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::ActivationSettlement {
                    slot: fixed32(slot)?,
                    signed: exports::input(signed, signed_len, 64 * 1024)?,
                    proof: exports::input(
                        proof,
                        proof_len,
                        KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
                    )?,
                },
            )
        })
    }
}

/// Query the protected epoch cursor with (0, empty), or verify one exact incumbent boundary certificate.
/// # Safety
/// Slot addresses 32 bytes; boundary addresses its stated length; output is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_epoch_v1(
    handle: u64,
    slot: *const u8,
    expected: u64,
    boundary: *const u8,
    boundary_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Epoch {
                    slot: fixed32(slot)?,
                    expected,
                    boundary: exports::input(
                        boundary,
                        boundary_len,
                        MAX_COMMIT_CERTIFICATE_BYTES_V1,
                    )?,
                },
            )
        })
    }
}
