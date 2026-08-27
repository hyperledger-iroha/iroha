// Durable autonomous-lane Queue release authority and claim finalization.
impl Kura {
    /// Authenticate the exact Kura half of one release-barrier Queue snapshot.
    ///
    /// This path is strictly read-only: it neither repairs view state nor
    /// advances entrypoint claims. It reopens the exact active-incarnation
    /// attempt, verifies its producer-authenticated payload and durable
    /// retirement, binds the caller's FIFO-ordered reservation group, rejects
    /// every staged, linked, oversized, missing, or noncanonical claim, and
    /// validates the complete ordered `Released* ReleasePending*` prefix.
    /// `Completed` additionally requires every claim to be exactly `Released`.
    ///
    /// The returned evidence supplies the current composed release state and
    /// immutable attempt anchor only. Startup recovery must pair that anchor
    /// with an independently authenticated signed lifecycle cursor for the
    /// same group before authorizing Queue action 25.
    pub(crate) fn authenticate_autonomous_lane_retirement_snapshot_evidence(
        &self,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_group: LaneQueueReservationGroupBindingV1,
        phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
    ) -> Result<AutonomousLaneRetirementSnapshotEvidenceV1> {
        payload
            .validate(payload.network_id, payload.epoch)
            .map_err(|error| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    format!("invalid autonomous retirement snapshot payload: {error}"),
                )
            })?;
        if !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous retirement snapshot differs from its exact payload",
            ));
        }
        let payload_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|reason| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        format!(
                            "invalid autonomous retirement snapshot reservation group: {reason}"
                        ),
                    )
                })?;
        let barrier = retirement.queue_release_barrier()?;
        let barrier_group =
            lane_queue_reservation_group_binding_from_ordered_keys(barrier.ordered_keys.iter())
                .map_err(|reason| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        format!("invalid autonomous retirement snapshot release barrier: {reason}"),
                    )
                })?;
        if payload_group != expected_group || barrier_group != expected_group {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous retirement snapshot names another ordered reservation group",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let descriptor = &payload.origin_proposal.descriptor;
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
            &entry,
            &self.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let _sidecar_guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                descriptor.lane_id,
                descriptor.lane_block_height,
                descriptor.proposal_height,
                payload.network_id,
                payload.epoch,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    attempt_path,
                    "missing exact autonomous attempt for retirement snapshot evidence",
                )
            })?;
        if record.artifact.executable_payload != *payload
            || record.retirement.as_ref() != Some(retirement)
        {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "durable autonomous attempt conflicts with retirement snapshot evidence",
            ));
        }
        let context = AutonomousLaneReleaseProjectionContext::from_payload(
            self,
            &record.artifact.executable_payload,
            retirement,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(self.store_root.clone(), message))?;
        if context.reservation_group != expected_group {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous retirement projection changed its ordered reservation group",
            ));
        }
        let (pending_prefix, released_prefix) = self
            .autonomous_lane_entrypoint_claim_release_progress_locked(
                &record.artifact.executable_payload,
                retirement,
            )?;
        context
            .retirement_snapshot_evidence(
                &record.artifact.executable_payload,
                phase,
                pending_prefix,
                released_prefix,
            )
            .map_err(|message| Self::invalid_lane_artifact_error(self.store_root.clone(), message))
    }
    /// Authenticate a retired non-producer replica before Queue proves exact
    /// FIFO-only ownership for its ordered group.
    ///
    /// This path is read-only. It reopens the exact durable retirement and
    /// current lane-height attempt, verifies the locally signed lifecycle
    /// cursor against the current Kura process record, and requires that the
    /// signed local actor differ from the frozen producer. Producers receive
    /// `None` and continue through the ordinary prepared-Queue corridor.
    pub(crate) fn authorize_autonomous_nonqueue_replica_claim_release(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<Option<AutonomousNonQueueReplicaClaimReleaseAuthorization>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        if retirement.version != AutonomousLaneSlotRetirementV1::VERSION
            || retirement.network_id != expected_network_id
            || retirement.epoch != expected_epoch
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica release authorization has an unsupported chain context",
            ));
        }
        let entry = self.lane_storage_entry(retirement.lane_id)?;
        let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
            &entry,
            &self.store_root,
            retirement.lane_block_height,
            retirement.proposal_height,
        );
        let _sidecar_guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                retirement.lane_id,
                retirement.lane_block_height,
                retirement.proposal_height,
                expected_network_id,
                expected_epoch,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    attempt_path,
                    "missing exact autonomous attempt for non-Queue replica release",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        if record.retirement.as_ref() != Some(retirement) || !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "non-Queue replica release differs from its exact durable retirement",
            ));
        }
        let Some(local_peer) = self.local_peer_id.get() else {
            return Ok(None);
        };
        if local_peer == &payload.producer {
            return Ok(None);
        }
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                &entry,
                retirement.lane_block_height,
                None,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "non-Queue replica release has no current lane-height attempt",
                )
            })?;
        if current.artifact.executable_payload != *payload {
            return Err(Self::invalid_lane_artifact_error(
                current.view_state_path,
                "non-Queue replica release attempt is no longer current",
            ));
        }
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(&entry, payload)?;
        let context =
            AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
        let binding = cursor.binding();
        let (_, signed_actor) = binding.local_validator_identity();
        if signed_actor != context.actor
            || binding.producer_actor_projection() != context.producer
            || signed_actor == context.producer
            || binding.reservation_group_binding() != context.reservation_group
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica release cursor changed its exact actor or reservation binding",
            ));
        }
        let (pending_prefix, released_prefix) =
            self.autonomous_lane_entrypoint_claim_release_progress_locked(payload, retirement)?;
        if pending_prefix != context.reservation_group.reservation_count {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica release lacks the full durable ReleasePending prefix",
            ));
        }
        let fifo_projection = context
            .replica_fifo_ownership_projection(released_prefix)
            .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?;
        Ok(Some(AutonomousNonQueueReplicaClaimReleaseAuthorization {
            store_root: self.store_root.clone(),
            release_barrier: retirement.queue_release_barrier()?,
            fifo_projection,
            retirement: retirement.clone(),
            cursor,
        }))
    }
    /// Consume Queue's exact FIFO-only proof and release this replica's Kura
    /// claims without manufacturing a Queue reservation owner.
    ///
    /// Queue's proof retains an exact-hash durability transition for the whole
    /// ordered group. This method deliberately keeps that proof alive while it
    /// revalidates the retired attempt and signed cursor and while every
    /// crash-resumable `ReleasePending -> Released` replacement is written,
    /// then returns the same move-only proof so the caller can retain the fence
    /// through terminal Queue evidence and Kura completion.
    pub(crate) fn finalize_autonomous_nonqueue_replica_claim_release<'queue>(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        authorization: crate::queue::DurableAutonomousNonQueueReplicaFifoAuthorization<'queue>,
    ) -> Result<crate::queue::DurableAutonomousNonQueueReplicaFifoAuthorization<'queue>> {
        let barrier = retirement.queue_release_barrier()?;
        let kura_authorization =
            authorization
                .authorization_for_kura(&barrier)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "non-Queue replica FIFO authority names another retirement barrier",
                    )
                })?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        if retirement.version != AutonomousLaneSlotRetirementV1::VERSION
            || retirement.network_id != expected_network_id
            || retirement.epoch != expected_epoch
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica claim finalization has an unsupported chain context",
            ));
        }
        let entry = self.lane_storage_entry(retirement.lane_id)?;
        let attempt_path = Self::autonomous_lane_block_attempt_path_for_entry(
            &entry,
            &self.store_root,
            retirement.lane_block_height,
            retirement.proposal_height,
        );
        let _sidecar_guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                retirement.lane_id,
                retirement.lane_block_height,
                retirement.proposal_height,
                expected_network_id,
                expected_epoch,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    attempt_path,
                    "missing exact autonomous attempt for non-Queue replica claim finalization",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        if record.retirement.as_ref() != Some(retirement) || !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "non-Queue replica claim finalization changed its durable retirement",
            ));
        }
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                &entry,
                retirement.lane_block_height,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "non-Queue replica claim finalization has no current lane-height attempt",
                )
            })?;
        if current.artifact.executable_payload != *payload {
            return Err(Self::invalid_lane_artifact_error(
                current.view_state_path,
                "non-Queue replica claim finalization attempt is no longer current",
            ));
        }
        let cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(&entry, payload)?;
        let context =
            AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
        let binding = cursor.binding();
        let (_, signed_actor) = binding.local_validator_identity();
        if !kura_authorization.matches_exact_durable_source(
            &self.store_root,
            retirement,
            &cursor,
        )
            || signed_actor != context.actor
            || binding.producer_actor_projection() != context.producer
            || signed_actor == context.producer
            || binding.reservation_group_binding() != context.reservation_group
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica FIFO proof changed its signed durable release source",
            ));
        }
        let (pending_prefix, _) =
            self.autonomous_lane_entrypoint_claim_release_progress_locked(payload, retirement)?;
        if pending_prefix != context.reservation_group.reservation_count {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "non-Queue replica claim finalization lacks the full ReleasePending prefix",
            ));
        }
        self.finalize_autonomous_lane_entrypoint_claim_release_with_mode_locked(
            pending_canonical_bytes,
            payload,
            retirement,
            AutonomousLaneClaimReleaseAuthorizationMode::ReplicaFifo,
        )?;
        self.require_autonomous_lane_entrypoint_claims_released_locked(payload, retirement)?;
        Ok(authorization)
    }
    /// Authenticate Queue's exact prepared-release boundary from durable Kura evidence.
    ///
    /// This reopens the exact historical attempt, verifies its retirement,
    /// repairs only a canonical `ReleasePending` crash prefix for the current
    /// attempt, and then mints a move-only authority bound to the complete
    /// ordered Queue barrier. A superseded attempt is accepted only through
    /// the existing authenticated supersession proof.
    pub(crate) fn authorize_autonomous_lane_queue_release_preparation(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<AutonomousLaneQueueReleasePreparationAuthorization> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        if retirement.version != AutonomousLaneSlotRetirementV1::VERSION
            || retirement.network_id != expected_network_id
            || retirement.epoch != expected_epoch
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous Queue release authorization has an unsupported chain context",
            ));
        }
        let barrier = retirement.queue_release_barrier()?;
        let entry = self.lane_storage_entry(retirement.lane_id)?;
        let slot_path = Self::autonomous_lane_block_latest_attempt_path_for_entry(
            &entry,
            &self.store_root,
            retirement.lane_block_height,
        );
        let _guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                retirement.lane_id,
                retirement.lane_block_height,
                retirement.proposal_height,
                expected_network_id,
                expected_epoch,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    slot_path,
                    "missing autonomous executable payload for Queue release authorization",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        if record.retirement.as_ref() != Some(retirement) || !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "Queue release authorization does not match its durable Kura retirement",
            ));
        }
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                &entry,
                retirement.lane_block_height,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "Queue release authorization has no current lane-height attempt",
                )
            })?;
        let claims_fully_released = if current.artifact.executable_payload == *payload {
            self.prepare_autonomous_lane_entrypoint_claim_release_locked(
                pending_canonical_bytes,
                payload,
                retirement,
            )?;
            let (pending_prefix, released_prefix) =
                self.autonomous_lane_entrypoint_claim_release_progress_locked(payload, retirement)?;
            let selected_count = u64::try_from(payload.entrypoint_hashes.len()).map_err(|_| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous release group size exceeds u64",
                )
            })?;
            if pending_prefix != selected_count {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "Queue release authorization lacks the full ReleasePending prefix",
                ));
            }
            released_prefix == selected_count
        } else {
            self.require_autonomous_lane_release_completed_or_superseded_locked(
                &entry, payload, retirement,
            )?;
            true
        };
        AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
            .and_then(|context| {
                context.queue_preparation_authorization(retirement, &barrier, claims_fully_released)
            })
            .map_err(|message| Self::invalid_lane_artifact_error(self.store_root.clone(), message))
    }
    /// Finish the claim half of an exact autonomous-slot release.
    ///
    /// Callers may invoke this only after Queue has durably prepared the
    /// byte-identical ordered barrier. Active claims are rejected here rather
    /// than skipping `ReleasePending`; exact `Released` retries are harmless.
    pub(crate) fn finalize_autonomous_lane_slot_release_with_authorization(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        queue_barrier: &LaneQueueReservationReleaseBarrierV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        authorization: DurableLaneQueueReleaseBarrierAuthorization,
    ) -> Result<AutonomousLaneQueueReleaseFinalizationAuthorization> {
        self.finalize_autonomous_lane_slot_release_inner(
            retirement,
            queue_barrier,
            expected_network_id,
            expected_epoch,
            AutonomousLaneQueueReleaseBarrierGate::Authorized(authorization),
        )
    }
    #[cfg(test)]
    pub(crate) fn finalize_autonomous_lane_slot_release(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        queue_barrier: &LaneQueueReservationReleaseBarrierV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<()> {
        self.finalize_autonomous_lane_slot_release_inner(
            retirement,
            queue_barrier,
            expected_network_id,
            expected_epoch,
            AutonomousLaneQueueReleaseBarrierGate::DirectTest,
        )
        .map(drop)
    }
    fn finalize_autonomous_lane_slot_release_inner(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        queue_barrier: &LaneQueueReservationReleaseBarrierV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        gate: AutonomousLaneQueueReleaseBarrierGate,
    ) -> Result<AutonomousLaneQueueReleaseFinalizationAuthorization> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        if retirement.version != AutonomousLaneSlotRetirementV1::VERSION
            || retirement.network_id != expected_network_id
            || retirement.epoch != expected_epoch
            || retirement.queue_release_barrier()? != *queue_barrier
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lane slot release has an unsupported version or chain context",
            ));
        }
        let entry = self.lane_storage_entry(retirement.lane_id)?;
        let slot_path = Self::autonomous_lane_block_latest_attempt_path_for_entry(
            &entry,
            &self.store_root,
            retirement.lane_block_height,
        );
        let _guard = self.sidecar_lock.lock();
        let record = self
            .read_autonomous_lane_block_attempt_record_locked(
                &entry,
                retirement.lane_id,
                retirement.lane_block_height,
                retirement.proposal_height,
                expected_network_id,
                expected_epoch,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    slot_path,
                    "missing autonomous executable payload for claim release",
                )
            })?;
        if record.retirement.as_ref() != Some(retirement)
            || !retirement.matches_payload(&record.artifact.executable_payload)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous lane claim release does not match its durable retirement",
            ));
        }
        let payload = &record.artifact.executable_payload;
        let finalization_authorization =
            AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
                .and_then(|context| {
                    context.queue_finalization_authorization(retirement, queue_barrier)
                })
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
        let current = self
            .read_current_autonomous_lane_block_record_self_context_locked(
                &entry,
                retirement.lane_block_height,
                Some(pending_canonical_bytes),
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous lane claim release has no current lane-height attempt",
                )
            })?;
        if current.artifact.executable_payload == *payload {
            let (pending_prefix, released_prefix) =
                self.autonomous_lane_entrypoint_claim_release_progress_locked(payload, retirement)?;
            let selected_count = u64::try_from(payload.entrypoint_hashes.len()).map_err(|_| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous release group size exceeds u64",
                )
            })?;
            if pending_prefix != selected_count {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous claim release lacks the full ReleasePending prefix",
                ));
            }
            let terminal_absence =
                gate.consume_for_claim_transition(queue_barrier)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                    })?;
            if terminal_absence && released_prefix != selected_count {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "forgotten Queue release cannot authorize pending Kura claims",
                ));
            }
            self.finalize_autonomous_lane_entrypoint_claim_release_locked(
                pending_canonical_bytes,
                payload,
                retirement,
            )?;
        } else {
            let _terminal_absence =
                gate.consume_for_claim_transition(queue_barrier)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                    })?;
            self.require_autonomous_lane_release_completed_or_superseded_locked(
                &entry, payload, retirement,
            )?;
        }
        Ok(finalization_authorization)
    }
}
