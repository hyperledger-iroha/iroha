//! Internal proof composition for contiguous source-program intervals.
//!
//! Every merge verifies two exact pinned child keys, binds all endpoint cells,
//! and retains both curves' IPA obligations. A checkpoint is not a finality
//! capability: its wrapper proof and both generator claims must be retained and
//! decided by the terminal owner. Only source-qualified keys may be installed;
//! this module does not infer a program's semantics from its public layout.

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp, PastaAffine, poseidon::hash_with_domain};
use iroha_plonk::{
    VerifyingKey,
    frontend::{Error, Region},
    pcs::ipa::PinnedParams,
    transcript::decode_point,
};
use iroha_plonk_gadgets::{GlueChip, Uint, Word, cells::to_u128, statement::foreign_limbs};
use iroha_plonk_recursion::{
    PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR,
    accumulation_circuit::{FoldInputCells, FoldPlan, FoldSource},
    codec::ScalarCells,
    verifier::{VerificationMode, VerifierChip, VerifierPlan},
};

use crate::{
    a_relation::{ProofMessageCells, VestaClaimCells},
    omega::OmegaPlan,
};

/// Domain for exact source endpoints and the complete carried Pallas obligation.
pub const SOURCE_BINDING_DOMAIN: u64 = u64::from_le_bytes(*b"kgwfinb1");

/// Native instance construction for a source leaf with explicit deciding fillers.
/// This computes exactly [`SourceCheckpoint::leaf`] and [`SourceCheckpoint::frame`];
/// it never verifies a source program or creates finality authority.
/// # Errors
/// Invalid endpoint geometry or pinned point encoding.
pub fn leaf_frame_native(endpoints: [Fp; 6]) -> Result<[Fp; 69], Error> {
    let start = to_u128(&endpoints[2]).ok_or(Error::Synthesis)?;
    let end = to_u128(&endpoints[3]).ok_or(Error::Synthesis)?;
    if endpoints[0] == Fp::ZERO || start >= end || end > u128::from(u32::MAX) {
        return Err(Error::Synthesis);
    }
    let pallas = decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?;
    let (px, py) = Option::from(pallas.coordinates()).ok_or(Error::Synthesis)?;
    let mut binding = endpoints.to_vec();
    binding.extend([Fp::from(16), px, py]);
    for _ in 0..16 {
        binding.extend([Fp::ONE, Fp::ZERO]);
    }
    let digest = hash_with_domain(SOURCE_BINDING_DOMAIN, &binding);
    let vesta = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?;
    let (vx, vy) = Option::from(vesta.coordinates()).ok_or(Error::Synthesis)?;
    let coordinates = [vx, vy]
        .into_iter()
        .flat_map(|v| foreign_limbs(&v).map(Fp::from_u128))
        .collect::<Vec<_>>();
    let mut vclaim = coordinates.clone();
    vclaim.extend([Fp::ONE; 16]);
    let mut frame = vec![digest, Fp::from(16)];
    for _ in 0..3 {
        frame.extend_from_slice(&vclaim);
    }
    frame.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    frame.extend(coordinates);
    frame.try_into().map_err(|_| Error::Synthesis)
}

/// A fixed source program's context and exact interval/state boundaries.
#[derive(Clone, Debug)]
pub struct SourceEndpoints {
    program: Word<Fp>,
    context: Word<Fp>,
    start: Uint<Fp, 32>,
    end: Uint<Fp, 32>,
    before: Word<Fp>,
    after: Word<Fp>,
}
impl SourceEndpoints {
    /// Bind a leaf to circuit-fixed program identity and a nonempty fixed interval.
    /// The leaf owner must derive both state digests from every live register and
    /// constrain the actual operations between them; hashing an asserted result
    /// alone proves nothing about that result.
    /// # Errors
    /// Zero program identity, empty/reversed interval, or layout errors.
    #[allow(clippy::too_many_arguments)]
    pub fn leaf(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        program_id: Fp,
        context: &Word<Fp>,
        start: u32,
        end: u32,
        before: &Word<Fp>,
        after: &Word<Fp>,
    ) -> Result<Self, Error> {
        if program_id == Fp::ZERO || start >= end {
            return Err(Error::Synthesis);
        }
        Ok(Self {
            program: chip.uint().glue().constant(region, program_id)?,
            context: context.clone(),
            start: chip.uint().constant(region, u128::from(start))?,
            end: chip.uint().constant(region, u128::from(end))?,
            before: before.clone(),
            after: after.clone(),
        })
    }

    /// Constrain an untrusted child's endpoint opening before verifying its wrapper.
    /// This constructor grants no authority. Every cell enters the child binding
    /// digest recomputed by [`SourceMergePlan::merge`].
    /// # Errors
    /// Layout errors; zero program or empty/reversed intervals are unsatisfiable.
    pub fn from_words(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        words: &[Word<Fp>; 6],
    ) -> Result<Self, Error> {
        chip.uint().glue().assert_nonzero(region, &words[0])?;
        let start = chip.uint().range_check::<32>(region, &words[2])?;
        let end = chip.uint().range_check::<32>(region, &words[3])?;
        chip.uint().assert_lt(region, &start, &end)?;
        Ok(Self {
            program: words[0].clone(),
            context: words[1].clone(),
            start,
            end,
            before: words[4].clone(),
            after: words[5].clone(),
        })
    }

    /// Exact order `[program, context, start, end, before, after]`.
    pub fn words(&self) -> [Word<Fp>; 6] {
        [
            self.program.clone(),
            self.context.clone(),
            self.start.word().clone(),
            self.end.word().clone(),
            self.before.clone(),
            self.after.clone(),
        ]
    }

    fn join(region: &mut Region<'_, Fp>, left: &Self, right: &Self) -> Result<Self, Error> {
        GlueChip::assert_equal(region, &left.program, &right.program)?;
        GlueChip::assert_equal(region, &left.context, &right.context)?;
        GlueChip::assert_equal(region, left.end.word(), right.start.word())?;
        GlueChip::assert_equal(region, &left.after, &right.before)?;
        Ok(Self {
            program: left.program.clone(),
            context: left.context.clone(),
            start: left.start.clone(),
            end: right.end.clone(),
            before: left.before.clone(),
            after: right.after.clone(),
        })
    }
}

fn binding_digest(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    endpoints: &SourceEndpoints,
    pallas: &FoldInputCells<Ep>,
) -> Result<Word<Fp>, Error> {
    if pallas.source() != FoldSource::Fixed(16) {
        return Err(Error::Synthesis);
    }
    let mut words = endpoints.words().to_vec();
    words.extend([
        pallas.source_k().clone(),
        pallas.g().x().clone(),
        pallas.g().y().clone(),
    ]);
    for u in pallas.challenges() {
        words.extend([u.lo().word().clone(), u.hi().word().clone()]);
    }
    chip.hash_words(region, SOURCE_BINDING_DOMAIN, &words)
}

fn trivial_pallas(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
) -> Result<FoldInputCells<Ep>, Error> {
    let point = decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?;
    let point = chip.constant_point(region, &Ep::from(point))?;
    let one = chip.uint().glue().constant(region, Fp::ONE)?;
    let one = ScalarCells::from_native_word(&mut chip.uint(), region, &one)?;
    FoldInputCells::from_normalized(
        chip,
        region,
        16,
        point,
        core::array::from_fn(|_| one.clone()),
    )
}

/// Internal source output with an undecided Pallas claim and two Vesta fold slots.
/// The public wrapper carries the folded Vesta claim; neither may be dropped.
#[derive(Clone, Debug)]
pub struct SourceCheckpoint {
    endpoints: SourceEndpoints,
    digest: Word<Fp>,
    pallas: FoldInputCells<Ep>,
    vesta: [VestaClaimCells; 2],
}
impl SourceCheckpoint {
    /// Close a real arithmetic leaf using explicit deciding fillers only.
    /// This is a framing component, not a replacement for the leaf's constraints.
    /// # Errors
    /// Layout or pinned filler decoding errors.
    pub fn leaf(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        endpoints: SourceEndpoints,
    ) -> Result<Self, Error> {
        let pallas = trivial_pallas(chip, region)?;
        let digest = binding_digest(chip, region, &endpoints, &pallas)?;
        let trivial = VestaClaimCells::trivial(chip, region)?;
        Ok(Self {
            endpoints,
            digest,
            pallas,
            vesta: [trivial.clone(), trivial],
        })
    }
    /// Exact source endpoints retained by the caller and bound in the wrapper digest.
    pub const fn endpoints(&self) -> &SourceEndpoints {
        &self.endpoints
    }
    /// Source binding digest exported by the internal wrapper.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Complete carried Pallas obligation, never an acceptance verdict.
    pub const fn pallas(&self) -> &FoldInputCells<Ep> {
        &self.pallas
    }

    /// Existing 69-word wrapper input, using separately installed internal keys.
    /// Slots retain left Vesta, own A opening, right Vesta and a pinned trivial
    /// claim. No lineage, operation, sigma or Payment authority is implied.
    /// # Errors
    /// Layout errors or a non-k16 child Vesta claim.
    pub fn frame(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Vec<Word<Fp>>, Error> {
        if self.vesta.iter().any(|v| v.source_k() != 16) {
            return Err(Error::Synthesis);
        }
        let mut words = vec![
            self.digest.clone(),
            chip.uint().glue().constant(region, Fp::from(16))?,
        ];
        words.extend(self.vesta[0].words());
        words.extend(self.vesta[1].words());
        let trivial = VestaClaimCells::trivial(chip, region)?;
        words.extend(trivial.words());
        for bit in [0, 1, 0] {
            words.push(chip.uint().glue().constant(region, Fp::from(bit))?);
        }
        for coordinate in trivial.coordinates() {
            words.extend([
                coordinate.lo().word().clone(),
                coordinate.hi().word().clone(),
            ]);
        }
        Ok(words)
    }
}

/// Original child data; no field is trusted before its pinned wrapper verifies.
#[derive(Clone, Copy)]
pub struct SourceChild<'a> {
    /// Opened exact program/context and interval/state boundaries.
    pub endpoints: &'a SourceEndpoints,
    /// Carried Pallas claim committed by the child binding digest.
    pub pallas: &'a FoldInputCells<Ep>,
    /// Exact Vesta claim from the child wrapper's public columns.
    pub vesta: &'a VestaClaimCells,
    /// Original descriptor-sized wrapper proof bytes and bound length.
    pub proof: &'a ProofMessageCells,
}

/// Fixed two-child verifier programs and complete hard Pallas fold schedule.
#[derive(Clone, Debug)]
pub struct SourceMergePlan {
    children: [(VerifierPlan<Ep>, VerifyingKey<Ep>); 2],
    fold: FoldPlan<Ep>,
    params: PinnedParams<Ep>,
}
impl SourceMergePlan {
    /// Pin both complete child keys. Their actual source semantics must be
    /// qualified by installation; descriptor compatibility alone is insufficient.
    /// # Errors
    /// Wrong descriptor, wrapper shape/types, key binding or parameter length.
    pub fn new(
        children: [(VerifierPlan<Ep>, VerifyingKey<Ep>); 2],
        params: &PinnedParams<Ep>,
    ) -> Result<Self, Error> {
        for (plan, key) in &children {
            let d = plan.binding().descriptor();
            if d.k != 16
                || d.instance_lengths != [1, 2, 16]
                || d.instance_types.as_deref() != Some(&OmegaPlan::instance_types())
                || key.descriptor_digest() != plan.binding().digest()
            {
                return Err(Error::Synthesis);
            }
            key.kagemusha_digest(plan.binding())
                .map_err(|_| Error::Synthesis)?;
        }
        let fold = FoldPlan::with_sources(params, vec![FoldSource::Fixed(16); 4])
            .map_err(|_| Error::Synthesis)?;
        Ok(Self {
            children,
            fold,
            params: params.clone(),
        })
    }

    /// Verify both children, prove exact continuity, and fold all four Pallas
    /// obligations in `[left carry, left opening, right carry, right opening]` order.
    /// Both Vesta claims enter the returned A frame for the mandatory wrapper fold.
    /// # Errors
    /// Invalid metadata/layout; failed proofs, joins or folds are unsatisfiable.
    pub fn merge(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        children: [SourceChild<'_>; 2],
        fold_proof: &ProofMessageCells,
    ) -> Result<SourceCheckpoint, Error> {
        let mut claims = Vec::with_capacity(4);
        for ((plan, key), child) in self.children.iter().zip(&children) {
            if child.vesta.source_k() != 16 {
                return Err(Error::Synthesis);
            }
            let digest = binding_digest(chip, region, child.endpoints, child.pallas)?;
            let digest = ScalarCells::from_native_word(&mut chip.uint(), region, &digest)?;
            let mut challenges = Vec::with_capacity(16);
            for u in child.vesta.challenges() {
                challenges.push(ScalarCells::from_native_word(&mut chip.uint(), region, u)?);
            }
            let instances = vec![vec![digest], child.vesta.coordinates().to_vec(), challenges];
            let key = chip.constant_key(region, plan, key)?;
            let verified = chip.verify(
                region,
                plan,
                &key,
                &instances,
                child.proof.messages(),
                child.proof.length(),
                VerificationMode::Hard,
            )?;
            claims.push(child.pallas.clone());
            claims.push(FoldInputCells::from_claim(chip, region, &verified.claim)?);
        }
        let endpoints =
            SourceEndpoints::join(region, children[0].endpoints, children[1].endpoints)?;
        let folded = chip.verify_fold(
            region,
            &self.fold,
            &claims,
            fold_proof.messages(),
            fold_proof.length(),
            VerificationMode::Hard,
        )?;
        let pallas = FoldInputCells::from_claim(chip, region, &folded.claim)?;
        let digest = binding_digest(chip, region, &endpoints, &pallas)?;
        Ok(SourceCheckpoint {
            endpoints,
            digest,
            pallas,
            vesta: [children[0].vesta.clone(), children[1].vesta.clone()],
        })
    }
}

mod circuit;
pub use circuit::{SourceMergeCircuit, SourceMergeConfig, SourceNodeEvidence};

#[cfg(test)]
mod tests;
