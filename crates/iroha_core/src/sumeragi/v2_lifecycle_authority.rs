//! Verified height authority for lifecycle scheduler episodes and rollover.
use super::schema;
#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::DurableCertifiedServeNegativeReceipt;
use crate::sumeragi::{
    v2::VerifiedHeightContext, v2_core::MAX_EFFECTS_PER_STEP,
    v2_worker::certified_serve_family_capacity,
};
use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::Hash;
use norito::codec::Encode;
use schema::{
    CapacityClass, CapacityGeometry, LifecycleContext, LifecycleDigest, LifecycleKey,
    MAX_LIFECYCLE_RECORDS_PER_HEIGHT, PhysicalSlotId, SchedulerEpisodeUniverse,
};
use std::collections::BTreeSet;
#[cfg(test)]
use std::path::PathBuf;
const ROSTER_IDENTITY_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:roster-identity:v1";
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
    reply_route_source_capacity: usize,
) -> Option<CapacityGeometry> {
    let consensus = MAX_EFFECTS_PER_STEP.checked_mul(2)?;
    let serve = certified_serve_family_capacity(
        roster_len,
        reply_route_source_capacity.max(1),
        certified_request_capacity,
    )
    .ok()?;
    if effect_work_capacity == 0 || certified_request_capacity == 0 || consensus == 0 || serve == 0
    {
        return None;
    }
    let geometry = CapacityGeometry::new([
        (CapacityClass::Consensus, consensus),
        (CapacityClass::Effect, effect_work_capacity),
        (CapacityClass::Serve, serve),
        (CapacityClass::Producer, serve),
    ]);
    let finite_classes = CapacityClass::ALL
        .iter()
        .all(|class| geometry.limit(*class) <= usize::from(u16::MAX) + 1);
    let bounded_live_records = CapacityClass::ALL
        .iter()
        .try_fold(0_usize, |sum, class| {
            sum.checked_add(geometry.limit(*class))
        })
        .is_some_and(|sum| sum <= MAX_LIFECYCLE_RECORDS_PER_HEIGHT);
    (finite_classes && bounded_live_records).then_some(geometry)
}
fn production_capacity_geometry(
    verified: &VerifiedHeightContext,
    config: &SumeragiV2Config,
    reply_route_source_capacity: usize,
) -> Option<CapacityGeometry> {
    capacity_geometry_from_limits(
        verified.context().roster.len(),
        usize::try_from(config.limits.effect_work_capacity).ok()?,
        usize::try_from(config.limits.certified_request_capacity).ok()?,
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
    }
}
