//! `SubmitSccpAttestationFaultV1`: equivocation evidence (`specs/sccp.md` §4.11). Owner: ws31.
//!
//! Any valid bridge signature over a non-canonical statement records a fault, evicts and bars
//! the key's peer and forces a new roster generation at the executing block: the roster rule
//! (§4.3.2) treats a height that recorded a fault as a rotation height, and a faulted key is a
//! zero slot from then on.
//!
//! TODO(phase 2, §4.11): apply the indexed consensus penalty to the validator bonded under the
//! faulty peer after `slashing_delay_blocks`.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1, SccpExemptClassV1},
    store,
    subjects::{SccpStatementDigests, fields},
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::sccp::SubmitSccpAttestationFaultV1,
    sccp::{
        events::{SccpAttestationFaultV1, SccpEvent},
        keys::{SccpAttestationFaultRecordV1, SccpFaultRefV1},
    },
};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::signature::recover_address;

/// Error text of evidence whose `(address, height)` fault is already recorded (§4.11).
pub const FAULT_ALREADY_RECORDED: &str = "SccpFaultAlreadyRecorded";

/// Verified fault evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedFault {
    address: [u8; 20],
    peer: PeerId,
    height: u64,
    statement_hash: [u8; 32],
}

/// Run the §4.11 validation of `instruction` against `world` at `current_height`.
fn check(
    world: &(impl WorldReadOnly + ?Sized),
    digests: &(impl SccpStatementDigests + ?Sized),
    current_height: u64,
    instruction: &SubmitSccpAttestationFaultV1,
) -> Result<CheckedFault, String> {
    if store::parameters::get(world).is_none() {
        return Err("SCCP does not exist on this network".into());
    }
    let statement = &instruction.statement;
    let statement_hash = fields(statement).digest(digests.taira_network_id().as_bytes());
    let address = recover_address(&statement_hash, &instruction.signature)
        .map_err(|error| format!("fault signature does not recover: {error}"))?;
    let peer = store::bridge_key_owners::get(world, &address)
        .cloned()
        .ok_or_else(|| "fault signer is not a registered bridge key".to_owned())?;
    // Every height below the executing height is certified and can never revert, so its canonical
    // statement digest exists iff a subject exists there.
    let faulty = statement.height >= current_height
        || digests
            .statement_digest(statement.height)
            .is_none_or(|canonical| canonical != statement_hash);
    if !faulty {
        return Err(format!(
            "the statement of height {} is canonical",
            statement.height
        ));
    }
    if store::attestation_faults::contains(world, &(address, statement.height)) {
        return Err(FAULT_ALREADY_RECORDED.into());
    }
    Ok(CheckedFault {
        address,
        peer,
        height: statement.height,
        statement_hash,
    })
}

/// Execute `SubmitSccpAttestationFaultV1` (§4.11).
///
/// Records the fault under `(address, height)`, marks the key faulted wherever the peer's
/// state holds it, bars the peer with this newest fault and emits `SccpAttestationFault`.
///
/// # Errors
///
/// Fails when the evidence does not verify, the statement is canonical, or the fault is
/// already recorded ([`FAULT_ALREADY_RECORDED`]).
pub fn execute_submit_fault(
    instruction: SubmitSccpAttestationFaultV1,
    _authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let current_height = state_transaction._curr_block.height().get();
    let CheckedFault {
        address,
        peer,
        height,
        statement_hash,
    } = check(
        &*state_transaction.world,
        &*state_transaction,
        current_height,
        &instruction,
    )
    .map_err(|reason| Error::InvariantViolation(format!("SCCP fault: {reason}").into()))?;
    store::attestation_faults::insert(
        state_transaction,
        (address, height),
        SccpAttestationFaultRecordV1 {
            peer: peer.clone(),
            statement_hash,
            reported_at_height: current_height,
        },
    )?;
    let mut state = store::bridge_keys::get(&*state_transaction.world, &peer)
        .cloned()
        .ok_or_else(|| {
            Error::InvariantViolation(
                format!("SCCP fault: bridge-key owner {peer} has no key state").into(),
            )
        })?;
    for key in state
        .active
        .iter_mut()
        .chain(state.pending.iter_mut())
        .chain(state.retired.iter_mut())
        .filter(|key| key.address == address)
    {
        key.faulted = true;
    }
    state.barred = Some(SccpFaultRefV1 { address, height });
    store::bridge_keys::insert(state_transaction, peer.clone(), state)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::AttestationFault(SccpAttestationFaultV1 {
            peer,
            address,
            height,
            statement_hash,
            reported_at_height: current_height,
        })));
    Ok(())
}

/// Pre-verify fault evidence against committed state at `next_block_height` and return its
/// admission keys (deduplicated by `(address, height)`).
///
/// # Errors
///
/// Rejects evidence that fails §4.11 validation or names an already recorded fault.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    digests: &(impl SccpStatementDigests + ?Sized),
    next_block_height: u64,
    instruction: &SubmitSccpAttestationFaultV1,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let checked = check(world, digests, next_block_height, instruction)
        .map_err(SccpAdmissionRejectV1::new)?;
    Ok(SccpAdmissionKeysV1::new(SccpExemptClassV1::Fault)
        .with_content(&(checked.address, checked.height)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header, peer};
    use iroha_data_model::sccp::{
        attestation::{SccpAttestationStatementV1, SccpAttestationSubjectV1},
        keys::{SccpBridgeKeyStateV1, SccpBridgeKeyV1},
        params::SccpParametersV1,
    };
    use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;

    /// Digests of a chain whose only subject is height 5 with digest `canonical`.
    struct Digests {
        canonical: Option<[u8; 32]>,
    }
    impl SccpStatementDigests for Digests {
        fn statement_digest(&self, height: u64) -> Option<[u8; 32]> {
            (height == 5).then_some(self.canonical).flatten()
        }
        fn taira_network_id(&self) -> iroha_data_model::NetworkId {
            network_id()
        }
        fn committed_time_ms(&self) -> u64 {
            0
        }
    }

    fn network_id() -> iroha_data_model::NetworkId {
        iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([7; 32])),
        )
    }

    fn statement(height: u64, sccp_root: [u8; 32]) -> SccpAttestationStatementV1 {
        SccpAttestationSubjectV1 {
            height,
            epoch: 0,
            timestamp_ms: height * 4_000,
            sccp_root,
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            generation: 1,
            roster_digest: [5; 32],
            next_roster_digest: [0; 32],
        }
        .statement([9; 32])
    }

    fn signed(
        key: &SccpBridgeKeyFileV1,
        network_id: &iroha_data_model::NetworkId,
        statement: SccpAttestationStatementV1,
    ) -> SubmitSccpAttestationFaultV1 {
        let digest = fields(&statement).digest(network_id.as_bytes());
        SubmitSccpAttestationFaultV1 {
            statement,
            signature: key.sign_digest(&digest).expect("sign"),
        }
    }

    /// Register `key` as the active key of `peer(1)`.
    fn register(stx: &mut StateTransaction<'_, '_>, key: &SccpBridgeKeyFileV1) {
        store::parameters::set(stx, Some(SccpParametersV1::taira_default()));
        let bridge_key = SccpBridgeKeyV1 {
            public_key: key.public_key().expect("public key"),
            address: key.address().expect("address"),
            activation_epoch: 0,
            registered_at_height: 1,
            faulted: false,
        };
        let state = SccpBridgeKeyStateV1 {
            active: Some(bridge_key),
            ..SccpBridgeKeyStateV1::default()
        };
        store::bridge_keys::insert(stx, peer(1), state).expect("state");
        store::bridge_key_owners::insert(stx, key.address().expect("address"), peer(1))
            .expect("owner");
    }

    #[test]
    fn only_faulty_statements_of_registered_keys_are_evidence() {
        let state = blank_state();
        let mut block = state.block(header(9));
        let mut stx = block.transaction();
        let key = SccpBridgeKeyFileV1::new([3; 32], 0).expect("key");
        let network_id = network_id();
        let canonical = statement(5, [1; 32]);
        let digests = Digests {
            canonical: Some(fields(&canonical).digest(network_id.as_bytes())),
        };
        let evidence = signed(&key, &network_id, statement(5, [2; 32]));
        let error = check(&*stx.world, &digests, 9, &evidence).expect_err("no SCCP");
        assert!(error.contains("does not exist"), "{error}");
        register(&mut stx, &key);

        // A differing field, a missing subject and a future height are all faulty.
        for evidence in [
            signed(&key, &network_id, statement(5, [2; 32])),
            signed(&key, &network_id, statement(4, [1; 32])),
            signed(&key, &network_id, statement(9, [1; 32])),
        ] {
            let checked = check(&*stx.world, &digests, 9, &evidence).expect("faulty");
            assert_eq!(checked.address, key.address().expect("address"));
            assert_eq!(checked.peer, peer(1));
        }
        // The canonical statement is not.
        let honest = signed(&key, &network_id, canonical);
        let error = check(&*stx.world, &digests, 9, &honest).expect_err("canonical");
        assert!(error.contains("canonical"), "{error}");
        // An unregistered signer is not evidence.
        let stranger = SccpBridgeKeyFileV1::new([4; 32], 0).expect("key");
        let error = check(
            &*stx.world,
            &digests,
            9,
            &signed(&stranger, &network_id, statement(5, [2; 32])),
        )
        .expect_err("unregistered");
        assert!(error.contains("not a registered bridge key"), "{error}");
    }

    #[test]
    fn a_fault_is_recorded_once_bars_the_peer_and_marks_the_key() {
        let state = blank_state();
        let mut block = state.block(header(9));
        let mut stx = block.transaction();
        let key = SccpBridgeKeyFileV1::new([3; 32], 0).expect("key");
        register(&mut stx, &key);
        // No subject exists in a blank state, so any committed height is faulty.
        let network_id = stx.taira_network_id();
        let evidence = signed(&key, &network_id, statement(4, [2; 32]));
        let digest = fields(&evidence.statement).digest(network_id.as_bytes());
        let address = key.address().expect("address");
        let keys = preverify(&*stx.world, &stx, 9, &evidence).expect("admissible");
        assert_eq!(keys.class, SccpExemptClassV1::Fault);
        assert_eq!(keys.content.len(), 1);

        execute_submit_fault(evidence.clone(), &authority(2), &mut stx).expect("record");
        let record = store::attestation_faults::get(&*stx.world, &(address, 4)).expect("fault");
        assert_eq!(record.peer, peer(1));
        assert_eq!(record.statement_hash, digest);
        assert_eq!(record.reported_at_height, 9);
        let state = store::bridge_keys::get(&*stx.world, &peer(1)).expect("state");
        assert!(state.active.expect("active").faulted);
        assert_eq!(state.barred, Some(SccpFaultRefV1 { address, height: 4 }));

        let error =
            execute_submit_fault(evidence.clone(), &authority(2), &mut stx).expect_err("duplicate");
        assert!(
            error.to_string().contains(FAULT_ALREADY_RECORDED),
            "{error}"
        );
        let reject = preverify(&*stx.world, &stx, 9, &evidence).expect_err("duplicate");
        assert!(reject.reason.contains(FAULT_ALREADY_RECORDED), "{reject}");
    }
}
