//! Petal stream framing for offline payload handoff.
//!
//! Petal stream encodes the raw `QrStreamFrame` bytes into a custom optical
//! grid with calibration anchors so a dedicated scanner can recover frames
//! from sakura-style animations.

use thiserror::Error;

/// Magic bytes that start every petal stream payload (`PS`).
pub const PETAL_STREAM_MAGIC: [u8; 2] = [0x50, 0x53];
/// Petal stream payload format version.
pub const PETAL_STREAM_VERSION: u8 = 1;
/// Header length in bytes.
pub const PETAL_STREAM_HEADER_LEN: usize = 9;
/// Default border thickness in cells.
pub const PETAL_STREAM_DEFAULT_BORDER: u8 = 1;
/// Default anchor size in cells.
pub const PETAL_STREAM_DEFAULT_ANCHOR: u8 = 3;
/// Default grid size candidates for auto sizing.
pub const PETAL_STREAM_GRID_SIZES: &[u16] = &[33, 37, 41, 45, 49, 53, 57, 61, 65, 69];
/// Basis-point scale used for deterministic capture success ratios.
pub const PETAL_CAPTURE_RATIO_BPS_SCALE: u16 = 10_000;
/// Default minimum success ratio for production capture gates (95%).
pub const PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS: u16 = 9_500;

/// Errors raised while encoding or decoding petal stream frames.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PetalStreamError {
    /// Options are invalid or unsupported.
    #[error("invalid petal stream options: {0}")]
    InvalidOptions(&'static str),
    /// Grid size cannot hold the payload.
    #[error("petal stream grid too small for payload")]
    CapacityExceeded,
    /// Payload length exceeds format limits.
    #[error("petal stream payload length exceeds u16")]
    PayloadTooLarge,
    /// Petal stream header is invalid.
    #[error("petal stream header invalid: {0}")]
    InvalidHeader(&'static str),
    /// Petal stream CRC32 mismatch.
    #[error("petal stream checksum mismatch")]
    ChecksumMismatch,
}

/// Encoder/decoder options for the petal stream grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetalStreamOptions {
    /// Total grid size (0 selects an automatic size).
    pub grid_size: u16,
    /// Border thickness in cells.
    pub border: u8,
    /// Anchor size in cells.
    pub anchor_size: u8,
}

impl Default for PetalStreamOptions {
    fn default() -> Self {
        Self {
            grid_size: 0,
            border: PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: PETAL_STREAM_DEFAULT_ANCHOR,
        }
    }
}

/// Bit grid representing a petal stream frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetalStreamGrid {
    /// Grid size (cells per side).
    pub grid_size: u16,
    /// Row-major bits for each cell.
    pub cells: Vec<bool>,
}

impl PetalStreamGrid {
    /// Create a grid from raw cells.
    ///
    /// # Errors
    /// Returns an error when `grid_size` is zero or `cells` length does not
    /// match `grid_size * grid_size`.
    pub fn new(grid_size: u16, cells: Vec<bool>) -> Result<Self, PetalStreamError> {
        let expected = grid_size as usize * grid_size as usize;
        if expected == 0 || cells.len() != expected {
            return Err(PetalStreamError::InvalidOptions("grid size mismatch"));
        }
        Ok(Self { grid_size, cells })
    }

    /// Read a cell value at (x, y).
    pub fn get(&self, x: u16, y: u16) -> Option<bool> {
        if x >= self.grid_size || y >= self.grid_size {
            return None;
        }
        let idx = y as usize * self.grid_size as usize + x as usize;
        self.cells.get(idx).copied()
    }
}

/// Sampled luminance grid for decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetalStreamSampleGrid {
    /// Grid size (cells per side).
    pub grid_size: u16,
    /// Row-major samples per cell (0..=255).
    pub samples: Vec<u8>,
}

impl PetalStreamSampleGrid {
    /// Create a sample grid from raw values.
    ///
    /// # Errors
    /// Returns an error when `grid_size` is zero or `samples` length does not
    /// match `grid_size * grid_size`.
    pub fn new(grid_size: u16, samples: Vec<u8>) -> Result<Self, PetalStreamError> {
        let expected = grid_size as usize * grid_size as usize;
        if expected == 0 || samples.len() != expected {
            return Err(PetalStreamError::InvalidOptions(
                "sample grid size mismatch",
            ));
        }
        Ok(Self { grid_size, samples })
    }
}

/// Deterministic capture profile used to score Petal Stream readability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetalStreamCaptureProfile {
    /// Number of deterministic capture attempts to simulate.
    pub attempts: u16,
    /// Nominal luminance for dark cells (0 = black).
    pub dark_luma: u8,
    /// Nominal luminance for light cells (255 = white).
    pub light_luma: u8,
    /// Per-cell deterministic luminance jitter applied to each attempt.
    pub luminance_jitter: u8,
}

impl Default for PetalStreamCaptureProfile {
    fn default() -> Self {
        Self {
            attempts: 12,
            dark_luma: 32,
            light_luma: 224,
            luminance_jitter: 24,
        }
    }
}

/// Deterministic capture score for a Petal Stream payload/profile pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetalStreamCaptureScore {
    /// Number of scheduled capture attempts.
    pub attempts: u16,
    /// Number of attempts that decoded back to the original payload.
    pub successes: u16,
}

impl PetalStreamCaptureScore {
    /// Return the success ratio in basis points.
    pub fn success_ratio_bps(&self) -> u16 {
        if self.attempts == 0 {
            return 0;
        }
        let numerator = u32::from(self.successes) * u32::from(PETAL_CAPTURE_RATIO_BPS_SCALE);
        u16::try_from(numerator / u32::from(self.attempts)).unwrap_or(PETAL_CAPTURE_RATIO_BPS_SCALE)
    }

    /// Return whether the score meets the requested minimum success ratio.
    ///
    /// # Errors
    /// Returns an error if `min_success_ratio_bps` exceeds 100%.
    pub fn meets_min_success_ratio_bps(
        &self,
        min_success_ratio_bps: u16,
    ) -> Result<bool, PetalStreamError> {
        if min_success_ratio_bps > PETAL_CAPTURE_RATIO_BPS_SCALE {
            return Err(PetalStreamError::InvalidOptions(
                "min_success_ratio_bps exceeds 100%",
            ));
        }
        Ok(self.success_ratio_bps() >= min_success_ratio_bps)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellRole {
    Border,
    AnchorDark,
    AnchorLight,
    Data,
}

/// Encoder for petal stream frames.
#[derive(Debug, Clone, Copy)]
pub struct PetalStreamEncoder;

impl PetalStreamEncoder {
    /// Encode payload bytes into a petal stream bit grid.
    ///
    /// # Errors
    /// Returns an error when options are invalid, payload length exceeds
    /// format limits, or the selected grid capacity cannot fit the payload.
    pub fn encode_grid(
        payload: &[u8],
        options: PetalStreamOptions,
    ) -> Result<PetalStreamGrid, PetalStreamError> {
        if payload.len() > u16::MAX as usize {
            return Err(PetalStreamError::PayloadTooLarge);
        }
        let grid_size = resolve_grid_size(payload.len(), options)?;
        let capacity = capacity_bits(grid_size, options)?;
        let bits_needed = (PETAL_STREAM_HEADER_LEN + payload.len()) * 8;
        if bits_needed > capacity {
            return Err(PetalStreamError::CapacityExceeded);
        }

        let header = encode_header(payload)?;
        let mut bits = Vec::with_capacity(bits_needed);
        push_bytes_as_bits(&header, &mut bits);
        push_bytes_as_bits(payload, &mut bits);

        let mut cells = vec![false; grid_size as usize * grid_size as usize];
        let mut bit_idx = 0usize;
        for y in 0..grid_size {
            for x in 0..grid_size {
                let idx = y as usize * grid_size as usize + x as usize;
                match cell_role(x, y, grid_size, options) {
                    CellRole::Border | CellRole::AnchorDark => cells[idx] = true,
                    CellRole::AnchorLight => cells[idx] = false,
                    CellRole::Data => {
                        if let Some(bit) = bits.get(bit_idx).copied() {
                            cells[idx] = bit;
                            bit_idx += 1;
                        }
                    }
                }
            }
        }

        Ok(PetalStreamGrid { grid_size, cells })
    }
}

/// Decoder for petal stream frames.
#[derive(Debug, Clone, Copy)]
pub struct PetalStreamDecoder;

impl PetalStreamDecoder {
    /// Decode payload bytes from a petal stream bit grid.
    ///
    /// # Errors
    /// Returns an error when options are invalid, grid geometry is inconsistent,
    /// or payload/header checks fail.
    pub fn decode_grid(
        grid: &PetalStreamGrid,
        options: PetalStreamOptions,
    ) -> Result<Vec<u8>, PetalStreamError> {
        let grid_size = resolve_grid_size_for_decode(grid.grid_size, options)?;
        if grid_size != grid.grid_size {
            return Err(PetalStreamError::InvalidOptions("grid size mismatch"));
        }
        let capacity = capacity_bits(grid_size, options)?;
        let mut bits = Vec::with_capacity(capacity);
        for y in 0..grid_size {
            for x in 0..grid_size {
                if cell_role(x, y, grid_size, options) == CellRole::Data
                    && let Some(bit) = grid.get(x, y)
                {
                    bits.push(bit);
                }
            }
        }
        let bytes = bits_to_bytes(&bits);
        decode_payload(&bytes)
    }

    /// Decode payload bytes from a sampled luminance grid.
    ///
    /// # Errors
    /// Returns an error when options are invalid, anchor sampling cannot derive
    /// a valid threshold, or payload/header checks fail.
    pub fn decode_samples(
        samples: &PetalStreamSampleGrid,
        options: PetalStreamOptions,
    ) -> Result<Vec<u8>, PetalStreamError> {
        let grid_size = resolve_grid_size_for_decode(samples.grid_size, options)?;
        if grid_size != samples.grid_size {
            return Err(PetalStreamError::InvalidOptions(
                "sample grid size mismatch",
            ));
        }
        let mut dark_sum = 0u64;
        let mut light_sum = 0u64;
        let mut dark_count = 0u64;
        let mut light_count = 0u64;
        for y in 0..grid_size {
            for x in 0..grid_size {
                let idx = y as usize * grid_size as usize + x as usize;
                let value = samples.samples[idx];
                match cell_role(x, y, grid_size, options) {
                    CellRole::AnchorDark => {
                        dark_sum += u64::from(value);
                        dark_count += 1;
                    }
                    CellRole::AnchorLight => {
                        light_sum += u64::from(value);
                        light_count += 1;
                    }
                    _ => {}
                }
            }
        }
        if dark_count == 0 || light_count == 0 {
            return Err(PetalStreamError::InvalidOptions("anchor sampling failed"));
        }
        let dark_avg = dark_sum / dark_count;
        let light_avg = light_sum / light_count;
        if dark_avg >= light_avg {
            return Err(PetalStreamError::InvalidOptions("anchor contrast too low"));
        }
        let threshold = u8::try_from(dark_avg.saturating_add(light_avg) / 2).unwrap_or(u8::MAX);
        let mut cells = vec![false; grid_size as usize * grid_size as usize];
        for (idx, sample) in samples.samples.iter().enumerate() {
            cells[idx] = *sample < threshold;
        }
        let grid = PetalStreamGrid { grid_size, cells };
        Self::decode_grid(&grid, options)
    }
}

/// Score deterministic capture readability for a payload.
///
/// # Errors
/// Returns an error when the profile or grid options are invalid, or when the
/// payload cannot be encoded into the selected grid.
pub fn score_petal_capture_profile(
    payload: &[u8],
    options: PetalStreamOptions,
    profile: PetalStreamCaptureProfile,
) -> Result<PetalStreamCaptureScore, PetalStreamError> {
    score_petal_capture_profile_with_seed(payload, options, profile, 0)
}

/// Score deterministic capture readability for a payload with an explicit seed.
///
/// The seed is mixed only into the deterministic luminance perturbation stream;
/// no runtime randomness is used, so the same inputs always produce the same
/// score on every node and host architecture.
///
/// # Errors
/// Returns an error when the profile or grid options are invalid, or when the
/// payload cannot be encoded into the selected grid.
pub fn score_petal_capture_profile_with_seed(
    payload: &[u8],
    options: PetalStreamOptions,
    profile: PetalStreamCaptureProfile,
    seed: u64,
) -> Result<PetalStreamCaptureScore, PetalStreamError> {
    validate_capture_profile(profile)?;
    let grid = PetalStreamEncoder::encode_grid(payload, options)?;
    let mut successes = 0u16;
    for attempt in 0..profile.attempts {
        let samples = render_capture_samples(&grid, profile, attempt, seed)?;
        if PetalStreamDecoder::decode_samples(&samples, options)
            .is_ok_and(|decoded| decoded == payload)
        {
            successes = successes.saturating_add(1);
        }
    }
    Ok(PetalStreamCaptureScore {
        attempts: profile.attempts,
        successes,
    })
}

/// Render one deterministic capture-attempt sample grid for a Petal bit grid.
///
/// The `attempt` and `seed` values select the deterministic luminance
/// perturbation stream. No runtime randomness is used, so identical inputs
/// produce identical samples on every host.
///
/// # Errors
/// Returns an error when the profile is invalid or the generated sample grid
/// would not match the input grid geometry.
pub fn render_petal_capture_samples_with_seed(
    grid: &PetalStreamGrid,
    profile: PetalStreamCaptureProfile,
    attempt: u16,
    seed: u64,
) -> Result<PetalStreamSampleGrid, PetalStreamError> {
    validate_capture_profile(profile)?;
    render_capture_samples(grid, profile, attempt, seed)
}

fn validate_capture_profile(profile: PetalStreamCaptureProfile) -> Result<(), PetalStreamError> {
    if profile.attempts == 0 {
        return Err(PetalStreamError::InvalidOptions(
            "capture attempts must be > 0",
        ));
    }
    if profile.dark_luma >= profile.light_luma {
        return Err(PetalStreamError::InvalidOptions(
            "dark_luma must be lower than light_luma",
        ));
    }
    Ok(())
}

fn render_capture_samples(
    grid: &PetalStreamGrid,
    profile: PetalStreamCaptureProfile,
    attempt: u16,
    seed: u64,
) -> Result<PetalStreamSampleGrid, PetalStreamError> {
    let mut samples = Vec::with_capacity(grid.cells.len());
    for (idx, cell) in grid.cells.iter().enumerate() {
        let base = if *cell {
            profile.dark_luma
        } else {
            profile.light_luma
        };
        samples.push(apply_luminance_jitter(
            base,
            profile.luminance_jitter,
            idx,
            attempt,
            seed,
        ));
    }
    PetalStreamSampleGrid::new(grid.grid_size, samples)
}

fn apply_luminance_jitter(base: u8, jitter: u8, index: usize, attempt: u16, seed: u64) -> u8 {
    if jitter == 0 {
        return base;
    }
    let span = u32::from(jitter) * 2 + 1;
    let seed32 = (seed as u32) ^ ((seed >> 32) as u32);
    let mixed = (index as u32)
        .wrapping_mul(1_103_515_245)
        .wrapping_add(u32::from(attempt).wrapping_mul(12_345))
        .wrapping_add(seed32.rotate_left(u32::from(attempt % 31)))
        .rotate_left(u32::from(attempt % 17));
    let offset = i16::try_from(mixed % span).unwrap_or(i16::MAX) - i16::from(jitter);
    let value = i16::from(base) + offset;
    value.clamp(0, 255) as u8
}

fn resolve_grid_size(
    payload_len: usize,
    options: PetalStreamOptions,
) -> Result<u16, PetalStreamError> {
    let border = options.border;
    let anchor_size = options.anchor_size;
    if border == 0 {
        return Err(PetalStreamError::InvalidOptions("border must be > 0"));
    }
    if anchor_size == 0 {
        return Err(PetalStreamError::InvalidOptions("anchor_size must be > 0"));
    }
    let bits_needed = (PETAL_STREAM_HEADER_LEN + payload_len) * 8;
    if options.grid_size != 0 {
        let capacity = capacity_bits(options.grid_size, options)?;
        if bits_needed > capacity {
            return Err(PetalStreamError::CapacityExceeded);
        }
        return Ok(options.grid_size);
    }
    for &candidate in PETAL_STREAM_GRID_SIZES {
        if candidate == 0 {
            continue;
        }
        if let Ok(capacity) = capacity_bits(candidate, options)
            && bits_needed <= capacity
        {
            return Ok(candidate);
        }
    }
    Err(PetalStreamError::CapacityExceeded)
}

fn resolve_grid_size_for_decode(
    grid_size: u16,
    options: PetalStreamOptions,
) -> Result<u16, PetalStreamError> {
    if options.grid_size != 0 {
        return Ok(options.grid_size);
    }
    if grid_size == 0 {
        return Err(PetalStreamError::InvalidOptions("grid size is zero"));
    }
    Ok(grid_size)
}

fn capacity_bits(size_cells: u16, options: PetalStreamOptions) -> Result<usize, PetalStreamError> {
    let border = i32::from(options.border);
    let anchor = i32::from(options.anchor_size);
    let side_len_i32 = i32::from(size_cells);
    if side_len_i32 <= 0 {
        return Err(PetalStreamError::InvalidOptions("grid size must be > 0"));
    }
    let min_grid = border * 2 + anchor * 2 + 1;
    if side_len_i32 < min_grid {
        return Err(PetalStreamError::InvalidOptions(
            "grid size too small for anchors",
        ));
    }
    let side_len = usize::try_from(side_len_i32).expect("grid validated as positive");
    let total = side_len * side_len;
    let border_cells = side_len * 4 - 4;
    let anchor_size = usize::from(options.anchor_size);
    let anchor_cells = anchor_size * anchor_size * 4;
    let data_cells = total.saturating_sub(border_cells + anchor_cells);
    Ok(data_cells)
}

fn cell_role(x: u16, y: u16, grid_size: u16, options: PetalStreamOptions) -> CellRole {
    let border = u16::from(options.border);
    let anchor = u16::from(options.anchor_size);
    if x < border || y < border || x >= grid_size - border || y >= grid_size - border {
        return CellRole::Border;
    }
    let right = grid_size.saturating_sub(border + anchor);
    let bottom = grid_size.saturating_sub(border + anchor);
    let in_left = x >= border && x < border + anchor;
    let in_right = x >= right && x < right + anchor;
    let in_top = y >= border && y < border + anchor;
    let in_bottom = y >= bottom && y < bottom + anchor;
    if in_left && in_top {
        return CellRole::AnchorDark;
    }
    if in_left && in_bottom {
        return CellRole::AnchorDark;
    }
    if in_right && in_top {
        return CellRole::AnchorLight;
    }
    if in_right && in_bottom {
        return CellRole::AnchorLight;
    }
    CellRole::Data
}

fn encode_header(payload: &[u8]) -> Result<Vec<u8>, PetalStreamError> {
    let payload_len =
        u16::try_from(payload.len()).map_err(|_| PetalStreamError::PayloadTooLarge)?;
    let crc = crc32(payload);
    let mut header = Vec::with_capacity(PETAL_STREAM_HEADER_LEN);
    header.extend_from_slice(&PETAL_STREAM_MAGIC);
    header.push(PETAL_STREAM_VERSION);
    header.extend_from_slice(&payload_len.to_le_bytes());
    header.extend_from_slice(&crc.to_le_bytes());
    Ok(header)
}

fn decode_payload(bytes: &[u8]) -> Result<Vec<u8>, PetalStreamError> {
    if bytes.len() < PETAL_STREAM_HEADER_LEN {
        return Err(PetalStreamError::InvalidHeader("header too short"));
    }
    if bytes[0] != PETAL_STREAM_MAGIC[0] || bytes[1] != PETAL_STREAM_MAGIC[1] {
        return Err(PetalStreamError::InvalidHeader("magic mismatch"));
    }
    if bytes[2] != PETAL_STREAM_VERSION {
        return Err(PetalStreamError::InvalidHeader("unsupported version"));
    }
    let payload_len = u16::from_le_bytes([bytes[3], bytes[4]]) as usize;
    let crc = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
    let start = PETAL_STREAM_HEADER_LEN;
    let end = start + payload_len;
    if end > bytes.len() {
        return Err(PetalStreamError::InvalidHeader(
            "payload length exceeds data",
        ));
    }
    let payload = bytes[start..end].to_vec();
    let expected = crc32(&payload);
    if expected != crc {
        return Err(PetalStreamError::ChecksumMismatch);
    }
    Ok(payload)
}

fn push_bytes_as_bits(bytes: &[u8], out: &mut Vec<bool>) {
    for &byte in bytes {
        for bit in (0..8).rev() {
            out.push(byte & (1 << bit) != 0);
        }
    }
}

fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len() / 8 + 1);
    for chunk in bits.chunks(8) {
        let mut value = 0u8;
        for (idx, bit) in chunk.iter().enumerate() {
            if *bit {
                value |= 1 << (7 - idx);
            }
        }
        out.push(value);
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        let mut c = (crc ^ u32::from(byte)) & 0xFF;
        for _ in 0..8 {
            if c & 1 == 1 {
                c = 0xEDB8_8320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
        }
        crc = (crc >> 8) ^ c;
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn petal_grid_roundtrip() {
        let payload = b"petal-stream-payload";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let decoded =
            PetalStreamDecoder::decode_grid(&grid, PetalStreamOptions::default()).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn petal_grid_rejects_crc_mismatch() {
        let payload = b"petal-stream-payload";
        let mut grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let header_bits = PETAL_STREAM_HEADER_LEN * 8;
        let mut flipped = false;
        let mut bit_idx = 0usize;
        for y in 0..grid.grid_size {
            for x in 0..grid.grid_size {
                if cell_role(x, y, grid.grid_size, PetalStreamOptions::default()) != CellRole::Data
                {
                    continue;
                }
                if bit_idx < header_bits {
                    bit_idx += 1;
                    continue;
                }
                let idx = y as usize * grid.grid_size as usize + x as usize;
                grid.cells[idx] = !grid.cells[idx];
                flipped = true;
                break;
            }
            if flipped {
                break;
            }
        }
        assert!(flipped, "must flip a data cell");
        let err = PetalStreamDecoder::decode_grid(&grid, PetalStreamOptions::default())
            .expect_err("decode should fail");
        assert_eq!(err, PetalStreamError::ChecksumMismatch);
    }

    #[test]
    fn petal_samples_decode_roundtrip() {
        let payload = b"petal-stream-samples";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let samples: Vec<u8> = grid
            .cells
            .iter()
            .map(|bit| if *bit { 32u8 } else { 224u8 })
            .collect();
        let sample_grid = PetalStreamSampleGrid::new(grid.grid_size, samples).expect("samples");
        let decoded =
            PetalStreamDecoder::decode_samples(&sample_grid, PetalStreamOptions::default())
                .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn petal_auto_grid_selects_candidate() {
        let payload = vec![0u8; 128];
        let grid = PetalStreamEncoder::encode_grid(&payload, PetalStreamOptions::default())
            .expect("encode");
        assert!(PETAL_STREAM_GRID_SIZES.contains(&grid.grid_size));
    }

    #[test]
    fn default_capture_profile_meets_production_gate() {
        let payload = b"sora-temple-capture-baseline";
        let score = score_petal_capture_profile(
            payload,
            PetalStreamOptions::default(),
            PetalStreamCaptureProfile::default(),
        )
        .expect("score profile");

        assert_eq!(
            score.attempts,
            PetalStreamCaptureProfile::default().attempts
        );
        assert_eq!(score.successes, score.attempts);
        assert_eq!(score.success_ratio_bps(), PETAL_CAPTURE_RATIO_BPS_SCALE);
        assert!(
            score
                .meets_min_success_ratio_bps(PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS)
                .expect("valid threshold")
        );
    }

    #[test]
    fn seeded_capture_profile_is_deterministic_and_default_seed_matches_unseeded() {
        let payload = b"sora-temple-capture-baseline";
        let options = PetalStreamOptions::default();
        let profile = PetalStreamCaptureProfile::default();
        let unseeded =
            score_petal_capture_profile(payload, options, profile).expect("unseeded score");
        let seeded_a =
            score_petal_capture_profile_with_seed(payload, options, profile, 42).expect("seeded a");
        let seeded_b =
            score_petal_capture_profile_with_seed(payload, options, profile, 42).expect("seeded b");
        let seed_zero =
            score_petal_capture_profile_with_seed(payload, options, profile, 0).expect("seed zero");

        assert_eq!(seeded_a, seeded_b);
        assert_eq!(seed_zero, unseeded);
    }

    #[test]
    fn low_contrast_capture_profile_fails_gate() {
        let payload = b"sora-temple-low-contrast";
        let score = score_petal_capture_profile(
            payload,
            PetalStreamOptions::default(),
            PetalStreamCaptureProfile {
                attempts: 4,
                dark_luma: 128,
                light_luma: 129,
                luminance_jitter: 0,
            },
        )
        .expect("score profile");

        assert_eq!(score.successes, 0);
        assert_eq!(score.success_ratio_bps(), 0);
        assert!(
            !score
                .meets_min_success_ratio_bps(PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS)
                .expect("valid threshold")
        );
    }

    #[test]
    fn capture_profile_rejects_invalid_options() {
        let err = score_petal_capture_profile(
            b"payload",
            PetalStreamOptions::default(),
            PetalStreamCaptureProfile {
                attempts: 0,
                ..PetalStreamCaptureProfile::default()
            },
        )
        .expect_err("zero attempts rejected");
        assert_eq!(
            err,
            PetalStreamError::InvalidOptions("capture attempts must be > 0")
        );

        let err = score_petal_capture_profile(
            b"payload",
            PetalStreamOptions::default(),
            PetalStreamCaptureProfile {
                dark_luma: 200,
                light_luma: 128,
                ..PetalStreamCaptureProfile::default()
            },
        )
        .expect_err("inverted luminance rejected");
        assert_eq!(
            err,
            PetalStreamError::InvalidOptions("dark_luma must be lower than light_luma")
        );

        let err = PetalStreamCaptureScore {
            attempts: 1,
            successes: 1,
        }
        .meets_min_success_ratio_bps(PETAL_CAPTURE_RATIO_BPS_SCALE + 1)
        .expect_err("invalid threshold rejected");
        assert_eq!(
            err,
            PetalStreamError::InvalidOptions("min_success_ratio_bps exceeds 100%")
        );
    }

    #[test]
    fn render_capture_samples_with_seed_is_public_and_deterministic() {
        let payload = b"sora-temple-capture-baseline";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let profile = PetalStreamCaptureProfile::default();

        let samples_a =
            render_petal_capture_samples_with_seed(&grid, profile, 3, 99).expect("samples a");
        let samples_b =
            render_petal_capture_samples_with_seed(&grid, profile, 3, 99).expect("samples b");
        let samples_other_attempt =
            render_petal_capture_samples_with_seed(&grid, profile, 4, 99).expect("samples c");

        assert_eq!(samples_a, samples_b);
        assert_ne!(samples_a, samples_other_attempt);
        let decoded = PetalStreamDecoder::decode_samples(&samples_a, PetalStreamOptions::default())
            .expect("decode samples");
        assert_eq!(decoded, payload);
    }
}
