use ff::Field as _;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        ff::PrimeField,
        pasta::{Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem},
};
use iroha_data_model::offline::{
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, OFFLINE_CASH_HALO2_K_V1,
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey,
    signature::{Signer as _, hazmat::PrehashSigner as _},
};

use super::{
    OfflineCashHalo2ParityV1, P256PackedStatementSourceV3,
    helper_abi::{
        ANDROID_CERTIFICATE_WORD_START, CURRENT_GUARD_WORD_START, HELPER_ABI_WORDS,
        HELPER_ANDROID_PRESENT_WORD, HELPER_INSTANCE_CELLS, HELPER_INSTANCE_CELLS_MAX,
        HELPER_PARITY_WORD, HELPER_PROTOCOL_WORD_START, HELPER_ROLE_WORD, HELPER_TO_LOW_WORD,
        HELPER_WORDS_PER_INSTANCE, NEXT_GUARD_WORD_START,
        OfflineCashGuardBundleChildPublicEqualityV1, OfflineCashHelperAbiErrorV1,
        OfflineCashHelperOperationV1, OfflineCashHelperPublicInstancesV1, RELEASE_WORD_START,
        pack_words_as_field,
    },
    helper_circuit::{
        OfflineCashEpAndroidKeyCertBindingCircuitV1, OfflineCashEpGuardBundleLeafBindingCircuitV1,
        OfflineCashEpGuardUseBindingCircuitV1, OfflineCashEpPlatformBindBindingCircuitV1,
        OfflineCashEqAndroidKeyCertBindingCircuitV1, OfflineCashEqGuardBundleLeafBindingCircuitV1,
        OfflineCashEqGuardUseBindingCircuitV1, OfflineCashEqPlatformBindBindingCircuitV1,
    },
    helper_relation::{
        OfflineCashAndroidKeyCertWitnessV1, OfflineCashHelperRelationInputV1,
        OfflineCashValidatedHelperRelationV1, guard_bindings_v1, platform_message_v1,
    },
    p256_packed_affine_ep_child_from_source_v3, p256_packed_affine_eq_child_from_source_v3,
    protocol::{
        OfflineCashHalo2CircuitRoleV1, OfflineCashRecursionActivationPreflightErrorV1,
        offline_cash_halo2_protocol_identity_v1, preflight_offline_cash_recursion_activation_v1,
    },
};
use crate::zk::pasta_ipa_recursion::{
    PastaIpaInstanceQueryV1, PastaIpaProofShapeV1, pasta_ipa_augmented_proof_shape_v1,
};

fn configured_helper_shape<F, C>(instance_query: PastaIpaInstanceQueryV1) -> PastaIpaProofShapeV1
where
    F: PrimeField,
    C: Circuit<F>,
{
    let mut constraints = ConstraintSystem::<F>::default();
    let _ = C::configure(&mut constraints);
    pasta_ipa_augmented_proof_shape_v1(&constraints, OFFLINE_CASH_HALO2_K_V1, instance_query)
        .expect("configured helper proof shape")
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes((&[seed; 32]).into()).expect("valid deterministic P-256 key")
}

fn device_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV2 {
    KagemushaDevicePublicKeyV2::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical uncompressed P-256 key")
}

fn device_signature(key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV2 {
    let signature: P256Signature = key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical low-S P-256 signature")
}

fn prehash_signature(key: &SigningKey, digest: &[u8; 32]) -> KagemushaDeviceSignatureV2 {
    let signature: P256Signature = key
        .sign_prehash(digest)
        .expect("valid deterministic P-256 prehash signature");
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical low-S P-256 prehash signature")
}

fn relation_input() -> OfflineCashHelperRelationInputV1 {
    OfflineCashHelperRelationInputV1 {
        operation: OfflineCashHelperOperationV1::SendSplit,
        release_id: [0x11; 32],
        context_digest: [0x12; 32],
        current_head: [0x13; 32],
        current_lineage_digest: [0x14; 32],
        transition_digest: [0x15; 32],
        wallet_binding: [0x16; 32],
        hardware_policy_id: [0x17; 32],
        guard_device_id: [0x18; 32],
        from_sequence: 7,
        to_sequence: 8,
    }
}

fn validated_relation(
    with_android: bool,
) -> Result<OfflineCashValidatedHelperRelationV1, OfflineCashHelperAbiErrorV1> {
    validated_relation_for_input(relation_input(), with_android)
}

fn validated_relation_for_input(
    input: OfflineCashHelperRelationInputV1,
    with_android: bool,
) -> Result<OfflineCashValidatedHelperRelationV1, OfflineCashHelperAbiErrorV1> {
    let (current, next) = guard_bindings_v1(&input);
    let message = platform_message_v1(&input, &current, &next)?;
    let platform_key = signing_key(7);
    let android = with_android.then(|| {
        let issuer = signing_key(9);
        let tbs_digest = [0xA2; 32];
        OfflineCashAndroidKeyCertWitnessV1::new(
            device_public_key(&issuer),
            prehash_signature(&issuer, &tbs_digest),
            [0xA1; 32],
            tbs_digest,
            [0xA3; 32],
        )
        .expect("valid normalized Android certificate")
    });
    OfflineCashValidatedHelperRelationV1::new(
        input,
        device_public_key(&platform_key),
        device_signature(&platform_key, &message),
        android,
    )
}

fn packed_fields<F: PrimeField>(words: &[u32; HELPER_ABI_WORDS]) -> Vec<F> {
    (0..HELPER_INSTANCE_CELLS)
        .map(|cell_index| {
            let start = cell_index * HELPER_WORDS_PER_INSTANCE;
            let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
            pack_words_as_field::<F>(&words[start..end])
        })
        .collect()
}

#[test]
fn helper_abi_is_184_words_and_27_field_neutral_cells() {
    assert_eq!(HELPER_ABI_WORDS, 184);
    assert_eq!(HELPER_WORDS_PER_INSTANCE, 7);
    assert_eq!(HELPER_INSTANCE_CELLS, 27);
    assert_eq!(HELPER_INSTANCE_CELLS_MAX, 32);
    assert!(Fp::CAPACITY >= 224);
    assert!(Fq::CAPACITY >= 224);

    let relation = validated_relation(true).expect("valid helper relation");
    assert!(relation.private_witness_is_retained_for_test());
    let eq = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )
        .expect("Eq GuardUse ABI");
    let ep = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
        )
        .expect("Ep PlatformBind ABI");
    assert_eq!(eq.statement(), Ok(relation.statement_for_test()));
    assert_eq!(ep.statement(), Ok(relation.statement_for_test()));
    for index in 0..HELPER_ABI_WORDS {
        if index == HELPER_PARITY_WORD
            || index == HELPER_ROLE_WORD
            || (HELPER_PROTOCOL_WORD_START..HELPER_PROTOCOL_WORD_START + 8).contains(&index)
        {
            continue;
        }
        assert_eq!(
            eq.words()[index],
            ep.words()[index],
            "semantic word {index}"
        );
    }
    assert_eq!(
        &eq.words()[HELPER_PROTOCOL_WORD_START..HELPER_PROTOCOL_WORD_START + 8],
        &offline_cash_halo2_protocol_identity_v1(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )
        .digest()
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("word")))
        .collect::<Vec<_>>()
    );

    let packed = eq.packed_cell_bytes();
    assert_eq!(
        OfflineCashHelperPublicInstancesV1::unpack_cell_bytes(&packed),
        Ok(*eq.words())
    );
    for (cell_index, bytes) in packed.iter().enumerate() {
        let start = cell_index * HELPER_WORDS_PER_INSTANCE;
        let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
        let fp = pack_words_as_field::<Fp>(&eq.words()[start..end]).to_repr();
        let fq = pack_words_as_field::<Fq>(&eq.words()[start..end]).to_repr();
        assert_eq!(&fp.as_ref()[..28], bytes);
        assert_eq!(&fq.as_ref()[..28], bytes);
        assert_eq!(&fp.as_ref()[..], &fq.as_ref()[..]);
    }
    assert!(
        packed.last().expect("last cell")[8..]
            .iter()
            .all(|byte| *byte == 0)
    );
    let mut noncanonical = packed;
    noncanonical[HELPER_INSTANCE_CELLS - 1][8] = 1;
    assert_eq!(
        OfflineCashHelperPublicInstancesV1::unpack_cell_bytes(&noncanonical),
        Err(OfflineCashHelperAbiErrorV1::NonCanonicalPacking)
    );
}

#[test]
fn private_relation_rejects_sequence_signature_key_and_android_substitution() {
    const P256_HALF_ORDER: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31,
        0x92, 0xa8,
    ];
    let mut high_s = P256_HALF_ORDER;
    high_s[31] += 1;
    let mut high_s_raw = [0_u8; 64];
    high_s_raw[31] = 1;
    high_s_raw[32..].copy_from_slice(&high_s);
    assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&high_s_raw).is_err());

    let platform_key = signing_key(7);
    let mut stale = relation_input();
    stale.to_sequence += 1;
    assert!(matches!(
        OfflineCashValidatedHelperRelationV1::new(
            stale,
            device_public_key(&platform_key),
            device_signature(&platform_key, b"wrong"),
            None,
        ),
        Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    ));

    let input = relation_input();
    let (current, next) = guard_bindings_v1(&input);
    let message = platform_message_v1(&input, &current, &next).expect("platform message");
    let wrong_key = signing_key(8);
    assert!(matches!(
        OfflineCashValidatedHelperRelationV1::new(
            input,
            device_public_key(&platform_key),
            device_signature(&wrong_key, &message),
            None,
        ),
        Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    ));
    assert!(KagemushaDevicePublicKeyV2::from_sec1_bytes(&[0x04; 65]).is_err());

    let issuer = signing_key(9);
    let signed_tbs = [0xA2; 32];
    let substituted_android = OfflineCashAndroidKeyCertWitnessV1::new(
        device_public_key(&issuer),
        prehash_signature(&issuer, &signed_tbs),
        [0xA1; 32],
        [0xA4; 32],
        [0xA3; 32],
    )
    .expect("structurally valid but substituted normalized certificate");
    assert!(matches!(
        OfflineCashValidatedHelperRelationV1::new(
            input,
            device_public_key(&platform_key),
            device_signature(&platform_key, &message),
            Some(substituted_android),
        ),
        Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    ));
}

#[test]
fn android_fixed_source_rejects_release_and_policy_mapping_substitution() {
    let input = relation_input();
    let (current, next) = guard_bindings_v1(&input);
    let message = platform_message_v1(&input, &current, &next).expect("platform message");
    let platform_key = signing_key(7);
    let make_android = |release_id, hardware_policy_digest| {
        let issuer = signing_key(9);
        let tbs_digest = [0xA2; 32];
        OfflineCashAndroidKeyCertWitnessV1::new(
            device_public_key(&issuer),
            prehash_signature(&issuer, &tbs_digest),
            [0xA1; 32],
            tbs_digest,
            [0xA3; 32],
        )
        .expect("valid normalized Android certificate")
        .bind_governance_for_test(release_id, hardware_policy_digest)
    };

    let _ = OfflineCashValidatedHelperRelationV1::new(
        input,
        device_public_key(&platform_key),
        device_signature(&platform_key, &message),
        Some(make_android(input.release_id, input.hardware_policy_id)),
    )
    .expect("exact release and policy mapping");

    let mut wrong_release = input.release_id;
    wrong_release[0] ^= 1;
    assert!(matches!(
        OfflineCashValidatedHelperRelationV1::new(
            input,
            device_public_key(&platform_key),
            device_signature(&platform_key, &message),
            Some(make_android(wrong_release, input.hardware_policy_id)),
        ),
        Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    ));

    let mut wrong_policy = input.hardware_policy_id;
    wrong_policy[0] ^= 1;
    assert!(matches!(
        OfflineCashValidatedHelperRelationV1::new(
            input,
            device_public_key(&platform_key),
            device_signature(&platform_key, &message),
            Some(make_android(input.release_id, wrong_policy)),
        ),
        Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    ));
}

#[test]
fn eq_and_ep_fixed_k16_guard_and_platform_binding_circuits_accept() {
    let relation = validated_relation(true).expect("valid helper relation");
    let eq_instances = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )
        .expect("Eq GuardUse instances");
    let eq_public = eq_instances.field_instances::<Fp>().to_vec();
    let eq_circuit =
        OfflineCashEqGuardUseBindingCircuitV1::new(&relation).expect("Eq GuardUse circuit");
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &eq_circuit, vec![eq_public])
        .expect("Eq helper synthesis")
        .assert_satisfied();

    let ep_instances = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
        )
        .expect("Ep PlatformBind instances");
    let ep_public = ep_instances.field_instances::<Fq>().to_vec();
    let ep_circuit =
        OfflineCashEpPlatformBindBindingCircuitV1::new(&relation).expect("Ep PlatformBind circuit");
    let ep_columns = ep_circuit.public_instance_columns();
    assert_eq!(ep_columns[0], ep_public);
    assert_eq!(ep_columns.len(), 2);
    assert_eq!(ep_columns[1].len(), 97);
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &ep_circuit, ep_columns)
        .expect("Ep helper synthesis")
        .assert_satisfied();
}

#[test]
fn eq_sha_recomputation_rejects_nonzero_public_digest_substitution() {
    let relation = validated_relation(false).expect("valid helper relation");
    let instances = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )
        .expect("Eq GuardUse instances");
    let mut words = *instances.words();
    words[RELEASE_WORD_START..RELEASE_WORD_START + 8].fill(0);
    words[RELEASE_WORD_START + 7] = 1;
    let public = packed_fields::<Fp>(&words);
    let circuit =
        OfflineCashEqGuardUseBindingCircuitV1::from_relation_and_words_for_test(&relation, words);
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
            .expect("Eq SHA substitution synthesis")
            .verify()
            .is_err(),
        "a nonzero but incorrect release digest must fail constrained SHA recomputation"
    );
}

#[test]
fn ep_sha_recomputation_rejects_private_platform_key_substitution() {
    let relation = validated_relation(false).expect("valid helper relation");
    let instances = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
        )
        .expect("Ep PlatformBind instances");
    let mut circuit =
        OfflineCashEpPlatformBindBindingCircuitV1::new(&relation).expect("Ep PlatformBind circuit");
    let public = circuit.public_instance_columns();
    assert_eq!(public[0], instances.field_instances::<Fq>().to_vec());
    circuit.mutate_platform_key_for_test();
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, public)
            .expect("Ep private-key substitution synthesis")
            .verify()
            .is_err(),
        "a private platform-key substitution must fail its public SHA-256 binding"
    );
}

#[test]
fn binding_circuit_rejects_role_sequence_digest_optionality_and_inequality_substitution() {
    let relation = validated_relation(false).expect("valid helper relation without Android cert");
    let instances = relation
        .public_instances(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )
        .expect("Eq GuardUse instances");

    let mut cases = Vec::new();
    let mut role = *instances.words();
    role[HELPER_ROLE_WORD] = OfflineCashHalo2CircuitRoleV1::PlatformBind as u32;
    cases.push(("role", role));
    let mut sequence = *instances.words();
    sequence[HELPER_TO_LOW_WORD] = sequence[HELPER_TO_LOW_WORD].wrapping_add(1);
    cases.push(("sequence", sequence));
    let mut required_zero = *instances.words();
    required_zero[RELEASE_WORD_START..RELEASE_WORD_START + 8].fill(0);
    cases.push(("required digest", required_zero));
    let mut absent_android = *instances.words();
    assert_eq!(absent_android[HELPER_ANDROID_PRESENT_WORD], 0);
    absent_android[ANDROID_CERTIFICATE_WORD_START] = 1;
    cases.push(("absent Android", absent_android));
    let mut equal_guards = *instances.words();
    equal_guards[NEXT_GUARD_WORD_START..NEXT_GUARD_WORD_START + 8].copy_from_slice(
        &instances.words()[CURRENT_GUARD_WORD_START..CURRENT_GUARD_WORD_START + 8],
    );
    cases.push(("equal guard bindings", equal_guards));

    for (label, words) in cases {
        let public = packed_fields::<Fp>(&words);
        let circuit = OfflineCashEqGuardUseBindingCircuitV1::from_relation_and_words_for_test(
            &relation, words,
        );
        assert!(
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
                .unwrap_or_else(|error| panic!("{label} synthesis failed unexpectedly: {error}"))
                .verify()
                .is_err(),
            "{label} substitution must violate the binding circuit"
        );
    }
}

#[test]
fn guard_bundle_child_public_equality_is_closed_for_eq_and_ep() {
    let relation = validated_relation(true).expect("valid helper relation");
    let without_android = validated_relation(false).expect("valid non-Android helper relation");
    let mut altered_input = relation_input();
    altered_input.release_id[0] ^= 1;
    let altered =
        validated_relation_for_input(altered_input, true).expect("valid altered helper relation");

    for parity in [OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2ParityV1::Ep] {
        let child_parity = parity;
        let reciprocal_parity = match parity {
            OfflineCashHalo2ParityV1::Eq => OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2ParityV1::Ep => OfflineCashHalo2ParityV1::Eq,
        };
        let bundle = relation
            .public_instances(parity, OfflineCashHalo2CircuitRoleV1::GuardBundle)
            .expect("bundle ABI");
        let guard = relation
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::GuardUse)
            .expect("guard ABI");
        let platform = relation
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::PlatformBind)
            .expect("platform ABI");
        let android = relation
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::AndroidKeyCert)
            .expect("Android ABI");
        let bundle_leaf = relation
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf)
            .expect("GuardBundleLeaf ABI");
        let boundary = OfflineCashGuardBundleChildPublicEqualityV1::new(
            &bundle,
            &guard,
            &platform,
            Some(&android),
            &bundle_leaf,
        )
        .expect("exact child equality boundary");
        assert_eq!(boundary.parent_parity(), parity);
        assert_eq!(boundary.child_parity(), child_parity);
        assert_eq!(boundary.statement(), relation.statement_for_test());
        assert!(boundary.android_child_present());

        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &guard,
                &platform,
                None,
                &bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &platform,
                &platform,
                Some(&android),
                &bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &guard,
                &platform,
                Some(&android),
                &guard,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));

        let altered_guard = altered
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::GuardUse)
            .expect("altered guard ABI");
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &altered_guard,
                &platform,
                Some(&android),
                &bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));

        let reciprocal_parity_guard = relation
            .public_instances(reciprocal_parity, OfflineCashHalo2CircuitRoleV1::GuardUse)
            .expect("reciprocal-parity guard ABI");
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &reciprocal_parity_guard,
                &platform,
                Some(&android),
                &bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));

        let mut protocol_swapped_guard = guard.clone();
        for index in HELPER_PROTOCOL_WORD_START..RELEASE_WORD_START {
            protocol_swapped_guard.overwrite_word_for_test(index, platform.words()[index]);
        }
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &bundle,
                &protocol_swapped_guard,
                &platform,
                Some(&android),
                &bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));

        for index in 0..HELPER_ABI_WORDS {
            if index == HELPER_PARITY_WORD
                || index == HELPER_ROLE_WORD
                || (HELPER_PROTOCOL_WORD_START..RELEASE_WORD_START).contains(&index)
            {
                continue;
            }
            let mut substituted = guard.clone();
            substituted.overwrite_word_for_test(index, guard.words()[index] ^ 1);
            assert!(
                matches!(
                    OfflineCashGuardBundleChildPublicEqualityV1::new(
                        &bundle,
                        &substituted,
                        &platform,
                        Some(&android),
                        &bundle_leaf,
                    ),
                    Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
                ),
                "semantic child word {index} substitution must fail"
            );
        }

        let absent_bundle = without_android
            .public_instances(parity, OfflineCashHalo2CircuitRoleV1::GuardBundle)
            .expect("absent-Android bundle ABI");
        let absent_guard = without_android
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::GuardUse)
            .expect("absent-Android guard ABI");
        let absent_platform = without_android
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::PlatformBind)
            .expect("absent-Android platform ABI");
        let absent_android = without_android
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::AndroidKeyCert)
            .expect("selector-aware absent Android ABI");
        let absent_bundle_leaf = without_android
            .public_instances(child_parity, OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf)
            .expect("absent-Android GuardBundleLeaf ABI");
        let absent_boundary = OfflineCashGuardBundleChildPublicEqualityV1::new(
            &absent_bundle,
            &absent_guard,
            &absent_platform,
            Some(&absent_android),
            &absent_bundle_leaf,
        )
        .expect("selector-aware Android child is canonically absent");
        assert!(!absent_boundary.android_child_present());
        assert!(matches!(
            OfflineCashGuardBundleChildPublicEqualityV1::new(
                &absent_bundle,
                &absent_guard,
                &absent_platform,
                Some(&android),
                &absent_bundle_leaf,
            ),
            Err(OfflineCashHelperAbiErrorV1::InvalidLayout)
        ));
    }
}

#[test]
fn p256_child_sources_enter_both_v3_candidates_without_opening_authority() {
    let relation = validated_relation(true).expect("valid helper relation");
    let mut exact_source = relation.platform_p256_child_statement_v3();
    let mut exact_frame = [0_u8; 161];
    exact_source
        .read_exact_statement(&mut exact_frame)
        .expect("one exact bounded statement read");
    assert_eq!(exact_frame[0], 0x04);
    assert_eq!(
        &exact_frame[65..97],
        &relation.statement_for_test().platform_message_digest
    );
    assert!(exact_source.read_exact_statement(&mut exact_frame).is_err());

    let _eq_platform =
        p256_packed_affine_eq_child_from_source_v3(relation.platform_p256_child_statement_v3())
            .expect("exact Eq platform P-256 candidate");
    let _ep_platform =
        p256_packed_affine_ep_child_from_source_v3(relation.platform_p256_child_statement_v3())
            .expect("exact Ep platform P-256 candidate");
    let _eq_android = p256_packed_affine_eq_child_from_source_v3(
        relation
            .android_p256_child_statement_v3()
            .expect("Android P-256 statement"),
    )
    .expect("exact Eq Android P-256 candidate");
    let _ep_android = p256_packed_affine_ep_child_from_source_v3(
        relation
            .android_p256_child_statement_v3()
            .expect("Android P-256 statement"),
    )
    .expect("exact Ep Android P-256 candidate");

    let without_android = validated_relation(false).expect("valid non-Android relation");
    assert!(without_android.android_p256_child_statement_v3().is_none());
    let _ = OfflineCashEqAndroidKeyCertBindingCircuitV1::new(&without_android)
        .expect("fixed-slot absent Eq Android leaf");
    let _ = OfflineCashEpAndroidKeyCertBindingCircuitV1::new(&without_android)
        .expect("fixed-slot absent Ep Android leaf");
}

#[test]
fn p256_aux_columns_bind_exact_key_and_digest_on_both_pasta_fields() {
    let relation = validated_relation(true).expect("valid helper relation");

    let eq_platform =
        OfflineCashEqPlatformBindBindingCircuitV1::new(&relation).expect("Eq PlatformBind");
    let eq_columns = eq_platform.public_instance_columns();
    assert_eq!(eq_columns.len(), 2);
    assert_eq!(eq_columns[1].len(), 97);
    let mut platform_statement = [0_u8; 161];
    relation
        .platform_p256_child_statement_v3()
        .read_exact_statement(&mut platform_statement)
        .expect("exact platform P-256 statement");
    assert_eq!(
        eq_columns[1],
        platform_statement[..97]
            .iter()
            .map(|byte| Fp::from(u64::from(*byte)))
            .collect::<Vec<_>>()
    );
    // The final 64 raw-signature bytes are intentionally absent: recursive
    // composition accepts any canonical low-S signature for this exact key and
    // prehash, while the P-256 child proves that signature's validity.
    assert_eq!(platform_statement.len() - eq_columns[1].len(), 64);
    let mut substituted_eq = eq_columns.clone();
    substituted_eq[1][1] += Fp::ONE;
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &eq_platform, substituted_eq)
            .expect("Eq auxiliary substitution synthesis")
            .verify()
            .is_err()
    );

    let ep_android =
        OfflineCashEpAndroidKeyCertBindingCircuitV1::new(&relation).expect("Ep AndroidKeyCert");
    let ep_columns = ep_android.public_instance_columns();
    assert_eq!(ep_columns.len(), 2);
    assert_eq!(ep_columns[1].len(), 97);
    let mut android_statement = [0_u8; 161];
    relation
        .android_p256_child_statement_v3()
        .expect("Android P-256 source")
        .read_exact_statement(&mut android_statement)
        .expect("exact Android P-256 statement");
    assert_eq!(
        ep_columns[1],
        android_statement[..97]
            .iter()
            .map(|byte| Fq::from(u64::from(*byte)))
            .collect::<Vec<_>>()
    );
    let mut substituted_ep = ep_columns.clone();
    substituted_ep[1][65] += Fq::ONE;
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &ep_android, substituted_ep)
            .expect("Ep auxiliary substitution synthesis")
            .verify()
            .is_err()
    );

    let absent = validated_relation(false).expect("valid absent-Android helper relation");
    let absent_android =
        OfflineCashEqAndroidKeyCertBindingCircuitV1::new(&absent).expect("absent Android leaf");
    let absent_columns = absent_android.public_instance_columns();
    assert_eq!(absent_columns.len(), 2);
    assert!(absent_columns[1].iter().all(|cell| *cell == Fp::ZERO));
}

#[test]
fn role_specialized_helper_shapes_are_exact_bounded_private_recursive_witnesses() {
    let queried_eq = configured_helper_shape::<Fp, OfflineCashEqGuardBundleLeafBindingCircuitV1>(
        PastaIpaInstanceQueryV1::Queried,
    );
    let queried_ep = configured_helper_shape::<Fq, OfflineCashEpGuardBundleLeafBindingCircuitV1>(
        PastaIpaInstanceQueryV1::Queried,
    );
    for shape in [&queried_eq, &queried_ep] {
        assert_eq!(shape.degree(), 9);
        assert_eq!(shape.advice_columns(), 23);
        assert_eq!(shape.instance_columns(), 1);
        assert_eq!(shape.advice_queries(), 43);
        assert_eq!(shape.instance_queries(), 1);
        assert_eq!(shape.fixed_queries(), 5);
        assert_eq!(shape.selectors(), 37);
        assert_eq!(shape.lookups(), 4);
        assert_eq!(shape.permutation_columns(), 22);
        assert_eq!(shape.permutation_chunks(), 4);
        assert_eq!(shape.point_sets(), 5);
        assert_eq!(shape.commitments(), 82);
        assert_eq!(shape.evaluations(), 147);
        assert_eq!(shape.transcript_elements(), 230);
        assert_eq!(shape.augmented_proof_bytes(), 7_360);
    }
    assert!(matches!(
        preflight_offline_cash_recursion_activation_v1(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            &queried_eq,
        ),
        Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidInstanceQuery {
                actual: PastaIpaInstanceQueryV1::Queried,
            }
        )
    ));

    let helper_roles = [
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            configured_helper_shape::<Fp, OfflineCashEqGuardUseBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            configured_helper_shape::<Fq, OfflineCashEpGuardUseBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
            configured_helper_shape::<Fp, OfflineCashEqPlatformBindBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
            configured_helper_shape::<Fq, OfflineCashEpPlatformBindBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
            configured_helper_shape::<Fp, OfflineCashEqAndroidKeyCertBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
            configured_helper_shape::<Fq, OfflineCashEpAndroidKeyCertBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            configured_helper_shape::<Fp, OfflineCashEqGuardBundleLeafBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            configured_helper_shape::<Fq, OfflineCashEpGuardBundleLeafBindingCircuitV1>(
                PastaIpaInstanceQueryV1::Direct,
            ),
        ),
    ];
    for (parity, role, shape) in helper_roles {
        let actual = shape.ordinary_proof_bytes();
        let (
            expected_advice,
            expected_queries,
            expected_selectors,
            expected_lookups,
            expected_instance_columns,
            expected_instance_queries,
            expected_permutation_columns,
            expected_permutation_chunks,
            expected_commitments,
            expected_evaluations,
            expected_elements,
            expected_ordinary_bytes,
        ) = if role == OfflineCashHalo2CircuitRoleV1::GuardUse {
            (34, 73, 59, 8, 1, 1, 30, 5, 106, 229, 336, 10_720)
        } else if matches!(
            role,
            OfflineCashHalo2CircuitRoleV1::PlatformBind
                | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
        ) {
            (23, 43, 37, 4, 2, 2, 23, 4, 82, 147, 230, 7_328)
        } else {
            (23, 43, 37, 4, 1, 1, 22, 4, 82, 146, 229, 7_296)
        };
        assert_eq!(shape.degree(), 9);
        assert_eq!(shape.advice_columns(), expected_advice);
        assert_eq!(shape.advice_queries(), expected_queries);
        assert_eq!(shape.instance_columns(), expected_instance_columns);
        assert_eq!(shape.instance_queries(), expected_instance_queries);
        assert_eq!(shape.selectors(), expected_selectors);
        assert_eq!(shape.lookups(), expected_lookups);
        assert_eq!(shape.permutation_columns(), expected_permutation_columns);
        assert_eq!(shape.permutation_chunks(), expected_permutation_chunks);
        assert_eq!(shape.point_sets(), 5);
        assert_eq!(shape.commitments(), expected_commitments);
        assert_eq!(shape.evaluations(), expected_evaluations);
        assert_eq!(shape.transcript_elements(), expected_elements);
        assert_eq!(actual, expected_ordinary_bytes);
        assert_eq!(shape.augmented_proof_bytes(), expected_ordinary_bytes + 32);
        assert!(preflight_offline_cash_recursion_activation_v1(parity, role, &shape).is_ok());
        let internal_max = super::protocol::offline_cash_internal_child_proof_max_bytes_v1(role)
            .expect("helper role has private recursive proof slot");
        assert!(actual <= internal_max);
    }
}

#[test]
fn all_role_parity_circuits_exist_while_activation_requires_authenticated_artifacts() {
    let relation = validated_relation(true).expect("valid helper relation");
    let _ = OfflineCashEqGuardUseBindingCircuitV1::new(&relation).expect("Eq GuardUse");
    let _ = OfflineCashEpGuardUseBindingCircuitV1::new(&relation).expect("Ep GuardUse");
    let _ = OfflineCashEqPlatformBindBindingCircuitV1::new(&relation).expect("Eq PlatformBind");
    let _ = OfflineCashEpPlatformBindBindingCircuitV1::new(&relation).expect("Ep PlatformBind");
    let _ = OfflineCashEqAndroidKeyCertBindingCircuitV1::new(&relation).expect("Eq AndroidKeyCert");
    let _ = OfflineCashEpAndroidKeyCertBindingCircuitV1::new(&relation).expect("Ep AndroidKeyCert");
    let _ =
        OfflineCashEqGuardBundleLeafBindingCircuitV1::new(&relation).expect("Eq GuardBundleLeaf");
    let _ =
        OfflineCashEpGuardBundleLeafBindingCircuitV1::new(&relation).expect("Ep GuardBundleLeaf");

    let backend = include_str!("halo2_backend.rs");
    assert!(backend.contains("PRODUCTION_ACTIVATION_BLOCKER_V1"));
    assert!(backend.contains("complete 34-artifact"));
    assert!(!backend.contains("HelperBindingCircuitV1"));
    assert!(!backend.contains("OfflineCashValidatedHelperRelationV1"));
    let circuit = include_str!("helper_circuit.rs");
    assert!(circuit.contains("const HELPER_SHA_JOBS_V1: usize = 9;"));
    assert!(circuit.contains("const GUARD_BUNDLE_LEAF_SHA_JOBS_V1: [usize; 1] = [8];"));
    assert!(circuit.contains("Table16Chip"));
    assert!(circuit.contains("p256_child_contract"));
    assert!(circuit.contains("authenticated-sibling-boundary"));
    assert!(!circuit.contains("verify_proof"));
    assert!(!circuit.contains("ecdsa_verify_no_pubkey_check"));
    let p256 = include_str!("p256_packed_affine_v3.rs");
    assert!(p256.contains("ordinary_proof_bytes: 4_544"));
    let protocol = include_str!("protocol.rs");
    assert!(protocol.contains("ordinary-poseidon4544/private-recursive-child-not-final-wire"));
    assert!(protocol.contains("bounded-cbor-der-keymint-x509-root-to-fixed-source"));
    assert!(protocol.contains("poseidon-direct-instance/ordinary-proof"));
    assert!(protocol.contains("reciprocal-serial-base-point-equations"));
    assert!(!protocol.contains(concat!("reciprocal-dense-", "msm-audit")));
    let relation_source = include_str!("helper_relation.rs");
    let declaration = relation_source
        .find("pub(super) struct OfflineCashValidatedHelperRelationV1")
        .expect("private relation declaration");
    let attributes = relation_source[..declaration]
        .rsplit_once("\n\n")
        .map_or("", |(_, attributes)| attributes);
    assert!(!attributes.contains("Clone"));
    assert!(relation_source.contains("impl Drop for OfflineCashHelperPrivateWitnessV1"));
    assert!(relation_source.contains("impl Drop for OfflineCashHelperCircuitWitnessV1"));
    assert!(relation_source.contains("impl Drop for OfflineCashP256ChildStatementSourceV3"));
    assert!(relation_source.contains("impl Drop for OfflineCashAndroidKeyCertWitnessV1"));
    assert!(relation_source.contains("from_governed_keymint"));
    let recursion_source = include_str!("helper_recursion.rs");
    assert!(recursion_source.contains("verify_poseidon_child_proof_native_v1"));
    assert!(recursion_source.contains("terminal_verify_eq_outer_and_carried_v1"));
    assert!(recursion_source.contains("terminal_verify_ep_outer_and_carried_v1"));
    assert!(!recursion_source.contains(concat!("verify_augmented_", "ipa_proof_v1")));
    assert!(!recursion_source.contains(concat!("decide_eq_", "history_v1")));
    assert!(!recursion_source.contains(concat!("decide_ep_", "history_v1")));
    let guard_recursion = include_str!("guard_bundle_recursion.rs");
    assert!(guard_recursion.contains("constrain_poseidon_child_proof_v1"));
    assert!(guard_recursion.contains("constrain_poseidon_folded_accumulator_instance_v1"));
    assert!(guard_recursion.contains("constrain_poseidon_reciprocal_audit_serial_v1"));
    assert!(guard_recursion.contains("OfflineCashPackedBaseTraceV1"));
    assert!(
        guard_recursion
            .contains("native pre-verification result nor an unclosed deferred equation")
    );
    assert!(relation_source.matches(".zeroize();").count() >= 7);
}
