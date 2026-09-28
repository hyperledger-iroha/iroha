//! Inbound light clients (`specs/sccp.md` §4.13). Owners: ws33 (Parliament half:
//! [`is_usable`], [`initialize`], [`install_checkpoint`], [`freeze`]) and ws41 (advance half:
//! [`execute_advance`], [`execute_report_equivocation`], [`verify_source_proof`],
//! [`preverify_keeper_advance`], [`prune`]).
//!
//! World state stores authenticated source validator sets with validity ranges and finalized
//! checkpoints. Every advance is permissionless and proof-carrying; the Parliament only
//! initializes, re-initializes, freezes and installs trusted checkpoints. Chain verification
//! itself lives in `iroha_sccp::light_client`.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1},
    not_wired,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    bridge::SccpNetworkV1,
    isi::sccp::{AdvanceSccpLightClientV1, ReportSccpLightClientEquivocationV1},
    sccp::{
        governance::{
            SccpFreezeLightClientActionV1, SccpInitializeLightClientActionV1,
            SccpInstallTrustedCheckpointActionV1,
        },
        inbound::SccpSourceProofBytesV1,
    },
};
use iroha_sccp::light_client::proof::SccpVerifiedProofV1;

/// Return whether the light client of `network` is installed, not frozen and within its
/// weak-subjectivity bound, so burns on its chain are provable (§4.13.2).
///
/// The skeleton reports every light client unusable until ws33 implements the check.
#[must_use]
pub fn is_usable(world: &(impl WorldReadOnly + ?Sized), network: SccpNetworkV1) -> bool {
    let _ = (world, network);
    // TODO(ws33): installed, not frozen and fresh against `ws_bound_ms` (§4.13.2).
    false
}

/// Apply an enacted `InitializeLightClient` (§4.14.3).
///
/// # Errors
///
/// Fails closed until ws33 implements initialization.
pub fn initialize(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _action: &SccpInitializeLightClientActionV1,
    _proposal_id: [u8; 32],
) -> Result<(), Error> {
    // TODO(ws33): expectation, fresh bootstrap, state and `SccpLightClientInitialized`.
    Err(not_wired("light-client initialization", "ws33"))
}

/// Apply an enacted `InstallTrustedCheckpoint` (§4.14.3).
///
/// # Errors
///
/// Fails closed until ws33 implements checkpoint installation.
pub fn install_checkpoint(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _action: &SccpInstallTrustedCheckpointActionV1,
    _proposal_id: [u8; 32],
) -> Result<(), Error> {
    // TODO(ws33): conflict check and `origin: Parliament` checkpoint (§4.13.1).
    Err(not_wired("trusted checkpoint installation", "ws33"))
}

/// Apply an enacted `FreezeLightClient` (§4.14.3).
///
/// # Errors
///
/// Fails closed until ws33 implements freezing.
pub fn freeze(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _action: &SccpFreezeLightClientActionV1,
    _proposal_id: [u8; 32],
) -> Result<(), Error> {
    // TODO(ws33): Parliament freeze and `SccpLightClientFrozen`.
    Err(not_wired("light-client freeze", "ws33"))
}

/// Execute `AdvanceSccpLightClientV1`.
///
/// # Errors
///
/// Fails closed until ws41 implements advances.
pub fn execute_advance(
    _instruction: AdvanceSccpLightClientV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws41): CAS guard, verification, sets, checkpoints and head (§4.13.2).
    Err(not_wired("AdvanceSccpLightClientV1 execution", "ws41"))
}

/// Execute `ReportSccpLightClientEquivocationV1`.
///
/// # Errors
///
/// Fails closed until ws41 implements equivocation reports.
pub fn execute_report_equivocation(
    _instruction: ReportSccpLightClientEquivocationV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws41): two conflicting quorum-valid records freeze the light client (§4.13.2).
    Err(not_wired(
        "ReportSccpLightClientEquivocationV1 execution",
        "ws41",
    ))
}

/// Verify an inbound or void `proof` against `network`'s light client, reserving verifier work,
/// and return the normalized source event and the checkpoints to record (§4.12.1 step 4).
///
/// # Errors
///
/// Fails closed until ws41 implements proof verification.
pub fn verify_source_proof(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _proof: &SccpSourceProofBytesV1,
) -> Result<SccpVerifiedProofV1, Error> {
    // TODO(ws41): `iroha_sccp::light_client` verification under `[zk.sccp]` work limits.
    Err(not_wired("source proof verification", "ws41"))
}

/// Pre-verify a keeper advance from a bridge key's account and return its admission keys (one
/// pending exempt advance per authority and network, §4.13.4).
///
/// # Errors
///
/// Rejects until ws41 implements pre-verification.
pub fn preverify_keeper_advance(
    world: &(impl WorldReadOnly + ?Sized),
    next_block_height: u64,
    instruction: &AdvanceSccpLightClientV1,
    authority: &AccountId,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let _ = (world, next_block_height, instruction, authority);
    // TODO(ws41): bridge-key authority, head movement and work limits (§4.13.4).
    Err(SccpAdmissionRejectV1::not_wired(
        "keeper advance pre-verification",
        "ws41",
    ))
}

/// Prune expired light-client sets and checkpoints, deleting at most `budget` records, and
/// return the number deleted (§4.13.1).
///
/// The skeleton deletes nothing until ws41 implements retention.
pub fn prune(state_transaction: &mut StateTransaction<'_, '_>, budget: usize) -> usize {
    let _ = (state_transaction, budget);
    // TODO(ws41): 180 d set retention, stride and Parliament checkpoints kept (§4.13.1).
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state, header,
    };

    #[test]
    fn skeleton_light_clients_are_unusable_and_fail_closed() {
        let state = blank_state();
        let view = state.world_view();
        assert!(!is_usable(&view, SccpNetworkV1::EthereumMainnet));
        let reject =
            preverify_keeper_advance(&view, 3, &SampleInstructions::advance(), &authority(1))
                .expect_err("skeleton");
        assert!(reject.reason.contains("TODO(ws41)"), "{reject}");
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(prune(&mut stx, 1_024), 0);
        let proof = SccpSourceProofBytesV1::new(vec![1, 2, 3]).expect("bounded proof");
        let error = verify_source_proof(&mut stx, SccpNetworkV1::EthereumMainnet, &proof)
            .expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws41)"), "{error}");
        let freeze_error = freeze(
            &mut stx,
            &SccpFreezeLightClientActionV1 {
                network: SccpNetworkV1::TronMainnet,
            },
            [2; 32],
        )
        .expect_err("skeleton");
        assert!(
            freeze_error.to_string().contains("TODO(ws33)"),
            "{freeze_error}"
        );
    }
}
