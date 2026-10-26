//! The turn layer: data in the way the lit petals point.
//!
//! A field lane tells the decoder exactly which petals of its sector are lit. The first
//! [`TURN_CARRIERS`] lit petals of the sector, in the fixed pseudo-random [`carrier_order`], carry
//! two more bits each: the petal is turned about its own centre by 0°, 45°, 90° or 135° from its
//! outward direction ([`turn_angle`]). A turned petal stays inside its own cell, so it moves no
//! light toward its neighbours and the field lanes read as before; a camera that resolves the
//! petals' shapes also reads the sector's turn lane.
//!
//! The turn lane of a sector is one Reed–Solomon codeword, RS(90, 68), XOR-whitened: four
//! fountain atoms from the repair range (ids from [`TURN_ID_BASE`], see [`turn_first_id`]) and a
//! CRC-32C check that binds them to the stream tag, frame and sector of the field lane they ride
//! on ([`turn_check`]). Carrier `j` shows bits `2j` and `2j + 1` of the transmitted codeword (most
//! significant bit first) as a Gray-coded position ([`turn_position`]). A sector with fewer than
//! [`TURN_CARRIERS`] lit petals turns none (about one sector in 60,000).
//!
//! [`read_turns`] reads the turn lanes of a decoded capture: every carrier's neighbourhood is
//! sampled in the petal's own outward frame, following a smooth correction of the pose fitted to
//! the carriers' centroids; the blur is the one under which the carriers and their lit neighbours
//! explain the image best; and each carrier takes the turn whose (slightly wider) template matches
//! best once its lit neighbours, at their own decided turns, are subtracted.

use std::sync::OnceLock;

use crate::crc::crc32c;
use crate::decode::{DecodedFrame, LaneResult};
use crate::fountain::Atom;
use crate::geometry::Pose;
use crate::image::Luma;
use crate::lanes::{ATOM_LEN, FIELD_WORD, Lane, encode_lane};
use crate::layout::{
    BAND_COUNT, BAND_OFFSETS, FIELD_CELLS, SECTORS, cell_angle, cell_band_slot, cell_center,
    petal_lit, sector_cells,
};
use crate::prng::Xorshift32;
use crate::rs::{ReedSolomon, RsError};
use crate::stream::{AtomPacket, LaneHeader};

/// Turn positions a carrier petal can take.
pub const TURN_POSITIONS: usize = 4;
/// Bits one carrier shows.
pub const TURN_BITS: usize = 2;
/// Carrier petals of a sector.
pub const TURN_CARRIERS: usize = 360;
/// Codeword length of a turn lane in bytes.
pub const TURN_WORD: usize = TURN_CARRIERS * TURN_BITS / 8;
/// Parity bytes of a turn lane.
pub const TURN_PARITY: usize = 22;
/// Data bytes of a turn lane: the atoms, then their check.
pub const TURN_DATA: usize = TURN_WORD - TURN_PARITY;
/// Atoms carried by each turn lane.
pub const TURN_ATOMS: usize = 4;
/// Bytes of the check that closes a turn lane's data.
pub const TURN_CHECK_LEN: usize = 4;
/// Fountain id of the first turn atom of frame 0. The field lanes and lane `D` number their atoms
/// from 0 and never come near it.
pub const TURN_ID_BASE: u32 = 1 << 31;

const _: () = {
    assert!(TURN_DATA == TURN_ATOMS * ATOM_LEN + TURN_CHECK_LEN);
    assert!(TURN_CARRIERS <= FIELD_WORD * 8);
    assert!(1 << TURN_BITS == TURN_POSITIONS);
    assert!((TURN_CARRIERS * TURN_BITS).is_multiple_of(8));
};

/// The angle (radians, clockwise on screen) by which a petal at turn position `position` is
/// turned from its outward direction: `π · position / 4`.
#[must_use]
pub fn turn_angle(position: u8) -> f64 {
    core::f64::consts::PI * f64::from(position) / TURN_POSITIONS as f64
}

/// The two bits a turn position shows: its Gray code `position ^ (position >> 1)`. Neighbouring
/// positions (135° and 0° neighbour too, a petal turned by 180° differing only by its notch) differ
/// in one bit.
#[must_use]
pub const fn turn_label(position: u8) -> u8 {
    position ^ (position >> 1)
}

/// The turn position that shows the two bits `label` (the inverse of [`turn_label`]).
#[must_use]
pub const fn turn_position(label: u8) -> u8 {
    label ^ (label >> 1)
}

/// The order in which the lit petals of sector `sector` become carriers: its codeword cells (the
/// first `8 ×` [`FIELD_WORD`] of [`sector_cells`]) shuffled by Fisher–Yates with a [`Xorshift32`]
/// seeded `0x4F52_4430 + sector` ("ORD0" … "ORD3"): for `i` from 839 down to 1, position `i`
/// swaps with position `next_u32() mod (i + 1)`.
///
/// # Panics
/// Panics when `sector` is not below [`SECTORS`].
#[must_use]
pub fn carrier_order(sector: usize) -> &'static [usize] {
    static ORDERS: OnceLock<Vec<Vec<usize>>> = OnceLock::new();
    &ORDERS.get_or_init(|| {
        (0..SECTORS)
            .map(|s| {
                let mut order = sector_cells(s)[..FIELD_WORD * 8].to_vec();
                let mut rng = Xorshift32::new(0x4F52_4430 + s as u32);
                for i in (1..order.len()).rev() {
                    let j = rng.next_u32() as usize % (i + 1);
                    order.swap(i, j);
                }
                order
            })
            .collect()
    })[sector]
}

/// The carriers of sector `sector` under the lit map `petals` (one flag per field cell): the
/// first [`TURN_CARRIERS`] lit cells in [`carrier_order`], or `None` when fewer are lit.
///
/// # Panics
/// Panics when `sector` is not below [`SECTORS`] or `petals` holds fewer than [`FIELD_CELLS`]
/// flags.
#[must_use]
pub fn carriers(petals: &[bool], sector: usize) -> Option<Vec<usize>> {
    let chosen: Vec<usize> = carrier_order(sector)
        .iter()
        .copied()
        .filter(|&cell| petals[cell])
        .take(TURN_CARRIERS)
        .collect();
    (chosen.len() == TURN_CARRIERS).then_some(chosen)
}

/// Fountain id of the first of the [`TURN_ATOMS`] atoms the turn lane of `sector` carries in
/// `frame`: `2³¹ + 16 · frame + 4 · sector`.
#[must_use]
pub fn turn_first_id(frame: u16, sector: u8) -> u32 {
    TURN_ID_BASE
        + u32::from(frame) * (SECTORS * TURN_ATOMS) as u32
        + u32::from(sector) * TURN_ATOMS as u32
}

/// The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the sector and
/// the atoms, so a turn lane read under the wrong field lane, frame or stream is refused.
#[must_use]
pub fn turn_check(header: LaneHeader, sector: u8, atoms: &[u8]) -> u32 {
    let mut bytes = Vec::with_capacity(4 + atoms.len());
    bytes.push(header.tag);
    bytes.extend_from_slice(&header.frame.to_be_bytes());
    bytes.push(sector);
    bytes.extend_from_slice(atoms);
    crc32c(&bytes)
}

/// The data of a turn lane: the atoms followed by their [`turn_check`] (big-endian).
///
/// # Panics
/// Panics when `atoms` is not [`TURN_ATOMS`] atoms long.
#[must_use]
pub fn turn_data(header: LaneHeader, sector: u8, atoms: &[u8]) -> Vec<u8> {
    assert_eq!(atoms.len(), TURN_ATOMS * ATOM_LEN, "turn atoms length");
    let mut data = atoms.to_vec();
    data.extend_from_slice(&turn_check(header, sector, atoms).to_be_bytes());
    data
}

/// The atoms of turn lane data whose check matches `header` and `sector`.
#[must_use]
pub fn turn_packet(header: LaneHeader, sector: u8, data: &[u8]) -> Option<AtomPacket> {
    if data.len() != TURN_DATA || usize::from(sector) >= SECTORS {
        return None;
    }
    let (atoms, check) = data.split_at(TURN_ATOMS * ATOM_LEN);
    if turn_check(header, sector, atoms).to_be_bytes() != check {
        return None;
    }
    Some(AtomPacket {
        header,
        first_id: turn_first_id(header.frame, sector),
        atoms: atoms
            .chunks_exact(ATOM_LEN)
            .map(|chunk| {
                let mut atom: Atom = [0; ATOM_LEN];
                atom.copy_from_slice(chunk);
                atom
            })
            .collect(),
    })
}

/// The whitening sequence of the turn lane of `sector`: one byte per codeword byte from a
/// [`Xorshift32`] seeded `0x5452_4E30 + sector` ("TRN0" … "TRN3").
#[must_use]
pub fn turn_whitening(sector: u8) -> Vec<u8> {
    let mut rng = Xorshift32::new(0x5452_4E30 + u32::from(sector));
    (0..TURN_WORD).map(|_| rng.next_byte()).collect()
}

/// Encodes turn lane data into the transmitted (whitened) codeword.
///
/// # Panics
/// Panics when `data` is not exactly [`TURN_DATA`] bytes.
#[must_use]
pub fn encode_turn_lane(sector: u8, data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len(), TURN_DATA, "turn data length mismatch");
    let mut word = ReedSolomon::new(TURN_PARITY).encode(data);
    for (byte, mask) in word.iter_mut().zip(turn_whitening(sector)) {
        *byte ^= mask;
    }
    word
}

/// Decodes a transmitted turn codeword, returning its data and how many byte positions the
/// Reed–Solomon decoder rewrote. The check is not verified here (see [`turn_packet`]).
///
/// # Errors
/// Returns [`RsError`] when the word is uncorrectable.
pub fn decode_turn_lane_counted(
    sector: u8,
    transmitted: &[u8],
    erasures: &[usize],
) -> Result<(Vec<u8>, usize), RsError> {
    if transmitted.len() != TURN_WORD {
        return Err(RsError::InvalidShape);
    }
    let mut word: Vec<u8> = transmitted
        .iter()
        .zip(turn_whitening(sector))
        .map(|(byte, mask)| byte ^ mask)
        .collect();
    let corrected = ReedSolomon::new(TURN_PARITY).decode(&mut word, erasures)?;
    word.truncate(TURN_DATA);
    Ok((word, corrected))
}

/// The turn position carrier `index` of a transmitted turn codeword shows.
#[must_use]
pub fn carrier_position(word: &[u8], index: usize) -> u8 {
    let byte = word[index / 4];
    turn_position(byte >> (6 - 2 * (index % 4)) & 3)
}

// ---- reading ------------------------------------------------------------------------------------

/// Half the side, in canvas units, of the square a petal template is tabulated over.
const TEMPLATE_HALF: f64 = 9.0;
/// Template table step in canvas units.
const TEMPLATE_STEP: f64 = 0.25;
/// Template table side in steps.
const TEMPLATE_SIDE: usize = 73;
/// Gaussian blurs (σ in canvas units) the turn reader fits.
pub const TURN_BLURS: [f64; 6] = [1.0, 1.4, 1.9, 2.5, 3.2, 4.0];
/// From this fitted blur on (3.2 units), the camera cannot resolve the turns and the reader stops.
const BLUR_LIMIT: usize = 4;
/// The window of a carrier is sampled every canvas unit over ±`GRID_HALF` units in its own frame.
const GRID_HALF: i32 = 6;
/// Window side in samples.
const GRID_SIDE: usize = 13;
/// σ in canvas units of the Gaussian that weights the window.
const WINDOW_SIGMA: f64 = 3.6;
/// σ in canvas units of the Gaussian that follows a carrier's centroid.
const CENTROID_SIGMA: f64 = 2.6;
/// Lit petals within this distance of a carrier are modelled as its neighbours.
const NEIGHBOUR_DISTANCE: f64 = 19.0;
/// A neighbour's template is subtracted within this distance of its centre. Radially adjacent cell
/// centres are 11 units apart and the window grid is integral, so a whole number here would put grid
/// points exactly on the boundary, where the last bit of a platform's `sin` and `cos` decides.
const NEIGHBOUR_REACH: f64 = 9.5;
/// The warp and the blur are fitted on every this-many-th carrier.
const SAMPLE_STEP: usize = 17;

/// A petal blurred by a Gaussian, tabulated in its own frame: `along` its outward direction and
/// `across` it, every [`TEMPLATE_STEP`] units over ±[`TEMPLATE_HALF`].
struct Template {
    table: Vec<f64>,
}

impl Template {
    /// The coverage of every table cell by the unturned petal (4 × 4 samples per cell), blurred
    /// by a Gaussian of `sigma` canvas units truncated at `3σ` (rows, then columns; taps outside
    /// the table count as zero, and the kernel is normalised by its full sum).
    fn new(sigma: f64) -> Self {
        let side = TEMPLATE_SIDE;
        let mut cover = vec![0.0f64; side * side];
        for j in 0..side {
            for i in 0..side {
                let along = -TEMPLATE_HALF + i as f64 * TEMPLATE_STEP;
                let across = -TEMPLATE_HALF + j as f64 * TEMPLATE_STEP;
                let mut hits = 0u32;
                for sj in 0..4 {
                    for si in 0..4 {
                        let da = (f64::from(si) - 1.5) * TEMPLATE_STEP / 4.0;
                        let dc = (f64::from(sj) - 1.5) * TEMPLATE_STEP / 4.0;
                        if petal_lit(along + da, across + dc, 0.0) {
                            hits += 1;
                        }
                    }
                }
                cover[j * side + i] = f64::from(hits) / 16.0;
            }
        }
        let radius = (3.0 * sigma / TEMPLATE_STEP).ceil() as i64;
        let kernel: Vec<f64> = (-radius..=radius)
            .map(|k| (-(k as f64 * TEMPLATE_STEP).powi(2) / (2.0 * sigma * sigma)).exp())
            .collect();
        let norm: f64 = kernel.iter().sum();
        let blur = |source: &[f64], rows: bool| -> Vec<f64> {
            let mut out = vec![0.0f64; side * side];
            for j in 0..side {
                for i in 0..side {
                    let mut sum = 0.0;
                    for (t, w) in kernel.iter().enumerate() {
                        let k = t as i64 - radius;
                        let (ii, jj) = if rows {
                            (i as i64 + k, j as i64)
                        } else {
                            (i as i64, j as i64 + k)
                        };
                        if ii >= 0 && jj >= 0 && (ii as usize) < side && (jj as usize) < side {
                            sum += w * source[jj as usize * side + ii as usize];
                        }
                    }
                    out[j * side + i] = sum / norm;
                }
            }
            out
        };
        Self {
            table: blur(&blur(&cover, true), false),
        }
    }

    /// The template at a point of the petal's own frame (bilinear; zero outside the table).
    fn at(&self, along: f64, across: f64) -> f64 {
        let x = (along + TEMPLATE_HALF) / TEMPLATE_STEP;
        let y = (across + TEMPLATE_HALF) / TEMPLATE_STEP;
        let last = (TEMPLATE_SIDE - 1) as f64;
        if !(x >= 0.0 && y >= 0.0 && x < last && y < last) {
            return 0.0;
        }
        let (i, j) = (x as usize, y as usize);
        let (fx, fy) = (x - i as f64, y - j as f64);
        let t = &self.table;
        let row = j * TEMPLATE_SIDE + i;
        let top = t[row] * (1.0 - fx) + t[row + 1] * fx;
        let bottom = t[row + TEMPLATE_SIDE] * (1.0 - fx) + t[row + TEMPLATE_SIDE + 1] * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// The turn reader's blurred petal template for blur `blur` (an index into [`TURN_BLURS`]) at a
/// point of the unturned petal's own frame (`along` its axis, `across` it): what conformance tests
/// compare an SDK's tables with.
///
/// # Panics
/// Panics when `blur` is not an index into [`TURN_BLURS`].
#[must_use]
pub fn turn_template(blur: usize, along: f64, across: f64) -> f64 {
    tables().templates[blur].at(along, across)
}

/// One window sample point: along, across, weight.
type GridPoint = (f64, f64, f64);

/// The window grid, along the outward direction fastest, and per blur the templates and the
/// weighted kernel of every turn position on the grid.
struct Tables {
    grid: Vec<GridPoint>,
    templates: Vec<Template>,
    kernels: Vec<[Vec<f64>; TURN_POSITIONS]>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut grid = Vec::new();
        for b in -GRID_HALF..=GRID_HALF {
            for a in -GRID_HALF..=GRID_HALF {
                let (a, b) = (f64::from(a), f64::from(b));
                let weight = (-(a * a + b * b) / (2.0 * WINDOW_SIGMA * WINDOW_SIGMA)).exp();
                grid.push((a, b, weight));
            }
        }
        let templates: Vec<Template> = TURN_BLURS.iter().map(|&s| Template::new(s)).collect();
        let kernels = templates
            .iter()
            .map(|template| {
                core::array::from_fn(|k| {
                    let (sin, cos) = turn_angle(k as u8).sin_cos();
                    grid.iter()
                        .map(|&(a, b, w)| w * template.at(a * cos + b * sin, -a * sin + b * cos))
                        .collect()
                })
            })
            .collect();
        Tables {
            grid,
            templates,
            kernels,
        }
    })
}

/// A field cell near another one: its index, and its centre and outward direction in the other
/// cell's outward frame.
#[derive(Clone, Copy)]
struct Near {
    cell: u16,
    along: f64,
    across: f64,
    angle: f64,
}

/// The cells within [`NEIGHBOUR_DISTANCE`] of every field cell (itself excluded), in index order.
/// Bands are 11 units apart, so only the two bands on either side can hold them.
fn neighbours() -> &'static Vec<Vec<Near>> {
    static TABLE: OnceLock<Vec<Vec<Near>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let centres: Vec<(f64, f64)> = (0..FIELD_CELLS).map(cell_center).collect();
        (0..FIELD_CELLS)
            .map(|cell| {
                let (x, y) = centres[cell];
                let angle = cell_angle(cell);
                let (sin, cos) = angle.sin_cos();
                let (band, _) = cell_band_slot(cell);
                let first = BAND_OFFSETS[band.saturating_sub(2)];
                let last = BAND_OFFSETS[(band + 3).min(BAND_COUNT)];
                (first..last)
                    .filter(|&other| other != cell)
                    .filter_map(|other| {
                        let (dx, dy) = (centres[other].0 - x, centres[other].1 - y);
                        (dx * dx + dy * dy <= NEIGHBOUR_DISTANCE * NEIGHBOUR_DISTANCE).then(|| {
                            Near {
                                cell: other as u16,
                                along: dx * cos + dy * sin,
                                across: -dx * sin + dy * cos,
                                angle: cell_angle(other) - angle,
                            }
                        })
                    })
                    .collect()
            })
            .collect()
    })
}

/// Samples the image on the window grid of the petal at `cell`, centred `shift` canvas units off
/// the cell centre, in the petal's outward frame (grid rows across, columns along). Within a window
/// the pose is taken as affine: with `o` the image point of the centre and `u`, `v` the image steps
/// of one unit along and across, grid point `(a, b)` is sampled at `(o + b·v) + a·u`.
fn sample_window(image: &Luma, pose: &Pose, cell: usize, shift: (f64, f64), out: &mut [f64]) {
    let (x, y) = cell_center(cell);
    let (x, y) = (x + shift.0, y + shift.1);
    let (sin, cos) = cell_angle(cell).sin_cos();
    let origin = pose.apply(x, y);
    let along = pose.apply(x + cos, y + sin);
    let across = pose.apply(x - sin, y + cos);
    let u = (along.0 - origin.0, along.1 - origin.1);
    let v = (across.0 - origin.0, across.1 - origin.1);
    let mut index = 0;
    for b in -GRID_HALF..=GRID_HALF {
        let b = f64::from(b);
        let (row_x, row_y) = (origin.0 + b * v.0, origin.1 + b * v.1);
        for a in -GRID_HALF..=GRID_HALF {
            let a = f64::from(a);
            out[index] = image.sample(row_x + a * u.0, row_y + a * u.1);
            index += 1;
        }
    }
}

/// Rescales a window so that its 10th percentile (of 169 samples, the 17th smallest) becomes 0 and
/// its 98th (the 166th smallest) 1 (a span below one grey level counts as one), clamped to
/// −0.2 … 1.2.
fn normalise(window: &mut [f64]) {
    let mut sorted = window.to_vec();
    let n = sorted.len();
    let (low, high) = (n / 10, n * 98 / 100);
    let (_, top, _) = sorted.select_nth_unstable_by(high, f64::total_cmp);
    let top = *top;
    let (_, bottom, _) = sorted[..high].select_nth_unstable_by(low, f64::total_cmp);
    let bottom = *bottom;
    let span = (top - bottom).max(1.0);
    for value in window.iter_mut() {
        *value = ((*value - bottom) / span).clamp(-0.2, 1.2);
    }
}

/// The centroid (along, across) of the light of a [`normalise`]d window: every sample weighs its
/// level (clamped to 0 … 1) times a Gaussian of σ [`CENTROID_SIGMA`] that starts at the window
/// centre and follows the centroid three times; the Gaussian is its along factor times its across
/// factor.
fn centroid(window: &[f64]) -> (f64, f64) {
    let (mut ea, mut eb) = (0.0, 0.0);
    let mut along = [0.0f64; GRID_SIDE];
    let mut across = [0.0f64; GRID_SIDE];
    let k = 2.0 * CENTROID_SIGMA * CENTROID_SIGMA;
    for _ in 0..3 {
        for (i, (ga, gb)) in along.iter_mut().zip(across.iter_mut()).enumerate() {
            let t = f64::from(i as i32 - GRID_HALF);
            *ga = (-(t - ea) * (t - ea) / k).exp();
            *gb = (-(t - eb) * (t - eb) / k).exp();
        }
        let (mut sum, mut sa, mut sb) = (0.0, 0.0, 0.0);
        for (j, row) in window.chunks_exact(GRID_SIDE).enumerate() {
            let b = f64::from(j as i32 - GRID_HALF);
            for (i, &level) in row.iter().enumerate() {
                let a = f64::from(i as i32 - GRID_HALF);
                let w = level.clamp(0.0, 1.0) * along[i] * across[j];
                sum += w;
                sa += w * a;
                sb += w * b;
            }
        }
        if sum <= 0.0 {
            break;
        }
        ea = sa / sum;
        eb = sb / sum;
    }
    (ea, eb)
}

/// The ten cubic monomials of a canvas point, centred on the canvas and scaled by 384.
fn monomials(x: f64, y: f64) -> [f64; 10] {
    let (x, y) = ((x - 512.0) / 384.0, (y - 512.0) / 384.0);
    [
        1.0,
        x,
        y,
        x * x,
        x * y,
        y * y,
        x * x * x,
        x * x * y,
        x * y * y,
        y * y * y,
    ]
}

/// The least-squares cubic through `values` at `points`: normal equations (summed point by point)
/// with 10⁻⁶ added to the diagonal, solved by Gauss–Jordan elimination with partial pivoting.
fn fit_cubic(points: &[(f64, f64)], values: &[f64]) -> [f64; 10] {
    let mut a = [[0.0f64; 11]; 10];
    for (&(x, y), &value) in points.iter().zip(values) {
        let f = monomials(x, y);
        for r in 0..10 {
            for c in 0..10 {
                a[r][c] += f[r] * f[c];
            }
            a[r][10] += f[r] * value;
        }
    }
    for (r, row) in a.iter_mut().enumerate() {
        row[r] += 1e-6;
    }
    for col in 0..10 {
        let mut pivot = col;
        for r in col + 1..10 {
            if a[r][col].abs() > a[pivot][col].abs() {
                pivot = r;
            }
        }
        a.swap(col, pivot);
        let p = a[col][col];
        if p.abs() < 1e-12 {
            continue;
        }
        for value in &mut a[col][col..] {
            *value /= p;
        }
        let pivot_row = a[col];
        for (r, row) in a.iter_mut().enumerate() {
            if r != col {
                let f = row[col];
                for (value, &pivot_value) in row[col..].iter_mut().zip(&pivot_row[col..]) {
                    *value -= f * pivot_value;
                }
            }
        }
    }
    core::array::from_fn(|r| a[r][10])
}

fn eval_cubic(coefficients: &[f64; 10], x: f64, y: f64) -> f64 {
    monomials(x, y)
        .iter()
        .zip(coefficients)
        .map(|(m, c)| m * c)
        .sum()
}

/// Adds `sign ×` the template of the neighbour `near`, turned by `turn`, to a window, at the grid
/// points within [`NEIGHBOUR_REACH`] of the neighbour's centre.
fn add_neighbour(window: &mut [f64], template: &Template, near: &Near, turn: f64, sign: f64) {
    let (sin, cos) = (near.angle + turn).sin_cos();
    let reach2 = NEIGHBOUR_REACH * NEIGHBOUR_REACH;
    // only the grid rows and columns within reach of the neighbour's centre
    let span = |centre: f64| {
        let low = (centre - NEIGHBOUR_REACH).ceil().max(-f64::from(GRID_HALF));
        let high = (centre + NEIGHBOUR_REACH).floor().min(f64::from(GRID_HALF));
        (low as i32, high as i32)
    };
    let (a0, a1) = span(near.along);
    let (b0, b1) = span(near.across);
    for b in b0..=b1 {
        let ub = f64::from(b) - near.across;
        let row = (b + GRID_HALF) as usize * GRID_SIDE;
        for a in a0..=a1 {
            let ua = f64::from(a) - near.along;
            if ua * ua + ub * ub > reach2 {
                continue;
            }
            window[row + (a + GRID_HALF) as usize] +=
                sign * template.at(ua * cos + ub * sin, -ua * sin + ub * cos);
        }
    }
}

fn dot(window: &[f64], kernel: &[f64]) -> f64 {
    window.iter().zip(kernel).map(|(x, k)| x * k).sum()
}

/// The best turn position for a window and how far its score leads the runner-up's (ties: the
/// lower position).
fn decide(window: &[f64], kernels: &[Vec<f64>; TURN_POSITIONS]) -> (u8, f64) {
    let scores: [f64; TURN_POSITIONS] = core::array::from_fn(|k| dot(window, &kernels[k]));
    let mut best = 0;
    for k in 1..TURN_POSITIONS {
        if scores[k] > scores[best] {
            best = k;
        }
    }
    let runner_up = (0..TURN_POSITIONS)
        .filter(|&k| k != best)
        .map(|k| scores[k])
        .fold(f64::NEG_INFINITY, f64::max);
    (best as u8, scores[best] - runner_up)
}

/// What the turn reader decided for one capture.
struct Reading {
    headers: [Option<LaneHeader>; SECTORS],
    ranges: [Option<core::ops::Range<usize>>; SECTORS],
    carriers: Vec<usize>,
    decided: Vec<(u8, f64)>,
    fitted: usize,
}

/// Decides the turn of every carrier of the sectors `frame` decoded, when a sector that `need`
/// names has carriers; with `limit`, not when the fitted blur reaches [`BLUR_LIMIT`].
///
/// 1. The lit petals of the decoded sectors come from their field lanes, and the carriers of each
///    from [`carriers`].
/// 2. The pose's smooth error: for every carrier in steps of [`SAMPLE_STEP`], the window is
///    sampled at its cell centre and [`normalise`]d, and its [`centroid`] turned into a canvas
///    offset; a cubic fitted to the offsets over the canvas moves every carrier's window, which is
///    then sampled and normalised.
/// 3. The blur: for every carrier in steps of [`SAMPLE_STEP`], the model of the petal at its best
///    turn plus its lit neighbours (within [`NEIGHBOUR_DISTANCE`]) at theirs (each decided on its
///    own window, unturned when it carries nothing) is correlated with the window, weighted by the
///    window Gaussian; the blur with the largest sum fits best.
/// 4. The turns: with the next wider blur's kernels, every carrier takes the best turn of its own
///    window, then, carrier by carrier in order, the best turn of its window with its lit
///    neighbours' templates at their latest turns subtracted.
fn decide_turns(
    image: &Luma,
    frame: &DecodedFrame,
    need: [bool; SECTORS],
    limit: bool,
) -> Option<Reading> {
    // the lit petals of every sector that decoded
    let mut lit = vec![false; FIELD_CELLS];
    let mut headers: [Option<LaneHeader>; SECTORS] = [None; SECTORS];
    for (sector, field) in frame.fields.iter().enumerate() {
        let Some(field) = field else {
            continue;
        };
        let [tag, high, low, ..] = field.data[..] else {
            continue;
        };
        headers[sector] = Some(LaneHeader {
            tag,
            frame: u16::from_be_bytes([high, low]),
        });
        let word = encode_lane(Lane::Field(sector as u8), &field.data);
        for (bit, &cell) in sector_cells(sector)[..FIELD_WORD * 8].iter().enumerate() {
            lit[cell] = word[bit / 8] >> (7 - bit % 8) & 1 == 1;
        }
    }
    // the carriers of every decoded sector, sector by sector in carrier order
    let mut carrier_cells: Vec<usize> = Vec::new();
    let mut ranges: [Option<core::ops::Range<usize>>; SECTORS] = Default::default();
    for sector in 0..SECTORS {
        if headers[sector].is_none() {
            continue;
        }
        if let Some(cells) = carriers(&lit, sector) {
            let start = carrier_cells.len();
            carrier_cells.extend(cells);
            ranges[sector] = Some(start..carrier_cells.len());
        }
    }
    if (0..SECTORS).all(|s| !need[s] || ranges[s].is_none()) {
        return None;
    }
    let tables = tables();
    let points = GRID_SIDE * GRID_SIDE;
    let mut windows = vec![0.0f64; carrier_cells.len() * points];
    // the pose's smooth error, from the centroids of a sample of the carriers
    let centres: Vec<(f64, f64)> = carrier_cells.iter().map(|&c| cell_center(c)).collect();
    let mut sampled = Vec::new();
    let mut offsets_x = Vec::new();
    let mut offsets_y = Vec::new();
    let mut scratch = vec![0.0f64; points];
    for index in (0..carrier_cells.len()).step_by(SAMPLE_STEP) {
        let cell = carrier_cells[index];
        sample_window(image, &frame.pose, cell, (0.0, 0.0), &mut scratch);
        normalise(&mut scratch);
        let (a, b) = centroid(&scratch);
        let (sin, cos) = cell_angle(cell).sin_cos();
        sampled.push(centres[index]);
        offsets_x.push(a * cos - b * sin);
        offsets_y.push(a * sin + b * cos);
    }
    let fit_x = fit_cubic(&sampled, &offsets_x);
    let fit_y = fit_cubic(&sampled, &offsets_y);
    for (index, &cell) in carrier_cells.iter().enumerate() {
        let (x, y) = centres[index];
        let shift = (eval_cubic(&fit_x, x, y), eval_cubic(&fit_y, x, y));
        let window = &mut windows[index * points..(index + 1) * points];
        sample_window(image, &frame.pose, cell, shift, window);
        normalise(window);
    }
    let window_of = |index: usize| &windows[index * points..(index + 1) * points];
    let mut carrier_of = vec![u32::MAX; FIELD_CELLS];
    for (index, &cell) in carrier_cells.iter().enumerate() {
        carrier_of[cell] = index as u32;
    }
    // the blur under which a sample of the carriers, each at its best turn with its lit neighbours
    // at theirs, explains the windows best
    let mut fitted = 0;
    let mut best_score = f64::NEG_INFINITY;
    let mut model = vec![0.0f64; points];
    for index in 0..TURN_BLURS.len() {
        let (template, kernels) = (&tables.templates[index], &tables.kernels[index]);
        let turn_of = |carrier: usize| decide(window_of(carrier), kernels).0;
        let mut score = 0.0;
        for i in (0..carrier_cells.len()).step_by(SAMPLE_STEP) {
            let (sin, cos) = turn_angle(turn_of(i)).sin_cos();
            for (m, &(a, b, _)) in model.iter_mut().zip(&tables.grid) {
                *m = template.at(a * cos + b * sin, -a * sin + b * cos);
            }
            for near in &neighbours()[carrier_cells[i]] {
                let other = usize::from(near.cell);
                if !lit[other] {
                    continue;
                }
                let turn = match carrier_of[other] {
                    u32::MAX => 0.0,
                    j => turn_angle(turn_of(j as usize)),
                };
                add_neighbour(&mut model, template, near, turn, 1.0);
            }
            let (mut cross, mut mm, mut xx) = (0.0, 0.0, 0.0);
            for ((&x, &m), &(_, _, w)) in window_of(i).iter().zip(&model).zip(&tables.grid) {
                cross += w * x * m;
                mm += w * m * m;
                xx += w * x * x;
            }
            score += cross / (mm * xx).sqrt().max(1e-9);
        }
        if score > best_score {
            best_score = score;
            fitted = index;
        }
    }
    if limit && fitted >= BLUR_LIMIT {
        return None;
    }
    // the turns, decided with the next wider blur
    let blur = (fitted + 1).min(TURN_BLURS.len() - 1);
    let (template, kernels) = (&tables.templates[blur], &tables.kernels[blur]);
    let mut decided: Vec<(u8, f64)> = (0..carrier_cells.len())
        .map(|i| decide(window_of(i), kernels))
        .collect();
    for (index, &cell) in carrier_cells.iter().enumerate() {
        scratch.copy_from_slice(window_of(index));
        for near in &neighbours()[cell] {
            let other = usize::from(near.cell);
            if !lit[other] {
                continue;
            }
            let turn = match carrier_of[other] {
                u32::MAX => 0.0,
                j => turn_angle(decided[j as usize].0),
            };
            add_neighbour(&mut scratch, template, near, turn, -1.0);
        }
        decided[index] = decide(&scratch, kernels);
    }
    Some(Reading {
        headers,
        ranges,
        carriers: carrier_cells,
        decided,
        fitted,
    })
}

/// Reads the turn lanes of the sectors `frame` decoded from `image`, except those in `skip`.
///
/// The carriers of every decoded sector are decided (so that each carrier's neighbours are
/// modelled at their own turns), but only the lanes of the sectors not in `skip` are decoded, and
/// none when the image is too blurred to show turns. A lane passes when its Reed–Solomon codeword
/// decodes, with 0, 2, 5, 7 or 11 of its least confident bytes erased (a byte's confidence is the
/// smallest lead of its four carriers' turns), into data whose [`turn_check`] matches its sector's
/// field lane header. Returns the lane data (atoms and check) per sector.
#[must_use]
pub fn read_turns(
    image: &Luma,
    frame: &DecodedFrame,
    skip: [bool; SECTORS],
) -> [Option<LaneResult>; SECTORS] {
    let mut results: [Option<LaneResult>; SECTORS] = Default::default();
    let Some(reading) = decide_turns(image, frame, skip.map(|s| !s), true) else {
        return results;
    };
    for sector in 0..SECTORS {
        let (Some(range), Some(header)) = (&reading.ranges[sector], reading.headers[sector]) else {
            continue;
        };
        if skip[sector] {
            continue;
        }
        let mut word = vec![0u8; TURN_WORD];
        let mut confidence = vec![f64::MAX; TURN_WORD];
        for (j, &(position, margin)) in reading.decided[range.clone()].iter().enumerate() {
            word[j / 4] |= turn_label(position) << (6 - 2 * (j % 4));
            let c = &mut confidence[j / 4];
            *c = c.min(margin);
        }
        results[sector] = decode_turn_word(sector as u8, header, &word, &confidence);
    }
    results
}

/// The turns a reader decided for a capture, for diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedTurns {
    /// The blur (Gaussian σ in canvas units) that fits the carriers best.
    pub blur: f64,
    /// Every carrier of the decoded sectors: its cell, the turn position decided for it, and how
    /// far that position's score led the runner-up's.
    pub carriers: Vec<(usize, u8, f64)>,
}

/// What [`read_turns`] decides for every carrier of the sectors `frame` decoded, before any
/// Reed–Solomon correction and whatever the blur; `None` when no decoded sector has carriers.
#[must_use]
pub fn observed_turns(image: &Luma, frame: &DecodedFrame) -> Option<ObservedTurns> {
    let reading = decide_turns(image, frame, [true; SECTORS], false)?;
    Some(ObservedTurns {
        blur: TURN_BLURS[reading.fitted],
        carriers: reading
            .carriers
            .iter()
            .zip(&reading.decided)
            .map(|(&cell, &(position, margin))| (cell, position, margin))
            .collect(),
    })
}

/// Tries Reed–Solomon with 0, 2, 5, 7 and 11 of the least confident bytes erased (ties keep byte
/// order) and accepts the first result whose check matches.
fn decode_turn_word(
    sector: u8,
    header: LaneHeader,
    word: &[u8],
    confidence: &[f64],
) -> Option<LaneResult> {
    let mut order: Vec<usize> = (0..word.len()).collect();
    order.sort_by(|&a, &b| confidence[a].total_cmp(&confidence[b]));
    let nsym = TURN_PARITY;
    let mut schedule = vec![0, nsym / 8, nsym / 4, nsym / 3, nsym / 2];
    schedule.dedup();
    for erasures in schedule {
        let positions: Vec<usize> = order.iter().copied().take(erasures).collect();
        let mut trial = word.to_vec();
        for &p in &positions {
            trial[p] = 0;
        }
        let Ok((data, corrected)) = decode_turn_lane_counted(sector, &trial, &positions) else {
            continue;
        };
        if turn_packet(header, sector, &data).is_some() {
            return Some(LaneResult {
                data,
                corrected,
                erasures: positions.len(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::{FIELD_DATA, FrameCells};
    use crate::layout::{PETAL_HALF_LENGTH, PETAL_RADIUS};

    fn atoms(seed: u8) -> Vec<u8> {
        (0..(TURN_ATOMS * ATOM_LEN) as u8)
            .map(|b| b.wrapping_mul(29) ^ seed)
            .collect()
    }

    #[test]
    fn lane_sizes_are_consistent() {
        assert_eq!((TURN_WORD, TURN_DATA), (90, 68));
        assert_eq!(turn_first_id(0, 0), 1 << 31);
        assert_eq!(turn_first_id(1, 3), (1 << 31) + 16 + 12);
        assert_eq!(turn_first_id(u16::MAX, 3) + 3, (1 << 31) + 16 * 65_535 + 15);
        // the base schedule stays far below the turn ids however long a stream runs
        assert!(crate::stream::first_atom_id(u16::MAX) + 21 < TURN_ID_BASE);
    }

    #[test]
    fn gray_labels_differ_in_one_bit_between_neighbouring_turns() {
        for position in 0..TURN_POSITIONS as u8 {
            assert_eq!(turn_position(turn_label(position)), position);
            let next = (position + 1) % TURN_POSITIONS as u8;
            assert_eq!((turn_label(position) ^ turn_label(next)).count_ones(), 1);
        }
        assert!((turn_angle(2) - core::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn carrier_orders_shuffle_each_sectors_codeword_cells() {
        for sector in 0..SECTORS {
            let mut order = carrier_order(sector).to_vec();
            assert_ne!(order, sector_cells(sector)[..FIELD_WORD * 8]);
            order.sort_unstable();
            let mut cells = sector_cells(sector)[..FIELD_WORD * 8].to_vec();
            cells.sort_unstable();
            assert_eq!(order, cells);
        }
        // the four shuffles differ
        assert_ne!(
            carrier_order(0)
                .iter()
                .map(|&c| sector_cells(0).iter().position(|&x| x == c))
                .collect::<Vec<_>>(),
            carrier_order(1)
                .iter()
                .map(|&c| sector_cells(1).iter().position(|&x| x == c))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_sector_with_too_few_lit_petals_carries_nothing() {
        let mut petals = vec![false; FIELD_CELLS];
        for &cell in &carrier_order(2)[..TURN_CARRIERS - 1] {
            petals[cell] = true;
        }
        assert!(carriers(&petals, 2).is_none());
        petals[carrier_order(2)[TURN_CARRIERS + 7]] = true;
        let chosen = carriers(&petals, 2).expect("enough lit petals");
        assert_eq!(chosen.len(), TURN_CARRIERS);
        assert_eq!(
            chosen[TURN_CARRIERS - 1],
            carrier_order(2)[TURN_CARRIERS + 7]
        );
    }

    #[test]
    fn turn_lanes_roundtrip_and_bind_their_header() {
        let header = LaneHeader { tag: 7, frame: 300 };
        let data = turn_data(header, 1, &atoms(3));
        let word = encode_turn_lane(1, &data);
        assert_eq!(word.len(), TURN_WORD);
        let (decoded, corrected) = decode_turn_lane_counted(1, &word, &[]).unwrap();
        assert_eq!((decoded.clone(), corrected), (data.clone(), 0));
        let packet = turn_packet(header, 1, &decoded).expect("check matches");
        assert_eq!(packet.first_id, turn_first_id(300, 1));
        assert_eq!(packet.atoms.len(), TURN_ATOMS);
        assert_eq!(packet.atoms[2][..], atoms(3)[32..48]);
        // another frame, tag or sector does not match the check
        assert!(turn_packet(LaneHeader { tag: 7, frame: 301 }, 1, &decoded).is_none());
        assert!(turn_packet(LaneHeader { tag: 8, frame: 300 }, 1, &decoded).is_none());
        assert!(turn_packet(header, 2, &decoded).is_none());
        // a codeword whitened for another sector does not decode
        assert!(decode_turn_lane_counted(0, &word, &[]).is_err());
    }

    #[test]
    fn carriers_show_the_codeword_two_bits_at_a_time() {
        let fields: Vec<Vec<u8>> = (0..SECTORS as u8)
            .map(|s| {
                let data: Vec<u8> = (0..FIELD_DATA as u8)
                    .map(|b| b.wrapping_mul(13) ^ s)
                    .collect();
                encode_lane(Lane::Field(s), &data)
            })
            .collect();
        let mut cells = FrameCells::from_words(
            &[0; crate::lanes::D_WORD],
            [&fields[0], &fields[1], &fields[2], &fields[3]],
        );
        assert!(cells.turns.iter().all(|&t| t == 0));
        let words: Vec<Vec<u8>> = (0..SECTORS as u8)
            .map(|s| encode_turn_lane(s, &turn_data(LaneHeader { tag: 1, frame: 2 }, s, &atoms(s))))
            .collect();
        cells.set_turns([&words[0], &words[1], &words[2], &words[3]]);
        for (sector, word) in words.iter().enumerate() {
            assert_eq!(cells.turn_word(sector).map(Vec::from).as_ref(), Some(word));
            let chosen = carriers(&cells.petals, sector).unwrap();
            for (j, &cell) in chosen.iter().enumerate() {
                assert_eq!(cells.turns[cell], carrier_position(word, j));
            }
        }
        // dark petals and lit petals beyond the carriers stay unturned
        let carried: usize = (0..SECTORS)
            .map(|s| {
                carriers(&cells.petals, s)
                    .unwrap()
                    .iter()
                    .filter(|&&c| cells.turns[c] != 0)
                    .count()
            })
            .sum();
        let turned = cells.turns.iter().filter(|&&t| t != 0).count();
        assert_eq!(turned, carried);
        assert!(turned > SECTORS * TURN_CARRIERS * 6 / 10);
    }

    #[test]
    fn a_turned_petal_stays_out_of_its_neighbours_sampling_discs() {
        // the nearest petal centres (radially adjacent cells) are 11 units apart; a petal reaches its
        // half-length from its centre whichever way it is turned
        let mut nearest = f64::MAX;
        for list in neighbours() {
            for near in list {
                nearest = nearest.min(near.along.hypot(near.across));
            }
        }
        assert!((nearest - 11.0).abs() < 1e-9, "{nearest}");
        assert!(
            nearest > f64::from(PETAL_HALF_LENGTH + PETAL_RADIUS),
            "{nearest}"
        );
    }

    fn stream() -> crate::stream::StreamEncoder {
        let mut rng = Xorshift32::new(5);
        let data: Vec<u8> = (0..6_000).map(|_| rng.next_byte()).collect();
        crate::stream::StreamEncoder::new(&data, 2).unwrap()
    }

    fn rendered(cells: &FrameCells) -> crate::image::Rgb {
        crate::render::render(
            cells,
            &crate::render::RenderOptions {
                size: 768,
                supersample: 2,
                ..crate::render::RenderOptions::default()
            },
        )
    }

    #[test]
    fn a_clean_render_reads_every_turn_lane_and_skips_what_it_is_told() {
        let encoder = stream();
        let truth = encoder.lane_data(6);
        let image = rendered(&encoder.cells(6)).to_luma();
        let options = crate::decode::DecodeOptions::default();
        let decoded = crate::decode::decode(&image, &options).expect("decodes");
        assert_eq!(decoded.letters(), "D0123");
        let turns = read_turns(&image, &decoded, [false; SECTORS]);
        for (sector, result) in turns.iter().enumerate() {
            let result = result
                .as_ref()
                .unwrap_or_else(|| panic!("turn lane {sector}"));
            assert_eq!(result.data, truth.turns[sector]);
            assert_eq!(result.corrected, 0);
        }
        let turns = read_turns(&image, &decoded, [true, false, true, true]);
        assert_eq!(
            turns.iter().map(Option::is_some).collect::<Vec<_>>(),
            [false, true, false, false]
        );
        // a sector whose field lane did not decode has no turn lane
        let mut partial = decoded.clone();
        partial.fields[2] = None;
        let turns = read_turns(&image, &partial, [false; SECTORS]);
        assert!(turns[2].is_none() && turns[3].is_some());
    }

    #[test]
    fn a_modern_camera_reads_the_turn_lanes() {
        let encoder = stream();
        let truth = encoder.lane_data(9);
        let source = rendered(&encoder.cells(9));
        let options = crate::decode::DecodeOptions::default();
        for (seed, rotation) in [(1u64, 17.0), (2, 205.0)] {
            let config = crate::sim::fit_to_frame(
                &crate::sim::CaptureConfig {
                    rotation_deg: rotation,
                    tilt_x_deg: 8.0,
                    seed,
                    ..crate::sim::CaptureConfig::modern()
                },
                4.0,
            );
            let image = crate::sim::capture(&source, &config);
            let decoded = crate::decode::decode(&image, &options).expect("decodes");
            let turns = read_turns(&image, &decoded, [false; SECTORS]);
            for (sector, (turn, field)) in turns.iter().zip(&decoded.fields).enumerate() {
                if field.is_some() {
                    assert_eq!(
                        turn.as_ref().map(|r| &r.data),
                        Some(&truth.turns[sector]),
                        "seed {seed} sector {sector}"
                    );
                }
            }
        }
    }

    #[test]
    fn unturned_petals_never_pass_for_a_turn_lane() {
        let encoder = stream();
        let mut cells = encoder.cells(3);
        cells.turns = [0; FIELD_CELLS];
        let image = rendered(&cells).to_luma();
        let decoded = crate::decode::decode(&image, &crate::decode::DecodeOptions::default())
            .expect("the field lanes read without turns");
        assert_eq!(decoded.letters(), "D0123");
        assert!(
            read_turns(&image, &decoded, [false; SECTORS])
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn templates_hold_a_unit_petal_and_kernels_tell_the_turns_apart() {
        let tables = tables();
        let sharp = &tables.templates[0];
        assert!(sharp.at(0.0, 0.0) > 0.99);
        assert!(sharp.at(0.0, 6.0) < 0.05 && sharp.at(8.5, 0.0) < 0.01);
        assert!(sharp.at(4.5, 0.0) > sharp.at(0.0, 3.4));
        for kernels in &tables.kernels {
            for (k, kernel) in kernels.iter().enumerate() {
                // a window showing exactly the petal at turn k picks turn k
                let (position, margin) = decide(kernel, kernels);
                assert_eq!(usize::from(position), k);
                assert!(margin > 0.0);
            }
        }
    }
}
