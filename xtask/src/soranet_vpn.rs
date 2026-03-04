use std::time::Duration;

use iroha_config::parameters::actual::SoranetVpn;
use iroha_data_model::soranet::vpn::{
    VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
    VpnControlPlaneV1, VpnCoverPlanEntryV1, VpnCoverScheduleV1, VpnExitClassParseError,
    VpnExitClassV1, VpnFlowLabelV1, VpnPaddedCellV1, VpnRouteV1, VpnSessionReceiptV1,
};
use thiserror::Error;

/// Errors raised when assembling VPN control-plane/receipt payloads from config.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VpnConfigError {
    /// Exit class label was not recognised.
    #[error(transparent)]
    ExitClass(#[from] VpnExitClassParseError),
    /// Lease duration exceeds the wire-encoded range.
    #[error("lease duration {lease_secs} seconds exceeds u32::MAX")]
    LeaseOverflow {
        /// Lease duration (seconds) that triggered the overflow.
        lease_secs: u64,
    },
}

/// Build a deterministic cover/data schedule from runtime configuration.
pub fn cover_schedule_from_config(
    cfg: &SoranetVpn,
    seed: [u8; 32],
    frames: usize,
) -> Vec<VpnCoverPlanEntryV1> {
    VpnCoverScheduleV1 {
        cover_to_data_per_mille: cfg.cover_to_data_per_mille,
        heartbeat_ms: cfg.heartbeat_ms,
        max_cover_burst: cfg.max_cover_burst,
        jitter_ms: cfg.jitter_ms,
    }
    .plan(seed, frames)
}

/// Convert an exit class label into the Norito enum.
pub fn exit_class_from_label(label: &str) -> Result<VpnExitClassV1, VpnConfigError> {
    VpnExitClassV1::try_from_label(label).map_err(VpnConfigError::from)
}

/// Assemble a control-plane envelope for guard/exit selection and DNS/route push.
pub fn control_plane_from_config(
    cfg: &SoranetVpn,
    entry_guard: [u8; 32],
    exit_guard: [u8; 32],
    dns_servers: Vec<String>,
    routes: Vec<VpnRouteV1>,
) -> Result<VpnControlPlaneV1, VpnConfigError> {
    let lease_seconds = lease_secs_u32(cfg.lease)?;
    Ok(VpnControlPlaneV1 {
        entry_guard,
        exit_guard,
        dns_servers,
        routes,
        exit_class: exit_class_from_label(&cfg.exit_class)?,
        lease_seconds,
    })
}

#[derive(Clone, Copy)]
pub struct FrameMeta {
    pub sequence: u64,
    pub ack: u64,
    pub class: VpnCellClassV1,
    pub flags: VpnCellFlagsV1,
}

/// Build a padded VPN frame using scheduler and padding hints from config.
///
/// The configured cell size is validated against the pinned [`VPN_CELL_LEN`].
pub fn frame_payload(
    cfg: &SoranetVpn,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    meta: FrameMeta,
    payload: Vec<u8>,
) -> Result<VpnPaddedCellV1, VpnCellError> {
    flow_label.ensure_width(cfg.flow_label_bits)?;
    if usize::from(cfg.cell_size_bytes) != VPN_CELL_LEN {
        return Err(VpnCellError::FrameLengthMismatch {
            expected: usize::from(cfg.cell_size_bytes),
            actual: VPN_CELL_LEN,
        });
    }

    let header = VpnCellHeaderV1 {
        version: 1,
        class: meta.class,
        flags: meta.flags,
        circuit_id,
        flow_label,
        sequence: meta.sequence,
        ack: meta.ack,
        padding_budget_ms: cfg.padding_budget_ms,
        payload_len: 0,
    };
    let cell = VpnCellV1 { header, payload };
    cell.into_padded_frame()
}

/// Build a billing/telemetry receipt for an exit session.
pub fn session_receipt(
    cfg: &SoranetVpn,
    session_id: [u8; 16],
    ingress_bytes: u64,
    egress_bytes: u64,
    cover_bytes: u64,
    uptime_secs: u32,
    meter_hash: [u8; 32],
) -> Result<VpnSessionReceiptV1, VpnConfigError> {
    Ok(VpnSessionReceiptV1 {
        session_id,
        ingress_bytes,
        egress_bytes,
        cover_bytes,
        uptime_secs,
        exit_class: exit_class_from_label(&cfg.exit_class)?,
        meter_hash,
    })
}

fn lease_secs_u32(lease: Duration) -> Result<u32, VpnConfigError> {
    let secs = lease.as_secs();
    u32::try_from(secs).map_err(|_| VpnConfigError::LeaseOverflow { lease_secs: secs })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> SoranetVpn {
        SoranetVpn::default()
    }

    #[test]
    fn cover_schedule_respects_ratio_and_burst() {
        let cfg = default_cfg();
        let plan = cover_schedule_from_config(&cfg, [0x22; 32], 16);
        let cover = plan.iter().filter(|entry| entry.is_cover).count();
        assert!((2..=6).contains(&cover), "unexpected cover count {cover}");
        // ensure no burst exceeds configured cap
        let mut streak = 0usize;
        for entry in &plan {
            if entry.is_cover {
                streak += 1;
                assert!(
                    streak as u16 <= cfg.max_cover_burst,
                    "cover burst exceeded cap"
                );
            } else {
                streak = 0;
            }
        }
    }

    #[test]
    fn framing_respects_cell_size() {
        let cfg = default_cfg();
        let flow_label = VpnFlowLabelV1::from_u32(7).expect("flow label");
        let frame = frame_payload(
            &cfg,
            [0xAA; 16],
            flow_label,
            FrameMeta {
                sequence: 9,
                ack: 0,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, true, false, false),
            },
            vec![0xBB; 64],
        )
        .expect("frame encodes");
        assert_eq!(VPN_CELL_LEN, frame.as_ref().len());
        assert_eq!(0xAA, frame.as_ref()[3]);
    }

    #[test]
    fn framing_rejects_mismatched_cell_size() {
        let mut cfg = default_cfg();
        cfg.cell_size_bytes = (VPN_CELL_LEN as u16).saturating_sub(8);
        let flow_label = VpnFlowLabelV1::from_u32(1).expect("flow label");
        let result = frame_payload(
            &cfg,
            [0x11; 16],
            flow_label,
            FrameMeta {
                sequence: 1,
                ack: 0,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, false, false, false),
            },
            vec![0x33; 4],
        );
        assert!(matches!(
            result,
            Err(VpnCellError::FrameLengthMismatch { expected, actual })
            if expected == cfg.cell_size_bytes as usize && actual == VPN_CELL_LEN
        ));
    }

    #[test]
    fn framing_zero_pads_and_threads_padding_budget() {
        let mut cfg = default_cfg();
        cfg.padding_budget_ms = 17;
        let flow_label = VpnFlowLabelV1::from_u32(9).expect("flow label");
        let payload = vec![0xCC; 5];
        let frame = frame_payload(
            &cfg,
            [0xAA; 16],
            flow_label,
            FrameMeta {
                sequence: 4,
                ack: 2,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, true, false, false),
            },
            payload.clone(),
        )
        .expect("frame");
        let parsed = frame.parse().expect("parsed");
        assert_eq!(cfg.padding_budget_ms, parsed.header.padding_budget_ms);
        assert_eq!(payload, parsed.payload);

        let payload_offset = VPN_CELL_LEN - VpnCellV1::max_payload_len();
        let padding_start = payload_offset + payload.len();
        assert!(
            frame.as_ref()[padding_start..]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn framing_rejects_flow_label_overflow_for_bits() {
        let mut cfg = default_cfg();
        cfg.flow_label_bits = 8;
        let flow_label = VpnFlowLabelV1::from_u32(0x1FF).expect("flow label");
        let result = frame_payload(
            &cfg,
            [0xAA; 16],
            flow_label,
            FrameMeta {
                sequence: 1,
                ack: 0,
                class: VpnCellClassV1::Data,
                flags: VpnCellFlagsV1::new(false, false, false, false),
            },
            vec![0x01; 8],
        );
        assert!(matches!(
            result,
            Err(VpnCellError::FlowLabelOverflow { max_bits: 8, .. })
        ));
    }

    #[test]
    fn control_plane_serializes_keys() {
        let cfg = default_cfg();
        let control = control_plane_from_config(
            &cfg,
            [0x01; 32],
            [0x02; 32],
            vec!["1.1.1.1".to_string()],
            Vec::new(),
        )
        .expect("control plane");
        assert_eq!(control.entry_guard, [0x01; 32]);
        assert_eq!(control.exit_guard, [0x02; 32]);
    }

    #[test]
    fn receipt_uses_exit_label() {
        let mut cfg = default_cfg();
        cfg.exit_class = "high-security".to_string();
        let receipt =
            session_receipt(&cfg, [0x11; 16], 10, 20, 5, 30, [0x55; 32]).expect("receipt");
        assert_eq!(VpnExitClassV1::HighSecurity, receipt.exit_class);
    }

    #[test]
    fn control_plane_rejects_unknown_exit_class() {
        let mut cfg = default_cfg();
        cfg.exit_class = "ultra-fast".to_string();
        let err = control_plane_from_config(&cfg, [0xAA; 32], [0xBB; 32], Vec::new(), Vec::new())
            .expect_err("exit class should fail");
        assert!(matches!(err, VpnConfigError::ExitClass(_)));
    }

    #[test]
    fn session_receipt_rejects_unknown_exit_class() {
        let mut cfg = default_cfg();
        cfg.exit_class = "premium".to_string();
        let err = session_receipt(&cfg, [0x22; 16], 1, 1, 1, 1, [0x11; 32])
            .expect_err("exit class should fail");
        assert!(matches!(err, VpnConfigError::ExitClass(_)));
    }

    #[test]
    fn control_plane_accepts_max_lease() {
        let mut cfg = default_cfg();
        cfg.lease = Duration::from_secs(u64::from(u32::MAX));
        let control =
            control_plane_from_config(&cfg, [0xAA; 32], [0xBB; 32], Vec::new(), Vec::new())
                .expect("max lease should be accepted");
        assert_eq!(u32::MAX, control.lease_seconds);
    }

    #[test]
    fn control_plane_rejects_lease_overflow() {
        let mut cfg = default_cfg();
        cfg.lease = Duration::from_secs(u64::from(u32::MAX) + 1);
        let err = control_plane_from_config(&cfg, [0xAA; 32], [0xBB; 32], Vec::new(), Vec::new())
            .expect_err("lease overflow should fail");
        assert!(
            matches!(err, VpnConfigError::LeaseOverflow { lease_secs } if lease_secs == u64::from(u32::MAX) + 1)
        );
    }
}
