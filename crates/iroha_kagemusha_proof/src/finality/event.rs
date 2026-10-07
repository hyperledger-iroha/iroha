//! Bind a parsed Load receipt to the exact native typed-event hash preimage.

use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    GlueChip, UintChip, Word,
    blake2b::{Blake2bChip, Blake2bDigest},
    cells::low_u128,
    statement::assign_canonical_limbs,
};

use super::LoadReceiptCells;

// Native `encode_adaptive_into(EventBox::Data(KagemushaLoadCommittedV1))`.
// This is the HashOf<EventBox> preimage, not a top-level canonical Norito frame.
// Enum tags, nested compact lengths and the sole fixed32 field are invariant;
// the shared fixture pins this prefix against the native codec.
pub const PREFIX: [u8; 13] = [2, 0, 0, 0, 40, 39, 38, 24, 0, 0, 0, 33, 32];

/// Exact canonical event encoding and marked `BLAKE2b` hash of the parsed receipt.
/// Inclusion and validator finality must still be proved by the enclosing source.
#[derive(Clone, Debug)]
pub struct LoadEventCells {
    preimage: [Word<Fp>; 45],
    hash: Blake2bDigest<Fp>,
}

impl LoadEventCells {
    /// Derive the event directly from the receipt's constrained transcript digest.
    ///
    /// Canonical integer decomposition rules out a `digest + p` byte alias before
    /// the native event hash is computed. No caller-supplied event fields or digest
    /// are accepted separately from the original receipt tape.
    /// # Errors
    /// Returns layout errors.
    pub fn from_receipt(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        receipt: &LoadReceiptCells,
    ) -> Result<Self, Error> {
        Self::from_digest(uint, blake, region, receipt.digest())
    }

    fn from_digest(
        uint: &mut UintChip<'_, Fp>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        digest: &Word<Fp>,
    ) -> Result<Self, Error> {
        let limbs = assign_canonical_limbs(uint, region, digest)?;
        let mut preimage = PREFIX
            .iter()
            .map(|byte| uint.glue().constant(region, Fp::from(u64::from(*byte))))
            .collect::<Result<Vec<_>, _>>()?;
        for limb in limbs.words() {
            let mut packed = uint.glue().constant(region, Fp::ZERO)?;
            for index in 0..16 {
                let byte = uint.assign::<8>(
                    region,
                    limb.value().map(|v| (low_u128(&v) >> (8 * index)) & 255),
                )?;
                packed = uint.glue().linear(
                    region,
                    &[
                        (Fp::ONE, &packed),
                        (Fp::from_u128(1_u128 << (8 * index)), byte.word()),
                    ],
                    Fp::ZERO,
                )?;
                preimage.push(byte.word().clone());
            }
            GlueChip::assert_equal(region, &packed, limb)?;
        }
        let preimage: [Word<Fp>; 45] = preimage.try_into().map_err(|_| Error::Synthesis)?;
        let hash = blake.hash_marked(region, &preimage)?;
        Ok(Self { preimage, hash })
    }

    /// Native `HashOf<EventBox>` preimage, including the exact nested variant tags.
    pub const fn preimage(&self) -> &[Word<Fp>; 45] {
        &self.preimage
    }

    /// Typed leaf hash to consume with the ordinary counted application Merkle chip.
    pub const fn hash(&self) -> &Blake2bDigest<Fp> {
        &self.hash
    }
}

#[cfg(test)]
mod tests;
