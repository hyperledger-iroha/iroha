//! Fixed A-split context framing; every continuation input is rebound by hash.

use super::{AProofPlan, LineagePublicCells, VerifiedQCells, VestaClaimCells};
use crate::operation_relation::{state::StateCells, statement::StatementCells};
use ff::PrimeField;
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    GlueChip, Uint, Word,
    bytes::tape::{ByteOrder, ByteRun, SegmentSpec},
    ecc::NonIdentityPoint,
};
use iroha_plonk_recursion::{
    accumulation_circuit::{FoldInputCells, FoldSource},
    codec::ScalarCells,
    obligation::{ModeCells, ledger::Variant},
    verifier::VerifierChip,
};

/// Separate digest domain for an internal split context, never a lineage head.
pub const CONTEXT_DOMAIN: [u8; 8] = *b"kgwctx_1";
const TAPE_DOMAIN: [u8; 8] = *b"kgwctap1";

/// A circuit-fixed external object category and exact carrier capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextObjectSpec {
    /// Unique nonzero category within this context schema.
    pub tag: u32,
    /// Fixed payload capacity, excluding its LE32 actual-length prefix.
    pub capacity: u32,
}
/// Exact object digest, actual length and byte commitment from one bound tape.
#[derive(Clone, Debug)]
pub struct ContextObjectCells {
    spec: ContextObjectSpec,
    authenticated_digest: Word<Fp>,
    length: Uint<Fp, 32>,
    tape_digest: Word<Fp>,
}
impl ContextObjectCells {
    /// Bind an object identity and its exact fixed carrier bytes.
    /// The operation/Q relation must authenticate `authenticated_digest` and
    /// tie it to these bytes; the context makes that same binding cross A1/A2.
    /// Actual lengths above the buffer capacity remain representable for soft
    /// malformed-input verdicts. The framing itself never claims authentication.
    ///
    /// # Errors
    /// Wrong fixed capacity/chunking, missing LE32 segment or layout failure.
    pub fn from_run(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        spec: &ContextObjectSpec,
        authenticated_digest: &Word<Fp>,
        run: &ByteRun<Fp>,
    ) -> Result<Self, Error> {
        if spec.tag == 0
            || run.len()
                != usize::try_from(spec.capacity)
                    .map_err(|_| Error::BoundsFailure)?
                    .checked_add(4)
                    .ok_or(Error::BoundsFailure)?
        {
            return Err(Error::Synthesis);
        }
        let mut offset = 0;
        for segment in run.primary() {
            let expected = (run.len() - offset).min(31);
            if segment.spec() != SegmentSpec::little(offset, expected)
                || segment.spec().order != ByteOrder::Little
            {
                return Err(Error::Synthesis);
            }
            offset += expected;
        }
        if offset != run.len() {
            return Err(Error::Synthesis);
        }
        let length = chip.uint().range_check::<32>(
            region,
            run.secondary_segment(SegmentSpec::little(0, 4))?.word(),
        )?;
        let tag = chip
            .uint()
            .glue()
            .constant(region, Fp::from(u64::from(spec.tag)))?;
        let capacity = chip
            .uint()
            .glue()
            .constant(region, Fp::from(u64::from(spec.capacity)))?;
        let mut words = vec![tag, capacity];
        words.extend(run.primary().iter().map(|s| s.word().clone()));
        let tape_digest = chip.hash_words(region, u64::from_le_bytes(TAPE_DOMAIN), &words)?;
        Ok(Self {
            spec: *spec,
            authenticated_digest: authenticated_digest.clone(),
            length,
            tape_digest,
        })
    }
}
/// State opening plus the exact public lineage prefix used by a split half.
#[derive(Clone, Copy, Debug)]
pub struct ContextState<'a> {
    /// Constrained 33-word core and 8-word rest opening.
    pub state: &'a StateCells,
    /// Corresponding public lineage header.
    pub public: &'a LineagePublicCells,
}
/// Original incoming lineage and both checked transport claims.
#[derive(Clone, Copy, Debug)]
pub struct ContextIncoming<'a> {
    /// Incoming public prefix, including its carried Omega identity.
    pub public: &'a LineagePublicCells,
    /// Checked original claim or its deterministic decoder dummy.
    pub pallas: &'a FoldInputCells<Ep>,
    /// Checked original claim or its deterministic decoder dummy.
    pub vesta: &'a VestaClaimCells,
}
/// Every input a continuation can consume, in a circuit-fixed schema.
/// No boolean here asserts that operation semantics have been checked.
pub struct ContextInputs<'a> {
    /// Validated own statement, of the plan's exact variant.
    pub own_statement: &'a StatementCells,
    /// Expected Send/Receive statement, present for incoming-sigma variants.
    pub incoming_statement: Option<&'a StatementCells>,
    /// Absent exactly for Bootstrap.
    pub predecessor: Option<ContextState<'a>>,
    /// Current operation's state opening and public header.
    pub successor: ContextState<'a>,
    /// Original incoming lineage, only in incoming-Omega variants.
    pub incoming: Option<ContextIncoming<'a>>,
    /// Exact Q instance columns, in the operation's fixed descriptor order.
    pub q_instances: &'a [Vec<Vec<ScalarCells<Ep>>>],
    /// Fixed object/tape schema; order and capacities cannot be witness-chosen.
    pub objects: &'a [ContextObjectCells],
    /// Global incoming modes: transported P, Omega opening, V, then sigma.
    pub modes: &'a [ModeCells<Fp>],
    /// Native corrections for incoming P and Omega opening, in that order.
    pub pallas_corrections: &'a [NonIdentityPoint<Fp>],
    /// Canonical foreign corrections for incoming V, then incoming sigma.
    pub vesta_corrections: &'a [[ScalarCells<Ep>; 2]],
}
/// Immutable operation/partition schema committed by both split halves.
#[derive(Clone, Debug)]
pub struct ContextPlan {
    operation: AProofPlan,
    first_q: usize,
    objects: Vec<ContextObjectSpec>,
    constants: Vec<Fp>,
}
impl ContextPlan {
    /// Pin a nonempty initial Q partition and every source Q key identity.
    /// W's key is pinned only in A2: placing it in A1 would create a VK cycle.
    ///
    /// # Errors
    /// Empty/out-of-range partition, repeated/zero object tags or key mismatch.
    pub fn new(
        operation: AProofPlan,
        first_q: usize,
        objects: Vec<ContextObjectSpec>,
    ) -> Result<Self, Error> {
        if first_q == 0 || first_q > operation.q_count() || objects.iter().any(|s| s.tag == 0) {
            return Err(Error::Synthesis);
        }
        for (i, spec) in objects.iter().enumerate() {
            if objects[..i].iter().any(|p| p.tag == spec.tag) {
                return Err(Error::Synthesis);
            }
        }
        let variant = Variant::ALL
            .iter()
            .position(|v| *v == operation.frame().variant())
            .ok_or(Error::Synthesis)?
            + 1;
        let mut constants = [1, variant, 1, first_q, operation.q_count(), objects.len()]
            .into_iter()
            .map(|v| {
                u64::try_from(v)
                    .map(Fp::from)
                    .map_err(|_| Error::BoundsFailure)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for index in 0..operation.q_count() {
            let q = operation.q(index).ok_or(Error::Synthesis)?;
            constants.push(
                q.key
                    .kagemusha_digest(q.verifier().binding())
                    .map_err(|_| Error::Synthesis)?,
            );
            for limb in q.verifier().binding().digest().chunks_exact(16) {
                constants.push(Fp::from_u128(u128::from_le_bytes(
                    limb.try_into().map_err(|_| Error::Synthesis)?,
                )));
            }
            let d = q.verifier().binding().descriptor();
            constants.push(Fp::from(
                u64::try_from(d.instance_lengths.len()).map_err(|_| Error::BoundsFailure)?,
            ));
            constants.extend(d.instance_lengths.iter().map(|n| Fp::from(u64::from(*n))));
        }
        for spec in &objects {
            constants.extend([
                Fp::from(u64::from(spec.tag)),
                Fp::from(u64::from(spec.capacity)),
            ]);
        }
        Ok(Self {
            operation,
            first_q,
            objects,
            constants,
        })
    }
    /// The complete operation plan, retained across both halves.
    pub const fn operation(&self) -> &AProofPlan {
        &self.operation
    }
    /// Q slots `[0, first_q_count)` belong to A1; every other slot belongs to A2.
    pub const fn first_q_count(&self) -> usize {
        self.first_q
    }
    /// Exact incoming mode count, including separate sigma and Omega obligations.
    pub fn mode_count(&self) -> usize {
        3 * usize::from(self.operation.frame().has_incoming())
            + usize::from(self.operation.sigma.slot_count() == 2)
    }
    /// Recompute the complete context and carried A1 Pallas claim.
    /// The consumer must additionally bind the appropriate verified Q partition
    /// using [`Self::bind_q_partition`] and verify W or produce the A1 proof.
    ///
    /// # Errors
    /// Wrong variant, presence, group size/order or layout failure.
    pub fn digest(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &ContextInputs<'_>,
        carried: &FoldInputCells<Ep>,
    ) -> Result<Word<Fp>, Error> {
        let incoming_sigma = self.operation.sigma.slot_count() == 2;
        let incoming_omega = self.operation.frame().has_incoming();
        if input.own_statement.variant() != self.operation.frame().variant()
            || input.predecessor.is_some() != self.operation.frame().has_predecessor()
            || input.incoming.is_some() != incoming_omega
            || input.incoming_statement.is_some() != incoming_sigma
            || input.q_instances.len() != self.operation.q_count()
            || input.objects.len() != self.objects.len()
            || input.modes.len() != self.mode_count()
            || input.pallas_corrections.len() != 2 * usize::from(incoming_omega)
            || input.vesta_corrections.len()
                != usize::from(incoming_omega) + usize::from(incoming_sigma)
        {
            return Err(Error::Synthesis);
        }
        let mut words = self
            .constants
            .iter()
            .map(|v| chip.uint().glue().constant(region, *v))
            .collect::<Result<Vec<_>, _>>()?;
        words.extend(input.own_statement.fields().iter().cloned());
        if let Some(statement) = input.incoming_statement {
            words.extend(statement.fields().iter().cloned());
        }
        for state in input.predecessor.into_iter().chain([input.successor]) {
            state
                .state
                .bind_lineage(&mut chip.uint(), region, state.public)?;
            words.extend(state.state.core().iter().cloned());
            words.extend(state.state.rest().iter().cloned());
            words.extend(state.public.fields().iter().cloned());
        }
        if let Some(incoming) = input.incoming {
            words.extend(incoming.public.fields().iter().cloned());
            push_pallas(&mut words, incoming.pallas)?;
            if incoming.vesta.source_k() != 16 {
                return Err(Error::Synthesis);
            }
            words.extend(incoming.vesta.words());
        }
        for (index, columns) in input.q_instances.iter().enumerate() {
            let d = self
                .operation
                .q(index)
                .ok_or(Error::Synthesis)?
                .verifier()
                .binding()
                .descriptor();
            if columns.len() != d.instance_lengths.len()
                || columns
                    .iter()
                    .zip(&d.instance_lengths)
                    .any(|(c, n)| c.len() != *n as usize)
            {
                return Err(Error::Synthesis);
            }
            for value in columns.iter().flatten() {
                words.extend([value.lo().word().clone(), value.hi().word().clone()]);
            }
        }
        for (spec, object) in self.objects.iter().zip(input.objects) {
            if spec != &object.spec {
                return Err(Error::Synthesis);
            }
            words.extend([
                object.authenticated_digest.clone(),
                object.length.word().clone(),
                object.tape_digest.clone(),
            ]);
        }
        for mode in input.modes {
            words.extend([
                mode.accept().word().clone(),
                mode.trivial().word().clone(),
                mode.corrected().word().clone(),
            ]);
        }
        for point in input.pallas_corrections {
            words.extend([point.x().clone(), point.y().clone()]);
        }
        for coordinates in input.vesta_corrections {
            for value in coordinates {
                words.extend([value.lo().word().clone(), value.hi().word().clone()]);
            }
        }
        push_pallas(&mut words, carried)?;
        chip.hash_words(region, u64::from_le_bytes(CONTEXT_DOMAIN), &words)
    }
    /// Copy-bind exactly one fixed Q partition to its hard verifier outputs.
    /// A1 uses `first=true`; A2 uses `first=false`. Both halves hash all Q
    /// instances, so neither can substitute an unverified instance column.
    ///
    /// # Errors
    /// Missing, duplicate, reordered or wrong-shaped Q outputs; layout failure.
    pub fn bind_q_partition(
        &self,
        region: &mut Region<'_, Fp>,
        input: &ContextInputs<'_>,
        verified: &[VerifiedQCells],
        first: bool,
    ) -> Result<(), Error> {
        let range = if first {
            0..self.first_q
        } else {
            self.first_q..self.operation.q_count()
        };
        if verified.len() != range.len() || input.q_instances.len() != self.operation.q_count() {
            return Err(Error::Synthesis);
        }
        for (expected, q) in range.zip(verified) {
            let columns = &input.q_instances[expected];
            if q.index != expected || q.instances.len() != columns.len() {
                return Err(Error::Synthesis);
            }
            for (actual, wanted) in q.instances.iter().zip(columns) {
                if actual.len() != wanted.len() {
                    return Err(Error::Synthesis);
                }
                for (a, b) in actual.iter().zip(wanted) {
                    GlueChip::assert_equal(region, a.lo().word(), b.lo().word())?;
                    GlueChip::assert_equal(region, a.hi().word(), b.hi().word())?;
                }
            }
        }
        Ok(())
    }
}
fn push_pallas(words: &mut Vec<Word<Fp>>, claim: &FoldInputCells<Ep>) -> Result<(), Error> {
    if claim.source() != FoldSource::Fixed(16) {
        return Err(Error::Synthesis);
    }
    words.extend([
        claim.source_k().clone(),
        claim.g().x().clone(),
        claim.g().y().clone(),
    ]);
    for value in claim.challenges() {
        words.extend([value.lo().word().clone(), value.hi().word().clone()]);
    }
    Ok(())
}
