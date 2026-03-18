//! Minimal VPN tunnel adapter plumbing.
//!
//! This adapter will evolve to bridge a TUN-like source into SoraNet cells. For now it
//! accepts opaque ingress/egress byte counts so the metrics and billing hooks can be
//! tested without a full tunnel implementation.

use blake3::Hasher;
use iroha_data_model::soranet::vpn::{VpnCellClassV1, VpnCellFlagsV1, VpnCellV1, VpnFlowLabelV1};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::vpn::{
    CoverFrameMeta, PaddedCell, VpnFrameBuildError, VpnFrameIoError, VpnOverlay, VpnSession,
    read_frame as read_padded_frame, schedule_frames, send_scheduled_frames_with_adapter,
    write_frame as write_padded_frame,
};

/// Batch parameters for building data-class VPN frames.
#[derive(Debug, Clone)]
pub struct VpnDataFrameBatch<'a> {
    /// Circuit identifier carried by the frames.
    pub circuit_id: [u8; 16],
    /// Flow label that tags the frames.
    pub flow_label: VpnFlowLabelV1,
    /// Sequence number assigned to the first payload.
    pub start_sequence: u64,
    /// Acknowledgement number to include on each frame.
    pub ack: u64,
    /// Flags applied to each generated frame.
    pub flags: VpnCellFlagsV1,
    /// Payloads to emit as data-class frames.
    pub payloads: &'a [Vec<u8>],
}

/// Lightweight adapter that wires a VPN session and overlay into async IO.
#[derive(Debug, Clone)]
pub struct VpnAdapter {
    session: VpnSession,
    overlay: VpnOverlay,
}

impl VpnAdapter {
    /// Create a new adapter over the given session and overlay.
    pub fn new(session: VpnSession, overlay: VpnOverlay) -> Self {
        Self { session, overlay }
    }

    /// Access the overlay used for padding and framing.
    pub fn overlay(&self) -> &VpnOverlay {
        &self.overlay
    }

    /// Access the underlying VPN session state.
    pub fn session(&self) -> &VpnSession {
        &self.session
    }

    /// Record bytes observed on the ingress side of the tunnel.
    pub fn record_ingress(&self, bytes: u64) {
        self.session.record_ingress(bytes, false);
    }

    /// Record bytes observed on the egress side of the tunnel.
    pub fn record_egress(&self, bytes: u64) {
        self.session.record_egress(bytes, false);
    }

    /// Record a counted ingress frame without parsing (counts both frame and bytes).
    pub fn record_ingress_frame_count(&self, bytes: u64, is_cover: bool) {
        self.session
            .metrics()
            .record_vpn_frame_ingress_count(1, is_cover);
        self.session.record_ingress(bytes, is_cover);
    }

    /// Record a counted egress frame without parsing (counts both frame and bytes).
    pub fn record_egress_frame_count(&self, bytes: u64, is_cover: bool) {
        self.session
            .metrics()
            .record_vpn_frame_egress_count(1, is_cover);
        self.session.record_egress(bytes, is_cover);
    }

    /// Pad and account for an egress VPN cell, returning the fixed-length frame.
    pub fn encode_egress_cell(&self, cell: VpnCellV1) -> Result<PaddedCell, VpnFrameBuildError> {
        let class = cell.header.class;
        let payload_len = cell.payload.len() as u64;
        let frame = self.overlay.pad_cell(cell)?;
        self.session.record_classified_egress(class, payload_len);
        Ok(frame)
    }

    /// Finalize the session into a receipt for billing/telemetry.
    pub fn finish_receipt(
        &self,
        session_id: [u8; 16],
        exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1,
        meter_hash: [u8; 32],
    ) -> iroha_data_model::soranet::vpn::VpnSessionReceiptV1 {
        self.session
            .finish_receipt(session_id, exit_class, meter_hash)
    }

    /// Build, pad, and account for a data cell in one step.
    pub fn encapsulate_data_cell(
        &self,
        circuit_id: [u8; 16],
        flow_label: VpnFlowLabelV1,
        sequence: u64,
        ack: u64,
        flags: VpnCellFlagsV1,
        payload: Vec<u8>,
    ) -> Result<PaddedCell, VpnFrameBuildError> {
        let cell = self
            .overlay
            .data_cell(circuit_id, flow_label, sequence, ack, flags, payload)?;
        self.encode_egress_cell(cell)
    }

    /// Parse and account for an ingress VPN frame, returning the parsed cell on success.
    pub fn record_frame_ingress(
        &self,
        frame: &[u8],
    ) -> Result<
        iroha_data_model::soranet::vpn::VpnCellV1,
        iroha_data_model::soranet::vpn::VpnCellError,
    > {
        self.session.record_frame_ingress(&self.overlay, frame)
    }

    /// Parse and account for an egress VPN frame, returning the parsed cell on success.
    pub fn record_frame_egress(
        &self,
        frame: &[u8],
    ) -> Result<
        iroha_data_model::soranet::vpn::VpnCellV1,
        iroha_data_model::soranet::vpn::VpnCellError,
    > {
        self.session.record_frame_egress(&self.overlay, frame)
    }

    /// Read, parse, and account for an ingress VPN frame from an async reader.
    pub async fn read_ingress_frame<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<VpnCellV1, VpnFrameIoError> {
        let cell = read_padded_frame(&self.overlay, reader).await?;
        self.session.record_parsed_ingress(&cell);
        Ok(cell)
    }

    /// Pad, write, and account for an egress VPN frame to an async writer.
    pub async fn write_egress_frame<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        cell: VpnCellV1,
    ) -> Result<(), VpnFrameIoError> {
        let padded = self.encode_egress_cell(cell)?;
        write_padded_frame(writer, &padded).await?;
        Ok(())
    }

    /// Encode and send a sequence of data payloads as VPN data cells.
    ///
    /// This helper is intended for tunnel bridges: it builds data-class cells with the
    /// supplied identifiers and uses the adapter for accounting.
    pub async fn send_data_frames<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        batch: VpnDataFrameBatch<'_>,
    ) -> Result<(), VpnFrameIoError> {
        for (idx, payload) in batch.payloads.iter().enumerate() {
            let cell = self.overlay.data_cell(
                batch.circuit_id,
                batch.flow_label,
                batch.start_sequence.saturating_add(idx as u64),
                batch.ack,
                batch.flags,
                payload.clone(),
            )?;
            self.write_egress_frame(writer, cell).await?;
        }
        Ok(())
    }

    /// Build, pace, and send data cells with optional cover, accounting via this adapter.
    pub async fn send_paced_data_frames<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        batch: VpnDataFrameBatch<'_>,
        cover_meta: CoverFrameMeta,
        seed: [u8; 32],
    ) -> Result<(), VpnFrameIoError> {
        let mut data_cells = Vec::with_capacity(batch.payloads.len());
        for (idx, payload) in batch.payloads.iter().enumerate() {
            let cell = self.overlay.data_cell(
                batch.circuit_id,
                batch.flow_label,
                batch.start_sequence.saturating_add(idx as u64),
                batch.ack,
                batch.flags,
                payload.clone(),
            )?;
            data_cells.push(cell);
        }
        let schedule = schedule_frames(&self.overlay, data_cells, cover_meta, seed)?;
        send_scheduled_frames_with_adapter(&schedule, writer, Some(self), Some(&self.session)).await
    }
}

/// Outcome of a bridge send operation.
/// Outcome of a bridge send operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpnBridgeSendOutcome {
    /// Number of user data frames emitted.
    pub data_frames: usize,
    /// Number of cover frames emitted.
    pub cover_frames: usize,
}

/// Helper that owns circuit identifiers and sequences for a tunnel bridge.
#[derive(Debug, Clone)]
pub struct VpnBridge {
    adapter: VpnAdapter,
    circuit_id: [u8; 16],
    flow_label: VpnFlowLabelV1,
    next_sequence: u64,
    next_cover_sequence: u64,
    ack: u64,
    flags: VpnCellFlagsV1,
    cover_flags: VpnCellFlagsV1,
    cover_seed: [u8; 32],
}

fn default_cover_seed(circuit_id: [u8; 16], flow_label: VpnFlowLabelV1) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"soranet-vpn-cover-seed");
    hasher.update(&circuit_id);
    hasher.update(&flow_label.bytes);
    let digest = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(digest.as_bytes());
    seed
}

impl VpnBridge {
    /// Construct a new bridge bound to a circuit and flow label.
    pub fn new(adapter: VpnAdapter, circuit_id: [u8; 16], flow_label: VpnFlowLabelV1) -> Self {
        Self {
            adapter,
            circuit_id,
            flow_label,
            next_sequence: 0,
            next_cover_sequence: 0,
            ack: 0,
            flags: VpnCellFlagsV1::new(false, false, false, false),
            cover_flags: VpnCellFlagsV1::new(true, false, false, false),
            cover_seed: default_cover_seed(circuit_id, flow_label),
        }
    }

    /// Update the ACK value propagated on subsequent frames.
    pub fn set_ack(&mut self, ack: u64) {
        self.ack = ack;
    }

    /// Override the data-frame flags used for future cells.
    pub fn set_flags(&mut self, flags: VpnCellFlagsV1) {
        self.flags = flags;
    }

    /// Override the cover-frame flags used for future schedules.
    pub fn set_cover_flags(&mut self, flags: VpnCellFlagsV1) {
        self.cover_flags = flags;
    }

    /// Update the deterministic seed used for cover scheduling.
    pub fn set_cover_seed(&mut self, seed: [u8; 32]) {
        self.cover_seed = seed;
    }

    /// Return the underlying adapter.
    pub fn adapter(&self) -> &VpnAdapter {
        &self.adapter
    }

    /// Split the bridge into its adapter and fixed identifiers.
    pub fn into_parts(self) -> (VpnAdapter, [u8; 16], VpnFlowLabelV1) {
        (self.adapter, self.circuit_id, self.flow_label)
    }

    /// Maximum payload supported by a single VPN data cell.
    pub fn max_payload_len(&self) -> usize {
        VpnCellV1::max_payload_len()
    }

    /// Send a batch of payloads as data frames, pacing and injecting cover as configured.
    pub async fn send_payloads<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        payloads: &[Vec<u8>],
    ) -> Result<VpnBridgeSendOutcome, VpnFrameIoError> {
        if payloads.is_empty() {
            return Ok(VpnBridgeSendOutcome {
                data_frames: 0,
                cover_frames: 0,
            });
        }

        let start_sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(payloads.len() as u64);
        let overlay = self.adapter.overlay();

        let mut data_cells = Vec::with_capacity(payloads.len());
        for (idx, payload) in payloads.iter().enumerate() {
            data_cells.push(overlay.data_cell(
                self.circuit_id,
                self.flow_label,
                start_sequence.saturating_add(idx as u64),
                self.ack,
                self.flags,
                payload.clone(),
            )?);
        }

        let mut cover_flags = self.cover_flags;
        if !cover_flags.is_cover() {
            cover_flags = VpnCellFlagsV1::from_bits(cover_flags.bits() | VpnCellFlagsV1::COVER);
        }
        let cover_meta = CoverFrameMeta {
            circuit_id: self.circuit_id,
            flow_label: self.flow_label,
            ack: self.ack,
            flags: cover_flags,
            start_sequence: self.next_cover_sequence,
        };
        let schedule = schedule_frames(overlay, data_cells, cover_meta, self.cover_seed)
            .map_err(VpnFrameIoError::Build)?;
        let cover_frames = schedule.iter().filter(|frame| frame.is_cover).count();
        self.next_cover_sequence = self.next_cover_sequence.saturating_add(cover_frames as u64);
        send_scheduled_frames_with_adapter(
            &schedule,
            writer,
            Some(&self.adapter),
            Some(self.adapter.session()),
        )
        .await?;
        Ok(VpnBridgeSendOutcome {
            data_frames: payloads.len(),
            cover_frames,
        })
    }

    /// Fragment and send a single buffer as one or more data frames.
    pub async fn send_buffer<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        buffer: &[u8],
    ) -> Result<VpnBridgeSendOutcome, VpnFrameIoError> {
        if buffer.is_empty() {
            return Ok(VpnBridgeSendOutcome {
                data_frames: 0,
                cover_frames: 0,
            });
        }

        let mut payloads = Vec::new();
        for chunk in buffer.chunks(VpnCellV1::max_payload_len()) {
            payloads.push(chunk.to_vec());
        }
        self.send_payloads(writer, &payloads).await
    }

    /// Pump bytes from a TUN-like reader into VPN frames written to `vpn_writer`.
    pub async fn pump_tun_to_vpn<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
        &mut self,
        tun_reader: &mut R,
        vpn_writer: &mut W,
        mtu: usize,
    ) -> Result<VpnBridgeSendOutcome, VpnFrameIoError> {
        let max_payload = mtu.min(self.max_payload_len()).max(1);
        let mut buffer = vec![0u8; max_payload];
        let mut total_outcome = VpnBridgeSendOutcome {
            data_frames: 0,
            cover_frames: 0,
        };

        loop {
            let read = tun_reader
                .read(&mut buffer)
                .await
                .map_err(VpnFrameIoError::Io)?;
            if read == 0 {
                break;
            }
            let outcome = self.send_buffer(vpn_writer, &buffer[..read]).await?;
            total_outcome.data_frames = total_outcome
                .data_frames
                .saturating_add(outcome.data_frames);
            total_outcome.cover_frames = total_outcome
                .cover_frames
                .saturating_add(outcome.cover_frames);
        }

        Ok(total_outcome)
    }

    /// Parse a single ingress cell from `vpn_reader` and write data payloads to the TUN writer.
    pub async fn forward_vpn_to_tun<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
        &self,
        vpn_reader: &mut R,
        tun_writer: &mut W,
    ) -> Result<(), VpnFrameIoError> {
        match self.adapter.read_ingress_frame(vpn_reader).await {
            Ok(cell) => match cell.header.class {
                VpnCellClassV1::Data => tun_writer
                    .write_all(&cell.payload)
                    .await
                    .map_err(VpnFrameIoError::Io),
                VpnCellClassV1::Cover | VpnCellClassV1::KeepAlive | VpnCellClassV1::Control => {
                    Ok(())
                }
            },
            Err(VpnFrameIoError::FrameLength { actual: 0, .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_data_model::soranet::vpn::VpnCellHeaderV1;

    use super::*;
    use crate::metrics::Metrics;

    #[test]
    fn bridge_derives_default_cover_seed() {
        let metrics = Arc::new(Metrics::new());
        let overlay = VpnOverlay::from_config(Default::default());
        let session = VpnSession::from_parts(Arc::clone(&metrics));
        let adapter = VpnAdapter::new(session, overlay);
        let circuit_id = [0xA5; 16];
        let flow_label = VpnFlowLabelV1::from_u32(0x1234).expect("flow label");
        let bridge = VpnBridge::new(adapter, circuit_id, flow_label);

        let expected = default_cover_seed(circuit_id, flow_label);
        assert_eq!(expected, bridge.cover_seed);
        assert_ne!([0u8; 32], bridge.cover_seed);
    }

    #[test]
    fn adapter_counts_cover_cells_on_encode() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let overlay = VpnOverlay::from_config(Default::default());
        let session = VpnSession::from_parts(Arc::clone(&metrics));
        let adapter = VpnAdapter::new(session, overlay);
        let padding_budget_ms = adapter.overlay().config().padding_budget_ms;

        let header = VpnCellHeaderV1 {
            version: 1,
            class: VpnCellClassV1::Cover,
            flags: VpnCellFlagsV1::new(true, false, false, false),
            circuit_id: [0x33; 16],
            flow_label: VpnFlowLabelV1::from_u32(1).expect("flow label"),
            sequence: 1,
            ack: 0,
            padding_budget_ms,
            payload_len: 0,
        };
        let cell = VpnCellV1 {
            header,
            payload: vec![0xAB; 5],
        };

        let _ = adapter
            .encode_egress_cell(cell)
            .expect("encoded cover cell");

        let snapshot = metrics.snapshot();
        assert_eq!(1, snapshot.vpn_frames);
        assert_eq!(1, snapshot.vpn_egress_frames);
        assert_eq!(1, snapshot.vpn_cover_frames);
        assert_eq!(5, snapshot.vpn_cover_bytes);
        assert_eq!(5, snapshot.vpn_cover_egress_bytes);
        assert_eq!(0, snapshot.vpn_data_bytes);
        assert_eq!(0, snapshot.vpn_data_egress_bytes);
    }
}
