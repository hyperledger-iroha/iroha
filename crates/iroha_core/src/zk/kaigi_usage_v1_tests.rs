//! Final usage proofs through Core's canonical-key and exact-instance boundary.
use super::*;
use halo2_proofs::halo2curves::ff::{Field, PrimeField};
use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
use kaigi_zk::{
    authorization_v1::KaigiAuthorizationWitnessV1,
    usage_v1::{KaigiUsageContextV1, KaigiUsagePublicInputsV1, compute_usage_v1},
};
use std::sync::OnceLock;

struct Fixture {
    vk_bytes: Vec<u8>,
    proof: Vec<u8>,
    instance: [halo2_backend::Scalar; 25],
}
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let context = KaigiUsageContextV1 {
            network_id: [0x21; 32],
            call_id: [1, 2, 3, 4, 5, 6],
            host_id: [11, 12, 13, 14, 15, 16],
            pre_roster_root: [0x43; 32],
            segment_index: 3,
            duration_ms: 1200,
            billed_gas: 345,
        };
        let mut secret = (halo2_backend::Scalar::from(2).pow_vartime([190])
            + halo2_backend::Scalar::from(31))
        .to_repr();
        let witness = KaigiAuthorizationWitnessV1::take_blinding(&mut secret).unwrap();
        assert_eq!(secret, [0; 32]);
        let outputs = compute_usage_v1(&context, &witness).unwrap();
        let instance = KaigiUsagePublicInputsV1 { context, outputs }.instance();
        let params = pasta_params_new(KAIGI_USAGE_CIRCUIT_K_V1);
        let empty = KaigiUsageCircuitV1::default();
        let vk = halo2_backend::keygen_vk(&params, &empty).unwrap();
        let pk = halo2_backend::keygen_pk(&params, vk.clone(), &empty).unwrap();
        let proof = halo2_backend::create_ipa_proof(
            &params,
            &pk,
            &[KaigiUsageCircuitV1::new(context, witness).unwrap()],
            &[&[&instance]],
        )
        .unwrap();
        let mut vk_bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_bytes, KAIGI_USAGE_CIRCUIT_K_V1);
        zk1::wrap_append_circuit_id(&mut vk_bytes, KAIGI_USAGE_CIRCUIT_ID_V1);
        zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk);
        Fixture {
            vk_bytes,
            proof,
            instance,
        }
    })
}
fn envelope(
    backend: &str,
    material: Vec<u8>,
    columns: &[&[halo2_backend::Scalar]],
) -> (ProofBox, VerifyingKeyBox) {
    let vk = VerifyingKeyBox::new(backend.to_owned(), material);
    let mut inner = zk1::wrap_start();
    zk1::wrap_append_proof(&mut inner, &fixture().proof);
    zk1::wrap_append_instances_pasta_fp_cols(columns, &mut inner);
    let outer = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_USAGE_CIRCUIT_ID_V1.to_owned(),
        vk_hash: hash_vk(&vk),
        public_inputs: KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
        proof_bytes: inner,
        aux: Vec::new(),
    };
    (
        ProofBox::new(
            backend.to_owned(),
            norito::encode_canonical(&outer).unwrap(),
        ),
        vk,
    )
}

/// Canonical real proof shared by the exact-registry positive regression.
pub(super) fn valid_envelope(backend: &str) -> (ProofBox, VerifyingKeyBox) {
    let fixture = fixture();
    envelope(backend, fixture.vk_bytes.clone(), &[&fixture.instance])
}

#[test]
fn final_usage_real_proof_binds_all_25_rows_and_both_admission_routes() {
    let fixture = fixture();
    for backend in [ZK_BACKEND_HALO2_IPA, KAIGI_USAGE_BACKEND_V1] {
        let (proof, vk) = envelope(backend, fixture.vk_bytes.clone(), &[&fixture.instance]);
        assert!(
            validate_builtin_halo2_ipa_verifying_key_v1(backend, KAIGI_USAGE_CIRCUIT_ID_V1, &vk)
                .is_ok()
        );
        assert!(verify_backend(backend, &proof, Some(&vk)), "{backend}");
    }
    // Both entry points delegate to the same exact relation. Mutate every
    // network/call/host/root/metric limb and C/U through its generic entry point.
    for row in 0..25 {
        let mut instance = fixture.instance;
        instance[row] += halo2_backend::Scalar::ONE;
        let (proof, vk) = envelope(ZK_BACKEND_HALO2_IPA, fixture.vk_bytes.clone(), &[&instance]);
        assert!(
            !verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk)),
            "usage row {row}"
        );
    }
}

#[test]
fn final_usage_rejects_old_single_output_and_all_nonexact_shapes() {
    let fixture = fixture();
    let mut trailing_zero = fixture.instance.to_vec();
    trailing_zero.push(halo2_backend::Scalar::ZERO);
    let shapes = [
        vec![],
        vec![&fixture.instance[24..]],
        vec![&fixture.instance[..24]],
        vec![trailing_zero.as_slice()],
        vec![&fixture.instance[..], &fixture.instance[..]],
        fixture.instance.chunks_exact(1).collect(),
    ];
    for columns in shapes {
        let (proof, vk) = envelope(ZK_BACKEND_HALO2_IPA, fixture.vk_bytes.clone(), &columns);
        assert!(
            !verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk)),
            "shape {:?}",
            columns.iter().map(|c| c.len()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn final_usage_rejects_k8_aliases_and_wrong_canonical_key() {
    let fixture = fixture();
    let (proof, vk) = envelope(
        ZK_BACKEND_HALO2_IPA,
        fixture.vk_bytes.clone(),
        &[&fixture.instance],
    );
    for alias in [
        "kaigi-usage-v1",
        KAIGI_USAGE_BACKEND_V1,
        "halo2/ipa:kaigi-usage-v1",
    ] {
        let mut outer: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).unwrap();
        outer.circuit_id = alias.to_owned();
        let changed = ProofBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            norito::encode_canonical(&outer).unwrap(),
        );
        assert!(!verify_backend(ZK_BACKEND_HALO2_IPA, &changed, Some(&vk)));
        assert!(
            validate_builtin_halo2_ipa_verifying_key_v1(ZK_BACKEND_HALO2_IPA, alias, &vk).is_err()
        );
    }
    let mut old_k = fixture.vk_bytes.clone();
    old_k[12..16].copy_from_slice(&8_u32.to_le_bytes());
    let (changed, changed_vk) = envelope(ZK_BACKEND_HALO2_IPA, old_k, &[&fixture.instance]);
    assert!(
        validate_builtin_halo2_ipa_verifying_key_v1(
            ZK_BACKEND_HALO2_IPA,
            KAIGI_USAGE_CIRCUIT_ID_V1,
            &changed_vk
        )
        .is_err()
    );
    assert!(!verify_backend(
        ZK_BACKEND_HALO2_IPA,
        &changed,
        Some(&changed_vk)
    ));
    let params = pasta_params_new(KAIGI_USAGE_CIRCUIT_K_V1);
    let other = halo2_backend::keygen_vk(&params, &pasta_tiny::Add).unwrap();
    let mut material = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut material, KAIGI_USAGE_CIRCUIT_K_V1);
    zk1::wrap_append_circuit_id(&mut material, KAIGI_USAGE_CIRCUIT_ID_V1);
    zk1::wrap_append_vk_pasta(&mut material, &other);
    let (changed, changed_vk) = envelope(ZK_BACKEND_HALO2_IPA, material, &[&fixture.instance]);
    assert!(
        validate_builtin_halo2_ipa_verifying_key_v1(
            ZK_BACKEND_HALO2_IPA,
            KAIGI_USAGE_CIRCUIT_ID_V1,
            &changed_vk
        )
        .is_err()
    );
    assert!(!verify_backend(
        ZK_BACKEND_HALO2_IPA,
        &changed,
        Some(&changed_vk)
    ));
}
