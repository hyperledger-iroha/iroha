//! Current SNS proof reads retain bounded source work as the certified chain grows.

use super::*;
use iroha_core::{
    state::{State as CoreState, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{Encode, domain::Domain, isi::Register, sns::lease::SnsLeaseProofV1};
use iroha_model_base::domain::DomainId;

fn limits() -> NativeFinalityLimits {
    // Small original sources also fit a deliberately lower configured allowance.
    lease_source_limits(48_000_000)
}

#[test]
fn lease_current_tip_uses_three_frames_at_increasing_heights_and_keeps_complete_proof() {
    let domain = DomainId::try_new("leaseproof", "universal").unwrap();
    let selector = iroha_core::sns::selector_for_domain(&domain).unwrap();
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_instructions = vec![Register::domain(Domain::new(domain)).into()];
    let mut chain = CertifiedTestChain::start(config)
        .map_err(|error| error.error)
        .unwrap();
    for height in [3, 12, 32] {
        for next in chain.height() + 1..=height {
            chain.commit_at(next * 1_000, Vec::new());
        }
        let view = chain.state().view();
        let record = iroha_core::sns::record_by_selector(view.world(), &selector)
            .unwrap()
            .unwrap();
        let tip = current_lease_tip(&view, height, limits()).unwrap();
        let original = chain.committed(height);
        assert_eq!(tip.id(), original.id());
        assert_eq!(tip.block_hash(), original.block_hash());
        assert_eq!(tip.result(), original.result());
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
            assert!(current_lease_tip(&view, height, insufficient).is_err());
        }
        for wrong_height in [1, height - 1, height + 1] {
            assert!(current_lease_tip(&view, wrong_height, limits()).is_err());
        }
        // Refused work cannot poison the unchanged original source.
        assert_eq!(
            current_lease_tip(&view, height, limits()).unwrap().id(),
            original.id()
        );
        drop(view);
        let budget = AllocationBudget::new(48_000_000);
        let source = budget
            .try_reserve_bytes(limits().journal_bytes + limits().allocated_bytes)
            .unwrap();
        let body = chain
            .state()
            .with_native_sns_lease_snapshot_v1(&tip, &selector, &budget, |world, bytes| {
                crate::native_projection_response::encode(
                    &SnsLeaseProofRefV1::new(world, bytes),
                    ResponseFormat::Norito,
                    MAX_SNS_LEASE_PROOF_BYTES_V1,
                    &budget,
                )
                .map_err(|error| error.to_string())
            })
            .unwrap();
        let proof = SnsLeaseProofV1::decode_frame(body.as_ref()).unwrap();
        assert_eq!(proof.record, record.encode());
        assert_eq!(
            proof.world.schema_hash,
            CoreState::native_world_schema_hash_v1().unwrap()
        );
        assert_eq!(
            proof.world.root().unwrap(),
            tip.commitment().execution.world_state_root
        );
        drop(body);
        drop(tip);
        drop(source);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn lease_current_tip_rejects_a_genuinely_signed_private_root() {
    let (chain, _, _) = iroha_core::sumeragi::test_chain::native_amx_approval_route_fixture();
    assert!(current_lease_tip(&chain.state().view(), chain.height(), limits()).is_err());
}

#[test]
fn lease_default_finality_allowance_handles_consecutive_maximum_payments_and_releases_body() {
    let chain = crate::finality_interval::maximum_paid_chain_for_testing();
    let working = usize::try_from(
        iroha_config::parameters::defaults::torii::finality_working_set_bytes(
            iroha_config::parameters::defaults::torii::QUERY_FANOUT_MAX_RETAINED_BYTES.0,
            iroha_config::parameters::defaults::torii::FINALITY_MAX_WORKING_SET_BYTES.0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(working, 384_000_000);
    let limits = lease_source_limits(working);
    assert_eq!(limits.block_bytes, NATIVE_FINALITY_MAX_BLOCK_BYTES);
    assert_eq!(limits.journal_bytes, NATIVE_FINALITY_MAX_JOURNAL_BYTES);
    assert_eq!(limits.block_count, 3);
    let pool = ByteWeightedMemoryPool::new(working).unwrap();
    let memory =
        QueryFanoutMemoryReservation::new(pool.try_acquire_parts([working as u64]).unwrap());
    let budget = AllocationBudget::new(working);
    let source = budget
        .try_reserve_bytes(limits.journal_bytes + limits.allocated_bytes)
        .unwrap();
    let view = chain.state().view();
    // Two genuine 10 MiB signed payments remain the current cut and its parent.
    // The generic 48 MB query split permanently refuses this valid original.
    assert_eq!(chain.height(), 3);
    assert!(current_lease_tip(&view, 3, lease_source_limits(48_000_000)).is_err());
    let tip = norito::core::with_decode_limits_scope(limits.decode_limits().unwrap(), || {
        crate::native_projection_response::current_global_tip(&view, 3, limits, unavailable)
    })
    .unwrap_or_else(|error| panic!("original SNS current-cut refusal: {error:?}"));
    let selector = iroha_core::sns::selector_for_domain(
        &DomainId::try_new("nativeproof", "universal").unwrap(),
    )
    .unwrap();
    let record = iroha_core::sns::record_by_selector(view.world(), &selector)
        .unwrap()
        .unwrap();
    drop(view);
    let mut body = chain
        .state()
        .with_native_sns_lease_snapshot_v1(&tip, &selector, &budget, |world, bytes| {
            crate::native_projection_response::encode(
                &SnsLeaseProofRefV1::new(world, bytes),
                ResponseFormat::Norito,
                MAX_SNS_LEASE_PROOF_BYTES_V1,
                &budget,
            )
            .map_err(|error| error.to_string())
        })
        .unwrap();
    body.memory = Some(memory);
    let body = Bytes::from_owner(body);
    let expected_root = tip.commitment().execution.world_state_root;
    drop(tip);
    drop(source);
    let proof = SnsLeaseProofV1::decode_frame(&body).unwrap();
    assert_eq!(proof.record, record.encode());
    assert_eq!(
        proof.world.schema_hash,
        CoreState::native_world_schema_hash_v1().unwrap()
    );
    assert_eq!(proof.world.root().unwrap(), expected_root);
    let last = body.slice(..1);
    drop(body);
    assert_eq!(pool.available_bytes(), 0);
    assert!(pool.try_acquire_parts([1]).is_none());
    assert!(budget.reserved_bytes() > 0);
    drop(last);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(pool.available_bytes(), working as u64);
}
