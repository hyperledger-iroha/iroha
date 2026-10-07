//! Fixed-artifact proof production and exact source-bound checkpoint restoration.

#[cfg(test)]
mod tests;

#[path = "checkpoint.rs"]
mod checkpoint;
pub use checkpoint::{CheckpointKind, CheckpointLayout};
#[path = "original.rs"]
mod original;

use super::circuit::{Continuation, Stage};
use super::*;
use crate::{
    a_relation::{
        native::artifact::KeyArtifact,
        split::{SplitPlan, WCircuit, WKey},
    },
    omega::OmegaWitness,
};
use iroha_plonk::{
    ProverConfig, ProverRandomness, ProvingKey, Witness, create_proof_owned_with_claim,
};

/// Installed A1–A10 and W0–W8 artifacts for the complete Receive owner schedule.
/// Only verifier metadata is retained. The caller borrows one matching stage
/// proving key for each proving call and may release it immediately afterward.
/// Package authentication belongs to artifact installation. Original PK intake
/// checks one stage against the fixed compiled source before returning its key.
pub struct Prover {
    plan: Plan,
    a: [KeyArtifact<Eq>; A_STAGE_COUNT],
    w: [KeyArtifact<Ep>; A_STAGE_COUNT - 1],
    wrappers: [WKey; A_STAGE_COUNT - 1],
}
impl Prover {
    /// Derive the four nonproof Receive predicates using the same bounded circuit
    /// owners. Actual Q/A/W/Omega and complete decisions remain mandatory.
    /// # Errors
    /// Invalid source binding, authenticated route or predicate assignment.
    pub fn propose_nonproof(&self, input: super::PredicateInputs) -> Result<[bool; 4], Error> {
        self.plan.propose_nonproof(input)
    }

    /// Install exact fixed-profile verifier metadata without retaining any PK.
    /// There is no runtime key generation or profile selection.
    /// # Errors
    /// Wrong public schema, internal/terminal profile or authenticated W stage identity.
    pub fn from_artifacts(
        plan: Plan,
        a: [KeyArtifact<Eq>; A_STAGE_COUNT],
        w: [KeyArtifact<Ep>; A_STAGE_COUNT - 1],
    ) -> Result<Self, Error> {
        let internal =
            super::super::artifact::source_descriptor::<StageCircuit>(INTERNAL_RANGE_BUSES)
                .ok_or(Error::Artifact)?;
        let terminal =
            super::super::artifact::source_descriptor::<StageCircuit>(TERMINAL_RANGE_BUSES)
                .ok_or(Error::Artifact)?;
        for (stage, key) in a.iter().enumerate() {
            let expected = if stage + 1 == A_STAGE_COUNT {
                &terminal
            } else {
                &internal
            };
            if key.binding() != expected {
                return Err(Error::Artifact);
            }
            VerifierPlan::new(key.binding().clone(), plan.vesta.clone())
                .map_err(|_| Error::Artifact)?;
        }
        let wrappers = (0..A_STAGE_COUNT - 1)
            .map(|i| {
                WKey::from_artifact(
                    plan.context(),
                    i,
                    w[i].binding().clone(),
                    plan.pallas.clone(),
                    w[i].key().clone(),
                )
                .map_err(|_| Error::Artifact)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Artifact)?;
        Ok(Self {
            plan,
            a,
            w,
            wrappers,
        })
    }
    /// Check original hard proofs and preserve every untrusted soft source proposal.
    /// # Errors
    /// Exact source mismatch, failed hard proof, over-envelope or non-deciding selection.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Session<'_>, Error> {
        Ok(Session {
            prover: self,
            prepared: self.plan.prepare(input, budget)?,
        })
    }
    /// Exact installed descriptor sequence in durable A/W checkpoint order.
    pub fn descriptors(&self) -> Vec<&DescriptorBinding> {
        let mut out = Vec::with_capacity(2 * A_STAGE_COUNT - 1);
        for i in 0..A_STAGE_COUNT {
            out.push(self.a[i].binding());
            if i + 1 < A_STAGE_COUNT {
                out.push(self.w[i].binding());
            }
        }
        out
    }
}

/// Source-bound proving session over fixed installed artifacts.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    fn require_a(&self, source: &ACheckpoint) -> Result<(), Error> {
        if source.stage >= A_STAGE_COUNT
            || !Arc::ptr_eq(&source.circuit.source, &self.prepared.source)
        {
            return Err(Error::Input);
        }
        Ok(())
    }
    fn verify_a(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<(), Error> {
        self.require_a(source)?;
        let key = &self.prover.a[source.stage];
        verify_full(
            &self.prepared.plan.vesta,
            key.binding(),
            key.key(),
            std::slice::from_ref(&source.public),
            &source.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        source
            .circuit
            .pallas
            .decide(&self.prepared.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        source
            .part
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        source
            .opening
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)
    }
    fn checked_a(
        &self,
        stage: usize,
        circuit: Stage,
        proof: Vec<u8>,
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        if stage >= A_STAGE_COUNT
            || circuit.continuation.as_ref().map_or(0, |c| c.plan.stage()) != stage
        {
            return Err(Error::Input);
        }
        let public = stage_public(&circuit)?;
        let key = &self.prover.a[stage];
        verify_full(
            &self.prepared.plan.vesta,
            key.binding(),
            key.key(),
            std::slice::from_ref(&public),
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_vesta(
            &self.prepared.plan.vesta,
            key.binding(),
            key.key(),
            std::slice::from_ref(&public),
            &proof,
            budget,
        )?;
        let part = circuit
            .continuation
            .as_ref()
            .map_or_else(|| circuit.source.own.part.clone(), |c| c.vesta.as_input());
        let checkpoint = ACheckpoint {
            stage,
            circuit,
            public,
            proof,
            part,
            opening,
        };
        self.verify_a(&checkpoint, budget)?;
        Ok(checkpoint)
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "the current stage PK is borrowed independently of source and proving randomness"
    )]
    fn prove_a(
        &self,
        stage: usize,
        circuit: Stage,
        key: &ProvingKey<Eq>,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        self.prover
            .a
            .get(stage)
            .ok_or(Error::Artifact)?
            .require_prover(key)
            .map_err(|_| Error::Artifact)?;
        let public = stage_public(&circuit)?;
        let actual = StageCircuit {
            inner: circuit.clone(),
        };
        let witness = Witness::from_circuit(key, &actual, std::slice::from_ref(&public))
            .map_err(|_| Error::Prover)?;
        let output = create_proof_owned_with_claim(
            &self.prepared.plan.vesta,
            key,
            witness,
            randomness,
            config,
        )
        .map_err(|_| Error::Prover)?;
        self.checked_a(stage, circuit, output.proof, budget)
    }
    /// Prove A1 with hard predecessor and own-sigma ownership.
    /// The borrowed A1 PK must match the installed identity before folding.
    /// # Errors
    /// Failed fixed-artifact assignment, proof or complete decide.
    pub fn first(
        &self,
        key: &ProvingKey<Eq>,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<ACheckpoint, Error> {
        self.prover.a[0]
            .require_prover(key)
            .map_err(|_| Error::Artifact)?;
        self.prove_a(
            0,
            self.prepared.first(salt, fold)?,
            key,
            randomness,
            config,
            fold.kernel_budget,
        )
    }
    /// Restore A1 by recomputing its exact source context from retained originals.
    /// # Errors
    /// Changed source bytes, noncanonical claim or failed full proof/decide.
    pub fn restore_first(
        &self,
        proof: Vec<u8>,
        pallas: &[u8],
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        let pallas = AccumulatorT::from_bytes(pallas).map_err(|_| Error::Input)?;
        let circuit = Stage {
            source: self.prepared.source.clone(),
            continuation: None,
            pallas: pallas.clone(),
            fold: vec![],
            known: true,
        };
        self.checked_a(0, circuit, proof, budget)
    }
    /// Prove the fixed W successor of a nonterminal A checkpoint.
    /// Every W retains source-part, own-A and two explicit trivial Vesta slots.
    /// The borrowed W PK is checked against this exact stage before verification.
    /// # Errors
    /// Foreign/terminal source, wrong installed key, proof or full-claim failure.
    #[allow(
        clippy::too_many_arguments,
        reason = "the current stage PK is borrowed independently of source and proving randomness"
    )]
    pub fn wrapper(
        &self,
        key: &ProvingKey<Ep>,
        source: &ACheckpoint,
        salt: Fq,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<WCheckpoint, Error> {
        self.prover
            .w
            .get(source.stage)
            .ok_or(Error::Artifact)?
            .require_prover(key)
            .map_err(|_| Error::Artifact)?;
        self.verify_a(source, fold.kernel_budget)?;
        if source.stage + 1 >= A_STAGE_COUNT {
            return Err(Error::Input);
        }
        let trivial = AccumulatorT::trivial(&self.prepared.plan.vesta, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let (vfold, vesta) = create_fold(
            &self.prepared.plan.vesta,
            &[
                source.part.clone(),
                source.opening.clone(),
                trivial.as_input(),
                trivial.as_input(),
            ],
            salt.to_repr(),
            fold,
        )
        .map_err(|_| Error::Proof)?;
        vesta
            .decide(&self.prepared.plan.vesta, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let a = &self.prover.a[source.stage];
        let allowed = a
            .key()
            .kagemusha_digest(a.binding())
            .map_err(|_| Error::Artifact)?;
        let circuit = WCircuit::new(
            self.prepared.plan.context(),
            source.stage,
            a.binding().clone(),
            self.prepared.plan.vesta.clone(),
            vec![allowed],
            OmegaWitness {
                key: a.key().clone(),
                instances: source.public.clone(),
                length: u32::try_from(source.proof.len()).map_err(|_| Error::Input)?,
                proof: source.proof.clone(),
                fold: vfold.to_bytes(),
            },
        )
        .map_err(|_| Error::Input)?;
        let public = omega_instances(source.public[0], &vesta)?;
        let witness = Witness::from_circuit(key, &circuit, &public).map_err(|_| Error::Prover)?;
        let output = create_proof_owned_with_claim(
            &self.prepared.plan.pallas,
            key,
            witness,
            randomness,
            config,
        )
        .map_err(|_| Error::Prover)?;
        self.restore_wrapper(source, output.proof, &vesta.to_bytes(), fold.kernel_budget)
    }
    /// Restore a W proof only for its exact preceding source A/context.
    /// # Errors
    /// Wrong stage/context/key, failed proof or changed/non-deciding Vesta claim.
    pub fn restore_wrapper(
        &self,
        source: &ACheckpoint,
        proof: Vec<u8>,
        vesta: &[u8],
        budget: MemoryBudget,
    ) -> Result<WCheckpoint, Error> {
        self.verify_a(source, budget)?;
        if source.stage + 1 >= A_STAGE_COUNT {
            return Err(Error::Input);
        }
        let vesta = AccumulatorT::from_bytes(vesta).map_err(|_| Error::Input)?;
        vesta
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let public = omega_instances(source.public[0], &vesta)?;
        let key = &self.prover.w[source.stage];
        verify_full(
            &self.prepared.plan.pallas,
            key.binding(),
            key.key(),
            &public,
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_pallas(
            &self.prepared.plan.pallas,
            key.binding(),
            key.key(),
            &public,
            &proof,
            budget,
        )?;
        Ok(WCheckpoint {
            source: source.clone(),
            proof,
            vesta,
            opening,
        })
    }
    fn continuation(
        &self,
        wrapper: &WCheckpoint,
        pallas: AccumulatorT<Ep>,
        fold: Vec<u8>,
    ) -> Result<Stage, Error> {
        self.require_a(&wrapper.source)?;
        let index = wrapper.source.stage + 1;
        if index >= A_STAGE_COUNT {
            return Err(Error::Input);
        }
        let split = SplitPlan::new(
            self.prepared.plan.context().clone(),
            index,
            self.prover.wrappers[index - 1].clone(),
            &self.prepared.plan.pallas,
        )
        .map_err(|_| Error::Artifact)?;
        let previous = &wrapper.source.circuit;
        Ok(Stage {
            source: self.prepared.source.clone(),
            continuation: Some(Continuation {
                plan: split,
                proof: wrapper.proof.clone(),
                carried: previous.pallas.clone(),
                vesta: wrapper.vesta.clone(),
            }),
            pallas,
            fold,
            known: true,
        })
    }
    fn prepare_advance(
        &self,
        wrapper: &WCheckpoint,
        salt: Fp,
        fold: &FoldConfig,
    ) -> Result<Stage, Error> {
        let wrapper = self.restore_wrapper(
            &wrapper.source,
            wrapper.proof.clone(),
            &wrapper.vesta.to_bytes(),
            fold.kernel_budget,
        )?;
        let index = wrapper.source.stage + 1;
        let mut claims = vec![
            wrapper.source.circuit.pallas.as_input(),
            wrapper.opening.clone(),
        ];
        if index + 1 == A_STAGE_COUNT {
            claims.extend(self.prepared.source.selected_pallas.iter().cloned());
        }
        claims.extend(
            self.prepared
                .plan
                .context()
                .q_partition(index)
                .ok_or(Error::Artifact)?
                .iter()
                .map(|i| self.prepared.source.q[*i].opening.clone()),
        );
        let (proof, pallas) =
            create_fold(&self.prepared.plan.pallas, &claims, salt.to_repr(), fold)
                .map_err(|_| Error::Proof)?;
        pallas
            .decide(&self.prepared.plan.pallas, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        self.continuation(&wrapper, pallas, proof.to_bytes().to_vec())
    }
    /// Prepare the exact next owner circuit and public frame from a checked W.
    /// Artifact verification can compare its fixed columns and copy constraints
    /// with the installed key before proving. No unverified source is accepted.
    /// # Errors
    /// Foreign source, omitted stage/claim, failed proof or non-deciding fold.
    pub fn next_circuit(
        &self,
        wrapper: &WCheckpoint,
        salt: Fp,
        fold: &FoldConfig,
    ) -> Result<(StageCircuit, Vec<Fp>), Error> {
        let circuit = self.prepare_advance(wrapper, salt, fold)?;
        let public = stage_public(&circuit)?;
        Ok((StageCircuit { inner: circuit }, public))
    }
    /// Prove the next exact A owner after a checked W checkpoint.
    /// The terminal folds the two selected incoming Pallas obligations exactly once.
    /// The borrowed next-A PK is authenticated before any fold or proof work.
    /// # Errors
    /// Foreign source, omitted stage/claim, wrong key, failed proof or full decide.
    #[allow(
        clippy::too_many_arguments,
        reason = "the current stage PK is borrowed independently of source and proving randomness"
    )]
    pub fn advance(
        &self,
        key: &ProvingKey<Eq>,
        wrapper: &WCheckpoint,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<ACheckpoint, Error> {
        let index = wrapper.source.stage + 1;
        self.prover
            .a
            .get(index)
            .ok_or(Error::Artifact)?
            .require_prover(key)
            .map_err(|_| Error::Artifact)?;
        let circuit = self.prepare_advance(wrapper, salt, fold)?;
        self.prove_a(index, circuit, key, randomness, config, fold.kernel_budget)
    }
    /// Restore a subsequent A from its exact W predecessor and carried Pallas bytes.
    /// # Errors
    /// Wrong retained context, malformed claim, failed proof or decide.
    pub fn restore_a(
        &self,
        wrapper: &WCheckpoint,
        proof: Vec<u8>,
        pallas: &[u8],
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        let wrapper = self.restore_wrapper(
            &wrapper.source,
            wrapper.proof.clone(),
            &wrapper.vesta.to_bytes(),
            budget,
        )?;
        let pallas = AccumulatorT::from_bytes(pallas).map_err(|_| Error::Input)?;
        let circuit = self.continuation(&wrapper, pallas, vec![])?;
        self.checked_a(wrapper.source.stage + 1, circuit, proof, budget)
    }
    /// Export the actual A10 terminal and every distinct final-Omega obligation.
    /// This is not a completed monetary head until the final Omega is proved and durable.
    /// # Errors
    /// Incomplete owner chain, foreign source or failed full proof/decide.
    pub fn terminal(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<Terminal, Error> {
        self.verify_a(source, budget)?;
        if source.stage + 1 != A_STAGE_COUNT {
            return Err(Error::Input);
        }
        let continuation = source.circuit.continuation.as_ref().ok_or(Error::Input)?;
        Ok(Terminal {
            proof: source.proof.clone(),
            instances: source.public.clone(),
            pallas: source.circuit.pallas.clone(),
            vesta_part: continuation.vesta.clone(),
            predecessor_vesta: self.prepared.source.predecessor.vesta.clone(),
            incoming_vesta: self.prepared.source.incoming_head.vesta.clone(),
            incoming_mode: self.prepared.source.modes[2],
            incoming_correction: self.prepared.source.vesta_correction,
            opening: source.opening.clone(),
        })
    }
}

/// Verified source-bound A proof and retained exact continuation source.
#[derive(Clone)]
pub struct ACheckpoint {
    stage: usize,
    circuit: Stage,
    public: Vec<Fp>,
    proof: Vec<u8>,
    part: FoldInput<Eq>,
    opening: FoldInput<Eq>,
}
impl ACheckpoint {
    /// Zero-based fixed owner stage.
    pub const fn stage(&self) -> usize {
        self.stage
    }
    /// Exact retained A proof bytes.
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Exact69-word public frame, including final mode/correction exports.
    pub fn instances(&self) -> &[Fp] {
        &self.public
    }
    /// Canonical full-k16 Pallas checkpoint bytes.
    pub fn pallas_bytes(&self) -> [u8; 544] {
        self.circuit.pallas.to_bytes()
    }
}
/// Verified W proof coupled to its exact source A and full Vesta obligation.
#[derive(Clone)]
pub struct WCheckpoint {
    source: ACheckpoint,
    proof: Vec<u8>,
    vesta: AccumulatorT<Eq>,
    opening: FoldInput<Ep>,
}
impl WCheckpoint {
    /// Zero-based preceding A owner.
    pub const fn stage(&self) -> usize {
        self.source.stage
    }
    /// Exact retained W proof bytes.
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Canonical full-k16 Vesta checkpoint bytes.
    pub fn vesta_bytes(&self) -> [u8; 544] {
        self.vesta.to_bytes()
    }
}
/// Complete terminal A proof and final-Omega obligations, with original incoming V.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Actual A10 proof under its installed terminal key.
    pub proof: Vec<u8>,
    /// Exact common69-word public frame.
    pub instances: Vec<Fp>,
    /// Accumulated Pallas claim, including selected incoming P/opening once.
    pub pallas: AccumulatorT<Ep>,
    /// Vesta part carried by the last W.
    pub vesta_part: AccumulatorT<Eq>,
    /// Hard receiver-predecessor Vesta obligation.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Original incoming Vesta obligation; Omega performs its committed mode selection.
    pub incoming_vesta: AccumulatorT<Eq>,
    /// Exact original incoming-V mode committed in the terminal frame.
    pub incoming_mode: IncomingMode,
    /// Exact distinct correction point when the mode is Corrected.
    pub incoming_correction: EqAffine,
    /// Actual terminal A opening, distinct from every carried obligation.
    pub opening: FoldInput<Eq>,
}
