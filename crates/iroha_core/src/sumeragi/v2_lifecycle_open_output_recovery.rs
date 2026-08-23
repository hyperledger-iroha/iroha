/// Complete move-only census of non-WAL lifecycle output rows recovered at cold open.
///
/// Entries remain keyed by their immutable LedgerV1 ordinal.  The candidate
/// clone admitted into the logical coordinator is comparison-only; executable
/// effect/pending ownership remains solely in each retained cold carrier.
#[derive(Debug)]
#[must_use = "cold lifecycle output recovery must be installed or retained intact"]
pub(in crate::sumeragi) struct PreparedLifecycleOutputRecoveryV1 {
    entries: BTreeMap<u128, super::replay_authority::AuthenticatedRecoveredLifecycleOutputV1>,
}

impl PreparedLifecycleOutputRecoveryV1 {
    fn assemble(
        ledger: &LifecycleLedgerV1,
        verified: &VerifiedHeightContext,
    ) -> Result<Self, LifecycleRecoveryAssemblyErrorKind> {
        let mut entries = BTreeMap::new();
        let mut keys = BTreeSet::new();
        for record in ledger.records().iter().filter(|record| {
            record.terminal() == Some(None)
                && matches!(
                    record.work_class(),
                    Some(
                        LifecycleWorkClass::Broadcast
                            | LifecycleWorkClass::EquivocationReport
                            | LifecycleWorkClass::InvalidBodyReport
                    )
                )
        }) {
            let work_class = record
                .work_class()
                .expect("filter decoded the output class");
            if work_class == LifecycleWorkClass::Broadcast
                && has_durable_sign_predecessor(ledger, record)
            {
                // The recovered-WAL projection must authenticate this child;
                // never downgrade it to the standalone signed-output path.
                continue;
            }
            let invalid_parent = if work_class == LifecycleWorkClass::InvalidBodyReport {
                Some(unique_invalid_body_validate_parent(ledger, record).ok_or(
                    LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                        ordinal: record.ordinal(),
                        work_class,
                        stage: record.stage().ok_or(
                            LifecycleRecoveryAssemblyErrorKind::InvalidDurableRecord {
                                ordinal: record.ordinal(),
                                field: "stage",
                            },
                        )?,
                    },
                )?)
            } else {
                None
            };
            let output = record
                .authenticate_recovered_lifecycle_output(ledger.context(), verified, invalid_parent)
                .ok_or_else(|| {
                    LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                        ordinal: record.ordinal(),
                        work_class,
                        stage: record.stage().expect("opened output row decoded its stage"),
                    }
                })?;
            if output.owner() != record.owner()
                || output.ordinal() != record.ordinal()
                || output.candidate().key != record.key().expect("opened output row has a key")
                || !keys.insert(output.candidate().key)
                || entries.insert(record.ordinal(), output).is_some()
            {
                return Err(
                    LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                        ordinal: record.ordinal(),
                        work_class,
                        stage: record.stage().expect("opened output row decoded its stage"),
                    },
                );
            }
        }
        Ok(Self { entries })
    }

    fn owns_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        self.entries.get(&record.ordinal()).is_some_and(|output| {
            output.owner() == record.owner()
                && record.key() == Some(output.candidate().key)
                && record.work_class() == Some(output.candidate().work_class)
                && record.stage() == Some(output.candidate().stage)
                && record.terminal() == Some(None)
                && record.reconstruction_source() == output.candidate().reconstruction_source
                && record.durable_payload() == Some(output.candidate().payload)
                && record.continuation() == Some(DurableContinuation::None)
                && record.replay_matches_candidate(output.candidate())
        })
    }

    fn splice_candidates(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if self.entries.values().any(|output| {
            candidates.contains_key(&output.candidate().key)
                || output.candidate().initial_state != super::InitialLifecycleState::Ready
        }) {
            return false;
        }
        for output in self.entries.values() {
            let candidate = output.candidate().clone();
            assert!(candidates.insert(candidate.key, candidate).is_none());
        }
        true
    }

    fn invalid_body_reports(
        &self,
    ) -> impl Iterator<Item = &super::replay_authority::AuthenticatedRecoveredLifecycleOutputV1>
    {
        self.entries
            .values()
            .filter(|output| output.requires_rejected_body_marker())
    }

    /// Return whether the cold census contains no executable output owner.
    pub(in crate::sumeragi) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Project the complete authenticated cold-output ordinal census.
    pub(super) fn exact_ready_ordinals_for_registry_census(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> Option<BTreeSet<u128>> {
        self.entries
            .iter()
            .all(|(ordinal, output)| {
                *ordinal == output.ordinal()
                    && coordinator.ready_index.contains(ordinal)
                    && recovered_output_matches_ready_coordinator_row(coordinator, output)
            })
            .then(|| self.entries.keys().copied().collect())
    }
}

impl AuthenticatedLifecycleRecoveryCut {
    /// Rejoin the still-owned cold outputs to the opened coordinator census.
    pub(super) fn exact_lifecycle_output_ordinals_for_registry_census(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> Option<BTreeSet<u128>> {
        self.lifecycle_outputs.as_ref().map_or_else(
            || Some(BTreeSet::new()),
            |outputs| outputs.exact_ready_ordinals_for_registry_census(coordinator),
        )
    }
}

/// Result of attempting the oldest authenticated cold output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum RecoveredLifecycleOutputSettlementV1 {
    /// No cold output remains owned by this height.
    Empty,
    /// Ordering or an active lease defers execution until an earlier owner settles.
    Deferred,
    /// The output source retained responsibility and the same Ready row remains owned.
    SourceRetained,
    /// The exact output service and same-row terminal fsync both completed.
    Completed,
}

/// Fail-stop cold-output settlement error retaining the durable row for restart.
#[derive(Debug)]
pub(in crate::sumeragi) enum RecoveredLifecycleOutputSettlementErrorV1<E> {
    /// The move-only carrier no longer matches the opened logical owner.
    InvalidAuthority(&'static str),
    /// External output I/O failed through the same guarded service as live output.
    Service(E),
    /// External output succeeded but its exact terminal LedgerV1 fsync failed.
    Durability,
}

impl super::ProductionLifecycleOwnerV1 {
    /// Rejoin every retained cold output to this owner's current coordinator.
    pub(super) fn exact_lifecycle_output_ordinals_for_registry_census(
        &self,
    ) -> Option<BTreeSet<u128>> {
        self.recovered_lifecycle_outputs.as_ref().map_or_else(
            || Some(BTreeSet::new()),
            |outputs| outputs.exact_ready_ordinals_for_registry_census(&self.coordinator),
        )
    }

    /// Return whether an authenticated cold output still awaits exact settlement.
    pub(in crate::sumeragi) fn has_recovered_lifecycle_outputs(&self) -> bool {
        self.recovered_lifecycle_outputs
            .as_ref()
            .is_some_and(|outputs| !outputs.entries.is_empty())
    }

    /// Execute and terminalize the oldest eligible authenticated cold output.
    ///
    /// The carrier remains in the owner through service I/O and the exact
    /// LedgerV1 successor fsync. It is removed only after publication succeeds,
    /// so every earlier failure leaves the same row restart-recoverable.
    pub(in crate::sumeragi) fn settle_next_recovered_lifecycle_output<E>(
        &mut self,
        execute: impl FnOnce(
            &crate::sumeragi::v2::AdapterEffect,
        ) -> Result<
            super::concrete_admission::LifecycleOutputServiceDispositionV1,
            E,
        >,
    ) -> Result<RecoveredLifecycleOutputSettlementV1, RecoveredLifecycleOutputSettlementErrorV1<E>>
    {
        let Self {
            verified,
            coordinator,
            recovered_lifecycle_outputs,
            ..
        } = self;
        let Some(outputs) = recovered_lifecycle_outputs.as_mut() else {
            return Ok(RecoveredLifecycleOutputSettlementV1::Empty);
        };
        let Some((&ordinal, output)) = outputs.entries.first_key_value() else {
            return Ok(RecoveredLifecycleOutputSettlementV1::Empty);
        };
        let Some(first_ready) = coordinator.ready_index.first().copied() else {
            return Err(RecoveredLifecycleOutputSettlementErrorV1::InvalidAuthority(
                "cold output remained nonterminal without a Ready ordinal",
            ));
        };
        if coordinator.active_lease.is_some() || first_ready < ordinal {
            return Ok(RecoveredLifecycleOutputSettlementV1::Deferred);
        }
        if first_ready != ordinal
            || !recovered_output_matches_ready_coordinator(verified, coordinator, output)
        {
            return Err(RecoveredLifecycleOutputSettlementErrorV1::InvalidAuthority(
                "cold output changed its exact Ready lifecycle row",
            ));
        }
        match execute(output.effect())
            .map_err(RecoveredLifecycleOutputSettlementErrorV1::Service)?
        {
            super::concrete_admission::LifecycleOutputServiceDispositionV1::Accepted => {}
            super::concrete_admission::LifecycleOutputServiceDispositionV1::SourceRetained => {
                return Ok(RecoveredLifecycleOutputSettlementV1::SourceRetained);
            }
        }

        let mut staged = coordinator.stage_durable_transaction();
        if staged
            .finish_terminal(ordinal, super::TerminalOutcome::Advanced)
            .is_err()
            || !recovered_output_terminal_successor_is_exact(coordinator, &staged, output)
        {
            return Err(RecoveredLifecycleOutputSettlementErrorV1::InvalidAuthority(
                "cold output could not form its exact terminal successor",
            ));
        }
        if coordinator.persist_exact_staged_successor(&staged).is_err() {
            coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            return Err(RecoveredLifecycleOutputSettlementErrorV1::Durability);
        }
        *coordinator = staged;
        let retired = outputs
            .entries
            .remove(&ordinal)
            .expect("fsynced cold output retains its move-only carrier");
        debug_assert_eq!(retired.ordinal(), ordinal);
        Ok(RecoveredLifecycleOutputSettlementV1::Completed)
    }
}

fn recovered_output_matches_ready_coordinator(
    verified: &VerifiedHeightContext,
    coordinator: &LifecycleCoordinator,
    output: &super::replay_authority::AuthenticatedRecoveredLifecycleOutputV1,
) -> bool {
    coordinator.active_context == super::projection::lifecycle_context(verified.context())
        && output.authenticates_settlement(verified)
        && recovered_output_matches_ready_coordinator_row(coordinator, output)
}

fn recovered_output_matches_ready_coordinator_row(
    coordinator: &LifecycleCoordinator,
    output: &super::replay_authority::AuthenticatedRecoveredLifecycleOutputV1,
) -> bool {
    let candidate = output.candidate();
    let ordinal = output.ordinal();
    let (Some(record), Some(metadata)) = (
        coordinator.records.get(&ordinal),
        coordinator.durable_records.get(&ordinal),
    ) else {
        return false;
    };
    let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
        return false;
    };
    coordinator.fault.is_none()
        && candidate.initial_state == super::InitialLifecycleState::Ready
        && record.owner == output.owner()
        && record.owner.causal_root() == candidate.causal_root
        && record.ordinal == ordinal
        && record.key == candidate.key
        && record.work_class == candidate.work_class
        && record.stage == candidate.stage
        && record.state == super::LifecycleState::Ready
        && record.physical_slots == physical
        && record.episode.slot_universe == universe
        && record.episode.consumed_slots == consumed
        && record.episode.frozen_predecessors.is_empty()
        && coordinator.key_index.get(&record.key) == Some(&ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && metadata.matches_admission(candidate)
        && metadata.continuation == DurableContinuation::None
}

fn recovered_output_terminal_successor_is_exact(
    current: &LifecycleCoordinator,
    staged: &LifecycleCoordinator,
    output: &super::replay_authority::AuthenticatedRecoveredLifecycleOutputV1,
) -> bool {
    let ordinal = output.ordinal();
    let (Some(current_record), Some(staged_record)) =
        (current.records.get(&ordinal), staged.records.get(&ordinal))
    else {
        return false;
    };
    staged.fault.is_none()
        && staged.active_context == current.active_context
        && staged.active_lease.is_none()
        && current_record.state == super::LifecycleState::Ready
        && staged_record.owner == current_record.owner
        && staged_record.ordinal == current_record.ordinal
        && staged_record.key == current_record.key
        && staged_record.work_class == current_record.work_class
        && staged_record.stage == current_record.stage
        && staged_record.physical_slots == current_record.physical_slots
        && staged_record.episode == current_record.episode
        && staged_record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
        && !staged.ready_index.contains(&ordinal)
        && staged.key_index == current.key_index
        && staged.owner_index == current.owner_index
        && staged.records.len() == current.records.len()
        && current
            .records
            .iter()
            .all(|(other, record)| *other == ordinal || staged.records.get(other) == Some(record))
        && super::ledger::LifecycleLedgerV1::from_coordinator(staged).is_ok()
}

fn has_durable_sign_predecessor(
    ledger: &LifecycleLedgerV1,
    child: &LifecycleLedgerRecordV1,
) -> bool {
    ledger.records().iter().any(|parent| {
        parent.owner() == child.owner()
            && matches!(
                parent.work_class(),
                Some(
                    LifecycleWorkClass::SignProposal
                        | LifecycleWorkClass::SignVote
                        | LifecycleWorkClass::SignTimeout
                )
            )
            && parent.terminal() == Some(Some(TerminalOutcome::Advanced))
            && parent
                .continuation()
                .and_then(DurableContinuation::successor_parts)
                .is_some_and(|(edge, ordinal)| {
                    ordinal == child.ordinal()
                        && matches!(
                            edge,
                            DurableContinuationEdge::SignProposalToBroadcast
                                | DurableContinuationEdge::SignPrepareToBroadcast
                                | DurableContinuationEdge::SignCommitToBroadcast
                                | DurableContinuationEdge::SignTimeoutToBroadcast
                        )
                })
    })
}

fn unique_invalid_body_validate_parent<'ledger>(
    ledger: &'ledger LifecycleLedgerV1,
    child: &LifecycleLedgerRecordV1,
) -> Option<&'ledger LifecycleLedgerRecordV1> {
    let mut parents = ledger.records().iter().filter(|parent| {
        parent.owner() == child.owner()
            && parent.work_class() == Some(LifecycleWorkClass::Validate)
            && parent.terminal() == Some(Some(TerminalOutcome::Advanced))
            && parent
                .continuation()
                .and_then(DurableContinuation::successor_parts)
                == Some((
                    DurableContinuationEdge::ValidateToInvalidBodyReport,
                    child.ordinal(),
                ))
    });
    let parent = parents.next()?;
    parents.next().is_none().then_some(parent)
}

#[cfg(all(test, feature = "bls"))]
mod output_recovery_tests {
    use super::*;
    use crate::sumeragi::{
        v2::{AdapterEffect, AdapterEquivocationEvidence},
        v2_body_store::{DurableBodyReceipt, DurableBodyValidationOutcome},
        v2_core::{EventTag, Generation},
        v2_lifecycle_coordinator::{
            OwnerId,
            replay_authority::{CertifiedFetchReplayEvidenceV1, DurableValidateReplayEvidenceV1},
        },
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

    fn verified_fixture() -> (VerifiedHeightContext, Vec<KeyPair>) {
        let mut keys = (0x91_u8..=0x94)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic cold-output BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("cold-output-recovery-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"cold output AMX"),
            execution_policy_hash: Hash::new(b"cold output execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0x95; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture proof of possession")
            })
            .collect();
        (
            VerifiedHeightContext::genesis(context, proofs).expect("verify fixture context"),
            keys,
        )
    }

    fn signed_vote(verified: &VerifiedHeightContext, keys: &[KeyPair], marker: u8) -> wire::Vote {
        let round = wire::ConsensusRound {
            context_id: verified.context().id(),
            height: verified.context().height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
            payload_hash: Hash::new([marker, 2]),
        };
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 3]),
                Hash::new([marker, 4]),
                Hash::new([marker, 5]),
                1,
                Hash::new([marker, 6]),
            ),
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        vote
    }

    fn direct_output_record(
        verified: &VerifiedHeightContext,
        effect: AdapterEffect,
        ordinal: u128,
    ) -> LifecycleLedgerRecordV1 {
        let context = super::super::projection::lifecycle_context(verified.context());
        let tag = EventTag::new(verified.context().height, 0, Generation::new(0));
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind direct output")
        .pop()
        .expect("one output owner");
        let pending = ownership
            .exact_pending_adapter_effect_binding(&effect)
            .expect("derive pending output");
        let prepared = super::super::work_registry::PreparedLifecycleAdmissionV1::direct_signed(
            context, verified, effect, pending,
        )
        .unwrap_or_else(|_| panic!("prepare direct output"));
        let candidate = prepared.candidate().clone();
        let owner = OwnerId::new(candidate.causal_root, ordinal);
        LifecycleLedgerRecordV1::new(
            candidate.key,
            owner,
            ordinal,
            candidate.work_class,
            candidate.stage,
            None,
            candidate.reconstruction_source,
            candidate.payload,
            candidate.replay_authority,
            DurableContinuation::None,
        )
        .expect("construct direct output row")
    }

    fn prepare_certificate(
        verified: &VerifiedHeightContext,
        keys: &[KeyPair],
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        corrupt: bool,
    ) -> wire::QuorumCertificate {
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"cold invalid parent state"),
            Hash::new(b"cold invalid events"),
            Hash::new(b"cold invalid trace"),
            1,
            Hash::new(b"cold invalid fee summary"),
        );
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate cold invalid-body PrepareQC"),
        };
        assert!(verified.verify_quorum_certificate(&certificate).is_ok());
        if corrupt {
            certificate.aggregate_signature[0] ^= 1;
        }
        certificate
    }

    #[allow(clippy::too_many_lines)]
    fn invalid_body_ledger(
        verified: &VerifiedHeightContext,
        keys: &[KeyPair],
        parent_ordinal: u128,
        child_ordinal: u128,
        corrupt_certificate: bool,
    ) -> (LifecycleLedgerV1, DurableBodyReceipt) {
        let round = wire::ConsensusRound {
            context_id: verified.context().id(),
            height: verified.context().height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"cold invalid block")),
            payload_hash: Hash::new(b"cold invalid payload"),
        };
        let manifest = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: verified.context().da_layout,
            chunk_hashes: vec![Hash::new(b"cold invalid chunk")],
            chunk_root: Hash::new(b"cold invalid chunk root"),
        };
        let durable = DurableBodyReceipt::for_test(
            verified.context().id(),
            round,
            subject,
            HashOf::new(&manifest),
        );
        let certificate = prepare_certificate(verified, keys, round, subject, corrupt_certificate);
        let tag = EventTag::new(round.height, round.view, Generation::new(1));
        let certified_sources = verified
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources,
            certificate: Some(certificate.clone()),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let fetch_owner = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, parent_ordinal)],
        )
        .expect("bind cold invalid-body Fetch")
        .pop()
        .expect("one Fetch owner");
        let fetch_pending = fetch_owner
            .exact_pending_adapter_effect_binding(&fetch_effect)
            .expect("mint cold invalid-body Fetch binding");
        let store_pending = fetch_pending
            .project_certified_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("project cold invalid-body Store");
        let response = wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(b"cold invalid request")),
            manifest: manifest.clone(),
            body: vec![0x51],
            responder: 0,
            signature: vec![0x52],
        };
        let fetch_replay = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            &fetch_effect,
            &response,
            &durable,
        )
        .expect("project cold invalid-body Fetch replay");
        let store_replay = fetch_replay
            .project_store_for_test(&store_effect, &durable)
            .expect("project cold invalid-body Store replay");
        let validate_pending = store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("project cold invalid-body Validate");
        let validate_replay = DurableValidateReplayEvidenceV1::certified(
            store_replay
                .project_validate(&store_effect, &durable, &validate_effect, &validate_pending)
                .expect("project cold invalid-body Validate replay"),
        );
        let validate_candidate = validate_replay
            .project_candidate_for_test(verified, &validate_effect, &durable, &validate_pending)
            .expect("project cold invalid-body Validate candidate");
        let report_effect = AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        };
        let report_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                &validate_effect,
                &report_effect,
            )
            .expect("project cold invalid-body Report binding");
        let report_candidate =
            super::super::replay_authority::exact_invalid_body_report_candidate_for_test(
                verified,
                &validate_replay,
                &validate_effect,
                &validate_pending,
                &durable,
                &report_effect,
                &report_pending,
            )
            .expect("project cold invalid-body Report candidate");
        assert_eq!(validate_candidate.causal_root, report_candidate.causal_root);
        assert!(child_ordinal > parent_ordinal);
        let owner = OwnerId::new(validate_candidate.causal_root, parent_ordinal);
        let parent = LifecycleLedgerRecordV1::new(
            validate_candidate.key,
            owner,
            parent_ordinal,
            validate_candidate.work_class,
            validate_candidate.stage,
            Some(TerminalOutcome::Advanced),
            validate_candidate.reconstruction_source,
            validate_candidate.payload,
            validate_candidate.replay_authority,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToInvalidBodyReport,
                child_ordinal,
            ),
        )
        .expect("construct cold invalid-body Validate parent");
        let child = LifecycleLedgerRecordV1::new(
            report_candidate.key,
            owner,
            child_ordinal,
            report_candidate.work_class,
            report_candidate.stage,
            None,
            report_candidate.reconstruction_source,
            report_candidate.payload,
            report_candidate.replay_authority,
            DurableContinuation::None,
        )
        .expect("construct cold invalid-body Report child");
        let ledger = LifecycleLedgerV1::new(
            super::super::projection::lifecycle_context(verified.context()),
            child_ordinal,
            vec![parent, child],
            BTreeMap::new(),
        )
        .expect("construct cold invalid-body ledger");
        (ledger, durable)
    }

    #[test]
    fn cold_output_recovery_accepts_authenticated_broadcast_at_exact_ordinal() {
        let (verified, keys) = verified_fixture();
        let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(
            signed_vote(&verified, &keys, 0xA1),
        ));
        let record = direct_output_record(&verified, AdapterEffect::Broadcast(message), 7);
        let ledger = LifecycleLedgerV1::new(
            super::super::projection::lifecycle_context(verified.context()),
            7,
            vec![record],
            BTreeMap::new(),
        )
        .expect("construct authenticated Broadcast ledger");
        let recovered = PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified)
            .expect("authenticate Broadcast cold output");
        let output = recovered.entries.get(&7).expect("retain exact ordinal");
        assert_eq!(output.ordinal(), 7);
        assert_eq!(output.owner(), ledger.records()[0].owner());
        assert!(output.authenticates_settlement(&verified));
    }

    #[test]
    fn cold_output_recovery_rejects_tampered_broadcast_signature() {
        let (verified, keys) = verified_fixture();
        let mut vote = signed_vote(&verified, &keys, 0xA2);
        vote.signature[0] ^= 1;
        let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote));
        let record = direct_output_record(&verified, AdapterEffect::Broadcast(message), 8);
        let ledger = LifecycleLedgerV1::new(
            super::super::projection::lifecycle_context(verified.context()),
            8,
            vec![record],
            BTreeMap::new(),
        )
        .expect("construct tampered Broadcast ledger");
        assert!(matches!(
            PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified),
            Err(
                LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                    ordinal: 8,
                    ..
                }
            )
        ));
    }

    #[test]
    fn cold_output_recovery_reconstructs_same_equivocation_row_after_restart() {
        let (verified, keys) = verified_fixture();
        let first = signed_vote(&verified, &keys, 0xA3);
        let second = signed_vote(&verified, &keys, 0xA4);
        let persisted = crate::sumeragi::evidence::canonicalize_v2_conflict(
            &wire::SumeragiV2Equivocation::PhaseVote { first, second },
        );
        let evidence: AdapterEquivocationEvidence = verified
            .authenticate_recovered_equivocation(&persisted)
            .expect("authenticate equivocation fixture");
        let record =
            direct_output_record(&verified, AdapterEffect::ReportEquivocation { evidence }, 9);
        let ledger = LifecycleLedgerV1::new(
            super::super::projection::lifecycle_context(verified.context()),
            9,
            vec![record],
            BTreeMap::new(),
        )
        .expect("construct equivocation ledger");
        let first_open = PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified)
            .expect("first cold open");
        let restart = PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified)
            .expect("restart cold open");
        let first = first_open.entries.get(&9).expect("first exact row");
        let second = restart.entries.get(&9).expect("restart exact row");
        assert_eq!(first.owner(), second.owner());
        assert_eq!(first.ordinal(), second.ordinal());
        assert_eq!(first.candidate(), second.candidate());
        assert!(second.authenticates_settlement(&verified));
    }

    #[test]
    fn cold_output_recovery_accepts_exact_invalid_body_parent_qc_and_marker() {
        let (verified, keys) = verified_fixture();
        let (ledger, durable) = invalid_body_ledger(&verified, &keys, 10, 11, false);
        let recovered = PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified)
            .expect("authenticate cold invalid-body Report");
        let report = recovered.entries.get(&11).expect("retain exact report row");
        assert!(report.authenticates_settlement(&verified));
        assert!(report.exactly_matches_rejected_body_outcome(
            &DurableBodyValidationOutcome::rejected_for_test(durable.clone())
        ));
        let foreign = DurableBodyReceipt::for_test(
            durable.context_id(),
            durable.round(),
            durable.subject(),
            HashOf::from_untyped_unchecked(Hash::new(b"foreign cold invalid manifest")),
        );
        assert!(!report.exactly_matches_rejected_body_outcome(
            &DurableBodyValidationOutcome::rejected_for_test(foreign)
        ));
    }

    #[test]
    fn cold_output_recovery_accepts_invalid_body_lineage_across_shared_ordinal_gap() {
        let (verified, keys) = verified_fixture();
        let (ledger, durable) = invalid_body_ledger(&verified, &keys, 20, 23, false);
        let recovered = PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified)
            .expect("authenticate invalid-body Report after intervening runtime ordinals");
        let report = recovered.entries.get(&23).expect("retain exact report row");
        assert!(report.authenticates_settlement(&verified));
        assert!(report.exactly_matches_rejected_body_outcome(
            &DurableBodyValidationOutcome::rejected_for_test(durable)
        ));
    }

    #[test]
    fn cold_output_recovery_rejects_invalid_body_with_corrupt_prepare_qc() {
        let (verified, keys) = verified_fixture();
        let (ledger, _durable) = invalid_body_ledger(&verified, &keys, 12, 13, true);
        assert!(matches!(
            PreparedLifecycleOutputRecoveryV1::assemble(&ledger, &verified),
            Err(
                LifecycleRecoveryAssemblyErrorKind::InvalidLifecycleOutputRecovery {
                    ordinal: 13,
                    ..
                }
            )
        ));
    }
}
