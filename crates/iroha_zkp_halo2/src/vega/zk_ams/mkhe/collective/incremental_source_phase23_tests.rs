use super::*;
fn test_digest_axes_v1() -> Phase23BundleDigestAxesV1 {
    Phase23BundleDigestAxesV1 {
        profile_digest: [1; 32],
        roster_digest: [2; 32],
        materialized_transcript_digest: [3; 32],
        batch_id: [4; 32],
        ordered_batch_input_digest: [5; 32],
        fold_count: 6,
        shape: ZkAmsPhase23AccumulatorShapeV1::new(
            PHASE23_X_VALUES_V1,
            PHASE23_U_AND_E_VALUES_V1,
            PHASE23_RE_VALUES_V1,
            PHASE23_W_VALUES_V1,
            PHASE23_RW_VALUES_V1,
        )
        .unwrap(),
        materialized_digest: [7; 32],
        key_digest: [8; 32],
        key_authority_digest: [9; 32],
        key_epoch: 10,
        source_receipt_digest: [11; 32],
        public_artifact_manifest_bound: true,
    }
}
fn test_manifest_digests_v1() -> [[u8; 32]; PHASE23_RECORD_COUNT_V1] {
    core::array::from_fn(|ordinal| [u8::try_from(ordinal + 1).unwrap(); 32])
}
#[test]
fn exact_record_schedule_is_x_u_e_re_w_rw() {
    let positions = (0..PHASE23_RECORD_COUNT_V1)
        .map(|ordinal| phase23_record_position_v1(u16::try_from(ordinal).unwrap()).unwrap())
        .collect::<Vec<_>>();
    let expected = [
        (ZkAmsPhase23RnsLinkFamilyV1::X, 1_usize),
        (ZkAmsPhase23RnsLinkFamilyV1::U, 16),
        (ZkAmsPhase23RnsLinkFamilyV1::E, 16),
        (ZkAmsPhase23RnsLinkFamilyV1::RE, 1),
        (ZkAmsPhase23RnsLinkFamilyV1::W, 8),
        (ZkAmsPhase23RnsLinkFamilyV1::RW, 1),
    ];
    let mut offset = 0;
    for (family, count) in expected {
        for (chunk_index, position) in positions[offset..offset + count].iter().enumerate() {
            assert_eq!(position.family, family);
            assert_eq!(usize::from(position.chunk_index), chunk_index);
            assert_eq!(usize::from(position.family_chunk_count), count);
            assert_ne!(position.layout_v1().unwrap().digest, [0; 32]);
            assert!(position.used_slots_v1().unwrap() > 0);
        }
        offset += count;
    }
    assert_eq!(offset, PHASE23_RECORD_COUNT_V1);
    assert_eq!(positions[0].used_slots_v1().unwrap(), 89);
    assert_eq!(positions[33].used_slots_v1().unwrap(), 1_024);
    assert_eq!(positions[42].used_slots_v1().unwrap(), 512);
    assert!(phase23_record_position_v1(43).is_err());
}
#[test]
fn hostile_schedule_coordinates_fail_before_the_encryption_core() {
    let position = phase23_record_position_v1(5).unwrap();
    let layout = position.layout_v1().unwrap();
    let mut packed = ZkAmsT256PackedPlaintextV1 {
        version: 1,
        profile_digest: layout.profile_digest,
        layout_digest: layout.digest,
        chunk_index: u32::from(position.chunk_index),
        used_slots: position.used_slots_v1().unwrap(),
        coefficients: Vec::new(),
        digest: [1; 32],
    };
    assert_eq!(
        require_expected_packed_coordinate_v1(position, layout, &packed).unwrap(),
        layout.slots_per_chunk
    );
    packed.chunk_index += 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.chunk_index = u32::from(position.chunk_index);
    packed.used_slots -= 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.used_slots = position.used_slots_v1().unwrap();
    packed.layout_digest[0] ^= 1;
    assert!(require_expected_packed_coordinate_v1(position, layout, &packed).is_err());
    packed.layout_digest = layout.digest;
    let foreign_layout = phase23_record_position_v1(0).unwrap().layout_v1().unwrap();
    assert!(require_expected_packed_coordinate_v1(position, foreign_layout, &packed).is_err());
}
#[test]
fn named_peak_includes_the_preallocated_secret_chunk_pool() {
    assert_eq!(PHASE23_SECRET_CHUNK_POOL_PAYLOAD_BYTES_V1, 7_340_064);
    assert!(PHASE23_SECRET_CHUNK_POOL_METADATA_BYTES_V1 > 0);
    assert!(PHASE23_NAMED_HEAP_PEAK_BYTES_V1 < 160 * 1_048_576);
    assert_eq!(PHASE23_ONE_PACKED_CHUNK_BYTES_V1, 4 * 1_048_576);
    assert_eq!(PHASE23_DECODER_WORKSPACE_BYTES_V1, 8 * 1_048_576);
    assert_eq!(PHASE23_COMPACT_MANIFEST_OWNER_BYTES_V1, 4_718_592);
}
#[test]
fn bundle_digest_has_an_independent_exact_kat_and_changes_every_bound_axis() {
    let axes = test_digest_axes_v1();
    let manifests = test_manifest_digests_v1();
    let digest = phase23_bundle_digest_from_frames_v1(axes, &manifests).unwrap();
    assert_eq!(
        hex::encode(digest),
        "99c927f9cf6b3772d28ae3776026266a17a8a9ea73082f7e551fc86a0ca4b1b6"
    );
    let mut changed_axes = axes;
    changed_axes.source_receipt_digest[0] ^= 1;
    assert_ne!(
        phase23_bundle_digest_from_frames_v1(changed_axes, &manifests).unwrap(),
        digest
    );
    let mut changed_shape = axes;
    changed_shape.shape.x += 1;
    assert!(phase23_bundle_digest_from_frames_v1(changed_shape, &manifests).is_err());
    let mut changed_manifest = manifests;
    changed_manifest[17][0] ^= 1;
    assert_ne!(
        phase23_bundle_digest_from_frames_v1(axes, &changed_manifest).unwrap(),
        digest
    );
    let mut unbound = axes;
    unbound.public_artifact_manifest_bound = false;
    assert!(phase23_bundle_digest_from_frames_v1(unbound, &manifests).is_err());
}
#[test]
fn structural_gate_preserves_validation_entropy_source_and_output_order() {
    let parent = include_str!("incremental_source.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let external = include_str!("../phase23_rns_link_external_source.rs");
    let packing = include_str!("../packing.rs");
    let core = parent
        .split("fn encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1")
        .nth(1)
        .unwrap()
        .split("pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1")
        .next()
        .unwrap();
    let validation = core
        .find("ValidatedT256PackedPlaintextV1::validate_for_release_limb_stream_v1")
        .unwrap();
    let pool_factory = core.find("prepare_before_entropy()?").unwrap();
    let prepared = core
        .find("PreparedStreamingCollectiveEncryptionV1::new_v1")
        .unwrap();
    let entropy = core.find("authenticated.activate_v1").unwrap();
    let source_callback = core.find("before_output_publication(").unwrap();
    let output = core.find("active.publish_all_v1").unwrap();
    assert!(validation < pool_factory);
    assert!(pool_factory < prepared);
    assert!(prepared < entropy);
    assert!(entropy < source_callback);
    assert!(source_callback < output);
    assert!(core.contains("F: FnOnce("));
    assert!(!core.contains("dyn Fn"));
    assert!(source.contains("try_reserve_exact(PHASE23_MAIN_BLOCKS_PER_RECORD_V1)"));
    assert!(source.contains("main.capacity() != PHASE23_MAIN_BLOCKS_PER_RECORD_V1"));
    assert!(source.contains("Phase23SecretRecordChunkPoolV1::try_new_exact_v1()?"));
    let coordinate_check = source
        .find("match require_expected_packed_coordinate_v1(position, layout, &packed)")
        .unwrap();
    let encryption_call = source
        .find("encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1(")
        .unwrap();
    assert!(coordinate_check < encryption_call);
    let canonical = source
        .find("write_next_canonical_plaintext_block_v1")
        .unwrap();
    let ephemeral = source.find("write_next_ephemeral_block_v1").unwrap();
    let error_zero = source.find("write_next_error_zero_block_v1").unwrap();
    let error_one = source.find("write_next_error_one_block_v1").unwrap();
    let nonce = source.find("write_next_nonce_v1").unwrap();
    assert!(canonical < ephemeral && ephemeral < error_zero);
    assert!(error_zero < error_one && error_one < nonce);
    assert!(source.contains("Some(Ok(packed))"));
    assert!(source.contains("&packed,"));
    assert!(external.contains("let mut live = self\n            .live\n            .take()"));
    assert!(packing.contains("impl Drop for ZkAmsT256PackedPlaintextV1"));
}
#[test]
fn structural_gate_is_fail_closed_and_returns_one_move_only_owner_only_on_success() {
    let parent = include_str!("incremental_source.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let external = include_str!("../phase23_rns_link_external_source.rs");
    let encrypted = include_str!("../phase23_encrypted.rs");
    let leaf = include_str!("../../../../../../iroha_confidential_spool/src/lib.rs");
    assert!(source.contains("let next = self.chunks.next()?;"));
    assert!(source.contains(">= PHASE23_RECORD_COUNT_V1"));
    assert!(source.contains("Err(error) => return Some(Err(error))"));
    assert!(source.contains("source.finish_v1()?"));
    assert!(source.contains("owner.validate_v1()?;\n    Ok(owner)"));
    assert!(parent.contains("authority.failed = true;"));
    assert!(parent.contains("active.kernel.canonical_plaintext"));
    assert!(parent.contains("active.kernel.ephemeral.as_slice()"));
    assert!(parent.contains("active.kernel.error_zero.as_slice()"));
    assert!(parent.contains("active.kernel.error_one.as_slice()"));
    assert!(parent.contains("active.kernel.input_identity.encryption_nonce.as_bytes()"));
    assert!(source.contains("pool.persist_exact_record_v1("));
    assert!(source.contains("struct ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>"));
    assert!(source.contains("source: ZkAmsPhase23RnsLinkExternalSourcePublicationV1"));
    assert!(source.contains("manifests: Vec<ZkAmsMkheStreamingCollectiveCiphertextV1>"));
    assert!(source.contains("public_artifact_manifest_bound: true"));
    assert!(
        !source.contains("impl<K, P> Clone for ZkAmsPhase23MaterializedEncryptedSourceOwnerV1")
    );
    assert!(!source.contains("Serialize"));
    assert!(!source.contains("Decode"));
    assert!(!source.contains("pub use"));
    assert!(source.contains("enum Phase23ContextCorrespondenceSealV1"));
    assert!(source.contains("#[cfg(test)]\n    TestOnly"));
    assert!(external.contains("const PUBLIC_ARTIFACT_MANIFEST_BOUND_V1: bool = false;"));
    assert!(external.contains("const SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1: bool = false;"));
    assert!(external.contains("const RELEASE_COMPLETE_V1: bool = false;"));
    assert!(encrypted.contains("impl Drop for ZkAmsPhase23MaterializedAccumulatorsV1"));
    assert!(leaf.contains("impl Drop for ConfidentialSpoolChunkV1"));
    assert!(!source.contains("mem::forget"));
}
#[test]
fn module_graph_and_context_authority_remain_private_and_fail_closed() {
    let incremental = include_str!("incremental_source.rs");
    let collective = include_str!("../collective.rs");
    let mkhe = include_str!("../../mkhe.rs");
    let source = include_str!("incremental_source_phase23.rs");
    let rns_link = include_str!("../phase23_rns_link.rs");
    let child_declaration =
        "#[path = \"incremental_source_phase23.rs\"]\nmod incremental_source_phase23;";
    assert_eq!(incremental.matches(child_declaration).count(), 1);
    for broad_declaration in [
        "pub mod incremental_source_phase23;",
        "pub(crate) mod incremental_source_phase23;",
        "pub(super) mod incremental_source_phase23;",
    ] {
        assert!(!incremental.contains(broad_declaration));
    }
    assert!(!collective.contains("incremental_source_phase23"));
    assert!(!mkhe.contains("incremental_source_phase23"));
    let global_declaration =
        "#[path = \"mkhe/global_lookup_statement_v1.rs\"]\nmod global_lookup_statement_v1;";
    assert_eq!(mkhe.matches(global_declaration).count(), 1);
    assert!(!mkhe.contains("pub mod global_lookup_statement_v1;"));
    assert!(!mkhe.contains("pub(crate) mod global_lookup_statement_v1;"));
    assert!(!mkhe.contains("pub(super) mod global_lookup_statement_v1;"));
    assert!(!mkhe.contains("pub use global_lookup_statement_v1"));
    for facade in [collective, mkhe] {
        assert!(!facade.contains("Phase23ContextCorrespondenceSealV1"));
        assert!(!facade.contains("materialize_encrypt_and_publish_phase23_source_v1"));
    }
    let context_impl = rns_link
        .split_once("impl ZkAmsPhase23RnsLinkContextV1 {")
        .unwrap()
        .1
        .split_once("\n}\n/// Producer-claimed roots")
        .unwrap()
        .0;
    assert_eq!(context_impl.matches("pub(super) fn new(").count(), 1);
    assert!(context_impl.contains(
        "#[cfg(test)]\n    #[allow(clippy::too_many_arguments)]\n    pub(super) fn new("
    ));
    assert!(rns_link.contains("#[cfg(test)]\ntype RnsLinkContextConstructorV1 = fn("));
    assert!(rns_link.contains(
        "#[cfg(test)]\nconst RNS_LINK_CONTEXT_SIGNATURE_GUARD_V1: RnsLinkContextConstructorV1"
    ));
    let seal_body = source
        .split_once("enum Phase23ContextCorrespondenceSealV1 {")
        .unwrap()
        .1
        .split_once("\n}")
        .unwrap()
        .0;
    assert_eq!(seal_body.trim(), "#[cfg(test)]\n    TestOnly,");
    assert_eq!(
        source.matches("Phase23ContextCorrespondenceSealV1").count(),
        2
    );
    assert!(!source.contains("Phase23ContextCorrespondenceSealV1::"));
    assert!(!source.contains("-> Phase23ContextCorrespondenceSealV1"));
    assert!(!source.contains("Result<Phase23ContextCorrespondenceSealV1"));
}
#[test]
fn source_files_remain_below_the_global_budget_without_exceptions() {
    assert!(
        include_str!("incremental_source_phase23.rs")
            .lines()
            .count()
            <= 900
    );
    assert!(
        include_str!("incremental_source_phase23_tests.rs")
            .lines()
            .count()
            <= 400
    );
}
