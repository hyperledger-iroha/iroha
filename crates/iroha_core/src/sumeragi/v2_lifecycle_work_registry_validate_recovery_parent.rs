/// Reconstruct one exact recovered Validate parent directly from durable storage.
///
/// This is the production restart-only replacement for the scheduler/lease
/// preparation path used by live work. LedgerV1 supplies the immutable owner
/// and ordinal, the body store transfers one exact revalidated marker, and the
/// runtime consumes the authenticated WAL vote into its successor. The holder
/// remains the only concrete-registry owner and returns only the existing
/// opaque authenticated repair plus its exact opened ledger wrapper.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::result_large_err, clippy::too_many_lines)]
pub(super) fn reconstruct_recovered_wal_validate_parent<'registry, 'body>(
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    verified: &VerifiedHeightContext,
    body_store: &'body mut V2BodyStore,
    ledger_root: &Path,
    recovered: RecoveredWalVoteSign,
) -> Result<
    (
        OpenedRecoveredWalValidateLedger,
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ),
    RecoveredWalParentFactoryError<'body>,
> {
    let context = projection::lifecycle_context(verified.context());
    let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(ledger_root, context) {
        Ok(opened) => opened,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::LedgerOpen {
                    _error: error,
                    _recovered: recovered,
                },
            });
        }
    };
    let ledger = OpenedRecoveredWalValidateLedger { store, opened };
    let body = match body_store.detach_recovered_validated_parent(&recovered) {
        Ok(body) => body,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::BodyMarker {
                    _error: error,
                    _ledger: ledger,
                    _recovered: recovered,
                },
            });
        }
    };
    if !body.exactly_matches_vote(&recovered) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let Some(parent) = ledger
        .opened
        .authenticate_recovered_wal_validate_parent(&recovered)
        .or_else(|| {
            ledger
                .opened
                .authenticate_recovered_wal_validate_parent_for_signed_broadcast(&recovered)
        })
    else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    };
    if !body.exactly_matches_ledger_parent(context, &parent) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let successor = match reconstruct_recovered_wal_vote_successor(&parent, recovered) {
        Ok(successor) => successor,
        Err(recovered) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::RuntimeParent {
                    _ledger: ledger,
                    _body: body,
                    _recovered: recovered,
                },
            });
        }
    };
    let repair = match authenticate_recovered_wal_vote_lifecycle_from_ledger_parent(
        verified, &parent, successor,
    ) {
        Ok(repair) => repair,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::Lifecycle {
                    _ledger: ledger,
                    _body: body,
                    _error: error,
                },
            });
        }
    };
    let registry_preflight = (|| {
        if !parent.matches_candidate(repair.parent())
            || (ledger
                .opened
                .stage_authenticated_wal_vote_repair(&repair)
                .is_err()
                && ledger
                    .opened
                    .recovered_phase_signed_broadcast_ordinals(&repair)
                    .is_none())
        {
            return None;
        }
        let (physical, universe, consumed) = repair.parent().physical_geometry.normalized().ok()?;
        if physical.len() != 1 || universe.len() != 1 || consumed != universe {
            return None;
        }
        let (&slot, &incumbent_digest) = physical.first_key_value()?;
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return None;
        }
        let address = ConcreteWorkAddress::new(parent.owner(), parent.ordinal(), slot)?;
        registry
            .entries
            .keys()
            .all(|installed| installed.owner != parent.owner())
            .then_some((address, incumbent_digest))
    })();
    let Some((address, incumbent_digest)) = registry_preflight else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::RegistryParent {
                _ledger: ledger,
                _repair: repair,
                _body: body,
            },
        });
    };

    // All fallible parent, ledger, body, and registry checks precede this
    // transfer. From here the detached marker moves directly into the sealed
    // completion and no pre-join error can discard it.
    let outcome = body.into_validation_outcome();
    let validated = outcome
        .validated_receipt()
        .expect("a recovered validated-body cut transfers one success outcome");
    let durable_receipt = validated.durable().clone();
    // Restart recovery obtains this hash from the semantically revalidated
    // marker reopened by this exact body-store instance. Unlike the live
    // transport path, there is no independently in-flight manifest carrier;
    // the checksummed receipt and store manifest were already compared before
    // the marker entered the validated recovery catalog.
    let expected_manifest_hash = durable_receipt.manifest_hash();
    let recovered_body_marker = durable_receipt.clone();
    let installed_digest =
        durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
            .expect("a validated recovered parent has one completion digest");
    let validation = DetachedRecoveredValidateCompletion {
        address,
        installed_digest,
        incumbent_address: address,
        incumbent_digest,
        durable_receipt,
        expected_manifest_hash,
        replay_evidence: DetachedValidateReplayEvidenceV1::RecoveredBodyMarker(
            recovered_body_marker,
        ),
        outcome,
    };
    let authority = AuthenticatedRecoveredWalValidateLifecycleRepair {
        repair,
        validation,
        reservation: RecoveredWalValidateRegistryReservation {
            registry,
            parent_address: address,
            child: None,
        },
    };
    debug_assert!(authority.concrete_pair_and_validation_are_exact());
    Ok((ledger, authority))
}

#[cfg(test)]
fn recovered_next_vote_projection_for_scheduler_fixture(
    verified: &VerifiedHeightContext,
    marker: u8,
) -> (
    RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    wire::Vote,
) {
    let context = verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: u64::from(marker),
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
        payload_hash: Hash::new([marker, 2]),
    };
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        round,
        subject,
        HashOf::from_untyped_unchecked(Hash::new([marker, 3])),
    );
    let validated = ValidatedBodyReceipt::for_test(durable);
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: validated.execution_commitment(),
        signer: 0,
        signature: Vec::new(),
    };
    let tag = EventTag::new(
        context.height,
        round.view,
        crate::sumeragi::v2_core::Generation::new(u64::from(marker).saturating_add(1)),
    );
    let seal = super::replay_authority::RecoveredLifecycleNextWalVoteSealV1::for_test(
        crate::sumeragi::v2::RecoveredWalFrameIdentity::for_test(
            u64::from(marker).saturating_add(8),
            u64::from(marker).saturating_add(9),
            [marker; 32],
        ),
        tag,
        vote.clone(),
        validated,
    )
    .expect("exact scheduler fixture WAL Vote seal");
    let projection = match
        crate::sumeragi::v2_runtime::project_recovered_lifecycle_next_wal_vote_candidate(
            verified, seal,
        ) {
        Ok(projection) => projection,
        Err(_) => panic!("exact scheduler fixture WAL Vote projection"),
    };
    (projection, vote)
}

#[cfg(test)]
impl super::concrete_admission::LifecycleWorkRegistryHolder {
    /// Count only closed recovered-WAL Sign rows after an installed cut drops.
    /// This test oracle exposes no address, effect, pending binding, or receipt.
    pub(crate) fn recovered_wal_sign_entry_count_for_test(&self) -> usize {
        self.registry_for_test()
            .entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
                )
            })
            .count()
    }

    /// Install one exact recovered Broadcast pair plus an unrelated Ready Sign.
    ///
    /// Only logical ordinals leave this helper. Every effect, WAL identity,
    /// pending binding, signature, and body receipt remains inside its closed
    /// concrete carrier.
    #[allow(clippy::too_many_lines)]
    pub(super) fn recovered_broadcast_pair_scheduler_fixture_for_test(
        coordinator: &mut LifecycleCoordinator,
        verified: &VerifiedHeightContext,
        local_signer: &iroha_crypto::KeyPair,
    ) -> (Self, u128, u128, u128) {
        assert!(coordinator.records.is_empty());
        assert_eq!(
            coordinator.active_context,
            super::projection::lifecycle_context(verified.context())
        );
        assert_eq!(
            verified.context().roster[0].validator,
            PeerId::new(local_signer.public_key().clone())
        );

        let (parent, parent_vote) =
            recovered_next_vote_projection_for_scheduler_fixture(verified, 0x41);
        let mut signed_vote = parent_vote.clone();
        signed_vote.signature = iroha_crypto::Signature::new(
            local_signer.private_key(),
            &SignRequest::Vote(parent_vote).signature_preimage(),
        )
        .payload()
        .to_vec();
        let broadcast =
            RecoveredLifecycleSignedBroadcastProjectionV1::from_next_wal_vote_for_scheduler_fixture(
                &parent,
                verified,
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(signed_vote),
                )),
            )
            .expect("signed Vote rejoins its exact scheduler fixture WAL parent");
        let (paired_next_sign, _) =
            recovered_next_vote_projection_for_scheduler_fixture(verified, 0x51);
        let (unrelated_sign, _) =
            recovered_next_vote_projection_for_scheduler_fixture(verified, 0x61);

        let (parent_owner, parent_ordinal) = parent
            .admit_into_scheduler_fixture(verified, coordinator)
            .expect("admit exact scheduler fixture parent Sign");
        let (broadcast_owner, broadcast_ordinal) =
            match coordinator.admit(AdmissionRequest::Candidate(broadcast.candidate().clone())) {
                super::AdmissionDecision::Admitted {
                    owner,
                    ordinal,
                    producer_turn_ordinal: None,
                } => (owner, ordinal),
                decision => panic!("admit exact scheduler fixture Broadcast: {decision:?}"),
            };
        assert_eq!(broadcast_owner, parent_owner);
        assert_eq!(parent_ordinal.checked_add(1), Some(broadcast_ordinal));
        let (paired_owner, paired_ordinal) = paired_next_sign
            .admit_into_scheduler_fixture(verified, coordinator)
            .expect("admit exact scheduler fixture paired Sign");
        assert_ne!(paired_owner, parent_owner);
        assert_eq!(broadcast_ordinal.checked_add(1), Some(paired_ordinal));
        let (unrelated_owner, unrelated_ordinal) = unrelated_sign
            .admit_into_scheduler_fixture(verified, coordinator)
            .expect("admit exact scheduler fixture unrelated Sign");
        assert_ne!(unrelated_owner, parent_owner);
        assert_ne!(unrelated_owner, paired_owner);

        coordinator
            .finish_terminal(parent_ordinal, super::TerminalOutcome::Advanced)
            .expect("terminalize the scheduler fixture parent Sign");
        coordinator
            .durable_records
            .get_mut(&parent_ordinal)
            .expect("terminal scheduler fixture parent retains metadata")
            .continuation = super::schema::DurableContinuation::successor(
            super::schema::DurableContinuationEdge::SignPrepareToBroadcast,
            broadcast_ordinal,
        );

        let address = |ordinal, class| {
            let record = coordinator
                .records
                .get(&ordinal)
                .expect("scheduler fixture ordinal retains one record");
            let (slot, digest) = exact_single_record_slot(record, class)
                .expect("scheduler fixture record retains one exact slot");
            (
                ConcreteWorkAddress::new(record.owner, ordinal, slot)
                    .expect("scheduler fixture record retains one exact address"),
                digest,
            )
        };
        let (parent_address, parent_digest) = address(
            parent_ordinal,
            LifecycleWorkClass::SignVote.capacity_class(),
        );
        let (broadcast_address, broadcast_digest) = address(
            broadcast_ordinal,
            LifecycleWorkClass::Broadcast.capacity_class(),
        );
        let (paired_address, paired_digest) = address(
            paired_ordinal,
            LifecycleWorkClass::SignVote.capacity_class(),
        );
        let (unrelated_address, unrelated_digest) = address(
            unrelated_ordinal,
            LifecycleWorkClass::SignVote.capacity_class(),
        );
        assert_eq!(parent.digest(), parent_digest);
        assert_eq!(broadcast.digest(), broadcast_digest);
        assert_eq!(paired_next_sign.digest(), paired_digest);
        assert_eq!(unrelated_sign.digest(), unrelated_digest);

        let parent = DurableRecoveredLifecycleNextWalVoteSignWork {
            projection: parent,
            verified: verified.clone(),
            address: parent_address,
            dispatch_key: None,
        };
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::NextWalVote(parent),
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((paired_address, paired_digest)),
                },
            ),
        };
        let paired_work = ConcreteLifecycleWork {
            digest: paired_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: paired_next_sign,
                    verified: verified.clone(),
                    address: paired_address,
                    dispatch_key: None,
                },
            ),
        };
        let unrelated_work = ConcreteLifecycleWork {
            digest: unrelated_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: unrelated_sign,
                    verified: verified.clone(),
                    address: unrelated_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(paired_work.validates_at(paired_address));
        assert!(unrelated_work.validates_at(unrelated_address));

        let mut registry = Self::empty();
        let entries = &mut registry.registry_for_test_mut().entries;
        assert!(entries.insert(broadcast_address, broadcast_work).is_none());
        assert!(entries.insert(paired_address, paired_work).is_none());
        assert!(entries.insert(unrelated_address, unrelated_work).is_none());
        registry
            .registry_for_test()
            .attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(
                coordinator,
                broadcast_ordinal,
                paired_ordinal,
            )
            .expect("exact scheduler fixture pair attests");
        registry
            .registry_for_test()
            .attest_ready_recovered_lifecycle_sign(coordinator, unrelated_ordinal)
            .expect("unrelated scheduler fixture Sign attests independently");
        (
            registry,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        )
    }

    /// Clear only one exact retained pair link for the independent-Sign case.
    pub(super) fn clear_recovered_broadcast_pair_link_for_test(
        &mut self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> bool {
        let Some(record) = coordinator.records.get(&broadcast_ordinal) else {
            return false;
        };
        let Some((slot, _)) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
        else {
            return false;
        };
        let Some(address) = ConcreteWorkAddress::new(record.owner, broadcast_ordinal, slot) else {
            return false;
        };
        let Some(work) = self.registry_for_test_mut().entries.get_mut(&address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &mut work.kind
        else {
            return false;
        };
        broadcast.paired_next_sign.take().is_some()
    }

    /// Corrupt only the retained pair digest while preserving both carriers.
    pub(super) fn corrupt_recovered_broadcast_pair_link_for_test(
        &mut self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> bool {
        let Some(record) = coordinator.records.get(&broadcast_ordinal) else {
            return false;
        };
        let Some((slot, _)) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
        else {
            return false;
        };
        let Some(address) = ConcreteWorkAddress::new(record.owner, broadcast_ordinal, slot) else {
            return false;
        };
        let Some(work) = self.registry_for_test_mut().entries.get_mut(&address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &mut work.kind
        else {
            return false;
        };
        let Some((next_address, next_digest)) = broadcast.paired_next_sign else {
            return false;
        };
        let mut corrupted = LifecycleDigest::new([0xD7; 32]);
        if corrupted == next_digest {
            corrupted = LifecycleDigest::new([0xD8; 32]);
        }
        broadcast.paired_next_sign = Some((next_address, corrupted));
        true
    }


    /// Assemble and install a genuine ordinary-Proposal validated completion.
    ///
    /// The retained signed Proposal enters the same authenticated fair-ingress
    /// replay mint as production dispatch. This helper then projects its exact
    /// Fetch-to-Store-to-Validate lineage, installs the closed completion, and
    /// returns only its exact scheduler coordinates. Callers must still enter
    /// the production Ready preparation path to borrow or detach the carrier.
    #[allow(clippy::too_many_lines)]
    fn install_remote_proposal_validate_completion_for_test(
        &mut self,
        verified: &VerifiedHeightContext,
        tag: EventTag,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> (TurnLease, PhysicalSlotId, CandidateAdmission) {
        assert_eq!(proposal.manifest, manifest);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round: proposal.round,
            subject: proposal.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let mut fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 1)],
        )
        .expect("bind genuine remote-Proposal Fetch fixture")
        .pop()
        .expect("one remote-Proposal Fetch fixture owner");
        assert!(
            fetch_ownership
                .bind_authenticated_remote_proposal_replay_for_test(proposal, &fetch_effect,)
        );
        let fetch_pending = fetch_ownership
            .pending_adapter_effect_binding(&fetch_effect)
            .expect("remote-Proposal Fetch retains one pending binding");
        let fetch_replay = fetch_ownership
            .exact_remote_proposal_fetch_replay(&fetch_effect)
            .expect("authenticated Proposal retains exact Fetch replay evidence");
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store_pending = fetch_pending
            .project_proposal_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("remote-Proposal Fetch projects exact Store binding");
        let store_replay = fetch_replay
            .project_exact_store(&store_effect, &store_pending)
            .expect("remote-Proposal Fetch projects exact Store replay evidence");
        let durable_receipt = validated_receipt.durable().clone();
        let stored_replay = store_replay
            .bind_durable_body(&store_effect, &durable_receipt)
            .expect("remote-Proposal Store binds its exact durable frame");
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let pending = store_pending
            .project_store_validate_successor(&store_effect, &effect)
            .expect("remote-Proposal Store projects exact Validate binding");
        let replay_evidence = stored_replay
            .project_exact_validate(&store_effect, &durable_receipt, &effect, &pending)
            .expect("remote-Proposal Store projects exact Validate replay evidence");
        let replay_evidence = DurableValidateReplayEvidenceV1::remote_proposal(replay_evidence);
        let projected = replay_evidence
            .project_installed_validate_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project genuine remote-Proposal recovered-WAL Validate fixture");
        let coordinator_candidate = projected.clone();
        assert_eq!(projected.work_class, LifecycleWorkClass::Validate);
        assert_eq!(projected.key.phase(), LifecyclePhase::Validate);
        assert_eq!(projected.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(
            projected.stage.predecessor_scope(),
            PredecessorScope::Independent
        );
        assert_eq!(projected.initial_state, InitialLifecycleState::Ready);
        let (physical_slots, universe, consumed) = projected
            .physical_geometry
            .normalized()
            .expect("normalize recovered-WAL Validate fixture geometry");
        assert_eq!(physical_slots.len(), 1);
        assert_eq!(universe.len(), 1);
        assert_eq!(consumed, universe);
        let (&slot, &incumbent_digest) = physical_slots
            .first_key_value()
            .expect("one recovered-WAL Validate fixture slot");
        let ordinal = 1;
        let owner = OwnerId::new(projected.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact recovered-WAL Validate fixture address");
        let expected_manifest_hash = durable_receipt.manifest_hash();
        assert_eq!(HashOf::new(&manifest), expected_manifest_hash);
        let incumbent = DurableValidateBody {
            address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(validate_validated_receipt_authority(&incumbent, &validated_receipt).is_ok());
        let outcome = DurableBodyValidationOutcome::validated_for_test(validated_receipt);
        let replacement_digest =
            durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
                .expect("validated recovered-WAL completion has one digest");
        assert_ne!(replacement_digest, incumbent_digest);
        let work = ConcreteLifecycleWork {
            digest: replacement_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(DurableValidateCompletion {
                address,
                incumbent,
                incumbent_digest,
                outcome,
            }),
        };
        self.registry_for_test_mut()
            .install(address, replacement_digest, work)
            .unwrap_or_else(|(error, _work)| {
                panic!("install recovered-WAL Validate fixture: {error:?}")
            });
        let mut ready_slots = physical_slots;
        assert_eq!(
            ready_slots.insert(slot, replacement_digest),
            Some(incumbent_digest)
        );
        let lease = TurnLease {
            id: LeaseId(1),
            ordinal,
            owner,
            key: projected.key,
            work_class: projected.work_class,
            stage: projected.stage,
            rank: super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: ready_slots,
            output_reservation: None,
        };
        (lease, slot, coordinator_candidate)
    }

    /// Assemble and install a genuine validated completion fixture, then reach
    /// the recovered-WAL cut through the production Ready preparation and
    /// detachment path.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn recovered_wal_validate_registry_cut_for_test<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        recovered: &RecoveredWalVoteSign,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> RecoveredWalValidateRegistryCut<'registry> {
        let tag = recovered.tag();
        let vote = recovered.vote();
        assert_eq!(proposal.round, vote.proposal_round);
        assert_eq!(proposal.subject, vote.subject);
        let (lease, slot, _candidate) = self.install_remote_proposal_validate_completion_for_test(
            verified,
            tag,
            proposal,
            manifest,
            validated_receipt,
        );
        let prepared = self
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, verified)
            .expect("prepare installed recovered-WAL Validate completion");
        prepared
            .into_recovered_wal_validate_registry_cut()
            .unwrap_or_else(|_prepared| panic!("validated recovered-WAL completion must detach"))
    }
}
