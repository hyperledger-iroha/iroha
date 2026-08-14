/// Joint per-height ownership of the fair-ingress durable gates.
///
/// Once both gates are live they must retire in one queue transaction: the
/// Serve binding and leader-wire binding describe carriers in the same lanes.
#[cfg(test)]
struct HeightIngressBindings {
    certified_serve: CertifiedServeIngressBinding,
    leader_wire: LeaderWireIngressBinding,
}
#[cfg(test)]
impl HeightIngressBindings {
    fn new(
        certified_serve: CertifiedServeIngressBinding,
        leader_wire: LeaderWireIngressBinding,
    ) -> Self {
        Self {
            certified_serve,
            leader_wire,
        }
    }
    fn retire(&mut self) -> Result<(), V2RunnerError> {
        match (
            self.certified_serve.gate.as_ref(),
            self.leader_wire.gate.as_ref(),
        ) {
            (None, None) => return Ok(()),
            (Some(_), None) | (None, Some(_)) => {
                return Err(V2RunnerError::Service(
                    "per-height ingress gates changed joint ownership".to_owned(),
                ));
            }
            (Some(_), Some(_)) => {}
        }
        if !Arc::ptr_eq(
            &self.certified_serve.ingress_ready,
            &self.leader_wire.ingress_ready,
        ) || !Arc::ptr_eq(
            &self.certified_serve.block_ingress,
            &self.leader_wire.block_ingress,
        ) {
            return Err(V2RunnerError::Service(
                "per-height ingress gates changed their shared queue".to_owned(),
            ));
        }
        close_ingress_for_rollover(
            &self.certified_serve.ingress_ready,
            &self.certified_serve.block_ingress,
        );
        self.certified_serve
            .block_ingress
            .unbind_height_ingress_gates(
                self.certified_serve
                    .gate
                    .as_ref()
                    .expect("joint binding retains the certified Serve gate"),
                self.leader_wire
                    .gate
                    .as_ref()
                    .expect("joint binding retains the leader-wire gate"),
            )
            .map_err(V2RunnerError::Service)?;
        self.certified_serve.gate = None;
        self.leader_wire.gate = None;
        Ok(())
    }
}
#[cfg(test)]
impl Drop for HeightIngressBindings {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            // Joint validation failed before mutation. Keep the shared queue
            // fail-closed and disarm the child guards: retrying their former
            // split teardown would recreate the carrierless-Ingress cut this
            // owner exists to prevent.
            close_ingress_for_rollover(
                &self.certified_serve.ingress_ready,
                &self.certified_serve.block_ingress,
            );
            self.certified_serve.gate = None;
            self.leader_wire.gate = None;
            iroha_logger::error!(
                %error,
                "failed to atomically retire the per-height ingress gates"
            );
        }
    }
}
struct V2StatusClearGuard {
    clear_on_drop: bool,
}
impl V2StatusClearGuard {
    fn new() -> Self {
        super::status::clear_v2_status();
        Self {
            clear_on_drop: false,
        }
    }
    fn clear_on_drop(&mut self) {
        self.clear_on_drop = true;
    }
}
impl Drop for V2StatusClearGuard {
    fn drop(&mut self) {
        if self.clear_on_drop {
            super::status::clear_v2_status();
        }
    }
}
fn close_ingress_for_rollover(ingress_ready: &AtomicBool, block_ingress: &FairV2Ingress) {
    ingress_ready.store(false, Ordering::Release);
    block_ingress.close();
}
#[cfg(test)]
fn open_ingress_for_active_height(
    output_guard: &ConsensusOutputGuard,
    ingress_ready: &AtomicBool,
    block_ingress: &FairV2Ingress,
    activation: Option<(PendingSuccessorActivation, wire::SumeragiV2Status)>,
) -> Result<(), V2RunnerError> {
    let Some(ingress_activation) = output_guard.begin_fail_stop_operation() else {
        return Err(V2RunnerError::RestartRequired);
    };
    if let Some((activation, successor)) = activation.as_ref() {
        activation.preflight_ingress_open(successor)?;
    }
    block_ingress.open().map_err(ingress_capacity_error)?;
    if let Some((activation, successor)) = activation
        && let Err(error) = activation.publish(successor)
    {
        close_ingress_for_rollover(ingress_ready, block_ingress);
        return Err(error);
    }
    // `FairV2Ingress::open` prepares the private queue, but callers cannot
    // enqueue until this release store. Keep readiness false across the
    // fallible, one-shot successor publication so no carrier can be accepted
    // and then discarded if the final authority reauthentication fails.
    ingress_ready.store(true, Ordering::Release);
    ingress_activation.complete();
    Ok(())
}
fn ingress_capacity_error(error: FairV2IngressCapacityError) -> V2RunnerError {
    if error.is_bytes() {
        V2RunnerError::IngressByteCapacity {
            configured: error.configured(),
            required: error.required(),
        }
    } else {
        V2RunnerError::IngressCapacity {
            configured: error.configured(),
            required: error.required(),
        }
    }
}
fn validate_deadline_duration(duration: Duration) -> Result<(), V2RunnerError> {
    Instant::now()
        .checked_add(duration)
        .ok_or(V2RunnerError::InvalidLimits)?;
    Ok(())
}
fn deadline_after(now: Instant, duration: Duration) -> Instant {
    now.checked_add(duration)
        .expect("consensus deadline duration was prevalidated before height startup")
}
fn initial_block_sync_deadline(
    height_started_at: Instant,
    round_timeout: Duration,
    eager_recovery: bool,
) -> Instant {
    if eager_recovery {
        height_started_at
    } else {
        deadline_after(height_started_at, round_timeout)
    }
}
const fn retain_eager_block_sync(
    recovering_interrupted_tip: bool,
    admitted_discovered_commit_qc: bool,
) -> bool {
    recovering_interrupted_tip || admitted_discovered_commit_qc
}
fn snapshot_successor_logical_time(
    anchor: &wire::SnapshotBootstrapAnchor,
    block_cadence: Duration,
) -> Result<Duration, V2RunnerError> {
    let cadence_ms =
        u64::try_from(block_cadence.as_millis()).map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
    if cadence_ms == 0 || Duration::from_millis(cadence_ms) != block_cadence {
        return Err(V2RunnerError::InvalidSnapshotBootstrapCadence);
    }
    let successor_ms = anchor
        .snapshot_block_creation_time_ms
        .checked_add(cadence_ms)
        .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
    Ok(Duration::from_millis(successor_ms))
}
fn canonical_executed_block_recovery_batches(
    needs: &[CanonicalExecutedBlockNeedV1],
    capacity: usize,
) -> Result<std::slice::Chunks<'_, CanonicalExecutedBlockNeedV1>, V2RunnerError> {
    if capacity == 0
        || needs.is_empty()
        || needs
            .windows(2)
            .any(|pair| pair[0].height >= pair[1].height)
    {
        return Err(V2RunnerError::Service(
            "canonical executed-block recovery needs are empty, unordered, duplicated, or have zero batch capacity"
                .to_owned(),
        ));
    }
    Ok(needs.chunks(capacity))
}
