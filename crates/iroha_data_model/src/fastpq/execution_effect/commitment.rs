//! Constant-space ordered commitments over the sole canonical effect frame.
//!
//! Borrowed inputs are encode-only projections of the original nominal DTOs.
//! They provide no source authority, retained ownership or alternate decoder.

use super::*;

/// Borrowed exact lifecycle and definition supplied by the original operation.
#[derive(Clone, Copy)]
pub struct FastpqExecutionAssetRefV1<'a> {
    /// Canonical definition identity.
    pub definition: &'a AssetDefinitionId,
    /// Original registry incarnation.
    pub incarnation: AxtAssetIncarnationV1,
}
/// Borrowed exact physical balance identity.
#[derive(Clone, Copy)]
pub struct FastpqExecutionBalanceRefV1<'a> {
    /// Exact asset and lifecycle.
    pub asset: FastpqExecutionAssetRefV1<'a>,
    /// Original canonical controller.
    pub account: &'a AccountId,
    /// Original resolved storage scope.
    pub scope: AssetBalanceScope,
}
/// Borrowed original arithmetic values, including sequential self-transfer states.
#[derive(Clone, Copy)]
pub struct FastpqExecutionTransferRefV1<'a> {
    /// Sender bucket.
    pub source: FastpqExecutionBalanceRefV1<'a>,
    /// Receiver bucket.
    pub destination: FastpqExecutionBalanceRefV1<'a>,
    /// Exact amount.
    pub amount: &'a Quantity,
    /// Sender before debit.
    pub source_before: &'a Quantity,
    /// Sender after debit.
    pub source_after: &'a Quantity,
    /// Receiver before credit.
    pub destination_before: &'a Quantity,
    /// Receiver after credit.
    pub destination_after: &'a Quantity,
}
/// Borrowed original balance and circulating-supply values.
#[derive(Clone, Copy)]
pub struct FastpqExecutionSupplyChangeRefV1<'a> {
    /// Exact affected balance.
    pub balance: FastpqExecutionBalanceRefV1<'a>,
    /// Exact amount.
    pub amount: &'a Quantity,
    /// Balance before mutation.
    pub balance_before: &'a Quantity,
    /// Balance after mutation.
    pub balance_after: &'a Quantity,
    /// Supply before mutation.
    pub supply_before: &'a Quantity,
    /// Supply after mutation.
    pub supply_after: &'a Quantity,
}
/// Borrowed operation with the same four tags as the sole owned effect kind.
#[derive(Clone, Copy)]
pub enum FastpqExecutionEffectKindRefV1<'a> {
    /// Transfer exact original values.
    Transfer(FastpqExecutionTransferRefV1<'a>),
    /// Mint exact original values.
    Mint(FastpqExecutionSupplyChangeRefV1<'a>),
    /// Burn exact original values.
    Burn(FastpqExecutionSupplyChangeRefV1<'a>),
    /// Retire the exact original zero-supply lifecycle.
    Retire(FastpqExecutionAssetRefV1<'a>),
}
/// Borrowed complete effect; no source authentication is implied.
#[derive(Clone, Copy)]
pub struct FastpqExecutionEffectRefV1<'a> {
    /// Contiguous zero-based executed ordinal.
    pub ordinal: u32,
    /// Exact original authority-set digest.
    pub authority_digest: Hash,
    /// Exact original authorization/purpose context.
    pub authorization_context: Hash,
    /// Complete operation and original quantities.
    pub kind: FastpqExecutionEffectKindRefV1<'a>,
}

impl<'a> From<&'a FastpqExecutionAssetV1> for FastpqExecutionAssetRefV1<'a> {
    fn from(value: &'a FastpqExecutionAssetV1) -> Self {
        Self {
            definition: &value.definition,
            incarnation: value.incarnation,
        }
    }
}
impl<'a> From<&'a FastpqExecutionBalanceV1> for FastpqExecutionBalanceRefV1<'a> {
    fn from(value: &'a FastpqExecutionBalanceV1) -> Self {
        Self {
            asset: (&value.asset).into(),
            account: &value.account,
            scope: value.scope,
        }
    }
}
impl<'a> From<&'a FastpqExecutionEffectV1> for FastpqExecutionEffectRefV1<'a> {
    fn from(value: &'a FastpqExecutionEffectV1) -> Self {
        let kind = match &value.kind {
            FastpqExecutionEffectKindV1::Transfer(v) => {
                FastpqExecutionEffectKindRefV1::Transfer(FastpqExecutionTransferRefV1 {
                    source: (&v.source).into(),
                    destination: (&v.destination).into(),
                    amount: &v.amount,
                    source_before: &v.source_before,
                    source_after: &v.source_after,
                    destination_before: &v.destination_before,
                    destination_after: &v.destination_after,
                })
            }
            FastpqExecutionEffectKindV1::Mint(v) | FastpqExecutionEffectKindV1::Burn(v) => {
                let borrowed = FastpqExecutionSupplyChangeRefV1 {
                    balance: (&v.balance).into(),
                    amount: &v.amount,
                    balance_before: &v.balance_before,
                    balance_after: &v.balance_after,
                    supply_before: &v.supply_before,
                    supply_after: &v.supply_after,
                };
                if matches!(&value.kind, FastpqExecutionEffectKindV1::Mint(_)) {
                    FastpqExecutionEffectKindRefV1::Mint(borrowed)
                } else {
                    FastpqExecutionEffectKindRefV1::Burn(borrowed)
                }
            }
            FastpqExecutionEffectKindV1::Retire(v) => {
                FastpqExecutionEffectKindRefV1::Retire(v.into())
            }
        };
        Self {
            ordinal: value.ordinal,
            authority_digest: value.authority_digest,
            authorization_context: value.authorization_context,
            kind,
        }
    }
}

// Payload-only forwarding, matching the existing SignedBlock borrowed projection.
// References never become nested schema identities or separate wire fields.
struct Field<'a, T>(&'a T);
impl<T: norito::SerializePayload> norito::SerializePayload for Field<'_, T> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::SerializePayload::serialize(self.0, writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[derive(norito::Encode)]
struct Asset<'a> {
    definition: Field<'a, AssetDefinitionId>,
    incarnation: AxtAssetIncarnationV1,
}
#[derive(norito::Encode)]
struct Balance<'a> {
    asset: Asset<'a>,
    account: Field<'a, AccountId>,
    scope: AssetBalanceScope,
}
#[derive(norito::Encode)]
struct Transfer<'a> {
    source: Balance<'a>,
    destination: Balance<'a>,
    amount: Field<'a, Quantity>,
    source_before: Field<'a, Quantity>,
    source_after: Field<'a, Quantity>,
    destination_before: Field<'a, Quantity>,
    destination_after: Field<'a, Quantity>,
}
#[derive(norito::Encode)]
struct Supply<'a> {
    balance: Balance<'a>,
    amount: Field<'a, Quantity>,
    balance_before: Field<'a, Quantity>,
    balance_after: Field<'a, Quantity>,
    supply_before: Field<'a, Quantity>,
    supply_after: Field<'a, Quantity>,
}
#[derive(norito::Encode)]
enum Kind<'a> {
    #[codec(index = 0)]
    Transfer(Transfer<'a>),
    #[codec(index = 1)]
    Mint(Supply<'a>),
    #[codec(index = 2)]
    Burn(Supply<'a>),
    #[codec(index = 3)]
    Retire(Asset<'a>),
}
#[derive(norito::Encode)]
struct Effect<'a> {
    ordinal: u32,
    authority_digest: Hash,
    authorization_context: Hash,
    kind: Kind<'a>,
}
// The framed root uses Rust alignment for its canonical payload padding.
const _: () = assert!(
    std::mem::align_of::<Effect<'static>>() == std::mem::align_of::<FastpqExecutionEffectV1>()
);

impl norito::NoritoSchema for Effect<'_> {
    fn nominal_name() -> String {
        <FastpqExecutionEffectV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <FastpqExecutionEffectV1 as norito::NoritoSchema>::frame_name()
    }
    fn static_nominal_name() -> Option<&'static str> {
        <FastpqExecutionEffectV1 as norito::NoritoSchema>::static_nominal_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        <FastpqExecutionEffectV1 as norito::NoritoSchema>::static_frame_name()
    }
}
impl<'a> From<FastpqExecutionAssetRefV1<'a>> for Asset<'a> {
    fn from(v: FastpqExecutionAssetRefV1<'a>) -> Self {
        Self {
            definition: Field(v.definition),
            incarnation: v.incarnation,
        }
    }
}
impl<'a> From<FastpqExecutionBalanceRefV1<'a>> for Balance<'a> {
    fn from(v: FastpqExecutionBalanceRefV1<'a>) -> Self {
        Self {
            asset: v.asset.into(),
            account: Field(v.account),
            scope: v.scope,
        }
    }
}
impl<'a> From<FastpqExecutionSupplyChangeRefV1<'a>> for Supply<'a> {
    fn from(v: FastpqExecutionSupplyChangeRefV1<'a>) -> Self {
        Self {
            balance: v.balance.into(),
            amount: Field(v.amount),
            balance_before: Field(v.balance_before),
            balance_after: Field(v.balance_after),
            supply_before: Field(v.supply_before),
            supply_after: Field(v.supply_after),
        }
    }
}
impl<'a> From<FastpqExecutionEffectRefV1<'a>> for Effect<'a> {
    fn from(v: FastpqExecutionEffectRefV1<'a>) -> Self {
        let kind = match v.kind {
            FastpqExecutionEffectKindRefV1::Transfer(v) => Kind::Transfer(Transfer {
                source: v.source.into(),
                destination: v.destination.into(),
                amount: Field(v.amount),
                source_before: Field(v.source_before),
                source_after: Field(v.source_after),
                destination_before: Field(v.destination_before),
                destination_after: Field(v.destination_after),
            }),
            FastpqExecutionEffectKindRefV1::Mint(v) => Kind::Mint(v.into()),
            FastpqExecutionEffectKindRefV1::Burn(v) => Kind::Burn(v.into()),
            FastpqExecutionEffectKindRefV1::Retire(v) => Kind::Retire(v.into()),
        };
        Self {
            ordinal: v.ordinal,
            authority_digest: v.authority_digest,
            authorization_context: v.authorization_context,
            kind,
        }
    }
}

/// Fixed-size, nonserialized accumulator for the sole ordered source digest.
///
/// This value is freely computable and grants no execution or finality authority.
/// Production ownership and atomic publication belong to the original executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastpqExecutionEffectCommitmentV1 {
    context: FastpqExecutionEffectContextV1,
    count: u32,
    chain: Hash,
}
impl FastpqExecutionEffectCommitmentV1 {
    /// Start an empty journal for the exact independently selected source context.
    #[must_use]
    pub fn new(context: FastpqExecutionEffectContextV1) -> Self {
        Self {
            context,
            count: 0,
            chain: Hash::new(b"fastpq:execution-effects:v1:empty|"),
        }
    }
    /// Borrow the original source context without altering the chain.
    #[must_use]
    pub const fn context(&self) -> &FastpqExecutionEffectContextV1 {
        &self.context
    }
    /// Return the exact contiguous effect count.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
    /// Append the complete borrowed canonical effect in its exact executed position.
    ///
    /// # Errors
    /// Refuses ordinal mismatch, count overflow, or canonical framing failure.
    /// Failure leaves both original count and chain unchanged.
    pub fn append(&mut self, effect: FastpqExecutionEffectRefV1<'_>) -> Result<(), norito::Error> {
        if effect.ordinal != self.count {
            return Err(norito::Error::NonCanonicalEncoding);
        }
        let next_count = self
            .count
            .checked_add(1)
            .ok_or(norito::Error::LengthMismatch)?;
        let effect_digest = canonical_effect_digest(
            b"fastpq:execution-effects:v1:effect|",
            &Effect::from(effect),
        )?;
        let chain = Hash::new_from_chunks(&[
            b"fastpq:execution-effects:v1:step|",
            self.chain.as_ref(),
            &self.count.to_le_bytes(),
            effect_digest.as_ref(),
        ]);
        self.chain = chain;
        self.count = next_count;
        Ok(())
    }
    /// Bind the complete source context, exact count and final ordered chain.
    ///
    /// # Errors
    /// Returns the canonical context encoding failure without a partial digest.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        let context =
            canonical_effect_digest(b"fastpq:execution-effects:v1:context|", &self.context)?;
        Ok(Hash::new_from_chunks(&[
            b"fastpq:execution-effects:v1:source|",
            context.as_ref(),
            &self.count.to_le_bytes(),
            self.chain.as_ref(),
        ]))
    }
}

#[cfg(test)]
pub(super) mod tests;
