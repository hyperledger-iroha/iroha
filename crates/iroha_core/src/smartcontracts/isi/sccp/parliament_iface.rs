//! The one seam through which SCCP calls Parliament-owned functions (`specs/sccp.md` §4.7,
//! §10.1, §10.2; `specs/parliament_private_ballot_design.md` §11).
//!
//! SCCP reads nothing else of the Parliament's ballot: it parses no certificate internals and
//! never calls the ballot code directly. The positioned pass is Parliament's: SCCP lists its
//! rank-three and pause items into the cursor that owner begins ([`ForeignPassOwnerV1`]),
//! and reads Parliament's terms of the due predicate through [`governance_due`]. The binding
//! ballot stays unavailable until the Parliament workstream qualifies and activates it, so
//! the launch gate stays closed. Unavailable latency is never read as zero.

use crate::state::WorldReadOnly;

#[cfg(test)]
pub(crate) use crate::governance::parliament_anonymous::world_pass::{
    NoParliamentV1, run_governance_pass,
};
pub(crate) use crate::governance::parliament_anonymous::{
    pass::GovernancePassItemV1, world_pass::ForeignPassOwnerV1,
};

/// Return whether the qualified post-quantum binding ballot is active on the chain (§10.2).
///
/// While it is false the SCCP launch gate is closed: no route is registered and the genesis
/// parameters hold, with no interim governance.
///
/// TODO: Parliament workstream (design §12.3 slices 4 and 5): this stays `false` until the
/// ballot profile is qualified, signerless execution exists and a production attempt can be
/// certified. The positioned pass alone does not make the ballot available.
#[must_use]
pub const fn parliament_binding_ballot_available() -> bool {
    false
}

/// Return the worst-case latency of one full-track SCCP Parliament attempt to a terminal
/// outcome, in milliseconds, or `None` while no qualified bound exists (§4.1, §10.1).
///
/// `None` never means zero: the lower bound of `fast_pause_hold_ms` is deferred, not met.
///
/// TODO: Parliament workstream (`iroha_core::governance::parliament::sccp_interface`): the
/// stub returns `None` until the committed ballot windows give a qualified bound.
#[must_use]
pub const fn parliament_sccp_attempt_latency_ms() -> Option<u64> {
    None
}

/// Parliament's terms of `governance_due(parent, h, t)` (`specs/sccp.md` §4.7.3) on the
/// committed parent `world`, whose block time is `committed_time_ms`: a casting clock, an
/// exact-slot draw, the height-progress ticket, a lifecycle item, a closure or an opening
/// with a free casting slot is due at height `next_height` and time `time_ms`.
///
/// Parliament's items run only in the pass of the global chain, so a root whose committed
/// scope is not global has no Parliament due work.
///
/// # Errors
/// As [`governance_due_on_global_root`].
pub(crate) fn governance_due(
    world: &impl WorldReadOnly,
    committed_time_ms: Option<u64>,
    next_height: u64,
    time_ms: u64,
) -> Result<bool, super::keepalive::DueWorkUnavailable> {
    if crate::sumeragi::lanes::routing::committed_root_scope(world)
        != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
    {
        return Ok(false);
    }
    governance_due_on_global_root(world, committed_time_ms, next_height, time_ms)
}

/// [`governance_due`] on the global chain's committed parent `world`.
///
/// A root whose selection does not install the anonymous family has no Parliament item:
/// the answer is `false`, never unavailability. A generation awaiting a slot below
/// `next_height` counts once that slot's pulse is committed. One awaiting the slot of
/// `next_height` itself counts when consensus requires the native pulse at that height, so
/// every valid candidate block carries it (K4); a slot that is no native pulse height never
/// gets a pulse and is no due work.
///
/// # Errors
/// [`super::keepalive::DueWorkUnavailable`] when the selection or the Parliament tables
/// cannot be read as a due set: unavailability is never an empty due set.
pub(crate) fn governance_due_on_global_root(
    world: &impl WorldReadOnly,
    committed_time_ms: Option<u64>,
    next_height: u64,
    time_ms: u64,
) -> Result<bool, super::keepalive::DueWorkUnavailable> {
    let Some(pulse_available) = pulse_availability(world, next_height)? else {
        return Ok(false);
    };
    crate::governance::parliament_anonymous::world_pass::parliament_due(
        world,
        next_height,
        time_ms,
        committed_time_ms,
        pulse_available,
    )
    .map_err(|_| super::keepalive::DueWorkUnavailable)
}

/// The least block time at which [`governance_due`] holds for the candidate block at
/// `next_height` on the committed parent `world`, whose block time is `committed_time_ms`:
/// Parliament's next-governance-progress time (design §9). `Some(0)` is work due at any
/// time; `None` means that only a committed pulse or a transaction can make Parliament work
/// due, which is never an invented deadline. A root that is not the global chain, or whose
/// selection does not install the family, has none.
///
/// # Errors
/// As [`governance_due_on_global_root`].
pub(crate) fn next_governance_progress_at_ms(
    world: &impl WorldReadOnly,
    committed_time_ms: Option<u64>,
    next_height: u64,
) -> Result<Option<u64>, super::keepalive::DueWorkUnavailable> {
    if crate::sumeragi::lanes::routing::committed_root_scope(world)
        != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
    {
        return Ok(None);
    }
    next_governance_progress_on_global_root(world, committed_time_ms, next_height)
}

/// [`next_governance_progress_at_ms`] on the global chain's committed parent `world`: the
/// least time at which [`governance_due_on_global_root`] holds for `next_height`.
///
/// # Errors
/// As [`governance_due_on_global_root`].
pub(crate) fn next_governance_progress_on_global_root(
    world: &impl WorldReadOnly,
    committed_time_ms: Option<u64>,
    next_height: u64,
) -> Result<Option<u64>, super::keepalive::DueWorkUnavailable> {
    let Some(pulse_available) = pulse_availability(world, next_height)? else {
        return Ok(None);
    };
    crate::governance::parliament_anonymous::world_pass::parliament_next_due_ms(
        world,
        next_height,
        committed_time_ms,
        pulse_available,
    )
    .map_err(|_| super::keepalive::DueWorkUnavailable)
}

/// Parliament's view of an exact pulse slot for the candidate block at `next_height` on the
/// global chain's committed parent `world`, or `None` when its selection does not install
/// the family. A slot below `next_height` is available once its pulse is committed; the slot
/// of `next_height` itself when consensus requires the native pulse at that height (K4).
fn pulse_availability<'world, W: WorldReadOnly>(
    world: &'world W,
    next_height: u64,
) -> Result<Option<impl Fn(u64) -> bool + 'world>, super::keepalive::DueWorkUnavailable> {
    use mv::storage::StorageReadOnly as _;

    // SPEC: §13.4 HC804 — a root without the family has no Parliament due work.
    let installed = crate::runtime::parliament_anonymous_installed(world.runtime_selection())
        .map_err(|_| super::keepalive::DueWorkUnavailable)?
        || cfg!(all(test, sumeragi_core_mutation = "HC804"));
    if !installed {
        return Ok(None);
    }
    let carried = world
        .consensus_schedule()
        .ready(next_height)
        .ok()
        .is_some_and(|config| {
            crate::sumeragi::epoch_beacon::required(
                iroha_data_model::block::consensus::SumeragiRootScope::Global,
                world,
                &config.epoch,
                next_height,
            ) == Ok(true)
        });
    // One chain has one logical beacon session: every committed pulse names its network.
    let session = world
        .global_beacon_pulses()
        .iter()
        .next()
        .map(|(_, pulse)| {
            iroha_data_model::governance::types::BeaconSessionId::for_network_v1(&pulse.network_id)
        });
    Ok(Some(move |slot| {
        (slot == next_height && carried)
            || session.is_some_and(|session| {
                world
                    .global_beacon_pulse_slots()
                    .get(&(session, slot))
                    .is_some()
            })
    }))
}

/// Borrowed original parent draw. The external owner enforces the 8 MiB / 65,536
/// citizen ceiling and samples without replacement; fields cannot be asserted by callers.
pub(super) struct PauseDraw<'a, 'world> {
    source: &'a crate::state::WorldBlock<'world>,
    pulse_id: [u8; 32],
    sequence: u64,
    seed: [u8; 32],
    pub(super) members: [Option<&'a iroha_data_model::account::AccountId>; 27],
    pub(super) count: usize,
    pub(super) unavailable:
        Option<iroha_data_model::sccp::events::SccpPausePanelUnavailableReasonV1>,
}
impl<'world> PauseDraw<'_, 'world> {
    pub(super) fn matches(
        &self,
        world: &crate::state::WorldBlock<'world>,
        pulse: [u8; 32],
        sequence: u64,
        seed: [u8; 32],
    ) -> bool {
        core::ptr::eq(self.source, world)
            && self.pulse_id == pulse
            && self.sequence == sequence
            && self.seed == seed
    }
}
/// TODO: supply the Parliament-owned canonical parent citizen snapshot and existing draw.
/// Unavailable dependency is a governance-local wait, not an empty candidate population.
pub(super) fn pause_draw<'a, 'world>(
    _: &'a crate::state::WorldBlock<'world>,
    _: [u8; 32],
    _: u64,
    _: [u8; 32],
    _: usize,
) -> Result<PauseDraw<'a, 'world>, super::keepalive::DueWorkUnavailable> {
    Err(super::keepalive::DueWorkUnavailable)
}

/// Eligibility for exactly one borrowed World, panel graph and observation time.
/// Only the external Parliament owner may issue it; no DATA constructor exists.
pub(super) struct PauseEligibility<'a, W: crate::state::WorldReadOnly + ?Sized> {
    world: &'a W,
    panel: &'a super::pause_record::PausePanelBody,
    time: u64,
    members: u32,
}
impl<W: crate::state::WorldReadOnly + ?Sized> PauseEligibility<'_, W> {
    pub(super) fn for_original(
        &self,
        world: &W,
        panel: &super::pause_record::PausePanelBody,
        time: u64,
    ) -> Result<u32, super::Error> {
        if !core::ptr::eq(self.world, world)
            || !core::ptr::eq(self.panel, panel)
            || time != self.time
        {
            return Err(super::Error::InvariantViolation(
                "SCCP eligibility observation changed source".into(),
            ));
        }
        Ok(self.members)
    }
}
/// Read citizenship, bond, slash, suspension and active-account eligibility at the exact
/// observation. TODO: connect the Parliament-owned eligible-citizen producer once landed.
pub(super) fn pause_eligibility<'a, W: crate::state::WorldReadOnly + ?Sized>(
    _: &'a W,
    _: &'a super::pause_record::PausePanelBody,
    _: u64,
) -> Result<PauseEligibility<'a, W>, super::Error> {
    Err(super::Error::InvariantViolation(
        "SCCP Parliament citizen eligibility is unavailable".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_binding_ballot_is_unavailable_until_the_parliament_provides_it() {
        assert!(!parliament_binding_ballot_available());
    }

    #[test]
    fn the_attempt_latency_is_unavailable_and_never_zero() {
        assert_eq!(parliament_sccp_attempt_latency_ms(), None);
    }

    #[test]
    fn a_root_without_the_anonymous_family_has_no_parliament_due_work() {
        use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};
        let state = blank_state();
        let block = state.block(header(2));
        for (height, time) in [(1, 0), (2, 1_000), (u64::MAX, u64::MAX)] {
            assert_eq!(
                governance_due(&block.world, Some(0), height, time),
                Ok(false),
                "a root that does not install the anonymous family has no Parliament due work"
            );
            assert_eq!(
                next_governance_progress_at_ms(&block.world, Some(time), height),
                Ok(None),
                "nor a next-governance-progress time"
            );
        }
    }

    #[test]
    fn unavailable_pause_authorities_never_supply_an_empty_draw_or_eligibility() {
        use crate::smartcontracts::isi::sccp::test_support::{
            blank_state, header, sample_pause_panel,
        };
        let state = blank_state();
        let block = state.block(header(2));
        assert!(pause_draw(&block.world, [1; 32], 1, [2; 32], 9).is_err());
        let panel = super::super::pause_record::RetainedSccpPausePanel::admit(
            &sample_pause_panel(1),
            &state.ivm_execution_budget(),
        )
        .unwrap();
        assert!(pause_eligibility(&block.world, &panel, 8000).is_err());
    }
}
