// Read-only ownership projected from retained lifecycle worker indexes.

/// Immutable authority carried by a Certified-Serve task at queue admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum LifecycleServeAuthorityKindV1 {
    /// Physical work retains the coordinator's claimed Serve lease.
    Claimed,
    /// Physical work revalidates an already-terminal Serve without a new lease.
    TerminalReplay,
}

/// One outstanding Certified-Serve owner, retained until exact acknowledgement.
///
/// This projection does not grant dispatch or completion authority. Its kind is
/// fixed by the admitted task and remains unchanged across queued, active and
/// completion-pending physical states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct LifecycleServeOwnershipV1 {
    /// Exact coordinator ordinal named by the admitted task.
    pub(in crate::sumeragi) lifecycle_ordinal: u128,
    /// Exact authenticated request bound to this physical owner.
    pub(in crate::sumeragi) request_hash: HashOf<wire::CertifiedBodyRequest>,
    /// Whether this worker retains a live claim or a terminal replay authority.
    pub(in crate::sumeragi) authority: LifecycleServeAuthorityKindV1,
}

impl V2IoCommandQueue {
    fn lifecycle_serve_ownership_snapshot(&self) -> Vec<LifecycleServeOwnershipV1> {
        self.lock()
            .lifecycle_serves
            .iter()
            .map(|(&lifecycle_ordinal, tracked)| LifecycleServeOwnershipV1 {
                lifecycle_ordinal,
                request_hash: tracked.request_hash,
                authority: tracked.authority,
            })
            .collect()
    }
}

impl ProductionV2Services {
    /// Read existing disk-persistence custody through exact completion acknowledgement.
    fn certified_fetch_persistence_work_snapshot(&self) -> BTreeSet<EffectWorkId> {
        self.io.as_ref().map_or_else(BTreeSet::new, |io| {
            io.command_tx
                .queue
                .lock()
                .work
                .iter()
                .filter_map(|(&work_id, tracked)| {
                    matches!(
                        &tracked.descriptor,
                        V2IoWorkDescriptor::PersistCertifiedFetchBody { .. }
                    )
                    .then_some(work_id)
                })
                .collect()
        })
    }

    /// A reconstruction cannot retire the request already owned by disk persistence.
    /// Keep that service owner until Phase B atomically consumes it; the disk FIFO
    /// remains eligible while this redundant local result waits.
    fn available_local_completion(&self) -> Option<&LocalCompletion> {
        let completion = self.local_completions.front()?;
        let LocalCompletion::Reconstructed { task, .. } = completion;
        (!self
            .certified_fetch_persistence_work_snapshot()
            .contains(&task.id()))
        .then_some(completion)
    }

    /// Whether physical lifecycle work still owes a completion without a lease.
    ///
    /// Validate and ordinary certified-Fetch persistence relinquish their
    /// coordinator leases before dispatch. Their retained queue owners remain
    /// authoritative through completion acknowledgement. Generic Runtime work
    /// and network-only Fetch waits do not belong to this census.
    pub(in crate::sumeragi) fn has_unleased_lifecycle_completion_work(&self) -> Option<bool> {
        self.io.as_ref().map(|io| {
            let state = io.command_tx.queue.lock();
            !state.lifecycle_validates.is_empty()
                || state.work.values().any(|tracked| {
                    matches!(
                        &tracked.descriptor,
                        V2IoWorkDescriptor::PersistCertifiedFetchBody { .. }
                    )
                })
        })
    }

    /// Inspect every retained Serve worker without consuming its task or result.
    ///
    /// Absence means the service has no I/O owner; it is not an empty census.
    /// Closing the worker receiver preserves completion-pending entries until
    /// their existing typed completion path acknowledges them.
    pub(in crate::sumeragi) fn lifecycle_serve_ownership_snapshot(
        &self,
    ) -> Option<Vec<LifecycleServeOwnershipV1>> {
        self.io
            .as_ref()
            .map(|io| io.command_tx.queue.lifecycle_serve_ownership_snapshot())
    }
}
