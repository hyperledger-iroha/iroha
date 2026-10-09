//! Exact endpoint/context linkage using native certificate bytes. These tests
//! do not replace verification of either child proof or authenticate a roster.

use super::*;
use ark_bls12_381::{G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::Zero;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct Link {
    context: CertificateContext,
    aggregation: [Fp; 6],
    signature: [Fp; 6],
    known: bool,
}
impl Link {
    fn endpoints(&mut self) {
        let context = self.context.aggregation.digest();
        self.aggregation = [
            Fp::from(aggregate::PROGRAM_ID),
            context,
            Fp::ZERO,
            Fp::from(u64::from(aggregate::PROGRAM_LENGTH)),
            aggregate::boundary_digest_native(context, false),
            aggregate::boundary_digest_native(context, true),
        ];
        let context = bls::context_digest_native(
            &self.context.message,
            &self.context.aggregation.aggregate_key,
            &self.context.signature,
        );
        self.signature = [
            Fp::from(BlsLeafPlan::PROGRAM_ID),
            context,
            Fp::ZERO,
            Fp::from(u64::from(BlsLeafPlan::LENGTH)),
            bls::boundary_digest_native(context, false),
            bls::boundary_digest_native(context, true),
        ];
    }
    fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        let values = values.map(|word| {
            if self.known {
                Value::known(word)
            } else {
                Value::unknown()
            }
        });
        chip.uint()
            .glue()
            .witnesses(region, &values)?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
    fn bytes<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [u8; N],
    ) -> Result<[Word<Fp>; N], Error> {
        self.words(chip, region, values.map(|v| Fp::from(u64::from(v))))
    }
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl Circuit<Fp> for Link {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).unwrap();
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        Config { verifier, public }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "complete certificate linkage",
            |mut region| {
                let aggregation = self.words(&mut chip, &mut region, self.aggregation)?;
                let signature = self.words(&mut chip, &mut region, self.signature)?;
                let aggregation =
                    SourceEndpoints::from_words(&mut chip, &mut region, &aggregation)?;
                let signature = SourceEndpoints::from_words(&mut chip, &mut region, &signature)?;
                let c = &self.context;
                let [root, members, faults] = self.words(
                    &mut chip,
                    &mut region,
                    [
                        c.aggregation.roster_root,
                        Fp::from(u64::from(c.aggregation.members)),
                        Fp::from(u64::from(c.aggregation.faults)),
                    ],
                )?;
                let bitmap = self.bytes(&mut chip, &mut region, c.aggregation.bitmap)?;
                let key = self.bytes(&mut chip, &mut region, c.aggregation.aggregate_key)?;
                let message = self.bytes(&mut chip, &mut region, c.message)?;
                let sig = self.bytes(&mut chip, &mut region, c.signature)?;
                Ok(CertificateLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &aggregation,
                    &signature,
                    CertificateInputs {
                        roster_root: &root,
                        members: &members,
                        faults: &faults,
                        bitmap: &bitmap,
                        aggregate_key: &key,
                        message: &message,
                        signature: &sig,
                    },
                )?
                .digest()
                .clone())
            },
        )?;
        layouter.constrain_instance(output.cell(), config.public, 0)
    }
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn fixture() -> Link {
    let json: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let bytes = |name: &str| hex(json.get(name).unwrap().as_str().unwrap());
    let keys = json
        .get("committee_public_keys_hex")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| hex(v.as_str().unwrap()).try_into().unwrap())
        .collect::<Vec<[u8; 48]>>();
    let bits = bytes("qc_bitmap_hex");
    let mut bitmap = [0; 4];
    bitmap[..bits.len()].copy_from_slice(&bits);
    let mut sum = G1Projective::zero();
    for (i, key) in keys.iter().enumerate() {
        if bitmap[i / 8] & (1 << (i % 8)) != 0 {
            sum += G1Affine::deserialize_compressed(key.as_slice()).unwrap();
        }
    }
    let mut key = Vec::new();
    sum.into_affine().serialize_compressed(&mut key).unwrap();
    let (roster_root, _) = crate::finality::roster::key_tree_native(&keys).unwrap();
    let mut link = Link {
        context: CertificateContext {
            aggregation: AggregateContext {
                roster_root,
                members: u8::try_from(keys.len()).unwrap(),
                faults: (u8::try_from(keys.len()).unwrap() - 1) / 3,
                bitmap,
                aggregate_key: key.try_into().unwrap(),
            },
            message: bytes("commit_vote_preimage_hex").try_into().unwrap(),
            signature: bytes("qc_aggregate_signature_hex").try_into().unwrap(),
        },
        aggregation: [Fp::ZERO; 6],
        signature: [Fp::ZERO; 6],
        known: true,
    };
    link.endpoints();
    link
}
fn accepts(circuit: &Link) -> bool {
    check_circuit(
        circuit,
        16,
        &[vec![circuit.context.digest()]],
        CheckMode::Strict,
    )
    .is_ok_and(|report| report.is_satisfied())
}

#[test]
fn native_certificate_bytes_link_both_complete_source_programs() {
    let honest = fixture();
    assert!(accepts(&honest));
    for side in 0..2 {
        for index in 0..6 {
            let mut changed = honest.clone();
            let endpoints = if side == 0 {
                &mut changed.aggregation
            } else {
                &mut changed.signature
            };
            endpoints[index] += Fp::ONE;
            assert!(!accepts(&changed), "source {side} endpoint {index}");
        }
    }
    let known = synthesize(&honest, 16, None).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}

#[test]
fn root_quorum_key_and_message_substitutions_cannot_reuse_source_contexts() {
    let honest = fixture();
    for field in 0..7 {
        let mut changed = honest.clone();
        match field {
            0 => changed.context.aggregation.roster_root += Fp::ONE,
            1 => changed.context.aggregation.members += 1,
            2 => changed.context.aggregation.faults += 1,
            3 => changed.context.aggregation.bitmap[0] ^= 1,
            4 => changed.context.aggregation.aggregate_key[47] ^= 1,
            5 => changed.context.message[164] ^= 1,
            _ => changed.context.signature[95] ^= 1,
        }
        assert!(!accepts(&changed), "context field {field}");
    }
    for bad_height in [false, true] {
        let mut changed = honest.clone();
        if bad_height {
            changed.context.message[85..93].copy_from_slice(&1u64.to_be_bytes());
        } else {
            changed.context.message[12] = 2;
        }
        changed.endpoints();
        assert!(!accepts(&changed), "changed native Commit domain or height");
    }
}
