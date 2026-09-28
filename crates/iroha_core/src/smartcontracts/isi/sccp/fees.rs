//! Fee exemption of SCCP transactions on success (`specs/sccp.md` §4.19). Owner: ws31.
//!
//! Attestations, fault evidence, keeper advances, recipient self-claims and bridge-key
//! registration are fee-exempt on success and charged the ordinary fee on failure. The
//! executor consults [`exempt_on_success`] where the Nexus fee is charged.
//!
//! Only a payload with an exempt shape ([`super::admission::exempt_shape`]) can be exempt, so
//! the per-block cap, which counts shapes, counts every exempt transaction of a block.

use super::{
    admission::{self, SccpExemptClassV1},
    bridge_keys, params, store,
};
use crate::state::WorldReadOnly;
use iroha_data_model::transaction::{Executable, TransactionPayload};

/// Return whether the transaction with `payload` is exempt from the Nexus fee when it succeeds.
#[must_use]
pub fn exempt_on_success(
    world: &(impl WorldReadOnly + ?Sized),
    payload: &TransactionPayload,
) -> bool {
    if !params::exists(world) {
        return false;
    }
    match admission::exempt_shape(payload) {
        // An attestation batch is exempt only from a registered bridge key's own account
        // (§4.8); admission pre-verified its entries against committed state.
        Some(SccpExemptClassV1::Attestation) => {
            bridge_keys::bridge_key_address_of(&payload.authority)
                .is_some_and(|address| store::bridge_key_owners::contains(world, &address))
        }
        // A key registration from the key's own account, once per peer per epoch (§4.2.3).
        Some(SccpExemptClassV1::KeyBinding) => {
            let Executable::Instructions(instructions) = &payload.instructions else {
                return false;
            };
            instructions
                .first()
                .and_then(|only| {
                    only.as_any()
                        .downcast_ref::<iroha_data_model::isi::sccp::SetSccpBridgeKeyV1>()
                })
                .is_some_and(|set| bridge_keys::binding_exempt(world, set, &payload.authority))
        }
        // Fault evidence succeeds only when it records a new fault (§4.11): a duplicate or a
        // canonical statement fails, and admission pre-verified it against committed state.
        Some(SccpExemptClassV1::Fault) => true,
        // A keeper advance is exempt from a live, unfaulted bridge key's account; admission
        // checked that it moves the head against committed state (§4.13.4).
        Some(SccpExemptClassV1::KeeperAdvance { .. }) => {
            bridge_keys::bridge_key_address_of(&payload.authority).is_some_and(|address| {
                store::bridge_key_owners::get(world, &address)
                    .and_then(|peer| store::bridge_keys::get(world, peer))
                    .is_some_and(|state| {
                        state
                            .active
                            .iter()
                            .chain(state.pending.iter())
                            .any(|key| key.address == address && !key.faulted)
                    })
            })
        }
        // A self-claim pays `inbound_self_claim_fee` from the proceeds at release; admission
        // pre-verified the recipient, the amount and the proof or pending record (§4.12.4).
        Some(SccpExemptClassV1::SelfClaim) => true,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, sample_signed_transaction};

    #[test]
    fn nothing_is_exempt_without_sccp() {
        let state = blank_state();
        let view = state.world_view();
        assert!(!exempt_on_success(
            &view,
            sample_signed_transaction().payload()
        ));
    }

    #[test]
    fn an_unshaped_payload_is_never_exempt_even_with_sccp() {
        use crate::smartcontracts::isi::sccp::{
            store,
            test_support::{SampleInstructions, header},
        };
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        store::parameters::set(
            &mut stx,
            Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default()),
        );
        let ordinary = sample_signed_transaction();
        assert_eq!(admission::exempt_shape(ordinary.payload()), None);
        assert!(!exempt_on_success(&*stx.world, ordinary.payload()));
        let mut record = ordinary.payload().clone();
        record.instructions = iroha_data_model::transaction::Executable::Instructions(
            vec![SampleInstructions::record().into()].into(),
        );
        assert!(!exempt_on_success(&*stx.world, &record));
    }

    #[test]
    fn attestations_are_exempt_only_from_registered_bridge_key_accounts() {
        use crate::smartcontracts::isi::sccp::{
            store,
            test_support::{SampleInstructions, header},
        };
        use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;

        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        store::parameters::set(
            &mut stx,
            Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default()),
        );
        let key = SccpBridgeKeyFileV1::new([7; 32], 0).expect("key");
        let account =
            bridge_keys::account_of(&key.public_key().expect("public key")).expect("account");
        let mut payload = sample_signed_transaction().payload().clone();
        payload.authority = account;
        payload.instructions = iroha_data_model::transaction::Executable::Instructions(
            vec![SampleInstructions::attestations().into()].into(),
        );
        assert!(
            !exempt_on_success(&*stx.world, &payload),
            "unregistered key"
        );
        store::bridge_key_owners::insert(
            &mut stx,
            key.address().expect("address"),
            crate::smartcontracts::isi::sccp::test_support::peer(1),
        )
        .expect("owner");
        assert!(exempt_on_success(&*stx.world, &payload));
    }

    #[test]
    fn fault_evidence_is_exempt_with_sccp_only() {
        use crate::smartcontracts::isi::sccp::{
            store,
            test_support::{SampleInstructions, header},
        };
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut payload = sample_signed_transaction().payload().clone();
        payload.instructions = iroha_data_model::transaction::Executable::Instructions(
            vec![SampleInstructions::fault().into()].into(),
        );
        assert!(!exempt_on_success(&*stx.world, &payload), "no SCCP");
        store::parameters::set(
            &mut stx,
            Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default()),
        );
        assert!(exempt_on_success(&*stx.world, &payload));
    }
}
