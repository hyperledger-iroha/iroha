use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static TEST_DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V2: [u64; 2] = [97, 113];

struct TestDirectoryV2(PathBuf);

impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = TEST_DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-spool-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated qPCS spool test directory");
        Self(path)
    }
}

impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty qPCS spool test directory");
    }
}

fn tiny_geometry_v2() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &TEST_MODULI_V2,
    }
}

fn test_context_v2() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x11; 32],
        source_algebra_binding_digest: [0x22; 32],
    }
}

#[cfg(unix)]
fn test_writer_v2(directory: &TestDirectoryV2) -> QPcsSpoolWriterV2 {
    QPcsSpoolWriterV2::create_with_geometry_v2(
        &directory.0,
        tiny_geometry_v2(),
        test_context_v2(),
        AuthenticatedReplayPermitV2::TestOnly,
    )
    .expect("create tiny concrete qPCS spools")
}

fn coefficient_chunk_v2(geometry: SpoolGeometryV2, values: &[u64]) -> ConfidentialSpoolChunkV1 {
    assert_eq!(
        values.len(),
        usize::from(geometry.coefficient_values_per_block)
    );
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(
        geometry
            .coefficient_block_bytes_v2()
            .expect("coefficient block bytes"),
    )
    .expect("allocate coefficient chunk");
    for (encoded, value) in chunk.as_mut_slice_v1().chunks_exact_mut(8).zip(values) {
        encoded.copy_from_slice(&value.to_be_bytes());
    }
    chunk
}

fn zero_coefficient_chunk_v2(geometry: SpoolGeometryV2) -> ConfidentialSpoolChunkV1 {
    ConfidentialSpoolChunkV1::new_zeroed_v1(
        geometry
            .coefficient_block_bytes_v2()
            .expect("coefficient block bytes"),
    )
    .expect("allocate zero coefficient chunk")
}

fn zero_lde_chunk_v2(geometry: SpoolGeometryV2) -> ConfidentialSpoolChunkV1 {
    ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.lde_block_bytes_v2().expect("LDE block bytes"))
        .expect("allocate zero LDE chunk")
}

#[cfg(unix)]
fn fill_zero_coefficients_v2(writer: &mut QPcsSpoolWriterV2, geometry: SpoolGeometryV2) {
    for _ in 0..geometry
        .coefficient_slot_count_v2()
        .expect("coefficient slots")
    {
        writer
            .push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry))
            .expect("write zero coefficient block");
    }
}

#[cfg(unix)]
fn replay_all_coefficients_v2(
    mut stage: replay_v2::QPcsCoefficientReplayStageV2,
    geometry: SpoolGeometryV2,
) -> replay_v2::QPcsCoefficientReplayStageV2 {
    let pairs = u16::from(geometry.limb_count_v2().unwrap()) * u16::from(OPENING_REPETITIONS_V2);
    for _ in 0..pairs * u16::from(COEFFICIENT_COMPONENTS_V2) {
        let mut reader = stage
            .begin_next_coefficient_row_v2()
            .expect("begin internally derived coefficient purpose");
        for _ in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
            assert!(reader.read_next_block_v2().is_ok());
        }
        stage = reader.complete_v2().expect("complete exact purpose");
    }
    stage
}

#[cfg(unix)]
fn fill_zero_spools_v2(
    mut writer: QPcsSpoolWriterV2,
    geometry: SpoolGeometryV2,
) -> replay_v2::QPcsSpoolSnapshotV2 {
    fill_zero_coefficients_v2(&mut writer, geometry);
    let stage = writer
        .seal_coefficients_for_replay_v2()
        .expect("seal coefficients before LDE completion");
    let mut stage = replay_all_coefficients_v2(stage, geometry);
    for _ in 0..geometry.lde_slot_count_v2().expect("LDE slots") {
        stage
            .push_lde_block_v2(zero_lde_chunk_v2(geometry))
            .expect("write zero LDE block");
    }
    stage.seal_lde_v2().expect("seal complete tiny spools")
}

fn to_hex_v2(bytes: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("write digest hex");
    }
    encoded
}

const RELEASE_PARAMETER_KAT_V2: [u8; 32] = [
    0xcc, 0x56, 0x91, 0x18, 0x77, 0xef, 0x83, 0xb0, 0x4c, 0x3c, 0xe8, 0x79, 0x64, 0x0f, 0x29, 0x43,
    0xce, 0xab, 0xe1, 0x3c, 0x38, 0xa7, 0x37, 0x2d, 0x5c, 0x4f, 0x69, 0x63, 0x7f, 0xe7, 0x75, 0x66,
];
const RELEASE_COEFFICIENT_MAPPING_KAT_V2: [u8; 32] = [
    0xcb, 0x63, 0xff, 0x50, 0x65, 0xe0, 0xdc, 0x06, 0x27, 0xc3, 0x2c, 0xdd, 0x03, 0x40, 0xa6, 0xb4,
    0x7e, 0x66, 0x29, 0xdd, 0x4f, 0xfe, 0xea, 0x44, 0x72, 0xbb, 0xdf, 0xa5, 0xb5, 0xce, 0xc5, 0x23,
];
const RELEASE_LDE_MAPPING_KAT_V2: [u8; 32] = [
    0x59, 0x3c, 0xc4, 0x36, 0x0b, 0x04, 0x95, 0xf2, 0x38, 0xc0, 0x6e, 0x8c, 0x50, 0xd9, 0xfc, 0xd5,
    0x77, 0xd2, 0x3f, 0x1a, 0xe0, 0x0a, 0x0f, 0x9e, 0xa8, 0xdd, 0x03, 0xa9, 0x77, 0xc1, 0x05, 0x66,
];
const RELEASE_COEFFICIENT_CONTEXT_KAT_V2: [u8; 32] = [
    0x71, 0x5f, 0x54, 0x7a, 0xbb, 0xe5, 0xa3, 0x4f, 0x3c, 0x8c, 0x3c, 0xaa, 0x97, 0x00, 0x60, 0x76,
    0x04, 0x76, 0x78, 0x6a, 0x91, 0xd3, 0x84, 0xea, 0x8f, 0xfa, 0xaf, 0x5b, 0x8c, 0x02, 0xa5, 0x56,
];
const RELEASE_LDE_CONTEXT_KAT_V2: [u8; 32] = [
    0x74, 0x33, 0x3e, 0x58, 0x86, 0x86, 0xba, 0x31, 0x27, 0x61, 0x85, 0x55, 0xdc, 0xbf, 0x1b, 0x96,
    0x04, 0x4d, 0xfa, 0x2d, 0x14, 0xd1, 0x7b, 0x1c, 0x10, 0x1d, 0x42, 0xa5, 0xd7, 0xfb, 0xbe, 0x11,
];

fn manual_parameter_oracle_v2() -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.parameters\0".to_vec();
    frame.extend_from_slice(&[2, 17, 19]);
    frame.extend_from_slice(&131_072_u32.to_be_bytes());
    frame.extend_from_slice(&524_288_u32.to_be_bytes());
    frame.extend_from_slice(&160_u16.to_be_bytes());
    frame.extend_from_slice(&[38, 5, 10, 18]);
    frame.extend_from_slice(b"P:2N/c[2N-1]=0;H:N/c[N-1]=0");
    frame.extend_from_slice(b"column=limb*10+repetition*2+role;P:0;H:1");
    frame.extend_from_slice(b"Bp=aP+bXUP;Bh=aX^NH+bX^(N+1)UH");
    for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
        frame.push(limb as u8);
        frame.extend_from_slice(&modulus.to_be_bytes());
    }
    crate::vega::sponge::keccak256(&frame)
}

fn manual_context_oracle_v2(role: u8, parameter: [u8; 32], mapping: [u8; 32]) -> [u8; 32] {
    let context = test_context_v2();
    let mut frame = b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.confidential-spool.context\0".to_vec();
    frame.extend_from_slice(&[2, role]);
    frame.extend_from_slice(&parameter);
    frame.extend_from_slice(&mapping);
    frame.extend_from_slice(&context.sealed_source_transcript_digest);
    frame.extend_from_slice(&context.source_algebra_binding_digest);
    crate::vega::sponge::keccak256(&frame)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ManualMappingMutationV2 {
    Clean,
    Formula,
    Coordinate,
    Order,
    Count,
}

fn manual_mapping_oracle_v2(
    parameter_digest: [u8; 32],
    coefficient: bool,
    mutation: ManualMappingMutationV2,
) -> [u8; 32] {
    let mut hash = crate::vega::sponge::Keccak256::new();
    if coefficient {
        hash.update(
            b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.coefficient-spool.exhaustive-mapping\0",
        );
    } else {
        hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.lde-spool.exhaustive-mapping\0");
    }
    hash.update(&[2]);
    hash.update(&parameter_digest);
    hash.update(&131_072_u32.to_be_bytes());
    hash.update(&[38, 5, 10]);

    if coefficient {
        hash.update(&[3]);
        hash.update(&1_024_u16.to_be_bytes());
        hash.update(&128_u64.to_be_bytes());
        hash.update(&72_960_u64.to_be_bytes());
        hash.update(&8_192_u64.to_be_bytes());
        let formula: &[u8] = if mutation == ManualMappingMutationV2::Formula {
            b"pair=limb*5+repetition;slot=((pair*blocks_per_component+block)*3)+component;component=p-low:9,p-high-top-zero:1,h-top-zero:2"
        } else {
            b"pair=limb*5+repetition;slot=((pair*blocks_per_component+block)*3)+component;component=p-low:0,p-high-top-zero:1,h-top-zero:2"
        };
        hash.update(formula);
        hash.update(b"canonical big-endian u64 residues;descriptor fixes values-per-block");
        hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.coefficient-spool.coordinate-enumeration.count-u64.tuple-slot-u64-limb-u8-repetition-u8-component-u8-block-u64\0");
        let encoded_count = if mutation == ManualMappingMutationV2::Count {
            72_959_u64
        } else {
            72_960_u64
        };
        hash.update(&encoded_count.to_be_bytes());
        for ordinal in 0_u64..72_960 {
            let slot = if mutation == ManualMappingMutationV2::Order {
                match ordinal {
                    0 => 1,
                    1 => 0,
                    _ => ordinal,
                }
            } else {
                ordinal
            };
            let pair_and_block = slot / 3;
            let pair = pair_and_block / 128;
            let limb = (pair / 5) as u8;
            let repetition = (pair % 5) as u8;
            let mut component = (slot % 3) as u8;
            let block = pair_and_block % 128;
            if mutation == ManualMappingMutationV2::Coordinate && ordinal == 0 {
                component = 1;
            }
            hash.update(&slot.to_be_bytes());
            hash.update(&[limb, repetition, component]);
            hash.update(&block.to_be_bytes());
        }
    } else {
        hash.update(&[2]);
        hash.update(&1_024_u16.to_be_bytes());
        hash.update(&512_u64.to_be_bytes());
        hash.update(&194_560_u64.to_be_bytes());
        hash.update(&16_384_u64.to_be_bytes());
        let formula: &[u8] = if mutation == ManualMappingMutationV2::Formula {
            b"column=limb*10+repetition*2+role;slot=column*blocks_per_column+block;role=p:0,h:1"
        } else {
            b"column=limb*10+repetition*2+role;slot=block*columns+column;role=p:0,h:1"
        };
        hash.update(formula);
        hash.update(
            b"canonical (c0,c1) big-endian u64 Fq2 values;descriptor fixes values-per-block",
        );
        hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.lde-spool.coordinate-enumeration.count-u64.tuple-slot-u64-limb-u8-repetition-u8-role-u8-block-u64\0");
        let encoded_count = if mutation == ManualMappingMutationV2::Count {
            194_559_u64
        } else {
            194_560_u64
        };
        hash.update(&encoded_count.to_be_bytes());
        for ordinal in 0_u64..194_560 {
            let slot = if mutation == ManualMappingMutationV2::Order {
                match ordinal {
                    0 => 1,
                    1 => 0,
                    _ => ordinal,
                }
            } else {
                ordinal
            };
            let block = slot / 380;
            let column = slot % 380;
            let limb = (column / 10) as u8;
            let row = column % 10;
            let repetition = (row / 2) as u8;
            let mut role = (row % 2) as u8;
            if mutation == ManualMappingMutationV2::Coordinate && ordinal == 0 {
                role = 1;
            }
            hash.update(&slot.to_be_bytes());
            hash.update(&[limb, repetition, role]);
            hash.update(&block.to_be_bytes());
        }
    }
    hash.finalize()
}

#[test]
fn release_slot_maps_are_bijective_and_have_exact_endpoints() {
    let geometry = SpoolGeometryV2::release_v2();
    geometry.validate_v2().expect("release geometry");

    let coefficient_slots = geometry
        .coefficient_slot_count_v2()
        .expect("coefficient slots");
    assert_eq!(coefficient_slots, RELEASE_COEFFICIENT_SLOTS_V2);
    let mut coefficient_seen = vec![false; coefficient_slots as usize];
    for slot in 0..coefficient_slots {
        let coordinate = coefficient_coordinate_v2(geometry, slot).expect("coefficient coordinate");
        let component = match coordinate.component {
            CoefficientComponentV2::ProductLow => 0,
            CoefficientComponentV2::ProductHighWithTopZero => 1,
            CoefficientComponentV2::QuotientWithTopZero => 2,
        };
        let pair = u64::from(coordinate.limb) * 5 + u64::from(coordinate.repetition);
        let independent_slot = ((pair * 128 + coordinate.block) * 3) + component;
        assert_eq!(independent_slot, slot);
        assert!(!coefficient_seen[independent_slot as usize]);
        coefficient_seen[independent_slot as usize] = true;
    }
    assert!(coefficient_seen.into_iter().all(|seen| seen));
    assert_eq!(
        coefficient_coordinate_v2(geometry, 0).unwrap(),
        CoefficientCoordinateV2 {
            limb: 0,
            repetition: 0,
            block: 0,
            component: CoefficientComponentV2::ProductLow,
        }
    );
    assert_eq!(
        coefficient_coordinate_v2(geometry, coefficient_slots - 1).unwrap(),
        CoefficientCoordinateV2 {
            limb: 37,
            repetition: 4,
            block: 127,
            component: CoefficientComponentV2::QuotientWithTopZero,
        }
    );

    let lde_slots = geometry.lde_slot_count_v2().expect("LDE slots");
    assert_eq!(lde_slots, RELEASE_LDE_SLOTS_V2);
    let mut lde_seen = vec![false; lde_slots as usize];
    for slot in 0..lde_slots {
        let coordinate = lde_coordinate_v2(geometry, slot).expect("LDE coordinate");
        let role = match coordinate.role {
            LdeRowRoleV2::Product => 0,
            LdeRowRoleV2::Quotient => 1,
        };
        let column = u64::from(coordinate.limb) * 10 + u64::from(coordinate.repetition) * 2 + role;
        let independent_slot = coordinate.block * 380 + column;
        assert_eq!(independent_slot, slot);
        assert!(!lde_seen[independent_slot as usize]);
        lde_seen[independent_slot as usize] = true;
    }
    assert!(lde_seen.into_iter().all(|seen| seen));
    assert_eq!(
        lde_coordinate_v2(geometry, 0).unwrap(),
        LdeCoordinateV2 {
            limb: 0,
            repetition: 0,
            role: LdeRowRoleV2::Product,
            block: 0,
        }
    );
    assert_eq!(
        lde_coordinate_v2(geometry, lde_slots - 1).unwrap(),
        LdeCoordinateV2 {
            limb: 37,
            repetition: 4,
            role: LdeRowRoleV2::Quotient,
            block: 511,
        }
    );
    assert_eq!(
        lde_coordinate_v2(geometry, 379).unwrap(),
        LdeCoordinateV2 {
            limb: 37,
            repetition: 4,
            role: LdeRowRoleV2::Quotient,
            block: 0,
        }
    );
    assert_eq!(
        lde_coordinate_v2(geometry, 380).unwrap(),
        LdeCoordinateV2 {
            limb: 0,
            repetition: 0,
            role: LdeRowRoleV2::Product,
            block: 1,
        }
    );
}

#[test]
fn release_parameter_mapping_and_accounting_kats_are_exact() {
    let geometry = SpoolGeometryV2::release_v2();
    let descriptor =
        FixedTenRowParameterDescriptorV2::from_geometry_v2(geometry).expect("descriptor");
    assert_eq!(descriptor.fixed_row_count, 10);
    assert_eq!(descriptor.product_fixed_width, 262_144);
    assert_eq!(descriptor.maximum_product_degree, 262_142);
    assert_eq!(descriptor.quotient_fixed_width, 131_072);
    assert_eq!(descriptor.maximum_quotient_degree, 131_070);
    assert_eq!(descriptor.query_count, 160);
    assert_eq!(descriptor.fri_rounds, 18);
    assert_eq!(descriptor.extension_degree, 2);

    let parameter = parameter_digest_v2(geometry).expect("V2 parameter digest");
    let coefficient = mapping_digest_v2(geometry, parameter, true).expect("coefficient mapping");
    let lde = mapping_digest_v2(geometry, parameter, false).expect("LDE mapping");
    assert_eq!(
        to_hex_v2(parameter),
        "cc56911877ef83b04c3ce879640f2943ceabe13c38a7372d5c4f69637fe77566"
    );
    assert_eq!(
        to_hex_v2(coefficient),
        "cb63ff5065e0dc0627c32cdd0340a6b47e6629dd4ffeea4472bbdfa5b5cec523"
    );
    assert_eq!(
        to_hex_v2(lde),
        "593cc4360b0495f238c06e8c50d9fcd577d23f1ae00a0f9ea8dd03a977c10566"
    );
    assert_eq!(parameter, manual_parameter_oracle_v2());
    assert_eq!(
        parameter,
        super::super::v2_soundness::parameter_digest_for_spool_parity_v2()
    );

    let context = test_context_v2();
    let coefficient_context =
        context_digest_v2(SpoolRoleV2::Coefficients, parameter, coefficient, context)
            .expect("coefficient context digest");
    let lde_context =
        context_digest_v2(SpoolRoleV2::Lde, parameter, lde, context).expect("LDE context digest");
    assert_eq!(coefficient_context, RELEASE_COEFFICIENT_CONTEXT_KAT_V2);
    assert_eq!(lde_context, RELEASE_LDE_CONTEXT_KAT_V2);
    assert_eq!(
        coefficient_context,
        manual_context_oracle_v2(1, parameter, coefficient)
    );
    assert_eq!(lde_context, manual_context_oracle_v2(2, parameter, lde));

    let current_v1 = super::super::zk_ams_phase23_rns_link_q_pcs_release_parameter_digest_v1()
        .expect("current V1 parameter digest");
    assert_ne!(parameter, current_v1);
    let v1_geometry = super::super::QPcsGeometryV1 {
        ring_degree: 8,
        domain_log: 5,
        query_count: 4,
    };
    assert_eq!(
        super::super::validate_polynomial_coefficients_v1(
            &[0; 16],
            97,
            super::super::RelationPolynomialRoleV1::Product,
            v1_geometry,
        ),
        Err(super::super::QPcsErrorV1::InvalidCoefficientCount)
    );
    assert_eq!(RELEASE_COEFFICIENT_SLOTS_V2, 72_960);
    assert_eq!(RELEASE_COEFFICIENT_BLOCK_BYTES_V2, 8_192);
    assert_eq!(RELEASE_COEFFICIENT_FILE_BYTES_V2, 598_855_680);
    assert_eq!(RELEASE_LDE_COLUMNS_V2, 380);
    assert_eq!(RELEASE_LDE_SLOTS_V2, 194_560);
    assert_eq!(RELEASE_LDE_BLOCK_BYTES_V2, 16_384);
    assert_eq!(RELEASE_LDE_FILE_BYTES_V2, 3_190_784_000);
    assert_eq!(RELEASE_TOTAL_FILE_BYTES_V2, 3_789_639_680);
}

#[test]
fn protocol_parameter_identity_excludes_storage_block_geometry() {
    let geometry = tiny_geometry_v2();
    let alternate = SpoolGeometryV2 {
        coefficient_values_per_block: 1,
        lde_values_per_block: 1,
        ..geometry
    };
    let parameter = parameter_digest_v2(geometry).expect("tiny protocol parameter");
    let alternate_parameter =
        parameter_digest_v2(alternate).expect("alternate storage protocol parameter");
    assert_eq!(parameter, alternate_parameter);
    assert_ne!(
        mapping_digest_v2(geometry, parameter, true).expect("coefficient mapping"),
        mapping_digest_v2(alternate, parameter, true).expect("alternate coefficient mapping")
    );
    assert_ne!(
        mapping_digest_v2(geometry, parameter, false).expect("LDE mapping"),
        mapping_digest_v2(alternate, parameter, false).expect("alternate LDE mapping")
    );
}

#[test]
fn exhaustive_mapping_frames_match_independent_oracles_and_reject_mutations() {
    let geometry = SpoolGeometryV2::release_v2();
    let parameter = parameter_digest_v2(geometry).expect("V2 parameter digest");
    assert_eq!(parameter, RELEASE_PARAMETER_KAT_V2);

    for (coefficient, expected) in [
        (true, RELEASE_COEFFICIENT_MAPPING_KAT_V2),
        (false, RELEASE_LDE_MAPPING_KAT_V2),
    ] {
        assert_eq!(
            mapping_digest_v2(geometry, parameter, coefficient).expect("production mapping"),
            expected
        );
        assert_eq!(
            manual_mapping_oracle_v2(parameter, coefficient, ManualMappingMutationV2::Clean),
            expected
        );
        for mutation in [
            ManualMappingMutationV2::Formula,
            ManualMappingMutationV2::Coordinate,
            ManualMappingMutationV2::Order,
            ManualMappingMutationV2::Count,
        ] {
            assert_ne!(
                manual_mapping_oracle_v2(parameter, coefficient, mutation),
                expected,
                "hostile mapping mutation reached the pinned KAT"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn canonical_top_zeros_round_trip_and_nonzero_or_noncanonical_values_poison() {
    let geometry = tiny_geometry_v2();
    let directory = TestDirectoryV2::new_v2();
    let snapshot = fill_zero_spools_v2(test_writer_v2(&directory), geometry);
    assert_ne!(snapshot.parameter_digest_v2(), [0; 32]);
    assert_ne!(snapshot.snapshot_binding_digest_v2(), [0; 32]);
    let mut c0 = snapshot
        .begin_c0_replay_v2()
        .expect("begin exact C0 replay");
    for _ in 0..geometry.lde_slot_count_v2().unwrap() {
        assert!(
            c0.read_next_block_column_v2()
                .unwrap()
                .bytes_v2()
                .iter()
                .all(|byte| *byte == 0)
        );
    }
    let _completed = c0.complete_v2().expect("complete exact C0 replay");

    let mut nonzero_pad = test_writer_v2(&directory);
    for _ in 0..4 {
        nonzero_pad
            .push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry))
            .unwrap();
    }
    assert_eq!(
        coefficient_coordinate_v2(geometry, 4).unwrap().component,
        CoefficientComponentV2::ProductHighWithTopZero
    );
    let bad_pad = coefficient_chunk_v2(geometry, &[0, 1]);
    assert_eq!(
        nonzero_pad.push_coefficient_block_v2(bad_pad),
        Err(QPcsSpoolErrorV2::NonZeroTopPadding)
    );
    assert_eq!(
        nonzero_pad.push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry)),
        Err(QPcsSpoolErrorV2::Poisoned)
    );

    let mut noncanonical = test_writer_v2(&directory);
    assert_eq!(
        noncanonical.push_coefficient_block_v2(coefficient_chunk_v2(geometry, &[97, 0])),
        Err(QPcsSpoolErrorV2::NonCanonicalResidue)
    );
    assert!(matches!(
        noncanonical.seal_coefficients_for_replay_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));

    let mut noncanonical_lde = test_writer_v2(&directory);
    fill_zero_coefficients_v2(&mut noncanonical_lde, geometry);
    let noncanonical_lde = noncanonical_lde.seal_coefficients_for_replay_v2().unwrap();
    let mut noncanonical_lde = replay_all_coefficients_v2(noncanonical_lde, geometry);
    let mut bad_lde = zero_lde_chunk_v2(geometry);
    bad_lde.as_mut_slice_v1()[..8].copy_from_slice(&97_u64.to_be_bytes());
    assert_eq!(
        noncanonical_lde.push_lde_block_v2(bad_lde),
        Err(QPcsSpoolErrorV2::NonCanonicalResidue)
    );
    assert_eq!(
        noncanonical_lde.push_lde_block_v2(zero_lde_chunk_v2(geometry)),
        Err(QPcsSpoolErrorV2::Poisoned)
    );
}

#[cfg(unix)]
#[test]
fn missing_extra_wrong_length_constructor_io_and_unwind_fail_closed() {
    let geometry = tiny_geometry_v2();
    let directory = TestDirectoryV2::new_v2();

    let missing = test_writer_v2(&directory);
    assert!(matches!(
        missing.seal_coefficients_for_replay_v2(),
        Err(QPcsSpoolErrorV2::MissingCoefficientBlocks)
    ));

    let reordered = test_writer_v2(&directory);
    assert!(matches!(
        reordered.seal_coefficients_for_replay_v2(),
        Err(QPcsSpoolErrorV2::MissingCoefficientBlocks)
    ));

    let mut missing_lde = test_writer_v2(&directory);
    fill_zero_coefficients_v2(&mut missing_lde, geometry);
    let missing_lde = missing_lde.seal_coefficients_for_replay_v2().unwrap();
    let missing_lde = replay_all_coefficients_v2(missing_lde, geometry);
    assert!(matches!(
        missing_lde.seal_lde_v2(),
        Err(QPcsSpoolErrorV2::MissingLdeBlocks)
    ));

    let mut extra = test_writer_v2(&directory);
    fill_zero_coefficients_v2(&mut extra, geometry);
    assert_eq!(
        extra.push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry)),
        Err(QPcsSpoolErrorV2::ExtraCoefficientBlock)
    );
    assert!(matches!(
        extra.seal_coefficients_for_replay_v2(),
        Err(QPcsSpoolErrorV2::Poisoned)
    ));

    let mut wrong_length = test_writer_v2(&directory);
    let short =
        ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.coefficient_block_bytes_v2().unwrap() - 1)
            .unwrap();
    assert_eq!(
        wrong_length.push_coefficient_block_v2(short),
        Err(QPcsSpoolErrorV2::InvalidChunkLength)
    );
    assert_eq!(
        wrong_length.push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry)),
        Err(QPcsSpoolErrorV2::Poisoned)
    );

    let absent = directory.0.join("absent");
    assert!(matches!(
        QPcsSpoolWriterV2::create_with_geometry_v2(
            &absent,
            geometry,
            test_context_v2(),
            AuthenticatedReplayPermitV2::TestOnly,
        ),
        Err(QPcsSpoolErrorV2::Leaf(
            ConfidentialSpoolErrorV1::FileOperation { .. }
        ))
    ));

    let mut unwound = test_writer_v2(&directory);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        unwound.panic_after_take_for_test_v2();
    }));
    assert!(panic.is_err());
    assert_eq!(
        unwound.push_coefficient_block_v2(zero_coefficient_chunk_v2(geometry)),
        Err(QPcsSpoolErrorV2::Poisoned)
    );
}

#[cfg(unix)]
#[test]
fn tiny_mask_identity_survives_fixed_width_streaming() {
    let geometry = tiny_geometry_v2();
    let modulus = TEST_MODULI_V2[0];
    let product = [1_u64, 2, 3, 4, 5, 6, 7, 0];
    let quotient = [4_u64, 3, 2, 0];
    let mask = [5_u64, 7, 3, 0];
    let mut expected_product = product;
    let mut expected_quotient = quotient;
    for index in 0..geometry.ring_degree as usize {
        expected_product[index] = (expected_product[index] + mask[index]) % modulus;
        expected_product[index + geometry.ring_degree as usize] =
            (expected_product[index + geometry.ring_degree as usize] + mask[index]) % modulus;
        expected_quotient[index] = (expected_quotient[index] + mask[index]) % modulus;
    }
    assert_eq!(expected_product[7], 0);
    assert_eq!(expected_quotient[3], 0);

    let directory = TestDirectoryV2::new_v2();
    let mut writer = test_writer_v2(&directory);
    for slot in 0..geometry.coefficient_slot_count_v2().unwrap() {
        let coordinate = coefficient_coordinate_v2(geometry, slot).unwrap();
        let start = coordinate.block as usize * usize::from(geometry.coefficient_values_per_block);
        let values: &[u64] = if coordinate.limb == 0 && coordinate.repetition == 0 {
            match coordinate.component {
                CoefficientComponentV2::ProductLow => &expected_product[start..start + 2],
                CoefficientComponentV2::ProductHighWithTopZero => {
                    let offset = geometry.ring_degree as usize + start;
                    &expected_product[offset..offset + 2]
                }
                CoefficientComponentV2::QuotientWithTopZero => &expected_quotient[start..start + 2],
            }
        } else {
            &[0, 0]
        };
        writer
            .push_coefficient_block_v2(coefficient_chunk_v2(geometry, values))
            .unwrap();
    }
    let mut stage = writer.seal_coefficients_for_replay_v2().unwrap();
    let mut streamed_product = [0_u64; 8];
    let mut streamed_quotient = [0_u64; 4];
    for component in [
        CoefficientComponentV2::ProductLow,
        CoefficientComponentV2::ProductHighWithTopZero,
        CoefficientComponentV2::QuotientWithTopZero,
    ] {
        let mut reader = stage.begin_next_coefficient_row_v2().unwrap();
        for block in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
            let chunk = reader.read_next_block_v2().unwrap();
            let values: Vec<u64> = chunk
                .bytes_v2()
                .chunks_exact(8)
                .map(|encoded| u64::from_be_bytes(encoded.try_into().unwrap()))
                .collect();
            let start = block as usize * 2;
            match component {
                CoefficientComponentV2::ProductLow => {
                    streamed_product[start..start + 2].copy_from_slice(&values);
                }
                CoefficientComponentV2::ProductHighWithTopZero => {
                    streamed_product[4 + start..4 + start + 2].copy_from_slice(&values);
                }
                CoefficientComponentV2::QuotientWithTopZero => {
                    streamed_quotient[start..start + 2].copy_from_slice(&values);
                }
            }
        }
        stage = reader.complete_v2().unwrap();
    }
    let pairs = u16::from(geometry.limb_count_v2().unwrap()) * u16::from(OPENING_REPETITIONS_V2);
    for _ in 0..(pairs - 1) * u16::from(COEFFICIENT_COMPONENTS_V2) {
        let mut reader = stage.begin_next_coefficient_row_v2().unwrap();
        for _ in 0..geometry.coefficient_blocks_per_component_v2().unwrap() {
            let _chunk = reader.read_next_block_v2().unwrap();
        }
        stage = reader.complete_v2().unwrap();
    }
    for _ in 0..geometry.lde_slot_count_v2().unwrap() {
        stage
            .push_lde_block_v2(zero_lde_chunk_v2(geometry))
            .unwrap();
    }
    let _snapshot = stage.seal_lde_v2().unwrap();
    assert_eq!(streamed_product, expected_product);
    assert_eq!(streamed_quotient, expected_quotient);
}

#[test]
fn source_guards_keep_v2_private_owned_uninhabited_and_non_authorizing() {
    let source = include_str!("phase23_rns_link_q_pcs_spool.rs");
    let tests = include_str!("phase23_rns_link_q_pcs_spool_tests.rs");
    let replay = include_str!("phase23_rns_link_q_pcs_spool/replay_v2.rs");
    let replay_tests = include_str!("phase23_rns_link_q_pcs_spool/replay_v2_tests.rs");
    let parent = include_str!("phase23_rns_link_q_pcs.rs");
    assert!(source.lines().count() <= 1_200);
    assert!(tests.lines().count() <= 1_000);
    assert!(replay.lines().count() <= 875);
    assert!(replay_tests.lines().count() <= 450);
    assert!(replay.lines().count() + replay_tests.lines().count() < 1_200);
    assert!(source.len() <= 55_000);
    assert!(tests.len() <= 45_000);
    for required in [
        "chunk: ConfidentialSpoolChunkV1",
        "let mut live = self.live.take().ok_or(QPcsSpoolErrorV2::Poisoned)?;",
        "source_aggregation: Infallible",
        "algebra_verification: Infallible",
        "ProductHighWithTopZero",
        "QuotientWithTopZero",
        "slot=block*columns+column",
        "COEFFICIENT_COORDINATE_ENUMERATION_DOMAIN_V2",
        "LDE_COORDINATE_ENUMERATION_DOMAIN_V2",
        "RELEASE_COEFFICIENT_SLOTS_V2: u64 = 72_960",
        "RELEASE_COEFFICIENT_FILE_BYTES_V2: u64 = 598_855_680",
        "RELEASE_LDE_SLOTS_V2: u64 = 194_560",
        "RELEASE_LDE_FILE_BYTES_V2: u64 = 3_190_784_000",
        "SOURCE_AGGREGATION_COMPLETE_V2: bool = false",
        "SOURCE_ALGEBRA_VERIFIED_V2: bool = false",
        "Q_PCS_MASKING_INTEGRATED_V2: bool = false",
        "Q_PCS_COMMITMENT_INTEGRATED_V2: bool = false",
        "Q_PCS_PROOF_INTEGRATED_V2: bool = false",
        "OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false",
        "RELEASE_READY_V2: bool = false",
        "RELEASE_COMPLETE_V2: bool = false",
        "enum AuthenticatedReplayPermitV2",
        "#[path = \"phase23_rns_link_q_pcs_spool/replay_v2.rs\"]\nmod replay_v2;",
    ] {
        assert!(
            source.contains(required),
            "missing production pin: {required}"
        );
    }
    for forbidden in [
        "pub struct",
        "pub enum",
        "pub trait",
        "pub fn",
        "pub(super)",
        "pub(crate)",
        "pub(in ",
        "pub use",
        "impl Clone for ",
        "fn into_inner",
        "dyn Fn",
        "dyn Read",
        "dyn Write",
        "mmap(",
        "memmap",
        "receipt_capability_audit",
        "phase23_rns_link_wire",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden production surface: {forbidden}"
        );
    }
    for required in [
        "REPLAY_FRI_TOTAL_FILE_BYTES_V2: u64 = 6_381_586_240",
        "CQ_COLUMN_FILE_BYTES_V2: u64 = 3_190_784_000",
        "ROW_SCRATCH_FILE_BYTES_V2: u64 = 8_396_800",
        "ROW_SCRATCH_MAX_SNAPSHOTS_V2: u8 = 2",
        "TRANSPOSE_LEAF_BYTES_V2: u64 = 6_080",
        "TRANSPOSE_WINDOW_PLAINTEXT_BYTES_V2: u64 = 6_225_920",
        "hash.update(&[layer]);",
        "pub(super) fn seal_coefficients_for_replay_v2",
        "pub(super) fn begin_next_coefficient_row_v2",
        "pub(super) fn begin_c0_replay_v2",
        "pub(super) fn begin_next_cq_transpose_window_v2",
        "pub(super) fn begin_next_fri_fold_column_v2",
        "pub(super) fn read_next_column_v2(",
        "pub(super) fn read_next_pair_v2(&mut self)",
    ] {
        assert!(
            replay.contains(required),
            "missing exact replay/storage pin: {required}"
        );
    }
    for forbidden in [
        "pub struct",
        "pub enum",
        "pub trait",
        "pub fn",
        "pub(crate)",
        "pub(in ",
        "pub use",
        "impl Clone for",
        "std::path",
        "PathBuf",
        "seek",
        "dyn Fn",
        "dyn Read",
        "dyn Write",
        "Serialize",
        "Deserialize",
        "Norito",
        "codec",
    ] {
        assert!(
            !replay.contains(forbidden),
            "forbidden replay surface: {forbidden}"
        );
    }
    assert!(replay.contains("fn bind_derived_replay_v2("));
    assert!(replay.contains("replay_permit: AuthenticatedReplayPermitV2"));
    assert!(!replay.contains("pub(super) fn bind_derived_replay_v2("));
    assert!(!replay.contains("pub(super) struct StorageLayoutDescriptorV2"));
    assert_eq!(
        replay
            .matches(".take().ok_or(QPcsSpoolErrorV2::Poisoned)?")
            .count(),
        17,
        "every replay transition must take its owner before validation or I/O"
    );
    for (owned_source, owner) in [
        (source, "struct LiveSpoolWritersV2"),
        (source, "struct QPcsSpoolWriterV2"),
        (replay, "struct QPcsCoefficientReplayStageV2"),
        (replay, "struct CoefficientReplayReaderV2"),
        (replay, "struct QPcsSpoolSnapshotV2"),
        (replay, "struct C0ReplayReaderV2"),
        (replay, "struct AuthenticatedReplayChunkV2"),
        (replay, "struct QPcsDerivedReplayV2"),
        (replay, "struct CqTransposeWindowReaderV2"),
        (replay, "struct FriFoldPairReaderV2"),
    ] {
        let offset = owned_source.find(owner).expect("owned spool declaration");
        assert!(
            owned_source[..offset]
                .lines()
                .rev()
                .take(5)
                .all(|line| !line.contains("derive")),
            "move-only spool owner gained a derive: {owner}"
        );
    }
    assert!(parent.contains("#[path = \"phase23_rns_link_q_pcs_spool.rs\"]\nmod spool;"));
    assert!(
        parent.contains(
            "#[cfg(test)]\n#[path = \"phase23_rns_link_q_pcs_masking.rs\"]\nmod masking;"
        )
    );
    assert!(!parent.contains(concat!("pub use ", "spool")));
}
