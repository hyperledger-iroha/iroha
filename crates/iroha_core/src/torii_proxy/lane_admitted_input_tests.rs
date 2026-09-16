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
        iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
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
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        &network, &signed, salt, 9,
    );
    input.entrypoint = TransactionEntrypoint::SealedReveal(
        iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            commitment, signed, salt,
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
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        &network, &signed, [0x71; 32], 9,
    );
    let payload = iroha_data_model::transaction::signed::SealedTransactionCommitmentPayload::new(
        network,
        iroha_data_model::account::AccountId::new(signer.public_key().clone()),
        commitment,
        1,
        9,
        None,
    );
    input.entrypoint = TransactionEntrypoint::SealedCommitment(
        iroha_data_model::transaction::signed::SignedSealedTransactionCommitment::sign(
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

/// Construct a structural native body with caller-supplied exact slots, without authority.
fn native_envelope_for_size_test(
    input: &LaneAdmittedInputV1,
    slots: Vec<iroha_data_model::block::lane_input::LaneInputRouteSlotV1>,
) -> iroha_data_model::block::lane_input::LaneInputPayloadV1 {
    use iroha_data_model::block::{
        lane_consensus::QueuePlanAdmissionPriorityV1,
        lane_input::{LANE_INPUT_VERSION_V1, LaneInputDescriptorV1, LaneInputPayloadV1},
    };
    LaneInputPayloadV1 {
        descriptor: LaneInputDescriptorV1 {
            version: LANE_INPUT_VERSION_V1,
            admission_priority: QueuePlanAdmissionPriorityV1::new(19, 3).unwrap(),
            admission_carrier_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"size-test first carrier",
            )),
            admitted_input_hash: Hash::new(norito::encode_canonical(input).unwrap()),
            slots,
        },
        input: input.clone(),
    }
}

#[test]
fn complete_input_envelope_bounds_match_real_seven_validator_mixed_quorums_and_signed_frames() {
    use iroha_data_model::block::lane_input::LaneInputRouteSlotV1;
    use std::sync::Arc;
    let (network, mut input, _) = complete_input_fixture();
    // Seven is an exact 3f+1 committee; admission durability is f+1 = 3.
    // Include every currently exercised signature width, including ML-DSA.
    let algorithms = [
        iroha_crypto::Algorithm::Ed25519,
        iroha_crypto::Algorithm::Secp256k1,
        iroha_crypto::Algorithm::BlsNormal,
        iroha_crypto::Algorithm::BlsSmall,
        iroha_crypto::Algorithm::MlDsa,
        iroha_crypto::Algorithm::Ed25519,
        iroha_crypto::Algorithm::BlsNormal,
    ];
    let keys = algorithms
        .into_iter()
        .enumerate()
        .map(|(index, algorithm)| {
            iroha_crypto::KeyPair::from_seed(
                vec![0x60 + u8::try_from(index).unwrap(); 32],
                algorithm,
            )
        })
        .collect::<Vec<_>>();
    let plan = input.routing_plan().unwrap();
    let mut context = input.certificate.binding.admission_context.clone();
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    context.route_incarnations[0].validator_set_hash = HashOf::new(&roster);
    context.route_incarnations[0].validator_set = roster;
    context.route_incarnations[0].validator_count = 7;
    context.route_incarnations[0].durability_threshold = 3;
    let binding =
        new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context, 73).unwrap();
    let bounds =
        maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &binding).unwrap();
    assert_eq!(
        bounds.complete_input_bytes,
        maximum_lane_admitted_input_encoded_len_v1(&input.entrypoint, &binding).unwrap()
    );
    assert_eq!(bounds.native_route_slots, 1);
    let source_key =
        iroha_crypto::KeyPair::from_seed(vec![0xD1; 32], iroha_crypto::Algorithm::BlsNormal);
    let target_key =
        iroha_crypto::KeyPair::from_seed(vec![0xD2; 32], iroha_crypto::Algorithm::BlsNormal);
    let target = PeerId::new(target_key.public_key().clone());
    let mut maximum = [0usize; 6];
    let mut quorum_count = 0;
    for first in 0..7 {
        for second in first + 1..7 {
            for third in second + 1..7 {
                quorum_count += 1;
                input.certificate = QueuePlanAdmissionCertificateV1 {
                    version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
                    binding: binding.clone(),
                    attestations: [first, second, third]
                        .into_iter()
                        .map(|index| {
                            let validator_index = u16::try_from(index).unwrap();
                            let preimage = queue_plan_admission_attestation_signing_bytes_v1(
                                binding.canonical_hash(),
                                validator_index,
                            )
                            .unwrap();
                            let signature = Signature::new(keys[index].private_key(), &preimage);
                            assert_eq!(
                                signature.payload().len(),
                                algorithms[index].signature_payload_len()
                            );
                            QueuePlanAdmissionAttestationV1 {
                                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                                validator_index,
                                signature,
                            }
                        })
                        .collect(),
                };
                let complete = norito::encode_canonical(&input).unwrap();
                decode_and_validate_lane_admitted_input_v1(&network, &complete).unwrap();
                let native = native_envelope_for_size_test(
                    &input,
                    vec![LaneInputRouteSlotV1 {
                        route: plan.coordinator_route(),
                        lane_incarnation: binding.admission_context.route_incarnations[0]
                            .lane_incarnation,
                        instance_id: Hash::new(
                            b"non-authorizing test instance, distinct from size value",
                        ),
                        lane_height: u64::MAX,
                    }],
                );
                native.validate_structure().unwrap();
                let native_bytes = norito::encode_canonical(&native).unwrap();
                let publication = crate::NetworkMessage::QueuePlanAdmissionPublication(Arc::new(
                    QueuePlanAdmissionPublicationV1 {
                        schema_version: QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1,
                        certificate: complete.clone(),
                    },
                ));
                let republication = crate::NetworkMessage::QueuePlanAdmissionCertificate(Arc::new(
                    complete.clone(),
                ));
                // Use the independent materialized owner: real BLS relay signature
                // plus the canonical Message::Data frame, not the sizing formula.
                let publication_bytes =
                    iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                        &source_key,
                        Some(target.clone()),
                        publication,
                    )
                    .unwrap();
                let republication_bytes =
                    iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                        &source_key,
                        Some(target.clone()),
                        republication,
                    )
                    .unwrap();
                let actual = [
                    complete.len(),
                    native_bytes.len(),
                    publication_bytes,
                    iroha_p2p::frame_queue_charge(publication_bytes).unwrap(),
                    republication_bytes,
                    iroha_p2p::frame_queue_charge(republication_bytes).unwrap(),
                ];
                let limits = [
                    bounds.complete_input_bytes,
                    bounds.native_payload_bytes,
                    bounds.publication_plaintext_bytes,
                    bounds.publication_queue_bytes,
                    bounds.republication_plaintext_bytes,
                    bounds.republication_queue_bytes,
                ];
                for index in 0..actual.len() {
                    assert!(
                        actual[index] <= limits[index],
                        "real quorum {first}/{second}/{third}, envelope {index}"
                    );
                    maximum[index] = maximum[index].max(actual[index]);
                }
            }
        }
    }
    assert_eq!(quorum_count, 35);
    assert_eq!(
        maximum,
        [
            bounds.complete_input_bytes,
            bounds.native_payload_bytes,
            bounds.publication_plaintext_bytes,
            bounds.publication_queue_bytes,
            bounds.republication_plaintext_bytes,
            bounds.republication_queue_bytes
        ]
    );
    // The configured frame cap covers encrypted body; its stream u32 prefix is
    // queue charge only. Exact bound fits; one byte less refuses the same frame.
    let cap = bounds.publication_queue_bytes - std::mem::size_of::<u32>();
    assert_eq!(
        iroha_p2p::frame_plaintext_cap(cap),
        bounds.publication_plaintext_bytes
    );
    assert!(iroha_p2p::frame_plaintext_cap(cap - 1) < bounds.publication_plaintext_bytes);
    assert!(bounds.native_payload_bytes > bounds.complete_input_bytes);
    assert!(bounds.publication_plaintext_bytes > bounds.complete_input_bytes);
    assert!(bounds.republication_plaintext_bytes > bounds.complete_input_bytes);
}

#[test]
fn complete_input_envelope_sizing_deduplicates_roles_and_rejects_conflicting_incarnations() {
    use iroha_data_model::block::lane_input::LaneInputRouteSlotV1;
    let (network, mut input, keys) = complete_input_fixture();
    let coordinator = input.routing_plan().unwrap().coordinator_route();
    let participant = RoutingDecision::new(LaneId::new(9), DataSpaceId::new(10));
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            RouteLeg::new(coordinator, RouteLegRole::Participant),
            RouteLeg::new(participant, RouteLegRole::Participant),
        ],
    );
    let mut context = input.certificate.binding.admission_context.clone();
    let original = context.route_incarnations[0].clone();
    let participant_incarnation = Hash::new(b"actual second route incarnation");
    context.route_incarnations = plan
        .legs()
        .into_iter()
        .map(|leg| {
            let mut bound = original.clone();
            bound.leg = leg;
            if leg.route == participant {
                bound.lane_incarnation = participant_incarnation;
            }
            bound
        })
        .collect();
    context.routing_plan_digest = plan.digest();
    let binding =
        new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context.clone(), 73)
            .unwrap();
    input.certificate = sign_complete_input_certificate(binding.clone(), &keys);
    let bytes = norito::encode_canonical(&input).unwrap();
    decode_and_validate_lane_admitted_input_v1(&network, &bytes).unwrap();
    let bounds =
        maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &binding).unwrap();
    assert_eq!(binding.admission_context.route_incarnations.len(), 3);
    assert_eq!(bounds.native_route_slots, 2);
    // Expected two slots are enumerated independently of the sizing map.
    let mut slots = vec![
        LaneInputRouteSlotV1 {
            route: coordinator,
            lane_incarnation: original.lane_incarnation,
            instance_id: Hash::new(b"coordinator instance"),
            lane_height: 7,
        },
        LaneInputRouteSlotV1 {
            route: participant,
            lane_incarnation: participant_incarnation,
            instance_id: Hash::new(b"participant instance"),
            lane_height: 3,
        },
    ];
    slots.sort_by_key(LaneInputRouteSlotV1::route_key);
    let native = native_envelope_for_size_test(&input, slots);
    native.validate_structure().unwrap();
    assert_eq!(
        norito::encode_canonical(&native).unwrap().len(),
        bounds.native_payload_bytes
    );
    context
        .route_incarnations
        .iter_mut()
        .find(|bound| bound.leg.route == coordinator && bound.leg.role == RouteLegRole::Participant)
        .unwrap()
        .lane_incarnation = Hash::new(b"conflicting same-route incarnation");
    match new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context, 73) {
        Err(_) => {} // An upstream structural owner may reject before sizing.
        Ok(conflicting) => assert!(
            maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &conflicting).is_err()
        ),
    }
}

#[test]
fn complete_input_envelope_sizing_is_canonical_and_bounds_materialization_before_allocation() {
    let (network, mut input, keys) = complete_input_fixture();
    let binding = input.certificate.binding.clone();
    let expected =
        maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &binding).unwrap();
    {
        let alternate =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate);
        assert_eq!(
            maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &binding).unwrap(),
            expected
        );
        assert_eq!(norito::core::get_decode_flags(), alternate);
    }
    let signer = iroha_crypto::KeyPair::from_seed(vec![0xE1; 32], iroha_crypto::Algorithm::Ed25519);
    input.entrypoint = TransactionEntrypoint::External(
        iroha_data_model::transaction::TransactionBuilder::new(
            network,
            iroha_data_model::account::AccountId::new(signer.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        // A legal string instruction exceeds the complete control cap without
        // first violating the independent Json value-size limit.
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "x".repeat(iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES),
        )])
        .with_admission_intent(
            iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
        )
        .sign(signer.private_key()),
    );
    let plan = input.routing_plan().unwrap();
    let oversized = new_queue_plan_admission_binding(
        &network,
        &input.entrypoint,
        &plan,
        binding.admission_context,
        73,
    )
    .unwrap();
    input.certificate = sign_complete_input_certificate(oversized.clone(), &keys);
    assert!(
        maximum_lane_admitted_input_encoded_len_v1(&input.entrypoint, &oversized).unwrap()
            > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES
    );
    assert!(
        maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &oversized)
            .unwrap_err()
            .contains("per-control cap")
    );
}

#[test]
fn complete_input_envelope_sizing_counts_maximum_distinct_native_route_slots() {
    use iroha_data_model::block::{
        lane_admission::MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1,
        lane_input::{LaneInputRouteSlotV1, MAX_LANE_INPUT_ROUTE_SLOTS},
    };
    let (network, mut input, keys) = complete_input_fixture();
    let coordinator = input.routing_plan().unwrap().coordinator_route();
    let participant =
        |index| RoutingDecision::new(LaneId::new(100 + index), DataSpaceId::new(100 + u64::from(index)));
    let plan = RoutingPlan::native_amx(
        coordinator,
        (0..MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1)
            .map(|index| {
                RouteLeg::new(
                    participant(u32::try_from(index).unwrap()),
                    RouteLegRole::Participant,
                )
            })
            .collect(),
    );
    let mut context = input.certificate.binding.admission_context.clone();
    let original = context.route_incarnations[0].clone();
    context.route_incarnations = plan
        .legs()
        .into_iter()
        .map(|leg| {
            let mut bound = original.clone();
            bound.leg = leg;
            bound.lane_incarnation = Hash::new(leg.route.lane_id.as_u32().to_be_bytes());
            bound
        })
        .collect();
    context.routing_plan_digest = plan.digest();
    let binding =
        new_queue_plan_admission_binding(&network, &input.entrypoint, &plan, context, 73).unwrap();
    input.certificate = sign_complete_input_certificate(binding.clone(), &keys);
    let bounds =
        maximum_lane_admitted_input_envelope_sizes_v1(&input.entrypoint, &binding).unwrap();
    assert_eq!(bounds.native_route_slots, MAX_LANE_INPUT_ROUTE_SLOTS);
    let mut slots = binding
        .admission_context
        .route_incarnations
        .iter()
        .enumerate()
        .map(|(index, bound)| LaneInputRouteSlotV1 {
            route: bound.leg.route,
            lane_incarnation: bound.lane_incarnation,
            instance_id: Hash::new(u64::try_from(index).unwrap().to_be_bytes()),
            lane_height: u64::try_from(index).unwrap() + 1,
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(LaneInputRouteSlotV1::route_key);
    let native = native_envelope_for_size_test(&input, slots);
    native.validate_structure().unwrap();
    assert_eq!(
        norito::encode_canonical(&native).unwrap().len(),
        bounds.native_payload_bytes
    );
    assert_eq!(
        norito::encode_canonical(&input).unwrap().len(),
        bounds.complete_input_bytes
    );
    let mut excessive = native.clone();
    excessive.descriptor.slots.push(LaneInputRouteSlotV1 {
        route: participant(10_000),
        lane_incarnation: Hash::new(b"extra route"),
        instance_id: Hash::new(b"extra instance"),
        lane_height: 1,
    });
    assert!(excessive.validate_structure().is_err());
}
