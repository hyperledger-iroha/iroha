use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        ff::PrimeField,
        pasta::{Fp, Fq},
    },
};
use iroha_data_model::offline::{
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, OFFLINE_CASH_HALO2_K_V1,
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey,
    signature::{Signer as _, hazmat::PrehashSigner as _},
};

use super::{
    OfflineCashHalo2ParityV1,
    helper_abi::{
        ANDROID_CERTIFICATE_WORD_START, CURRENT_GUARD_WORD_START, HELPER_ABI_WORDS,
        HELPER_ANDROID_PRESENT_WORD, HELPER_INSTANCE_CELLS, HELPER_INSTANCE_CELLS_MAX,
        HELPER_PROTOCOL_WORD_START, HELPER_ROLE_WORD, HELPER_TO_LOW_WORD,
        HELPER_WORDS_PER_INSTANCE, NEXT_GUARD_WORD_START, OfflineCashHelperAbiErrorV1,
        OfflineCashHelperOperationV1, OfflineCashHelperPublicInstancesV1, RELEASE_WORD_START,
        pack_words_as_field,
    },
    helper_circuit::{
        OfflineCashEpAndroidKeyCertBindingCircuitV1, OfflineCashEpGuardBundleBindingCircuitV1,
        OfflineCashEpGuardUseBindingCircuitV1, OfflineCashEpPlatformBindBindingCircuitV1,
        OfflineCashEqAndroidKeyCertBindingCircuitV1, OfflineCashEqGuardBundleBindingCircuitV1,
        OfflineCashEqGuardUseBindingCircuitV1, OfflineCashEqPlatformBindBindingCircuitV1,
    },
    helper_relation::{
        OfflineCashAndroidKeyCertWitnessV1, OfflineCashHelperRelationInputV1,
        OfflineCashValidatedHelperRelationV1, guard_bindings_v1, platform_message_v1,
    },
    protocol::{OfflineCashHalo2CircuitRoleV1, offline_cash_halo2_protocol_identity_v1},
};

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
    let input = relation_input();
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
        if index == 3
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
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &ep_circuit, vec![ep_public])
        .expect("Ep helper synthesis")
        .assert_satisfied();
}

#[test]
fn digest_presence_spans_eight_words_across_seven_word_public_cells() {
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
    let circuit = OfflineCashEqGuardUseBindingCircuitV1::from_words_for_test(words);
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
        .expect("eight-limb digest synthesis")
        .assert_satisfied();
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
        let circuit = OfflineCashEqGuardUseBindingCircuitV1::from_words_for_test(words);
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
fn all_role_parity_circuit_types_are_constructible_but_non_authorizing() {
    let relation = validated_relation(true).expect("valid helper relation");
    let _ = OfflineCashEqGuardUseBindingCircuitV1::new(&relation).expect("Eq GuardUse");
    let _ = OfflineCashEpGuardUseBindingCircuitV1::new(&relation).expect("Ep GuardUse");
    let _ = OfflineCashEqPlatformBindBindingCircuitV1::new(&relation).expect("Eq PlatformBind");
    let _ = OfflineCashEpPlatformBindBindingCircuitV1::new(&relation).expect("Ep PlatformBind");
    let _ = OfflineCashEqAndroidKeyCertBindingCircuitV1::new(&relation).expect("Eq AndroidKeyCert");
    let _ = OfflineCashEpAndroidKeyCertBindingCircuitV1::new(&relation).expect("Ep AndroidKeyCert");
    let _ = OfflineCashEqGuardBundleBindingCircuitV1::new(&relation).expect("Eq GuardBundle");
    let _ = OfflineCashEpGuardBundleBindingCircuitV1::new(&relation).expect("Ep GuardBundle");

    let backend = include_str!("halo2_backend.rs");
    assert!(backend.contains("VerificationUnavailable"));
    assert!(!backend.contains("HelperBindingCircuitV1"));
    assert!(!backend.contains("OfflineCashValidatedHelperRelationV1"));
    let circuit = include_str!("helper_circuit.rs");
    assert!(circuit.contains("not yet prove P-256 ECDSA"));
    assert!(!circuit.contains("verify_proof"));
    assert!(!circuit.contains("ecdsa_verify_no_pubkey_check"));
    let protocol = include_str!("protocol.rs");
    assert!(protocol.contains("p256-circuit+der-keymint+child-ipa-recursion-deferred"));
    let relation_source = include_str!("helper_relation.rs");
    let declaration = relation_source
        .find("pub(super) struct OfflineCashValidatedHelperRelationV1")
        .expect("private relation declaration");
    let attributes = relation_source[..declaration]
        .rsplit_once("\n\n")
        .map_or("", |(_, attributes)| attributes);
    assert!(!attributes.contains("Clone"));
    assert!(relation_source.contains("impl Drop for OfflineCashHelperPrivateWitnessV1"));
    assert!(relation_source.contains("impl Drop for OfflineCashAndroidKeyCertWitnessV1"));
    assert!(relation_source.matches(".zeroize();").count() >= 7);
}
