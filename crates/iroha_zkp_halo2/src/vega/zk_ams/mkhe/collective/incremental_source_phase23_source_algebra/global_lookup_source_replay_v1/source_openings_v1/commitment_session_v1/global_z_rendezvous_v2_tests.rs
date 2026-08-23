use super::*;

fn radix_pre_z_v2() -> RadixPreZBindingRecordV2 {
    let mut record = RadixPreZBindingRecordV2 {
        fixed_axes_digest: [0x11; 32],
        materialization_record_digest: [0x22; 32],
        mapping_digest: [0x33; 32],
        commitment_root: [0x44; 32],
        commitment_count: 12_384,
        binding_digest: [0; 32],
    };
    record.binding_digest = radix_pre_z_digest_v2(&record);
    record
}

fn global_pre_z_v2(radix: &RadixPreZBindingRecordV2) -> GlobalPreZBindingRecordV2 {
    let mut record = GlobalPreZBindingRecordV2 {
        proof_session_context_digest: [0x55; 32],
        inventory_root: [0x66; 32],
        radix_pre_z_binding_digest: radix.binding_digest,
        cross_field_pre_z_binding_digest: [0x77; 32],
        global_context_digest: [0x88; 32],
        commitment_count: PRE_Z_PHYSICAL_COMMITMENTS_V2,
        binding_digest: [0; 32],
    };
    record.binding_digest = global_pre_z_digest_v2(&record);
    record
}

fn pre_z_session_v2() -> GlobalLookupCommitmentSessionV2<PreZCompleteV2> {
    let radix = radix_pre_z_v2();
    let global = global_pre_z_v2(&radix);
    GlobalLookupCommitmentSessionV2 {
        live: Some(SessionLiveV2 {
            _owner: PreZInventoryOwnerV2::TestOnly,
            radix_pre_z: radix,
            global_pre_z: global,
            next_physical_ordinal: PRE_Z_PHYSICAL_COMMITMENTS_V2,
            post_z_binding_digest: None,
        }),
        state: PhantomData,
    }
}

fn rendezvous_v2(panic_after_radix: bool) -> GlobalZRendezvousV2 {
    let session = pre_z_session_v2();
    let digest = session.live.as_ref().unwrap().global_pre_z.binding_digest;
    session
        .rendezvous_v2(
            DerivedGlobalZOwnerV2::test_only_v2(Scalar::from_u64(65_537), digest).unwrap(),
            PostZMaterializerAuthorityV2::TestOnly { panic_after_radix },
        )
        .unwrap()
}

fn radix_inputs_v2(count: u32) -> RadixDsInverseInputsV2 {
    RadixDsInverseInputsV2 {
        _seal: PostZMaterializerInputSealV2::TestOnly,
        shared_inverse_root: [0x99; 32],
        commitment_count: count,
    }
}

fn added_inputs_v2() -> AddedLookupInverseInputsV2 {
    AddedLookupInverseInputsV2 {
        _seal: PostZMaterializerInputSealV2::TestOnly,
        added_inverse_root: [0xaa; 32],
        global_inverse_root: [0xbb; 32],
        commitment_count: ADDED_INVERSE_COMMITMENTS_V2,
    }
}

#[test]
fn physical_roles_are_dense_unique_and_exact() {
    let mut next = 0;
    let mut pre_z = 0;
    let mut post_z = 0;
    let mut residual = 0;
    for role in PHYSICAL_ROLES_V2 {
        assert_eq!(role.first_ordinal, next);
        next += role.count;
        match role.phase {
            PhysicalPhaseV2::ChallengeIndependent => pre_z += role.count,
            PhysicalPhaseV2::JointPostZ => post_z += role.count,
            PhysicalPhaseV2::PostDeltaResidual => residual += role.count,
        }
    }
    assert_eq!((pre_z, post_z, residual, next), (39_338, 31_768, 3, 71_109));
    assert_eq!(PHYSICAL_ROLES_V2[14].first_ordinal, 39_338);
    assert_eq!(PHYSICAL_ROLES_V2[21].first_ordinal, 71_106);
}

#[test]
fn alias_map_pairs_radix_and_global_roles_to_one_physical_slot() {
    for ordinal in 0..SHARED_INVERSE_COMMITMENTS_V2 {
        let alias = shared_inverse_alias_v2(ordinal).unwrap();
        assert_eq!(alias.alias_ordinal, ordinal);
        if ordinal < 5_848 {
            assert_eq!(
                alias.radix_purpose,
                LogicalInversePurposeV2::RadixDifference
            );
            assert_eq!(
                alias.global_purpose,
                LogicalInversePurposeV2::GlobalDifference
            );
            assert_eq!(alias.physical_ordinal, 39_338 + ordinal);
        } else {
            assert_eq!(alias.radix_purpose, LogicalInversePurposeV2::RadixSum);
            assert_eq!(alias.global_purpose, LogicalInversePurposeV2::GlobalSum);
            assert_eq!(alias.physical_ordinal, 45_186 + ordinal - 5_848);
        }
    }
    assert!(shared_inverse_alias_v2(11_696).is_err());
}

#[test]
fn manifests_and_binding_records_have_literal_kats() {
    assert_eq!(
        hex::encode(physical_manifest_digest_v2()),
        "651bd890a27de94e7ccc6d20c279ddc87d048143002f1fc460b95f986ef5956c"
    );
    assert_eq!(
        hex::encode(alias_manifest_digest_v2().unwrap()),
        "a1f7d9af090465e75e4db83db3f294f97ef646fb27ab93b9d2482b1cc73372a4"
    );
    let radix = radix_pre_z_v2();
    let global = global_pre_z_v2(&radix);
    assert_eq!(
        hex::encode(radix.binding_digest),
        "bad25802f4fe6b0924b3c20925bf19a3b68051b0b38ac263d6e19900a368c664"
    );
    assert_eq!(
        hex::encode(global.binding_digest),
        "1874e6e30cdf10f6301b7b77f39ba8ea72202e2c29720647be0ffcf3c568c90d"
    );
    let post = rendezvous_v2(false)
        .materialize_post_z_v2(radix_inputs_v2(11_696), added_inputs_v2())
        .unwrap();
    assert_eq!(
        hex::encode(post.radix.binding_digest),
        "dd6e969b1bd0a2fb88a8a177a3a5bbb27cd66f133148aa5641a35a6f428bc744"
    );
    assert_eq!(
        hex::encode(post.global.binding_digest),
        "702d0672e02e5eef07dc963a92c27591bc20466bd7b46a02206194a1a87bca30"
    );
    assert_eq!(
        post.session.live.as_ref().unwrap().next_physical_ordinal,
        POST_Z_COMPLETE_ORDINAL_V2
    );
}

#[test]
fn binding_mutations_and_same_z_context_mismatch_fail_closed() {
    let mut radix = radix_pre_z_v2();
    radix.commitment_root[0] ^= 1;
    assert!(radix.validate_v2().is_err());
    let mut session = pre_z_session_v2();
    session.live.as_mut().unwrap().global_pre_z.inventory_root[0] ^= 1;
    let digest = [0x42; 32];
    assert!(
        session
            .rendezvous_v2(
                DerivedGlobalZOwnerV2::test_only_v2(Scalar::from_u64(65_537), digest).unwrap(),
                PostZMaterializerAuthorityV2::TestOnly {
                    panic_after_radix: false
                },
            )
            .is_err()
    );
    assert!(DerivedGlobalZOwnerV2::test_only_v2(Scalar::from_u64(32_767), [1; 32]).is_err());
}

#[test]
fn independently_rehashed_global_radix_mismatch_is_consumed_before_materialization() {
    let mut session = pre_z_session_v2();
    let live = session.live.as_mut().unwrap();
    live.global_pre_z.radix_pre_z_binding_digest[0] ^= 1;
    live.global_pre_z.binding_digest = global_pre_z_digest_v2(&live.global_pre_z);
    assert_eq!(live.radix_pre_z.validate_v2(), Ok(()));
    assert_eq!(live.global_pre_z.validate_v2(), Ok(()));
    assert_ne!(
        live.global_pre_z.radix_pre_z_binding_digest,
        live.radix_pre_z.binding_digest
    );
    let hostile_transcript_digest = live.global_pre_z.binding_digest;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.rendezvous_v2(
            DerivedGlobalZOwnerV2::test_only_v2(
                Scalar::from_u64(65_537),
                hostile_transcript_digest,
            )
            .unwrap(),
            PostZMaterializerAuthorityV2::TestOnly {
                panic_after_radix: true,
            },
        )
    }));
    assert!(matches!(
        outcome,
        Ok(Err(ZkAmsMkheErrorV1::InvalidPhase23Fold))
    ));
}

#[test]
fn error_and_unwind_poison_the_rendezvous() {
    let mut failed = rendezvous_v2(false);
    assert!(
        failed
            .materialize_post_z_in_place_v2(radix_inputs_v2(11_695), added_inputs_v2())
            .is_err()
    );
    assert!(
        failed
            .materialize_post_z_in_place_v2(radix_inputs_v2(11_696), added_inputs_v2())
            .is_err()
    );

    let mut unwound = rendezvous_v2(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = unwound.materialize_post_z_in_place_v2(radix_inputs_v2(11_696), added_inputs_v2());
    }));
    assert!(result.is_err());
    assert!(
        unwound
            .materialize_post_z_in_place_v2(radix_inputs_v2(11_696), added_inputs_v2())
            .is_err()
    );
}

#[test]
fn accounting_and_every_authority_gate_remain_fail_closed() {
    assert_eq!(PROOF_WIRE_SAVING_BYTES_V2, 385_968);
    assert_eq!(BLINDING_SAVING_BYTES_V2, 374_272);
    assert_eq!(SEMANTIC_SAVING_BYTES_V2, 760_240);
    assert_eq!(AUTH_TAG_SAVING_BYTES_V2, 187_136);
    assert_eq!(FILE_SAVING_BYTES_V2, 947_376);
    assert_eq!(WRITE_AND_SEAL_IO_SAVING_BYTES_V2, 1_894_752);
    assert_eq!(KNOWN_UNIFIED_LOWER_BOUND_BYTES_V2, 32_844_686);
    assert_eq!(PROVISIONAL_MARGIN_BYTES_V2, 709_746);
    assert!(ACCOUNTING_LANGUAGE_V2.starts_with(b"known=32844686+V+F"));
    assert_eq!(
        (
            VECTOR_PROOF_WIRE_BYTES_V2,
            NEW_ENVELOPE_FRAMING_BYTES_V2,
            CONDITIONAL_TOTAL_BYTES_V2,
            CONDITIONAL_MARGIN_BYTES_V2
        ),
        (None, None, None, None)
    );
    for gate in [
        PRE_Z_COMPLETION_INHABITED_V2,
        POST_Z_MATERIALIZERS_INHABITED_V2,
        PROOF_VERIFIED_V2,
        ZERO_KNOWLEDGE_ACCEPTED_V2,
        COMPLETE_ACCOUNTING_QUALIFIED_V2,
        AUTHORITY_MINTED_V2,
        RSS_QUALIFIED_V2,
        OPERATIONAL_RECEIPT_ACCEPTED_V2,
        RELEASE_READY_V2,
    ] {
        assert!(!gate);
    }
}

#[test]
fn source_guards_forbid_escape_and_pin_v1_anchors() {
    let production = include_str!("global_z_rendezvous_v2.rs");
    let tests = include_str!("global_z_rendezvous_v2_tests.rs");
    let parent = include_str!("../commitment_session_v1.rs");
    let challenge_v1 = include_str!("../../../../../global_lookup_statement_v1/challenge_v1.rs");
    assert!(production.lines().count() <= 650);
    assert!(production.len() <= 30_000);
    assert!(tests.lines().count() <= 350);
    assert!(tests.len() <= 18_000);
    assert_eq!(parent.matches("mod global_z_rendezvous_v2;").count(), 1);
    let radix_validation = production.find("live.radix_pre_z.validate_v2()?;").unwrap();
    let global_validation = production
        .find("live.global_pre_z.validate_v2()?;")
        .unwrap();
    let linkage = production
        .find("live.global_pre_z.radix_pre_z_binding_digest != live.radix_pre_z.binding_digest")
        .unwrap();
    let derived_z_check = production
        .find("derived_z.pre_z_transcript_digest != live.global_pre_z.binding_digest")
        .unwrap();
    let owner_construction = production.find("Ok(GlobalZRendezvousV2 {").unwrap();
    assert!(
        radix_validation < global_validation
            && global_validation < linkage
            && linkage < derived_z_check
            && derived_z_check < owner_construction
    );
    for required in [
        "complete_challenge_independent_inventory: Infallible",
        "radix_inverse_materializer: Infallible",
        "global_lookup_transcript: Infallible",
        "DENSE_PHYSICAL_INVENTORY_V2: u32 = 71_109",
        "let mut live = self\n            .live\n            .take()",
        "live.derived_z.scalar.as_ref()",
    ] {
        assert!(production.contains(required), "missing guard: {required}");
    }
    for forbidden in [
        "impl Clone for GlobalLookupCommitmentSessionV2",
        "impl Clone for GlobalZRendezvousV2",
        "impl Deref",
        "fn z_v2",
        "fn scalar_v2",
        "fn into_parts",
        "dyn Fn",
        "callback",
        "Vec<Point>",
        "Ticket",
        "snapshot",
        "Serialize",
        "Deserialize",
    ] {
        assert!(!production.contains(forbidden), "forbidden: {forbidden}");
    }
    assert!(challenge_v1.contains("iroha.zk-ams.v1.phase23.global-lookup.challenge\\0"));
    assert!(challenge_v1.contains("const LOOKUP_Z_ORDINAL_V1: u32 = 0;"));
}
