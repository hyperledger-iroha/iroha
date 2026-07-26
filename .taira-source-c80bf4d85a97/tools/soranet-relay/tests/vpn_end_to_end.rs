use std::sync::Arc;

use iroha_data_model::soranet::vpn::{VPN_CELL_LEN, VpnCellClassV1, VpnFlowLabelV1};
use soranet_relay::{
    config::{VpnConfig, VpnCoverTrafficConfig},
    metrics::{Metrics, VpnRuntimeState},
    vpn::VpnOverlay,
};
use tokio::io::duplex;

#[tokio::test]
async fn vpn_end_to_end_records_metrics_and_receipts() {
    let mut cfg = VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 700,
            heartbeat_ms: 5,
            max_cover_burst: 3,
            max_jitter_millis: 2,
        },
        pacing_millis: 5,
        ..VpnConfig::default()
    };
    cfg.validate().expect("vpn config should validate");

    let overlay = VpnOverlay::from_config(cfg);
    let entry_metrics = Arc::new(Metrics::new());
    entry_metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    entry_metrics.set_vpn_runtime_state(VpnRuntimeState::Active);

    let exit_metrics = Arc::new(Metrics::new());
    exit_metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    exit_metrics.set_vpn_runtime_state(VpnRuntimeState::Active);

    let circuit_id = [0xAC; 16];
    let flow_label = VpnFlowLabelV1::from_u32(7).expect("flow label");
    let mut bridge = overlay.start_bridge(Arc::clone(&entry_metrics), circuit_id, flow_label);
    bridge.set_cover_seed([0x55; 32]);

    let payloads = vec![vec![0x11; 32], vec![0x22; 8], vec![0x33; 4]];
    let total_data_bytes: u64 = payloads.iter().map(|payload| payload.len() as u64).sum();

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 16);
    let send_outcome = bridge
        .send_payloads(&mut writer, &payloads)
        .await
        .expect("payloads sent");
    assert!(
        send_outcome.cover_frames > 0,
        "cover scheduling should inject cover cells"
    );

    let exit_adapter = overlay.start_adapter(Arc::clone(&exit_metrics));
    let mut ingress_data = 0usize;
    let mut ingress_cover = 0usize;
    for _ in 0..(send_outcome.data_frames + send_outcome.cover_frames) {
        let cell = exit_adapter
            .read_ingress_frame(&mut reader)
            .await
            .expect("ingress frame");
        match cell.header.class {
            VpnCellClassV1::Data => ingress_data += 1,
            VpnCellClassV1::Cover => ingress_cover += 1,
            other => panic!("unexpected class {other:?}"),
        }
    }
    assert_eq!(ingress_data, send_outcome.data_frames);
    assert_eq!(ingress_cover, send_outcome.cover_frames);

    let entry_snapshot = entry_metrics.snapshot();
    assert_eq!(entry_snapshot.vpn_sessions, 1);
    let total_frames = (send_outcome.data_frames + send_outcome.cover_frames) as u64;
    assert_eq!(entry_snapshot.vpn_egress_frames, total_frames);
    assert_eq!(
        entry_snapshot.vpn_cover_egress_frames,
        send_outcome.cover_frames as u64
    );
    assert_eq!(
        entry_snapshot.vpn_data_egress_frames,
        send_outcome.data_frames as u64
    );
    assert_eq!(entry_snapshot.vpn_data_egress_bytes, total_data_bytes);
    assert_eq!(entry_snapshot.vpn_cover_egress_bytes, 0);
    assert_eq!(entry_snapshot.vpn_egress_bytes, total_data_bytes);

    let exit_snapshot = exit_metrics.snapshot();
    assert_eq!(exit_snapshot.vpn_ingress_frames, total_frames);
    assert_eq!(
        exit_snapshot.vpn_cover_ingress_frames,
        send_outcome.cover_frames as u64
    );
    assert_eq!(
        exit_snapshot.vpn_data_ingress_frames,
        send_outcome.data_frames as u64
    );
    assert_eq!(exit_snapshot.vpn_data_ingress_bytes, total_data_bytes);
    assert_eq!(exit_snapshot.vpn_cover_ingress_bytes, 0);
    assert_eq!(exit_snapshot.vpn_ingress_bytes, total_data_bytes);

    let session_id_entry = [0x01; 16];
    let entry_receipt = bridge.adapter().session().finish_receipt(
        session_id_entry,
        overlay.exit_class(),
        overlay.meter_hash(),
    );
    assert_eq!(entry_receipt.egress_bytes, total_data_bytes);
    assert_eq!(entry_receipt.cover_bytes, 0);
    assert_eq!(entry_receipt.exit_class, overlay.exit_class());
    assert_eq!(entry_receipt.meter_hash, overlay.meter_hash());

    let session_id_exit = [0x02; 16];
    let exit_receipt = exit_adapter.session().finish_receipt(
        session_id_exit,
        overlay.exit_class(),
        overlay.meter_hash(),
    );
    assert_eq!(exit_receipt.ingress_bytes, total_data_bytes);
    assert_eq!(exit_receipt.cover_bytes, 0);
    assert_eq!(exit_receipt.exit_class, overlay.exit_class());
    assert_eq!(exit_receipt.meter_hash, overlay.meter_hash());
}
