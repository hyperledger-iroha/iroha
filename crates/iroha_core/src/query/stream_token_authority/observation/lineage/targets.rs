//! Fixed storage for the five role-11 proof coordinates; equal heights retain every predicate.

use super::Error;

pub(super) const RESERVE: u8 = 1;
pub(super) const TERMINAL: u8 = 2;
pub(super) const FLOOR: u8 = 4;
pub(super) const CHECK: u8 = 8;
pub(super) const TIP: u8 = 16;

#[derive(Clone, Copy, Default)]
struct Target {
    height: u64,
    kinds: u8,
}

pub(super) struct Targets {
    targets: [Target; 5],
    len: usize,
    cursor: usize,
    next_height: Option<u64>,
    expected: u8,
    seen: u8,
    failed: bool,
}
impl Targets {
    pub(super) fn new(input: [Option<(u64, u8)>; 5]) -> Result<Self, Error> {
        let mut this = Self {
            targets: [Target::default(); 5],
            len: 0,
            cursor: 0,
            next_height: None,
            expected: 0,
            seen: 0,
            failed: false,
        };
        for (height, kind) in input.into_iter().flatten() {
            if height == 0 || !kind.is_power_of_two() || kind > TIP || this.expected & kind != 0 {
                return Err(Error::Finality);
            }
            this.expected |= kind;
            let offset = this.targets[..this.len].partition_point(|target| target.height < height);
            if offset < this.len && this.targets[offset].height == height {
                this.targets[offset].kinds |= kind;
            } else {
                this.targets.copy_within(offset..this.len, offset + 1);
                this.targets[offset] = Target {
                    height,
                    kinds: kind,
                };
                this.len += 1;
            }
        }
        if this.len == 0 {
            return Err(Error::Finality);
        }
        this.next_height = Some(this.start());
        Ok(this)
    }
    pub(super) fn start(&self) -> u64 {
        self.targets[0].height
    }
    pub(super) fn end(&self) -> u64 {
        self.targets[self.len - 1].height
    }
    pub(super) fn consume(&mut self, height: u64) -> Result<(), Error> {
        if self.failed || self.next_height != Some(height) {
            self.failed = true;
            return Err(Error::Finality);
        }
        if self.cursor < self.len && height == self.targets[self.cursor].height {
            self.seen |= self.targets[self.cursor].kinds;
            self.cursor += 1;
        }
        self.next_height = if height == self.end() {
            None
        } else {
            height.checked_add(1)
        };
        Ok(())
    }
    pub(super) fn finish(self) -> Result<(), Error> {
        if self.failed
            || self.next_height.is_some()
            || self.cursor != self.len
            || self.seen != self.expected
        {
            Err(Error::Finality)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sorted_coalesced_targets_retain_all_five_checks() {
        let mut targets = Targets::new([
            Some((5, TIP)),
            Some((3, RESERVE)),
            Some((4, TERMINAL)),
            Some((4, FLOOR)),
            Some((5, CHECK)),
        ])
        .unwrap();
        assert_eq!(targets.len, 3);
        assert_eq!((targets.start(), targets.end()), (3, 5));
        for height in 3..=5 {
            targets.consume(height).unwrap();
        }
        assert_eq!(targets.seen, 31);
        targets.finish().unwrap();
    }
    #[test]
    fn zero_duplicate_kind_omitted_and_repeated_heights_refuse() {
        assert!(Targets::new([None; 5]).is_err());
        assert!(Targets::new([Some((0, FLOOR)), None, None, None, None]).is_err());
        assert!(Targets::new([Some((1, FLOOR)), Some((2, FLOOR)), None, None, None]).is_err());
        for kind in [0, 3, 32] {
            assert!(Targets::new([Some((1, kind)), None, None, None, None]).is_err());
        }
        let make = || Targets::new([Some((1, FLOOR)), Some((3, TIP)), None, None, None]).unwrap();
        assert!(make().finish().is_err());
        let mut skip = make();
        skip.consume(1).unwrap();
        assert!(skip.consume(3).is_err());
        assert!(skip.consume(2).is_err());
        assert!(skip.finish().is_err());
        let mut repeat = make();
        repeat.consume(1).unwrap();
        assert!(repeat.consume(1).is_err());
        assert!(repeat.finish().is_err());
        let mut short = make();
        short.consume(1).unwrap();
        short.consume(2).unwrap();
        assert!(short.finish().is_err());
    }
    #[test]
    fn maximum_height_ends_without_wraparound() {
        let mut targets = Targets::new([
            Some((u64::MAX, FLOOR)),
            Some((u64::MAX, TIP)),
            None,
            None,
            None,
        ])
        .unwrap();
        targets.consume(u64::MAX).unwrap();
        targets.finish().unwrap();
    }
    #[test]
    fn five_distinct_coordinates_fit_exact_capacity() {
        let mut targets = Targets::new([
            Some((5, TIP)),
            Some((4, CHECK)),
            Some((3, FLOOR)),
            Some((2, TERMINAL)),
            Some((1, RESERVE)),
        ])
        .unwrap();
        assert_eq!(targets.len, 5);
        for height in 1..=5 {
            targets.consume(height).unwrap();
        }
        targets.finish().unwrap();
    }
}
