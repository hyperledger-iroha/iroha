//! Actual maximum signed-payment frames through the native certified interval reader.

#[path = "maximum_payment_block.rs"]
mod block_boundary;

use super::*;
use crate::sumeragi::test_chain::native_payment::{PaymentKind, maximum_payment_chain};
use crate::test_allocations::{LiveAllocationStats, live_allocations_during};

// Diagnostic accounting only: this does not allocate a four-GiB buffer and is
// never a production grant. The independent live census reports actual owners.
const GENEROUS_TEST_GRAPH_BYTES: usize = 4 * 1024 * 1024 * 1024;
// Explicit retained-owner refusal control: this is the old ordinary 48,000,000-byte
// bodyless profile, not the dedicated native-finality default.
const ORDINARY_INHERITED_GRAPH_BYTES: usize = 33_312_848;

fn default_frame_bytes() -> usize {
    crate::sumeragi::finality::native_finality_frame_budget_bytes(
        usize::try_from(iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.0).unwrap(),
        MAX_FINALITY_BLOCK_BYTES,
        64,
    )
    .unwrap()
}

fn default_graph_lower_bound() -> usize {
    let working = usize::try_from(
        iroha_config::parameters::defaults::torii::FINALITY_MAX_WORKING_SET_BYTES.0,
    )
    .unwrap();
    let fixed = usize::try_from(
        iroha_config::parameters::defaults::torii::QUERY_FANOUT_FIXED_OVERHEAD_BYTES_V1,
    )
    .unwrap();
    // Core cannot depend on Torii's concrete Body/Bytes/request-control types.
    // Torii's source-coupled test proves its actual response controls <= this
    // existing fixed allowance. Reserving that allowance a second time gives a
    // conservative lower bound, strictly no larger than the actual native grant.
    // No new pool/cap or guessed platform-dependent control layout is introduced.
    working
        .checked_sub(default_frame_bytes())
        .unwrap()
        .checked_sub(fixed)
        .unwrap()
        .checked_sub(fixed)
        .unwrap()
}

#[inline(never)]
fn read_maximum(
    chain: &CertifiedTestChain,
    graph_bytes: usize,
) -> Result<NativeFinalityProofInterval, NativeFinalityProofIntervalError> {
    let view = chain.state().view();
    let source = NativeFinalityProofSource::new(
        view.chain_id(),
        view.network_id(),
        view.block_hashes(),
        view.kura(),
    );
    let owner = NativeFinalityProofBudget::new(
        AllocationBudget::new(default_frame_bytes()),
        AllocationBudget::new(graph_bytes),
        norito::DecodeLimits::new(
            MAX_FINALITY_BLOCK_BYTES,
            MAX_FINALITY_BLOCK_BYTES,
            usize::MAX,
            usize::MAX,
            128,
        ),
    );
    build_proof_interval(&source, &owner, &limits(3, 3), &AtomicBool::new(false))
}

#[test]
fn maximum_payment_native_finality_reports_actual_decoder_costs() {
    let chain = maximum_payment_chain(PaymentKind::Rejected);
    let ((result, live), records) =
        super::super::super::interval_budget::cost_observation::measure(|| {
            live_allocations_during(65_536, || read_maximum(&chain, GENEROUS_TEST_GRAPH_BYTES))
        });
    eprintln!("maximum native finality decoder admissions: {records:?}");
    report_live_cost(live);
    let interval = result.expect("generous explicit test custody observes the genuine native path");
    let proof = interval.proof(0).unwrap();
    eprintln!(
        "maximum native finality target wire={} portable_frame={}",
        proof.block_wire.len(),
        norito::canonical_frame_len(proof).unwrap()
    );
    assert!(proof.block_wire.len() >= 10 * 1024 * 1024);
    assert!(proof.block_wire.len() <= MAX_FINALITY_BLOCK_BYTES);
    assert!(
        records
            .iter()
            .any(|(kind, bytes)| *kind == "target" && *bytes >= 10 * 1024 * 1024)
    );
}

#[test]
fn maximum_payment_finality_fits_default_native_graph_custody() {
    let chain = maximum_payment_chain(PaymentKind::Rejected);
    let interval = read_maximum(&chain, default_graph_lower_bound())
        .expect("a conservative lower bound of default native graph custody admits the maximum signed payment");
    assert!(interval.proof(0).unwrap().block_wire.len() >= 10 * 1024 * 1024);
}

#[test]
fn maximum_successful_payment_native_finality_reports_actual_decoder_costs() {
    let chain = maximum_payment_chain(PaymentKind::Applied);
    let ((result, live), records) =
        super::super::super::interval_budget::cost_observation::measure(|| {
            live_allocations_during(65_536, || read_maximum(&chain, GENEROUS_TEST_GRAPH_BYTES))
        });
    eprintln!("maximum successful native finality decoder admissions: {records:?}");
    report_live_cost(live);
    let interval = result.expect("explicit test custody observes the genuine paid native path");
    let proof = interval.proof(0).unwrap();
    eprintln!(
        "maximum successful native finality target wire={} portable_frame={}",
        proof.block_wire.len(),
        norito::canonical_frame_len(proof).unwrap()
    );
    assert!(proof.block_wire.len() >= 10 * 1024 * 1024);
    assert!(proof.block_wire.len() <= MAX_FINALITY_BLOCK_BYTES);
    assert!(
        records
            .iter()
            .any(|(kind, bytes)| *kind == "target" && *bytes >= 10 * 1024 * 1024)
    );
}

fn report_live_cost(live: LiveAllocationStats) {
    eprintln!("maximum native finality allocator-visible live census: {live:?}");
    assert!(
        !live.incomplete,
        "fixed live census must capture every observed owner"
    );
}

#[test]
fn maximum_successful_payment_finality_fits_default_native_graph_custody() {
    let chain = maximum_payment_chain(PaymentKind::Applied);
    let interval = read_maximum(&chain, default_graph_lower_bound())
        .expect("a conservative lower bound of default native graph custody admits the maximum successful payment");
    assert!(interval.proof(0).unwrap().block_wire.len() >= 10 * 1024 * 1024);
}

#[test]
fn ordinary_inherited_graph_custody_refuses_both_maximum_payment_outcomes() {
    for kind in [PaymentKind::Rejected, PaymentKind::Applied] {
        let chain = maximum_payment_chain(kind);
        assert!(
            matches!(
                read_maximum(&chain, ORDINARY_INHERITED_GRAPH_BYTES),
                Err(NativeFinalityProofIntervalError::Deferred(_))
            ),
            "an inherited ordinary owner must not gain the new native profile"
        );
    }
}
