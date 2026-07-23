//! This module provides an implementation of a variant of (Turbo)[PLONK][plonk]
//! that is designed specifically for the polynomial commitment scheme described
//! in the [Halo][halo] paper.
//!
//! [halo]: https://eprint.iacr.org/2019/1021
//! [plonk]: https://eprint.iacr.org/2019/953

use blake2b_simd::Params as Blake2bParams;
use group::ff::{Field, FromUniformBytes, PrimeField};

use crate::SerdeFormat;
use crate::arithmetic::CurveAffine;
use crate::helpers::{
    SerdeCurveAffine, SerdePrimeField, polynomial_slice_byte_length, read_polynomial_vec,
    write_polynomial_slice, write_polynomial_slice_streaming,
};
use crate::poly::{Coeff, EvaluationDomain, LagrangeCoeff, PinnedEvaluationDomain, Polynomial};
use crate::transcript::{ChallengeScalar, EncodedChallenge, Transcript};

mod assigned;
mod circuit;
mod error;
mod evaluation;
mod keygen;
mod lookup;
pub mod permutation;
// mod shuffle;
mod vanishing;

mod prover;
mod verifier;

pub use assigned::*;
pub use circuit::*;
pub use error::*;
pub use keygen::*;
pub use prover::*;
pub use verifier::*;

use evaluation::Evaluator;

use std::io;

// Version byte + domain degree + selector-compression flag + fixed-column count.
const VERIFYING_KEY_SERIALIZED_HEADER_BYTES: usize = 1 + 4 + 1 + 4;

/// This is a verifying key which allows for the verification of proofs for a
/// particular circuit.
#[derive(Clone, Debug)]
pub struct VerifyingKey<C: CurveAffine> {
    domain: EvaluationDomain<C::Scalar>,
    fixed_commitments: Vec<C>,
    permutation: permutation::VerifyingKey<C>,
    cs: ConstraintSystem<C::Scalar>,
    /// Cached maximum degree of `cs` (which doesn't change after construction).
    cs_degree: usize,
    /// The representative of this `VerifyingKey` in transcripts.
    transcript_repr: C::Scalar,
    selectors: Vec<Vec<bool>>,
    /// Whether selector compression is turned on or not.
    compress_selectors: bool,
}

impl<C: SerdeCurveAffine> VerifyingKey<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>, // the FromUniformBytes<64> should not be necessary: currently serialization always stores a Blake2b hash of verifying key; this should be removed
{
    /// Writes a verifying key to a buffer.
    ///
    /// Writes a curve element according to `format`:
    /// - `Processed`: Writes a compressed curve element with coordinates in standard form.
    /// Writes a field element in standard form, with endianness specified by the
    /// `PrimeField` implementation.
    /// - Otherwise: Writes an uncompressed curve element with coordinates in Montgomery form
    /// Writes a field element into raw bytes in its internal Montgomery representation,
    /// WITHOUT performing the expensive Montgomery reduction.
    pub fn write<W: io::Write>(&self, writer: &mut W, format: SerdeFormat) -> io::Result<()> {
        // Version byte that will be checked on read.
        writer.write_all(&[0x02])?;
        writer.write_all(&self.domain.k().to_le_bytes())?;
        writer.write_all(&[self.compress_selectors as u8])?;
        writer.write_all(&(self.fixed_commitments.len() as u32).to_le_bytes())?;
        for commitment in &self.fixed_commitments {
            commitment.write(writer, format)?;
        }
        self.permutation.write(writer, format)?;

        if !self.compress_selectors {
            assert!(self.selectors.is_empty());
        }
        // write self.selectors
        for selector in &self.selectors {
            // since `selector` is filled with `bool`, we pack them 8 at a time into bytes and then write
            for bits in selector.chunks(8) {
                writer.write_all(&[crate::helpers::pack(bits)])?;
            }
        }
        Ok(())
    }

    /// Reads a verification key from a buffer.
    ///
    /// Reads a curve element from the buffer and parses it according to the `format`:
    /// - `Processed`: Reads a compressed curve element and decompresses it.
    /// Reads a field element in standard form, with endianness specified by the
    /// `PrimeField` implementation, and checks that the element is less than the modulus.
    /// - `RawBytes`: Reads an uncompressed curve element with coordinates in Montgomery form.
    /// Checks that field elements are less than modulus, and then checks that the point is on the curve.
    /// - `RawBytesUnchecked`: Reads an uncompressed curve element with coordinates in Montgomery form;
    /// does not perform any checks
    pub fn read<R: io::Read, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        format: SerdeFormat,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
        let mut version_byte = [0u8; 1];
        reader.read_exact(&mut version_byte)?;
        if 0x02 != version_byte[0] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected version byte",
            ));
        }
        let mut k = [0u8; 4];
        reader.read_exact(&mut k)?;
        let k = u32::from_le_bytes(k);
        let mut compress_selectors = [0u8; 1];
        reader.read_exact(&mut compress_selectors)?;
        if compress_selectors[0] != 0 && compress_selectors[0] != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected compress_selectors not boolean",
            ));
        }
        let compress_selectors = compress_selectors[0] == 1;
        let (domain, cs, _) = keygen::create_domain::<C, ConcreteCircuit>(
            k,
            #[cfg(feature = "circuit-params")]
            params,
        );
        let mut num_fixed_columns = [0u8; 4];
        reader.read_exact(&mut num_fixed_columns)?;
        let num_fixed_columns = u32::from_le_bytes(num_fixed_columns);

        let fixed_commitments: Vec<_> = (0..num_fixed_columns)
            .map(|_| C::read(reader, format))
            .collect::<io::Result<_>>()?;

        let permutation = permutation::VerifyingKey::read(reader, &cs.permutation, format)?;

        let (cs, selectors) = if compress_selectors {
            // read selectors
            let selectors: Vec<Vec<bool>> = vec![vec![false; 1 << k]; cs.num_selectors]
                .into_iter()
                .map(|mut selector| {
                    let mut selector_bytes = vec![0u8; (selector.len() + 7) / 8];
                    reader.read_exact(&mut selector_bytes)?;
                    for (bits, byte) in selector.chunks_mut(8).zip(selector_bytes) {
                        crate::helpers::unpack(byte, bits);
                    }
                    Ok(selector)
                })
                .collect::<io::Result<_>>()?;
            let (cs, _) = cs.compress_selectors(selectors.clone());
            (cs, selectors)
        } else {
            // we still need to replace selectors with fixed Expressions in `cs`
            let fake_selectors = vec![vec![false]; cs.num_selectors];
            let (cs, _) = cs.directly_convert_selectors_to_fixed(fake_selectors);
            (cs, vec![])
        };

        Ok(Self::from_parts(
            domain,
            fixed_commitments,
            permutation,
            cs,
            selectors,
            compress_selectors,
        ))
    }

    /// Writes a verifying key to a vector of bytes using [`Self::write`].
    pub fn to_bytes(&self, format: SerdeFormat) -> Vec<u8> {
        let mut bytes = Vec::<u8>::with_capacity(self.bytes_length());
        Self::write(self, &mut bytes, format).expect("Writing to vector should not fail");
        bytes
    }

    /// Reads a verification key from a slice of bytes using [`Self::read`].
    pub fn from_bytes<ConcreteCircuit: Circuit<C::Scalar>>(
        mut bytes: &[u8],
        format: SerdeFormat,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
        Self::read::<_, ConcreteCircuit>(
            &mut bytes,
            format,
            #[cfg(feature = "circuit-params")]
            params,
        )
    }
}

fn write_polynomial_vec_consuming<W: io::Write, F: SerdePrimeField, B>(
    polynomials: Vec<Polynomial<F, B>>,
    writer: &mut W,
    format: SerdeFormat,
) -> io::Result<()> {
    writer.write_all(&(polynomials.len() as u32).to_be_bytes())?;
    for poly in polynomials {
        poly.write_consuming(writer, format)?;
    }
    Ok(())
}

impl<C: CurveAffine> VerifyingKey<C> {
    fn bytes_length(&self) -> usize {
        VERIFYING_KEY_SERIALIZED_HEADER_BYTES
            + (self.fixed_commitments.len() * C::default().to_bytes().as_ref().len())
            + self.permutation.bytes_length()
            + self.selectors.len()
                * (self
                    .selectors
                    .get(0)
                    .map(|selector| (selector.len() + 7) / 8)
                    .unwrap_or(0))
    }

    fn from_parts(
        domain: EvaluationDomain<C::Scalar>,
        fixed_commitments: Vec<C>,
        permutation: permutation::VerifyingKey<C>,
        cs: ConstraintSystem<C::Scalar>,
        selectors: Vec<Vec<bool>>,
        compress_selectors: bool,
    ) -> Self
    where
        C::Scalar: FromUniformBytes<64>,
    {
        // Compute cached values.
        let cs_degree = cs.degree();

        let mut vk = Self {
            domain,
            fixed_commitments,
            permutation,
            cs,
            cs_degree,
            // Temporary, this is not pinned.
            transcript_repr: C::Scalar::ZERO,
            selectors,
            compress_selectors,
        };

        let mut hasher = Blake2bParams::new()
            .hash_length(64)
            .personal(b"Halo2-Verify-Key")
            .to_state();

        let s = format!("{:?}", vk.pinned());

        hasher.update(&(s.len() as u64).to_le_bytes());
        hasher.update(s.as_bytes());

        // Hash in final Blake2bState
        vk.transcript_repr = C::Scalar::from_uniform_bytes(hasher.finalize().as_array());

        vk
    }

    /// Hashes a verification key into a transcript.
    pub fn hash_into<E: EncodedChallenge<C>, T: Transcript<C, E>>(
        &self,
        transcript: &mut T,
    ) -> io::Result<()> {
        transcript.common_scalar(self.transcript_repr)?;

        Ok(())
    }

    /// Obtains a pinned representation of this verification key that contains
    /// the minimal information necessary to reconstruct the verification key.
    pub fn pinned(&self) -> PinnedVerificationKey<'_, C> {
        PinnedVerificationKey {
            base_modulus: C::Base::MODULUS,
            scalar_modulus: C::Scalar::MODULUS,
            domain: self.domain.pinned(),
            fixed_commitments: &self.fixed_commitments,
            permutation: &self.permutation,
            cs: self.cs.pinned(),
        }
    }

    /// Returns commitments of fixed polynomials
    pub fn fixed_commitments(&self) -> &Vec<C> {
        &self.fixed_commitments
    }

    /// Returns `VerifyingKey` of permutation
    pub fn permutation(&self) -> &permutation::VerifyingKey<C> {
        &self.permutation
    }

    /// Returns `ConstraintSystem`
    pub fn cs(&self) -> &ConstraintSystem<C::Scalar> {
        &self.cs
    }

    /// Returns representative of this `VerifyingKey` in transcripts
    pub fn transcript_repr(&self) -> C::Scalar {
        self.transcript_repr
    }
}

/// Minimal representation of a verification key that can be used to identify
/// its active contents.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PinnedVerificationKey<'a, C: CurveAffine> {
    base_modulus: &'static str,
    scalar_modulus: &'static str,
    domain: PinnedEvaluationDomain<'a, C::Scalar>,
    cs: PinnedConstraintSystem<'a, C::Scalar>,
    fixed_commitments: &'a Vec<C>,
    permutation: &'a permutation::VerifyingKey<C>,
}
/// This is a proving key which allows for the creation of proofs for a
/// particular circuit.
#[derive(Clone, Debug)]
pub struct ProvingKey<C: CurveAffine> {
    vk: VerifyingKey<C>,
    l0: Polynomial<C::Scalar, Coeff>,
    l_last: Polynomial<C::Scalar, Coeff>,
    l_active_row: Polynomial<C::Scalar, Coeff>,
    fixed_values: Vec<Polynomial<C::Scalar, LagrangeCoeff>>,
    fixed_polys: Vec<Polynomial<C::Scalar, Coeff>>,
    permutation: permutation::ProvingKey<C>,
    ev: Evaluator<C>,
}

impl<C: CurveAffine> ProvingKey<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    /// Get the underlying [`VerifyingKey`].
    pub fn get_vk(&self) -> &VerifyingKey<C> {
        &self.vk
    }

    /// Gets the total number of bytes in the serialization of `self`
    fn bytes_length(&self) -> usize {
        let scalar_len = C::Scalar::default().to_repr().as_ref().len();
        self.vk.bytes_length()
            + 12
            + scalar_len * (self.l0.len() + self.l_last.len() + self.l_active_row.len())
            + polynomial_slice_byte_length(&self.fixed_values)
            + polynomial_slice_byte_length(&self.fixed_polys)
            + self.permutation.bytes_length()
    }
}

impl<C: SerdeCurveAffine> ProvingKey<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Writes a proving key to a buffer.
    ///
    /// Writes a curve element according to `format`:
    /// - `Processed`: Writes a compressed curve element with coordinates in standard form.
    /// Writes a field element in standard form, with endianness specified by the
    /// `PrimeField` implementation.
    /// - Otherwise: Writes an uncompressed curve element with coordinates in Montgomery form
    /// Writes a field element into raw bytes in its internal Montgomery representation,
    /// WITHOUT performing the expensive Montgomery reduction.
    /// Does so by first writing the verifying key and then serializing the rest of the data (in the form of field polynomials)
    pub fn write<W: io::Write>(&self, writer: &mut W, format: SerdeFormat) -> io::Result<()> {
        self.vk.write(writer, format)?;
        self.l0.write(writer, format);
        self.l_last.write(writer, format);
        self.l_active_row.write(writer, format);
        write_polynomial_slice(&self.fixed_values, writer, format);
        write_polynomial_slice(&self.fixed_polys, writer, format);
        self.permutation.write(writer, format);
        Ok(())
    }

    /// Streams a proving key without consuming it and propagates every sink error.
    ///
    /// This produces the same bytes as [`Self::write`] while leaving the key
    /// available for a subsequent consuming proof.
    pub fn write_streaming<W: io::Write>(
        &self,
        writer: &mut W,
        format: SerdeFormat,
    ) -> io::Result<()> {
        self.vk.write(writer, format)?;
        self.l0.write_streaming(writer, format)?;
        self.l_last.write_streaming(writer, format)?;
        self.l_active_row.write_streaming(writer, format)?;
        write_polynomial_slice_streaming(&self.fixed_values, writer, format)?;
        write_polynomial_slice_streaming(&self.fixed_polys, writer, format)?;
        self.permutation.write_streaming(writer, format)
    }

    /// Reads a proving key from a buffer.
    /// Does so by reading verification key first, and then deserializing the rest of the file into the remaining proving key data.
    ///
    /// Reads a curve element from the buffer and parses it according to the `format`:
    /// - `Processed`: Reads a compressed curve element and decompresses it.
    /// Reads a field element in standard form, with endianness specified by the
    /// `PrimeField` implementation, and checks that the element is less than the modulus.
    /// - `RawBytes`: Reads an uncompressed curve element with coordinates in Montgomery form.
    /// Checks that field elements are less than modulus, and then checks that the point is on the curve.
    /// - `RawBytesUnchecked`: Reads an uncompressed curve element with coordinates in Montgomery form;
    /// does not perform any checks
    pub fn read<R: io::Read, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        format: SerdeFormat,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
        let vk = VerifyingKey::<C>::read::<R, ConcreteCircuit>(
            reader,
            format,
            #[cfg(feature = "circuit-params")]
            params,
        )?;
        let l0 = Polynomial::read(reader, format);
        let l_last = Polynomial::read(reader, format);
        let l_active_row = Polynomial::read(reader, format);
        let fixed_values = read_polynomial_vec(reader, format);
        let fixed_polys = read_polynomial_vec(reader, format);
        let permutation = permutation::ProvingKey::read(reader, format);
        let ev = Evaluator::new(vk.cs());
        Ok(Self {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation,
            ev,
        })
    }

    /// Writes a proving key to a vector of bytes using [`Self::write`].
    pub fn to_bytes(&self, format: SerdeFormat) -> Vec<u8> {
        let mut bytes = Vec::<u8>::with_capacity(self.bytes_length());
        Self::write(self, &mut bytes, format).expect("Writing to vector should not fail");
        bytes
    }

    /// Writes a proving key directly to a sink while dropping fields as they are serialized.
    ///
    /// Unlike [`Self::to_bytes`], this consumes the key and never allocates a
    /// second release-sized byte vector. Callers writing large processed keys
    /// should prefer this method and provide their final file or framed sink.
    pub fn write_consuming<W: io::Write>(
        self,
        writer: &mut W,
        format: SerdeFormat,
    ) -> io::Result<()> {
        let Self {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation,
            ev: _,
        } = self;
        vk.write(writer, format)?;
        l0.write_consuming(writer, format)?;
        l_last.write_consuming(writer, format)?;
        l_active_row.write_consuming(writer, format)?;
        write_polynomial_vec_consuming(fixed_values, writer, format)?;
        write_polynomial_vec_consuming(fixed_polys, writer, format)?;
        permutation.write_consuming(writer, format)?;
        Ok(())
    }

    /// Writes a proving key to a vector while dropping fields as they are serialized.
    pub fn into_bytes(self, format: SerdeFormat) -> Vec<u8> {
        let mut bytes = Vec::<u8>::with_capacity(self.bytes_length());
        self.write_consuming(&mut bytes, format)
            .expect("Writing to vector should not fail");
        bytes
    }

    /// Reads a proving key from a slice of bytes using [`Self::read`].
    pub fn from_bytes<ConcreteCircuit: Circuit<C::Scalar>>(
        mut bytes: &[u8],
        format: SerdeFormat,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
        Self::read::<_, ConcreteCircuit>(
            &mut bytes,
            format,
            #[cfg(feature = "circuit-params")]
            params,
        )
    }
}

impl<C: CurveAffine> VerifyingKey<C> {
    /// Get the underlying [`EvaluationDomain`].
    pub fn get_domain(&self) -> &EvaluationDomain<C::Scalar> {
        &self.domain
    }
}

#[derive(Clone, Copy, Debug)]
struct Theta;
type ChallengeTheta<F> = ChallengeScalar<F, Theta>;

#[derive(Clone, Copy, Debug)]
struct Beta;
type ChallengeBeta<F> = ChallengeScalar<F, Beta>;

#[derive(Clone, Copy, Debug)]
struct Gamma;
type ChallengeGamma<F> = ChallengeScalar<F, Gamma>;

#[derive(Clone, Copy, Debug)]
struct Y;
type ChallengeY<F> = ChallengeScalar<F, Y>;

#[derive(Clone, Copy, Debug)]
struct X;
type ChallengeX<F> = ChallengeScalar<F, X>;

#[cfg(test)]
mod tests {
    use super::{
        Advice, Circuit, Column, ConstraintSystem, Error, KeygenWithExtractorError, Selector,
        SerdeFormat, keygen_pk, keygen_pk_consuming_with, keygen_pk2, keygen_vk,
        keygen_vk_consuming_with, keygen_vk_custom,
    };
    use crate::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::bn256::G1Affine,
        poly::{Rotation, commitment::ParamsProver, ipa::commitment::ParamsIPA},
    };
    use group::ff::Field;
    use std::{
        io,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    struct FailingWriter {
        accepted: usize,
        fail_after: usize,
    }

    impl io::Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.accepted >= self.fail_after {
                return Err(io::Error::new(io::ErrorKind::StorageFull, "sink is full"));
            }
            let accepted = bytes.len().min(self.fail_after - self.accepted);
            self.accepted += accepted;
            Ok(accepted)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct EmptyCircuit;

    impl<F: Field> Circuit<F> for EmptyCircuit {
        type Config = ();
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            *self
        }

        fn configure(_meta: &mut ConstraintSystem<F>) -> Self::Config {}

        fn synthesize(
            &self,
            _config: Self::Config,
            _layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct CircuitLifetime {
        synthesized: AtomicBool,
        dropped: AtomicBool,
    }

    struct TrackedCircuit {
        lifetime: Option<Arc<CircuitLifetime>>,
        fail_synthesis: bool,
    }

    impl Drop for TrackedCircuit {
        fn drop(&mut self) {
            if let Some(lifetime) = &self.lifetime {
                lifetime.dropped.store(true, Ordering::SeqCst);
            }
        }
    }

    #[derive(Clone, Copy)]
    struct TrackedConfig {
        left: Column<Advice>,
        right: Column<Advice>,
        selector: Selector,
    }

    impl<F: Field> Circuit<F> for TrackedCircuit {
        type Config = TrackedConfig;
        type FloorPlanner = SimpleFloorPlanner;
        #[cfg(feature = "circuit-params")]
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self {
                lifetime: None,
                fail_synthesis: self.fail_synthesis,
            }
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let left = meta.advice_column();
            let right = meta.advice_column();
            let selector = meta.selector();
            meta.enable_equality(left);
            meta.enable_equality(right);
            meta.create_gate("tracked selector", |meta| {
                let selector = meta.query_selector(selector);
                let left = meta.query_advice(left, Rotation::cur());
                let right = meta.query_advice(right, Rotation::cur());
                vec![selector * (left - right)]
            });
            TrackedConfig {
                left,
                right,
                selector,
            }
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            if let Some(lifetime) = &self.lifetime {
                lifetime.synthesized.store(true, Ordering::SeqCst);
            }
            if self.fail_synthesis {
                return Err(Error::Synthesis);
            }
            layouter.assign_region(
                || "tracked region",
                |mut region| {
                    config.selector.enable(&mut region, 0)?;
                    let left = region.assign_advice(config.left, 0, Value::known(F::ONE));
                    left.copy_advice(&mut region, config.right, 0);
                    Ok(())
                },
            )
        }
    }

    fn tracked_circuit(fail_synthesis: bool) -> (TrackedCircuit, Arc<CircuitLifetime>) {
        let lifetime = Arc::new(CircuitLifetime::default());
        (
            TrackedCircuit {
                lifetime: Some(Arc::clone(&lifetime)),
                fail_synthesis,
            },
            lifetime,
        )
    }

    #[test]
    fn consuming_keygen_preserves_bytes_and_drops_circuits_on_all_paths() {
        let params = ParamsIPA::<G1Affine>::new(3);
        let reference = TrackedCircuit {
            lifetime: None,
            fail_synthesis: false,
        };
        let borrowed_vk = keygen_vk(&params, &reference).expect("borrowed VK generation");
        let borrowed_pk =
            keygen_pk(&params, borrowed_vk.clone(), &reference).expect("borrowed PK generation");
        let combined_pk = keygen_pk2(&params, &reference, false).expect("combined PK generation");
        assert_eq!(
            borrowed_pk.to_bytes(SerdeFormat::Processed),
            combined_pk.to_bytes(SerdeFormat::Processed),
            "streaming permutation VK commitments before fixed expansion and reusing a supplied \
             VK domain must preserve processed key bytes"
        );
        let compressed_vk =
            keygen_vk_custom(&params, &reference, true).expect("compressed VK generation");
        let compressed_combined_pk =
            keygen_pk2(&params, &reference, true).expect("compressed combined PK generation");
        assert_eq!(
            compressed_vk.to_bytes(SerdeFormat::Processed),
            compressed_combined_pk
                .get_vk()
                .to_bytes(SerdeFormat::Processed),
            "permutation-first VK generation must preserve compressed-selector bytes"
        );

        let (circuit, lifetime) = tracked_circuit(false);
        let (consuming_vk, extracted) = keygen_vk_consuming_with(&params, circuit, |circuit| {
            let lifetime = circuit.lifetime.as_ref().expect("tracked circuit");
            assert!(lifetime.synthesized.load(Ordering::SeqCst));
            assert!(!lifetime.dropped.load(Ordering::SeqCst));
            Ok::<_, &'static str>("vk metadata")
        })
        .expect("consuming VK generation");
        assert_eq!(extracted, "vk metadata");
        assert!(lifetime.dropped.load(Ordering::SeqCst));
        assert_eq!(
            borrowed_vk.to_bytes(SerdeFormat::Processed),
            consuming_vk.to_bytes(SerdeFormat::Processed)
        );

        let (circuit, lifetime) = tracked_circuit(false);
        let (consuming_pk, extracted) =
            keygen_pk_consuming_with(&params, borrowed_vk.clone(), circuit, |circuit| {
                let lifetime = circuit.lifetime.as_ref().expect("tracked circuit");
                assert!(lifetime.synthesized.load(Ordering::SeqCst));
                assert!(!lifetime.dropped.load(Ordering::SeqCst));
                Ok::<_, &'static str>("pk metadata")
            })
            .expect("consuming PK generation");
        assert_eq!(extracted, "pk metadata");
        assert!(lifetime.dropped.load(Ordering::SeqCst));
        assert_eq!(
            borrowed_pk.to_bytes(SerdeFormat::Processed),
            consuming_pk.to_bytes(SerdeFormat::Processed)
        );

        let (circuit, lifetime) = tracked_circuit(false);
        let extractor_failure =
            keygen_pk_consuming_with(&params, borrowed_vk.clone(), circuit, |_| {
                Err::<(), _>("metadata rejected")
            });
        assert!(matches!(
            extractor_failure,
            Err(KeygenWithExtractorError::Extractor("metadata rejected"))
        ));
        assert!(lifetime.dropped.load(Ordering::SeqCst));

        let (circuit, lifetime) = tracked_circuit(true);
        let extractor_called = Arc::new(AtomicBool::new(false));
        let extractor_called_in_closure = Arc::clone(&extractor_called);
        let synthesis_failure =
            keygen_pk_consuming_with(&params, borrowed_vk, circuit, move |_| {
                extractor_called_in_closure.store(true, Ordering::SeqCst);
                Ok::<_, &'static str>(())
            });
        assert!(matches!(
            synthesis_failure,
            Err(KeygenWithExtractorError::Keygen(Error::Synthesis))
        ));
        assert!(!extractor_called.load(Ordering::SeqCst));
        assert!(lifetime.dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn processed_key_byte_estimates_match_serialized_lengths() {
        // Eight rows keep this regression test small while leaving enough room for
        // Halo2's blinding rows.
        let params = ParamsIPA::<G1Affine>::new(3);
        let vk = keygen_vk(&params, &EmptyCircuit).expect("verifying-key generation should work");

        let expected_vk_bytes = vk.bytes_length();
        let vk_bytes = vk.to_bytes(SerdeFormat::Processed);
        assert_eq!(expected_vk_bytes, vk_bytes.len());
        assert_eq!(expected_vk_bytes, vk_bytes.capacity());

        let pk = keygen_pk(&params, vk, &EmptyCircuit).expect("proving-key generation should work");
        let expected_pk_bytes = pk.bytes_length();
        let pk_bytes = pk.to_bytes(SerdeFormat::Processed);
        assert_eq!(expected_pk_bytes, pk_bytes.len());
        assert_eq!(expected_pk_bytes, pk_bytes.capacity());

        let vk = keygen_vk(&params, &EmptyCircuit).expect("verifying-key generation should work");
        let pk = keygen_pk(&params, vk, &EmptyCircuit).expect("proving-key generation should work");
        let mut borrowed_stream = Vec::new();
        pk.write_streaming(&mut borrowed_stream, SerdeFormat::Processed)
            .expect("non-consuming proving-key stream should succeed");
        assert_eq!(pk_bytes, borrowed_stream);

        let fail_after = pk.get_vk().to_bytes(SerdeFormat::Processed).len() + 1;
        let mut failing_borrowed = FailingWriter {
            accepted: 0,
            fail_after,
        };
        let failure = pk
            .write_streaming(&mut failing_borrowed, SerdeFormat::Processed)
            .expect_err("a full non-consuming sink must be reported to the caller");
        assert_eq!(failure.kind(), io::ErrorKind::StorageFull);
        assert_eq!(pk_bytes, pk.to_bytes(SerdeFormat::Processed));

        let mut streamed = Vec::new();
        pk.write_consuming(&mut streamed, SerdeFormat::Processed)
            .expect("consuming proving-key write should succeed");
        assert_eq!(pk_bytes, streamed);

        let vk = keygen_vk(&params, &EmptyCircuit).expect("verifying-key generation should work");
        let pk = keygen_pk(&params, vk, &EmptyCircuit).expect("proving-key generation should work");
        let fail_after = pk.get_vk().to_bytes(SerdeFormat::Processed).len() + 1;
        let mut failing = FailingWriter {
            accepted: 0,
            fail_after,
        };
        let failure = pk
            .write_consuming(&mut failing, SerdeFormat::Processed)
            .expect_err("a full sink must be reported to the caller");
        assert_eq!(failure.kind(), io::ErrorKind::StorageFull);
    }
}
