// Complete bounded I/O command queue state machine in its original source order.
impl V2IoCommandQueue {
    fn lock(&self) -> std::sync::MutexGuard<'_, V2IoCommandQueueState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn capture_lifecycle_capacity<'a>(
        self: &'a Arc<Self>,
        operation: ConsensusFailStopOperation<'a>,
        output_guard: Arc<ConsensusOutputGuard>,
        target: LifecycleIngressIoTargetSeal,
    ) -> Result<
        V2IoLifecycleCapacityCapture<'a>,
        (
            LifecycleIoCapacityCaptureFailure,
            LifecycleIngressIoTargetSeal,
        ),
    > {
        let class = match target.kind() {
            LifecycleIngressIoTargetKind::CertifiedServe => V2IoAdmissionClass::Auxiliary,
            LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence => {
                V2IoAdmissionClass::Consensus
            }
            LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence => {
                V2IoAdmissionClass::Consensus
            }
        };
        let state = self.lock();
        if !state.sender_open || !state.receiver_open {
            drop(operation);
            return Err((LifecycleIoCapacityCaptureFailure::Disconnected, target));
        }
        if self.admission.lifecycle_capacity_generation_exhausted() {
            drop(operation);
            return Err((
                LifecycleIoCapacityCaptureFailure::GenerationExhausted,
                target,
            ));
        }
        let predecessor_debt = match u64::try_from(state.commands.len()) {
            Ok(debt) => debt,
            Err(_) => {
                drop(operation);
                return Err((LifecycleIoCapacityCaptureFailure::PositionOverflow, target));
            }
        };
        if state.commands.len() >= self.capacity || !self.admission.try_reserve(class) {
            let observed_generation = self.admission.lifecycle_capacity_generation();
            drop(state);
            operation.complete();
            return Ok(V2IoLifecycleCapacityCapture::Unavailable(
                LifecycleIoCapacityWait {
                    queue: Arc::clone(self),
                    output_guard,
                    target,
                    observed_generation,
                },
            ));
        }
        Ok(V2IoLifecycleCapacityCapture::Reserved(
            LifecycleIoCapacityReservation {
                queue: self.as_ref(),
                state: Some(state),
                operation: Some(operation),
                target: Some(target),
                predecessor_debt,
            },
        ))
    }
    fn capture_recovered_lifecycle_sign_capacity<'a>(
        self: &'a Arc<Self>,
        operation: ConsensusFailStopOperation<'a>,
        key: RecoveredLifecycleSignDispatchKeyV1,
    ) -> Result<
        RecoveredLifecycleSignCapacityCaptureV1<'a>,
        RecoveredLifecycleSignCapacityCaptureErrorV1,
    > {
        let state = self.lock();
        if !state.sender_open || !state.receiver_open {
            operation.complete();
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::Disconnected);
        }
        if state.recovered_lifecycle_signs.contains_key(&key) {
            operation.complete();
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::AlreadyDispatched);
        }
        let predecessor_debt = match u64::try_from(state.commands.len()) {
            Ok(debt) => debt,
            Err(_) => {
                operation.complete();
                return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::PositionOverflow);
            }
        };
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            operation.complete();
            return Ok(RecoveredLifecycleSignCapacityCaptureV1::Unavailable);
        }
        Ok(RecoveredLifecycleSignCapacityCaptureV1::Reserved(
            RecoveredLifecycleSignCapacityReservationV1 {
                queue: self.as_ref(),
                state: Some(state),
                operation: Some(operation),
                key,
                predecessor_debt,
            },
        ))
    }
    fn capture_lifecycle_durable_validate_capacity<'a>(
        self: &'a Arc<Self>,
        operation: ConsensusFailStopOperation<'a>,
        key: LifecycleDurableValidateDispatchKeyV1,
    ) -> Result<
        LifecycleDurableValidateCapacityCaptureV1<'a>,
        LifecycleDurableValidateCapacityCaptureErrorV1,
    > {
        let state = self.lock();
        if !state.sender_open || !state.receiver_open {
            operation.complete();
            return Err(LifecycleDurableValidateCapacityCaptureErrorV1::Disconnected);
        }
        if state.lifecycle_durable_validates.contains_key(&key) {
            operation.complete();
            return Err(LifecycleDurableValidateCapacityCaptureErrorV1::AlreadyDispatched);
        }
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            operation.complete();
            return Ok(LifecycleDurableValidateCapacityCaptureV1::Unavailable);
        }
        Ok(LifecycleDurableValidateCapacityCaptureV1::Reserved(
            LifecycleDurableValidateCapacityReservationV1 {
                queue: self.as_ref(),
                state: Some(state),
                operation: Some(operation),
                key,
            },
        ))
    }
    /// Project worker capacity for one recovered candidate without changing the queue cut.
    fn recovered_completion_worker_capacity(&self, state: &V2IoCommandQueueState) -> bool {
        state.commands.len() < self.capacity
            && self.admission.queued() < self.admission.limit(V2IoAdmissionClass::Consensus)
    }

    fn try_send_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        if let Some(key) = command.recovered_decision_apply_key() {
            return Err(V2IoTrySendError::UnreservedRecoveredDecisionApply { key, command });
        }
        assert!(
            command.recovered_lifecycle_sign_key().is_none(),
            "recovered Sign commands require their locked lifecycle reservation"
        );
        assert!(
            command.recovered_decision_fetch_key().is_none(),
            "recovered Decision Fetch persistence requires its locked lifecycle reservation"
        );
        assert!(
            command.lifecycle_durable_validate_key().is_none(),
            "durable Validate commands require their locked lifecycle reservation"
        );
        assert!(
            command.lifecycle_certified_serve_ordinal().is_none(),
            "lifecycle Certified-Serve commands require their locked auxiliary reservation"
        );
        let descriptor = command.work_descriptor();
        let mut state = self.lock();
        if !state.sender_open || !state.receiver_open {
            return Err(V2IoTrySendError::Disconnected(command));
        }
        if let Some((work_id, descriptor)) = &descriptor
            && let Some(existing) = state.work.get(work_id)
        {
            if existing.descriptor == *descriptor {
                return Ok(());
            }
            return Err(V2IoTrySendError::ConflictingWorkId {
                work_id: *work_id,
                command,
            });
        }
        if state.commands.len() >= self.capacity || !self.admission.try_reserve(class) {
            return Err(V2IoTrySendError::Full(command));
        }
        if let Some((work_id, descriptor)) = descriptor {
            let replaced = state.work.insert(
                work_id,
                V2IoTrackedWork {
                    descriptor,
                    state: V2IoWorkState::Queued,
                },
            );
            debug_assert!(replaced.is_none());
        }
        state.commands.push_back(command);
        drop(state);
        self.ready.notify_one();
        Ok(())
    }
    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        let mut state = self.lock();
        let Some(tracked) = state.work.get(&work_id) else {
            return Err(format!(
                "Sumeragi v2 I/O work {} has no tracked owner",
                work_id.get()
            ));
        };
        if tracked.descriptor.cancellable_kind() != Some(expected_kind) {
            return Err(format!(
                "Sumeragi v2 I/O work {} was reused by a conflicting command",
                work_id.get()
            ));
        }
        if matches!(
            tracked.state,
            V2IoWorkState::Active | V2IoWorkState::CompletionPending
        ) {
            return Ok(false);
        }
        let index = state
            .commands
            .iter()
            .position(|command| command.work_id() == Some(work_id))
            .expect("queued Sumeragi v2 work must have a FIFO owner");
        let removed = state
            .commands
            .remove(index)
            .expect("located Sumeragi v2 work must remain queued");
        debug_assert_eq!(removed.work_id(), Some(work_id));
        debug_assert_eq!(removed.cancellable_kind(), Some(expected_kind));
        state
            .work
            .remove(&work_id)
            .expect("removed Sumeragi v2 work must have an ownership record");
        self.admission.release();
        drop(state);
        self.ready.notify_all();
        Ok(true)
    }
    fn recv(&self) -> Result<V2IoCommand, ()> {
        let mut state = self.lock();
        loop {
            if let Some(command) = state.commands.pop_front() {
                self.admission.release();
                if let Some(work_id) = command.work_id() {
                    let tracked = state
                        .work
                        .get_mut(&work_id)
                        .expect("queued Sumeragi v2 command must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.recovered_decision_apply_key() {
                    let tracked = state
                        .recovered_decision_applies
                        .get_mut(&key)
                        .expect("queued recovered Decision Apply must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.recovered_lifecycle_sign_key() {
                    let tracked = state
                        .recovered_lifecycle_signs
                        .get_mut(&key)
                        .expect("queued recovered Sign must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.lifecycle_durable_validate_key() {
                    let tracked = state
                        .lifecycle_durable_validates
                        .get_mut(&key)
                        .expect("queued durable Validate must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.recovered_decision_fetch_key() {
                    let tracked = state
                        .recovered_decision_fetch_bodies
                        .get_mut(&key)
                        .expect("queued recovered Decision Fetch body must retain its owner");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(ordinal) = command.lifecycle_certified_serve_ordinal() {
                    let tracked = state
                        .lifecycle_serves
                        .get_mut(&ordinal)
                        .expect("queued lifecycle Certified-Serve must retain its exact owner");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                return Ok(command);
            }
            if !state.sender_open {
                return Err(());
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
    #[cfg(test)]
    fn try_recv(&self) -> Result<V2IoCommand, mpsc::TryRecvError> {
        let mut state = self.lock();
        let Some(command) = state.commands.pop_front() else {
            return if state.sender_open {
                Err(mpsc::TryRecvError::Empty)
            } else {
                Err(mpsc::TryRecvError::Disconnected)
            };
        };
        self.admission.release();
        if let Some(work_id) = command.work_id() {
            let tracked = state
                .work
                .get_mut(&work_id)
                .expect("queued Sumeragi v2 command must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.recovered_decision_apply_key() {
            let tracked = state
                .recovered_decision_applies
                .get_mut(&key)
                .expect("queued recovered Decision Apply must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.recovered_lifecycle_sign_key() {
            let tracked = state
                .recovered_lifecycle_signs
                .get_mut(&key)
                .expect("queued recovered Sign must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.lifecycle_durable_validate_key() {
            let tracked = state
                .lifecycle_durable_validates
                .get_mut(&key)
                .expect("queued durable Validate must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.recovered_decision_fetch_key() {
            let tracked = state
                .recovered_decision_fetch_bodies
                .get_mut(&key)
                .expect("queued recovered Decision Fetch body must retain its owner");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(ordinal) = command.lifecycle_certified_serve_ordinal() {
            let tracked = state
                .lifecycle_serves
                .get_mut(&ordinal)
                .expect("queued lifecycle Certified-Serve must retain its exact owner");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        Ok(command)
    }
    fn complete_work(&self, work_id: EffectWorkId) {
        let mut state = self.lock();
        let tracked = state
            .work
            .get_mut(&work_id)
            .expect("completed Sumeragi v2 work must have an ownership record");
        assert_eq!(tracked.state, V2IoWorkState::Active);
        tracked.state = V2IoWorkState::CompletionPending;
    }
    fn complete_recovered_decision_apply(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
        result: &RecoveredDecisionApplyWorkerResultV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_applies
            .get_mut(&key)
            .ok_or_else(|| {
                "completed recovered Decision Apply lost its lifecycle owner".to_owned()
            })?;
        if tracked.state != V2IoWorkState::Active || result.dispatch_key() != key {
            return Err(
                "completed recovered Decision Apply changed its exact dispatch material".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_recovered_lifecycle_sign(
        &self,
        key: RecoveredLifecycleSignDispatchKeyV1,
        result: &RecoveredLifecycleSignWorkerResultV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_lifecycle_signs
            .get_mut(&key)
            .ok_or_else(|| "completed recovered Sign lost its lifecycle owner".to_owned())?;
        if tracked.state != V2IoWorkState::Active
            || result.dispatch_key() != key
            || !result.is_exact()
        {
            return Err("completed recovered Sign changed its exact dispatch material".to_owned());
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_lifecycle_durable_validate(
        &self,
        key: LifecycleDurableValidateDispatchKeyV1,
        result: &LifecycleDurableValidateWorkerResultV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_durable_validates
            .get_mut(&key)
            .ok_or_else(|| "completed durable Validate lost its lifecycle owner".to_owned())?;
        if tracked.state != V2IoWorkState::Active || result.dispatch_key() != key {
            return Err(
                "completed durable Validate changed its exact address/digest identity".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_recovered_decision_fetch_body(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        completion: &RecoveredDecisionFetchBodyPersistenceCompletionV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_fetch_bodies
            .get_mut(&key)
            .ok_or_else(|| {
                "completed recovered Decision Fetch body lost its lifecycle owner".to_owned()
            })?;
        if tracked.state != V2IoWorkState::Active
            || completion.dispatch_key() != key
            || completion.id() != tracked.id
            || completion.response_hash() != tracked.response_hash
        {
            return Err(
                "completed recovered Decision Fetch body changed its exact persistence material"
                    .to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_lifecycle_certified_serve(
        &self,
        result: &LifecycleCertifiedServeWorkerResultV1,
    ) -> Result<(), String> {
        let ordinal = result.lifecycle_ordinal();
        let mut state = self.lock();
        let tracked = state.lifecycle_serves.get_mut(&ordinal).ok_or_else(|| {
            "completed lifecycle Certified-Serve lost its exact queue owner".to_owned()
        })?;
        if tracked.state != V2IoWorkState::Active
            || tracked.request_hash != result.request_hash()
            || result.response.request_hash != tracked.request_hash
        {
            return Err(
                "completed lifecycle Certified-Serve changed its lease or request".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn retry_recovered_decision_apply<T: RecoveredDecisionApplyRetryTaskV1>(
        &self,
        task: T,
    ) -> Result<(), RecoveredDecisionApplyRetryQueueErrorV1<T>> {
        let key = task.dispatch_key();
        let mut state = self.lock();
        if !state.sender_open
            || !state.receiver_open
            || !self
                .admission
                .recovered_decision_apply_completion_is_exact(key)
            || state
                .recovered_decision_applies
                .get(&key)
                .is_none_or(|tracked| tracked.state != V2IoWorkState::CompletionPending)
            || state
                .commands
                .iter()
                .any(|command| command.recovered_decision_apply_key() == Some(key))
        {
            return Err(RecoveredDecisionApplyRetryQueueErrorV1::InvalidOwner(task));
        }
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            return Err(RecoveredDecisionApplyRetryQueueErrorV1::Unavailable(task));
        }
        // Transfer the exact keyed completion slot back to the command FIFO
        // while the same queue cut is still locked. The worker cannot publish
        // the replacement completion before the old owner is gone.
        assert!(
            self.admission
                .transfer_recovered_decision_apply_completion(key),
            "locked recovered Apply retry must retain its exact completion owner"
        );
        state
            .recovered_decision_applies
            .get_mut(&key)
            .expect("validated recovered Apply retry retains its command owner")
            .state = V2IoWorkState::Queued;
        state.commands.push_back(task.into_command());
        drop(state);
        self.ready.notify_all();
        Ok(())
    }
    fn retry_lifecycle_durable_validate(
        &self,
        dispatch: DurableValidateDispatch,
    ) -> Result<
        (),
        (
            LifecycleDurableValidateRetryQueueErrorV1,
            DurableValidateDispatch,
        ),
    > {
        let key = dispatch.dispatch_key();
        let mut state = self.lock();
        if !state.sender_open
            || !state.receiver_open
            || state
                .lifecycle_durable_validates
                .get(&key)
                .is_none_or(|tracked| tracked.state != V2IoWorkState::CompletionPending)
            || state
                .commands
                .iter()
                .any(|command| command.lifecycle_durable_validate_key() == Some(key))
        {
            return Err((
                LifecycleDurableValidateRetryQueueErrorV1::InvalidOwner,
                dispatch,
            ));
        }
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            return Err((
                LifecycleDurableValidateRetryQueueErrorV1::Unavailable,
                dispatch,
            ));
        }
        state
            .lifecycle_durable_validates
            .get_mut(&key)
            .expect("validated durable Validate retry retains its command owner")
            .state = V2IoWorkState::Queued;
        state
            .commands
            .push_back(V2IoCommand::LifecycleDurableValidate(dispatch));
        drop(state);
        self.ready.notify_all();
        Ok(())
    }
    fn acknowledge_completion(&self, work_id: EffectWorkId) {
        let mut state = self.lock();
        let tracked = state
            .work
            .remove(&work_id)
            .expect("delivered Sumeragi v2 completion must have an ownership record");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn prepare_recovered_decision_apply_ack(
        self: &Arc<Self>,
        key: RecoveredDecisionApplyDispatchKeyV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<RecoveredDecisionApplyWorkAckV1, String> {
        let state = self.lock();
        let tracked = state.recovered_decision_applies.get(&key).ok_or_else(|| {
            "recovered Decision Apply completion lost its exact command owner".to_owned()
        })?;
        if tracked.state != V2IoWorkState::CompletionPending {
            return Err(
                "recovered Decision Apply completion crossed a non-pending command owner"
                    .to_owned(),
            );
        }
        if !self
            .admission
            .recovered_decision_apply_completion_is_exact(key)
        {
            return Err(
                "recovered Decision Apply completion changed its bounded FIFO ownership".to_owned(),
            );
        }
        drop(state);
        Ok(RecoveredDecisionApplyWorkAckV1 {
            queue: Arc::clone(self),
            output_guard,
            key,
            armed: true,
        })
    }
    fn transfer_recovered_lifecycle_sign_completion(
        self: &Arc<Self>,
        key: RecoveredLifecycleSignDispatchKeyV1,
        ownership_position: usize,
    ) -> bool {
        let state = self.lock();
        state
            .recovered_lifecycle_signs
            .get(&key)
            .is_some_and(|tracked| tracked.state == V2IoWorkState::CompletionPending)
            && self
                .admission
                .transfer_recovered_lifecycle_sign_completion_at(key, ownership_position)
    }
    fn acknowledge_recovered_decision_apply(&self, key: RecoveredDecisionApplyDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_applies
            .remove(&key)
            .expect("settled recovered Decision Apply must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn acknowledge_recovered_lifecycle_sign(&self, key: RecoveredLifecycleSignDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .recovered_lifecycle_signs
            .remove(&key)
            .expect("settled recovered Sign must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn acknowledge_lifecycle_durable_validate(&self, key: LifecycleDurableValidateDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_durable_validates
            .remove(&key)
            .expect("settled durable Validate retains its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn acknowledge_recovered_decision_fetch_body(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        id: super::v2_lifecycle_coordinator::RecoveredDecisionFetchBodyPersistenceIdV1,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    ) {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_fetch_bodies
            .remove(&key)
            .expect("settled recovered Decision Fetch must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(tracked.id, id);
        assert_eq!(tracked.response_hash, response_hash);
    }
    fn transfer_lifecycle_certified_serve_completion(
        self: &Arc<Self>,
        ordinal: u128,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        ownership_position: usize,
    ) -> bool {
        let state = self.lock();
        let pending = state.lifecycle_serves.get(&ordinal).is_some_and(|tracked| {
            tracked.state == V2IoWorkState::CompletionPending
                && tracked.request_hash == request_hash
        });
        drop(state);
        pending
            && self
                .admission
                .transfer_lifecycle_certified_serve_completion_at(ordinal, ownership_position)
    }
    fn acknowledge_lifecycle_certified_serve(
        &self,
        ordinal: u128,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_serves
            .remove(&ordinal)
            .expect("settled lifecycle Certified-Serve retains its exact queue owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(tracked.request_hash, request_hash);
    }
    fn prepare_certified_fetch_body_persistence_ack(
        self: &Arc<Self>,
        completion: &CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<CertifiedFetchBodyPersistenceWorkAck, String> {
        let work_id = completion.work_id();
        let descriptor = V2IoWorkDescriptor::PersistCertifiedFetchBody {
            id: completion.id(),
            response_hash: completion.response_hash(),
        };
        let state = self.lock();
        let tracked = state.work.get(&work_id).ok_or_else(|| {
            format!(
                "persisted certified-Fetch body work {} lost its exact command owner",
                work_id.get()
            )
        })?;
        if tracked.state != V2IoWorkState::CompletionPending || tracked.descriptor != descriptor {
            return Err(format!(
                "persisted certified-Fetch body work {} changed its exact command owner",
                work_id.get()
            ));
        }
        drop(state);
        Ok(CertifiedFetchBodyPersistenceWorkAck {
            queue: Arc::clone(self),
            output_guard,
            work_id,
            descriptor,
            armed: true,
        })
    }
    fn acknowledge_exact_lifecycle_completion(
        &self,
        work_id: EffectWorkId,
        descriptor: &V2IoWorkDescriptor,
    ) {
        let mut state = self.lock();
        let tracked = state
            .work
            .get(&work_id)
            .expect("preflighted lifecycle completion retains its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(&tracked.descriptor, descriptor);
        state
            .work
            .remove(&work_id)
            .expect("preflighted lifecycle completion work remains indexed");
    }
    fn close_sender(&self) {
        let mut state = self.lock();
        state.sender_open = false;
        drop(state);
        self.ready.notify_all();
    }
    fn close_receiver(&self) {
        let mut state = self.lock();
        if !state.receiver_open {
            return;
        }
        state.receiver_open = false;
        let queued = state.commands.len();
        assert!(
            state
                .lifecycle_serves
                .values()
                .all(|tracked| tracked.state == V2IoWorkState::CompletionPending),
            "receiver teardown cannot abandon a queued or active lifecycle Certified-Serve"
        );
        assert!(
            state
                .lifecycle_durable_validates
                .values()
                .all(|tracked| tracked.state == V2IoWorkState::CompletionPending),
            "receiver teardown cannot abandon queued or active durable Validate work"
        );
        state.commands.clear();
        // A normal Shutdown/Retire exit can close the command receiver while
        // already-sent completions remain buffered. Keep those ownership
        // records until the serialized handle drains and acknowledges them.
        state
            .work
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_decision_applies
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_lifecycle_signs
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_decision_fetch_bodies
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .lifecycle_durable_validates
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .lifecycle_serves
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        for _ in 0..queued {
            self.admission.release();
        }
        drop(state);
        self.ready.notify_all();
    }
}
