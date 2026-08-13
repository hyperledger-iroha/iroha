fn unsealed_prune_capacity_fixture() -> KuraPruneCapacityAdmissionV2 {
    KuraPruneCapacityAdmissionV2 {
        source_physical_bytes: 0,
        pending_canonical_bytes: 0,
        post_wsv_reserved_bytes: 0,
        certified_bundle_reserved_bytes: 0,
        autonomous_terminal_reserved_bytes: 0,
        intent_bytes: 0,
        marker_temporary_bytes: 1,
        marker_stable_growth_bytes: 0,
        roster: CommitRosterJournalPruneProjectionV2::none(),
        admitted_peak_bytes: 0,
    }
}

fn seal_prune_intent_fixture(mut intent: KuraPruneIntentV2) -> KuraPruneIntentV2 {
    for _ in 0..4 {
        intent.capacity.intent_bytes = u64::try_from(
            norito::encode_canonical(&intent)
                .expect("encode prune-intent fixture")
                .len(),
        )
        .expect("prune-intent fixture length fits u64");
        intent.capacity.admitted_peak_bytes = intent
            .capacity
            .required_peak_bytes(intent.sidecar_rewrite)
            .expect("prune-intent fixture capacity fits u64");
    }
    let final_len = u64::try_from(
        norito::encode_canonical(&intent)
            .expect("encode sealed prune-intent fixture")
            .len(),
    )
    .expect("sealed prune-intent fixture length fits u64");
    assert_eq!(intent.capacity.intent_bytes, final_len);
    assert!(intent.capacity.is_canonical(intent.sidecar_rewrite));
    intent
}

fn admit_prune_intent_fixture(kura: &Kura, mut intent: KuraPruneIntentV2) -> KuraPruneIntentV2 {
    let pending_canonical_bytes = kura
        .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
        .expect("measure prune-intent fixture pending capacity");
    let roster = kura
        .roster_log
        .read()
        .project_truncate_to_height(intent.target_height)
        .expect("project prune-intent fixture roster publication");
    let (marker_temporary_bytes, marker_stable_growth_bytes) = kura
        .canonical_prune_commit_marker_projection(intent.target_height)
        .expect("project prune-intent fixture block marker publication");
    intent.capacity = kura
        .canonical_prune_capacity_admission_snapshot(
            pending_canonical_bytes,
            marker_temporary_bytes,
            marker_stable_growth_bytes,
            roster,
        )
        .expect("snapshot prune-intent fixture capacity");
    kura.seal_and_validate_canonical_prune_capacity_admission(intent)
        .expect("seal prune-intent fixture admission")
}

fn archival_roster_row_fixture(
    height: u64,
    block_hash: HashOf<BlockHeader>,
    roster: Vec<PeerId>,
) -> (Qc, iroha_data_model::consensus::ValidatorSetCheckpoint) {
    let state_root = Hash::prehashed([0_u8; Hash::LENGTH]);
    let signers_bitmap = vec![0b0000_0001];
    let aggregate_signature = vec![0xAB; 96];
    let qc = Qc {
        phase: Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: state_root,
        post_state_root: state_root,
        height,
        view: 0,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_owned(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster.clone(),
        aggregate: QcAggregate {
            signers_bitmap: signers_bitmap.clone(),
            bls_aggregate_signature: aggregate_signature.clone(),
        },
    };
    let checkpoint = iroha_data_model::consensus::ValidatorSetCheckpoint::new(
        height,
        qc.view,
        block_hash,
        state_root,
        state_root,
        roster,
        signers_bitmap,
        aggregate_signature,
        VALIDATOR_SET_HASH_VERSION_V1,
        None,
    );
    (qc, checkpoint)
}

#[test]
fn recovery_control_files_reject_cap_plus_one_before_decode() {
    fn create_sparse(path: &std::path::Path, len: u64) {
        std::fs::File::create(path)
            .expect("create oversized recovery control file")
            .set_len(len)
            .expect("size oversized recovery control file");
    }
    let kura = super::Kura::blank_kura_for_testing();
    let blocks_root = kura.active_blocks_dir.lock().clone();
    let rollback_path = super::Kura::rollback_intent_path(&blocks_root);
    let rollback_temp_path = rollback_path.with_extension("norito.tmp");
    for path in [&rollback_path, &rollback_temp_path] {
        create_sparse(
            path,
            u64::try_from(super::MAX_ROLLBACK_INTENT_V1_BYTES).expect("rollback cap fits u64") + 1,
        );
        assert!(
            super::Kura::load_rollback_intent(&blocks_root).is_err(),
            "main and temporary rollback intents must reject cap-plus-one metadata before reading"
        );
        std::fs::remove_file(path).expect("remove oversized rollback intent");
    }
    let association_path = kura.canonical_association_stage_path();
    create_sparse(
        &association_path,
        super::MAX_CANONICAL_ASSOCIATION_STAGE_BYTES + 1,
    );
    assert!(
        kura.read_canonical_association_stage().is_err(),
        "canonical association stage must reject cap-plus-one metadata before reading"
    );
    std::fs::remove_file(&association_path).expect("remove oversized canonical association stage");
    {
        let block_store = kura.block_store.lock();
        let rewrite_path = block_store.da_block_rewrite_stage_path();
        create_sparse(&rewrite_path, super::MAX_DA_BLOCK_REWRITE_STAGE_BYTES + 1);
        assert!(
            block_store.read_da_block_rewrite_stage().is_err(),
            "DA rewrite stage must reject cap-plus-one metadata before reading"
        );
        std::fs::remove_file(&rewrite_path).expect("remove oversized DA rewrite stage");
    }
    let claim_path = blocks_root.join("oversized-autonomous-claim.norito");
    create_sparse(
        &claim_path,
        u64::try_from(super::AUTONOMOUS_LANE_ENTRYPOINT_CLAIM_MAX_BYTES)
            .expect("claim cap fits u64")
            + 1,
    );
    assert!(
        super::Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).is_err(),
        "autonomous claim must reject cap-plus-one metadata before reading"
    );
}
