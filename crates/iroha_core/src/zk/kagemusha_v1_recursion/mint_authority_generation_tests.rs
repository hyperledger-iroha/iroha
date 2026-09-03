//! Synthetic mint-authority artifact plumbing and checked-header regressions.
//!
//! No fixture below is a genuine parameter set, key, proof, or authenticated
//! release. Header rejection exercises the checked reader's pre-body bounds;
//! it does not measure allocator usage or qualify complete K16 proof resources.

use std::collections::BTreeSet;

use super::*;

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

fn synthetic_artifacts() -> KagemushaGeneratedMintAuthorityArtifactsV1 {
    let blob = |tag: u8| Arc::<[u8]>::from(vec![tag; usize::from(tag) + 11]);
    // Resolver descriptors require the exact parameter byte length. These
    // repeated-byte buffers satisfy storage shape only, not ParamsIPA decoding.
    let parameter_len = usize::try_from(KAGEMUSHA_PARAMS_BYTES_V1)
        .expect("fixed parameter descriptor length fits usize");
    let mut inner_eq_circuit_params = test_base_params();
    inner_eq_circuit_params.num_advice_per_phase = vec![3];
    let mut inner_ep_circuit_params = test_base_params();
    inner_ep_circuit_params.num_advice_per_phase = vec![4];
    KagemushaGeneratedMintAuthorityArtifactsV1 {
        eq_parameters: Arc::from(vec![1; parameter_len]),
        ep_parameters: Arc::from(vec![2; parameter_len]),
        eq_proving_key: blob(3),
        eq_verifying_key: blob(4),
        ep_proving_key: blob(5),
        ep_verifying_key: blob(6),
        eq_circuit_params: test_base_params(),
        ep_circuit_params: test_base_params(),
        inner_eq_proving_key: blob(7),
        inner_eq_verifying_key: blob(8),
        inner_ep_proving_key: blob(9),
        inner_ep_verifying_key: blob(10),
        inner_eq_circuit_params,
        inner_ep_circuit_params,
        release_id: [0x21; 32],
        eq_protocol_digest: [0x22; 32],
        ep_protocol_digest: [0x23; 32],
        genesis_roster_id: [0x24; 32],
    }
}

fn expected_blobs(
    artifacts: &KagemushaGeneratedMintAuthorityArtifactsV1,
) -> [(KagemushaArtifactRoleV1, &Arc<[u8]>); 10] {
    use KagemushaArtifactRoleV1 as Role;
    [
        (Role::ParamsEq, &artifacts.eq_parameters),
        (Role::ParamsEp, &artifacts.ep_parameters),
        (Role::MintCreditPkEq, &artifacts.eq_proving_key),
        (Role::MintCreditVkEq, &artifacts.eq_verifying_key),
        (Role::MintCreditPkEp, &artifacts.ep_proving_key),
        (Role::MintCreditVkEp, &artifacts.ep_verifying_key),
        (Role::InnerMintCreditPkEq, &artifacts.inner_eq_proving_key),
        (Role::InnerMintCreditVkEq, &artifacts.inner_eq_verifying_key),
        (Role::InnerMintCreditPkEp, &artifacts.inner_ep_proving_key),
        (Role::InnerMintCreditVkEp, &artifacts.inner_ep_verifying_key),
    ]
}

#[test]
fn mint_authority_artifact_bindings_cover_ten_distinct_roles_in_order() {
    let artifacts = synthetic_artifacts();
    let bindings = artifacts.bindings();
    assert_eq!(bindings.len(), 10);
    assert_eq!(
        bindings
            .iter()
            .map(|entry| entry.role)
            .collect::<BTreeSet<_>>()
            .len(),
        10
    );
    assert_eq!(
        bindings
            .iter()
            .map(|entry| entry.sha256)
            .collect::<BTreeSet<_>>()
            .len(),
        10,
        "every inner/outer and parity slot has distinct synthetic bytes"
    );
    for (entry, (role, bytes)) in bindings.iter().zip(expected_blobs(&artifacts)) {
        assert_eq!(entry.role, role);
        assert_eq!(entry.byte_len, bytes.len() as u64);
        assert_eq!(
            entry.sha256,
            <[u8; 32]>::from(Sha256::digest(bytes.as_ref()))
        );
    }
    assert!(!same_base_params(
        &artifacts.inner_eq_circuit_params,
        &artifacts.eq_circuit_params
    ));
    assert!(!same_base_params(
        &artifacts.inner_ep_circuit_params,
        &artifacts.ep_circuit_params
    ));
    assert_eq!(artifacts.release_id, [0x21; 32]);
    assert_eq!(artifacts.eq_protocol_digest, [0x22; 32]);
    assert_eq!(artifacts.ep_protocol_digest, [0x23; 32]);
    assert_eq!(artifacts.genesis_roster_id, [0x24; 32]);
}

#[test]
fn mint_authority_artifact_install_resolves_ten_blobs_and_preserves_existing_entries() {
    let artifacts = synthetic_artifacts();
    let mut resolver = KagemushaMemoryArtifactResolverV1::default();
    let preserved = Arc::<[u8]>::from(b"unrelated existing state verifying-key storage".as_slice());
    let preserved_binding = KagemushaArtifactBindingV1 {
        role: KagemushaArtifactRoleV1::StateVkEq,
        sha256: Sha256::digest(preserved.as_ref()).into(),
        byte_len: preserved.len() as u64,
    };
    resolver.insert(Arc::clone(&preserved));
    artifacts.install_into(&mut resolver);
    assert_eq!(resolver.len(), 11);
    for (entry, (role, bytes)) in artifacts
        .bindings()
        .into_iter()
        .zip(expected_blobs(&artifacts))
    {
        assert_eq!(entry.role, role);
        let resolved = resolver
            .resolve_bytes(entry)
            .expect("installed synthetic blob");
        assert_eq!(resolved.as_ref(), bytes.as_ref());
        let mut wrong_digest = entry;
        wrong_digest.sha256[0] ^= 0x80;
        assert!(resolver.resolve_bytes(wrong_digest).is_err());
        let mut wrong_length = entry;
        wrong_length.byte_len -= 1;
        assert!(resolver.resolve_bytes(wrong_length).is_err());
    }
    assert_eq!(
        resolver.resolve_bytes(preserved_binding).unwrap().as_ref(),
        preserved.as_ref()
    );
    artifacts.install_into(&mut resolver);
    assert_eq!(
        resolver.len(),
        11,
        "installation is content-address idempotent"
    );
    assert_eq!(
        resolver.resolve_bytes(preserved_binding).unwrap().as_ref(),
        preserved.as_ref()
    );
}

fn vk_header(version: u8, k: u32, selectors: u8, fixed_count: u32) -> [u8; 10] {
    let mut bytes = [0; 10];
    bytes[0] = version;
    bytes[1..5].copy_from_slice(&k.to_le_bytes());
    bytes[5] = selectors;
    bytes[6..10].copy_from_slice(&fixed_count.to_le_bytes());
    bytes
}

fn decode_error_reason<T>(
    result: Result<T, KagemushaArtifactGenerationErrorV1>,
    expected_parity: KagemushaPastaParityV1,
    expected_kind: &'static str,
) -> String {
    match result {
        Err(KagemushaArtifactGenerationErrorV1::KeyDecode {
            parity,
            kind,
            reason,
        }) => {
            assert_eq!(parity, expected_parity);
            assert_eq!(kind, expected_kind);
            reason
        }
        Err(error) => panic!("expected checked key-decode failure, got {error}"),
        Ok(_) => panic!("header-only malformed bytes must not decode as a verifying key"),
    }
}

macro_rules! check_authority_vk_headers {
    ($curve:ty, $parity:expr, $read_inner:ident, $read_outer:ident) => {{
        type VkReader = fn(
            &[u8],
            BaseCircuitParams,
        )
            -> Result<VerifyingKey<$curve>, KagemushaArtifactGenerationErrorV1>;
        let readers: [(VkReader, &str); 2] = [
            ($read_inner, "inner mint-authority verifying key"),
            ($read_outer, "mint-authority verifying key"),
        ];
        let header = vk_header(0x02, KAGEMUSHA_HALO2_K_V1, 0, 1);
        for (reader, kind) in readers {
            for length in 0..header.len() {
                let result =
                    std::panic::catch_unwind(|| reader(&header[..length], test_base_params()));
                decode_error_reason(
                    result.expect("truncated header must not panic"),
                    $parity,
                    kind,
                );
            }
            // Each complete header is rejected before domain construction or
            // allocation of key commitments. No encoded point/body is supplied.
            let mut malformed = vec![
                (
                    vk_header(0, KAGEMUSHA_HALO2_K_V1, 0, 1),
                    "unexpected version byte",
                ),
                (
                    vk_header(0x02, KAGEMUSHA_HALO2_K_V1, 2, 1),
                    "unexpected compress_selectors not boolean",
                ),
                (
                    vk_header(0x02, KAGEMUSHA_HALO2_K_V1, 0, u32::MAX),
                    "fixed-column count does not match configured bounds",
                ),
            ];
            for k in [0, 6, 15, 17, u32::MAX] {
                malformed.push((
                    vk_header(0x02, k, 0, 1),
                    "key degree does not match expected domain",
                ));
            }
            for (bytes, expected_reason) in malformed {
                let result = std::panic::catch_unwind(|| reader(&bytes, test_base_params()));
                let reason = decode_error_reason(
                    result.expect("malformed header must not panic"),
                    $parity,
                    kind,
                );
                assert!(
                    reason.contains(expected_reason),
                    "unexpected rejection: {reason}"
                );
            }
        }
    }};
}

#[test]
fn checked_mint_authority_inner_outer_eq_vk_readers_reject_malformed_headers() {
    check_authority_vk_headers!(
        EqAffine,
        KagemushaPastaParityV1::Eq,
        read_eq_inner_mint_vk,
        read_eq_mint_vk
    );
}

#[test]
fn checked_mint_authority_inner_outer_ep_vk_readers_reject_malformed_headers() {
    check_authority_vk_headers!(
        EpAffine,
        KagemushaPastaParityV1::Ep,
        read_ep_inner_mint_vk,
        read_ep_mint_vk
    );
}
