//! Full-interval result/certificate statement linkage; these checks do not
//! replace either child proof or authenticate a validator roster.
use super::*;
use crate::finality::{aggregate::AggregateContext, certificate::CertificateContext};
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
    context: CertifiedResultContext,
    sources: [[Fp; 6]; 2],
    known: bool,
}
impl Link {
    fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(
                region,
                &values.map(|v| {
                    if self.known {
                        Value::known(v)
                    } else {
                        Value::unknown()
                    }
                }),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)
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
        let digest = layouter.assign_region(
            || "full certified result binding",
            |mut region| {
                let left = self.words(&mut chip, &mut region, self.sources[0])?;
                let right = self.words(&mut chip, &mut region, self.sources[1])?;
                let left = SourceEndpoints::from_words(&mut chip, &mut region, &left)?;
                let right = SourceEndpoints::from_words(&mut chip, &mut region, &right)?;
                let [root, len] = self.words(
                    &mut chip,
                    &mut region,
                    [
                        self.context.root,
                        Fp::from(u64::from(self.context.frame_len)),
                    ],
                )?;
                let len = chip.uint().range_check::<32>(&mut region, &len)?;
                let statement = CertificateStatementCells::assign(
                    &mut chip,
                    &mut region,
                    if self.known {
                        Value::known(self.context.certificate)
                    } else {
                        Value::unknown()
                    },
                )?;
                Ok(CertifiedResultLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &left,
                    &right,
                    &statement,
                    &root,
                    &len,
                )?
                .digest()
                .clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
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
    let frame = bytes("result_preimage_hex");
    let expected = bytes("result_hash_hex").try_into().unwrap();
    let scan = result_scan::prepare_result_scan(&frame, expected).unwrap();
    let context = CertifiedResultContext {
        certificate: CertificateContext {
            aggregation: AggregateContext {
                roster_root,
                members: u8::try_from(keys.len()).unwrap(),
                faults: (u8::try_from(keys.len()).unwrap() - 1) / 3,
                bitmap,
                aggregate_key: key.try_into().unwrap(),
            },
            message: bytes("commit_vote_preimage_hex").try_into().unwrap(),
            signature: bytes("qc_aggregate_signature_hex").try_into().unwrap(),
        }
        .statement(),
        root: scan[0].context().root,
        frame_len: frame.len().try_into().unwrap(),
    };
    Link {
        context,
        sources: context.source_endpoints(),
        known: true,
    }
}

fn accepts(circuit: &Link) -> bool {
    check_circuit(
        circuit,
        16,
        &[vec![circuit.context.digest()]],
        CheckMode::Strict,
    )
    .is_ok_and(|r| r.is_satisfied())
}
#[test]
fn complete_certified_result_context_pins_every_original_endpoint() {
    let honest = fixture();
    assert!(accepts(&honest));
    for side in 0..2 {
        for field in 0..6 {
            let mut changed = honest.clone();
            changed.sources[side][field] += Fp::ONE;
            assert!(!accepts(&changed), "source {side} endpoint {field}");
        }
    }
    let known = synthesize(&honest, 16, None).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}
#[test]
fn certificate_and_scan_cannot_substitute_different_signed_results() {
    let honest = fixture();
    for mutation in 0..5 {
        let mut changed = honest.clone();
        match mutation {
            0 => changed.context.root += Fp::ONE,
            1 => changed.context.frame_len += 128,
            2 => changed.context.certificate.message[133] ^= 1,
            3 => changed.context.certificate.roster_root += Fp::ONE,
            _ => changed.context.certificate.message[101] ^= 1,
        }
        assert!(!accepts(&changed), "context {mutation}");
    }
    // Give the changed vote an internally consistent certificate endpoint while
    // retaining the genuine original scan; only a full R match may compose.
    let mut changed = honest.clone();
    changed.context.certificate.message[133] ^= 1;
    changed.sources[0] = changed.context.source_endpoints()[0];
    assert!(!accepts(&changed));
    // Conversely, a claimed scan for changed R cannot reuse the original QC.
    changed.sources[0] = honest.sources[0];
    changed.sources[1] = changed.context.source_endpoints()[1];
    assert!(!accepts(&changed));
}
