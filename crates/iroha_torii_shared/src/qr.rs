//! In-tree QR Code encoder used by Torii and CLI visual offline flows.
//!
//! The implementation supports normal QR Code symbols in byte mode for
//! versions 1 through 40 and all four standard error-correction levels.  It is
//! intentionally narrow: Iroha only needs deterministic byte payload rendering,
//! so Kanji/alphanumeric segmentation and Micro QR are left out.

use std::{error::Error, fmt};

const MIN_VERSION: u8 = 1;
const MAX_VERSION: u8 = 40;
const MODE_BYTE: u32 = 0b0100;
const QUIET_ZONE_MODULES: u32 = 4;
const PAD_CODEWORDS: [u8; 2] = [0xEC, 0x11];

const ECC_CODEWORDS_PER_BLOCK: [[u8; 41]; 4] = [
    [
        0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28,
        30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28,
        28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    ],
    [
        0, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30,
        30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24,
        30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
];

const NUM_ERROR_CORRECTION_BLOCKS: [[u8; 41]; 4] = [
    [
        0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13,
        14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
    ],
    [
        0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21,
        23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
    ],
    [
        0, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29,
        34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68,
    ],
    [
        0, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32,
        35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81,
    ],
];

const ALIGNMENT_PATTERN_POSITIONS: [&[u8]; 41] = [
    &[],
    &[],
    &[6, 18],
    &[6, 22],
    &[6, 26],
    &[6, 30],
    &[6, 34],
    &[6, 22, 38],
    &[6, 24, 42],
    &[6, 26, 46],
    &[6, 28, 50],
    &[6, 30, 54],
    &[6, 32, 58],
    &[6, 34, 62],
    &[6, 26, 46, 66],
    &[6, 26, 48, 70],
    &[6, 26, 50, 74],
    &[6, 30, 54, 78],
    &[6, 30, 56, 82],
    &[6, 30, 58, 86],
    &[6, 34, 62, 90],
    &[6, 28, 50, 72, 94],
    &[6, 26, 50, 74, 98],
    &[6, 30, 54, 78, 102],
    &[6, 28, 54, 80, 106],
    &[6, 32, 58, 84, 110],
    &[6, 30, 58, 86, 114],
    &[6, 34, 62, 90, 118],
    &[6, 26, 50, 74, 98, 122],
    &[6, 30, 54, 78, 102, 126],
    &[6, 26, 52, 78, 104, 130],
    &[6, 30, 56, 82, 108, 134],
    &[6, 34, 60, 86, 112, 138],
    &[6, 30, 58, 86, 114, 142],
    &[6, 34, 62, 90, 118, 146],
    &[6, 30, 54, 78, 102, 126, 150],
    &[6, 24, 50, 76, 102, 128, 154],
    &[6, 28, 54, 80, 106, 132, 158],
    &[6, 32, 58, 84, 110, 136, 162],
    &[6, 26, 54, 82, 110, 138, 166],
    &[6, 30, 58, 86, 114, 142, 170],
];

/// Standard QR Code error-correction level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EcLevel {
    /// Low error correction.
    L,
    /// Medium error correction.
    M,
    /// Quartile error correction.
    Q,
    /// High error correction.
    H,
}

impl EcLevel {
    const fn table_index(self) -> usize {
        match self {
            Self::L => 0,
            Self::M => 1,
            Self::Q => 2,
            Self::H => 3,
        }
    }

    const fn format_bits(self) -> u32 {
        match self {
            Self::L => 1,
            Self::M => 0,
            Self::Q => 3,
            Self::H => 2,
        }
    }
}

/// Error returned when a QR Code cannot be generated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrError {
    /// The input does not fit in a version-40 byte-mode QR Code.
    DataTooLong {
        /// Number of input bytes.
        bytes: usize,
    },
    /// The implementation detected an impossible QR table state.
    TableInvariant {
        /// Human-readable invariant description.
        detail: &'static str,
    },
}

impl fmt::Display for QrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataTooLong { bytes } => {
                write!(f, "payload of {bytes} bytes does not fit in a QR Code")
            }
            Self::TableInvariant { detail } => write!(f, "invalid QR table invariant: {detail}"),
        }
    }
}

impl Error for QrError {}

/// Light or dark QR module color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    /// Light module.
    Light,
    /// Dark module.
    Dark,
}

/// Encoded normal QR Code symbol.
#[derive(Clone, Debug)]
pub struct QrCode {
    version: u8,
    size: usize,
    modules: Vec<bool>,
    function_modules: Vec<bool>,
}

impl QrCode {
    /// Encode bytes using byte mode and medium error correction.
    pub fn new(data: &[u8]) -> Result<Self, QrError> {
        Self::with_error_correction_level(data, EcLevel::M)
    }

    /// Encode bytes using byte mode and the requested error-correction level.
    pub fn with_error_correction_level(data: &[u8], level: EcLevel) -> Result<Self, QrError> {
        let version = choose_version(data.len(), level)?;
        let data_codewords = encode_data_codewords(data, version, level)?;
        let codewords = add_error_correction_and_interleave(&data_codewords, version, level)?;
        let mut base = Self::blank(version);
        base.draw_function_patterns();

        let mut best: Option<(i32, Self)> = None;
        for mask in 0..8 {
            let mut candidate = base.clone();
            candidate.draw_codewords(&codewords, mask);
            candidate.draw_format_bits(level, mask);
            let penalty = candidate.penalty_score();
            if best
                .as_ref()
                .is_none_or(|(best_penalty, _)| penalty < *best_penalty)
            {
                best = Some((penalty, candidate));
            }
        }

        best.map(|(_, code)| code)
            .ok_or(QrError::TableInvariant { detail: "no masks" })
    }

    /// Return the normal QR Code version number, from 1 to 40.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Return the module width of the QR symbol without quiet-zone modules.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.size
    }

    /// Return the quiet-zone width, in modules.
    #[must_use]
    pub const fn quiet_zone(&self) -> u32 {
        QUIET_ZONE_MODULES
    }

    /// Return whether the module at `(x, y)` is dark.
    #[must_use]
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        self.modules[self.index(x, y)]
    }

    /// Return whether the module at `(x, y)` is reserved for QR function data.
    #[must_use]
    pub fn is_functional(&self, x: usize, y: usize) -> bool {
        self.function_modules[self.index(x, y)]
    }

    /// Return all modules as color values in row-major order.
    #[must_use]
    pub fn to_colors(&self) -> Vec<Color> {
        self.modules
            .iter()
            .map(|&dark| if dark { Color::Dark } else { Color::Light })
            .collect()
    }

    /// Render an SVG document with the requested pixel dimensions and palette.
    #[must_use]
    pub fn to_svg(&self, dimension: u32, dark: &str, light: &str) -> String {
        let quiet = self.quiet_zone();
        let total = self.width() as u32 + quiet * 2;
        let mut path = String::new();
        for y in 0..self.size {
            for x in 0..self.size {
                if self.is_dark(x, y) {
                    let x = x as u32 + quiet;
                    let y = y as u32 + quiet;
                    path.push_str(&format!("M{x},{y}h1v1h-1z"));
                }
            }
        }
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{dimension}\" height=\"{dimension}\" viewBox=\"0 0 {total} {total}\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"{light}\"/><path d=\"{path}\" fill=\"{dark}\"/></svg>"
        )
    }

    /// Render a grayscale bitmap with dark modules as `0` and light modules as `255`.
    #[must_use]
    pub fn to_luma8(&self, dimension: u32) -> (u32, u32, Vec<u8>) {
        let quiet = self.quiet_zone();
        let total_modules = self.width() as u32 + quiet * 2;
        let module_size = (dimension / total_modules).max(1);
        let side = total_modules * module_size;
        let mut data = vec![255u8; (side * side) as usize];
        for y in 0..self.size {
            for x in 0..self.size {
                if !self.is_dark(x, y) {
                    continue;
                }
                let start_x = (x as u32 + quiet) * module_size;
                let start_y = (y as u32 + quiet) * module_size;
                for py in start_y..start_y + module_size {
                    let row_start = (py * side) as usize;
                    for px in start_x..start_x + module_size {
                        data[row_start + px as usize] = 0;
                    }
                }
            }
        }
        (side, side, data)
    }

    fn blank(version: u8) -> Self {
        let size = usize::from(version) * 4 + 17;
        Self {
            version,
            size,
            modules: vec![false; size * size],
            function_modules: vec![false; size * size],
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        assert!(x < self.size && y < self.size);
        y * self.size + x
    }

    fn set_module(&mut self, x: usize, y: usize, dark: bool) {
        let idx = self.index(x, y);
        self.modules[idx] = dark;
    }

    fn set_function_module(&mut self, x: usize, y: usize, dark: bool) {
        let idx = self.index(x, y);
        self.modules[idx] = dark;
        self.function_modules[idx] = true;
    }

    fn draw_function_patterns(&mut self) {
        let size = self.size;
        self.draw_finder_pattern(3, 3);
        self.draw_finder_pattern(size - 4, 3);
        self.draw_finder_pattern(3, size - 4);

        for i in 0..size {
            if !self.function_modules[self.index(6, i)] {
                self.set_function_module(6, i, i % 2 == 0);
            }
            if !self.function_modules[self.index(i, 6)] {
                self.set_function_module(i, 6, i % 2 == 0);
            }
        }

        for &cy in ALIGNMENT_PATTERN_POSITIONS[usize::from(self.version)] {
            for &cx in ALIGNMENT_PATTERN_POSITIONS[usize::from(self.version)] {
                let near_top = cy == 6;
                let near_left = cx == 6;
                let near_right = usize::from(cx) == size - 7;
                if (near_top && (near_left || near_right)) || (near_left && cy as usize == size - 7)
                {
                    continue;
                }
                self.draw_alignment_pattern(usize::from(cx), usize::from(cy));
            }
        }

        self.draw_format_bits(EcLevel::M, 0);
        if self.version >= 7 {
            self.draw_version_bits();
        }
    }

    fn draw_finder_pattern(&mut self, cx: usize, cy: usize) {
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                let x = cx as i32 + dx;
                let y = cy as i32 + dy;
                if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
                    continue;
                }
                let dist = dx.abs().max(dy.abs());
                let dark = dist != 2 && dist != 4;
                self.set_function_module(x as usize, y as usize, dark);
            }
        }
    }

    fn draw_alignment_pattern(&mut self, cx: usize, cy: usize) {
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let dist = dx.abs().max(dy.abs());
                self.set_function_module(
                    (cx as i32 + dx) as usize,
                    (cy as i32 + dy) as usize,
                    dist != 1,
                );
            }
        }
    }

    fn draw_format_bits(&mut self, level: EcLevel, mask: u8) {
        let bits = format_bits(level, mask);
        for i in 0..=5 {
            self.set_function_module(8, i, bit(bits, i));
        }
        self.set_function_module(8, 7, bit(bits, 6));
        self.set_function_module(8, 8, bit(bits, 7));
        self.set_function_module(7, 8, bit(bits, 8));
        for i in 9..15 {
            self.set_function_module(14 - i, 8, bit(bits, i));
        }

        let size = self.size;
        for i in 0..8 {
            self.set_function_module(size - 1 - i, 8, bit(bits, i));
        }
        for i in 8..15 {
            self.set_function_module(8, size - 15 + i, bit(bits, i));
        }
        self.set_function_module(8, size - 8, true);
    }

    fn draw_version_bits(&mut self) {
        let mut rem = u32::from(self.version);
        for _ in 0..12 {
            rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
        }
        let bits = (u32::from(self.version) << 12) | rem;
        let size = self.size;
        for i in 0..18 {
            let dark = bit(bits, i);
            let a = size - 11 + i % 3;
            let b = i / 3;
            self.set_function_module(a, b, dark);
            self.set_function_module(b, a, dark);
        }
    }

    fn draw_codewords(&mut self, codewords: &[u8], mask: u8) {
        let mut bit_index = 0usize;
        let total_bits = codewords.len() * 8;
        let mut right = self.size - 1;
        let mut upward = true;
        while right > 0 {
            if right == 6 {
                right -= 1;
            }
            for vert in 0..self.size {
                let y = if upward { self.size - 1 - vert } else { vert };
                for x in [right, right - 1] {
                    if self.function_modules[self.index(x, y)] {
                        continue;
                    }
                    let mut dark = false;
                    if bit_index < total_bits {
                        dark = ((codewords[bit_index >> 3] >> (7 - (bit_index & 7))) & 1) != 0;
                        bit_index += 1;
                    }
                    if mask_bit(mask, x, y) {
                        dark = !dark;
                    }
                    self.set_module(x, y, dark);
                }
            }
            upward = !upward;
            right = right.saturating_sub(2);
        }
    }

    fn penalty_score(&self) -> i32 {
        let mut result = 0;
        result += self.penalty_adjacent_runs();
        result += self.penalty_blocks();
        result += self.penalty_finder_like_patterns();
        result += self.penalty_balance();
        result
    }

    fn penalty_adjacent_runs(&self) -> i32 {
        let mut result = 0;
        for y in 0..self.size {
            let mut run_color = self.is_dark(0, y);
            let mut run_len = 1;
            for x in 1..self.size {
                let color = self.is_dark(x, y);
                if color == run_color {
                    run_len += 1;
                    if run_len == 5 {
                        result += 3;
                    } else if run_len > 5 {
                        result += 1;
                    }
                } else {
                    run_color = color;
                    run_len = 1;
                }
            }
        }
        for x in 0..self.size {
            let mut run_color = self.is_dark(x, 0);
            let mut run_len = 1;
            for y in 1..self.size {
                let color = self.is_dark(x, y);
                if color == run_color {
                    run_len += 1;
                    if run_len == 5 {
                        result += 3;
                    } else if run_len > 5 {
                        result += 1;
                    }
                } else {
                    run_color = color;
                    run_len = 1;
                }
            }
        }
        result
    }

    fn penalty_blocks(&self) -> i32 {
        let mut result = 0;
        for y in 0..self.size - 1 {
            for x in 0..self.size - 1 {
                let color = self.is_dark(x, y);
                if color == self.is_dark(x + 1, y)
                    && color == self.is_dark(x, y + 1)
                    && color == self.is_dark(x + 1, y + 1)
                {
                    result += 3;
                }
            }
        }
        result
    }

    fn penalty_finder_like_patterns(&self) -> i32 {
        let mut result = 0;
        for y in 0..self.size {
            for x in 0..self.size {
                if x + 10 < self.size && self.has_finder_like_row(x, y) {
                    result += 40;
                }
                if y + 10 < self.size && self.has_finder_like_col(x, y) {
                    result += 40;
                }
            }
        }
        result
    }

    fn has_finder_like_row(&self, x: usize, y: usize) -> bool {
        let pattern = [
            true, false, true, true, true, false, true, false, false, false, false,
        ];
        let inverse = [
            false, false, false, false, true, false, true, true, true, false, true,
        ];
        pattern
            .iter()
            .enumerate()
            .all(|(i, &value)| self.is_dark(x + i, y) == value)
            || inverse
                .iter()
                .enumerate()
                .all(|(i, &value)| self.is_dark(x + i, y) == value)
    }

    fn has_finder_like_col(&self, x: usize, y: usize) -> bool {
        let pattern = [
            true, false, true, true, true, false, true, false, false, false, false,
        ];
        let inverse = [
            false, false, false, false, true, false, true, true, true, false, true,
        ];
        pattern
            .iter()
            .enumerate()
            .all(|(i, &value)| self.is_dark(x, y + i) == value)
            || inverse
                .iter()
                .enumerate()
                .all(|(i, &value)| self.is_dark(x, y + i) == value)
    }

    fn penalty_balance(&self) -> i32 {
        let dark = self.modules.iter().filter(|&&module| module).count() as i32;
        let total = (self.size * self.size) as i32;
        let k = ((dark * 20 - total * 10).abs() + total - 1) / total - 1;
        k * 10
    }
}

fn choose_version(data_len: usize, level: EcLevel) -> Result<u8, QrError> {
    for version in MIN_VERSION..=MAX_VERSION {
        let capacity_bits = usize::from(num_data_codewords(version, level)) * 8;
        let required_bits = 4 + char_count_bits(version) + data_len * 8;
        if required_bits <= capacity_bits {
            return Ok(version);
        }
    }
    Err(QrError::DataTooLong { bytes: data_len })
}

fn encode_data_codewords(data: &[u8], version: u8, level: EcLevel) -> Result<Vec<u8>, QrError> {
    let capacity_bits = usize::from(num_data_codewords(version, level)) * 8;
    let mut bits = BitBuffer::default();
    bits.append_bits(MODE_BYTE, 4);
    bits.append_bits(data.len() as u32, char_count_bits(version));
    for &byte in data {
        bits.append_bits(u32::from(byte), 8);
    }
    if bits.len() > capacity_bits {
        return Err(QrError::DataTooLong { bytes: data.len() });
    }
    let terminator = (capacity_bits - bits.len()).min(4);
    bits.append_bits(0, terminator);
    while bits.len() % 8 != 0 {
        bits.append_bits(0, 1);
    }
    let mut data_codewords = bits.into_bytes();
    let capacity_bytes = capacity_bits / 8;
    let mut pad_index = 0;
    while data_codewords.len() < capacity_bytes {
        data_codewords.push(PAD_CODEWORDS[pad_index & 1]);
        pad_index += 1;
    }
    Ok(data_codewords)
}

fn add_error_correction_and_interleave(
    data: &[u8],
    version: u8,
    level: EcLevel,
) -> Result<Vec<u8>, QrError> {
    let table = level.table_index();
    let num_blocks = usize::from(NUM_ERROR_CORRECTION_BLOCKS[table][usize::from(version)]);
    let block_ecc_len = usize::from(ECC_CODEWORDS_PER_BLOCK[table][usize::from(version)]);
    let raw_codewords = num_raw_data_modules(version) / 8;
    let num_short_blocks = num_blocks - raw_codewords % num_blocks;
    let short_block_len = raw_codewords / num_blocks;
    let short_data_len =
        short_block_len
            .checked_sub(block_ecc_len)
            .ok_or(QrError::TableInvariant {
                detail: "EC block longer than raw block",
            })?;
    let divisor = reed_solomon_divisor(block_ecc_len);
    let mut blocks = Vec::with_capacity(num_blocks);
    let mut data_offset = 0usize;
    for block_index in 0..num_blocks {
        let data_len = short_data_len + usize::from(block_index >= num_short_blocks);
        let block_data =
            data.get(data_offset..data_offset + data_len)
                .ok_or(QrError::TableInvariant {
                    detail: "data block split exceeds payload",
                })?;
        data_offset += data_len;
        let ecc = reed_solomon_remainder(block_data, &divisor);
        let mut block =
            Vec::with_capacity(data_len + usize::from(block_index < num_short_blocks) + ecc.len());
        block.extend_from_slice(block_data);
        if block_index < num_short_blocks {
            block.push(0);
        }
        block.extend_from_slice(&ecc);
        blocks.push(block);
    }
    if data_offset != data.len() {
        return Err(QrError::TableInvariant {
            detail: "unused data after block split",
        });
    }

    let mut result = Vec::with_capacity(raw_codewords);
    for i in 0..blocks[0].len() {
        for (block_index, block) in blocks.iter().enumerate() {
            if i == short_data_len && block_index < num_short_blocks {
                continue;
            }
            if let Some(&byte) = block.get(i) {
                result.push(byte);
            }
        }
    }
    Ok(result)
}

fn num_data_codewords(version: u8, level: EcLevel) -> u16 {
    let raw = num_raw_data_modules(version) / 8;
    let table = level.table_index();
    let ecc_per_block = usize::from(ECC_CODEWORDS_PER_BLOCK[table][usize::from(version)]);
    let blocks = usize::from(NUM_ERROR_CORRECTION_BLOCKS[table][usize::from(version)]);
    (raw - ecc_per_block * blocks) as u16
}

fn num_raw_data_modules(version: u8) -> usize {
    let version = usize::from(version);
    let mut result = (16 * version + 128) * version + 64;
    if version >= 2 {
        let num_align = version / 7 + 2;
        result -= (25 * num_align - 10) * num_align - 55;
        if version >= 7 {
            result -= 36;
        }
    }
    result
}

const fn char_count_bits(version: u8) -> usize {
    if version <= 9 { 8 } else { 16 }
}

#[derive(Default)]
struct BitBuffer {
    bits: Vec<bool>,
}

impl BitBuffer {
    fn append_bits(&mut self, value: u32, len: usize) {
        for i in (0..len).rev() {
            self.bits.push(((value >> i) & 1) != 0);
        }
    }

    fn len(&self) -> usize {
        self.bits.len()
    }

    fn into_bytes(self) -> Vec<u8> {
        let mut out = vec![0u8; self.bits.len().div_ceil(8)];
        for (i, bit) in self.bits.into_iter().enumerate() {
            if bit {
                out[i >> 3] |= 1 << (7 - (i & 7));
            }
        }
        out
    }
}

fn reed_solomon_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0u8; degree];
    result[degree - 1] = 1;
    let mut root = 1u8;
    for i in 0..degree {
        for j in 0..degree {
            result[j] = reed_solomon_multiply(result[j], root);
            if j + 1 < degree {
                result[j] ^= result[j + 1];
            }
        }
        root = reed_solomon_multiply(root, 0x02);
        debug_assert!(i < degree);
    }
    result
}

fn reed_solomon_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; divisor.len()];
    for &byte in data {
        let factor = byte ^ result.remove(0);
        result.push(0);
        for (dst, &coefficient) in result.iter_mut().zip(divisor) {
            *dst ^= reed_solomon_multiply(coefficient, factor);
        }
    }
    result
}

fn reed_solomon_multiply(x: u8, y: u8) -> u8 {
    let mut z = 0u16;
    let mut x = u16::from(x);
    let mut y = u16::from(y);
    while y != 0 {
        if y & 1 != 0 {
            z ^= x;
        }
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= 0x11D;
        }
        y >>= 1;
    }
    z as u8
}

fn format_bits(level: EcLevel, mask: u8) -> u32 {
    let data = (level.format_bits() << 3) | u32::from(mask);
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    ((data << 10) | rem) ^ 0x5412
}

fn mask_bit(mask: u8, x: usize, y: usize) -> bool {
    match mask {
        0 => (x + y).is_multiple_of(2),
        1 => y.is_multiple_of(2),
        2 => x.is_multiple_of(3),
        3 => (x + y).is_multiple_of(3),
        4 => (x / 3 + y / 2).is_multiple_of(2),
        5 => (x * y) % 2 + (x * y) % 3 == 0,
        6 => ((x * y) % 2 + (x * y) % 3).is_multiple_of(2),
        7 => ((x + y) % 2 + (x * y) % 3).is_multiple_of(2),
        _ => unreachable!("mask is in 0..8"),
    }
}

fn bit(value: u32, index: usize) -> bool {
    ((value >> index) & 1) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_basic_payload() {
        let code = QrCode::new(b"iroha").expect("qr");
        assert_eq!(code.version(), 1);
        assert_eq!(code.width(), 21);
        assert!(code.is_functional(3, 3));
        assert!(code.is_dark(3, 3));
    }

    #[test]
    fn renders_svg_without_xml_prelude() {
        let code = QrCode::new(b"iroha").expect("qr");
        let svg = code.to_svg(192, "#000000", "#FFFFFF");
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("fill=\"#000000\""));
        assert!(svg.contains("fill=\"#FFFFFF\""));
    }

    #[test]
    fn rejects_oversized_payload() {
        let payload = vec![0u8; 4_000];
        let err = QrCode::with_error_correction_level(&payload, EcLevel::H).expect_err("too big");
        assert!(matches!(err, QrError::DataTooLong { .. }));
    }
}
