//! Counter codec/signature controls; synthetic fixture counts do not qualify World computation.

use super::*;
use crate::{
    account::{MultisigMember, MultisigPolicy},
    isi::Log,
    level::Level,
};
use iroha_crypto::Algorithm;
use iroha_model_base::topology::DataSpaceId;

fn network(tag: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(tag)))
}

fn scope() -> SumeragiRootScope {
    SumeragiRootScope::Dataspace {
        parent_network_id: network(b"counter fixture parent"),
        dataspace_id: DataSpaceId::new(u64::MAX - 1),
    }
}

fn reader_key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn semantic(action: u16) -> CounterSemanticV1 {
    CounterSemanticV1 {
        action_id: action,
        case_id: 1,
        step: action,
        role: CounterTransactionRoleV1::Business,
        category: CounterCategoryV1::Transfer,
        party: CounterPartyV1::Psp1,
        result: CounterResultV1::Applied,
        rejection: CounterRejectionV1::None,
        instruction_movements: vec![CounterCategoryV1::Transfer],
        instruction_parties: vec![CounterPartyV1::Psp1],
    }
}

fn executable_binding(action: u16, authority: &AccountId) -> CounterExecutableBindingV1 {
    // These are synthetic codec expectations, not approved business execution. Core's executed
    // substitution control uses real approved action originals and refuses an unrelated Log.
    let executable = Executable::from([Log::new(Level::INFO, format!("fixture action {action}"))]);
    CounterExecutableBindingV1 {
        action_id: action,
        authority: authority.clone(),
        executable_hash: HashOf::try_new(&executable).unwrap(),
    }
}

fn policy(selected_network: NetworkId) -> PrivateCountersPolicyV1 {
    let readers = [17, 18, 19].map(|seed| AccountId::new(reader_key(seed).public_key().clone()));
    let mut authorities = readers.to_vec();
    authorities.sort();
    let contract =
        ContractAddress::derive(&selected_network, &readers[0], 0, scope().dataspace_id()).unwrap();
    let run_binding = CounterRunBindingV1::Connected {
        definition_id: "boiw_13".into(),
        logical_baseline_hash: [1; 32],
        session_hash: [2; 32],
        provider_generation_hash: [3; 32],
    };
    let expected_executables = vec![
        executable_binding(1, &readers[0]),
        executable_binding(2, &readers[0]),
    ];
    let plan_hash =
        counter_plan_commitment_v1(&[semantic(1), semantic(2)], &expected_executables).unwrap();
    PrivateCountersPolicyV1 {
        version: 1,
        network_id: selected_network,
        scope: scope(),
        purpose: CounterPurposeV1::ConnectedProvider,
        run_id: run_binding.commitment().unwrap(),
        run_binding,
        readers: readers.to_vec(),
        authorities,
        plan_hash,
        expected_executables,
        contracts: vec![[4; 32]],
        contract_errors: vec![CounterContractErrorV1 {
            contract_address: contract,
            contract: "counter_fixture".into(),
            error_type: "fixture/counter@1::counter_fixture::refusal".into(),
            schema_hash: [5; 32],
            name: "holding_limit_exceeded".into(),
            code: 7,
            code_hash: Hash::new(b"fixture native code identity"),
            artifact_hash: [4; 32],
            rejection: CounterRejectionV1::HoldingLimitExceeded,
        }],
        first_height: 2,
        last_height: MAX_PRIVATE_COUNTER_HEIGHT_V1,
        limits: CounterLimitsV1 {
            max_entries: 1024,
            max_groups: 256,
            max_carrier_work: 10_000,
            max_total_work: 10_000,
            max_source_bytes: 8 * 1024 * 1024,
            max_retained_bytes: 8 * 1024 * 1024,
            max_time_to_live_ms: 1_000,
            max_clock_skew_ms: 5,
            max_signature_age_ms: 500,
        },
    }
}

fn structural_cut() -> CounterCutV1 {
    CounterCutV1 {
        height: 2,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fixture header")),
        context_id: Hash::new(b"fixture decision"),
        world_root: Hash::new(b"synthetic fixture World root"),
        epoch_context_id: [6; 32],
    }
}

fn manifest(policy: &PrivateCountersPolicyV1) -> PrivateCountersManifestV1 {
    let mut entries = (1_u16..=2)
        .map(|action| CounterManifestEntryV1 {
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::new(action.to_le_bytes())),
            block_height: 2,
            authority: policy.readers[0].clone(),
            semantic: semantic(action),
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.entrypoint_hash);
    PrivateCountersManifestV1 {
        version: 1,
        network_id: policy.network_id,
        scope: policy.scope,
        purpose: policy.purpose,
        run_id: policy.run_id,
        policy_hash: policy.commitment().unwrap(),
        entries,
    }
}

fn request(
    policy: &PrivateCountersPolicyV1,
    cut: CounterCutV1,
    time: u64,
) -> SignedPrivateCountersRequestV1 {
    PrivateCountersRequestV1 {
        domain: PRIVATE_COUNTER_REQUEST_DOMAIN_V1,
        version: 1,
        network_id: policy.network_id,
        scope: policy.scope,
        authority: policy.readers[0].clone(),
        purpose: policy.purpose,
        policy_hash: policy.commitment().unwrap(),
        manifest_hash: manifest(policy).commitment().unwrap(),
        cut,
        creation_time_ms: time,
        time_to_live_ms: NonZeroU64::new(100).unwrap(),
        nonce: [7; 32],
    }
    .try_sign(&reader_key(17))
    .unwrap()
}

fn claim(request: &SignedPrivateCountersRequestV1, block_time: u64) -> PrivateCountersClaimV1 {
    PrivateCountersClaimV1 {
        version: 1,
        network_id: request.payload.network_id,
        scope: request.payload.scope,
        request_hash: request.original_hash().unwrap(),
        authority: request.payload.authority.clone(),
        reader: CounterReaderV1::Operator,
        purpose: request.payload.purpose,
        policy_hash: request.payload.policy_hash,
        manifest_hash: request.payload.manifest_hash,
        cut: request.payload.cut.clone(),
        certified_block_time_ms: block_time,
        nonce: request.payload.nonce,
        groups: vec![CounterGroupV1 {
            key: CounterGroupKeyV1 {
                party: CounterPartyV1::Psp1,
                role: CounterTransactionRoleV1::Business,
                category: CounterCategoryV1::Transfer,
                result: CounterResultV1::Applied,
                rejection: CounterRejectionV1::None,
            },
            count: 2,
        }],
    }
}

fn member(
    claim: &PrivateCountersClaimV1,
    index: u16,
    key: &KeyPair,
    now: u64,
) -> CounterMemberAttestationV1 {
    let body = CounterMemberBodyV1 {
        domain: PRIVATE_COUNTER_MEMBER_DOMAIN_V1,
        version: 1,
        claim_hash: claim.commitment().unwrap(),
        member_index: index,
        observed_at_ms: now,
    };
    let signature =
        Signature::try_new(key.private_key(), &body.signing_preimage().unwrap()).unwrap();
    CounterMemberAttestationV1 { body, signature }
}

#[test]
fn all_original_frames_roundtrip_in_both_native_codecs() {
    let policy = policy(network(b"codec fixture child"));
    let manifest = manifest(&policy);
    let request = request(&policy, structural_cut(), 1_000);
    let claim = claim(&request, 999);
    let key = KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal);
    let attestation = member(&claim, 0, &key, 1_000);
    let response = PrivateCountersResponseV1 {
        claim: claim.clone(),
        attestation: attestation.clone(),
    };
    let certificate = PrivateCountersCertificateV1 {
        claim,
        attestations: vec![attestation],
    };
    macro_rules! roundtrip {
        ($value:expr, $ty:ty) => {{
            let value = $value;
            let wire = value.encode_canonical().unwrap();
            assert_eq!(<$ty>::decode_bounded_canonical(&wire).unwrap(), value);
            let json = norito::json::to_json(&value).unwrap();
            assert_eq!(norito::json::from_str::<$ty>(&json).unwrap(), value);
            let mut trailing = wire.clone();
            trailing.push(0);
            assert!(<$ty>::decode_bounded_canonical(&trailing).is_err());
            assert!(<$ty>::decode_bounded_canonical(&wire[..wire.len() - 1]).is_err());
        }};
    }
    roundtrip!(policy, PrivateCountersPolicyV1);
    roundtrip!(manifest, PrivateCountersManifestV1);
    roundtrip!(request, SignedPrivateCountersRequestV1);
    roundtrip!(response, PrivateCountersResponseV1);
    roundtrip!(certificate, PrivateCountersCertificateV1);
}

#[test]
fn frames_are_bounded_before_decode_or_output_allocation() {
    assert_eq!(
        canonical(
            &vec![0_u8; MAX_PRIVATE_COUNTER_FRAME_BYTES_V1],
            MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
        ),
        Err(PrivateCountersErrorV1::Bounds)
    );
    assert_eq!(
        SignedPrivateCountersRequestV1::decode_bounded_canonical(&[]),
        Err(PrivateCountersErrorV1::Bounds)
    );
    assert_eq!(
        SignedPrivateCountersRequestV1::decode_bounded_canonical(&vec![
            0;
            MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
                + 1
        ]),
        Err(PrivateCountersErrorV1::Bounds)
    );
    let policy = policy(network(b"budget fixture child"));
    let wire = policy.encode_canonical().unwrap();
    let zero = norito::DecodeLimits::new(1024, wire.len(), 131_072, 0, 32);
    assert!(
        norito::core::with_decode_limits_scope(zero, || {
            PrivateCountersPolicyV1::decode_bounded_canonical(&wire)
        })
        .is_err()
    );
    assert_eq!(
        frame_decode_limits(MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1).max_total_allocated_bytes(),
        16 * 1024 * 1024
    );
    assert_eq!(
        frame_decode_limits(MAX_PRIVATE_COUNTER_FRAME_BYTES_V1).max_sequence_elements(),
        1024
    );
}

#[test]
fn original_run_bindings_preserve_external_sha_bytes_and_purpose() {
    let connected = policy(network(b"binding child")).run_binding;
    assert_eq!(connected.purpose(), CounterPurposeV1::ConnectedProvider);
    let interactions = CounterRunBindingV1::Interactions {
        run_id: "run_1".into(),
        run_namespace: "selected".into(),
        definition_id: "boiw_13".into(),
        definition_hash: [8; 32],
        bindings_hash: [9; 32],
    };
    assert_eq!(
        interactions.purpose(),
        CounterPurposeV1::WalkthroughInteractions
    );
    assert_ne!(
        connected.commitment().unwrap(),
        interactions.commitment().unwrap()
    );
    let mut absent = interactions.clone();
    if let CounterRunBindingV1::Interactions {
        definition_hash, ..
    } = &mut absent
    {
        *definition_hash = [0; 32];
    }
    assert!(absent.validate().is_err());
    let mut changed = interactions.clone();
    if let CounterRunBindingV1::Interactions {
        definition_hash, ..
    } = &mut changed
    {
        definition_hash[31] ^= 1;
    }
    assert_ne!(
        interactions.commitment().unwrap(),
        changed.commitment().unwrap()
    );
    if let CounterRunBindingV1::Interactions { run_id, .. } = &mut changed {
        *run_id = "unselected run with spaces".into();
    }
    assert!(changed.validate().is_err());
}

#[test]
fn finite_cuts_and_policy_limits_refuse_absent_epoch_and_excess_allowances() {
    let mut cut = structural_cut();
    cut.epoch_context_id = [0; 32];
    assert!(cut.validate().is_err());
    cut.epoch_context_id = [6; 32];
    cut.height = MAX_PRIVATE_COUNTER_HEIGHT_V1 + 1;
    assert!(cut.validate().is_err());
    let mut limits = policy(network(b"limit child")).limits;
    limits.max_carrier_work = 10_001;
    assert_eq!(limits.validate(), Err(PrivateCountersErrorV1::Bounds));
    limits.max_carrier_work = 10_000;
    limits.max_groups = 0;
    assert_eq!(limits.validate(), Err(PrivateCountersErrorV1::Bounds));
}

#[test]
fn policy_roles_require_distinct_actual_single_key_accounts() {
    let mut policy = policy(network(b"acl child"));
    for (index, role) in [
        CounterReaderV1::Operator,
        CounterReaderV1::Psp1,
        CounterReaderV1::Psp2,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(policy.reader(&policy.readers[index]).unwrap(), role);
    }
    assert_eq!(
        policy.reader(&AccountId::new(reader_key(20).public_key().clone())),
        Err(PrivateCountersErrorV1::Unauthorized)
    );
    policy.readers[2] = policy.readers[1].clone();
    assert!(policy.validate().is_err());
    let mut policy = self::policy(network(b"multisig child"));
    let multisig = MultisigPolicy::new(
        1,
        vec![MultisigMember::new(reader_key(20).public_key().clone(), 1).unwrap()],
    )
    .unwrap();
    policy.readers[2] = AccountId::new_multisig(multisig);
    assert_eq!(
        policy.validate(),
        Err(PrivateCountersErrorV1::UnsupportedMultisig)
    );
}

#[test]
fn policy_reader_list_requires_exactly_three_current_roles() {
    let mut missing = policy(network(b"missing reader child"));
    missing.readers.pop();
    assert_eq!(missing.validate(), Err(PrivateCountersErrorV1::Context));
    let mut extra = policy(network(b"extra reader child"));
    extra
        .readers
        .push(AccountId::new(reader_key(20).public_key().clone()));
    assert_eq!(extra.validate(), Err(PrivateCountersErrorV1::Context));
    for invalid in [missing, extra] {
        let json = norito::json::to_json(&invalid).unwrap();
        let decoded = norito::json::from_str::<PrivateCountersPolicyV1>(&json).unwrap();
        assert_eq!(decoded.validate(), Err(PrivateCountersErrorV1::Context));
        assert!(decoded.encode_canonical().is_err());
    }
}

#[test]
fn policy_error_mapping_binds_nominal_identity_and_released_artifact() {
    let mut policy = policy(network(b"nominal child"));
    policy.validate().unwrap();
    let original = policy.commitment().unwrap();
    policy.contract_errors[0].code_hash = Hash::new(b"different native code");
    assert_ne!(policy.commitment().unwrap(), original);
    policy.contract_errors[0].artifact_hash = [10; 32];
    assert!(policy.validate().is_err());
    let mut policy = self::policy(network(b"duplicate nominal child"));
    let mut duplicate = policy.contract_errors[0].clone();
    duplicate.rejection = CounterRejectionV1::WalletLimitExceeded;
    policy.contract_errors.push(duplicate);
    assert!(policy.validate().is_err());
    policy.contract_errors.truncate(1);
    policy.contract_errors[0].code = 0;
    assert!(policy.validate().is_err());
}

#[test]
fn semantic_plan_commits_complete_ordered_descriptors_without_hardcoded_cardinality() {
    let selected = [semantic(1), semantic(105)];
    let authority = AccountId::new(reader_key(17).public_key().clone());
    let bindings = [
        executable_binding(1, &authority),
        executable_binding(105, &authority),
    ];
    let hash = counter_plan_commitment_v1(&selected, &bindings).unwrap();
    let mut changed = selected.clone();
    changed[1].party = CounterPartyV1::Psp2;
    assert_ne!(
        counter_plan_commitment_v1(&changed, &bindings).unwrap(),
        hash
    );
    assert!(counter_plan_commitment_v1(&[], &[]).is_err());
    assert!(counter_plan_commitment_v1(&[semantic(1), semantic(1)], &bindings).is_err());
    assert!(counter_plan_commitment_v1(&[semantic(105), semantic(1)], &bindings).is_err());
    assert!(counter_plan_commitment_v1(&selected, &bindings[..1]).is_err());
    let mut bad = semantic(1);
    bad.instruction_parties.clear();
    assert!(bad.validate().is_err());
    bad = semantic(1);
    bad.rejection = CounterRejectionV1::InsufficientBalance;
    assert!(bad.validate().is_err());
}

#[test]
fn executable_expectation_and_full_plan_refuse_substituted_authority_or_instructions() {
    let mut policy = policy(network(b"pre-submit binding child"));
    let original_policy = policy.commitment().unwrap();
    let original_plan =
        counter_plan_commitment_v1(&[semantic(1), semantic(2)], &policy.expected_executables)
            .unwrap();
    let original_binding = policy.expected_executables[0].commitment().unwrap();
    let substituted = Executable::from([Log::new(
        Level::INFO,
        "same claimed action, different actual executable".into(),
    )]);
    policy.expected_executables[0].executable_hash = HashOf::try_new(&substituted).unwrap();
    assert_ne!(
        policy.expected_executables[0].commitment().unwrap(),
        original_binding
    );
    assert_ne!(policy.commitment().unwrap(), original_policy);
    assert_ne!(
        counter_plan_commitment_v1(&[semantic(1), semantic(2)], &policy.expected_executables)
            .unwrap(),
        original_plan
    );
    policy.expected_executables[0].authority = policy.readers[1].clone();
    assert_ne!(
        counter_plan_commitment_v1(&[semantic(1), semantic(2)], &policy.expected_executables)
            .unwrap(),
        original_plan
    );
    let sealed = manifest(&policy);
    assert!(sealed.validate_against_policy(&policy).is_err());
    policy.expected_executables[0].authority = AccountId::new(reader_key(20).public_key().clone());
    assert!(policy.validate().is_err());
    policy.expected_executables[0].authority = policy.readers[0].clone();
    policy.expected_executables[1].action_id = 1;
    assert!(policy.validate().is_err());
    policy.expected_executables[0].action_id = 0;
    assert!(policy.expected_executables[0].validate().is_err());
}

#[test]
fn manifest_refuses_foreign_policy_duplicates_unknown_authority_and_height() {
    let policy = policy(network(b"manifest child"));
    let original = manifest(&policy);
    original.validate_against_policy(&policy).unwrap();
    let mut bad = original.clone();
    bad.entries[1].entrypoint_hash = bad.entries[0].entrypoint_hash;
    assert!(bad.validate().is_err());
    bad = original.clone();
    bad.entries[1].semantic.action_id = bad.entries[0].semantic.action_id;
    assert!(bad.validate_against_policy(&policy).is_err());
    bad = original.clone();
    bad.entries[0].authority = AccountId::new(reader_key(20).public_key().clone());
    assert!(bad.validate_against_policy(&policy).is_err());
    bad = original.clone();
    bad.entries[0].block_height = MAX_PRIVATE_COUNTER_HEIGHT_V1 + 1;
    assert!(bad.validate().is_err());
    bad = original;
    bad.policy_hash = Hash::new(b"foreign policy");
    assert!(bad.validate_against_policy(&policy).is_err());
}

#[test]
fn request_signature_and_native_time_bind_the_entire_payload() {
    let policy = policy(network(b"signed reader child"));
    let original = request(&policy, structural_cut(), 1_000);
    original.verify_signature().unwrap();
    assert_eq!(
        original.payload.validate_at(&policy, 1_000).unwrap(),
        CounterReaderV1::Operator
    );
    assert_eq!(
        original.payload.validate_at(&policy, 1_106),
        Err(PrivateCountersErrorV1::Freshness)
    );
    assert_eq!(
        original.payload.validate_at(&policy, 994),
        Err(PrivateCountersErrorV1::Freshness)
    );
    let mut changed = original.clone();
    changed.payload.nonce[0] ^= 1;
    assert_eq!(
        changed.verify_signature(),
        Err(PrivateCountersErrorV1::Signature)
    );
    assert_eq!(
        original.payload.clone().try_sign(&reader_key(18)),
        Err(PrivateCountersErrorV1::Unauthorized)
    );
    changed.payload.nonce = [0; 32];
    assert!(changed.validate().is_err());
    changed = original;
    changed.payload.creation_time_ms = u64::MAX;
    assert!(changed.validate().is_err());
}

#[test]
fn closed_group_projection_refuses_identifiers_status_mismatch_and_overflow() {
    let policy = policy(network(b"group child"));
    let request = request(&policy, structural_cut(), 1_000);
    let mut claim = claim(&request, 999);
    claim.groups[0].count = 0;
    assert!(claim.validate().is_err());
    claim.groups[0].count = u64::MAX;
    assert!(claim.validate().is_err());
    claim.groups[0].count = 1;
    claim.groups[0].key.rejection = CounterRejectionV1::HoldingLimitExceeded;
    assert!(claim.validate().is_err());
    claim.groups[0].key.rejection = CounterRejectionV1::None;
    claim.purpose = CounterPurposeV1::WalkthroughInteractions;
    assert!(claim.validate().is_err());
    claim.groups[0].key.party = CounterPartyV1::None;
    claim.validate().unwrap();
    claim.groups.push(claim.groups[0].clone());
    assert!(claim.validate().is_err());
}

#[test]
fn member_signing_preimage_is_distinct_and_original_collection_does_no_arithmetic() {
    let policy = policy(network(b"collection child"));
    let request = request(&policy, structural_cut(), 1_000);
    let claim = claim(&request, 999);
    let key = KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal);
    let response = |index| PrivateCountersResponseV1 {
        claim: claim.clone(),
        attestation: member(&claim, index, &key, 1_000),
    };
    let originals = vec![
        response(2).encode_canonical().unwrap(),
        response(0).encode_canonical().unwrap(),
    ];
    let joined = PrivateCountersCertificateV1::decode_bounded_canonical(
        &collect_private_counters_v1(&originals).unwrap(),
    )
    .unwrap();
    assert_eq!(joined.claim, claim);
    assert_eq!(
        joined
            .attestations
            .iter()
            .map(|item| item.body.member_index)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert!(collect_private_counters_v1(&[originals[0].clone(), originals[0].clone()]).is_err());
    let mut different = response(1);
    different.claim.groups[0].count += 1;
    different.attestation = member(&different.claim, 1, &key, 1_000);
    assert!(
        collect_private_counters_v1(&[originals[0].clone(), different.encode_canonical().unwrap()])
            .is_err()
    );
    let mut wrong = response(0).attestation.body;
    wrong.domain = PRIVATE_COUNTER_REQUEST_DOMAIN_V1;
    assert!(wrong.signing_preimage().is_err());
    wrong.domain = PRIVATE_COUNTER_MEMBER_DOMAIN_V1;
    wrong.member_index = 31;
    assert!(wrong.signing_preimage().is_err());
}

#[cfg(feature = "transparent_api")]
mod native_finality {
    use super::*;
    use crate::sumeragi_finality::{test_fixtures::NativeFinalityFixture, verify_checkpoint_page};

    struct Fixture {
        page: VerifiedFinalityPage,
        policy: PrivateCountersPolicyV1,
        request: SignedPrivateCountersRequestV1,
        certificate: PrivateCountersCertificateV1,
        expected: PrivateCountersExpectedV1,
        keys: Vec<KeyPair>,
        now: u64,
    }

    impl Fixture {
        fn new() -> Self {
            // Fixed public fixture keys sign synthetic counts only. Actual Core computation,
            // account originals and deployed policies need independent executed-World controls.
            let mut native =
                NativeFinalityFixture::start_with_scope("private-counter-model-fixture", scope());
            let checkpoint = native.checkpoint();
            let first = native.genesis_proof().clone();
            let second = native.certify(native.block_with_submitted_work(native.next_header()));
            let page = verify_checkpoint_page(
                native.network_id(),
                &checkpoint,
                &[first, second],
                2,
                32 * 1024 * 1024,
            )
            .unwrap();
            let policy = policy(native.network_id());
            let cut = CounterCutV1::from_verified(page.tip()).unwrap();
            let now = page.tip().header().creation_time_ms + 1;
            let request = request(&policy, cut, now);
            let claim = claim(&request, page.tip().header().creation_time_ms);
            let mut keys = (1..=4)
                .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
                .collect::<Vec<_>>();
            keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
            let attestations = keys
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, key)| member(&claim, index as u16, key, now))
                .collect();
            let certificate = PrivateCountersCertificateV1 {
                claim,
                attestations,
            };
            let expected = PrivateCountersExpectedV1 {
                network_id: policy.network_id,
                scope: policy.scope,
                authority: policy.readers[0].clone(),
                purpose: policy.purpose,
                policy_hash: policy.commitment().unwrap(),
                manifest_hash: request.payload.manifest_hash,
                nonce: request.payload.nonce,
            };
            Self {
                page,
                policy,
                request,
                certificate,
                expected,
                keys,
                now,
            }
        }

        fn verify(&self) -> Result<VerifiedPrivateCountersV1, PrivateCountersErrorV1> {
            verify_private_counters_v1(
                &self.request.encode_canonical()?,
                &self.certificate.encode_canonical()?,
                &self.policy.encode_canonical()?,
                &self.page,
                &self.expected,
                self.now,
            )
        }
    }

    #[test]
    fn native_committee_certificate_verifies_under_the_independent_private_cut() {
        let mut fixture = Fixture::new();
        let verified = fixture.verify().unwrap();
        assert_eq!(verified.claim(), &fixture.certificate.claim);
        assert_eq!(verified.cut().context_id(), fixture.page.tip().context_id());
        assert_eq!(verified.member_indices(), [0, 1, 2]);
        fixture.certificate.attestations.push(member(
            &fixture.certificate.claim,
            3,
            &fixture.keys[3],
            fixture.now,
        ));
        assert_eq!(fixture.verify().unwrap().member_indices(), [0, 1, 2, 3]);
    }

    #[test]
    fn native_verifier_refuses_subquorum_foreign_nonce_scope_and_cut() {
        let mut fixture = Fixture::new();
        fixture.certificate.attestations.pop();
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Quorum)
        ));
        let mut fixture = Fixture::new();
        fixture.expected.nonce[0] ^= 1;
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Context)
        ));
        let mut fixture = Fixture::new();
        fixture.expected.scope = SumeragiRootScope::Global;
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Context)
        ));
        let mut fixture = Fixture::new();
        fixture.request.payload.cut.context_id = Hash::new(b"foreign certified decision");
        fixture.request = fixture.request.payload.try_sign(&reader_key(17)).unwrap();
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Context)
        ));
        let mut fixture = Fixture::new();
        fixture.policy.expected_executables[0].executable_hash =
            HashOf::try_new(&Executable::from([Log::new(
                Level::INFO,
                "foreign executable expectation".into(),
            )]))
            .unwrap();
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Context)
        ));
    }

    #[test]
    fn native_verifier_refuses_repeated_member_wrong_key_and_ordinary_signature() {
        let mut fixture = Fixture::new();
        fixture.certificate.attestations[1] = fixture.certificate.attestations[0].clone();
        assert!(fixture.verify().is_err());
        let mut fixture = Fixture::new();
        fixture.certificate.attestations[0] =
            member(&fixture.certificate.claim, 0, &fixture.keys[1], fixture.now);
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Signature)
        ));
        let mut fixture = Fixture::new();
        fixture.certificate.attestations[0].signature = Signature::try_new(
            fixture.keys[0].private_key(),
            b"ordinary consensus or generic hash preimage",
        )
        .unwrap();
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Signature)
        ));
    }

    #[test]
    fn native_verifier_refuses_changed_counts_and_stale_or_future_member_clocks() {
        let mut fixture = Fixture::new();
        fixture.certificate.claim.groups[0].count += 1;
        assert!(fixture.verify().is_err());
        let mut fixture = Fixture::new();
        fixture.certificate.attestations[0] = member(
            &fixture.certificate.claim,
            0,
            &fixture.keys[0],
            fixture.now + 6,
        );
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Freshness)
        ));
        let mut fixture = Fixture::new();
        fixture.now += 106;
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Freshness)
        ));
        let mut fixture = Fixture::new();
        fixture.request.payload.time_to_live_ms = NonZeroU64::new(1_000).unwrap();
        fixture.request = fixture.request.payload.try_sign(&reader_key(17)).unwrap();
        fixture.certificate.claim.request_hash = fixture.request.original_hash().unwrap();
        fixture.certificate.attestations = fixture
            .keys
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, key)| member(&fixture.certificate.claim, index as u16, key, fixture.now))
            .collect();
        fixture.now += 501;
        assert!(matches!(
            fixture.verify(),
            Err(PrivateCountersErrorV1::Freshness)
        ));
    }
}
