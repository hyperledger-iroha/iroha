//! Genuine sparse-store joins preserve native custody and exact successor authority.

use super::*;
use crate::sumeragi::finality::{
    NativeFinalityProofBudget, NativeFinalityProofIntervalLimits, NativeFinalityProofSource,
    build_proof_interval, native_finality_graph_floor_bytes,
};
use iroha_allocation::AllocationBudget;
use std::{num::NonZeroU64, sync::atomic::AtomicBool, time::Instant};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}

#[test]
fn portable_resume_reads_only_actual_successor_bodies_and_continues_native_walk() {
    run(|| {
        let (_directory, _config, kura, mut fixture) =
            crate::kura::checkpoint_prefix::tests::imported();
        fixture.chain.commit(Vec::new());
        kura.persist_block_immediate_for_bench(fixture.chain.committed(4).block())
            .unwrap();
        let hashes: Vec<_> = (1..=4)
            .map(|height| fixture.chain.committed(height).block_hash())
            .collect();
        let network = fixture.chain.network_id();
        let reader = CertifiedChain::from_pinned(
            fixture.chain.state().chain_id_ref(),
            &network,
            &hashes,
            &kura,
            &fixture.chain.state().ivm_execution_budget(),
        )
        .unwrap();
        assert!(reader.committed(2).is_err());
        assert!(reader.certified(2).is_err());
        let (actual, counts) = relation_counts::measure(|| {
            reader
                .walk(3, 4)
                .map(|block| {
                    let block = block.unwrap();
                    (
                        block.height(),
                        block.committed().block_hash(),
                        block.committed().result(),
                    )
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(
            actual,
            (3..=4)
                .map(|height| {
                    let original = fixture.chain.committed(height);
                    (height, original.block_hash(), original.result())
                })
                .collect::<Vec<_>>()
        );
        assert!(
            !counts.frames.contains(&2),
            "omitted H never becomes a native frame"
        );
        assert!(counts.qcs.contains(&3) && counts.qcs.contains(&4));
        assert_eq!(
            reader.certified(3).unwrap().committed().block_hash(),
            hashes[2]
        );
        assert!(
            reader.committed(2).is_err(),
            "a warm cursor cannot manufacture omitted custody"
        );
    });
}

#[test]
fn portable_resume_query_exports_original_native_wire_and_refuses_missing_height() {
    run(|| {
        let (_directory, _config, kura, fixture) =
            crate::kura::checkpoint_prefix::tests::imported();
        let hashes: Vec<_> = (1..=3)
            .map(|height| fixture.chain.committed(height).block_hash())
            .collect();
        let network = fixture.chain.network_id();
        let source = NativeFinalityProofSource::new(
            fixture.chain.state().chain_id_ref(),
            &network,
            &hashes,
            &kura,
        );
        let frames = AllocationBudget::new(64 * 1024 * 1024);
        let owner = NativeFinalityProofBudget::new(
            frames.clone(),
            AllocationBudget::new(native_finality_graph_floor_bytes().unwrap() + 64 * 1024 * 1024),
            norito::DecodeLimits::new(
                1024 * 1024,
                64 * 1024 * 1024,
                8 * 1024 * 1024,
                usize::MAX,
                128,
            ),
        );
        let limits = |height| {
            NativeFinalityProofIntervalLimits::new(
                NonZeroU64::new(height).unwrap(),
                NonZeroU64::new(height).unwrap(),
                NonZeroUsize::new(iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES)
                    .unwrap(),
                NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
                Instant::now() + std::time::Duration::from_mins(15),
            )
            .unwrap()
        };
        let response =
            build_proof_interval(&source, &owner, &limits(3), &AtomicBool::new(false)).unwrap();
        assert_eq!(response.len(), 1);
        assert_eq!(
            response.proof(0).unwrap().block_wire,
            fixture.chain.committed(3).block().encode_wire().unwrap()
        );
        drop(response);
        assert!(
            build_proof_interval(&source, &owner, &limits(2), &AtomicBool::new(false)).is_err()
        );
        assert_eq!(
            frames.reserved_bytes(),
            0,
            "query refusal releases original frame custody"
        );
    });
}

#[test]
fn portable_resume_rejects_a_changed_pinned_checkpoint_journal() {
    run(|| {
        let (_directory, _config, kura, fixture) =
            crate::kura::checkpoint_prefix::tests::imported();
        let mut hashes: Vec<_> = (1..=3)
            .map(|height| fixture.chain.committed(height).block_hash())
            .collect();
        hashes[1] = HashOf::from_untyped_unchecked(Hash::new(b"foreign omitted parent"));
        let network = fixture.chain.network_id();
        let reader = CertifiedChain::from_pinned(
            fixture.chain.state().chain_id_ref(),
            &network,
            &hashes,
            &kura,
            &fixture.chain.state().ivm_execution_budget(),
        )
        .unwrap();
        assert!(reader.certified(3).is_err());
    });
}
