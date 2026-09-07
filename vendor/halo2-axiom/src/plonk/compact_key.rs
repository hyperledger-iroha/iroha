//! Explicit one-basis proving-key storage, with the existing in-memory representation.
//!
//! This codec is independent of `SerdeFormat`: existing key encodings and callers are unchanged.
//! It stores each fixed/permutation polynomial in Lagrange form once, then reconstructs the
//! coefficient form using the verification key's exact domain. It saves artifact bytes, not
//! resident proving-key memory. Authentication, role binding and outer EOF remain caller duties.

use super::{
    Circuit, Coeff, EvaluationDomain, Evaluator, LagrangeCoeff, Polynomial, ProvingKey,
    SerdeCurveAffine, SerdeFormat, SerdePrimeField, VerifyingKey, permutation,
    read_polynomial_vec_checked, write_polynomial_slice_streaming, write_polynomial_vec_consuming,
};
use blake2b_simd::Params as Blake2bParams;
use group::ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use std::io::{self, Read, Write};

const MAGIC: &[u8; 16] = b"Halo2CompactPK1\0";
const HEADER_BYTES: u64 = 16 + 32 + 8;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Bind the curve equation, generator, both field moduli and field representation byte order.
/// Every part is length-prefixed, in the fixed order below; the codec version is also explicit.
fn curve_domain<C: SerdeCurveAffine>() -> [u8; 32] {
    let generator = C::generator().to_bytes();
    let a = C::a().to_repr();
    let b = C::b().to_repr();
    let base_one = C::Base::ONE.to_repr();
    let scalar_one = C::Scalar::ONE.to_repr();
    let mut digest = Blake2bParams::new()
        .hash_length(32)
        .personal(b"Halo2-PK-Codec1")
        .to_state();
    for bytes in [
        MAGIC.as_slice(),
        C::Base::MODULUS.as_bytes(),
        C::Scalar::MODULUS.as_bytes(),
        base_one.as_ref(),
        scalar_one.as_ref(),
        a.as_ref(),
        b.as_ref(),
        generator.as_ref(),
    ] {
        digest.update(&(bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    let mut result = [0; 32];
    result.copy_from_slice(digest.finalize().as_bytes());
    result
}

fn checked_rows<C: SerdeCurveAffine>(vk: &VerifyingKey<C>) -> io::Result<usize> {
    let k = vk.domain.k();
    let rows = 1_usize
        .checked_shl(k)
        .filter(|rows| k <= C::Scalar::S && u32::try_from(*rows).is_ok())
        .ok_or_else(|| invalid("compact key domain is unsupported"))?;
    if rows as u64 != vk.domain.get_n()
        || rows < vk.cs.minimum_rows()
        || vk.fixed_commitments.len() != vk.cs.num_fixed_columns
        || vk.permutation.commitments().len() != vk.cs.permutation.columns.len()
        || (vk.compress_selectors && vk.selectors.len() != vk.cs.num_selectors)
        || (!vk.compress_selectors && !vk.selectors.is_empty())
        || vk.selectors.iter().any(|selector| selector.len() != rows)
    {
        return Err(invalid(
            "compact key verification-key shape is inconsistent",
        ));
    }
    u32::try_from(vk.fixed_commitments.len())
        .map_err(|_| invalid("compact fixed-column count exceeds its wire bound"))?;
    u32::try_from(vk.cs.permutation.columns.len())
        .map_err(|_| invalid("compact permutation count exceeds its wire bound"))?;
    Ok(rows)
}

#[derive(Default)]
struct ByteCounter(u64);

impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(u64::try_from(bytes.len()).map_err(|_| invalid("byte count overflow"))?)
            .ok_or_else(|| invalid("byte count overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn byte_len<C: SerdeCurveAffine>(vk: &VerifyingKey<C>) -> io::Result<u64>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let rows = checked_rows(vk)?;
    let mut vk_bytes = ByteCounter::default();
    vk.write(&mut vk_bytes, SerdeFormat::Processed)?;
    let scalar_bytes = C::Scalar::ZERO.to_repr().as_ref().len() as u64;
    let polynomial_bytes = (rows as u64)
        .checked_mul(scalar_bytes)
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or_else(|| invalid("compact polynomial size overflow"))?;
    let polynomial_count = 3_u64
        .checked_add(vk.cs.num_fixed_columns as u64)
        .and_then(|count| count.checked_add(vk.cs.permutation.columns.len() as u64))
        .ok_or_else(|| invalid("compact polynomial count overflow"))?;
    polynomial_bytes
        .checked_mul(polynomial_count)
        .and_then(|bytes| bytes.checked_add(8)) // Two u32 polynomial-vector lengths.
        .and_then(|bytes| bytes.checked_add(vk_bytes.0))
        .and_then(|bytes| bytes.checked_add(HEADER_BYTES))
        .ok_or_else(|| invalid("compact proving-key size overflow"))
}

fn coefficients<F: WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    polynomial: &Polynomial<F, LagrangeCoeff>,
) -> io::Result<Polynomial<F, Coeff>> {
    let mut values = Vec::new();
    values.try_reserve_exact(polynomial.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "cannot reserve compact coefficient copy",
        )
    })?;
    values.extend_from_slice(polynomial);
    Ok(domain.lagrange_to_coeff(domain.lagrange_from_vec(values)))
}

fn reconstruct<F: WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    lagrange: &[Polynomial<F, LagrangeCoeff>],
) -> io::Result<Vec<Polynomial<F, Coeff>>> {
    let mut result = Vec::new();
    result.try_reserve_exact(lagrange.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "cannot reserve compact coefficient vector",
        )
    })?;
    for polynomial in lagrange {
        result.push(coefficients(domain, polynomial)?);
    }
    Ok(result)
}

fn validate_bases<F: WithSmallOrderMulGroup<3>>(
    domain: &EvaluationDomain<F>,
    lagrange: &[Polynomial<F, LagrangeCoeff>],
    coeff: &[Polynomial<F, Coeff>],
    columns: usize,
    rows: usize,
) -> io::Result<()> {
    if lagrange.len() != columns
        || coeff.len() != columns
        || lagrange.iter().any(|polynomial| polynomial.len() != rows)
        || coeff.iter().any(|polynomial| polynomial.len() != rows)
    {
        return Err(invalid("compact key polynomial shape is inconsistent"));
    }
    for (lagrange, coeff) in lagrange.iter().zip(coeff) {
        let expected = coefficients(domain, lagrange)?;
        if expected[..] != coeff[..] {
            return Err(invalid("compact key polynomial bases disagree"));
        }
    }
    Ok(())
}

impl<C: SerdeCurveAffine> ProvingKey<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Return the compact-v1 frame size for this verification key's configured shape.
    ///
    /// This checks dimensions only. [`Self::write_compact_v1`] additionally rejects disagreement
    /// between stored coefficient/Lagrange forms instead of silently changing a legacy-loaded key.
    pub fn compact_v1_bytes_length(&self) -> io::Result<u64> {
        byte_len(&self.vk)
    }

    /// Write an explicitly tagged compact-v1 frame, preserving the full processed key on reload.
    ///
    /// The frame is: 16-byte magic/version, 32-byte curve domain, u64-LE total frame length,
    /// processed VK, three processed coefficient masks, processed fixed Lagrange vector, then
    /// processed permutation Lagrange vector. Polynomial lengths/counts retain their u32-BE
    /// encoding. There is no compression inference or fallback to other formats.
    ///
    /// Both stored bases are compared with an exact inverse FFT before output starts. A malformed
    /// legacy key with inconsistent bases is rejected. VK commitments and mask coefficients are
    /// preserved verbatim; this is not a commitment/authenticity check and requires no parameters.
    /// Validation uses one domain polynomial of extra scalar storage at a time. Every writer error
    /// propagates; the caller owns flushing and publishing its output sink.
    pub fn write_compact_v1<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let length = self.validated_compact_v1_length()?;
        writer.write_all(MAGIC)?;
        writer.write_all(&curve_domain::<C>())?;
        writer.write_all(&length.to_le_bytes())?;
        self.vk.write(writer, SerdeFormat::Processed)?;
        for polynomial in [&self.l0, &self.l_last, &self.l_active_row] {
            polynomial.write_streaming(writer, SerdeFormat::Processed)?;
        }
        write_polynomial_slice_streaming(&self.fixed_values, writer, SerdeFormat::Processed)?;
        write_polynomial_slice_streaming(
            &self.permutation.permutations,
            writer,
            SerdeFormat::Processed,
        )
    }

    /// Consume a key while writing the exact same compact-v1 frame as [`Self::write_compact_v1`].
    ///
    /// All shape and basis checks finish before the first output write. The temporary inverse-FFT
    /// copies are then gone; both coefficient vectors and the evaluator are dropped before the
    /// header is emitted. The VK is dropped after its bytes, and each mask/Lagrange polynomial is
    /// consumed and released as it is serialized. Errors and unwinding drop all remaining fields.
    /// No second artifact-sized byte vector is allocated by this method.
    ///
    /// Validation still retains the complete input key plus one temporary domain polynomial and
    /// existing FFT workspace. This does not lower the pre-validation key-generation peak or
    /// guarantee lower process RSS. The caller owns sink flushing and atomic publication; a sink
    /// failure can leave a partial frame, which must not be published.
    pub fn write_compact_v1_consuming<W: Write>(self, writer: &mut W) -> io::Result<()> {
        let length = self.validated_compact_v1_length()?;
        let Self {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation,
            ev,
        } = self;
        let permutation::ProvingKey {
            permutations,
            polys,
        } = permutation;
        drop(fixed_polys);
        drop(polys);
        drop(ev);

        writer.write_all(MAGIC)?;
        writer.write_all(&curve_domain::<C>())?;
        writer.write_all(&length.to_le_bytes())?;
        vk.write(writer, SerdeFormat::Processed)?;
        drop(vk);
        l0.write_consuming(writer, SerdeFormat::Processed)?;
        l_last.write_consuming(writer, SerdeFormat::Processed)?;
        l_active_row.write_consuming(writer, SerdeFormat::Processed)?;
        write_polynomial_vec_consuming(fixed_values, writer, SerdeFormat::Processed)?;
        write_polynomial_vec_consuming(permutations, writer, SerdeFormat::Processed)
    }

    /// Validate the complete encoded shape and both stored bases before either writer emits bytes.
    fn validated_compact_v1_length(&self) -> io::Result<u64> {
        let length = self.compact_v1_bytes_length()?;
        let rows = checked_rows(&self.vk)?;
        if [&self.l0, &self.l_last, &self.l_active_row]
            .iter()
            .any(|polynomial| polynomial.len() != rows)
        {
            return Err(invalid("compact key mask length is inconsistent"));
        }
        validate_bases(
            &self.vk.domain,
            &self.fixed_values,
            &self.fixed_polys,
            self.vk.cs.num_fixed_columns,
            rows,
        )?;
        validate_bases(
            &self.vk.domain,
            &self.permutation.permutations,
            &self.permutation.polys,
            self.vk.cs.permutation.columns.len(),
            rows,
        )?;
        Ok(length)
    }

    /// Decode exactly one compact-v1 frame with trusted domain, length and circuit parameters.
    ///
    /// `expected_bytes` is the exact externally authenticated frame length, not an untrusted
    /// header value. `expected_k` and circuit parameters also come from trusted local policy.
    /// The reader is bounded to this length before any input is read; the header must match it,
    /// and the configured VK determines the exact body size before body allocation. All processed
    /// points/scalars and polynomial counts/lengths use checked readers. An incomplete frame or
    /// extra bytes inside it are rejected. Bytes outside the frame are left to the caller's
    /// framing/EOF policy; authentication must complete before exposing the returned key.
    ///
    /// Fixed/permutation coefficients are reconstructed in the exact VK domain. The result has
    /// both bases and the normal evaluator, so loading does not reduce resident key memory.
    pub fn read_compact_v1_checked<R: Read, ConcreteCircuit: Circuit<C::Scalar>>(
        reader: &mut R,
        expected_k: u32,
        expected_bytes: u64,
        #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
    ) -> io::Result<Self> {
        if expected_bytes < HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "compact frame bound is too small",
            ));
        }
        let mut frame = reader.take(expected_bytes);
        let mut magic = [0; 16];
        let mut curve = [0; 32];
        let mut length = [0; 8];
        frame.read_exact(&mut magic)?;
        if magic != *MAGIC {
            return Err(invalid("unexpected compact proving-key format"));
        }
        frame.read_exact(&mut curve)?;
        if curve != curve_domain::<C>() {
            return Err(invalid("compact proving-key curve domain mismatch"));
        }
        frame.read_exact(&mut length)?;
        if u64::from_le_bytes(length) != expected_bytes {
            return Err(invalid("compact proving-key frame length mismatch"));
        }
        let vk = VerifyingKey::<C>::read_checked::<_, ConcreteCircuit>(
            &mut frame,
            SerdeFormat::Processed,
            expected_k,
            #[cfg(feature = "circuit-params")]
            params,
        )?;
        if byte_len(&vk)? != expected_bytes {
            return Err(invalid(
                "compact proving-key length disagrees with configured shape",
            ));
        }
        let rows = checked_rows(&vk)?;
        let l0 = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        let l_last = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        let l_active_row = Polynomial::read_checked(&mut frame, SerdeFormat::Processed, rows)?;
        let fixed_values = read_polynomial_vec_checked(
            &mut frame,
            SerdeFormat::Processed,
            vk.cs.num_fixed_columns,
            rows,
        )?;
        let permutations = read_polynomial_vec_checked(
            &mut frame,
            SerdeFormat::Processed,
            vk.cs.permutation.columns.len(),
            rows,
        )?;
        if frame.limit() != 0 {
            return Err(invalid("compact proving-key frame was not fully consumed"));
        }
        let fixed_polys = reconstruct(&vk.domain, &fixed_values)?;
        let polys = reconstruct(&vk.domain, &permutations)?;
        let ev = Evaluator::new(vk.cs());
        Ok(Self {
            vk,
            l0,
            l_last,
            l_active_row,
            fixed_values,
            fixed_polys,
            permutation: permutation::ProvingKey {
                permutations,
                polys,
            },
            ev,
        })
    }
}

#[cfg(test)]
mod tests;
