//! Portable admission input controls; structural validity grants no custody or authority.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn fixture() -> (
    RoutingPlan,
    QueuePlanAdmissionContextV1,
    QueuePlanGlobalAdmissionIdentityV1,
) {
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
    let identity = QueuePlanGlobalAdmissionIdentityV1 {
        version: QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V1,
        // These are untrusted identity bytes, with no registry or certificate authority.
        network_id_digest: Hash::new(b"lane admission model network"),
        request_id: Hash::new(b"lane admission model request"),
    };
    (plan, context, identity)
}

#[test]
fn lane_admission_current_dtos_roundtrip_canonical_and_json() {
    let (plan, context, identity) = fixture();
    context.validate_for_routing_plan(&plan).unwrap();
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
    roundtrip!(QueuePlanGlobalAdmissionIdentityV1, identity);
    roundtrip!(
        QueuePlanRouteIncarnationV1,
        context.route_incarnations[0].clone()
    );
    roundtrip!(QueuePlanAdmissionContextV1, context.clone());
}

#[test]
fn lane_admission_routing_normalizes_only_at_explicit_constructor() {
    let (canonical, context, _) = fixture();
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
    assert_eq!(context.routing_plan().unwrap(), canonical);
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
    assert!(context.validate_for_routing_plan(&decoded).is_err());
    let mut bad_digest = native.clone();
    bad_digest.plan_digest = Hash::new(b"forged plan digest");
    assert!(
        context
            .validate_for_routing_plan(&RoutingPlan::NativeAmx(bad_digest))
            .is_err()
    );
    let single = RoutingPlan::single(native.coordinator.route);
    let mut context = context.clone();
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
    let (plan, context, _) = fixture();
    let original = context;
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
        leg.validator_count = u16::try_from(MAX_LANE_CONSENSUS_VALIDATORS + 1).unwrap();
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
    let (plan, context, _) = fixture();
    let coordinator = plan.coordinator_route();
    for count in [0, MAX_QUEUE_PLAN_NATIVE_AMX_PARTICIPANTS_V1 + 1] {
        let participants = (0..count)
            .map(|index| {
                RouteLeg::new(
                    RoutingDecision::new(
                        LaneId::new(u32::try_from(index).unwrap() + 10),
                        DataSpaceId::new(index as u64 + 20),
                    ),
                    RouteLegRole::Participant,
                )
            })
            .collect();
        let plan = RoutingPlan::native_amx(coordinator, participants);
        assert!(context.validate_for_routing_plan(&plan).is_err());
    }
    let RoutingPlan::NativeAmx(mut native) = plan else {
        unreachable!()
    };
    native.participants.push(native.participants[0]);
    assert!(
        context
            .validate_for_routing_plan(&RoutingPlan::NativeAmx(native))
            .is_err()
    );
}

#[test]
fn lane_admission_routing_digests_ignore_ambient_codec_layout() {
    let (plan, _, _) = fixture();
    let single = RoutingPlan::single(plan.coordinator_route());
    let native_digest = plan.digest();
    let single_digest = single.digest();
    let alternative =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _guard = norito::core::DecodeFlagsGuard::enter(alternative);
    assert_eq!(plan.digest(), native_digest);
    assert_eq!(single.digest(), single_digest);
}

#[test]
fn lane_admission_json_requires_nullable_slots_and_enum_tags() {
    let (plan, context, _) = fixture();
    let mut genesis_context = context;
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
    let (plan, context, _) = fixture();
    let mut bytes = norito::encode_canonical(&context).unwrap();
    assert!(norito::decode_canonical::<QueuePlanGlobalAdmissionIdentityV1>(&bytes).is_err());
    bytes.push(0);
    assert!(norito::decode_canonical::<QueuePlanAdmissionContextV1>(&bytes).is_err());
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
}

#[test]
fn lane_admission_routing_digest_vectors_use_current_domains() {
    let (plan, _, _) = fixture();
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
