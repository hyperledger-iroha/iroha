//! Fixed-owner Receive continuation over real sigma, signature-Q and Omega proofs.
//!
//! Burn uses an unrelated but real receiver Omega under a correctly signed
//! Send package. Accepted credit uses distinct payer Load and receiver Bootstrap
//! heads proved under the same immutable compact catalog. Every owner must fit
//! k16, including fixed maximum incoming capacities; these diagnostic catalogs
//! do not admit the Receive terminal or establish full release qualification.
#![allow(clippy::duplicate_mod)]
#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
/// Real rooted receiver predecessor, with explicit partial-catalog provenance.
#[path = "bootstrap_omega.rs"]
pub mod bootstrap_outer;
mod common;
/// Genuine distinct payer/receiver heads rebuilt under one compact outer key.
#[path = "compact_catalog.rs"]
mod compact_catalog;
/// Real Receive/Send source proofs and exact depth32 map witnesses.
#[path = "a_receive.rs"]
pub mod receive_components;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    a_relation::{
        AProofPlan, IncomingLineageCells, LineagePublicCells, ProofMessageCells, QProofPlan,
        SigmaBindingCells, VestaClaimCells,
        context::{
            ContextIncoming, ContextIncomingProof, ContextInputs, ContextObjectSpec, ContextPlan,
            ContextPredecessor, ContextState,
        },
        incoming_transport::IncomingTransportPlan,
        own::OwnPolicy,
        receive::{
            MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES, PAYMENT_PROOF_BUDGET, ReceiveObjectInputs,
            ReceiveObjectSources, ReceiveObjects, ReceiveProofDigest, ReceiveProofInputs,
            ReceiveProofSources, ReceiveSignedObjects, ReceiveStageInputs, ReceiveStagePlan,
            ReceiveStageWitness,
            authorization::{ReceiveAuthorizationObjects, ReceiveAuthorizationSources},
        },
        results::ReceiveResultClaims,
        schedule::{OperationTask, sigma_selector},
        split::{
            ContextLinkCells, SplitPlan, WCircuit, WKey, close_first, close_stage, resume_context,
        },
        verify_predecessor, verify_q,
    },
    admin_sigma::StateWitness,
    operation_relation::{
        incoming_statement::IncomingStatementCells,
        map_effects::{InsertCells, ReceiveMapWitness},
        objects::ObjectKind,
        state::StateCells,
        statement::StatementCells,
    },
    q_sigma::native::IncomingMode,
    q_signature::{QSignatureCircuit, QSignaturePlan, SignatureWitness},
    tree::IndexedInsert,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, ProverOutput, ProvingKey, VerifyingKey, Witness,
    check::{CheckMode, check},
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{
    UintChip, Word,
    bytes::{
        chunk_segments, p_bytes_native,
        tape::{BytesChip, BytesConfig, SegmentSpec},
        variable::ActiveBytes,
    },
    imt::{LeafCells, OpeningCells, PathCells},
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::{ModeCells, ledger::Variant},
    verifier::{VerifierChip, VerifierConfig, VerifierKeyCells, VerifierPlan},
};
use std::sync::Arc;

#[derive(Clone)]
struct QSource {
    plan: QProofPlan,
    proof: Vec<u8>,
    instances: Vec<Vec<Fq>>,
    opening: FoldInput<Ep>,
}
/// Exact original lineage artifact, independent of the operation that produced it.
#[derive(Clone)]
struct Head {
    state: StateWitness,
    key: VerifyingKey<Ep>,
    binding: iroha_plonk::DescriptorBinding,
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    vesta: AccumulatorT<Eq>,
    opening: FoldInput<Ep>,
}
impl From<bootstrap_outer::RootedBootstrapOmega> for Head {
    fn from(artifact: bootstrap_outer::RootedBootstrapOmega) -> Self {
        Self {
            state: StateWitness::from(&artifact.source.state),
            key: artifact.key,
            binding: artifact.binding,
            proof: artifact.proof,
            pallas: artifact.source.pallas,
            vesta: artifact.vesta,
            opening: artifact.opening,
        }
    }
}
struct Source {
    own: receive_components::ReceiveQ,
    predecessor: Head,
    incoming_head: Head,
    accept: bool,
    plan: ReceiveStagePlan,
    q: Vec<QSource>,
    objects: [Vec<u8>; 11],
    commitments: Vec<[Fp; 3]>,
    incoming: Vec<u8>,
    params: PinnedParams<Ep>,
}
impl Source {
    fn mode_words(&self) -> [Fp; 3] {
        if self.accept {
            [Fp::ONE, Fp::ZERO, Fp::ZERO]
        } else {
            [Fp::ZERO, Fp::ONE, Fp::ZERO]
        }
    }
}
#[derive(Clone, Copy, Debug)]
enum IngestionCapacity {
    Descriptor,
    CanonicalEnvelope,
}
impl IngestionCapacity {
    fn capacities(self, omega: usize, sigma: usize) -> [usize; 2] {
        match self {
            Self::Descriptor => [omega, sigma],
            Self::CanonicalEnvelope => [MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES],
        }
    }
}
fn frame(bytes: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    out.extend(bytes);
    out
}
fn public_bytes(fields: &[Fp; 18]) -> Vec<u8> {
    let mut out = 1u16.to_le_bytes().to_vec();
    for i in [1, 2, 3, 4] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[5].to_repr());
    for i in [6, 7] {
        out.extend(&fields[i].to_repr()[..16]);
    }
    out.extend(fields[8].to_repr());
    out.push(4);
    for i in [10, 9, 12, 11] {
        out.extend(fields[i].to_repr()[..16].iter().rev());
    }
    out.extend(&fields[13].to_repr()[..13]);
    out.extend(&fields[14].to_repr()[..16]);
    out.extend(fields[15].to_repr());
    out.extend(fields[16].to_repr());
    assert_eq!(out.len(), 320);
    out
}
fn signed_digest(kind: ObjectKind, raw: &[u8]) -> Fp {
    let end = kind.body_len();
    let mut words = vec![p_bytes_native(kind.signing_domain(), &raw[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            raw[end + offset..end + offset + 16].try_into().unwrap(),
        )));
    }
    hash_with_domain(kind.object_domain(), &words)
}
fn commitment(spec: ContextObjectSpec, raw: &[u8], digest: Fp, active: bool) -> [Fp; 3] {
    let mut words = vec![
        Fp::from(u64::from(spec.tag)),
        Fp::from(u64::from(spec.capacity)),
    ];
    let tape_digest = if active {
        words.extend([
            Fp::from(u64::try_from(raw.len()).unwrap()),
            p_bytes_native(u64::from_le_bytes(*b"kgwcact1"), &frame(raw)),
        ]);
        hash_with_domain(u64::from_le_bytes(*b"kgwcact1"), &words)
    } else {
        for chunk in frame(raw).chunks(31) {
            let mut repr = [0; 32];
            repr[..chunk.len()].copy_from_slice(chunk);
            words.push(Fp::from_repr(repr).unwrap());
        }
        hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &words)
    };
    [
        digest,
        Fp::from(u64::try_from(raw.len()).unwrap()),
        tape_digest,
    ]
}
fn receipt(
    statement: &[Fp; 26],
    wallet: &[Fp],
    proof: Fp,
    payment: Fp,
    secret: u64,
    nonce: u64,
) -> bootstrap_objects::Signed {
    let operation = hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[wallet[0], wallet[1], statement[16], statement[17]],
    );
    let mut body = 1u16.to_le_bytes().to_vec();
    body.extend(bootstrap_objects::id(statement[3], statement[4]));
    body.extend(bootstrap_objects::id(wallet[0], wallet[1]));
    body.extend(bootstrap_objects::small_id(31, 32));
    body.extend(&statement[9].to_repr()[..16]);
    for word in [
        operation,
        statement[14],
        statement[15],
        hash_with_domain(iroha_plonk_gadgets::statement::STATEMENT_DOMAIN, statement),
        proof,
    ] {
        body.extend(word.to_repr());
    }
    body.extend(bootstrap_objects::small_id(501, 502));
    body.extend(payment.to_repr());
    bootstrap_objects::sign(ObjectKind::Receipt, body, secret, nonce)
}
fn signature_q(
    params: &PinnedParams<Ep>,
    schema: &QSignaturePlan,
    witnesses: Vec<SignatureWitness>,
    seed: u8,
) -> QSource {
    let circuit = QSignatureCircuit::new(schema.clone(), witnesses).unwrap();
    let instances = circuit
        .instances(&vec![true; schema.slots().len()])
        .unwrap()
        .to_vec();
    let key = keygen_pk_v2(
        params,
        &circuit,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let output = create_proof_owned_with_claim(
        params,
        &key,
        Witness::from_circuit(&key, &circuit, &instances).unwrap(),
        common::recovery(seed),
        ProverConfig::default(),
    )
    .unwrap();
    output
        .opening
        .decide(params, MemoryBudget::DEFAULT)
        .unwrap();
    QSource {
        plan: QProofPlan::new(
            VerifierPlan::new(key.binding().clone(), params.clone()).unwrap(),
            key.vk().clone(),
        )
        .unwrap(),
        proof: output.proof,
        instances,
        opening: FoldInput::from_opening(*output.opening.g(), output.opening.challenges()).unwrap(),
    }
}
fn source(predecessor: Head, payer: Option<Head>, capacity: IngestionCapacity) -> Source {
    use OperationTask::*;
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let before = predecessor.state;
    let accept = payer.is_some();
    let incoming_head = payer.unwrap_or_else(|| predecessor.clone());
    assert_eq!(incoming_head.binding, predecessor.binding);
    assert_eq!(incoming_head.key.to_bytes(), predecessor.key.to_bytes());
    assert_eq!(incoming_head.state.lineage[17], before.lineage[17]);
    let mut own = if accept {
        receive_components::genuine_receive_source_for_heads(
            &before,
            &incoming_head.state,
            true,
            true,
            IncomingMode::Accept,
        )
    } else {
        receive_components::genuine_receive_source_for(&before, false, false, IncomingMode::Trivial)
    };
    let mut incoming = public_bytes(&incoming_head.state.lineage);
    incoming.extend(&incoming_head.proof);
    incoming.extend(incoming_head.pallas.to_bytes());
    incoming.extend(incoming_head.vesta.to_bytes());
    assert!(incoming.len() - 320 <= 4_821);
    assert!(incoming.len() - 320 + own.incoming_sigma.len() <= PAYMENT_PROOF_BUDGET);
    let proof_digest = p_bytes_native(
        u64::from_le_bytes(*b"kgwprf_1"),
        &[frame(&incoming), frame(&own.incoming_sigma)].concat(),
    );
    let send_receipt = receipt(
        &own.send.statement,
        &own.send.before.lineage[6..8],
        proof_digest,
        Fp::ZERO,
        29,
        71,
    );
    let request = own.send.objects[1].clone();
    let payer = own.send.objects[0].clone();
    let package = hash_with_domain(
        u64::from_le_bytes(*b"kgwpkg_1"),
        &[
            hash_with_domain(
                iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
                &own.send.statement,
            ),
            proof_digest,
            send_receipt.digest(),
        ],
    );
    let mut payment = 1u16.to_le_bytes().to_vec();
    payment.extend(signed_digest(ObjectKind::Request, &request).to_repr());
    payment.extend(bootstrap_objects::sec1(bootstrap_objects::key(43)));
    payment.extend(signed_digest(ObjectKind::Credential, &payer).to_repr());
    payment.extend(package.to_repr());
    assert_eq!(payment.len(), 163);
    let payment_digest = p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &payment);
    own.bind_payment(payment_digest, accept, accept);
    let own_receipt = receipt(
        &own.witness.statement,
        &before.lineage[6..8],
        p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &frame(&own.sigma)),
        payment_digest,
        43,
        73,
    );
    let (_, certificate, credential) =
        bootstrap_objects::enrollment_for(bootstrap_objects::Identity::Receiver);
    assert_eq!(credential.digest(), before.core[7]);
    let policy = OwnPolicy::new([1, 2], [31, 32], bootstrap_objects::key(23)).unwrap();
    let schemas = ReceiveStagePlan::signature_schemas(Variant::Receive, policy).unwrap();
    let q_own = signature_q(
        &params,
        &schemas[0],
        vec![
            own_receipt.signature,
            credential.signature,
            certificate.signature,
        ],
        245,
    );
    let request_signature = {
        let (_, _, receiver) =
            bootstrap_objects::enrollment_for(bootstrap_objects::Identity::Receiver);
        assert_eq!(receiver.signature.key, credential.signature.key);
        // Re-sign the exact Request body under its actual receiver key; the
        // deterministic nonce must reproduce the retained signature bytes.
        let signed = bootstrap_objects::sign(
            ObjectKind::Request,
            request[..ObjectKind::Request.body_len()].to_vec(),
            43,
            59,
        );
        assert_eq!(signed.bytes, request);
        signed.signature
    };
    let q_incoming = signature_q(
        &params,
        &schemas[1],
        vec![send_receipt.signature, request_signature],
        246,
    );
    let q = vec![
        QSource {
            plan: own.q.clone(),
            proof: own.proof.clone(),
            instances: own.instances.clone(),
            opening: own.opening.clone(),
        },
        q_own,
        q_incoming,
    ];
    let operation = AProofPlan::new(
        Variant::Receive,
        own.sigma_plan.clone(),
        q.iter().map(|q| q.plan.clone()).collect(),
        Some(VerifierPlan::new(predecessor.binding.clone(), params.clone()).unwrap()),
        &params,
    )
    .unwrap();
    let [omega_capacity, sigma_capacity] =
        capacity.capacities(incoming.len(), own.incoming_sigma.len());
    assert!(incoming.len() <= omega_capacity && own.incoming_sigma.len() <= sigma_capacity);
    eprintln!(
        "RECEIVE_INGESTION {capacity:?} omega_capacity={omega_capacity} sigma_capacity={sigma_capacity}"
    );
    let specs =
        ReceiveStagePlan::context_specs(Variant::Receive, omega_capacity, sigma_capacity).unwrap();
    let context = ContextPlan::with_schedule(
        operation,
        vec![
            vec![],
            vec![0],
            vec![],
            vec![],
            vec![],
            vec![1],
            vec![2],
            vec![],
            vec![],
        ],
        Some(0),
        specs.clone(),
    )
    .unwrap()
    .with_operation_tasks(vec![
        vec![],
        vec![],
        vec![ReceiveProofs],
        vec![ReceiveProofDigest],
        vec![ReceiveObjects],
        vec![ReceiveAuthorization, ReceiveOwnProof],
        vec![ReceiveSignatures],
        vec![ReceiveNonmembership, ReceiveBlacklist],
        vec![ReceiveEffects],
    ])
    .unwrap();
    let plan = ReceiveStagePlan::new(context, policy).unwrap();
    let objects = [
        request.clone(),
        payer.clone(),
        send_receipt.bytes,
        payment,
        incoming.clone(),
        own.incoming_sigma.clone(),
        credential.bytes.clone(),
        certificate.bytes.clone(),
        own_receipt.bytes,
        credential.bytes,
        certificate.bytes,
    ];
    let kinds = [
        Some(ObjectKind::Request),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Receipt),
        None,
        None,
        None,
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
        Some(ObjectKind::Receipt),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
    ];
    let commitments = specs
        .into_iter()
        .enumerate()
        .map(|(i, spec)| {
            let digest = kinds[i].map_or_else(
                || match i {
                    3 => payment_digest,
                    4 => proof_digest,
                    5 => p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &frame(&objects[i])),
                    _ => unreachable!(),
                },
                |kind| signed_digest(kind, &objects[i]),
            );
            commitment(spec, &objects[i], digest, matches!(i, 4 | 5))
        })
        .collect();
    Source {
        own,
        predecessor,
        incoming_head,
        accept,
        plan,
        q,
        objects,
        commitments,
        incoming,
        params,
    }
}

#[derive(Clone, Copy)]
struct Cells {
    known: bool,
}
impl Cells {
    fn value<T: Copy>(self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn bytes(self, source: &[u8]) -> Vec<Value<u8>> {
        source.iter().map(|v| self.value(*v)).collect()
    }
    fn words<const N: usize>(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: &[Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
    fn scalar(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Fq,
    ) -> Result<ScalarCells<Ep>, Error> {
        let [low, high] = foreign_limbs(&value);
        let low = chip.uint().assign::<128>(region, self.value(low))?;
        let high = chip.uint().assign::<127>(region, self.value(high))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &low, &high)
    }
    fn q_instance(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Fq,
        ty: InstanceType,
    ) -> Result<ScalarCells<Ep>, Error> {
        if matches!(ty, InstanceType::Bounded | InstanceType::Bits(0..=253)) {
            // A hard Q input of this declared type has an injective native Fp
            // representation. Preserve that checked certificate for downstream
            // context/type checks; never reduce a foreign scalar modulo p.
            let native =
                Option::<Fp>::from(Fp::from_repr(value.to_repr())).ok_or(Error::Synthesis)?;
            let word = chip.uint().glue().witness(region, self.value(native))?;
            ScalarCells::from_native_word(&mut chip.uint(), region, &word)
        } else {
            self.scalar(chip, region, value)
        }
    }
    fn pallas(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: &FoldInput<Ep>,
    ) -> Result<FoldInputCells<Ep>, Error> {
        let point = chip.witness_point(region, self.value(Ep::from(*value.g())))?;
        let scalars = value
            .challenges()
            .iter()
            .map(|v| self.scalar(chip, region, *v))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        FoldInputCells::from_normalized(chip, region, value.source_k(), point, scalars)
    }
    fn vesta(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: &AccumulatorT<Eq>,
    ) -> Result<VestaClaimCells, Error> {
        let (x, y) = Option::from(value.g().coordinates()).ok_or(Error::Synthesis)?;
        let coordinates = [self.scalar(chip, region, x)?, self.scalar(chip, region, y)?];
        let challenges = self.words(chip, region, value.challenges())?;
        VestaClaimCells::constrain(chip, region, 16, coordinates, challenges)
    }
    fn state(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        state: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), Error> {
        let core = self.words(chip, region, &state.core)?;
        let rest = self.words(chip, region, &state.rest)?;
        let state_cells = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = self.words(chip, region, &state.lineage)?;
        Ok((
            state_cells,
            LineagePublicCells::constrain(&mut chip.uint(), region, &public)?,
        ))
    }
    fn proof(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        raw: &[u8],
    ) -> Result<ProofMessageCells, Error> {
        if !raw.len().is_multiple_of(32) {
            return Err(Error::Synthesis);
        }
        let length = chip.uint().assign::<32>(
            region,
            self.value(u128::try_from(raw.len()).map_err(|_| Error::BoundsFailure)?),
        )?;
        let messages = raw
            .chunks_exact(32)
            .map(|chunk| {
                iroha_plonk_gadgets::bytes::element::LeElement::assign(
                    &mut chip.uint(),
                    region,
                    self.value(chunk.try_into().map_err(|_| Error::Synthesis)?),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        assert!(ProofMessageCells::from_messages(Vec::new(), length.clone()).is_err());
        ProofMessageCells::from_messages(messages, length)
    }
    fn key(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        key: &VerifyingKey<Ep>,
    ) -> Result<VerifierKeyCells<Ep>, Error> {
        let iroha_plonk::transcript::TranscriptRepr::Base(repr) = *key.transcript_repr() else {
            return Err(Error::Synthesis);
        };
        let fixed = key
            .fixed_commitments()
            .iter()
            .map(|p| self.value(Ep::from(*p)))
            .collect::<Vec<_>>();
        let permutation = key
            .permutation_commitments()
            .iter()
            .map(|p| self.value(Ep::from(*p)))
            .collect::<Vec<_>>();
        chip.witness_key(region, self.value(repr), &fixed, &permutation)
    }
    fn active(
        self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        raw: &[u8],
        capacity: usize,
        segments: &[SegmentSpec],
    ) -> Result<ActiveBytes<Fp>, Error> {
        let values = if self.known {
            Value::known(raw.to_vec())
        } else {
            Value::unknown()
        };
        ActiveBytes::assign(&mut chip.uint(), bytes, region, capacity, &values, segments)
    }
    fn insertion(
        self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        witness: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, Error> {
        let leaf = witness.leaf;
        let values = uint
            .glue()
            .witnesses(
                region,
                &[leaf.key, leaf.value, leaf.next_key].map(|v| self.value(v)),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let mut paths = Vec::new();
        for (index, siblings) in [
            (witness.leaf_slot, witness.leaf_siblings),
            (witness.slot, witness.slot_siblings),
        ] {
            let index = uint
                .glue()
                .witness(region, self.value(Fp::from(u64::from(index))))?;
            let siblings = uint
                .glue()
                .witnesses(region, &siblings.map(|v| self.value(v)))?
                .try_into()
                .map_err(|_| Error::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| Error::Synthesis)?;
        Ok(InsertCells {
            low: OpeningCells {
                leaf: LeafCells::from_words(values),
                path: low,
            },
            slot,
        })
    }
}
#[derive(Clone)]
struct Continuation {
    plan: SplitPlan,
    proof: Vec<u8>,
    carried: AccumulatorT<Ep>,
    vesta: AccumulatorT<Eq>,
    history: Vec<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
}
#[derive(Clone)]
struct Stage {
    source: Arc<Source>,
    continuation: Option<Continuation>,
    pallas: AccumulatorT<Ep>,
    first: AccumulatorT<Ep>,
    fold: Vec<u8>,
    known: bool,
    mutation: Option<ContinuationMutation>,
}
#[derive(Clone, Copy, Debug)]
enum ContinuationMutation {
    Object { slot: usize, word: usize },
    SwapObjects,
    Result(usize),
    Opening,
    Mode,
    IncomingStatement,
    QChunk,
    DropHistory,
    ReverseHistory,
}
#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl Circuit<Fp> for Stage {
    type Config = Config;
    type Params = usize;
    type FloorPlanner = SimpleFloorPlanner;
    fn params(&self) -> usize {
        // Only operation-terminal A keys can enter the uniform tagged3 Omega
        // catalog. Internal tagged4 keys are pinned by their exact W wrapper.
        if self
            .continuation
            .as_ref()
            .is_some_and(|c| c.plan.is_terminal())
        {
            3
        } else {
            4
        }
    }
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        Self::configure_with_params(meta, 4)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, buses: usize) -> Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, buses).unwrap();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        Config {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let cells = Cells { known: self.known };
        let source = &self.source;
        let stage = self.continuation.as_ref().map_or(0, |c| c.plan.stage());
        let plan = source.plan.context();
        let tasks = plan.operation_tasks(stage).ok_or(Error::Synthesis)?;
        let out = layouter.assign_region(
            || "genuine Receive fixed owner stage",
            |mut region| {
                let (old, pred_public) =
                    cells.state(&mut chip, &mut region, &source.own.witness.before)?;
                let (new, next_public) =
                    cells.state(&mut chip, &mut region, &source.own.witness.after)?;
                let fields = cells.words(&mut chip, &mut region, &source.own.witness.statement)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Receive,
                    &fields,
                )?;
                let mut incoming_fields = source.own.send.statement;
                if matches!(self.mutation, Some(ContinuationMutation::IncomingStatement)) {
                    incoming_fields[17] += Fp::ONE;
                }
                let fields = cells.words(&mut chip, &mut region, &incoming_fields)?;
                let lanes = chip.operation_lanes()?;
                let incoming_statement = IncomingStatementCells::constrain(
                    &mut UintChip::new(lanes.glue, lanes.range),
                    lanes.hash,
                    &mut region,
                    Variant::Send,
                    &fields,
                )?;
                let q_instances = source
                    .q
                    .iter()
                    .enumerate()
                    .map(|(q_index, q)| {
                        q.instances
                            .iter()
                            .enumerate()
                            .map(|(column, col)| {
                                col.iter()
                                    .enumerate()
                                    .map(|(index, v)| {
                                        let mut value = *v;
                                        if q_index == 0
                                            && column == 0
                                            && index
                                                == source
                                                    .own
                                                    .sigma_plan
                                                    .chunk_range(1)
                                                    .ok_or(Error::Synthesis)?
                                                    .start
                                            && matches!(
                                                self.mutation,
                                                Some(ContinuationMutation::QChunk)
                                            )
                                        {
                                            value += Fq::ONE;
                                        }
                                        let ty = *q
                                            .plan
                                            .verifier()
                                            .binding()
                                            .descriptor()
                                            .instance_types
                                            .as_ref()
                                            .and_then(|types| types.get(column))
                                            .ok_or(Error::Synthesis)?;
                                        cells.q_instance(&mut chip, &mut region, value, ty)
                                    })
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, _>>()?;
                let pp = cells.pallas(
                    &mut chip,
                    &mut region,
                    &source.predecessor.pallas.as_input(),
                )?;
                let pv = cells.vesta(&mut chip, &mut region, &source.predecessor.vesta)?;
                let ip = cells.pallas(
                    &mut chip,
                    &mut region,
                    &source.incoming_head.pallas.as_input(),
                )?;
                let iv = cells.vesta(&mut chip, &mut region, &source.incoming_head.vesta)?;
                let original_fields =
                    cells.words(&mut chip, &mut region, &source.incoming_head.state.lineage)?;
                let public_valid = chip.uint().glue().boolean(&mut region, cells.value(true))?;
                let incoming_public = IncomingLineageCells::constrain(
                    &mut chip.uint(),
                    &mut region,
                    &original_fields,
                    &public_valid,
                )?;
                let modes = (0..4)
                    .map(|index| {
                        let values = if index == 0
                            && matches!(self.mutation, Some(ContinuationMutation::Mode))
                        {
                            if source.accept {
                                [Fp::ZERO, Fp::ONE, Fp::ZERO]
                            } else {
                                [Fp::ONE, Fp::ZERO, Fp::ZERO]
                            }
                        } else {
                            source.mode_words()
                        };
                        let bits = cells.words(&mut chip, &mut region, &values)?;
                        ModeCells::constrain(chip.uint().glue(), &mut region, &bits)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let trivial = AccumulatorT::trivial(&source.params, MemoryBudget::DEFAULT)
                    .map_err(|_| Error::Synthesis)?;
                let corrections = [
                    chip.witness_point(&mut region, cells.value(Ep::from(*trivial.g())))?,
                    chip.witness_point(&mut region, cells.value(Ep::from(*trivial.g())))?,
                ];
                let trivial_v =
                    AccumulatorT::<Eq>::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT)
                        .map_err(|_| Error::Synthesis)?;
                let (x, y) = Option::from(trivial_v.g().coordinates()).ok_or(Error::Synthesis)?;
                let vcorrections = [[
                    cells.scalar(&mut chip, &mut region, x)?,
                    cells.scalar(&mut chip, &mut region, y)?,
                ]];
                let exported = if matches!(self.mutation, Some(ContinuationMutation::Opening)) {
                    trivial.as_input()
                } else {
                    source.incoming_head.opening.clone()
                };
                let opening = cells.pallas(&mut chip, &mut region, &exported)?;
                let mut verdicts = [true, source.accept, true, true, true];
                if let Some(ContinuationMutation::Result(index)) = self.mutation {
                    verdicts[index] = !verdicts[index];
                }
                let results = ReceiveResultClaims::assign(
                    chip.uint().glue(),
                    &mut region,
                    plan.receive_results().ok_or(Error::Synthesis)?,
                    verdicts.map(|v| cells.value(v)),
                )?
                .with_opening(&opening)?;
                let mut proposals = source.commitments.clone();
                match self.mutation {
                    Some(ContinuationMutation::Object { slot, word }) => {
                        proposals[slot][word] += Fp::ONE;
                    }
                    Some(ContinuationMutation::SwapObjects) => proposals.swap(0, 1),
                    _ => {}
                }
                let commitments = proposals
                    .iter()
                    .map(|v| v.map(|v| cells.value(v)))
                    .collect::<Vec<_>>();
                let context =
                    plan.assign_receive_object_claims(&mut chip, &mut region, &commitments)?;
                let own_index = chip.uint().glue().constant(
                    &mut region,
                    Fp::from(u64::from(sigma_selector(4, 0).ok_or(Error::Synthesis)?)),
                )?;
                let incoming_index = chip.uint().glue().constant(
                    &mut region,
                    Fp::from(u64::from(sigma_selector(3, 0).ok_or(Error::Synthesis)?)),
                )?;
                let own_chunks = source
                    .own
                    .sigma_plan
                    .chunk_range(0)
                    .ok_or(Error::Synthesis)?
                    .map(|i| {
                        iroha_kagemusha_proof::a_relation::bounded_word(
                            &mut chip,
                            &mut region,
                            &q_instances[0][0][i],
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let incoming_chunks = source
                    .own
                    .sigma_plan
                    .chunk_range(1)
                    .ok_or(Error::Synthesis)?
                    .map(|i| {
                        iroha_kagemusha_proof::a_relation::bounded_word(
                            &mut chip,
                            &mut region,
                            &q_instances[0][0][i],
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut own_sigma =
                    SigmaBindingCells::from_statement(&statement, own_index.clone(), own_chunks);
                let mut incoming_sigma = SigmaBindingCells::from_incoming(
                    &incoming_statement,
                    incoming_index.clone(),
                    incoming_chunks,
                );
                if tasks.contains(&OperationTask::ReceiveOwnProof) {
                    let raw = frame(&source.own.sigma);
                    let run = bytes.run(
                        &mut region,
                        &cells.bytes(&raw),
                        &chunk_segments(0, raw.len()),
                        &[SegmentSpec::little(0, 4)],
                    )?;
                    own_sigma = SigmaBindingCells::from_run(
                        &mut chip,
                        &mut region,
                        &statement,
                        own_index,
                        &run,
                    )?;
                }
                let needs_transport = tasks.iter().any(|t| {
                    matches!(
                        t,
                        OperationTask::ReceiveObjects | OperationTask::ReceiveProofs
                    )
                });
                let needs_digest = tasks.contains(&OperationTask::ReceiveProofDigest);
                let transport_plan = IncomingTransportPlan::new(plan.operation())?;
                let omega_raw = if needs_transport || needs_digest {
                    let segments = if needs_transport {
                        transport_plan.active_segments()?
                    } else {
                        vec![]
                    };
                    Some(cells.active(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &source.incoming,
                        plan.object_specs()[4].capacity as usize,
                        &segments,
                    )?)
                } else {
                    None
                };
                let sigma_raw = if tasks.contains(&OperationTask::ReceiveObjects) || needs_digest {
                    let segments = if tasks.contains(&OperationTask::ReceiveObjects) {
                        SigmaBindingCells::incoming_segments(source.own.incoming_sigma.len())?
                    } else {
                        vec![]
                    };
                    Some(cells.active(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &source.own.incoming_sigma,
                        plan.object_specs()[5].capacity as usize,
                        &segments,
                    )?)
                } else {
                    None
                };
                let transport = if needs_transport {
                    Some(transport_plan.decode_active(
                        &mut chip,
                        &mut region,
                        omega_raw.as_ref().ok_or(Error::Synthesis)?,
                        next_public.omega_key_digest(),
                    )?)
                } else {
                    None
                };
                if tasks.contains(&OperationTask::ReceiveObjects) {
                    incoming_sigma = SigmaBindingCells::from_incoming_active(
                        &mut chip,
                        &mut region,
                        &incoming_statement,
                        incoming_index,
                        sigma_raw.as_ref().ok_or(Error::Synthesis)?,
                        source.own.incoming_sigma.len(),
                    )?;
                }
                let proof_digest = if needs_digest {
                    Some(ReceiveProofDigest::from_active(
                        &mut chip,
                        &mut region,
                        omega_raw.as_ref().ok_or(Error::Synthesis)?,
                        sigma_raw.as_ref().ok_or(Error::Synthesis)?,
                        context[5].authenticated_digest(),
                    )?)
                } else {
                    None
                };
                let values = source.objects.each_ref().map(|raw| cells.bytes(raw));
                let objects = if tasks.contains(&OperationTask::ReceiveObjects) {
                    Some(ReceiveObjects::decode(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        OwnPolicy::new([1, 2], [31, 32], bootstrap_objects::key(23))?,
                        ReceiveObjectSources {
                            request: &values[0],
                            payer: &values[1],
                            receipt: &values[2],
                            payment: &values[3],
                        },
                        ReceiveObjectInputs {
                            own: &statement,
                            receiver: &pred_public,
                            incoming: transport.as_ref().ok_or(Error::Synthesis)?,
                            sigma: &incoming_sigma,
                            consuming_digest: context[4].authenticated_digest(),
                        },
                    )?)
                } else {
                    None
                };
                let signed = if tasks.iter().any(|t| {
                    matches!(
                        t,
                        OperationTask::ReceiveSignatures | OperationTask::ReceiveBlacklist
                    )
                }) {
                    Some(ReceiveSignedObjects::decode(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        [&values[0], &values[1], &values[2]],
                    )?)
                } else {
                    None
                };
                let auth = if tasks.iter().any(|t| {
                    matches!(
                        t,
                        OperationTask::ReceiveAuthorization
                            | OperationTask::ReceiveOwnProof
                            | OperationTask::ReceiveSignatures
                    )
                }) {
                    Some(ReceiveAuthorizationObjects::decode(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        Variant::Receive,
                        ReceiveAuthorizationSources {
                            current: &values[6],
                            certificate: &values[7],
                            receipt: &values[8],
                            quoted: [&values[9], &values[10]],
                        },
                    )?)
                } else {
                    None
                };
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: Some(&incoming_statement),
                    predecessor: Some(ContextPredecessor {
                        state: &old,
                        public: &pred_public,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &new,
                        public: &next_public,
                    },
                    incoming: Some(ContextIncoming {
                        public: &incoming_public,
                        pallas: &ip,
                        vesta: &iv,
                        proof: ContextIncomingProof::ReceiveActive,
                    }),
                    q_instances: &q_instances,
                    objects: &context,
                    modes: &modes,
                    pallas_corrections: &corrections,
                    vesta_corrections: &vcorrections,
                    receive_results: Some(&results),
                };
                let sigma = [own_sigma.clone(), incoming_sigma.clone()];
                let pred = if stage == 0 {
                    let key = cells.key(&mut chip, &mut region, &source.predecessor.key)?;
                    let proof = cells.proof(&mut chip, &mut region, &source.predecessor.proof)?;
                    Some(verify_predecessor(
                        &mut chip,
                        &mut region,
                        plan.operation(),
                        &key,
                        &pred_public,
                        &next_public,
                        &pp,
                        &pv,
                        &proof,
                    )?)
                } else {
                    None
                };
                let mut verified = Vec::new();
                let mut own_signature = None;
                let mut incoming_signature = None;
                for index in plan.q_partition(stage).ok_or(Error::Synthesis)? {
                    let proof = cells.proof(&mut chip, &mut region, &source.q[*index].proof)?;
                    let q = verify_q(
                        &mut chip,
                        &mut region,
                        plan.operation(),
                        *index,
                        &q_instances[*index],
                        &proof,
                    )?;
                    if *index > 0 {
                        let slots = iroha_kagemusha_proof::a_relation::bind_signature_q(
                            &mut chip,
                            &mut region,
                            plan.operation(),
                            *index,
                            source
                                .plan
                                .signature_schema(*index)
                                .ok_or(Error::Synthesis)?,
                            &q,
                        )?;
                        if *index == 1 {
                            own_signature = Some(slots)
                        } else {
                            incoming_signature = Some(slots)
                        }
                    }
                    verified.push(q);
                }
                let proof_sources = transport
                    .as_ref()
                    .map(|transport| {
                        ReceiveProofSources::from_active_omega(transport, &incoming_sigma)
                    })
                    .transpose()?;
                let proof_key = if tasks.contains(&OperationTask::ReceiveProofs) {
                    Some(cells.key(&mut chip, &mut region, &source.incoming_head.key)?)
                } else {
                    None
                };
                let nonmembership = if tasks.contains(&OperationTask::ReceiveNonmembership) {
                    Some(
                        cells
                            .insertion(&mut chip.uint(), &mut region, &source.own.witness.consumed)?
                            .low,
                    )
                } else {
                    None
                };
                let blacklist = if tasks.contains(&OperationTask::ReceiveBlacklist) {
                    Some(
                        cells
                            .insertion(&mut chip.uint(), &mut region, &source.own.witness.consumed)?
                            .low,
                    )
                } else {
                    None
                };
                let effects = if tasks.contains(&OperationTask::ReceiveEffects) {
                    let consumed = cells.insertion(
                        &mut chip.uint(),
                        &mut region,
                        &source.own.witness.consumed,
                    )?;
                    let credit = cells.insertion(
                        &mut chip.uint(),
                        &mut region,
                        &source.own.witness.credit,
                    )?;
                    let inserted_key = statement.fields()[17].clone();
                    let inserted_value = chip.hash_words(
                        &mut region,
                        iroha_kagemusha_proof::operation_relation::map_effects::CONSUMED_DOMAIN,
                        &[
                            inserted_key.clone(),
                            statement.fields()[20].clone(),
                            statement.fields()[9].clone(),
                        ],
                    )?;
                    let insert = chip
                        .uint()
                        .glue()
                        .boolean(&mut region, cells.value(source.own.witness.insert))?;
                    Some(ReceiveMapWitness {
                        consumed,
                        credit,
                        inserted_key,
                        inserted_value,
                        insert,
                        payment_digest: context[3].authenticated_digest().clone(),
                    })
                } else {
                    None
                };
                source.plan.constrain_stage(
                    &mut chip,
                    &mut region,
                    u32::try_from(stage).map_err(|_| Error::BoundsFailure)?,
                    ReceiveStageInputs {
                        context: &input,
                        proof_digest: proof_digest.as_ref(),
                        objects: objects.as_ref(),
                        signed: signed.as_ref(),
                        authorization: auth.as_ref(),
                        own_sigma: tasks
                            .contains(&OperationTask::ReceiveOwnProof)
                            .then_some(&own_sigma),
                    },
                    ReceiveStageWitness {
                        proofs: proof_key.as_ref().map(|key| ReceiveProofInputs {
                            sources: proof_sources.as_ref().unwrap(),
                            omega_key: key,
                            own_sigma: &own_sigma,
                        }),
                        own_signatures: own_signature.as_ref(),
                        incoming_signatures: incoming_signature.as_ref(),
                        nonmembership: nonmembership.as_ref(),
                        blacklist: blacklist.as_ref(),
                        effects: effects.as_ref(),
                    },
                )?;
                let fold = cells.proof(&mut chip, &mut region, &self.fold)?;
                if let Some(continuation) = &self.continuation {
                    let pallas =
                        cells.pallas(&mut chip, &mut region, &continuation.carried.as_input())?;
                    let vesta = cells.vesta(&mut chip, &mut region, &continuation.vesta)?;
                    let mut retained_history = continuation.history.clone();
                    match self.mutation {
                        Some(ContinuationMutation::DropHistory) => {
                            retained_history.pop();
                        }
                        Some(ContinuationMutation::ReverseHistory) => retained_history.reverse(),
                        _ => {}
                    }
                    let history = retained_history
                        .iter()
                        .map(|(p, v)| {
                            Ok(ContextLinkCells {
                                pallas: cells.pallas(&mut chip, &mut region, &p.as_input())?,
                                vesta: cells.vesta(&mut chip, &mut region, v)?,
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?;
                    let proof = cells.proof(&mut chip, &mut region, &continuation.proof)?;
                    let resumed = resume_context(
                        &mut chip,
                        &mut region,
                        &continuation.plan,
                        &input,
                        &history,
                        &pallas,
                        &vesta,
                        &proof,
                        &sigma,
                    )?;
                    let selected = if continuation.plan.is_terminal() {
                        Some(resumed.select_receive_incoming(
                            &mut chip,
                            &mut region,
                            &continuation.plan,
                        )?)
                    } else {
                        None
                    };
                    let closed = close_stage(
                        &mut chip,
                        &mut region,
                        &continuation.plan,
                        &resumed,
                        None,
                        selected.as_ref().map(|v| &v.0),
                        selected.as_ref().map(|v| &v.1),
                        &verified,
                        &fold,
                    )?;
                    if continuation.plan.is_terminal() {
                        closed.words(&mut chip, &mut region, &next_public)
                    } else {
                        closed.continuation()?.words(&mut chip, &mut region)
                    }
                } else {
                    close_first(
                        &mut chip,
                        &mut region,
                        plan,
                        &input,
                        pred.as_ref(),
                        &verified,
                        &sigma,
                        Some(&fold),
                        &source.params,
                    )?
                    .words(&mut chip, &mut region)
                }
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}

fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) {
    let (x, y) = claim.g().coordinates().unwrap();
    words.extend([Fp::from(u64::from(claim.source_k())), x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
}
fn vesta_words(claim: &FoldInput<Eq>) -> Vec<Fp> {
    let (x, y) = claim.g().coordinates().unwrap();
    let mut out = Vec::new();
    for v in [x, y] {
        out.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    out.extend(claim.challenges());
    out
}
fn context_digest(source: &Source, first: &AccumulatorT<Ep>) -> Fp {
    let mut words = source.plan.context().schema().to_vec();
    words.extend(source.own.witness.statement);
    words.extend(source.own.send.statement);
    let before = &source.own.witness.before;
    let after = &source.own.witness.after;
    words.extend(before.core);
    words.extend(before.rest);
    words.extend(before.lineage);
    push_pallas(&mut words, &source.predecessor.pallas.as_input());
    words.extend(vesta_words(&source.predecessor.vesta.as_input()));
    words.extend(after.core);
    words.extend(after.rest);
    words.extend(after.lineage);
    words.extend(source.incoming_head.state.lineage);
    words.push(Fp::ONE);
    push_pallas(&mut words, &source.incoming_head.pallas.as_input());
    words.extend(vesta_words(&source.incoming_head.vesta.as_input()));
    for q in &source.q {
        for (column, ty) in q.instances.iter().zip(
            q.plan
                .verifier()
                .binding()
                .descriptor()
                .instance_types
                .as_ref()
                .unwrap(),
        ) {
            for value in column {
                if matches!(*ty, InstanceType::Bounded | InstanceType::Bits(0..=253)) {
                    words.push(Fp::from_repr(value.to_repr()).unwrap());
                } else {
                    words.extend(foreign_limbs(value).map(Fp::from_u128));
                }
            }
        }
    }
    words.extend(source.commitments.iter().flatten().copied());
    words.extend([
        Fp::ONE,
        Fp::from(u64::from(source.accept)),
        Fp::ONE,
        Fp::ONE,
        Fp::ONE,
    ]);
    push_pallas(&mut words, &source.incoming_head.opening);
    for _ in 0..4 {
        words.extend(source.mode_words());
    }
    let trivial = AccumulatorT::trivial(&source.params, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = trivial.g().coordinates().unwrap();
    words.extend([x, y, x, y]);
    let trivial = AccumulatorT::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
    words.extend(&vesta_words(&trivial.as_input())[..4]);
    push_pallas(&mut words, &first.as_input());
    hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words)
}
fn continued_digest(
    plan: &ContextPlan,
    stage: usize,
    previous: Fp,
    old: &AccumulatorT<Ep>,
    vesta: &AccumulatorT<Eq>,
    current: &AccumulatorT<Ep>,
) -> Fp {
    let mut words = vec![
        Fp::ONE,
        plan.schema()[1],
        Fp::from(u64::try_from(stage + 1).unwrap()),
        previous,
    ];
    push_pallas(&mut words, &old.as_input());
    words.extend(vesta_words(&vesta.as_input()));
    push_pallas(&mut words, &current.as_input());
    hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words)
}
fn internal_public(digest: Fp, part: &FoldInput<Eq>) -> Vec<Vec<Fp>> {
    let mut words = vec![digest, Fp::from(u64::from(part.source_k()))];
    words.extend(vesta_words(part));
    let trivial = AccumulatorT::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
    let trivial = vesta_words(&trivial.as_input());
    words.extend(&trivial);
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    assert_eq!(words.len(), 69);
    vec![words]
}
impl Stage {
    fn public(&self) -> Vec<Vec<Fp>> {
        let root = context_digest(&self.source, &self.first);
        let Some(continuation) = &self.continuation else {
            return internal_public(root, &self.source.own.part);
        };
        if !continuation.plan.is_terminal() {
            let mut digest = root;
            for (i, (p, v)) in continuation.history.iter().enumerate() {
                let next = continuation
                    .history
                    .get(i + 1)
                    .map_or(&continuation.carried, |(p, _)| p);
                digest = continued_digest(self.source.plan.context(), i + 1, digest, p, v, next);
            }
            digest = continued_digest(
                self.source.plan.context(),
                continuation.plan.stage(),
                digest,
                &continuation.carried,
                &continuation.vesta,
                &self.pallas,
            );
            return internal_public(digest, &continuation.vesta.as_input());
        }
        let mut lineage = self.source.own.witness.after.lineage.to_vec();
        let (x, y) = self.pallas.g().coordinates().unwrap();
        lineage.extend([x, y]);
        for u in self.pallas.challenges() {
            lineage.extend(foreign_limbs(u).map(Fp::from_u128));
        }
        let mut words = vec![
            hash_with_domain(u64::from_le_bytes(*b"kgwomg_1"), &lineage),
            Fp::from(16),
        ];
        words.extend(vesta_words(&continuation.vesta.as_input()));
        words.extend(vesta_words(&self.source.predecessor.vesta.as_input()));
        // A exports the ORIGINAL incoming V and its mode; Omega performs the
        // mode selection and includes the chosen deciding claim exactly once.
        words.extend(vesta_words(&self.source.incoming_head.vesta.as_input()));
        words.extend(self.source.mode_words());
        let trivial =
            AccumulatorT::trivial(&common::vesta_params(16), MemoryBudget::DEFAULT).unwrap();
        words.extend(&vesta_words(&trivial.as_input())[..4]);
        assert_eq!(words.len(), 69);
        vec![words]
    }
}
fn prove_stage(circuit: &Stage) -> (ProvingKey<Eq>, ProverOutput<Eq>, Vec<Vec<Fp>>) {
    let public = circuit.public();
    let stage = circuit.continuation.as_ref().map_or(0, |c| c.plan.stage());
    let (assigned, k) = match synthesize(circuit, 16, Some(&public)) {
        Ok(a) => (a, 16),
        Err(error) => {
            eprintln!(
                "Receive A{} k16 capacity failure {error:?}; diagnostic k18 only",
                stage + 1
            );
            (synthesize(circuit, 18, Some(&public)).unwrap(), 18)
        }
    };
    let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
    assert!(
        report.is_satisfied(),
        "stage={} failures={:?}",
        stage + 1,
        &report.failures()[..report.failures().len().min(12)]
    );
    let unknown = synthesize(&circuit.without_witnesses(), k, None).unwrap();
    assert_eq!(assigned.tables.fixed(), unknown.tables.fixed());
    assert_eq!(assigned.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        assigned.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let rows: Vec<_> = assigned
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .collect();
    eprintln!(
        "Receive A{} taggedA{} tasks={:?} Q={:?} lanes={rows:?} k16_fit={}",
        stage + 1,
        circuit.params(),
        circuit.source.plan.context().operation_tasks(stage),
        circuit.source.plan.context().q_partition(stage),
        k == 16
    );
    assert_eq!(k, 16, "Receive stage must fit unchanged k16 gate");
    drop(assigned);
    drop(unknown);
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    let params = common::vesta_params(16);
    let key = keygen_pk_v2(&params, circuit, &config).unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        Witness::from_circuit(&key, circuit, &public).unwrap(),
        common::recovery(200 + u8::try_from(stage).unwrap()),
        ProverConfig::default(),
    )
    .unwrap();
    proof
        .opening
        .decide(&params, MemoryBudget::DEFAULT)
        .unwrap();
    (key, proof, public)
}

fn assert_continuation_rejects(circuit: &Stage, mutation: ContinuationMutation) {
    let hostile = Stage {
        mutation: Some(mutation),
        ..circuit.clone()
    };
    if let Ok(assigned) = synthesize(&hostile, 16, Some(&circuit.public())) {
        let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
        assert!(
            !report.is_satisfied(),
            "authenticated W continuation accepted {mutation:?}"
        );
    }
}

fn assert_foreign_wrapper_rejects(circuit: &Stage, previous_wrapper: &[u8]) {
    let mut hostile = circuit.clone();
    assert_eq!(
        hostile.continuation.as_ref().unwrap().proof.len(),
        previous_wrapper.len(),
        "foreign W mutation must preserve the admitted message schedule"
    );
    hostile.continuation.as_mut().unwrap().proof = previous_wrapper.to_vec();
    if let Ok(assigned) = synthesize(&hostile, 16, Some(&circuit.public())) {
        let report = check(&assigned.cs, &assigned.tables, CheckMode::Strict).unwrap();
        assert!(
            !report.is_satisfied(),
            "continuation accepted a real W proof under another preceding-stage key"
        );
    }
}
#[test]
#[ignore = "real receiver Bootstrap, both sigmas, three Qs and nine fixed A/W stages; run optimized"]
fn genuine_receive_burn_owner_chain_preserves_all_proofs_and_result_claims() {
    receive_owner_chain(IngestionCapacity::Descriptor);
}

#[test]
#[ignore = "real compact predecessor and every Receive owner at fixed maximum external capacities; run optimized"]
fn canonical_envelope_receive_burn_owner_chain() {
    receive_owner_chain(IngestionCapacity::CanonicalEnvelope);
}

#[test]
#[ignore = "genuine payer Load and receiver Bootstrap, real Send/Receive sigmas, all fixed Receive owners at maximum capacity"]
fn canonical_envelope_receive_accepts_exact_payer_and_receiver_heads() {
    let wallets = compact_catalog::compact_payer_load_and_receiver();
    let receiver = Head {
        state: StateWitness::from(&wallets.receiver.source.state),
        key: wallets.receiver.key,
        binding: wallets.receiver.binding,
        proof: wallets.receiver.proof,
        pallas: wallets.receiver.source.pallas,
        vesta: wallets.receiver.vesta,
        opening: wallets.receiver.opening,
    };
    let payer = Head {
        state: wallets.payer.source.state,
        key: wallets.payer.key,
        binding: wallets.payer.binding,
        proof: wallets.payer.proof,
        pallas: wallets.payer.source.pallas,
        vesta: wallets.payer.vesta,
        opening: wallets.payer.opening,
    };
    assert_ne!(receiver.state.core[5..7], payer.state.core[5..7]);
    let source = source(receiver, Some(payer), IngestionCapacity::CanonicalEnvelope);
    assert_eq!(source.own.send.before.core, source.incoming_head.state.core);
    assert_eq!(source.own.send.before.rest, source.incoming_head.state.rest);
    assert_eq!(
        source.own.send.before.lineage,
        source.incoming_head.state.lineage
    );
    assert_eq!(
        source.own.witness.after.core[iroha_kagemusha_proof::witness::core_index::BALANCE]
            - source.own.witness.before.core[iroha_kagemusha_proof::witness::core_index::BALANCE],
        source.own.witness.statement[20],
    );
    assert_eq!(
        source.own.witness.after.lineage[14], source.own.witness.before.lineage[14],
        "accepted value never enters the burn accumulator",
    );
    prove_receive_owner_chain(source);
}

fn receive_owner_chain(capacity: IngestionCapacity) {
    let receiver = bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap_with_identity(
        bootstrap_outer::bootstrap_chain::BootstrapIdentity::Receiver,
    );
    prove_receive_owner_chain(source(receiver.into(), None, capacity));
}

fn prove_receive_owner_chain(source: Source) {
    let source = Arc::new(source);
    let (first_fold, pallas) = create_fold(
        &source.params,
        &[
            source.predecessor.pallas.as_input(),
            source.predecessor.opening.clone(),
        ],
        Fp::from(247).to_repr(),
        &FoldConfig::default(),
    )
    .unwrap();
    let first = Stage {
        source: source.clone(),
        continuation: None,
        pallas: pallas.clone(),
        first: pallas.clone(),
        fold: first_fold.to_bytes().to_vec(),
        known: true,
        mutation: None,
    };
    let (mut key, mut proof, mut public) = prove_stage(&first);
    let mut carried = pallas;
    let mut part = source.own.part.clone();
    let mut history = Vec::new();
    let mut previous_wrapper: Option<Vec<u8>> = None;
    let vparams = common::vesta_params(16);
    for stage in 1..source.plan.context().stage_count() {
        let own = FoldInput::from_opening(*proof.opening.g(), proof.opening.challenges()).unwrap();
        let trivial = AccumulatorT::trivial(&vparams, MemoryBudget::DEFAULT)
            .unwrap()
            .as_input();
        let (vfold, vesta) = create_fold(
            &vparams,
            &[part, own, trivial.clone(), trivial],
            Fq::from(251 + u64::try_from(stage).unwrap()).to_repr(),
            &FoldConfig::default(),
        )
        .unwrap();
        vesta.decide(&vparams, MemoryBudget::DEFAULT).unwrap();
        let witness = iroha_kagemusha_proof::omega::OmegaWitness {
            key: key.vk().clone(),
            instances: public[0].clone(),
            length: proof.proof.len().try_into().unwrap(),
            proof: proof.proof,
            fold: vfold.to_bytes(),
        };
        let wrapper = WCircuit::new(
            source.plan.context(),
            stage - 1,
            key.binding().clone(),
            vparams.clone(),
            vec![key.vk().kagemusha_digest(key.binding()).unwrap()],
            witness,
        )
        .unwrap();
        let (wkey, wprover) = WKey::keygen(&wrapper, &source.params).unwrap();
        let (x, y) = vesta.g().coordinates().unwrap();
        let wpublic = vec![
            vec![Fq::from_repr(public[0][0].to_repr()).unwrap()],
            vec![x, y],
            vesta
                .challenges()
                .iter()
                .map(|v| Fq::from_repr(v.to_repr()).unwrap())
                .collect(),
        ];
        let wproof = create_proof_owned_with_claim(
            &source.params,
            &wprover,
            Witness::from_circuit(&wprover, &wrapper, &wpublic).unwrap(),
            common::recovery(210 + u8::try_from(stage).unwrap()),
            ProverConfig::default(),
        )
        .unwrap();
        wproof
            .opening
            .decide(&source.params, MemoryBudget::DEFAULT)
            .unwrap();
        let mut claims = vec![
            carried.as_input(),
            FoldInput::from_opening(*wproof.opening.g(), wproof.opening.challenges()).unwrap(),
        ];
        let split =
            SplitPlan::new(source.plan.context().clone(), stage, wkey, &source.params).unwrap();
        if split.is_terminal() {
            if source.accept {
                claims.extend([
                    source.incoming_head.pallas.as_input(),
                    source.incoming_head.opening.clone(),
                ]);
            } else {
                let trivial = AccumulatorT::trivial(&source.params, MemoryBudget::DEFAULT).unwrap();
                claims.extend([trivial.as_input(), trivial.as_input()]);
            }
        }
        claims.extend(
            source
                .plan
                .context()
                .q_partition(stage)
                .unwrap()
                .iter()
                .map(|i| source.q[*i].opening.clone()),
        );
        let (fold, pallas) = create_fold(
            &source.params,
            &claims,
            Fp::from(261 + u64::try_from(stage).unwrap()).to_repr(),
            &FoldConfig::default(),
        )
        .unwrap();
        let continuation = Stage {
            source: source.clone(),
            continuation: Some(Continuation {
                plan: split,
                proof: wproof.proof,
                carried: carried.clone(),
                vesta: vesta.clone(),
                history: history.clone(),
            }),
            pallas: pallas.clone(),
            first: first.first.clone(),
            fold: fold.to_bytes().to_vec(),
            known: true,
            mutation: None,
        };
        (key, proof, public) = prove_stage(&continuation);
        if let Some(previous) = &previous_wrapper {
            assert_foreign_wrapper_rejects(&continuation, previous);
        }
        previous_wrapper = Some(continuation.continuation.as_ref().unwrap().proof.clone());
        if stage == 1 {
            // These values are proposals in this Q-only stage, so only the
            // genuine preceding W/context binding can authenticate them here.
            for slot in 0..11 {
                for word in 0..3 {
                    assert_continuation_rejects(
                        &continuation,
                        ContinuationMutation::Object { slot, word },
                    );
                }
            }
            for index in 0..5 {
                assert_continuation_rejects(&continuation, ContinuationMutation::Result(index));
            }
            for mutation in [
                ContinuationMutation::SwapObjects,
                ContinuationMutation::Opening,
                ContinuationMutation::Mode,
                ContinuationMutation::IncomingStatement,
                ContinuationMutation::QChunk,
            ] {
                assert_continuation_rejects(&continuation, mutation);
            }
            eprintln!(
                "Receive genuine W rejects all33 object words, all5 results, source reorder, opening, mode and statement substitution"
            );
        }
        if stage == 3 {
            assert_continuation_rejects(&continuation, ContinuationMutation::DropHistory);
            assert_continuation_rejects(&continuation, ContinuationMutation::ReverseHistory);
        }
        if stage + 1 < source.plan.context().stage_count() {
            history.push((carried, vesta.clone()));
        }
        carried = pallas;
        part = vesta.as_input();
    }
    carried
        .decide(&source.params, MemoryBudget::DEFAULT)
        .unwrap();
    eprintln!(
        "RECEIVE_OWNER_CHAIN stages={} accepted_credit={} original_sources_bound=true final_catalog=false release_qualified=false",
        source.plan.context().stage_count(),
        source.accept,
    );
}
