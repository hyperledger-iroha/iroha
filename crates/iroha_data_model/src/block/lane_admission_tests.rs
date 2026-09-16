//! Portable admission input controls; structural validity grants no custody or authority.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn fixture() -> (NetworkId, RoutingPlan, QueuePlanAdmissionBindingV1) {
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"lane admission model network",
    )));
    let plan = RoutingPlan::native_amx(
        RoutingDecision::new(LaneId::new(2), DataSpaceId::new(5)),
        vec![
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(7), DataSpaceId::new(9)),
                RouteLegRole::Participant,
            ),
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(3), DataSpaceId::new(6)),
                RouteLegRole::Participant,
            ),
        ],
    );
    let validators: Vec<_> = (1..=4)
        .map(|seed| {
            let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            PeerId::new(key.public_key().clone())
        })
        .collect();
    let context = QueuePlanAdmissionContextV1 {
        version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
        authority_height: 12,
        proposal_height: 13,
        predecessor_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"actual predecessor",
        ))),
        routing_plan_digest: plan.digest(),
        route_incarnations: plan
            .legs()
            .into_iter()
            .map(|leg| QueuePlanRouteIncarnationV1 {
                leg,
                lane_incarnation: Hash::new(leg.route.lane_id.as_u32().to_le_bytes()),
                validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_set: validators.clone(),
                validator_count: 4,
                durability_threshold: 2,
            })
            .collect(),
    };
    let entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"untrusted input reference"));
    let binding = QueuePlanAdmissionBindingV1 {
        version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        network_id_digest: queue_plan_admission_network_id_digest(&network),
        request_id: queue_plan_synced_request_id(&network, entrypoint_hash),
        entrypoint_hash,
        signed_transaction_hash: None,
        routing_plan_digest: plan.digest(),
        admission_context: context,
        enqueue_timestamp_ms: 73,
        queue_plan_journal_version: QUEUE_PLAN_JOURNAL_CLAIM_VERSION_V1,
        durable_admission_version: QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V1,
        // Deliberately not a physical journal claim. Core must compare exact transaction bytes.
        journal_record_digest: Hash::new(b"shape-only journal digest"),
    };
    (network, plan, binding)
}

#[test]
fn lane_admission_all_eleven_dtos_roundtrip_canonical_and_json() {
    let (_, plan, binding) = fixture();
    binding.validate_structure().unwrap();
    macro_rules! roundtrip {
        ($ty:ty, $value:expr) => {{
            let value: $ty = $value;
            let bytes = norito::encode_canonical(&value).unwrap();
            let decoded: $ty = norito::decode_canonical(&bytes).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
            let json = norito::json::to_json(&value).unwrap();
            assert_eq!(norito::json::from_str::<$ty>(&json).unwrap(), value);
        }};
    }
    roundtrip!(RoutingDecision, plan.coordinator_route());
    roundtrip!(RouteLegRole, RouteLegRole::Coordinator);
    roundtrip!(RouteLegRole, RouteLegRole::Participant);
    roundtrip!(RouteLeg, plan.coordinator_leg());
    let RoutingPlan::NativeAmx(native) = &plan else {
        unreachable!()
    };
    roundtrip!(NativeAmxRoutingPlan, native.clone());
    roundtrip!(RoutingPlan, plan.clone());
    roundtrip!(RoutingPlan, RoutingPlan::single(plan.coordinator_route()));
    roundtrip!(
        QueuePlanGlobalAdmissionIdentityV1,
        binding.global_admission_identity()
    );
    roundtrip!(
        QueuePlanRouteIncarnationV1,
        binding.admission_context.route_incarnations[0].clone()
    );
    roundtrip!(
        QueuePlanAdmissionContextV1,
        binding.admission_context.clone()
    );
    roundtrip!(QueuePlanAdmissionRegistryKeyV1, binding.registry_key());
    roundtrip!(QueuePlanAdmissionRegistryValueV1, binding.registry_value());
    roundtrip!(QueuePlanAdmissionBindingV1, binding);
}

#[test]
fn lane_admission_routing_normalizes_only_at_explicit_constructor() {
    let (_, canonical, binding) = fixture();
    let RoutingPlan::NativeAmx(native) = &canonical else {
        unreachable!()
    };
    let mut legs = native.participants.clone();
    legs.reverse();
    legs.push(legs[0]);
    for leg in &mut legs {
        leg.role = RouteLegRole::Coordinator;
    }
    assert_eq!(
        RoutingPlan::native_amx(native.coordinator.route, legs),
        canonical
    );
    assert_eq!(binding.routing_plan().unwrap(), canonical);
    assert_eq!(
        RoutingDecision::default(),
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    );
    let mut noncanonical = native.clone();
    noncanonical.participants.reverse();
    let noncanonical = RoutingPlan::NativeAmx(noncanonical);
    let encoded = norito::encode_canonical(&noncanonical).unwrap();
    let decoded: RoutingPlan = norito::decode_canonical(&encoded).unwrap();
    assert_eq!(
        decoded, noncanonical,
        "decoding must not silently sort untrusted input"
    );
    assert!(
        binding
            .admission_context
            .validate_for_routing_plan(&decoded)
            .is_err()
    );
    let mut bad_digest = native.clone();
    bad_digest.plan_digest = Hash::new(b"forged plan digest");
    assert!(
        binding
            .admission_context
            .validate_for_routing_plan(&RoutingPlan::NativeAmx(bad_digest))
            .is_err()
    );
    let single = RoutingPlan::single(native.coordinator.route);
    let mut context = binding.admission_context.clone();
    context.route_incarnations.truncate(1);
    context.routing_plan_digest = single.digest();
    context.validate_for_routing_plan(&single).unwrap();
    let wrong_role = RoutingPlan::Single(RouteLeg::new(
        native.coordinator.route,
        RouteLegRole::Participant,
    ));
    assert!(context.validate_for_routing_plan(&wrong_role).is_err());
}

#[test]
fn lane_admission_context_rejects_each_identity_geometry_and_order_mutation() {
    let (_, plan, binding) = fixture();
    let original = binding.admission_context;
    let mut cases = Vec::new();
    macro_rules! mutation {
        ($label:literal, $value:ident, $body:block) => {{
            let mut $value = original.clone();
            $body
            cases.push(($label, $value));
        }};
    }
    mutation!("version", v, {
        v.version += 1;
    });
    mutation!("height gap", v, {
        v.proposal_height += 1;
    });
    mutation!("height overflow", v, {
        v.authority_height = u64::MAX;
    });
    mutation!("missing predecessor", v, {
        v.predecessor_block_hash = None;
    });
    mutation!("zero predecessor", v, {
        v.predecessor_block_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])));
    });
    mutation!("wrong plan", v, {
        v.routing_plan_digest = Hash::new(b"other plan");
    });
    mutation!("missing route", v, {
        v.route_incarnations.pop();
    });
    mutation!("reordered routes", v, {
        v.route_incarnations.swap(1, 2);
    });
    mutation!("wrong role", v, {
        v.route_incarnations[1].leg.role = RouteLegRole::Coordinator;
    });
    mutation!("zero incarnation", v, {
        v.route_incarnations[0].lane_incarnation = Hash::prehashed([0; 32]);
    });
    mutation!("roster version", v, {
        v.route_incarnations[0].validator_set_hash_version += 1;
    });
    mutation!("zero roster hash", v, {
        v.route_incarnations[0].validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0; 32]));
    });
    mutation!("roster count", v, {
        v.route_incarnations[0].validator_count = 3;
    });
    mutation!("empty roster", v, {
        v.route_incarnations[0].validator_set.clear();
        v.route_incarnations[0].validator_count = 0;
    });
    mutation!("oversize roster", v, {
        let leg = &mut v.route_incarnations[0];
        leg.validator_set.resize(
            MAX_LANE_CONSENSUS_VALIDATORS + 1,
            leg.validator_set[0].clone(),
        );
        leg.validator_count = (MAX_LANE_CONSENSUS_VALIDATORS + 1) as u16;
    });
    mutation!("duplicate roster with matching hash", v, {
        let leg = &mut v.route_incarnations[0];
        leg.validator_set[1] = leg.validator_set[0].clone();
        leg.validator_set_hash = HashOf::new(&leg.validator_set);
    });
    mutation!("changed roster order", v, {
        v.route_incarnations[0].validator_set.swap(0, 1);
    });
    mutation!("wrong threshold", v, {
        v.route_incarnations[0].durability_threshold = 3;
    });
    for (label, context) in cases {
        assert!(context.validate_for_routing_plan(&plan).is_err(), "{label}");
    }
    let mut genesis = original;
    genesis.authority_height = 0;
    genesis.proposal_height = 1;
    genesis.predecessor_block_hash = None;
    genesis.validate_for_routing_plan(&plan).unwrap();
    genesis.predecessor_block_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"unexpected genesis predecessor",
    )));
    assert!(genesis.validate_for_routing_plan(&plan).is_err());
}

#[test]
fn lane_admission_native_participant_bounds_and_duplicate_controls() {
    let (_, plan, binding) = fixture();
    let coordinator = plan.coordinator_route();
    for count in [0, MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1 + 1] {
        let participants = (0..count)
            .map(|index| {
                RouteLeg::new(
                    RoutingDecision::new(
                        LaneId::new(index as u32 + 10),
                        DataSpaceId::new(index as u64 + 20),
                    ),
                    RouteLegRole::Participant,
                )
            })
            .collect();
        let plan = RoutingPlan::native_amx(coordinator, participants);
        assert!(
            binding
                .admission_context
                .validate_for_routing_plan(&plan)
                .is_err()
        );
    }
    let RoutingPlan::NativeAmx(mut native) = plan else {
        unreachable!()
    };
    native.participants.push(native.participants[0]);
    assert!(
        binding
            .admission_context
            .validate_for_routing_plan(&RoutingPlan::NativeAmx(native))
            .is_err()
    );
}

#[test]
fn lane_admission_binding_rejects_versions_request_substitution_and_zero_claim() {
    let (_, _, original) = fixture();
    let mut cases = Vec::new();
    macro_rules! mutation {
        ($label:literal, $value:ident, $body:block) => {{
            let mut $value = original.clone(); $body cases.push(($label, $value));
        }};
    }
    mutation!("binding version", v, {
        v.version += 1;
    });
    mutation!("context version", v, {
        v.admission_context.version += 1;
    });
    mutation!("journal version", v, {
        v.queue_plan_journal_version += 1;
    });
    mutation!("durable claim version", v, {
        v.durable_admission_version += 1;
    });
    mutation!("zero network", v, {
        v.network_id_digest = Hash::prehashed([0; 32]);
    });
    mutation!("zero request", v, {
        v.request_id = Hash::prehashed([0; 32]);
    });
    mutation!("zero journal", v, {
        v.journal_record_digest = Hash::prehashed([0; 32]);
    });
    mutation!("different entrypoint", v, {
        v.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"different entrypoint"));
    });
    mutation!("different network", v, {
        v.network_id_digest = Hash::new(b"different network");
    });
    mutation!("different plan", v, {
        v.routing_plan_digest = Hash::new(b"different plan");
    });
    for (label, binding) in cases {
        assert!(binding.validate_structure().is_err(), "{label}");
    }
    assert_eq!(
        original.registry_key().entrypoint_hash,
        original.entrypoint_hash
    );
    assert_eq!(
        original.registry_key().network_id_digest,
        original.network_id_digest
    );
    assert_eq!(
        original.registry_value().binding_hash,
        original.canonical_hash()
    );
    assert_eq!(
        original.global_admission_identity().request_id,
        original.request_id
    );
    let mut changed = original.clone();
    changed.journal_record_digest = Hash::new(b"another unverified journal claim");
    changed.validate_structure().unwrap();
    assert_ne!(
        changed.canonical_hash(),
        original.canonical_hash(),
        "structure alone intentionally does not verify a physical claim"
    );
}

#[test]
fn lane_admission_hash_domains_and_ambient_layout_are_explicit() {
    let (network, _, binding) = fixture();
    let network_digest = queue_plan_admission_network_id_digest(&network);
    assert_eq!(
        network_digest,
        Hash::new_from_chunks(&[
            b"iroha:torii:queue-plan-admission-network:v1\0",
            network.as_bytes()
        ])
    );
    let request_bytes = norito::encode_canonical(&(
        "torii:proxy:queue-plan-synced:v1",
        network_digest,
        binding.entrypoint_hash,
    ))
    .unwrap();
    assert_eq!(binding.request_id, Hash::new(request_bytes));
    let bytes = norito::encode_canonical(&binding).unwrap();
    assert_eq!(
        binding.canonical_hash(),
        Hash::new_from_chunks(&[b"iroha:torii:queue-plan-admission-binding:v1\0", &bytes])
    );
    let changed_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"different genesis",
    )));
    assert_ne!(
        queue_plan_synced_request_id(&changed_network, binding.entrypoint_hash),
        binding.request_id
    );
    let canonical_hash = binding.canonical_hash();
    let alternative =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _guard = norito::core::DecodeFlagsGuard::enter(alternative);
    assert_eq!(binding.canonical_hash(), canonical_hash);
    assert_eq!(
        queue_plan_synced_request_id(&network, binding.entrypoint_hash),
        binding.request_id
    );
}

#[test]
fn lane_admission_json_requires_nullable_slots_and_enum_tags() {
    let (_, plan, binding) = fixture();
    let mut value = norito::json::to_value(&binding).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("signed_transaction_hash");
    assert!(norito::json::from_value::<QueuePlanAdmissionBindingV1>(value).is_err());
    let mut genesis_context = binding.admission_context.clone();
    genesis_context.authority_height = 0;
    genesis_context.proposal_height = 1;
    genesis_context.predecessor_block_hash = None;
    genesis_context.validate_for_routing_plan(&plan).unwrap();
    let json = norito::json::to_json(&genesis_context).unwrap();
    assert_eq!(
        norito::json::from_str::<QueuePlanAdmissionContextV1>(&json).unwrap(),
        genesis_context,
        "an explicit null predecessor remains valid at the genesis boundary"
    );
    let mut value = norito::json::to_value(&genesis_context).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("predecessor_block_hash");
    assert!(norito::json::from_value::<QueuePlanAdmissionContextV1>(value).is_err());
    let mut value = norito::json::to_value(&plan).unwrap();
    value.as_object_mut().unwrap().remove("kind");
    assert!(norito::json::from_value::<RoutingPlan>(value).is_err());
    let mut value = norito::json::to_value(&RouteLegRole::Participant).unwrap();
    value.as_object_mut().unwrap().remove("role");
    assert!(norito::json::from_value::<RouteLegRole>(value).is_err());
}

#[test]
fn lane_admission_canonical_frames_reject_other_owner_and_trailing_bytes() {
    let (_, plan, binding) = fixture();
    let mut bytes = norito::encode_canonical(&binding).unwrap();
    assert!(norito::decode_canonical::<QueuePlanAdmissionContextV1>(&bytes).is_err());
    bytes.push(0);
    assert!(norito::decode_canonical::<QueuePlanAdmissionBindingV1>(&bytes).is_err());
    let bytes = norito::encode_canonical(&plan.coordinator_leg()).unwrap();
    assert!(norito::decode_canonical::<RoutingPlan>(&bytes).is_err());
}

#[test]
fn lane_admission_canonical_schema_frame_vectors() {
    use norito::schema::identity::{NoritoSchema, frame_hash};
    macro_rules! check {
        ($ty:ty, $name:literal, $hash:expr) => {
            assert_eq!(<$ty as NoritoSchema>::nominal_name(), $name);
            assert_eq!(frame_hash::<$ty>(), $hash);
        };
    }
    check!(
        RoutingDecision,
        "iroha_data_model::block::lane_admission::RoutingDecision",
        [
            0x36, 0xfa, 0x44, 0x0a, 0x62, 0xdd, 0x6e, 0xa4, 0xc7, 0x0d, 0xc9, 0x2b, 0x60, 0xa6,
            0xfb, 0xa6
        ]
    );
    check!(
        RouteLegRole,
        "iroha_data_model::block::lane_admission::RouteLegRole",
        [
            0xa8, 0x0e, 0x49, 0xbd, 0xf5, 0x18, 0x80, 0x7a, 0x6a, 0x48, 0xaa, 0xb0, 0xa9, 0xbe,
            0x2d, 0x17
        ]
    );
    check!(
        RouteLeg,
        "iroha_data_model::block::lane_admission::RouteLeg",
        [
            0xb7, 0x90, 0x54, 0x96, 0xa7, 0x84, 0x82, 0x3a, 0x8a, 0xa1, 0x45, 0x50, 0x7e, 0x46,
            0x93, 0x2b
        ]
    );
    check!(
        NativeAmxRoutingPlan,
        "iroha_data_model::block::lane_admission::NativeAmxRoutingPlan",
        [
            0x1f, 0x9e, 0x8a, 0xe8, 0x53, 0x46, 0xe3, 0x58, 0x91, 0xb5, 0x00, 0xbf, 0x91, 0x54,
            0xc9, 0x41
        ]
    );
    check!(
        RoutingPlan,
        "iroha_data_model::block::lane_admission::RoutingPlan",
        [
            0x6e, 0xc5, 0xfc, 0xe5, 0x6b, 0x75, 0x75, 0x96, 0x10, 0x74, 0x50, 0xb1, 0x02, 0x72,
            0x33, 0xf0
        ]
    );
    check!(
        QueuePlanGlobalAdmissionIdentityV1,
        "iroha_data_model::block::lane_admission::QueuePlanGlobalAdmissionIdentityV1",
        [
            0x7b, 0xc7, 0x18, 0xbe, 0xac, 0x09, 0xf0, 0xf2, 0x06, 0xbc, 0xbf, 0xae, 0x39, 0xdd,
            0x68, 0xd6
        ]
    );
    check!(
        QueuePlanRouteIncarnationV1,
        "iroha_data_model::block::lane_admission::QueuePlanRouteIncarnationV1",
        [
            0xa9, 0x41, 0x6d, 0xc7, 0x05, 0x6c, 0x5e, 0xa8, 0x50, 0xe4, 0xb3, 0xd3, 0x84, 0xaf,
            0xba, 0xbd
        ]
    );
    check!(
        QueuePlanAdmissionContextV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionContextV1",
        [
            0x94, 0x26, 0x36, 0xea, 0x36, 0x5f, 0xad, 0x98, 0x38, 0x43, 0x98, 0xc7, 0xfc, 0xc5,
            0x22, 0xb2
        ]
    );
    check!(
        QueuePlanAdmissionRegistryKeyV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionRegistryKeyV1",
        [
            0x99, 0x33, 0x1d, 0x58, 0xfd, 0x00, 0x7a, 0x41, 0xe8, 0x24, 0xdb, 0x66, 0xdc, 0x2f,
            0x1e, 0x50
        ]
    );
    check!(
        QueuePlanAdmissionRegistryValueV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionRegistryValueV1",
        [
            0x90, 0xc6, 0xb3, 0x15, 0xa1, 0x47, 0x7b, 0x70, 0x82, 0x41, 0x0b, 0xc1, 0xa5, 0x95,
            0x80, 0x3c
        ]
    );
    check!(
        QueuePlanAdmissionBindingV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionBindingV1",
        [
            0xa3, 0x4e, 0x2e, 0xe9, 0x48, 0x1b, 0x99, 0x35, 0x70, 0xac, 0x91, 0xe0, 0x2e, 0x82,
            0x42, 0xef
        ]
    );
}

#[test]
fn lane_admission_routing_digest_vectors_preserve_existing_domains() {
    let (_, plan, _) = fixture();
    assert_eq!(
        plan.digest(),
        Hash::prehashed([
            0xd4, 0x8b, 0x47, 0xb6, 0x84, 0xf7, 0x12, 0x72, 0x7c, 0x08, 0x99, 0x87, 0x63, 0xaf,
            0xec, 0xcc, 0x90, 0xad, 0x4c, 0x1d, 0xc3, 0xf2, 0x78, 0x4c, 0x12, 0x03, 0xa5, 0x78,
            0x88, 0x50, 0xbd, 0x41
        ])
    );
    assert_eq!(
        RoutingPlan::single(plan.coordinator_route()).digest(),
        Hash::prehashed([
            0x45, 0x8c, 0x5d, 0x39, 0xdb, 0x3b, 0x92, 0x61, 0xaf, 0xc1, 0x54, 0x54, 0xf3, 0xce,
            0xaa, 0xf8, 0x7c, 0x8e, 0x42, 0x72, 0xaf, 0x9a, 0x45, 0xab, 0x74, 0xce, 0x4e, 0x2c,
            0x93, 0x86, 0xe2, 0xfb
        ])
    );
}

fn complete_input_model_fixture() -> LaneAdmittedInputV1 {
    let (network, _, mut binding) = fixture();
    let key = KeyPair::from_seed(vec![0x37; 32], Algorithm::Ed25519);
    let signed = crate::transaction::TransactionBuilder::new(
        network,
        crate::account::AccountId::new(key.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(key.private_key());
    binding.signed_transaction_hash = Some(signed.hash());
    let entrypoint = TransactionEntrypoint::External(signed);
    binding.entrypoint_hash = entrypoint.hash();
    binding.request_id = queue_plan_synced_request_id(&network, binding.entrypoint_hash);
    LaneAdmittedInputV1 {
        entrypoint,
        certificate: QueuePlanAdmissionCertificateV1 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
            binding,
            // Pure wire fixture: this signature is deliberately not an admission attestation.
            attestations: vec![QueuePlanAdmissionAttestationV1 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                validator_index: 0,
                signature: Signature::new(key.private_key(), b"untrusted model fixture"),
            }],
        },
    }
}

#[test]
fn complete_lane_admission_model_roundtrips_certificate_and_exact_entrypoint() {
    let input = complete_input_model_fixture();
    macro_rules! roundtrip {
        ($ty:ty, $value:expr) => {{
            let value: $ty = $value;
            let bytes = norito::encode_canonical(&value).unwrap();
            let decoded: $ty = norito::decode_canonical(&bytes).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
            let json = norito::json::to_json(&value).unwrap();
            assert_eq!(norito::json::from_str::<$ty>(&json).unwrap(), value);
        }};
    }
    roundtrip!(
        QueuePlanAdmissionAttestationV1,
        input.certificate.attestations[0].clone()
    );
    roundtrip!(QueuePlanAdmissionCertificateV1, input.certificate.clone());
    roundtrip!(LaneAdmittedInputV1, input.clone());
    assert_eq!(
        input.routing_plan().unwrap(),
        input.certificate.binding.routing_plan().unwrap()
    );
    let mut changed = input;
    changed.certificate.binding.routing_plan_digest = Hash::new(b"unbound routing plan");
    assert!(changed.routing_plan().is_err());
}

#[test]
fn complete_lane_admission_model_requires_both_slots_and_rejects_duplicate_plan_field() {
    let input = complete_input_model_fixture();
    for key in ["entrypoint", "certificate"] {
        let mut value = norito::json::to_value(&input).unwrap();
        value.as_object_mut().unwrap().remove(key);
        assert!(norito::json::from_value::<LaneAdmittedInputV1>(value).is_err());
    }
    let mut value = norito::json::to_value(&input).unwrap();
    value.as_object_mut().unwrap().insert(
        "routing_plan".to_owned(),
        norito::json::to_value(&input.routing_plan().unwrap()).unwrap(),
    );
    assert!(
        norito::json::from_value::<LaneAdmittedInputV1>(value).is_err(),
        "the plan has exactly one owner in the binding"
    );
    let bytes = norito::encode_canonical(&input.certificate).unwrap();
    assert!(norito::decode_canonical::<LaneAdmittedInputV1>(&bytes).is_err());
    let bytes = norito::encode_canonical(&input).unwrap();
    assert!(norito::decode_canonical::<QueuePlanAdmissionCertificateV1>(&bytes).is_err());
}

#[test]
fn complete_lane_admission_schema_frame_vectors() {
    use norito::schema::identity::{NoritoSchema, frame_hash};
    macro_rules! check {
        ($ty:ty, $name:literal, $hash:expr) => {
            assert_eq!(<$ty as NoritoSchema>::nominal_name(), $name);
            assert_eq!(frame_hash::<$ty>(), $hash);
        };
    }
    check!(
        QueuePlanAdmissionAttestationV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionAttestationV1",
        [
            0x93, 0x4b, 0x2c, 0x59, 0xe3, 0x92, 0xb8, 0x1e, 0x44, 0x4c, 0x6d, 0x20, 0x7f, 0x14,
            0x1b, 0x64
        ]
    );
    check!(
        QueuePlanAdmissionCertificateV1,
        "iroha_data_model::block::lane_admission::QueuePlanAdmissionCertificateV1",
        [
            0xf7, 0xa0, 0x65, 0xd3, 0xad, 0x63, 0xd0, 0x99, 0xf9, 0xaa, 0x5b, 0xb4, 0xa0, 0x1b,
            0x84, 0x0f
        ]
    );
    check!(
        LaneAdmittedInputV1,
        "iroha_data_model::block::lane_admission::LaneAdmittedInputV1",
        [
            0x74, 0xde, 0x06, 0xf5, 0xae, 0x8e, 0x07, 0xd4, 0x03, 0xfd, 0x9b, 0x87, 0xc0, 0x74,
            0x51, 0x7c
        ]
    );
}
