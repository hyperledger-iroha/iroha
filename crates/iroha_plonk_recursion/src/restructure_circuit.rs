//! Constrained verification of a restructuring, bit for bit
//! [`crate::verify_restructure`].
//!
//! The wrap verifies the committed-mask opening of a relaxed claim
//! accumulator's `G*` at `rho`: the statement and the private tape are hashed
//! into the digest whose opening must equal `D`; `zeta` and the nonzero-mapped
//! round challenges are squeezed from the Fold-domain transcript after the
//! public context and `D`; and the complete group equation
//!
//! ```text
//! G* + (v - c b') zeta U + sum_j (u_j^-1 L_j + u_j R_j) - c G' - gamma W = 0,
//! b' = prod_j (1 + rho_{k-1-j} (u_j - 1))
//! ```
//!
//! must hold. Restructuring is the wallet's own compression step, so only hard
//! verification exists: any failure, including a malformed message, leaves
//! the circuit unsatisfiable. The output is the undecided structured claim
//! `(G', u')`, exactly as natively.

use ff::Field;
use group::CurveAffine;
use iroha_pasta::PastaCurve;
use iroha_plonk::{
    frontend::{Error, Region},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{
    Uint, Word,
    bytes::element::LeElement,
    ecc::{AssignedPoint, EccChip},
    range::u128::UintChip,
};

use crate::{
    FOLD_WITNESS_BYTES, K, K_U32,
    codec::{ScalarCells, decode_generator_soft, decode_point_soft, decode_scalar_soft},
    restructure::{DIGEST_DOMAIN, context},
    transcript::{Domain, TranscriptChip},
    verifier::{GeneratorClaimCells, VerifierChip},
};

/// The pinned parameter points and public context of a restructuring.
#[derive(Clone, Debug)]
pub struct RestructurePlan<C: PastaCurve> {
    g0: C::AffineExt,
    g1: C::AffineExt,
    w: C::AffineExt,
    auxiliary: C::AffineExt,
    context: [C::Base; 4],
}

impl<C: PastaCurve> RestructurePlan<C> {
    /// Pins the parameter prefix, `W`, `U` and the restructuring context.
    ///
    /// # Errors
    /// Parameters below k16.
    pub fn new(params: &PinnedParams<C>) -> Result<Self, crate::Error> {
        params.require_k(K_U32).map_err(crate::Error::Parameters)?;
        let g = params.params().g();
        Ok(Self {
            g0: g[0],
            g1: g[1],
            w: params.params().w(),
            auxiliary: params.params().u(),
            context: context(params),
        })
    }
}

/// The statement in circuit. The caller binds it to the restructuring
/// sumcheck that produced `rho` and `v` and to the folded `G*`.
#[derive(Clone, Debug)]
pub struct RestructureStatementCells<C: PastaCurve> {
    /// `G*`, a complete curve-or-identity point.
    pub commitment: AssignedPoint<C::Base>,
    /// `rho`: sixteen canonical scalars, little-endian index bits.
    pub point: Vec<ScalarCells<C>>,
    /// `v`.
    pub value: ScalarCells<C>,
}

/// The constrained result of a restructuring verification.
#[derive(Clone, Debug)]
pub struct RestructureOutputCells<C: PastaCurve> {
    /// The structured claim `(G', u')`, undecided.
    pub claim: GeneratorClaimCells<C>,
    /// Diagnostic: the squeezed `zeta`.
    pub zeta: ScalarCells<C>,
    /// Diagnostic: the private input digest.
    pub digest: Word<C::Base>,
    /// Diagnostic: the mask correction `gamma`.
    pub gamma: ScalarCells<C>,
}

impl<C: PastaCurve> VerifierChip<C> {
    /// Verifies the exact 2,176-byte restructuring witness for `statement`.
    ///
    /// `messages` is `D`, sixteen masked `L`/`R` pairs, `c`, `G'`, `rho_D` and
    /// thirty-two masks, linked by the caller to the same byte tape as the LE32
    /// `length`. The statement cells must already be bound by the caller.
    ///
    /// # Errors
    /// Structural shape mismatches or layout errors. Every byte content that
    /// natively fails makes the circuit unsatisfiable.
    pub fn verify_restructure(
        &mut self,
        region: &mut Region<'_, C::Base>,
        plan: &RestructurePlan<C>,
        statement: &RestructureStatementCells<C>,
        messages: &[LeElement<C::Base>],
        length: &Uint<C::Base, 32>,
    ) -> Result<RestructureOutputCells<C>, Error> {
        if messages.len() != FOLD_WITNESS_BYTES / 32 || statement.point.len() != K {
            return Err(Error::Synthesis);
        }
        let expected_length = self
            .glue
            .constant(region, C::Base::from(FOLD_WITNESS_BYTES as u64))?;
        let mut valid = self
            .glue
            .is_equal(region, length.word(), &expected_length)?;

        // D, rho_D and the masks.
        let d = decode_point_soft::<C>(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            &mut self.ecc,
            region,
            &messages[0],
        )?;
        self.combine(region, &mut valid, &d.valid)?;
        let rho = decode_scalar_soft::<C>(&mut self.uint(), region, &messages[2 * K + 3])?;
        self.combine(region, &mut valid, &rho.valid)?;
        let mut masks = Vec::with_capacity(2 * K);
        for message in &messages[2 * K + 4..] {
            let mask = decode_scalar_soft::<C>(&mut self.uint(), region, message)?;
            self.combine(region, &mut valid, &mask.valid)?;
            masks.push(mask.value);
        }

        // The private digest of the context, G*, rho, v and every mask; its
        // opening must be D.
        let mut words = Vec::new();
        for x in plan.context {
            words.push(self.glue.constant(region, x)?);
        }
        words.extend([
            statement.commitment.x().clone(),
            statement.commitment.y().clone(),
        ]);
        let scalars = statement
            .point
            .iter()
            .chain(core::iter::once(&statement.value))
            .chain(&masks);
        for scalar in scalars {
            if let Some(x) = scalar.native_word() {
                words.push(x.clone());
            } else {
                words.extend([scalar.lo().word().clone(), scalar.hi().word().clone()]);
            }
        }
        let mut duplex = self.duplex.take().ok_or(Error::Synthesis)?;
        if !duplex.is_clear() || duplex.buffered() != 0 {
            return Err(Error::Synthesis);
        }
        duplex.absorb_constant(C::Base::from(u64::from_le_bytes(DIGEST_DOMAIN)));
        duplex.absorb_constant(C::Base::from(words.len() as u64));
        for word in &words {
            duplex.absorb(word);
        }
        let digest = duplex.squeeze_and_clear(region)?;
        let limbs = iroha_plonk_gadgets::ff::CanonicalS6::from_native_word(
            &mut self.uint(),
            region,
            &digest,
        )?;
        let lo = ScalarCells::<C>::from_native_word(&mut self.uint(), region, limbs.lo().word())?;
        let hi = ScalarCells::<C>::from_native_word(&mut self.uint(), region, limbs.hi().word())?;
        let mut opening = self.ecc.constant_point(region, &C::identity())?;
        for (point, scalar) in [(plan.g0, &lo), (plan.g1, &hi), (plan.w, &rho.value)] {
            let point = self.ecc.constant_point(region, &point.to_curve())?;
            let scalar = self.import(region, scalar)?;
            let term = self.scale_point(region, &point, &scalar)?;
            opening = self.ecc.add(region, &opening, &term)?;
        }
        let neg_d = EccChip::<C>::neg(&mut self.glue, region, d.value.point())?;
        let difference = self.ecc.add(region, &opening, &neg_d)?;
        let opening_ok = EccChip::<C>::is_identity(&mut self.glue, region, &difference)?;
        self.combine(region, &mut valid, &opening_ok)?;

        // The Fold-domain transcript: context, D, zeta, then the rounds.
        let mut transcript = TranscriptChip::from_duplex(duplex, Domain::Fold)?;
        for value in plan.context {
            transcript.common_constant(value);
        }
        transcript.common_point(&d.value);
        let zeta_cells = transcript.squeeze_scalar::<C>(&mut self.uint(), region)?;
        let zeta = self.import(region, &zeta_cells)?;

        let mut equation = statement.commitment.clone();
        let mut gamma_pairs = Vec::with_capacity(2 * K);
        let mut rounds = Vec::with_capacity(K);
        for round in 0..K {
            let left = decode_point_soft::<C>(
                &mut UintChip::new(&mut self.glue, &mut self.range),
                &mut self.ecc,
                region,
                &messages[1 + 2 * round],
            )?;
            let right = decode_point_soft::<C>(
                &mut UintChip::new(&mut self.glue, &mut self.range),
                &mut self.ecc,
                region,
                &messages[2 + 2 * round],
            )?;
            self.combine(region, &mut valid, &left.valid)?;
            self.combine(region, &mut valid, &right.valid)?;
            transcript.common_point(&left.value);
            transcript.common_point(&right.value);
            let cells = transcript.squeeze_scalar::<C>(&mut self.uint(), region)?;
            let cells = cells.nonzero_or_one(&mut self.uint(), region)?;
            let challenge = self.import(region, &cells)?;
            let inverse = self.arithmetic.ff.inverse(region, &challenge)?;
            let left_mask = self.import(region, &masks[2 * round])?;
            let right_mask = self.import(region, &masks[2 * round + 1])?;
            gamma_pairs.push((left_mask, inverse.clone()));
            gamma_pairs.push((right_mask, challenge.clone()));
            let left_term = self.scale_point(region, left.value.point(), &inverse)?;
            let right_term = self.scale_point(region, right.value.point(), &challenge)?;
            equation = self.ecc.add(region, &equation, &left_term)?;
            equation = self.ecc.add(region, &equation, &right_term)?;
            rounds.push(cells);
        }
        let coefficient = decode_scalar_soft::<C>(&mut self.uint(), region, &messages[1 + 2 * K])?;
        self.combine(region, &mut valid, &coefficient.valid)?;
        // As in a fold, the private c is never absorbed into a live continuation.
        let suffix = decode_generator_soft::<C>(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            &mut self.ecc,
            region,
            &messages[2 + 2 * K],
        )?;
        self.combine(region, &mut valid, &suffix.valid)?;
        self.duplex = Some(transcript.into_duplex());

        // b' = prod_j (1 + rho_{k-1-j} (u_j - 1)): u_j folds the top index bit
        // first, rho is little-endian.
        let one = self.constant(region, C::ScalarExt::ONE)?;
        let mut folded = one.clone();
        for (cells, rho_cells) in rounds.iter().zip(statement.point.iter().rev()) {
            let challenge = self.import(region, cells)?;
            let coordinate = self.import(region, rho_cells)?;
            let shifted = self.sub(region, &challenge, &one)?;
            let product = self.mul(region, &coordinate, &shifted)?;
            let factor = self.add(region, &one, &product)?;
            folded = self.mul(region, &folded, &factor)?;
        }
        let c = self.import(region, &coefficient.value)?;
        let value = self.import(region, &statement.value)?;
        let opened = self.mul(region, &c, &folded)?;
        let auxiliary_coefficient = self.sub(region, &value, &opened)?;
        let auxiliary_coefficient = self.mul(region, &auxiliary_coefficient, &zeta)?;
        let auxiliary = self
            .ecc
            .constant_point(region, &plan.auxiliary.to_curve())?;
        let term = self.scale_point(region, &auxiliary, &auxiliary_coefficient)?;
        equation = self.ecc.add(region, &equation, &term)?;
        let neg_c = self.neg(region, &c)?;
        let term = self.scale_point(region, suffix.value.point(), &neg_c)?;
        equation = self.ecc.add(region, &equation, &term)?;
        // The masks and round challenges keep their checked imports; batched
        // dot products compute gamma modulo the scalar field.
        let mut gamma = self.constant(region, C::ScalarExt::ZERO)?;
        for chunk in gamma_pairs.chunks(8) {
            let pairs = chunk.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>();
            let part = self.arithmetic.dot(
                &mut UintChip::new(&mut self.glue, &mut self.range),
                region,
                &pairs,
            )?;
            gamma = self.add(region, &gamma, &part)?;
        }
        let w = self.ecc.constant_point(region, &plan.w.to_curve())?;
        let negative_gamma = self.neg(region, &gamma)?;
        let correction = self.scale_point(region, &w, &negative_gamma)?;
        equation = self.ecc.add(region, &equation, &correction)?;
        let gamma = self.export(region, &gamma)?;
        let zero = EccChip::<C>::is_identity(&mut self.glue, region, &equation)?;
        self.combine(region, &mut valid, &zero)?;
        self.glue
            .enforce_constant(region, valid.word(), C::Base::ONE)?;
        Ok(RestructureOutputCells {
            claim: GeneratorClaimCells {
                k: K_U32,
                g: suffix.value,
                challenges: rounds,
            },
            zeta: zeta_cells,
            digest,
            gamma,
        })
    }
}
