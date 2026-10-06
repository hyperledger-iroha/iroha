//! Production Bootstrap A1/W/A2 assembly from original signed object tapes.
//!
//! The installed owner supplies fixed Q, A and W artifacts and converts canonical
//! G1 objects to these typed proof inputs. Every Q proof is verified in full here;
//! A1 binds its sigma tape, A2 authenticates the original Enrollment certificate,
//! Credential and Advance Receipt and retains Q0, W and Q1 opening obligations.
//! The terminal A proof is an input to the separately installed final Omega
//! catalog. Neither an intermediate W nor this terminal output is monetary
//! completion or a transported Omega. No method generates runtime artifact keys.

use core::fmt;
use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_pasta::{
    Ep, Eq, EqAffine, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain,
};
use iroha_plonk::{
    ProverConfig, ProverRandomness, ProvingKey, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error as LayoutError, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::ipa::PinnedParams,
    transcript::decode_point,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{
    GlueChip, Word,
    bytes::{
        element::le_message_segments,
        le_value, p_bytes_native,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    p256::VerifyMode,
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, K, VESTA_TRIVIAL_GENERATOR,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig},
};

use super::super::{
    AProofPlan, LineagePublicCells, ProofMessageCells, SigmaBindingCells, VestaClaimCells,
    bind_signature_q,
    bootstrap::{BootstrapInputs, BootstrapObjects, BootstrapPolicy},
    context::{ContextInputs, ContextPlan, ContextState},
    schedule::OperationTask,
    split::{SplitPlan, WCircuit, WKey, close_first, close_stage, resume_context},
    verify_q, verify_sigma,
};
use crate::{
    admin_sigma::BootstrapWitness,
    omega::OmegaWitness,
    operation_relation::{objects::ObjectKind, state::StateCells, statement::StatementCells},
    q_signature::{QSignaturePlan, SignatureKey},
};

/// The fixed source profile measured with the complete Bootstrap/Load catalog.
/// A witness cannot select another bus count or a generic fallback profile.
pub const SOURCE_RANGE_BUSES: usize = 4;

/// Production preparation or proof failure. No failure changes a monetary head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The installed stage/Q key, schema or pinned parameters differ.
    Artifact,
    /// A supplied original, frame, obligation or typed input is malformed.
    Input,
    /// A source or generated proof or complete decide failed.
    Proof,
    /// The exact installed circuit failed assignment or proving.
    Prover,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native Bootstrap: {self:?}")
    }
}
impl std::error::Error for Error {}

/// One original Q proof and the exact public columns that proof authenticates.
/// Public instance values are witnesses, never a native acceptance verdict.
#[derive(Clone, Debug)]
pub struct QInput {
    /// Descriptor-sized proof bytes retained by the owning native producer.
    pub proof: Vec<u8>,
    /// Exact descriptor-shaped public columns.
    pub instances: Vec<Vec<Fq>>,
}

/// Original post-Advance inputs for Bootstrap's complete lineage composition.
/// The native G1 owner validates enrollment, canonical objects and custody before
/// conversion. These inputs are additionally checked by the actual A circuits.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Exact core33, rest8, public18 and statement26 field transcripts.
    pub state: BootstrapWitness,
    /// Original unframed sigma bytes, identical to Q0's exported carrier.
    pub sigma: Vec<u8>,
    /// Original canonical Certificate, Credential and Advance Receipt bytes.
    pub objects: [Vec<u8>; 3],
    /// The hard sigma-verification Q, followed by the hard three-signature Q.
    pub q: [QInput; 2],
}

/// Fixed Bootstrap circuit metadata from the authenticated native artifact owner.
/// No field is populated from an operation's witness or wire proof profile.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    policy: BootstrapPolicy,
    signatures: QSignaturePlan,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin exactly Q0 in A1 and Q1 plus all Bootstrap tasks in A2.
    ///
    /// Signature slots are hard `[receipt, credential, certificate]`, with a
    /// fixed root for the certificate. The certificate/root equality is also
    /// constrained by `BootstrapObjects::authenticate` under `policy`.
    ///
    /// # Errors
    /// Wrong variant, slot/schema count, parameter k or signature key policy.
    pub fn new(
        operation: AProofPlan,
        policy: BootstrapPolicy,
        signatures: QSignaturePlan,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        if operation.frame().variant() != Variant::Bootstrap
            || operation.frame().part_source_k() != crate::admin_sigma::BOOTSTRAP_K
            || operation.q_count() != 2
        {
            return Err(Error::Artifact);
        }
        pallas.require_k(16).map_err(|_| Error::Artifact)?;
        vesta.require_k(16).map_err(|_| Error::Artifact)?;
        let slots = signatures.slots();
        if slots.len() != 3
            || slots.iter().any(|slot| slot.mode != VerifyMode::Hard)
            || slots[0].key != SignatureKey::Variable
            || slots[1].key != SignatureKey::Variable
            || !matches!(slots[2].key, SignatureKey::Fixed(_))
        {
            return Err(Error::Artifact);
        }
        let q1 = operation
            .q(1)
            .ok_or(Error::Artifact)?
            .verifier()
            .binding()
            .descriptor();
        if q1.instance_lengths != [signatures.instance_length() as u32]
            || q1.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
        {
            return Err(Error::Artifact);
        }
        let context = ContextPlan::new(
            operation,
            1,
            BootstrapObjects::context_specs()
                .map_err(|_| Error::Artifact)?
                .to_vec(),
        )
        .and_then(|plan| {
            plan.with_operation_tasks(vec![
                vec![],
                vec![
                    OperationTask::BootstrapState,
                    OperationTask::BootstrapAuthorization,
                ],
            ])
        })
        .map_err(|_| Error::Artifact)?;
        Ok(Self {
            context,
            policy,
            signatures,
            pallas,
            vesta,
        })
    }

    /// Fixed complete context schema, including the exact Q keys and task owners.
    #[must_use]
    pub const fn context(&self) -> &ContextPlan {
        &self.context
    }

    /// Validate original dimensions and verify both source Q proofs in full.
    /// The resulting openings are independently decided, never caller supplied.
    ///
    /// # Errors
    /// Invalid originals, substituted public columns or failed proof/decide.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Prepared, Error> {
        for (kind, original) in object_kinds().into_iter().zip(&input.objects) {
            if original.len() != kind.body_len() + 64 {
                return Err(Error::Input);
            }
        }
        let sigma_class = self
            .context
            .operation()
            .sigma
            .class(0)
            .ok_or(Error::Artifact)?;
        if input.sigma.len() != sigma_class.verifier().proof_length() {
            return Err(Error::Input);
        }
        let mut openings = Vec::with_capacity(2);
        for (index, original) in input.q.iter().enumerate() {
            let fixed = self.context.operation().q(index).ok_or(Error::Artifact)?;
            verify_full(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &original.instances,
                &original.proof,
                budget,
            )
            .map_err(|_| Error::Proof)?;
            let claim = accumulate_generator(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &original.instances,
                &original.proof,
                budget,
            )
            .map_err(|_| Error::Proof)?;
            let opening = FoldInput::from_opening(*claim.g(), claim.challenges())
                .map_err(|_| Error::Proof)?;
            opening
                .decide(&self.pallas, budget)
                .map_err(|_| Error::Proof)?;
            openings.push(opening);
        }
        check_sigma_tape(&input)?;
        let part = q_sigma_part(
            &input.q[0],
            self.context.operation().frame().part_source_k(),
        )?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        let original = Arc::new(input);
        Ok(Prepared {
            plan: Arc::new(self.clone()),
            original,
            openings: openings.try_into().map_err(|_| Error::Input)?,
            part,
        })
    }
}

fn object_kinds() -> [ObjectKind; 3] {
    [
        ObjectKind::Certificate,
        ObjectKind::Credential,
        ObjectKind::Receipt,
    ]
}

fn check_sigma_tape(input: &Inputs) -> Result<(), Error> {
    let bounded = input.q[0].instances.first().ok_or(Error::Input)?;
    let length = u32::try_from(input.sigma.len()).map_err(|_| Error::Input)?;
    let mut raw = length.to_le_bytes().to_vec();
    raw.extend(&input.sigma);
    let chunks = raw
        .chunks(31)
        .map(|bytes| le_value::<Fq>(bytes).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    let statement = hash_with_domain(
        iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
        &input.state.statement,
    );
    let statement = Option::<Fq>::from(Fq::from_repr(statement.to_repr())).ok_or(Error::Input)?;
    if bounded.len() != 1 + chunks.len() + K
        || bounded[0] != statement
        || bounded[1..1 + chunks.len()] != chunks
        || input.q[0].instances.get(2).map(Vec::as_slice) != Some(&[Fq::ZERO])
    {
        return Err(Error::Input);
    }
    Ok(())
}

/// Already installed proving artifacts for this single Bootstrap source profile.
/// The native artifact loader authenticates these exact keys before construction.
/// Each runtime session then uses these fixed keys without a witness profile choice.
pub struct Prover {
    plan: Plan,
    first: Arc<ProvingKey<Eq>>,
    wrapper: Arc<ProvingKey<Ep>>,
    terminal: Arc<ProvingKey<Eq>>,
    w: WKey,
}
impl Prover {
    /// Install the exact A1/W0/A2 proving keys under the fixed two-stage plan.
    /// This method imports and checks keys; it never generates one.
    ///
    /// # Errors
    /// Wrong stage/curve/k, heterogeneous A frames or invalid W metadata.
    pub fn from_artifacts(
        plan: Plan,
        first: Arc<ProvingKey<Eq>>,
        wrapper: Arc<ProvingKey<Ep>>,
        terminal: Arc<ProvingKey<Eq>>,
    ) -> Result<Self, Error> {
        for key in [&first, &terminal] {
            let d = key.binding().descriptor();
            if d.k != 16
                || d.instance_lengths != [69]
                || d.instance_types.as_deref() != Some(&[InstanceType::Bounded])
            {
                return Err(Error::Artifact);
            }
        }
        if first.binding() != terminal.binding() {
            return Err(Error::Artifact);
        }
        let w = WKey::from_artifact(
            &plan.context,
            0,
            wrapper.binding().clone(),
            plan.pallas.clone(),
            wrapper.vk().clone(),
        )
        .map_err(|_| Error::Artifact)?;
        Ok(Self {
            plan,
            first,
            wrapper,
            terminal,
            w,
        })
    }

    /// Verify source inputs and create a session using only these installed keys.
    ///
    /// # Errors
    /// Any source proof, complete decide or original-tape mismatch.
    pub fn prepare(&self, inputs: Inputs, budget: MemoryBudget) -> Result<Session<'_>, Error> {
        Ok(Session {
            prover: self,
            prepared: self.plan.prepare(inputs, budget)?,
        })
    }

    /// Fixed descriptors of the three checkpoint/proof stages, in actual order.
    #[must_use]
    pub fn descriptors(&self) -> [&iroha_plonk::DescriptorBinding; 3] {
        [
            self.first.binding(),
            self.wrapper.binding(),
            self.terminal.binding(),
        ]
    }
}

/// One exact Bootstrap source coupled to the immutable installed proving keys.
/// The custody owner retains source originals and checkpoints before advancing.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    /// Compute the actual first checkpoint using the installed A1 key.
    /// # Errors
    /// Wrong installed circuit, failed proof or complete decide.
    pub fn first(
        &self,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<FirstCheckpoint, Error> {
        self.prepared
            .prove_first(&self.prover.first, randomness, config)
    }
    /// Reverify A1 proof bytes reloaded from the source-bound custody archive.
    /// # Errors
    /// A substituted checkpoint, context, source key or failed proof.
    pub fn restore_first(
        &self,
        proof: Vec<u8>,
        budget: MemoryBudget,
    ) -> Result<FirstCheckpoint, Error> {
        let mut public = frame(
            &self.prepared.original.state.lineage,
            &self.prepared.openings[0],
            &self.prepared.part,
        )?;
        public[0] = self.prepared.context_digest()?;
        let checkpoint = FirstCheckpoint {
            public,
            proof,
            carried: self.prepared.openings[0].clone(),
        };
        self.prepared.resume_first(
            self.prover.first.vk(),
            self.prover.first.binding(),
            checkpoint,
            budget,
        )
    }
    /// Compute W0 from the exact first checkpoint using the installed wrapper key.
    /// # Errors
    /// A wrong source checkpoint or failed proof, fold or decide.
    pub fn wrapper(
        &self,
        first: &FirstCheckpoint,
        salt: Fq,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<WrapperCheckpoint, Error> {
        self.prepared.prove_wrapper(
            first,
            &self.prover.first,
            &self.prover.wrapper,
            salt,
            fold,
            randomness,
            config,
        )
    }
    /// Reverify retained W proof and exact canonical Vesta claim from custody.
    /// # Errors
    /// A changed original context, noncanonical accumulator or failed proof/decide.
    pub fn restore_wrapper(
        &self,
        proof: Vec<u8>,
        vesta: &[u8],
        budget: MemoryBudget,
    ) -> Result<WrapperCheckpoint, Error> {
        let vesta = AccumulatorT::from_bytes(vesta).map_err(|_| Error::Input)?;
        let checkpoint = WrapperCheckpoint {
            proof,
            vesta,
            context: self.prepared.context_digest()?,
        };
        self.prepared
            .resume_wrapper(&self.prover.w, checkpoint, budget)
    }
    /// Compute the actual terminal A2 result using its installed key.
    /// # Errors
    /// A wrong W, source/profile mismatch or failed proof/fold/decide.
    pub fn terminal(
        &self,
        wrapper: &WrapperCheckpoint,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<Terminal, Error> {
        self.prepared.prove_terminal(
            wrapper,
            &self.prover.w,
            &self.prover.terminal,
            salt,
            fold,
            randomness,
            config,
        )
    }
}

fn q_sigma_part(input: &QInput, source_k: u32) -> Result<FoldInput<Eq>, Error> {
    let [bounded, point, indices, verdicts, source] = input.instances.as_slice() else {
        return Err(Error::Input);
    };
    if point.len() != 2
        || indices.len() != 1
        || verdicts.as_slice() != [Fq::ONE]
        || source.as_slice() != [Fq::from(u64::from(source_k))]
        || bounded.len() < K
    {
        return Err(Error::Input);
    }
    let g = Option::<EqAffine>::from(EqAffine::from_xy(point[0], point[1])).ok_or(Error::Input)?;
    let challenges = bounded[bounded.len() - K..]
        .iter()
        .map(|value| Option::<Fp>::from(Fp::from_repr(value.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    FoldInput::from_normalized(
        g,
        source_k,
        challenges.try_into().map_err(|_| Error::Input)?,
    )
    .map_err(|_| Error::Input)
}

/// Prepared exact source Q proofs and all of their checked opening obligations.
/// Private fields prevent substituting an unverified opening or another tape.
#[derive(Clone)]
pub struct Prepared {
    plan: Arc<Plan>,
    original: Arc<Inputs>,
    openings: [FoldInput<Ep>; 2],
    part: FoldInput<Eq>,
}
impl Prepared {
    /// First A stage. It binds the actual sigma bytes and every deferred object
    /// and Q instance into the same committed context used by A2.
    #[must_use]
    pub fn first_circuit(&self) -> StageCircuit {
        StageCircuit {
            prepared: self.clone(),
            stage: Stage::First,
            known: true,
        }
    }

    /// Generate A1 only with its already installed fixed proving key.
    /// Its complete opening is verified and decided before returning.
    ///
    /// # Errors
    /// A mismatching key/circuit, failed proof or malformed public frame.
    pub fn prove_first(
        &self,
        key: &ProvingKey<Eq>,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<FirstCheckpoint, Error> {
        let circuit = self.first_circuit();
        let carried = &self.openings[0];
        let mut public = frame(&self.original.state.lineage, carried, &self.part)?;
        public[0] = self.context_digest()?;
        let proof = prove_vesta(&self.plan, key, &circuit, &public, randomness, config)?;
        Ok(FirstCheckpoint {
            public,
            proof,
            carried: carried.clone(),
        })
    }

    /// Reverify a durably retained A1 checkpoint against these exact originals
    /// and the already installed stage key. No checkpoint verdict is trusted.
    ///
    /// # Errors
    /// Another context/public frame, wrong source key or failed proof/decide.
    pub fn resume_first(
        &self,
        key: &VerifyingKey<Eq>,
        binding: &iroha_plonk::DescriptorBinding,
        checkpoint: FirstCheckpoint,
        budget: MemoryBudget,
    ) -> Result<FirstCheckpoint, Error> {
        let mut expected = frame(&self.original.state.lineage, &self.openings[0], &self.part)?;
        expected[0] = self.context_digest()?;
        if checkpoint.public != expected || checkpoint.carried != self.openings[0] {
            return Err(Error::Input);
        }
        verify_full(
            &self.plan.vesta,
            binding,
            key,
            &[expected],
            &checkpoint.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        Ok(checkpoint)
    }

    /// Build the fixed W0 relation consuming this exact A1 key and original proof.
    /// A full-k16 pinned trivial slot is explicit even when sigma is shorter.
    ///
    /// # Errors
    /// A1 public/descriptor mismatch or failure of the exact four-input fold.
    pub fn wrapper_circuit(
        &self,
        first: &FirstCheckpoint,
        a1: &ProvingKey<Eq>,
        salt: Fq,
        config: &FoldConfig,
    ) -> Result<(WCircuit, Vec<Vec<Fq>>, AccumulatorT<Eq>), Error> {
        self.resume_first(a1.vk(), a1.binding(), first.clone(), config.kernel_budget)?;
        let opening = accumulate_generator(
            &self.plan.vesta,
            a1.binding(),
            a1.vk(),
            &[first.public.clone()],
            &first.proof,
            config.kernel_budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
            .map_err(|_| Error::Proof)?;
        let trivial = AccumulatorT::trivial(&self.plan.vesta, config.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let (fold, vesta) = create_fold(
            &self.plan.vesta,
            &[
                self.part.clone(),
                opening,
                trivial.as_input(),
                trivial.as_input(),
            ],
            salt.to_repr(),
            config,
        )
        .map_err(|_| Error::Proof)?;
        vesta
            .decide(&self.plan.vesta, config.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let allowed = a1
            .vk()
            .kagemusha_digest(a1.binding())
            .map_err(|_| Error::Artifact)?;
        let circuit = WCircuit::new(
            &self.plan.context,
            0,
            a1.binding().clone(),
            self.plan.vesta.clone(),
            vec![allowed],
            OmegaWitness {
                key: a1.vk().clone(),
                instances: first.public.clone(),
                proof: first.proof.clone(),
                length: u32::try_from(first.proof.len()).map_err(|_| Error::Input)?,
                fold: fold.to_bytes(),
            },
        )
        .map_err(|_| Error::Input)?;
        let instances = omega_instances(self.context_digest()?, &vesta)?;
        Ok((circuit, instances, vesta))
    }

    /// Prove W0 using the installed key. This checkpoint remains internal and
    /// cannot be used as the final Omega key or monetary completion.
    ///
    /// # Errors
    /// Wrong first/wrapper key, malformed instances, failed proof or decide.
    pub fn prove_wrapper(
        &self,
        first: &FirstCheckpoint,
        a1: &ProvingKey<Eq>,
        w: &ProvingKey<Ep>,
        salt: Fq,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<WrapperCheckpoint, Error> {
        let (circuit, instances, vesta) = self.wrapper_circuit(first, a1, salt, fold)?;
        let witness = Witness::from_circuit(w, &circuit, &instances).map_err(|_| Error::Prover)?;
        let output =
            create_proof_owned_with_claim(&self.plan.pallas, w, witness, randomness, config)
                .map_err(|_| Error::Prover)?;
        verify_full(
            &self.plan.pallas,
            w.binding(),
            w.vk(),
            &instances,
            &output.proof,
            fold.kernel_budget,
        )
        .map_err(|_| Error::Proof)?;
        Ok(WrapperCheckpoint {
            proof: output.proof,
            vesta,
            context: self.context_digest()?,
        })
    }

    /// Reverify the exact retained W proof, its complete Vesta obligation and
    /// its context binding before continuation. Transported metadata is never
    /// accepted as an operation-authorization boolean.
    ///
    /// # Errors
    /// Context substitution, wrong W key or failed proof/decide.
    pub fn resume_wrapper(
        &self,
        w: &WKey,
        checkpoint: WrapperCheckpoint,
        budget: MemoryBudget,
    ) -> Result<WrapperCheckpoint, Error> {
        if checkpoint.context != self.context_digest()? {
            return Err(Error::Input);
        }
        let public = omega_instances(checkpoint.context, &checkpoint.vesta)?;
        verify_full(
            &self.plan.pallas,
            w.verifier().binding(),
            w.verifying_key(),
            &public,
            &checkpoint.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        checkpoint
            .vesta
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        Ok(checkpoint)
    }

    /// Prepare A2's exact three-input Pallas fold: Q0, W0's opening, Q1.
    /// Its signature Q is hard-verified and authenticates the same three tapes.
    ///
    /// # Errors
    /// Wrong W/schema, changed originals or a failed proof/fold/decide.
    pub fn terminal_circuit(
        &self,
        wrapper: &WrapperCheckpoint,
        w: &WKey,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<(StageCircuit, Vec<Fp>, AccumulatorT<Ep>), Error> {
        self.resume_wrapper(w, wrapper.clone(), config.kernel_budget)?;
        let public = omega_instances(wrapper.context, &wrapper.vesta)?;
        let opening = accumulate_generator(
            &self.plan.pallas,
            w.verifier().binding(),
            w.verifying_key(),
            &public,
            &wrapper.proof,
            config.kernel_budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
            .map_err(|_| Error::Proof)?;
        let (fold, pallas) = create_fold(
            &self.plan.pallas,
            &[self.openings[0].clone(), opening, self.openings[1].clone()],
            salt.to_repr(),
            config,
        )
        .map_err(|_| Error::Proof)?;
        pallas
            .decide(&self.plan.pallas, config.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let split = SplitPlan::new(self.plan.context.clone(), 1, w.clone(), &self.plan.pallas)
            .map_err(|_| Error::Artifact)?;
        let circuit = StageCircuit {
            prepared: self.clone(),
            known: true,
            stage: Stage::Last {
                split,
                wrapper: wrapper.clone(),
                fold: fold.to_bytes(),
            },
        };
        let public = frame(
            &self.original.state.lineage,
            &pallas.as_input(),
            &wrapper.vesta.as_input(),
        )?;
        Ok((circuit, public, pallas))
    }

    /// Complete the actual terminal A2 proof under its installed key and retain
    /// its own opening, final Pallas claim and forwarded Vesta claim for Omega.
    ///
    /// # Errors
    /// Another source profile, failed circuit/proof or any failed complete decide.
    pub fn prove_terminal(
        &self,
        wrapper: &WrapperCheckpoint,
        w: &WKey,
        a2: &ProvingKey<Eq>,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<Terminal, Error> {
        let (circuit, public, pallas) = self.terminal_circuit(wrapper, w, salt, fold)?;
        let proof = prove_vesta(&self.plan, a2, &circuit, &public, randomness, config)?;
        let opening = accumulate_generator(
            &self.plan.vesta,
            a2.binding(),
            a2.vk(),
            &[public.clone()],
            &proof,
            fold.kernel_budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
            .map_err(|_| Error::Proof)?;
        opening
            .decide(&self.plan.vesta, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        Ok(Terminal {
            proof,
            instances: public,
            pallas,
            vesta: wrapper.vesta.clone(),
            opening,
        })
    }

    fn context_digest(&self) -> Result<Fp, Error> {
        let mut words = self.plan.context.schema().to_vec();
        words.extend(self.original.state.statement);
        words.extend(self.original.state.core);
        words.extend(self.original.state.rest);
        words.extend(self.original.state.lineage);
        for value in self
            .original
            .q
            .iter()
            .flat_map(|q| q.instances.iter().flatten())
        {
            words.extend(foreign_limbs(value).map(Fp::from_u128));
        }
        for ((index, kind), original) in object_kinds()
            .into_iter()
            .enumerate()
            .zip(&self.original.objects)
        {
            let length = u32::try_from(original.len()).map_err(|_| Error::Input)?;
            let mut raw = length.to_le_bytes().to_vec();
            raw.extend(original);
            let mut tape = vec![Fp::from((index + 1) as u64), Fp::from(u64::from(length))];
            for chunk in raw.chunks(31) {
                tape.push(le_value::<Fp>(chunk).ok_or(Error::Input)?);
            }
            words.extend([
                object_digest(kind, original)?,
                Fp::from(u64::from(length)),
                hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &tape),
            ]);
        }
        push_pallas(&mut words, &self.openings[0])?;
        Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words))
    }
}

/// Exact proof/public context of a completed first stage. The custody owner
/// encodes this checkpoint with its source-bound Norito archive metadata.
#[derive(Clone, Debug)]
pub struct FirstCheckpoint {
    public: Vec<Fp>,
    proof: Vec<u8>,
    carried: FoldInput<Ep>,
}
impl FirstCheckpoint {
    /// Original A1 proof bytes; never monetary completion.
    #[must_use]
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Exact public frame authenticated by A1.
    #[must_use]
    pub fn instances(&self) -> &[Fp] {
        &self.public
    }
}
/// Internal W result. All fields are proved or recomputed from the same
/// originals; the custody owner retains this separately from final Omega.
#[derive(Clone, Debug)]
pub struct WrapperCheckpoint {
    proof: Vec<u8>,
    vesta: AccumulatorT<Eq>,
    context: Fp,
}
impl WrapperCheckpoint {
    /// Original internal W proof.
    #[must_use]
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Canonical deciding Vesta accumulator authenticated by W.
    #[must_use]
    pub const fn vesta(&self) -> &AccumulatorT<Eq> {
        &self.vesta
    }
    /// Context digest binding both states, all originals and all Q public values.
    #[must_use]
    pub const fn context(&self) -> Fp {
        self.context
    }
}
/// Actual terminal Bootstrap A proof and every obligation final Omega must retain.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Original terminal A2 proof under the installed source key.
    pub proof: Vec<u8>,
    /// Canonical homogeneous 69-word A frame.
    pub instances: Vec<Fp>,
    /// Full Pallas output bound through D_A.
    pub pallas: AccumulatorT<Ep>,
    /// Full Vesta part forwarded from W.
    pub vesta: AccumulatorT<Eq>,
    /// The terminal A proof's own Vesta opening, a separate Omega input slot.
    pub opening: FoldInput<Eq>,
}

fn prove_vesta(
    plan: &Plan,
    key: &ProvingKey<Eq>,
    circuit: &StageCircuit,
    public: &[Fp],
    randomness: ProverRandomness<'_>,
    config: ProverConfig,
) -> Result<Vec<u8>, Error> {
    let d = key.binding().descriptor();
    if d.k != 16
        || d.instance_lengths != [69]
        || d.instance_types.as_deref() != Some(&[InstanceType::Bounded])
    {
        return Err(Error::Artifact);
    }
    let public = [public.to_vec()];
    let witness = Witness::from_circuit(key, circuit, &public).map_err(|_| Error::Prover)?;
    let output = create_proof_owned_with_claim(&plan.vesta, key, witness, randomness, config)
        .map_err(|_| Error::Prover)?;
    verify_full(
        &plan.vesta,
        key.binding(),
        key.vk(),
        &public,
        &output.proof,
        MemoryBudget::DEFAULT,
    )
    .map_err(|_| Error::Proof)?;
    Ok(output.proof)
}

fn object_digest(kind: ObjectKind, bytes: &[u8]) -> Result<Fp, Error> {
    let end = kind.body_len();
    if bytes.len() != end + 64 {
        return Err(Error::Input);
    }
    let mut words = vec![p_bytes_native(kind.signing_domain(), &bytes[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            bytes[end + offset..end + offset + 16]
                .try_into()
                .map_err(|_| Error::Input)?,
        )));
    }
    Ok(hash_with_domain(kind.object_domain(), &words))
}
fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) -> Result<(), Error> {
    if claim.source_k() != 16 {
        return Err(Error::Input);
    }
    let (x, y) = Option::<(Fp, Fp)>::from(claim.g().coordinates()).ok_or(Error::Input)?;
    words.extend([Fp::from(16), x, y]);
    for value in claim.challenges() {
        words.extend(foreign_limbs(value).map(Fp::from_u128));
    }
    Ok(())
}
fn frame(
    fields: &[Fp; 18],
    pallas: &FoldInput<Ep>,
    part: &FoldInput<Eq>,
) -> Result<Vec<Fp>, Error> {
    let mut digest = fields.to_vec();
    let mut claim = Vec::new();
    push_pallas(&mut claim, pallas)?;
    digest.extend(&claim[1..]);
    let mut out = vec![
        hash_with_domain(super::super::LINEAGE_DOMAIN, &digest),
        Fp::from(u64::from(part.source_k())),
    ];
    let (x, y) = Option::<(Fq, Fq)>::from(part.g().coordinates()).ok_or(Error::Input)?;
    for value in [x, y] {
        out.extend(foreign_limbs(&value).map(Fp::from_u128));
    }
    out.extend(part.challenges());
    let trivial = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).map_err(|_| Error::Artifact)?;
    let (x, y) = Option::<(Fq, Fq)>::from(trivial.coordinates()).ok_or(Error::Artifact)?;
    let coordinates = [x, y]
        .into_iter()
        .flat_map(|v| foreign_limbs(&v).map(Fp::from_u128))
        .collect::<Vec<_>>();
    for _ in 0..2 {
        out.extend(&coordinates);
        out.extend([Fp::ONE; K]);
    }
    out.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    out.extend(coordinates);
    if out.len() != 69 {
        return Err(Error::Input);
    }
    Ok(out)
}
fn omega_instances(context: Fp, vesta: &AccumulatorT<Eq>) -> Result<Vec<Vec<Fq>>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(vesta.g().coordinates()).ok_or(Error::Input)?;
    let digest = Option::<Fq>::from(Fq::from_repr(context.to_repr())).ok_or(Error::Input)?;
    let challenges = vesta
        .challenges()
        .iter()
        .map(|u| Option::<Fq>::from(Fq::from_repr(u.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![vec![digest], vec![x, y], challenges])
}

/// Fixed source-stage configuration. Its columns and range buses are metadata.
#[derive(Clone, Debug)]
pub struct StageConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
#[derive(Clone)]
enum Stage {
    First,
    Last {
        split: SplitPlan,
        wrapper: WrapperCheckpoint,
        fold: [u8; 1120],
    },
}
/// Complete first or terminal Bootstrap circuit. Stage and keys are fixed by
/// native assembly; source evidence and original tapes remain private witnesses.
#[derive(Clone)]
pub struct StageCircuit {
    prepared: Prepared,
    stage: Stage,
    known: bool,
}
impl StageCircuit {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn assign(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Vec<Word<Fp>>, LayoutError> {
        let source = &self.prepared.original;
        let plan = &self.prepared.plan;
        let core = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.core.map(|v| self.value(v)))?;
        let rest = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.rest.map(|v| self.value(v)))?;
        let state = StateCells::constrain_with_verifier(
            chip,
            region,
            &core.try_into().map_err(|_| LayoutError::Synthesis)?,
            &rest.try_into().map_err(|_| LayoutError::Synthesis)?,
        )?;
        let public = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.lineage.map(|v| self.value(v)))?;
        let public = LineagePublicCells::constrain(
            &mut chip.uint(),
            region,
            &public.try_into().map_err(|_| LayoutError::Synthesis)?,
        )?;
        let statement = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.statement.map(|v| self.value(v)))?;
        let statement = StatementCells::constrain_with_verifier(
            chip,
            region,
            Variant::Bootstrap,
            &statement.try_into().map_err(|_| LayoutError::Synthesis)?,
        )?;
        let mut q = Vec::new();
        for original in &source.q {
            let mut columns = Vec::new();
            for column in &original.instances {
                columns.push(
                    column
                        .iter()
                        .map(|v| scalar(chip, region, self.value(*v)))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            q.push(columns);
        }
        let values = source
            .objects
            .each_ref()
            .map(|object| object.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
        let objects =
            BootstrapObjects::decode(chip, bytes, region, values.each_ref().map(Vec::as_slice))?;
        let context_objects = objects.context().to_vec();
        let carried = &self.prepared.openings[0];
        let point = chip.witness_point(region, self.value(Ep::from(*carried.g())))?;
        let challenges = carried
            .challenges()
            .iter()
            .map(|v| scalar(chip, region, self.value(*v)))
            .collect::<Result<Vec<_>, _>>()?;
        let pallas = FoldInputCells::from_normalized(
            chip,
            region,
            16,
            point,
            challenges.try_into().map_err(|_| LayoutError::Synthesis)?,
        )?;
        let input = ContextInputs {
            own_statement: &statement,
            incoming_statement: None,
            predecessor: None,
            successor: ContextState {
                state: &state,
                public: &public,
            },
            incoming: None,
            q_instances: &q,
            objects: &context_objects,
            modes: &[],
            pallas_corrections: &[],
            vesta_corrections: &[],
            receive_results: None,
        };
        match &self.stage {
            Stage::First => {
                let proof = carrier(chip, bytes, region, &source.q[0].proof, self.known)?;
                let sigma =
                    sigma_binding(chip, bytes, region, &statement, &source.sigma, self.known)?;
                let (verified, _) = verify_sigma(
                    chip,
                    region,
                    plan.context.operation(),
                    &q[0],
                    &proof,
                    &sigma,
                )?;
                let output = close_first(
                    chip,
                    region,
                    &plan.context,
                    &input,
                    None,
                    &[verified],
                    &sigma,
                    None,
                    &plan.pallas,
                )?;
                output.words(chip, region)
            }
            Stage::Last {
                split,
                wrapper,
                fold,
            } => {
                let proof = carrier(chip, bytes, region, &wrapper.proof, self.known)?;
                let sigma =
                    sigma_binding(chip, bytes, region, &statement, &source.sigma, self.known)?;
                let (x, y) = Option::<(Fq, Fq)>::from(wrapper.vesta.g().coordinates())
                    .ok_or(LayoutError::Synthesis)?;
                let coordinates = [
                    scalar(chip, region, self.value(x))?,
                    scalar(chip, region, self.value(y))?,
                ];
                let challenges = wrapper
                    .vesta
                    .challenges()
                    .iter()
                    .map(|v| chip.uint().glue().witness(region, self.value(*v)))
                    .collect::<Result<Vec<_>, _>>()?;
                let vesta = VestaClaimCells::constrain(
                    chip,
                    region,
                    16,
                    coordinates,
                    challenges.try_into().map_err(|_| LayoutError::Synthesis)?,
                )?;
                let resumed = resume_context(
                    chip,
                    region,
                    split,
                    &input,
                    &[],
                    &pallas,
                    &vesta,
                    &proof,
                    &sigma,
                )?;
                let proof = carrier(chip, bytes, region, &source.q[1].proof, self.known)?;
                let verified = verify_q(chip, region, plan.context.operation(), 1, &q[1], &proof)?;
                let slots = bind_signature_q(
                    chip,
                    region,
                    plan.context.operation(),
                    1,
                    &plan.signatures,
                    &verified,
                )?;
                objects.authenticate(
                    chip,
                    region,
                    plan.policy,
                    BootstrapInputs {
                        state: &state,
                        lineage: &public,
                        sigma: &sigma[0],
                        signatures: slots.slots(),
                    },
                )?;
                for (slot, expected) in slots.slots().iter().zip(q[1][0].chunks_exact(10)) {
                    let actual = std::iter::once(slot.message())
                        .chain(slot.key())
                        .chain(slot.signature())
                        .chain(std::iter::once(slot.valid().word()));
                    for (actual, expected) in actual.zip(expected) {
                        let expected = super::super::bounded_word(chip, region, expected)?;
                        GlueChip::assert_equal(region, actual, &expected)?;
                    }
                }
                let fold = carrier(chip, bytes, region, fold, self.known)?;
                let output = close_stage(
                    chip,
                    region,
                    split,
                    &resumed,
                    None,
                    None,
                    None,
                    &[verified],
                    &fold,
                )?;
                output.words(chip, region, &public)
            }
        }
    }
}
impl Circuit<Fp> for StageCircuit {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        let verifier = VerifierConfig::configure_serialized_foreign(meta, SOURCE_RANGE_BUSES)
            .expect("fixed Bootstrap source buses are valid");
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        StageConfig {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(
        &self,
        config: StageConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "complete production Bootstrap stage",
            |mut region| self.assign(&mut chip, &mut bytes, &mut region),
        )?;
        if output.len() != 69 {
            return Err(LayoutError::Synthesis);
        }
        for (row, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
fn scalar(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    value: Value<Fq>,
) -> Result<ScalarCells<Ep>, LayoutError> {
    let lo = chip
        .uint()
        .assign::<128>(region, value.map(|v| foreign_limbs(&v)[0]))?;
    let hi = chip
        .uint()
        .assign::<127>(region, value.map(|v| foreign_limbs(&v)[1]))?;
    ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
}
fn carrier(
    chip: &mut VerifierChip<Ep>,
    bytes: &mut BytesChip<Fp>,
    region: &mut Region<'_, Fp>,
    body: &[u8],
    known: bool,
) -> Result<ProofMessageCells, LayoutError> {
    let length = u32::try_from(body.len()).map_err(|_| LayoutError::BoundsFailure)?;
    let raw = length
        .to_le_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .map(|v| {
            if known {
                Value::known(v)
            } else {
                Value::unknown()
            }
        })
        .collect::<Vec<_>>();
    let mut segments = vec![SegmentSpec::little(0, 4)];
    segments.extend(le_message_segments(4, body.len() / 32));
    let run = bytes.run(
        region,
        &raw,
        &raw.chunks(31).map(<[Value<u8>]>::len).collect::<Vec<_>>(),
        &segments,
    )?;
    ProofMessageCells::from_run(chip, region, &run, 0, body.len())
}
fn sigma_binding(
    chip: &mut VerifierChip<Ep>,
    bytes: &mut BytesChip<Fp>,
    region: &mut Region<'_, Fp>,
    statement: &StatementCells,
    sigma: &[u8],
    known: bool,
) -> Result<Vec<SigmaBindingCells>, LayoutError> {
    let length = u32::try_from(sigma.len()).map_err(|_| LayoutError::BoundsFailure)?;
    let raw = length
        .to_le_bytes()
        .into_iter()
        .chain(sigma.iter().copied())
        .map(|v| {
            if known {
                Value::known(v)
            } else {
                Value::unknown()
            }
        })
        .collect::<Vec<_>>();
    let run = bytes.run(
        region,
        &raw,
        &raw.chunks(31).map(<[Value<u8>]>::len).collect::<Vec<_>>(),
        &[SegmentSpec::little(0, 4)],
    )?;
    let selector = chip.uint().glue().constant(region, Fp::ZERO)?;
    Ok(vec![SigmaBindingCells::from_run(
        chip, region, statement, selector, &run,
    )?])
}

#[cfg(test)]
#[path = "bootstrap/tests.rs"]
mod tests;
