//! The names route must read its current native decision without replaying historical QCs.

use super::*;
use iroha_core::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_JOURNAL_BYTES, NativeFinalityLimits,
    },
};

fn limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: NATIVE_FINALITY_MAX_BLOCK_BYTES,
        journal_bytes: NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        // Exactly the constructor's signed genesis and the native tip/parent.
        block_count: 3,
        allocated_bytes: 128 * 1024 * 1024,
    }
}

#[test]
fn names_current_tip_uses_three_original_frames_at_later_height_and_keeps_source_bounds() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1))
        .map_err(|error| error.error)
        .unwrap();
    for height in [3, 12, 32] {
        for next in chain.height() + 1..=height {
            chain.commit_at(next * 1_000, Vec::new());
        }
        let view = chain.state().view();
        let read = |limits: NativeFinalityLimits| {
            norito::core::with_decode_limits_scope(limits.decode_limits().unwrap(), || {
                current_names_tip(&view, height, SumeragiRootScope::Global, limits)
            })
        };
        let baseline_started = std::time::Instant::now();
        let baseline = CertifiedChain::new(&view)
            .unwrap()
            .certified(height)
            .unwrap()
            .into_committed();
        let baseline_elapsed = baseline_started.elapsed();
        let recent_started = std::time::Instant::now();
        let actual = read(limits()).unwrap();
        let recent_elapsed = recent_started.elapsed();
        eprintln!(
            "names current-tip height={height} old_prefix_us={} recent_us={} original_source_frames=3",
            baseline_elapsed.as_micros(),
            recent_elapsed.as_micros(),
        );
        let original = chain.committed(height);
        assert_eq!(actual.id(), original.id());
        assert_eq!(actual.id(), baseline.id());
        assert_eq!(actual.block_hash(), original.block_hash());
        assert_eq!(actual.core_hash(), original.core_hash());
        assert_eq!(actual.result(), original.result());
        for insufficient in [
            NativeFinalityLimits {
                block_count: 2,
                ..limits()
            },
            NativeFinalityLimits {
                block_bytes: 1,
                ..limits()
            },
            NativeFinalityLimits {
                allocated_bytes: 1,
                ..limits()
            },
        ] {
            assert!(read(insufficient).is_err());
        }
        // A local refusal cannot poison the original source for a later read.
        assert_eq!(read(limits()).unwrap().id(), original.id());
        for different_height in [1, height - 1, height + 1] {
            assert!(
                current_names_tip(&view, different_height, SumeragiRootScope::Global, limits())
                    .is_err()
            );
        }
    }
}

#[test]
fn names_current_tip_preserves_actual_signed_dataspace_scope_and_rejects_other_roots() {
    let (chain, approval, _signer) =
        iroha_core::sumeragi::test_chain::native_amx_approval_route_fixture();
    let view = chain.state().view();
    let actual_scope = SumeragiRootScope::Dataspace {
        parent_network_id: approval.global_network,
        dataspace_id: approval.dataspace,
    };
    let read = |scope| {
        norito::core::with_decode_limits_scope(limits().decode_limits().unwrap(), || {
            current_names_tip(&view, 2, scope, limits())
        })
    };
    let actual = read(actual_scope).unwrap();
    assert_eq!(actual.id(), chain.committed(2).id());
    assert!(read(SumeragiRootScope::Global).is_err());
    let other_scope = SumeragiRootScope::Dataspace {
        parent_network_id: approval.global_network,
        dataspace_id: iroha_data_model::nexus::DataSpaceId::new(
            approval.dataspace.as_u64().wrapping_add(1),
        ),
    };
    assert!(read(other_scope).is_err());
    assert_eq!(read(actual_scope).unwrap().id(), actual.id());
}

#[test]
fn names_status_height_mismatch_is_refused_before_current_proof_work() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1))
        .map_err(|error| error.error)
        .unwrap();
    chain.commit_at(1_000, Vec::new());
    let height = chain.height();
    let status = iroha_data_model::sumeragi::SumeragiStatus {
        protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
        config_fingerprint: iroha_crypto::Hash::new(b"names status height fixture"),
        beacon_horizon: None,
        instance: chain.instance().0,
        height: height + 1,
        view: 0,
        stage: 0,
        leader: None,
        proxy_tail: None,
        high_qc_view: None,
        level: 0,
        start_level: 0,
        t_retx_ms: 100,
        committed_height: height,
        applied_height: height,
        awaiting: false,
        signer: Some(chain.validators()[0].0.public_key().clone()),
        unanchored: false,
        abstaining: false,
        halted: None,
        footprint: iroha_data_model::sumeragi::SumeragiFootprint::default(),
    };
    require_status_height(&status, height).unwrap();
    for (committed, applied) in [
        (height - 1, height - 1),
        (height + 1, height + 1),
        (height, height - 1),
        (height - 1, height),
    ] {
        let mut different = status.clone();
        different.committed_height = committed;
        different.applied_height = applied;
        let error = require_status_height(&different, height).unwrap_err();
        assert_eq!(
            finalize(Err(error)).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(different.committed_height, committed);
        assert_eq!(different.applied_height, applied);
    }
    require_status_height(&status, height).unwrap();
}
