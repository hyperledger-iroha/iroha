//! Original committed archive availability, integrity and refusal controls.

use super::*;
use crate::{
    state::{NativeExecutionProjectionV1, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_data_model::block::consensus::ExecKv;

fn committed_chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain
}

fn original_path(chain: &CertifiedTestChain) -> std::path::PathBuf {
    chain
        .kura()
        .store_root()
        .join("native-contexts")
        .join(format!(
            "{:020}-{}.nrt",
            2,
            hex::encode(chain.committed(2).block_hash().as_ref()),
        ))
}

#[test]
fn committed_fee_archive_preserves_original_corpus_and_rejects_absent_exposure() {
    let chain = committed_chain();
    let view = chain.state().view();
    let proof = committed_fee_evidence(&view, 2).unwrap();
    assert!(
        proof.verify(
            chain
                .committed(2)
                .commitment()
                .execution
                .ordinary_writes_root
        )
    );
    assert_eq!(
        proof
            .snapshot_witness
            .commitment()
            .unwrap()
            .evaluated_height,
        2
    );
    let key = "reward_history/ExposureArchive/absent".parse().unwrap();
    assert!(matches!(
        committed_reward_exposure(&view, 2, &key, Hash::new(b"absent")),
        Err(ExecutionAttemptError::Rejected(reason)) if reason.contains("source is absent")
    ));
}

#[test]
fn committed_reward_archive_missing_corrupt_and_changed_root_defer_without_fabrication() {
    let chain = committed_chain();
    let path = original_path(&chain);
    let original = std::fs::read(&path).unwrap();
    let hidden = path.with_extension("retained-test-original");
    std::fs::rename(&path, &hidden).unwrap();
    let missing = committed_fee_evidence(&chain.state().view(), 2);
    std::fs::rename(&hidden, &path).unwrap();
    assert!(
        matches!(missing, Err(ExecutionAttemptError::Deferred(reason))
        if reason.reason() == ExecutionDeferral::CanonicalHistoryUnavailable)
    );

    std::fs::write(&path, b"invalid canonical archive").unwrap();
    let corrupt = committed_fee_evidence(&chain.state().view(), 2);
    std::fs::write(&path, &original).unwrap();
    assert!(
        matches!(corrupt, Err(ExecutionAttemptError::Deferred(reason))
        if reason.reason() == ExecutionDeferral::CanonicalHistoryUnavailable)
    );

    let mut projection: NativeExecutionProjectionV1 = norito::decode_canonical(&original).unwrap();
    projection.ordinary_writes.push(ExecKv {
        key: b"unexecuted-reward-source".to_vec(),
        value: vec![1],
    });
    std::fs::write(&path, norito::to_bytes(&projection).unwrap()).unwrap();
    let changed = committed_fee_evidence(&chain.state().view(), 2);
    std::fs::write(&path, &original).unwrap();
    assert!(
        matches!(changed, Err(ExecutionAttemptError::Deferred(reason))
        if reason.reason() == ExecutionDeferral::CanonicalHistoryUnavailable)
    );
    committed_fee_evidence(&chain.state().view(), 2).unwrap();
}

#[test]
fn reward_archive_capacity_preserves_original_pool_refusal() {
    let budget = AllocationBudget::new(32);
    let held = budget.try_reserve_bytes(32).unwrap();
    let expected = budget.try_reserve_bytes(1).unwrap_err();
    let actual = ChargedBuffer::<u8>::new(1, &budget)
        .err()
        .expect("zero-capacity pool refuses the original buffer");
    let error = archive_error(NativeContextArchiveError::Allocation(actual));
    assert!(matches!(error, ExecutionAttemptError::Deferred(reason)
        if reason.allocation_refusal() == Some(&expected)));
    drop(held);
    ChargedBuffer::<u8>::new(1, &budget).unwrap();
}
