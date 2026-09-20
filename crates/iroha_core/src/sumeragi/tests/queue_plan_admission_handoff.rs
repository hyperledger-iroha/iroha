#[cfg(test)]
#[test]
fn queue_plan_admission_notification_retains_dirty_state_before_ingress_ready() {
    let (handle, _block, _lane_relay) = test_sumeragi_handle(4);
    handle.ingress_ready.store(false, Ordering::Release);
    assert!(!handle.notify_pending_queue_plan_admission());
    assert!(
        handle
            .pending_queue_plan_admission_dirty
            .load(Ordering::Acquire)
    );
}

#[test]
fn queue_plan_admission_notification_retains_dirty_state_when_wake_is_saturated() {
    let (mut handle, _block, _lane_relay) = test_sumeragi_handle(4);
    let (wake_tx, _wake_rx) = mpsc::sync_channel(1);
    handle.wake = wake_tx;
    handle
        .wake
        .try_send(())
        .expect("fill the bounded wake slot");
    assert!(handle.notify_pending_queue_plan_admission());
    assert!(
        handle
            .pending_queue_plan_admission_dirty
            .load(Ordering::Acquire)
    );
}

#[cfg(test)]
#[test]
fn queue_plan_admission_ingress_rejects_empty_and_oversized_bodies_before_enqueue() {
    let (handle, _block, lane_relay) = test_sumeragi_handle(4);
    let sender = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
    for certificate in [
        Arc::new(Vec::new()),
        Arc::new(vec![
            0xA5;
            iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES
                + 1
        ]),
    ] {
        assert!(matches!(
            handle.try_incoming_lane_relay_owned(LaneRelayMessage::QueuePlanAdmissionCertificate {
                sender: sender.clone(),
                certificate,
            }),
            SumeragiIngressDisposition::Rejected(_)
        ));
    }
    assert!(lane_relay.try_recv().is_err());
}

/// Feature-gated real ingress owner used by dependent-crate liveness tests.
///
/// The harness exposes only ordinary public ingress attempts and one exact
/// dequeue operation; production queue internals remain private.
#[cfg(any(
    test,
    feature = "sumeragi-main-loop-tests",
    feature = "iroha-core-tests"
))]
pub struct SumeragiIngressTestHarness {
    handle: SumeragiHandle,
    block: Arc<FairV2Ingress>,
    _lane_relay: mpsc::Receiver<LaneRelayMessage>,
}

#[cfg(any(
    test,
    feature = "sumeragi-main-loop-tests",
    feature = "iroha-core-tests"
))]
impl SumeragiIngressTestHarness {
    /// Construct an open bounded ingress with an empty validator roster.
    #[must_use]
    pub fn new(block_capacity: usize) -> Self {
        let (handle, block, lane_relay) = test_sumeragi_handle(block_capacity);
        Self {
            handle,
            block,
            _lane_relay: lane_relay,
        }
    }

    /// Clone the genuine production ingress handle.
    #[must_use]
    pub fn handle(&self) -> SumeragiHandle {
        self.handle.clone()
    }

    /// Set only the fixture's live-owner readiness to model a successor handoff.
    /// Authenticated capacity and all production admission checks remain unchanged.
    pub fn set_admission_ready(&self, ready: bool) {
        self.handle.ingress_ready.store(ready, Ordering::Release);
    }

    /// Authenticate a real genesis context before exposing its admission capacity.
    /// This fixture uses the production verifier and capacity publisher; an
    /// arbitrary layout or a missing proof of possession cannot initialize it.
    ///
    /// # Errors
    /// Rejects invalid context, proofs, local resource geometry or repeated setup.
    pub fn authenticate_admission_capacity(
        &self,
        context: iroha_data_model::block::consensus_v2::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
        config: &iroha_config::parameters::actual::Sumeragi,
    ) -> Result<(), String> {
        let mode = context.mode;
        let verified = v2::VerifiedHeightContext::genesis(context, proofs_of_possession)
            .map_err(|error| error.to_string())?;
        let config = config
            .v2_config(Duration::from_secs(1), mode)
            .map_err(|error| error.to_string())?;
        admission_capacity::publish_authenticated_capacity(
            &self.handle.admission_capacity,
            &verified,
            &config,
        )
    }

    /// Remove one exact block occurrence and release its bounded inner owner.
    #[must_use]
    pub fn pop_block(&self) -> Option<InboundBlockMessage> {
        self.block.try_recv_if(|_| true)
    }
}
