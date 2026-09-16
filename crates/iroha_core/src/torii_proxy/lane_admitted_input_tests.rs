// Complete input authentication uses real existing admission producers and signatures.

fn sign_complete_input_certificate(
    binding: QueuePlanAdmissionBindingV1,
    keys: &[iroha_crypto::KeyPair],
) -> QueuePlanAdmissionCertificateV1 {
    let threshold =
        usize::from(binding.admission_context.route_incarnations[0].durability_threshold);
    let attestations = keys
        .iter()
        .take(threshold)
        .enumerate()
        .map(|(index, key)| {
            let index = u16::try_from(index).unwrap();
            let preimage =
                queue_plan_admission_attestation_signing_bytes_v1(binding.canonical_hash(), index)
                    .unwrap();
            QueuePlanAdmissionAttestationV1 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                validator_index: index,
                signature: Signature::new(key.private_key(), &preimage),
            }
        })
        .collect();
    QueuePlanAdmissionCertificateV1 {
        version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
        binding,
        attestations,
    }
}

fn complete_input_fixture() -> (NetworkId, LaneAdmittedInputV1, Vec<iroha_crypto::KeyPair>) {
    let (network, entrypoint, plan, original, _) = single_route_admission_fixture();
    let keys = (0..4)
        .map(|seed| {
            iroha_crypto::KeyPair::from_seed(vec![seed + 31; 32], iroha_crypto::Algorithm::Ed25519)
        })
        .collect::<Vec<_>>();
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut context = original.admission_context;
    let leg = &mut context.route_incarnations[0];
    leg.validator_set_hash = HashOf::new(&roster);
    leg.validator_set = roster;
    leg.validator_count = 4;
    leg.durability_threshold = 2; // Admission durability f+1, distinct from lane Decision 2f+1.
    let binding =
        new_queue_plan_admission_binding(&network, &entrypoint, &plan, context, 73).unwrap();
    let certificate = sign_complete_input_certificate(binding, &keys);
    (
        network,
        LaneAdmittedInputV1 {
            entrypoint,
            certificate,
        },
        keys,
    )
}

#[test]
fn complete_admitted_input_authenticates_once_and_preserves_exact_source() {
    let (network, input, _) = complete_input_fixture();
    let bytes = norito::encode_canonical(&input).unwrap();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = std::rc::Rc::clone(&calls);
    let validated = observe_queue_plan_authentication_for_test(
        move || observed.set(observed.get() + 1),
        || decode_and_validate_lane_admitted_input_v1(&network, &bytes),
    )
    .unwrap();
    assert_eq!(
        calls.get(),
        1,
        "one actual certificate authentication boundary"
    );
    assert_eq!(validated.input(), &input);
    assert_eq!(validated.entrypoint(), &input.entrypoint);
    assert_eq!(validated.certificate().certificate, input.certificate);
    assert_eq!(
        validated.certificate().binding_hash,
        input.certificate.binding.canonical_hash()
    );
    assert_eq!(norito::encode_canonical(validated.input()).unwrap(), bytes);
    assert_eq!(validated.into_input(), input);
}

#[test]
fn complete_admitted_input_requires_body_and_full_durability_quorum() {
    let (network, mut input, _) = complete_input_fixture();
    let certificate_only = norito::encode_canonical(&input.certificate).unwrap();
    decode_and_validate_queue_plan_admission_certificate_v1(&network, &certificate_only).unwrap();
    assert!(decode_and_validate_lane_admitted_input_v1(&network, &certificate_only).is_err());
    input.certificate.attestations.pop();
    validate_queue_plan_admission_certificate_v1(
        &network,
        input.certificate.clone(),
        QueuePlanAdmissionCertificateStrengthV1::Partial,
    )
    .unwrap();
    let bytes = norito::encode_canonical(&input).unwrap();
    assert!(
        decode_and_validate_lane_admitted_input_v1(&network, &bytes).is_err(),
        "a valid partial response is not complete source authority"
    );
}

#[test]
fn complete_admitted_input_rejects_validly_attested_body_identity_and_claim_mismatch() {
    let (network, input, keys) = complete_input_fixture();
    let mut mutations = Vec::new();
    let mut changed = input.clone();
    changed.certificate.binding.signed_transaction_hash = Some(HashOf::from_untyped_unchecked(
        Hash::new(b"different signed identity"),
    ));
    mutations.push(("signed transaction identity", changed));
    let mut changed = input.clone();
    changed.certificate.binding.enqueue_timestamp_ms += 1;
    mutations.push(("exact journal timestamp", changed));
    let mut changed = input.clone();
    changed
        .certificate
        .binding
        .admission_context
        .route_incarnations[0]
        .lane_incarnation = Hash::new(b"different bound incarnation");
    mutations.push(("exact journal context", changed));
    let mut changed = input.clone();
    changed.certificate.binding.journal_record_digest = Hash::new(b"unrelated journal record");
    mutations.push(("exact journal digest", changed));
    for (label, mut changed) in mutations {
        changed.certificate = sign_complete_input_certificate(changed.certificate.binding, &keys);
        validate_queue_plan_admission_certificate_v1(
            &network,
            changed.certificate.clone(),
            QueuePlanAdmissionCertificateStrengthV1::Quorum,
        )
        .unwrap();
        assert!(
            decode_and_validate_lane_admitted_input_v1(
                &network,
                &norito::encode_canonical(&changed).unwrap()
            )
            .is_err(),
            "{label}: valid signatures alone do not bind the supplied body"
        );
    }
    let mut changed = input.clone();
    let TransactionEntrypoint::External(signed) = changed.entrypoint else {
        unreachable!()
    };
    let salt = [0x51; 32];
    changed.entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::SealedTransactionReveal::new(
            iroha_data_model::transaction::compute_sealed_transaction_commitment(
                &network, &signed, salt, 9,
            ),
            signed,
            salt,
        ),
    );
    assert!(
        decode_and_validate_lane_admitted_input_v1(
            &network,
            &norito::encode_canonical(&changed).unwrap()
        )
        .is_err(),
        "same enclosed signed payload does not authorize a distinct outer reveal"
    );
    let other_network = torii_proxy_test_network_id(b"foreign complete input network");
    assert!(
        decode_and_validate_lane_admitted_input_v1(
            &other_network,
            &norito::encode_canonical(&input).unwrap()
        )
        .is_err()
    );
}

#[test]
fn complete_admitted_input_preserves_both_coordinator_and_participant_on_same_route() {
    let (network, mut input, keys) = complete_input_fixture();
    let coordinator = input.routing_plan().unwrap().coordinator_route();
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            RouteLeg::new(coordinator, RouteLegRole::Participant),
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(9), DataSpaceId::new(10)),
                RouteLegRole::Participant,
            ),
        ],
    );
    let mut context = input.certificate.binding.admission_context.clone();
    let original = context.route_incarnations[0].clone();
    context.route_incarnations = plan
        .legs()
        .into_iter()
        .map(|leg| {
            let mut bound = original.clone();
            bound.leg = leg;
            if leg.route != coordinator {
                bound.lane_incarnation = Hash::new(b"other route incarnation");
            }
            bound
        })
        .collect();
    context.routing_plan_digest = plan.digest();
    let binding =
        new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context, 73).unwrap();
    input.certificate = sign_complete_input_certificate(binding, &keys);
    let validated = decode_and_validate_lane_admitted_input_v1(
        &network,
        &norito::encode_canonical(&input).unwrap(),
    )
    .unwrap();
    let actual = validated.input().routing_plan().unwrap();
    assert_eq!(actual, plan);
    let local = actual
        .legs()
        .into_iter()
        .filter(|leg| leg.route == coordinator)
        .collect::<Vec<_>>();
    assert_eq!(
        local.len(),
        2,
        "one route has two semantic roles in the original plan"
    );
    assert_eq!(local[0].role, RouteLegRole::Coordinator);
    assert_eq!(local[1].role, RouteLegRole::Participant);
}

#[test]
fn complete_admitted_input_preserves_sealed_reveal_outer_and_signed_identities() {
    let (network, mut input, keys) = complete_input_fixture();
    let TransactionEntrypoint::External(signed) = input.entrypoint else {
        unreachable!()
    };
    let signed_hash = signed.hash();
    let salt = [0x61; 32];
    let commitment = iroha_data_model::transaction::compute_sealed_transaction_commitment(
        &network, &signed, salt, 9,
    );
    input.entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::SealedTransactionReveal::new(commitment, signed, salt),
    );
    let plan = input.certificate.binding.routing_plan().unwrap();
    let binding = new_queue_plan_admission_binding(
        &network,
        &input.entrypoint,
        &plan,
        input.certificate.binding.admission_context,
        73,
    )
    .unwrap();
    assert_eq!(binding.signed_transaction_hash, Some(signed_hash));
    assert_ne!(Hash::from(binding.entrypoint_hash), Hash::from(signed_hash));
    input.certificate = sign_complete_input_certificate(binding, &keys);
    let validated = decode_and_validate_lane_admitted_input_v1(
        &network,
        &norito::encode_canonical(&input).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validated.entrypoint(),
        TransactionEntrypoint::SealedReveal(_)
    ));
    assert_eq!(validated.input(), &input);
}

#[test]
fn complete_admitted_input_decoder_enforces_existing_control_cap_and_canonical_frame() {
    let (network, input, _) = complete_input_fixture();
    let bytes = norito::encode_canonical(&input).unwrap();
    assert!(decode_and_validate_lane_admitted_input_v1(&network, &[]).is_err());
    assert!(
        decode_and_validate_lane_admitted_input_v1(
            &network,
            &vec![0; iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES + 1]
        )
        .is_err()
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_and_validate_lane_admitted_input_v1(&network, &trailing).is_err());
    let alternate = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _guard = norito::core::DecodeFlagsGuard::enter(alternate);
    let validated = decode_and_validate_lane_admitted_input_v1(&network, &bytes).unwrap();
    assert_eq!(norito::encode_canonical(validated.input()).unwrap(), bytes);
}

#[test]
fn complete_admitted_input_preserves_sealed_commitment_without_fabricated_signed_identity() {
    let (network, mut input, keys) = complete_input_fixture();
    let TransactionEntrypoint::External(signed) = input.entrypoint else {
        unreachable!()
    };
    let signer = iroha_crypto::KeyPair::from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519);
    let commitment = iroha_data_model::transaction::compute_sealed_transaction_commitment(
        &network, &signed, [0x71; 32], 9,
    );
    let payload = iroha_data_model::transaction::SealedTransactionCommitmentPayload::new(
        network,
        iroha_data_model::account::AccountId::new(signer.public_key().clone()),
        commitment,
        1,
        9,
        None,
    );
    input.entrypoint = TransactionEntrypoint::SealedCommitment(
        iroha_data_model::transaction::SignedSealedTransactionCommitment::sign(
            payload,
            signer.private_key(),
        ),
    );
    let plan = input.certificate.binding.routing_plan().unwrap();
    let binding = new_queue_plan_admission_binding(
        &network,
        &input.entrypoint,
        &plan,
        input.certificate.binding.admission_context,
        73,
    )
    .unwrap();
    assert_eq!(binding.signed_transaction_hash, None);
    input.certificate = sign_complete_input_certificate(binding, &keys);
    let validated = decode_and_validate_lane_admitted_input_v1(
        &network,
        &norito::encode_canonical(&input).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validated.entrypoint(),
        TransactionEntrypoint::SealedCommitment(_)
    ));
    assert_eq!(validated.input(), &input);
}

#[test]
fn complete_admitted_input_allows_metadata_larger_than_certificate_key_sequence_bound() {
    let (network, mut input, keys) = complete_input_fixture();
    let signer = iroha_crypto::KeyPair::from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519);
    let length = iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES + 1_024;
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "large_input".parse().unwrap(),
        iroha_primitives::json::Json::new("x".repeat(length)),
    );
    let signed = iroha_data_model::transaction::TransactionBuilder::new(
        network,
        iroha_data_model::account::AccountId::new(signer.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_metadata(metadata)
    .sign(signer.private_key());
    signed.verify_signature().unwrap();
    input.entrypoint = TransactionEntrypoint::External(signed);
    let plan = input.routing_plan().unwrap();
    let binding = new_queue_plan_admission_binding(
        &network,
        &input.entrypoint,
        &plan,
        input.certificate.binding.admission_context,
        73,
    )
    .unwrap();
    input.certificate = sign_complete_input_certificate(binding, &keys);
    let bytes = norito::encode_canonical(&input).unwrap();
    assert!(bytes.len() > iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES + 1);
    assert!(bytes.len() < iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES);
    let validated = decode_and_validate_lane_admitted_input_v1(&network, &bytes).unwrap();
    assert_eq!(validated.input(), &input);
    // Application and configured metadata policy remain the body validator's job;
    // this boundary must not accidentally impose a certificate-key-sized field cap.
}

#[test]
fn complete_admitted_input_size_bound_matches_all_real_mixed_quorums() {
    let (network, mut input, _) = complete_input_fixture();
    let algorithms = [
        iroha_crypto::Algorithm::Ed25519,
        iroha_crypto::Algorithm::Secp256k1,
        iroha_crypto::Algorithm::BlsNormal,
        iroha_crypto::Algorithm::BlsSmall,
    ];
    let keys = algorithms
        .into_iter()
        .enumerate()
        .map(|(i, algorithm)| {
            iroha_crypto::KeyPair::from_seed(vec![0x40 + u8::try_from(i).unwrap(); 32], algorithm)
        })
        .collect::<Vec<_>>();
    let plan = input.routing_plan().unwrap();
    let mut context = input.certificate.binding.admission_context;
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    context.route_incarnations[0].validator_set_hash = HashOf::new(&roster);
    context.route_incarnations[0].validator_set = roster;
    let binding =
        new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context, 73).unwrap();
    let bound = maximum_lane_admitted_input_encoded_len_v1(&input.entrypoint, &binding).unwrap();
    let mut largest_real_input = 0;
    for first in 0..4 {
        for second in first + 1..4 {
            let attestations = [first, second]
                .into_iter()
                .map(|i| {
                    let index = u16::try_from(i).unwrap();
                    let preimage = queue_plan_admission_attestation_signing_bytes_v1(
                        binding.canonical_hash(),
                        index,
                    )
                    .unwrap();
                    let signature = Signature::new(keys[i].private_key(), &preimage);
                    assert_eq!(
                        signature.payload().len(),
                        algorithms[i].signature_payload_len()
                    );
                    QueuePlanAdmissionAttestationV1 {
                        version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                        validator_index: index,
                        signature,
                    }
                })
                .collect();
            input.certificate = QueuePlanAdmissionCertificateV1 {
                version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
                binding: binding.clone(),
                attestations,
            };
            let bytes = norito::encode_canonical(&input).unwrap();
            decode_and_validate_lane_admitted_input_v1(&network, &bytes).unwrap();
            assert!(
                bytes.len() <= bound,
                "real signer subset {first}/{second} exceeds preflight"
            );
            largest_real_input = largest_real_input.max(bytes.len());
        }
    }
    assert_eq!(
        largest_real_input, bound,
        "the bound includes exact canonical framing and the largest valid quorum"
    );
}

#[test]
fn complete_admitted_input_size_bound_rejects_inconsistent_claims() {
    let (_, input, _) = complete_input_fixture();
    let mut binding = input.certificate.binding;
    binding.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong input"));
    assert!(maximum_lane_admitted_input_encoded_len_v1(&input.entrypoint, &binding).is_err());
    binding.admission_context.route_incarnations[0].durability_threshold = 0;
    assert!(maximum_lane_admitted_input_encoded_len_v1(&input.entrypoint, &binding).is_err());
}
