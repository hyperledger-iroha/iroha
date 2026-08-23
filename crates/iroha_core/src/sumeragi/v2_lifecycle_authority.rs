//! Verified height authority for lifecycle scheduler episodes and rollover.
use super::schema;
use crate::sumeragi::v2::VerifiedHeightContext;
#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::DurableCertifiedServeNegativeReceipt;
use iroha_config::parameters::actual::{SumeragiV2Config, sumeragi_v2_lifecycle_capacity_geometry};
use iroha_crypto::Hash;
use norito::codec::Encode;
use schema::{
    CapacityClass, CapacityGeometry, LifecycleContext, LifecycleDigest, LifecycleKey,
    PhysicalSlotId, SchedulerEpisodeUniverse,
};
#[cfg(test)]
use std::path::PathBuf;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};
const ROSTER_IDENTITY_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:roster-identity:v1";

#[derive(Debug)]
struct PendingLifecycleOrdinalRange {
    seal: Arc<()>,
    first: u128,
    last: u128,
    successor: u128,
}

#[derive(Debug)]
struct LifecycleOrdinalAuthorityState {
    next: Option<u128>,
    pending: Option<PendingLifecycleOrdinalRange>,
}

#[derive(Debug)]
struct SharedLifecycleOrdinalAuthority {
    state: Mutex<LifecycleOrdinalAuthorityState>,
    durable_publication: Condvar,
}

impl SharedLifecycleOrdinalAuthority {
    fn after_high_watermark(high_watermark: u128) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(LifecycleOrdinalAuthorityState {
                next: high_watermark.checked_add(1),
                pending: None,
            }),
            durable_publication: Condvar::new(),
        })
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, LifecycleOrdinalAuthorityState>, String> {
        self.state
            .lock()
            .map_err(|_| "Sumeragi v2 lifecycle ordinal authority was poisoned".to_owned())
    }

    fn wait_for_durable_publication<'a>(
        &self,
        mut state: MutexGuard<'a, LifecycleOrdinalAuthorityState>,
    ) -> Result<MutexGuard<'a, LifecycleOrdinalAuthorityState>, String> {
        while state.pending.is_some() {
            state = self
                .durable_publication
                .wait(state)
                .map_err(|_| "Sumeragi v2 lifecycle ordinal authority was poisoned".to_owned())?;
        }
        Ok(state)
    }

    fn prospective_range(
        next: Option<u128>,
        count: usize,
    ) -> Result<(Option<u128>, Option<u128>), String> {
        if count == 0 {
            return Ok((None, next));
        }
        let first = next.ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        let offset = u128::try_from(count - 1)
            .map_err(|_| "Sumeragi v2 lifecycle admission range is not representable".to_owned())?;
        let last = first.checked_add(offset).ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        let successor = last.checked_add(1).ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        Ok((Some(first), Some(successor)))
    }
}

/// Restricted runtime/fair-ingress handle for actor-global scheduler ordinals.
#[derive(Clone, Debug)]
pub(in crate::sumeragi) struct RuntimeLifecycleOrdinalAuthority {
    shared: Arc<SharedLifecycleOrdinalAuthority>,
}

impl RuntimeLifecycleOrdinalAuthority {
    /// Reserve a range and advance only after the local owner commits.
    pub(in crate::sumeragi) fn with_checked_reservation<T, E>(
        &self,
        count: usize,
        commit: impl FnOnce(u128, u128) -> Result<T, E>,
    ) -> Result<Result<T, E>, String> {
        let state = self.shared.lock_state()?;
        let mut state = self.shared.wait_for_durable_publication(state)?;
        let (first, successor) =
            SharedLifecycleOrdinalAuthority::prospective_range(state.next, count)?;
        let first = first.ok_or_else(|| {
            "Sumeragi v2 lifecycle ordinal reservation must contain an owner".to_owned()
        })?;
        let successor = successor.ok_or_else(|| {
            "Sumeragi v2 lifecycle ordinal reservation lost its successor".to_owned()
        })?;
        let committed = commit(first, successor);
        if committed.is_ok() {
            state.next = Some(successor);
        }
        Ok(committed)
    }

    /// Inspect the current cursor while excluding a pending durable publication.
    pub(in crate::sumeragi) fn with_checked_current<T, E>(
        &self,
        inspect: impl FnOnce(u128) -> Result<T, E>,
    ) -> Result<Result<T, E>, String> {
        let state = self.shared.lock_state()?;
        let state = self.shared.wait_for_durable_publication(state)?;
        let current = state.next.ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        Ok(inspect(current))
    }

    /// Reserve one or more actor-global ordinals immediately.
    pub(in crate::sumeragi) fn reserve_range(
        &self,
        count: usize,
    ) -> Result<(Option<u128>, Option<u128>), String> {
        let state = self.shared.lock_state()?;
        let mut state = self.shared.wait_for_durable_publication(state)?;
        let reserved = SharedLifecycleOrdinalAuthority::prospective_range(state.next, count)?;
        if count != 0 {
            state.next = reserved.1;
        }
        Ok(reserved)
    }

    /// Advance past a restored durable high-watermark before live ingress opens.
    pub(in crate::sumeragi) fn advance_past(&self, high_watermark: u128) -> Result<(), String> {
        let mut state = self.shared.lock_state()?;
        if state.pending.is_some() {
            return Err(
                "Sumeragi v2 lifecycle ordinal restoration crossed a pending publication"
                    .to_owned(),
            );
        }
        match state.next {
            None => {
                Err("Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned())
            }
            Some(candidate) if candidate <= high_watermark => {
                state.next = Some(high_watermark.checked_add(1).ok_or_else(|| {
                    "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
                })?);
                Ok(())
            }
            Some(_) => Ok(()),
        }
    }

    /// Read the next unused committed ordinal without reserving it.
    pub(in crate::sumeragi) fn next_ordinal(&self) -> Result<Option<u128>, String> {
        self.shared.lock_state().map(|state| state.next)
    }

    /// Test whether an ordinal precedes the committed actor-global cursor.
    pub(in crate::sumeragi) fn recognizes_minted(&self, ordinal: u128) -> Result<bool, String> {
        if ordinal == 0 {
            return Ok(false);
        }
        self.shared
            .lock_state()
            .map(|state| state.next.is_some_and(|next| ordinal < next))
    }
}

/// Restricted coordinator handle for reserving durable scheduler ordinals.
#[derive(Clone, Debug)]
pub(super) struct CoordinatorLifecycleOrdinalAuthority {
    shared: Arc<SharedLifecycleOrdinalAuthority>,
}

/// Failure to create a coordinator-owned durable ordinal reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableLifecycleOrdinalReservationError {
    /// No complete range and following cursor remain representable.
    Exhausted,
    /// Shared authority was poisoned or already retained another publication.
    Invariant,
}

impl CoordinatorLifecycleOrdinalAuthority {
    /// Verify that launch restoration placed the shared cursor after the ledger.
    pub(super) fn recognizes_high_water(&self, high_water: u128) -> Result<bool, String> {
        self.shared.lock_state().map(|state| {
            state.pending.is_none() && state.next.is_some_and(|next| next > high_water)
        })
    }

    /// Reserve a still-unpublished range strictly after both shared and ledger cuts.
    pub(super) fn begin_durable_range(
        &self,
        coordinator_high_water: u128,
        count: usize,
    ) -> Result<DurableLifecycleOrdinalReservation, DurableLifecycleOrdinalReservationError> {
        if count == 0 {
            return Err(DurableLifecycleOrdinalReservationError::Invariant);
        }
        let mut state = self
            .shared
            .lock_state()
            .map_err(|_| DurableLifecycleOrdinalReservationError::Invariant)?;
        if state.pending.is_some() {
            return Err(DurableLifecycleOrdinalReservationError::Invariant);
        }
        let ledger_successor = coordinator_high_water
            .checked_add(1)
            .ok_or(DurableLifecycleOrdinalReservationError::Exhausted)?;
        let source_next = state
            .next
            .ok_or(DurableLifecycleOrdinalReservationError::Exhausted)?;
        let first = source_next.max(ledger_successor);
        let (first, successor) =
            SharedLifecycleOrdinalAuthority::prospective_range(Some(first), count)
                .map_err(|_| DurableLifecycleOrdinalReservationError::Exhausted)?;
        let first = first.expect("non-empty durable reservation has a first ordinal");
        let successor = successor.expect("non-empty durable reservation has a successor");
        let last = successor - 1;
        let seal = Arc::new(());
        state.pending = Some(PendingLifecycleOrdinalRange {
            seal: Arc::clone(&seal),
            first,
            last,
            successor,
        });
        Ok(DurableLifecycleOrdinalReservation {
            shared: Arc::clone(&self.shared),
            seal,
            first,
            last,
            successor,
            committed: AtomicBool::new(false),
        })
    }
}

/// Affine coordinator reservation committed only after its LedgerV1 successor is durable.
#[derive(Debug)]
pub(super) struct DurableLifecycleOrdinalReservation {
    shared: Arc<SharedLifecycleOrdinalAuthority>,
    seal: Arc<()>,
    first: u128,
    last: u128,
    successor: u128,
    committed: AtomicBool,
}

impl DurableLifecycleOrdinalReservation {
    /// First ordinal in the pending contiguous range.
    pub(super) const fn first(&self) -> u128 {
        self.first
    }

    /// Last ordinal in the pending contiguous range.
    pub(super) const fn last(&self) -> u128 {
        self.last
    }

    /// Publish the range to runtime/fair ingress after LedgerV1 fsync.
    pub(super) fn commit_after_durable_publication(&self) -> Result<(), String> {
        if self.committed.load(Ordering::Acquire) {
            return Err("durable lifecycle ordinal reservation was already committed".to_owned());
        }
        let mut state = self.shared.lock_state()?;
        let Some(pending) = state.pending.as_ref() else {
            return Err("durable lifecycle ordinal reservation lost its pending fence".to_owned());
        };
        if !Arc::ptr_eq(&pending.seal, &self.seal)
            || pending.first != self.first
            || pending.last != self.last
            || pending.successor != self.successor
        {
            return Err("durable lifecycle ordinal reservation changed before commit".to_owned());
        }
        state.next = Some(self.successor);
        state.pending = None;
        self.committed.store(true, Ordering::Release);
        drop(state);
        self.shared.durable_publication.notify_all();
        Ok(())
    }
}

impl Drop for DurableLifecycleOrdinalReservation {
    fn drop(&mut self) {
        if self.committed.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut state) = self.shared.lock_state() else {
            return;
        };
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| Arc::ptr_eq(&pending.seal, &self.seal))
        {
            state.pending = None;
            drop(state);
            self.shared.durable_publication.notify_all();
        }
    }
}

/// Construct paired restricted views over one actor-global ordinal namespace.
pub(super) fn lifecycle_ordinal_authorities_after_high_watermark(
    high_watermark: u128,
) -> (
    RuntimeLifecycleOrdinalAuthority,
    CoordinatorLifecycleOrdinalAuthority,
) {
    let shared = SharedLifecycleOrdinalAuthority::after_high_watermark(high_watermark);
    (
        RuntimeLifecycleOrdinalAuthority {
            shared: Arc::clone(&shared),
        },
        CoordinatorLifecycleOrdinalAuthority { shared },
    )
}

/// Construct a standalone runtime view for focused runtime owners and tests.
pub(in crate::sumeragi) fn runtime_lifecycle_ordinal_authority_after_high_watermark(
    high_watermark: u128,
) -> RuntimeLifecycleOrdinalAuthority {
    lifecycle_ordinal_authorities_after_high_watermark(high_watermark).0
}
/// Opaque, verified source of every scheduler-episode universe for one height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AuthenticatedEpisodeAuthority {
    context: LifecycleContext,
    ordered_roster: Vec<LifecycleDigest>,
    leader_start: usize,
    capacity_geometry: CapacityGeometry,
}
impl AuthenticatedEpisodeAuthority {
    /// Derive authority exclusively from a cryptographically verified height context.
    fn from_verified_height_context(
        verified: &VerifiedHeightContext,
        capacity_geometry: CapacityGeometry,
    ) -> Option<Self> {
        let wire_context = verified.context();
        let context_hash = wire_context.id().0;
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context_hash.as_ref());
        let context = LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height);
        let ordered_roster = wire_context.roster.iter().map(|validator| {
            let encoded = validator.validator.encode();
            let mut preimage = Vec::with_capacity(ROSTER_IDENTITY_DOMAIN.len() + encoded.len());
            preimage.extend_from_slice(ROSTER_IDENTITY_DOMAIN);
            preimage.extend_from_slice(&encoded);
            let hash = Hash::new(preimage);
            let mut digest = [0_u8; 32];
            digest.copy_from_slice(hash.as_ref());
            LifecycleDigest::new(digest)
        });
        let leader_start = usize::try_from(wire_context.leader(0)).ok()?;
        Self::from_authenticated_parts(context, ordered_roster, leader_start, capacity_geometry)
    }
    #[cfg(test)]
    pub(super) fn test(
        context: LifecycleContext,
        ordered_roster: impl IntoIterator<Item = LifecycleDigest>,
        leader_start: usize,
        capacity_geometry: CapacityGeometry,
    ) -> Option<Self> {
        Self::from_authenticated_parts(context, ordered_roster, leader_start, capacity_geometry)
    }
    fn from_authenticated_parts(
        context: LifecycleContext,
        ordered_roster: impl IntoIterator<Item = LifecycleDigest>,
        leader_start: usize,
        capacity_geometry: CapacityGeometry,
    ) -> Option<Self> {
        let ordered_roster: Vec<_> = ordered_roster.into_iter().collect();
        let roster_is_finite = u16::try_from(ordered_roster.len()).is_ok();
        let roster_is_unique = ordered_roster
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            == ordered_roster.len();
        let capacity_is_finite = CapacityClass::ALL.iter().all(|class| {
            capacity_geometry
                .limits
                .get(class)
                .is_some_and(|limit| *limit <= usize::from(u16::MAX) + 1)
        });
        (!ordered_roster.is_empty()
            && roster_is_finite
            && roster_is_unique
            && capacity_is_finite
            && leader_start < ordered_roster.len())
        .then_some(Self {
            context,
            ordered_roster,
            leader_start,
            capacity_geometry,
        })
    }
    pub(super) const fn context(&self) -> LifecycleContext {
        self.context
    }
    pub(super) const fn capacity_geometry(&self) -> &CapacityGeometry {
        &self.capacity_geometry
    }
    pub(super) fn universe_for(&self, key: LifecycleKey) -> Option<SchedulerEpisodeUniverse> {
        if key.context != self.context.id
            || key.round.height != self.context.height
            || key
                .proposal_round
                .is_some_and(|round| round.height != self.context.height)
        {
            return None;
        }
        let target = key.scheduler_target();
        let roster_len = u64::try_from(self.ordered_roster.len()).ok()?;
        let view_offset = usize::try_from(key.round.view % roster_len).ok()?;
        let leader =
            self.ordered_roster[(self.leader_start + view_offset) % self.ordered_roster.len()];
        let roster_len = u16::try_from(self.ordered_roster.len()).ok()?;
        Some(SchedulerEpisodeUniverse {
            target,
            context: self.context.id,
            leader,
            view: key.round.view,
            subject: key.subject,
            phase: key.phase,
            authenticated_roster_slots: (0..roster_len).collect(),
            capacity_geometry: self.capacity_geometry.limits.clone(),
        })
    }
    pub(super) fn admits_slots(
        &self,
        class: CapacityClass,
        slots: &BTreeSet<PhysicalSlotId>,
    ) -> bool {
        slots.iter().all(|slot| {
            slot.capacity_class() == Some(class)
                && usize::from(slot.index()) < self.capacity_geometry.limit(class)
        })
    }
}
fn capacity_geometry_from_limits(
    roster_len: usize,
    effect_work_capacity: usize,
    certified_request_capacity: usize,
    authenticated_non_validator_source_capacity: usize,
) -> Option<CapacityGeometry> {
    let shared = sumeragi_v2_lifecycle_capacity_geometry(
        roster_len,
        effect_work_capacity,
        certified_request_capacity,
        authenticated_non_validator_source_capacity,
    )
    .ok()?;
    if effect_work_capacity == 0 || certified_request_capacity == 0 {
        return None;
    }
    let geometry = CapacityGeometry::new([
        (CapacityClass::Consensus, shared.consensus),
        (CapacityClass::Effect, shared.effect),
        (CapacityClass::Serve, shared.serve),
        (CapacityClass::Producer, shared.producer),
    ]);
    Some(geometry)
}
fn production_capacity_geometry_from_limits(
    roster_len: usize,
    effect_work_capacity: usize,
    certified_request_capacity: usize,
    authenticated_non_validator_source_capacity: usize,
    reply_route_source_capacity: usize,
) -> Option<CapacityGeometry> {
    if authenticated_non_validator_source_capacity == 0
        || authenticated_non_validator_source_capacity > reply_route_source_capacity
    {
        return None;
    }
    capacity_geometry_from_limits(
        roster_len,
        effect_work_capacity,
        certified_request_capacity,
        authenticated_non_validator_source_capacity,
    )
}
fn production_capacity_geometry(
    verified: &VerifiedHeightContext,
    config: &SumeragiV2Config,
    reply_route_source_capacity: usize,
) -> Option<CapacityGeometry> {
    production_capacity_geometry_from_limits(
        verified.context().roster.len(),
        usize::try_from(config.limits.effect_work_capacity).ok()?,
        usize::try_from(config.limits.certified_request_capacity).ok()?,
        usize::try_from(config.limits.authenticated_non_validator_source_capacity).ok()?,
        reply_route_source_capacity,
    )
}
/// Derive the complete production episode authority without accepting caller
/// supplied capacity geometry.
pub(super) fn production_authority(
    verified: &VerifiedHeightContext,
    config: &SumeragiV2Config,
    reply_route_source_capacity: usize,
) -> Option<AuthenticatedEpisodeAuthority> {
    let capacity_geometry =
        production_capacity_geometry(verified, config, reply_route_source_capacity)?;
    AuthenticatedEpisodeAuthority::from_verified_height_context(verified, capacity_geometry)
}
/// Build the smallest exact authority needed by focused recovered-WAL open
/// tests. Production always uses [`production_authority`] and configured
/// capacity geometry.
#[cfg(test)]
pub(super) fn recovered_wal_test_authority(
    verified: &VerifiedHeightContext,
) -> Option<AuthenticatedEpisodeAuthority> {
    let geometry = CapacityGeometry::new([
        (CapacityClass::Consensus, 1),
        (CapacityClass::Effect, 1),
        (CapacityClass::Serve, 1),
        (CapacityClass::Producer, 1),
    ]);
    AuthenticatedEpisodeAuthority::from_verified_height_context(verified, geometry)
}
/// Build exact bounded capacity for focused consuming storage-owner tests.
#[cfg(test)]
pub(super) fn lifecycle_storage_owner_test_authority(
    verified: &VerifiedHeightContext,
    effect_capacity: usize,
    serve_capacity: usize,
) -> Option<AuthenticatedEpisodeAuthority> {
    let geometry = CapacityGeometry::new([
        (CapacityClass::Consensus, 1),
        (CapacityClass::Effect, effect_capacity.max(1)),
        (CapacityClass::Serve, serve_capacity.max(1)),
        (CapacityClass::Producer, serve_capacity.max(1)),
    ]);
    AuthenticatedEpisodeAuthority::from_verified_height_context(verified, geometry)
}
/// Typed height rollover snapshot carrying an opaque verified successor authority.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RolloverSnapshot {
    pub(super) retired_context: LifecycleContext,
    pub(super) successor_context: LifecycleContext,
    pub(super) successor_predecessor: LifecycleDigest,
    pub(super) successor_authority: AuthenticatedEpisodeAuthority,
    pub(super) successor_ledger_root: Option<PathBuf>,
    pub(super) serve_cancellations: Vec<DurableCertifiedServeNegativeReceipt>,
    pub(super) retained_high_water: u128,
    pub(super) retire_ordinals: BTreeSet<u128>,
    pub(super) retire_admission_keys: BTreeSet<LifecycleKey>,
}
#[cfg(test)]
pub(super) fn test_authority(
    context: LifecycleContext,
    ordered_roster: impl IntoIterator<Item = LifecycleDigest>,
    leader_start: usize,
    capacity_geometry: CapacityGeometry,
) -> Option<AuthenticatedEpisodeAuthority> {
    AuthenticatedEpisodeAuthority::test(context, ordered_roster, leader_start, capacity_geometry)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_capacity_geometry_matches_shared_runtime_resources() {
        let geometry = capacity_geometry_from_limits(4, 8, 3, 2)
            .expect("bounded production limits produce finite geometry");
        assert_eq!(geometry.limit(CapacityClass::Consensus), 16);
        assert_eq!(geometry.limit(CapacityClass::Effect), 8);
        assert_eq!(geometry.limit(CapacityClass::Serve), 20);
        assert_eq!(geometry.limit(CapacityClass::Producer), 20);
        assert!(capacity_geometry_from_limits(4, 0, 3, 2).is_none());
        assert!(capacity_geometry_from_limits(4, 8, 1, 32_768).is_none());
        assert!(
            capacity_geometry_from_limits(4, 256, 163, 120).is_none(),
            "reply-route capacity must not be reused as the authenticated-source owner bound"
        );
        let large_source_geometry = capacity_geometry_from_limits(4, 256, 163, 32)
            .expect("a large explicit authenticated-source bound remains representable");
        assert_eq!(large_source_geometry.limit(CapacityClass::Serve), 10_440);
        assert_eq!(large_source_geometry.limit(CapacityClass::Producer), 10_440);
    }

    #[test]
    fn four_validator_geometry_uses_authenticated_ingress_source_bound() {
        let geometry = production_capacity_geometry_from_limits(4, 256, 512, 2, 120)
            .expect("four-validator integration geometry is representable");
        assert_eq!(geometry.limit(CapacityClass::Consensus), 16);
        assert_eq!(geometry.limit(CapacityClass::Effect), 256);
        assert_eq!(geometry.limit(CapacityClass::Serve), 2_056);
        assert_eq!(geometry.limit(CapacityClass::Producer), 2_056);
        assert_eq!(
            CapacityClass::ALL
                .into_iter()
                .map(|class| geometry.limit(class))
                .sum::<usize>(),
            4_384
        );
        assert!(production_capacity_geometry_from_limits(4, 256, 512, 121, 120).is_none());
    }
}
