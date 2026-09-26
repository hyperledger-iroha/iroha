//! Frozen economics for public standalone conviction voting.
//!
//! These arithmetic rules do not authenticate a voter, confidential bond or private ballot.
//! Anonymous elections still require their own reviewed credential and closed-corpus relation.

use crate::{account::AccountId, asset::AssetDefinitionId};
use iroha_primitives::numeric::{MAX_DECIMAL_SCALE, Quantity};
use norito::codec::{Decode, Encode};

/// Required context discriminator; missing context is not a first-release wire layout.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(tag = "kind", content = "content", deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::conviction::PlainVotingContextV1")]
pub enum PlainVotingContextV1 {
    /// The referendum is proof-backed; this public PLAIN context does not apply.
    NotApplicable,
    /// Immutable public-ballot economics supplied when the referendum is created.
    Conviction(PlainConvictionPolicyV1),
}

/// Exact asset dimension, weighting policy and custody frozen before any public ballot.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::conviction::PlainConvictionPolicyV1")]
pub struct PlainConvictionPolicyV1 {
    /// Canonical asset whose actual escrowed quantity backs the ballot.
    pub asset_definition_id: AssetDefinitionId,
    /// Decimal scale defining one smallest voting unit, fixed in the election context.
    pub asset_scale: u32,
    /// Positive lock-duration step in blocks.
    pub conviction_step_blocks: u64,
    /// Positive cap on the conviction multiplier.
    pub max_conviction: u64,
    /// Positive numerator of the inclusive approval threshold, frozen before voting.
    pub approval_threshold_numerator: u64,
    /// Nonzero denominator of the inclusive approval threshold.
    pub approval_threshold_denominator: u64,
    /// Minimum exact total conviction weight required for approval.
    pub minimum_turnout: u128,
    /// Minimum quantity; zero does not waive escrow for a positive bond.
    pub minimum_bond: Quantity,
    /// Account holding every positive public voting bond.
    pub bond_escrow_account: AccountId,
    /// Account receiving a slash under the existing governance custody protocol.
    pub slash_receiver_account: AccountId,
}

/// Exact immutable public result retained after voting closes and custody unlocks.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::conviction::PlainVotingDecisionV1")]
pub struct PlainVotingDecisionV1 {
    /// Total affirmative weight.
    pub approve: u128,
    /// Total negative weight.
    pub reject: u128,
    /// Total abstention weight.
    pub abstain: u128,
    /// Decision under the frozen threshold and turnout policy.
    pub approved: bool,
}
/// Required lifecycle discriminator; closed public results cannot be recomputed from released locks.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(tag = "kind", content = "content", deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::conviction::PlainVotingResultV1")]
pub enum PlainVotingResultV1 {
    /// Proof-backed referendum outside the public result protocol.
    NotApplicable,
    /// Public referendum has not closed.
    Pending,
    /// Immutable public tally and decision recorded before custody release.
    Decided(PlainVotingDecisionV1),
}

/// A frozen context or proposed conviction update is outside its exact integer domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvictionErrorV1 {
    /// Scale, conviction parameters, or approval threshold are invalid.
    #[error("invalid frozen conviction policy")]
    InvalidPolicy,
    /// The quantity has fractional units at the frozen asset scale.
    #[error("bond is not representable at the frozen asset scale")]
    FractionalUnits,
    /// Unit conversion or weight exceeds the exact `u128` domain.
    #[error("conviction units or weight exceed u128")]
    Overflow,
    /// A replacement reduces its amount, requested duration or absolute expiry.
    #[error("conviction update cannot reduce existing lock amount, duration or expiry")]
    ReducedLock,
    /// A replacement increases neither its bond nor its absolute expiry.
    #[error("conviction update must increase the bond or extend its lock")]
    UnchangedLock,
}

impl PlainConvictionPolicyV1 {
    /// Check bounded parameters and the minimum's exact unit representation.
    ///
    /// # Errors
    /// Rejects invalid scale or conviction parameters, a zero approval threshold,
    /// or an unrepresentable minimum.
    pub fn validate(&self) -> Result<(), ConvictionErrorV1> {
        if self.asset_scale > MAX_DECIMAL_SCALE
            || self.conviction_step_blocks == 0
            || self.max_conviction == 0
            || self.approval_threshold_denominator == 0
            || self.approval_threshold_numerator == 0
            || self.approval_threshold_numerator > self.approval_threshold_denominator
        {
            return Err(ConvictionErrorV1::InvalidPolicy);
        }
        self.units(&self.minimum_bond)?;
        Ok(())
    }

    /// Decide an exact final tally using only this election's frozen policy.
    ///
    /// # Errors
    /// Rejects invalid policy or aggregate overflow.
    pub fn decide(&self, tally: [u128; 3]) -> Result<PlainVotingDecisionV1, ConvictionErrorV1> {
        self.validate()?;
        let [approve, reject, abstain] = tally;
        let decisive = approve
            .checked_add(reject)
            .ok_or(ConvictionErrorV1::Overflow)?;
        let turnout = decisive
            .checked_add(abstain)
            .ok_or(ConvictionErrorV1::Overflow)?;
        let wide = |value: u128, factor: u64| {
            let low_word = |word: u128| {
                u64::try_from(word & u128::from(u64::MAX)).expect("masked word fits exactly in u64")
            };
            let low = u128::from(low_word(value)) * u128::from(factor);
            let high = (value >> 64) * u128::from(factor) + (low >> 64);
            [low_word(high >> 64), low_word(high), low_word(low)]
        };
        Ok(PlainVotingDecisionV1 {
            approve,
            reject,
            abstain,
            approved: turnout >= self.minimum_turnout
                && decisive != 0
                && wide(approve, self.approval_threshold_denominator)
                    >= wide(decisive, self.approval_threshold_numerator),
        })
    }

    /// Convert a canonical quantity to exact smallest units without rounding.
    ///
    /// # Errors
    /// Rejects an invalid frozen scale, fractional smallest units or `u128` overflow.
    pub fn units(&self, amount: &Quantity) -> Result<u128, ConvictionErrorV1> {
        if self.asset_scale > MAX_DECIMAL_SCALE {
            return Err(ConvictionErrorV1::InvalidPolicy);
        }
        let exponent = self
            .asset_scale
            .checked_sub(amount.scale())
            .ok_or(ConvictionErrorV1::FractionalUnits)?;
        let mantissa = amount
            .as_numeric()
            .try_mantissa_u128()
            .ok_or(ConvictionErrorV1::Overflow)?;
        mantissa
            .checked_mul(10_u128.pow(exponent))
            .ok_or(ConvictionErrorV1::Overflow)
    }

    /// Compute `floor(sqrt(units)) * min(1 + duration / step, maximum)` exactly.
    ///
    /// # Errors
    /// Rejects an invalid policy or unrepresentable quantity/weight.
    pub fn weight(&self, amount: &Quantity, duration: u64) -> Result<u128, ConvictionErrorV1> {
        self.validate()?;
        let units = self.units(amount)?;
        conviction_weight_from_units_v1(
            units,
            duration,
            self.conviction_step_blocks,
            self.max_conviction,
        )
    }
}

/// Compute exact conviction weight from an asset's smallest units.
///
/// The caller must authenticate the election, asset scale, bond, ballot and proof separately.
///
/// # Errors
/// Returns an invalid-policy error for a zero step or maximum, or an overflow error if the
/// checked weight exceeds `u128`.
pub fn conviction_weight_from_units_v1(
    units: u128,
    duration_blocks: u64,
    step_blocks: u64,
    max_conviction: u64,
) -> Result<u128, ConvictionErrorV1> {
    if step_blocks == 0 || max_conviction == 0 {
        return Err(ConvictionErrorV1::InvalidPolicy);
    }
    let base = integer_sqrt(units);
    let factor = (u128::from(duration_blocks / step_blocks) + 1).min(u128::from(max_conviction));
    base.checked_mul(factor).ok_or(ConvictionErrorV1::Overflow)
}

/// Check the strict increase and nondecrease rules for an existing conviction position.
///
/// Choice and owner checks are separate authenticated host/proof obligations. Requiring the
/// requested duration to remain nondecreasing prevents a later-height update from reducing the
/// multiplier while reusing the same absolute expiry.
///
/// # Errors
/// Rejects reduced amount/duration/expiry or a replacement with no actual bond/expiry increase.
pub fn validate_conviction_update_v1(
    previous_amount: &Quantity,
    previous_duration: u64,
    previous_expiry: u64,
    next_amount: &Quantity,
    next_duration: u64,
    next_expiry: u64,
) -> Result<(), ConvictionErrorV1> {
    if next_amount < previous_amount
        || next_duration < previous_duration
        || next_expiry < previous_expiry
    {
        return Err(ConvictionErrorV1::ReducedLock);
    }
    if next_amount == previous_amount && next_expiry == previous_expiry {
        return Err(ConvictionErrorV1::UnchangedLock);
    }
    Ok(())
}

fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut root = value;
    loop {
        let next = u128::midpoint(root, value / root);
        if next >= root {
            return root;
        }
        root = next;
    }
}

#[cfg(test)]
mod tests;
