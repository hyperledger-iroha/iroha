//! SCCP v1 consensus parameters (`specs/sccp.md` §4.1). Owner: ws31.
//!
//! `SccpParametersV1` lives in dedicated world state written only by the genesis-only
//! `InitializeSccpV1` and the Parliament-enacted `SetParameters`. SCCP exists on a network iff
//! the parameters are present. `enabled = false` stops only value-moving effects.

use super::store;
use crate::state::WorldReadOnly;
use iroha_data_model::sccp::params::SccpParametersV1;

/// Return the SCCP parameters, or `None` when SCCP does not exist on this network.
#[must_use]
pub fn parameters(world: &(impl WorldReadOnly + ?Sized)) -> Option<SccpParametersV1> {
    *store::parameters::get(world)
}

/// Return whether SCCP exists on this network (its parameters are present).
#[must_use]
pub fn exists(world: &(impl WorldReadOnly + ?Sized)) -> bool {
    store::parameters::get(world).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn absent_parameters_mean_sccp_does_not_exist() {
        let state = blank_state();
        let view = state.world_view();
        assert_eq!(parameters(&view), None);
        assert!(!exists(&view));
    }

    #[test]
    fn parameters_follow_the_stored_switch() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut stored = SccpParametersV1::taira_default();
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert_eq!(parameters(&*stx.world), Some(stored));
        assert_eq!(parameters(&*stx.world).map(|p| p.enabled), Some(true));
        stored.enabled = false;
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert_eq!(parameters(&*stx.world).map(|p| p.enabled), Some(false));
    }
}
