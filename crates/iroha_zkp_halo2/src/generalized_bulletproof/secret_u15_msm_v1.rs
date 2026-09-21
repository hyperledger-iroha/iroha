//! Exact fifteen-bit private digits under one immutable canonical public basis.
//!
//! This arithmetic child does not issue resource authority. Its concrete MKHE
//! caller must retain the actual original-session reservation before entering
//! allocation or reading secret coefficients. No arbitrary scalar is truncated.
use super::*;
use zeroize::Zeroizing;

pub(crate) const SECRET_U15_PLANE_LEN_V1: usize = 16_384;
const DIGIT_BOUND_V1: u16 = 1 << 15;
const DIGIT_WINDOWS_V1: usize = 4;

/// Move-only exact-length digits, erased before their allocation is released.
pub(crate) struct SecretU15PlaneV1 {
    values: Vec<u16>,
}
impl SecretU15PlaneV1 {
    /// Allocate first, then copy each bounded value from its existing owner.
    pub(crate) fn from_source_v1(
        exact_len: usize,
        mut source: impl FnMut(usize) -> Result<u16, GeneralizedBulletproofErrorV1>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if exact_len != SECRET_U15_PLANE_LEN_V1 {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        test_allocation_boundary_v1(AllocationSiteV1::Digits)?;
        let mut owned = Self {
            values: try_exact_capacity_vec_v1(SECRET_U15_PLANE_LEN_V1)?,
        };
        for index in 0..SECRET_U15_PLANE_LEN_V1 {
            let value = Zeroizing::new(source(index)?);
            if *value >= DIGIT_BOUND_V1 {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
            owned.values.push(*value);
        }
        Ok(owned)
    }

    pub(crate) const fn owned_bytes_v1() -> usize {
        SECRET_U15_PLANE_LEN_V1 * core::mem::size_of::<u16>() + core::mem::size_of::<Self>()
    }
}
impl Drop for SecretU15PlaneV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.values);
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *values);
        #[cfg(test)]
        record_digit_clear_v1(values);
    }
}

/// Session-owned table from the suite's canonical generators, never caller points.
/// Its public allocation is immutable after construction and is not globally cached.
pub(crate) struct CanonicalU15PublicTableV1<S: ProofSuite> {
    tables: Vec<[S::Point; SECRET_MSM_TABLE_ENTRIES_V1]>,
    blinding_generator: S::Point,
}
impl<S: ProofSuite> CanonicalU15PublicTableV1<S> {
    /// Allocate the exact table before borrowing the canonical public basis.
    pub(crate) fn new_v1() -> Result<Self, GeneralizedBulletproofErrorV1> {
        test_allocation_boundary_v1(AllocationSiteV1::Table)?;
        let mut tables = try_exact_capacity_vec_v1(SECRET_U15_PLANE_LEN_V1)?;
        let generators = S::generators().reduce(SECRET_U15_PLANE_LEN_V1)?;
        for generator in generators.g_bold {
            let mut row = [S::Point::identity(); SECRET_MSM_TABLE_ENTRIES_V1];
            for index in 1..SECRET_MSM_TABLE_ENTRIES_V1 {
                row[index] = row[index - 1] + *generator;
                #[cfg(test)]
                record_work_v1(WorkKindV1::TableAdd);
            }
            tables.push(row);
        }
        Ok(Self {
            tables,
            blinding_generator: generators.h,
        })
    }

    pub(crate) const fn heap_bytes_v1() -> usize {
        SECRET_U15_PLANE_LEN_V1 * core::mem::size_of::<[S::Point; SECRET_MSM_TABLE_ENTRIES_V1]>()
    }

    pub(crate) const fn construction_scratch_bytes_v1() -> usize {
        core::mem::size_of::<[S::Point; SECRET_MSM_TABLE_ENTRIES_V1]>()
            + core::mem::size_of::<ProofGeneratorView<'static, S>>()
    }

    /// Evaluate the bounded plane with four fixed windows and the original
    /// full-width one-term blinding builder. The digit plane is consumed here.
    pub(crate) fn commitment_v1(
        &mut self,
        digits: SecretU15PlaneV1,
        blinding: &S::Scalar,
    ) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1> {
        if digits.values.len() != SECRET_U15_PLANE_LEN_V1
            || digits.values.capacity() != SECRET_U15_PLANE_LEN_V1
            || self.tables.len() != SECRET_U15_PLANE_LEN_V1
            || self.tables.capacity() != SECRET_U15_PLANE_LEN_V1
            || blinding.is_zero()
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let mut accumulated = SecretPoint::new(S::Point::identity());
        for (rows, values) in self
            .tables
            .chunks(SECRET_MSM_CHUNK_TERMS_V1)
            .zip(digits.values.chunks(SECRET_MSM_CHUNK_TERMS_V1))
        {
            accumulated.add_assign_secret(evaluate_digit_chunk_v1::<S>(rows, values)?);
            #[cfg(test)]
            record_work_v1(WorkKindV1::Fold);
        }
        drop(digits);
        // Arbitrary canonical rho always retains all 256 bits and the existing
        // secret-independent implementation. Never use the digit kernel here.
        let mut rho = SecretMultiexpBuilder::<S>::new(1)?;
        rho.push(blinding, &self.blinding_generator)?;
        #[cfg(test)]
        record_work_v1(WorkKindV1::RhoTerm);
        accumulated.add_assign_secret(rho.evaluate()?);
        #[cfg(test)]
        record_work_v1(WorkKindV1::Combine);
        if accumulated.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::PointIdentity);
        }
        Ok(accumulated)
    }

    /// Conservative payload sum of this kernel's named mutable owners and the
    /// unchanged one-term rho builder. Caller inputs, canonical basis creation,
    /// allocator metadata and compiler call frames remain outside this counter.
    pub(crate) const fn evaluation_scratch_bytes_v1() -> usize {
        SecretU15PlaneV1::owned_bytes_v1()
            + core::mem::size_of::<SecretMultiexpBuilder<S>>()
            + core::mem::size_of::<SecretMsmTerm<S>>()
            + core::mem::size_of::<SecretMsmChunkResults<S>>()
            + core::mem::size_of::<Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1>>()
            + core::mem::size_of::<SecretPointTable<S::Point>>()
            + 2 * core::mem::size_of::<SecretPointTableRow<S::Point>>()
            + core::mem::size_of::<SecretScalarEncodings>()
            + core::mem::size_of::<SecretBytes<32>>()
            + 12 * core::mem::size_of::<SecretPoint<S::Point>>()
            + core::mem::size_of::<Zeroizing<u16>>()
    }
}

fn evaluate_digit_chunk_v1<S: ProofSuite>(
    tables: &[[S::Point; SECRET_MSM_TABLE_ENTRIES_V1]],
    digits: &[u16],
) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1> {
    if tables.is_empty() || tables.len() > SECRET_MSM_CHUNK_TERMS_V1 || tables.len() != digits.len()
    {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let mut result = SecretPoint::new(S::Point::identity());
    for window in (0..DIGIT_WINDOWS_V1).rev() {
        #[cfg(test)]
        record_work_v1(WorkKindV1::Window);
        for _ in 0..SECRET_MSM_WINDOW_BITS_V1 {
            result.double_assign();
            #[cfg(test)]
            record_work_v1(WorkKindV1::Double);
        }
        let shift = window * SECRET_MSM_WINDOW_BITS_V1;
        for (table, digit) in tables.iter().zip(digits) {
            let mut selected = SecretPoint::new(S::Point::identity());
            for (candidate, point) in table.iter().enumerate() {
                // Fixed accesses: neither the digit nor its nibble is an index.
                let difference = Zeroizing::new(((digit >> shift) & 15) ^ candidate as u16);
                let choice = ((difference.wrapping_sub(1) >> 8) & 1) as u8;
                selected.select_assign(point, choice);
                #[cfg(test)]
                record_work_v1(WorkKindV1::Select);
            }
            result.add_assign_secret(selected);
            #[cfg(test)]
            record_work_v1(WorkKindV1::WindowAdd);
        }
    }
    Ok(result)
}

#[derive(Clone, Copy)]
enum AllocationSiteV1 {
    Table,
    Digits,
}
#[inline]
fn test_allocation_boundary_v1(
    _site: AllocationSiteV1,
) -> Result<(), GeneralizedBulletproofErrorV1> {
    #[cfg(test)]
    test_controls_v1::allocation_v1(_site)?;
    Ok(())
}

#[cfg(test)]
#[path = "secret_u15_msm_v1_tests.rs"]
mod tests;
#[cfg(test)]
use test_controls_v1::{WorkKindV1, record_digit_clear_v1, record_work_v1};
#[cfg(test)]
pub(crate) use tests::test_controls_v1;
