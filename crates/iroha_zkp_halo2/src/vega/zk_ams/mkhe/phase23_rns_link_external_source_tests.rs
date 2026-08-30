use super::*;
use core::mem::size_of;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
fn release_context_v1() -> ZkAmsPhase23RnsLinkContextV1 {
    ZkAmsPhase23RnsLinkContextV1::new(
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        [0x44; 32],
        [0x55; 32],
        [0x66; 32],
        super::super::zk_ams_phase23_release_map_set_digest_v1().unwrap(),
    )
    .unwrap()
}
static NEXT_TEST_DIRECTORY_V1: AtomicU64 = AtomicU64::new(0);
struct TestDirectoryV1(PathBuf);
impl TestDirectoryV1 {
    fn new_v1(label: &str) -> Self {
        let ordinal = NEXT_TEST_DIRECTORY_V1.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iroha-phase23-rns-link-source-{label}-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn secret_only_geometry_is_exact_and_smaller_than_the_full_mirror() {
    let plan = EXTERNAL_SOURCE_LINK_PLAN_V1;
    assert_eq!(plan.current_state_owner_bytes, 3_686_793_216);
    assert_eq!(plan.prior_full_mirror_bytes, 3_829_526_544);
    assert_eq!(plan.secret_total_file_bytes, 316_239_888);
    assert_eq!(plan.named_persistent_slot_cursor_bytes, 16);
    assert_eq!(plan.max_single_owned_chunk_bytes, 8_192);
    assert!(plan.secret_total_file_bytes < plan.prior_full_mirror_bytes);
    assert_eq!(plan.proposed_specialized_encryption_bytes, 9_445_392);
    assert_eq!(plan.masked_q_pcs_isolated_heap_bytes, 74_662_064);
    assert_eq!(plan.named_combined_heap_bytes, 84_107_456);
    assert!(plan.named_combined_heap_bytes < 160 * 1024 * 1024);
    assert!(plan.confidential_backend_wired);
    assert!(!plan.public_artifact_manifest_bound);
    assert!(!plan.source_relation_polynomials_constructed);
    assert!(!plan.source_algebra_verified);
    assert!(!plan.zero_knowledge_masking_complete);
    assert!(!plan.q_pcs_handoff_complete);
    assert!(!plan.operational_receipt_accepted);
    assert!(!plan.release_complete);
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ManualMutationV1 {
    Clean,
    MainSlot,
    NonceSlot,
    Coordinate,
    Order,
    Encoding,
}
fn manual_encoding_frame_v1(hash: &mut Keccak256, tag: &[u8], width: u64, count: u16) {
    hash.update(&(tag.len() as u16).to_be_bytes());
    hash.update(tag);
    hash.update(&width.to_be_bytes());
    hash.update(&count.to_be_bytes());
}
fn manual_mapping_oracle_v1(geometry_digest: [u8; 32], mutation: ManualMutationV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.rns-link.secret-source-mapping");
    hash.update(&[1]);
    hash.update(&geometry_digest);
    hash.update(&43_u16.to_be_bytes());
    hash.update(&2_u16.to_be_bytes());
    hash.update(&3_268_u32.to_be_bytes());
    hash.update(&131_072_u32.to_be_bytes());
    hash.update(&38_528_u64.to_be_bytes());
    hash.update(&8_192_u64.to_be_bytes());
    hash.update(&43_u64.to_be_bytes());
    hash.update(&32_u64.to_be_bytes());
    manual_encoding_frame_v1(&mut hash, b"fresh-encryption-nonce:raw-bytes", 32, 1);
    let components = [
        (1_u8, 0_u16, 512_u16),
        (2, 512, 128),
        (3, 640, 128),
        (4, 768, 128),
    ];
    let canonical_tag = b"canonical-plaintext:coefficient:big-endian".as_slice();
    let signed_tag = b"encryption-witness-r-e0-e1:i64:twos-complement:big-endian".as_slice();
    for record in 0_u16..43 {
        let (family, chunk, chunks, used): (u8, u16, u16, u32) = match record {
            0 => (1, 0, 1, 89),
            1..=16 => (2, record - 1, 16, 65_536),
            17..=32 => (3, record - 17, 16, 65_536),
            33 => (4, 0, 1, 1_024),
            34..=41 => (5, record - 34, 8, 65_536),
            42 => (6, 0, 1, 512),
            _ => unreachable!(),
        };
        hash.update(&record.to_be_bytes());
        hash.update(&[family]);
        hash.update(&chunk.to_be_bytes());
        hash.update(&chunks.to_be_bytes());
        hash.update(&used.to_be_bytes());
        for &(component, first, blocks) in &components {
            let (tag, width, count): (&[u8], u64, u16) = if component == 1 {
                (canonical_tag, 32, 256)
            } else {
                (signed_tag, 8, 1_024)
            };
            let tag = if mutation == ManualMutationV1::Encoding && record == 0 && component == 1 {
                b"canonical-plaintext:coefficient:little-endian".as_slice()
            } else {
                tag
            };
            hash.update(&[component]);
            hash.update(&first.to_be_bytes());
            hash.update(&blocks.to_be_bytes());
            manual_encoding_frame_v1(&mut hash, tag, width, count);
        }
    }
    let equations = if mutation == ManualMutationV1::Order {
        [(1_u8, 2_u8, 2_u8), (0, 1, 1)]
    } else {
        [(0_u8, 1_u8, 1_u8), (1, 2, 2)]
    };
    for (equation, public_key, ciphertext) in equations {
        hash.update(&[equation, public_key, ciphertext]);
    }
    hash.update(b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-main-mapping");
    hash.update(&38_528_u64.to_be_bytes());
    for record in 0_u16..43 {
        for &(component, first, blocks) in &components {
            for block in 0..blocks {
                let mut slot = u64::from(record) * 896 + u64::from(first + block);
                if mutation == ManualMutationV1::MainSlot && (record, component, block) == (0, 1, 0)
                {
                    slot = 1;
                }
                hash.update(&record.to_be_bytes());
                hash.update(&[component]);
                hash.update(&block.to_be_bytes());
                hash.update(&slot.to_be_bytes());
            }
        }
    }
    hash.update(b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-nonce-mapping");
    hash.update(&43_u64.to_be_bytes());
    for record in 0_u16..43 {
        let slot = u64::from(record)
            + u64::from(u8::from(
                mutation == ManualMutationV1::NonceSlot && record == 0,
            ));
        hash.update(&record.to_be_bytes());
        hash.update(&slot.to_be_bytes());
    }
    hash.update(b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-relation-mapping");
    hash.update(&3_268_u32.to_be_bytes());
    for record in 0_u16..43 {
        for (equation, _, _) in equations {
            for limb in 0_u16..38 {
                let mut coordinate =
                    (u32::from(record) * 2 + u32::from(equation)) * 38 + u32::from(limb);
                if mutation == ManualMutationV1::Coordinate && (record, equation, limb) == (0, 0, 0)
                {
                    coordinate = 1;
                }
                hash.update(&record.to_be_bytes());
                hash.update(&[equation]);
                hash.update(&limb.to_be_bytes());
                hash.update(&coordinate.to_be_bytes());
            }
        }
    }
    hash.finalize()
}
#[test]
fn absolute_mapping_frame_matches_independent_literal_kat_and_rejects_mutations() {
    const KAT: [u8; 32] = [
        0xa5, 0xf9, 0x1a, 0xd6, 0xe7, 0x9f, 0x45, 0xd4, 0xd0, 0xe8, 0xc1, 0xd8, 0xf8, 0xb8, 0x60,
        0x5d, 0x1a, 0xe6, 0x07, 0x6f, 0xb9, 0xb2, 0x01, 0x75, 0x7e, 0x7e, 0x54, 0xee, 0x05, 0xf7,
        0x0e, 0xb4,
    ];
    let geometry_digest = [0xA5; 32];
    assert_eq!(
        source_mapping_digest_from_geometry_digest_v1(geometry_digest).unwrap(),
        KAT
    );
    assert_eq!(
        manual_mapping_oracle_v1(geometry_digest, ManualMutationV1::Clean),
        KAT
    );
    for mutation in [
        ManualMutationV1::MainSlot,
        ManualMutationV1::NonceSlot,
        ManualMutationV1::Coordinate,
        ManualMutationV1::Order,
        ManualMutationV1::Encoding,
    ] {
        assert_ne!(manual_mapping_oracle_v1(geometry_digest, mutation), KAT);
    }
}
#[test]
#[cfg(unix)]
fn concrete_assembly_rejects_reorder_and_missing_data_and_poison_is_terminal() {
    let directory = TestDirectoryV1::new_v1("poison");
    let mut assembly =
        ZkAmsPhase23RnsLinkExternalSourceAssemblyV1::begin_v1(release_context_v1(), &directory.0)
            .unwrap();
    let out_of_order = ZkAmsPhase23RnsLinkSecretChunkV1::new_main_block_zeroed_v1().unwrap();
    assert_eq!(
        assembly.write_next_ephemeral_block_v1(0, 0, out_of_order),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    );
    let correct_but_after_poison =
        ZkAmsPhase23RnsLinkSecretChunkV1::new_main_block_zeroed_v1().unwrap();
    assert_eq!(
        assembly.write_next_canonical_plaintext_block_v1(0, 0, correct_but_after_poison),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    );
    let missing_directory = TestDirectoryV1::new_v1("missing");
    let missing = ZkAmsPhase23RnsLinkExternalSourceAssemblyV1::begin_v1(
        release_context_v1(),
        &missing_directory.0,
    )
    .unwrap();
    assert!(matches!(
        missing.finish_v1(),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    ));
}
#[test]
fn source_surface_is_move_only_bounded_concrete_and_non_authorizing() {
    let source = include_str!("phase23_rns_link_external_source.rs");
    let test_source = include_str!("phase23_rns_link_external_source_tests.rs");
    let production = source
        .split("#[cfg(test)]\n#[path = \"phase23_rns_link_external_source_tests.rs\"]\nmod tests;")
        .next()
        .expect("production source prefix");
    let parent = include_str!("phase23_rns_link.rs");
    let adapter = include_str!("phase23_rns_link_external_spool.rs");
    let spool_leaf = include_str!("../../../../../iroha_crypto/src/confidential_spool.rs");
    let crate_manifest = include_str!("../../../../Cargo.toml");
    assert!(source.lines().count() <= 1_050);
    assert!(source.len() <= 50_000);
    assert!(test_source.lines().count() <= 400);
    assert!(test_source.len() <= 18_000);
    assert!(size_of::<ZkAmsPhase23RnsLinkExternalSourceAssemblyV1>() <= 1_024);
    assert!(size_of::<ZkAmsPhase23RnsLinkExternalSourcePublicationV1>() <= 1_280);
    for move_only in [
        "ZkAmsPhase23RnsLinkSecretChunkV1",
        "ZkAmsPhase23RnsLinkSourceProviderReceiptV1",
        "ZkAmsPhase23RnsLinkSourceSnapshotReceiptV1",
        "ZkAmsPhase23RnsLinkSourcePublicationReceiptV1",
        "ZkAmsPhase23RnsLinkExternalSourceAssemblyV1",
        "ZkAmsPhase23RnsLinkExternalSourcePublicationV1",
    ] {
        let declaration_offset = production
            .find(&format!("struct {move_only}"))
            .unwrap_or_else(|| panic!("missing move-only type: {move_only}"));
        let header_offset = production[..declaration_offset]
            .rfind("\n\n")
            .map_or(0, |offset| offset + 2);
        let header = &production[header_offset..declaration_offset];
        assert!(
            !header.contains("#[derive("),
            "move-only type has a derive: {move_only}"
        );
    }
    for forbidden in [
        "Box<",
        "SecretPolynomial",
        "RnsPolynomial",
        "impl Fn",
        "dyn Fn",
        "pub trait",
        "Norito",
        "serde",
        "Encode",
        "Decode",
        "destination: &mut [u8]",
        "coefficients: &[",
        "plaintext: &[",
        "provider: &mut",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden confidential-source escape: {forbidden}"
        );
    }
    assert!(!production.contains("pub struct"));
    assert!(production.matches(".live\n            .take()").count() >= 2);
    assert_eq!(production.matches(": bool = true;").count(), 1);
    assert!(production.contains("const CONFIDENTIAL_BACKEND_WIRED_V1: bool = true;"));
    for false_axis in [
        "PUBLIC_ARTIFACT_MANIFEST_BOUND_V1",
        "SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1",
        "SOURCE_ALGEBRA_VERIFIED_V1",
        "ZERO_KNOWLEDGE_MASKING_COMPLETE_V1",
        "Q_PCS_HANDOFF_COMPLETE_V1",
        "OPERATIONAL_RECEIPT_ACCEPTED_V1",
        "RELEASE_COMPLETE_V1",
    ] {
        assert!(production.contains(&format!("const {false_axis}: bool = false;")));
    }
    assert!(parent.contains("mod external_source;"));
    assert!(parent.contains("ZkAmsPhase23RnsLinkExternalSourceAssemblyV1"));
    assert!(parent.contains("ZkAmsPhase23RnsLinkSecretChunkV1"));
    assert!(adapter.contains("ConfidentialSpoolWriterV1"));
    assert!(adapter.contains("ConfidentialSpoolSnapshotV1"));
    assert!(adapter.contains("ordered_record_topology_root"));
    assert!(!adapter.contains("#[derive(Clone"));
    assert!(!adapter.contains("#[derive(Debug"));
    assert!(spool_leaf.contains("CONFIDENTIAL_SPOOL_PHASE23_SECRET_MAIN_SLOTS_V1"));
    assert!(spool_leaf.contains("CONFIDENTIAL_SPOOL_PHASE23_SECRET_NONCE_SLOTS_V1"));
    assert!(
        crate_manifest.contains("iroha_crypto = { workspace = true, default-features = false }")
    );
}
