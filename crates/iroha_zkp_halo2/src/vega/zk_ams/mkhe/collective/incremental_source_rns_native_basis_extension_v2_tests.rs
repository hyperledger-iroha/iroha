use super::*;

fn fixture_axes_v2() -> RnsNativeBasisExtensionAxesV2 {
    RnsNativeBasisExtensionAxesV2 {
        release_profile_digest: [0x11; 32],
        target_profile_digest: [0x22; 32],
        security_certificate_digest: [0x33; 32],
        roster_digest: [0x44; 32],
        key_material_digest: [0x55; 32],
        epoch: 0x0102_0304_0506_0708,
        cpk_transcript_digest: [0x66; 32],
        parties: core::array::from_fn(|index| {
            ZkAmsMkhePartyIdV1::new([index as u8 + 1; 32]).expect("nonzero fixture party")
        }),
    }
}

const FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2: [u8; 32] = [0x77; 32];

fn fixture_completion_v2(
    record_ordinal: u8,
    sample_index: u64,
    emitted_limb_count: u8,
    key_tail_integrity_digest: [u8; 32],
    digest_byte: u8,
) -> RnsNativeCiphertextTailCompletionV2 {
    RnsNativeCiphertextTailCompletionV2 {
        key_tail_integrity_digest,
        record_ordinal,
        sample_index,
        emitted_limb_count,
        coefficient_digest: [digest_byte; 32],
    }
}

fn fixture_lifecycle_v2() -> RnsNativeCiphertextTailLifecycleV2 {
    RnsNativeCiphertextTailLifecycleV2::from_key_tail_integrity_digest_v2(
        FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
    )
    .expect("nonzero private fixture integrity digest")
}

#[test]
fn exact_38_to_40_geometry_and_missing_tail_census_are_frozen() {
    validate_profile_prefix_extension_v2().expect("exact prefix extension");
    assert_eq!(LEGACY_LIMB_COUNT_V2, 38);
    assert_eq!(TARGET_LIMB_COUNT_V2, 40);
    assert_eq!(TAIL_LIMB_COUNT_V2, 2);
    assert_eq!(PUBLIC_POLYNOMIAL_ROLE_COUNT_V2, 88);
    assert_eq!(FULL_OBJECT_COUNT_V2, 3_520);
    assert_eq!(LEGACY_OBJECT_COUNT_V2, 3_344);
    assert_eq!(MISSING_TAIL_OBJECT_COUNT_V2, 176);
    assert_eq!(KEY_TAIL_OBJECT_COUNT_V2, 4);
    assert_eq!(CIPHERTEXT_TAIL_OBJECT_COUNT_V2, 172);
    assert_eq!(CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2, 4);
    assert_eq!(
        RECORD_COUNT_V2 * usize::from(CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2),
        CIPHERTEXT_TAIL_OBJECT_COUNT_V2
    );
    assert!(RNS_NATIVE_BASIS_EXTENSION_CONTRACT_IMPLEMENTED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_PRE_ENTROPY_CALL_SITE_INTEGRATED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_PRODUCTION_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_SOURCE_ADAPTER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_INTEGRATED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_RELEASE_AUTHORIZED_V2);
    assert_eq!(CHUNK_COUNT_PER_OBJECT_V2, 128);
    assert_eq!(OBJECT_BYTES_V2, 1_048_580);
    assert_eq!(TAIL_CAS_BYTES_V2, 184_550_080);
    assert_eq!(
        &ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[..LEGACY_LIMB_COUNT_V2],
        RELEASE_MODULI_V1.as_slice()
    );
    assert_eq!(
        &ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[..LEGACY_LIMB_COUNT_V2],
        RELEASE_NEGACYCLIC_ROOTS_V1.as_slice()
    );
    assert_eq!(
        &ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[LEGACY_LIMB_COUNT_V2..],
        &[1_152_921_504_403_947_521, 1_152_921_504_396_869_633]
    );
    assert_eq!(
        &ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[LEGACY_LIMB_COUNT_V2..],
        &[22_173_257_170_052_426, 24_990_432_311_765_759]
    );
}

#[test]
fn resource_record_counts_only_the_additive_tail_and_exact_named_heap() {
    let resources = &RNS_NATIVE_BASIS_EXTENSION_RESOURCES_V2;
    assert_eq!(resources.full_object_count, 3_520);
    assert_eq!(resources.legacy_object_count, 3_344);
    assert_eq!(resources.missing_tail_object_count, 176);
    assert_eq!(resources.chunks_per_object, 128);
    assert_eq!(resources.object_bytes, 1_048_580);
    assert_eq!(resources.tail_coefficient_count, 23_068_672);
    assert_eq!(resources.tail_cas_bytes, 184_550_080);
    assert_eq!(resources.b_tail_negacyclic_products, 16);
    assert_eq!(resources.c_tail_negacyclic_products, 172);
    assert_eq!(resources.total_tail_negacyclic_work_units, 443_547_648);
    assert_eq!(resources.public_key_tail_coefficient_bytes, 4_194_304);
    assert_eq!(resources.two_limb_workspace_bytes, 2_097_152);
    assert_eq!(resources.builder_named_heap_peak_bytes, 6_291_456);
    assert_eq!(resources.ciphertext_kernel_owned_heap_bytes, 2_097_152);
    assert_eq!(
        resources.ciphertext_workspace_required_pre_entropy_bytes,
        2_097_152
    );
    assert_eq!(resources.ciphertext_kernel_constructor_allocation_bytes, 0);
    assert_eq!(resources.key_owner_plus_kernel_resident_bytes, 6_291_456);
    assert_eq!(resources.synchronous_same_opening_borrowed_bytes, 7_340_064);
    assert_eq!(resources.encoder_scratch_bytes, 8_192);
    assert_eq!(resources.public_a_frame_stack_bytes, 306);
    assert_eq!(resources.party_contribution_digest_bytes, 256);
    assert_eq!(resources.max_public_a_tail_candidates, 33_554_432);
    assert_eq!(resources.max_public_a_tail_xof_bytes, 268_435_456);
}

#[test]
fn public_a_tail_domain_and_limb_frames_have_pinned_shake_kats() {
    // This private fixture deliberately bypasses the governed axes constructor
    // so the XOF frame has stable, human-auditable bytes. Production derivation
    // calls `axes.validate_v2()` before filling a release-size limb.
    let axes = fixture_axes_v2();
    assert_eq!(PUBLIC_A_TAIL_FRAME_BYTES_V2, 306);
    let mut limb_38 = [0_u64; 8];
    let mut limb_39 = [0_u64; 8];
    fill_public_a_tail_from_frame_v2(&axes, 38, &mut limb_38).expect("q38 A prefix");
    fill_public_a_tail_from_frame_v2(&axes, 39, &mut limb_39).expect("q39 A prefix");
    assert_eq!(
        fill_public_a_tail_from_frame_v2(&axes, 37, &mut limb_38),
        Err(RnsNativeBasisExtensionErrorV2::InvalidAxes)
    );
    assert_eq!(
        fill_public_a_tail_from_frame_v2(&axes, 40, &mut limb_38),
        Err(RnsNativeBasisExtensionErrorV2::InvalidAxes)
    );
    assert_eq!(
        limb_38,
        [
            536_412_858_023_351_016,
            416_036_751_681_663_369,
            152_508_371_971_030_417,
            968_426_556_368_064_604,
            1_098_117_046_359_451_042,
            120_509_462_310_281_270,
            107_080_445_097_285_853,
            64_152_501_207_696_264,
        ]
    );
    assert_eq!(
        limb_39,
        [
            851_741_198_444_929_798,
            1_143_852_084_375_076_523,
            533_329_305_546_652_051,
            103_951_699_052_201_155,
            96_355_611_517_435_335,
            160_657_961_869_173_693,
            236_090_013_661_933_871,
            80_963_481_013_915_089,
        ]
    );
    assert_ne!(limb_38, limb_39);

    let mut changed_axes = fixture_axes_v2();
    changed_axes.target_profile_digest[0] ^= 1;
    let mut changed = [0_u64; 8];
    fill_public_a_tail_from_frame_v2(&changed_axes, 38, &mut changed)
        .expect("changed target profile frame");
    assert_ne!(changed, limb_38);

    let mut changed_axes = fixture_axes_v2();
    changed_axes.cpk_transcript_digest[31] ^= 1;
    fill_public_a_tail_from_frame_v2(&changed_axes, 38, &mut changed).expect("changed CPK frame");
    assert_ne!(changed, limb_38);
}

#[test]
fn canonical_tail_ordinals_are_unique_and_match_full_3520_order() {
    let mut previous_full = None;
    for tail_ordinal in 0..MISSING_TAIL_OBJECT_COUNT_V2 {
        let position =
            RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(tail_ordinal).expect("position");
        assert_eq!(
            position.tail_ordinal_v2().expect("round trip"),
            tail_ordinal
        );
        let full = position.full_source_ordinal_v2().expect("full ordinal");
        assert!(full < FULL_OBJECT_COUNT_V2);
        if let Some(previous) = previous_full {
            assert!(full > previous);
        }
        previous_full = Some(full);
    }
    assert_eq!(
        RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(0)
            .unwrap()
            .full_source_ordinal_v2(),
        Ok(38)
    );
    assert_eq!(
        RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(3)
            .unwrap()
            .full_source_ordinal_v2(),
        Ok(79)
    );
    let last_c0 = RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(89).unwrap();
    assert_eq!(last_c0.role_v2(), RnsNativeTailObjectRoleV2::CiphertextC0);
    assert_eq!(last_c0.record_ordinal_v2(), Some(42));
    assert_eq!(last_c0.limb_v2(), 39);
    assert_eq!(last_c0.full_source_ordinal_v2(), Ok(1_799));
    let first_c1 = RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(90).unwrap();
    assert_eq!(first_c1.role_v2(), RnsNativeTailObjectRoleV2::CiphertextC1);
    assert_eq!(first_c1.record_ordinal_v2(), Some(0));
    assert_eq!(first_c1.limb_v2(), 38);
    assert_eq!(first_c1.full_source_ordinal_v2(), Ok(1_838));
    assert_eq!(
        RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(175)
            .unwrap()
            .full_source_ordinal_v2(),
        Ok(3_519)
    );
    assert_eq!(
        RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(176),
        Err(RnsNativeBasisExtensionErrorV2::InvalidPosition)
    );
}

#[test]
fn canonical_encoder_requires_prefix_and_all_128_natural_order_chunks() {
    let position = RnsNativeTailSourcePositionV2::new_ciphertext_v2(
        RnsNativeTailObjectRoleV2::CiphertextC1,
        42,
        39,
    )
    .unwrap();
    let mut coefficients = try_zeroed_u64_vec_v2(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1).unwrap();
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient = index as u64 * 17 + 5;
    }

    let incomplete = RnsNativeTailObjectEncoderV2::new_v2(position, &coefficients).unwrap();
    assert_eq!(
        incomplete.finish_v2().err(),
        Some(RnsNativeBasisExtensionErrorV2::IncompleteObject)
    );

    let mut poisoned = RnsNativeTailObjectEncoderV2::new_v2(position, &coefficients).unwrap();
    let mut chunk = [0_u8; CHUNK_BYTES_V2];
    assert_eq!(
        poisoned.write_next_chunk_v2(&mut chunk),
        Err(RnsNativeBasisExtensionErrorV2::IncompleteObject)
    );
    let mut prefix = [0_u8; COUNT_PREFIX_BYTES_V2];
    assert_eq!(
        poisoned.write_count_prefix_v2(&mut prefix),
        Err(RnsNativeBasisExtensionErrorV2::Poisoned)
    );

    let mut encoder = RnsNativeTailObjectEncoderV2::new_v2(position, &coefficients).unwrap();
    encoder.write_count_prefix_v2(&mut prefix).unwrap();
    assert_eq!(prefix, [0x00, 0x02, 0x00, 0x00]);
    for expected_chunk in 0..CHUNK_COUNT_PER_OBJECT_V2 {
        assert_eq!(
            encoder.write_next_chunk_v2(&mut chunk),
            Ok(expected_chunk as u16)
        );
        if expected_chunk == 0 {
            assert_eq!(&chunk[..8], &5_u64.to_be_bytes());
            assert_eq!(&chunk[8..16], &22_u64.to_be_bytes());
        }
    }
    assert_eq!(&chunk[CHUNK_BYTES_V2 - 8..], &2_228_212_u64.to_be_bytes());
    let encoded = encoder.finish_v2().unwrap();
    assert_eq!(encoded.position, position);
    assert_eq!(encoded.encoded_bytes, 1_048_580);
    assert_eq!(
        hex::encode(encoded.encoded_bytes_digest),
        "4d6bae724b49ee963b855b19584420cc7222bd40e310dfefa5ed7d0d75fafce1"
    );

    coefficients[0] = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[39];
    assert_eq!(
        RnsNativeTailObjectEncoderV2::new_v2(position, &coefficients).err(),
        Some(RnsNativeBasisExtensionErrorV2::InvalidResidue)
    );
    assert_eq!(
        RnsNativeTailObjectEncoderV2::new_v2(position, &coefficients[..1]).err(),
        Some(RnsNativeBasisExtensionErrorV2::InvalidResidue)
    );
}

#[test]
fn b_and_c_tail_coefficient_rules_have_pinned_centered_t256_kats() {
    let q38 = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[38];
    let products = [0, 1, q38 - 1, 123, 456];
    let errors = [-2, -1, 0, 1, 2];
    let expected_b = [
        1_052_622_903_393_880_903,
        1_102_772_203_898_914_211,
        1,
        50_149_300_505_033_186,
        100_298_601_010_066_162,
    ];
    for ((product, error), expected) in products.into_iter().zip(errors).zip(expected_b) {
        assert_eq!(
            party_b_tail_contribution_coefficient_v2(product, error, q38),
            Ok(expected)
        );
    }

    let mut c0 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let mut c0_error = vec![0_i64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let mut plaintext = vec![[0_u8; 32]; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    c0[..5].copy_from_slice(&products);
    c0_error[..5].copy_from_slice(&errors);
    plaintext[1][31] = 1;
    plaintext[2][31] = 2;
    plaintext[3] = VEGA_T256_SCALAR_MODULUS_BE_V1;
    plaintext[3][31] -= 1;
    plaintext[4][31] = 17;
    add_error_and_optional_message_v2(&mut c0, &c0_error, Some(&plaintext), q38).unwrap();
    assert_eq!(
        &c0[..5],
        &[
            1_052_622_903_393_880_903,
            1_102_772_203_898_914_214,
            1,
            50_149_300_505_033_431,
            100_298_601_010_067_091,
        ]
    );
    assert!(c0[5..].iter().all(|coefficient| *coefficient == 0));

    let q39 = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[39];
    let mut c1 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    c1[..5].copy_from_slice(&[0, 1, q39 - 1, 123, 456]);
    add_error_and_optional_message_v2(&mut c1, &c0_error, None, q39).unwrap();
    assert_eq!(
        &c1[..5],
        &[
            503_167_205_062_021_592,
            251_583_602_531_010_797,
            1_152_921_504_396_869_632,
            901_337_901_865_858_960,
            649_754_299_334_848_497,
        ]
    );
}

#[test]
fn both_tail_roots_have_release_degree_negacyclic_product_kats() {
    for limb in LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2 {
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
        let root = ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb];
        let mut left = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        let mut right = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        let mut signed_right = vec![0_i64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        left[0] = 3;
        left[1] = 5;
        left[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1] = 7;
        signed_right[0] = 1;
        signed_right[2] = -1;
        signed_right[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1] = 1;
        negacyclic_multiply_signed_rhs_two_limb_v1(
            &mut left,
            &mut right,
            &signed_right,
            modulus,
            root,
        )
        .expect("tail-root negacyclic product");
        assert_eq!(left[0], modulus - 2);
        assert_eq!(left[1], 12);
        assert_eq!(left[2], modulus - 3);
        assert_eq!(left[3], modulus - 5);
        assert_eq!(left[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 2], modulus - 7);
        assert_eq!(left[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1], 10);
        for (index, coefficient) in left.iter().enumerate() {
            if ![
                0,
                1,
                2,
                3,
                ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 2,
                ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1,
            ]
            .contains(&index)
            {
                assert_eq!(*coefficient, 0);
            }
        }
        clear_u64_slice_v2(&mut right);
    }
}

#[test]
fn scoped_workspace_is_cleared_when_an_unwind_is_caught() {
    let mut workspace = RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2().unwrap();
    let rejected: Result<(), RnsNativeBasisExtensionErrorV2> = (|| {
        let mut lease = workspace.lease_v2();
        lease.left_mut()[1] = 11;
        lease.right_mut()[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 2] = 13;
        Err(RnsNativeBasisExtensionErrorV2::VisitorRejected)
    })();
    assert_eq!(
        rejected,
        Err(RnsNativeBasisExtensionErrorV2::VisitorRejected)
    );
    assert!(
        workspace
            .workspace
            .left
            .as_slice()
            .iter()
            .all(|value| *value == 0)
    );
    assert!(
        workspace
            .workspace
            .right
            .as_slice()
            .iter()
            .all(|value| *value == 0)
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut lease = workspace.lease_v2();
        lease.left_mut()[0] = 7;
        lease.right_mut()[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1] = 9;
        panic!("exercise unwind cleanup");
    }));
    assert!(result.is_err());
    assert!(
        workspace
            .workspace
            .left
            .as_slice()
            .iter()
            .all(|value| *value == 0)
    );
    assert!(
        workspace
            .workspace
            .right
            .as_slice()
            .iter()
            .all(|value| *value == 0)
    );
}

#[test]
fn allocated_workspace_owner_makes_the_live_kernel_constructor_allocation_free() {
    let workspace = RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2().unwrap();
    assert_eq!(
        workspace.workspace.left.as_slice().len(),
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );
    assert_eq!(
        workspace.workspace.right.as_slice().len(),
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );
    assert_eq!(
        workspace.workspace.left.0.capacity(),
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );
    assert_eq!(
        workspace.workspace.right.0.capacity(),
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );

    let source = include_str!("incremental_source_rns_native_basis_extension_v2.rs");
    let allocator = source
        .split_once("fn allocate_workspace_v2(")
        .and_then(|(_, suffix)| suffix.split_once("fn lease_v2(").map(|(body, _)| body))
        .expect("explicit workspace allocator");
    assert!(allocator.contains("ZeroizingTwoLimbWorkspaceV2::new_v2()?"));
    let live_constructor = source
        .split_once("impl<'key, 'opening> RnsNativeCiphertextTailKernelV2")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("pub(super) fn emit_v2")
                .map(|(constructor, _)| constructor)
        })
        .expect("live kernel constructor");
    let opening_owner = source
        .split_once("pub(super) struct RnsNativeSynchronousSameOpeningBorrowV2")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("impl<'opening> RnsNativeSynchronousSameOpeningBorrowV2")
                .map(|(fields, _)| fields)
        })
        .expect("same-opening owner fields");
    assert!(opening_owner.contains("workspace: RnsNativeCiphertextTailWorkspaceOwnerV2"));
    assert!(live_constructor.contains("opening: RnsNativeSynchronousSameOpeningBorrowV2"));
    assert!(!live_constructor.contains("workspace: RnsNativeCiphertextTailWorkspaceOwnerV2"));
    assert!(!live_constructor.contains("ZeroizingTwoLimbWorkspaceV2::new_v2"));
    assert!(!live_constructor.contains("try_zeroed_u64_vec_v2"));
    assert!(!live_constructor.contains("try_reserve_exact"));
}

#[test]
fn synchronous_opening_requires_matching_sample_ordinal_and_nonzero_inherited_nonce() {
    let mut ephemeral = vec![0_i64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    ephemeral[0] = 1;
    let canonical_plaintext = vec![[0_u8; 32]; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let error_zero = vec![0_i64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let error_one = vec![0_i64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let nonzero_nonce = [0x88; 32];
    let zero_nonce = [0_u8; 32];

    {
        let valid = RnsNativeSynchronousSameOpeningBorrowV2 {
            workspace: RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2().unwrap(),
            key_tail_integrity_digest: FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            record_ordinal: 7,
            sample_index: 7,
            canonical_plaintext: &canonical_plaintext,
            ephemeral: &ephemeral,
            error_zero: &error_zero,
            error_one: &error_one,
            encryption_nonce: &nonzero_nonce,
        };
        assert_eq!(valid.validate_v2(), Ok(()));
    }
    {
        let wrong_sample = RnsNativeSynchronousSameOpeningBorrowV2 {
            workspace: RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2().unwrap(),
            key_tail_integrity_digest: FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            record_ordinal: 7,
            sample_index: 8,
            canonical_plaintext: &canonical_plaintext,
            ephemeral: &ephemeral,
            error_zero: &error_zero,
            error_one: &error_one,
            encryption_nonce: &nonzero_nonce,
        };
        assert_eq!(
            wrong_sample.validate_v2(),
            Err(RnsNativeBasisExtensionErrorV2::InvalidSameOpening)
        );
    }
    {
        let zero_nonce_opening = RnsNativeSynchronousSameOpeningBorrowV2 {
            workspace: RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2().unwrap(),
            key_tail_integrity_digest: FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            record_ordinal: 7,
            sample_index: 7,
            canonical_plaintext: &canonical_plaintext,
            ephemeral: &ephemeral,
            error_zero: &error_zero,
            error_one: &error_one,
            encryption_nonce: &zero_nonce,
        };
        assert_eq!(
            zero_nonce_opening.validate_v2(),
            Err(RnsNativeBasisExtensionErrorV2::InvalidSameOpening)
        );
    }
}

#[test]
fn lifecycle_accepts_exactly_records_zero_through_42_and_binds_every_completion() {
    let mut canonical = fixture_lifecycle_v2();
    let mut changed = fixture_lifecycle_v2();
    for record in 0..RECORD_COUNT_V2 as u8 {
        canonical
            .accept_record_completion_v2(fixture_completion_v2(
                record,
                u64::from(record),
                CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2,
                FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
                record + 1,
            ))
            .expect("strict canonical record order");
        changed
            .accept_record_completion_v2(fixture_completion_v2(
                record,
                u64::from(record),
                CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2,
                FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
                if record == 21 { 0xfe } else { record + 1 },
            ))
            .expect("strict changed record order");
    }
    let canonical = canonical.finish_v2().expect("complete lifecycle");
    let changed = changed.finish_v2().expect("complete changed lifecycle");
    assert_eq!(canonical.record_count, 43);
    assert_eq!(canonical.emitted_limb_count, 172);
    assert_ne!(canonical.completion_digest, [0; 32]);
    assert_ne!(canonical.completion_digest, changed.completion_digest);
}

#[test]
fn lifecycle_rejects_duplicate_hole_out_of_order_and_incomplete_runs() {
    let mut out_of_order = fixture_lifecycle_v2();
    assert_eq!(
        out_of_order.accept_record_completion_v2(fixture_completion_v2(
            1,
            1,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            1,
        )),
        Err(RnsNativeBasisExtensionErrorV2::InvalidRecordOrder)
    );
    assert_eq!(
        out_of_order.accept_record_completion_v2(fixture_completion_v2(
            0,
            0,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            1,
        )),
        Err(RnsNativeBasisExtensionErrorV2::Poisoned)
    );

    let mut duplicate = fixture_lifecycle_v2();
    duplicate
        .accept_record_completion_v2(fixture_completion_v2(
            0,
            0,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            1,
        ))
        .unwrap();
    assert_eq!(
        duplicate.accept_record_completion_v2(fixture_completion_v2(
            0,
            0,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            1,
        )),
        Err(RnsNativeBasisExtensionErrorV2::InvalidRecordOrder)
    );

    let mut hole = fixture_lifecycle_v2();
    hole.accept_record_completion_v2(fixture_completion_v2(
        0,
        0,
        4,
        FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
        1,
    ))
    .unwrap();
    assert_eq!(
        hole.accept_record_completion_v2(fixture_completion_v2(
            2,
            2,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            3,
        )),
        Err(RnsNativeBasisExtensionErrorV2::InvalidRecordOrder)
    );

    let mut incomplete = fixture_lifecycle_v2();
    for record in 0..42_u8 {
        incomplete
            .accept_record_completion_v2(fixture_completion_v2(
                record,
                u64::from(record),
                4,
                FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
                record + 1,
            ))
            .unwrap();
    }
    assert_eq!(
        incomplete.finish_v2().err(),
        Some(RnsNativeBasisExtensionErrorV2::IncompleteLifecycle)
    );
}

#[test]
fn lifecycle_rejects_sample_limb_digest_and_key_mismatches_then_stays_poisoned() {
    assert_eq!(
        RnsNativeCiphertextTailLifecycleV2::from_key_tail_integrity_digest_v2([0; 32]).err(),
        Some(RnsNativeBasisExtensionErrorV2::InvalidRecordCompletion)
    );
    for invalid in [
        fixture_completion_v2(0, 1, 4, FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2, 1),
        fixture_completion_v2(0, 0, 3, FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2, 1),
        fixture_completion_v2(0, 0, 4, FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2, 0),
        fixture_completion_v2(0, 0, 4, [0x99; 32], 1),
    ] {
        let mut lifecycle = fixture_lifecycle_v2();
        assert_eq!(
            lifecycle.accept_record_completion_v2(invalid),
            Err(RnsNativeBasisExtensionErrorV2::InvalidRecordCompletion)
        );
        assert_eq!(
            lifecycle.accept_record_completion_v2(fixture_completion_v2(
                0,
                0,
                4,
                FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
                1,
            )),
            Err(RnsNativeBasisExtensionErrorV2::Poisoned)
        );
    }
}

#[test]
fn lifecycle_transition_remains_poisoned_when_an_unwind_is_caught() {
    let mut lifecycle = fixture_lifecycle_v2();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        lifecycle.begin_transition_v2().unwrap();
        panic!("exercise lifecycle unwind fail-closure");
    }));
    assert!(result.is_err());
    assert_eq!(
        lifecycle.accept_record_completion_v2(fixture_completion_v2(
            0,
            0,
            4,
            FIXTURE_KEY_TAIL_INTEGRITY_DIGEST_V2,
            1,
        )),
        Err(RnsNativeBasisExtensionErrorV2::Poisoned)
    );
}

struct PositionCollectorV2(Vec<RnsNativeTailSourcePositionV2>);

impl RnsNativeTailCoefficientVisitorV2 for PositionCollectorV2 {
    fn visit_tail_coefficients_v2(
        &mut self,
        position: RnsNativeTailSourcePositionV2,
        coefficients: &[u64],
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        assert_eq!(coefficients.len(), ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1);
        self.0.push(position);
        Ok(())
    }
}

#[test]
fn key_tail_handoff_is_a_b_and_has_no_terminal_result() {
    let tail_coefficients = TAIL_LIMB_COUNT_V2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
    let owner = RnsNativeCollectiveKeyTailOwnerV2 {
        axes: fixture_axes_v2(),
        public_a_tail: vec![0; tail_coefficients],
        collective_b_tail: vec![0; tail_coefficients],
        integrity_digest: [0x77; 32],
    };
    let mut visitor = PositionCollectorV2(Vec::new());
    owner
        .visit_key_tail_coefficients_v2(&mut visitor)
        .expect("key tail visit");
    assert_eq!(visitor.0.len(), 4);
    assert_eq!(visitor.0[0].tail_ordinal_v2(), Ok(0));
    assert_eq!(visitor.0[1].tail_ordinal_v2(), Ok(1));
    assert_eq!(visitor.0[2].tail_ordinal_v2(), Ok(2));
    assert_eq!(visitor.0[3].tail_ordinal_v2(), Ok(3));
}

#[test]
fn source_inventory_keeps_the_tranche_private_move_only_and_fail_closed() {
    let source = include_str!("incremental_source_rns_native_basis_extension_v2.rs");
    let resource_initializer = source
        .split_once("RNS_NATIVE_BASIS_EXTENSION_RESOURCES_V2:")
        .and_then(|(_, suffix)| suffix.split_once("};").map(|(initializer, _)| initializer))
        .expect("resource initializer");
    assert_eq!(
        resource_initializer.matches("legacy_object_count:").count(),
        1
    );
    let ordinal_transition = source
        .split_once("fn from_tail_ordinal_v2(")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("pub(super) const fn role_v2")
                .map(|(transition, _)| transition)
        })
        .expect("tail-ordinal transition");
    assert_eq!(
        ordinal_transition
            .matches("return Self::new_key_v2(")
            .count(),
        2
    );
    assert!(source.contains("ZkAmsMkheAdmittedCpkPartyV1"));
    assert!(source.contains("absorb_v1_admitted_party_v2"));
    assert!(source.contains("state.secret().coefficients.as_slice()"));
    assert!(source.contains("state.public_error().coefficients.as_slice()"));
    assert!(source.contains("from_v1_synchronous_callback_v2"));
    assert!(source.contains("workspace: RnsNativeCiphertextTailWorkspaceOwnerV2"));
    assert!(source.contains("workspace.validate_workspace_allocation_v2()?"));
    assert!(source.contains("canonical_plaintext: &'opening [[u8; 32]]"));
    assert!(source.contains("ephemeral: &'opening [i64]"));
    assert!(source.contains("error_zero: &'opening [i64]"));
    assert!(source.contains("error_one: &'opening [i64]"));
    assert!(source.contains("self.sample_index != u64::from(self.record_ordinal)"));
    assert!(source.contains("*self.encryption_nonce == [0; 32]"));
    assert!(
        source
            .matches("negacyclic_multiply_signed_rhs_two_limb_v1")
            .count()
            >= 3
    );
    assert!(source.contains("self.poisoned = true"));
    assert!(source.contains("impl Drop for ZeroizingWorkspaceLeaseV2"));
    assert!(source.contains("impl Drop for ZeroizingU64VecV2"));
    assert!(source.contains("RnsNativeCiphertextTailWorkspaceOwnerV2"));
    assert!(source.contains("allocate_workspace_v2"));
    assert!(!source.contains("preallocate_before_entropy_v2"));
    assert!(source.contains("ciphertext_workspace_required_pre_entropy_bytes"));
    assert!(!source.contains("ciphertext_workspace_preallocated_before_entropy_bytes"));
    assert!(source.contains("RnsNativeCiphertextTailLifecycleV2"));
    assert!(source.contains("CIPHERTEXT_TAIL_LIFECYCLE_DOMAIN_V2"));
    assert!(
        source.contains("completion.emitted_limb_count != CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2")
    );
    assert!(source.contains("completion_hash.update(&completion.coefficient_digest)"));
    assert_eq!(
        source
            .matches("RnsNativeCiphertextTailCompletionV2 {")
            .count(),
        2
    );
    assert!(!source.contains("pub(super) fn from_key_tail_integrity_digest_v2"));
    assert!(!source.contains("impl Clone for RnsNativeCollectiveKeyTail"));
    assert!(!source.contains("impl Clone for RnsNativeCiphertextTailKernel"));
    assert!(!source.contains("impl Clone for RnsNativeCiphertextTailCompletion"));
    assert!(!source.contains("impl Clone for RnsNativeCiphertextTailAggregateChecksum"));
    assert!(!source.contains("impl Clone for RnsNativeCiphertextTailWorkspaceOwner"));
    assert!(!source.contains("impl Clone for RnsNativeCiphertextTailLifecycle"));
    assert!(!source.contains("impl Copy for RnsNativeCiphertextTailCompletion"));
    assert!(!source.contains("impl Copy for RnsNativeCiphertextTailAggregateChecksum"));
    assert!(!source.contains("impl Copy for RnsNativeCiphertextTailWorkspaceOwner"));
    assert!(!source.contains("impl Copy for RnsNativeCiphertextTailLifecycle"));
    assert!(!source.contains("RnsNativePublicPolynomialCoefficientSourceV1"));
    assert!(!source.contains("ZkAmsMkheDirectObjectPublication"));
    assert!(!source.contains("from_legacy"));
    assert!(!source.contains("crt_interpolate"));
    assert!(!source.contains("lift_legacy_residue"));
    assert!(source.contains("PREFIX_RECOMPUTATION_ALLOWED_V2: bool = false"));
    assert!(source.contains("FAKE_TAIL_LIFT_ALLOWED_V2: bool = false"));
    assert!(source.contains("CONTRACT_IMPLEMENTED_V2: bool = true"));
    assert!(source.contains("PRE_ENTROPY_CALL_SITE_INTEGRATED_V2: bool = false"));
    assert_eq!(
        source
            .matches("RNS_NATIVE_BASIS_EXTENSION_PRE_ENTROPY_CALL_SITE_INTEGRATED_V2")
            .count(),
        3
    );
    assert!(source.contains("PRODUCTION_OWNER_AVAILABLE_V2: bool = false"));
    assert!(source.contains("SOURCE_ADAPTER_AVAILABLE_V2: bool = false"));
    assert!(source.contains("INTEGRATED_V2: bool = false"));
    assert!(source.contains("RELEASE_AUTHORIZED_V2: bool = false"));
    assert!(source.contains("Binding it to V1 authority/receipts"));
    assert!(source.contains("CAS publication remain"));
    assert!(source.contains("cannot prove that a future parent call site"));
}

#[test]
fn parent_declares_exactly_one_private_basis_extension_child_v2() {
    let parent_source = include_str!("incremental_source.rs");
    let path_attribute = "#[path = \"incremental_source_rns_native_basis_extension_v2.rs\"]";
    let private_declaration = "mod incremental_source_rns_native_basis_extension_v2;";

    assert_eq!(
        parent_source
            .lines()
            .filter(|line| line.trim() == path_attribute)
            .count(),
        1
    );
    assert_eq!(
        parent_source
            .lines()
            .filter(|line| line.trim() == private_declaration)
            .count(),
        1
    );
    assert!(!parent_source.lines().any(|line| {
        let line = line.trim();
        line.contains("incremental_source_rns_native_basis_extension_v2")
            && (line.starts_with("pub mod ")
                || line.starts_with("pub(")
                || line.starts_with("pub "))
    }));
}
