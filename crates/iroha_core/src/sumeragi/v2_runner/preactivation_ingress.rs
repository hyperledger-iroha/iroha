//! Borrow-scoped ingress aperture for preactivation canonical-body recovery.

use super::*;

/// Borrowed ownership of the exact runner ingress during preactivation recovery.
///
/// This aperture intentionally opens the retained activation's normal per-height fair ingress, as the
/// legacy recovery loop does. Its caller must still dequeue only messages
/// admitted by the canonical executed-body recovery predicate. The exact
/// activation authority remains mutably borrowed for the aperture's lifetime,
/// so it cannot publish status or be substituted while recovery is live.
#[must_use = "the preactivation ingress aperture must remain live for recovery"]
pub(in crate::sumeragi) struct ProductionLifecycleCanonicalRecoveryIngressV1<'activation> {
    ingress_ready: &'activation Arc<AtomicBool>,
    block_ingress: &'activation Arc<FairV2Ingress>,
    open: bool,
}

impl ProductionLifecycleCanonicalRecoveryIngressV1<'_> {
    /// Borrow the exact opened ingress for the canonical recovery predicate.
    #[allow(
        dead_code,
        reason = "consumed at the pending atomic lifecycle runner cutover"
    )]
    pub(in crate::sumeragi) fn ingress(&self) -> &FairV2Ingress {
        self.block_ingress.as_ref()
    }

    fn close(&mut self) {
        if self.open {
            // Stop producers before closing the queue, matching the live-height
            // rollover boundary and preventing admission into a closing owner.
            self.ingress_ready.store(false, Ordering::Release);
            self.block_ingress.close();
            self.open = false;
        }
    }

    /// Close the temporary aperture and revalidate its exact quiescent state.
    pub(in crate::sumeragi) fn close_and_verify(mut self) -> bool {
        self.close();
        !self.ingress_ready.load(Ordering::Acquire) && !self.block_ingress.state.lock().open
    }
}

impl Drop for ProductionLifecycleCanonicalRecoveryIngressV1<'_> {
    fn drop(&mut self) {
        self.close();
    }
}

fn open_canonical_recovery_ingress<'activation>(
    ingress_ready: &'activation Arc<AtomicBool>,
    block_ingress: &'activation Arc<FairV2Ingress>,
    launched_ingress: &Arc<FairV2Ingress>,
) -> Result<ProductionLifecycleCanonicalRecoveryIngressV1<'activation>, V2RunnerError> {
    if !Arc::ptr_eq(block_ingress, launched_ingress) {
        ingress_ready.store(false, Ordering::Release);
        block_ingress.close();
        return Err(V2RunnerError::LifecycleActivationIngressMismatch);
    }
    if ingress_ready.load(Ordering::Acquire) || block_ingress.state.lock().open {
        ingress_ready.store(false, Ordering::Release);
        block_ingress.close();
        return Err(V2RunnerError::Service(
            "preactivation canonical recovery requires one closed runner ingress".to_owned(),
        ));
    }
    block_ingress.open().map_err(ingress_capacity_error)?;
    // The queue is fully opened before producers observe readiness. Failure
    // before this release store therefore cannot admit a carrier into a closed
    // or foreign queue.
    ingress_ready.store(true, Ordering::Release);
    Ok(ProductionLifecycleCanonicalRecoveryIngressV1 {
        ingress_ready,
        block_ingress,
        open: true,
    })
}

impl ProductionLifecycleRunnerActivationV1 {
    /// Borrow this activation's exact ingress for preactivation recovery.
    pub(in crate::sumeragi) fn open_canonical_recovery_ingress(
        &mut self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<ProductionLifecycleCanonicalRecoveryIngressV1<'_>, V2RunnerError> {
        open_canonical_recovery_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }
}

impl ProductionLifecycleCompleteTipRunnerActivationV1 {
    /// Borrow this CompleteTip activation's exact ingress without publishing H+1.
    pub(in crate::sumeragi) fn open_canonical_recovery_ingress(
        &mut self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<ProductionLifecycleCanonicalRecoveryIngressV1<'_>, V2RunnerError> {
        open_canonical_recovery_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }
}

impl ProductionLifecyclePendingKuraRunnerActivationV1 {
    /// Borrow this interrupted-tip activation's exact ingress before no-clock activation.
    pub(in crate::sumeragi) fn open_canonical_recovery_ingress(
        &mut self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<ProductionLifecycleCanonicalRecoveryIngressV1<'_>, V2RunnerError> {
        open_canonical_recovery_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }
}
