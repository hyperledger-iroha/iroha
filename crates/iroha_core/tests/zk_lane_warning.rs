#![doc = "ZK lane reporting: background verification emits a non-forking pipeline warning."]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]
//! ZK lane reporting: background verification emits a non-forking pipeline warning.

use std::{num::NonZeroU64, sync::Arc};

use iroha_core::pipeline::zk_lane;
use iroha_crypto::streaming::TransportCapabilityResolutionSnapshot;
use iroha_data_model::events::pipeline::PipelineEventBox;
use norito::streaming::{CapabilityFlags, HpkeSuite, PrivacyBucketGranularity};

#[tokio::test]
async fn zk_lane_emits_warning_on_rejected_trace() {
    // Register a local events sender to capture warnings
    let (tx, mut rx) = tokio::sync::broadcast::channel::<iroha_data_model::events::EventBox>(16);
    zk_lane::register_events_sender(tx.clone());

    // Start the ZK lane (enabled=true). Use a tiny batch size to flush quickly.
    let cfg = iroha_config::parameters::actual::Halo2 {
        enabled: true,
        curve: iroha_config::parameters::actual::ZkCurve::Pasta,
        backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 10,
        verifier_max_batch: 2,
        ..iroha_config::parameters::actual::Halo2::default()
    };
    let _ = zk_lane::start(&cfg);

    // Build a task whose constraint fails: gpr[0] = 1 but requires zero at cycle 0
    let mut gpr = [0u64; 256];
    gpr[0] = 1;
    let trace = vec![ivm::zk::RegisterState {
        pc: 0,
        gpr,
        tags: [false; 256],
    }];
    let constraints = vec![ivm::zk::Constraint::Zero { reg: 0, cycle: 0 }];
    let job = zk_lane::ZkTask {
        tx_hash: None,
        code_hash: [0xCD; 32],
        program: Arc::new(vec![0x01, 0x00, 0x00, 0x00]),
        header: Some(iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(5).unwrap(),
            None,
            None,
            None,
            0,
            0,
        )),
        trace,
        constraints,
        mem_log: Vec::new(),
        reg_log: Vec::new(),
        step_log: Vec::new(),
        transport_capabilities: None,
        negotiated_capabilities: None,
    };
    assert!(
        zk_lane::try_submit(job),
        "zk lane must accept tasks when started"
    );

    // Expect a Pipeline::Warning with kind = "zk_trace_rejected" soon
    use tokio::time::{Duration, Instant, sleep};
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut warned = false;
    while Instant::now() < deadline {
        if let Ok(ev) = rx.try_recv() {
            if let iroha_data_model::events::EventBox::Pipeline(pb) = ev {
                if let PipelineEventBox::Warning(w) = pb {
                    if w.kind == "zk_trace_rejected" {
                        warned = true;
                        break;
                    }
                }
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(warned, "expected a pipeline warning for rejected ZK trace");
}

#[test]
fn zk_task_digest_reflects_transport_metadata() {
    let trace = vec![ivm::zk::RegisterState {
        pc: 0,
        gpr: [0u64; 256],
        tags: [false; 256],
    }];
    let mut base = zk_lane::ZkTask {
        tx_hash: None,
        code_hash: [0xAB; 32],
        program: Arc::new(vec![0x01, 0x00, 0x00, 0x00]),
        header: None,
        trace,
        constraints: Vec::new(),
        mem_log: Vec::new(),
        reg_log: Vec::new(),
        step_log: Vec::new(),
        transport_capabilities: None,
        negotiated_capabilities: None,
    };
    let base_digest = base.digest();

    let snapshot = TransportCapabilityResolutionSnapshot {
        hpke_suite: HpkeSuite::Kyber768AuthPsk,
        use_datagram: true,
        max_segment_datagram_size: 1350,
        fec_feedback_interval_ms: 25,
        privacy_bucket_granularity: PrivacyBucketGranularity::StandardV1,
    };
    let mut with_transport = base.clone();
    with_transport.transport_capabilities = Some(snapshot);
    assert_ne!(base_digest, with_transport.digest());

    let mut with_flags = base.clone();
    with_flags.negotiated_capabilities = Some(CapabilityFlags::from_bits(0b101));
    assert_ne!(base_digest, with_flags.digest());

    // Ensure both metadata fields together still produce deterministic digests.
    let mut combined = base.clone();
    combined.transport_capabilities = with_transport.transport_capabilities.clone();
    combined.negotiated_capabilities = with_flags.negotiated_capabilities;
    assert_ne!(base_digest, combined.digest());
}
