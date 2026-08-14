impl LifecycleLedgerV1 {
    /// Hash the canonical V1 encoding, independent of its store path.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn frame_identity(&self) -> LifecycleDigest {
        let mut preimage = Vec::from(&b"iroha:sumeragi:v2:lifecycle-ledger-frame:v1"[..]);
        preimage.extend_from_slice(&self.encode());
        LifecycleDigest::new(*Hash::new(preimage).as_ref())
    }

    pub(super) fn from_coordinator(
        coordinator: &LifecycleCoordinator,
    ) -> Result<Self, LifecycleLedgerError> {
        let records = coordinator
            .records
            .values()
            .map(|record| {
                let metadata = coordinator
                    .durable_records
                    .get(&record.ordinal)
                    .ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "logical record has no durable reconstruction metadata".to_owned(),
                        )
                    })?;
                let terminal = match record.state {
                    LifecycleState::Terminal(outcome) => Some(outcome),
                    LifecycleState::Waiting(_)
                    | LifecycleState::Ready
                    | LifecycleState::Claimed(_) => None,
                };
                LifecycleLedgerRecordV1::new(
                    record.key,
                    record.owner,
                    record.ordinal,
                    record.work_class,
                    record.stage,
                    terminal,
                    metadata.reconstruction_source,
                    metadata.payload,
                    metadata.replay_authority.clone(),
                    metadata.continuation,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            coordinator.active_context,
            coordinator.high_water,
            records,
            coordinator.producer_debts.clone(),
        )
    }

    pub(super) fn recovery_snapshot(
        &self,
        mut physical_slot_universes: BTreeMap<u128, BTreeSet<PhysicalSlotId>>,
    ) -> Result<RecoverySnapshot, LifecycleLedgerError> {
        if physical_slot_universes.len() != self.records.len()
            || self
                .records
                .iter()
                .any(|record| !physical_slot_universes.contains_key(&record.ordinal))
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "storage reconciliation does not cover every durable record exactly once"
                    .to_owned(),
            ));
        }
        let records = self
            .records
            .iter()
            .map(|record| {
                let key = record.key().ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "durable record key cannot be decoded".to_owned(),
                    )
                })?;
                Ok(RecoveredLifecycleRecord::new(
                    key,
                    record.owner(),
                    record.ordinal(),
                    record.work_class().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable work class cannot be decoded".to_owned(),
                        )
                    })?,
                    record.stage().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable stage cannot be decoded".to_owned(),
                        )
                    })?,
                    record.terminal().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable terminal cannot be decoded".to_owned(),
                        )
                    })?,
                    record.reconstruction_source(),
                    record.durable_payload().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable payload cannot be decoded".to_owned(),
                        )
                    })?,
                    record.replay_authority.clone(),
                    record.continuation().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable continuation cannot be decoded".to_owned(),
                        )
                    })?,
                    physical_slot_universes
                        .remove(&record.ordinal)
                        .expect("exact coverage checked above"),
                ))
            })
            .collect::<Result<Vec<_>, LifecycleLedgerError>>()?;
        let producer_debts = self
            .producer_debts
            .iter()
            .map(|debt| (debt.serve_ordinal(), debt.producer_ordinal()))
            .collect();
        Ok(RecoverySnapshot::new(
            self.context(),
            self.high_water(),
            records,
            producer_debts,
        ))
    }

    /// Construct and validate a canonical durable ledger.
    pub(super) fn new(
        context: LifecycleContext,
        high_water: u128,
        mut records: Vec<LifecycleLedgerRecordV1>,
        producer_debts: BTreeMap<u128, u128>,
    ) -> Result<Self, LifecycleLedgerError> {
        records.sort_by_key(LifecycleLedgerRecordV1::ordinal);
        let producer_debts = producer_debts
            .into_iter()
            .map(|(serve, producer)| LifecycleProducerDebtV1::new(serve, producer))
            .collect();
        let ledger = Self {
            format_version: LEDGER_VERSION,
            context: *context.id().as_bytes(),
            height: context.height(),
            high_water,
            records,
            producer_debts,
        };
        ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        Ok(ledger)
    }

    /// Construct the empty ledger for a validated height context.
    pub(super) fn empty(context: LifecycleContext) -> Self {
        Self {
            format_version: LEDGER_VERSION,
            context: *context.id().as_bytes(),
            height: context.height(),
            high_water: 0,
            records: Vec::new(),
            producer_debts: Vec::new(),
        }
    }

    /// Return the typed persisted context.
    pub(super) const fn context(&self) -> LifecycleContext {
        LifecycleContext::new(LifecycleDigest::new(self.context), self.height)
    }

    /// Return the durable ordinal high-water mark.
    pub(super) const fn high_water(&self) -> u128 {
        self.high_water
    }

    /// Borrow the canonical ordinal-ordered records.
    pub(super) fn records(&self) -> &[LifecycleLedgerRecordV1] {
        &self.records
    }

    /// Classify every exact committed Broadcast-plus-next-Sign pair in this frame.
    ///
    /// Classification is purely durable and does not decode any runtime WAL or
    /// body authority. A valid ledger may retain unrelated later records, so a
    /// pair is selected by its transaction-local Broadcast/next-Sign adjacency
    /// rather than by the global high-water mark.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(
        &self,
    ) -> Result<Vec<RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1>, LifecycleLedgerError>
    {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        let ledger_frame_identity = self.frame_identity();
        let index = RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records);
        Ok(self
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.stage().map(LifecycleStage::kind),
                    Some(
                        LifecycleStageKind::BroadcastProposal
                            | LifecycleStageKind::BroadcastPrepareVote
                    )
                )
            })
            .filter_map(|record| {
                self.project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(
                    record.ordinal(),
                    ledger_frame_identity,
                    &index,
                )
            })
            .collect())
    }

    fn project_recovered_lifecycle_signed_broadcast_and_sign_at(
        &self,
        broadcast_ordinal: u128,
    ) -> Option<RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).ok()?;
        let index = RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records);
        self.project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(
            broadcast_ordinal,
            self.frame_identity(),
            &index,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(
        &self,
        broadcast_ordinal: u128,
        ledger_frame_identity: LifecycleDigest,
        index: &RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1,
    ) -> Option<RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1> {
        let broadcast = self
            .records
            .binary_search_by_key(&broadcast_ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| self.records.get(index))?;
        let next_sign_ordinal = broadcast_ordinal.checked_add(1)?;
        let next_sign = self
            .records
            .binary_search_by_key(&next_sign_ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| self.records.get(index))?;

        let parent = self
            .records
            .get(index.unique_parent_index(broadcast_ordinal)?)?;
        let (broadcast_edge, observed_broadcast_ordinal) =
            parent.continuation()?.successor_parts()?;
        let parent_key = parent.key()?;
        let parent_stage = parent.stage()?;
        let parent_payload = parent.durable_payload()?;
        let broadcast_key = broadcast.key()?;
        let broadcast_stage = broadcast.stage()?;
        let broadcast_payload = broadcast.durable_payload()?;
        let next_sign_key = next_sign.key()?;
        let next_sign_stage = next_sign.stage()?;
        let parent_owner = parent.owner();
        let next_sign_owner = next_sign.owner();
        let parent_ordinal = parent.ordinal();

        if observed_broadcast_ordinal != broadcast_ordinal
            || parent.ordinal() >= broadcast_ordinal
            || parent.terminal()? != Some(TerminalOutcome::Advanced)
            || parent_stage.predecessor_scope() != PredecessorScope::Independent
            || parent_payload != DurablePayloadReference::None
            || parent.reconstruction_source() != parent_owner.causal_root().digest()
            || broadcast.owner() != parent_owner
            || broadcast.work_class() != Some(LifecycleWorkClass::Broadcast)
            || broadcast_stage.predecessor_scope() != PredecessorScope::Independent
            || broadcast.terminal()? != None
            || broadcast.reconstruction_source() != parent.reconstruction_source()
            || broadcast_payload != DurablePayloadReference::None
            || broadcast.continuation()? != DurableContinuation::None
            || broadcast
                .project_recovered_signed_broadcast_child(self.context())
                .is_none()
            || !durable_continuation_successor_is_exact(
                broadcast_edge,
                parent.work_class()?,
                parent_key,
                parent_stage,
                LifecycleWorkClass::Broadcast,
                broadcast_key,
                broadcast_stage,
            )
            || signed_broadcast_continuation_is_exact(
                broadcast_edge,
                &parent.replay_authority,
                parent_payload,
                &broadcast.replay_authority,
                broadcast_payload,
            ) != Some(true)
            || next_sign.work_class() != Some(LifecycleWorkClass::SignVote)
            || next_sign_stage.predecessor_scope() != PredecessorScope::Independent
            || next_sign.terminal()? != None
            || next_sign.durable_payload()? != DurablePayloadReference::None
            || next_sign.continuation()? != DurableContinuation::None
            || next_sign_owner == parent_owner
            || next_sign_owner.first_admission_ordinal() != next_sign_ordinal
            || next_sign.reconstruction_source() != next_sign_owner.causal_root().digest()
            || index.owner_record_count(next_sign_owner) != 1
            || index.has_incoming_edge(next_sign_ordinal)
            || !recovered_broadcast_and_next_sign_keys_are_exact(
                broadcast_key,
                broadcast_stage,
                next_sign_key,
                next_sign_stage,
            )
            || !next_sign.replay_authority.structurally_matches_record(
                self.context(),
                next_sign_key,
                LifecycleWorkClass::SignVote,
                next_sign_stage,
                DurablePayloadReference::None,
            )
        {
            return None;
        }

        let parent_record_count = index.owner_record_count(parent_owner);
        let parent_classification = match (
            parent_key.phase(),
            parent.work_class()?,
            parent_stage.kind(),
            broadcast_edge,
            broadcast_key.phase(),
            broadcast_stage.kind(),
        ) {
            (
                LifecyclePhase::Proposal,
                LifecycleWorkClass::SignProposal,
                LifecycleStageKind::SignProposal,
                DurableContinuationEdge::SignProposalToBroadcast,
                LifecyclePhase::BroadcastProposal,
                LifecycleStageKind::BroadcastProposal,
            ) if parent_record_count == 2
                && parent_owner.first_admission_ordinal() == parent.ordinal()
                && !index.has_incoming_edge(parent.ordinal()) =>
            {
                RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
            }
            (
                LifecyclePhase::Prepare,
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignPrepareVote,
                DurableContinuationEdge::SignPrepareToBroadcast,
                LifecyclePhase::BroadcastPrepareVote,
                LifecycleStageKind::BroadcastPrepareVote,
            ) if parent_record_count == 3 => {
                let validate = self
                    .records
                    .get(index.unique_parent_index(parent.ordinal())?)?;
                let validate_key = validate.key()?;
                let validate_stage = validate.stage()?;
                let validate_payload = validate.durable_payload()?;
                if validate.ordinal() >= parent.ordinal()
                    || validate.owner() != parent_owner
                    || validate.ordinal() != parent_owner.first_admission_ordinal()
                    || validate.work_class() != Some(LifecycleWorkClass::Validate)
                    || validate_key.phase() != LifecyclePhase::Validate
                    || validate_stage
                        != LifecycleStage::new(
                            LifecycleStageKind::ValidateBody,
                            PredecessorScope::Independent,
                        )
                    || validate.terminal()? != Some(TerminalOutcome::Advanced)
                    || validate.reconstruction_source() != parent.reconstruction_source()
                    || !durable_validate_payload_is_exact(validate_key, validate_payload)
                    || !durable_continuation_payload_is_exact(
                        DurableContinuationEdge::ValidateToSignPrepare,
                        validate_payload,
                        parent_payload,
                    )
                    || validate.continuation()?.successor_parts()
                        != Some((
                            DurableContinuationEdge::ValidateToSignPrepare,
                            parent.ordinal(),
                        ))
                    || !durable_continuation_successor_is_exact(
                        DurableContinuationEdge::ValidateToSignPrepare,
                        LifecycleWorkClass::Validate,
                        validate_key,
                        validate_stage,
                        LifecycleWorkClass::SignVote,
                        parent_key,
                        parent_stage,
                    )
                {
                    return None;
                }
                RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                    validate_ordinal: validate.ordinal(),
                }
            }
            _ => return None,
        };

        Some(RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1 {
            ledger_frame_identity,
            parent: parent_classification,
            parent_ordinal,
            broadcast_ordinal,
            next_sign_ordinal,
        })
    }

    /// Stage the exact all-row tombstone successor for finalized-height retirement.
    ///
    /// Existing terminal rows remain byte-for-byte unchanged. Every live
    /// Certified-Serve row must consume one payload-store-authenticated
    /// terminal update together with its adjacent live ProducerTurn; an
    /// already-terminal Serve with a live ProducerTurn must consume the
    /// corresponding no-update coverage proof. Every other live row becomes a
    /// `Cancelled` tombstone without changing its immutable admission or replay
    /// material. No durable write occurs in this method.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn stage_finalized_height_all_row_retirement(
        &self,
        mut serve_reconciliation: CompleteTipServeRetirementReconciliationV1,
    ) -> Result<StagedFinalizationRetirementV1, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !serve_reconciliation.authenticates_source(self) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized-height Serve retirement belongs to another ledger frame".to_owned(),
            ));
        }

        let mut consumed_producers = BTreeSet::new();
        let mut retired_records = Vec::with_capacity(self.records.len());
        for record in &self.records {
            if consumed_producers.remove(&record.ordinal()) {
                continue;
            }
            let work_class = record.work_class().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "finalized-height retirement encountered an undecodable work class".to_owned(),
                )
            })?;
            let terminal = record.terminal().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "finalized-height retirement encountered an undecodable terminal state"
                        .to_owned(),
                )
            })?;

            if work_class == LifecycleWorkClass::CertifiedServe {
                let producer_ordinal = record.ordinal().checked_add(1).ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "finalized-height Serve producer ordinal exhausted".to_owned(),
                    )
                })?;
                let producer = self
                    .records
                    .binary_search_by_key(&producer_ordinal, LifecycleLedgerRecordV1::ordinal)
                    .ok()
                    .and_then(|index| self.records.get(index))
                    .ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "finalized-height Serve lost its adjacent ProducerTurn".to_owned(),
                        )
                    })?;
                if producer.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                    || producer.owner() != record.owner()
                    || producer.terminal().is_none()
                {
                    return Err(LifecycleLedgerError::InvalidLedger(
                        "finalized-height Serve/ProducerTurn pair changed before retirement"
                            .to_owned(),
                    ));
                }

                match (
                    terminal,
                    producer.terminal().expect("producer terminal decoded"),
                ) {
                    (None, None) => {
                        let update = serve_reconciliation
                            .take_terminal_update_for_exact_pair(record, producer)
                            .ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "live finalized-height Serve has no exact terminal payload update"
                                        .to_owned(),
                                )
                            })?;
                        let (payload, outcome, serve_replay, producer_replay) = update
                            .consume_for_exact_ledger_pair(record, producer)
                            .ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "finalized-height Serve terminal update changed before staging"
                                        .to_owned(),
                                )
                            })?;
                        retired_records.push(Self::terminalized_record(
                            record,
                            outcome,
                            payload,
                            serve_replay,
                        )?);
                        retired_records.push(Self::terminalized_record(
                            producer,
                            TerminalOutcome::Cancelled,
                            producer.durable_payload().ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "finalized-height ProducerTurn payload is undecodable"
                                        .to_owned(),
                                )
                            })?,
                            producer_replay,
                        )?);
                        consumed_producers.insert(producer_ordinal);
                    }
                    (Some(_), None) => {
                        if !serve_reconciliation
                            .take_terminal_serve_live_producer_coverage(record, producer)
                        {
                            return Err(LifecycleLedgerError::InvalidLedger(
                                "terminal finalized-height Serve has no exact live ProducerTurn coverage"
                                    .to_owned(),
                            ));
                        }
                        retired_records.push(record.clone());
                        retired_records.push(Self::cancelled_record(producer)?);
                        consumed_producers.insert(producer_ordinal);
                    }
                    (Some(_), Some(_)) => {
                        retired_records.push(record.clone());
                        retired_records.push(producer.clone());
                        consumed_producers.insert(producer_ordinal);
                    }
                    (None, Some(_)) => {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "live finalized-height Serve has an already-terminal ProducerTurn"
                                .to_owned(),
                        ));
                    }
                }
                continue;
            }

            if work_class == LifecycleWorkClass::ProducerTurn {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "finalized-height ProducerTurn was not consumed by its exact Serve owner"
                        .to_owned(),
                ));
            }
            retired_records.push(if terminal.is_some() {
                record.clone()
            } else {
                Self::cancelled_record(record)?
            });
        }

        if !consumed_producers.is_empty() || !serve_reconciliation.is_drained() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized-height Serve retirement census was not consumed exactly once"
                    .to_owned(),
            ));
        }
        let retired = Self::new(
            self.context(),
            self.high_water,
            retired_records,
            BTreeMap::new(),
        )?;
        if retired.high_water != self.high_water
            || retired.records.len() != self.records.len()
            || retired
                .records
                .iter()
                .any(|record| record.terminal() == Some(None))
            || !retired.producer_debts.is_empty()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized-height all-row retirement did not reach the exact quiescent frame"
                    .to_owned(),
            ));
        }
        Ok(StagedFinalizationRetirementV1 {
            current: self.clone(),
            retired,
        })
    }

    fn cancelled_record(
        record: &LifecycleLedgerRecordV1,
    ) -> Result<LifecycleLedgerRecordV1, LifecycleLedgerError> {
        let payload = record.durable_payload().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "finalized-height retirement encountered an undecodable payload".to_owned(),
            )
        })?;
        Self::terminalized_record(
            record,
            TerminalOutcome::Cancelled,
            payload,
            record.replay_authority.clone(),
        )
    }

    fn terminalized_record(
        record: &LifecycleLedgerRecordV1,
        outcome: TerminalOutcome,
        payload: DurablePayloadReference,
        replay_authority: LifecycleReplayAuthorityV1,
    ) -> Result<LifecycleLedgerRecordV1, LifecycleLedgerError> {
        if record.terminal() != Some(None)
            || record.continuation() != Some(DurableContinuation::None)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized-height retirement can terminalize only one live uncontinued row"
                    .to_owned(),
            ));
        }
        LifecycleLedgerRecordV1::new(
            record.key().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "finalized-height retirement encountered an undecodable key".to_owned(),
                )
            })?,
            record.owner(),
            record.ordinal(),
            record.work_class().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "finalized-height retirement encountered an undecodable work class".to_owned(),
                )
            })?,
            record.stage().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable stage".to_owned(),
                )
            })?,
            Some(outcome),
            record.reconstruction_source(),
            payload,
            replay_authority,
            DurableContinuation::None,
        )
    }

    /// Authenticate every live BodyFrame-backed Fetch against this exact frame.
    ///
    /// Unlike the consuming storage-cut constructor, this internal phase keeps
    /// the ledger and body store borrowed. Recovered-WAL startup uses it only
    /// after the exact Validate-to-Sign repair is fsynced, so the resulting
    /// census is bound to the final frame which the coordinator will open.
    pub(super) fn authenticate_durable_certified_fetch_startup(
        &self,
        verified: &VerifiedHeightContext,
        store: &V2BodyStore,
    ) -> Result<
        super::replay_authority::PreparedDurableCertifiedFetchStartupV1,
        DurableCertifiedFetchRecoveryError,
    > {
        self.authenticate_durable_certified_fetch_census(verified, store)?
            .into_startup(self)
            .ok_or(DurableCertifiedFetchRecoveryError::InvalidStorageCut)
    }

    fn authenticate_durable_certified_fetch_census(
        &self,
        verified: &VerifiedHeightContext,
        store: &V2BodyStore,
    ) -> Result<
        AuthenticatedRecoveredDurableCertifiedFetchCensusV1,
        DurableCertifiedFetchRecoveryError,
    > {
        if self.context() != projection::lifecycle_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext);
        }
        if !store.matches_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext);
        }
        let mut entries = Vec::new();
        for record in self.records.iter().filter(|record| {
            record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.terminal() == Some(None)
                && matches!(
                    record.durable_payload(),
                    Some(DurablePayloadReference::BodyFrame(_))
                )
        }) {
            let Some(DurablePayloadReference::BodyFrame(reference)) = record.durable_payload()
            else {
                return Err(DurableCertifiedFetchRecoveryError::InvalidLedgerRow);
            };
            entries.push(
                record
                    .authenticate_durable_certified_fetch(verified, || {
                        projection::authenticate_durable_body_frame_recovery(
                            self.context(),
                            store,
                            reference,
                        )
                    })?
                    .ok_or(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)?,
            );
        }
        let census = seal_recovered_durable_certified_fetch_census(
            DurableCertifiedFetchLedgerCensusPermit::new(self),
            entries,
        )
        .ok_or(DurableCertifiedFetchRecoveryError::AmbiguousCensus)?;
        Ok(census)
    }

    /// Consume this exact opened ledger and body store into one Ready-Fetch cut.
    ///
    /// Authentication censes every live BodyFrame-backed Fetch row before
    /// either storage owner is moved. Success retains both owners and the
    /// verified context beside the opaque census, preventing a caller from
    /// reminting individual rows or swapping a foreign store before the future
    /// coordinator-open/registry-install transaction consumes the whole cut.
    /// Every error is startup-fatal for these opened instances and consumes
    /// both storage owners; callers must abort this process startup rather than
    /// reopen either path and retry in-process with partially observed state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn into_durable_certified_fetch_storage_recovery_cut(
        self,
        verified: VerifiedHeightContext,
        ledger_store: LifecycleLedgerStoreV1,
        store: V2BodyStore,
    ) -> Result<
        AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1,
        DurableCertifiedFetchRecoveryError,
    > {
        if self.context() != projection::lifecycle_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext);
        }
        if ledger_store.context != self.context()
            || !ledger_store.load().is_ok_and(|opened| opened == self)
        {
            return Err(DurableCertifiedFetchRecoveryError::InvalidLedgerStore);
        }
        if !store.matches_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext);
        }
        let census = self.authenticate_durable_certified_fetch_census(&verified, &store)?;
        let cut = AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1 {
            verified,
            ledger_store,
            ledger: self,
            body_store: store,
            census,
        };
        cut.is_exact()
            .then_some(cut)
            .ok_or(DurableCertifiedFetchRecoveryError::InvalidStorageCut)
    }

    #[cfg(test)]
    /// Substitute one structurally valid but foreign replay origin generation.
    pub(super) fn with_foreign_replay_authority_for_test(&self, ordinal: u128) -> Option<Self> {
        let mut changed = self.clone();
        let record = changed
            .records
            .iter_mut()
            .find(|record| record.ordinal == ordinal)?;
        record.replay_authority = record
            .replay_authority
            .with_foreign_origin_generation_for_test()?;
        changed.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).ok()?;
        Some(changed)
    }

    /// Borrow the canonical Serve-to-producer debts.
    pub(super) fn producer_debts(&self) -> &[LifecycleProducerDebtV1] {
        &self.producer_debts
    }

    /// Authenticate the unique live or already-repaired Validate parent of one WAL vote.
    ///
    /// This read-only projection binds the complete WAL identity to the exact
    /// LedgerV1 owner and admission ordinal. It accepts only the two crash
    /// states surrounding the existing fsync seam: a live uncontinued parent,
    /// or the exact Advanced-parent/live-Sign-child pair.
    pub(super) fn authenticate_recovered_wal_validate_parent(
        &self,
        recovered: &RecoveredWalVoteSign,
    ) -> Option<AuthenticatedRecoveredWalValidateLedgerParent> {
        self.authenticate_recovered_wal_validate_parent_shape(recovered, false)
    }

    /// Authenticate the Validate parent of an already-fsynced Sign-to-Broadcast edge.
    ///
    /// This is deliberately distinct from the live/repaired-Sign stutter above:
    /// it requires the recovered Sign itself to be `Advanced` to one live
    /// Broadcast child. The later WAL repair and verified-height projection
    /// rejoin the complete signed envelope before any carrier is installed.
    pub(super) fn authenticate_recovered_wal_validate_parent_for_signed_broadcast(
        &self,
        recovered: &RecoveredWalVoteSign,
    ) -> Option<AuthenticatedRecoveredWalValidateLedgerParent> {
        self.authenticate_recovered_wal_validate_parent_shape(recovered, true)
    }

    fn authenticate_recovered_wal_validate_parent_shape(
        &self,
        recovered: &RecoveredWalVoteSign,
        require_signed_broadcast: bool,
    ) -> Option<AuthenticatedRecoveredWalValidateLedgerParent> {
        let vote = recovered.vote();
        let mut context_bytes = [0_u8; 32];
        context_bytes.copy_from_slice(vote.round.context_id.0.as_ref());
        let wal_authority_is_exact = match vote.phase {
            wire::GlobalPhase::Prepare => recovered.prepare_certificate().is_none(),
            wire::GlobalPhase::Commit => recovered.prepare_certificate().is_some_and(|prepare| {
                prepare.phase == wire::GlobalPhase::Prepare
                    && prepare.round == vote.round
                    && prepare.proposal_round == vote.proposal_round
                    && prepare.subject == vote.subject
                    && prepare.execution_commitment == vote.execution_commitment
            }),
        };
        let tag_matches_vote = recovered.tag().height() == vote.round.height
            && match vote.phase {
                wire::GlobalPhase::Prepare => recovered.tag().view() == vote.round.view,
                wire::GlobalPhase::Commit => recovered.tag().view() >= vote.round.view,
            };
        if !recovered.wal_identity().is_exact()
            || !recovered.replay_evidence_is_exact()
            || !wal_authority_is_exact
            || self.context().id() != LifecycleDigest::new(context_bytes)
            || self.context().height() != vote.round.height
            || vote.proposal_round.context_id != vote.round.context_id
            || vote.proposal_round.height != vote.round.height
            || !tag_matches_vote
        {
            return None;
        }
        let subject = projection::block_subject(vote.subject);
        let commitment = projection::execution_commitment(vote.execution_commitment);
        let round = LifecycleRound::new(vote.round.height, vote.round.view);
        let proposal_round =
            LifecycleRound::new(vote.proposal_round.height, vote.proposal_round.view);
        let mut parents = self.records.iter().filter(|record| {
            let Some(key) = record.key() else {
                return false;
            };
            let authority_is_exact = match vote.phase {
                wire::GlobalPhase::Prepare => key.execution_commitment().is_none(),
                wire::GlobalPhase::Commit => key
                    .execution_commitment()
                    .is_none_or(|candidate| candidate == commitment),
            };
            key.context() == self.context().id()
                && key.round() == round
                && key.proposal_round() == Some(proposal_round)
                && key.subject() == Some(subject)
                && key.phase() == LifecyclePhase::Validate
                && authority_is_exact
                && record.work_class() == Some(LifecycleWorkClass::Validate)
                && record.stage()
                    == Some(LifecycleStage::new(
                        LifecycleStageKind::ValidateBody,
                        PredecessorScope::Independent,
                    ))
                && record.reconstruction_source() == record.owner().causal_root().digest()
                && record
                    .durable_payload()
                    .is_some_and(|payload| durable_validate_payload_is_exact(key, payload))
        });
        let parent = parents.next()?;
        if parents.next().is_some() {
            return None;
        }
        let parent_key = parent.key()?;
        let inherited_prepare_authority = parent_key.execution_commitment().is_some();
        let edge = match vote.phase {
            wire::GlobalPhase::Prepare => DurableContinuationEdge::ValidateToSignPrepare,
            wire::GlobalPhase::Commit => DurableContinuationEdge::ValidateToSignCommit,
        };
        let child_phase = match vote.phase {
            wire::GlobalPhase::Prepare => LifecyclePhase::Prepare,
            wire::GlobalPhase::Commit => LifecyclePhase::Commit,
        };
        let child_stage = match vote.phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        };
        let child_key = LifecycleKey::new(
            self.context().id(),
            round,
            Some(proposal_round),
            Some(subject),
            child_phase,
            Some(commitment),
        );
        match (parent.terminal()?, parent.continuation()?) {
            (None, DurableContinuation::None) if !require_signed_broadcast => {
                if self
                    .records
                    .iter()
                    .any(|record| record.key() == Some(child_key))
                {
                    return None;
                }
            }
            (Some(TerminalOutcome::Advanced), continuation) => {
                let (observed_edge, child_ordinal) = continuation.successor_parts()?;
                let child = self
                    .records
                    .iter()
                    .find(|record| record.ordinal() == child_ordinal)?;
                if observed_edge != edge
                    || child.key() != Some(child_key)
                    || child.owner() != parent.owner()
                    || child.work_class() != Some(LifecycleWorkClass::SignVote)
                    || child.stage()
                        != Some(LifecycleStage::new(
                            child_stage,
                            PredecessorScope::Independent,
                        ))
                    || child.reconstruction_source() != parent.reconstruction_source()
                    || child.durable_payload() != Some(DurablePayloadReference::None)
                {
                    return None;
                }
                match (child.terminal()?, child.continuation()?) {
                    (None, DurableContinuation::None) if !require_signed_broadcast => {}
                    (Some(TerminalOutcome::Advanced), signed_continuation)
                        if require_signed_broadcast =>
                    {
                        let expected_edge = match vote.phase {
                            wire::GlobalPhase::Prepare => {
                                DurableContinuationEdge::SignPrepareToBroadcast
                            }
                            wire::GlobalPhase::Commit => {
                                DurableContinuationEdge::SignCommitToBroadcast
                            }
                        };
                        let (signed_edge, broadcast_ordinal) =
                            signed_continuation.successor_parts()?;
                        let broadcast = self
                            .records
                            .iter()
                            .find(|record| record.ordinal() == broadcast_ordinal)?;
                        if signed_edge != expected_edge
                            || broadcast.owner() != parent.owner()
                            || broadcast.work_class() != Some(LifecycleWorkClass::Broadcast)
                            || broadcast.terminal() != Some(None)
                            || broadcast.reconstruction_source() != parent.reconstruction_source()
                            || broadcast.durable_payload() != Some(DurablePayloadReference::None)
                            || broadcast.continuation() != Some(DurableContinuation::None)
                        {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
        Some(AuthenticatedRecoveredWalValidateLedgerParent {
            key: parent_key,
            owner: parent.owner(),
            ordinal: parent.ordinal(),
            payload: parent.durable_payload()?,
            replay_authority: parent.replay_authority.clone(),
            inherited_prepare_authority,
            wal_identity: recovered.wal_identity(),
            tag: recovered.tag(),
            vote: vote.clone(),
        })
    }

    /// Stage exactly one standalone Proposal/Timeout control Sign row.
    ///
    /// An exact existing row stutters without rewriting it. Absence appends
    /// the deterministic `high_water + 1` successor. A same-key row with any
    /// changed owner, metadata, payload, stage, replay authority, or terminal
    /// shape is a hard error and is never repaired in place.
    pub(super) fn stage_authenticated_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Sign belongs to another lifecycle context".to_owned(),
            ));
        }
        let mut matching = self
            .records
            .iter()
            .filter(|record| projection.names_record(record));
        if let Some(record) = matching.next() {
            if matching.next().is_some() || !projection.exactly_matches_record(record) {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "existing recovered control Sign row changed exact admission metadata"
                        .to_owned(),
                ));
            }
            return Ok((self.clone(), record.ordinal(), false));
        }

        let ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Sign ordinal exhausted".to_owned(),
            )
        })?;
        let mut staged = self.clone();
        staged.records.push(projection.fresh_record(ordinal)?);
        staged.high_water = ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.exactly_matches_ledger_at(&staged, ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged recovered control Sign successor is not exact".to_owned(),
            ));
        }
        Ok((staged, ordinal, true))
    }

    /// Authenticate the exact crash cut after a recovered control Sign fsynced
    /// its sole live Broadcast child.
    ///
    /// The parent continuation selects the child before any replay bytes are
    /// decoded. The recovered WAL carrier then reconstructs the complete
    /// pending/candidate binding, the verified height checks the signature,
    /// and this method compares that candidate back to the selected LedgerV1
    /// row. Proposal remains excluded by the later adapter cold-replay gate.
    pub(super) fn authenticate_recovered_control_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        projection: &AuthenticatedRecoveredWalControlProjection,
    ) -> Result<
        (
            super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
            u128,
            u128,
        ),
        LifecycleLedgerError,
    > {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.is_exact(verified) || !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Sign changed its verified context".to_owned(),
            ));
        }
        let mut parents = self
            .records
            .iter()
            .filter(|record| projection.names_record(record));
        let parent = parents.next().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast lost its WAL Sign parent".to_owned(),
            )
        })?;
        if parents.next().is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast has multiple WAL Sign parents".to_owned(),
            ));
        }
        let (_, child_ordinal) = parent
            .continuation()
            .and_then(DurableContinuation::successor_parts)
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered control Sign has no Broadcast continuation".to_owned(),
                )
            })?;
        if !projection.exactly_matches_advanced_record(parent, child_ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Sign changed its exact Advanced row".to_owned(),
            ));
        }
        let child = self
            .records
            .binary_search_by_key(&child_ordinal, |record| record.ordinal())
            .ok()
            .and_then(|index| self.records.get(index))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered control Sign lost its Broadcast child".to_owned(),
                )
            })?;
        let child_authority = child
            .project_recovered_signed_broadcast_child(self.context())
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered control Broadcast row is not a canonical live child".to_owned(),
                )
            })?;
        let broadcast = projection
            .recover_durable_signed_broadcast(verified, child_authority)
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered control Broadcast failed WAL and roster authentication".to_owned(),
                )
            })?;
        if child.owner() != parent.owner()
            || child.reconstruction_source() != parent.reconstruction_source()
            || !broadcast.exactly_matches_record(child, parent.owner())
            || self
                .records
                .iter()
                .filter(|record| record.owner() == parent.owner())
                .count()
                != 2
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast changed its exact owner or child row".to_owned(),
            ));
        }
        Ok((broadcast, parent.ordinal(), child_ordinal))
    }

    /// Authenticate one exact Proposal-Sign/Broadcast/next-Vote crash cut.
    ///
    /// The frame classifier first binds the transaction-local adjacent child
    /// rows to the complete ledger hash. The recovered control-WAL authority
    /// then names the exact Advanced Proposal Sign, while the combined
    /// WAL/body projection must reproduce both live child rows. Unrelated
    /// authenticated rows may coexist and are not claimed by this join.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn authenticate_recovered_control_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        control: &AuthenticatedRecoveredWalControlProjection,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    ) -> Result<RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1, LifecycleLedgerError>
    {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !control.is_exact(verified) || !control.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign changed its verified context".to_owned(),
            ));
        }

        let mut matching = self
            .recovered_lifecycle_signed_broadcast_and_sign_pairs()?
            .into_iter()
            .filter(|pair| {
                pair.parent() == RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
                    && self
                        .records
                        .binary_search_by_key(&pair.parent_ordinal(), |record| record.ordinal())
                        .ok()
                        .and_then(|index| self.records.get(index))
                        .is_some_and(|record| control.names_record(record))
            });
        let pair = matching.next().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign lost its frame-bound pair".to_owned(),
            )
        })?;
        if matching.next().is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign matched multiple durable pairs".to_owned(),
            ));
        }

        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let parent = record_at(pair.parent_ordinal()).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign lost its parent row".to_owned(),
            )
        })?;
        let broadcast = record_at(pair.broadcast_ordinal()).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign lost its Broadcast row".to_owned(),
            )
        })?;
        let next_sign = record_at(pair.next_sign_ordinal()).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign lost its next Sign row".to_owned(),
            )
        })?;
        if !control.exactly_matches_advanced_record(parent, pair.broadcast_ordinal())
            || !combined.exactly_matches_fresh_records(self.context(), broadcast, next_sign)
            || !pair.exactly_matches_ledger(self)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Broadcast-and-Sign changed its exact durable children"
                    .to_owned(),
            ));
        }
        Ok(pair)
    }

    pub(super) fn recovered_phase_signed_broadcast_ordinals(
        &self,
        repair: &AuthenticatedWalVoteLifecycleRepair,
    ) -> Option<(u128, u128, u128)> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).ok()?;
        if !repair.concrete_pair_is_exact() {
            return None;
        }
        let parent_candidate = repair.parent();
        let sign_candidate = repair.child();
        let mut parents = self
            .records
            .iter()
            .filter(|record| record.key() == Some(parent_candidate.key));
        let parent = parents.next()?;
        if parents.next().is_some() || !record_matches_recovery_candidate(parent, parent_candidate)
        {
            return None;
        }
        let (validate_edge, sign_ordinal) = parent.continuation()?.successor_parts()?;
        let mut signs = self
            .records
            .iter()
            .filter(|record| record.key() == Some(sign_candidate.key));
        let sign = signs.next()?;
        if signs.next().is_some()
            || validate_edge != repair.edge()
            || parent.terminal()? != Some(TerminalOutcome::Advanced)
            || sign.ordinal() != sign_ordinal
            || sign.owner() != parent.owner()
            || !record_matches_recovery_candidate(sign, sign_candidate)
            || sign.terminal()? != Some(TerminalOutcome::Advanced)
        {
            return None;
        }
        let expected_broadcast_edge =
            match (sign_candidate.key.phase(), sign_candidate.stage.kind()) {
                (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                    DurableContinuationEdge::SignPrepareToBroadcast
                }
                (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                    DurableContinuationEdge::SignCommitToBroadcast
                }
                _ => return None,
            };
        let (broadcast_edge, broadcast_ordinal) = sign.continuation()?.successor_parts()?;
        let broadcast = self
            .records
            .binary_search_by_key(&broadcast_ordinal, |record| record.ordinal())
            .ok()
            .and_then(|index| self.records.get(index))?;
        (broadcast_edge == expected_broadcast_edge
            && broadcast.owner() == parent.owner()
            && broadcast.work_class() == Some(LifecycleWorkClass::Broadcast)
            && broadcast.terminal() == Some(None)
            && broadcast.reconstruction_source() == parent.reconstruction_source()
            && broadcast.durable_payload() == Some(DurablePayloadReference::None)
            && broadcast.continuation() == Some(DurableContinuation::None)
            && self
                .records
                .iter()
                .filter(|record| record.owner() == parent.owner())
                .count()
                == 3)
            .then_some((parent.ordinal(), sign_ordinal, broadcast_ordinal))
    }

    /// Authenticate an already-fsynced recovered Validate→Sign→Broadcast chain
    /// before consuming the repair into its exact frame receipt.
    pub(super) fn authenticate_recovered_phase_signed_broadcast_repair(
        &self,
        verified: &VerifiedHeightContext,
        repair: &AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        (
            super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
            u128,
            u128,
            u128,
        ),
        LifecycleLedgerError,
    > {
        let (parent_ordinal, sign_ordinal, broadcast_ordinal) = self
            .recovered_phase_signed_broadcast_ordinals(repair)
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered phase Broadcast changed its Validate-to-Sign lineage".to_owned(),
                )
            })?;
        let sign = self
            .records
            .binary_search_by_key(&sign_ordinal, |record| record.ordinal())
            .ok()
            .and_then(|index| self.records.get(index))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered phase Broadcast lost its Sign row".to_owned(),
                )
            })?;
        let broadcast_record = self
            .records
            .binary_search_by_key(&broadcast_ordinal, |record| record.ordinal())
            .ok()
            .and_then(|index| self.records.get(index))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered phase Broadcast lost its child row".to_owned(),
                )
            })?;
        let child = broadcast_record
            .project_recovered_signed_broadcast_child(self.context())
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered phase Broadcast row is not a canonical live child".to_owned(),
                )
            })?;
        let broadcast = repair
            .recover_durable_signed_broadcast(verified, child)
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered phase Broadcast failed WAL and roster authentication".to_owned(),
                )
            })?;
        if !broadcast.exactly_matches_record(broadcast_record, sign.owner()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast changed its exact child row".to_owned(),
            ));
        }
        Ok((broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal))
    }

    /// Authenticate an already-fsynced recovered Validate→Sign→Broadcast chain.
    pub(super) fn authenticate_recovered_phase_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        repair: &DurableAuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        (
            super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
            u128,
            u128,
            u128,
        ),
        LifecycleLedgerError,
    > {
        let (broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal) =
            self.authenticate_recovered_phase_signed_broadcast_repair(verified, repair.repair())?;
        if repair.child_ordinal() != sign_ordinal {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast changed its durable Sign ordinal".to_owned(),
            ));
        }
        Ok((broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal))
    }

    /// Authenticate one exact Validate/Prepare-Sign/Broadcast plus Commit-Sign cut.
    ///
    /// The durable phase repair independently authenticates the historical
    /// Prepare Broadcast, while the combined WAL/body projection must retain
    /// that same child and the adjacent standalone Commit Sign. Unrelated
    /// authenticated records remain outside this frame-bound pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn authenticate_recovered_phase_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        repair: &DurableAuthenticatedWalVoteLifecycleRepair,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    ) -> Result<RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1, LifecycleLedgerError>
    {
        let (broadcast, validate_ordinal, sign_ordinal, broadcast_ordinal) =
            self.authenticate_recovered_phase_signed_broadcast(verified, repair)?;
        if !combined.broadcast_exactly_matches(&broadcast) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign changed its signed child".to_owned(),
            ));
        }
        let mut matching = self
            .recovered_lifecycle_signed_broadcast_and_sign_pairs()?
            .into_iter()
            .filter(|pair| {
                pair.parent()
                    == RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                        validate_ordinal,
                    }
                    && pair.parent_ordinal() == sign_ordinal
                    && pair.broadcast_ordinal() == broadcast_ordinal
            });
        let pair = matching.next().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign lost its frame-bound pair".to_owned(),
            )
        })?;
        if matching.next().is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign matched multiple durable pairs".to_owned(),
            ));
        }
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let broadcast_record = record_at(pair.broadcast_ordinal()).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign lost its Broadcast row".to_owned(),
            )
        })?;
        let next_sign_record = record_at(pair.next_sign_ordinal()).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign lost its Commit-Sign row".to_owned(),
            )
        })?;
        if !combined.exactly_matches_fresh_records(
            self.context(),
            broadcast_record,
            next_sign_record,
        ) || !pair.exactly_matches_ledger(self)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered phase Broadcast-and-Sign changed its exact durable children".to_owned(),
            ));
        }
        Ok(pair)
    }

    /// Stage exactly one standalone recovered Decision Fetch row.
    ///
    /// An exact existing row is a read-only stutter. Absence appends only the
    /// deterministic `high_water + 1` successor. Same-key drift in owner,
    /// replay source, payload, stage, or terminal state is never repaired.
    pub(super) fn stage_authenticated_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch belongs to another lifecycle context".to_owned(),
            ));
        }
        let mut matching = self
            .records
            .iter()
            .filter(|record| projection.names_record(record));
        if let Some(record) = matching.next() {
            if matching.next().is_some() || !projection.exactly_matches_record(record) {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "existing recovered Decision Fetch row changed exact admission metadata"
                        .to_owned(),
                ));
            }
            return Ok((self.clone(), record.ordinal(), false));
        }

        let ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch ordinal exhausted".to_owned(),
            )
        })?;
        let mut staged = self.clone();
        staged.records.push(projection.fresh_record(ordinal)?);
        staged.high_water = ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.exactly_matches_ledger_at(&staged, ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged recovered Decision Fetch successor is not exact".to_owned(),
            ));
        }
        Ok((staged, ordinal, true))
    }

    /// Authenticate the crash cut after a recovered Fetch advanced to one live Store.
    ///
    /// The WAL parent remains payload-free. The sole same-owner child must be
    /// the exact body-frame Store projected by the recovered reducer replay;
    /// partial or foreign owner history is rejected without changing storage.
    pub(super) fn authenticate_recovered_decision_fetch_store(
        &self,
        fetch_projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        store_projection: &RecoveredDecisionFetchStoreProjectionV1,
    ) -> Result<(u128, u128), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !fetch_projection.belongs_to_context(self.context())
            || store_projection.context() != self.context()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Store belongs to another lifecycle context".to_owned(),
            ));
        }
        let matching = self
            .records
            .iter()
            .filter(|record| fetch_projection.names_record(record))
            .collect::<Vec<_>>();
        let [fetch] = matching.as_slice() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Store requires one exact WAL Fetch parent".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::FetchToStore, store_ordinal)) = fetch
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch lost its Store continuation".to_owned(),
            ));
        };
        let store = self
            .records
            .binary_search_by_key(&store_ordinal, LifecycleLedgerRecordV1::ordinal)
            .ok()
            .and_then(|index| self.records.get(index))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Store continuation is a partial durable prefix".to_owned(),
                )
            })?;
        let owner = fetch.owner();
        let store_slot =
            PhysicalSlotId::for_capacity(LifecycleWorkClass::Store.capacity_class(), 0);
        let store_address =
            super::work_registry::ConcreteWorkAddress::new(owner, store_ordinal, store_slot)
                .ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "recovered Decision Store address is not representable".to_owned(),
                    )
                })?;
        if self
            .records
            .iter()
            .filter(|record| record.owner() == owner)
            .count()
            != 2
            || !fetch_projection.exactly_matches_advanced_apply_parent(fetch, store_ordinal)
            || !store_projection.exactly_matches_record(store, owner)
            || !store_projection.validates_at(
                self.context(),
                store_address,
                store_projection.digest(),
            )
            || store_ordinal > self.high_water
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch-to-Store crash cut changed exact durable semantics"
                    .to_owned(),
            ));
        }
        Ok((fetch.ordinal(), store_ordinal))
    }

    /// Stage or exactly coalesce the first-release recovered Decision body chain.
    ///
    /// The payload-free Decision Fetch must already be durable. A live exact
    /// Fetch advances directly to three adjacent BodyFrame successors in one
    /// prospective frame. A crash-cut live Store advances to an exact adjacent
    /// Validate/Apply tail, and an already complete four-row chain stutters
    /// without rewriting. Missing Fetch, other partial prefixes, foreign
    /// same-owner rows, or any semantic drift fail closed; history is never
    /// synthesized.
    pub(super) fn stage_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.reject_terminal_recovered_decision_apply(projection)?;
        self.stage_recovered_decision_apply_projection(projection)
    }

    fn stage_recovered_decision_apply_projection(
        &self,
        projection: &impl RecoveredDecisionApplyStageProjectionV1,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        let lineage = projection.lineage();
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Apply belongs to another lifecycle context".to_owned(),
            ));
        }
        let matching = self
            .records
            .iter()
            .filter(|record| projection.names_fetch_record(record))
            .collect::<Vec<_>>();
        let [fetch] = matching.as_slice() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "first-release recovered Decision Apply requires one exact durable Fetch parent"
                    .to_owned(),
            ));
        };
        let owner = fetch.owner();
        let owner_records = self
            .records
            .iter()
            .filter(|record| record.owner() == owner)
            .collect::<Vec<_>>();

        if projection.exactly_matches_live_fetch(fetch) {
            if owner_records.len() != 1 || fetch.ordinal() != owner.first_admission_ordinal() {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "live Decision Fetch owner already names foreign lifecycle history".to_owned(),
                ));
            }
            let store_ordinal = self.high_water.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Store ordinal exhausted".to_owned(),
                )
            })?;
            let validate_ordinal = store_ordinal.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Validate ordinal exhausted".to_owned(),
                )
            })?;
            let apply_ordinal = validate_ordinal.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Apply ordinal exhausted".to_owned(),
                )
            })?;
            let [store, validate, apply] = lineage
                .successor_records(owner, store_ordinal, validate_ordinal, apply_ordinal)
                .ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "recovered Decision successors lost exact owner or lineage".to_owned(),
                    )
                })?;
            let mut staged = self.clone();
            let fetch_index = staged
                .records
                .iter()
                .position(|record| record.ordinal() == fetch.ordinal())
                .expect("the exact Fetch parent belongs to the cloned ledger");
            staged.records[fetch_index].terminal =
                Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
            staged.records[fetch_index].continuation =
                PersistedDurableContinuationV1::from_schema(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ));
            staged.records.extend([store, validate, apply]);
            staged.records.sort_by_key(LifecycleLedgerRecordV1::ordinal);
            staged.high_water = apply_ordinal;
            staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
            return Ok((staged, apply_ordinal, true));
        }

        let Some((DurableContinuationEdge::FetchToStore, store_ordinal)) = fetch
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch is neither live nor an exact complete parent".to_owned(),
            ));
        };
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let Some(store) = record_at(store_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        if !projection.exactly_matches_advanced_fetch(fetch, store_ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coalesced recovered Decision body chain changed exact durable semantics"
                    .to_owned(),
            ));
        }

        if lineage.exactly_matches_live_store_record(owner, store) {
            if owner_records.len() != 2 {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "live recovered Decision Store owner names foreign lifecycle history"
                        .to_owned(),
                ));
            }
            let validate_ordinal = self.high_water.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Validate ordinal exhausted after Store restart".to_owned(),
                )
            })?;
            let apply_ordinal = validate_ordinal.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Apply ordinal exhausted after Store restart".to_owned(),
                )
            })?;
            let [advanced_store, validate, apply] = lineage
                .successor_records_after_live_store(owner, store, validate_ordinal, apply_ordinal)
                .ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "recovered Decision Store restart lost exact body lineage".to_owned(),
                    )
                })?;
            let mut staged = self.clone();
            let store_index = staged
                .records
                .iter()
                .position(|record| record.ordinal() == store_ordinal)
                .expect("the exact Store parent belongs to the cloned ledger");
            staged.records[store_index] = advanced_store;
            staged.records.extend([validate, apply]);
            staged.records.sort_by_key(LifecycleLedgerRecordV1::ordinal);
            staged.high_water = apply_ordinal;
            staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
            return Ok((staged, apply_ordinal, true));
        }

        let Some((DurableContinuationEdge::StoreToValidate, validate_ordinal)) = store
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        let Some(validate) = record_at(validate_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::ValidateToApply, apply_ordinal)) = validate
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        let Some(apply) = record_at(apply_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        if owner_records.len() != 4
            || !lineage.exactly_matches_successor_records(owner, store, validate, apply)
            || apply_ordinal > self.high_water
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coalesced recovered Decision body chain changed exact durable semantics"
                    .to_owned(),
            ));
        }
        Ok((self.clone(), apply_ordinal, false))
    }

    /// Authenticate an already terminal recovered Decision body chain.
    ///
    /// This oracle never feeds storage-only recovery: a terminal Apply must
    /// not be reconstructed as Ready. It only seals the exact four-row
    /// predecessor shape for the CompleteTip retirement transaction, whose
    /// caller must additionally join the full Kura artifact and receipt.
    pub(super) fn authenticate_terminal_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<u128, LifecycleLedgerError> {
        self.authenticate_terminal_recovered_decision_apply_projection(projection)
    }

    fn reject_terminal_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self
            .authenticate_terminal_recovered_decision_apply(projection)
            .is_ok()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn reject_terminal_recovered_decision_apply_projection(
        &self,
        projection: &impl TerminalRecoveredDecisionApplyProjectionV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self
            .authenticate_terminal_recovered_decision_apply_projection(projection)
            .is_ok()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn authenticate_terminal_recovered_decision_apply_projection(
        &self,
        projection: &impl TerminalRecoveredDecisionApplyProjectionV1,
    ) -> Result<u128, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply belongs to another lifecycle context".to_owned(),
            ));
        }
        let matching = self
            .records
            .iter()
            .filter(|record| projection.names_fetch_record(record))
            .collect::<Vec<_>>();
        let [fetch] = matching.as_slice() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires one exact Fetch parent".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::FetchToStore, store_ordinal)) = fetch
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Fetch lost its Store continuation".to_owned(),
            ));
        };
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let Some(store) = record_at(store_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain is incomplete".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::StoreToValidate, validate_ordinal)) = store
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain lost its Validate continuation".to_owned(),
            ));
        };
        let Some(validate) = record_at(validate_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain is incomplete".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::ValidateToApply, apply_ordinal)) = validate
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain lost its Apply continuation".to_owned(),
            ));
        };
        let Some(apply) = record_at(apply_ordinal) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain is incomplete".to_owned(),
            ));
        };
        let owner = fetch.owner();
        if self
            .records
            .iter()
            .filter(|record| record.owner() == owner)
            .count()
            != 4
            || !projection.exactly_matches_advanced_apply_parent(fetch, store_ordinal)
            || !projection.exactly_matches_terminal_successor_records(owner, store, validate, apply)
            || apply_ordinal > self.high_water
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain changed exact durable semantics".to_owned(),
            ));
        }
        Ok(apply_ordinal)
    }

    /// Join one terminal recovered-Decision Apply row to the complete
    /// Kura-authenticated CompleteTip evidence retained for successor startup.
    ///
    /// This remains a predecessor-chain oracle, not retirement authority: it
    /// neither censes nor retires unrelated live rows, leases, waits, debt,
    /// capacity, Serve payloads, or either durable publication target. The
    /// consuming retirement transaction proves those independently before it
    /// mints the activation token.
    pub(in crate::sumeragi) fn authenticate_complete_tip_terminal_apply(
        &self,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> Result<u128, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        let predecessor = complete_tip.predecessor();
        if self.context().height() != predecessor.height() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle ledger belongs to another height".to_owned(),
            ));
        }
        let mut terminal_applies = self.records.iter().filter(|record| {
            record.work_class() == Some(LifecycleWorkClass::Apply)
                && record.stage().is_some_and(|stage| {
                    stage.kind() == LifecycleStageKind::ApplyDecision
                        && stage.predecessor_scope() == PredecessorScope::Independent
                })
                && record.terminal() == Some(Some(TerminalOutcome::Advanced))
                && record.continuation() == Some(DurableContinuation::None)
                && complete_tip.authorizes_terminal_apply_replay(&record.replay_authority)
        });
        let apply = terminal_applies.next().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "CompleteTip finality has no exact terminal Decision Apply row".to_owned(),
            )
        })?;
        if terminal_applies.next().is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip finality names multiple terminal Decision Apply rows".to_owned(),
            ));
        }
        let apply_ordinal = apply.ordinal();
        let validate_ordinal = apply_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Validate predecessor".to_owned(),
            )
        })?;
        let store_ordinal = validate_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Store predecessor".to_owned(),
            )
        })?;
        let fetch_ordinal = store_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Fetch predecessor".to_owned(),
            )
        })?;
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let (Some(fetch), Some(store), Some(validate)) = (
            record_at(fetch_ordinal),
            record_at(store_ordinal),
            record_at(validate_ordinal),
        ) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle body chain is incomplete".to_owned(),
            ));
        };
        let owner = apply.owner();
        if fetch.owner() != owner
            || store.owner() != owner
            || validate.owner() != owner
            || owner.first_admission_ordinal() != fetch_ordinal
            || [fetch, store, validate, apply]
                .iter()
                .any(|record| record.reconstruction_source() != owner.causal_root().digest())
            || self
                .records
                .iter()
                .filter(|record| record.owner() == owner)
                .count()
                != 4
            || fetch.work_class() != Some(LifecycleWorkClass::Fetch)
            || store.work_class() != Some(LifecycleWorkClass::Store)
            || validate.work_class() != Some(LifecycleWorkClass::Validate)
            || fetch.terminal() != Some(Some(TerminalOutcome::Advanced))
            || store.terminal() != Some(Some(TerminalOutcome::Advanced))
            || validate.terminal() != Some(Some(TerminalOutcome::Advanced))
            || fetch.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ))
            || store.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::StoreToValidate,
                    validate_ordinal,
                ))
            || validate.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToApply,
                    apply_ordinal,
                ))
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::FetchToStore,
                &fetch.replay_authority,
                fetch
                    .durable_payload()
                    .expect("validated ledger Fetch payload"),
                &store.replay_authority,
                store
                    .durable_payload()
                    .expect("validated ledger Store payload"),
            ) != Some(true)
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::StoreToValidate,
                &store.replay_authority,
                store
                    .durable_payload()
                    .expect("validated ledger Store payload"),
                &validate.replay_authority,
                validate
                    .durable_payload()
                    .expect("validated ledger Validate payload"),
            ) != Some(true)
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::ValidateToApply,
                &validate.replay_authority,
                validate
                    .durable_payload()
                    .expect("validated ledger Validate payload"),
                &apply.replay_authority,
                apply
                    .durable_payload()
                    .expect("validated ledger Apply payload"),
            ) != Some(true)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle body chain changed exact durable semantics"
                    .to_owned(),
            ));
        }
        Ok(apply_ordinal)
    }

    /// Consume the exact opened predecessor store, frame, and CompleteTip proof
    /// into one non-decomposable authentication cut.
    ///
    /// The store is reloaded both before and after the terminal-chain join.
    /// Every failure consumes all three inputs and requires startup to restart;
    /// no caller can recover the CompleteTip activation or substitute a
    /// detached same-byte frame for the retained opened store handle.
    fn into_complete_tip_terminal_apply_store_join(
        self,
        ledger_store: LifecycleLedgerStoreV1,
        complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> Result<AuthenticatedCompleteTipTerminalApplyStoreJoinV1, LifecycleLedgerError> {
        if !ledger_store.is_authorized_complete_tip_predecessor_target(&complete_tip)
            || ledger_store.context != self.context()
            || ledger_store.load()? != self
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip predecessor store target or frame changed before authentication"
                    .to_owned(),
            ));
        }
        let apply_ordinal = self.authenticate_complete_tip_terminal_apply(&complete_tip)?;
        let cut = AuthenticatedCompleteTipTerminalApplyStoreJoinV1 {
            complete_tip,
            ledger_store,
            ledger: self,
            apply_ordinal,
        };
        if !cut.is_exact()? {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip predecessor cut changed during authentication".to_owned(),
            ));
        }
        Ok(cut)
    }

    /// Purely stage one adapter-authenticated WAL-ahead Validate-to-Sign repair.
    ///
    /// The only mutable shape is an exact live Validate parent with no child.
    /// It becomes `Advanced` and names a newly appended, same-owner Sign row at
    /// `high_water + 1`. An already repaired exact pair stutters. Every other
    /// parent/child arrangement fails before the unified startup transaction
    /// can persist the returned ledger.
    pub(super) fn stage_authenticated_wal_vote_repair(
        &self,
        repair: &AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !repair.concrete_pair_is_exact() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL repair lost its concrete effect binding".to_owned(),
            ));
        }
        let parent_candidate = repair.parent();
        let child_candidate = repair.child();
        let parent_index = self
            .records
            .iter()
            .position(|record| record.key() == Some(parent_candidate.key))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered WAL vote has no durable Validate parent".to_owned(),
                )
            })?;
        let parent = &self.records[parent_index];
        if !record_matches_recovery_candidate(parent, parent_candidate)
            || parent.work_class() != Some(LifecycleWorkClass::Validate)
            || parent.stage().is_none_or(|stage| {
                stage.kind() != LifecycleStageKind::ValidateBody
                    || stage.predecessor_scope() != PredecessorScope::Independent
            })
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL vote changed its durable Validate parent".to_owned(),
            ));
        }

        let existing_child = self
            .records
            .iter()
            .find(|record| record.key() == Some(child_candidate.key));
        let continuation = parent.continuation().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered WAL parent continuation cannot be decoded".to_owned(),
            )
        })?;
        let terminal = parent.terminal().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered WAL parent terminal cannot be decoded".to_owned(),
            )
        })?;

        if let Some((edge, child_ordinal)) = continuation.successor_parts() {
            let child = existing_child.ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered WAL continuation lost its Sign child".to_owned(),
                )
            })?;
            if terminal != Some(TerminalOutcome::Advanced)
                || edge != repair.edge()
                || child.ordinal() != child_ordinal
                || child.owner() != parent.owner()
                || !record_matches_recovery_candidate(child, child_candidate)
                || child.terminal() != Some(None)
                || child.continuation() != Some(DurableContinuation::None)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "recovered WAL continuation conflicts with the durable Sign pair".to_owned(),
                ));
            }
            return Ok((self.clone(), child_ordinal, false));
        }

        if terminal.is_some()
            || continuation != DurableContinuation::None
            || existing_child.is_some()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL vote does not match a live uncontinued Validate".to_owned(),
            ));
        }
        let child_ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger("recovered WAL Sign ordinal exhausted".to_owned())
        })?;
        let mut staged = self.clone();
        staged.records[parent_index].terminal =
            Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
        staged.records[parent_index].continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(repair.edge(), child_ordinal),
        );
        staged.records.push(LifecycleLedgerRecordV1::new(
            child_candidate.key,
            parent.owner(),
            child_ordinal,
            child_candidate.work_class,
            child_candidate.stage,
            None,
            child_candidate.reconstruction_source,
            child_candidate.payload,
            child_candidate.replay_authority.clone(),
            DurableContinuation::None,
        )?);
        staged.high_water = child_ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        Ok((staged, child_ordinal, true))
    }

    fn validate(&self, max_records: usize) -> Result<(), LifecycleLedgerError> {
        if self.format_version != LEDGER_VERSION || self.records.len() > max_records {
            return Err(LifecycleLedgerError::InvalidLedger(
                "format version or record bound is invalid".to_owned(),
            ));
        }
        let context = self.context();
        let mut ordinals = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut serve_requests = BTreeSet::new();
        let mut continuation_successors = BTreeSet::new();
        if self
            .records
            .windows(2)
            .any(|window| window[0].ordinal >= window[1].ordinal)
            || self
                .producer_debts
                .windows(2)
                .any(|window| window[0].serve_ordinal >= window[1].serve_ordinal)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "records or producer debts are not canonically ordered".to_owned(),
            ));
        }
        for record in &self.records {
            if !record.validate(context, self.high_water)
                || !ordinals.insert(record.ordinal)
                || !keys.insert(record.key)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "record identity, context, or schema is invalid".to_owned(),
                ));
            }
            let owner = record.owner();
            if owners
                .insert(owner.causal_root(), owner)
                .is_some_and(|known| known != owner)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "one causal root names multiple lifecycle owners".to_owned(),
                ));
            }
            if record.work_class() == Some(LifecycleWorkClass::CertifiedServe)
                && record
                    .durable_payload()
                    .and_then(DurablePayloadReference::request)
                    .is_none_or(|request| !serve_requests.insert(request))
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "one exact signed Serve request names multiple lifecycle records".to_owned(),
                ));
            }
        }
        for record in &self.records {
            let continuation = record
                .continuation()
                .expect("records validated before successor edges");
            if continuation != DurableContinuation::None
                && record.reconstruction_source() != record.owner().causal_root().digest()
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "durable continuation is not bound to its causal owner".to_owned(),
                ));
            }
            if continuation == DurableContinuation::AdvancedNoSuccessor
                && !durable_validate_payload_is_exact(
                    record
                        .key()
                        .expect("records validated before continuation payloads"),
                    record
                        .durable_payload()
                        .expect("records validated before continuation payloads"),
                )
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "advanced Validate without a successor lost its exact body frame".to_owned(),
                ));
            }
            let Some((edge, successor_ordinal)) = continuation.successor_parts() else {
                continue;
            };
            let successor = self
                .records
                .binary_search_by_key(&successor_ordinal, |candidate| candidate.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            if !continuation_successors.insert(successor_ordinal)
                || successor.is_none_or(|successor| {
                    let parent_payload = record
                        .durable_payload()
                        .expect("records validated before successor edges");
                    let successor_payload = successor
                        .durable_payload()
                        .expect("records validated before successor edges");
                    let payload_and_replay_are_exact =
                        recovered_decision_body_continuation_is_exact(
                            edge,
                            &record.replay_authority,
                            parent_payload,
                            &successor.replay_authority,
                            successor_payload,
                        )
                        .or_else(|| {
                            signed_broadcast_continuation_is_exact(
                                edge,
                                &record.replay_authority,
                                parent_payload,
                                &successor.replay_authority,
                                successor_payload,
                            )
                        })
                        .unwrap_or_else(|| {
                            durable_continuation_payload_is_exact(
                                edge,
                                parent_payload,
                                successor_payload,
                            )
                        });
                    successor.owner() != record.owner()
                        || successor.reconstruction_source() != record.reconstruction_source()
                        || !payload_and_replay_are_exact
                        || !durable_continuation_successor_is_exact(
                            edge,
                            record
                                .work_class()
                                .expect("records validated before successor edges"),
                            record
                                .key()
                                .expect("records validated before successor edges"),
                            record
                                .stage()
                                .expect("records validated before successor edges"),
                            successor
                                .work_class()
                                .expect("records validated before successor edges"),
                            successor
                                .key()
                                .expect("records validated before successor edges"),
                            successor
                                .stage()
                                .expect("records validated before successor edges"),
                        )
                })
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "advanced body-stage successor is missing, aliased, or semantically foreign"
                        .to_owned(),
                ));
            }
        }
        for record in &self.records {
            let owner = record.owner();
            if self
                .records
                .binary_search_by_key(&owner.first_admission_ordinal(), |candidate| {
                    candidate.ordinal
                })
                .ok()
                .and_then(|index| self.records.get(index))
                .is_none_or(|first| first.owner() != owner)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "owner first ordinal has no matching tombstone or record".to_owned(),
                ));
            }
        }
        self.validate_debts()
    }

    fn validate_debts(&self) -> Result<(), LifecycleLedgerError> {
        let mut serves = BTreeSet::new();
        let mut producers = BTreeSet::new();
        for debt in &self.producer_debts {
            if !serves.insert(debt.serve_ordinal)
                || !producers.insert(debt.producer_ordinal)
                || debt.serve_ordinal.checked_add(1) != Some(debt.producer_ordinal)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "producer debt is non-adjacent or non-bijective".to_owned(),
                ));
            }
            let serve = self
                .records
                .binary_search_by_key(&debt.serve_ordinal, |record| record.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            let producer = self
                .records
                .binary_search_by_key(&debt.producer_ordinal, |record| record.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            if serve.and_then(LifecycleLedgerRecordV1::work_class)
                != Some(LifecycleWorkClass::CertifiedServe)
                || producer.and_then(LifecycleLedgerRecordV1::work_class)
                    != Some(LifecycleWorkClass::ProducerTurn)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "producer debt does not name a Serve/producer pair".to_owned(),
                ));
            }
        }
        for record in &self.records {
            let work_class = record.work_class().expect("records validated before debts");
            let terminal = record.terminal().expect("records validated before debts");
            match work_class {
                LifecycleWorkClass::CertifiedServe => {
                    let Some(producer_ordinal) = record.ordinal.checked_add(1) else {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve ordinal cannot address its producer".to_owned(),
                        ));
                    };
                    let Some(producer) = self
                        .records
                        .binary_search_by_key(&producer_ordinal, |candidate| candidate.ordinal)
                        .ok()
                        .and_then(|index| self.records.get(index))
                    else {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve record has no adjacent producer record".to_owned(),
                        ));
                    };
                    if producer.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                        || producer.owner() != record.owner()
                        || producer.reconstruction_source() != record.reconstruction_source()
                        || !producer
                            .replay_authority
                            .same_persisted_family(&record.replay_authority)
                        || !serve_and_producer_keys_match(
                            record.key().expect("records validated before debts"),
                            producer.key().expect("records validated before debts"),
                        )
                        || (terminal == Some(TerminalOutcome::Cancelled)
                            && producer.terminal() != Some(Some(TerminalOutcome::Cancelled)))
                        || (terminal.is_none() && !serves.contains(&record.ordinal))
                        || serves.contains(&record.ordinal)
                            != producer
                                .terminal()
                                .expect("records validated before debts")
                                .is_none()
                    {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve/producer atomic pair is inconsistent".to_owned(),
                        ));
                    }
                }
                LifecycleWorkClass::ProducerTurn => {
                    let live = terminal.is_none();
                    let serve = record.ordinal.checked_sub(1).and_then(|ordinal| {
                        self.records
                            .binary_search_by_key(&ordinal, |candidate| candidate.ordinal)
                            .ok()
                            .and_then(|index| self.records.get(index))
                    });
                    if serve.and_then(LifecycleLedgerRecordV1::work_class)
                        != Some(LifecycleWorkClass::CertifiedServe)
                        || serve.is_none_or(|serve| serve.owner() != record.owner())
                        || serve.is_none_or(|serve| {
                            !serve_and_producer_keys_match(
                                serve.key().expect("records validated before debts"),
                                record.key().expect("records validated before debts"),
                            )
                        })
                        || producers.contains(&record.ordinal) != live
                    {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "producer debt does not match producer terminality".to_owned(),
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
