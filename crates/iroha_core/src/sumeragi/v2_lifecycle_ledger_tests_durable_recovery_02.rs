use crate::sumeragi::v2_lifecycle_coordinator::ProductionSchedulerInputsError;

#[derive(Clone, Copy)]
enum StandaloneValidateOriginFixture {
    LocalBody,
    RemoteProposal {
        valid_signature: bool,
    },
    RefinedRemoteProposal {
        phase: wire::GlobalPhase,
        corrupt_qc: bool,
    },
}

fn run_durable_recovery_test_on_stack(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("sumeragi-v2-durable-recovery-test".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(body)
        .expect("spawn durable-recovery test thread")
        .join()
        .expect("durable-recovery test thread completes");
}

#[allow(clippy::too_many_lines)]
fn standalone_validate_record(
    fixture: &RecoveryFixture,
    store: &mut V2BodyStore,
    view: u64,
    marker: u8,
    ordinal: u128,
    origin: StandaloneValidateOriginFixture,
) -> LifecycleLedgerRecordV1 {
    use crate::sumeragi::{
        v2::AdapterEffect,
        v2_lifecycle_coordinator::work_registry::{
            PreparedLocalBodyValidateReplayPreAdmission,
            PreparedRemoteProposalFetchReplayPreAdmission,
        },
        v2_runtime::{
            LocalProposalEffectOwnership, RuntimeEffectOwnership,
            bind_adapter_effect_batch_ownership,
        },
    };

    let context = fixture.verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let leader = context.leader(view);
    let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
    let header = BlockHeader::new(
        NonZeroU64::new(context.height).expect("fixture height is non-zero"),
        None,
        None,
        None,
        4_000 + u64::from(marker),
        view,
    );
    let block_signature =
        SignatureOf::try_from_hash(fixture.keys[leader_index].private_key(), header.hash())
            .expect("sign standalone Validate block");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(leader), block_signature),
        header,
        Vec::new(),
    );
    let body = block
        .encode_wire()
        .expect("encode standalone Validate SignedBlockWire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    let chunks = wire::encode_payload_chunks(context.da_layout, &body)
        .expect("encode standalone Validate chunks");
    let manifest = wire::PayloadManifest::derive(
        context,
        round,
        subject,
        u64::try_from(body.len()).expect("standalone body length fits u64"),
        &chunks,
    )
    .expect("derive standalone Validate manifest");
    let durable_receipt = store
        .store(manifest.clone(), body)
        .expect("fsync standalone Validate body");
    let tag = EventTag::new(context.height, view, Generation::new(u64::from(marker)));
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
    let prepared = match origin {
        StandaloneValidateOriginFixture::LocalBody => {
            let store_ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&store_effect),
                vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
            )
            .expect("bind standalone local Store owner")
            .pop()
            .expect("one standalone local Store owner");
            let local =
                LocalProposalEffectOwnership::for_test(store_ownership, &store_effect, &manifest)
                    .expect("seal standalone local Store replay");
            let validate_ownership = local
                .exact_store_task_ownership(&store_effect, &manifest)
                .expect("project standalone local Store scheduling owner")
                .rebind_as_inherited_adapter_effect(&validate_effect)
                .expect("project standalone local Validate owner");
            let replay = local
                .project_exact_validate(
                    &store_effect,
                    &manifest,
                    &durable_receipt,
                    &validate_effect,
                    &validate_ownership,
                )
                .unwrap_or_else(|_| panic!("project standalone local Validate replay"));
            PreparedLocalBodyValidateReplayPreAdmission::seal_exact_validate(
                validate_effect,
                validate_ownership,
                durable_receipt,
                replay,
            )
            .unwrap_or_else(|_| panic!("seal standalone local Validate pre-admission"))
            .prepare_lifecycle_admission(fixture.lifecycle_context(), &fixture.verified)
            .unwrap_or_else(|_| panic!("prepare standalone local Validate admission"))
        }
        StandaloneValidateOriginFixture::RemoteProposal { .. }
        | StandaloneValidateOriginFixture::RefinedRemoteProposal { .. } => {
            let valid_signature = match origin {
                StandaloneValidateOriginFixture::RemoteProposal { valid_signature } => {
                    valid_signature
                }
                StandaloneValidateOriginFixture::RefinedRemoteProposal { .. } => true,
                StandaloneValidateOriginFixture::LocalBody => unreachable!(),
            };
            let mut proposal = wire::Proposal {
                round,
                proposer: leader,
                subject,
                manifest: manifest.clone(),
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: Vec::new(),
            };
            proposal.signature = Signature::new(
                fixture.keys[leader_index].private_key(),
                &proposal.signature_preimage(),
            )
            .payload()
            .to_vec();
            if !valid_signature {
                proposal.signature[0] ^= 1;
            }
            let fetch_effect = AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            };
            let mut fetch_ownership = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&fetch_effect),
                vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
            )
            .expect("bind standalone remote Fetch owner")
            .pop()
            .expect("one standalone remote Fetch owner");
            let store_ownership = fetch_ownership
                .rebind_as_inherited_adapter_effect(&store_effect)
                .expect("project standalone remote Store owner");
            let ordinary_validate_ownership = store_ownership
                .rebind_as_inherited_adapter_effect(&validate_effect)
                .expect("project standalone remote Validate owner");
            assert!(
                fetch_ownership
                    .bind_authenticated_remote_proposal_replay_for_test(proposal, &fetch_effect,)
            );
            let stored = PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(
                fetch_effect,
                fetch_ownership,
            )
            .unwrap_or_else(|_| panic!("seal standalone remote Fetch pre-admission"))
            .project_store(store_effect, store_ownership)
            .unwrap_or_else(|_| panic!("project standalone remote Store pre-admission"))
            .bind_durable_body(durable_receipt)
            .unwrap_or_else(|_| panic!("bind standalone remote durable body"));
            let validate = match origin {
                StandaloneValidateOriginFixture::RemoteProposal { .. } => stored
                    .project_validate(validate_effect.clone(), ordinary_validate_ownership, None)
                    .unwrap_or_else(|_| panic!("project standalone remote Validate pre-admission")),
                StandaloneValidateOriginFixture::RefinedRemoteProposal { phase, corrupt_qc } => {
                    let execution_commitment =
                        wire::ExecutionCommitment::without_topups_or_merge_carrier(
                            Hash::new([marker, 0xA1]),
                            Hash::new([marker, 0xA2]),
                            Hash::new([marker, 0xA3]),
                            1,
                            Hash::new([marker, 0xA4]),
                        );
                    let preimage = wire::Vote {
                        round,
                        proposal_round: round,
                        phase,
                        subject,
                        execution_commitment,
                        signer: 0,
                        signature: Vec::new(),
                    }
                    .signature_preimage();
                    let shares = fixture.keys[..3]
                        .iter()
                        .map(|key| {
                            Signature::new(key.private_key(), &preimage)
                                .payload()
                                .to_vec()
                        })
                        .collect::<Vec<_>>();
                    let mut certificate = wire::QuorumCertificate {
                        round,
                        proposal_round: round,
                        phase,
                        subject,
                        execution_commitment,
                        signers: vec![0, 1, 2],
                        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                        )
                        .expect("aggregate refined standalone Proposal QC"),
                    };
                    if corrupt_qc {
                        certificate.aggregate_signature[0] ^= 1;
                    }
                    let certified_fetch = AdapterEffect::FetchBody {
                        tag,
                        round,
                        subject,
                        manifest: Some(manifest.clone()),
                        certified_sources: context
                            .roster
                            .iter()
                            .map(|entry| entry.validator.clone())
                            .collect(),
                        certificate: Some(certificate.clone()),
                    };
                    let certified_validate_ownership = bind_adapter_effect_batch_ownership(
                        core::slice::from_ref(&certified_fetch),
                        vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal + 1)],
                    )
                    .expect("bind refined standalone remote Fetch authority")
                    .pop()
                    .expect("one refined standalone remote Fetch owner")
                    .rebind_as_inherited_adapter_effect(&validate_effect)
                    .expect("carry refined standalone authority into Validate");
                    let validate_ownership = ordinary_validate_ownership
                        .adopt_incumbent_body_stage_for_retry_or_authority(
                            &certified_validate_ownership,
                            &validate_effect,
                        )
                        .expect("retain standalone Proposal owner under refined authority");
                    match phase {
                        wire::GlobalPhase::Prepare => stored
                            .project_validate(
                                validate_effect.clone(),
                                validate_ownership,
                                Some(&certificate),
                            )
                            .unwrap_or_else(|_| {
                                panic!("project Prepare-refined standalone Proposal Validate")
                            }),
                        wire::GlobalPhase::Commit => stored
                            .project_validate_after_durable_decision(
                                validate_effect.clone(),
                                validate_ownership,
                                &certificate,
                            )
                            .unwrap_or_else(|_| {
                                panic!("project Commit-refined standalone Proposal Validate")
                            }),
                    }
                }
                StandaloneValidateOriginFixture::LocalBody => unreachable!(),
            };
            validate
                .prepare_lifecycle_admission(fixture.lifecycle_context(), &fixture.verified)
                .unwrap_or_else(|_| panic!("prepare standalone remote Validate admission"))
        }
    };
    let candidate = prepared.candidate().clone();
    assert_eq!(candidate.work_class, LifecycleWorkClass::Validate);
    assert_eq!(candidate.stage.kind(), LifecycleStageKind::ValidateBody);
    assert_eq!(candidate.initial_state, InitialLifecycleState::Ready);
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
    .expect("construct standalone Validate LedgerV1 row")
}

#[test]
fn complete_tip_terminal_apply_store_join_rejects_store_drift() {
    let fixture = RecoveryFixture::new("complete-tip-predecessor-drift", 0x49);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let directory = TempDir::new().expect("temporary drifted predecessor ledger");
    let complete_tip =
        complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
    let (store, empty) =
        LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
            .expect("open drifted CompleteTip predecessor store");
    store
        .persist(&ledger)
        .expect("persist terminal CompleteTip predecessor");
    let apply_ordinal = ledger
        .authenticate_complete_tip_terminal_apply(&complete_tip)
        .expect("authenticate terminal Apply before store drift");
    store
        .persist(&empty)
        .expect("replace predecessor before cut authentication");
    assert!(
        ledger
            .into_complete_tip_terminal_apply_store_join(
                store,
                complete_tip,
                CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(apply_ordinal),
            )
            .is_err(),
        "a changed attached frame cannot mint predecessor authority"
    );
}
#[test]
fn complete_tip_terminal_apply_store_join_rejects_an_identical_foreign_target() {
    let fixture = RecoveryFixture::new("complete-tip-predecessor-foreign-target", 0x4A);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let canonical_kura = Kura::blank_kura_for_testing();
    let foreign_kura = Kura::blank_kura_for_testing();
    let complete_tip =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, canonical_kura.as_ref());
    let foreign_root = foreign_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (foreign_store, empty) =
        LifecycleLedgerStoreV1::open(&foreign_root, fixture.lifecycle_context())
            .expect("open foreign predecessor store");
    assert!(empty.records().is_empty());
    foreign_store
        .persist(&ledger)
        .expect("copy exact terminal predecessor frame to foreign root");
    let apply_ordinal = ledger
        .authenticate_complete_tip_terminal_apply(&complete_tip)
        .expect("authenticate terminal Apply for canonical CompleteTip");
    assert!(
        ledger
            .into_complete_tip_terminal_apply_store_join(
                foreign_store,
                complete_tip,
                CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(apply_ordinal),
            )
            .is_err(),
        "byte-identical ledger data cannot substitute for the Kura-bound target"
    );
}
#[test]
fn complete_tip_successor_target_initializes_and_accepts_an_exact_descendant() {
    let context = LifecycleContext::new(LifecycleDigest::new([0xA1; 32]), 2);
    let directory = TempDir::new().expect("temporary CompleteTip successor target");
    let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
        root: directory.path().join("successor"),
        context,
    };
    let (store, initialized) = target
        .open_initialized_or_descendant(4)
        .expect("initialize successor at predecessor high-water");
    assert_eq!(initialized.high_water(), 4);
    assert!(initialized.records().is_empty());
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xA2; 32])), 5);
    let descendant = LifecycleLedgerV1::new(
        context,
        5,
        vec![unrelated_live_record(context, owner, 5, 0xA3)],
        BTreeMap::new(),
    )
    .expect("construct exact successor descendant");
    store
        .persist_exact_successor(&initialized, &descendant)
        .expect("publish descendant above retained ordinal floor");
    let (_, reopened) = target
        .open_initialized_or_descendant(4)
        .expect("preserve a valid nonempty descendant without rewriting it");
    assert_eq!(reopened, descendant);
}
#[test]
fn complete_tip_successor_target_rejects_a_foreign_ordinal_floor() {
    let context = LifecycleContext::new(LifecycleDigest::new([0xB1; 32]), 2);
    let directory = TempDir::new().expect("temporary foreign-floor successor target");
    let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
        root: directory.path().join("successor"),
        context,
    };
    let (store, empty) =
        LifecycleLedgerStoreV1::open(&target.root, context).expect("open foreign-floor successor");
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xB2; 32])), 4);
    let foreign = LifecycleLedgerV1::new(
        context,
        4,
        vec![unrelated_live_record(context, owner, 4, 0xB3)],
        BTreeMap::new(),
    )
    .expect("construct independently zero-based successor frame");
    store
        .persist_exact_successor(&empty, &foreign)
        .expect("persist foreign successor fixture");
    assert!(target.open_initialized_or_descendant(4).is_err());
}
#[test]
fn terminal_recovered_decision_oracle_rejects_a_live_apply() {
    let fixture = RecoveryFixture::new("terminal-decision-live-apply", 0x35);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let mut records = ledger.records.clone();
    records[3].terminal = None;
    let live = LifecycleLedgerV1::new(
        ledger.context(),
        ledger.high_water(),
        records,
        BTreeMap::new(),
    )
    .expect("construct otherwise exact chain with a live Apply");
    assert!(
        live.authenticate_terminal_recovered_decision_apply_projection(&projection)
            .is_err()
    );
}
#[test]
fn terminal_recovered_decision_oracle_rejects_extra_same_owner_history() {
    let fixture = RecoveryFixture::new("terminal-decision-same-owner", 0x39);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let owner = projection.fetch.owner();
    let mut records = ledger.records.clone();
    records.push(unrelated_live_record(ledger.context(), owner, 5, 0xE2));
    let with_extra_owner_history =
        LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
            .expect("construct terminal chain with foreign same-owner history");
    assert!(
        with_extra_owner_history
            .authenticate_terminal_recovered_decision_apply_projection(&projection)
            .is_err()
    );
}
#[test]
fn terminal_recovered_decision_oracle_is_chain_local_and_allows_a_foreign_live_row() {
    let fixture = RecoveryFixture::new("terminal-decision-chain-local", 0x3D);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let foreign_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"foreign live row outside terminal Decision chain").as_ref(),
    ));
    let foreign_owner = OwnerId::new(foreign_root, 5);
    let mut records = ledger.records.clone();
    records.push(unrelated_live_record(
        ledger.context(),
        foreign_owner,
        5,
        0xE3,
    ));
    let with_foreign_live = LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
        .expect("construct terminal chain beside one foreign live row");
    assert_eq!(
        with_foreign_live
            .authenticate_terminal_recovered_decision_apply_projection(&projection)
            .expect("the terminal oracle is intentionally limited to one owner chain"),
        4
    );
}
#[test]
fn recovered_decision_stage_guard_routes_terminal_chain_to_complete_tip_retirement() {
    let fixture = RecoveryFixture::new("terminal-decision-stage-guard", 0x41);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let error = ledger
        .reject_terminal_recovered_decision_apply_projection(&projection)
        .expect_err("terminal Apply cannot re-enter live recovered staging");
    assert!(matches!(
        error,
        LifecycleLedgerError::InvalidLedger(reason)
            if reason == "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
    ));
}
fn admit_and_claim_serve(
    fixture: &RecoveryFixture,
    owner: &mut ProductionLifecycleOwnerV1,
    request: &AuthenticatedCertifiedBodyRequest,
) -> super::super::super::TurnLease {
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let admitted = owner.admit_selected_certified_serve(target, &fixture.keys[0], request);
    assert!(matches!(
        admitted.decision(),
        Some(super::super::super::AdmissionDecision::Admitted { .. })
    ));
    owner.claim_certified_serve_for_test()
}
fn move_body_store_to_test_worker(owner: &mut ProductionLifecycleOwnerV1) -> V2BodyStore {
    let body_store = owner
        .body_store
        .take()
        .expect("prelaunch test owner retains its exact body store");
    assert!(owner.body_store_identity.is_none());
    owner.body_store_identity = Some(body_store.instance_identity());
    body_store
}
#[test]
fn consuming_storage_cut_censes_every_live_fetch_and_binds_exact_ledger_frame() {
    let fixture = RecoveryFixture::new("durable-ready-fetch-census", 0x31);
    let directory = TempDir::new().expect("temporary durable Ready-Fetch store");
    let mut store = fixture.open_store(&directory);
    let first = fixture.fetch_record(&mut store, 0, 0x41, 1, None, false);
    let second = fixture.fetch_record(&mut store, 1, 0x42, 2, None, false);
    let ledger = fixture.ledger(vec![first, second]);
    let ledger_directory = TempDir::new().expect("temporary durable Ready-Fetch lifecycle ledger");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let mut cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            store,
        )
        .expect("all live durable Fetch rows form one consuming storage cut");
    assert_eq!(
        cut.ledger
            .records
            .iter()
            .filter(|record| record.work_class() == Some(LifecycleWorkClass::Fetch))
            .count(),
        2,
    );
    assert!(cut.is_exact(), "the opaque census covers both live rows");
    cut.ledger.high_water += 1;
    assert!(
        !cut.is_exact(),
        "the census cannot cross even a structurally harmless foreign ledger frame",
    );
}
#[test]
fn production_owner_opens_empty_and_two_fetch_storage_atomically() {
    let empty_fixture = RecoveryFixture::new("empty-production-lifecycle-owner", 0x11);
    let empty_body_directory = TempDir::new().expect("temporary empty production body store");
    let empty_body_store = empty_fixture.open_store(&empty_body_directory);
    let empty_payload_directory = TempDir::new().expect("temporary empty production payload store");
    let (empty_payload_store, empty_payloads) =
        empty_fixture.open_empty_serve_payloads(&empty_payload_directory, &empty_body_store);
    let empty_ledger = empty_fixture.ledger(Vec::new());
    let empty_ledger_directory = TempDir::new().expect("temporary empty production ledger store");
    let empty_ledger_store = empty_fixture.persist_ledger(&empty_ledger_directory, &empty_ledger);
    let empty_cut = empty_ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            empty_fixture.verified.clone(),
            empty_ledger_store,
            empty_body_store,
        )
        .expect("seal empty production storage cut");
    let mut empty_owner = empty_cut
        .open_owner_for_test(empty_payload_store, empty_payloads)
        .expect("open empty production lifecycle owner");
    assert!(empty_owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(empty_owner.live_fetch_count_for_test(), 0);
    assert_eq!(empty_owner.plan_direct_registry_turn(), Ok(TurnPlan::Idle));
    let fixture = RecoveryFixture::new("two-fetch-production-lifecycle-owner", 0x21);
    let body_directory = TempDir::new().expect("temporary two-Fetch body store");
    let mut body_store = fixture.open_store(&body_directory);
    let first = fixture.fetch_record(&mut body_store, 0, 0x31, 1, None, false);
    let second = fixture.fetch_record(&mut body_store, 1, 0x32, 2, None, false);
    let payload_directory = TempDir::new().expect("temporary two-Fetch payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(vec![first, second]);
    let ledger_directory = TempDir::new().expect("temporary two-Fetch ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal two-Fetch production storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("open two-Fetch production lifecycle owner");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(owner.live_fetch_count_for_test(), 2);
}

fn cold_broadcast_output_fixture(
    fixture: &RecoveryFixture,
    ordinal: u128,
) -> (
    crate::sumeragi::v2::AdapterEffect,
    LifecycleLedgerRecordV1,
) {
    use crate::sumeragi::{
        v2::AdapterEffect,
        v2_lifecycle_coordinator::work_registry::PreparedLifecycleAdmissionV1,
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    let context = fixture.verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"source-retained cold Broadcast block",
        )),
        payload_hash: Hash::new(b"source-retained cold Broadcast payload"),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"source-retained cold Broadcast parent state"),
        Hash::new(b"source-retained cold Broadcast post state"),
        Hash::new(b"source-retained cold Broadcast writes"),
        1,
        Hash::new(b"source-retained cold Broadcast fee summary"),
    );
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(fixture.keys[0].private_key(), &vote.signature_preimage())
        .payload()
        .to_vec();
    let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(vote),
    ));
    let tag = EventTag::new(context.height, round.view, Generation::new(0x23));
    let ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
    )
    .expect("bind cold Broadcast output")
    .pop()
    .expect("one cold Broadcast owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&effect)
        .expect("derive cold Broadcast pending owner");
    let prepared = PreparedLifecycleAdmissionV1::direct_signed(
        fixture.lifecycle_context(),
        &fixture.verified,
        effect.clone(),
        pending,
    )
    .unwrap_or_else(|_| panic!("prepare authenticated cold Broadcast output"));
    let candidate = prepared.candidate().clone();
    let expected_owner = OwnerId::new(candidate.causal_root, ordinal);
    let record = LifecycleLedgerRecordV1::new(
        candidate.key,
        expected_owner,
        ordinal,
        candidate.work_class,
        candidate.stage,
        None,
        candidate.reconstruction_source,
        candidate.payload,
        candidate.replay_authority,
        DurableContinuation::None,
    )
    .expect("construct authenticated cold Broadcast row");
    (effect, record)
}

#[test]
#[allow(clippy::too_many_lines)]
fn cold_broadcast_source_retention_preserves_ready_row_until_exact_acceptance() {
    use crate::sumeragi::v2_lifecycle_coordinator::{
        LifecycleOutputServiceDispositionV1, ProductionCompletionReadyWorkV1,
        open::RecoveredLifecycleOutputSettlementV1,
    };

    let fixture = RecoveryFixture::new("source-retained-cold-broadcast", 0x23);
    let ordinal = 1;
    let (effect, record) = cold_broadcast_output_fixture(&fixture, ordinal);
    let expected_owner = record.owner();

    let body_directory = TempDir::new().expect("temporary cold Broadcast body store");
    let body_store = fixture.open_store(&body_directory);
    let payload_directory = TempDir::new().expect("temporary cold Broadcast payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(vec![record]);
    let ledger_directory = TempDir::new().expect("temporary cold Broadcast ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("authenticate cold Broadcast storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("cold-open authenticated Broadcast owner");
    assert!(owner.has_recovered_lifecycle_outputs());
    assert_eq!(owner.coordinator.records[&ordinal].owner, expected_owner);
    assert_eq!(
        owner.coordinator.records[&ordinal].state,
        LifecycleState::Ready
    );
    assert!(
        owner
            .exact_lifecycle_output_ordinals_for_registry_census()
            .is_some_and(|ordinals| ordinals == [ordinal].into_iter().collect())
    );
    assert!(owner.coordinator.ready_index.remove(&ordinal));
    assert!(
        owner
            .exact_lifecycle_output_ordinals_for_registry_census()
            .is_none()
    );
    assert!(owner.coordinator.ready_index.insert(ordinal));
    assert_eq!(
        owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None),
        ProductionCompletionReadyWorkV1::PassThrough,
        "the authenticated cold Broadcast remains passive while its owner holds settlement"
    );
    let recovered_outputs = owner
        .recovered_lifecycle_outputs
        .take()
        .expect("cold Broadcast owner retains its move-only output census");
    assert_eq!(
        owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None),
        ProductionCompletionReadyWorkV1::Invalid,
        "a logical Ready Broadcast cannot pass without its cold owner or a registry carrier"
    );
    owner.recovered_lifecycle_outputs = Some(recovered_outputs);

    let calls = std::cell::Cell::new(0_u8);
    assert!(matches!(
        owner.settle_next_recovered_lifecycle_output(|observed| {
            assert_eq!(observed, &effect);
            calls.set(calls.get().saturating_add(1));
            Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                LifecycleOutputServiceDispositionV1::SourceRetained,
            )
        }),
        Ok(RecoveredLifecycleOutputSettlementV1::SourceRetained)
    ));
    assert_eq!(calls.get(), 1);
    assert!(owner.has_recovered_lifecycle_outputs());
    assert_eq!(
        owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None),
        ProductionCompletionReadyWorkV1::PassThrough,
        "SourceRetained preserves the authenticated passive scheduler carrier"
    );
    assert_eq!(owner.coordinator.records[&ordinal].owner, expected_owner);
    assert_eq!(
        owner.coordinator.records[&ordinal].state,
        LifecycleState::Ready
    );
    let retained = owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("cold Broadcast owner retains its ledger store")
        .load()
        .expect("reload source-retained cold Broadcast row");
    assert_eq!(retained.records()[0].owner(), expected_owner);
    assert_eq!(retained.records()[0].terminal(), Some(None));

    assert!(matches!(
        owner.settle_next_recovered_lifecycle_output(|observed| {
            assert_eq!(observed, &effect);
            calls.set(calls.get().saturating_add(1));
            Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                LifecycleOutputServiceDispositionV1::Accepted,
            )
        }),
        Ok(RecoveredLifecycleOutputSettlementV1::Completed)
    ));
    assert_eq!(calls.get(), 2);
    assert!(!owner.has_recovered_lifecycle_outputs());
    assert_eq!(
        owner.coordinator.records[&ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    let terminal = owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("accepted cold Broadcast retains its ledger store")
        .load()
        .expect("reload terminal cold Broadcast row");
    assert_eq!(terminal.records()[0].owner(), expected_owner);
    assert_eq!(
        terminal.records()[0].terminal(),
        Some(Some(TerminalOutcome::Advanced))
    );
}

#[test]
fn later_cold_broadcast_stays_passive_until_an_older_fetch_retires() {
    use crate::sumeragi::v2_lifecycle_coordinator::{
        LifecycleOutputServiceDispositionV1, ProductionCompletionReadyWorkV1,
        open::RecoveredLifecycleOutputSettlementV1, work_registry::ConcreteWorkAddress,
    };

    let fixture = RecoveryFixture::new("older-fetch-before-cold-broadcast", 0x24);
    let body_directory = TempDir::new().expect("temporary ordered cold-output body store");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch_ordinal = 1;
    let broadcast_ordinal = 2;
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x34, fetch_ordinal, None, false);
    let (broadcast_effect, broadcast) =
        cold_broadcast_output_fixture(&fixture, broadcast_ordinal);
    let payload_directory = TempDir::new().expect("temporary ordered cold-output payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(vec![fetch, broadcast]);
    let ledger_directory = TempDir::new().expect("temporary ordered cold-output ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("authenticate ordered cold-output storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("cold-open ordered lifecycle owner");

    let calls = std::cell::Cell::new(0_u8);
    assert_eq!(
        owner
            .settle_next_recovered_lifecycle_output(|_| {
                calls.set(calls.get().saturating_add(1));
                Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                    LifecycleOutputServiceDispositionV1::Accepted,
                )
            })
            .expect("an older Ready row defers cold-output settlement"),
        RecoveredLifecycleOutputSettlementV1::Deferred
    );
    assert_eq!(calls.get(), 0, "Deferred cannot enter output service");
    assert_eq!(
        owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None),
        ProductionCompletionReadyWorkV1::CompletionIo,
        "the older Fetch remains schedulable while the later cold Broadcast is passive"
    );

    let fetch_record = owner.coordinator.records[&fetch_ordinal].clone();
    let (&fetch_slot, &fetch_digest) = fetch_record
        .physical_slots
        .first_key_value()
        .expect("recovered Fetch retains one exact physical slot");
    let fetch_address =
        ConcreteWorkAddress::new(fetch_record.owner, fetch_ordinal, fetch_slot)
            .expect("recovered Fetch retains an exact registry address");
    let mut staged = owner.coordinator.stage_durable_transaction();
    staged
        .finish_terminal(fetch_ordinal, TerminalOutcome::Cancelled)
        .expect("retire the already-authenticated older Fetch fixture");
    owner
        .coordinator
        .persist_exact_staged_successor(&staged)
        .expect("publish the older Fetch terminal cut");
    drop(
        owner
            .registry
            .registry_mut()
            .rollback_exact(fetch_address, fetch_digest)
            .expect("retire the exact older Fetch registry carrier"),
    );
    owner.coordinator = staged;

    assert_eq!(
        owner
            .settle_next_recovered_lifecycle_output(|observed| {
                assert_eq!(observed, &broadcast_effect);
                calls.set(calls.get().saturating_add(1));
                Ok::<LifecycleOutputServiceDispositionV1, &'static str>(
                    LifecycleOutputServiceDispositionV1::Accepted,
                )
            })
            .expect("the next bounded retry settles the newly-oldest cold Broadcast"),
        RecoveredLifecycleOutputSettlementV1::Completed
    );
    assert_eq!(calls.get(), 1);
    assert!(!owner.has_recovered_lifecycle_outputs());
    assert_eq!(
        owner.coordinator.records[&broadcast_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
}

#[test]
fn production_owner_cold_opens_exact_ready_store_crash_boundary() {
    let fixture = RecoveryFixture::new("ready-store-cold-open", 0x25);
    let body_directory = TempDir::new().expect("temporary Ready Store body store");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x35, 1, None, false);
    let seed = fixture.ledger(vec![fetch]);
    let records = seed
        .authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store)
        .expect("authenticate the durable Fetch origin")
        .project_ready_store_records_for_test(&fixture.verified)
        .expect("project the exact terminal Fetch to live Store crash boundary");
    let ledger = fixture.ledger(records);
    let payload_directory = TempDir::new().expect("temporary Ready Store payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger_directory = TempDir::new().expect("temporary Ready Store ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal the exact Ready Store recovery cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("cold-open the exact Ready Store carrier");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(owner.live_body_pipeline_counts_for_test(), (0, 1, 0));
    let TurnPlan::Execute(lease) = owner
        .plan_direct_registry_turn()
        .expect("the recovered Store registry census is schedulable")
    else {
        panic!("the recovered Store must be Ready")
    };
    assert_eq!(lease.work_class(), LifecycleWorkClass::Store);
}

#[test]
fn production_owner_cold_opens_exact_ready_validate_crash_boundary() {
    let fixture = RecoveryFixture::new("ready-validate-cold-open", 0x29);
    let body_directory = TempDir::new().expect("temporary Ready Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x39, 1, None, false);
    let seed = fixture.ledger(vec![fetch]);
    let records = seed
        .authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store)
        .expect("authenticate the durable Fetch origin")
        .project_ready_validate_records_for_test(&fixture.verified)
        .expect("project the exact terminal Fetch/Store to live Validate crash boundary");
    let ledger = fixture.ledger(records);
    let payload_directory = TempDir::new().expect("temporary Ready Validate payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger_directory = TempDir::new().expect("temporary Ready Validate ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal the exact Ready Validate recovery cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("cold-open the exact Ready Validate carrier");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(owner.live_body_pipeline_counts_for_test(), (0, 0, 1));
    assert!(matches!(
        owner.plan_direct_registry_turn(),
        Err(ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal: 3 })
    ));
}

fn assert_cold_opens_standalone_validate(
    fixture: &RecoveryFixture,
    body_store: V2BodyStore,
    record: LifecycleLedgerRecordV1,
) {
    let expected_owner = record.owner();
    let expected_ordinal = record.ordinal();
    let expected_authority = record.replay_authority.clone();
    let ledger = fixture.ledger(vec![record]);
    let payload_directory = TempDir::new().expect("temporary standalone Validate payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger_directory = TempDir::new().expect("temporary standalone Validate ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("authenticate standalone Validate storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("cold-open standalone Validate lifecycle owner");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(owner.live_body_pipeline_counts_for_test(), (0, 0, 1));
    let recovered = &owner.coordinator.records[&expected_ordinal];
    let recovered_metadata = &owner.coordinator.durable_records[&expected_ordinal];
    assert_eq!(recovered.owner, expected_owner);
    assert_eq!(recovered.ordinal, expected_ordinal);
    assert_eq!(recovered_metadata.replay_authority, expected_authority);
    assert!(matches!(
        owner.plan_direct_registry_turn(),
        Err(ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal })
            if ordinal == expected_ordinal
    ));
}

#[test]
fn production_owner_cold_opens_exact_standalone_local_body_validate() {
    let fixture = RecoveryFixture::new("standalone-local-validate-cold-open", 0x51);
    let body_directory = TempDir::new().expect("temporary standalone local Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        0,
        0x61,
        7,
        StandaloneValidateOriginFixture::LocalBody,
    );
    assert_cold_opens_standalone_validate(&fixture, body_store, record);
}

#[test]
fn production_owner_cold_opens_exact_standalone_remote_proposal_validate() {
    let fixture = RecoveryFixture::new("standalone-remote-validate-cold-open", 0x55);
    let body_directory = TempDir::new().expect("temporary standalone remote Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        0,
        0x65,
        9,
        StandaloneValidateOriginFixture::RemoteProposal {
            valid_signature: true,
        },
    );
    assert_cold_opens_standalone_validate(&fixture, body_store, record);
}

#[test]
fn production_owner_cold_opens_refined_standalone_remote_proposal_validate() {
    for (phase, marker, ordinal) in [
        (wire::GlobalPhase::Prepare, 0x66, 10),
        (wire::GlobalPhase::Commit, 0x67, 11),
    ] {
        let fixture = RecoveryFixture::new("standalone-refined-remote-validate-cold-open", 0x56);
        let body_directory =
            TempDir::new().expect("temporary refined standalone remote Validate body store");
        let mut body_store = fixture.open_store(&body_directory);
        let record = standalone_validate_record(
            &fixture,
            &mut body_store,
            0,
            marker,
            ordinal,
            StandaloneValidateOriginFixture::RefinedRemoteProposal {
                phase,
                corrupt_qc: false,
            },
        );
        assert_cold_opens_standalone_validate(&fixture, body_store, record);
    }
}

#[test]
fn refined_standalone_remote_proposal_validate_rejects_an_invalid_qc() {
    let fixture = RecoveryFixture::new("standalone-refined-remote-invalid-qc", 0x57);
    let body_directory =
        TempDir::new().expect("temporary invalid refined remote Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        0,
        0x68,
        12,
        StandaloneValidateOriginFixture::RefinedRemoteProposal {
            phase: wire::GlobalPhase::Commit,
            corrupt_qc: true,
        },
    );
    let ledger = fixture.ledger(vec![record]);
    assert!(matches!(
        ledger.authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store,),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidReplayJoin)
    ));
}

#[test]
fn standalone_validate_cold_census_rejects_a_foreign_body_store() {
    let fixture = RecoveryFixture::new("standalone-validate-foreign-body", 0x59);
    let canonical_directory = TempDir::new().expect("temporary canonical Validate body store");
    let mut canonical_store = fixture.open_store(&canonical_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut canonical_store,
        0,
        0x69,
        11,
        StandaloneValidateOriginFixture::LocalBody,
    );
    let ledger = fixture.ledger(vec![record]);
    let foreign_directory = TempDir::new().expect("temporary foreign Validate body store");
    let foreign_store = fixture.open_store(&foreign_directory);
    assert!(matches!(
        ledger.authenticate_durable_certified_body_pipeline_census(
            &fixture.verified,
            &foreign_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::BodyFrame(_))
    ));
}

#[test]
fn standalone_validate_cold_census_rejects_a_foreign_owner_root() {
    let fixture = RecoveryFixture::new("standalone-validate-foreign-owner", 0x5D);
    let body_directory = TempDir::new().expect("temporary foreign-owner Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        0,
        0x6D,
        13,
        StandaloneValidateOriginFixture::LocalBody,
    );
    let foreign_owner = OwnerId::new(record.owner().causal_root(), 12);
    let continuation = record
        .continuation()
        .expect("standalone Validate continuation");
    let foreign = LifecycleLedgerRecordV1::new(
        record.key().expect("standalone Validate key"),
        foreign_owner,
        record.ordinal(),
        record.work_class().expect("standalone Validate class"),
        record.stage().expect("standalone Validate stage"),
        record
            .terminal()
            .expect("standalone Validate terminal decode"),
        foreign_owner.causal_root().digest(),
        record
            .durable_payload()
            .expect("standalone Validate payload"),
        record.replay_authority,
        continuation,
    )
    .expect("construct structurally valid foreign-owner Validate row");
    let owner_root = unrelated_live_record(
        fixture.lifecycle_context(),
        foreign_owner,
        foreign_owner.first_admission_ordinal(),
        0x7D,
    );
    let ledger = fixture.ledger(vec![owner_root, foreign]);
    assert!(matches!(
        ledger.authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store,),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidLedgerRow)
    ));
}

#[test]
fn standalone_validate_cold_census_rejects_an_unauthenticated_proposal() {
    let fixture = RecoveryFixture::new("standalone-validate-invalid-proposal", 0x61);
    let body_directory = TempDir::new().expect("temporary invalid-Proposal Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        0,
        0x71,
        15,
        StandaloneValidateOriginFixture::RemoteProposal {
            valid_signature: false,
        },
    );
    let ledger = fixture.ledger(vec![record]);
    assert!(matches!(
        ledger.authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store,),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidReplayJoin)
    ));
}

#[test]
fn standalone_validate_cold_census_rejects_a_certified_origin() {
    let fixture = RecoveryFixture::new("standalone-validate-certified-origin", 0x65);
    let as_standalone = |validate: LifecycleLedgerRecordV1| {
        let ordinal = validate.ordinal();
        let standalone_owner = OwnerId::new(validate.owner().causal_root(), ordinal);
        LifecycleLedgerRecordV1::new(
            validate.key().expect("certified Validate key"),
            standalone_owner,
            ordinal,
            LifecycleWorkClass::Validate,
            validate.stage().expect("certified Validate stage"),
            None,
            standalone_owner.causal_root().digest(),
            validate
                .durable_payload()
                .expect("certified Validate body frame"),
            validate.replay_authority,
            DurableContinuation::None,
        )
        .expect("construct standalone certified-origin Validate row")
    };
    let body_directory = TempDir::new().expect("temporary certified-origin Validate body store");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x75, 1, None, false);
    let seed = fixture.ledger(vec![fetch]);
    let mut records = seed
        .authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store)
        .expect("authenticate certified Fetch fixture")
        .project_ready_validate_records_for_test(&fixture.verified)
        .expect("project certified Validate fixture");
    let validate = records.pop().expect("certified fixture has a Validate row");
    let standalone = as_standalone(validate);
    let ledger = fixture.ledger(vec![standalone]);
    assert!(matches!(
        ledger.authenticate_durable_certified_body_pipeline_census(&fixture.verified, &body_store,),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidReplayJoin)
    ));

    let genesis_directory = TempDir::new().expect("temporary authenticated-genesis body store");
    let mut genesis_store = V2BodyStore::open_with_policy(
        genesis_directory.path(),
        fixture.verified.context().clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.keys[0].public_key().clone()),
    )
    .expect("open authenticated-genesis body store");
    let fetch = fixture.fetch_record_with_block_signature(
        &mut genesis_store,
        0,
        0x76,
        1,
        None,
        false,
        Some((0, 0)),
    );
    let seed = fixture.ledger(vec![fetch]);
    let mut records = seed
        .authenticate_durable_certified_body_pipeline_census(&fixture.verified, &genesis_store)
        .expect("authenticate genesis-certified Fetch fixture")
        .project_ready_validate_records_for_test(&fixture.verified)
        .expect("project genesis-certified Validate fixture");
    let validate = records
        .pop()
        .expect("genesis-certified fixture has a Validate row");
    assert_cold_opens_standalone_validate(&fixture, genesis_store, as_standalone(validate));
}

#[test]
fn production_owner_keeps_terminal_validate_and_live_serve_together() {
    let fixture = RecoveryFixture::new("terminal-validate-live-serve-owner", 0x41);
    let body_directory = TempDir::new().expect("temporary coexistence body store");
    let mut body_store = fixture.open_store(&body_directory);
    let terminal_validate = fixture.terminal_validate_record(&mut body_store, 1, 0x51, 3);
    let payload_directory = TempDir::new().expect("temporary coexistence payload store");
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(payload_directory.path(), fixture.verified.context())
            .expect("open coexistence Certified-Serve payload store");
    let request = fixture.authenticated_serve_request(0, 0x52, 3);
    let receipt = payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &request)
        .expect("persist coexistence Certified-Serve request");
    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
        .expect("construct coexistence lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
    assert!(matches!(
        coordinator
            .admit_certified_serve(&fixture.verified, &request, receipt)
            .expect("project coexistence Certified-Serve request"),
        super::super::super::AdmissionDecision::Admitted { .. }
    ));
    let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project coexistence Serve ledger");
    let mut records = serve_ledger.records.clone();
    records.push(terminal_validate);
    let producer_debts = serve_ledger
        .producer_debts
        .iter()
        .map(|debt| (debt.serve_ordinal(), debt.producer_ordinal()))
        .collect();
    let ledger = LifecycleLedgerV1::new(fixture.lifecycle_context(), 3, records, producer_debts)
        .expect("construct terminal-Validate/live-Serve ledger");
    drop(payload_store);
    let (payload_store, recovered_payloads) =
        CertifiedServePayloadStoreV1::open(payload_directory.path(), fixture.verified.context())
            .expect("reopen coexistence Certified-Serve payload store");
    let payloads = recovered_payloads
        .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
        .expect("authenticate coexistence Certified-Serve payload");
    let ledger_directory = TempDir::new().expect("temporary coexistence ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal coexistence storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("open terminal-Validate/live-Serve production owner");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    assert_eq!(owner.live_fetch_count_for_test(), 0);
    assert_eq!(owner.terminal_validate_count_for_test(), 1);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1),
        "live Serve and dormant adjacent ProducerTurn both retain exact carriers",
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .one_certified_serve_pair_shares_replay_family(),
        "startup carriers retain the same whole replay family",
    );
}

#[test]
fn fresh_certified_serve_publishes_exact_ledger_beside_fetch_and_broadcast() {
    run_durable_recovery_test_on_stack(|| {
        let fixture = RecoveryFixture::new("fresh-serve-owner", 0x81);
    let body_directory = TempDir::new().expect("temporary fresh Serve body store");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x82, 1, None, false);
    let payload_directory = TempDir::new().expect("temporary fresh Serve payload store");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(vec![fetch]);
    let ledger_directory = TempDir::new().expect("temporary fresh Serve ledger store");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal fresh Serve storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("open fresh Serve production owner");
    let context = fixture.verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 4,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new([0x82, 0xA1])),
        payload_hash: Hash::new([0x82, 0xA2]),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new([0x82, 0xB1]),
        Hash::new([0x82, 0xB2]),
        Hash::new([0x82, 0xB3]),
        1,
        Hash::new([0x82, 0xB4]),
    );
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(
        fixture.keys[0].private_key(),
        &crate::sumeragi::v2::SignRequest::Vote(vote.clone()).signature_preimage(),
    )
    .payload()
    .to_vec();
    let broadcast = crate::sumeragi::v2::AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(vote),
    ));
    let ownership = crate::sumeragi::v2_runtime::bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&broadcast),
        vec![
            crate::sumeragi::v2_runtime::RuntimeEffectOwnership::fresh_for_test(
                EventTag::new(context.height, round.view, Generation::new(1)),
                0x82,
            ),
        ],
    )
    .expect("bind unrelated live Broadcast")
    .pop()
    .expect("one unrelated live Broadcast owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&broadcast)
        .expect("mint unrelated live Broadcast binding");
    let prepared = owner
        .coordinator
        .prepare_direct_signed_lifecycle_admission(&fixture.verified, broadcast, pending)
        .expect("unrelated live Broadcast has mandatory replay authority");
    assert!(matches!(
        owner
            .coordinator
            .admit_prepared_lifecycle(&mut owner.registry, prepared),
        super::super::super::concrete_admission::AdapterEffectAdmissionTransaction::Admitted(
            super::super::super::AdmissionDecision::Admitted { ordinal: 2, .. }
        )
    ));
    assert!(
        owner
            .registry
            .registry_mut()
            .exactly_covers_all_live_work(&fixture.verified, &owner.coordinator)
    );
    let request = fixture.authenticated_serve_request(1, 0x83, 3);
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );

    let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(matches!(
        outcome.decision(),
        Some(super::super::super::AdmissionDecision::Admitted {
            ordinal: 3,
            producer_turn_ordinal: Some(4),
            ..
        })
    ));
    assert!(!outcome.restart_required());
    let Ok(continuation) = outcome.into_safe_continuation() else {
        panic!("published fresh Serve must return its safe selector continuation")
    };
    assert!(continuation.failure().is_none());
    assert!(
        continuation
            .into_target()
            .matches_certified_serve_request(request.request_hash())
    );
    assert_eq!(owner.live_fetch_count_for_test(), 1);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .one_certified_serve_pair_shares_replay_family()
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .exactly_covers_all_live_work(&fixture.verified, &owner.coordinator)
    );
    let store = owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("fresh owner retains LedgerV1 store");
    assert_eq!(
        store.load().expect("reload fresh Serve LedgerV1"),
        LifecycleLedgerV1::from_coordinator(&owner.coordinator)
            .expect("project fresh Serve coordinator")
    );

    let retry_target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        2,
    );
    let retry = owner.admit_selected_certified_serve(retry_target, &fixture.keys[0], &request);
    assert!(matches!(
        retry.decision(),
        Some(super::super::super::AdmissionDecision::Retry { ordinal: 3, .. })
    ));
    assert!(retry.into_safe_continuation().is_ok());
        assert_eq!(owner.live_fetch_count_for_test(), 1);
        assert_eq!(
            owner.certified_serve_and_producer_carrier_counts_for_test(),
            (1, 1),
            "idempotent retry must preserve the unrelated Fetch and exact shared pair"
        );
    });
}

#[test]
fn terminal_owner_publishes_completed_and_reopens_exact_producer_carrier() {
    let fixture = RecoveryFixture::new("terminal-owner-completed", 0x85);
    let body_directory = TempDir::new().expect("temporary completed-owner body store");
    let payload_directory = TempDir::new().expect("temporary completed-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary completed-owner ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let serve_ordinal = lease.ordinal();
    let producer_ordinal = serve_ordinal + 1;

    owner
        .settle_certified_serve_completed(lease, &request, &durable_body, &response)
        .expect("owner publishes exact completed Serve terminal");

    let response_digest =
        LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
    assert_eq!(
        owner.coordinator.records[&serve_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert_eq!(
        owner.coordinator.records[&producer_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(owner.coordinator.active_lease, None);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .exactly_covers_recovered_ready_work(&owner.coordinator)
    );
    let on_disk = owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("completed owner retains LedgerV1 store")
        .load()
        .expect("reload completed owner LedgerV1");
    assert_eq!(
        on_disk,
        LifecycleLedgerV1::from_coordinator(&owner.coordinator)
            .expect("project completed owner coordinator")
    );
    drop(owner);

    let body_store = fixture.open_store(&body_directory);
    let (payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(payload_directory.path(), fixture.verified.context())
            .expect("reopen completed-owner payload store");
    let payloads = recovered
        .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
        .expect("authenticate completed-owner payloads");
    let (ledger_store, ledger) =
        LifecycleLedgerStoreV1::open(ledger_directory.path(), fixture.lifecycle_context())
            .expect("reopen completed-owner LedgerV1");
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal completed-owner restart cut");
    let mut reopened = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("reopen completed production owner");
    assert_eq!(
        reopened.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
    assert_eq!(
        reopened.coordinator.records[&serve_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert!(
        reopened
            .registry
            .registry_mut()
            .exactly_covers_recovered_ready_work(&reopened.coordinator)
    );
}

#[test]
fn parked_recovered_broadcast_allows_exact_producer_claim() {
    let fixture = RecoveryFixture::new("parked-broadcast-producer-claim", 0x89);
    let directory = TempDir::new().expect("temporary parked-Broadcast owner storage");
    let (mut owner, broadcast_ordinal, paired_ordinal, unrelated_ordinal) =
        ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
            fixture.verified.clone(),
            &fixture.keys[0],
            directory.path(),
        );
    let second_broadcast_ordinal = owner
        .add_recovered_broadcast_scheduler_fixture_for_test(&fixture.keys[0], 0xD9)
        .expect("add a second exact unpaired recovered Broadcast");
    let (request, durable_body, response) = fixture.completed_serve_exchange(
        owner
            .body_store
            .as_mut()
            .expect("recovered owner retains its exact body store"),
        7,
        0xC9,
        3,
    );

    assert!(owner.park_recovered_broadcast_for_census_test(broadcast_ordinal));
    assert!(owner.park_recovered_broadcast_for_census_test(second_broadcast_ordinal));
    assert!(owner.retire_unrelated_sign_for_finalization_test(paired_ordinal));
    assert!(owner.retire_unrelated_sign_for_finalization_test(unrelated_ordinal));
    assert!(
        owner
            .registry
            .registry_mut()
            .exactly_covers_all_live_work(&fixture.verified, &owner.coordinator),
        "the terminal paired-Sign lineage must not block fresh Serve admission"
    );

    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let admitted = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(matches!(
        admitted.decision(),
        Some(super::super::super::AdmissionDecision::Admitted { .. })
    ));
    let serve_ordinal = *owner
        .coordinator
        .ready_index
        .iter()
        .find(|ordinal| {
            owner.coordinator.records[ordinal].work_class == LifecycleWorkClass::CertifiedServe
        })
        .expect("fresh Serve owns the sole Ready service row");
    let serve_record = &owner.coordinator.records[&serve_ordinal];
    let TurnPlan::Execute(serve_lease) = owner.coordinator.plan_turn(
        super::super::super::SchedulerInputs::new(
            [],
            [(
                serve_ordinal,
                super::super::super::SchedulerReadyInputs::new(serve_record, None, [0; 6]),
            )],
        )
        .expect("one exact Ready Serve scheduler row"),
    ) else {
        panic!("fresh Serve must acquire its exact lifecycle lease")
    };
    assert_eq!(serve_lease.work_class(), LifecycleWorkClass::CertifiedServe);
    owner
        .settle_certified_serve_completed(serve_lease, &request, &durable_body, &response)
        .expect("publish exact completed Serve beside parked Broadcast");

    let producer_ordinal = serve_ordinal
        .checked_add(1)
        .expect("adjacent ProducerTurn ordinal remains representable");
    let ledger = LifecycleLedgerV1::from_coordinator(&owner.coordinator)
        .expect("project exact pre-Producer ledger");
    let attestation = owner
        .registry
        .registry_mut()
        .attest_ready_producer_turn_census(&fixture.verified, &owner.coordinator, &ledger)
        .expect("attest complete ProducerTurn census")
        .expect("completed Serve exposes one Ready ProducerTurn");
    let producer_record = &owner.coordinator.records[&producer_ordinal];
    let TurnPlan::Execute(producer_lease) = owner.coordinator.plan_turn(
        super::super::super::SchedulerInputs::new(
            [],
            [(
                producer_ordinal,
                super::super::super::SchedulerReadyInputs::new(producer_record, None, [0; 6]),
            )],
        )
        .expect("one exact Ready Producer scheduler row"),
    ) else {
        panic!("completed Serve must release its adjacent ProducerTurn")
    };
    assert_eq!(
        producer_lease.work_class(),
        LifecycleWorkClass::ProducerTurn
    );
    let claimed = owner
        .registry
        .registry_mut()
        .project_claimed_producer_turn(
            &fixture.verified,
            &owner.coordinator,
            &ledger,
            producer_lease,
            attestation,
        )
        .expect("parked Broadcast permits the exact active-Producer census");
    assert_eq!(claimed.lease().ordinal(), producer_ordinal);
}

#[test]
fn launched_terminal_owner_settles_exact_worker_body_readback() {
    let fixture = RecoveryFixture::new("terminal-owner-worker-readback", 0x86);
    let body_directory = TempDir::new().expect("temporary worker-readback body store");
    let payload_directory = TempDir::new().expect("temporary worker-readback payload store");
    let ledger_directory = TempDir::new().expect("temporary worker-readback ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let worker_body_store = move_body_store_to_test_worker(&mut owner);
    assert!(owner.body_store.is_none());
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let serve_ordinal = lease.ordinal();
    let producer_ordinal = serve_ordinal + 1;

    let mismatched_readback = worker_body_store
        .read_durable_body_for_certified_serve(&durable_body)
        .expect("worker reloads exact durable body before response construction");
    let mut mismatched_response = response.clone();
    mismatched_response.body[0] ^= 0x80;
    let error = owner
        .settle_certified_serve_worker_completed(
            lease,
            &request,
            mismatched_readback,
            &mismatched_response,
        )
        .expect_err("response bytes cannot diverge from the exact worker readback");
    assert!(!error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
    );
    let lease = error
        .into_lease()
        .expect("body mismatch is rejected before terminal publication");
    assert!(matches!(
        owner.coordinator.records[&serve_ordinal].state,
        LifecycleState::Claimed(_)
    ));

    let exact_readback = worker_body_store
        .read_durable_body_for_certified_serve(&durable_body)
        .expect("worker reloads an exact completion readback");
    owner
        .settle_certified_serve_worker_completed(lease, &request, exact_readback, &response)
        .expect("launched owner consumes exact worker readback without owning the body store");

    let response_digest =
        LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
    assert_eq!(
        owner.coordinator.records[&serve_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert_eq!(
        owner.coordinator.records[&producer_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(owner.coordinator.active_lease, None);
}
#[test]
fn launched_terminal_owner_rejects_foreign_worker_store_instance() {
    let fixture = RecoveryFixture::new("terminal-owner-foreign-worker-store", 0x87);
    let body_directory = TempDir::new().expect("temporary canonical worker body store");
    let foreign_body_directory = TempDir::new().expect("temporary foreign worker body store");
    let payload_directory = TempDir::new().expect("temporary foreign-worker payload store");
    let ledger_directory = TempDir::new().expect("temporary foreign-worker ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let worker_body_store = move_body_store_to_test_worker(&mut owner);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let active_lease = lease.clone();
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    let pending_payloads = snapshot_files(payload_directory.path());

    let mut foreign_body_store = fixture.open_store(&foreign_body_directory);
    let foreign_receipt = foreign_body_store
        .store(response.manifest.clone(), response.body.clone())
        .expect("foreign store persists byte-identical response body");
    let foreign_readback = foreign_body_store
        .read_durable_body_for_certified_serve(&foreign_receipt)
        .expect("foreign store can mint only its own exact readback");
    let error = owner
        .settle_certified_serve_worker_completed(lease, &request, foreign_readback, &response)
        .expect_err("byte-identical foreign store cannot settle this launched owner");
    assert!(error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(active_lease));
    assert_eq!(snapshot_files(payload_directory.path()), pending_payloads);
    assert!(owner.body_store.is_none());
    assert!(
        worker_body_store.instance_identity().same_instance(
            owner
                .body_store_identity
                .as_ref()
                .expect("retained store seal")
        )
    );
    assert!(
        !foreign_body_store.instance_identity().same_instance(
            owner
                .body_store_identity
                .as_ref()
                .expect("retained store seal")
        )
    );
    assert_eq!(durable_body.subject(), response.manifest.subject);
}
#[test]
fn terminal_owner_publishes_rejected_failed_and_cancelled_carrier_shapes() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    for (index, outcome) in [
        CertifiedServePayloadNegativeOutcome::Rejected(37),
        CertifiedServePayloadNegativeOutcome::Failed(41),
        CertifiedServePayloadNegativeOutcome::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = RecoveryFixture::new(
            &format!("terminal-owner-negative-{index}"),
            0x89 + u8::try_from(index).expect("small terminal fixture index") * 4,
        );
        let body_directory = TempDir::new().expect("temporary negative-owner body store");
        let payload_directory = TempDir::new().expect("temporary negative-owner payload store");
        let ledger_directory = TempDir::new().expect("temporary negative-owner ledger store");
        let mut owner =
            fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
        let request = fixture.authenticated_serve_request(0, 0x90, 3);
        let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
        let serve_ordinal = lease.ordinal();
        let producer_ordinal = serve_ordinal + 1;
        owner
            .settle_certified_serve_negative(lease, &request, outcome)
            .expect("owner publishes exact negative Serve terminal");
        let expected = match outcome {
            CertifiedServePayloadNegativeOutcome::Rejected(code) => TerminalOutcome::Rejected(code),
            CertifiedServePayloadNegativeOutcome::Failed(code) => TerminalOutcome::Failed(code),
            CertifiedServePayloadNegativeOutcome::Cancelled => TerminalOutcome::Cancelled,
        };
        assert_eq!(
            owner.coordinator.records[&serve_ordinal].state,
            LifecycleState::Terminal(expected)
        );
        let cancelled = outcome == CertifiedServePayloadNegativeOutcome::Cancelled;
        assert_eq!(
            owner.coordinator.records[&producer_ordinal].state,
            if cancelled {
                LifecycleState::Terminal(TerminalOutcome::Cancelled)
            } else {
                LifecycleState::Ready
            }
        );
        assert_eq!(
            owner.certified_serve_and_producer_carrier_counts_for_test(),
            if cancelled { (0, 0) } else { (0, 1) }
        );
        assert_eq!(
            owner.coordinator.producer_debts.get(&serve_ordinal),
            (!cancelled).then_some(&producer_ordinal)
        );
        assert!(
            owner
                .registry
                .registry_mut()
                .exactly_covers_recovered_ready_work(&owner.coordinator)
        );
    }
}
#[test]
fn terminal_owner_returns_foreign_request_and_body_before_publication() {
    let fixture = RecoveryFixture::new("terminal-owner-input-rejection", 0x99);
    let body_directory = TempDir::new().expect("temporary input-owner body store");
    let payload_directory = TempDir::new().expect("temporary input-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary input-owner ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    let payloads = snapshot_files(payload_directory.path());
    let foreign = fixture.authenticated_serve_request(1, 0x9A, 3);
    let mut foreign_lease = lease.clone();
    foreign_lease.ordinal = foreign_lease
        .ordinal
        .checked_add(2)
        .expect("small foreign lease ordinal");
    let error = owner
        .settle_certified_serve_completed(foreign_lease, &request, &durable_body, &response)
        .expect_err("foreign lease is rejected before terminal persistence");
    assert!(!error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::Coordinator
    );
    assert!(error.into_lease().is_ok());
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
    assert_eq!(owner.coordinator.fault(), None);
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    let error = owner
        .settle_certified_serve_completed(lease, &foreign, &durable_body, &response)
        .expect_err("foreign request is rejected before terminal persistence");
    assert!(!error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::RequestAuthority
    );
    let lease = error
        .into_lease()
        .expect("prepublication rejection returns the exact active lease");
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
    assert_eq!(owner.coordinator.fault(), None);
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
    let foreign_receipt = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
        fixture.verified.context().id(),
        response.manifest.round,
        response.manifest.subject,
        iroha_crypto::HashOf::new(&response.manifest),
    );
    let error = owner
        .settle_certified_serve_completed(lease, &request, &foreign_receipt, &response)
        .expect_err("foreign durable receipt is rejected before terminal persistence");
    assert!(!error.restart_required());
    let lease = error
        .into_lease()
        .expect("foreign body receipt returns the exact active lease");
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
    assert_eq!(owner.coordinator.fault(), None);
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
    let mut foreign_body = response.clone();
    foreign_body.body.push(0);
    let error = owner
        .settle_certified_serve_completed(lease, &request, &durable_body, &foreign_body)
        .expect_err("foreign response body is rejected before terminal persistence");
    assert!(!error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
    );
    let lease = error
        .into_lease()
        .expect("foreign response body returns the exact active lease");
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
    assert_eq!(owner.coordinator.fault(), None);
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    let retained_body_store = owner
        .body_store
        .take()
        .expect("unlaunched owner still retains its exact body store");
    let error = owner
        .settle_certified_serve_completed(lease, &request, &durable_body, &response)
        .expect_err("completion without the retained body store is prepublication-safe");
    assert!(!error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable
    );
    let lease = error
        .into_lease()
        .expect("unavailable body store returns the exact active lease");
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(lease));
    assert_eq!(owner.coordinator.fault(), None);
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    drop(retained_body_store);
}
#[test]
fn terminal_owner_faults_on_corrupt_owned_body_after_receipt_mint() {
    let fixture = RecoveryFixture::new("terminal-owner-owned-body-corruption", 0x9B);
    let body_directory = TempDir::new().expect("temporary corrupt-owner body store");
    let payload_directory = TempDir::new().expect("temporary corrupt-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary corrupt-owner ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let active_lease = lease.clone();
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    let pending_payloads = snapshot_files(payload_directory.path());
    let ledger = owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("terminal owner retains LedgerV1 store")
        .load()
        .expect("load pre-corruption LedgerV1");
    owner
        .body_store
        .as_ref()
        .expect("unlaunched owner retains its exact body store")
        .corrupt_owned_frame_for_test(&durable_body)
        .expect("replace the already-accepted body frame");
    let error = owner
        .settle_certified_serve_completed(lease, &request, &durable_body, &response)
        .expect_err("reload corruption after receipt ownership requires restart");
    assert!(error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
    );
    assert!(
        error.into_lease().is_err(),
        "accepted-store corruption must not release a safe retry lease"
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(active_lease));
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
    assert_eq!(snapshot_files(payload_directory.path()), pending_payloads);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .one_certified_serve_pair_shares_replay_family()
    );
    assert_eq!(
        owner
            .coordinator
            .ledger_store
            .as_ref()
            .expect("faulted owner retains LedgerV1 store")
            .load()
            .expect("reload unchanged LedgerV1"),
        ledger
    );
}
#[test]
fn terminal_registry_rejects_every_arbitrary_staged_drift_before_callback() {
    for (index, drift) in [
        StagedTerminalDrift::Record,
        StagedTerminalDrift::Index,
        StagedTerminalDrift::Debt,
        StagedTerminalDrift::Capacity,
        StagedTerminalDrift::HighWater,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = RecoveryFixture::new(
            &format!("terminal-staged-drift-{index}"),
            0xB0 + u8::try_from(index).expect("small drift index") * 4,
        );
        let body_directory = TempDir::new().expect("temporary staged-drift body store");
        let payload_directory = TempDir::new().expect("temporary staged-drift payload store");
        let ledger_directory = TempDir::new().expect("temporary staged-drift ledger store");
        let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
            &body_directory,
            &payload_directory,
            &ledger_directory,
        );
        let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
        let serve_ordinal = lease.ordinal();
        let producer_ordinal = owner.coordinator.producer_debts[&serve_ordinal];
        let receipt = owner
            .payload_store
            .persist_completed_with_exact_body(
                &request,
                &durable_body,
                owner
                    .body_store
                    .as_ref()
                    .expect("unlaunched owner retains body store"),
                &response,
            )
            .expect("persist terminal receipt for staged-drift preflight");
        let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
            owner.coordinator.active_context,
            &owner.coordinator.records[&serve_ordinal],
            &owner.coordinator.durable_records[&serve_ordinal],
            &owner.coordinator.records[&producer_ordinal],
            &owner.coordinator.durable_records[&producer_ordinal],
            receipt,
        )
        .expect("seal exact terminal replay pair");
        let transition = owner
            .registry
            .registry_mut()
            .prepare_certified_serve_terminal_transition(
                &fixture.verified,
                &owner.coordinator,
                &lease,
                &request,
                &terminal,
            )
            .expect("prepare exact terminal registry transition");
        let outcome = terminal.terminal_outcome();
        let mut staged = owner.coordinator.stage_durable_transaction();
        staged.reduce_settle_turn(
            lease.clone(),
            super::super::super::TurnOutcome::Terminal(outcome),
            Some(terminal),
        );
        assert_eq!(staged.fault(), None);
        match drift {
            StagedTerminalDrift::Record => {
                let mut extra = staged.records[&producer_ordinal].clone();
                extra.ordinal = u128::MAX - 1;
                assert!(staged.records.insert(extra.ordinal, extra).is_none());
            }
            StagedTerminalDrift::Index => {
                let key = staged.records[&serve_ordinal].key;
                assert!(staged.key_index.remove(&key).is_some());
            }
            StagedTerminalDrift::Debt => {
                assert!(staged.producer_debts.remove(&serve_ordinal).is_some());
            }
            StagedTerminalDrift::Capacity => {
                *staged
                    .capacity_used
                    .get_mut(&super::super::super::CapacityClass::Effect)
                    .expect("effect capacity counter exists") += 1;
            }
            StagedTerminalDrift::HighWater => {
                staged.high_water = staged
                    .high_water
                    .checked_add(1)
                    .expect("fixture high-water has room");
            }
        }
        let records = owner.coordinator.records.clone();
        let durable_records = owner.coordinator.durable_records.clone();
        let mut callback_invoked = false;
        let result = owner
            .registry
            .registry_mut()
            .publish_certified_serve_terminal_transition(
                transition,
                &fixture.verified,
                &owner.coordinator,
                &staged,
                &lease,
                || {
                    callback_invoked = true;
                    Ok::<(), ()>(())
                },
            );
        assert!(matches!(
                    result,
                    Err(
                        super::super::super::work_registry::CertifiedServeTerminalRegistryPublicationError::Preflight(
                            _
                        )
                    )
                ));
        assert!(!callback_invoked);
        assert_eq!(owner.coordinator.records, records);
        assert_eq!(owner.coordinator.durable_records, durable_records);
        assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
        assert_eq!(
            owner.certified_serve_and_producer_carrier_counts_for_test(),
            (1, 1)
        );
        assert!(
            owner
                .registry
                .registry_mut()
                .one_certified_serve_pair_shares_replay_family()
        );
        assert!(
            owner
                .registry
                .registry_mut()
                .preflight_certified_serve_terminal_owner_state(
                    &fixture.verified,
                    &owner.coordinator,
                    &lease,
                )
        );
    }
}
#[test]
fn terminal_owner_registry_mismatch_faults_before_payload_persistence() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    let fixture = RecoveryFixture::new("terminal-owner-registry-mismatch", 0x9D);
    let body_directory = TempDir::new().expect("temporary registry-owner body store");
    let payload_directory = TempDir::new().expect("temporary registry-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary registry-owner ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let request = fixture.authenticated_serve_request(0, 0x9E, 3);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let active_lease = lease.clone();
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    assert!(
        owner
            .registry
            .registry_mut()
            .remove_one_certified_serve_carrier_for_test()
    );
    let payloads = snapshot_files(payload_directory.path());
    let error = owner
        .settle_certified_serve_negative(
            lease,
            &request,
            CertifiedServePayloadNegativeOutcome::Rejected(43),
        )
        .expect_err("private registry mismatch requires restart");
    assert!(error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::Registry
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(active_lease));
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
    assert_eq!(snapshot_files(payload_directory.path()), payloads);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1),
        "terminal preflight must not mutate the already-mismatched registry"
    );
}
#[test]
fn terminal_owner_ledger_drift_restores_both_current_carriers() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    let fixture = RecoveryFixture::new("terminal-owner-ledger-drift", 0xA1);
    let body_directory = TempDir::new().expect("temporary drift-owner body store");
    let payload_directory = TempDir::new().expect("temporary drift-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary drift-owner ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let request = fixture.authenticated_serve_request(0, 0xA2, 3);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let active_lease = lease.clone();
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("terminal owner retains LedgerV1 store")
        .persist(&fixture.ledger(Vec::new()))
        .expect("drift the on-disk LedgerV1 before terminal publication");
    let pending_payloads = snapshot_files(payload_directory.path());
    let error = owner
        .settle_certified_serve_negative(
            lease,
            &request,
            CertifiedServePayloadNegativeOutcome::Failed(47),
        )
        .expect_err("exact LedgerV1 drift rejects terminal successor");
    assert!(error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::Ledger
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(active_lease));
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1),
        "Ledger failure restores the byte-for-byte current Serve/Producer pair"
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .one_certified_serve_pair_shares_replay_family()
    );
    assert_ne!(
        snapshot_files(payload_directory.path()),
        pending_payloads,
        "the fsynced terminal payload remains as a startup reconciliation tail"
    );
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
}
#[test]
fn terminal_owner_postrename_sync_failure_keeps_logical_and_registry_state() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    let fixture = RecoveryFixture::new("terminal-owner-postrename", 0xA5);
    let body_directory = TempDir::new().expect("temporary postrename-owner body store");
    let payload_directory = TempDir::new().expect("temporary postrename-owner payload store");
    let ledger_directory = TempDir::new().expect("temporary postrename-owner ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let request = fixture.authenticated_serve_request(0, 0xA6, 3);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    let active_lease = lease.clone();
    let records = owner.coordinator.records.clone();
    let durable_records = owner.coordinator.durable_records.clone();
    let pending_payloads = snapshot_files(payload_directory.path());
    owner
        .payload_store
        .fail_next_publish_directory_sync_for_test();
    let error = owner
        .settle_certified_serve_negative(
            lease,
            &request,
            CertifiedServePayloadNegativeOutcome::Rejected(53),
        )
        .expect_err("post-rename sync ambiguity requires restart");
    assert!(error.restart_required());
    assert_eq!(
        error.failure(),
        super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(owner.coordinator.durable_records, durable_records);
    assert_eq!(owner.coordinator.active_lease, Some(active_lease));
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
    assert_ne!(
        snapshot_files(payload_directory.path()),
        pending_payloads,
        "ambiguous renamed terminal frame remains for startup"
    );
}
#[test]
fn fresh_certified_serve_rejects_foreign_target_and_rolls_back_capacity_wait() {
    let fixture = RecoveryFixture::new("fresh-serve-preledger", 0x91);
    let body_directory = TempDir::new().expect("temporary preledger body store");
    let payload_directory = TempDir::new().expect("temporary preledger payload store");
    let ledger_directory = TempDir::new().expect("temporary preledger ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let request = fixture.authenticated_serve_request(0, 0x92, 3);
    let foreign = fixture.authenticated_serve_request(1, 0x93, 3);
    let foreign_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            foreign.request_hash(),
            1,
        );
    let payload_before = snapshot_files(payload_directory.path());
    let foreign_outcome =
        owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
    assert_eq!(
                foreign_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::SelectorAuthority
                )
            );
    let Ok(foreign_continuation) = foreign_outcome.into_safe_continuation() else {
        panic!("foreign target rejection is a safe pre-persistence continuation")
    };
    let recovered_foreign_target = foreign_continuation.into_target();
    assert!(recovered_foreign_target.matches_certified_serve_request(foreign.request_hash()));
    assert!(!recovered_foreign_target.matches_certified_serve_request(request.request_hash()));
    assert_eq!(snapshot_files(payload_directory.path()), payload_before);
    let admitted_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            request.request_hash(),
            2,
        );
    assert!(
        owner
            .admit_selected_certified_serve(admitted_target, &fixture.keys[0], &request)
            .into_safe_continuation()
            .is_ok()
    );
    let payload_after_first = snapshot_files(payload_directory.path());
    let waiting = fixture.authenticated_serve_request(2, 0x94, 3);
    let waiting_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            waiting.request_hash(),
            3,
        );
    let waiting_outcome =
        owner.admit_selected_certified_serve(waiting_target, &fixture.keys[0], &waiting);
    assert!(matches!(
        waiting_outcome.decision(),
        Some(super::super::super::AdmissionDecision::WaitForCapacity(_))
    ));
    assert_eq!(
        waiting_outcome.failure(),
        Some(
            super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
        )
    );
    let Ok(waiting_continuation) = waiting_outcome.into_safe_continuation() else {
        panic!("proven Pending rollback must release the selector continuation")
    };
    assert!(matches!(
        waiting_continuation.decision(),
        Some(super::super::super::AdmissionDecision::WaitForCapacity(_))
    ));
    assert!(
        waiting_continuation
            .into_target()
            .matches_certified_serve_request(waiting.request_hash())
    );
    assert_eq!(
        snapshot_files(payload_directory.path()),
        payload_after_first,
        "a proven pre-ledger capacity decline must synchronously remove only its fresh Pending frame"
    );
    assert_eq!(owner.coordinator.records.len(), 2);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (1, 1)
    );
}
#[test]
fn fresh_certified_serve_postledger_failure_retains_tail_and_requires_restart() {
    let fixture = RecoveryFixture::new("fresh-serve-restart", 0xA1);
    let body_directory = TempDir::new().expect("temporary restart body store");
    let payload_directory = TempDir::new().expect("temporary restart payload store");
    let ledger_directory = TempDir::new().expect("temporary restart ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let changed =
        LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
            .expect("construct changed pre-publication LedgerV1");
    owner
        .coordinator
        .ledger_store
        .as_ref()
        .expect("fresh owner retains LedgerV1 store")
        .persist(&changed)
        .expect("replace LedgerV1 before exact successor publication");
    let request = fixture.authenticated_serve_request(0, 0xA2, 3);
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(outcome.restart_required());
    assert_eq!(
        outcome.failure(),
        Some(super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Ledger)
    );
    let Err(retained) = outcome.into_safe_continuation() else {
        panic!("post-ledger failure must not release the selector target")
    };
    assert!(retained.restart_required());
    drop(retained);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 0),
        "failed LedgerV1 publication rolls back both staged registry carriers"
    );
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
    assert_ne!(
        snapshot_files(payload_directory.path()),
        BTreeMap::new(),
        "the authenticated post-fsync payload tail remains for restart recovery"
    );
    let reentry = fixture.authenticated_serve_request(1, 0xA3, 3);
    let reentry_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            reentry.request_hash(),
            2,
        );
    let payload_before_reentry = snapshot_files(payload_directory.path());
    let reentry_outcome =
        owner.admit_selected_certified_serve(reentry_target, &fixture.keys[0], &reentry);
    assert!(reentry_outcome.restart_required());
    assert_eq!(
        reentry_outcome.failure(),
        Some(
            super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
        )
    );
    assert!(reentry_outcome.into_safe_continuation().is_err());
    assert_eq!(
        snapshot_files(payload_directory.path()),
        payload_before_reentry,
        "a faulted owner must retain the new selector target without touching payload storage"
    );
}
#[test]
fn fresh_certified_serve_postrename_sync_failure_requires_restart() {
    let fixture = RecoveryFixture::new("fresh-serve-postrename-sync", 0xA5);
    let body_directory = TempDir::new().expect("temporary post-rename body store");
    let payload_directory = TempDir::new().expect("temporary post-rename payload store");
    let ledger_directory = TempDir::new().expect("temporary post-rename ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    owner
        .payload_store
        .fail_next_publish_directory_sync_for_test();
    let request = fixture.authenticated_serve_request(0, 0xA6, 3);
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(outcome.restart_required());
    assert_eq!(
        outcome.failure(),
        Some(
            super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::PayloadStore
        )
    );
    assert!(outcome.into_safe_continuation().is_err());
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
    assert!(owner.coordinator.records.is_empty());
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 0)
    );
    assert_ne!(
        snapshot_files(payload_directory.path()),
        BTreeMap::new(),
        "the renamed frame is an opaque crash tail, never a retryable unchanged attempt"
    );
}
#[test]
fn ledgerless_owner_requires_restart_before_selector_validation() {
    let fixture = RecoveryFixture::new("ledgerless-serve-owner", 0xA9);
    let body_directory = TempDir::new().expect("temporary ledgerless body store");
    let payload_directory = TempDir::new().expect("temporary ledgerless payload store");
    let ledger_directory = TempDir::new().expect("temporary ledgerless ledger store");
    let mut owner =
        fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
    let request = fixture.authenticated_serve_request(0, 0xAA, 3);
    let foreign = fixture.authenticated_serve_request(1, 0xAB, 3);
    let foreign_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            foreign.request_hash(),
            1,
        );
    let _detached_store = owner
        .coordinator
        .ledger_store
        .take()
        .expect("fresh owner starts with its exact LedgerV1 store");
    let outcome = owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
    assert!(outcome.restart_required());
    assert_eq!(
        outcome.failure(),
        Some(
            super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
        )
    );
    assert!(outcome.into_safe_continuation().is_err());
    assert_eq!(snapshot_files(payload_directory.path()), BTreeMap::new());
}
#[test]
fn completed_certified_serve_tombstone_replays_without_a_serve_carrier() {
    let fixture = RecoveryFixture::new("completed-serve-replay", 0xB1);
    let body_directory = TempDir::new().expect("temporary completed body store");
    let payload_directory = TempDir::new().expect("temporary completed payload store");
    let ledger_directory = TempDir::new().expect("temporary completed ledger store");
    let (mut owner, request) = fixture.open_terminal_serve_owner(
        &body_directory,
        &payload_directory,
        &ledger_directory,
        ServeTerminalFixture::Completed,
    );
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(matches!(
        outcome.decision(),
        Some(super::super::super::AdmissionDecision::ReplayTerminal {
            outcome: TerminalOutcome::Completed(Some(_)),
            ..
        })
    ));
    let continuation = outcome
        .into_safe_continuation()
        .unwrap_or_else(|_| panic!("completed tombstone retains a safe replay continuation"));
    let (_target, terminal_replay) = continuation.into_target_and_terminal_replay();
    let terminal_replay = terminal_replay
        .expect("completed tombstone must retain its move-only response replay authority");
    assert!(terminal_replay.authorizes_request(&request));
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
    let foreign_retainer_target =
        super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            request.request_hash(),
            2,
        );
    let foreign_retainer =
        owner.admit_selected_certified_serve(foreign_retainer_target, &fixture.keys[1], &request);
    assert!(foreign_retainer.restart_required());
    assert_eq!(
        foreign_retainer.failure(),
        Some(
            super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
        )
    );
    assert!(foreign_retainer.into_safe_continuation().is_err());
    assert_eq!(
        owner.coordinator.fault(),
        Some(super::super::super::CoordinatorFault::DurabilityFailure)
    );
}
#[test]
fn completed_certified_serve_replay_requires_exact_worker_readback() {
    let fixture = RecoveryFixture::new("completed-serve-worker-replay", 0xB3);
    let body_directory = TempDir::new().expect("temporary replay body store");
    let payload_directory = TempDir::new().expect("temporary replay payload store");
    let ledger_directory = TempDir::new().expect("temporary replay ledger store");
    let (mut owner, request, durable_body, response) =
        fixture.open_completed_serve_owner(&body_directory, &payload_directory, &ledger_directory);
    let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
    owner
        .settle_certified_serve_completed(lease, &request, &durable_body, &response)
        .expect("publish exact completed Serve tombstone");
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let replay = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(matches!(
        replay.decision(),
        Some(super::super::super::AdmissionDecision::ReplayTerminal { .. })
    ));
    let continuation = replay
        .into_safe_continuation()
        .unwrap_or_else(|_| panic!("completed tombstone retains a safe worker replay"));
    let (_target, authorization) = continuation.into_target_and_terminal_replay();
    let authorization = authorization.expect("completed tombstone seals response replay");
    let body_store = move_body_store_to_test_worker(&mut owner);
    let readback = body_store
        .read_durable_body_for_certified_serve(&durable_body)
        .expect("worker rereads exact durable Serve body");
    owner
        .verify_certified_serve_terminal_replay(authorization, &request, readback, &response)
        .expect("exact worker readback reproduces terminal response authority");
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
}
#[test]
fn payload_store_ahead_terminal_startup_installs_only_the_live_producer() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    let fixture = RecoveryFixture::new("store-ahead-serve-replay", 0xB5);
    let body_directory = TempDir::new().expect("temporary store-ahead body store");
    let payload_directory = TempDir::new().expect("temporary store-ahead payload store");
    let ledger_directory = TempDir::new().expect("temporary store-ahead ledger store");
    let body_store = fixture.open_store(&body_directory);
    let request = fixture.authenticated_serve_request(0, 0xD3, 3);
    let (mut payload_store, recovery) =
        CertifiedServePayloadStoreV1::open(payload_directory.path(), fixture.verified.context())
            .expect("open store-ahead Serve payload store");
    assert!(recovery.is_empty());
    let pending = payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &request)
        .expect("persist store-ahead Pending frame");
    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
        .expect("construct store-ahead lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
    assert!(matches!(
        coordinator
            .admit_certified_serve(&fixture.verified, &request, pending)
            .expect("project store-ahead Serve request"),
        super::super::super::AdmissionDecision::Admitted { .. }
    ));
    let ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project Pending store-ahead LedgerV1");
    let _ = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Rejected(29),
        )
        .expect("persist store-ahead negative tombstone");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    drop(payload_store);
    let (payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(payload_directory.path(), fixture.verified.context())
            .expect("reopen store-ahead Serve payload store");
    let payloads = recovered
        .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
        .expect("authenticate store-ahead Serve payload");
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal store-ahead storage cut");
    let mut owner = cut
        .open_owner_for_test(payload_store, payloads)
        .expect("open store-ahead production owner");
    assert_eq!(
        owner.coordinator.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Rejected(29))
    );
    assert!(
        !owner.coordinator.records[&1].physical_slots.is_empty(),
        "store-ahead settlement retains non-executable former Pending geometry"
    );
    assert_eq!(owner.coordinator.records[&2].state, LifecycleState::Ready);
    assert_eq!(
        owner.certified_serve_and_producer_carrier_counts_for_test(),
        (0, 1)
    );
    assert!(
        owner
            .registry
            .registry_mut()
            .exactly_covers_recovered_ready_work(&owner.coordinator)
    );
    let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
        fixture.verified.context(),
        request.request_hash(),
        1,
    );
    let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
    assert!(matches!(
        outcome.decision(),
        Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
    ));
    let continuation = outcome
        .into_safe_continuation()
        .unwrap_or_else(|_| panic!("negative tombstone retains a safe stutter continuation"));
    assert!(
        continuation.into_target_and_terminal_replay().1.is_none(),
        "negative tombstones must never mint response replay authority"
    );
}
#[test]
fn negative_and_cancelled_certified_serve_tombstones_stutter_exactly() {
    use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;
    for (index, terminal) in [
        CertifiedServePayloadNegativeOutcome::Rejected(17),
        CertifiedServePayloadNegativeOutcome::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = RecoveryFixture::new(
            &format!("negative-serve-replay-{index}"),
            0xC1 + u8::try_from(index).expect("small fixture index") * 4,
        );
        let body_directory = TempDir::new().expect("temporary negative body store");
        let payload_directory = TempDir::new().expect("temporary negative payload store");
        let ledger_directory = TempDir::new().expect("temporary negative ledger store");
        let (mut owner, request) = fixture.open_terminal_serve_owner(
            &body_directory,
            &payload_directory,
            &ledger_directory,
            ServeTerminalFixture::Negative(terminal),
        );
        let expected_carriers = match terminal {
            CertifiedServePayloadNegativeOutcome::Cancelled => (0, 0),
            CertifiedServePayloadNegativeOutcome::Rejected(_)
            | CertifiedServePayloadNegativeOutcome::Failed(_) => (0, 1),
        };
        assert_eq!(
            owner.certified_serve_and_producer_carrier_counts_for_test(),
            expected_carriers
        );
        let target = super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
            fixture.verified.context(),
            request.request_hash(),
            1,
        );
        let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
        assert!(matches!(
            outcome.decision(),
            Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
        ));
        let continuation = outcome
            .into_safe_continuation()
            .unwrap_or_else(|_| panic!("negative tombstone retains a safe stutter continuation"));
        assert!(
            continuation.into_target_and_terminal_replay().1.is_none(),
            "negative tombstones must never mint response replay authority"
        );
        assert_eq!(
            owner.certified_serve_and_producer_carrier_counts_for_test(),
            expected_carriers
        );
        let foreign_retainer_target =
            super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                fixture.verified.context(),
                request.request_hash(),
                2,
            );
        let foreign_retainer = owner.admit_selected_certified_serve(
            foreign_retainer_target,
            &fixture.keys[1],
            &request,
        );
        assert!(foreign_retainer.restart_required());
        assert_eq!(
                    foreign_retainer.failure(),
                    Some(
                        super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                    )
                );
        assert!(foreign_retainer.into_safe_continuation().is_err());
        assert_eq!(
            owner.coordinator.fault(),
            Some(super::super::super::CoordinatorFault::DurabilityFailure)
        );
    }
}
#[test]
fn production_owner_rejects_changed_store_and_corrupt_census_without_further_writes() {
    let fixture = RecoveryFixture::new("changed-production-owner-store", 0x61);
    let body_directory = TempDir::new().expect("temporary changed-store body root");
    let body_store = fixture.open_store(&body_directory);
    let payload_directory = TempDir::new().expect("temporary changed-store payload root");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(Vec::new());
    let ledger_directory = TempDir::new().expect("temporary changed-store ledger root");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal changed-store production cut");
    let changed =
        LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
            .expect("construct same-context changed ledger frame");
    cut.ledger_store
        .persist(&changed)
        .expect("replace the retained store after cut mint");
    let ledger_after_external_change = snapshot_files(ledger_directory.path());
    let body_before_failure = snapshot_files(body_directory.path());
    let payload_before_failure = snapshot_files(payload_directory.path());
    let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
        panic!("same-store frame change must fail closed")
    };
    assert!(matches!(
        error.kind,
        ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
            | ProductionLifecycleStartupErrorKindV1::LedgerFrameMismatch
    ));
    assert_eq!(
        snapshot_files(ledger_directory.path()),
        ledger_after_external_change
    );
    assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
    assert_eq!(
        snapshot_files(payload_directory.path()),
        payload_before_failure
    );
    let fixture = RecoveryFixture::new("corrupt-production-owner-census", 0x71);
    let body_directory = TempDir::new().expect("temporary corrupt-census body root");
    let mut body_store = fixture.open_store(&body_directory);
    let fetch = fixture.fetch_record(&mut body_store, 0, 0x72, 1, None, false);
    let payload_directory = TempDir::new().expect("temporary corrupt-census payload root");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger = fixture.ledger(vec![fetch]);
    let ledger_directory = TempDir::new().expect("temporary corrupt-census ledger root");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let mut cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal corrupt-census production cut");
    cut.corrupt_fetch_census_for_test();
    let ledger_before_failure = snapshot_files(ledger_directory.path());
    let body_before_failure = snapshot_files(body_directory.path());
    let payload_before_failure = snapshot_files(payload_directory.path());
    let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
        panic!("corrupt all-row Fetch census must fail closed")
    };
    assert!(matches!(
        error.kind,
        ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
    ));
    assert_eq!(
        snapshot_files(ledger_directory.path()),
        ledger_before_failure
    );
    assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
    assert_eq!(
        snapshot_files(payload_directory.path()),
        payload_before_failure
    );
}
#[test]
fn production_owner_rejects_an_unsupported_live_class_before_publication() {
    let fixture = RecoveryFixture::new("unsupported-live-production-owner", 0x81);
    let replay = super::super::super::replay_authority::exact_record_fixture(
        fixture.lifecycle_context(),
        LifecycleStageKind::SignProposal,
        0x82,
    );
    let causal_root = CausalRoot::new(LifecycleDigest::new([0x83; 32]));
    let record = LifecycleLedgerRecordV1::new(
        replay.key,
        OwnerId::new(causal_root, 1),
        1,
        replay.work_class,
        replay.stage,
        None,
        causal_root.digest(),
        replay.payload,
        replay.authority,
        DurableContinuation::None,
    )
    .expect("construct unsupported live SignProposal row");
    let ledger = fixture.ledger(vec![record]);
    let body_directory = TempDir::new().expect("temporary unsupported-live body root");
    let body_store = fixture.open_store(&body_directory);
    let payload_directory = TempDir::new().expect("temporary unsupported-live payload root");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let ledger_directory = TempDir::new().expect("temporary unsupported-live ledger root");
    let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("seal unsupported-live storage cut before exhaustive classification");
    let before = (
        snapshot_files(ledger_directory.path()),
        snapshot_files(body_directory.path()),
        snapshot_files(payload_directory.path()),
    );
    let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
        panic!("unsupported live class must fail closed")
    };
    assert!(matches!(
        error.kind,
        ProductionLifecycleStartupErrorKindV1::Recovery(_)
    ));
    assert_eq!(snapshot_files(ledger_directory.path()), before.0);
    assert_eq!(snapshot_files(body_directory.path()), before.1);
    assert_eq!(snapshot_files(payload_directory.path()), before.2);
}
#[test]
fn consuming_storage_cut_rejects_foreign_context_store_sources_and_qc() {
    let fixture = RecoveryFixture::new("durable-ready-fetch-rejections", 0x51);
    let foreign = RecoveryFixture::new("foreign-durable-ready-fetch", 0x61);
    let exact_empty_ledger = fixture.ledger(Vec::new());
    let exact_empty_body_directory = TempDir::new().expect("temporary exact empty body store");
    let exact_empty_body_store = fixture.open_store(&exact_empty_body_directory);
    let foreign_ledger = foreign.ledger(Vec::new());
    let foreign_ledger_directory =
        TempDir::new().expect("temporary foreign lifecycle ledger store");
    let foreign_ledger_store = foreign.persist_ledger(&foreign_ledger_directory, &foreign_ledger);
    assert!(matches!(
        exact_empty_ledger.into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            foreign_ledger_store,
            exact_empty_body_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidLedgerStore)
    ));
    let foreign_context_directory = TempDir::new().expect("temporary foreign-context body store");
    let mut foreign_context_store = fixture.open_store(&foreign_context_directory);
    let foreign_context_record =
        fixture.fetch_record(&mut foreign_context_store, 0, 0x71, 1, None, false);
    let foreign_context_ledger = fixture.ledger(vec![foreign_context_record]);
    let foreign_context_ledger_directory =
        TempDir::new().expect("temporary foreign-context lifecycle ledger");
    let foreign_context_ledger_store =
        fixture.persist_ledger(&foreign_context_ledger_directory, &foreign_context_ledger);
    assert!(matches!(
        foreign_context_ledger.into_durable_certified_body_pipeline_storage_recovery_cut(
            foreign.verified.clone(),
            foreign_context_ledger_store,
            foreign_context_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidVerifiedContext)
    ));
    let foreign_store_directory = TempDir::new().expect("temporary exact-context body store");
    let mut exact_store = fixture.open_store(&foreign_store_directory);
    let exact_record = fixture.fetch_record(&mut exact_store, 0, 0x72, 1, None, false);
    let foreign_body_directory = TempDir::new().expect("temporary foreign body-store context");
    let foreign_store = foreign.open_store(&foreign_body_directory);
    let exact_ledger = fixture.ledger(vec![exact_record]);
    let exact_ledger_directory = TempDir::new().expect("temporary exact-context lifecycle ledger");
    let exact_ledger_store = fixture.persist_ledger(&exact_ledger_directory, &exact_ledger);
    assert!(matches!(
        exact_ledger.into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            exact_ledger_store,
            foreign_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidBodyStoreContext)
    ));
    let wrong_sources_directory = TempDir::new().expect("temporary wrong-sources body store");
    let mut wrong_sources_store = fixture.open_store(&wrong_sources_directory);
    let wrong_sources = vec![fixture.verified.context().roster[0].validator.clone()];
    let wrong_sources_record = fixture.fetch_record(
        &mut wrong_sources_store,
        0,
        0x73,
        1,
        Some(wrong_sources),
        false,
    );
    assert!(
                wrong_sources_record
                    .authenticate_durable_certified_fetch_origin(&fixture.verified, || -> Result<
                        AuthenticatedDurableBodyFrameRecovery,
                        DurableBodyFrameRecoveryError,
                    > {
                        panic!("body-store authority must not be minted before source rejection")
                    })
                    .expect("source rejection does not inspect the body store")
                    .is_none()
            );
    let wrong_sources_ledger = fixture.ledger(vec![wrong_sources_record]);
    let wrong_sources_ledger_directory =
        TempDir::new().expect("temporary wrong-sources lifecycle ledger");
    let wrong_sources_ledger_store =
        fixture.persist_ledger(&wrong_sources_ledger_directory, &wrong_sources_ledger);
    assert!(matches!(
        wrong_sources_ledger.into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            wrong_sources_ledger_store,
            wrong_sources_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidReplayJoin)
    ));
    let corrupt_qc_directory = TempDir::new().expect("temporary corrupt-QC body store");
    let mut corrupt_qc_store = fixture.open_store(&corrupt_qc_directory);
    let corrupt_qc_record = fixture.fetch_record(&mut corrupt_qc_store, 0, 0x74, 1, None, true);
    let corrupt_qc_ledger = fixture.ledger(vec![corrupt_qc_record]);
    let corrupt_qc_ledger_directory =
        TempDir::new().expect("temporary corrupt-QC lifecycle ledger");
    let corrupt_qc_ledger_store =
        fixture.persist_ledger(&corrupt_qc_ledger_directory, &corrupt_qc_ledger);
    assert!(matches!(
        corrupt_qc_ledger.into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            corrupt_qc_ledger_store,
            corrupt_qc_store,
        ),
        Err(DurableCertifiedBodyPipelineRecoveryError::InvalidReplayJoin)
    ));
}
