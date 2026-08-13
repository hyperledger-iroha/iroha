/// One inert replay family shared by a Certified-Serve request and its
/// atomically reserved ProducerTurn.
///
/// The family is runtime-only authority. Its canonical storage source remains
/// private and cannot be decoded or reconstructed from source parts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedServeStorageReplayFamilyV1 {
    source: CertifiedServeStorageSourceV1,
}

/// Opaque replay evidence for one exact post-fsync Certified-Serve record.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Certified-Serve replay evidence must remain with its reserved producer turn"]
struct CertifiedServeReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
    payload: ReplayPayloadBindingV1,
}

/// Opaque replay evidence for the dormant ProducerTurn reserved beside one
/// exact Certified-Serve request.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "ProducerTurn replay evidence must remain with its Certified-Serve origin"]
struct CertifiedServeProducerTurnReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
}

/// Closed pair preserving one common post-fsync storage origin across the
/// Certified-Serve record and its reserved ProducerTurn.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the Certified-Serve replay pair has not entered durable admission"]
pub(super) struct CertifiedServeReplayEvidencePairV1 {
    serve: CertifiedServeReplayEvidenceV1,
    producer: CertifiedServeProducerTurnReplayEvidenceV1,
}

/// One move-only terminal replay-family replacement for an adjacent
/// Certified-Serve/ProducerTurn pair.
///
/// Production construction is closed over either a post-fsync terminal
/// receipt or an authenticated payload-store recovery record. In particular,
/// no caller can inject the terminal payload-frame hash as raw bytes.
#[derive(Debug)]
#[must_use = "the terminal Certified-Serve replay pair must be installed atomically"]
pub(super) struct CertifiedServeTerminalReplayAuthorityPairV1 {
    terminal_payload: DurablePayloadReference,
    terminal_outcome: TerminalOutcome,
    serve: LifecycleReplayAuthorityV1,
    producer: LifecycleReplayAuthorityV1,
}

impl CertifiedServeTerminalReplayAuthorityPairV1 {
    /// Return the terminal tombstone bound by this sealed pair.
    pub(super) const fn terminal_outcome(&self) -> TerminalOutcome {
        self.terminal_outcome
    }

    /// Clone this still-sealed terminal family into one whole concrete-carrier
    /// proof. No serve/producer authority or raw frame hash leaves the replay
    /// module; the registry may install the returned pair only through its
    /// typed terminal transition.
    pub(super) fn terminal_carrier_replay_evidence(
        &self,
    ) -> Option<CertifiedServeReplayEvidencePairV1> {
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &self.serve.source else {
            return None;
        };
        let family = Arc::new(CertifiedServeStorageReplayFamilyV1 {
            source: source.clone(),
        });
        let evidence = CertifiedServeReplayEvidencePairV1 {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload: ReplayPayloadBindingV1::from_payload(self.terminal_payload),
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        (evidence.shares_exact_storage_origin()
            && evidence.serve.exactly_matches_authority(&self.serve)
            && evidence.producer.exactly_matches_authority(&self.producer))
        .then_some(evidence)
    }

    /// Rebind one live Pending pair from an exact post-fsync completion
    /// receipt.
    pub(super) fn from_completed_receipt(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        receipt: DurableCertifiedServeCompletedReceipt,
    ) -> Option<Self> {
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = serve_metadata.payload
        else {
            return None;
        };
        if request != digest_from_bytes(receipt.id().request_hash().as_ref())
            || certificate != digest_from_bytes(receipt.certificate_hash().as_ref())
        {
            return None;
        }
        let response = digest_from_bytes(receipt.response_hash().as_ref());
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            },
            TerminalOutcome::Completed(Some(response)),
            receipt.payload_hash(),
        )
    }

    /// Rebind one live Pending pair from an exact post-fsync negative receipt.
    pub(super) fn from_negative_receipt(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        receipt: DurableCertifiedServeNegativeReceipt,
    ) -> Option<Self> {
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = serve_metadata.payload
        else {
            return None;
        };
        if request != digest_from_bytes(receipt.id().request_hash().as_ref())
            || certificate != digest_from_bytes(receipt.certificate_hash().as_ref())
        {
            return None;
        }
        let outcome = match receipt.outcome() {
            CertifiedServePayloadNegativeOutcome::Cancelled => {
                DurableServeNegativeOutcome::Cancelled
            }
            CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                DurableServeNegativeOutcome::Rejected(code)
            }
            CertifiedServePayloadNegativeOutcome::Failed(code) => {
                DurableServeNegativeOutcome::Failed(code)
            }
        };
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome,
            },
            outcome.terminal(),
            receipt.payload_hash(),
        )
    }

    /// Seal the terminal replay pair recovered from one authenticated payload
    /// frame and its independently reconstructed admission candidate.
    pub(super) fn from_authenticated_recovery(
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
        candidate: &CandidateAdmission,
        terminal_payload: DurablePayloadReference,
        terminal_outcome: TerminalOutcome,
    ) -> Option<Self> {
        if !recovered.exactly_matches_persisted_payload()
            || !terminal_payload
                .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(terminal_outcome))
            || candidate.work_class != LifecycleWorkClass::CertifiedServe
            || !candidate.payload.same_admission_material(terminal_payload)
        {
            return None;
        }
        let recovered_payload = recovered_certified_serve_payload(recovered)?;
        if recovered_payload != ReplayPayloadBindingV1::from_payload(terminal_payload) {
            return None;
        }
        let recovered_family = exact_certified_serve_storage_replay_family(
            active_context,
            recovered.request(),
            recovered.payload_hash(),
            recovered.local_retainer(),
        )?;
        let recovered_source =
            LifecycleReplaySourceV1::CertifiedServeStorage(recovered_family.source.clone());
        if candidate.replay_authority.source != recovered_source {
            return None;
        }
        let producer = candidate.producer_turn.as_ref()?;
        let serve = LifecycleReplayAuthorityV1 {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: recovered_payload,
            source: recovered_source,
        };
        let sealed = Self {
            terminal_payload,
            terminal_outcome,
            serve,
            producer: producer.replay_authority.clone(),
        };
        sealed
            .pending_candidate_matches_terminal_family(active_context, candidate)
            .then_some(sealed)
    }

    #[allow(clippy::too_many_arguments)]
    fn from_terminal_frame(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        terminal_payload: DurablePayloadReference,
        terminal_outcome: TerminalOutcome,
        terminal_frame_hash: Hash,
    ) -> Option<Self> {
        if !terminal_payload
            .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(terminal_outcome))
            || !serve_metadata
                .payload
                .same_admission_material(terminal_payload)
        {
            return None;
        }
        let LifecycleReplaySourceV1::CertifiedServeStorage(mut source) =
            serve_metadata.replay_authority.source.clone()
        else {
            return None;
        };
        source.payload_hash = *terminal_frame_hash.as_ref();
        let terminal_source = LifecycleReplaySourceV1::CertifiedServeStorage(source);
        let sealed = Self {
            terminal_payload,
            terminal_outcome,
            serve: LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(terminal_payload),
                source: terminal_source.clone(),
            },
            producer: LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: terminal_source,
            },
        };
        sealed
            .exactly_advances_pending_records(
                active_context,
                serve_record,
                serve_metadata,
                producer_record,
                producer_metadata,
            )
            .then_some(sealed)
    }

    /// Construct a synthetic terminal-frame transition for pure reducer tests.
    /// Production paths have no raw-hash constructor and must use a durable
    /// receipt or authenticated recovery record.
    #[cfg(test)]
    pub(super) fn from_test_terminal_outcome(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        terminal_outcome: TerminalOutcome,
    ) -> Option<Self> {
        let terminal_payload = serve_metadata.payload.terminalized(terminal_outcome)?;
        let mut preimage = Vec::with_capacity(Hash::LENGTH + 2);
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) =
            &serve_metadata.replay_authority.source
        else {
            return None;
        };
        preimage.extend_from_slice(&source.payload_hash);
        preimage.push(0xFF);
        preimage.push(match terminal_outcome {
            TerminalOutcome::Advanced => 0,
            TerminalOutcome::Completed(_) => 1,
            TerminalOutcome::Cancelled => 2,
            TerminalOutcome::Rejected(_) => 3,
            TerminalOutcome::Failed(_) => 4,
        });
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            terminal_payload,
            terminal_outcome,
            Hash::new(preimage),
        )
    }

    /// Prove the sole permitted payload-store-ahead transition: one exact
    /// Pending ledger pair advances to this authenticated terminal frame while
    /// request, certificate, local retainer, keys, and stages remain fixed.
    pub(super) fn exactly_advances_pending_records(
        &self,
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
    ) -> bool {
        self.exactly_advances_pending_coordinates(
            active_context,
            serve_record.key,
            serve_record.owner,
            serve_record.ordinal,
            serve_record.stage,
            serve_metadata.reconstruction_source,
            serve_metadata.payload,
            &serve_metadata.replay_authority,
            producer_record.key,
            producer_record.owner,
            producer_record.ordinal,
            producer_record.stage,
            producer_metadata.reconstruction_source,
            producer_metadata.payload,
            &producer_metadata.replay_authority,
        )
    }

    /// Match a Pending logical pair after both authorities have already been
    /// rebound to this exact terminal frame family but before the terminal
    /// payload/tombstone is installed.
    pub(super) fn exactly_matches_rebound_records(
        &self,
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
    ) -> bool {
        let DurablePayloadReference::CertifiedServePending { .. } = serve_metadata.payload else {
            return false;
        };
        serve_record.ordinal.checked_add(1) == Some(producer_record.ordinal)
            && serve_record.work_class == LifecycleWorkClass::CertifiedServe
            && serve_record.stage.kind() == LifecycleStageKind::CertifiedServe
            && producer_record.work_class == LifecycleWorkClass::ProducerTurn
            && producer_record.stage.kind() == LifecycleStageKind::ProducerTurn
            && serve_record.owner == producer_record.owner
            && serve_and_producer_keys_match(serve_record.key, producer_record.key)
            && serve_metadata.reconstruction_source == producer_metadata.reconstruction_source
            && serve_metadata.reconstruction_source == serve_record.owner.causal_root().digest()
            && producer_metadata.payload == DurablePayloadReference::None
            && self
                .terminal_payload
                .same_admission_material(serve_metadata.payload)
            && serve_metadata.replay_authority == self.serve
            && producer_metadata.replay_authority == self.producer
            && self.serve.structurally_matches_record(
                active_context,
                serve_record.key,
                LifecycleWorkClass::CertifiedServe,
                serve_record.stage,
                self.terminal_payload,
            )
            && self.producer.structurally_matches_record(
                active_context,
                producer_record.key,
                LifecycleWorkClass::ProducerTurn,
                producer_record.stage,
                DurablePayloadReference::None,
            )
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Apply the transition-only oracle to decoded ledger coordinates without
    /// exposing either encoded authority outside the lifecycle subsystem.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn exactly_advances_pending_coordinates(
        &self,
        active_context: LifecycleContext,
        serve_key: LifecycleKey,
        serve_owner: OwnerId,
        serve_ordinal: u128,
        serve_stage: LifecycleStage,
        serve_reconstruction_source: LifecycleDigest,
        serve_payload: DurablePayloadReference,
        serve_authority: &LifecycleReplayAuthorityV1,
        producer_key: LifecycleKey,
        producer_owner: OwnerId,
        producer_ordinal: u128,
        producer_stage: LifecycleStage,
        producer_reconstruction_source: LifecycleDigest,
        producer_payload: DurablePayloadReference,
        producer_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let DurablePayloadReference::CertifiedServePending { .. } = serve_payload else {
            return false;
        };
        if serve_ordinal.checked_add(1) != Some(producer_ordinal)
            || serve_stage.kind() != LifecycleStageKind::CertifiedServe
            || producer_stage.kind() != LifecycleStageKind::ProducerTurn
            || serve_owner != producer_owner
            || !serve_and_producer_keys_match(serve_key, producer_key)
            || producer_payload != DurablePayloadReference::None
            || serve_reconstruction_source != producer_reconstruction_source
            || serve_reconstruction_source != serve_owner.causal_root().digest()
            || !serve_authority.same_persisted_family(producer_authority)
            || !serve_authority.structurally_matches_record(
                active_context,
                serve_key,
                LifecycleWorkClass::CertifiedServe,
                serve_stage,
                serve_payload,
            )
            || !producer_authority.structurally_matches_record(
                active_context,
                producer_key,
                LifecycleWorkClass::ProducerTurn,
                producer_stage,
                DurablePayloadReference::None,
            )
            || !self.terminal_payload.same_admission_material(serve_payload)
            || !self.serve.structurally_matches_record(
                active_context,
                serve_key,
                LifecycleWorkClass::CertifiedServe,
                serve_stage,
                self.terminal_payload,
            )
            || !self.producer.structurally_matches_record(
                active_context,
                producer_key,
                LifecycleWorkClass::ProducerTurn,
                producer_stage,
                DurablePayloadReference::None,
            )
            || !self.serve.same_persisted_family(&self.producer)
        {
            return false;
        }
        certified_serve_sources_share_origin_except_frame(
            &serve_authority.source,
            &self.serve.source,
        )
    }

    /// Match the exact terminal candidate reconstructed from the same
    /// authenticated recovery frame.
    pub(super) fn exactly_matches_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        let Some(producer) = candidate.producer_turn.as_ref() else {
            return false;
        };
        candidate.work_class == LifecycleWorkClass::CertifiedServe
            && candidate.payload == self.terminal_payload
            && candidate.replay_authority_is_exact(active_context)
            && candidate.replay_authority == self.serve
            && producer.replay_authority == self.producer
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Replace the Pending projection fields with the exact terminal payload
    /// and authority derived from the same authenticated recovery frame.
    pub(super) fn bind_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &mut CandidateAdmission,
    ) -> bool {
        if !self.pending_candidate_matches_terminal_family(active_context, candidate) {
            return false;
        }
        candidate.payload = self.terminal_payload;
        candidate.replay_authority = self.serve.clone();
        self.exactly_matches_recovered_candidate(active_context, candidate)
    }

    fn pending_candidate_matches_terminal_family(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        let Some(producer) = candidate.producer_turn.as_ref() else {
            return false;
        };
        matches!(
            candidate.payload,
            DurablePayloadReference::CertifiedServePending { .. }
        ) && candidate.work_class == LifecycleWorkClass::CertifiedServe
            && candidate
                .payload
                .same_admission_material(self.terminal_payload)
            && candidate.replay_authority_is_exact(active_context)
            && candidate
                .replay_authority
                .same_persisted_family(&self.serve)
            && producer.replay_authority == self.producer
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Consume the authenticated frame transition into the exact terminal
    /// payload, outcome, and separately encoded adjacent authorities.
    pub(super) fn consume_terminal_rebind(
        self,
    ) -> (
        DurablePayloadReference,
        TerminalOutcome,
        LifecycleReplayAuthorityV1,
        LifecycleReplayAuthorityV1,
    ) {
        (
            self.terminal_payload,
            self.terminal_outcome,
            self.serve,
            self.producer,
        )
    }
}

fn certified_serve_sources_share_origin_except_frame(
    pending: &LifecycleReplaySourceV1,
    terminal: &LifecycleReplaySourceV1,
) -> bool {
    let (
        LifecycleReplaySourceV1::CertifiedServeStorage(pending),
        LifecycleReplaySourceV1::CertifiedServeStorage(terminal),
    ) = (pending, terminal)
    else {
        return false;
    };
    pending.request == terminal.request && pending.local_retainer == terminal.local_retainer
}

// The pair is consumed only by the fixed adjacent CandidateAdmission factory;
// its decoded ledger descendants remain inert and cannot reconstruct this pair.

impl CertifiedServeReplayEvidencePairV1 {
    /// Seal one exact freshly persisted Pending request and its ProducerTurn.
    pub(super) fn from_post_fsync_pending(
        active_context: LifecycleContext,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> Option<Self> {
        if !receipt.exactly_matches_pending(authenticated)
            || receipt.id().request_hash() != authenticated.request_hash()
            || receipt.certificate_hash() != HashOf::new(&authenticated.request().certificate)
        {
            return None;
        }
        let payload = certified_serve_pending_payload(authenticated);
        let family = exact_certified_serve_storage_replay_family(
            active_context,
            authenticated,
            receipt.payload_hash(),
            receipt.local_retainer(),
        )?;
        let evidence = Self {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload,
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        evidence
            .exactly_matches_post_fsync_pending(active_context, authenticated, receipt)
            .then_some(evidence)
    }

    /// Reconstruct the same closed pair only from a fully authenticated
    /// payload-store recovery record.
    pub(super) fn from_authenticated_recovery(
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
    ) -> Option<Self> {
        if !recovered.exactly_matches_persisted_payload() {
            return None;
        }
        let payload = recovered_certified_serve_payload(recovered)?;
        let family = exact_certified_serve_storage_replay_family(
            active_context,
            recovered.request(),
            recovered.payload_hash(),
            recovered.local_retainer(),
        )?;
        let evidence = Self {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload,
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        evidence
            .exactly_matches_recovered(active_context, recovered)
            .then_some(evidence)
    }

    /// Project one exact shared storage family into the adjacent durable
    /// admission pair without consuming the runtime-only family. Semantic keys,
    /// stages, payloads, authorities, slots, and physical digests are all
    /// derived here. The same pair can then move, whole, into the two concrete
    /// registry carriers.
    pub(super) fn admission_candidate(
        &self,
        active_context: LifecycleContext,
    ) -> Option<CandidateAdmission> {
        let storage_payload = self.serve.payload.durable_payload()?;
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        let serve_stage = LifecycleStage::new(
            LifecycleStageKind::CertifiedServe,
            PredecessorScope::ReadyOrdinalPrefix,
        );
        let producer_stage = LifecycleStage::new(
            LifecycleStageKind::ProducerTurn,
            PredecessorScope::ProducerHandoffBarrier,
        );
        let serve_shape = self
            .serve
            .family
            .source
            .project(active_context, serve_stage.kind(), &self.serve.payload)
            .ok()?;
        let producer_shape = self
            .producer
            .family
            .source
            .project(
                active_context,
                producer_stage.kind(),
                &ReplayPayloadBindingV1::None,
            )
            .ok()?;
        if !self.exactly_matches_serve_record(
            active_context,
            serve_shape.key,
            serve_stage,
            storage_payload,
            storage_payload_hash,
        ) || !self.exactly_matches_producer_record(
            active_context,
            producer_shape.key,
            producer_stage,
            DurablePayloadReference::None,
            storage_payload_hash,
        ) {
            return None;
        }
        let source =
            LifecycleReplaySourceV1::CertifiedServeStorage(self.serve.family.source.clone());
        let request = digest_from_bytes(HashOf::new(&self.serve.family.source.request).as_ref());
        let certificate =
            digest_from_bytes(HashOf::new(&self.serve.family.source.request.certificate).as_ref());
        let serve_payload = DurablePayloadReference::certified_serve_pending(request, certificate);
        let serve_authority = LifecycleReplayAuthorityV1::decode_canonical(
            &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(serve_payload),
                source: source.clone(),
            }
            .encode(),
        )
        .ok()?;
        let producer_authority = LifecycleReplayAuthorityV1::decode_canonical(
            &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source,
            }
            .encode(),
        )
        .ok()?;
        let reconstruction_source = request;
        let serve_slot =
            PhysicalSlotId::for_capacity(LifecycleWorkClass::CertifiedServe.capacity_class(), 0);
        let producer_slot =
            PhysicalSlotId::for_capacity(LifecycleWorkClass::ProducerTurn.capacity_class(), 0);
        Some(CandidateAdmission::new(
            serve_shape.key,
            CausalRoot::new(reconstruction_source),
            LifecycleWorkClass::CertifiedServe,
            serve_stage,
            InitialLifecycleState::Ready,
            reconstruction_source,
            serve_payload,
            serve_authority,
            PhysicalGeometry::new(
                [PhysicalSlot::new(
                    serve_slot,
                    digest_from_hash(&storage_payload_hash),
                )],
                [serve_slot],
            ),
            Some(ProducerTurnAdmission::new(
                producer_shape.key,
                producer_stage,
                reconstruction_source,
                producer_authority,
                PhysicalGeometry::new(
                    [PhysicalSlot::new(
                        producer_slot,
                        self.producer_physical_digest(),
                    )],
                    [producer_slot],
                ),
            )),
        ))
    }

    /// Compare the retained Serve evidence with one exact logical record and
    /// its independently retained payload-store frame hash.
    pub(super) fn exactly_matches_serve_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.shares_exact_storage_origin()
            && self.serve.exactly_matches_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
    }

    /// Match one exact terminal Serve row without inventing an executable
    /// physical carrier. Steady terminal Ledger rows reopen with empty geometry,
    /// while payload-store-ahead reconciliation may retain the former Pending
    /// geometry in its reconciled tombstone. Neither shape is executable: the
    /// retained storage family derives its own frame hash and must still match
    /// the complete logical/durable authority.
    pub(super) fn exactly_matches_terminal_serve_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
    ) -> bool {
        let super::LifecycleState::Terminal(outcome) = record.state else {
            return false;
        };
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        record.work_class == LifecycleWorkClass::CertifiedServe
            && record.owner.causal_root().digest() == metadata.reconstruction_source
            && metadata
                .payload
                .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(outcome))
            && self.exactly_matches_serve_record(
                active_context,
                record.key,
                record.stage,
                metadata.payload,
                storage_payload_hash,
            )
            && self
                .serve
                .exactly_matches_authority(&metadata.replay_authority)
    }

    /// Compare the retained ProducerTurn evidence with one exact dormant
    /// logical record while retaining the same payload-store origin.
    pub(super) fn exactly_matches_producer_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.shares_exact_storage_origin()
            && self.producer.exactly_matches_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
    }

    fn shares_exact_storage_origin(&self) -> bool {
        Arc::ptr_eq(&self.serve.family, &self.producer.family)
    }

    /// Match the exact one-slot Serve carrier without exposing the payload-store
    /// frame hash retained by this family.
    pub(super) fn exactly_matches_serve_carrier(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        physical_digest: LifecycleDigest,
        replay_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        physical_digest == digest_from_hash(&storage_payload_hash)
            && self.exactly_matches_serve_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
            && self.serve.exactly_matches_authority(replay_authority)
    }

    /// Match the exact one-slot ProducerTurn carrier while retaining the same
    /// opaque payload-store family as its Serve origin.
    pub(super) fn exactly_matches_producer_carrier(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        physical_digest: LifecycleDigest,
        replay_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        physical_digest == self.producer_physical_digest()
            && self.exactly_matches_producer_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
            && self.producer.exactly_matches_authority(replay_authority)
    }

    fn producer_physical_digest(&self) -> LifecycleDigest {
        certified_serve_producer_physical_digest(&self.serve.family.source)
    }

    fn exactly_matches_post_fsync_pending(
        &self,
        active_context: LifecycleContext,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> bool {
        self.shares_exact_storage_origin()
            && receipt.exactly_matches_pending(authenticated)
            && self.serve.payload == certified_serve_pending_payload(authenticated)
            && self
                .serve
                .family
                .source
                .project(
                    active_context,
                    LifecycleStageKind::CertifiedServe,
                    &self.serve.payload,
                )
                .is_ok()
            && exact_certified_serve_storage_replay_family(
                active_context,
                authenticated,
                receipt.payload_hash(),
                receipt.local_retainer(),
            )
            .is_some_and(|expected| {
                expected.as_ref() == self.serve.family.as_ref()
                    && receipt.id().request_hash() == authenticated.request_hash()
                    && receipt.certificate_hash()
                        == HashOf::new(&authenticated.request().certificate)
            })
    }

    fn exactly_matches_recovered(
        &self,
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
    ) -> bool {
        self.shares_exact_storage_origin()
            && recovered.exactly_matches_persisted_payload()
            && recovered_certified_serve_payload(recovered).as_ref() == Some(&self.serve.payload)
            && self
                .serve
                .family
                .source
                .project(
                    active_context,
                    LifecycleStageKind::CertifiedServe,
                    &self.serve.payload,
                )
                .is_ok()
            && exact_certified_serve_storage_replay_family(
                active_context,
                recovered.request(),
                recovered.payload_hash(),
                recovered.local_retainer(),
            )
            .is_some_and(|expected| {
                expected.as_ref() == self.serve.family.as_ref()
                    && recovered.id().request_hash() == recovered.request().request_hash()
                    && recovered.certificate_hash()
                        == HashOf::new(&recovered.request().request().certificate)
            })
    }
}

impl CertifiedServeReplayEvidenceV1 {
    fn exactly_matches_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.family.source.payload_hash == *storage_payload_hash.as_ref()
            && self.exactly_matches_authority(&LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(payload),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            })
            && LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: self.payload.clone(),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
            .validate_record(
                active_context,
                key,
                LifecycleWorkClass::CertifiedServe,
                stage,
                payload,
            )
            .is_ok()
    }

    fn exactly_matches_authority(&self, authority: &LifecycleReplayAuthorityV1) -> bool {
        authority
            == &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: self.payload.clone(),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
    }
}

impl CertifiedServeProducerTurnReplayEvidenceV1 {
    fn exactly_matches_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.family.source.payload_hash == *storage_payload_hash.as_ref()
            && self.exactly_matches_authority(&LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(payload),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            })
            && LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
            .validate_record(
                active_context,
                key,
                LifecycleWorkClass::ProducerTurn,
                stage,
                payload,
            )
            .is_ok()
    }

    fn exactly_matches_authority(&self, authority: &LifecycleReplayAuthorityV1) -> bool {
        authority
            == &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
    }
}

fn certified_serve_producer_physical_digest(
    source: &CertifiedServeStorageSourceV1,
) -> LifecycleDigest {
    let request_hash = HashOf::new(&source.request);
    let mut projection =
        Vec::with_capacity(PRODUCER_TURN_PHYSICAL_DOMAIN.len() + size_of::<u64>() + Hash::LENGTH);
    projection.extend_from_slice(PRODUCER_TURN_PHYSICAL_DOMAIN);
    append_field(&mut projection, request_hash.as_ref());
    digest_from_hash(&Hash::new(projection))
}

fn exact_certified_serve_storage_replay_family(
    active_context: LifecycleContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    storage_payload_hash: Hash,
    local_retainer: wire::ValidatorIndex,
) -> Option<Arc<CertifiedServeStorageReplayFamilyV1>> {
    let request = authenticated.request();
    let local_retainer_index = usize::try_from(local_retainer).ok()?;
    if authenticated.request_hash() != HashOf::new(request)
        || local_retainer_index >= wire::MAX_VALIDATORS_PER_HEIGHT
        || request
            .certificate
            .signers
            .binary_search(&local_retainer)
            .is_err()
    {
        return None;
    }
    let source = CertifiedServeStorageSourceV1 {
        request: request.clone(),
        payload_hash: *storage_payload_hash.as_ref(),
        local_retainer,
    };
    let serve = source
        .project(
            active_context,
            LifecycleStageKind::CertifiedServe,
            &certified_serve_pending_payload(authenticated),
        )
        .ok()?;
    let producer = source
        .project(
            active_context,
            LifecycleStageKind::ProducerTurn,
            &ReplayPayloadBindingV1::None,
        )
        .ok()?;
    if super::schema::producer_turn_key_for_serve(serve.key) != Some(producer.key)
        || serve.work_class != LifecycleWorkClass::CertifiedServe
        || producer.work_class != LifecycleWorkClass::ProducerTurn
    {
        return None;
    }
    Some(Arc::new(CertifiedServeStorageReplayFamilyV1 { source }))
}

fn certified_serve_pending_payload(
    authenticated: &AuthenticatedCertifiedBodyRequest,
) -> ReplayPayloadBindingV1 {
    ReplayPayloadBindingV1::CertifiedServePending {
        request: *authenticated.request_hash().as_ref(),
        certificate: *HashOf::new(&authenticated.request().certificate).as_ref(),
    }
}

fn recovered_certified_serve_payload(
    recovered: &AuthenticatedRecoveredCertifiedServePayload,
) -> Option<ReplayPayloadBindingV1> {
    let request = recovered.request();
    let request_hash = request.request_hash();
    let certificate_hash = recovered.certificate_hash();
    if recovered.id().request_hash() != request_hash
        || request_hash != HashOf::new(request.request())
        || certificate_hash != HashOf::new(&request.request().certificate)
    {
        return None;
    }
    Some(match recovered.state() {
        AuthenticatedRecoveredCertifiedServePayloadState::Pending => {
            ReplayPayloadBindingV1::CertifiedServePending {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
            }
        }
        AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) => {
            ReplayPayloadBindingV1::CertifiedServeCompleted {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
                response: *completed.response_hash().as_ref(),
            }
        }
        AuthenticatedRecoveredCertifiedServePayloadState::Negative(outcome) => {
            let outcome = match outcome {
                CertifiedServePayloadNegativeOutcome::Cancelled => {
                    DurableServeNegativeOutcome::Cancelled
                }
                CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                    DurableServeNegativeOutcome::Rejected(*code)
                }
                CertifiedServePayloadNegativeOutcome::Failed(code) => {
                    DurableServeNegativeOutcome::Failed(*code)
                }
            };
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::CertifiedServeNegative {
                request: LifecycleDigest::new(*request_hash.as_ref()),
                certificate: LifecycleDigest::new(*certificate_hash.as_ref()),
                outcome,
            })
        }
    })
}
