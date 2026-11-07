//! KAGEMUSHA-Fold: the folded lineage program (output `IVC.md` and
//! `RECEIVE-STEPS.md` of the folding lane).
//!
//! A segment of a lineage is one two-curve IVC chain (`iroha_fold::ivc`): Q
//! steps are circuits over `Fq` that fold the preceding A instance (committed
//! on Vesta), A steps are circuits over `Fp` that fold the preceding Q
//! instance (committed on Pallas). An operation is a fixed run of half-steps.
//! A Receive runs today's Receive relation, with its Q leaves folded instead of
//! proved and its A stages chained by the IVC state instead of W proofs:
//!
//! | half-step | side | kind | folds | work |
//! |---|---|---|---|---|
//! | 1 | Q | [`QKind::Open`] | the previous operation's last A | frame only |
//! | 2 | A | [`AKind::Receive1`] | `Open` | stage A1: consumed and credit effects, transition |
//! | 3 | Q | [`QKind::Sigma`] | `Receive1` | today's Q0: the `σ_send` soft verifier |
//! | 4 | A | [`AKind::Receive3`] | `Sigma` | stage A3: proofs, objects; binds Q0's columns |
//! | 5 | Q | [`QKind::Signature`] | `Receive3` | today's Q1: the merged signature leaf |
//! | 6 | A | [`AKind::Receive4`] | `Signature` | stage A4: signatures, authorization; binds Q1's columns |
//! | 7 | Q | [`QKind::Close`] | `Receive4` | frame only |
//! | 8 | A | [`AKind::Receive5`] | `Close` | stage A5: nonmembership, effects, proof digest |
//!
//! Today's stage A2 only verifies Q0; the folded A3 binds Q0's columns where
//! its owners read them, so A2 has no folded counterpart.
//!
//! Every relation kind is fixed per circuit, and so is the kind it folds
//! (`ChainSide::slot`): a step can only fold an instance of the kind its
//! schedule names, because that kind's accumulator decides against that
//! kind's relation. A side keeps one accumulator per kind of the other side,
//! then its own claim accumulator ([`A_DIGESTS`], [`Q_DIGESTS`]).
//!
//! The A state ([`A_STATE`] words) is the lineage public fields (as `D_A`
//! hashes them), the digest of the current operation's step context (zero
//! between operations) and the program counter (the number of the operation's
//! A sub-steps done, zero between operations). The Q state is empty: Q steps
//! export their leaves' public columns, which the following A step reads from
//! the instance it folds.

use ff::Field;
use iroha_fold::{
    circuit::{FoldVerifierChips, FoldVerifierConfig, ivc::{fold_frame, frame_outputs}},
    ivc::{ChainSide, FrameWitness, embed, ivc_hash},
    relation::FoldShape,
};
use iroha_pasta::{Eq, Fp, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
};

use crate::a_relation::LINEAGE_FIELDS;

/// IVC-hash domain of the A side.
pub const IVC_A: [u8; 32] = *b"kagemusha-fold lineage A side v1";
/// IVC-hash domain of the Q side.
pub const IVC_Q: [u8; 32] = *b"kagemusha-fold lineage Q side v1";
/// Claim-accumulator domain of the A side (Pallas claims).
pub const CLAIM_A: [u8; 32] = *b"kagemusha-fold A claim acc v1...";
/// Claim-accumulator domain of the Q side (Vesta claims).
pub const CLAIM_Q: [u8; 32] = *b"kagemusha-fold Q claim acc v1...";
/// The segment base word every IVC hash carries.
pub const BASE: u64 = u64::from_le_bytes(*b"kgwfseg1");

/// Words of the A state: the lineage public fields, the step-context digest,
/// the program counter.
pub const A_STATE: usize = LINEAGE_FIELDS + 2;
/// Position of the step-context digest in the A state.
pub const A_CONTEXT: usize = LINEAGE_FIELDS;
/// Position of the program counter in the A state.
pub const A_PC: usize = LINEAGE_FIELDS + 1;
/// Words of the Q state.
pub const Q_STATE: usize = 0;
/// Length of an A instance's frame column (`[passthrough, h_out]`).
pub const A_FRAME: usize = 2;
/// Length of a Q instance's frame column (`[passthrough, h_out]`); a Q leaf's
/// public columns follow as further instance columns.
pub const Q_FRAME: usize = 2;

/// A relations of the folded lineage, in the order of Q's digest list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AKind {
    /// Receive stage A1 (consumed and credit effects, transition).
    Receive1,
    /// Receive stage A3 (proofs, objects).
    Receive3,
    /// Receive stage A4 (signatures, authorization).
    Receive4,
    /// Receive stage A5 (nonmembership, effects, proof digest).
    Receive5,
}

/// Q relations of the folded lineage, in the order of A's digest list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QKind {
    /// Frame only, opening an operation.
    Open,
    /// The `σ_send` soft verifier (today's Q0).
    Sigma,
    /// The merged signature leaf (today's Q1).
    Signature,
    /// Frame only, before a Receive's last A sub-step.
    Close,
}

/// A's digest list: one accumulator per Q kind, then its claim accumulator.
pub const A_DIGESTS: usize = 4 + 1;
/// Q's digest list: one accumulator per A kind, then its claim accumulator.
pub const Q_DIGESTS: usize = 4 + 1;
/// Position of a side's claim-accumulator digest in its list.
pub const CLAIM_SLOT: usize = 4;

impl AKind {
    /// Every A kind, in digest order.
    pub const ALL: [Self; 4] = [Self::Receive1, Self::Receive3, Self::Receive4, Self::Receive5];

    /// The fold domain of this relation (the domain of its instances' and
    /// accumulators' transcripts).
    #[must_use]
    pub const fn domain(self) -> [u8; 32] {
        match self {
            Self::Receive1 => *b"kagemusha-fold Receive A1 rel v1",
            Self::Receive3 => *b"kagemusha-fold Receive A3 rel v1",
            Self::Receive4 => *b"kagemusha-fold Receive A4 rel v1",
            Self::Receive5 => *b"kagemusha-fold Receive A5 rel v1",
        }
    }

    /// The Q kind a step of this kind folds.
    #[must_use]
    pub const fn folds(self) -> QKind {
        match self {
            Self::Receive1 => QKind::Open,
            Self::Receive3 => QKind::Sigma,
            Self::Receive4 => QKind::Signature,
            Self::Receive5 => QKind::Close,
        }
    }

    /// The frame of a step of this kind.
    #[must_use]
    pub const fn chain(self) -> ChainSide {
        ChainSide {
            ivc_domain: IVC_A,
            fold_domain: self.folds().domain(),
            first_step: 2,
            slot: self.folds() as usize,
            digests: A_DIGESTS,
        }
    }
}

impl QKind {
    /// Every Q kind, in digest order.
    pub const ALL: [Self; 4] = [Self::Open, Self::Sigma, Self::Signature, Self::Close];

    /// The fold domain of this relation.
    #[must_use]
    pub const fn domain(self) -> [u8; 32] {
        match self {
            Self::Open => *b"kagemusha-fold Receive Qopen v1.",
            Self::Sigma => *b"kagemusha-fold Receive Qsigma v1",
            Self::Signature => *b"kagemusha-fold Receive Qsig v1..",
            Self::Close => *b"kagemusha-fold Receive Qclose v1",
        }
    }

    /// The A kind a step of this kind folds.
    #[must_use]
    pub const fn folds(self) -> AKind {
        match self {
            Self::Open => AKind::Receive5,
            Self::Sigma => AKind::Receive1,
            Self::Signature => AKind::Receive3,
            Self::Close => AKind::Receive4,
        }
    }

    /// The frame of a step of this kind.
    #[must_use]
    pub const fn chain(self) -> ChainSide {
        ChainSide {
            ivc_domain: IVC_Q,
            fold_domain: self.folds().domain(),
            first_step: 1,
            slot: self.folds() as usize,
            digests: Q_DIGESTS,
        }
    }
}

/// The passthrough of a segment's first instance (a Q step without
/// predecessor): A's base hash `H_A(0, base, a_base, [0, ..])`, a circuit
/// constant of the Q steps that can open a segment. `a_base` is the A state
/// the segment starts from.
#[must_use]
pub fn start_passthrough(a_base: &[Fp]) -> Fq {
    embed(&ivc_hash::<Fp>(&IVC_A, 0, Fp::from(BASE), a_base, &[Fp::ZERO; A_DIGESTS]))
}

/// A Q step without business ([`QKind::Open`], [`QKind::Close`]): it folds the
/// preceding A instance and passes its digests through.
#[derive(Clone, Debug)]
pub struct QFrameStep {
    /// The kind of this step.
    pub kind: QKind,
    /// The shape of the A relation it folds.
    pub shape: FoldShape,
    /// The passthrough of a segment's first instance ([`start_passthrough`]).
    pub start: Fq,
    /// The frame witness (`None` at key generation).
    pub witness: Option<FrameWitness<Eq>>,
}

/// Columns of a [`QFrameStep`].
#[derive(Clone, Debug)]
pub struct QFrameConfig {
    fold: FoldVerifierConfig<Eq>,
    frame: Column<Instance>,
}

impl Circuit<Fq> for QFrameStep {
    type Config = QFrameConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { witness: None, ..self.clone() }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> QFrameConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let frame = meta.instance_column(Q_FRAME);
        meta.enable_equality(frame);
        QFrameConfig { fold, frame }
    }

    fn synthesize(&self, config: QFrameConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        if !matches!(self.kind, QKind::Open | QKind::Close) {
            return Err(Error::Synthesis);
        }
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let chain = self.kind.chain();
        let outputs = layouter.assign_region(
            || "folded lineage Q frame step",
            |mut region| {
                let mut lanes = chips.lanes();
                let frame = fold_frame(
                    &mut lanes,
                    &mut region,
                    &chain,
                    &self.shape,
                    Fq::from(BASE),
                    self.start,
                    Q_STATE,
                    self.witness.as_ref(),
                )?;
                frame_outputs(&mut lanes, &mut region, &chain, &frame, &[], &frame.digests, &[])
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_folds_the_kind_before_it_in_its_own_slot() {
        for kind in AKind::ALL {
            let chain = kind.chain();
            assert!(chain.is_valid() && chain.slot < CLAIM_SLOT);
            assert_eq!(QKind::ALL[chain.slot], kind.folds());
            assert_eq!(chain.fold_domain, kind.folds().domain());
        }
        for kind in QKind::ALL {
            let chain = kind.chain();
            assert!(chain.is_valid() && chain.slot < CLAIM_SLOT);
            assert_eq!(AKind::ALL[chain.slot], kind.folds());
        }
        // The Receive's half-steps alternate Open, A1, Sigma, A3, Signature, A4,
        // Close, A5, and the next operation opens by folding A5.
        let mut q = QKind::Open;
        for a in AKind::ALL {
            assert_eq!(a.folds(), q);
            q = QKind::ALL.into_iter().find(|next| next.folds() == a).expect("a Q kind folds every A kind");
        }
        assert_eq!(q, QKind::Open);
        let mut domains: Vec<_> = AKind::ALL.iter().map(|k| k.domain()).chain(QKind::ALL.iter().map(|k| k.domain())).collect();
        domains.extend([IVC_A, IVC_Q, CLAIM_A, CLAIM_Q]);
        let count = domains.len();
        domains.sort_unstable();
        domains.dedup();
        assert_eq!(domains.len(), count, "every domain is distinct");
    }
}
