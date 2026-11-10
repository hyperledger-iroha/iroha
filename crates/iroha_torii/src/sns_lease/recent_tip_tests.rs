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
    let budget = AllocationBudget::new(48_000_000);
    for height in [3, 12, 32] {
        for next in chain.height() + 1..=height {
            chain.commit_at(next * 1_000, Vec::new());
        }
        let view = chain.state().view();
        let record = iroha_core::sns::record_by_selector(view.world(), &selector)
            .unwrap()
            .unwrap();
        let tip = current_lease_tip(&view, height, limits(), &budget).unwrap();
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
            assert!(current_lease_tip(&view, height, insufficient, &budget).is_err());
        }
        for wrong_height in [1, height - 1, height + 1] {
            assert!(current_lease_tip(&view, wrong_height, limits(), &budget).is_err());
        }
        // Refused work cannot poison the unchanged original source.
        assert_eq!(
            current_lease_tip(&view, height, limits(), &budget)
                .unwrap()
                .id(),
            original.id()
        );
        drop(view);
        let body = chain
            .state()
            .with_native_sns_lease_cut_v1(&tip, &selector, &budget, |world, bytes| {
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
        assert_eq!(tip.world_state_root(), proof.world.root().unwrap());
        drop(body);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn lease_current_tip_rejects_a_genuinely_signed_private_root() {
    let (chain, _, _) = iroha_core::sumeragi::test_chain::native_amx_approval_route_fixture();
    assert!(
        current_lease_tip(
            &chain.state().view(),
            chain.height(),
            limits(),
            &AllocationBudget::new(48_000_000)
        )
        .is_err()
    );
}

#[test]
fn lease_source_allowance_retains_two_maximum_projections_and_request_controls() {
    let working = usize::try_from(
        iroha_config::parameters::defaults::torii::FINALITY_MAX_WORKING_SET_BYTES.0,
    )
    .unwrap();
    let limits = lease_source_limits(working);
    let source_bytes = limits.journal_bytes + limits.allocated_bytes;
    let controls = crate::query_fanout_fixed_overhead_bytes().unwrap();
    let required = source_bytes + controls + 2 * MAX_SNS_LEASE_PROOF_BYTES_V1;
    assert_eq!(limits.allocated_bytes, 240_000_000);
    assert_eq!(working - source_bytes, 76_891_136);
    assert!(required <= working);

    for capacity in [required - 1, required] {
        let budget = AllocationBudget::new(capacity);
        let source = budget.try_reserve_bytes(source_bytes).unwrap();
        let control = budget.try_reserve_bytes(controls).unwrap();
        let original =
            iroha_allocation::ChargedBuffer::<u8>::new(MAX_SNS_LEASE_PROOF_BYTES_V1, &budget)
                .unwrap();
        let response =
            iroha_allocation::ChargedBuffer::<u8>::new(MAX_SNS_LEASE_PROOF_BYTES_V1, &budget);
        assert_eq!(response.is_ok(), capacity == required);
        assert!(budget.peak_reserved_bytes() <= capacity);
        drop((response, original, control, source));
        assert_eq!(budget.reserved_bytes(), 0);
    }
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
    let view = chain.state().view();
    // Two genuine 10 MiB signed payments remain the current cut and its parent.
    // The generic 48 MB query split permanently refuses this valid original.
    assert_eq!(chain.height(), 3);
    assert!(current_lease_tip(&view, 3, lease_source_limits(48_000_000), &budget).is_err());
    let original_limits = limits.decode_limits().unwrap();
    let old_limit = norito::core::DecodeBudgetContext::new(original_limits);
    assert!(
        old_limit
            .with(|| crate::native_projection_response::current_global_tip(
                &view,
                3,
                limits,
                unavailable
            ))
            .is_err()
    );
    let observation =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 128);
    let original = norito::core::DecodeBudgetContext::new(observation);
    let original_tip = original
        .with(|| {
            crate::native_projection_response::current_global_tip(&view, 3, limits, unavailable)
        })
        .unwrap();
    let decoded_bytes = usize::try_from(original.consumed_allocated_bytes()).unwrap();
    assert!(decoded_bytes > limits.allocated_bytes);
    let decoder = norito::core::DecodeBudgetContext::new(observation);
    let tip = decoder
        .with(|| current_lease_tip(&view, 3, limits, &budget))
        .unwrap();
    assert_eq!(tip.id(), original_tip.id());
    assert_eq!(
        decoder.consumed_allocated_bytes(),
        original.consumed_allocated_bytes(),
        "retiring physical bodies must preserve every original logical charge"
    );
    drop(original_tip);
    for logical in [decoded_bytes - 1, decoded_bytes] {
        let inherited = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            original_limits.max_sequence_elements(),
            original_limits.max_field_bytes(),
            original_limits.max_total_elements(),
            logical,
            original_limits.max_nesting_depth(),
        ));
        let result = inherited.with(|| current_lease_tip(&view, 3, limits, &budget));
        assert_eq!(
            result.is_ok(),
            logical == decoded_bytes,
            "an inherited logical owner remains cumulative across retired physical stages"
        );
    }
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "the sealed scalar cut retains no decoded graph"
    );
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
        .with_native_sns_lease_cut_v1(&tip, &selector, &budget, |world, bytes| {
            crate::native_projection_response::encode(
                &SnsLeaseProofRefV1::new(world, bytes),
                ResponseFormat::Norito,
                MAX_SNS_LEASE_PROOF_BYTES_V1,
                &budget,
            )
            .map_err(|error| error.to_string())
        })
        .unwrap();
    eprintln!(
        "SNS current-cut custody: decoded={decoded_bytes} decoder_limit={} source={} peak={} remaining={} response={}",
        limits.allocated_bytes,
        limits.journal_bytes + limits.allocated_bytes,
        budget.peak_reserved_bytes(),
        working - budget.peak_reserved_bytes(),
        body.as_ref().len(),
    );
    assert!(budget.peak_reserved_bytes() <= working);
    body.memory = Some(memory);
    let body = Bytes::from_owner(body);
    let expected_root = tip.world_state_root();
    let proof = SnsLeaseProofV1::decode_frame(&body).unwrap();
    assert_eq!(proof.record, record.encode());
    assert_eq!(
        proof.world.schema_hash,
        CoreState::native_world_schema_hash_v1().unwrap()
    );
    assert_eq!(expected_root, proof.world.root().unwrap());
    let last = body.slice(..1);
    drop(body);
    assert_eq!(pool.available_bytes(), 0);
    assert!(pool.try_acquire_parts([1]).is_none());
    assert!(budget.reserved_bytes() > 0);
    drop(last);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(pool.available_bytes(), working as u64);
}
