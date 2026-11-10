//! Public selection, original native missing-row refusal and nonpublication controls.

use super::*;
use crate::replay_check::tests::{store_config, tree};
use clap::Parser as _;
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

fn request() -> ExportRequest {
    // Public selectors only; these values are never imported as a native State or authority.
    let genesis =
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"selector-only"));
    ExportRequest {
        schema: REQUEST_SCHEMA.into(),
        scheme_id_hex: "11".repeat(32),
        credit_id_hex: "22".repeat(32),
        network_id: NetworkId::from_genesis_hash(genesis),
        genesis_hash: genesis,
        chain_id: "selector-only".into(),
        height: 2,
        block_hash: genesis,
        proof_from_height: 1,
    }
}

#[test]
fn selectors_and_closed_request_refuse_ambiguous_or_unbounded_inputs() {
    let base = request();
    base.validate().unwrap();
    for bad in ["", "11", &"00".repeat(32), &"AA".repeat(32)] {
        assert!(selector(bad).is_err());
    }
    for (first, height) in [(0, 2), (3, 2), (1, 1), (1, MAX_PROOFS + 1)] {
        let mut changed = base.clone();
        changed.proof_from_height = first;
        changed.height = height;
        assert!(changed.validate().is_err());
    }
    let mut value = norito::json::to_json(&base).unwrap();
    value.insert_str(1, "\"unknown\":true,");
    assert!(norito::json::from_str::<ExportRequest>(&value).is_err());
    let mut duplicate = norito::json::to_json(&base).unwrap();
    duplicate.insert_str(1, "\"height\":2,");
    assert!(norito::json::from_str::<ExportRequest>(&duplicate).is_err());
}

#[test]
fn selected_export_flags_require_replay_pair_and_refuse_other_continuations() {
    let pair = [
        "--world-payout-export-request",
        "/request.json",
        "--world-payout-export-output",
        "/output",
    ];
    assert!(
        crate::Args::try_parse_from(["iroha3d", "--check-replay"].into_iter().chain(pair)).is_ok()
    );
    assert!(crate::Args::try_parse_from(["iroha3d"].into_iter().chain(pair)).is_err());
    for flag in [
        "--world-payout-export-request",
        "--world-payout-export-output",
    ] {
        assert!(
            crate::Args::try_parse_from(["iroha3d", "--check-replay", flag, "/value"]).is_err()
        );
    }
    for extra in [
        vec!["--check-storage"],
        vec!["--check-config"],
        vec![
            "--managed-listeners",
            "/listener",
            "--managed-listener-deadline-ns",
            "1",
        ],
        vec!["--rehearse-taira-activation", "/pool"],
        vec![
            "--rehearse-sorafs-repair",
            "/signed",
            "--repair-execution-time-ms",
            "1234",
        ],
        vec![
            "--runtime-addition-review-request",
            "/review",
            "--runtime-addition-review-output",
            "/review-output",
        ],
    ] {
        assert!(
            crate::Args::try_parse_from(
                ["iroha3d", "--check-replay"]
                    .into_iter()
                    .chain(pair)
                    .chain(extra)
            )
            .is_err()
        );
    }
}

#[test]
fn export_does_not_publish_without_authenticated_nonempty_replay() {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    let path = root.join("request.json");
    std::fs::write(&path, norito::json::to_vec(&request()).unwrap()).unwrap();
    let before = tree(&root);
    let output = root.join("export");
    assert!(run_world_payout_export(&store_config(&root), &path, &output, None).is_err());
    assert!(!output.exists());
    assert_eq!(tree(&root), before);
}

#[test]
fn actual_native_cut_without_payout_refuses_before_callback_or_publication() {
    // Engineering chain is a negative test only. It is never emitted for shipping admission.
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let before = tree(chain.kura().store_root().as_path());
    let frames = AllocationBudget::new(128 * MIB);
    let graphs = AllocationBudget::new(256 * MIB);
    let tip = read_current_certified_cut(
        &chain.state().view(),
        chain.height(),
        chain.root_scope(),
        CurrentCutSourceLimits::new(
            NATIVE_FINALITY_MAX_BLOCK_BYTES,
            NATIVE_FINALITY_MAX_JOURNAL_BYTES,
            NATIVE_FINALITY_MAX_BLOCK_COUNT,
        )
        .unwrap(),
        &frames,
        &graphs,
    )
    .unwrap();
    let mut called = false;
    let result = chain.state().with_native_kagemusha_fee_snapshot_v1(
        &tip,
        [1; 32],
        [2; 32],
        &AllocationBudget::new(128 * MIB),
        |_, _| {
            called = true;
            Ok(())
        },
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("payout row is absent")
    );
    assert!(!called);
    assert_eq!(tree(chain.kura().store_root().as_path()), before);
}

#[test]
fn matched_snapshot_at_tip_cannot_export_without_reexecuting_a_block() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let state = Arc::clone(chain.state());
    let height = chain.height();
    let core = replay_check::check_replay(
        ReplayCheckInputs {
            state: Arc::clone(&state),
            source_lanes: chain.kura().store_root().join("lanes"),
            genesis_account: chain.genesis_account().clone(),
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        },
        &mut |_, _| panic!("an already restored tip must not execute another block"),
    )
    .unwrap();
    assert_eq!(core.outcome, ReplayCheckOutcome::Matched);
    assert_eq!(core.start, ReplayCheckStart::Snapshot(height));
    assert_eq!(core.replayed_to, height);

    let view = state.view();
    let genesis_hash = chain.genesis().hash();
    let selector = ExportRequest {
        network_id: view.network_id,
        genesis_hash,
        chain_id: view.chain_id.to_string(),
        height,
        block_hash: view.latest_block_hash().unwrap(),
        proof_from_height: height,
        ..request()
    };
    drop(view);
    let original = norito::json::to_vec(&selector).unwrap();
    let matched = MatchedReplay {
        state: state.as_ref(),
        report: &core,
        tip_height: height,
        genesis_hash,
    };
    let failure = prepare(&matched, &selector, &original)
        .err()
        .expect("snapshot-only equality cannot qualify fee export");
    assert_eq!(
        failure,
        "fee export requires a block re-executed after the replay start"
    );
}

#[test]
fn suffix_deadline_covers_final_local_encoding_and_releases_expired_original() {
    let before = Instant::now();
    let deadline = before + Duration::from_secs(1);
    let budget = AllocationBudget::new(4096);
    let available = budget.limit_bytes();
    assert_eq!(budget.reserved_bytes(), 0);
    let value = 42u64;

    // An expired entry must fail before even testing the intentionally zero frame bound.
    let early = retain_suffix_proof(1, &value, 0, &budget, deadline, || deadline);
    assert_eq!(
        early.err().unwrap(),
        "finality proof suffix deadline expired"
    );
    assert_eq!(budget.reserved_bytes(), 0);

    for finish in [deadline, deadline + Duration::from_nanos(1)] {
        let mut samples = [before, finish].into_iter();
        let late = retain_suffix_proof(1, &value, available, &budget, deadline, || {
            let sample = samples.next().expect("unexpected clock read");
            if sample == finish {
                assert!(budget.reserved_bytes() > 0);
            }
            sample
        });
        assert_eq!(
            late.err().unwrap(),
            "finality proof suffix deadline expired"
        );
        assert!(samples.next().is_none());
        assert_eq!(budget.reserved_bytes(), 0);
    }

    let on_time = retain_suffix_proof(1, &value, available, &budget, deadline, || before).unwrap();
    assert_eq!(on_time.descriptor.name, "proof-0001.norito");
    assert_eq!(
        on_time.descriptor.sha256,
        hex::encode(iroha_crypto::sha256(&on_time.bytes))
    );
    assert_eq!(budget.reserved_bytes(), on_time.bytes.len());
    drop(on_time);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn retained_payout_bytes_hold_the_original_pool_charge_until_drop() {
    let payload = b"public payout bytes";
    let budget = AllocationBudget::new(payload.len());
    let first = retain_bytes("fee-payout.norito".into(), payload, &budget).unwrap();
    assert_eq!(first.bytes.as_slice(), payload);
    assert!(first._charge.belongs_to(&budget));
    assert_eq!(
        first._charge.layout(),
        Layout::array::<u8>(payload.len()).unwrap()
    );
    assert_eq!(budget.reserved_bytes(), payload.len());
    assert!(retain_bytes("fee-payout.norito".into(), payload, &budget).is_err());
    assert_eq!(budget.reserved_bytes(), payload.len());
    drop(first);
    assert_eq!(budget.reserved_bytes(), 0);
    let retry = retain_bytes("fee-payout.norito".into(), payload, &budget).unwrap();
    assert_eq!(retry.bytes.as_slice(), payload);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), 0);
}
