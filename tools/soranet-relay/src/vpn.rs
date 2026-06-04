//! VPN overlay configuration, parsing, and accounting hooks.
//!
//! This module wires the relay metrics into the SoraNet VPN cell format so ingress/egress
//! accounting works end-to-end while the tunnel runtime handles fixed-size framing,
//! pacing, and cover injection.

use std::{
    cmp::max,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use iroha_data_model::soranet::{
    RelayId,
    vpn::{
        VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
        VpnControlPlaneV1, VpnCoverPlanEntryV1, VpnCoverScheduleV1, VpnExitClassV1, VpnFlowLabelV1,
        VpnHelperTicketV1, VpnPaddedCellV1, VpnRouteV1, VpnSessionReceiptV1,
        VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1,
    },
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::{Instant as TokioInstant, sleep_until},
};

use crate::{
    config::{ConfigError, VpnConfig},
    metrics::Metrics,
    vpn_adapter::{VpnAdapter, VpnBridge},
};

fn unix_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn unix_now_ms() -> u64 {
    unix_time_ms(SystemTime::now())
}

/// Padded cell with the computed payload length retained for accounting.
#[derive(Debug, Clone)]
pub struct PaddedCell {
    /// Fully padded fixed-length frame.
    pub frame: VpnPaddedCellV1,
    /// Unpadded payload length carried in the header.
    pub payload_len: u16,
}

/// Errors surfaced when building frames from runtime configuration.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VpnFrameBuildError {
    /// The configured cell size does not match the pinned layout.
    #[error("vpn cell size {actual}B does not match pinned length {expected}B")]
    CellSizeMismatch { expected: usize, actual: usize },
    /// Frame failed validation while being padded.
    #[error(transparent)]
    Cell(#[from] VpnCellError),
}

/// Errors surfaced while reading or writing VPN frames.
#[derive(Debug, Error)]
pub enum VpnFrameIoError {
    /// I/O failure while reading or writing frame bytes.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Frame could not be prepared due to invalid layout or config.
    #[error(transparent)]
    Build(#[from] VpnFrameBuildError),
    /// Frame failed validation during parsing.
    #[error(transparent)]
    Parse(#[from] VpnCellError),
    /// Frame length did not match the pinned cell size.
    #[error("padded frame length {actual}B does not match expected {expected}B")]
    FrameLength { expected: usize, actual: usize },
}

/// Cover frame metadata that stays constant across scheduled cover cells.
#[derive(Debug, Clone, Copy)]
pub struct CoverFrameMeta {
    /// Circuit identifier for the tunnel.
    pub circuit_id: [u8; 16],
    /// Flow label to stamp on cover cells.
    pub flow_label: VpnFlowLabelV1,
    /// Latest acknowledged sequence number.
    pub ack: u64,
    /// Flags propagated to cover frames (cover bit enforced).
    pub flags: VpnCellFlagsV1,
    /// Starting sequence number for scheduled cover frames.
    pub start_sequence: u64,
}

/// Frame scheduled for transmission at `deadline` relative to the start of the pump.
#[derive(Debug, Clone)]
pub struct ScheduledFrame {
    /// Deadline relative to the start of the schedule.
    pub deadline: Duration,
    /// Padded frame to emit.
    pub frame: VpnPaddedCellV1,
    /// Payload length carried by the frame.
    pub payload_len: u16,
    /// Whether the scheduled frame is a cover cell.
    pub is_cover: bool,
}

/// Overlay that handles VPN cell framing, validation, and billing metadata.
#[derive(Debug, Clone)]
pub struct VpnOverlay {
    config: VpnConfig,
    exit_class: VpnExitClassV1,
    meter_hash: [u8; 32],
    routes: Vec<VpnRouteV1>,
    dns_overrides: Vec<String>,
}

impl VpnOverlay {
    /// Build an overlay from VPN configuration without panicking on malformed fields.
    pub fn try_from_config(mut config: VpnConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        let exit_class = VpnExitClassV1::try_from_label(&config.exit_class).map_err(|error| {
            ConfigError::Vpn(format!(
                "vpn.exit_class must be standard|low-latency|high-security: {error}"
            ))
        })?;
        let meter_hash = config.try_meter_hash_bytes()?;
        let routes = config.parse_route_push()?;
        let dns_overrides = config.parse_dns_overrides()?;
        let _ = config.try_helper_ticket_secret_bytes()?;
        let _ = config.try_backend_endpoint()?;
        let _ = config.try_backend_bootstrap_secret_bytes()?;
        Ok(Self {
            config,
            exit_class,
            meter_hash,
            routes,
            dns_overrides,
        })
    }

    /// Build an overlay from validated VPN configuration.
    pub fn from_config(config: VpnConfig) -> Self {
        Self::try_from_config(config).expect("vpn config should validate before overlay creation")
    }

    /// Access the raw VPN configuration.
    pub fn config(&self) -> &VpnConfig {
        &self.config
    }

    /// Return the exit-class label advertised by this overlay.
    pub fn exit_class(&self) -> VpnExitClassV1 {
        self.exit_class
    }

    /// Return the billing meter hash in raw bytes.
    pub fn meter_hash(&self) -> [u8; 32] {
        self.meter_hash
    }

    /// Return the session and byte meter labels configured for billing.
    pub fn billing_labels(&self) -> (&str, &str) {
        (
            self.config.billing.session_meter_label.as_str(),
            self.config.billing.byte_meter_label.as_str(),
        )
    }

    /// Build a control-plane envelope for clients using the configured routes/DNS.
    pub fn control_plane_envelope(
        &self,
        entry_guard: RelayId,
        exit_guard: RelayId,
    ) -> VpnControlPlaneV1 {
        VpnControlPlaneV1 {
            entry_guard,
            exit_guard,
            dns_servers: self.dns_overrides.clone(),
            routes: self.routes.clone(),
            exit_class: self.exit_class,
            lease_seconds: self.config.lease_secs,
        }
    }

    /// Parse and validate a padded VPN cell frame using the configured cell length.
    pub fn parse_frame(&self, frame: &[u8]) -> Result<VpnCellV1, VpnCellError> {
        let configured_len = usize::from(self.config.cell_size_bytes);
        if configured_len != VPN_CELL_LEN {
            return Err(VpnCellError::FrameLengthMismatch {
                expected: configured_len,
                actual: frame.len(),
            });
        }
        let cell =
            VpnPaddedCellV1::parse_bytes_with_flow_label_bits(frame, self.config.flow_label_bits)?;
        self.validate_cell(&cell)?;
        Ok(cell)
    }

    /// Pad and validate an outbound VPN cell into its fixed frame.
    pub fn pad_cell(&self, mut cell: VpnCellV1) -> Result<PaddedCell, VpnFrameBuildError> {
        self.ensure_cell_size()?;
        cell.header
            .flow_label
            .ensure_width(self.config.flow_label_bits)?;
        self.validate_flags(&cell)?;
        cell.header.padding_budget_ms = self.config.padding_budget_ms;
        let payload_len_usize = cell.payload.len();
        let payload_len = u16::try_from(payload_len_usize).map_err(|_| {
            VpnFrameBuildError::Cell(VpnCellError::PayloadLengthMismatch {
                declared: u16::MAX,
                actual: payload_len_usize,
            })
        })?;
        if payload_len_usize > VpnCellV1::max_payload_len() {
            return Err(VpnFrameBuildError::Cell(VpnCellError::PayloadTooLarge {
                max: VpnCellV1::max_payload_len(),
                actual: payload_len_usize,
            }));
        }
        let padded = cell.into_padded_frame()?;
        Ok(PaddedCell {
            payload_len,
            frame: padded,
        })
    }

    /// Construct a data cell honoring the overlay's flow-label width and padding budget.
    pub fn data_cell(
        &self,
        circuit_id: [u8; 16],
        flow_label: VpnFlowLabelV1,
        sequence: u64,
        ack: u64,
        flags: VpnCellFlagsV1,
        payload: Vec<u8>,
    ) -> Result<VpnCellV1, VpnFrameBuildError> {
        flow_label.ensure_width(self.config.flow_label_bits)?;
        let payload_len = u16::try_from(payload.len()).map_err(|_| {
            VpnFrameBuildError::Cell(VpnCellError::PayloadLengthMismatch {
                declared: u16::MAX,
                actual: payload.len(),
            })
        })?;
        if payload.len() > VpnCellV1::max_payload_len() {
            return Err(VpnFrameBuildError::Cell(VpnCellError::PayloadTooLarge {
                max: VpnCellV1::max_payload_len(),
                actual: payload.len(),
            }));
        }
        let cell = VpnCellV1 {
            header: VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Data,
                flags,
                circuit_id,
                flow_label,
                sequence,
                ack,
                padding_budget_ms: self.config.padding_budget_ms,
                payload_len,
            },
            payload,
        };
        self.validate_flags(&cell)?;
        Ok(cell)
    }

    /// Build a cover frame using the configured padding budget and supplied metadata.
    pub fn cover_frame(
        &self,
        meta: &CoverFrameMeta,
        sequence: u64,
    ) -> Result<PaddedCell, VpnFrameBuildError> {
        self.ensure_cell_size()?;
        meta.flow_label.ensure_width(self.config.flow_label_bits)?;
        let mut flags = meta.flags;
        if !flags.is_cover() {
            flags = VpnCellFlagsV1::from_bits(flags.bits() | VpnCellFlagsV1::COVER);
        }
        let cell = VpnCellV1 {
            header: VpnCellHeaderV1 {
                version: 1,
                class: VpnCellClassV1::Cover,
                flags,
                circuit_id: meta.circuit_id,
                flow_label: meta.flow_label,
                sequence,
                ack: meta.ack,
                padding_budget_ms: self.config.padding_budget_ms,
                payload_len: 0,
            },
            payload: Vec::new(),
        };
        self.validate_flags(&cell)?;
        let padded = cell.into_padded_frame()?;
        Ok(PaddedCell {
            frame: padded,
            payload_len: 0,
        })
    }

    /// Start a new VPN session and return a handle for accounting.
    pub fn start_session(&self, metrics: Arc<Metrics>) -> VpnSession {
        metrics.record_vpn_session();
        VpnSession::from_parts(metrics)
    }

    /// Bind a started session to receipt metadata derived from the overlay configuration.
    pub fn bind_session(&self, session: VpnSession, session_id: [u8; 16]) -> VpnSessionHandle {
        VpnSessionHandle::new(session, session_id, self.exit_class, self.meter_hash)
    }

    /// Bind a started helper-authenticated VPN session to the ticket metadata.
    pub fn bind_helper_session(
        &self,
        session: VpnSession,
        helper_ticket: VpnHelperTicketV1,
    ) -> VpnSessionHandle {
        VpnSessionHandle::from_helper_ticket(
            session,
            helper_ticket,
            self.exit_class,
            self.meter_hash,
        )
    }

    /// Start a new VPN session and return an adapter for recording ingress/egress.
    pub fn start_adapter(&self, metrics: Arc<Metrics>) -> VpnAdapter {
        let session = self.start_session(metrics);
        VpnAdapter::new(session, self.clone())
    }

    /// Start a new VPN session and return a bridge bound to the supplied identifiers.
    pub fn start_bridge(
        &self,
        metrics: Arc<Metrics>,
        circuit_id: [u8; 16],
        flow_label: VpnFlowLabelV1,
    ) -> VpnBridge {
        let adapter = self.start_adapter(metrics);
        VpnBridge::new(adapter, circuit_id, flow_label)
    }

    fn ensure_cell_size(&self) -> Result<(), VpnFrameBuildError> {
        let configured = usize::from(self.config.cell_size_bytes);
        if configured != VPN_CELL_LEN {
            return Err(VpnFrameBuildError::CellSizeMismatch {
                expected: VPN_CELL_LEN,
                actual: configured,
            });
        }
        Ok(())
    }

    fn validate_flags(&self, cell: &VpnCellV1) -> Result<(), VpnFrameBuildError> {
        if cell.header.flags.has_unknown_bits() {
            return Err(VpnFrameBuildError::Cell(VpnCellError::InvalidFlags {
                bits: cell.header.flags.bits(),
                allowed: VpnCellFlagsV1::ALLOWED_MASK,
            }));
        }
        let is_cover_class = cell.header.class == VpnCellClassV1::Cover;
        if cell.header.flags.is_cover() != is_cover_class {
            return Err(VpnFrameBuildError::Cell(VpnCellError::FlagClassMismatch {
                class: cell.header.class,
                flags: cell.header.flags,
            }));
        }
        Ok(())
    }

    fn validate_cell(&self, cell: &VpnCellV1) -> Result<(), VpnCellError> {
        if cell.header.padding_budget_ms != self.config.padding_budget_ms {
            return Err(VpnCellError::PaddingBudgetMismatch {
                expected: self.config.padding_budget_ms,
                actual: cell.header.padding_budget_ms,
            });
        }
        let is_cover_class = cell.header.class == VpnCellClassV1::Cover;
        if cell.header.flags.is_cover() != is_cover_class {
            return Err(VpnCellError::FlagClassMismatch {
                class: cell.header.class,
                flags: cell.header.flags,
            });
        }
        Ok(())
    }
}

/// Read and validate a padded VPN cell from the provided reader.
pub async fn read_frame<R: AsyncRead + Unpin>(
    overlay: &VpnOverlay,
    reader: &mut R,
) -> Result<VpnCellV1, VpnFrameIoError> {
    let mut frame = [0u8; VPN_CELL_LEN];
    let mut read = 0usize;
    while read < VPN_CELL_LEN {
        let n = reader
            .read(&mut frame[read..])
            .await
            .map_err(VpnFrameIoError::Io)?;
        if n == 0 {
            return Err(VpnFrameIoError::FrameLength {
                expected: VPN_CELL_LEN,
                actual: read,
            });
        }
        read += n;
    }
    overlay.parse_frame(&frame).map_err(VpnFrameIoError::from)
}

/// Write a padded VPN cell frame to the provided writer.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    cell: &PaddedCell,
) -> Result<(), VpnFrameIoError> {
    let bytes = cell.frame.as_ref();
    if bytes.len() != VPN_CELL_LEN {
        return Err(VpnFrameIoError::FrameLength {
            expected: VPN_CELL_LEN,
            actual: bytes.len(),
        });
    }
    writer.write_all(bytes).await.map_err(VpnFrameIoError::Io)
}

fn cover_plan_from_config(
    config: &VpnConfig,
    frames: usize,
    seed: [u8; 32],
) -> Vec<VpnCoverPlanEntryV1> {
    if !config.cover.enabled {
        return (0..frames)
            .map(|idx| VpnCoverPlanEntryV1 {
                slot_ms: config.pacing_millis.saturating_mul(idx as u64),
                is_cover: false,
            })
            .collect();
    }

    let cover_ratio = config.cover.cover_to_data_per_mille.min(1_000);
    VpnCoverScheduleV1 {
        cover_to_data_per_mille: cover_ratio,
        heartbeat_ms: config.cover.heartbeat_ms,
        max_cover_burst: config.cover.max_cover_burst,
        jitter_ms: config.cover.max_jitter_millis,
    }
    .plan(seed, frames)
}

/// Build a paced schedule that interleaves data frames with cover cells.
pub fn schedule_frames(
    overlay: &VpnOverlay,
    data_cells: Vec<VpnCellV1>,
    cover_meta: CoverFrameMeta,
    seed: [u8; 32],
) -> Result<Vec<ScheduledFrame>, VpnFrameBuildError> {
    let mut padded_data = Vec::with_capacity(data_cells.len());
    for cell in data_cells {
        padded_data.push(overlay.pad_cell(cell)?);
    }

    let mut total_frames = padded_data.len();
    let mut plan = cover_plan_from_config(&overlay.config, total_frames, seed);
    while plan.iter().filter(|entry| !entry.is_cover).count() < padded_data.len() {
        total_frames = total_frames.saturating_add(1);
        plan = cover_plan_from_config(&overlay.config, total_frames, seed);
    }

    let mut data_iter = padded_data.into_iter();
    let mut schedule = Vec::with_capacity(total_frames);
    let mut cover_sequence = cover_meta.start_sequence;
    let mut last_deadline_ms: u64 = 0;

    for (idx, entry) in plan.into_iter().enumerate() {
        let scheduled_ms = entry.slot_ms;
        let deadline_ms = if idx == 0 {
            scheduled_ms
        } else {
            max(
                scheduled_ms,
                last_deadline_ms.saturating_add(overlay.config.pacing_millis),
            )
        };

        let (prepared, is_cover) = if entry.is_cover {
            (overlay.cover_frame(&cover_meta, cover_sequence)?, true)
        } else if let Some(data) = data_iter.next() {
            (data, false)
        } else {
            (overlay.cover_frame(&cover_meta, cover_sequence)?, true)
        };

        if is_cover {
            cover_sequence = cover_sequence.saturating_add(1);
        }

        last_deadline_ms = deadline_ms;
        schedule.push(ScheduledFrame {
            deadline: Duration::from_millis(deadline_ms),
            payload_len: prepared.payload_len,
            frame: prepared.frame,
            is_cover,
        });
    }

    Ok(schedule)
}

/// Emit the scheduled frames using the supplied writer and optional metrics session.
pub async fn send_scheduled_frames<W: AsyncWrite + Unpin>(
    schedule: &[ScheduledFrame],
    writer: &mut W,
    session: Option<&VpnSession>,
) -> Result<(), VpnFrameIoError> {
    send_scheduled_frames_with_adapter(schedule, writer, None, session).await
}

/// Emit the scheduled frames using the supplied writer and optional accounting adapter.
pub async fn send_scheduled_frames_with_adapter<W: AsyncWrite + Unpin>(
    schedule: &[ScheduledFrame],
    writer: &mut W,
    adapter: Option<&VpnAdapter>,
    session: Option<&VpnSession>,
) -> Result<(), VpnFrameIoError> {
    let start = TokioInstant::now();
    for scheduled in schedule {
        let deadline = start + scheduled.deadline;
        sleep_until(deadline).await;
        writer
            .write_all(scheduled.frame.as_ref())
            .await
            .map_err(VpnFrameIoError::Io)?;
        if let Some(adapter) = adapter {
            adapter.record_egress_frame_count(u64::from(scheduled.payload_len), scheduled.is_cover);
        } else if let Some(session) = session {
            session
                .metrics()
                .record_vpn_frame_egress_count(1, scheduled.is_cover);
            session.record_egress(u64::from(scheduled.payload_len), scheduled.is_cover);
        }
    }
    Ok(())
}

/// Tracks VPN session accounting for the tunnel runtime.
#[derive(Debug, Clone)]
pub struct VpnSession {
    metrics: Arc<Metrics>,
    state: Arc<VpnSessionState>,
}

#[derive(Debug)]
struct VpnSessionState {
    ingress_bytes: AtomicU64,
    egress_bytes: AtomicU64,
    cover_bytes: AtomicU64,
    last_ingress_sequence: AtomicU64,
    last_egress_sequence: AtomicU64,
    started_at: Instant,
    started_at_ms: u64,
}

impl VpnSession {
    /// Construct a session without incrementing the session counter. Intended for tests or
    /// adapters that already bumped the session meter elsewhere.
    pub fn from_parts(metrics: Arc<Metrics>) -> Self {
        Self {
            metrics,
            state: Arc::new(VpnSessionState {
                ingress_bytes: AtomicU64::new(0),
                egress_bytes: AtomicU64::new(0),
                cover_bytes: AtomicU64::new(0),
                last_ingress_sequence: AtomicU64::new(u64::MAX),
                last_egress_sequence: AtomicU64::new(u64::MAX),
                started_at: Instant::now(),
                started_at_ms: unix_now_ms(),
            }),
        }
    }

    /// Record aggregate byte counts against the session.
    pub fn record_bytes(&self, bytes: u64) {
        self.metrics.record_vpn_bytes(bytes);
    }

    /// Expose the metrics registry backing this session.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn record_ingress(&self, bytes: u64, is_cover: bool) {
        self.metrics.record_vpn_ingress(bytes, is_cover);
        self.state.ingress_bytes.fetch_add(bytes, Ordering::Relaxed);
        if is_cover {
            self.state.cover_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    pub fn record_egress(&self, bytes: u64, is_cover: bool) {
        self.metrics.record_vpn_egress(bytes, is_cover);
        self.state.egress_bytes.fetch_add(bytes, Ordering::Relaxed);
        if is_cover {
            self.state.cover_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_classified_ingress(&self, class: VpnCellClassV1, payload_len: u64) {
        match class {
            VpnCellClassV1::Data => {
                self.metrics.record_vpn_frame_ingress(false);
                self.record_ingress(payload_len, false);
            }
            VpnCellClassV1::Cover => {
                self.metrics.record_vpn_frame_ingress(true);
                self.record_ingress(payload_len, true);
            }
            VpnCellClassV1::KeepAlive | VpnCellClassV1::Control => {
                self.metrics.record_vpn_control_ingress(payload_len);
            }
        }
    }

    pub(crate) fn record_classified_egress(&self, class: VpnCellClassV1, payload_len: u64) {
        match class {
            VpnCellClassV1::Data => {
                self.metrics.record_vpn_frame_egress(false);
                self.record_egress(payload_len, false);
            }
            VpnCellClassV1::Cover => {
                self.metrics.record_vpn_frame_egress(true);
                self.record_egress(payload_len, true);
            }
            VpnCellClassV1::KeepAlive | VpnCellClassV1::Control => {
                self.metrics.record_vpn_control_egress(payload_len);
            }
        }
    }

    /// Parse and account for an ingress VPN frame. Returns the parsed cell on success.
    ///
    /// Control/keepalive cells are tracked via control metrics and excluded from receipts.
    pub fn record_frame_ingress(
        &self,
        overlay: &VpnOverlay,
        frame: &[u8],
    ) -> Result<VpnCellV1, VpnCellError> {
        let cell = overlay.parse_frame(frame)?;
        self.record_ingress_sequence(cell.header.sequence)?;
        self.record_parsed_ingress(&cell);
        Ok(cell)
    }

    /// Parse and account for an egress VPN frame. Returns the parsed cell on success.
    ///
    /// Control/keepalive cells are tracked via control metrics and excluded from receipts.
    pub fn record_frame_egress(
        &self,
        overlay: &VpnOverlay,
        frame: &[u8],
    ) -> Result<VpnCellV1, VpnCellError> {
        let cell = overlay.parse_frame(frame)?;
        self.record_egress_sequence(cell.header.sequence)?;
        self.record_parsed_egress(&cell);
        Ok(cell)
    }

    pub(crate) fn record_ingress_sequence(&self, sequence: u64) -> Result<(), VpnCellError> {
        record_monotonic_sequence(&self.state.last_ingress_sequence, sequence)
    }

    pub(crate) fn record_egress_sequence(&self, sequence: u64) -> Result<(), VpnCellError> {
        record_monotonic_sequence(&self.state.last_egress_sequence, sequence)
    }

    /// Account for a parsed ingress cell without re-validating the frame.
    pub(crate) fn record_parsed_ingress(&self, cell: &VpnCellV1) {
        self.record_classified_ingress(cell.header.class, cell.payload.len() as u64);
    }

    /// Account for a parsed egress cell without re-validating the frame.
    pub(crate) fn record_parsed_egress(&self, cell: &VpnCellV1) {
        self.record_classified_egress(cell.header.class, cell.payload.len() as u64);
    }

    /// Finalize the session into a telemetry/billing receipt.
    pub fn finish_receipt(
        &self,
        session_id: [u8; 16],
        exit_class: VpnExitClassV1,
        meter_hash: [u8; 32],
    ) -> VpnSessionReceiptV1 {
        let uptime_secs = self
            .state
            .started_at
            .elapsed()
            .as_secs()
            .min(u64::from(u32::MAX)) as u32;
        VpnSessionReceiptV1 {
            session_id,
            quote_id: [0u8; 32],
            payment_tx_hash: [0u8; 32],
            account_hash: [0u8; 32],
            relay_id: [0u8; 32],
            ingress_bytes: self.state.ingress_bytes.load(Ordering::Relaxed),
            egress_bytes: self.state.egress_bytes.load(Ordering::Relaxed),
            cover_bytes: self.state.cover_bytes.load(Ordering::Relaxed),
            uptime_secs,
            started_at_ms: self.state.started_at_ms,
            ended_at_ms: unix_now_ms(),
            exit_class,
            meter_hash,
            earned_fee_nanos: 0,
            highest_voucher_sequence: 0,
            client_voucher_hash: [0u8; 32],
        }
    }
}

fn record_monotonic_sequence(last: &AtomicU64, sequence: u64) -> Result<(), VpnCellError> {
    loop {
        let previous = last.load(Ordering::Acquire);
        if previous != u64::MAX && sequence <= previous {
            return Err(VpnCellError::NonMonotonicSequence {
                last: previous,
                actual: sequence,
            });
        }
        if last
            .compare_exchange(previous, sequence, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Ok(());
        }
    }
}

/// Session handle that carries receipt metadata alongside accounting.
#[derive(Debug, Clone)]
pub struct VpnSessionHandle {
    session: VpnSession,
    session_id: [u8; 16],
    quote_id: [u8; 32],
    account_hash: [u8; 32],
    relay_id: [u8; 32],
    payment_tx_hash: [u8; 32],
    highest_voucher: Arc<Mutex<Option<VpnUsageVoucherEnvelopeV1>>>,
    exit_class: VpnExitClassV1,
    meter_hash: [u8; 32],
}

/// Operator settlement payload emitted when a VPN session has an accepted client voucher.
#[derive(Debug, Clone)]
pub struct VpnSettlementArtifact {
    /// Relay receipt committing to the highest accepted client voucher.
    pub receipt: VpnSessionReceiptV1,
    /// Highest client-signed voucher accepted by the relay for this session.
    pub voucher: VpnUsageVoucherV1,
    /// Earned fee carried by the accepted voucher envelope.
    pub earned_fee_nanos: u64,
}

impl VpnSessionHandle {
    pub fn new(
        session: VpnSession,
        session_id: [u8; 16],
        exit_class: VpnExitClassV1,
        meter_hash: [u8; 32],
    ) -> Self {
        Self {
            session,
            session_id,
            quote_id: [0u8; 32],
            account_hash: [0u8; 32],
            relay_id: [0u8; 32],
            payment_tx_hash: [0u8; 32],
            highest_voucher: Arc::new(Mutex::new(None)),
            exit_class,
            meter_hash,
        }
    }

    pub fn from_helper_ticket(
        session: VpnSession,
        helper_ticket: VpnHelperTicketV1,
        exit_class: VpnExitClassV1,
        meter_hash: [u8; 32],
    ) -> Self {
        Self {
            session,
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            account_hash: helper_ticket.account_hash,
            relay_id: helper_ticket.relay_id,
            payment_tx_hash: helper_ticket.payment_tx_hash,
            highest_voucher: Arc::new(Mutex::new(None)),
            exit_class,
            meter_hash,
        }
    }

    pub fn session(&self) -> &VpnSession {
        &self.session
    }

    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }

    /// Record the highest client-signed usage voucher accepted by the relay.
    pub fn record_usage_voucher(&self, envelope: VpnUsageVoucherEnvelopeV1) {
        let mut highest = self
            .highest_voucher
            .lock()
            .expect("vpn voucher mutex should not be poisoned");
        let should_replace = highest
            .as_ref()
            .map(|current| envelope.voucher.body.sequence > current.voucher.body.sequence)
            .unwrap_or(true);
        if should_replace {
            *highest = Some(envelope);
        }
    }

    fn finalize_receipt(&self, voucher: Option<&VpnUsageVoucherEnvelopeV1>) -> VpnSessionReceiptV1 {
        let mut receipt =
            self.session
                .finish_receipt(self.session_id, self.exit_class, self.meter_hash);
        receipt.quote_id = self.quote_id;
        receipt.account_hash = self.account_hash;
        receipt.relay_id = self.relay_id;
        receipt.payment_tx_hash = self.payment_tx_hash;
        if let Some(envelope) = voucher {
            receipt.earned_fee_nanos = envelope.earned_fee_nanos;
            receipt.highest_voucher_sequence = envelope.voucher.body.sequence;
            receipt.client_voucher_hash = envelope.voucher.hash();
        }
        receipt
    }

    /// Finalize the handle into a billing/telemetry receipt.
    pub fn receipt(&self) -> VpnSessionReceiptV1 {
        let voucher = self
            .highest_voucher
            .lock()
            .expect("vpn voucher mutex should not be poisoned")
            .clone();
        self.finalize_receipt(voucher.as_ref())
    }

    /// Finalize the handle into an operator settlement artifact if a voucher was accepted.
    pub fn settlement_artifact(&self) -> Option<VpnSettlementArtifact> {
        let voucher = self
            .highest_voucher
            .lock()
            .expect("vpn voucher mutex should not be poisoned")
            .clone()?;
        Some(VpnSettlementArtifact {
            receipt: self.finalize_receipt(Some(&voucher)),
            voucher: voucher.voucher,
            earned_fee_nanos: voucher.earned_fee_nanos,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, UNIX_EPOCH},
    };

    use crate::metrics::Metrics;

    use super::*;

    #[test]
    fn unix_time_ms_saturates_pre_epoch_clock() {
        assert_eq!(unix_time_ms(UNIX_EPOCH - Duration::from_secs(1)), 0);
        assert_eq!(unix_time_ms(UNIX_EPOCH + Duration::from_millis(42)), 42);
    }

    #[test]
    fn cover_plan_allows_full_ratio() {
        let frames = 64usize;
        let mut cfg = VpnConfig::default();
        cfg.cover.enabled = true;
        cfg.cover.cover_to_data_per_mille = 1_000;
        cfg.cover.max_cover_burst = frames as u16;
        cfg.cover.heartbeat_ms = 1;
        cfg.cover.max_jitter_millis = 0;

        let mut capped = cfg.clone();
        capped.cover.cover_to_data_per_mille = 999;

        let mut found = false;
        let mut plan_full = Vec::new();
        let mut plan_capped = Vec::new();
        for byte in 0u8..=255 {
            let seed = [byte; 32];
            plan_full = cover_plan_from_config(&cfg, frames, seed);
            plan_capped = cover_plan_from_config(&capped, frames, seed);
            if plan_full != plan_capped {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "expected to find a seed that distinguishes 1000 from 999"
        );
        assert!(plan_full.iter().all(|entry| entry.is_cover));
        assert!(plan_capped.iter().any(|entry| !entry.is_cover));
    }

    #[test]
    fn session_records_cover_bytes_from_manual_counts() {
        let metrics = Arc::new(Metrics::new());
        metrics.set_vpn_meter_labels("vpn.session", "vpn.egress.bytes");
        let session = VpnSession::from_parts(Arc::clone(&metrics));

        session.record_ingress(5, true);
        session.record_egress(7, true);

        let receipt = session.finish_receipt([0xAA; 16], VpnExitClassV1::Standard, [0xBB; 32]);
        assert_eq!(5, receipt.ingress_bytes);
        assert_eq!(7, receipt.egress_bytes);
        assert_eq!(12, receipt.cover_bytes);
    }
}
