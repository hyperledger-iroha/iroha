//! The KAGEMUSHA-Fold compression wraps (output `IVC.md` sections 7 and 11).
//!
//! After the last business instance `A_n` of an operation, four k16 PIPA-R
//! proofs turn the folded state into an Omega in today's transport. Each side
//! keeps one accumulator per relation kind of the other side, so each decider
//! is one batch over every kind ([`WrapSpec`]).
//!
//! - **S**, over `Fp` (a Vesta proof): the commitment side of the Q-side
//!   decider (Pallas group operations) and its bound opening
//!   ([`s::SCircuit`]);
//! - **R**, over `Fq` (a Pallas proof): verifies S and hashes S's Q crossing
//!   into `D_SQ`, the fold of `A_n` into its kind's accumulator (or its
//!   start, when `Q_{n-1}` kept none), `A_n`'s passthrough recomputed as
//!   `Q_{n-1}`'s `h_out`, and the A-side decider's commitment side and bound
//!   opening ([`r::RCircuit`]);
//! - **P**, over `Fp` (a Vesta proof): verifies R, the field side of the
//!   A-side decider against R's values, the Pallas claim accumulator's
//!   committed-mask opening, the Vesta claim accumulator's restructuring
//!   sumcheck, the PIPA-AS fold of the Q decider's claim (S's, forwarded by
//!   R), the restructured Pallas claim and R's claim into `acc_P`, `D_A`,
//!   and `A_n`'s recomputed `h_out` from a state at an operation boundary
//!   over S's Q digests ([`p::PCircuit`]);
//! - **Omega**, over `Fq` (a Pallas proof, today's three public columns
//!   `[D_A]`, `acc_V.G`, `acc_V.u`): verifies P, the field side of the Q-side
//!   decider checked against `D_SQ`, the Pallas claims' restructuring
//!   sumcheck, the Vesta claims' committed-mask opening, and the PIPA-AS fold
//!   of P's claim, R's claim for S, the A decider's claim and the
//!   restructured Vesta claim into `acc_V` ([`omega::OmegaCircuit`]).
//!
//! Each decider is split (DESIGN.md section 9.0): its field messages are
//! native in the circuit over its scalar field, its commitments in a circuit
//! over its commitment curve's base field. The values that cross are rows of
//! a verified proof's public column ([`crossing`]): S's for the Q side (R
//! verifies S and hashes them into `D_SQ`, which Omega recomputes), R's for
//! the A side (P verifies R), P's for the rest (Omega verifies P), so every
//! crossing value is bound on both sides. The prover computes everything
//! natively, then proves S, R, P and Omega in order ([`native`]).
//!
//! Every proof before payout stays: every accumulator, both claim
//! accumulators, S, R and P are decided or folded into the transported
//! `acc_P` (inside `D_A`) and `acc_V`, which the consumer decides as today.

pub mod crossing;
pub mod native;
pub mod omega;
pub mod p;
pub mod r;
pub mod s;

use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    circuit::{FoldLanes, ScalarCell},
    decider_v2::DeciderV2Plan,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve};
use iroha_plonk::{
    frontend::{Error, Region, Value},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{
    Uint,
    bytes::{
        element::{LeElement, decode_le_element, le_message_segments},
        tape::BytesChip,
    },
};
use iroha_plonk_recursion::{FOLD_WITNESS_BYTES, K, verifier::VerifierChip};

use crate::a_relation::LINEAGE_FIELDS;

/// Rounds of every k16 claim and fold (`iroha_plonk_recursion::K`).
pub(crate) const K_U32: u32 = K as u32;

/// Why the wraps could not be built or proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrapError {
    /// Relations, parameters or inputs the wraps do not admit.
    Input,
    /// A native decider, fold or restructuring step failed.
    Fold,
    /// Key generation or proving failed.
    Proof,
}

impl core::fmt::Display for WrapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Input => "fold wrap input not admitted",
            Self::Fold => "fold wrap native step failed",
            Self::Proof => "fold wrap key generation or proof failed",
        })
    }
}

impl std::error::Error for WrapError {}

/// The domains the wraps use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Domains {
    /// Fold domain of every Q kind (the `U_Q` digests and their batch
    /// decider).
    pub q_fold: [u8; 32],
    /// Fold domain of every A kind (the `U_A` digests, `A_n`'s fold and
    /// their batch decider).
    pub a_fold: [u8; 32],
    /// IVC hash domain of the A side (`A_n`'s `h_out`).
    pub a_ivc: [u8; 32],
    /// IVC hash domain of the Q side (`Q_{n-1}`'s `h_out`).
    pub q_ivc: [u8; 32],
    /// Domain of the Pallas claim accumulator A keeps.
    pub pallas_claims: [u8; 32],
    /// Domain of the Vesta claim accumulator Q keeps.
    pub vesta_claims: [u8; 32],
}

/// What the wraps fix about the chain they finish: the relation kinds of
/// each side, the kind of the last A instance, keys and parameters, the
/// domains and bases, the state lengths, where `D_A`'s lineage fields sit in
/// the final A state, and which A state words are zero at an operation
/// boundary.
///
/// A side keeps one accumulator per kind of the other side, in kind order,
/// then its claim accumulator; its frame hashes their digests in that order.
#[derive(Clone, Debug)]
pub struct WrapSpec {
    /// The Q relations, in the order of A's digest list.
    pub q: Vec<FoldedRelation<Fq>>,
    /// The A relations, in the order of Q's digest list.
    pub a: Vec<FoldedRelation<Fp>>,
    /// The kind of `A_n` (an index into `a`): Omega folds `A_n` into that
    /// kind's accumulator, or starts it when `Q_{n-1}` kept none.
    pub last: usize,
    /// Q's commitment key (monomial generators of `params_q`).
    pub key_q: CommitKey<Ep>,
    /// A's commitment key (monomial generators of `params_a`).
    pub key_a: CommitKey<Eq>,
    /// k16 Pallas parameters: Q accumulators and claims, `acc_P`, Omega's proof.
    pub params_q: PinnedParams<Ep>,
    /// k16 Vesta parameters: A accumulators and claims, `acc_V`, P's proof.
    pub params_a: PinnedParams<Eq>,
    /// The domains.
    pub domains: Domains,
    /// The A side's segment base.
    pub base_a: Fp,
    /// The Q side's segment base.
    pub base_q: Fq,
    /// Words of the A state `z_A`.
    pub a_state: usize,
    /// Words of the Q state `z_Q`.
    pub q_state: usize,
    /// The positions in `z_A` of `D_A`'s public lineage fields.
    pub lineage: [usize; LINEAGE_FIELDS],
    /// The positions in `z_A` that are zero between operations (the step
    /// context and the program counter): P accepts only such a state.
    pub boundary: Vec<usize>,
}

/// The instance copy term `I` of an accumulator on its commitment side, in
/// foreign arithmetic, as `decider_v2::instance_copy_terms`: the sum over
/// the plan's instance copy cells of
/// `1 / (gamma + v + eta id) - 1 / (gamma + v + eta sigma)`, where `v` is the
/// public cell (the field side imports `I` instead of the public columns).
pub(crate) fn copy_terms_cells<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    plan: &DeciderV2Plan<C>,
    public: &[Vec<ScalarCell<C>>],
    gamma: &ScalarCell<C>,
    eta: &ScalarCell<C>,
) -> Result<ScalarCell<C>, Error> {
    let mut total = lanes.constant(region, C::ScalarExt::ZERO)?;
    let zero = total.clone();
    for &(column, row, id, sigma) in plan.instance_copy_cells() {
        let value = public
            .get(column)
            .and_then(|c| c.get(row))
            .map_or_else(|| zero.clone(), |cell| cell.ff.clone());
        let base = lanes.add(region, &gamma.ff, &value)?;
        let id = lanes.constant(region, C::ScalarExt::from(id))?;
        let sigma = lanes.constant(region, C::ScalarExt::from(sigma))?;
        let id_term = lanes.mul(region, &eta.ff, &id)?;
        let sigma_term = lanes.mul(region, &eta.ff, &sigma)?;
        let left = lanes.add(region, &base, &id_term)?;
        let right = lanes.add(region, &base, &sigma_term)?;
        let left = lanes.ff.inverse(region, &left)?;
        let right = lanes.ff.inverse(region, &right)?;
        let difference = lanes.sub(region, &left, &right)?;
        total = lanes.add(region, &total, &difference)?;
    }
    lanes.export(region, total)
}

/// Lays out byte strings as one tape and decodes their 32-byte little-endian
/// elements: `parts` are `(bytes, length)`, `None` for key generation.
pub(crate) fn byte_elements<C: PastaCurve>(
    bytes: &mut BytesChip<C::Base>,
    chip: &mut VerifierChip<C>,
    region: &mut Region<'_, C::Base>,
    parts: &[(Option<&[u8]>, usize)],
) -> Result<Vec<LeElement<C::Base>>, Error> {
    let mut values = Vec::new();
    for (part, length) in parts {
        if length % 32 != 0 || part.is_some_and(|p| p.len() != *length) {
            return Err(Error::Synthesis);
        }
        match part {
            Some(part) => values.extend(part.iter().map(|byte| Value::known(*byte))),
            None => values.extend(core::iter::repeat_n(Value::unknown(), *length)),
        }
    }
    let count = values.len() / 32;
    let run = bytes.run(
        region,
        &values,
        &vec![16; 2 * count],
        &le_message_segments(0, count),
    )?;
    (0..count)
        .map(|index| decode_le_element(&mut chip.uint(), region, &run, 32 * index))
        .collect()
}

/// The LE32 length of a fold or restructuring witness.
pub(crate) fn witness_length<C: PastaCurve>(
    chip: &mut VerifierChip<C>,
    region: &mut Region<'_, C::Base>,
) -> Result<Uint<C::Base, 32>, Error> {
    chip.uint()
        .assign::<32>(region, Value::known(FOLD_WITNESS_BYTES as u128))
}
