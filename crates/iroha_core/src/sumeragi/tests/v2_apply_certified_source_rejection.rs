// Exact authenticated adverse controls for retired execution-bearing MergeQC carriers.

/// Sign exact three-of-four lane QCs with the canonical executable READY body.
fn certified_source_certificate_for_rejection(
    fixture: &ApplyFixture,
    payload: &LaneExecutablePayloadV1,
    keys: &[KeyPair],
) -> (
    iroha_data_model::block::consensus::LaneBlockCertificateV1,
    Vec<crate::lane_consensus::LaneBlockVoteV1>,
    Vec<crate::lane_consensus::LaneBlockVoteV1>,
) {
    let proposal = &payload.origin_proposal;
    assert_eq!(proposal.descriptor.validator_count, 4);
    assert_eq!(proposal.descriptor.min_quorum, 3);
    assert_eq!(
        proposal.descriptor.validator_set,
        keys.iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    assert!(
        keys.iter().all(|key| fixture
            .validator_keys
            .iter()
            .any(|known| known.public_key() == key.public_key())),
        "all fixture lane signers are authenticated validators"
    );
    let availability = crate::lane_consensus::lane_payload_availability_body(
        payload,
        proposal,
        payload.network_id,
        payload.epoch,
    )
    .expect("exact second-cycle executable availability");
    let validator_set_pops = keys
        .iter()
        .map(|validator_key| {
            iroha_crypto::bls_normal_pop_prove(validator_key.private_key())
                .expect("PoP in the exact lane committee order")
        })
        .collect::<Vec<_>>();
    let votes = |phase| {
        keys[..3]
            .iter()
            .map(|key| {
                let body = proposal.vote_body(phase);
                let signer = PeerId::new(key.public_key().clone());
                let ready = (phase == CertPhase::Prepare).then(|| {
                    crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                        availability.clone(),
                        signer.clone(),
                        validator_set_pops.clone(),
                        key.private_key(),
                    )
                    .expect("sign exact second-cycle READY")
                });
                crate::lane_consensus::LaneBlockVoteV1 {
                    bls_signature: Signature::try_new(
                        key.private_key(),
                        &body.signature_preimage(),
                    )
                    .expect("sign exact second-cycle lane vote")
                    .payload()
                    .to_vec(),
                    body,
                    signer,
                    payload_availability_vote: ready,
                }
            })
            .collect::<Vec<_>>()
    };
    let prepare_votes = votes(CertPhase::Prepare);
    let commit_votes = votes(CertPhase::Commit);
    let qc = |phase, votes: &[crate::lane_consensus::LaneBlockVoteV1]| {
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(phase),
            proposal.descriptor.validator_set.clone(),
            votes,
        )
        .expect("aggregate exactly three of four lane votes")
    };
    (
        iroha_data_model::block::consensus::LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: qc(CertPhase::Prepare, &prepare_votes),
            commit_qc: qc(CertPhase::Commit, &commit_votes),
        },
        prepare_votes,
        commit_votes,
    )
}

fn certified_merge_entry_for_source_rejection(
    candidate: &crate::merge::MergeLedgerCandidate,
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> MergeLedgerEntry {
    let validators = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    assert_eq!(validators.len(), 4);
    assert_eq!(
        validators,
        keys.iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    let validator_hash = HashOf::new(&validators);
    let digest = crate::merge::merge_qc_message_digest(
        &context.network_id,
        candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_hash,
    );
    let signatures = keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), digest.as_ref())
                .expect("sign certified merge vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let proofs = keys[..3]
        .iter()
        .enumerate()
        .map(|(index, key)| iroha_data_model::merge::MergeSignerProof {
            signer: u32::try_from(index).expect("merge signer index"),
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("certified merge signer PoP"),
        })
        .collect::<Vec<_>>();
    let qc = MergeQuorumCertificate::new(
        candidate.view,
        candidate.epoch_id,
        candidate.carrier_height,
        candidate.carrier_parent_hash,
        context.network_id,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_hash,
        validators,
        vec![0b0000_0111],
        proofs,
        iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate certified merge three-of-four signatures"),
        digest,
    );
    let entry = candidate.clone().into_entry(qc);
    crate::sumeragi::v2_lane_work::authenticate_merge_entry_for_height_context(context, &entry)
        .expect("authenticate certified merge against the exact frozen context");
    entry
}

/// Build a genuinely signed carrier that the canonical source gate must reject.
fn assert_retired_merge_carrier_rejected(
    fixture: &ApplyFixture,
    service: &V2ApplyService,
    context: &wire::HeightContext,
    entry: &MergeLedgerEntry,
    header: &BlockHeader,
    keys: &[KeyPair],
) {
    let height = context.height;
    assert_eq!(header.height().get(), height);
    let parent_height =
        NonZeroUsize::new(usize::try_from(height - 1).expect("merge parent height"))
            .expect("nonzero merge parent");
    let parent = fixture
        .kura
        .get_block(parent_height)
        .expect("read actual merge parent");
    assert_eq!(fixture.state.latest_block_hash_fast(), Some(parent.hash()));
    assert_eq!(header.prev_block_hash(), Some(parent.hash()));
    let (_, time) = TimeSource::new_mock(header.creation_time());
    let confidential_features = {
        let view = fixture.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    let view = entry.merge_qc.view;
    assert_eq!(header.view_change_index(), view);
    let leader = context.leader(view);
    let body = BlockBuilder::new_with_time_source(Vec::new(), time)
        .chain(view, Some(parent.as_ref()))
        .bind_certified_merge_application_context(header)
        .expect("bind certified application context")
        .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &fixture.state.nexus_snapshot(),
            height,
        )))
        .with_confidential_features(confidential_features)
        .with_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_merge_entry(CertifiedMergeLedgerReference::new(entry)),
        ))
        .try_sign_with_index(
            keys[usize::try_from(leader).expect("merge leader")].private_key(),
            u64::from(leader),
        )
        .expect("sign certified merge carrier")
        .unpack(|_| {});
    let body = SignedBlock::from(body);
    assert_eq!(body.header().height().get(), height);
    assert_retired_merge_candidate_rejected(fixture, service, context, &body, entry);
}

/// Valid certificates and source material cannot create a third execution source.
fn assert_retired_merge_candidate_rejected(
    fixture: &ApplyFixture,
    service: &V2ApplyService,
    context: &wire::HeightContext,
    body: &SignedBlock,
    entry: &MergeLedgerEntry,
) {
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("execution-bearing adverse control");
    let before_state = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref())
        .expect("capture exact applying State");
    let before_ledger = fixture.state.merge_ledger.snapshot();
    let before_height = fixture.state.committed_height();
    let before_tree = completed_secondary_tree(&fixture.kura.store_root());
    let membership = batch
        .lanes
        .iter()
        .flat_map(|lane| lane.entrypoint_hashes.iter())
        .map(|hash| {
            let hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(*hash);
            (hash, fixture.state.has_committed_entrypoint(hash))
        })
        .collect::<Vec<_>>();
    for _ in 0..2 {
        let error = service
            .validate_candidate(context, body)
            .expect_err("MergeQC cannot substitute for the canonical native Decision source");
        assert!(
            matches!(&error, V2ApplyError::Validation(reason)
            if reason.contains("ordinary output source contains a competing native owner")),
            "retired source must fail at its canonical source guard: {error:?}"
        );
        assert_eq!(fixture.state.committed_height(), before_height);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()).unwrap(),
            before_state
        );
        assert_eq!(fixture.state.merge_ledger.snapshot(), before_ledger);
        for (hash, committed) in &membership {
            assert_eq!(fixture.state.has_committed_entrypoint(*hash), *committed);
        }
        assert_eq!(
            completed_secondary_tree(&fixture.kura.store_root()),
            before_tree,
            "rejected execution cannot acknowledge, rewrite or discard durable source custody"
        );
    }
}
