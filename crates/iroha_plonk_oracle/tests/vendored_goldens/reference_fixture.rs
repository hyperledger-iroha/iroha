//! Genuine independently checkable reference-verifier inputs, never accepted verdicts.

use core::fmt::Write as _;
use halo2_axiom::halo2curves::ff::PrimeField;
use halo2_axiom::poly::commitment::Params;
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    cs::{InstanceModeV1, InstanceType, ProofSuffixV1, TranscriptV1, TranscriptV2},
    keys::{KeygenConfig, KeygenConfigV2, ProvingKey, keygen_from_tables_v2},
    prover::{ProverConfig, ProverRandomness, create_proof},
    verifier::verify_full,
};
use iroha_plonk_oracle::convert::{CurveBridge, Pallas, Vesta};
use norito::json::{Map, Value};

use crate::{
    cases::{Family, Setup, setup},
    golden_proof_bytes::digest_hex,
    kagemusha_parity::kagemusha_keys,
};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("String write");
    }
    out
}

fn record<B: CurveBridge>(
    name: &str,
    source: &Setup<B>,
    pk: &ProvingKey<B::Native>,
    proof: &[u8],
    oracle: bool,
) -> Value {
    let mut fields = Map::from_iter([
        ("name".into(), Value::String(format!("{}/{name}", B::NAME))),
        ("curve".into(), Value::String(B::NAME.into())),
        ("k".into(), Value::from(u64::from(source.k))),
        (
            "descriptor_version".into(),
            Value::from(if pk.binding().is_v2() { 2_u64 } else { 1 }),
        ),
        (
            "descriptor".into(),
            Value::String(hex(pk.binding().encoded())),
        ),
        (
            "verifying_key".into(),
            Value::String(hex(pk.vk().to_bytes())),
        ),
        ("proof".into(), Value::String(hex(proof))),
        ("proof_sha256".into(), Value::String(digest_hex(proof))),
        (
            "instances".into(),
            Value::Array(
                source
                    .native_instances()
                    .iter()
                    .map(|column| {
                        Value::Array(
                            column
                                .iter()
                                .map(|v| Value::String(hex(v.to_repr().as_ref())))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
    ]);
    if oracle {
        fields.insert(
            "oracle_repr".into(),
            Value::String(hex(source.transcript_repr.to_repr().as_ref())),
        );
    }
    Value::Object(fields)
}

fn production<B: CurveBridge>(name: &str, source: &Setup<B>, pk: &ProvingKey<B::Native>) -> Value {
    let witness = source.exported.witness(pk).expect("exact source witness");
    let proof = create_proof(
        &source.params,
        pk,
        &witness,
        ProverRandomness::fixed_seed_for_tests([42; 32]),
        ProverConfig::default(),
    )
    .expect("genuine native production proof");
    verify_full(
        &source.params,
        pk.binding(),
        pk.vk(),
        &source.native_instances(),
        &proof,
        MemoryBudget::DEFAULT,
    )
    .expect("complete native verification");
    record(name, source, pk, &proof, false)
}

fn v2_key<B: CurveBridge>(source: &Setup<B>, config: &KeygenConfigV2) -> ProvingKey<B::Native> {
    let export = &source.exported;
    keygen_from_tables_v2(
        &source.params,
        export.constraint_system().clone(),
        export.fixed().to_vec(),
        export.selectors().to_vec(),
        &export.permutation().expect("permutation"),
        config,
    )
    .expect("V2 key")
}

fn oracle_cases<B: CurveBridge>(source: &Setup<B>, cases: &mut Vec<Value>) {
    let k = source.k;
    let proof = source.prove_vendored([42; 32]);
    source
        .verify_vendored(&source.vendored_instances(), &proof)
        .expect("original full verifier");
    cases.push(record(
        &format!("oracle/blake/{k}"),
        source,
        &source.pk,
        &proof,
        true,
    ));
    let keys = kagemusha_keys(source);
    let proof = source.prove_vendored_kagemusha([42; 32]);
    source
        .verify_vendored_kagemusha(&source.vendored_instances(), &proof)
        .expect("original augmented verifier");
    cases.push(record(
        &format!("oracle/poseidon/{k}"),
        source,
        &keys.pk,
        &proof,
        true,
    ));
}

fn v1_cases<B: CurveBridge>(source: &Setup<B>, cases: &mut Vec<Value>) {
    for (name, transcript) in [
        ("blake", TranscriptV1::Blake2bChallenge255),
        ("poseidon", TranscriptV1::KagemushaPoseidonRp57),
    ] {
        for mode in [InstanceModeV1::Committed, InstanceModeV1::Direct] {
            for compress in [false, true] {
                let mut config = KeygenConfig::new(transcript);
                config.instance_mode = mode;
                config.compress_selectors = compress;
                config.proof_suffix = if compress {
                    ProofSuffixV1::FoldedGenerator
                } else {
                    ProofSuffixV1::None
                };
                let pk = source
                    .exported
                    .keygen(&source.params, &config)
                    .expect("V1 key");
                cases.push(production(
                    &format!("v1/{name}/{mode:?}/{compress}"),
                    source,
                    &pk,
                ));
            }
        }
    }
}

fn v2_cases<B: CurveBridge>(source: &Setup<B>, cases: &mut Vec<Value>) {
    for (name, transcript) in [
        ("blake", TranscriptV2::Blake2bChallenge255),
        ("poseidon", TranscriptV2::KagemushaPoseidonRp57),
        ("base", TranscriptV2::KagemushaPoseidonRp57Base),
    ] {
        for mode in [InstanceModeV1::Committed, InstanceModeV1::Direct] {
            if transcript == TranscriptV2::KagemushaPoseidonRp57Base
                && mode == InstanceModeV1::Committed
            {
                continue;
            }
            let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Field]);
            config.transcript = transcript;
            config.instance_mode = mode;
            let pk = v2_key(source, &config);
            cases.push(production(
                &format!("v2/{name}/{mode:?}/field"),
                source,
                &pk,
            ));
        }
    }
    let config = KeygenConfigV2::pipa_r(vec![InstanceType::Bits(4)]);
    let pk = v2_key(source, &config);
    cases.push(production("v2/base/k6/bits4", source, &pk));
}

fn curve_cases<B: CurveBridge>(parameters: &mut Map, cases: &mut Vec<Value>) {
    for (family, k) in [
        (Family::Sigma, 6),
        (Family::Sigma, 7),
        (Family::Wide, 8),
        (Family::Sigma, 9),
        (Family::Wide, 10),
    ] {
        let source = setup::<B>(family, k);
        let mut original = Vec::new();
        source
            .vendored_params
            .write(&mut original)
            .expect("original params");
        assert_eq!(
            original,
            source.params.params().to_bytes(),
            "independent full parameter bytes"
        );
        parameters.insert(format!("{}/{k}", B::NAME), Value::String(hex(&original)));
        if k == 6 || k == 8 {
            oracle_cases(&source, cases);
        }
        if k == 6 {
            v1_cases(&source, cases);
            v2_cases(&source, cases);
        }
        let mut config = KeygenConfigV2::pipa_r(vec![if k == 6 {
            InstanceType::Bounded
        } else {
            InstanceType::Field
        }]);
        config.compress_selectors = k != 8;
        let pk = v2_key(&source, &config);
        cases.push(production(
            &format!("v2/base/k{k}/bounded-or-field"),
            &source,
            &pk,
        ));
    }
}

fn document() -> Value {
    let mut parameters = Map::new();
    let mut cases = Vec::new();
    curve_cases::<Pallas>(&mut parameters, &mut cases);
    curve_cases::<Vesta>(&mut parameters, &mut cases);
    Value::Object(Map::from_iter([
        (
            "format".into(),
            Value::String("iroha.pipa.reference.v1".into()),
        ),
        ("parameters".into(), Value::Object(parameters)),
        ("cases".into(), Value::Array(cases)),
    ]))
}

#[test]
#[ignore = "Explicit maintenance capture of genuine complete proof inputs; never rewrites fixtures."]
fn capture_reference_verifier_inputs() {
    println!(
        "REFERENCE_CAPTURE={}",
        norito::json::to_json(&document()).expect("capture JSON")
    );
}

#[test]
fn reference_verifier_inputs_match_genuine_sources() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/native_prover/reference_v1.json");
    let captured: Value =
        norito::json::from_str(&std::fs::read_to_string(path).expect("retained reference fixture"))
            .expect("fixture JSON");
    assert_eq!(
        document(),
        captured,
        "review source changes before an explicit new capture"
    );
}
