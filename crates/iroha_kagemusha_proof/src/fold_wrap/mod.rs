//! The KAGEMUSHA-Fold compression wraps (output `IVC.md` sections 7 and 11).
//!
//! After the last business instance `A_n`, two k16 PIPA-R proofs turn the
//! folded state into an Omega in today's transport:
//!
//! - **P**, over `Fp` (a Vesta proof): the commitment side of the Q-side
//!   decider (Pallas group operations) and its bound opening, the field side
//!   of the A-side decider (native `Fp`), the Pallas claim accumulator's
//!   committed-mask opening, the Vesta claim accumulator's restructuring
//!   sumcheck, the PIPA-AS fold into `acc_P`, `D_A`, and `A_n`'s recomputed
//!   `h_out` ([`p::PCircuit`]);
//! - **Omega**, over `Fq` (a Pallas proof, today's three public columns
//!   `[D_A]`, `acc_V.G`, `acc_V.u`): verifies P, the field side of the Q-side
//!   decider, the Pallas claims' restructuring sumcheck, the fold of `A_n`
//!   into `U_A` with `A_n`'s passthrough recomputed, the commitment side of the
//!   A-side decider and its opening, the Vesta claims' committed-mask opening,
//!   and the PIPA-AS fold into `acc_V` ([`omega::OmegaCircuit`]).
//!
//! Each decider is split (DESIGN.md section 9.0): its field messages are
//! native in the circuit over its scalar field, its commitments in the circuit
//! over its commitment curve's base field. The values that cross (digests,
//! field data, challenges, the batched value and the folded eq) are rows of
//! P's public column ([`crossing`]); Omega verifies P, so every crossing value
//! is bound on both sides. The prover computes everything natively, proves P,
//! then Omega ([`native`]).
//!
//! Every proof before payout stays: both final accumulators, both claim
//! accumulators and P itself are decided or folded into the transported
//! `acc_P` (inside `D_A`) and `acc_V`, which the consumer decides as today.

pub mod crossing;
pub mod native;
pub mod omega;
pub mod p;

use iroha_fold::{CommitKey, FoldedRelation};
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
    /// Fold domain of the Q relation (`U_Q`'s digest and decider).
    pub q_fold: [u8; 32],
    /// Fold domain of the last A relation (`U_A`, `A_n`'s fold and decider).
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

/// What the wraps fix about the chain they finish: the relations, keys and
/// parameters, the domains and bases, the state lengths, and where `D_A`'s
/// lineage fields sit in the final A state.
#[derive(Clone, Debug)]
pub struct WrapSpec {
    /// The Q relation (`U_Q`'s instances).
    pub q: FoldedRelation<Fq>,
    /// The last A relation (`U_A`'s instances and `A_n`).
    pub a: FoldedRelation<Fp>,
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
