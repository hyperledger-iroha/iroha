//! Normative frame geometry.
//!
//! All coordinates are *design units* on a square canvas of [`CANVAS`] units
//! with the origin at the top-left and `y` growing downward. A renderer scales
//! the canvas to any pixel size; a decoder maps camera pixels back to design
//! units. Nothing here depends on floating-point rounding for bit placement:
//! cell *order* is defined by integer indices only, and the slot count of every
//! band is a fixed table.

use std::sync::OnceLock;

/// Canvas side length in design units.
pub const CANVAS: f32 = 1024.0;
/// Canvas centre coordinate on both axes.
pub const CENTER: f32 = 512.0;

/// Number of petal bands (concentric rings of field cells).
pub const BAND_COUNT: usize = 34;
/// Radius of band 0.
pub const BAND_INNER: f32 = 16.0;
/// Radial distance between consecutive bands.
pub const BAND_SPACING: f32 = 11.0;
/// Cells on each band, inner band first: `4 · round(2π r / 50)` for the band radius `r`, about
/// one cell every 12.5 units. Every count is a multiple of four, so the four sectors split each
/// band evenly and the field maps onto itself under quarter turns.
pub const BAND_SLOTS: [u16; BAND_COUNT] = [
    8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124,
    128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
];
/// Number of field cells.
pub const FIELD_CELLS: usize = 3376;
/// Angular sectors of the field; each is one lane codeword.
pub const SECTORS: usize = 4;
/// Cells in each sector.
pub const SECTOR_CELLS: usize = FIELD_CELLS / SECTORS;
/// Radius of the disc a petal stands in for: decoders sample within it, and no other petal,
/// however it is turned, reaches into it.
pub const PETAL_RADIUS: f32 = 4.6;
/// Half the length of a drawn petal, along its axis (the outward radial direction, turned by the
/// petal's turn, see [`crate::turns`]).
pub const PETAL_HALF_LENGTH: f32 = 6.0;
/// Half the width of a drawn petal.
pub const PETAL_HALF_WIDTH: f32 = 3.73;
/// Radius of the notch cut into each petal tip; it tells a petal from one turned by half a turn.
pub const PETAL_NOTCH_RADIUS: f32 = 1.12;
/// Distance from a petal centre to its notch centre, along its axis.
pub const PETAL_NOTCH_DISTANCE: f32 = 6.24;

const fn band_offsets() -> [usize; BAND_COUNT + 1] {
    let mut offsets = [0usize; BAND_COUNT + 1];
    let mut band = 0;
    while band < BAND_COUNT {
        assert!(
            BAND_SLOTS[band].is_multiple_of(4),
            "band slot counts are multiples of four"
        );
        offsets[band + 1] = offsets[band] + BAND_SLOTS[band] as usize;
        band += 1;
    }
    assert!(
        offsets[BAND_COUNT] == FIELD_CELLS,
        "the band table must hold every cell"
    );
    offsets
}

/// Index of the first cell of every band (and the total at the end).
pub static BAND_OFFSETS: [usize; BAND_COUNT + 1] = band_offsets();

/// Radius of band `band`.
#[must_use]
pub fn band_radius(band: usize) -> f32 {
    BAND_INNER + BAND_SPACING * band as f32
}

/// Fraction of a slot by which band `band` is turned: odd bands sit half a slot over, so
/// neighbouring bands interleave.
#[must_use]
pub fn band_phase(band: usize) -> f64 {
    if band % 2 == 1 { 0.5 } else { 0.0 }
}

/// Band and slot of field cell `cell`.
#[must_use]
pub fn cell_band_slot(cell: usize) -> (usize, usize) {
    let band = BAND_OFFSETS.partition_point(|&start| start <= cell) - 1;
    (band, cell - BAND_OFFSETS[band])
}

/// Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell `cell`.
#[must_use]
pub fn cell_angle(cell: usize) -> f64 {
    let (band, slot) = cell_band_slot(cell);
    core::f64::consts::TAU * (slot as f64 + band_phase(band)) / f64::from(BAND_SLOTS[band])
}

/// Canvas coordinates of the centre of field cell `cell`.
#[must_use]
pub fn cell_center(cell: usize) -> (f64, f64) {
    let (band, _) = cell_band_slot(cell);
    let radius = f64::from(band_radius(band));
    let angle = cell_angle(cell);
    (
        f64::from(CENTER) + radius * angle.cos(),
        f64::from(CENTER) + radius * angle.sin(),
    )
}

/// The cells of sector `sector` in bit order: band by band from the centre out, and within a
/// band in ascending slot order. Sector `s` holds slots `s·n/4 … (s+1)·n/4 − 1` of every band of
/// `n` slots. The first `8 ×` [`crate::lanes::FIELD_WORD`] cells carry the sector's codeword; the
/// rest are spare and stay dark.
///
/// # Panics
/// Panics when `sector` is not below [`SECTORS`].
#[must_use]
pub fn sector_cells(sector: usize) -> &'static [usize] {
    static TABLE: OnceLock<Vec<Vec<usize>>> = OnceLock::new();
    &TABLE.get_or_init(|| {
        (0..SECTORS)
            .map(|s| {
                let mut cells = Vec::with_capacity(SECTOR_CELLS);
                for band in 0..BAND_COUNT {
                    let quarter = usize::from(BAND_SLOTS[band]) / SECTORS;
                    for slot in s * quarter..(s + 1) * quarter {
                        cells.push(BAND_OFFSETS[band] + slot);
                    }
                }
                cells
            })
            .collect()
    })[sector]
}

/// Number of lattice columns and rows of the `天` mask.
pub const MASK_GRID: usize = 20;
/// Canvas coordinate of the mask lattice's left and top edge.
pub const MASK_ORIGIN: f32 = 182.0;
/// Mask lattice pitch in design units.
pub const MASK_PITCH: f32 = 33.0;
/// Radius beyond which the `天` backdrop is not drawn.
pub const BACKDROP_RADIUS: f32 = 390.0;

/// The `天` silhouette, one string per lattice row from top to bottom. `#` marks a lattice
/// cell inside the character. Petals whose centre lies in a `#` cell are drawn in the light
/// colour, the others in pink, and the `#` cells inside [`BACKDROP_RADIUS`] are drawn as a
/// faint backdrop, so the character shows through the petals.
pub const MASK: [&str; MASK_GRID] = [
    "....############....",
    "...##############...",
    "..################..",
    ".##################.",
    "###..............###",
    "###..............###",
    "#########..#########",
    "#########..#########",
    "###..............###",
    "###..............###",
    "########....########",
    "########....########",
    "#######......#######",
    "#######..##..#######",
    "######..####..######",
    "#####...####...#####",
    ".###...######...###.",
    "..###.########.###..",
    "......########......",
    ".....##########.....",
];

/// Whether the canvas point `(x, y)` lies inside the `天` mask.
#[must_use]
pub fn in_mask(x: f64, y: f64) -> bool {
    let (origin, pitch) = (f64::from(MASK_ORIGIN), f64::from(MASK_PITCH));
    if x < origin || y < origin {
        return false;
    }
    let (col, row) = (
        ((x - origin) / pitch) as usize,
        ((y - origin) / pitch) as usize,
    );
    col < MASK_GRID && row < MASK_GRID && MASK[row].as_bytes()[col] == b'#'
}

/// Whether field cell `cell` lies inside the `天` (drawn in the light colour).
#[must_use]
pub fn cell_light(cell: usize) -> bool {
    static LIGHT: OnceLock<Vec<bool>> = OnceLock::new();
    LIGHT.get_or_init(|| {
        (0..FIELD_CELLS)
            .map(|c| {
                let (x, y) = cell_center(c);
                in_mask(x, y)
            })
            .collect()
    })[cell]
}

/// Number of concentric dot rings.
pub const RING_COUNT: usize = 3;
/// Ring radii in design units.
pub const RING_RADII: [f32; RING_COUNT] = [405.0, 440.0, 475.0];
/// Dot slots on each ring (all multiples of four so the three cardinal gates
/// sit exactly on a slot).
pub const RING_SLOTS: [usize; RING_COUNT] = [88, 100, 108];
/// Radius of a drawn ring dot.
pub const DOT_RADIUS: f32 = 9.0;
/// Total dot slots over the three rings.
pub const TOTAL_SLOTS: usize = RING_SLOTS[0] + RING_SLOTS[1] + RING_SLOTS[2];

/// Dots in each cardinal gate, per ring, for the right, bottom and left
/// gates. There is deliberately no gate at the top.
pub const GATE_DOTS: [[usize; RING_COUNT]; 3] = [[1, 1, 3], [2, 2, 2], [1, 2, 2]];

/// What a ring slot is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotRole {
    /// A gate dot, always lit.
    Gate,
    /// A slot next to a gate, always dark.
    Guard,
    /// Carries data bit `n` of lane `D`.
    Data(u16),
    /// Unused data slot, always dark.
    Spare,
}

/// Number of lane `D` bits carried by the rings.
pub const D_BITS: usize = 240;

/// Offset of ring `ring` inside the flat slot index space.
#[must_use]
pub const fn ring_offset(ring: usize) -> usize {
    match ring {
        0 => 0,
        1 => RING_SLOTS[0],
        _ => RING_SLOTS[0] + RING_SLOTS[1],
    }
}

fn gate_slots(ring: usize) -> (Vec<usize>, Vec<usize>) {
    let n = RING_SLOTS[ring];
    let bases = [0, n / 4, n / 2]; // right, bottom, left
    let mut gates = Vec::new();
    let mut guards = Vec::new();
    for (gate, &base) in bases.iter().enumerate() {
        let count = GATE_DOTS[gate][ring];
        let (first, last) = match count {
            1 => (base, base),
            2 => (base, base + 1),
            _ => (base + n - 1, base + 1),
        };
        let span = (last + n - first) % n + 1;
        for step in 0..span {
            gates.push((first + step) % n);
        }
        guards.push((first + n - 1) % n);
        guards.push((last + 1) % n);
    }
    (gates, guards)
}

/// Computes the role of every ring slot in flat index order.
#[must_use]
pub fn slot_roles() -> Vec<SlotRole> {
    let mut roles = vec![SlotRole::Spare; TOTAL_SLOTS];
    for ring in 0..RING_COUNT {
        let (gates, guards) = gate_slots(ring);
        let offset = ring_offset(ring);
        for slot in guards {
            roles[offset + slot] = SlotRole::Guard;
        }
        for slot in gates {
            roles[offset + slot] = SlotRole::Gate;
        }
    }
    let mut next = 0u16;
    for role in &mut roles {
        if *role == SlotRole::Spare && usize::from(next) < D_BITS {
            *role = SlotRole::Data(next);
            next += 1;
        }
    }
    roles
}

/// Flat slot index of every lane `D` bit, in bit order.
#[must_use]
pub fn data_slots() -> Vec<usize> {
    let mut slots = vec![0usize; D_BITS];
    for (index, role) in slot_roles().into_iter().enumerate() {
        if let SlotRole::Data(bit) = role {
            slots[usize::from(bit)] = index;
        }
    }
    slots
}

/// Splits a flat slot index into `(ring, slot)`.
#[must_use]
pub fn split_slot(flat: usize) -> (usize, usize) {
    if flat < RING_SLOTS[0] {
        (0, flat)
    } else if flat < RING_SLOTS[0] + RING_SLOTS[1] {
        (1, flat - RING_SLOTS[0])
    } else {
        (2, flat - RING_SLOTS[0] - RING_SLOTS[1])
    }
}

/// Canvas coordinates of the centre of ring slot `slot` on ring `ring`.
///
/// Slot `0` is at 3 o'clock and slots advance clockwise on the screen.
#[must_use]
pub fn slot_center(ring: usize, slot: usize) -> (f32, f32) {
    let theta = core::f32::consts::TAU * slot as f32 / RING_SLOTS[ring] as f32;
    (
        CENTER + RING_RADII[ring] * theta.cos(),
        CENTER + RING_RADII[ring] * theta.sin(),
    )
}

/// Canvas coordinates of the four corner finders, clockwise from top-left.
pub const FINDER_CENTERS: [(f32, f32); 4] =
    [(72.0, 72.0), (952.0, 72.0), (952.0, 952.0), (72.0, 952.0)];
/// Radius of the finder's solid centre disc.
pub const FINDER_CORE: f32 = 12.0;
/// Number of petals of a finder blossom.
pub const FINDER_PETALS: usize = 5;
/// Distance from the finder centre to each petal centre.
pub const FINDER_PETAL_DISTANCE: f32 = 34.0;
/// Radius of each petal.
pub const FINDER_PETAL_RADIUS: f32 = 26.0;
/// Radius of the notch cut into each petal tip.
pub const FINDER_NOTCH_RADIUS: f32 = 6.0;
/// Outer radius of a finder blossom (tip of a petal).
pub const FINDER_OUTER: f32 = 60.0;

/// Returns whether the point `(dx, dy)`, relative to a finder centre, is lit.
///
/// A finder is a solid five-petal sakura blossom whose first petal points
/// straight up. The petal notches are cosmetic; decoders only rely on the
/// blossom being one large, isolated, roughly round blob.
#[must_use]
pub fn finder_lit(dx: f64, dy: f64) -> bool {
    use core::f64::consts::{FRAC_PI_2, TAU};
    if (dx * dx + dy * dy).sqrt() <= f64::from(FINDER_CORE) {
        return true;
    }
    let (distance, radius, notch) = (
        f64::from(FINDER_PETAL_DISTANCE),
        f64::from(FINDER_PETAL_RADIUS),
        f64::from(FINDER_NOTCH_RADIUS),
    );
    for petal in 0..FINDER_PETALS {
        let angle = -FRAC_PI_2 + TAU * petal as f64 / FINDER_PETALS as f64;
        let (cx, cy) = (distance * angle.cos(), distance * angle.sin());
        if ((dx - cx).powi(2) + (dy - cy).powi(2)).sqrt() <= radius {
            let (nx, ny) = (
                f64::from(FINDER_OUTER) * angle.cos(),
                f64::from(FINDER_OUTER) * angle.sin(),
            );
            return ((dx - nx).powi(2) + (dy - ny).powi(2)).sqrt() > notch;
        }
    }
    false
}

/// Returns whether the point `(dx, dy)`, relative to the centre of a petal that points along
/// `angle`, is lit: an ellipse of half-length [`PETAL_HALF_LENGTH`] along `angle` and half-width
/// [`PETAL_HALF_WIDTH`], minus the notch at its tip.
#[must_use]
pub fn petal_lit(dx: f64, dy: f64, angle: f64) -> bool {
    let (sin, cos) = angle.sin_cos();
    let along = dx * cos + dy * sin;
    let across = -dx * sin + dy * cos;
    let (a, b) = (f64::from(PETAL_HALF_LENGTH), f64::from(PETAL_HALF_WIDTH));
    if (along / a).powi(2) + (across / b).powi(2) > 1.0 {
        return false;
    }
    let notch = f64::from(PETAL_NOTCH_DISTANCE);
    (along - notch).powi(2) + across.powi(2) > f64::from(PETAL_NOTCH_RADIUS).powi(2)
}

// ---- the card: the square and a band of katakana tiles above and below it ----------------------

/// Side of a katakana tile ([`crate::kana`]).
pub const KANA_TILE: f32 = 25.0;
/// Pitch of the katakana tile lattice.
pub const KANA_PITCH: f32 = 29.0;
/// Side of a tile's glyph box, centred in the tile.
pub const KANA_GLYPH_BOX: f32 = 23.0;
/// Corner radius of a tile.
pub const KANA_CORNER: f32 = 3.0;
/// Tile rows of each band.
pub const KANA_ROWS: usize = 10;
/// Tile columns of each band.
pub const KANA_COLUMNS: usize = 35;
/// Gap between a band's lattice and the square.
pub const KANA_GAP: f32 = 24.0;
/// Margin outside a band's lattice.
pub const KANA_MARGIN: f32 = 8.0;
/// Height of a band with its gap and margin (322 units).
pub const KANA_BAND_HEIGHT: f32 = KANA_MARGIN + KANA_ROWS as f32 * KANA_PITCH + KANA_GAP;
/// Height of the card: the square with a band above and below it (1668 units).
pub const CARD_HEIGHT: f32 = CANVAS + 2.0 * KANA_BAND_HEIGHT;
/// Card `y` of the square's top edge: card coordinates are square coordinates plus this in `y`.
pub const SQUARE_TOP: f32 = KANA_BAND_HEIGHT;
/// Tile centres keep at least this distance from every finder centre: the blossom's radius, a
/// blossom diameter of clearance, and a tile's half-diagonal, so no tile looks like part of a
/// finder.
pub const KANA_FINDER_CLEARANCE: f32 = 198.0;
/// Bands: above the square and below it.
pub const KANA_BANDS: usize = 2;
/// Pilot tiles of each band.
pub const KANA_PILOTS: usize = 8;

/// The pilot tiles of each band as `(row, column)` (band 1 mirrors band 0's rows, so the pilots
/// sit at the same distances from the square).
pub const KANA_PILOT_CELLS: [[(u8, u8); KANA_PILOTS]; KANA_BANDS] = [
    [
        (0, 0),
        (0, 17),
        (0, 34),
        (4, 0),
        (4, 34),
        (9, 8),
        (9, 17),
        (9, 26),
    ],
    [
        (9, 0),
        (9, 17),
        (9, 34),
        (5, 0),
        (5, 34),
        (0, 8),
        (0, 17),
        (0, 26),
    ],
];
/// What every pilot shows: `(glyph, light)` in pilot order (ロ ケ ヒ ム ス ワ ア カ).
pub const KANA_PILOT_SYMBOLS: [(u8, bool); KANA_PILOTS] = [
    (1, true),
    (11, false),
    (13, true),
    (9, false),
    (14, false),
    (6, true),
    (12, false),
    (7, true),
];

/// One katakana tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KanaTile {
    /// Band: 0 above the square, 1 below it.
    pub band: u8,
    /// Row within its band, top first.
    pub row: u8,
    /// Column, left first.
    pub column: u8,
    /// Left edge of the tile square in square coordinates.
    pub x: f64,
    /// Top edge of the tile square in square coordinates.
    pub y: f64,
    /// Index among the pilots of its band, when it is one.
    pub pilot: Option<u8>,
}

impl KanaTile {
    /// Centre of the tile in square coordinates.
    #[must_use]
    pub fn centre(&self) -> (f64, f64) {
        let half = f64::from(KANA_TILE) / 2.0;
        (self.x + half, self.y + half)
    }
}

/// Top of a band's lattice in square coordinates.
fn kana_lattice_top(band: u8) -> f64 {
    if band == 0 {
        -f64::from(KANA_BAND_HEIGHT) + f64::from(KANA_MARGIN)
    } else {
        f64::from(CANVAS) + f64::from(KANA_GAP)
    }
}

/// Left edge of the lattice (the columns are centred on the card).
fn kana_lattice_left() -> f64 {
    (f64::from(CANVAS) - KANA_COLUMNS as f64 * f64::from(KANA_PITCH)) / 2.0
}

/// Every tile of both bands: band 0 first, each in reading order (rows top first, columns left
/// first). A lattice cell whose tile centre lies within [`KANA_FINDER_CLEARANCE`] of a finder
/// centre has no tile (306 tiles per band remain).
#[must_use]
pub fn kana_tiles() -> &'static [KanaTile] {
    static TILES: OnceLock<Vec<KanaTile>> = OnceLock::new();
    TILES.get_or_init(|| {
        let inset = f64::from(KANA_PITCH - KANA_TILE) / 2.0;
        let clearance = f64::from(KANA_FINDER_CLEARANCE);
        let mut tiles = Vec::new();
        for band in 0..KANA_BANDS as u8 {
            for row in 0..KANA_ROWS as u8 {
                for column in 0..KANA_COLUMNS as u8 {
                    let x = kana_lattice_left() + f64::from(KANA_PITCH) * f64::from(column) + inset;
                    let y = kana_lattice_top(band) + f64::from(KANA_PITCH) * f64::from(row) + inset;
                    let half = f64::from(KANA_TILE) / 2.0;
                    let (cx, cy) = (x + half, y + half);
                    let clear = FINDER_CENTERS.iter().all(|&(fx, fy)| {
                        let (dx, dy) = (cx - f64::from(fx), cy - f64::from(fy));
                        dx * dx + dy * dy >= clearance * clearance
                    });
                    if !clear {
                        continue;
                    }
                    let pilot = KANA_PILOT_CELLS[usize::from(band)]
                        .iter()
                        .position(|&cell| cell == (row, column))
                        .map(|p| p as u8);
                    tiles.push(KanaTile {
                        band,
                        row,
                        column,
                        x,
                        y,
                        pilot,
                    });
                }
            }
        }
        tiles
    })
}

/// The data tiles of `band` (its tiles that are not pilots) in reading order, as indices into
/// [`kana_tiles`].
///
/// # Panics
/// Panics when `band` is not below [`KANA_BANDS`].
#[must_use]
pub fn kana_data_tiles(band: u8) -> &'static [usize] {
    static DATA: OnceLock<[Vec<usize>; KANA_BANDS]> = OnceLock::new();
    &DATA.get_or_init(|| {
        core::array::from_fn(|b| {
            kana_tiles()
                .iter()
                .enumerate()
                .filter(|(_, t)| usize::from(t.band) == b && t.pilot.is_none())
                .map(|(i, _)| i)
                .collect()
        })
    })[usize::from(band)]
}

/// The pilot tiles of `band` in pilot order, as indices into [`kana_tiles`].
///
/// # Panics
/// Panics when `band` is not below [`KANA_BANDS`].
#[must_use]
pub fn kana_pilot_tiles(band: u8) -> [usize; KANA_PILOTS] {
    static PILOTS: OnceLock<[[usize; KANA_PILOTS]; KANA_BANDS]> = OnceLock::new();
    PILOTS.get_or_init(|| {
        core::array::from_fn(|b| {
            core::array::from_fn(|p| {
                kana_tiles()
                    .iter()
                    .position(|t| usize::from(t.band) == b && t.pilot == Some(p as u8))
                    .expect("every pilot cell holds a tile")
            })
        })
    })[usize::from(band)]
}

/// The tile under the square-coordinate point `(x, y)` and the point's position inside the
/// tile's square (`0..=`[`KANA_TILE`] on both axes), or `None` outside every tile square.
#[must_use]
pub fn kana_tile_at(x: f64, y: f64) -> Option<(usize, f64, f64)> {
    static INDEX: OnceLock<Vec<Option<usize>>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let mut index = vec![None; KANA_BANDS * KANA_ROWS * KANA_COLUMNS];
        for (i, tile) in kana_tiles().iter().enumerate() {
            index[(usize::from(tile.band) * KANA_ROWS + usize::from(tile.row)) * KANA_COLUMNS
                + usize::from(tile.column)] = Some(i);
        }
        index
    });
    let band: u8 = if y < 0.0 {
        0
    } else if y >= f64::from(CANVAS) {
        1
    } else {
        return None;
    };
    let pitch = f64::from(KANA_PITCH);
    let (gx, gy) = (
        (x - kana_lattice_left()) / pitch,
        (y - kana_lattice_top(band)) / pitch,
    );
    if gx < 0.0 || gy < 0.0 {
        return None;
    }
    let (column, row) = (gx.floor() as usize, gy.floor() as usize);
    if column >= KANA_COLUMNS || row >= KANA_ROWS {
        return None;
    }
    let tile = index[(usize::from(band) * KANA_ROWS + row) * KANA_COLUMNS + column]?;
    let inset = f64::from(KANA_PITCH - KANA_TILE) / 2.0;
    let (u, v) = (
        (gx - column as f64) * pitch - inset,
        (gy - row as f64) * pitch - inset,
    );
    let side = f64::from(KANA_TILE);
    ((0.0..=side).contains(&u) && (0.0..=side).contains(&v)).then_some((tile, u, v))
}

/// Whether the tile-local point `(u, v)` lies inside the rounded tile square.
#[must_use]
pub fn kana_inside(u: f64, v: f64) -> bool {
    let (side, corner) = (f64::from(KANA_TILE), f64::from(KANA_CORNER));
    if !(0.0..=side).contains(&u) || !(0.0..=side).contains(&v) {
        return false;
    }
    let cx = u.clamp(corner, side - corner);
    let cy = v.clamp(corner, side - corner);
    (u - cx).powi(2) + (v - cy).powi(2) <= corner * corner
}

/// Whether the tile-local point `(u, v)` lies on `glyph`'s strokes: inside the tile square and
/// inked in the glyph box (inset by half the difference of tile and box).
#[must_use]
pub fn kana_inked(glyph: u8, u: f64, v: f64) -> bool {
    let inset = f64::from(KANA_TILE - KANA_GLYPH_BOX) / 2.0;
    let scale = f64::from(crate::glyphs::GLYPH_GRID) / f64::from(KANA_GLYPH_BOX);
    kana_inside(u, v)
        && crate::glyphs::is_inked(usize::from(glyph), (u - inset) * scale, (v - inset) * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_holds_306_tiles_per_band_and_eight_pilots_each() {
        assert!((KANA_BAND_HEIGHT - 322.0).abs() < f32::EPSILON);
        assert!((CARD_HEIGHT - 1668.0).abs() < f32::EPSILON);
        let tiles = kana_tiles();
        for band in 0..KANA_BANDS as u8 {
            let in_band: Vec<_> = tiles.iter().filter(|t| t.band == band).collect();
            assert_eq!(in_band.len(), 306, "band {band}");
            assert_eq!(
                in_band.iter().filter(|t| t.pilot.is_some()).count(),
                KANA_PILOTS
            );
            assert_eq!(kana_data_tiles(band).len(), 306 - KANA_PILOTS);
            for (p, &tile) in kana_pilot_tiles(band).iter().enumerate() {
                assert_eq!(tiles[tile].pilot, Some(p as u8));
            }
        }
        // the bands mirror each other about the square's middle
        for tile in tiles.iter().filter(|t| t.band == 0) {
            let mirror = tiles
                .iter()
                .find(|t| {
                    t.band == 1
                        && t.column == tile.column
                        && usize::from(t.row) == KANA_ROWS - 1 - usize::from(tile.row)
                })
                .expect("mirrored tile");
            assert!((mirror.y + f64::from(KANA_TILE) - f64::from(CANVAS) + tile.y).abs() < 1e-9);
            assert_eq!(mirror.pilot, tile.pilot);
        }
        // every tile centre maps back to its tile, and the square holds none
        for (i, tile) in tiles.iter().enumerate() {
            let (cx, cy) = tile.centre();
            assert_eq!(kana_tile_at(cx, cy).map(|t| t.0), Some(i));
        }
        assert!(kana_tile_at(512.0, 512.0).is_none());
    }

    #[test]
    fn mask_is_mirror_symmetric_and_shows_which_way_is_up() {
        for row in MASK {
            assert_eq!(row.len(), MASK_GRID);
            let bytes = row.as_bytes();
            for col in 0..MASK_GRID / 2 {
                assert_eq!(
                    bytes[col],
                    bytes[MASK_GRID - 1 - col],
                    "row {row} not mirrored"
                );
            }
        }
        assert_ne!(MASK[0], MASK[MASK_GRID - 1]);
    }

    #[test]
    fn bands_hold_every_cell_and_split_into_equal_sectors() {
        assert_eq!(BAND_OFFSETS[BAND_COUNT], FIELD_CELLS);
        let mut seen = vec![false; FIELD_CELLS];
        for sector in 0..SECTORS {
            let cells = sector_cells(sector);
            assert_eq!(cells.len(), SECTOR_CELLS);
            for &cell in cells {
                assert!(!seen[cell], "cell {cell} in two sectors");
                seen[cell] = true;
                // a cell sits in its own quarter turn (odd bands lean half a slot forward)
                let (band, slot) = cell_band_slot(cell);
                let quarter = usize::from(BAND_SLOTS[band]) / SECTORS;
                assert_eq!(slot / quarter, sector);
                let turn = cell_angle(cell) / core::f64::consts::TAU * SECTORS as f64;
                assert!(turn > sector as f64 - 1e-9 && turn < sector as f64 + 1.0);
            }
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn band_slots_follow_the_pitch_rule() {
        for (band, &slots) in BAND_SLOTS.iter().enumerate() {
            let raw = core::f64::consts::TAU * f64::from(band_radius(band)) / 50.0;
            assert_eq!(usize::from(slots), 4 * (raw.round() as usize).max(1));
        }
    }

    #[test]
    fn the_field_maps_onto_itself_under_a_quarter_turn() {
        let c = f64::from(CENTER);
        for cell in 0..FIELD_CELLS {
            let (band, slot) = cell_band_slot(cell);
            let n = usize::from(BAND_SLOTS[band]);
            let turned = BAND_OFFSETS[band] + (slot + n / 4) % n;
            let (x, y) = cell_center(cell);
            let (tx, ty) = cell_center(turned);
            // a quarter turn clockwise on screen about the centre
            assert!((tx - (c - (y - c))).abs() < 1e-9 && (ty - (c + (x - c))).abs() < 1e-9);
        }
    }

    #[test]
    fn petals_stay_clear_of_their_neighbours_and_the_rings() {
        // a petal reaches at most its half-length from its centre, whichever way it is turned
        let outer = band_radius(BAND_COUNT - 1) + PETAL_HALF_LENGTH;
        assert!(outer + 10.0 < RING_RADII[0] - DOT_RADIUS);
        for cell in 0..FIELD_CELLS {
            let (x, y) = cell_center(cell);
            let (band, _) = cell_band_slot(cell);
            let near =
                BAND_OFFSETS[band.saturating_sub(1)]..BAND_OFFSETS[(band + 2).min(BAND_COUNT)];
            for other in near {
                if other == cell {
                    continue;
                }
                let (ox, oy) = cell_center(other);
                let distance = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();
                assert!(
                    distance > f64::from(PETAL_HALF_LENGTH + PETAL_RADIUS),
                    "cells {cell} and {other} are {distance} apart"
                );
            }
        }
    }

    #[test]
    fn the_light_cells_draw_the_character() {
        let light = (0..FIELD_CELLS).filter(|&c| cell_light(c)).count();
        assert!((1200..2400).contains(&light), "{light} light cells");
        assert!(in_mask(512.0, 190.0), "the top bar");
        assert!(!in_mask(512.0, 350.0), "the gap under the second bar");
    }

    #[test]
    fn ring_slots_provide_exactly_the_lane_d_capacity() {
        let roles = slot_roles();
        let data = roles
            .iter()
            .filter(|r| matches!(r, SlotRole::Data(_)))
            .count();
        let gates = roles.iter().filter(|r| **r == SlotRole::Gate).count();
        assert_eq!(data, D_BITS);
        assert_eq!(gates, 4 + 5 + 7);
        assert_eq!(data_slots().len(), D_BITS);
        assert_eq!(
            roles.iter().filter(|r| **r == SlotRole::Spare).count(),
            TOTAL_SLOTS - D_BITS - 16 - 18
        );
    }

    #[test]
    fn gates_never_touch_the_top() {
        for (ring, &n) in RING_SLOTS.iter().enumerate() {
            let (gates, guards) = gate_slots(ring);
            for slot in gates.into_iter().chain(guards) {
                assert!(
                    slot.abs_diff(3 * n / 4) > 2,
                    "ring {ring} slot {slot} near the top"
                );
            }
        }
    }

    #[test]
    fn finders_and_rings_do_not_overlap() {
        let outermost = RING_RADII[2] + DOT_RADIUS;
        for &(x, y) in &FINDER_CENTERS {
            let distance = ((x - CENTER).powi(2) + (y - CENTER).powi(2)).sqrt();
            assert!(distance - FINDER_OUTER > outermost + 20.0);
        }
    }

    #[test]
    fn a_petal_is_lit_at_its_centre_and_notched_at_its_tip() {
        assert!(petal_lit(0.0, 0.0, 0.3));
        let tip = f64::from(PETAL_NOTCH_DISTANCE);
        assert!(!petal_lit(tip * 0.3f64.cos(), tip * 0.3f64.sin(), 0.3));
        assert!(!petal_lit(0.0, 10.0, 0.0));
    }
}
