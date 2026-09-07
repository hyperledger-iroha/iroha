//! Real final V1 Kaigi proofs through the canonical Core verifier boundary.

use super::*;
use halo2_proofs::halo2curves::ff::{Field, PrimeField};
use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
use kaigi_zk::authorization_v1::{
    KaigiAuthorizationActionV1, KaigiAuthorizationContextV1, KaigiAuthorizationPublicInputsV1,
    KaigiAuthorizationWitnessV1, compute_authorization_v1,
};
use std::sync::OnceLock;

struct Fixture {
    vk_bytes: Vec<u8>,
    proof: Vec<u8>,
    instance: [halo2_backend::Scalar; KAIGI_AUTHORIZATION_INSTANCE_ROWS_V1],
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let context = KaigiAuthorizationContextV1 {
            network_id: [0x21; 32],
            call_id: [1, 2, 3, 4, 5, 6],
            host_id: [11, 12, 13, 14, 15, 16],
            subject_id: [21, 22, 23, 24, 25, 26],
            participation_sequence: 1,
            action: KaigiAuthorizationActionV1::Join,
            pre_roster_root: [0x43; 32],
        };
        let mut secret = (halo2_backend::Scalar::from(2).pow_vartime([190])
            + halo2_backend::Scalar::from(31))
        .to_repr();
        let witness = KaigiAuthorizationWitnessV1::take_blinding(&mut secret).unwrap();
        assert_eq!(secret, [0; 32]);
        let outputs = compute_authorization_v1(&context, &witness).unwrap();
        let instance = KaigiAuthorizationPublicInputsV1 { context, outputs }.instance();
        let params = pasta_params_new(KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
        let empty = KaigiAuthorizationCircuitV1::default();
        let vk = halo2_backend::keygen_vk(&params, &empty).unwrap();
        let pk = halo2_backend::keygen_pk(&params, vk.clone(), &empty).unwrap();
        let circuit = KaigiAuthorizationCircuitV1::new(context, witness).unwrap();
        let proof =
            halo2_backend::create_ipa_proof(&params, &pk, &[circuit], &[&[&instance]]).unwrap();
        let mut vk_bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_bytes, KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
        zk1::wrap_append_circuit_id(&mut vk_bytes, KAIGI_AUTHORIZATION_CIRCUIT_ID_V1);
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
    vk_bytes: Vec<u8>,
    columns: &[&[halo2_backend::Scalar]],
) -> (ProofBox, VerifyingKeyBox) {
    let vk = VerifyingKeyBox::new(backend.to_owned(), vk_bytes);
    let mut inner = zk1::wrap_start();
    zk1::wrap_append_proof(&mut inner, &fixture().proof);
    zk1::wrap_append_instances_pasta_fp_cols(columns, &mut inner);
    let outer = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_AUTHORIZATION_CIRCUIT_ID_V1.to_owned(),
        vk_hash: hash_vk(&vk),
        public_inputs: KAIGI_AUTHORIZATION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
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

fn mutate_outer(proof: &ProofBox, mutate: impl FnOnce(&mut OpenVerifyEnvelope)) -> ProofBox {
    let mut outer: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).unwrap();
    mutate(&mut outer);
    ProofBox::new(
        proof.backend.clone(),
        norito::encode_canonical(&outer).unwrap(),
    )
}

/// Canonical real proof shared by the exact-registry positive regression.
pub(super) fn valid_envelope(backend: &str) -> (ProofBox, VerifyingKeyBox) {
    let fixture = fixture();
    envelope(backend, fixture.vk_bytes.clone(), &[&fixture.instance])
}

#[test]
fn final_kaigi_real_proof_verifies_and_binds_all_31_rows() {
    let fixture = fixture();
    for backend in [ZK_BACKEND_HALO2_IPA, KAIGI_AUTHORIZATION_BACKEND_V1] {
        let (proof, vk) = envelope(backend, fixture.vk_bytes.clone(), &[&fixture.instance]);
        assert!(
            validate_builtin_halo2_ipa_verifying_key_v1(
                backend,
                KAIGI_AUTHORIZATION_CIRCUIT_ID_V1,
                &vk
            )
            .is_ok()
        );
        assert!(verify_backend(backend, &proof, Some(&vk)), "{backend}");
        for row in 0..KAIGI_AUTHORIZATION_INSTANCE_ROWS_V1 {
            let mut instance = fixture.instance;
            instance[row] += halo2_backend::Scalar::ONE;
            let (changed, _) = envelope(backend, fixture.vk_bytes.clone(), &[&instance]);
            assert!(
                !verify_backend(backend, &changed, Some(&vk)),
                "{backend} row {row}"
            );
        }
    }
}

#[test]
fn final_kaigi_rejects_nonexact_instance_dimensions() {
    let fixture = fixture();
    let mut extra_zero = fixture.instance.to_vec();
    extra_zero.push(halo2_backend::Scalar::ZERO);
    // A trailing zero may be algebraically invisible to an IPA instance
    // commitment. The final wire schema still permits exactly 31 rows.
    let transposed: Vec<&[halo2_backend::Scalar]> = fixture.instance.chunks_exact(1).collect();
    let shapes = [
        vec![],
        vec![&fixture.instance[..30]],
        vec![extra_zero.as_slice()],
        vec![&fixture.instance[..], &fixture.instance[..]],
        transposed,
    ];
    for backend in [ZK_BACKEND_HALO2_IPA, KAIGI_AUTHORIZATION_BACKEND_V1] {
        for columns in &shapes {
            let (proof, vk) = envelope(backend, fixture.vk_bytes.clone(), columns);
            assert!(
                !verify_backend(backend, &proof, Some(&vk)),
                "{backend} shape {:?}",
                columns.iter().map(|c| c.len()).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn final_kaigi_rejects_retired_labels_schema_metadata_and_relabelled_keys() {
    let fixture = fixture();
    let (proof, vk) = envelope(
        ZK_BACKEND_HALO2_IPA,
        fixture.vk_bytes.clone(),
        &[&fixture.instance],
    );
    for retired in [
        "kaigi-roster-v1",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/ipa/kaigi-roster-v1",
    ] {
        assert!(!halo2_open_verify_circuit_id_is_production_v1(retired));
        assert!(halo2_ipa_public_inputs_schema_v1(retired).is_none());
        assert!(halo2_ipa_canonical_k_v1(retired).is_none());
        assert!(!verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &mutate_outer(&proof, |outer| outer.circuit_id = retired.to_owned()),
            Some(&vk)
        ));
        let (retired_proof, retired_vk) =
            envelope(retired, fixture.vk_bytes.clone(), &[&fixture.instance]);
        assert!(!verify_backend(retired, &retired_proof, Some(&retired_vk)));
    }
    for schema in [
        b"kaigi-roster-v1".as_slice(),
        b"kaigi-usage-v1",
        b"kaigi-authorization-v1\0",
    ] {
        assert!(!verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &mutate_outer(&proof, |outer| outer.public_inputs = schema.to_vec()),
            Some(&vk)
        ));
    }
    for alias in [
        "kaigi-authorization-v1",
        KAIGI_AUTHORIZATION_BACKEND_V1,
        "halo2/ipa:kaigi-authorization-v1",
    ] {
        assert!(!verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &mutate_outer(&proof, |outer| outer.circuit_id = alias.to_owned()),
            Some(&vk)
        ));
        assert!(
            validate_builtin_halo2_ipa_verifying_key_v1(ZK_BACKEND_HALO2_IPA, alias, &vk).is_err()
        );
    }
    let mut wrong_k = fixture.vk_bytes.clone();
    // First fixed TLV is IPAK: magic4 + tag4 + len4 + u32 value.
    wrong_k[12..16].copy_from_slice(&8_u32.to_le_bytes());
    let mut duplicate_k = fixture.vk_bytes.clone();
    zk1::wrap_append_ipa_k(&mut duplicate_k, KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
    for material in [wrong_k, duplicate_k] {
        let (changed, changed_vk) = envelope(ZK_BACKEND_HALO2_IPA, material, &[&fixture.instance]);
        assert!(
            validate_builtin_halo2_ipa_verifying_key_v1(
                ZK_BACKEND_HALO2_IPA,
                KAIGI_AUTHORIZATION_CIRCUIT_ID_V1,
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
    // Keep CID, k, public schema and outer VK hash mutually consistent while
    // substituting an unrelated constraint system: canonical key equality wins.
    let params = pasta_params_new(KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
    let other = halo2_backend::keygen_vk(&params, &pasta_tiny::Add).unwrap();
    let mut material = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut material, KAIGI_AUTHORIZATION_CIRCUIT_K_V1);
    zk1::wrap_append_circuit_id(&mut material, KAIGI_AUTHORIZATION_CIRCUIT_ID_V1);
    zk1::wrap_append_vk_pasta(&mut material, &other);
    let (changed, changed_vk) = envelope(ZK_BACKEND_HALO2_IPA, material, &[&fixture.instance]);
    assert!(
        validate_builtin_halo2_ipa_verifying_key_v1(
            ZK_BACKEND_HALO2_IPA,
            KAIGI_AUTHORIZATION_CIRCUIT_ID_V1,
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
