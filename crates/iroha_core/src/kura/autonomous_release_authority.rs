// Durable autonomous-lane Queue release authority and claim finalization.
impl Kura {
    /// Retire one nonproducer replica without consuming the producer's Queue
    /// reservation or ordered-release authority.
    ///
    /// The Queue token is acquired before this method is entered and retains
    /// per-entrypoint transition fences through every Kura fsync below. It can
    /// authenticate only exhaustive local Queue absence or a byte-identical
    /// ordinary FIFO copy. Kura independently reopens the signed cursor and
    /// exact payload before advancing claims and atomically publishing a
    /// replica-specific Complete terminal outcome.
    pub(crate) fn retire_autonomous_lane_slot_with_replica_queue_disposition(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        cursor_read: AutonomousLifecycleCursorRead,
        authorization: AutonomousLaneReplicaQueueDispositionAuthorization<'_>,
    ) -> Result<()> {
        if retirement.version != AutonomousLaneSlotRetirementV1::VERSION
            || retirement.network_id != expected_network_id
            || retirement.epoch != expected_epoch
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement has an unsupported version or chain context",
            ));
        }
        let barrier = retirement.queue_release_barrier()?;
        let expected_cursor = cursor_read.cursor().cloned().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement lacks its completed signed lifecycle cursor",
            )
        })?;
        let (_, local_actor) = expected_cursor.binding().local_validator_identity();
        if local_actor == expected_cursor.binding().producer_actor_projection() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "producer lifecycle cursor cannot use replica Queue retirement",
            ));
        }
        let disposition = authorization
            .consume_for_kura(&cursor_read, &barrier.ordered_keys)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "replica Queue disposition authority changed before Kura retirement",
                )
            })?;
        let (exact_ordinary_fifo_preserved, source_disposition, _queue_fence) = match disposition {
            AutonomousLaneReplicaQueueDisposition::ExactOrdinaryFifo(fence) => (
                true,
                AutonomousLifecycleReplicaQueueDispositionV1::ExactOrdinaryFifo,
                fence,
            ),
            AutonomousLaneReplicaQueueDisposition::StrictQueueAbsent(fence) => (
                false,
                AutonomousLifecycleReplicaQueueDispositionV1::StrictQueueAbsent,
                fence,
            ),
        };

        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(retirement.lane_id)?;
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
                    self.store_root.clone(),
                    "replica retirement lost its exact executable payload",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        if !retirement.matches_payload(payload)
            || payload.reservation_keys.as_slice() != barrier.ordered_keys.as_slice()
        {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "replica retirement differs from its durable payload or ordered Queue group",
            ));
        }
        let durable_cursor =
            self.read_autonomous_lifecycle_cursor_for_terminal_outcome_locked(&entry, payload)?;
        if durable_cursor != expected_cursor
            || durable_cursor.binding().reservation_group_binding()
                != lane_queue_reservation_group_binding_from_ordered_keys(
                    payload.reservation_keys.iter(),
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement signed cursor changed after Queue authorization",
            ));
        }
        let (_, durable_local_actor) = durable_cursor.binding().local_validator_identity();
        if durable_local_actor != local_actor
            || durable_local_actor == durable_cursor.binding().producer_actor_projection()
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement cursor changed its nonproducer actor",
            ));
        }
        let context =
            AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?;
        if context.actor != durable_local_actor
            || context.actor == context.producer
            || context.reservation_group != durable_cursor.binding().reservation_group_binding()
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement projection changed its signed actor or reservation group",
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
                    "replica retirement has no current lane attempt",
                )
            })?;
        let exact_attempt_is_current = current.artifact.executable_payload == *payload;
        self.persist_autonomous_lane_slot_retirement_for_replica_locked(
            pending_canonical_bytes,
            &entry,
            &record,
            retirement,
            expected_network_id,
            expected_epoch,
            exact_attempt_is_current,
        )?;
        let selected_count = u64::try_from(payload.entrypoint_hashes.len())?;
        let released_prefix = if exact_attempt_is_current {
            self.prepare_autonomous_lane_entrypoint_claim_release_for_replica_locked(
                pending_canonical_bytes,
                payload,
                retirement,
                exact_ordinary_fifo_preserved,
            )?;
            let (pending_prefix, released_prefix) = self
                .autonomous_lane_entrypoint_replica_claim_release_progress_locked(
                    payload,
                    retirement,
                    source_disposition,
                )?;
            if pending_prefix != selected_count || released_prefix > pending_prefix {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "replica retirement lacks the complete canonical claim prefix",
                ));
            }
            released_prefix
        } else {
            self.require_autonomous_lane_replica_release_completed_or_superseded_locked(
                &entry,
                payload,
                retirement,
                source_disposition,
            )?;
            selected_count
        };
        let expected_reservation_state = if exact_ordinary_fifo_preserved {
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED
        } else {
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT
        };
        if exact_attempt_is_current && released_prefix == 0 {
            let observed = context
                .observe_replica_queue_release_transition(exact_ordinary_fifo_preserved)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                })?
                .into_projection();
            if observed.after.queue.reservation_state != expected_reservation_state {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "checked replica Queue observation changed its terminal disposition",
                ));
            }
        } else {
            // A nonzero prefix consists only of replica-specific claims that
            // durably bind this exact Queue disposition. Reconstruct that
            // crash prefix instead of replaying action 28 from prefix zero.
            let recovered =
                context.replica_queue_release_state(exact_ordinary_fifo_preserved, released_prefix);
            if !production_in_flight_first_release_state_kernel(recovered)
                || recovered.queue.reservation_state != expected_reservation_state
                || recovered.release.released_prefix != released_prefix
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "replica retirement failed to reconstruct its durable Queue disposition prefix",
                ));
            }
        }
        if exact_attempt_is_current {
            self.finalize_autonomous_lane_entrypoint_claim_release_for_replica_locked(
                pending_canonical_bytes,
                payload,
                retirement,
                exact_ordinary_fifo_preserved,
            )?;
            self.require_autonomous_lane_entrypoint_claims_released_for_replica_locked(
                payload,
                retirement,
                source_disposition,
            )?;
        }
        let source = AutonomousLifecycleTerminalOutcomeSourceV1::RetiredReplicaQueueDisposition {
            retirement_hash: retirement.digest()?,
            queue_disposition: source_disposition,
        };
        self.autonomous_lifecycle_terminal_source_matches_replica_queue_disposition_locked(
            Some(pending_canonical_bytes),
            &entry,
            payload,
            Some(retirement),
            source,
        )?;
        let terminal = context.replica_queue_terminal_state(exact_ordinary_fifo_preserved);
        if !production_in_flight_first_release_state_kernel(terminal)
            || production_in_flight_first_release_terminal_owner(terminal).is_none()
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica retirement failed its terminal ownership projection",
            ));
        }
        let complete = self.persist_autonomous_lifecycle_replica_terminal_outcome_complete_locked(
            pending_canonical_bytes,
            &entry,
            payload,
            source,
            terminal,
        )?;
        self.complete_autonomous_lane_entrypoint_claims_released_for_replica_locked(
            pending_canonical_bytes,
            Some(pending_canonical_bytes),
            payload,
            retirement,
            source_disposition,
            &complete,
        )?;
        // `_queue_fence` was declared before every Kura guard above. Rust's
        // reverse drop order therefore releases sidecar/geometry/canonical/
        // prune locks first and the Queue fence only after the Complete write
        // and directory fsync, avoiding a Kura -> Queue lock edge.
        Ok(())
    }

    /// Persist the retirement while the caller retains the same sidecar lock
    /// used to revalidate the Queue-authorized signed cursor. Replica claim
    /// preparation remains in the caller because it must use the
    /// disposition-specific durable claim state.
    fn persist_autonomous_lane_slot_retirement_for_replica_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        record: &AutonomousLaneBlockDurableRecord,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        exact_attempt_is_current: bool,
    ) -> Result<()> {
        if let Some(existing) = record.retirement.as_ref() {
            if existing == retirement {
                return Ok(());
            }
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path.clone(),
                "conflicting autonomous replica slot retirement is already durable",
            ));
        }
        if !exact_attempt_is_current {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path.clone(),
                "historical replica attempt lacks its previously durable retirement",
            ));
        }
        let (certified_data_path, certified_index_path) =
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
        if self
            .read_active_certified_lane_block_artifact_from_paths_durability_attested_locked(
                entry,
                retirement.lane_block_height,
                &certified_data_path,
                &certified_index_path,
                true,
            )
            .is_some()
        {
            return Err(Self::invalid_lane_artifact_error(
                certified_data_path,
                "certified autonomous lane block cannot be retired as a replica",
            ));
        }
        let mut state = AutonomousLaneBlockViewState::from_artifact(&record.artifact);
        state.retirement = Some(retirement.clone());
        let authorization = self
            .authorize_autonomous_lane_slot_retirement_persistence(
                &record.artifact.executable_payload,
                retirement,
                &record.view_state_path,
            )
            .map_err(|message| {
                Self::invalid_lane_artifact_error(record.view_state_path.clone(), message)
            })?;
        let projection: ProductionInFlightFirstReleaseTransitionProjection = authorization
            .consume_for_persistence(
                &record.artifact.executable_payload,
                retirement,
                &record.view_state_path,
            )
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    record.view_state_path.clone(),
                    "autonomous replica slot-retirement authority changed before persistence",
                )
            })?;
        if projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path.clone(),
                "autonomous replica slot-retirement authority names another transition",
            ));
        }
        let checked =
            check_production_in_flight_first_release_transition(projection).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    record.view_state_path.clone(),
                    "autonomous replica slot-retirement persistence failed the composed transition gate",
                )
            })?;
        if checked.into_projection() != projection {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path.clone(),
                "checked autonomous replica slot-retirement projection changed before persistence",
            ));
        }
        self.write_autonomous_lane_block_view_state_record_locked(
            pending_canonical_bytes,
            &record.artifact.executable_payload,
            &state,
            &record.view_state_path,
            expected_network_id,
            expected_epoch,
        )?;
        Ok(())
    }

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
    /// Build an exact replica `ReplicaReleased*` crash cut without publishing
    /// its terminal outcome. Production must use Queue's move-only fence via
    /// `retire_autonomous_lane_slot_with_replica_queue_disposition`.
    #[cfg(test)]
    pub(crate) fn finalize_autonomous_lane_slot_replica_release_for_test(
        &self,
        retirement: &AutonomousLaneSlotRetirementV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        queue_disposition: AutonomousLifecycleReplicaQueueDispositionV1,
    ) -> Result<()> {
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
                "replica claim test cut has an unsupported version or chain context",
            ));
        }
        let entry = self.lane_storage_entry(retirement.lane_id)?;
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
                    self.store_root.clone(),
                    "replica claim test cut lost its exact payload attempt",
                )
            })?;
        let payload = &record.artifact.executable_payload;
        if record.retirement.as_ref() != Some(retirement) || !retirement.matches_payload(payload) {
            return Err(Self::invalid_lane_artifact_error(
                record.view_state_path,
                "replica claim test cut differs from its durable retirement",
            ));
        }
        let exact_ordinary_fifo_preserved =
            queue_disposition == AutonomousLifecycleReplicaQueueDispositionV1::ExactOrdinaryFifo;
        self.prepare_autonomous_lane_entrypoint_claim_release_for_replica_locked(
            pending_canonical_bytes,
            payload,
            retirement,
            exact_ordinary_fifo_preserved,
        )?;
        let (_, released_prefix) = self
            .autonomous_lane_entrypoint_replica_claim_release_progress_locked(
                payload,
                retirement,
                queue_disposition,
            )?;
        if released_prefix != 0
            || AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)
                .and_then(|context| {
                    context.observe_replica_queue_release_transition(exact_ordinary_fifo_preserved)
                })
                .is_err()
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica claim test cut does not begin at checked action 28",
            ));
        }
        self.finalize_autonomous_lane_entrypoint_claim_release_for_replica_locked(
            pending_canonical_bytes,
            payload,
            retirement,
            exact_ordinary_fifo_preserved,
        )
    }
    /// Recreate the crash cut after a Complete replica outcome was synced but
    /// before its disposition-bound claims were fully sealed.
    #[cfg(test)]
    pub(crate) fn downgrade_autonomous_lane_replica_complete_claim_suffix_for_test(
        &self,
        payload: &LaneExecutablePayloadV1,
        suffix_len: usize,
    ) -> Result<()> {
        if suffix_len == 0 || suffix_len > payload.entrypoint_hashes.len() {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "replica Complete crash-cut suffix is empty or out of bounds",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(payload.origin_proposal.descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, &payload.origin_proposal.descriptor)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        for entrypoint_hash in payload
            .entrypoint_hashes
            .iter()
            .skip(payload.entrypoint_hashes.len() - suffix_len)
        {
            let path = Self::autonomous_lane_entrypoint_claim_path(
                &self.store_root,
                &payload.network_id,
                entrypoint_hash,
            );
            let mut claim = Self::decode_autonomous_lane_entrypoint_claim(&path)
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
            if !claim.owns_payload(payload)
                || !self.autonomous_lane_entrypoint_claim_path_matches(&claim, &path)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "replica Complete crash-cut claim differs from its payload or hash path",
                ));
            }
            let AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(
                retirement_hash,
                queue_disposition,
                _,
            ) = claim.state
            else {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "replica Complete crash-cut requires an already sealed claim suffix",
                ));
            };
            claim.state = AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(
                retirement_hash,
                queue_disposition,
            );
            let bytes = norito::encode_canonical(&claim).map_err(Error::NoritoFrame)?;
            if bytes.is_empty() || bytes.len() > AUTONOMOUS_LANE_ENTRYPOINT_CLAIM_MAX_BYTES {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "replica Complete crash-cut claim exceeds its hard byte limit",
                ));
            }
            let before = Self::file_len_or_zero(&path)?;
            self.write_atomic_synced_replace(&path, &bytes)?;
            let after = Self::file_len_or_zero(&path)?;
            self.update_disk_usage_delta(before, after);
        }
        accounting_mutation.finish();
        Ok(())
    }
    /// Count exact raw and Complete replica claims for one payload.
    #[cfg(test)]
    pub(crate) fn autonomous_lane_replica_claim_seal_counts_for_test(
        &self,
        payload: &LaneExecutablePayloadV1,
    ) -> Result<(usize, usize)> {
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(payload.origin_proposal.descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, &payload.origin_proposal.descriptor)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let mut raw = 0_usize;
        let mut complete = 0_usize;
        for entrypoint_hash in &payload.entrypoint_hashes {
            let path = Self::autonomous_lane_entrypoint_claim_path(
                &self.store_root,
                &payload.network_id,
                entrypoint_hash,
            );
            let claim = Self::decode_autonomous_lane_entrypoint_claim(&path)
                .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
            if !claim.owns_payload(payload)
                || !self.autonomous_lane_entrypoint_claim_path_matches(&claim, &path)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "replica claim seal count differs from its payload or hash path",
                ));
            }
            match claim.state {
                AutonomousLaneEntrypointClaimStateV1::ReplicaReleased(_, _) => raw += 1,
                AutonomousLaneEntrypointClaimStateV1::ReplicaReleasedComplete(_, _, _) => {
                    complete += 1;
                }
                _ => {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "replica claim seal count found a non-replica claim",
                    ));
                }
            }
        }
        Ok((raw, complete))
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
