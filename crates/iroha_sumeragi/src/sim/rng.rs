//! The simulator's single source of randomness: `SplitMix64` (Steele, Lea, Flood 2014), seeded
//! once per run. Probabilities are integers in parts per million; no floats anywhere.

/// One million: the denominator of every probability (`ppm`).
pub const PPM: u32 = 1_000_000;

/// A deterministic `SplitMix64` generator.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`0` for `n = 0`), without modulo bias.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let x = self.next_u64();
            if x < zone {
                return x % n;
            }
        }
    }

    /// Uniform `usize` in `0..n`.
    pub fn index(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from(self.below(n64)).unwrap_or(0)
    }

    /// Uniform in `lo..=hi` (`lo` if `hi < lo`).
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.below(hi - lo + 1)
    }

    /// `true` with probability `ppm / 1_000_000`.
    pub fn chance(&mut self, ppm: u32) -> bool {
        ppm > 0 && self.below(u64::from(PPM)) < u64::from(ppm)
    }

    /// A random element of `items`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.index(items.len()))
        }
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }

    /// An independent generator derived from this one (for per-component streams).
    #[must_use]
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_u64() ^ 0xd1b5_4a32_d192_ed03)
    }
}

/// A stable 64-bit mix of a string and a number (scenario name + seed → world seed).
pub fn seed_of(name: &str, seed: u64) -> u64 {
    let mut rng = Rng::new(seed);
    for byte in name.bytes() {
        rng.state ^= u64::from(byte);
        rng.next_u64();
    }
    rng.next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reference_values() {
        // Reference outputs of SplitMix64 seeded with 0.
        let mut rng = Rng::new(0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(rng.next_u64(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn ranges_and_determinism() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1_000 {
            let x = a.range(3, 9);
            assert_eq!(x, b.range(3, 9));
            assert!((3..=9).contains(&x));
            assert!(a.below(7) < 7);
            b.below(7);
        }
        assert_eq!(a.below(0), 0);
        assert_eq!(a.range(5, 5), 5);
        assert!(!a.chance(0));
        assert!(a.chance(PPM));
        let mut v: Vec<u32> = (0..20).collect();
        a.shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
        assert_ne!(seed_of("F1", 1), seed_of("F2", 1));
        assert_ne!(seed_of("F1", 1), seed_of("F1", 2));
    }
}
