//! Mint transport artifact plumbing and checked-reader regressions.
//!
//! The storage blobs and structurally valid VK encodings below are synthetic.
//! They are not proving artifacts, accepted mint proofs, an authenticated
//! release, or evidence that the complete K16 circuits meet resource ceilings.

use std::collections::BTreeSet;

use halo2_proofs::plonk::{Circuit, ConstraintSystem};

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    mint_authorization::{
        KagemushaMintAuthorizationEpCircuitV1, KagemushaMintAuthorizationEqCircuitV1,
    },
    mint_transport_decider::{
        KagemushaMintAuthorizationTransportEpCircuitV1,
        KagemushaMintAuthorizationTransportEqCircuitV1,
    },
};

fn test_base_params() -> BaseCircuitParams {
    BaseCircuitParams {
        k: KAGEMUSHA_HALO2_K_V1 as usize,
        num_advice_per_phase: vec![2],
        num_fixed: 1,
        num_lookup_advice_per_phase: vec![1],
        lookup_bits: Some((KAGEMUSHA_HALO2_K_V1 - 1) as usize),
        num_instance_columns: 1,
    }
}

fn synthetic_artifacts() -> KagemushaGeneratedMintAuthorizationArtifactsV1 {
    // Every key role receives distinct bytes and length so accidental
    // inner/outer or parity reuse cannot satisfy the resolver assertions.
    let blob = |tag: u8| Arc::<[u8]>::from(vec![tag; usize::from(tag) + 11]);
    // Parameter roles have an exact descriptor length even in storage tests.
    // These repeated-byte buffers are not serialized ParamsIPA instances.
    let parameter_len = usize::try_from(KAGEMUSHA_PARAMS_BYTES_V1)
        .expect("fixed parameter descriptor length fits usize");
    let mut inner_eq_circuit_params = test_base_params();
    inner_eq_circuit_params.num_advice_per_phase = vec![3];
    let mut inner_ep_circuit_params = test_base_params();
    inner_ep_circuit_params.num_advice_per_phase = vec![4];
    let mut enabled_hardware_profiles = [[0; 32]; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1];
    enabled_hardware_profiles[0] = [0x31; 32];
    KagemushaGeneratedMintAuthorizationArtifactsV1 {
        eq_parameters: Arc::from(vec![1; parameter_len]),
        ep_parameters: Arc::from(vec![2; parameter_len]),
        eq_proving_key: blob(3),
        eq_verifying_key: blob(4),
        ep_proving_key: blob(5),
        ep_verifying_key: blob(6),
        eq_circuit_params: test_base_params(),
        ep_circuit_params: test_base_params(),
        eq_protocol_digest: [0x21; 32],
        ep_protocol_digest: [0x22; 32],
        inner_eq_proving_key: blob(7),
        inner_eq_verifying_key: blob(8),
        inner_ep_proving_key: blob(9),
        inner_ep_verifying_key: blob(10),
        inner_eq_circuit_params,
        inner_ep_circuit_params,
        enabled_hardware_profiles,
    }
}

fn expected_key_blobs(
    artifacts: &KagemushaGeneratedMintAuthorizationArtifactsV1,
) -> [(KagemushaArtifactRoleV1, &Arc<[u8]>); 8] {
    use KagemushaArtifactRoleV1 as Role;
    [
        (Role::MintAuthorizationPkEq, &artifacts.eq_proving_key),
        (Role::MintAuthorizationVkEq, &artifacts.eq_verifying_key),
        (Role::MintAuthorizationPkEp, &artifacts.ep_proving_key),
        (Role::MintAuthorizationVkEp, &artifacts.ep_verifying_key),
        (
            Role::InnerMintAuthorizationPkEq,
            &artifacts.inner_eq_proving_key,
        ),
        (
            Role::InnerMintAuthorizationVkEq,
            &artifacts.inner_eq_verifying_key,
        ),
        (
            Role::InnerMintAuthorizationPkEp,
            &artifacts.inner_ep_proving_key,
        ),
        (
            Role::InnerMintAuthorizationVkEp,
            &artifacts.inner_ep_verifying_key,
        ),
    ]
}

#[test]
fn mint_authorization_artifact_bindings_cover_all_eight_distinct_key_roles() {
    let artifacts = synthetic_artifacts();
    let expected = expected_key_blobs(&artifacts);
    let bindings = artifacts.bindings();
    assert_eq!(bindings.len(), 8);
    assert_eq!(
        bindings
            .iter()
            .map(|entry| entry.role)
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
    assert_eq!(
        bindings
            .iter()
            .map(|entry| entry.sha256)
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
    for (actual, (role, bytes)) in bindings.iter().zip(expected) {
        assert_eq!(actual.role, role);
        assert_eq!(actual.byte_len, bytes.len() as u64);
        assert_eq!(
            actual.sha256,
            <[u8; 32]>::from(Sha256::digest(bytes.as_ref()))
        );
    }
    assert_eq!(artifacts.enabled_hardware_profiles[0], [0x31; 32]);
    assert!(
        artifacts.enabled_hardware_profiles[1..]
            .iter()
            .all(|profile| *profile == [0; 32])
    );
    assert!(!same_base_params(
        &artifacts.inner_eq_circuit_params,
        &artifacts.eq_circuit_params
    ));
    assert!(!same_base_params(
        &artifacts.inner_ep_circuit_params,
        &artifacts.ep_circuit_params
    ));
}

#[test]
fn mint_authorization_install_resolves_all_ten_blobs_without_replacing_existing_artifacts() {
    let artifacts = synthetic_artifacts();
    let mut resolver = KagemushaMemoryArtifactResolverV1::default();
    let preserved = Arc::<[u8]>::from(b"separate already installed artifact".as_slice());
    resolver.insert(Arc::clone(&preserved));
    let preserved_binding = binding(KagemushaArtifactRoleV1::StateVkEq, preserved.as_ref());
    assert_eq!(resolver.len(), 1);

    artifacts.install_into(&mut resolver);
    assert_eq!(
        resolver.len(),
        11,
        "two parameters and eight keys are installed"
    );
    let expected = [
        (KagemushaArtifactRoleV1::ParamsEq, &artifacts.eq_parameters),
        (KagemushaArtifactRoleV1::ParamsEp, &artifacts.ep_parameters),
    ]
    .into_iter()
    .chain(expected_key_blobs(&artifacts));
    for (role, expected_bytes) in expected {
        let entry = binding(role, expected_bytes.as_ref());
        let resolved = resolver
            .resolve_bytes(entry)
            .expect("installed synthetic content address");
        assert_eq!(resolved.as_ref(), expected_bytes.as_ref());
        let mut substituted = entry;
        substituted.sha256[0] ^= 0x80;
        assert!(resolver.resolve_bytes(substituted).is_err());
        let mut truncated = entry;
        truncated.byte_len -= 1;
        assert!(resolver.resolve_bytes(truncated).is_err());
    }
    assert_eq!(
        resolver.resolve_bytes(preserved_binding).unwrap().as_ref(),
        preserved.as_ref()
    );
    artifacts.install_into(&mut resolver);
    assert_eq!(
        resolver.len(),
        11,
        "reinstallation is content-address idempotent"
    );
    assert_eq!(
        resolver.resolve_bytes(preserved_binding).unwrap().as_ref(),
        preserved.as_ref()
    );
}

/// Produce a canonical *encoding* with the actual circuit's key layout, but no
/// key generation or proof. Repeated generator commitments are not that circuit's VK.
fn synthetic_vk_encoding<F, T>(point: &[u8]) -> Vec<u8>
where
    F: ff::PrimeField,
    T: Circuit<F, Params = BaseCircuitParams>,
{
    assert_eq!(point.len(), 32);
    let mut cs = ConstraintSystem::<F>::default();
    T::configure_with_params(&mut cs, test_base_params());
    let fixed_count = cs
        .num_fixed_columns()
        .checked_add(cs.num_selectors())
        .unwrap();
    assert_ne!(fixed_count, 0);
    let point_count = fixed_count
        .checked_add(cs.permutation().get_columns().len())
        .unwrap();
    let mut bytes = vec![0x02];
    bytes.extend_from_slice(&KAGEMUSHA_HALO2_K_V1.to_le_bytes());
    bytes.push(0); // Canonical uncompressed-selector layout, not raw curve encoding.
    bytes.extend_from_slice(&u32::try_from(fixed_count).unwrap().to_le_bytes());
    for _ in 0..point_count {
        bytes.extend_from_slice(point);
    }
    bytes
}

fn malformed_vk_encodings(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut malformed = Vec::new();
    let mut wrong_version = bytes.to_vec();
    wrong_version[0] = 0;
    malformed.push(wrong_version);
    for k in [0, 6, 15, 17, u32::MAX] {
        let mut wrong_degree = bytes.to_vec();
        wrong_degree[1..5].copy_from_slice(&k.to_le_bytes());
        malformed.push(wrong_degree);
    }
    let mut wrong_selector_mode = bytes.to_vec();
    wrong_selector_mode[5] = 2;
    malformed.push(wrong_selector_mode);
    for count in [0, u32::MAX] {
        let mut wrong_count = bytes[..10].to_vec();
        wrong_count[6..10].copy_from_slice(&count.to_le_bytes());
        malformed.push(wrong_count);
    }
    let mut invalid_point = bytes.to_vec();
    invalid_point[10..42].fill(0xFF);
    malformed.push(invalid_point);
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    malformed.push(trailing);
    for end in [0, 1, 5, 6, 9, 10, 11, 41, 42, bytes.len() - 1] {
        malformed.push(bytes[..end].to_vec());
    }
    malformed
}

macro_rules! check_mint_vk_readers {
    ($curve:ty, $field:ty, $inner:ty, $outer:ty, $read_inner:ident, $read_outer:ident) => {{
        let point = <$curve>::generator().to_bytes();
        let inner_bytes = synthetic_vk_encoding::<$field, $inner>(point.as_ref());
        let outer_bytes = synthetic_vk_encoding::<$field, $outer>(point.as_ref());
        assert_ne!(
            inner_bytes.len(),
            outer_bytes.len(),
            "inner and outer configured layouts differ"
        );
        let inner = $read_inner(&inner_bytes, test_base_params())
            .expect("canonical synthetic inner VK encoding");
        let outer = $read_outer(&outer_bytes, test_base_params())
            .expect("canonical synthetic outer VK encoding");
        assert_eq!(inner.to_bytes(SerdeFormat::Processed), inner_bytes);
        assert_eq!(outer.to_bytes(SerdeFormat::Processed), outer_bytes);
        assert!(
            $read_inner(&outer_bytes, test_base_params()).is_err(),
            "outer key cannot be decoded as the inner family"
        );
        assert!(
            $read_outer(&inner_bytes, test_base_params()).is_err(),
            "inner key cannot be decoded as the outer family"
        );
        type VkReader = fn(
            &[u8],
            BaseCircuitParams,
        )
            -> Result<VerifyingKey<$curve>, KagemushaArtifactGenerationErrorV1>;
        let readers: [(&[u8], VkReader); 2] = [
            (inner_bytes.as_slice(), $read_inner),
            (outer_bytes.as_slice(), $read_outer),
        ];
        for (bytes, reader) in readers {
            for malformed in malformed_vk_encodings(bytes) {
                let result = std::panic::catch_unwind(|| reader(&malformed, test_base_params()));
                assert!(result.expect("malformed mint VK must not panic").is_err());
            }
        }
    }};
}

#[test]
fn checked_mint_authorization_inner_outer_vk_readers_reject_malformed_eq_encodings() {
    check_mint_vk_readers!(
        EqAffine,
        Fp,
        KagemushaMintAuthorizationEqCircuitV1,
        KagemushaMintAuthorizationTransportEqCircuitV1,
        read_eq_inner_mint_authorization_vk,
        read_eq_mint_authorization_vk
    );
}

#[test]
fn checked_mint_authorization_inner_outer_vk_readers_reject_malformed_ep_encodings() {
    check_mint_vk_readers!(
        EpAffine,
        Fq,
        KagemushaMintAuthorizationEpCircuitV1,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        read_ep_inner_mint_authorization_vk,
        read_ep_mint_authorization_vk
    );
}
