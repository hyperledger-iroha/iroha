use core::ops::Range;

use objc2::encode::{Encode, Encoding, RefEncode};

use crate::Foundation::NSUInteger;

/// Rust native representation of Objective-C's [`NSRange`].
///
/// `NSRange` models a half-open interval `[location, location + length)` and
/// is used pervasively by Foundation APIs to describe sub-slices of strings,
/// arrays, and other ordered collections. The helpers provided in this module
/// mirror the inline functions from `<Foundation/NSRange.h>` so crates built on
/// top of `icrate` can manipulate ranges without dropping down to the C ABI.
///
/// See Apple's developer documentation for additional background on the type:
/// <https://developer.apple.com/documentation/foundation/nsrange>
#[repr(C)]
// PartialEq is same as NSEqualRanges
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NSRange {
    /// The lower bound of the range (inclusive).
    pub location: NSUInteger,
    /// The number of items in the range, starting from `location`.
    pub length: NSUInteger,
}

impl NSRange {
    /// Create a new range with the given values.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    /// assert_eq!(NSRange::new(3, 2), NSRange::from(3..5));
    /// ```
    #[inline]
    #[doc(alias = "NSMakeRange")]
    pub const fn new(location: usize, length: usize) -> Self {
        // Equivalent to NSMakeRange
        Self { location, length }
    }

    /// Returns `true` if the range contains no items.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    ///
    /// assert!(!NSRange::from(3..5).is_empty());
    /// assert!( NSRange::from(3..3).is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns `true` if the index is within the range.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    ///
    /// assert!(!NSRange::from(3..5).contains(2));
    /// assert!( NSRange::from(3..5).contains(3));
    /// assert!( NSRange::from(3..5).contains(4));
    /// assert!(!NSRange::from(3..5).contains(5));
    ///
    /// assert!(!NSRange::from(3..3).contains(3));
    /// ```
    #[inline]
    #[doc(alias = "NSLocationInRange")]
    pub fn contains(&self, index: usize) -> bool {
        // Same as NSLocationInRange
        if let Some(len) = index.checked_sub(self.location) {
            len < self.length
        } else {
            // index < self.location
            false
        }
    }

    /// Returns the upper bound of the range (exclusive).
    #[inline]
    #[doc(alias = "NSMaxRange")]
    pub fn end(&self) -> usize {
        self.location
            .checked_add(self.length)
            .expect("NSRange too large")
    }

    /// Parses a range from the string representation used by AppKit.
    ///
    /// Accepts the format produced by `NSStringFromRange`, namely
    /// `"{location, length}"`. Whitespace around the braces or comma is
    /// ignored. Returns `None` if the input does not conform to that layout
    /// or if either component fails to parse.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    ///
    /// assert_eq!(NSRange::from_string("{3, 2}"), Some(NSRange::new(3, 2)));
    /// assert_eq!(NSRange::from_string("  {0,0}  "), Some(NSRange::new(0, 0)));
    /// assert_eq!(NSRange::from_string("invalid"), None);
    /// ```
    #[inline]
    #[doc(alias = "NSRangeFromString")]
    pub fn from_string(string: &str) -> Option<Self> {
        let trimmed = string.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return None;
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut parts = inner.split(',');
        let location = parts.next()?.trim().parse::<usize>().ok()?;
        let length = parts.next()?.trim().parse::<usize>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self::new(location, length))
    }

    /// Returns the smallest range that contains both `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    ///
    /// let a = NSRange::from(3..5);
    /// let b = NSRange::from(10..12);
    /// assert_eq!(a.union(&b), NSRange::from(3..12));
    /// assert_eq!(b.union(&a), NSRange::from(3..12));
    /// assert_eq!(a.union(&NSRange::from(4..4)), a);
    /// ```
    #[inline]
    #[doc(alias = "NSUnionRange")]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let start = core::cmp::min(self.location, other.location);
        let end = core::cmp::max(self.end(), other.end());
        Self::new(start, end - start)
    }

    /// Returns the overlap between `self` and `other`.
    ///
    /// If the ranges do not overlap, the returned range has zero length and
    /// its location is set to the greater of the two lower bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::NSRange;
    ///
    /// let a = NSRange::from(3..8);
    /// let b = NSRange::from(5..10);
    /// assert_eq!(a.intersection(&b), NSRange::from(5..8));
    /// assert_eq!(b.intersection(&a), NSRange::from(5..8));
    ///
    /// let c = NSRange::from(10..12);
    /// assert_eq!(a.intersection(&c), NSRange::new(10, 0));
    /// ```
    #[inline]
    #[doc(alias = "NSIntersectionRange")]
    pub fn intersection(&self, other: &Self) -> Self {
        let start = core::cmp::max(self.location, other.location);
        let end = core::cmp::min(self.end(), other.end());
        if end <= start {
            Self::new(start, 0)
        } else {
            Self::new(start, end - start)
        }
    }
}

// Sadly, we can't do this:
// impl RangeBounds<usize> for NSRange {
//     fn start_bound(&self) -> Bound<&usize> {
//         Bound::Included(&self.location)
//     }
//     fn end_bound(&self) -> Bound<&usize> {
//         Bound::Excluded(&(self.location + self.length))
//     }
// }

impl From<Range<usize>> for NSRange {
    fn from(range: Range<usize>) -> Self {
        let length = range
            .end
            .checked_sub(range.start)
            .expect("Range end < start");
        Self {
            location: range.start,
            length,
        }
    }
}

impl From<NSRange> for Range<usize> {
    #[inline]
    fn from(nsrange: NSRange) -> Self {
        Self {
            start: nsrange.location,
            end: nsrange.end(),
        }
    }
}

unsafe impl Encode for NSRange {
    const ENCODING: Encoding = Encoding::Struct("_NSRange", &[usize::ENCODING, usize::ENCODING]);
}

unsafe impl RefEncode for NSRange {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_range() {
        let cases: &[(Range<usize>, NSRange)] = &[
            (0..0, NSRange::new(0, 0)),
            (0..10, NSRange::new(0, 10)),
            (10..10, NSRange::new(10, 0)),
            (10..20, NSRange::new(10, 10)),
        ];

        for (range, expected) in cases {
            assert_eq!(NSRange::from(range.clone()), *expected);
        }
    }

    #[test]
    fn test_union_and_intersection() {
        let a = NSRange::from(3..8);
        let b = NSRange::from(5..10);
        assert_eq!(a.union(&b), NSRange::from(3..10));
        assert_eq!(b.union(&a), NSRange::from(3..10));

        assert_eq!(a.intersection(&b), NSRange::from(5..8));
        assert_eq!(b.intersection(&a), NSRange::from(5..8));

        let disjoint = NSRange::from(11..15);
        assert_eq!(a.intersection(&disjoint), NSRange::new(11, 0));
        assert_eq!(disjoint.intersection(&a), NSRange::new(11, 0));
        assert_eq!(a.union(&disjoint), NSRange::from(3..15));
    }

    #[test]
    #[should_panic = "Range end < start"]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_from_range_inverted() {
        let _ = NSRange::from(10..0);
    }

    #[test]
    fn test_contains() {
        let range = NSRange::from(10..20);
        assert!(!range.contains(0));
        assert!(!range.contains(9));
        assert!(range.contains(10));
        assert!(range.contains(11));
        assert!(!range.contains(20));
        assert!(!range.contains(21));
    }

    #[test]
    fn test_end() {
        let range = NSRange::from(10..20);
        assert!(!range.contains(0));
        assert!(!range.contains(9));
        assert!(range.contains(10));
        assert!(range.contains(11));
        assert!(!range.contains(20));
        assert!(!range.contains(21));
    }

    #[test]
    #[should_panic = "NSRange too large"]
    fn test_end_large() {
        let _ = NSRange::new(usize::MAX, usize::MAX).end();
    }

    #[test]
    fn test_from_string() {
        assert_eq!(NSRange::from_string("{3, 5}"), Some(NSRange::new(3, 5)));
        assert_eq!(NSRange::from_string(" {0,0} "), Some(NSRange::new(0, 0)));
        assert_eq!(NSRange::from_string("{10,0}"), Some(NSRange::new(10, 0)));
        assert_eq!(NSRange::from_string("{5,}"), None);
        assert_eq!(NSRange::from_string("{a, 1}"), None);
        assert_eq!(NSRange::from_string(""), None);
        assert_eq!(NSRange::from_string("{1,2,3}"), None);
    }
}
