//! Exact child authority, replay, signature and canonical-wire regressions.

use super::*;
use crate::{
    isi::{InstructionBox, child_runtime::AuthorizeChildRuntimeActionV1},
    runtime::lifecycle::RuntimeNoticeV1,
};
use iroha_crypto::{Algorithm, HashOf};

fn fixture() -> (ChildRuntimeAdminStateV1, Vec<KeyPair>) {
    let mut keys: Vec<_> = (1..=3)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    let root = ChildRootIdentityV1 {
        network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"child"))),
        parent_network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"parent",
        ))),
        dataspace: DataSpaceId::new((1 << 40) + 7),
        instance: [8; 32],
    };
    let notice = RuntimeNoticeV1 {
        blocks: 2,
        ms: 100,
        approval_cushion_ms: 10,
    };
    (
        ChildRuntimeAdminStateV1 {
            root,
            policy: ChildRuntimeAdminPolicyV1 {
                revision: 1,
                admins: keys.iter().map(|key| key.public_key().clone()).collect(),
                threshold: 2,
                notice: RuntimeUpgradePolicyV1 {
                    activation: notice,
                    disable: notice,
                },
                provenance: None,
            },
            next_nonce: 0,
        },
        keys,
    )
}

fn certificate(
    state: &ChildRuntimeAdminStateV1,
    keys: &[KeyPair],
) -> ChildRuntimeAdminCertificateV1 {
    let plan = ChildRuntimeAdminPlanV1 {
        root: state.root,
        policy_id: state.policy.id().unwrap(),
        policy_revision: state.policy.revision,
        nonce: state.next_nonce,
        valid_from: 5,
        valid_until: 10,
        action: ChildRuntimeAdminActionV1::Cancel(ChangeSetId([9; 32])),
    };
    let signatures = keys
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&plan, key).unwrap())
        .collect();
    ChildRuntimeAdminCertificateV1 { plan, signatures }
}

#[test]
fn policy_and_certificate_admit_sixty_four_distinct_signers_and_refuse_sixty_five() {
    let (mut state, _) = fixture();
    let mut keys: Vec<_> = (1..=65)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    state.policy.admins = keys[..64]
        .iter()
        .map(|key| key.public_key().clone())
        .collect();
    state.policy.threshold = 64;
    state.policy.validate().unwrap();
    let mut signed = certificate(&state, &keys[..64]);
    signed.verify_against(&state, 5).unwrap();

    signed
        .signatures
        .push(ChildAdminSignatureV1::sign(&signed.plan, &keys[64]).unwrap());
    assert!(matches!(
        signed.verify_signatures(),
        Err(ChildAdminErrorV1::Shape(
            "invalid certificate signer order or count"
        ))
    ));
    state.policy.admins.push(keys[64].public_key().clone());
    assert!(matches!(
        state.policy.validate(),
        Err(ChildAdminErrorV1::Shape("invalid admin roster or revision"))
    ));
}

#[test]
fn foundation_reader_checks_json_size_before_decoding_the_recipe() {
    use super::foundation::{ChildRuntimeFoundationV1, MAX_CHILD_FOUNDATION_BYTES_V1};
    use crate::parameter::CustomParameter;
    use iroha_primitives::json::Json;

    for size in [
        MAX_CHILD_FOUNDATION_BYTES_V1,
        MAX_CHILD_FOUNDATION_BYTES_V1 + 1,
    ] {
        let payload = Json::try_new("x".repeat(size - 2)).unwrap();
        assert_eq!(payload.get().len(), size);
        let parameter =
            CustomParameter::new(ChildRuntimeFoundationV1::parameter_id().clone(), payload);
        let result = ChildRuntimeFoundationV1::from_custom_parameter(&parameter);
        if size == MAX_CHILD_FOUNDATION_BYTES_V1 {
            // At the limit the reader reaches the decoder, which requires a recipe object.
            assert!(matches!(result, Err(ChildAdminErrorV1::Resource(_))));
        } else {
            assert!(matches!(
                result,
                Err(ChildAdminErrorV1::Shape(
                    "invalid child foundation parameter identity or size"
                ))
            ));
        }
    }
}

#[test]
fn exact_threshold_certificate_and_instruction_round_trip() {
    let (state, keys) = fixture();
    let certificate = certificate(&state, &keys[..2]);
    certificate.verify_against(&state, 5).unwrap();
    certificate.verify_against(&state, 9).unwrap();
    let bytes = certificate.to_bytes().unwrap();
    assert_eq!(
        ChildRuntimeAdminCertificateV1::from_bytes(&bytes).unwrap(),
        certificate
    );
    let json = norito::json::to_json(&certificate).unwrap();
    assert_eq!(
        norito::json::from_str::<ChildRuntimeAdminCertificateV1>(&json).unwrap(),
        certificate
    );
    let boxed: InstructionBox = AuthorizeChildRuntimeActionV1 {
        certificate: certificate.clone(),
    }
    .into();
    assert_eq!(
        crate::isi::instruction_wire_id(&boxed),
        Some(AuthorizeChildRuntimeActionV1::WIRE_ID)
    );
    let encoded = norito::to_bytes(&boxed).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<InstructionBox>(&encoded).unwrap(),
        boxed
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(ChildRuntimeAdminCertificateV1::from_bytes(&trailing).is_err());
    let mut malformed = vec![0; MAX_CHILD_ADMIN_CERTIFICATE_BYTES_V1];
    assert!(matches!(
        ChildRuntimeAdminCertificateV1::from_bytes(&malformed),
        Err(ChildAdminErrorV1::Resource(_))
    ));
    malformed.push(0);
    assert!(matches!(
        ChildRuntimeAdminCertificateV1::from_bytes(&malformed),
        Err(ChildAdminErrorV1::TooLarge)
    ));
    assert_eq!(
        certificate.plan.id().unwrap(),
        certificate.plan.clone().id().unwrap()
    );
}

#[test]
fn root_policy_nonce_and_height_cannot_be_replayed() {
    let (state, keys) = fixture();
    let certificate = certificate(&state, &keys[..2]);
    for mutate in [
        |state: &mut ChildRuntimeAdminStateV1| state.root.instance[0] ^= 1,
        |state: &mut ChildRuntimeAdminStateV1| state.root.dataspace = DataSpaceId::new(7),
        |state: &mut ChildRuntimeAdminStateV1| state.root.parent_network = state.root.network,
        |state: &mut ChildRuntimeAdminStateV1| state.root.network = state.root.parent_network,
        |state: &mut ChildRuntimeAdminStateV1| state.policy.revision += 1,
        |state: &mut ChildRuntimeAdminStateV1| state.policy.threshold = 1,
        |state: &mut ChildRuntimeAdminStateV1| state.next_nonce += 1,
        |state: &mut ChildRuntimeAdminStateV1| state.next_nonce = u64::MAX,
    ] {
        let mut altered = state.clone();
        mutate(&mut altered);
        assert!(certificate.verify_against(&altered, 6).is_err());
    }
    assert!(matches!(
        certificate.verify_against(&state, 4),
        Err(ChildAdminErrorV1::Validity)
    ));
    assert!(matches!(
        certificate.verify_against(&state, 10),
        Err(ChildAdminErrorV1::Validity)
    ));
    let mut altered = certificate.clone();
    altered.plan.nonce += 1;
    assert!(altered.verify_signatures().is_err());
    altered = certificate;
    altered.plan.action = ChildRuntimeAdminActionV1::Cancel(ChangeSetId([1; 32]));
    assert!(altered.verify_signatures().is_err());
}

#[test]
fn every_signature_is_distinct_valid_and_currently_authorized() {
    let (state, keys) = fixture();
    assert!(matches!(
        certificate(&state, &keys[..1]).verify_against(&state, 6),
        Err(ChildAdminErrorV1::Threshold)
    ));
    let mut signed = certificate(&state, &keys[..2]);
    signed.signatures.push(signed.signatures[0].clone());
    assert!(signed.verify_signatures().is_err());
    signed = certificate(&state, &keys[..2]);
    signed.signatures.reverse();
    assert!(signed.verify_signatures().is_err());
    signed = certificate(&state, &keys);
    let wrong = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    signed.signatures[2].signature =
        Signature::try_new(wrong.private_key(), &signed.plan.signing_bytes().unwrap()).unwrap();
    assert!(matches!(
        signed.verify_against(&state, 6),
        Err(ChildAdminErrorV1::BadSignature(2))
    ));
    signed = certificate(&state, &keys[..2]);
    signed
        .signatures
        .push(ChildAdminSignatureV1::sign(&signed.plan, &wrong).unwrap());
    signed.signatures.sort_by(|a, b| a.signer.cmp(&b.signer));
    assert!(matches!(
        signed.verify_against(&state, 6),
        Err(ChildAdminErrorV1::UnauthorizedSigner)
    ));
}

#[test]
fn authority_has_no_implicit_defaults_and_rotation_advances_once() {
    let (state, keys) = fixture();
    for mutate in [
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.admins.clear(),
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.admins.reverse(),
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.admins.push(policy.admins[0].clone()),
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.threshold = 0,
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.threshold = 4,
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.revision = 0,
        |policy: &mut ChildRuntimeAdminPolicyV1| policy.notice.activation.blocks = 0,
    ] {
        let mut altered = state.policy.clone();
        mutate(&mut altered);
        assert!(altered.validate().is_err());
    }
    let mut rotation = certificate(&state, &keys[..2]);
    let mut next = state.policy.clone();
    next.revision += 1;
    next.threshold = 1;
    rotation.plan.action = ChildRuntimeAdminActionV1::RotatePolicy(next.clone());
    rotation.signatures = keys[..2]
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&rotation.plan, key).unwrap())
        .collect();
    rotation.verify_against(&state, 6).unwrap();
    next.revision += 1;
    rotation.plan.action = ChildRuntimeAdminActionV1::RotatePolicy(next);
    assert!(rotation.plan.validate().is_err());
    let mut root = state.root;
    root.dataspace = DataSpaceId::UNIVERSAL;
    assert!(root.validate().is_err());
    root = state.root;
    root.instance = [0; 32];
    assert!(root.validate().is_err());
    assert_eq!(state.root.scope().dataspace_id(), state.root.dataspace);
}

#[test]
fn streamed_policy_and_plan_ids_match_original_frames_without_output_or_scratch_allocation() {
    use crate::amx_prepare_streaming_allocations::measured;
    use norito::core::{DecodeBudgetContext, WireLayout, WireLayoutGuard};

    let (state, keys) = fixture();
    let plan = certificate(&state, &keys[..2]).plan;
    let mut rotation = plan.clone();
    let mut next = state.policy.clone();
    next.revision += 1;
    rotation.action = ChildRuntimeAdminActionV1::RotatePolicy(next);
    let original = |domain: &[u8], frame: Vec<u8>| -> [u8; 32] {
        Hash::new_from_chunks(&[domain, &frame]).into()
    };
    let expected_policy = original(
        CHILD_ADMIN_POLICY_DOMAIN_V1,
        norito::encode_canonical(&state.policy).unwrap(),
    );
    let expected_plan = original(
        CHILD_ADMIN_PLAN_DOMAIN_V1,
        norito::encode_canonical(&plan).unwrap(),
    );
    let expected_rotation = original(
        CHILD_ADMIN_PLAN_DOMAIN_V1,
        norito::encode_canonical(&rotation).unwrap(),
    );
    // Construct the accounting control before observing the production identity calls. The
    // physical allocator census also detects uncharged ByteSink/Vec output allocations.
    let context = DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        64,
    ));
    for layout in [WireLayout::Fixed, WireLayout::Compact, WireLayout::Dense] {
        let _layout = WireLayoutGuard::enter(layout);
        let ((policy, plan, rotation), requests) =
            context.with(|| measured(|| (state.policy.id(), plan.id(), rotation.id())));
        assert_eq!(
            requests, [0; 3],
            "identity hashing allocated owned output or scratch"
        );
        assert_eq!(policy.unwrap(), expected_policy);
        assert_eq!(plan.unwrap(), expected_plan);
        assert_eq!(rotation.unwrap(), expected_rotation);
        assert_eq!(context.consumed_allocated_bytes(), 0);
    }
}

#[test]
fn enclosing_codec_refusal_retains_its_original_retry_classification() {
    // A HashMap serializer genuinely needs sorted-entry scratch. The ordinary policy
    // does not, so zero allocation credit must not manufacture a policy refusal.
    let payload = std::collections::HashMap::from([(3_u64, 7_u64), (1, 9)]);
    let expected: [u8; 32] = Hash::new_from_chunks(&[
        CHILD_ADMIN_POLICY_DOMAIN_V1,
        &norito::encode_canonical(&payload).unwrap(),
    ])
    .into();
    let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        64,
    ));
    let result = context.with(|| digest(CHILD_ADMIN_POLICY_DOMAIN_V1, &payload));
    let Err(ChildAdminErrorV1::Resource(original)) = result else {
        panic!("digest must retain the original serializer scratch refusal");
    };
    assert_eq!(
        original.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    let retained = original.local_refusal().unwrap();
    drop(context);
    assert_eq!(original.into_local_refusal().unwrap(), retained);
    assert_eq!(
        digest(CHILD_ADMIN_POLICY_DOMAIN_V1, &payload).unwrap(),
        expected
    );
}

#[test]
fn signature_preimage_reserves_its_complete_domain_and_frame_in_original_scope() {
    let (state, keys) = fixture();
    let plan = certificate(&state, &keys[..2]).plan;
    let limits = |bytes| norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, 64);
    let (encoded, usage) =
        norito::core::with_decode_limits_measured(limits(usize::MAX), || plan.to_bytes());
    let frame = encoded.unwrap();
    let required =
        usage.total_allocated_bytes() + CHILD_ADMIN_SIGNATURE_DOMAIN_V1.len() + frame.len();
    let (signed, usage) =
        norito::core::with_decode_limits_measured(limits(required), || plan.signing_bytes());
    let signed = signed.unwrap();
    assert_eq!(usage.total_allocated_bytes(), required);
    assert_eq!(
        &signed[..CHILD_ADMIN_SIGNATURE_DOMAIN_V1.len()],
        CHILD_ADMIN_SIGNATURE_DOMAIN_V1
    );
    assert_eq!(&signed[CHILD_ADMIN_SIGNATURE_DOMAIN_V1.len()..], frame);
    let refused =
        norito::core::with_decode_limits_scope(limits(required - 1), || plan.signing_bytes());
    let Err(ChildAdminErrorV1::Resource(original)) = refused else {
        panic!("preimage allocation must retain original refusal");
    };
    assert!(original.into_local_refusal().is_ok());
}
