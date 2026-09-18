//! Authenticated ownership of an indexed original structured proving-key capture.
//!
//! This is an internal format/storage owner, not proving or release admission. The existing
//! loaders must still check standalone VK bytes, protocol/profile identities, parameter prefixes,
//! role separation and witnesses. TODO: integrate indexed polynomial access and the complete
//! stored prover before replacing the dense production loaders; never fabricate a normal PK.
//! The retained VK/domain/CS and scanner permutation bitmap still need memory accounting.
//! The indexed loader is test-only until that integration; the bounded canonical digest writer
//! remains available to the production artifact loader.

use std::io;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use ff::FromUniformBytes;
#[cfg(test)]
use halo2_proofs::{
    SerdeCurveAffine, SerdePrimeField,
    halo2curves::pasta::{EpAffine, EqAffine},
    plonk::{Circuit, IndexedStructuredProvingKeyV1, VerifyingKey},
};
use sha2::{Digest as _, Sha256};

use super::KagemushaArtifactBindingV1;
#[cfg(test)]
use super::{
    KagemushaArtifactByteResolverV1, KagemushaArtifactDescriptorV1, KagemushaArtifactErrorV1,
    KagemushaArtifactKindV1, KagemushaArtifactRoleV1, KagemushaAuthenticatedArtifactSetV1,
    KagemushaPastaParityV1,
    stored_capture::{CapturedProvingKeyArtifactV1, CapturedProvingKeyReaderV1},
};

#[cfg(test)]
mod sealed {
    pub trait PastaCurve {}
    impl PastaCurve for super::EqAffine {}
    impl PastaCurve for super::EpAffine {}
}

/// Closed mapping from the original Pasta curve to its release role parity.
#[cfg(test)]
pub(in crate::zk::kagemusha_v1_recursion) trait KagemushaIndexedKeyCurveV1:
    SerdeCurveAffine + sealed::PastaCurve
{
    /// Parity of this concrete curve; a caller cannot relabel its frame.
    const PARITY: KagemushaPastaParityV1;
}

#[cfg(test)]
impl KagemushaIndexedKeyCurveV1 for EqAffine {
    const PARITY: KagemushaPastaParityV1 = KagemushaPastaParityV1::Eq;
}

#[cfg(test)]
impl KagemushaIndexedKeyCurveV1 for EpAffine {
    const PARITY: KagemushaPastaParityV1 = KagemushaPastaParityV1::Ep;
}

/// One checked index inseparably owned with the original release-bound capture it indexed.
///
/// Fields and construction are private. This has no clone, replacement-source constructor or
/// mutable index/VK accessor. A checked embedded VK is not standalone release verification.
#[cfg(test)]
pub(in crate::zk::kagemusha_v1_recursion) struct AuthenticatedIndexedProvingKeyV1<
    C: SerdeCurveAffine,
> {
    capture: CapturedProvingKeyArtifactV1,
    index: IndexedStructuredProvingKeyV1<C>,
}

#[cfg(test)]
impl<R: KagemushaArtifactByteResolverV1> KagemushaAuthenticatedArtifactSetV1<R> {
    /// Capture and index exactly this set's original proving-key source under trusted shape inputs.
    ///
    /// The expected k and original circuit parameters must come from the fixed role's trusted
    /// profile. This factory does not substitute its own circuit or satisfy later loader checks.
    pub(in crate::zk::kagemusha_v1_recursion) fn capture_indexed_proving_key<C, ConcreteCircuit>(
        &self,
        role: KagemushaArtifactRoleV1,
        parity: KagemushaPastaParityV1,
        expected_k: u32,
        circuit_params: ConcreteCircuit::Params,
        directory: &Path,
    ) -> Result<AuthenticatedIndexedProvingKeyV1<C>, KagemushaArtifactErrorV1>
    where
        C: KagemushaIndexedKeyCurveV1,
        C::Scalar: SerdePrimeField + FromUniformBytes<64>,
        ConcreteCircuit: Circuit<C::Scalar>,
    {
        let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
        let binding = self.binding(role);
        descriptor.validate_binding(binding)?;
        if descriptor.kind != KagemushaArtifactKindV1::ProvingKey
            || descriptor.parity != parity
            || parity != C::PARITY
        {
            return Err(KagemushaArtifactErrorV1::InvalidBinding(role));
        }
        let rows = 1_usize
            .checked_shl(expected_k)
            .filter(|rows| u32::try_from(*rows).is_ok())
            .ok_or_else(|| indexed_key_error(role, "unsupported trusted proving-key degree"))?;
        let capture = self.capture_proving_key(role, directory)?;
        if capture.binding() != binding || !capture.matches_artifact_set(self) {
            return Err(KagemushaArtifactErrorV1::InvalidBinding(role));
        }
        let (capture, index) = capture.with_reader(|reader| {
            let mut canonical = CanonicalArtifactDigestWriterV1::new(binding.byte_len);
            let index = IndexedStructuredProvingKeyV1::<C>::read_checked::<_, _, ConcreteCircuit>(
                reader,
                expected_k,
                binding.byte_len,
                circuit_params,
                &mut canonical,
            )
            .map_err(|error| {
                indexed_key_error(role, &format!("structured key indexing: {error}"))
            })?;
            if index.get_vk().get_domain().k() != expected_k
                || index.rows() != rows
                || index.frame_bytes() != binding.byte_len
                || !canonical.matches(binding)
            {
                return Err(indexed_key_error(
                    role,
                    "indexed key shape or canonical digest mismatch",
                ));
            }
            Ok(index)
        })?;
        // with_reader has now checked both its fatal latch and the original snapshot's presence.
        Ok(AuthenticatedIndexedProvingKeyV1 { capture, index })
    }
}

#[cfg(test)]
impl<C: SerdeCurveAffine> AuthenticatedIndexedProvingKeyV1<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Borrow the checked original embedded VK for the existing standalone/profile comparisons.
    pub(in crate::zk::kagemusha_v1_recursion) fn checked_vk(&self) -> &VerifyingKey<C> {
        self.index.get_vk()
    }

    /// Return the original authenticated source's exact role, length and digest.
    pub(in crate::zk::kagemusha_v1_recursion) fn binding(&self) -> KagemushaArtifactBindingV1 {
        self.capture.binding()
    }

    /// Check all retained release/profile/protocol/provider identities against this artifact set.
    pub(in crate::zk::kagemusha_v1_recursion) fn matches_artifact_set<
        R: KagemushaArtifactByteResolverV1,
    >(
        &self,
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    ) -> bool {
        self.capture.matches_artifact_set(artifacts)
    }

    /// Consume this owner and lend only its own original source alongside its own immutable index.
    ///
    /// A callback error or unwind drops both. Even a swallowed read failure or read unwind cannot
    /// restore the owner: capture performs its fatal/presence gate after the callback returns.
    pub(in crate::zk::kagemusha_v1_recursion) fn with_original_reader<T, E>(
        self,
        consume: impl FnOnce(
            &IndexedStructuredProvingKeyV1<C>,
            &mut CapturedProvingKeyReaderV1<'_>,
        ) -> Result<T, E>,
    ) -> Result<(Self, T), E>
    where
        E: From<KagemushaArtifactErrorV1>,
    {
        let Self { capture, index } = self;
        let (capture, value) = capture.with_reader(|reader| consume(&index, reader))?;
        Ok((Self { capture, index }, value))
    }
}

#[cfg(test)]
fn indexed_key_error(role: KagemushaArtifactRoleV1, reason: &str) -> KagemushaArtifactErrorV1 {
    KagemushaArtifactErrorV1::Read {
        role,
        reason: reason.to_owned(),
    }
}

/// Hash canonical preprocessing directly into the authenticated length bound. This never
/// allocates a second key-sized buffer, and every nested writer propagates its sink errors.
#[cfg(feature = "zk-halo2-ipa")]
pub(in crate::zk::kagemusha_v1_recursion) struct CanonicalArtifactDigestWriterV1 {
    digest: Sha256,
    pub(in crate::zk::kagemusha_v1_recursion) written: u64,
    maximum: u64,
}

#[cfg(feature = "zk-halo2-ipa")]
impl CanonicalArtifactDigestWriterV1 {
    pub(in crate::zk::kagemusha_v1_recursion) fn new(maximum: u64) -> Self {
        Self {
            digest: Sha256::new(),
            written: 0,
            maximum,
        }
    }

    pub(in crate::zk::kagemusha_v1_recursion) fn matches(
        self,
        binding: KagemushaArtifactBindingV1,
    ) -> bool {
        self.written == binding.byte_len
            && <[u8; 32]>::from(self.digest.finalize()) == binding.sha256
    }
}

#[cfg(feature = "zk-halo2-ipa")]
impl io::Write for CanonicalArtifactDigestWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = u64::try_from(bytes.len())
            .map_err(|_| io::Error::other("canonical artifact byte count overflow"))?;
        let next = self
            .written
            .checked_add(count)
            .filter(|next| *next <= self.maximum)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "canonical artifact exceeds authenticated byte length",
                )
            })?;
        self.digest.update(bytes);
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "stored_key_tests.rs"]
mod tests;
