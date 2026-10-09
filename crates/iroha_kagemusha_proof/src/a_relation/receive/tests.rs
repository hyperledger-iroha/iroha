//! Active incoming object composition; no recursive proof is accepted here.

use super::*;
use crate::{
    a_relation::incoming_transport::{IncomingTransportPlan, tests::decoder_fixture},
    operation_relation::incoming_statement::IncomingStatementCells,
};
use ff::{Field, PrimeField};
use iroha_pasta::poseidon::hash_with_domain;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, synthesize},
};
use iroha_plonk_gadgets::{
    bytes::{p_bytes_native, tape::BytesConfig, variable::ActiveBytes},
    p256::native::Affine,
};
use iroha_plonk_recursion::verifier::VerifierConfig;

mod signatures;

pub(super) fn recursive_program_fixture() -> (crate::a_relation::AProofPlan, OwnPolicy) {
    let fixture = Objects::fixture().with_context();
    (fixture.context.unwrap().operation().clone(), fixture.policy)
}

#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
#[derive(Clone)]
struct Objects {
    plan: IncomingTransportPlan,
    omega: Vec<u8>,
    sigma: Vec<u8>,
    source: [Vec<u8>; 4],
    own: [Fp; 26],
    incoming: [Fp; 26],
    receiver: [Fp; 18],
    policy: OwnPolicy,
    index: u64,
    valid: bool,
    known: bool,
    context: Option<ContextPlan>,
    mutation: BindMutation,
}
#[derive(Clone, Copy)]
enum BindMutation {
    None,
    Result,
    Owner,
    QDigest,
    QIndex,
    QBytes,
    Own,
    Receiver,
    Header,
    ActiveLength,
    Commitment { slot: usize, word: usize },
    ProofProjectionOnly,
    WrongProofProjection,
}
impl Circuit<Fp> for Objects {
    type Config = Config;
    type Params = ();
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure_serialized_foreign(meta, 4).unwrap();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(2);
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
        let result = layouter.assign_region(
            || "active Receive objects",
            |mut region| {
                let value = |v| {
                    if self.known {
                        Value::known(v)
                    } else {
                        Value::unknown()
                    }
                };
                let raw = if self.known {
                    Value::known(self.omega.clone())
                } else {
                    Value::unknown()
                };
                let omega = ActiveBytes::assign(
                    &mut chip.uint(),
                    &mut bytes,
                    &mut region,
                    MAX_OMEGA_RAW_BYTES,
                    &raw,
                    &self.plan.active_segments()?,
                )?;
                let key = chip.uint().glue().constant(&mut region, Fp::from(41))?;
                let incoming = self
                    .plan
                    .decode_active(&mut chip, &mut region, &omega, &key)?;
                let lanes = chip.operation_lanes()?;
                let mut uint = UintChip::new(lanes.glue, lanes.range);
                let own = uint
                    .glue()
                    .witnesses(&mut region, &self.own.map(value))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let own = StatementCells::constrain(
                    &mut uint,
                    lanes.hash,
                    &mut region,
                    Variant::Receive,
                    &own,
                )?;
                let statement = uint
                    .glue()
                    .witnesses(&mut region, &self.incoming.map(value))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = IncomingStatementCells::constrain(
                    &mut uint,
                    lanes.hash,
                    &mut region,
                    Variant::Send,
                    &statement,
                )?;
                let receiver = uint
                    .glue()
                    .witnesses(&mut region, &self.receiver.map(value))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let receiver = LineagePublicCells::constrain(&mut uint, &mut region, &receiver)?;
                let raw = if self.known {
                    Value::known(self.sigma.clone())
                } else {
                    Value::unknown()
                };
                let sigma = ActiveBytes::assign(
                    &mut chip.uint(),
                    &mut bytes,
                    &mut region,
                    MAX_SIGMA_RAW_BYTES,
                    &raw,
                    &SigmaBindingCells::incoming_segments(self.sigma_view())?,
                )?;
                let index = chip
                    .uint()
                    .glue()
                    .witness(&mut region, value(Fp::from(self.index)))?;
                let sigma = SigmaBindingCells::from_incoming_active(
                    &mut chip,
                    &mut region,
                    &statement,
                    index,
                    &sigma,
                    self.sigma_view(),
                )?;
                let source = self.source.each_ref().map(|raw| {
                    raw.iter()
                        .map(|b| {
                            if self.known {
                                Value::known(*b)
                            } else {
                                Value::unknown()
                            }
                        })
                        .collect::<Vec<_>>()
                });
                let objects = ReceiveObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    self.policy,
                    ReceiveObjectSources {
                        request: &source[0],
                        payer: &source[1],
                        receipt: &source[2],
                        payment: &source[3],
                    },
                    ReceiveObjectInputs {
                        own: &own,
                        receiver: &receiver,
                        incoming: &incoming,
                        sigma: &sigma,
                    },
                )?;
                assert_eq!(objects.context().len(), 6);
                assert_eq!(objects.objects()[0].kind(), ObjectKind::Request);
                if let Some(plan) = &self.context {
                    self.bind_context(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        plan,
                        &objects,
                        &own,
                        &receiver,
                        &incoming,
                        &sigma,
                    )?;
                }
                Ok([
                    objects.payment().payment().digest().clone(),
                    objects.valid.word().clone(),
                ])
            },
        )?;
        for (i, word) in result.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
pub(super) fn decode(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(core::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn fp(hex: &str) -> Fp {
    Fp::from_repr(decode(hex).try_into().unwrap())
        .into_option()
        .unwrap()
}
fn half(bytes: &[u8]) -> Fp {
    Fp::from_u128(u128::from_le_bytes(bytes.try_into().unwrap()))
}
pub(super) fn object_digest(kind: ObjectKind, source: &[u8]) -> Fp {
    let body = kind.body_len();
    let mut words = vec![p_bytes_native(kind.signing_domain(), &source[..body])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            source[body + offset..body + offset + 16]
                .try_into()
                .unwrap(),
        )));
    }
    hash_with_domain(kind.object_domain(), &words)
}
impl Objects {
    #[allow(
        clippy::too_many_arguments,
        reason = "component fixture preserves each exact context source"
    )]
    fn bind_context(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        plan: &ContextPlan,
        objects: &ReceiveObjects,
        own: &StatementCells,
        receiver: &LineagePublicCells,
        incoming: &IncomingTransportCells,
        sigma: &SigmaBindingCells,
    ) -> Result<(), Error> {
        use crate::a_relation::{
            IncomingLineageCells,
            context::{ContextIncoming, ContextPredecessor, ContextState},
            results::{ReceiveResultClaims, ReceiveResultTag},
        };
        use crate::operation_relation::state::StateCells;
        use iroha_plonk_recursion::codec::ScalarCells;
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let mut words = plan
            .operation()
            .sigma
            .instance_lengths()
            .map(|n| vec![zero.clone(); n]);
        words[0][0] = own.digest().clone();
        words[0][1] = sigma.statement().digest().clone();
        words[2][1] = sigma.key_index().clone();
        for (i, chunk) in plan
            .operation()
            .sigma
            .chunk_range(1)
            .ok_or(Error::Synthesis)?
            .zip(sigma.proof_chunks())
        {
            words[0][i] = chunk.clone();
        }
        let mutation_slot = match self.mutation {
            BindMutation::QDigest => Some((0, 1)),
            BindMutation::QIndex => Some((2, 1)),
            BindMutation::QBytes => Some((
                0,
                plan.operation()
                    .sigma
                    .chunk_range(1)
                    .ok_or(Error::Synthesis)?
                    .start,
            )),
            _ => None,
        };
        if let Some((col, row)) = mutation_slot {
            words[col][row] = chip
                .uint()
                .glue()
                .add_constant(region, &words[col][row], Fp::ONE)?;
        }
        let instances = words
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|word| ScalarCells::from_native_word(&mut chip.uint(), region, word))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instances = [instances];
        let j: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        let fields = |name: &str| {
            j["field_encodings"]["controlled_state"][name]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| {
                    if self.known {
                        Value::known(fp(s.as_str().unwrap()))
                    } else {
                        Value::unknown()
                    }
                })
                .collect::<Vec<_>>()
        };
        let core = chip
            .uint()
            .glue()
            .witnesses(region, &fields("core_items"))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let rest = chip
            .uint()
            .glue()
            .witnesses(region, &fields("rest_items"))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let own = if matches!(self.mutation, BindMutation::Own) {
            let mut fields = own.fields().clone();
            fields[17] = chip
                .uint()
                .glue()
                .add_constant(region, &fields[17], Fp::ONE)?;
            let lanes = chip.operation_lanes()?;
            StatementCells::constrain(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                Variant::Receive,
                &fields,
            )?
        } else {
            own.clone()
        };
        let receiver = if matches!(self.mutation, BindMutation::Receiver) {
            let mut fields = receiver.fields().clone();
            fields[5] = chip
                .uint()
                .glue()
                .add_constant(region, &fields[5], Fp::ONE)?;
            LineagePublicCells::constrain(&mut chip.uint(), region, &fields)?
        } else {
            receiver.clone()
        };
        let public = if matches!(self.mutation, BindMutation::Header) {
            let mut fields = incoming.public().fields().clone();
            fields[5] = chip
                .uint()
                .glue()
                .add_constant(region, &fields[5], Fp::ONE)?;
            IncomingLineageCells::constrain(
                &mut chip.uint(),
                region,
                &fields,
                incoming.public().valid(),
            )?
        } else {
            incoming.public().clone()
        };
        let signed = |name: &str| {
            let row = j["signatures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["object"].as_str() == Some(name))
                .unwrap();
            let mut raw = decode(row["transcript_hex"].as_str().unwrap());
            raw.extend(decode(row["signature_hex"].as_str().unwrap()));
            raw.into_iter()
                .map(|v| {
                    if self.known {
                        Value::known(v)
                    } else {
                        Value::unknown()
                    }
                })
                .collect::<Vec<_>>()
        };
        let credential = signed("payer credential");
        let certificate = signed("payer issuer certificate");
        let receipt = signed("Receive receipt binding the Payment digest");
        let auth = authorization::ReceiveAuthorizationObjects::decode(
            chip,
            bytes,
            region,
            Variant::Receive,
            authorization::ReceiveAuthorizationSources {
                current: &credential,
                certificate: &certificate,
                receipt: &receipt,
                quoted: [&credential, &certificate],
            },
        )?;
        let mut context = objects.context().to_vec();
        context.extend_from_slice(auth.context());
        if matches!(self.mutation, BindMutation::ActiveLength) {
            let mut raw = self.omega.clone();
            raw.push(0);
            let raw = if self.known {
                Value::known(raw)
            } else {
                Value::unknown()
            };
            let active = ActiveBytes::assign(
                &mut chip.uint(),
                bytes,
                region,
                MAX_OMEGA_RAW_BYTES,
                &raw,
                &[],
            )?;
            context[4] = ContextObjectCells::from_active(
                chip,
                region,
                objects.specs[4],
                context[4].authenticated_digest(),
                &active,
            )?;
        }
        let mut proposed = context
            .iter()
            .map(|object| object.commitment_words().map(|word| word.value()))
            .collect::<Vec<_>>();
        let mutated = match self.mutation {
            BindMutation::Commitment { slot, word } => Some((slot, word)),
            BindMutation::ProofProjectionOnly => Some((4, 0)),
            _ => None,
        };
        if let Some((slot, word)) = mutated {
            proposed[slot][word] = proposed[slot][word].map(|v| v + Fp::ONE);
        }
        let context = plan.assign_receive_object_claims(chip, region, &proposed)?;
        let plan_result = plan.receive_results().ok_or(Error::Synthesis)?;
        let mut values = [Value::known(true); 5];
        values[ReceiveResultTag::Objects as usize - 1] = if self.known {
            Value::known(self.valid ^ matches!(self.mutation, BindMutation::Result))
        } else {
            Value::unknown()
        };
        let claims = ReceiveResultClaims::assign(chip.uint().glue(), region, plan_result, values)?;
        let input = ContextInputs {
            own_statement: &own,
            incoming_statement: Some(sigma.incoming_statement()?),
            predecessor: Some(ContextPredecessor {
                state: &state,
                public: &receiver,
                pallas: incoming.pallas(),
                vesta: incoming.vesta(),
            }),
            successor: ContextState {
                state: &state,
                public: &receiver,
            },
            incoming: Some(ContextIncoming {
                public: &public,
                pallas: incoming.pallas(),
                proof: if matches!(self.mutation, BindMutation::WrongProofProjection) {
                    crate::a_relation::context::ContextIncomingProof::Messages(incoming.proof())
                } else {
                    crate::a_relation::context::ContextIncomingProof::ReceiveActive
                },
                vesta: incoming.vesta(),
            }),
            q_instances: &instances,
            objects: &context,
            modes: &[],
            pallas_corrections: &[],
            vesta_corrections: &[],
            receive_results: Some(&claims),
        };
        // This tests exact proposed commitments and their typed owners. It
        // intentionally does not certify an A/W chain or signature semantics.
        let proof = ReceiveProofSources::from_active(incoming, sigma)?;
        proof.bind_context(chip, region, plan, &input)?;
        if matches!(self.mutation, BindMutation::ProofProjectionOnly) {
            return Ok(());
        }
        objects
            .signed_sources()
            .bind_context(region, plan, &input)?;
        auth.bind_context(region, plan, &input)?;
        objects.bind_result(
            chip,
            region,
            plan,
            u32::from(matches!(self.mutation, BindMutation::Owner)),
            &input,
        )
    }

    fn with_context(mut self) -> Self {
        use crate::a_relation::{
            AProofPlan, QProofPlan, binding::tests::sigma_fixture, schedule::OperationTask,
        };
        use iroha_pasta::Fq;
        use iroha_plonk::{
            keys::{KeygenConfigV2, keygen_pk_v2},
            pcs::ipa::PinnedParams,
        };
        use iroha_plonk_recursion::verifier::VerifierPlan;
        #[derive(Clone)]
        struct Export([usize; 5]);
        impl Circuit<Fq> for Export {
            type Config = (iroha_plonk_gadgets::GlueConfig, [Column<Instance>; 5]);
            type Params = [usize; 5];
            type FloorPlanner = SimpleFloorPlanner;
            fn params(&self) -> Self::Params {
                self.0
            }
            fn without_witnesses(&self) -> Self {
                self.clone()
            }
            fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
                Self::configure_with_params(meta, [1; 5])
            }
            fn configure_with_params(
                meta: &mut ConstraintSystem<Fq>,
                lengths: [usize; 5],
            ) -> Self::Config {
                let columns = core::array::from_fn(|_| meta.advice_column());
                let fixed = meta.fixed_column();
                let glue = iroha_plonk_gadgets::GlueConfig::configure(meta, columns, fixed);
                let public = lengths.map(|n| {
                    let col = meta.instance_column(n);
                    meta.enable_equality(col);
                    col
                });
                (glue, public)
            }
            fn synthesize(
                &self,
                (config, public): Self::Config,
                mut layouter: impl Layouter<Fq>,
            ) -> Result<(), Error> {
                let mut glue = GlueChip::new(config);
                let words = layouter.assign_region(
                    || "export-link key fixture",
                    |mut region| {
                        (0..self.0.iter().sum::<usize>())
                            .map(|_| glue.witness(&mut region, Value::unknown()))
                            .collect::<Result<Vec<_>, _>>()
                    },
                )?;
                let mut offset = 0;
                for (column, len) in public.into_iter().zip(self.0) {
                    for row in 0..len {
                        layouter.constrain_instance(words[offset + row].cell(), column, row)?;
                    }
                    offset += len;
                }
                Ok(())
            }
        }
        let sigma = sigma_fixture();
        let params = PinnedParams::<Ep>::derive(16).unwrap();
        let key = keygen_pk_v2(
            &params,
            &Export(sigma.instance_lengths()),
            &KeygenConfigV2::pipa_r(crate::q_sigma::QSigmaPlan::instance_types().to_vec()),
        )
        .unwrap();
        let verifier = VerifierPlan::new(key.binding().clone(), params.clone()).unwrap();
        let q = QProofPlan::new(verifier, key.vk().clone()).unwrap();
        let operation = AProofPlan::new(
            Variant::Receive,
            sigma,
            vec![q],
            Some(self.plan.verifier().clone()),
            &params,
        )
        .unwrap();
        let context = ContextPlan::with_schedule(
            operation,
            vec![vec![], vec![0]],
            Some(0),
            ReceiveStagePlan::context_specs(
                Variant::Receive,
                MAX_OMEGA_RAW_BYTES,
                MAX_SIGMA_RAW_BYTES,
            )
            .unwrap(),
        )
        .unwrap()
        .with_operation_tasks(vec![
            OperationTask::required(Variant::Receive)
                .unwrap()
                .iter()
                .copied()
                .filter(|task| *task != OperationTask::ReceiveEffects)
                .collect(),
            vec![OperationTask::ReceiveEffects],
        ])
        .unwrap();
        self.context = Some(context);
        self
    }

    fn fixture() -> Self {
        let j: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        let signed = |name| {
            let row = j["signatures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["object"].as_str() == Some(name))
                .unwrap();
            let mut bytes = decode(row["transcript_hex"].as_str().unwrap());
            bytes.extend(decode(row["signature_hex"].as_str().unwrap()));
            bytes
        };
        let fields = |name: &str| {
            j["field_encodings"][name]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| fp(s.as_str().unwrap()))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let source = [
            signed("Request"),
            signed("payer credential"),
            signed("Send receipt"),
            decode(
                j["poseidon"]["large_input_digests"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["domain"].as_str() == Some("kgwpay_1"))
                    .unwrap()["body_hex"]
                    .as_str()
                    .unwrap(),
            ),
        ];
        let incoming: [Fp; 26] = fields("send_statement");
        let (plan, mut omega) = decoder_fixture();
        let mut body = 1u16.to_le_bytes().to_vec();
        for pair in [&incoming[3..5], &incoming[1..3]] {
            for h in pair {
                body.extend_from_slice(&h.to_repr()[..16]);
            }
        }
        body.extend(incoming[14].to_repr());
        body.extend(&source[0][66..98]);
        body.extend(incoming[7].to_repr());
        body.extend(&source[1][130..195]);
        body.push(1);
        body.extend(&source[0][306..314]);
        body.extend(0u32.to_le_bytes());
        body.extend(&incoming[12].to_repr()[..16]);
        body.extend(incoming[13].to_repr());
        body.extend(Fp::from(42).to_repr());
        omega[..320].copy_from_slice(&body);
        let mut receiver = [Fp::ZERO; 18];
        receiver[0] = Fp::ONE;
        receiver[1..3].copy_from_slice(&incoming[3..5]);
        receiver[3..5].copy_from_slice(&incoming[1..3]);
        receiver[6] = half(&source[0][130..146]);
        receiver[7] = half(&source[0][146..162]);
        receiver[13] = Fp::ONE;
        let key = decode(
            j["keys"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"].as_str() == Some("receiver_payment"))
                .unwrap()["public_key_hex"]
                .as_str()
                .unwrap(),
        );
        for (i, index) in [10, 9, 12, 11].into_iter().enumerate() {
            receiver[index] = Fp::from_u128(u128::from_be_bytes(
                key[1 + 16 * i..17 + 16 * i].try_into().unwrap(),
            ));
        }
        let pair = |bytes: &[u8]| {
            [
                u128::from_le_bytes(bytes[..16].try_into().unwrap()),
                u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
            ]
        };
        let policy = OwnPolicy::new(
            pair(&source[0][2..34]),
            pair(&source[2][66..98]),
            Affine::GENERATOR,
        )
        .unwrap();
        let mut out = Self {
            plan,
            omega,
            sigma: vec![19; 32],
            source,
            own: fields("receive_statement"),
            incoming,
            receiver,
            policy,
            index: 2,
            valid: true,
            known: true,
            context: None,
            mutation: BindMutation::None,
        };
        out.rebind();
        out
    }
    fn rebind(&mut self) {
        let framed = |raw: &[u8]| {
            let mut out = u32::try_from(raw.len()).unwrap().to_le_bytes().to_vec();
            out.extend(raw);
            out
        };
        let proof: Fp = p_bytes_native(
            u64::from_le_bytes(*b"kgwprf_1"),
            &[framed(&self.omega), framed(&self.sigma)].concat(),
        );
        self.source[2][242..274].copy_from_slice(&proof.to_repr());
        let package = hash_with_domain(
            u64::from_le_bytes(*b"kgwpkg_1"),
            &[
                hash_with_domain(u64::from_le_bytes(*b"kgwstmt1"), &self.incoming),
                proof,
                object_digest(ObjectKind::Receipt, &self.source[2]),
            ],
        );
        self.source[3][131..163].copy_from_slice(&package.to_repr());
    }
    fn accepts(&self) -> bool {
        check_circuit(
            self,
            16,
            &[vec![
                p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &self.source[3]),
                Fp::from(u64::from(self.valid)),
            ]],
            CheckMode::Strict,
        )
        .is_ok_and(|r| r.is_satisfied())
    }
    fn sigma_view(&self) -> usize {
        self.context.as_ref().map_or(32, |c| {
            c.operation()
                .sigma
                .class(1)
                .unwrap()
                .verifier()
                .proof_length()
        })
    }
    fn rejects(mut self) {
        self.valid = false;
        assert!(self.accepts(), "total false");
        self.valid = true;
        assert!(!self.accepts(), "forged true");
    }
}

#[test]
fn active_receive_object_result_binds_raw_tapes_selector_and_joint_length() {
    let c = Objects::fixture();
    assert!(c.accepts());
    for (index, offset) in [(0, 66), (0, 346), (1, 130), (2, 242), (3, 2), (3, 131)] {
        let mut wrong = c.clone();
        wrong.source[index][offset] ^= 1;
        for valid in [false, true] {
            wrong.valid = valid;
            assert!(
                !wrong.accepts(),
                "content-address substitution {index}/{offset}"
            );
        }
    }
    let mut wrong = c.clone();
    wrong.index = 3;
    wrong.valid = false;
    assert!(
        !wrong.accepts(),
        "wrong selected key cannot manufacture burn"
    );
    for index in [1, 2, 3, 4, 17, 18, 19, 20] {
        let mut wrong = c.clone();
        wrong.own[index] += Fp::ONE;
        for valid in [false, true] {
            wrong.valid = valid;
            assert!(
                !wrong.accepts(),
                "own scope/effect {index} cannot manufacture burn"
            );
        }
    }
    // These fields are the already-authenticated receiver view in this
    // component. A foreign original Request must remain a soft false input.
    let mut foreign_receiver = c.clone();
    foreign_receiver.receiver[6] += Fp::ONE;
    foreign_receiver.rejects();
    let mut foreign_asset = c.clone();
    foreign_asset.source[0][34] ^= 1;
    foreign_asset.rebind();
    // Rebind the outer Request content address while leaving the original
    // Send statement and authenticated own asset fixed.
    let request_digest = object_digest(ObjectKind::Request, &foreign_asset.source[0]);
    foreign_asset.source[3][2..34].copy_from_slice(&request_digest.to_repr());
    let mut request_fields = Vec::new();
    let mut cursor = 0;
    for (index, width) in [
        2, 32, 32, 32, 32, 32, 32, 16, 32, 16, 32, 16, 8, 32, 8, 8, 32, 32, 32,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = &c.source[0][cursor..cursor + width];
        if matches!(index, 1..=6 | 18) {
            request_fields.extend([half(&bytes[..16]), half(&bytes[16..])]);
        } else {
            let mut repr = [0; 32];
            repr[..width].copy_from_slice(bytes);
            request_fields.push(Fp::from_repr(repr).unwrap());
        }
        cursor += width;
    }
    assert_eq!(cursor, ObjectKind::Request.body_len());
    assert_eq!(request_fields.len(), 26);
    assert_eq!(
        hash_with_domain(u64::from_le_bytes(*b"kgwcrdt1"), &request_fields),
        c.own[17]
    );
    request_fields[3] = half(&foreign_asset.source[0][34..50]);
    request_fields[4] = half(&foreign_asset.source[0][50..66]);
    foreign_asset.own[17] = hash_with_domain(u64::from_le_bytes(*b"kgwcrdt1"), &request_fields);
    foreign_asset.rejects();
    let mut wrong_scope = c.clone();
    wrong_scope.receiver[3] += Fp::ONE;
    for valid in [false, true] {
        wrong_scope.valid = valid;
        assert!(
            !wrong_scope.accepts(),
            "own relation is authenticated state, not a soft input"
        );
    }
    for delta in [0, 1] {
        let mut length = c.clone();
        length
            .sigma
            .resize(MAX_OMEGA_RAW_BYTES + delta - length.omega.len(), 0);
        length.rebind();
        if delta == 0 {
            assert!(length.accepts(), "inclusive joint length boundary");
        } else {
            length.rejects();
        }
    }
    let mut short = c.clone();
    short.omega.truncate(319);
    short.rebind();
    short.rejects();
    let known = synthesize(&c, 16, None).unwrap();
    let unknown = synthesize(&c.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    let lanes = known
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |i| i + 1))
        .collect::<Vec<_>>();
    eprintln!("Receive active Objects component lanes={lanes:?}");
}

#[test]
fn objects_result_cannot_change_owner_or_splice_context_inputs() {
    let c = Objects::fixture().with_context();
    assert!(c.accepts());
    for mutation in [
        BindMutation::Result,
        BindMutation::Owner,
        BindMutation::QDigest,
        BindMutation::QIndex,
        BindMutation::QBytes,
        BindMutation::Own,
        BindMutation::Receiver,
        BindMutation::Header,
        BindMutation::ActiveLength,
    ] {
        let wrong = Objects {
            mutation,
            ..c.clone()
        };
        assert!(!wrong.accepts());
    }
    let mut rejected = c.clone();
    rejected.source[3][0] ^= 1;
    rejected.valid = false;
    assert!(rejected.accepts(), "exact bound false result is permitted");
    rejected.mutation = BindMutation::Result;
    assert!(!rejected.accepts(), "false producer cannot claim true");
    let known = synthesize(&c, 16, None).unwrap();
    let unknown = synthesize(&c.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}

#[test]
fn proposed_receive_object_triples_require_every_raw_owner() {
    let c = Objects::fixture().with_context();
    assert!(c.accepts());
    let mut wrong_projection = c.clone();
    wrong_projection.mutation = BindMutation::WrongProofProjection;
    assert!(
        !wrong_projection.accepts(),
        "Receive requires its fixed active source schema"
    );
    for slot in 0..11 {
        for word in 0..3 {
            let mut changed = c.clone();
            changed.mutation = BindMutation::Commitment { slot, word };
            assert!(
                !changed.accepts(),
                "proposed object slot={slot} word={word}"
            );
        }
    }
    let mut isolated = c.clone();
    isolated.mutation = BindMutation::ProofProjectionOnly;
    assert!(
        isolated.accepts(),
        "Proofs alone intentionally does not authenticate the combined digest"
    );
    let mut complete = c;
    complete.mutation = BindMutation::Commitment { slot: 4, word: 0 };
    assert!(
        !complete.accepts(),
        "Objects must reject the same forged combined digest"
    );
}
