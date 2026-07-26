//! Consensus-visible tags and register conventions for Kotodama V1 numbers.
//!
//! These values are part of ABI V1. Their numeric discriminants are stable and
//! must be changed together with the ABI hash and golden tests.

/// Stable failure codes returned by fallible numeric syscalls in `r11`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum NumericFaultV1 {
    /// The canonical result exceeds the signed 512-bit integer domain.
    MantissaOverflow = 1,
    /// The canonical exact decimal result requires a scale greater than 28.
    ScaleOverflow = 2,
    /// Division by zero was requested.
    DivisionByZero = 3,
    /// An exact quotient has a non-terminating decimal expansion.
    RepeatingDecimal = 4,
    /// An exact terminating quotient needs more than 28 decimal places.
    ExactDivisionScaleOverflow = 5,
    /// A requested output scale is outside `0..=28`.
    InvalidScale = 6,
    /// An exact conversion would discard a fractional component or exceed its target.
    InexactConversion = 7,
    /// A negative value was converted to the nominal quantity domain.
    NegativeQuantity = 8,
    /// Quantity subtraction would produce a negative value.
    QuantityUnderflow = 9,
    /// A rounded operation received an unknown rounding-mode tag.
    InvalidRoundingMode = 10,
    /// A fallible operation received an unknown failure-mode tag.
    InvalidFailureMode = 11,
    /// A register required to be zero by the syscall contract was nonzero.
    ReservedRegisterNonZero = 12,
}

impl NumericFaultV1 {
    /// Decode a stable ABI tag.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            1 => Self::MantissaOverflow,
            2 => Self::ScaleOverflow,
            3 => Self::DivisionByZero,
            4 => Self::RepeatingDecimal,
            5 => Self::ExactDivisionScaleOverflow,
            6 => Self::InvalidScale,
            7 => Self::InexactConversion,
            8 => Self::NegativeQuantity,
            9 => Self::QuantityUnderflow,
            10 => Self::InvalidRoundingMode,
            11 => Self::InvalidFailureMode,
            12 => Self::ReservedRegisterNonZero,
            _ => return None,
        })
    }

    /// Return the stable ABI tag.
    #[must_use]
    pub const fn tag(self) -> u64 {
        self as u64
    }
}

/// Stable rounding-mode tags supplied to rounded decimal operations in `r13`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum RoundingModeV1 {
    /// Truncate toward zero.
    TowardZero = 0,
    /// Round away from zero whenever the discarded remainder is nonzero.
    AwayFromZero = 1,
    /// Round toward negative infinity.
    Floor = 2,
    /// Round toward positive infinity.
    Ceil = 3,
    /// Round to nearest, resolving ties toward an even mantissa.
    NearestEven = 4,
    /// Round to nearest, resolving ties away from zero.
    NearestAway = 5,
    /// Round to nearest, resolving ties toward zero.
    NearestTowardZero = 6,
}

impl RoundingModeV1 {
    /// Decode a stable ABI tag.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            0 => Self::TowardZero,
            1 => Self::AwayFromZero,
            2 => Self::Floor,
            3 => Self::Ceil,
            4 => Self::NearestEven,
            5 => Self::NearestAway,
            6 => Self::NearestTowardZero,
            _ => return None,
        })
    }

    /// Return the stable ABI tag.
    #[must_use]
    pub const fn tag(self) -> u64 {
        self as u64
    }
}

/// Stable pointer/envelope validation fault codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum PointerAbiFaultV1 {
    /// The guest address does not identify public readable memory.
    InvalidAddress = 1,
    /// The pointer type identifier is unknown.
    UnknownType = 2,
    /// The pointer type is known but disallowed by ABI V1.
    TypeNotAllowed = 3,
    /// The pointer has a known but unexpected type.
    WrongType = 4,
    /// The outer envelope version is unsupported.
    InvalidEnvelopeVersion = 5,
    /// A declared length exceeds the hard bound for its type.
    OversizedLength = 6,
    /// The declared envelope is truncated or its length arithmetic overflows.
    TruncatedEnvelope = 7,
    /// The payload digest does not authenticate the snapshotted payload.
    PayloadHashMismatch = 8,
    /// The schema-bound Norito frame is malformed or uses invalid flags.
    MalformedFrame = 9,
    /// The schema hash does not match the pointer type's V1 schema.
    SchemaMismatch = 10,
    /// The value has a non-minimal or otherwise noncanonical representation.
    NonCanonical = 11,
}

impl PointerAbiFaultV1 {
    /// Decode a stable ABI tag.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            1 => Self::InvalidAddress,
            2 => Self::UnknownType,
            3 => Self::TypeNotAllowed,
            4 => Self::WrongType,
            5 => Self::InvalidEnvelopeVersion,
            6 => Self::OversizedLength,
            7 => Self::TruncatedEnvelope,
            8 => Self::PayloadHashMismatch,
            9 => Self::MalformedFrame,
            10 => Self::SchemaMismatch,
            11 => Self::NonCanonical,
            _ => return None,
        })
    }

    /// Return the stable ABI tag.
    #[must_use]
    pub const fn tag(self) -> u64 {
        self as u64
    }
}

/// Result pointer/value register for numeric syscalls.
pub const NUMERIC_RESULT_REGISTER: usize = 10;
/// Status register: zero on success, otherwise a [`NumericFaultV1`] tag.
pub const NUMERIC_STATUS_REGISTER: usize = 11;
/// Requested decimal scale register for rounded operations.
pub const NUMERIC_SCALE_REGISTER: usize = 12;
/// Rounding-mode register for rounded operations.
pub const NUMERIC_ROUNDING_REGISTER: usize = 13;
/// Failure-mode register for arithmetic operations: zero traps, one returns status.
pub const NUMERIC_FAILURE_MODE_REGISTER: usize = 14;
/// Trap on an arithmetic-domain failure.
pub const NUMERIC_FAILURE_TRAP: u64 = 0;
/// Return an arithmetic-domain failure in `r11` without trapping.
pub const NUMERIC_FAILURE_STATUS: u64 = 1;

#[cfg(test)]
mod tests {
    use super::{NumericFaultV1, PointerAbiFaultV1, RoundingModeV1};

    #[test]
    fn numeric_fault_tags_are_complete_and_stable() {
        for tag in 1..=12 {
            assert_eq!(
                NumericFaultV1::from_tag(tag).map(NumericFaultV1::tag),
                Some(tag)
            );
        }
        assert_eq!(NumericFaultV1::from_tag(0), None);
        assert_eq!(NumericFaultV1::from_tag(13), None);
    }

    #[test]
    fn rounding_tags_are_complete_and_stable() {
        for tag in 0..=6 {
            assert_eq!(
                RoundingModeV1::from_tag(tag).map(RoundingModeV1::tag),
                Some(tag)
            );
        }
        assert_eq!(RoundingModeV1::from_tag(7), None);
    }

    #[test]
    fn pointer_fault_tags_are_complete_and_stable() {
        for tag in 1..=11 {
            assert_eq!(
                PointerAbiFaultV1::from_tag(tag).map(PointerAbiFaultV1::tag),
                Some(tag)
            );
        }
        assert_eq!(PointerAbiFaultV1::from_tag(0), None);
        assert_eq!(PointerAbiFaultV1::from_tag(12), None);
    }
}
