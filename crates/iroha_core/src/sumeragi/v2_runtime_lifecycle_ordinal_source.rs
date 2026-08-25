/// Actor-global source for immutable lifecycle admission ordinals.
///
/// Runtime FIFO admissions, fresh clock/effect roots, and the exact Serve
/// ingress gate share one source for the active height. The source stores the
/// next unused ordinal rather than an event count; a durable Serve waiter can
/// therefore seed a restarted actor past its retained high-watermark before
/// any reconstructed runtime owner is minted.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeLifecycleOrdinalSource {
    authority: RuntimeLifecycleOrdinalAuthority,
}
impl RuntimeLifecycleOrdinalSource {
    /// Construct a source strictly after a durable high-watermark.
    ///
    /// The runtime surface intentionally constructs only its restricted view;
    /// coordinator handles cannot be injected through this type. The first
    /// reservable ordinal is exactly one greater than `high_watermark`, unless
    /// the watermark exhausts the `u128` namespace.
    /// This keeps runtime callers within the runtime-restricted authority API.
    pub(crate) fn after_high_watermark(high_watermark: u128) -> Self {
        Self {
            authority: runtime_lifecycle_ordinal_authority_after_high_watermark(high_watermark),
        }
    }
    /// Wrap the runtime-restricted half of the paired production launch authority.
    pub(in crate::sumeragi) const fn from_authority(
        authority: RuntimeLifecycleOrdinalAuthority,
    ) -> Self {
        Self { authority }
    }
    /// Reserve one globally unique ordinal.
    pub(crate) fn reserve_one(&self) -> Result<u128, String> {
        self.reserve_range(1)?
            .0
            .ok_or_else(|| "Sumeragi v2 lifecycle ordinal source returned no owner".to_owned())
    }
    /// Hold the actor-global source while a prospective FIFO owner is fully
    /// checked and committed to its local ingress.
    ///
    /// The source advances only after `commit` returns successfully. Holding
    /// the same mutex across the closure prevents another actor from taking
    /// the prospective range between identity validation and local commit.
    fn with_checked_reservation<T>(
        &self,
        count: usize,
        commit: impl FnOnce(u128, u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        if count == 0 {
            return Err(EnqueueError::FailClosed);
        }
        self.authority
            .with_checked_reservation(count, commit)
            .map_err(|_| EnqueueError::FailClosed)?
    }
    /// Hold the source at one already-minted successor while a reservation is
    /// materialized without allocating another ordinal.
    fn with_checked_current<T>(
        &self,
        commit: impl FnOnce(u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        self.authority
            .with_checked_current(commit)
            .map_err(|_| EnqueueError::FailClosed)?
    }
    fn reserve_range(&self, count: usize) -> Result<(Option<u128>, Option<u128>), String> {
        self.authority.reserve_range(count)
    }
    /// Advance a live source past a high-watermark restored by another owner.
    pub(crate) fn advance_past(&self, high_watermark: u128) -> Result<(), String> {
        self.authority.advance_past(high_watermark)
    }
    /// Read the next unused ordinal without reserving it.
    ///
    /// Runtime ingress uses this to initialize its diagnostic mirror from the
    /// same actor-global source that owns all lifecycle reservations.
    pub(super) fn next_ordinal(&self) -> Result<Option<u128>, String> {
        self.authority.next_ordinal()
    }
    /// Inspect the next actor-global lifecycle ordinal in tests.
    #[cfg(test)]
    pub(crate) fn next_ordinal_for_test(&self) -> Result<Option<u128>, String> {
        self.next_ordinal()
    }
    fn recognizes_minted(&self, ordinal: u128) -> Result<bool, String> {
        self.authority.recognizes_minted(ordinal)
    }
}
