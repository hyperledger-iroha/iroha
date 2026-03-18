use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::ffi::NSUInteger;

#[cfg(target_pointer_width = "64")]
type InnerFloat = f64;
#[cfg(not(target_pointer_width = "64"))]
type InnerFloat = f32;

/// The basic type for all floating-point values.
///
/// This is [`f32`] on 32-bit platforms and [`f64`] on 64-bit platforms.
///
/// This technically belongs to the `CoreGraphics` framework, but we define it
/// here for convenience.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/coregraphics/cgfloat?language=objc).
// Defined in CoreGraphics/CGBase.h
// TODO: Use a newtype here?
pub type CGFloat = InnerFloat;

// NSGeometry types are aliases to CGGeometry types on iOS, tvOS, watchOS and
// macOS 64bit (and hence their Objective-C encodings are different).
//
// TODO: Adjust `objc2-encode` so that this is handled there, and so that we
// can effectively forget about it and use `NS` and `CG` types equally.
#[cfg(all(
    feature = "apple",
    not(all(target_os = "macos", target_pointer_width = "32"))
))]
mod names {
    pub(super) const POINT: &str = "CGPoint";
    pub(super) const SIZE: &str = "CGSize";
    pub(super) const RECT: &str = "CGRect";
}

#[cfg(any(
    feature = "gnustep-1-7",
    all(target_os = "macos", target_pointer_width = "32")
))]
mod names {
    pub(super) const POINT: &str = "_NSPoint";
    pub(super) const SIZE: &str = "_NSSize";
    pub(super) const RECT: &str = "_NSRect";
}

/// A point in a two-dimensional coordinate system.
///
/// This technically belongs to the `CoreGraphics` framework, but we define it
/// here for convenience.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cgpoint?language=objc).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CGPoint {
    /// The x-coordinate of the point.
    pub x: CGFloat,
    /// The y-coordinate of the point.
    pub y: CGFloat,
}

unsafe impl Encode for CGPoint {
    const ENCODING: Encoding =
        Encoding::Struct(names::POINT, &[CGFloat::ENCODING, CGFloat::ENCODING]);
}

unsafe impl RefEncode for CGPoint {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

impl CGPoint {
    /// Create a new point with the given coordinates.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::CGPoint;
    /// assert_eq!(CGPoint::new(10.0, -2.3), CGPoint { x: 10.0, y: -2.3 });
    /// ```
    #[inline]
    #[doc(alias = "NSMakePoint")]
    #[doc(alias = "CGPointMake")]
    pub const fn new(x: CGFloat, y: CGFloat) -> Self {
        Self { x, y }
    }

    /// A point with both coordinates set to `0.0`.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::CGPoint;
    /// assert_eq!(CGPoint::ZERO, CGPoint { x: 0.0, y: 0.0 });
    /// ```
    #[doc(alias = "NSZeroPoint")]
    #[doc(alias = "CGPointZero")]
    #[doc(alias = "ORIGIN")]
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

/// A two-dimensional size.
///
/// As this is sometimes used to represent a distance vector, rather than a
/// physical size, the width and height are _not_ guaranteed to be
/// non-negative! Methods that expect that must use one of [`CGSize::abs`] or
/// [`CGRect::standardize`].
///
/// This technically belongs to the `CoreGraphics` framework, but we define it
/// here for convenience.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cgsize?language=objc).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CGSize {
    /// The dimensions along the x-axis.
    pub width: CGFloat,
    /// The dimensions along the y-axis.
    pub height: CGFloat,
}

unsafe impl Encode for CGSize {
    const ENCODING: Encoding =
        Encoding::Struct(names::SIZE, &[CGFloat::ENCODING, CGFloat::ENCODING]);
}

unsafe impl RefEncode for CGSize {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

impl CGSize {
    /// Create a new size with the given dimensions.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::CGSize;
    /// let size = CGSize::new(10.0, 2.3);
    /// assert_eq!(size.width, 10.0);
    /// assert_eq!(size.height, 2.3);
    /// ```
    ///
    /// Negative values are allowed (though often undesired).
    ///
    /// ```
    /// use icrate::Foundation::CGSize;
    /// let size = CGSize::new(-1.0, 0.0);
    /// assert_eq!(size.width, -1.0);
    /// ```
    #[inline]
    #[doc(alias = "NSMakeSize")]
    #[doc(alias = "CGSizeMake")]
    pub const fn new(width: CGFloat, height: CGFloat) -> Self {
        // The documentation for NSSize explicitly says:
        // > If the value of width or height is negative, however, the
        // > behavior of some methods may be undefined.
        //
        // But since this type can come from FFI, we'll leave it up to the
        // user to ensure that it is used safely.
        Self { width, height }
    }

    /// Convert the size to a non-negative size.
    ///
    /// This can be used to convert the size to a safe value.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::CGSize;
    /// assert_eq!(CGSize::new(-1.0, 1.0).abs(), CGSize::new(1.0, 1.0));
    /// ```
    #[inline]
    pub fn abs(self) -> Self {
        Self::new(self.width.abs(), self.height.abs())
    }

    /// A size that is 0.0 in both dimensions.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::CGSize;
    /// assert_eq!(CGSize::ZERO, CGSize { width: 0.0, height: 0.0 });
    /// ```
    #[doc(alias = "NSZeroSize")]
    #[doc(alias = "CGSizeZero")]
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

/// The location and dimensions of a rectangle.
///
/// In the default Core Graphics coordinate space (macOS), the origin is
/// located in the lower-left corner of the rectangle and the rectangle
/// extends towards the upper-right corner.
///
/// If the context has a flipped coordinate space (iOS, tvOS, watchOS) the
/// origin is in the upper-left corner and the rectangle extends towards the
/// lower-right corner.
///
/// This technically belongs to the `CoreGraphics` framework, but we define it
/// here for convenience.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cgrect?language=objc).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CGRect {
    /// The coordinates of the rectangle’s origin.
    pub origin: CGPoint,
    /// The dimensions of the rectangle.
    pub size: CGSize,
}

unsafe impl Encode for CGRect {
    const ENCODING: Encoding =
        Encoding::Struct(names::RECT, &[CGPoint::ENCODING, CGSize::ENCODING]);
}

unsafe impl RefEncode for CGRect {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

impl CGRect {
    /// Create a new rectangle with the given origin and dimensions.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::{CGPoint, CGRect, CGSize};
    /// let origin = CGPoint::new(10.0, -2.3);
    /// let size = CGSize::new(5.0, 0.0);
    /// let rect = CGRect::new(origin, size);
    /// ```
    #[inline]
    #[doc(alias = "NSMakeRect")]
    #[doc(alias = "CGRectMake")]
    pub const fn new(origin: CGPoint, size: CGSize) -> Self {
        Self { origin, size }
    }

    /// A rectangle with origin (0.0, 0.0) and zero width and height.
    #[doc(alias = "NSZeroRect")]
    #[doc(alias = "CGRectZero")]
    pub const ZERO: Self = Self::new(CGPoint::ZERO, CGSize::ZERO);

    /// Returns a rectangle with a positive width and height.
    ///
    /// This is often useful
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::{CGPoint, CGRect, CGSize};
    /// let origin = CGPoint::new(1.0, 1.0);
    /// let size = CGSize::new(-5.0, -2.0);
    /// let rect = CGRect::new(origin, size);
    /// assert_eq!(rect.standardize().size, CGSize::new(5.0, 2.0));
    /// ```
    #[inline]
    #[doc(alias = "CGRectStandardize")]
    pub fn standardize(self) -> Self {
        Self::new(self.origin, self.size.abs())
    }

    /// The smallest coordinate of the rectangle.
    #[inline]
    #[doc(alias = "CGRectGetMinX")]
    #[doc(alias = "CGRectGetMinY")]
    #[doc(alias = "NSMinX")]
    #[doc(alias = "NSMinY")]
    pub fn min(self) -> CGPoint {
        self.origin
    }

    /// The center point of the rectangle.
    #[inline]
    #[doc(alias = "CGRectGetMidX")]
    #[doc(alias = "CGRectGetMidY")]
    #[doc(alias = "NSMidX")]
    #[doc(alias = "NSMidY")]
    pub fn mid(self) -> CGPoint {
        CGPoint::new(
            self.origin.x + (self.size.width * 0.5),
            self.origin.y + (self.size.height * 0.5),
        )
    }

    /// The largest coordinate of the rectangle.
    #[inline]
    #[doc(alias = "CGRectGetMaxX")]
    #[doc(alias = "CGRectGetMaxY")]
    #[doc(alias = "NSMaxX")]
    #[doc(alias = "NSMaxY")]
    pub fn max(self) -> CGPoint {
        CGPoint::new(
            self.origin.x + self.size.width,
            self.origin.y + self.size.height,
        )
    }

    /// Returns whether a rectangle has zero width or height.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::{CGPoint, CGRect, CGSize};
    /// assert!(CGRect::ZERO.is_empty());
    /// let point = CGPoint::new(1.0, 2.0);
    /// assert!(CGRect::new(point, CGSize::ZERO).is_empty());
    /// assert!(!CGRect::new(point, CGSize::new(1.0, 1.0)).is_empty());
    /// ```
    #[inline]
    #[doc(alias = "CGRectIsEmpty")]
    pub fn is_empty(self) -> bool {
        self.size.width <= 0.0 || self.size.height <= 0.0
    }

    /// Returns `true` if this rectangle is the Core Graphics null sentinel.
    #[inline]
    #[doc(alias = "CGRectIsNull")]
    pub fn is_null(self) -> bool {
        self.origin.x == CGFloat::INFINITY && self.origin.y == CGFloat::INFINITY
    }

    /// Returns `true` if this rectangle represents an unbounded extent.
    #[inline]
    #[doc(alias = "CGRectIsInfinite")]
    pub fn is_infinite(self) -> bool {
        (self.size.width.is_infinite() || self.size.height.is_infinite()) && !self.is_null()
    }

    #[inline]
    fn axis_bounds(origin: CGFloat, extent: CGFloat) -> Option<(CGFloat, CGFloat)> {
        if origin.is_nan() || extent.is_nan() {
            return None;
        }

        let mut min = origin;
        let mut max = if extent.is_infinite() {
            if extent.is_sign_positive() {
                CGFloat::INFINITY
            } else {
                CGFloat::NEG_INFINITY
            }
        } else {
            origin + extent
        };

        if extent < 0.0 || (extent.is_infinite() && extent.is_sign_negative()) {
            core::mem::swap(&mut min, &mut max);
        }

        Some((min, max))
    }

    #[inline]
    fn bounding_edges(self) -> Option<(CGFloat, CGFloat, CGFloat, CGFloat)> {
        let (min_x, max_x) = Self::axis_bounds(self.origin.x, self.size.width)?;
        let (min_y, max_y) = Self::axis_bounds(self.origin.y, self.size.height)?;
        Some((min_x, min_y, max_x, max_y))
    }

    #[inline]
    fn clamp_division_extent(amount: CGFloat, total: CGFloat) -> (CGFloat, bool) {
        if amount.is_nan() || total.is_nan() || total <= 0.0 {
            return (0.0, false);
        }
        if amount <= 0.0 {
            return (0.0, false);
        }
        if total.is_infinite() {
            if amount.is_infinite() {
                (total, true)
            } else {
                (amount, false)
            }
        } else if amount >= total {
            (total, true)
        } else {
            (amount, false)
        }
    }

    #[inline]
    fn remaining_extent(total: CGFloat, slice: CGFloat, took_all: bool) -> CGFloat {
        if took_all {
            0.0
        } else if total.is_infinite() && slice.is_infinite() {
            0.0
        } else {
            total - slice
        }
    }

    /// Split this rectangle into a slice adjacent to `edge` and the remainder.
    ///
    /// This mirrors [`NSDivideRect`] and [`CGRectDivide`]. The returned pair
    /// contains the slice first and the remainder second. When `amount` is less
    /// than or equal to zero, the slice collapses to zero width/height and the
    /// remainder equals the original rectangle. When `amount` exceeds the span
    /// of the rectangle on the chosen axis, the entire rectangle becomes the
    /// slice and the remainder collapses along that axis.
    #[inline]
    #[doc(alias = "NSDivideRect")]
    #[doc(alias = "CGRectDivide")]
    pub fn divide(self, amount: CGFloat, edge: NSRectEdge) -> (Self, Self) {
        if self.is_null() {
            return (Self::null(), Self::null());
        }

        let Some((min_x, min_y, max_x, max_y)) = self.bounding_edges() else {
            return (Self::null(), Self::null());
        };

        let width = max_x - min_x;
        let height = max_y - min_y;

        let edge_value = edge;
        if edge_value == NSRectEdgeMinX || edge_value == NSMinXEdge {
            let (slice_width, took_all) = Self::clamp_division_extent(amount, width);
            let remainder_width = Self::remaining_extent(width, slice_width, took_all);
            let remainder_origin_x = if took_all {
                max_x
            } else {
                min_x + slice_width
            };

            (
                Self::new(
                    CGPoint::new(min_x, min_y),
                    CGSize::new(slice_width, height),
                ),
                Self::new(
                    CGPoint::new(remainder_origin_x, min_y),
                    CGSize::new(remainder_width, height),
                ),
            )
        } else if edge_value == NSRectEdgeMaxX || edge_value == NSMaxXEdge {
            let (slice_width, took_all) = Self::clamp_division_extent(amount, width);
            let remainder_width = Self::remaining_extent(width, slice_width, took_all);
            let slice_origin_x = if took_all {
                min_x
            } else {
                max_x - slice_width
            };

            (
                Self::new(
                    CGPoint::new(slice_origin_x, min_y),
                    CGSize::new(slice_width, height),
                ),
                Self::new(
                    CGPoint::new(min_x, min_y),
                    CGSize::new(remainder_width, height),
                ),
            )
        } else if edge_value == NSRectEdgeMinY || edge_value == NSMinYEdge {
            let (slice_height, took_all) = Self::clamp_division_extent(amount, height);
            let remainder_height = Self::remaining_extent(height, slice_height, took_all);
            let remainder_origin_y = if took_all {
                max_y
            } else {
                min_y + slice_height
            };

            (
                Self::new(
                    CGPoint::new(min_x, min_y),
                    CGSize::new(width, slice_height),
                ),
                Self::new(
                    CGPoint::new(min_x, remainder_origin_y),
                    CGSize::new(width, remainder_height),
                ),
            )
        } else if edge_value == NSRectEdgeMaxY || edge_value == NSMaxYEdge {
            let (slice_height, took_all) = Self::clamp_division_extent(amount, height);
            let remainder_height = Self::remaining_extent(height, slice_height, took_all);
            let slice_origin_y = if took_all {
                min_y
            } else {
                max_y - slice_height
            };

            (
                Self::new(
                    CGPoint::new(min_x, slice_origin_y),
                    CGSize::new(width, slice_height),
                ),
                Self::new(
                    CGPoint::new(min_x, min_y),
                    CGSize::new(width, remainder_height),
                ),
            )
        } else {
            self.divide(amount, NSRectEdgeMinX)
        }
    }

    /// Returns the smallest rectangle with integral coordinates covering `self`.
    ///
    /// This mirrors `CGRectIntegral` by flooring the lower bounds and
    /// ceiling the upper bounds. Null rectangles remain null and infinite
    /// rectangles stay infinite.
    #[inline]
    #[doc(alias = "NSIntegralRect")]
    #[doc(alias = "CGRectIntegral")]
    pub fn integral(self) -> Self {
        if self.is_null() {
            return Self::null();
        }
        if self.is_infinite() {
            return Self::infinite();
        }

        let Some((min_x, min_y, max_x, max_y)) = self.bounding_edges() else {
            return Self::null();
        };

        let floor_min_x = min_x.floor();
        let floor_min_y = min_y.floor();
        let ceil_max_x = max_x.ceil();
        let ceil_max_y = max_y.ceil();

        Self::new(
            CGPoint::new(floor_min_x, floor_min_y),
            CGSize::new(ceil_max_x - floor_min_x, ceil_max_y - floor_min_y),
        )
    }

    /// Returns a rectangle inset by the given amounts along each axis.
    ///
    /// Positive values shrink the rectangle, while negative values expand it.
    /// Mirrors `CGRectInset`, preserving the null and infinite sentinels.
    #[inline]
    #[doc(alias = "NSInsetRect")]
    #[doc(alias = "CGRectInset")]
    pub fn inset(self, dx: CGFloat, dy: CGFloat) -> Self {
        if self.is_null() {
            return Self::null();
        }
        if self.is_infinite() {
            return Self::infinite();
        }

        Self::new(
            CGPoint::new(self.origin.x + dx, self.origin.y + dy),
            CGSize::new(self.size.width - (dx * 2.0), self.size.height - (dy * 2.0)),
        )
    }
    /// Returns `true` if the given point lies within this rectangle.
    ///
    /// The check uses the same half-open semantics as `CGRectContainsPoint`:
    /// points on the left or bottom edge are considered inside, while points
    /// on the right or top edge are outside.
    ///
    /// Rectangles with zero or negative width/height never contain any point.
    #[inline]
    #[doc(alias = "NSPointInRect")]
    #[doc(alias = "CGRectContainsPoint")]
    pub fn contains_point(self, point: CGPoint) -> bool {
        if !(self.size.width > 0.0 && self.size.height > 0.0) {
            return false;
        }
        let min_x = self.origin.x;
        let min_y = self.origin.y;
        let max_x = self.origin.x + self.size.width;
        let max_y = self.origin.y + self.size.height;

        point.x >= min_x && point.x < max_x && point.y >= min_y && point.y < max_y
    }

    /// Returns `true` if the given point lies within this rectangle, accounting
    /// for flipped coordinate systems.
    ///
    /// Mirrors `NSMouseInRect` by treating the edge corresponding to the "top"
    /// of the coordinate system as exclusive and the opposite edge as inclusive.
    /// When `flipped` is `true`, the origin is considered the top-left corner and
    /// therefore excluded; otherwise the upper edge is excluded.
    #[inline]
    #[doc(alias = "NSMouseInRect")]
    pub fn mouse_in_rect(self, point: CGPoint, flipped: bool) -> bool {
        let x0 = self.origin.x;
        let x1 = self.origin.x + self.size.width;
        let (min_x, max_x) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        if !(max_x > min_x) {
            return false;
        }

        let y0 = self.origin.y;
        let y1 = self.origin.y + self.size.height;
        let (min_y, max_y) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        if !(max_y > min_y) {
            return false;
        }

        if !(point.x >= min_x && point.x < max_x) {
            return false;
        }

        if flipped {
            point.y > min_y && point.y <= max_y
        } else {
            point.y >= min_y && point.y < max_y
        }
    }

    /// Returns `true` if this rectangle intersects `other`.
    ///
    /// The comparison mirrors `CGRectIntersectsRect`: empty rectangles never
    /// intersect, and touching edges without overlapping area yield `false`.
    #[inline]
    #[doc(alias = "NSIntersectsRect")]
    #[doc(alias = "CGRectIntersectsRect")]
    pub fn intersects_rect(self, other: Self) -> bool {
        let a = self.standardize();
        let b = other.standardize();
        if a.is_empty() || b.is_empty() {
            return false;
        }

        let a_min_x = a.origin.x;
        let a_min_y = a.origin.y;
        let a_max_x = a.origin.x + a.size.width;
        let a_max_y = a.origin.y + a.size.height;

        let b_min_x = b.origin.x;
        let b_min_y = b.origin.y;
        let b_max_x = b.origin.x + b.size.width;
        let b_max_y = b.origin.y + b.size.height;

        !(b_min_x >= a_max_x
            || b_max_x <= a_min_x
            || b_min_y >= a_max_y
            || b_max_y <= a_min_y)
    }

    /// Returns `true` if `other` lies completely inside this rectangle.
    ///
    /// This mirrors the behaviour of `CGRectContainsRect`, operating on the
    /// standardized (positive-size) forms of both rectangles.
    #[inline]
    #[doc(alias = "NSContainsRect")]
    #[doc(alias = "CGRectContainsRect")]
    pub fn contains_rect(self, other: Self) -> bool {
        let a = self.standardize();
        if a.is_empty() {
            return false;
        }
        let b = other.standardize();

        let a_min_x = a.origin.x;
        let a_min_y = a.origin.y;
        let a_max_x = a.origin.x + a.size.width;
        let a_max_y = a.origin.y + a.size.height;

        let b_min_x = b.origin.x;
        let b_min_y = b.origin.y;
        let b_max_x = b.origin.x + b.size.width;
        let b_max_y = b.origin.y + b.size.height;

        b_min_x >= a_min_x
            && b_min_y >= a_min_y
            && b_max_x <= a_max_x
            && b_max_y <= a_max_y
    }

    /// Returns the overlapping area of two rectangles.
    ///
    /// If the rectangles do not intersect, the null sentinel is returned.
    #[inline]
    #[doc(alias = "NSIntersectionRect")]
    #[doc(alias = "CGRectIntersection")]
    pub fn intersection(self, other: Self) -> Self {
        if self.is_null() || other.is_null() {
            return Self::null();
        }

        if self.is_infinite() {
            return other;
        }
        if other.is_infinite() {
            return self;
        }

        let Some((a_min_x, a_min_y, a_max_x, a_max_y)) = self.bounding_edges() else {
            return Self::null();
        };
        let Some((b_min_x, b_min_y, b_max_x, b_max_y)) = other.bounding_edges() else {
            return Self::null();
        };

        let min_x = a_min_x.max(b_min_x);
        let min_y = a_min_y.max(b_min_y);
        let max_x = a_max_x.min(b_max_x);
        let max_y = a_max_y.min(b_max_y);

        if !(min_x < max_x && min_y < max_y) {
            return Self::null();
        }

        Self::new(
            CGPoint::new(min_x, min_y),
            CGSize::new(max_x - min_x, max_y - min_y),
        )
    }

    /// Returns the smallest rectangle covering `self` and `other`.
    ///
    /// If either rectangle is null, the other rectangle is returned.
    #[inline]
    #[doc(alias = "NSUnionRect")]
    #[doc(alias = "CGRectUnion")]
    pub fn union(self, other: Self) -> Self {
        if self.is_null() {
            return other;
        }
        if other.is_null() {
            return self;
        }
        if self.is_infinite() || other.is_infinite() {
            return Self::infinite();
        }

        let Some((a_min_x, a_min_y, a_max_x, a_max_y)) = self.bounding_edges() else {
            return Self::null();
        };
        let Some((b_min_x, b_min_y, b_max_x, b_max_y)) = other.bounding_edges() else {
            return Self::null();
        };

        let min_x = a_min_x.min(b_min_x);
        let min_y = a_min_y.min(b_min_y);
        let max_x = a_max_x.max(b_max_x);
        let max_y = a_max_y.max(b_max_y);

        Self::new(
            CGPoint::new(min_x, min_y),
            CGSize::new(max_x - min_x, max_y - min_y),
        )
    }

    /// Offsets the rectangle by the given deltas in the x and y directions.
    ///
    /// Returns a rectangle with the same size and the origin moved by the
    /// supplied amounts.
    ///
    /// # Examples
    ///
    /// ```
    /// use icrate::Foundation::{CGPoint, CGRect, CGSize};
    /// let rect = CGRect::new(CGPoint::new(2.0, 3.0), CGSize::new(4.0, 5.0));
    /// let moved = rect.offset(1.5, -2.0);
    /// assert_eq!(moved.origin, CGPoint::new(3.5, 1.0));
    /// assert_eq!(moved.size, rect.size);
    /// ```
    #[inline]
    #[doc(alias = "NSOffsetRect")]
    #[doc(alias = "CGRectOffset")]
    pub fn offset(self, dx: CGFloat, dy: CGFloat) -> Self {
        Self {
            origin: CGPoint::new(self.origin.x + dx, self.origin.y + dy),
            size: self.size,
        }
    }

    /// Width component of the rectangle.
    #[inline]
    #[doc(alias = "NSWidth")]
    #[doc(alias = "CGRectGetWidth")]
    pub fn width(self) -> CGFloat {
        self.size.width
    }

    /// Height component of the rectangle.
    #[inline]
    #[doc(alias = "NSHeight")]
    #[doc(alias = "CGRectGetHeight")]
    pub fn height(self) -> CGFloat {
        self.size.height
    }

    /// Canonical null rectangle (`CGRectNull`).
    #[inline]
    #[doc(alias = "CGRectNull")]
    pub fn null() -> Self {
        Self {
            origin: CGPoint::new(CGFloat::INFINITY, CGFloat::INFINITY),
            size: CGSize::new(0.0, 0.0),
        }
    }

    /// Canonical infinite rectangle (`CGRectInfinite`).
    #[inline]
    #[doc(alias = "CGRectInfinite")]
    pub fn infinite() -> Self {
        Self {
            origin: CGPoint::new(-CGFloat::INFINITY, -CGFloat::INFINITY),
            size: CGSize::new(CGFloat::INFINITY, CGFloat::INFINITY),
        }
    }
}

/// A point in a Cartesian coordinate system.
///
/// This is a convenience alias for [`CGPoint`]. For ease of use, it is
/// available on all platforms, though in practice it is only useful on macOS.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/foundation/nspoint?language=objc).
pub type NSPoint = CGPoint;

/// A two-dimensional size.
///
/// This is a convenience alias for [`CGSize`]. For ease of use, it is
/// available on all platforms, though in practice it is only useful on macOS.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/foundation/nssize?language=objc).
pub type NSSize = CGSize;

/// A rectangle.
///
/// This is a convenience alias for [`CGRect`]. For ease of use, it is
/// available on all platforms, though in practice it is only useful on macOS.
///
/// See [Apple's documentation](https://developer.apple.com/documentation/foundation/nsrect?language=objc).
pub type NSRect = CGRect;

#[inline]
#[doc(alias = "NSDivideRect")]
#[doc(alias = "CGRectDivide")]
#[cfg_attr(not(test), allow(dead_code))]
pub fn divide_rect(rect: CGRect, amount: CGFloat, edge: NSRectEdge) -> (CGRect, CGRect) {
    rect.divide(amount, edge)
}

ns_enum!(
    #[underlying(NSUInteger)]
    pub enum NSRectEdge {
        NSRectEdgeMinX = 0,
        NSRectEdgeMinY = 1,
        NSRectEdgeMaxX = 2,
        NSRectEdgeMaxY = 3,
        NSMinXEdge = NSRectEdgeMinX,
        NSMinYEdge = NSRectEdgeMinY,
        NSMaxXEdge = NSRectEdgeMaxX,
        NSMaxYEdge = NSRectEdgeMaxY,
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgsize_new() {
        CGSize::new(1.0, 1.0);
        CGSize::new(0.0, -0.0);
        CGSize::new(-0.0, 0.0);
        CGSize::new(-0.0, -0.0);
        CGSize::new(-1.0, -1.0);
        CGSize::new(-1.0, 1.0);
        CGSize::new(1.0, -1.0);
    }

    #[test]
    fn rect_contains_point() {
        let rect = CGRect::new(CGPoint::new(10.0, 5.0), CGSize::new(4.0, 3.0));
        assert!(rect.contains_point(CGPoint::new(10.0, 5.0))); // lower-left corner
        assert!(rect.contains_point(CGPoint::new(11.5, 6.5)));
        assert!(!rect.contains_point(CGPoint::new(14.0, 8.0))); // top-right edge (exclusive)
        assert!(!rect.contains_point(CGPoint::new(9.99, 5.0)));

        // Empty and negative dimensions never contain points.
        let empty = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 1.0));
        assert!(!empty.contains_point(CGPoint::new(0.0, 0.0)));

        let inverted = CGRect::new(CGPoint::new(4.0, 4.0), CGSize::new(-2.0, 2.0));
        assert!(!inverted.contains_point(CGPoint::new(3.0, 5.0)));
    }

    #[test]
    fn rect_divide_min_x_edge() {
        let rect = CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(100.0, 50.0));
        let (slice, remainder) = rect.divide(30.0, NSRectEdgeMinX);
        assert_eq!(
            slice,
            CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(30.0, 50.0))
        );
        assert_eq!(
            remainder,
            CGRect::new(CGPoint::new(40.0, 20.0), CGSize::new(70.0, 50.0))
        );
    }

    #[test]
    fn rect_divide_max_x_edge() {
        let rect = CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(100.0, 50.0));
        let (slice, remainder) = rect.divide(20.0, NSRectEdgeMaxX);
        assert_eq!(
            slice,
            CGRect::new(CGPoint::new(90.0, 20.0), CGSize::new(20.0, 50.0))
        );
        assert_eq!(
            remainder,
            CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(80.0, 50.0))
        );
    }

    #[test]
    fn rect_divide_negative_and_overshoot() {
        let rect = CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(100.0, 50.0));

        let (slice_zero, remainder_zero) = rect.divide(-5.0, NSRectEdgeMinX);
        assert_eq!(
            slice_zero,
            CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(0.0, 50.0))
        );
        assert_eq!(
            remainder_zero,
            CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(100.0, 50.0))
        );

        let (slice_full, remainder_full) = rect.divide(500.0, NSRectEdgeMinX);
        assert_eq!(
            slice_full,
            CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(100.0, 50.0))
        );
        assert_eq!(
            remainder_full,
            CGRect::new(CGPoint::new(110.0, 20.0), CGSize::new(0.0, 50.0))
        );
    }

    #[test]
    fn rect_divide_negative_width() {
        let rect = CGRect::new(CGPoint::new(10.0, 20.0), CGSize::new(-100.0, 50.0));
        let (slice, remainder) = rect.divide(30.0, NSRectEdgeMinX);
        assert_eq!(
            slice,
            CGRect::new(CGPoint::new(-90.0, 20.0), CGSize::new(30.0, 50.0))
        );
        assert_eq!(
            remainder,
            CGRect::new(CGPoint::new(-60.0, 20.0), CGSize::new(70.0, 50.0))
        );
    }

    #[test]
    fn rect_divide_zero_and_null() {
        let zero_rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 0.0));
        let (slice_zero, remainder_zero) = zero_rect.divide(10.0, NSRectEdgeMinX);
        assert_eq!(slice_zero, zero_rect);
        assert_eq!(remainder_zero, zero_rect);

        let null_rect = CGRect::null();
        let (slice_null, remainder_null) = null_rect.divide(10.0, NSRectEdgeMinX);
        assert_eq!(slice_null, CGRect::null());
        assert_eq!(remainder_null, CGRect::null());
    }

    #[test]
    fn divide_rect_alias() {
        let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(10.0, 4.0));
        let direct = rect.divide(2.0, NSRectEdgeMinY);
        let via_fn = divide_rect(rect, 2.0, NSRectEdgeMinY);
        assert_eq!(direct, via_fn);
    }

    #[test]
    fn rect_mouse_in_rect_semantics() {
        let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(10.0, 8.0));
        assert!(rect.mouse_in_rect(CGPoint::new(0.0, 0.0), false));
        assert!(rect.mouse_in_rect(CGPoint::new(9.999, 4.0), false));
        assert!(!rect.mouse_in_rect(CGPoint::new(10.0, 4.0), false)); // right edge exclusive
        assert!(!rect.mouse_in_rect(CGPoint::new(5.0, 8.0), false)); // top edge exclusive

        assert!(!rect.mouse_in_rect(CGPoint::new(5.0, 0.0), true)); // top edge (flipped) exclusive
        assert!(rect.mouse_in_rect(CGPoint::new(5.0, 8.0), true)); // bottom edge inclusive when flipped
        assert!(!rect.mouse_in_rect(CGPoint::new(5.0, 8.001), true));
    }

    #[test]
    fn rect_contains_rect() {
        let outer = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(10.0, 10.0));
        let inner = CGRect::new(CGPoint::new(2.0, 3.0), CGSize::new(4.0, 5.0));
        assert!(outer.contains_rect(inner));
        assert!(!inner.contains_rect(outer));

        let touching_edge = CGRect::new(CGPoint::new(10.0, 10.0), CGSize::new(2.0, 2.0));
        assert!(!outer.contains_rect(touching_edge));

        let empty = CGRect::new(CGPoint::new(5.0, 5.0), CGSize::new(0.0, 0.0));
        assert!(outer.contains_rect(empty));

        let negative = CGRect::new(CGPoint::new(6.0, 6.0), CGSize::new(-1.0, -1.0));
        assert!(outer.contains_rect(negative));
    }

    #[test]
    fn rect_intersects_rect() {
        let a = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(5.0, 5.0));
        let b = CGRect::new(CGPoint::new(4.0, 4.0), CGSize::new(3.0, 3.0));
        assert!(a.intersects_rect(b));

        let disjoint = CGRect::new(CGPoint::new(10.0, 10.0), CGSize::new(2.0, 2.0));
        assert!(!a.intersects_rect(disjoint));

        let touching = CGRect::new(CGPoint::new(5.0, 0.0), CGSize::new(2.0, 2.0));
        assert!(!a.intersects_rect(touching));

        let empty = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(0.0, 5.0));
        assert!(!a.intersects_rect(empty));
    }

    #[test]
    fn rect_null_and_infinite_flags() {
        let null = CGRect::null();
        assert!(null.is_null());
        assert!(null.is_empty());
        assert!(!null.is_infinite());

        let infinite = CGRect::infinite();
        assert!(!infinite.is_null());
        assert!(infinite.is_infinite());

        let finite = CGRect::new(CGPoint::new(1.0, 2.0), CGSize::new(3.0, 4.0));
        assert!(!finite.is_null());
        assert!(!finite.is_infinite());
    }

    #[test]
    fn rect_intersection_behaviour() {
        let a = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(4.0, 4.0));
        let b = CGRect::new(CGPoint::new(2.0, 1.0), CGSize::new(4.0, 3.0));
        let intersection = a.intersection(b);
        assert_eq!(intersection.origin, CGPoint::new(2.0, 1.0));
        assert_eq!(intersection.size, CGSize::new(2.0, 3.0));

        let disjoint = CGRect::new(CGPoint::new(10.0, 10.0), CGSize::new(2.0, 2.0));
        assert!(a.intersection(disjoint).is_null());

        let touching = CGRect::new(CGPoint::new(4.0, -1.0), CGSize::new(2.0, 5.0));
        assert!(a.intersection(touching).is_null());

        let inverted = CGRect::new(CGPoint::new(4.0, 4.0), CGSize::new(-2.0, -2.0));
        let inverted_intersection = a.intersection(inverted);
        assert!(!inverted_intersection.is_null());
        assert_eq!(inverted_intersection.origin, CGPoint::new(2.0, 2.0));
        assert_eq!(inverted_intersection.size, CGSize::new(2.0, 2.0));

        let infinite = CGRect::infinite();
        assert_eq!(infinite.intersection(a), a);
        assert_eq!(a.intersection(infinite), a);
    }

    #[test]
    fn rect_union_behaviour() {
        let a = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(4.0, 4.0));
        let b = CGRect::new(CGPoint::new(2.0, -1.0), CGSize::new(4.0, 3.0));
        let union = a.union(b);
        assert_eq!(union.origin, CGPoint::new(0.0, -1.0));
        assert_eq!(union.size, CGSize::new(6.0, 5.0));

        let null = CGRect::null();
        assert_eq!(a.union(null), a);
        assert_eq!(null.union(a), a);

        let inverted = CGRect::new(CGPoint::new(6.0, 3.0), CGSize::new(-2.0, 4.0));
        let union_with_inverted = a.union(inverted);
        assert_eq!(union_with_inverted.origin, CGPoint::new(0.0, 0.0));
        assert_eq!(union_with_inverted.size, CGSize::new(6.0, 7.0));

        let infinite = CGRect::infinite();
        assert!(a.union(infinite).is_infinite());
        assert!(infinite.union(a).is_infinite());
    }

    #[test]
    fn rect_inset_behaviour() {
        let rect = CGRect::new(CGPoint::new(1.0, 2.0), CGSize::new(6.0, 8.0));
        let inset = rect.inset(1.5, 2.0);
        assert_eq!(inset.origin, CGPoint::new(2.5, 4.0));
        assert_eq!(inset.size, CGSize::new(3.0, 4.0));

        // Negative inset grows the rectangle.
        let expanded = rect.inset(-1.0, -0.5);
        assert_eq!(expanded.origin, CGPoint::new(0.0, 1.5));
        assert_eq!(expanded.size, CGSize::new(8.0, 9.0));

        assert!(CGRect::null().inset(1.0, 1.0).is_null());
        assert!(CGRect::infinite().inset(1.0, 1.0).is_infinite());
    }

    #[test]
    fn rect_integral_behaviour() {
        let rect = CGRect::new(CGPoint::new(0.2, 0.5), CGSize::new(3.4, 2.2));
        let integral = rect.integral();
        assert_eq!(integral.origin, CGPoint::new(0.0, 0.0));
        assert_eq!(integral.size, CGSize::new(4.0, 3.0));

        let already_integral = CGRect::new(CGPoint::new(1.0, 2.0), CGSize::new(3.0, 4.0));
        assert_eq!(already_integral.integral(), already_integral);

        let negative_extent = CGRect::new(CGPoint::new(5.1, 8.9), CGSize::new(-2.9, 1.2));
        let integral_negative = negative_extent.integral();
        assert_eq!(integral_negative.origin, CGPoint::new(2.0, 8.0));
        assert_eq!(integral_negative.size, CGSize::new(4.0, 3.0));

        let point_rect = CGRect::new(CGPoint::new(1.5, 1.5), CGSize::ZERO);
        let integral_point = point_rect.integral();
        assert_eq!(integral_point.origin, CGPoint::new(1.0, 1.0));
        assert_eq!(integral_point.size, CGSize::new(1.0, 1.0));

        assert!(CGRect::null().integral().is_null());
        assert!(CGRect::infinite().integral().is_infinite());
    }

    #[test]
    fn rect_is_empty_matches_coregraphics_rules() {
        let rect = CGRect::new(CGPoint::new(1.0, 1.0), CGSize::new(2.0, 3.0));
        assert!(!rect.is_empty());
        assert!(CGRect::new(CGPoint::ZERO, CGSize::new(0.0, 5.0)).is_empty());
        assert!(CGRect::new(CGPoint::ZERO, CGSize::new(-1.0, 5.0)).is_empty());

        let nan_width = CGRect::new(CGPoint::ZERO, CGSize::new(CGFloat::NAN, 5.0));
        assert!(!nan_width.is_empty());
        let nan_height = CGRect::new(CGPoint::ZERO, CGSize::new(5.0, CGFloat::NAN));
        assert!(!nan_height.is_empty());
    }

    #[test]
    fn rect_width_height() {
        let rect = CGRect::new(CGPoint::new(1.0, 2.0), CGSize::new(3.5, 4.5));
        assert_eq!(rect.width(), 3.5);
        assert_eq!(rect.height(), 4.5);
    }

    // We know the Rust implementation handles NaN, infinite, negative zero
    // and so on properly, so let's ensure that NSEqualXXX handles these as
    // well (so that we're confident that the implementations are equivalent).
    #[test]
    #[cfg(any(all(feature = "apple", target_os = "macos"), feature = "gnustep-1-7"))] // or macabi
    fn test_partial_eq() {
        use crate::Foundation::{NSEqualPoints, NSEqualRects, NSEqualSizes};

        // We assume that comparisons handle e.g. `x` and `y` in the same way,
        // therefore we set the coordinates / dimensions to the same.
        let cases: &[(CGFloat, CGFloat)] = &[
            (0.0, 0.0),
            (-0.0, -0.0),
            (0.0, -0.0),
            (1.0, 1.0 + CGFloat::EPSILON),
            (0.0, CGFloat::MIN_POSITIVE),
            (0.0, CGFloat::EPSILON),
            (1.0, 1.0),
            (1.0, -1.0),
            // Infinity
            (CGFloat::INFINITY, CGFloat::INFINITY),
            (CGFloat::INFINITY, CGFloat::NEG_INFINITY),
            (CGFloat::NEG_INFINITY, CGFloat::NEG_INFINITY),
            // NaN
            (CGFloat::NAN, 0.0),
            (CGFloat::NAN, 1.0),
            (CGFloat::NAN, CGFloat::NAN),
            (CGFloat::NAN, -CGFloat::NAN),
            (-CGFloat::NAN, -CGFloat::NAN),
            (CGFloat::NAN, CGFloat::INFINITY),
        ];

        for case in cases {
            let point_a = NSPoint::new(case.0, case.1);
            let point_b = NSPoint::new(case.0, case.1);
            let actual = unsafe { NSEqualPoints(point_a, point_b).as_bool() };
            assert_eq!(point_a == point_b, actual);

            if case.0 >= 0.0 && case.1 >= 0.0 {
                let size_a = NSSize::new(case.0, case.1);
                let size_b = NSSize::new(case.0, case.1);
                let actual = unsafe { NSEqualSizes(size_a, size_b).as_bool() };
                assert_eq!(size_a == size_b, actual);

                let rect_a = NSRect::new(point_a, size_a);
                let rect_b = NSRect::new(point_b, size_b);
                let actual = unsafe { NSEqualRects(rect_a, rect_b).as_bool() };
                assert_eq!(rect_a == rect_b, actual);
            }
        }
    }
}
