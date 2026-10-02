//! The one pseudo-random generator shared by whitening and the fountain code.

/// Marsaglia xorshift32 with shifts 13, 17, 5.
///
/// The generator is part of the wire format: whitening sequences and fountain
/// masks are derived from it, so every implementation must match bit for bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    /// Creates a generator; a zero seed is replaced by `0xDEADBEEF` because
    /// xorshift cannot leave the all-zero state.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0xDEAD_BEEF } else { seed },
        }
    }

    /// Advances the generator and returns the next 32-bit word.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Returns the top byte of the next word.
    pub fn next_byte(&mut self) -> u8 {
        (self.next_u32() >> 24) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_sequence() {
        // First outputs of xorshift32 seeded with 1 (Marsaglia 2003).
        let mut rng = Xorshift32::new(1);
        assert_eq!(rng.next_u32(), 270_369);
        assert_eq!(rng.next_u32(), 67_634_689);
        assert_eq!(rng.next_u32(), 2_647_435_461);
    }

    #[test]
    fn zero_seed_is_remapped() {
        assert_eq!(Xorshift32::new(0), Xorshift32::new(0xDEAD_BEEF));
    }
}
