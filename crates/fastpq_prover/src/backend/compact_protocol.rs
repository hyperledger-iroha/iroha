//! Fixed AIR interfaces and independent public-digest algebra checks.
//!
//! The sole proof engine is `deep_engine`; this module contains no proof DTO,
//! decoder, transcript, prover, verifier or predecessor protocol implementation.

#[cfg(test)]
use super::{GOLDILOCKS_MODULUS, fixed_domain::FixedTraceDomain};
#[cfg(test)]
use crate::{Error, Result};
#[cfg(test)]
use fastpq_isi::FASTPQ_FINAL_V1;

/// Exact trusted relation geometry and circuit identity; never taken from a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FixedAirSchema {
    /// Subgroup order, including all physical padding.
    pub(super) trace_rows: usize,
    /// Complete committed base-column width in canonical order.
    pub(super) width: usize,
    /// Exact independently mixed numerator count in canonical order.
    pub(super) constraints: usize,
    /// Versioned circuit, column, slot, selector and public-packing identity.
    pub(super) identity: &'static str,
}

/// Prepared prover callback; mutable captures may retain per-proof scratch space.
#[cfg(test)]
pub(super) type ProverEvaluator<'a> =
    Box<dyn FnMut(usize, u64, &[u64], &[u64]) -> Result<Vec<u64>> + Send + 'a>;

/// Immutable prover preparation shared across jobs; each evaluator owns its scratch.
#[cfg(test)]
pub(super) trait PreparedAir: Sync {
    /// Create a worker-local evaluator borrowing only immutable prepared data.
    fn evaluator(&self) -> ProverEvaluator<'_>;
}

/// A caller-authenticated fixed relation with bounded public-input evaluation.
///
/// The normal protocol's closed `DeepRelation` bridge selects the complete AIR.
/// Test-only base-field evaluation and prepared callbacks remain useful for
/// independent polynomial checks; they cannot authorize a proof or declare a
/// protocol geometry. Masking and extension-field degree bounds belong to the
/// normal DEEP polynomial and quotient owners.
pub(super) trait FixedAir: Sync {
    /// Return the fixed geometry and exact constraint/schema identity.
    fn schema(&self) -> FixedAirSchema;
    /// Exact canonical public bytes, already authenticated by the surrounding caller.
    fn statement_bytes(&self) -> &[u8];
    /// Evaluate every base-field numerator at x from complete current/next rows.
    #[cfg(test)]
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>>;
    /// Prepare prover-only acceleration without changing the verifier relation.
    #[cfg(test)]
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>>
    where
        Self: Sized,
    {
        Ok(Box::new(DirectPrepared { relation: self }))
    }
}

#[cfg(test)]
struct DirectPrepared<'a, R: FixedAir + ?Sized> {
    relation: &'a R,
}

#[cfg(test)]
impl<R: FixedAir + ?Sized> PreparedAir for DirectPrepared<'_, R> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        Box::new(move |_, point, current, next| self.relation.evaluate(point, current, next))
    }
}

#[cfg(test)]
fn canonical_base(value: u64, context: &'static str, indices: &[usize]) -> Result<()> {
    if value >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context,
            indices: indices.to_vec(),
        });
    }
    Ok(())
}

/// Independent public-marked-digest polynomial reference for base-field tests.
///
/// The 680 hash slots are followed by eight independently mixed public digest
/// limb equalities at fixed export row407. This does not claim a public preimage
/// or zero knowledge; it tests every bit of the exact marked public export.
#[cfg(test)]
pub(super) struct HashDigestAir {
    ledger: super::compact_hash_quotient::CompactHashQuotient,
    public_digest: [u8; 32],
    public_limbs: [u64; 8],
    export_point: u64,
}

#[cfg(test)]
impl HashDigestAir {
    /// Fix the complete public digest before any proof challenge.
    pub(super) fn new(public_digest: [u8; 32]) -> Result<Self> {
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, 512)?.generator;
        Ok(Self {
            ledger: super::compact_hash_quotient::CompactHashQuotient::new(&FASTPQ_FINAL_V1, 512)?,
            public_limbs: core::array::from_fn(|limb| {
                u64::from(u32::from_le_bytes(
                    public_digest[4 * limb..4 * limb + 4]
                        .try_into()
                        .expect("fixed u32 public digest limb"),
                ))
            }),
            public_digest,
            export_point: super::field_pow(generator, 407),
        })
    }

    fn finish_residues(
        &self,
        point: u64,
        digest: &[u64; 8],
        hash: &super::compact_hash_quotient::HashNumerators<u64>,
    ) -> Result<Vec<u64>> {
        canonical_base(point, "compact_public_digest_point", &[])?;
        let mask = if point == self.export_point {
            1
        } else {
            let numerator = super::mul_mod(
                super::sub_mod(super::field_pow(point, 512), 1),
                self.export_point,
            );
            let denominator = super::mul_mod(512, super::sub_mod(point, self.export_point));
            super::mul_mod(numerator, super::field_inverse(denominator))
        };
        let mut residues = Vec::with_capacity(688);
        residues.extend_from_slice(&hash.local);
        residues.extend_from_slice(&hash.transitions);
        residues.extend(
            digest
                .iter()
                .zip(self.public_limbs)
                .map(|(&value, expected)| super::mul_mod(mask, super::sub_mod(value, expected))),
        );
        Ok(residues)
    }
}

#[cfg(test)]
impl FixedAir for HashDigestAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            trace_rows: 512,
            width: 310,
            constraints: 688,
            identity: "compact-blake2b256:physical512:columns310:hash680:public-marked-digest8:v1",
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.public_digest
    }

    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        let current = crate::gadgets::compact_trace_columns::decode_hash_row(current)?;
        let next = crate::gadgets::compact_trace_columns::decode_hash_row(next)?;
        self.finish_residues(
            point,
            &current.digest,
            &self.ledger.evaluate(point, &current, &next)?,
        )
    }

    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        Ok(Box::new(PreparedHashDigest {
            relation: self,
            cycle: self.ledger.prepare_prover_masks()?,
        }))
    }
}

#[cfg(test)]
struct PreparedHashDigest<'a> {
    relation: &'a HashDigestAir,
    cycle: super::compact_hash_quotient::ProverMaskCycle<'a>,
}

#[cfg(test)]
impl PreparedAir for PreparedHashDigest<'_> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        let mut scratch = self.relation.ledger.evaluation_scratch::<u64>();
        Box::new(move |index, point, current, next| {
            let current = crate::gadgets::compact_trace_columns::decode_hash_row(current)?;
            let next = crate::gadgets::compact_trace_columns::decode_hash_row(next)?;
            let hash = self
                .cycle
                .evaluate_with_scratch(index, &current, &next, &mut scratch)?;
            self.relation.finish_residues(point, &current.digest, &hash)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gadgets::{
        compact_blake2b_air::{CompactHashWitness, CompactRow},
        compact_trace_columns::hash_row_cells,
    };
    #[test]
    fn fixed_public_digest_constraints_cover_every_bit_at_exact_export_position() {
        let bytes = [71; 83];
        let witness = CompactHashWitness::from_bytes(&bytes).unwrap();
        let digest = *iroha_crypto::Hash::new(bytes).as_ref();
        let valid = HashDigestAir::new(digest).unwrap();
        let current = hash_row_cells(&witness.rows()[407]);
        let next = hash_row_cells(&CompactRow::zero());
        assert!(
            valid
                .evaluate(valid.export_point, &current, &next)
                .unwrap()
                .iter()
                .all(|&value| value == 0)
        );
        for bit in 0..256 {
            let mut changed = digest;
            changed[bit / 8] ^= 1 << (bit % 8);
            let relation = HashDigestAir::new(changed).unwrap();
            let residues = relation
                .evaluate(relation.export_point, &current, &next)
                .unwrap();
            assert_ne!(residues[680 + bit / 32], 0, "public bit={bit}");
            assert_eq!(
                residues[680..].iter().filter(|&&value| value != 0).count(),
                1
            );
        }
    }

    #[test]
    fn fixed_air_default_preparation_preserves_the_reference_callback() {
        struct Reference;
        impl FixedAir for Reference {
            fn schema(&self) -> FixedAirSchema {
                FixedAirSchema {
                    trace_rows: 1,
                    width: 1,
                    constraints: 1,
                    identity: "test-only-reference",
                }
            }
            fn statement_bytes(&self) -> &[u8] {
                b"public reference"
            }
            fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
                Ok(vec![point + current[0] + next[0]])
            }
        }
        let reference = Reference;
        let prepared = reference.prepare_prover().unwrap();
        assert_eq!(prepared.evaluator()(0, 2, &[3], &[5]).unwrap(), vec![10]);
    }
}
