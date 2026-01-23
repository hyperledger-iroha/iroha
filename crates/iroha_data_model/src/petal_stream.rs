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
pub const PETAL_STREAM_GRID_SIZES: &[u16] = &[
    33, 37, 41, 45, 49, 53, 57, 61, 65, 69,
];

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
    pub fn new(grid_size: u16, samples: Vec<u8>) -> Result<Self, PetalStreamError> {
        let expected = grid_size as usize * grid_size as usize;
        if expected == 0 || samples.len() != expected {
            return Err(PetalStreamError::InvalidOptions("sample grid size mismatch"));
        }
        Ok(Self { grid_size, samples })
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
                if cell_role(x, y, grid_size, options) == CellRole::Data {
                    if let Some(bit) = grid.get(x, y) {
                        bits.push(bit);
                    }
                }
            }
        }
        let bytes = bits_to_bytes(&bits);
        decode_payload(&bytes)
    }

    /// Decode payload bytes from a sampled luminance grid.
    pub fn decode_samples(
        samples: &PetalStreamSampleGrid,
        options: PetalStreamOptions,
    ) -> Result<Vec<u8>, PetalStreamError> {
        let grid_size = resolve_grid_size_for_decode(samples.grid_size, options)?;
        if grid_size != samples.grid_size {
            return Err(PetalStreamError::InvalidOptions("sample grid size mismatch"));
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
                        dark_sum += value as u64;
                        dark_count += 1;
                    }
                    CellRole::AnchorLight => {
                        light_sum += value as u64;
                        light_count += 1;
                    }
                    _ => {}
                }
            }
        }
        if dark_count == 0 || light_count == 0 {
            return Err(PetalStreamError::InvalidOptions("anchor sampling failed"));
        }
        let dark_avg = dark_sum as f64 / dark_count as f64;
        let light_avg = light_sum as f64 / light_count as f64;
        if dark_avg >= light_avg {
            return Err(PetalStreamError::InvalidOptions("anchor contrast too low"));
        }
        let threshold = ((dark_avg + light_avg) / 2.0).round() as u8;
        let mut cells = vec![false; grid_size as usize * grid_size as usize];
        for (idx, sample) in samples.samples.iter().enumerate() {
            cells[idx] = *sample < threshold;
        }
        let grid = PetalStreamGrid { grid_size, cells };
        Self::decode_grid(&grid, options)
    }
}

fn resolve_grid_size(payload_len: usize, options: PetalStreamOptions) -> Result<u16, PetalStreamError> {
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
        if let Ok(capacity) = capacity_bits(candidate, options) {
            if bits_needed <= capacity {
                return Ok(candidate);
            }
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

fn capacity_bits(grid_size: u16, options: PetalStreamOptions) -> Result<usize, PetalStreamError> {
    let border = options.border as i32;
    let anchor = options.anchor_size as i32;
    let grid = grid_size as i32;
    if grid <= 0 {
        return Err(PetalStreamError::InvalidOptions("grid size must be > 0"));
    }
    let min_grid = border * 2 + anchor * 2 + 1;
    if grid < min_grid {
        return Err(PetalStreamError::InvalidOptions("grid size too small for anchors"));
    }
    let total = (grid as usize) * (grid as usize);
    let border_cells = (grid as usize) * 4 - 4;
    let anchor_cells = (options.anchor_size as usize)
        * (options.anchor_size as usize)
        * 4;
    let data_cells = total.saturating_sub(border_cells + anchor_cells);
    Ok(data_cells)
}

fn cell_role(x: u16, y: u16, grid_size: u16, options: PetalStreamOptions) -> CellRole {
    let border = options.border as u16;
    let anchor = options.anchor_size as u16;
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
    let payload_len = u16::try_from(payload.len())
        .map_err(|_| PetalStreamError::PayloadTooLarge)?;
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
        return Err(PetalStreamError::InvalidHeader("payload length exceeds data"));
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
        let idx = grid.cells.iter().position(|bit| *bit).expect("bit");
        grid.cells[idx] = !grid.cells[idx];
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
}
