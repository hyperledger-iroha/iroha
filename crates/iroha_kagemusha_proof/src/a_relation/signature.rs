//! Signature public inputs extracted only from a hard-verified, admitted Q.

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, Word, ff::Nat, p256::VerifyMode};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{AProofPlan, VerifiedQCells, bounded_word};
use crate::q_signature::{QSignaturePlan, SLOT_WORDS, SignatureKey};

/// The exact message, raw unsigned key/signature and verdict proved by one Q
/// slot. No constructor from arbitrary scalar cells is exposed.
#[must_use = "bind the message, key and signature to the authenticated object and include its soft verdict"]
#[derive(Clone, Debug)]
pub struct SignatureProofCells {
    message: Word<Fp>,
    key: [Word<Fp>; 4],
    signature: [Word<Fp>; 4],
    valid: Bit<Fp>,
    key_policy: SignatureKey,
}
impl SignatureProofCells {
    /// Canonical Fp signing-message digest, before SHA-256.
    pub const fn message(&self) -> &Word<Fp> {
        &self.message
    }
    /// Exact raw x/y as low128/high128, including bit255.
    pub const fn key(&self) -> &[Word<Fp>; 4] {
        &self.key
    }
    /// Exact raw r/s as low128/high128, including bit255.
    pub const fn signature(&self) -> &[Word<Fp>; 4] {
        &self.signature
    }
    /// The proved exact ECDSA verdict; hard slots are constrained true.
    pub const fn valid(&self) -> &Bit<Fp> {
        &self.valid
    }
    /// Immutable key policy enforced while extracting this admitted Q slot.
    /// Fixed coordinates are constrained to constants, never witness-selected.
    pub const fn key_policy(&self) -> SignatureKey {
        self.key_policy
    }
}

/// Bind an admitted hard Q output to a fixed signature schema and extract its
/// slots. The exact key digest, index and one-Bounded-column descriptor must
/// agree with the operation plan. Raw limbs retain all128bits and may encode
/// invalid P-256 coordinates or signatures in soft slots.
///
/// # Errors
/// Wrong Q index/schema/column count or layout failure. Wrong key identity,
/// fixed key limbs, nonboolean verdicts and false hard verdicts are unsatisfiable.
pub fn bind_signature_q(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    operation: &AProofPlan,
    index: usize,
    schema: &QSignaturePlan,
    verified: &VerifiedQCells,
) -> Result<Vec<SignatureProofCells>, Error> {
    let fixed = operation.q(index).ok_or(Error::Synthesis)?;
    let descriptor = fixed.verifier().binding().descriptor();
    if verified.index != index
        || descriptor.instance_lengths
            != [u32::try_from(schema.instance_length()).map_err(|_| Error::BoundsFailure)?]
        || descriptor.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
        || verified.instances.len() != 1
        || verified.instances[0].len() != schema.instance_length()
    {
        return Err(Error::Synthesis);
    }
    GlueChip::assert_constant(
        region,
        &verified.key_digest,
        fixed
            .key
            .kagemusha_digest(fixed.verifier().binding())
            .map_err(|_| Error::Synthesis)?,
    )?;
    let mut out = Vec::with_capacity(schema.slots().len());
    for (slot, values) in schema
        .slots()
        .iter()
        .zip(verified.instances[0].chunks_exact(SLOT_WORDS))
    {
        let words = values
            .iter()
            .map(|scalar| bounded_word(chip, region, scalar))
            .collect::<Result<Vec<_>, _>>()?;
        for word in &words[1..9] {
            chip.uint().range_check::<128>(region, word)?;
        }
        let valid = chip.uint().glue().assert_bool(region, &words[9])?;
        if slot.mode == VerifyMode::Hard {
            GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        }
        if let SignatureKey::Fixed(key) = slot.key {
            for (words, coordinate) in words[1..5].chunks_exact(2).zip([key.x, key.y]) {
                let coordinate = Nat::from_words(coordinate);
                for (word, integer) in words
                    .iter()
                    .zip([coordinate.low_u128(), coordinate.shr(128).low_u128()])
                {
                    GlueChip::assert_constant(region, word, Fp::from_u128(integer))?;
                }
            }
        }
        out.push(SignatureProofCells {
            message: words[0].clone(),
            key: core::array::from_fn(|i| words[1 + i].clone()),
            signature: core::array::from_fn(|i| words[5 + i].clone()),
            valid,
            key_policy: slot.key,
        });
    }
    Ok(out)
}
