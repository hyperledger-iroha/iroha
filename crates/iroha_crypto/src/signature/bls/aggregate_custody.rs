//! Explicit caller-owned pairing scratch for the canonical normal aggregate relation.

use blst::{Pairing, blst_fp12, blst_p2_affine};
use blstrs::{G1Projective, G2Projective};
use group::{Curve as _, Group as _};

use super::uncached::{self, HASH_TO_FIELD_DST, NORMAL_PREFIX, Orientation};
use crate::{BlsNormalPopVerifiedKey, SignatureVerificationError};

/// Reusable normal-BLS aggregate verifier with one explicitly admitted heap owner.
///
/// The caller supplies the custody token for the exact pairing-context backing.
/// This token remains owned through every verdict and is released only after that
/// backing is dropped. Verification borrows the PoP-verified keys and allocates
/// no heap collections, prepared-point buffers, signature copies or cache entries.
/// The constructor uses the allocator normally after logical admission; physical
/// allocator exhaustion is not represented by its admission error.
#[derive(Debug)]
#[must_use = "dropping the verifier releases its original pairing backing and custody token"]
pub struct BlsNormalAggregateScratch<O = ()> {
    // Declaration order is intentional: release the backing before its funding.
    pairing: Pairing,
    _owner: O,
}

impl<O> BlsNormalAggregateScratch<O> {
    /// Exact heap backing bytes allocated by the existing `blst::Pairing` owner.
    #[must_use]
    #[allow(unsafe_code)]
    pub fn backing_bytes() -> usize {
        // SAFETY: this pointer-free upstream function reports its context size.
        // Pairing::new allocates precisely this many complete u64 words.
        let bytes = unsafe { blst::blst_pairing_sizeof() };
        (bytes / core::mem::size_of::<u64>()) * core::mem::size_of::<u64>()
    }

    /// Admit the exact backing before allocating and retain the returned owner.
    ///
    /// # Errors
    /// Returns the caller's unchanged typed refusal. Refusal allocates no pairing
    /// context and cannot become a cryptographic rejection.
    pub fn new<E>(admit: impl FnOnce(usize) -> Result<O, E>) -> Result<Self, E> {
        let owner = admit(Self::backing_bytes())?;
        Ok(Self {
            pairing: Pairing::new(true, HASH_TO_FIELD_DST),
            _owner: owner,
        })
    }

    /// Verify a grouped aggregate without allocating or retaining borrowed input.
    ///
    /// Each group contains a repeatable, cloneable iterator of PoP-verified keys and one
    /// message. Groups must have distinct messages and no repeated key within a
    /// group. Cloning an iterator must preserve its order and contents. The same
    /// key may sign distinct messages. Iterators are rescanned to
    /// enforce these conditions without heap sets. Each call resets the original
    /// context, including calls following rejected or differently shaped proofs.
    ///
    /// # Errors
    /// Returns the same canonical signature parse or relation rejection as the
    /// ordinary aggregate API, in fixed unformatted form. There is no resource
    /// refusal after construction; the original scratch owner remains reusable.
    pub fn verify<'a, G, K>(
        &mut self,
        groups: G,
        aggregate: &[u8],
    ) -> Result<(), SignatureVerificationError>
    where
        G: Iterator<Item = (K, &'a [u8])> + Clone,
        K: Iterator<Item = &'a BlsNormalPopVerifiedKey> + Clone,
    {
        self.pairing.init(true, HASH_TO_FIELD_DST);
        if groups.clone().next().is_none() {
            return Err(SignatureVerificationError::bad_signature());
        }
        // Preserve the original public adapter's geometry/duplicate rejection
        // before canonical aggregate-signature parsing.
        for (group_index, (keys, message)) in groups.clone().enumerate() {
            if keys.clone().next().is_none()
                || groups
                    .clone()
                    .take(group_index)
                    .any(|(_, prior)| prior == message)
            {
                return Err(SignatureVerificationError::bad_signature());
            }
            for (key_index, key) in keys.clone().enumerate() {
                if keys.clone().take(key_index).any(|prior| prior == key) {
                    return Err(SignatureVerificationError::bad_signature());
                }
            }
        }
        let signature = match uncached::signature(Orientation::Normal, aggregate)
            .map_err(SignatureVerificationError::from_bls)?
        {
            uncached::Signature::Normal(signature) => signature,
            uncached::Signature::Small(_) => {
                return Err(SignatureVerificationError::bad_signature());
            }
        };
        for (keys, message) in groups {
            let sum = keys.fold(G1Projective::identity(), |sum, key| sum + key.point);
            if bool::from(sum.is_identity()) {
                return Err(SignatureVerificationError::bad_signature());
            }
            // This is the existing contextual w3f Message point, independently
            // covered by the shared uncached suite's original-point parity test.
            let message = G2Projective::hash_to_curve(message, HASH_TO_FIELD_DST, NORMAL_PREFIX);
            self.pairing
                .raw_aggregate(message.to_affine().as_ref(), sum.to_affine().as_ref());
        }
        self.pairing.commit();
        let mut signature_pairing = blst_fp12::default();
        let signature: &blst_p2_affine = signature.as_ref();
        Pairing::aggregated(&mut signature_pairing, signature);
        if self.pairing.finalverify(Some(&signature_pairing)) {
            Ok(())
        } else {
            Err(SignatureVerificationError::bad_signature())
        }
    }
}

#[cfg(test)]
mod tests;
