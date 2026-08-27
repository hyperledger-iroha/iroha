//! VPN overlay configuration, parsing, and accounting hooks.
//!
//! This module wires the relay metrics into the SoraNet VPN cell format so ingress/egress
//! accounting works end-to-end while the tunnel runtime handles fixed-size framing,
//! pacing, and cover injection.
use crate::{
    config::{ConfigError, VPN_MAX_COVER_BURST_CELLS_V1, VpnConfig},
    metrics::Metrics,
    vpn_adapter::{VpnAdapter, VpnBridge},
};
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::soranet::{
    RelayId,
    vpn::{
        VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
        VpnControlPlaneV1, VpnCoverPlanEntryV1, VpnCoverScheduleV1, VpnExitClassV1, VpnFlowLabelV1,
        VpnHelperTicketV1, VpnPaddedCellV1, VpnRouteV1, VpnSessionReceiptV1,
        VpnSignedSessionReceiptV1, VpnTariffV1, VpnUsageVoucherEnvelopeV1, VpnUsageVoucherV1,
        vpn_tariff_meter_hash_v1,
    },
};
use iroha_primitives::numeric::Quantity;
use std::{
    cmp::max,
    fmt,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::{Instant as TokioInstant, sleep_until},
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
pub struct PaddedCell {
    /// Fully padded fixed-length frame.
    pub frame: VpnPaddedCellV1,
    /// Unpadded payload length carried in the header.
    pub payload_len: u16,
}
impl fmt::Debug for PaddedCell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PaddedCell")
            .field("frame", &"<redacted>")
            .field("payload_len", &self.payload_len)
            .finish()
    }
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
    /// Operating-system randomness was unavailable for cover scheduling.
    #[error("operating-system randomness is unavailable for VPN cover scheduling")]
    CoverRandomnessUnavailable,
    /// A caller attempted to install an inert cover-scheduling seed.
    #[error("VPN cover-scheduling seed must not be all zero")]
    InvalidCoverSeed,
    /// The directional frame sequence cannot advance without wrapping.
    #[error("VPN frame sequence space is exhausted")]
    SequenceExhausted,
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
    /// Per-session replay state is unavailable.
    #[error(transparent)]
    SessionState(#[from] VpnSessionStateError),
    /// Cell routing metadata did not match the authenticated VPN session.
    #[error("VPN cell does not match the authenticated session binding")]
    SessionBindingMismatch,
    /// Frame length did not match the pinned cell size.
    #[error("padded frame length {actual}B does not match expected {expected}B")]
    FrameLength { expected: usize, actual: usize },
}
/// Errors surfaced by mutable per-session replay state.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum VpnSessionStateError {
    /// Cell validation failed before its sequence could be committed.
    #[error(transparent)]
    Cell(#[from] VpnCellError),
    /// Replay state was poisoned and the session is permanently unavailable.
    #[error("VPN session replay state is unavailable")]
    StateUnavailable,
}
/// Errors surfaced by prepaid VPN billing state.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VpnBillingError {
    /// Replay or billing state was poisoned and the session is permanently unavailable.
    #[error("VPN session billing state is unavailable")]
    StateUnavailable,
    /// Settlement inputs could not produce a valid receipt.
    #[error("{0}")]
    Settlement(String),
}
impl VpnBillingError {
    fn settlement(message: impl Into<String>) -> Self {
        Self::Settlement(message.into())
    }
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
pub struct ScheduledFrame {
    /// Deadline relative to the start of the schedule.
    pub deadline: Duration,
    /// Padded frame to emit.
    pub frame: VpnPaddedCellV1,
    /// Payload length carried by the frame.
    pub payload_len: u16,
    /// Direction-wide sequence reserved before the frame write begins.
    pub sequence: u64,
    /// Whether the scheduled frame is a cover cell.
    pub is_cover: bool,
}
impl fmt::Debug for ScheduledFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScheduledFrame")
            .field("deadline", &self.deadline)
            .field("frame", &"<redacted>")
            .field("payload_len", &self.payload_len)
            .field("sequence", &self.sequence)
            .field("is_cover", &self.is_cover)
            .finish()
    }
}
/// Overlay that handles VPN cell framing, validation, and billing metadata.
pub struct VpnOverlay {
    config: VpnConfig,
    helper_ticket_issuer_public_key: Option<PublicKey>,
    backend_bootstrap_secret: Option<[u8; 32]>,
    exit_class: VpnExitClassV1,
    meter_hash: [u8; 32],
    routes: Vec<VpnRouteV1>,
    dns_overrides: Vec<String>,
}
impl std::fmt::Debug for VpnOverlay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VpnOverlay")
            .field("config", &"<redacted>")
            .field("helper_ticket_issuer_public_key", &"<pinned>")
            .field("backend_bootstrap_secret", &"<redacted>")
            .field("exit_class", &self.exit_class)
            .field("meter_hash", &"<redacted>")
            .field("route_count", &self.routes.len())
            .field("dns_override_count", &self.dns_overrides.len())
            .finish()
    }
}
impl Drop for VpnOverlay {
    fn drop(&mut self) {
        clear_vpn_overlay_secret(&mut self.backend_bootstrap_secret);
    }
}
fn clear_vpn_overlay_secret(backend_bootstrap_secret: &mut Option<[u8; 32]>) {
    if let Some(secret) = backend_bootstrap_secret {
        zeroize::Zeroize::zeroize(secret);
    }
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
        let helper_ticket_issuer_public_key = config.try_helper_ticket_issuer_public_key()?;
        let _ = config.try_backend_endpoint()?;
        let backend_bootstrap_secret = config.try_backend_bootstrap_secret_bytes()?;
        Ok(Self {
            config,
            helper_ticket_issuer_public_key,
            backend_bootstrap_secret,
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
    /// Return the startup-pinned helper-ticket issuer public key.
    pub fn helper_ticket_issuer_public_key(&self) -> Option<&PublicKey> {
        self.helper_ticket_issuer_public_key.as_ref()
    }
    /// Return the startup-loaded backend bootstrap authentication secret.
    pub fn backend_bootstrap_secret(&self) -> Option<&[u8; 32]> {
        self.backend_bootstrap_secret.as_ref()
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
        helper_ticket: &VpnHelperTicketV1,
        relay_identity_key: Arc<KeyPair>,
    ) -> Result<VpnSessionHandle, String> {
        VpnSessionHandle::from_helper_ticket(
            session,
            helper_ticket,
            self.exit_class,
            self.meter_hash,
            relay_identity_key,
        )
    }
    /// Start a new VPN session and return an adapter for recording ingress/egress.
    pub fn start_adapter(&self, metrics: Arc<Metrics>) -> VpnAdapter {
        let session = self.start_session(metrics);
        VpnAdapter::new(session, self.framing_only_overlay())
    }
    /// Start a new VPN session and return a bridge bound to the supplied identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`VpnFrameBuildError::CoverRandomnessUnavailable`] when the operating
    /// system cannot provide a fresh, nonzero cover-scheduling seed.
    pub fn start_bridge(
        &self,
        metrics: Arc<Metrics>,
        circuit_id: [u8; 16],
        flow_label: VpnFlowLabelV1,
    ) -> Result<VpnBridge, VpnFrameBuildError> {
        let adapter = self.start_adapter(metrics);
        VpnBridge::new(adapter, circuit_id, flow_label)
    }
    fn framing_only_overlay(&self) -> Self {
        Self {
            config: self.config.clone(),
            helper_ticket_issuer_public_key: None,
            backend_bootstrap_secret: None,
            exit_class: self.exit_class,
            meter_hash: self.meter_hash,
            routes: self.routes.clone(),
            dns_overrides: self.dns_overrides.clone(),
        }
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
    let mut frame = VpnPaddedCellV1::zeroed();
    let mut read = 0usize;
    while read < VPN_CELL_LEN {
        let n = reader
            .read(&mut frame.as_mut()[read..])
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
    overlay
        .parse_frame(frame.as_ref())
        .map_err(VpnFrameIoError::from)
}
/// Write a padded VPN cell frame to the provided writer.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    cell: &PaddedCell,
) -> Result<(), VpnFrameIoError> {
    write_frame_bytes(writer, cell.frame.as_ref()).await
}
async fn write_frame_bytes<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
) -> Result<(), VpnFrameIoError> {
    if bytes.len() != VPN_CELL_LEN {
        return Err(VpnFrameIoError::FrameLength {
            expected: VPN_CELL_LEN,
            actual: bytes.len(),
        });
    }
    writer.write_all(bytes).await.map_err(VpnFrameIoError::Io)?;
    // A SoraNet `RecordWriter` intentionally buffers the most recently accepted
    // authenticated record. Flush at the application-cell boundary so a final
    // or one-cell packet cannot wait for an unrelated later write.
    writer.flush().await.map_err(VpnFrameIoError::Io)
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
        max_cover_burst: config
            .cover
            .max_cover_burst
            .min(VPN_MAX_COVER_BURST_CELLS_V1),
        jitter_ms: config.cover.max_jitter_millis,
    }
    .plan(seed, frames)
}
/// Build a paced schedule that interleaves data frames with cover cells.
///
/// Every cell is assigned one contiguous, direction-wide sequence in transmission
/// order, beginning at [`CoverFrameMeta::start_sequence`]. Incoming data-cell
/// sequence values are intentionally replaced so cover and data traffic cannot
/// create overlapping replay windows.
pub fn schedule_frames(
    overlay: &VpnOverlay,
    data_cells: Vec<VpnCellV1>,
    cover_meta: CoverFrameMeta,
    seed: [u8; 32],
) -> Result<Vec<ScheduledFrame>, VpnFrameBuildError> {
    if seed.iter().all(|byte| *byte == 0) {
        return Err(VpnFrameBuildError::InvalidCoverSeed);
    }
    let data_frame_count = data_cells.len();
    let maximum_frames = if overlay.config.cover.enabled {
        let cover_burst = overlay
            .config
            .cover
            .max_cover_burst
            .min(VPN_MAX_COVER_BURST_CELLS_V1);
        data_frame_count
            .checked_mul(usize::from(cover_burst) + 1)
            .ok_or(VpnFrameBuildError::SequenceExhausted)?
    } else {
        data_frame_count
    };
    // The deterministic plan is prefix-stable. Generate the bounded worst-case
    // prefix once, then retain the shortest prefix containing every data slot.
    // Rebuilding every one-frame extension made scheduling quadratic.
    let mut plan = cover_plan_from_config(&overlay.config, maximum_frames, seed);
    let total_frames = if data_frame_count == 0 {
        0
    } else {
        let mut data_slots = 0usize;
        plan.iter()
            .position(|entry| {
                if !entry.is_cover {
                    data_slots = data_slots.saturating_add(1);
                }
                data_slots == data_frame_count
            })
            .map(|index| index + 1)
            .ok_or(VpnFrameBuildError::SequenceExhausted)?
    };
    plan.truncate(total_frames);
    let frame_count =
        u64::try_from(total_frames).map_err(|_| VpnFrameBuildError::SequenceExhausted)?;
    cover_meta
        .start_sequence
        .checked_add(frame_count)
        .ok_or(VpnFrameBuildError::SequenceExhausted)?;
    let mut data_iter = data_cells.into_iter();
    let mut schedule = Vec::with_capacity(total_frames);
    let mut sequence = cover_meta.start_sequence;
    let mut last_deadline_ms: u64 = 0;
    for (idx, entry) in plan.into_iter().enumerate() {
        let assigned_sequence = sequence;
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
            (overlay.cover_frame(&cover_meta, sequence)?, true)
        } else if let Some(mut data) = data_iter.next() {
            data.header.sequence = sequence;
            (overlay.pad_cell(data)?, false)
        } else {
            (overlay.cover_frame(&cover_meta, sequence)?, true)
        };
        sequence = sequence
            .checked_add(1)
            .ok_or(VpnFrameBuildError::SequenceExhausted)?;
        last_deadline_ms = deadline_ms;
        schedule.push(ScheduledFrame {
            deadline: Duration::from_millis(deadline_ms),
            payload_len: prepared.payload_len,
            frame: prepared.frame,
            sequence: assigned_sequence,
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
        // Burn the sequence before attempting I/O. Reusing it after an
        // ambiguous partial write would violate the direction-wide replay
        // invariant; accounting is still committed only after a full write.
        if let Some(adapter) = adapter {
            adapter
                .session()
                .record_egress_sequence(scheduled.sequence)?;
        } else if let Some(session) = session {
            session.record_egress_sequence(scheduled.sequence)?;
        }
        write_frame_bytes(writer, scheduled.frame.as_ref()).await?;
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
    last_ingress_sequence: Mutex<Option<u64>>,
    last_egress_sequence: Mutex<Option<u64>>,
    unavailable: AtomicBool,
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
                last_ingress_sequence: Mutex::new(None),
                last_egress_sequence: Mutex::new(None),
                unavailable: AtomicBool::new(false),
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
        atomic_saturating_add(&self.state.ingress_bytes, bytes);
        if is_cover {
            atomic_saturating_add(&self.state.cover_bytes, bytes);
        }
    }
    pub fn record_egress(&self, bytes: u64, is_cover: bool) {
        self.metrics.record_vpn_egress(bytes, is_cover);
        atomic_saturating_add(&self.state.egress_bytes, bytes);
        if is_cover {
            atomic_saturating_add(&self.state.cover_bytes, bytes);
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
    ) -> Result<VpnCellV1, VpnSessionStateError> {
        let cell = overlay
            .parse_frame(frame)
            .map_err(VpnSessionStateError::Cell)?;
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
    ) -> Result<VpnCellV1, VpnSessionStateError> {
        let cell = overlay
            .parse_frame(frame)
            .map_err(VpnSessionStateError::Cell)?;
        self.record_egress_sequence(cell.header.sequence)?;
        self.record_parsed_egress(&cell);
        Ok(cell)
    }
    pub(crate) fn record_ingress_sequence(
        &self,
        sequence: u64,
    ) -> Result<(), VpnSessionStateError> {
        record_monotonic_sequence(
            &self.state.last_ingress_sequence,
            &self.state.unavailable,
            sequence,
        )
    }
    pub(crate) fn record_egress_sequence(&self, sequence: u64) -> Result<(), VpnSessionStateError> {
        record_monotonic_sequence(
            &self.state.last_egress_sequence,
            &self.state.unavailable,
            sequence,
        )
    }
    pub(crate) fn ensure_state_available(&self) -> Result<(), VpnSessionStateError> {
        if self.state.unavailable.load(Ordering::Acquire)
            || self.state.last_ingress_sequence.is_poisoned()
            || self.state.last_egress_sequence.is_poisoned()
        {
            self.state.unavailable.store(true, Ordering::Release);
            return Err(VpnSessionStateError::StateUnavailable);
        }
        Ok(())
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
        // Round the monotonic duration up so a sub-second client voucher can
        // never exceed the receipt's coarse whole-second authorization bound.
        let elapsed = self.state.started_at.elapsed();
        let uptime_secs = elapsed
            .as_secs()
            .saturating_add(u64::from(elapsed.subsec_nanos() != 0))
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
            earned_fee: Quantity::zero(),
            highest_voucher_sequence: 0,
            client_voucher_hash: [0u8; 32],
        }
    }
}
fn atomic_saturating_add(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}
fn record_monotonic_sequence(
    last: &Mutex<Option<u64>>,
    unavailable: &AtomicBool,
    sequence: u64,
) -> Result<(), VpnSessionStateError> {
    if unavailable.load(Ordering::Acquire) {
        return Err(VpnSessionStateError::StateUnavailable);
    }
    let mut guard = match last.lock() {
        Ok(guard) => guard,
        Err(_) => {
            unavailable.store(true, Ordering::Release);
            return Err(VpnSessionStateError::StateUnavailable);
        }
    };
    if unavailable.load(Ordering::Acquire) {
        return Err(VpnSessionStateError::StateUnavailable);
    }
    if let Some(previous) = *guard
        && sequence <= previous
    {
        return Err(VpnSessionStateError::Cell(
            VpnCellError::NonMonotonicSequence {
                last: previous,
                actual: sequence,
            },
        ));
    }
    *guard = Some(sequence);
    Ok(())
}
/// Session handle that carries receipt metadata alongside accounting.
#[derive(Debug, Default)]
struct VpnMeteredServiceWindow {
    started_at_ms: Option<u64>,
    started_at: Option<Instant>,
    elapsed_ms: Option<u64>,
}
#[derive(Debug, Default)]
struct VpnMeteredUsage {
    ingress_bytes: AtomicU64,
    egress_bytes: AtomicU64,
    service_window: Mutex<VpnMeteredServiceWindow>,
    unavailable: AtomicBool,
}
fn lock_billing_state<'a, T>(
    state: &'a Mutex<T>,
    unavailable: &AtomicBool,
) -> Result<MutexGuard<'a, T>, VpnBillingError> {
    if unavailable.load(Ordering::Acquire) {
        return Err(VpnBillingError::StateUnavailable);
    }
    let guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => {
            unavailable.store(true, Ordering::Release);
            return Err(VpnBillingError::StateUnavailable);
        }
    };
    if unavailable.load(Ordering::Acquire) {
        return Err(VpnBillingError::StateUnavailable);
    }
    Ok(guard)
}
#[derive(Clone)]
pub struct VpnSessionHandle {
    session: VpnSession,
    session_id: [u8; 16],
    quote_id: [u8; 32],
    lease_id: [u8; 32],
    account_hash: [u8; 32],
    relay_id: [u8; 32],
    payment_tx_hash: [u8; 32],
    valid_after_ms: u64,
    expires_at_ms: u64,
    highest_voucher: Arc<Mutex<Option<VpnUsageVoucherEnvelopeV1>>>,
    tariff: Option<VpnTariffV1>,
    metered_usage: Arc<VpnMeteredUsage>,
    exit_class: VpnExitClassV1,
    meter_hash: [u8; 32],
    relay_identity_key: Option<Arc<KeyPair>>,
}
impl fmt::Debug for VpnSessionHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VpnSessionHandle")
            .field("session", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("quote_id", &"<redacted>")
            .field("lease_id", &"<redacted>")
            .field("account_hash", &"<redacted>")
            .field("relay_id", &"<redacted>")
            .field("payment_tx_hash", &"<redacted>")
            .field("valid_after_ms", &"<redacted>")
            .field("expires_at_ms", &"<redacted>")
            .field("highest_voucher", &"<redacted>")
            .field("tariff", &"<redacted>")
            .field("metered_usage", &"<redacted>")
            .field("exit_class", &self.exit_class)
            .field("meter_hash", &"<redacted>")
            .field("relay_identity_key", &"<redacted>")
            .finish()
    }
}
/// Operator settlement payload emitted when a VPN session has an accepted client voucher.
#[derive(Clone)]
pub struct VpnSettlementArtifact {
    /// Consensus lease identifier authorized by the operator-signed helper ticket.
    pub lease_id: [u8; 32],
    /// Relay receipt committing to the highest accepted client voucher.
    pub receipt: VpnSignedSessionReceiptV1,
    /// Highest client-signed voucher accepted by the relay for this session.
    pub voucher: VpnUsageVoucherV1,
    /// Earned fee computed from actual receipt usage within the prepaid ceilings.
    pub earned_fee: Quantity,
}
impl fmt::Debug for VpnSettlementArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VpnSettlementArtifact")
            .field("lease_id", &"<redacted>")
            .field("receipt", &"<redacted>")
            .field("voucher", &"<redacted>")
            .field("earned_fee", &"<redacted>")
            .finish()
    }
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
            lease_id: [0u8; 32],
            account_hash: [0u8; 32],
            relay_id: [0u8; 32],
            payment_tx_hash: [0u8; 32],
            valid_after_ms: 0,
            expires_at_ms: u64::MAX,
            highest_voucher: Arc::new(Mutex::new(None)),
            tariff: None,
            metered_usage: Arc::new(VpnMeteredUsage::default()),
            exit_class,
            meter_hash,
            relay_identity_key: None,
        }
    }
    pub fn from_helper_ticket(
        session: VpnSession,
        helper_ticket: &VpnHelperTicketV1,
        exit_class: VpnExitClassV1,
        _meter_hash: [u8; 32],
        relay_identity_key: Arc<KeyPair>,
    ) -> Result<Self, String> {
        let (relay_algorithm, relay_public_key) = relay_identity_key
            .public_key()
            .try_to_bytes()
            .map_err(|error| format!("vpn relay identity key is malformed: {error}"))?;
        if relay_algorithm != Algorithm::Ed25519 {
            return Err("vpn relay identity key must use Ed25519".to_owned());
        }
        if relay_public_key != helper_ticket.relay_id {
            return Err(
                "vpn relay identity key does not match the helper-ticket relay id".to_owned(),
            );
        }
        let meter_hash = vpn_tariff_meter_hash_v1(&helper_ticket.tariff);
        let tariff = helper_ticket.tariff.clone();
        Ok(Self {
            session,
            session_id: helper_ticket.session_id,
            quote_id: helper_ticket.quote_id,
            lease_id: helper_ticket.lease_id,
            account_hash: helper_ticket.account_hash,
            relay_id: helper_ticket.relay_id,
            payment_tx_hash: helper_ticket.payment_tx_hash,
            valid_after_ms: helper_ticket.valid_after_ms,
            expires_at_ms: helper_ticket.expires_at_ms,
            highest_voucher: Arc::new(Mutex::new(None)),
            tariff: Some(tariff),
            metered_usage: Arc::new(VpnMeteredUsage::default()),
            exit_class,
            meter_hash,
            relay_identity_key: Some(relay_identity_key),
        })
    }
    pub fn session(&self) -> &VpnSession {
        &self.session
    }
    pub fn session_id(&self) -> [u8; 16] {
        self.session_id
    }
    /// Return the signed lower timestamp bound retained for WAL recovery validation.
    pub(crate) fn valid_after_ms(&self) -> u64 {
        self.valid_after_ms
    }
    /// Return the signed exclusive session expiry retained for WAL recovery validation.
    pub(crate) fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }
    /// Return the signed tariff retained for durable settlement recovery.
    pub(crate) fn tariff(&self) -> Option<&VpnTariffV1> {
        self.tariff.as_ref()
    }
    /// Reject forwarding and settlement after any replay or billing lock is poisoned.
    pub(crate) fn ensure_forwarding_available(&self) -> Result<(), VpnBillingError> {
        if self.session.ensure_state_available().is_err()
            || self.metered_usage.unavailable.load(Ordering::Acquire)
            || self.highest_voucher.is_poisoned()
            || self.metered_usage.service_window.is_poisoned()
        {
            self.metered_usage
                .unavailable
                .store(true, Ordering::Release);
            return Err(VpnBillingError::StateUnavailable);
        }
        Ok(())
    }
    /// Start the billable service interval after prepaid admission and backend readiness.
    pub(crate) fn begin_metered_service(&self, started_at_ms: u64) -> Result<(), VpnBillingError> {
        self.ensure_forwarding_available()?;
        let started_at_ms =
            lock_billing_state(&self.highest_voucher, &self.metered_usage.unavailable)?
                .as_ref()
                .map_or(started_at_ms, |envelope| {
                    started_at_ms.max(envelope.voucher.body.issued_at_ms)
                });
        let mut window = lock_billing_state(
            &self.metered_usage.service_window,
            &self.metered_usage.unavailable,
        )?;
        if window.started_at_ms.is_none() {
            window.started_at_ms = Some(started_at_ms);
            window.started_at = Some(Instant::now());
        }
        Ok(())
    }
    /// End the billable service interval as soon as forwarding stops.
    ///
    /// The wall-clock sample is intentionally ignored for billing duration;
    /// only monotonic elapsed time can survive a clock rollback without
    /// under-reporting service.
    pub(crate) fn end_metered_service(&self, _ended_at_ms: u64) -> Result<(), VpnBillingError> {
        self.ensure_forwarding_available()?;
        let mut window = lock_billing_state(
            &self.metered_usage.service_window,
            &self.metered_usage.unavailable,
        )?;
        if window.elapsed_ms.is_none()
            && let Some(started_at) = window.started_at
        {
            window.elapsed_ms =
                Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
        }
        Ok(())
    }
    /// Record one client-to-relay user packet after it was forwarded successfully.
    pub(crate) fn record_metered_ingress(&self, bytes: u64) -> Result<(), VpnBillingError> {
        self.ensure_forwarding_available()?;
        atomic_saturating_add(&self.metered_usage.ingress_bytes, bytes);
        Ok(())
    }
    /// Record one relay-to-client user packet after it was forwarded successfully.
    pub(crate) fn record_metered_egress(&self, bytes: u64) -> Result<(), VpnBillingError> {
        self.ensure_forwarding_available()?;
        atomic_saturating_add(&self.metered_usage.egress_bytes, bytes);
        Ok(())
    }
    /// Record the highest client-signed usage voucher accepted by the relay.
    pub fn record_usage_voucher(
        &self,
        envelope: VpnUsageVoucherEnvelopeV1,
    ) -> Result<(), VpnBillingError> {
        self.ensure_forwarding_available()?;
        let mut highest =
            lock_billing_state(&self.highest_voucher, &self.metered_usage.unavailable)?;
        let should_replace = highest
            .as_ref()
            .map(|current| envelope.voucher.body.sequence > current.voucher.body.sequence)
            .unwrap_or(true);
        if should_replace {
            *highest = Some(envelope);
        }
        Ok(())
    }
    fn finalize_receipt(
        &self,
        voucher: Option<&VpnUsageVoucherEnvelopeV1>,
    ) -> Result<VpnSessionReceiptV1, VpnBillingError> {
        self.ensure_forwarding_available()?;
        let mut receipt =
            self.session
                .finish_receipt(self.session_id, self.exit_class, self.meter_hash);
        receipt.ended_at_ms = receipt.ended_at_ms.min(self.expires_at_ms);
        receipt.uptime_secs = receipt
            .ended_at_ms
            .saturating_sub(receipt.started_at_ms)
            .div_ceil(1_000)
            .min(u64::from(u32::MAX)) as u32;
        receipt.quote_id = self.quote_id;
        receipt.account_hash = self.account_hash;
        receipt.relay_id = self.relay_id;
        receipt.payment_tx_hash = self.payment_tx_hash;
        if let Some(envelope) = voucher {
            let body = &envelope.voucher.body;
            let service_window = lock_billing_state(
                &self.metered_usage.service_window,
                &self.metered_usage.unavailable,
            )?;
            let (started_at_ms, observed_active_ms) =
                match (service_window.started_at_ms, service_window.started_at) {
                    (Some(started_at_ms), Some(started_at)) => (
                        started_at_ms,
                        service_window.elapsed_ms.unwrap_or_else(|| {
                            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
                        }),
                    ),
                    _ => (body.issued_at_ms, 0),
                };
            let active_ms = observed_active_ms
                .min(body.active_ms)
                .min(self.expires_at_ms.saturating_sub(started_at_ms));
            let ended_at_ms = started_at_ms.saturating_add(active_ms);
            let ingress_bytes = self.metered_usage.ingress_bytes.load(Ordering::Relaxed);
            let egress_bytes = self.metered_usage.egress_bytes.load(Ordering::Relaxed);
            if !body.authorizes(ingress_bytes, egress_bytes, active_ms) {
                return Err(VpnBillingError::settlement(
                    "relay-observed VPN usage exceeds the accepted prepaid voucher",
                ));
            }
            // Settlement reports relay-observed payload and time, all bounded
            // by the highest client-signed prepaid voucher. Cover accounting
            // remains local telemetry and is not consensus evidence.
            receipt.ingress_bytes = ingress_bytes;
            receipt.egress_bytes = egress_bytes;
            receipt.cover_bytes = 0;
            receipt.started_at_ms = started_at_ms;
            receipt.ended_at_ms = ended_at_ms;
            receipt.uptime_secs = u32::try_from(active_ms.div_ceil(1_000).min(u64::from(u32::MAX)))
                .map_err(|_| {
                    VpnBillingError::settlement(
                        "vpn settlement active time exceeds the receipt range",
                    )
                })?;
            receipt.meter_hash = self.meter_hash;
            receipt.earned_fee = match self.tariff.as_ref() {
                Some(tariff) => tariff
                    .fee_for_usage(ingress_bytes, egress_bytes, active_ms)
                    .map_err(|error| {
                        VpnBillingError::settlement(format!(
                            "vpn settlement tariff arithmetic failed: {error}"
                        ))
                    })?,
                None => Quantity::zero(),
            };
            receipt.highest_voucher_sequence = body.sequence;
            receipt.client_voucher_hash = envelope.voucher.hash();
        }
        Ok(receipt)
    }
    fn sign_settlement_receipt(
        &self,
        receipt: VpnSessionReceiptV1,
    ) -> Result<VpnSignedSessionReceiptV1, VpnBillingError> {
        let relay_identity_key = self.relay_identity_key.as_ref().ok_or_else(|| {
            VpnBillingError::settlement(
                "vpn settlement receipt is missing the relay identity signer",
            )
        })?;
        VpnSignedSessionReceiptV1::try_sign(receipt, relay_identity_key.private_key()).map_err(
            |error| {
                VpnBillingError::settlement(format!(
                    "vpn settlement receipt signing failed: {error}"
                ))
            },
        )
    }
    /// Finalize the handle into a billing/telemetry receipt.
    pub fn receipt(&self) -> Result<VpnSessionReceiptV1, VpnBillingError> {
        self.ensure_forwarding_available()?;
        let voucher =
            lock_billing_state(&self.highest_voucher, &self.metered_usage.unavailable)?.clone();
        self.finalize_receipt(voucher.as_ref())
    }
    /// Build the zero-usage receipt retained in the crash-recovery WAL.
    ///
    /// The first release never reserves a client's prepaid ceilings as earned
    /// service. A crash therefore recovers zero usage; only graceful
    /// finalization may persist relay-observed bytes and elapsed service time.
    pub(crate) fn pre_service_settlement_artifact(
        &self,
        envelope: &VpnUsageVoucherEnvelopeV1,
    ) -> Result<VpnSettlementArtifact, VpnBillingError> {
        self.ensure_forwarding_available()?;
        let tariff = self.tariff.as_ref().ok_or_else(|| {
            VpnBillingError::settlement("vpn settlement reservation is missing the signed tariff")
        })?;
        let body = &envelope.voucher.body;
        let mut receipt = self.finalize_receipt(Some(envelope))?;
        let (started_at_ms, ingress_bytes, egress_bytes, active_ms) = (body.issued_at_ms, 0, 0, 0);
        let ended_at_ms = started_at_ms.checked_add(active_ms).ok_or_else(|| {
            VpnBillingError::settlement("vpn settlement reservation timestamp overflowed")
        })?;
        if started_at_ms < self.valid_after_ms || ended_at_ms > self.expires_at_ms {
            return Err(VpnBillingError::settlement(
                "vpn settlement reservation falls outside the signed helper-ticket window",
            ));
        }
        if body.issued_at_ms < self.valid_after_ms
            || body.issued_at_ms >= self.expires_at_ms
            || body.issued_at_ms > ended_at_ms
        {
            return Err(VpnBillingError::settlement(
                "vpn settlement reservation cannot project a consensus-valid voucher timestamp",
            ));
        }
        if !body.authorizes(ingress_bytes, egress_bytes, active_ms) {
            return Err(VpnBillingError::settlement(
                "vpn settlement reservation exceeds the signed prepaid ceilings",
            ));
        }
        let earned_fee = tariff
            .fee_for_usage(ingress_bytes, egress_bytes, active_ms)
            .map_err(|error| {
                VpnBillingError::settlement(format!(
                    "vpn settlement reservation tariff arithmetic failed: {error}"
                ))
            })?;
        receipt.ingress_bytes = ingress_bytes;
        receipt.egress_bytes = egress_bytes;
        receipt.cover_bytes = 0;
        receipt.started_at_ms = started_at_ms;
        receipt.ended_at_ms = ended_at_ms;
        receipt.uptime_secs = u32::try_from(active_ms.div_ceil(1_000)).map_err(|_| {
            VpnBillingError::settlement(
                "vpn settlement reservation active time exceeds the receipt range",
            )
        })?;
        receipt.meter_hash = self.meter_hash;
        receipt.earned_fee = earned_fee.clone();
        receipt.highest_voucher_sequence = body.sequence;
        receipt.client_voucher_hash = envelope.voucher.hash();
        let receipt = self.sign_settlement_receipt(receipt)?;
        Ok(VpnSettlementArtifact {
            lease_id: self.lease_id,
            receipt,
            voucher: envelope.voucher.clone(),
            earned_fee,
        })
    }
    /// Finalize the handle into an operator settlement artifact if a voucher was accepted.
    pub fn settlement_artifact(&self) -> Result<Option<VpnSettlementArtifact>, VpnBillingError> {
        self.ensure_forwarding_available()?;
        let voucher =
            lock_billing_state(&self.highest_voucher, &self.metered_usage.unavailable)?.clone();
        let Some(voucher) = voucher else {
            return Ok(None);
        };
        let receipt = self.finalize_receipt(Some(&voucher))?;
        let earned_fee = receipt.earned_fee.clone();
        let receipt = self.sign_settlement_receipt(receipt)?;
        Ok(Some(VpnSettlementArtifact {
            lease_id: self.lease_id,
            earned_fee,
            receipt,
            voucher: voucher.voucher,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::soranet::vpn::VpnUsageVoucherBodyV1;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::{Duration, UNIX_EPOCH},
    };

    struct FlushFailWriter;

    impl AsyncWrite for FlushFailWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Ready(Ok(buffer.len()))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Err(std::io::Error::other("fixture flush failure")))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    fn poison<T>(mutex: &Mutex<T>) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = mutex.lock().expect("unpoisoned fixture mutex");
            panic!("poison security-state fixture mutex");
        }));
        assert!(result.is_err());
    }

    #[test]
    fn poisoned_replay_state_permanently_rejects_the_session() {
        let session = VpnSession::from_parts(Arc::new(Metrics::new()));
        poison(&session.state.last_ingress_sequence);

        assert_eq!(
            session.record_ingress_sequence(1),
            Err(VpnSessionStateError::StateUnavailable)
        );
        session.state.last_ingress_sequence.clear_poison();
        assert_eq!(
            session.record_egress_sequence(1),
            Err(VpnSessionStateError::StateUnavailable),
            "clearing the mutex poison must not reopen the session"
        );
    }

    #[tokio::test]
    async fn scheduled_frame_flush_failure_burns_sequence_without_accounting() {
        let session = VpnSession::from_parts(Arc::new(Metrics::new()));
        let schedule = [ScheduledFrame {
            deadline: Duration::ZERO,
            frame: VpnPaddedCellV1::zeroed(),
            payload_len: 17,
            sequence: 42,
            is_cover: false,
        }];
        let error = send_scheduled_frames(&schedule, &mut FlushFailWriter, Some(&session))
            .await
            .expect_err("a failed transport flush must abort the schedule");
        assert!(matches!(error, VpnFrameIoError::Io(_)));
        assert_eq!(
            *session
                .state
                .last_egress_sequence
                .lock()
                .expect("sequence fixture lock"),
            Some(42),
            "an ambiguously written sequence must never be reused"
        );
        assert_eq!(session.state.egress_bytes.load(Ordering::Relaxed), 0);
        assert_eq!(session.state.cover_bytes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn poisoned_billing_state_permanently_blocks_forwarding_and_settlement() {
        let handle = VpnSessionHandle::new(
            VpnSession::from_parts(Arc::new(Metrics::new())),
            [0x11; 16],
            VpnExitClassV1::Standard,
            [0x22; 32],
        );
        poison(&handle.highest_voucher);

        assert_eq!(
            handle.ensure_forwarding_available(),
            Err(VpnBillingError::StateUnavailable)
        );
        handle.highest_voucher.clear_poison();
        assert!(
            matches!(handle.receipt(), Err(VpnBillingError::StateUnavailable)),
            "clearing the mutex poison must not reopen settlement"
        );

        let window_handle = VpnSessionHandle::new(
            VpnSession::from_parts(Arc::new(Metrics::new())),
            [0x33; 16],
            VpnExitClassV1::Standard,
            [0x44; 32],
        );
        poison(&window_handle.metered_usage.service_window);
        assert!(matches!(
            window_handle.settlement_artifact(),
            Err(VpnBillingError::StateUnavailable)
        ));
    }

    #[test]
    fn overlay_debug_redacts_and_drop_clears_backend_secret() {
        let issuer =
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("issuer keypair");
        let overlay = VpnOverlay {
            config: VpnConfig::default(),
            helper_ticket_issuer_public_key: Some(issuer.public_key().clone()),
            backend_bootstrap_secret: Some([0x5A; 32]),
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0; 32],
            routes: Vec::new(),
            dns_overrides: Vec::new(),
        };
        let rendered = format!("{overlay:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("165, 165"));
        assert!(!rendered.contains("90, 90"));
        let framing = overlay.framing_only_overlay();
        assert!(framing.helper_ticket_issuer_public_key().is_none());
        assert!(framing.backend_bootstrap_secret().is_none());
        assert_eq!(framing.meter_hash(), overlay.meter_hash());

        let mut backend = Some([0x5A; 32]);
        clear_vpn_overlay_secret(&mut backend);
        assert_eq!(backend, Some([0; 32]));
    }
    #[test]
    fn vpn_frame_and_session_debug_output_redacts_traffic_identity() {
        let overlay = VpnOverlay::from_config(VpnConfig::default());
        let flow_label = VpnFlowLabelV1::from_u32(1).expect("flow label");
        let padded = overlay
            .pad_cell(
                overlay
                    .data_cell(
                        [0xA5; 16],
                        flow_label,
                        1,
                        0,
                        VpnCellFlagsV1::new(false, false, false, false),
                        vec![0xA5; 4],
                    )
                    .expect("data cell"),
            )
            .expect("padded cell");
        let padded_rendered = format!("{padded:?}");
        let scheduled = ScheduledFrame {
            deadline: Duration::ZERO,
            frame: padded.frame,
            payload_len: padded.payload_len,
            sequence: 1,
            is_cover: false,
        };
        let metrics = Arc::new(Metrics::new());
        let handle = VpnSessionHandle::new(
            VpnSession::from_parts(metrics),
            [0xA5; 16],
            VpnExitClassV1::Standard,
            [0xA5; 32],
        );
        for rendered in [
            padded_rendered,
            format!("{scheduled:?}"),
            format!("{handle:?}"),
        ] {
            assert!(rendered.contains("<redacted>"));
            assert!(!rendered.contains("165, 165"));
        }
    }
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
    #[test]
    fn session_accounting_saturates_instead_of_wrapping() {
        let metrics = Arc::new(Metrics::new());
        let session = VpnSession::from_parts(metrics);
        session
            .state
            .ingress_bytes
            .store(u64::MAX - 1, Ordering::Relaxed);
        session
            .state
            .egress_bytes
            .store(u64::MAX - 1, Ordering::Relaxed);
        session
            .state
            .cover_bytes
            .store(u64::MAX - 1, Ordering::Relaxed);
        session.record_ingress(2, true);
        session.record_egress(2, true);
        let receipt = session.finish_receipt([0; 16], VpnExitClassV1::Standard, [0; 32]);
        assert_eq!(u64::MAX, receipt.ingress_bytes);
        assert_eq!(u64::MAX, receipt.egress_bytes);
        assert_eq!(u64::MAX, receipt.cover_bytes);
    }
    #[test]
    fn metered_receipt_uses_monotonic_elapsed_time_across_wall_clock_rollback() {
        let tariff = VpnTariffV1 {
            lease_fee: Quantity::from(10_u64),
            active_fee_per_minute: Quantity::from(1_u64),
            ingress_fee_per_mib: Quantity::zero(),
            egress_fee_per_mib: Quantity::zero(),
        };
        let metering_keys =
            KeyPair::try_from_seed(vec![0x66; 32], Algorithm::Ed25519).expect("metering keypair");
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            relay_id: [0x33; 32],
            sequence: 1,
            ingress_bytes: 1,
            egress_bytes: 1,
            active_ms: 1_000,
            issued_at_ms: 10_000,
        };
        let voucher = VpnUsageVoucherV1::try_sign(body, metering_keys.private_key())
            .expect("signed prepaid voucher");
        let envelope = VpnUsageVoucherEnvelopeV1 {
            fee_ceiling: tariff.fee_ceiling(&body).expect("voucher fee ceiling"),
            voucher,
        };
        let metrics = Arc::new(Metrics::new());
        let mut handle = VpnSessionHandle::new(
            VpnSession::from_parts(metrics),
            body.session_id,
            VpnExitClassV1::Standard,
            vpn_tariff_meter_hash_v1(&tariff),
        );
        handle.tariff = Some(tariff.clone());
        handle
            .record_usage_voucher(envelope)
            .expect("voucher state available");
        handle
            .begin_metered_service(body.issued_at_ms)
            .expect("billing state available");
        std::thread::sleep(Duration::from_millis(5));
        handle
            .end_metered_service(body.issued_at_ms.saturating_sub(1_000))
            .expect("billing state available");

        let receipt = handle.receipt().expect("receipt state available");
        let active_ms = receipt.ended_at_ms - receipt.started_at_ms;
        assert!((1..=body.active_ms).contains(&active_ms));
        assert_eq!(receipt.started_at_ms, body.issued_at_ms);
        assert_eq!(receipt.ended_at_ms, body.issued_at_ms + active_ms);
        assert_eq!(receipt.uptime_secs, 1);
        assert_eq!(
            receipt.earned_fee,
            tariff
                .fee_for_usage(0, 0, active_ms)
                .expect("monotonic actual fee")
        );
        assert!(!receipt.earned_fee.is_zero());
    }
}
