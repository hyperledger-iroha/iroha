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
                    8132,
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
                    10000,
                    &raw,
                    &SigmaBindingCells::incoming_segments(32)?,
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
                    32,
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
fn decode(hex: &str) -> Vec<u8> {
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
fn object_digest(kind: ObjectKind, source: &[u8]) -> Fp {
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
        wrong.rejects();
    }
    let mut wrong = c.clone();
    wrong.index = 3;
    wrong.valid = false;
    assert!(
        !wrong.accepts(),
        "wrong selected key cannot manufacture burn"
    );
    let mut wrong = c.clone();
    wrong.own[17] += Fp::ONE;
    wrong.rejects();
    let mut wrong = c.clone();
    wrong.receiver[3] += Fp::ONE;
    wrong.rejects();
    for delta in [0, 1] {
        let mut length = c.clone();
        length.sigma.resize(8597 + delta - length.omega.len(), 0);
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
}
