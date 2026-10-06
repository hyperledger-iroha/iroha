//! Fixed ordered aggregation of exactly one normal-validator quorum.
//!
//! The roster root must come from a genesis-rooted schedule proof. This program
//! binds each selected original compressed key, sums it exactly once, and exports
//! the exact compressed aggregate consumed by the BLS program. Normal consensus
//! admission supplies individual subgroup/PoP validity; these leaves alone do not
//! authenticate a roster or a signature.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{GlueChip, Word, bls12_381::curve::{G1AffineWitness, G1Value}};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};
use super::{
    consensus::QuorumCells,
    continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native},
    roster::verify_key_path,
};

/// Fixed first-release ordered aggregation program.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwaggp1");
/// Start, 31 fixed seat steps, and exact compressed-result check.
pub const PROGRAM_LENGTH: u32 = 33;
/// Context commitment domain, separate from normal consensus authority.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwaggc1");
/// Full running-point state commitment domain.
pub const STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwaggs1");

/// Untrusted aggregate source context; the enclosing finality proof authenticates
/// its root/count and binds its bitmap and output key to the same Commit proof.
#[derive(Clone, Debug)]
pub struct AggregateContext {
    /// Ordered authenticated roster's internal commitment.
    pub roster_root: Fp,
    /// Global committee size, constrained to `3f+1`.
    pub members: u8,
    /// Fault bound, constrained to 1 through 10.
    pub faults: u8,
    /// Exact LSB-first signer bitmap, padded to four bytes.
    pub bitmap: [u8; 4],
    /// Exact nonidentity aggregate key consumed by BLS verification.
    pub aggregate_key: [u8; 48],
}

fn native_pack(bytes: &[u8]) -> Fp {
    bytes.iter().rev().fold(Fp::ZERO, |a, b| a * Fp::from(256) + Fp::from(u64::from(*b)))
}
impl AggregateContext {
    /// Native context encoding for witness generation, not an authorization check.
    pub fn digest(&self) -> Fp {
        let mut words = vec![Fp::from(PROGRAM_ID), self.roster_root, Fp::from(u64::from(self.members)), Fp::from(u64::from(self.faults))];
        words.extend(self.bitmap.map(|v| Fp::from(u64::from(v))));
        words.extend(self.aggregate_key.chunks_exact(16).map(native_pack));
        hash_with_domain(CONTEXT_DOMAIN, &words)
    }
}

/// Bind the exact authenticated roster/quorum/output context in another source.
/// Byte ranges are checked; quorum geometry is checked by every aggregation leaf.
/// # Errors
/// Circuit layout errors; values outside byte range are unsatisfiable.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, roster_root: &Word<Fp>,
    members: &Word<Fp>, faults: &Word<Fp>, bitmap: &[Word<Fp>; 4], aggregate: &[Word<Fp>; 48],
) -> Result<Word<Fp>, Error> {
    let mut words = vec![chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?, roster_root.clone(), members.clone(), faults.clone()];
    for word in [members, faults].into_iter().chain(bitmap.iter()) { chip.uint().range_check::<8>(region, word)?; }
    words.extend(bitmap.iter().cloned());
    for chunk in aggregate.chunks_exact(16) {
        let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
        for byte in chunk.iter().rev() {
            chip.uint().range_check::<8>(region, byte)?;
            packed = chip.uint().glue().linear(region, &[(Fp::from(256), &packed), (Fp::ONE, byte)], Fp::ZERO)?;
        }
        words.push(packed);
    }
    chip.hash_words(region, CONTEXT_DOMAIN, &words)
}

/// Deterministic context-bound starting or completed state, with no registers.
pub fn boundary_digest_native(context: Fp, terminal: bool) -> Fp {
    state_native(context, if terminal { PROGRAM_LENGTH } else { 0 }, None)
}
/// Constrained version of the deterministic starting/completed state digest.
/// # Errors
/// Circuit layout errors.
pub fn boundary_digest_cells(
    chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, context: &Word<Fp>, terminal: bool,
) -> Result<Word<Fp>, Error> {
    state_cells(chip, region, context, if terminal { PROGRAM_LENGTH } else { 0 }, None)
}
fn state_native(context: Fp, cursor: u32, point: Option<&G1AffineWitness>) -> Fp {
    let mut words = vec![Fp::from(PROGRAM_ID), context, Fp::from(u64::from(cursor)), Fp::from(u64::from(point.is_some()))];
    if let Some(p) = point { words.extend(p.x.into_iter().chain(p.y).map(Fp::from)); words.push(Fp::from(u64::from(p.infinity))); }
    hash_with_domain(STATE_DOMAIN, &words)
}
fn state_cells(
    chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, context: &Word<Fp>, cursor: u32, point: Option<&G1Value<Fp>>,
) -> Result<Word<Fp>, Error> {
    let mut words = Vec::new();
    for value in [PROGRAM_ID, u64::from(cursor), u64::from(point.is_some())] {
        words.push(chip.uint().glue().constant(region, Fp::from(value))?);
    }
    words.insert(1, context.clone());
    if let Some(p) = point { words.extend(p.x().limbs().iter().chain(p.y().limbs()).cloned()); words.push(p.infinity().word().clone()); }
    chip.hash_words(region, STATE_DOMAIN, &words)
}

/// One original roster leaf opening and its decompressed affine witness.
#[derive(Clone, Debug)]
pub struct AggregateSeat {
    /// Original compressed key, or zero for an inactive seat.
    pub key: [u8; 48],
    /// Five siblings in the authenticated ordered roster commitment.
    pub path: [Fp; 5],
    /// Untrusted canonical on-curve decomposition; inactive seats may use any
    /// nonidentity point because their selection bit is constrained to zero.
    pub point: G1AffineWitness,
}

/// One actual fixed aggregation transition with untrusted before/after openings.
#[derive(Clone, Debug)]
pub struct AggregateLeafCircuit {
    cursor: u32,
    context: AggregateContext,
    before: Option<G1AffineWitness>,
    after: Option<G1AffineWitness>,
    seat: Option<AggregateSeat>,
    known: bool,
}
/// Verifier-compatible arithmetic lanes and the internal 69-word source frame.
#[derive(Clone, Debug)]
pub struct AggregateConfig { verifier: VerifierConfig<Ep>, public: Column<Instance> }
impl AggregateLeafCircuit {
    /// Select an exact fixed transition and check witness shape only.
    /// Arithmetic, quorum, membership, and output equality are circuit constraints.
    /// # Errors
    /// Cursor outside the program or witness openings for a different phase.
    pub fn new(cursor: u32, context: AggregateContext, before: Option<G1AffineWitness>, after: Option<G1AffineWitness>, seat: Option<AggregateSeat>) -> Result<Self, Error> {
        if cursor >= PROGRAM_LENGTH || before.is_some() != (cursor > 0) || after.is_some() != (cursor < 32) || seat.is_some() != (1..32).contains(&cursor) { return Err(Error::Synthesis); }
        Ok(Self { cursor, context, before, after, seat, known: true })
    }
    /// Exact source endpoints computed from the supplied openings; no authority
    /// is implied until the fixed circuit and complete program are proved.
    pub fn endpoints(&self) -> [Fp; 6] {
        let context = self.context.digest();
        [Fp::from(PROGRAM_ID), context, Fp::from(u64::from(self.cursor)), Fp::from(u64::from(self.cursor + 1)), state_native(context, self.cursor, self.before.as_ref()), state_native(context, self.cursor + 1, self.after.as_ref())]
    }
    /// Exact source frame for native proving. False output openings do not satisfy it.
    /// # Errors
    /// Invalid internal endpoint or pinned filler encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> { Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()]) }
    fn value<T: Copy>(&self, value: T) -> Value<T> { if self.known { Value::known(value) } else { Value::unknown() } }
    fn words<const N: usize>(&self, chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, values: [Fp; N]) -> Result<[Word<Fp>; N], Error> {
        chip.uint().glue().witnesses(region, &values.map(|v| self.value(v)))?.try_into().map_err(|_| Error::Synthesis)
    }
}
impl Circuit<Fp> for AggregateLeafCircuit {
    type Config = AggregateConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self { Self { known: false, ..self.clone() } }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).expect("fixed aggregation profile");
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        AggregateConfig { verifier, public }
    }
    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(|| "exact normal-validator quorum sum", |mut region| {
            let [root, n, f, length] = self.words(&mut chip, &mut region, [self.context.roster_root, Fp::from(u64::from(self.context.members)), Fp::from(u64::from(self.context.faults)), Fp::from(u64::from(self.context.members.div_ceil(8)))])?;
            let bitmap = self.words(&mut chip, &mut region, self.context.bitmap.map(|v| Fp::from(u64::from(v))))?;
            let aggregate = self.words(&mut chip, &mut region, self.context.aggregate_key.map(|v| Fp::from(u64::from(v))))?;
            let quorum = QuorumCells::from_bitmap(&mut chip.uint(), &mut region, &n, &f, &length, &bitmap)?;
            let context = context_digest_cells(&mut chip, &mut region, &root, &n, &f, &bitmap, &aggregate)?;
            let before = self.before.map(|p| chip.bls381().assign_g1(&mut region, self.value(p))).transpose()?;
            let before_digest = state_cells(&mut chip, &mut region, &context, self.cursor, before.as_ref())?;
            let after = if self.cursor == 0 {
                Some(chip.bls381().identity_g1(&mut region)?)
            } else if self.cursor == 32 {
                let encoded = chip.bls381().compressed_g1(&mut region, before.as_ref().ok_or(Error::Synthesis)?)?;
                for (a, b) in encoded.iter().zip(&aggregate) { GlueChip::assert_equal(&mut region, a, b)?; }
                None
            } else {
                let index = u8::try_from(self.cursor - 1).map_err(|_| Error::Synthesis)?;
                let seat = self.seat.as_ref().ok_or(Error::Synthesis)?;
                let key = self.words(&mut chip, &mut region, seat.key.map(|v| Fp::from(u64::from(v))))?;
                let path = self.words(&mut chip, &mut region, seat.path)?;
                verify_key_path(&mut chip, &mut region, index, &key, &path, &root)?;
                let point = chip.bls381().assign_g1(&mut region, self.value(seat.point))?;
                let encoded = chip.bls381().compressed_g1(&mut region, &point)?;
                let position = chip.uint().constant::<5>(&mut region, u128::from(index))?;
                let active = chip.uint().lt(&mut region, &position, quorum.members())?;
                for (original, encoded) in key.iter().zip(&encoded) {
                    let expected = chip.uint().glue().mul(&mut region, active.word(), encoded)?;
                    GlueChip::assert_equal(&mut region, original, &expected)?;
                }
                let before = before.as_ref().ok_or(Error::Synthesis)?;
                let sum = chip.bls381().add_g1(&mut region, before, &point)?;
                Some(chip.bls381().select_g1(&mut region, &quorum.selected()[usize::from(index)], &sum, before)?)
            };
            let after_digest = state_cells(&mut chip, &mut region, &context, self.cursor + 1, after.as_ref())?;
            let endpoints = SourceEndpoints::leaf(&mut chip, &mut region, Fp::from(PROGRAM_ID), &context, self.cursor, self.cursor + 1, &before_digest, &after_digest)?;
            SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?.frame(&mut chip, &mut region)
        })?;
        for (i, word) in output.iter().enumerate() { layouter.constrain_instance(word.cell(), config.public, i)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

mod native;
pub use native::prepare_aggregation;
