//! Normative frame geometry.
//!
//! All coordinates are *design units* on a square canvas of [`CANVAS`] units
//! with the origin at the top-left and `y` growing downward. A renderer scales
//! the canvas to any pixel size; a decoder maps camera pixels back to design
//! units. Nothing here depends on floating-point rounding for bit placement:
//! cell *order* is defined by integer indices only.

/// Canvas side length in design units.
pub const CANVAS: f32 = 1024.0;
/// Canvas centre coordinate on both axes.
pub const CENTER: f32 = 512.0;

/// Number of tile lattice columns and rows.
pub const TILE_GRID: usize = 20;
/// Canvas coordinate of the lattice's left and top edge.
pub const TILE_ORIGIN: f32 = 222.0;
/// Lattice pitch in design units.
pub const TILE_PITCH: f32 = 29.0;
/// Side of a drawn tile in design units (pitch minus a 4-unit gutter).
pub const TILE_SIZE: f32 = 25.0;
/// Side of the square glyph box inside a tile.
pub const GLYPH_BOX: f32 = 23.0;
/// Number of data tiles in the `天` mask.
pub const TILE_COUNT: usize = 256;

/// The `天` silhouette, one string per lattice row from top to bottom.
/// `#` marks a data tile. The mask is mirror-symmetric left to right and not
/// symmetric top to bottom, so it also tells a decoder which way is up.
pub const MASK: [&str; TILE_GRID] = [
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

const fn build_tiles() -> [(u8, u8); TILE_COUNT] {
    let mut tiles = [(0u8, 0u8); TILE_COUNT];
    let mut count = 0;
    let mut row = 0;
    while row < TILE_GRID {
        let bytes = MASK[row].as_bytes();
        let mut col = 0;
        while col < TILE_GRID {
            if bytes[col] == b'#' {
                tiles[count] = (col as u8, row as u8);
                count += 1;
            }
            col += 1;
        }
        row += 1;
    }
    assert!(count == TILE_COUNT, "mask must contain exactly 256 tiles");
    tiles
}

/// Lattice `(column, row)` of every data tile in row-major order.
pub static TILES: [(u8, u8); TILE_COUNT] = build_tiles();

/// Canvas coordinates of the centre of tile `index`.
#[must_use]
pub fn tile_center(index: usize) -> (f32, f32) {
    let (col, row) = TILES[index];
    (
        TILE_ORIGIN + TILE_PITCH * (f32::from(col) + 0.5),
        TILE_ORIGIN + TILE_PITCH * (f32::from(row) + 0.5),
    )
}

/// Number of concentric dot rings.
pub const RING_COUNT: usize = 3;
/// Ring radii in design units.
pub const RING_RADII: [f32; RING_COUNT] = [360.0, 410.0, 460.0];
/// Dot slots on each ring (all multiples of four so the three cardinal gates
/// sit exactly on a slot).
pub const RING_SLOTS: [usize; RING_COUNT] = [80, 92, 104];
/// Radius of a drawn ring dot.
pub const DOT_RADIUS: f32 = 11.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_has_256_symmetric_tiles() {
        assert_eq!(TILES.len(), 256);
        for row in MASK {
            assert_eq!(row.len(), TILE_GRID);
            let bytes = row.as_bytes();
            for col in 0..TILE_GRID / 2 {
                assert_eq!(
                    bytes[col],
                    bytes[TILE_GRID - 1 - col],
                    "row {row} not mirrored"
                );
            }
        }
        assert_ne!(
            MASK[0],
            MASK[TILE_GRID - 1],
            "mask must show top from bottom"
        );
    }

    #[test]
    fn tiles_are_row_major_and_inside_the_canvas() {
        let mut previous = (0u8, 0u8);
        for (index, &(col, row)) in TILES.iter().enumerate() {
            if index > 0 {
                assert!((row, col) > (previous.1, previous.0));
            }
            previous = (col, row);
            let (x, y) = tile_center(index);
            assert!((0.0..CANVAS).contains(&x) && (0.0..CANVAS).contains(&y));
        }
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
        // the two spare slots are the last non-reserved slots of the outer ring
        assert_eq!(roles.iter().filter(|r| **r == SlotRole::Spare).count(), 2);
    }

    #[test]
    fn gates_never_touch_the_top() {
        for (ring, &n) in RING_SLOTS.iter().enumerate() {
            let (gates, guards) = gate_slots(ring);
            for slot in gates.into_iter().chain(guards) {
                let top = 3 * n / 4;
                assert!(
                    slot.abs_diff(top) > 2,
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
        let farthest_corner = (0..TILE_COUNT)
            .flat_map(|i| {
                let (x, y) = tile_center(i);
                let h = TILE_SIZE / 2.0;
                [
                    (x - h, y - h),
                    (x + h, y - h),
                    (x - h, y + h),
                    (x + h, y + h),
                ]
            })
            .map(|(x, y)| ((x - CENTER).powi(2) + (y - CENTER).powi(2)).sqrt())
            .fold(0.0f32, f32::max);
        assert!(
            farthest_corner + 10.0 < RING_RADII[0] - DOT_RADIUS,
            "tile corners come within {} units of the inner ring",
            RING_RADII[0] - DOT_RADIUS - farthest_corner
        );
    }
}
