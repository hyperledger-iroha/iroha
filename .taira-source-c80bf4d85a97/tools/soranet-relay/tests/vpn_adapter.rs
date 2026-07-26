use std::{sync::Arc, time::Duration};

use iroha_data_model::soranet::vpn::{
    VPN_CELL_LEN, VpnCellClassV1, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1, VpnExitClassV1,
    VpnFlowLabelV1,
};
use soranet_relay::{
    config::{VpnConfig, VpnCoverTrafficConfig},
    metrics::Metrics,
    vpn::{
        CoverFrameMeta, VpnFrameIoError, VpnOverlay, VpnSession, read_frame, schedule_frames,
        send_scheduled_frames_with_adapter, write_frame,
    },
    vpn_adapter::{VpnAdapter, VpnBridge, VpnDataFrameBatch},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

#[test]
fn vpn_adapter_records_ingress_and_egress() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let session = VpnSession::from_parts(Arc::clone(&metrics));
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = VpnAdapter::new(session, overlay);

    adapter.record_ingress(512);
    adapter.record_egress(256);

    let snapshot = metrics.snapshot();
    assert_eq!(
        snapshot.vpn_sessions, 0,
        "adapter should not increment sessions"
    );
    assert_eq!(snapshot.vpn_bytes, 768);
    assert_eq!(snapshot.vpn_ingress_bytes, 512);
    assert_eq!(snapshot.vpn_egress_bytes, 256);
    assert_eq!(snapshot.vpn_data_bytes, 768);
    assert_eq!(snapshot.vpn_data_ingress_bytes, 512);
    assert_eq!(snapshot.vpn_data_egress_bytes, 256);
    assert_eq!(snapshot.vpn_cover_bytes, 0);
    assert_eq!(snapshot.vpn_cover_ingress_bytes, 0);
    assert_eq!(snapshot.vpn_cover_egress_bytes, 0);
}

#[test]
fn overlay_creates_adapter_and_increments_sessions() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        pacing_millis: 10,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 800,
            heartbeat_ms: 10,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    adapter.record_ingress(100);
    adapter.record_egress(60);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.vpn_sessions, 1);
    assert_eq!(snapshot.vpn_ingress_bytes, 100);
    assert_eq!(snapshot.vpn_egress_bytes, 60);
    assert_eq!(snapshot.vpn_bytes, 160);
    assert_eq!(snapshot.vpn_data_bytes, 160);
    assert_eq!(snapshot.vpn_data_ingress_bytes, 100);
    assert_eq!(snapshot.vpn_data_egress_bytes, 60);
    assert_eq!(snapshot.vpn_cover_bytes, 0);
}

#[tokio::test]
async fn overlay_creates_bridge_and_increments_sessions() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..Default::default()
    });
    let mut bridge = overlay.start_bridge(
        Arc::clone(&metrics),
        [0xAB; 16],
        VpnFlowLabelV1::from_u32(6).unwrap(),
    );

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 2);
    let outcome = bridge
        .send_payloads(&mut writer, &[vec![0x01, 0x02, 0x03]])
        .await
        .expect("bridge send");
    assert_eq!(1, outcome.data_frames);
    assert_eq!(0, outcome.cover_frames);

    let frame = read_frame(&overlay, &mut reader).await.expect("frame");
    assert_eq!(frame.header.sequence, 0);
    assert_eq!(frame.payload, vec![0x01, 0x02, 0x03]);

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_sessions);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(3, snapshot.vpn_egress_bytes);
    assert_eq!(3, snapshot.vpn_data_bytes);
    assert_eq!(3, snapshot.vpn_data_egress_bytes);
    assert_eq!(1, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_data_egress_frames);
    assert_eq!(0, snapshot.vpn_cover_frames);
    assert_eq!(0, snapshot.vpn_cover_bytes);
}

#[test]
fn adapter_parses_and_accounts_frames() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        padding_budget_ms: 5,
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));

    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0x99; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x77).expect("flow"),
        sequence: 5,
        ack: 0,
        padding_budget_ms: 5,
        payload_len: 0,
    };
    let payload = vec![0xCC; 10];
    let frame = VpnCellV1 { header, payload }
        .into_padded_frame()
        .expect("frame");

    let parsed_in = adapter.record_frame_ingress(&frame.bytes).expect("ingress");
    assert_eq!(10, parsed_in.payload.len());

    let parsed_out = adapter.record_frame_egress(&frame.bytes).expect("egress");
    assert_eq!(10, parsed_out.payload.len());

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_sessions);
    assert_eq!(10, snapshot.vpn_ingress_bytes);
    assert_eq!(10, snapshot.vpn_egress_bytes);
    assert_eq!(20, snapshot.vpn_bytes);
    assert_eq!(2, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(20, snapshot.vpn_data_bytes);
    assert_eq!(10, snapshot.vpn_data_ingress_bytes);
    assert_eq!(10, snapshot.vpn_data_egress_bytes);
    assert_eq!(2, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_data_ingress_frames);
    assert_eq!(1, snapshot.vpn_data_egress_frames);
    assert_eq!(0, snapshot.vpn_cover_frames);
    assert_eq!(0, snapshot.vpn_cover_bytes);
}

#[tokio::test]
async fn adapter_tracks_control_frames_separately() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let padding_budget_ms = overlay.config().padding_budget_ms;

    let ingress_cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Control,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x10; 16],
            flow_label: VpnFlowLabelV1::from_u32(8).expect("flow"),
            sequence: 1,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xAA; 12],
    };
    let ingress_frame = overlay
        .pad_cell(ingress_cell)
        .expect("control ingress frame");
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 2);
    write_frame(&mut writer, &ingress_frame)
        .await
        .expect("write control ingress frame");
    let parsed = adapter
        .read_ingress_frame(&mut reader)
        .await
        .expect("ingress parsed");
    assert_eq!(VpnCellClassV1::Control, parsed.header.class);

    let egress_cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Control,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x20; 16],
            flow_label: VpnFlowLabelV1::from_u32(9).expect("flow"),
            sequence: 2,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xBB; 5],
    };
    let _ = adapter
        .encode_egress_cell(egress_cell)
        .expect("control egress frame");

    let receipt = adapter.finish_receipt([0xAA; 16], VpnExitClassV1::Standard, [0xBB; 32]);
    assert_eq!(0, receipt.ingress_bytes);
    assert_eq!(0, receipt.egress_bytes);
    assert_eq!(0, receipt.cover_bytes);

    let snapshot = metrics.snapshot();
    assert_eq!(0, snapshot.vpn_frames);
    assert_eq!(0, snapshot.vpn_ingress_frames);
    assert_eq!(0, snapshot.vpn_egress_frames);
    assert_eq!(0, snapshot.vpn_bytes);
    assert_eq!(0, snapshot.vpn_ingress_bytes);
    assert_eq!(0, snapshot.vpn_egress_bytes);
    assert_eq!(0, snapshot.vpn_data_frames);
    assert_eq!(0, snapshot.vpn_data_bytes);
    assert_eq!(0, snapshot.vpn_cover_frames);
    assert_eq!(0, snapshot.vpn_cover_bytes);
    assert_eq!(2, snapshot.vpn_control_frames);
    assert_eq!(1, snapshot.vpn_control_ingress_frames);
    assert_eq!(1, snapshot.vpn_control_egress_frames);
    assert_eq!(17, snapshot.vpn_control_bytes);
    assert_eq!(12, snapshot.vpn_control_ingress_bytes);
    assert_eq!(5, snapshot.vpn_control_egress_bytes);
}

#[test]
fn adapter_counts_frame_only_hooks() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = overlay.start_adapter(Arc::clone(&metrics));

    adapter.record_ingress_frame_count(1200, false);
    adapter.record_egress_frame_count(64, true);

    let snapshot = metrics.snapshot();
    assert_eq!(2, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(1200 + 64, snapshot.vpn_bytes);
    assert_eq!(1200, snapshot.vpn_ingress_bytes);
    assert_eq!(64, snapshot.vpn_egress_bytes);
    assert_eq!(1, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_data_ingress_frames);
    assert_eq!(1200, snapshot.vpn_data_bytes);
    assert_eq!(1200, snapshot.vpn_data_ingress_bytes);
    assert_eq!(0, snapshot.vpn_data_egress_bytes);
    assert_eq!(1, snapshot.vpn_cover_frames);
    assert_eq!(1, snapshot.vpn_cover_egress_frames);
    assert_eq!(64, snapshot.vpn_cover_bytes);
    assert_eq!(64, snapshot.vpn_cover_egress_bytes);
}

#[test]
fn adapter_encodes_egress_cell() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = overlay.start_adapter(Arc::clone(&metrics));

    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0xAA; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x42).expect("flow"),
        sequence: 99,
        ack: 0,
        padding_budget_ms: 5,
        payload_len: 0,
    };
    let payload = vec![0xEE; 20];
    let cell = VpnCellV1 { header, payload };

    let frame = adapter.encode_egress_cell(cell).expect("encoded frame");
    assert_eq!(
        frame.frame.as_ref().len(),
        iroha_data_model::soranet::vpn::VPN_CELL_LEN
    );
    let parsed = overlay.parse_frame(frame.frame.as_ref()).expect("parsed");
    assert_eq!(parsed.payload.len(), 20);

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(20, snapshot.vpn_bytes);
    assert_eq!(20, snapshot.vpn_egress_bytes);
    assert_eq!(20, snapshot.vpn_data_bytes);
    assert_eq!(20, snapshot.vpn_data_egress_bytes);
    assert_eq!(1, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_data_egress_frames);
    assert_eq!(0, snapshot.vpn_cover_bytes);
    assert_eq!(0, snapshot.vpn_cover_frames);
}

#[test]
fn session_can_emit_receipt_with_cover_bytes() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        padding_budget_ms: 5,
        ..Default::default()
    });
    let session = overlay.start_session(Arc::clone(&metrics));

    // Data cell ingress
    let data_header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0x01; 16],
        flow_label: VpnFlowLabelV1::from_u32(3).expect("flow"),
        sequence: 1,
        ack: 0,
        padding_budget_ms: 5,
        payload_len: 0,
    };
    let data_frame = VpnCellV1 {
        header: data_header,
        payload: vec![0xAA; 8],
    }
    .into_padded_frame()
    .expect("data frame");
    let _ = session
        .record_frame_ingress(&overlay, &data_frame.bytes)
        .expect("parsed data ingress");

    // Cover cell egress
    let cover_header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Cover,
        flags: VpnCellFlagsV1::new(true, false, false, false),
        circuit_id: [0x02; 16],
        flow_label: VpnFlowLabelV1::from_u32(4).expect("flow"),
        sequence: 2,
        ack: 0,
        padding_budget_ms: 5,
        payload_len: 0,
    };
    let cover_frame = VpnCellV1 {
        header: cover_header,
        payload: vec![0xBB; 6],
    }
    .into_padded_frame()
    .expect("cover frame");
    let _ = session
        .record_frame_egress(&overlay, &cover_frame.bytes)
        .expect("parsed cover egress");

    let receipt = session.finish_receipt([0x11; 16], VpnExitClassV1::Standard, [0x22; 32]);
    assert_eq!(8, receipt.ingress_bytes);
    assert_eq!(6, receipt.cover_bytes);
    assert_eq!(6, receipt.egress_bytes);
    assert_eq!(VpnExitClassV1::Standard, receipt.exit_class);
    assert!(receipt.uptime_secs <= 1);

    let snapshot = metrics.snapshot();
    assert_eq!(2, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_cover_frames);
    assert_eq!(8, snapshot.vpn_data_ingress_bytes);
    assert_eq!(6, snapshot.vpn_cover_egress_bytes);
}

#[test]
fn adapter_emits_receipt() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));

    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0xAA; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x42).expect("flow"),
        sequence: 1,
        ack: 0,
        padding_budget_ms: overlay.config().padding_budget_ms,
        payload_len: 0,
    };
    let frame = VpnCellV1 {
        header,
        payload: vec![0xEE; 10],
    }
    .into_padded_frame()
    .expect("frame");

    adapter
        .record_frame_ingress(frame.as_ref())
        .expect("ingress parsed");

    let receipt = adapter.finish_receipt([0xAA; 16], VpnExitClassV1::LowLatency, [0x55; 32]);
    assert_eq!(10, receipt.ingress_bytes);
    assert_eq!(0, receipt.cover_bytes);
    assert_eq!(VpnExitClassV1::LowLatency, receipt.exit_class);
}

#[test]
fn overlay_binds_session_receipt_metadata() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let mut cfg = VpnConfig {
        enabled: true,
        exit_class: "high-security".to_string(),
        ..Default::default()
    };
    cfg.billing.meter_hash_hex = hex::encode([0x44u8; 32]);
    let overlay = VpnOverlay::from_config(cfg);
    let session = overlay.start_session(Arc::clone(&metrics));
    let handle = overlay.bind_session(session, [0xCC; 16]);

    let receipt = handle.receipt();
    assert_eq!(VpnExitClassV1::HighSecurity, receipt.exit_class);
    assert_eq!([0x44u8; 32], receipt.meter_hash);
    assert_eq!([0xCC; 16], receipt.session_id);
}

#[test]
fn adapter_encapsulates_data_cell() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = overlay.start_adapter(Arc::clone(&metrics));

    let padded = adapter
        .encapsulate_data_cell(
            [0xDA; 16],
            VpnFlowLabelV1::from_u32(5).expect("flow"),
            42,
            0,
            VpnCellFlagsV1::new(false, false, false, false),
            vec![0xFF; 8],
        )
        .expect("encapsulated");
    let parsed = overlay
        .parse_frame(padded.frame.as_ref())
        .expect("parsed frame");
    assert_eq!(parsed.payload, vec![0xFF; 8]);
    assert_eq!(42, parsed.header.sequence);

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(8, snapshot.vpn_bytes);
    assert_eq!(8, snapshot.vpn_egress_bytes);
    assert_eq!(8, snapshot.vpn_data_bytes);
    assert_eq!(8, snapshot.vpn_data_egress_bytes);
    assert_eq!(1, snapshot.vpn_data_frames);
}

#[tokio::test]
async fn adapter_stream_round_trip_records_metrics() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..VpnConfig::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let padding_budget_ms = overlay.config().padding_budget_ms;

    let cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x99; 16],
            flow_label: VpnFlowLabelV1::from_u32(7).expect("flow"),
            sequence: 3,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xAB; 10],
    };

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 2);
    adapter
        .write_egress_frame(&mut writer, cell.clone())
        .await
        .expect("write frame");
    let parsed = adapter
        .read_ingress_frame(&mut reader)
        .await
        .expect("read frame");

    assert_eq!(cell.payload, parsed.payload);
    assert_eq!(padding_budget_ms, parsed.header.padding_budget_ms);

    let snapshot = metrics.snapshot();
    assert_eq!(2, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(20, snapshot.vpn_bytes);
    assert_eq!(10, snapshot.vpn_ingress_bytes);
    assert_eq!(10, snapshot.vpn_egress_bytes);
    assert_eq!(20, snapshot.vpn_data_bytes);
    assert_eq!(10, snapshot.vpn_data_ingress_bytes);
    assert_eq!(10, snapshot.vpn_data_egress_bytes);
    assert_eq!(2, snapshot.vpn_data_frames);
    assert_eq!(1, snapshot.vpn_data_ingress_frames);
    assert_eq!(1, snapshot.vpn_data_egress_frames);
    assert_eq!(0, snapshot.vpn_cover_bytes);
}

#[tokio::test]
async fn adapter_rejects_truncated_ingress_frame() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..VpnConfig::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let padding_budget_ms = overlay.config().padding_budget_ms;
    let padded = overlay
        .pad_cell(VpnCellV1 {
            header: VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, false, false, false),
                circuit_id: [0x01; 16],
                flow_label: VpnFlowLabelV1::from_u32(12).expect("flow"),
                sequence: 5,
                ack: 0,
                padding_budget_ms,
                payload_len: 0,
            },
            payload: vec![0xCC; 4],
        })
        .expect("padded cell");

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN / 2);
    writer
        .write_all(&padded.frame.as_ref()[..64])
        .await
        .expect("partial write");
    drop(writer);

    let err = adapter
        .read_ingress_frame(&mut reader)
        .await
        .expect_err("truncated frame should fail");
    assert!(matches!(
        err,
        VpnFrameIoError::FrameLength { expected, actual }
        if expected == VPN_CELL_LEN && actual < VPN_CELL_LEN
    ));

    let snapshot = metrics.snapshot();
    assert_eq!(0, snapshot.vpn_frames);
    assert_eq!(0, snapshot.vpn_bytes);
}

#[tokio::test]
async fn adapter_sends_data_frames_with_accounting() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(Default::default());
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let flow = VpnFlowLabelV1::from_u32(9).expect("flow");
    let flags = VpnCellFlagsV1::new(false, false, false, false);
    let batch = VpnDataFrameBatch {
        circuit_id: [0xAC; 16],
        flow_label: flow,
        start_sequence: 10,
        ack: 0,
        flags,
        payloads: &[vec![1u8; 4], vec![2u8; 6]],
    };

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 16);
    adapter
        .send_data_frames(&mut writer, batch)
        .await
        .expect("sent frames");

    let first = read_frame(&overlay, &mut reader)
        .await
        .expect("first frame");
    let second = read_frame(&overlay, &mut reader)
        .await
        .expect("second frame");
    assert_eq!(first.header.sequence, 10);
    assert_eq!(second.header.sequence, 11);
    assert_eq!(first.payload, vec![1u8; 4]);
    assert_eq!(second.payload, vec![2u8; 6]);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.vpn_frames, 2);
    assert_eq!(snapshot.vpn_egress_frames, 2);
    assert_eq!(snapshot.vpn_bytes, 10);
    assert_eq!(snapshot.vpn_egress_bytes, 10);
    assert_eq!(snapshot.vpn_data_bytes, 10);
    assert_eq!(snapshot.vpn_data_egress_bytes, 10);
    assert_eq!(snapshot.vpn_data_frames, 2);
}

#[tokio::test]
async fn adapter_paces_data_frames_and_cover() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 900,
            heartbeat_ms: 10,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let flow = VpnFlowLabelV1::from_u32(4).expect("flow");
    let cover_meta = CoverFrameMeta {
        circuit_id: [0xCC; 16],
        flow_label: flow,
        ack: 0,
        flags: VpnCellFlagsV1::new(true, false, false, false),
        start_sequence: 50,
    };
    let payloads = &[vec![0xAB; 2], vec![0xCD; 3]];
    let mut data_cells = Vec::new();
    for (idx, payload) in payloads.iter().enumerate() {
        data_cells.push(
            overlay
                .data_cell(
                    [0xCC; 16],
                    flow,
                    10 + idx as u64,
                    0,
                    VpnCellFlagsV1::new(false, false, false, false),
                    payload.clone(),
                )
                .expect("data cell"),
        );
    }
    let schedule = schedule_frames(&overlay, data_cells, cover_meta, [0x11; 32]).expect("schedule");

    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 16);
    send_scheduled_frames_with_adapter(
        &schedule,
        &mut writer,
        Some(&adapter),
        Some(adapter.session()),
    )
    .await
    .expect("paced frames sent");

    let mut frames = Vec::new();
    for _ in 0..schedule.len() {
        frames.push(read_frame(&overlay, &mut reader).await.expect("frame"));
    }
    assert!(
        frames
            .iter()
            .any(|f| f.header.class == VpnCellClassV1::Cover)
    );
    let data_sequences: Vec<u64> = frames
        .iter()
        .filter(|f| f.header.class == VpnCellClassV1::Data)
        .map(|f| f.header.sequence)
        .collect();
    assert!(data_sequences.contains(&10));
    assert!(data_sequences.contains(&11));

    let snapshot = metrics.snapshot();
    let cover_frames = schedule.iter().filter(|frame| frame.is_cover).count() as u64;
    let data_frames = schedule.len() as u64 - cover_frames;
    assert_eq!(snapshot.vpn_egress_frames, schedule.len() as u64);
    assert_eq!(snapshot.vpn_data_frames, data_frames);
    assert_eq!(snapshot.vpn_cover_frames, cover_frames);
    assert_eq!(snapshot.vpn_data_bytes, 5);
    assert_eq!(snapshot.vpn_cover_bytes, 0);
}

#[tokio::test]
async fn bridge_sends_data_without_cover() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let mut bridge = VpnBridge::new(adapter, [0x10; 16], VpnFlowLabelV1::from_u32(1).unwrap());

    let payloads = vec![vec![0xAA; 3], vec![0xBB; 2]];
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 4);
    let outcome = bridge
        .send_payloads(&mut writer, &payloads)
        .await
        .expect("bridge send");
    assert_eq!(2, outcome.data_frames);
    assert_eq!(0, outcome.cover_frames);

    let first = read_frame(&overlay, &mut reader)
        .await
        .expect("first frame");
    let second = read_frame(&overlay, &mut reader)
        .await
        .expect("second frame");
    assert_eq!(0, first.header.sequence);
    assert_eq!(1, second.header.sequence);
    assert_eq!(payloads[0], first.payload);
    assert_eq!(payloads[1], second.payload);

    let snapshot = metrics.snapshot();
    assert_eq!(2, snapshot.vpn_egress_frames);
    assert_eq!(
        (payloads[0].len() + payloads[1].len()) as u64,
        snapshot.vpn_egress_bytes
    );
    assert_eq!(
        (payloads[0].len() + payloads[1].len()) as u64,
        snapshot.vpn_data_bytes
    );
    assert_eq!(2, snapshot.vpn_data_frames);
    assert_eq!(0, snapshot.vpn_cover_frames);
}

#[tokio::test]
async fn bridge_injects_cover_when_enabled() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 900,
            heartbeat_ms: 10,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let mut bridge = VpnBridge::new(adapter, [0x21; 16], VpnFlowLabelV1::from_u32(2).unwrap());
    bridge.set_ack(7);
    bridge.set_cover_seed([0x55; 32]);

    let payloads = vec![vec![0xCA; 4], vec![0xFE; 5]];
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 6);
    let outcome = bridge
        .send_payloads(&mut writer, &payloads)
        .await
        .expect("bridge send");
    assert_eq!(payloads.len(), outcome.data_frames);
    assert!(outcome.cover_frames > 0);

    let mut frames = Vec::new();
    for _ in 0..(outcome.data_frames + outcome.cover_frames) {
        frames.push(read_frame(&overlay, &mut reader).await.expect("frame"));
    }
    let data_sequences: Vec<u64> = frames
        .iter()
        .filter(|f| f.header.class == VpnCellClassV1::Data)
        .map(|f| f.header.sequence)
        .collect();
    assert_eq!(vec![0, 1], data_sequences);
    let cover_sequences: Vec<u64> = frames
        .iter()
        .filter(|f| f.header.class == VpnCellClassV1::Cover)
        .map(|f| f.header.sequence)
        .collect();
    assert!(
        cover_sequences.windows(2).all(|pair| pair[1] > pair[0]),
        "cover sequences should increase"
    );
    assert!(
        frames
            .iter()
            .all(|f| f.header.ack == 7
                && f.header.flow_label == VpnFlowLabelV1::from_u32(2).unwrap())
    );

    let snapshot = metrics.snapshot();
    assert_eq!(
        (outcome.data_frames + outcome.cover_frames) as u64,
        snapshot.vpn_egress_frames
    );
    assert_eq!(outcome.data_frames as u64, snapshot.vpn_data_frames);
    assert_eq!(outcome.cover_frames as u64, snapshot.vpn_cover_frames);
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.len() as u64)
            .sum::<u64>(),
        snapshot.vpn_data_bytes
    );
    assert_eq!(0, snapshot.vpn_cover_bytes);
}

#[tokio::test]
async fn relay_metrics_cover_and_data_end_to_end() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 700,
            heartbeat_ms: 5,
            max_cover_burst: 1,
            max_jitter_millis: 0,
        },
        pacing_millis: 5,
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let mut bridge = VpnBridge::new(
        adapter.clone(),
        [0x99; 16],
        VpnFlowLabelV1::from_u32(13).unwrap(),
    );
    bridge.set_cover_seed([0xEE; 32]);

    let payloads = vec![vec![0xAB; 5], vec![0xCD; 7]];
    let (mut relay_io, mut exit_io) = duplex(VPN_CELL_LEN * 8);

    let outcome = bridge
        .send_payloads(&mut relay_io, &payloads)
        .await
        .expect("payload send");
    let outbound_frames = outcome.data_frames + outcome.cover_frames;

    let mut data_bytes = 0u64;
    let mut cover_bytes = 0u64;
    for _ in 0..outbound_frames {
        let cell = read_frame(&overlay, &mut exit_io)
            .await
            .expect("frame from relay");
        match cell.header.class {
            VpnCellClassV1::Data => data_bytes += cell.payload.len() as u64,
            VpnCellClassV1::Cover => cover_bytes += cell.payload.len() as u64,
            VpnCellClassV1::KeepAlive | VpnCellClassV1::Control => {
                panic!("unexpected control-plane frame {cell:?}")
            }
        }
    }

    let response_payload = vec![0xFA; 9];
    let response = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x88; 16],
            flow_label: VpnFlowLabelV1::from_u32(13).unwrap(),
            sequence: 25,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: response_payload.clone(),
    };
    let padded = overlay.pad_cell(response).expect("pad response");
    write_frame(&mut exit_io, &padded)
        .await
        .expect("write response");

    let parsed = adapter
        .read_ingress_frame(&mut relay_io)
        .await
        .expect("ingress response");
    assert_eq!(parsed.payload, response_payload);

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_sessions);
    assert_eq!(
        outcome.data_frames as u64 + outcome.cover_frames as u64,
        snapshot.vpn_egress_frames
    );
    assert_eq!(outcome.data_frames as u64, snapshot.vpn_data_egress_frames);
    assert_eq!(
        outcome.cover_frames as u64,
        snapshot.vpn_cover_egress_frames
    );
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_data_ingress_frames);
    assert_eq!(0, snapshot.vpn_cover_ingress_frames);
    assert_eq!(
        data_bytes + cover_bytes + response_payload.len() as u64,
        snapshot.vpn_bytes
    );
    assert_eq!(
        data_bytes + response_payload.len() as u64,
        snapshot.vpn_data_bytes
    );
    assert_eq!(data_bytes, snapshot.vpn_data_egress_bytes);
    assert_eq!(
        response_payload.len() as u64,
        snapshot.vpn_data_ingress_bytes
    );
    assert_eq!(cover_bytes, snapshot.vpn_cover_bytes);
    assert_eq!(cover_bytes, snapshot.vpn_cover_egress_bytes);
}

#[tokio::test]
async fn bridge_fragments_large_payload() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    });
    let adapter = overlay.start_adapter(Arc::clone(&metrics));
    let mut bridge = VpnBridge::new(adapter, [0x33; 16], VpnFlowLabelV1::from_u32(3).unwrap());

    let max_payload = VpnCellV1::max_payload_len();
    let buffer = vec![0xDE; max_payload + 5];
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 4);
    let outcome = bridge
        .send_buffer(&mut writer, &buffer)
        .await
        .expect("bridge send");
    assert_eq!(2, outcome.data_frames);
    assert_eq!(0, outcome.cover_frames);

    let mut frames = Vec::new();
    for _ in 0..outcome.data_frames {
        frames.push(read_frame(&overlay, &mut reader).await.expect("frame"));
    }
    assert_eq!(frames[0].payload.len(), max_payload);
    assert_eq!(frames[1].payload.len(), 5);
    assert_eq!(frames[0].header.sequence, 0);
    assert_eq!(frames[1].header.sequence, 1);
    assert_eq!(frames[0].payload[0], 0xDE);
    assert_eq!(frames[1].payload[0], 0xDE);

    let snapshot = metrics.snapshot();
    assert_eq!(2, snapshot.vpn_egress_frames);
    assert_eq!((max_payload as u64 + 5), snapshot.vpn_egress_bytes);
    assert_eq!((max_payload as u64 + 5), snapshot.vpn_data_bytes);
    assert_eq!(2, snapshot.vpn_data_frames);
}

#[tokio::test]
async fn bridge_pumps_tun_to_vpn() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        cover: VpnCoverTrafficConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    });
    let mut bridge = overlay.start_bridge(
        Arc::clone(&metrics),
        [0x44; 16],
        VpnFlowLabelV1::from_u32(4).unwrap(),
    );

    let (mut tun_writer, mut tun_reader) = duplex(2048);
    let (mut vpn_writer, mut vpn_reader) = duplex(VPN_CELL_LEN * 4);
    let pump = tokio::spawn(async move {
        bridge
            .pump_tun_to_vpn(&mut tun_reader, &mut vpn_writer, 512)
            .await
    });

    let payload = vec![0xAA; 40];
    tun_writer.write_all(&payload).await.expect("tun write");
    drop(tun_writer);

    let outcome = pump.await.expect("pump join").expect("pump result");
    assert_eq!(1, outcome.data_frames);
    assert_eq!(0, outcome.cover_frames);

    let frame = read_frame(&overlay, &mut vpn_reader).await.expect("frame");
    assert_eq!(payload, frame.payload);
    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_egress_frames);
    assert_eq!(payload.len() as u64, snapshot.vpn_egress_bytes);
    assert_eq!(payload.len() as u64, snapshot.vpn_data_bytes);
    assert_eq!(1, snapshot.vpn_data_frames);
}

#[tokio::test]
async fn bridge_forwards_vpn_to_tun() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..Default::default()
    });
    let bridge = overlay.start_bridge(
        Arc::clone(&metrics),
        [0x55; 16],
        VpnFlowLabelV1::from_u32(5).unwrap(),
    );

    let payload = vec![0xBC; 12];
    let padded = overlay
        .pad_cell(VpnCellV1 {
            header: VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, false, false, false),
                circuit_id: [0x55; 16],
                flow_label: VpnFlowLabelV1::from_u32(5).unwrap(),
                sequence: 0,
                ack: 0,
                padding_budget_ms: overlay.config().padding_budget_ms,
                payload_len: 0,
            },
            payload: payload.clone(),
        })
        .expect("padded");

    let (mut vpn_writer, mut vpn_reader) = duplex(VPN_CELL_LEN * 2);
    vpn_writer
        .write_all(padded.frame.as_ref())
        .await
        .expect("write frame");
    drop(vpn_writer);

    let (mut tun_writer, mut tun_reader) = duplex(2048);
    tokio::time::timeout(
        Duration::from_secs(5),
        bridge.forward_vpn_to_tun(&mut vpn_reader, &mut tun_writer),
    )
    .await
    .expect("vpn->tun forward timed out")
    .expect("forward");
    drop(tun_writer);
    let mut received = Vec::new();
    tun_reader
        .read_to_end(&mut received)
        .await
        .expect("read tun");
    assert_eq!(payload, received);

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_data_ingress_frames);
    assert_eq!(payload.len() as u64, snapshot.vpn_data_ingress_bytes);
}

#[tokio::test]
async fn bridge_drops_cover_frames_on_tun_forward() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        ..Default::default()
    });
    let bridge = overlay.start_bridge(
        Arc::clone(&metrics),
        [0x77; 16],
        VpnFlowLabelV1::from_u32(9).unwrap(),
    );

    let payload = vec![0xAD; 7];
    let padded = overlay
        .pad_cell(VpnCellV1 {
            header: VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Cover,
                flags: VpnCellFlagsV1::new(true, false, false, false),
                circuit_id: [0x77; 16],
                flow_label: VpnFlowLabelV1::from_u32(9).unwrap(),
                sequence: 0,
                ack: 0,
                padding_budget_ms: overlay.config().padding_budget_ms,
                payload_len: 0,
            },
            payload: payload.clone(),
        })
        .expect("padded cover");

    let (mut vpn_writer, mut vpn_reader) = duplex(VPN_CELL_LEN * 2);
    vpn_writer
        .write_all(padded.frame.as_ref())
        .await
        .expect("write cover frame");
    drop(vpn_writer);

    let (mut tun_writer, mut tun_reader) = duplex(2048);
    tokio::time::timeout(
        Duration::from_secs(5),
        bridge.forward_vpn_to_tun(&mut vpn_reader, &mut tun_writer),
    )
    .await
    .expect("vpn->tun forward timed out")
    .expect("forward");
    drop(tun_writer);

    let mut received = Vec::new();
    tun_reader
        .read_to_end(&mut received)
        .await
        .expect("read tun");
    assert!(received.is_empty(), "cover payload should be dropped");

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_cover_ingress_frames);
    assert_eq!(0, snapshot.vpn_data_ingress_frames);
    assert_eq!(payload.len() as u64, snapshot.vpn_cover_ingress_bytes);
    assert_eq!(0, snapshot.vpn_data_ingress_bytes);
}

#[tokio::test]
async fn bridge_and_adapter_end_to_end_record_metrics() {
    let entry_metrics = Arc::new(Metrics::new());
    entry_metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let exit_metrics = Arc::new(Metrics::new());
    exit_metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");

    let make_cfg = || VpnConfig {
        enabled: true,
        pacing_millis: 1,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 999,
            heartbeat_ms: 0,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        ..VpnConfig::default()
    };
    let overlay_entry = VpnOverlay::from_config(make_cfg());
    let overlay_exit = VpnOverlay::from_config(make_cfg());

    let mut bridge = overlay_entry.start_bridge(
        Arc::clone(&entry_metrics),
        [0xCD; 16],
        VpnFlowLabelV1::from_u32(12).expect("flow"),
    );
    bridge.set_cover_seed([0x11; 32]);
    let adapter_exit = overlay_exit.start_adapter(Arc::clone(&exit_metrics));

    let payloads = vec![vec![0x01; 4], vec![0xAA; 6]];
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 8);
    let outcome = bridge
        .send_payloads(&mut writer, &payloads)
        .await
        .expect("send payloads");
    let total_frames = outcome.data_frames + outcome.cover_frames;
    assert!(
        outcome.cover_frames > 0,
        "cover schedule should inject at least one cover frame"
    );

    let mut received_payloads = Vec::new();
    for _ in 0..total_frames {
        let cell = adapter_exit
            .read_ingress_frame(&mut reader)
            .await
            .expect("ingress frame");
        if cell.header.class == VpnCellClassV1::Data {
            received_payloads.push(cell.payload);
        } else {
            assert_eq!(cell.header.class, VpnCellClassV1::Cover);
        }
    }
    assert_eq!(payloads, received_payloads);

    let sent_bytes: u64 = payloads.iter().map(|p| p.len() as u64).sum();
    let entry_snapshot = entry_metrics.snapshot();
    assert_eq!(entry_snapshot.vpn_sessions, 1);
    assert_eq!(entry_snapshot.vpn_egress_frames, total_frames as u64);
    assert_eq!(
        entry_snapshot.vpn_data_egress_frames,
        outcome.data_frames as u64
    );
    assert_eq!(
        entry_snapshot.vpn_cover_egress_frames,
        outcome.cover_frames as u64
    );
    assert_eq!(entry_snapshot.vpn_data_egress_bytes, sent_bytes);
    assert_eq!(entry_snapshot.vpn_cover_egress_bytes, 0);

    let exit_snapshot = exit_metrics.snapshot();
    assert_eq!(exit_snapshot.vpn_sessions, 1);
    assert_eq!(exit_snapshot.vpn_ingress_frames, total_frames as u64);
    assert_eq!(
        exit_snapshot.vpn_data_ingress_frames,
        outcome.data_frames as u64
    );
    assert_eq!(
        exit_snapshot.vpn_cover_ingress_frames,
        outcome.cover_frames as u64
    );
    assert_eq!(exit_snapshot.vpn_data_ingress_bytes, sent_bytes);
    assert_eq!(exit_snapshot.vpn_cover_ingress_bytes, 0);
}
