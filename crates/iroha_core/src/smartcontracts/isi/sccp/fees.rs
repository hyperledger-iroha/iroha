//! Fee exemption of SCCP transactions on success (`specs/sccp.md` §4.19). Owner: ws31.
//!
//! Attestations, fault evidence, keeper advances, recipient self-claims and bridge-key
//! registration are fee-exempt on success and charged the ordinary fee on failure. The
//! executor consults [`exempt_on_success`] where the Nexus fee is charged.
//!
//! Only a payload with an exempt shape ([`super::admission::exempt_shape`]) can be exempt, so
//! the per-block cap, which counts shapes, counts every exempt transaction of a block.

use super::{admission, params};
use crate::state::WorldReadOnly;
use iroha_data_model::transaction::TransactionPayload;

/// Return whether the transaction with `payload` is exempt from the Nexus fee when it succeeds.
#[must_use]
pub fn exempt_on_success(
    world: &(impl WorldReadOnly + ?Sized),
    payload: &TransactionPayload,
) -> bool {
    if !params::exists(world) || admission::exempt_shape(payload).is_none() {
        return false;
    }
    // TODO(ws31): the state conditions of §4.19 for the shape (for example a key binding only
    // once per peer per epoch, a self-claim only by the payload's recipient), charging the
    // ordinary fee on failure.
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, sample_signed_transaction};

    #[test]
    fn no_transaction_is_exempt_until_implemented() {
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
}
