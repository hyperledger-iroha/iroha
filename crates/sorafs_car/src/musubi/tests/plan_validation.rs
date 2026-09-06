//! Adversarial plan-only admission tests for the two canonical Musubi verification phases.

use super::{FixtureFault, bundle_fixture};
use crate::musubi::{
    MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1, MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1,
    MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1, MusubiBundleIntegritySurfaceV1,
    plan::{MusubiPlanValidationContextV1, resolve_chunk_profile_v1, validate_plan_commitment_v1},
};
use crate::{
    CarBuildPlan, DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES, FileEntry,
    compute_chunk_plan_digest_sha3,
};
use iroha_data_model::musubi::{
    MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1, MUSUBI_MAX_FILES_V1, MusubiArchiveCommitmentV1,
    MusubiContentDigestV1,
};

const CONTEXTS: [MusubiPlanValidationContextV1; 2] = [
    MusubiPlanValidationContextV1::SeedIngress,
    MusubiPlanValidationContextV1::ProviderFetch,
];

// Only geometry and the plan-related commitment fields are rebound here. These specimens make no
// claim about payload, CAR, PoR or semantic transcript verification.
fn plan_only_fixture(
    mut sources: Vec<FileEntry>,
    descriptor_bytes: usize,
) -> (CarBuildPlan, MusubiArchiveCommitmentV1) {
    let source_count = sources.len();
    sources.extend([
        FileEntry {
            path: MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1
                .split('/')
                .map(str::to_owned)
                .collect(),
            data: vec![0x31; descriptor_bytes],
        },
        FileEntry {
            path: MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1
                .split('/')
                .map(str::to_owned)
                .collect(),
            data: vec![0x32],
        },
        FileEntry {
            path: MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1
                .split('/')
                .map(str::to_owned)
                .collect(),
            data: vec![0x33],
        },
    ]);
    let (plan, _) = CarBuildPlan::from_files(sources).expect("canonical plan geometry");
    let mut commitment = bundle_fixture(FixtureFault::None).commitment;
    commitment.content_length = plan.content_length;
    commitment.file_count = u32::try_from(source_count).expect("bounded source count");
    commitment.chunk_count = u32::try_from(plan.chunks.len()).expect("bounded chunk count");
    commitment.chunk_plan_digest =
        MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(&plan.chunks));
    commitment.validate().expect("plan-only commitment shape");
    (plan, commitment)
}

#[test]
fn registered_chunker_requires_every_exact_identity_field() {
    let fixture = bundle_fixture(FixtureFault::None);
    assert_eq!(
        resolve_chunk_profile_v1(&fixture.commitment).expect("registered profile"),
        fixture.plan.chunk_profile
    );
    let mutations: [fn(&mut MusubiArchiveCommitmentV1); 5] = [
        |value| value.chunker.profile_id = u32::MAX,
        |value| value.chunker.namespace.push_str("-alias"),
        |value| value.chunker.name.push_str("-alias"),
        |value| value.chunker.semver.push_str("-alias"),
        |value| value.chunker.multihash_code ^= 1,
    ];
    for mutate in mutations {
        let mut commitment = fixture.commitment.clone();
        mutate(&mut commitment);
        assert_eq!(
            resolve_chunk_profile_v1(&commitment)
                .expect_err("exact identity required")
                .surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );
        for context in CONTEXTS {
            assert_eq!(
                validate_plan_commitment_v1(&fixture.plan, &commitment, context)
                    .expect_err("no profile alias")
                    .surface(),
                MusubiBundleIntegritySurfaceV1::ArchiveCommitment
            );
        }
    }
}

#[test]
fn plan_admission_binds_counts_lengths_and_ordered_chunk_digest() {
    let fixture = bundle_fixture(FixtureFault::None);
    let mutations: [fn(&mut MusubiArchiveCommitmentV1); 4] = [
        |value| value.content_length += 1,
        |value| value.chunk_count += 1,
        |value| value.file_count += 1,
        |value| value.chunk_plan_digest = MusubiContentDigestV1::new([0x56; 32]),
    ];
    for context in CONTEXTS {
        validate_plan_commitment_v1(&fixture.plan, &fixture.commitment, context)
            .expect("valid fixture");
        for mutate in mutations {
            let mut commitment = fixture.commitment.clone();
            mutate(&mut commitment);
            assert_eq!(
                validate_plan_commitment_v1(&fixture.plan, &commitment, context)
                    .expect_err("exact plan binding required")
                    .surface(),
                MusubiBundleIntegritySurfaceV1::ArchiveCommitment
            );
        }
        let mut plan = fixture.plan.clone();
        plan.chunks[0].digest[0] ^= 1;
        plan.validate()
            .expect("digest substitution keeps geometry valid");
        assert_eq!(
            validate_plan_commitment_v1(&plan, &fixture.commitment, context)
                .expect_err("chunk digest substitution")
                .surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );
    }
}

#[test]
fn exact_metadata_inventory_rejects_reserved_extra_entries() {
    let fixture = bundle_fixture(FixtureFault::None);
    let mut plan = fixture.plan.clone();
    let source = plan.files.last_mut().expect("last source file");
    assert_eq!(source.path, ["Musubi.toml"]);
    source.path = vec![".musubi".to_owned(), "zz-unknown.norito".to_owned()];
    plan.validate()
        .expect("reserved substitution keeps structural order and coverage");
    for context in CONTEXTS {
        assert_eq!(
            validate_plan_commitment_v1(&plan, &fixture.commitment, context)
                .expect_err("closed metadata inventory")
                .surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );
    }
}

#[test]
fn structural_errors_precede_portable_source_tree_errors() {
    let fixture = bundle_fixture(FixtureFault::None);
    let mut plan = fixture.plan.clone();
    plan.files.last_mut().expect("source file").path =
        vec![".muſubi".to_owned(), "semantic-release.norito".to_owned()];
    plan.validate()
        .expect("Unicode alias is structurally valid before Musubi identity checking");
    for context in CONTEXTS {
        assert_eq!(
            validate_plan_commitment_v1(&plan, &fixture.commitment, context)
                .expect_err("portable collision")
                .surface(),
            MusubiBundleIntegritySurfaceV1::SourceTree
        );
    }
    plan.chunks[0].offset = 1;
    for context in CONTEXTS {
        assert_eq!(
            validate_plan_commitment_v1(&plan, &fixture.commitment, context)
                .expect_err("malformed geometry wins before joined-path checks")
                .surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );
    }
}

#[test]
fn provider_metadata_capture_policy_is_not_an_ingress_wire_limit() {
    let (mut plan, commitment) = plan_only_fixture(
        vec![FileEntry {
            path: vec!["source.ko".to_owned()],
            data: vec![0x34],
        }],
        usize::try_from(MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1).expect("descriptor bound") + 1,
    );
    validate_plan_commitment_v1(
        &plan,
        &commitment,
        MusubiPlanValidationContextV1::SeedIngress,
    )
    .expect("ingress admits bounded plan geometry");
    assert_eq!(
        validate_plan_commitment_v1(
            &plan,
            &commitment,
            MusubiPlanValidationContextV1::ProviderFetch
        )
        .expect_err("provider enforces metadata capture bound")
        .surface(),
        MusubiBundleIntegritySurfaceV1::ArchiveCommitment
    );
    plan.files.last_mut().expect("source file").path =
        vec![".muſubi".to_owned(), "semantic-release.norito".to_owned()];
    plan.validate()
        .expect("portable alias does not break structural geometry");
    assert_eq!(
        validate_plan_commitment_v1(
            &plan,
            &commitment,
            MusubiPlanValidationContextV1::SeedIngress
        )
        .expect_err("ingress detects portable collision")
        .surface(),
        MusubiBundleIntegritySurfaceV1::SourceTree
    );
    assert_eq!(
        validate_plan_commitment_v1(
            &plan,
            &commitment,
            MusubiPlanValidationContextV1::ProviderFetch
        )
        .expect_err("provider capture failure precedes portable collision")
        .surface(),
        MusubiBundleIntegritySurfaceV1::ArchiveCommitment
    );
}

#[test]
fn seed_ingress_keeps_public_maximum_high_path_heap_geometry() {
    let sources = (0..MUSUBI_MAX_FILES_V1)
        .map(|file_index| FileEntry {
            path: (0..64)
                .map(|component_index| {
                    format!("f{file_index:04}c{component_index:02}{}", "x".repeat(55))
                })
                .collect(),
            data: vec![u8::try_from(file_index % 251).expect("fixture byte")],
        })
        .collect();
    let (plan, commitment) = plan_only_fixture(sources, 1);
    let estimated = plan
        .validate()
        .expect("bounded canonical geometry")
        .estimated_ingest_heap_bytes();
    assert!(
        estimated > 24 * 1024 * 1024,
        "exercise the old rejected geometry"
    );
    assert!(estimated <= DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES);
    validate_plan_commitment_v1(
        &plan,
        &commitment,
        MusubiPlanValidationContextV1::SeedIngress,
    )
    .expect("retain ingress heap policy");
}
