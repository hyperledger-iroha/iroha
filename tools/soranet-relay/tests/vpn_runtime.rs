use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_data_model::soranet::vpn::{
    VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1, VpnFlowLabelV1,
};
use soranet_relay::{
    config::{VpnConfig, VpnCoverTrafficConfig},
    metrics::Metrics,
    vpn::{
        CoverFrameMeta, VpnFrameIoError, VpnOverlay, read_frame, schedule_frames,
        send_scheduled_frames, write_frame,
    },
};
use tokio::io::{AsyncWriteExt, duplex};

#[tokio::test]
async fn frame_io_roundtrip_validates_payload() {
    let overlay = VpnOverlay::from_config(VpnConfig::default());
    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, true, false, false),
        circuit_id: [0xAB; 16],
        flow_label: VpnFlowLabelV1::from_u32(7).expect("flow"),
        sequence: 1,
        ack: 0,
        padding_budget_ms: overlay.config().padding_budget_ms,
        payload_len: 0,
    };
    let cell = VpnCellV1 {
        header,
        payload: vec![0x11; 8],
    };
    let padded = overlay.pad_cell(cell).expect("padded frame");
    let (mut writer, mut reader) = duplex(2 * iroha_data_model::soranet::vpn::VPN_CELL_LEN);

    let write = write_frame(&mut writer, &padded);
    let read = read_frame(&overlay, &mut reader);
    let (_, parsed) = tokio::join!(write, read);

    let parsed = parsed.expect("parsed frame");
    assert_eq!(parsed.payload, vec![0x11; 8]);
    assert_eq!(parsed.header.flags.bits(), VpnCellFlagsV1::REQUIRE_ACK);
}

#[tokio::test]
async fn frame_io_rejects_invalid_class() {
    let overlay = VpnOverlay::from_config(VpnConfig::default());
    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0xCD; 16],
        flow_label: VpnFlowLabelV1::from_u32(3).expect("flow"),
        sequence: 0,
        ack: 0,
        padding_budget_ms: overlay.config().padding_budget_ms,
        payload_len: 0,
    };
    let cell = VpnCellV1 {
        header,
        payload: Vec::new(),
    };
    let padded = overlay.pad_cell(cell).expect("padded frame");
    let mut bad_bytes = padded.frame.bytes;
    bad_bytes[1] = 9;
    let (mut writer, mut reader) = duplex(2 * iroha_data_model::soranet::vpn::VPN_CELL_LEN);
    writer.write_all(&bad_bytes).await.expect("written");

    let err = read_frame(&overlay, &mut reader)
        .await
        .expect_err("invalid class should fail");
    match err {
        VpnFrameIoError::Parse(VpnCellError::InvalidClass(tag)) => assert_eq!(9, tag),
        other => panic!("unexpected error {other:?}"),
    }
}

#[tokio::test]
async fn scheduler_applies_pacing_without_cover() {
    let mut cfg = VpnConfig::default();
    cfg.cover.enabled = false;
    cfg.pacing_millis = 25;
    let pacing = cfg.pacing_millis;
    let overlay = VpnOverlay::from_config(cfg);
    let cover_meta = CoverFrameMeta {
        circuit_id: [0x01; 16],
        flow_label: VpnFlowLabelV1::from_u32(1).expect("flow"),
        ack: 0,
        flags: VpnCellFlagsV1::new(true, false, false, false),
        start_sequence: 0,
    };
    let data_cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x01; 16],
            flow_label: VpnFlowLabelV1::from_u32(1).expect("flow"),
            sequence: 0,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xAA; 4],
    };
    let schedule = schedule_frames(
        &overlay,
        vec![data_cell.clone(), data_cell],
        cover_meta,
        [0x11; 32],
    )
    .expect("schedule");
    assert_eq!(schedule.len(), 2);
    assert!(
        schedule
            .windows(2)
            .all(|pair| pair[1].deadline >= pair[0].deadline)
    );

    let (mut writer, mut reader) = duplex(2 * iroha_data_model::soranet::vpn::VPN_CELL_LEN);
    let send_schedule = schedule.clone();
    let send_handle = tokio::spawn(async move {
        send_scheduled_frames(&send_schedule, &mut writer, None)
            .await
            .expect("schedule sent");
    });
    let read_overlay = overlay.clone();
    let read_handle = tokio::spawn(async move {
        let mut arrivals = Vec::new();
        for _ in 0..2 {
            let _ = read_frame(&read_overlay, &mut reader).await.expect("frame");
            arrivals.push(Instant::now());
        }
        arrivals
    });

    send_handle.await.expect("send joined");
    let arrivals = read_handle.await.expect("read joined");
    assert_eq!(arrivals.len(), 2);
    let delta = arrivals[1]
        .checked_duration_since(arrivals[0])
        .expect("monotonic instants");
    assert!(
        delta + Duration::from_millis(5) >= Duration::from_millis(pacing),
        "pacing gap should respect configured minimum (delta: {delta:?})"
    );
}

#[tokio::test]
async fn cover_frames_are_injected_when_enabled() {
    let cfg = VpnConfig {
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 800,
            heartbeat_ms: 10,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        pacing_millis: 10,
        ..VpnConfig::default()
    };
    let overlay = VpnOverlay::from_config(cfg);
    let cover_meta = CoverFrameMeta {
        circuit_id: [0x09; 16],
        flow_label: VpnFlowLabelV1::from_u32(2).expect("flow"),
        ack: 0,
        flags: VpnCellFlagsV1::new(true, false, false, false),
        start_sequence: 10,
    };
    let data_cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x09; 16],
            flow_label: VpnFlowLabelV1::from_u32(2).expect("flow"),
            sequence: 1,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xBB; 2],
    };

    let schedule =
        schedule_frames(&overlay, vec![data_cell], cover_meta, [0x22; 32]).expect("schedule");
    assert!(
        schedule.iter().any(|frame| frame.is_cover),
        "cover frames should be present"
    );
    assert!(
        schedule
            .windows(2)
            .all(|pair| pair[1].deadline >= pair[0].deadline)
    );
}

#[tokio::test]
async fn cover_and_data_metrics_accounted_separately() {
    let cfg = VpnConfig {
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 500,
            heartbeat_ms: 10,
            max_cover_burst: 2,
            max_jitter_millis: 0,
        },
        pacing_millis: 5,
        ..VpnConfig::default()
    };
    let overlay = VpnOverlay::from_config(cfg);
    let cover_meta = CoverFrameMeta {
        circuit_id: [0x44; 16],
        flow_label: VpnFlowLabelV1::from_u32(7).expect("flow"),
        ack: 1,
        flags: VpnCellFlagsV1::new(true, false, false, false),
        start_sequence: 20,
    };
    let data_payload = vec![0xAA; 6];
    let data_cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x44; 16],
            flow_label: VpnFlowLabelV1::from_u32(7).expect("flow"),
            sequence: 2,
            ack: 1,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: data_payload.clone(),
    };
    let schedule =
        schedule_frames(&overlay, vec![data_cell], cover_meta, [0x33; 32]).expect("schedule");
    let cover_frames = schedule.iter().filter(|frame| frame.is_cover).count() as u64;
    let data_frames = schedule.len() as u64 - cover_frames;
    assert!(cover_frames > 0, "schedule should inject cover frames");

    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let session = overlay.start_session(Arc::clone(&metrics));

    let (mut writer, mut reader) = duplex(2 * iroha_data_model::soranet::vpn::VPN_CELL_LEN);
    let send_schedule = schedule.clone();
    let send = tokio::spawn(async move {
        send_scheduled_frames(&send_schedule, &mut writer, Some(&session))
            .await
            .expect("schedule sent");
    });
    let read_overlay = overlay.clone();
    let read = tokio::spawn(async move {
        let mut frames = Vec::new();
        for _ in 0..schedule.len() {
            frames.push(read_frame(&read_overlay, &mut reader).await.expect("frame"));
        }
        frames
    });
    let (_, frames) = tokio::join!(send, read);
    let frames = frames.expect("read frames");
    assert_eq!(frames.len() as u64, data_frames + cover_frames);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.vpn_sessions, 1);
    assert_eq!(snapshot.vpn_egress_frames, data_frames + cover_frames);
    assert_eq!(snapshot.vpn_data_frames, data_frames);
    assert_eq!(snapshot.vpn_cover_frames, cover_frames);
    assert_eq!(snapshot.vpn_data_bytes, data_payload.len() as u64);
    assert_eq!(snapshot.vpn_cover_bytes, 0);
    assert_eq!(snapshot.vpn_data_egress_bytes, data_payload.len() as u64);
    assert_eq!(snapshot.vpn_cover_egress_bytes, 0);
}
