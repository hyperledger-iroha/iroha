//! SCCP v1 consensus parameters (`specs/sccp.md` §4.1). Owner: ws31; ws20 implemented the
//! read accessors.
//!
//! `SccpParametersV1` lives in dedicated world state written only by the genesis-only
//! `InitializeSccpV1` and the Parliament-enacted `SetParameters`. SCCP exists on a network iff
//! the parameters are present. `enabled = false` stops only value-moving effects.

use super::{Error, store};
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

/// Return whether SCCP exists and its value-moving effects are enabled (§4.1 `enabled`).
#[must_use]
pub fn enabled(world: &(impl WorldReadOnly + ?Sized)) -> bool {
    parameters(world).is_some_and(|parameters| parameters.enabled)
}

/// Return the SCCP parameters, failing when SCCP does not exist on this network.
///
/// # Errors
///
/// Fails with an invariant violation when `sccp_parameters` is absent.
pub fn require(world: &(impl WorldReadOnly + ?Sized)) -> Result<SccpParametersV1, Error> {
    parameters(world)
        .ok_or_else(|| Error::InvariantViolation("SCCP does not exist on this network".into()))
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
        assert!(!enabled(&view));
        assert!(format!("{}", require(&view).unwrap_err()).contains("does not exist"));
    }

    #[test]
    fn enabled_follows_the_stored_switch() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut stored = SccpParametersV1::taira_default();
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert!(enabled(&*stx.world));
        assert_eq!(parameters(&*stx.world), Some(stored));
        assert_eq!(require(&*stx.world), Ok(stored));
        stored.enabled = false;
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert!(!enabled(&*stx.world));
    }
}
