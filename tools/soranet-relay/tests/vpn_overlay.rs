use std::sync::Arc;

use iroha_data_model::soranet::vpn::{
    VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
    VpnFlowLabelV1,
};
use soranet_relay::{
    config::VpnConfig,
    metrics::Metrics,
    vpn::{
        CoverFrameMeta, VpnFrameBuildError, VpnFrameIoError, VpnOverlay, read_frame, write_frame,
    },
};
use tokio::{
    io::{AsyncWriteExt, duplex},
    runtime::Runtime,
};

fn overlay() -> VpnOverlay {
    let cfg = VpnConfig {
        enabled: true,
        ..VpnConfig::default()
    };
    VpnOverlay::from_config(cfg)
}

#[test]
fn overlay_parses_valid_frame() {
    let overlay = overlay();
    let padding_budget_ms = overlay.config().padding_budget_ms;
    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, true, false, false),
        circuit_id: [0x44; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x10203).expect("flow label"),
        sequence: 1,
        ack: 0,
        padding_budget_ms,
        payload_len: 0,
    };
    let payload = vec![0xAB; 4];
    let padded = VpnCellV1 { header, payload }
        .into_padded_frame()
        .expect("padded");

    let parsed = overlay.parse_frame(&padded.bytes).expect("parsed");
    assert_eq!(VpnCellClassV1::Data, parsed.header.class);
    assert_eq!(0x10203, parsed.header.flow_label.to_u32());
    assert_eq!(4u16, parsed.header.payload_len);
    assert_eq!(vec![0xAB; 4], parsed.payload);
}

#[test]
fn overlay_rejects_short_frame() {
    let overlay = overlay();
    let short = vec![0u8; VPN_CELL_LEN - 1];
    let err = overlay.parse_frame(&short).expect_err("parse should fail");
    assert!(matches!(
        err,
        VpnCellError::FrameLengthMismatch { expected, actual }
        if expected == VPN_CELL_LEN && actual == VPN_CELL_LEN - 1
    ));
}

#[test]
fn overlay_rejects_flow_label_overflow_for_configured_width() {
    let mut cfg = VpnConfig {
        enabled: true,
        padding_budget_ms: 1,
        ..VpnConfig::default()
    };
    cfg.flow_label_bits = 8;
    let overlay = VpnOverlay::from_config(cfg);

    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0xAA; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x1FF).expect("flow label"),
        sequence: 0,
        ack: 0,
        padding_budget_ms: 1,
        payload_len: 0,
    };
    let frame = VpnCellV1 {
        header,
        payload: vec![],
    }
    .into_padded_frame()
    .expect("frame");

    let err = overlay.parse_frame(&frame.bytes).expect_err("overflow");
    assert!(matches!(
        err,
        VpnCellError::FlowLabelOverflow { max_bits: 8, .. }
    ));
}

#[test]
fn session_records_frame_ingress_and_egress() {
    let metrics = Arc::new(Metrics::new());
    metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
    let overlay = VpnOverlay::from_config(VpnConfig {
        enabled: true,
        padding_budget_ms: 5,
        ..VpnConfig::default()
    });
    let session = overlay.start_session(Arc::clone(&metrics));
    let header = VpnCellHeaderV1 {
        version: 1,
        class: VpnCellClassV1::Data,
        flags: VpnCellFlagsV1::new(false, false, false, false),
        circuit_id: [0x55; 16],
        flow_label: VpnFlowLabelV1::from_u32(0x22).expect("flow"),
        sequence: 10,
        ack: 0,
        padding_budget_ms: 5,
        payload_len: 0,
    };
    let payload = vec![0xDE; 6];
    let frame = VpnCellV1 { header, payload }
        .into_padded_frame()
        .expect("frame");

    let parsed_in = session
        .record_frame_ingress(&overlay, &frame.bytes)
        .expect("ingress parsed");
    assert_eq!(6, parsed_in.payload.len());

    let parsed_out = session
        .record_frame_egress(&overlay, &frame.bytes)
        .expect("egress parsed");
    assert_eq!(6, parsed_out.payload.len());

    let snapshot = metrics.snapshot();
    assert_eq!(1, snapshot.vpn_sessions);
    assert_eq!(6, snapshot.vpn_ingress_bytes);
    assert_eq!(6, snapshot.vpn_egress_bytes);
    assert_eq!(12, snapshot.vpn_bytes);
    assert_eq!(2, snapshot.vpn_frames);
    assert_eq!(1, snapshot.vpn_ingress_frames);
    assert_eq!(1, snapshot.vpn_egress_frames);
}

#[test]
fn overlay_rejects_padding_budget_mismatch() {
    let overlay = overlay();
    let mut frame = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x44; 16],
            flow_label: VpnFlowLabelV1::from_u32(3).expect("flow"),
            sequence: 0,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms + 1,
            payload_len: 0,
        },
        payload: vec![0xAA; 8],
    }
    .into_padded_frame()
    .expect("frame");
    frame.bytes[frame.bytes.len() - 1] = 0;

    let err = overlay
        .parse_frame(&frame.bytes)
        .expect_err("padding budget should be validated");
    assert!(matches!(err, VpnCellError::PaddingBudgetMismatch { .. }));
}

#[test]
fn overlay_rejects_cover_flag_mismatch() {
    let overlay = overlay();
    let frame = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(true, false, false, false),
            circuit_id: [0x01; 16],
            flow_label: VpnFlowLabelV1::from_u32(1).expect("flow"),
            sequence: 0,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: Vec::new(),
    }
    .into_padded_frame()
    .expect("frame");
    let err = overlay
        .parse_frame(&frame.bytes)
        .expect_err("flag mismatch");
    assert!(matches!(
        err,
        VpnCellError::FlagClassMismatch {
            class: VpnCellClassV1::Data,
            ..
        }
    ));
}

#[test]
fn overlay_rejects_cover_cell_without_cover_flag() {
    let overlay = overlay();
    let cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Cover,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0x10; 16],
            flow_label: VpnFlowLabelV1::from_u32(2).expect("flow"),
            sequence: 0,
            ack: 0,
            padding_budget_ms: overlay.config().padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xAB; 4],
    };

    let err = overlay
        .pad_cell(cell)
        .expect_err("cover flag should be required");
    assert!(matches!(
        err,
        VpnFrameBuildError::Cell(VpnCellError::FlagClassMismatch {
            class: VpnCellClassV1::Cover,
            ..
        })
    ));
}

#[test]
fn overlay_rejects_cover_frame_with_unknown_flags() {
    let overlay = overlay();
    let meta = CoverFrameMeta {
        circuit_id: [0x77; 16],
        flow_label: VpnFlowLabelV1::from_u32(5).expect("flow"),
        ack: 0,
        flags: VpnCellFlagsV1::from_bits(0x80),
        start_sequence: 0,
    };

    let err = overlay
        .cover_frame(&meta, 1)
        .expect_err("invalid flags should be rejected");
    assert!(matches!(
        err,
        VpnFrameBuildError::Cell(VpnCellError::InvalidFlags { bits, .. }) if bits & 0x80 != 0
    ));
}

#[test]
fn overlay_read_write_round_trip() {
    let overlay = overlay();
    let padding_budget_ms = overlay.config().padding_budget_ms;
    let cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, true, false, false),
            circuit_id: [0xAB; 16],
            flow_label: VpnFlowLabelV1::from_u32(5).expect("flow"),
            sequence: 7,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![1, 2, 3, 4],
    };
    let padded = overlay.pad_cell(cell).expect("padded");
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN * 2);
    Runtime::new().unwrap().block_on(async move {
        write_frame(&mut writer, &padded).await.expect("write");
        let parsed = read_frame(&overlay, &mut reader).await.expect("read");
        assert_eq!(parsed.payload, vec![1, 2, 3, 4]);
        assert_eq!(padding_budget_ms, parsed.header.padding_budget_ms);
    });
}

#[test]
fn overlay_rejects_truncated_stream_frames() {
    let overlay = overlay();
    let padding_budget_ms = overlay.config().padding_budget_ms;
    let cell = VpnCellV1 {
        header: VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Data,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            circuit_id: [0xCD; 16],
            flow_label: VpnFlowLabelV1::from_u32(11).expect("flow"),
            sequence: 2,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        },
        payload: vec![0xEF; 6],
    };
    let padded = overlay.pad_cell(cell).expect("padded");
    let (mut writer, mut reader) = duplex(VPN_CELL_LEN / 2);
    Runtime::new().unwrap().block_on(async move {
        writer
            .write_all(&padded.frame.as_ref()[..128])
            .await
            .expect("partial write");
        drop(writer);
        let err = read_frame(&overlay, &mut reader)
            .await
            .expect_err("truncated frame should be rejected");
        assert!(matches!(
            err,
            VpnFrameIoError::FrameLength { expected, actual }
            if expected == VPN_CELL_LEN && actual < VPN_CELL_LEN
        ));
    });
}
