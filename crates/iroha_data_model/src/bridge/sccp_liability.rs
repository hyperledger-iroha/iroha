//! Canonical SORA-side SCCP route liability accounting.

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Outstanding SORA-home asset liability held by one immutable SCCP route revision.
///
/// The value is expressed in the route's canonical unsigned payload units. A
/// persisted row must therefore be nonzero; routes with no liability omit the
/// row entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpRouteLiabilityV1 {
    /// Exact outstanding liability in canonical route payload units.
    pub outstanding_liability: u128,
}

impl SccpRouteLiabilityV1 {
    /// Construct a canonical nonzero liability row.
    #[must_use]
    pub const fn new(outstanding_liability: u128) -> Option<Self> {
        if outstanding_liability == 0 {
            None
        } else {
            Some(Self {
                outstanding_liability,
            })
        }
    }

    /// Add an outbound lock using checked arithmetic and an immutable route cap.
    #[must_use]
    pub const fn checked_credit(self, amount: u128, maximum: u128) -> Option<Self> {
        if amount == 0 || maximum == 0 {
            return None;
        }
        let outstanding_liability = match self.outstanding_liability.checked_add(amount) {
            Some(value) if value <= maximum => value,
            _ => return None,
        };
        Self::new(outstanding_liability)
    }

    /// Remove a validated external burn using checked arithmetic.
    ///
    /// The outer `None` reports invalid zero input or underflow. The inner
    /// `None` is the canonical empty result, which callers represent by removing
    /// the persisted row.
    #[must_use]
    pub const fn checked_debit(self, amount: u128) -> Option<Option<Self>> {
        if amount == 0 {
            return None;
        }
        let remaining = match self.outstanding_liability.checked_sub(amount) {
            Some(value) => value,
            None => return None,
        };
        Some(Self::new(remaining))
    }

    /// Return whether this value is a canonical persisted row.
    #[must_use]
    pub const fn is_well_formed(self) -> bool {
        self.outstanding_liability != 0
    }
}

#[cfg(test)]
mod tests {
    use super::SccpRouteLiabilityV1;

    #[test]
    fn liability_credit_is_checked_and_capped() {
        let liability = SccpRouteLiabilityV1::new(7).expect("nonzero liability");
        assert_eq!(
            liability.checked_credit(5, 12),
            SccpRouteLiabilityV1::new(12)
        );
        assert_eq!(liability.checked_credit(6, 12), None);
        assert_eq!(
            SccpRouteLiabilityV1::new(u128::MAX)
                .expect("nonzero liability")
                .checked_credit(1, u128::MAX),
            None
        );
        assert_eq!(liability.checked_credit(0, 12), None);
    }

    #[test]
    fn liability_debit_rejects_underflow_and_omits_zero() {
        let liability = SccpRouteLiabilityV1::new(7).expect("nonzero liability");
        assert_eq!(
            liability.checked_debit(2),
            Some(SccpRouteLiabilityV1::new(5))
        );
        assert_eq!(liability.checked_debit(7), Some(None));
        assert_eq!(liability.checked_debit(8), None);
        assert_eq!(liability.checked_debit(0), None);
        assert_eq!(SccpRouteLiabilityV1::new(0), None);
    }
}
