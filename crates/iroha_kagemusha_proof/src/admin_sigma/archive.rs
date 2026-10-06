//! One `ArchiveSent` sigma class shared by both delivery-evidence forms.

use super::*;

/// Canonical state openings and `ArchiveSent` statement preimage.
///
/// A hard-authenticates the pending descriptor and core removal, then retains
/// or removes the adjusted pending entry according to its complete incoming
/// evidence verdict. This leaf preserves all other state and moves no value.
#[derive(Clone, Copy, Debug)]
pub struct ArchiveWitness {
    /// Previous state; it may have unfolded steps awaiting local folding.
    pub predecessor: StateWitness,
    /// Committed state after sequence advancement and core pending removal.
    pub successor: StateWitness,
    /// Exact 26-field `ArchiveSent` statement.
    pub statement: [Fp; 26],
}

/// Fixed k12 `ArchiveSent` leaf for both Receive-package and `CreditStatus` evidence.
/// Evidence kinds have the same hard statement/state rules and distinct A owners.
#[derive(Clone, Copy, Debug)]
pub struct ArchiveCircuit {
    witness: ArchiveWitness,
    known: bool,
}
impl ArchiveCircuit {
    /// Carry original state openings without accepting any delivery evidence.
    #[must_use]
    pub const fn new(witness: &ArchiveWitness) -> Self {
        Self {
            witness: *witness,
            known: true,
        }
    }
    /// Exact bounded public statement digest.
    #[must_use]
    pub fn instances(&self) -> [Vec<Fp>; 1] {
        [vec![hash_with_domain(
            STATEMENT_DOMAIN,
            &self.witness.statement,
        )]]
    }
    /// Exact native PIPA-R instance membership.
    #[must_use]
    pub const fn instance_types() -> [InstanceType; 1] {
        [InstanceType::Bounded]
    }
}
impl Circuit<Fp> for ArchiveCircuit {
    type Config = AdminConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..*self
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        BootstrapCircuit::configure(meta)
    }
    fn synthesize(&self, config: Self::Config, layouter: impl Layouter<Fp>) -> Result<(), Error> {
        // Both evidence forms use tag5 and the same two effect fields. A's fixed
        // variant selects the full incoming relation and its no-op verdict.
        load::Transition {
            predecessor: &self.witness.predecessor,
            successor: &self.witness.successor,
            statement: &self.witness.statement,
            variant: Variant::ArchiveReceive,
            known: self.known,
        }
        .synthesize(config, layouter)
    }
}
