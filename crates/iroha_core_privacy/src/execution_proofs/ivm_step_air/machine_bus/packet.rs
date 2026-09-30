//! Canonical typed state packets; no initializer or access authority is conferred.

use super::F;

pub(super) const WIDTH: usize = 26;
pub(super) const SPACE: usize = 0;
pub(super) const VM: usize = 1;
pub(super) const GENERATION: usize = 2;
pub(super) const INDEX: usize = 3;
pub(super) const KEY: usize = 4;
pub(super) const CLOCK: usize = 5;
pub(super) const ENABLED: usize = 6;
pub(super) const WRITE: usize = 7;
pub(super) const BEFORE: usize = 8;
pub(super) const AFTER: usize = 16;
pub(super) const BEFORE_TAG: usize = 24;
pub(super) const AFTER_TAG: usize = 25;

/// Disjoint internal cell classes. These are not wire type identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Space {
    /// A physical 16-byte memory cell and its sixteen private-byte bits.
    Memory = 1,
    /// One 64-bit register and its single private tag.
    Register = 2,
    /// Sixteen initialization bits belonging to one frame generation.
    Initialization = 3,
    /// One 64-bit protected owner/control value.
    Owner = 4,
}

/// Public qualification input, not an authorized machine request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Event {
    /// Disjoint cell class.
    pub(super) space: Space,
    /// Machine identity within the eventual invocation relation.
    pub(super) vm: u8,
    /// Initialization or owner generation; memory and register cells use zero.
    pub(super) generation: u16,
    /// Cell index within the class and machine.
    pub(super) index: u32,
    /// Whether this event may replace the cell value and private mask.
    pub(super) write: bool,
    /// Exact sixteen bytes read before this event.
    pub(super) before: [u8; 16],
    /// Exact sixteen bytes after this event.
    pub(super) after: [u8; 16],
    /// Private-byte mask before the event; typed classes restrict unused bits.
    pub(super) before_private: u16,
    /// Private-byte mask after the event; typed classes restrict unused bits.
    pub(super) after_private: u16,
}

impl Event {
    pub(super) fn fields(&self, slot: usize) -> [F; WIDTH] {
        let mut row = [F::ZERO; WIDTH];
        row[SPACE] = F(self.space as u64);
        row[VM] = F(u64::from(self.vm));
        row[GENERATION] = F(u64::from(self.generation));
        row[INDEX] = F(u64::from(self.index));
        row[KEY] = F(u64::from(self.index)
            + (u64::from(self.generation) << 32)
            + (u64::from(self.vm) << 48)
            + ((self.space as u64) << 56));
        row[CLOCK] = F(slot as u64);
        row[ENABLED] = F::ONE;
        row[WRITE] = F(u64::from(self.write));
        for (offset, bytes) in [(BEFORE, &self.before), (AFTER, &self.after)] {
            for (limb, pair) in bytes.chunks_exact(2).enumerate() {
                row[offset + limb] = F(u64::from(u16::from_le_bytes([pair[0], pair[1]])));
            }
        }
        row[BEFORE_TAG] = F(u64::from(self.before_private));
        row[AFTER_TAG] = F(u64::from(self.after_private));
        row
    }
}

pub(super) fn half(row: &[F], offset: usize, half: usize) -> u64 {
    (0..4).fold(0, |word, limb| {
        word | (row[offset + half * 4 + limb].0 << (16 * limb))
    })
}
