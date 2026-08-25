//! Inline regression coverage for the Kagemusha terminal registry.
use super::{test_support::candidate_binding_profile, *};
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    domain::DomainId,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_CARGO_FUZZ_VERSION_OUTPUT_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_CARGO_FUZZ_VERSION_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_CARGO_PROXY_CONTRACT_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_CARGO_PROXY_PROGRAM_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_RUSTC_VERSION_OUTPUT_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_RUSTC_VERSION_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_FUZZ_TARGET_TRIPLE_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_MIN_FUZZ_EXECUTIONS_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_SIGNATURE_DOMAIN_V1,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
        KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
        KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4, KagemushaExactBytesDigestV1,
        KagemushaInternalValidationCommandOutcomeV1, KagemushaInternalValidationFuzzOutcomeV1,
        KagemushaInternalValidationFuzzTargetV1, KagemushaInternalValidationToolRoleV1,
        KagemushaInternalValidationToolV1, KagemushaInternalValidationWorkingDirectoryV1,
        KagemushaRecursiveSpendArtifactManifestV4,
        KagemushaRecursiveSpendCryptographicReviewApprovalV4,
        KagemushaRecursiveSpendCryptographicReviewEvidenceV4,
        KagemushaRecursiveSpendCryptographicReviewPayloadV4,
        KagemushaRecursiveSpendInternalValidationReceiptBodyV1,
        KagemushaRecursiveSpendInternalValidationReceiptV1,
        KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendReleaseApprovalRoleV1,
        KagemushaRecursiveSpendReleaseApprovalV4, KagemushaRecursiveSpendReleaseAttestationV4,
        KagemushaRecursiveSpendReleaseRolePolicyV1, KagemushaReleaseVerificationError,
        KagemushaReviewedSourceClosureV1, KagemushaReviewedTrackedCargoLockV2,
        KagemushaTopUpFinalityRosterArtifactReferenceV4,
        kagemusha_internal_validation_runner_identity_sha256_v1,
    },
};
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
struct KagemushaCatalogTestTempDir {
    // Drop the directory before the later-declared guard releases the lane.
    directory: tempfile::TempDir,
    _guard: std::sync::MutexGuard<'static, ()>,
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl KagemushaCatalogTestTempDir {
    fn path(&self) -> &Path {
        self.directory.path()
    }
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn kagemusha_catalog_test_tempdir() -> std::io::Result<KagemushaCatalogTestTempDir> {
    static TEMP_DIR_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    let guard = TEMP_DIR_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let temporary_root = std::fs::canonicalize(std::env::temp_dir())?;
    // Darwin's per-user `T` directory is mutated continuously by parallel
    // tests. Catalog validation deliberately snapshots every absolute-path
    // ancestor, so use its stable `0` sibling when that standard layout is
    // available. Other platforms and custom TMPDIR layouts retain the
    // canonical system temporary root as a portable fallback.
    #[cfg(target_os = "macos")]
    let temporary_root = temporary_root
        .parent()
        .map(|parent| parent.join("0"))
        .filter(|candidate| candidate.is_dir())
        .map_or(Ok(temporary_root), std::fs::canonicalize)?;
    let base = temporary_root.join(format!(
        "iroha-kagemusha-catalog-test-tmp-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&base)?;
    let directory = tempfile::Builder::new()
        .prefix("catalog-")
        .tempdir_in(base)?;
    Ok(KagemushaCatalogTestTempDir {
        directory,
        _guard: guard,
    })
}
#[cfg(target_os = "macos")]
struct MacosAclGuard {
    path: PathBuf,
}
#[cfg(target_os = "macos")]
impl Drop for MacosAclGuard {
    fn drop(&mut self) {
        let _ = std::process::Command::new("/bin/chmod")
            .arg("-N")
            .arg(&self.path)
            .status();
    }
}
#[cfg(target_os = "macos")]
fn add_macos_acl(path: &Path, entry: &str) -> MacosAclGuard {
    let output = std::process::Command::new("/bin/chmod")
        .arg("+a")
        .arg(entry)
        .arg(path)
        .output()
        .expect("run macOS chmod");
    assert!(
        output.status.success(),
        "chmod +a failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    MacosAclGuard {
        path: path.to_path_buf(),
    }
}
include!("core_tests.rs");
include!("validator_qualification_tests.rs");
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_is_canonical_versioned_norito_and_rejects_tamper() {
    let policy = Path::new("/sealed-fixture/policy.norito");
    let artifacts = Path::new("/sealed-fixture/artifacts");
    let seal = qualification_seal_fixture(policy, artifacts);
    let sealed_eq = &seal.releases[0].step_eq;
    let qualified_eq = sealed_eq.to_qualified().expect("qualified Eq fixture");
    assert_eq!(
        KagemushaCatalogSealedParityQualificationV1::from_qualified(
            &qualified_eq,
            sealed_eq.compiled_protocol_structure_sha256,
        )
        .expect("separately bound structure and identity"),
        sealed_eq.clone()
    );
    assert!(
        KagemushaCatalogSealedParityQualificationV1::from_qualified(&qualified_eq, [0; 32],)
            .is_err()
    );
    assert!(
        KagemushaCatalogSealedParityQualificationV1::from_qualified(
            &qualified_eq,
            qualified_eq.compiled_protocol_identity_sha256(),
        )
        .is_err()
    );
    let bytes = seal.canonical_bytes().expect("canonical seal bytes");
    let decoded: KagemushaCatalogQualificationSealV1 =
        norito::decode_canonical(&bytes).expect("decode canonical seal");
    assert_eq!(decoded, seal);
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        norito::decode_canonical::<KagemushaCatalogQualificationSealV1>(&trailing).is_err(),
        "trailing seal bytes must not decode canonically"
    );
    let mut wrong_schema = seal.clone();
    wrong_schema.schema.push_str(".tampered");
    assert!(wrong_schema.canonical_bytes().is_err());
    let mut wrong_version = seal;
    wrong_version.version = wrong_version.version.saturating_add(1);
    assert!(wrong_version.canonical_bytes().is_err());
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_rejects_stale_build_executable_and_source_facts() {
    let policy = Path::new("/sealed-fixture/policy.norito");
    let artifacts = Path::new("/sealed-fixture/artifacts");
    let seal = qualification_seal_fixture(policy, artifacts);
    seal.validate_for_configured_runtime(policy, artifacts)
        .expect("matching runtime binding");
    let mut stale_build = seal.clone();
    stale_build.build_fingerprint_sha256[0] ^= 1;
    assert!(
        stale_build
            .validate_for_configured_runtime(policy, artifacts)
            .is_err()
    );
    let mut stale_executable = seal.clone();
    stale_executable.canonical_executable_path = "/sealed-fixture/other-irohad".to_owned();
    assert!(
        stale_executable
            .validate_for_configured_runtime(policy, artifacts)
            .is_err()
    );
    let (authenticated, promotion, _) = authenticated_candidate_binding_release();
    let candidate = authenticated
        .manifest()
        .immutable_candidate()
        .expect("fixture candidate");
    promotion
        .validate_against_candidate_and_authenticated_release(&candidate, &authenticated)
        .expect("matching promotion provenance");
    let mut wrong_projection_promotion = promotion.clone();
    wrong_projection_promotion.authenticated_source_seal_projection_sha256[0] ^= 1;
    assert!(
        wrong_projection_promotion
            .validate_against_candidate_and_authenticated_release(&candidate, &authenticated)
            .is_err()
    );
    let mut wrong_cargo_promotion = promotion.clone();
    wrong_cargo_promotion.reviewed_cargo_binary_sha256[0] ^= 1;
    assert!(
        wrong_cargo_promotion
            .validate_against_candidate_and_authenticated_release(&candidate, &authenticated)
            .is_err()
    );
    let mut wrong_rustc_promotion = promotion.clone();
    wrong_rustc_promotion.reviewed_rustc_binary_sha256[0] ^= 1;
    assert!(
        wrong_rustc_promotion
            .validate_against_candidate_and_authenticated_release(&candidate, &authenticated)
            .is_err()
    );
    let promotion_bytes =
        norito::encode_canonical(&promotion).expect("canonical promotion fixture");
    for parity in [
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleParityV1::StepEp,
    ] {
        let sealed_parity = match parity {
            KagemushaPastaCycleParityV1::StepEq => &seal.releases[0].step_eq,
            KagemushaPastaCycleParityV1::StepEp => &seal.releases[0].step_ep,
        };
        assert_ne!(
            sealed_parity.compiled_protocol_identity_sha256,
            profile(authenticated.manifest(), parity)
                .expect("fixture parity profile")
                .compiled_protocol_structure_sha256,
            "the sealed full protocol identity must not be mistaken for its value-free structure digest"
        );
        assert_eq!(
            sealed_parity.compiled_protocol_structure_sha256,
            profile(authenticated.manifest(), parity)
                .expect("fixture parity profile")
                .compiled_protocol_structure_sha256,
            "the value-free structure digest must be sealed separately"
        );
    }
    validate_sealed_release_qualification_v1(
        &seal.releases[0],
        &authenticated,
        &promotion_bytes,
        authenticated.manifest().qualification_receipt_sha256,
    )
    .expect("matching sealed source facts");
    let mut stale_source = seal.releases[0].clone();
    stale_source.source_tree_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &stale_source,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut stale_projection = seal.releases[0].clone();
    stale_projection.authenticated_source_seal_projection_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &stale_projection,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut stale_cargo = seal.releases[0].clone();
    stale_cargo.reviewed_cargo_binary_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &stale_cargo,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut stale_rustc = seal.releases[0].clone();
    stale_rustc.reviewed_rustc_binary_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &stale_rustc,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut tampered_receipt = seal.releases[0].clone();
    tampered_receipt.qualification_receipt_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &tampered_receipt,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut tampered_internal_validation_receipt = seal.releases[0].clone();
    tampered_internal_validation_receipt.internal_validation_receipt_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &tampered_internal_validation_receipt,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut substituted_qualified_candidate = seal.releases[0].clone();
    substituted_qualified_candidate.qualified_candidate_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &substituted_qualified_candidate,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut tampered_artifact = seal.releases[0].clone();
    tampered_artifact.artifacts[0].artifact.payload_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &tampered_artifact,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
    let mut tampered_structure = seal.releases[0].clone();
    tampered_structure
        .step_eq
        .compiled_protocol_structure_sha256[0] ^= 1;
    assert!(
        validate_sealed_release_qualification_v1(
            &tampered_structure,
            &authenticated,
            &promotion_bytes,
            authenticated.manifest().qualification_receipt_sha256,
        )
        .is_err()
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_missing_or_malformed_file_fails_closed() {
    let temporary = kagemusha_catalog_test_tempdir().expect("ACL-free temporary seal root");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let policy = temporary.path().join("policy.norito");
    let artifacts = temporary.path().join("artifacts");
    let missing = temporary.path().join("missing-seal.norito");
    let missing_error = KagemushaReleaseCatalogV4::load_with_qualification_seal_for_trusted_uid(
        &policy,
        &artifacts,
        DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
        &missing,
        trusted_uid,
    )
    .err()
    .expect("missing qualification seal must fail closed");
    assert!(
        missing_error.contains("qualification seal") || missing_error.contains("failed to inspect")
    );
    let malformed = temporary.path().join("malformed-seal.norito");
    std::fs::write(&malformed, b"not canonical Norito")
        .expect("write malformed qualification seal");
    let malformed =
        std::fs::canonicalize(malformed).expect("canonical malformed qualification seal");
    let malformed_error = KagemushaReleaseCatalogV4::load_with_qualification_seal_for_trusted_uid(
        &policy,
        &artifacts,
        DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
        &malformed,
        trusted_uid,
    )
    .err()
    .expect("malformed qualification seal must fail closed");
    assert!(malformed_error.contains("decode") || malformed_error.contains("seal"));
}
#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_executable_stat_tamper_fails_without_rehashing() {
    let executable =
        current_kagemusha_catalog_executable_path_v1().expect("current test executable");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let mut captured = BTreeMap::new();
    let digest = capture_trusted_catalog_file_v1(
        &executable,
        "current test executable",
        trusted_uid,
        &mut captured,
        true,
    )
    .expect("capture test executable")
    .expect("test executable digest");
    assert_ne!(digest, [0; 32]);
    let paths = captured.into_values().collect::<Vec<_>>();
    verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid)
        .expect("matching executable stat identity");
    let mut stale = paths;
    let executable_entry = stale
        .iter_mut()
        .find(|entry| entry.canonical_path == executable.to_string_lossy())
        .expect("sealed executable path");
    executable_entry.stat.changed_seconds = executable_entry.stat.changed_seconds.saturating_add(1);
    assert!(
        verify_kagemusha_catalog_sealed_paths_v1(&stale, trusted_uid).is_err(),
        "fast startup must reject stale executable stat without hashing the binary"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_trust_rejects_third_party_owner_and_writable_mode() {
    let trusted_uid = 501;
    let base = KagemushaCatalogSealedStatV1 {
        device: 1,
        inode: 2,
        mode: 0o100440,
        owner_uid: trusted_uid,
        owner_gid: 20,
        links: 1,
        length: 3,
        modified_seconds: 4,
        modified_nanoseconds: 5,
        changed_seconds: 6,
        changed_nanoseconds: 7,
    };
    ensure_root_trusted_stat_v1(base, trusted_uid, "fixture")
        .expect("test fixture owner is trusted");
    let mut third_party = base;
    third_party.owner_uid = trusted_uid + 1;
    assert!(ensure_root_trusted_stat_v1(third_party, trusted_uid, "fixture").is_err());
    let mut writable = base;
    writable.mode |= 0o020;
    assert!(ensure_root_trusted_stat_v1(writable, trusted_uid, "fixture").is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn qualification_seal_trust_rejects_extended_acl_write_grants() {
    use std::os::unix::fs::MetadataExt as _;
    let temporary = kagemusha_catalog_test_tempdir().expect("ACL fixture root");
    let source_path = temporary.path().join("source.bin");
    std::fs::write(&source_path, b"trusted source").expect("write trusted source fixture");
    let canonical_source =
        std::fs::canonicalize(&source_path).expect("canonical trusted source fixture");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let source_error = {
        let _acl = add_macos_acl(&canonical_source, "everyone allow write");
        let metadata =
            std::fs::symlink_metadata(&canonical_source).expect("inspect ACL source fixture");
        assert_eq!(
            metadata.mode() & 0o022,
            0,
            "ACL grant must not rely on writable POSIX mode bits"
        );
        capture_trusted_catalog_file_v1(
            &canonical_source,
            "ACL source fixture",
            trusted_uid,
            &mut BTreeMap::new(),
            false,
        )
        .expect_err("an ACL-writable trusted source must fail closed")
    };
    assert!(source_error.contains("extended ACL"));
    let seal_path = temporary.path().join("seal.norito");
    std::fs::write(&seal_path, b"not decoded").expect("write seal ACL fixture");
    let canonical_seal = std::fs::canonicalize(&seal_path).expect("canonical seal ACL fixture");
    let seal_error = {
        let _acl = add_macos_acl(&canonical_seal, "everyone allow write");
        read_root_trusted_kagemusha_catalog_qualification_seal_v1(&canonical_seal, trusted_uid)
            .expect_err("an ACL-writable root-trusted seal must fail before decoding")
    };
    assert!(seal_error.contains("extended ACL"));
}
#[test]
fn decoded_catalog_estimate_accounts_for_params_and_vk_expansion() {
    let (authenticated, _, _) = authenticated_candidate_binding_release();
    let estimate = estimate_catalog_release_memory_v4(authenticated.manifest())
        .expect("candidate-binding memory estimate");
    let rows = 1_u64 << KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
    let params = rows * PARSED_PARAMS_BYTES_PER_ROW_V4 * 2;
    let verifier_domains = rows * PARSED_VERIFYING_KEY_DOMAIN_BYTES_PER_ROW_V4 * 2;
    let retained_and_parsed_vk = 64 * (1 + PARSED_VERIFYING_KEY_EXPANSION_V4) * 2;
    let raw_persistent = CATALOG_RELEASE_METADATA_PERSISTENT_BYTES_V4
        + params
        + verifier_domains
        + retained_and_parsed_vk;
    let expected_persistent =
        checked_decoded_estimate_headroom_v4(raw_persistent).expect("expected persistent estimate");
    let largest_bounded_role = authenticated
        .manifest()
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .filter(|artifact| artifact.kind != KagemushaPastaCycleArtifactKindV4::ProvingKey)
        .map(|artifact| artifact.payload_size_bytes)
        .max()
        .expect("bounded role payload");
    let expected_peak = checked_decoded_estimate_headroom_v4(
        raw_persistent
            + CATALOG_RELEASE_METADATA_TRANSIENT_BYTES_V4
            + largest_bounded_role
            + u64::try_from(KAGEMUSHA_PK_STREAM_AUTHENTICATION_BUFFER_BYTES_V5)
                .expect("PK stream scratch fits u64"),
    )
    .expect("expected peak estimate");
    assert_eq!(estimate.persistent_bytes, expected_persistent);
    assert_eq!(estimate.peak_load_bytes, expected_peak);
    assert!(estimate.peak_load_bytes <= DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4);
}
#[test]
fn decoded_catalog_default_matches_configured_safety_ceiling() {
    assert_eq!(
        DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
        iroha_config::parameters::defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES
    );
}
#[test]
fn decoded_catalog_headroom_rounds_up() {
    assert_eq!(
        checked_decoded_estimate_headroom_v4(1).expect("one-byte estimate"),
        2
    );
    assert_eq!(
        checked_decoded_estimate_headroom_v4(4).expect("aligned estimate"),
        5
    );
}
#[test]
fn catalog_preflight_rejects_inexact_proving_key_before_halo_parsing() {
    let (authenticated, _, _) = authenticated_candidate_binding_release();
    let mut manifest = authenticated.manifest().clone();
    for profile in &mut manifest.profiles {
        let sizes = kagemusha_artifact_encoding_sizes_v4(&profile.circuit_params, profile.parity)
            .expect("fixture artifact encoding sizes");
        for descriptor in &mut profile.artifacts {
            match descriptor.kind {
                KagemushaPastaCycleArtifactKindV4::ParamsIpa => {
                    descriptor.payload_size_bytes = sizes.parameters_bytes;
                }
                KagemushaPastaCycleArtifactKindV4::VerifyingKey => {
                    descriptor.payload_size_bytes = sizes.verifying_key_bytes;
                }
                KagemushaPastaCycleArtifactKindV4::ProvingKey
                | KagemushaPastaCycleArtifactKindV4::BootstrapWitness => {}
            }
        }
    }
    let error = validate_catalog_artifact_encoding_sizes_v4(&manifest)
        .expect_err("an inexact proving-key descriptor must fail before parsing");
    assert!(error.contains("proving key descriptor length 64"));
    assert!(error.contains("exact authenticated shape length"));
}
#[test]
fn decoded_catalog_estimate_rejects_shift_overflow() {
    let (authenticated, _, _) = authenticated_candidate_binding_release();
    let mut manifest = authenticated.manifest().clone();
    manifest.profiles[0].ipa_k = u64::BITS;
    assert!(estimate_catalog_release_memory_v4(&manifest).is_err());
}
#[test]
fn decoded_catalog_loader_rejects_zero_budget_before_filesystem_access() {
    let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
        Path::new("missing-policy"),
        Path::new("missing-artifacts"),
        0,
    )
    .err()
    .expect("zero decoded budget must fail first");
    assert!(error.contains("must be greater than zero"));
}
#[test]
fn decoded_catalog_loader_rejects_budget_above_safety_ceiling() {
    let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
        Path::new("missing-policy"),
        Path::new("missing-artifacts"),
        DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4 + 1,
    )
    .err()
    .expect("an over-ceiling decoded budget must fail before filesystem access");
    assert!(error.contains("non-raiseable"));
    assert!(error.contains(&DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4.to_string()));
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn decoded_catalog_loader_enforces_budget_before_artifact_reads() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    let (authenticated, _, _) = authenticated_candidate_binding_release();
    let mut manifest = authenticated.manifest().clone();
    for profile in &mut manifest.profiles {
        let sizes = kagemusha_artifact_encoding_sizes_v4(&profile.circuit_params, profile.parity)
            .expect("compact artifact encoding sizes");
        for descriptor in &mut profile.artifacts {
            let payload_size = match descriptor.kind {
                KagemushaPastaCycleArtifactKindV4::ParamsIpa => sizes.parameters_bytes,
                KagemushaPastaCycleArtifactKindV4::ProvingKey => sizes.proving_key_bytes,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey => sizes.verifying_key_bytes,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness => {
                    descriptor.payload_size_bytes
                }
            };
            descriptor.payload_size_bytes = payload_size;
            descriptor.size_bytes = payload_size
                .checked_add(4_096)
                .expect("compact framed artifact size");
        }
    }
    let mut candidate_manifest = manifest.clone();
    candidate_manifest.qualification_receipt_sha256 = [0; 32];
    candidate_manifest.qualified_candidate_sha256 = [0; 32];
    candidate_manifest.internal_validation_receipt_sha256 = [0; 32];
    candidate_manifest.benchmark_evidence_sha256 = [0; 32];
    candidate_manifest.cryptographic_review_sha256 = [0; 32];
    candidate_manifest.release_attestation_sha256 = [0; 32];
    let candidate = iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4 {
        schema: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
        manifest: candidate_manifest,
    };
    manifest.qualified_candidate_sha256 =
        iroha_data_model::offline::kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate.sha256().expect("compact candidate digest"),
            manifest.qualification_receipt_sha256,
        );
    let manifest_bytes = norito::to_bytes(&manifest).expect("canonical compact manifest");
    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
    let release = artifacts.join(hex::encode(manifest_sha256));
    std::fs::create_dir_all(&release).expect("create compact release directory");
    std::fs::write(release.join(MANIFEST_FILE_NAME_V4), manifest_bytes)
        .expect("write compact manifest");
    let estimate =
        estimate_catalog_release_memory_v4(&manifest).expect("compact catalog memory estimate");
    assert_eq!(estimate.peak_load_bytes, 279_192_800);
    assert!(estimate.peak_load_bytes <= DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4);
    let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
        &policy,
        &artifacts,
        estimate.peak_load_bytes - 1,
    )
    .err()
    .expect("a one-byte-short decoded budget must fail before inventory reads");
    assert!(
        error.contains("decoded catalog memory estimate"),
        "unexpected error: {error}"
    );
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("the intentionally incomplete inventory must still fail closed");
    assert!(
        !error.contains("decoded catalog memory estimate") && error.contains("inventory"),
        "default loader rejected before the bounded inventory check: {error}"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn canonical_temporary_root(temporary: &KagemushaCatalogTestTempDir) -> PathBuf {
    std::fs::canonicalize(temporary.path()).expect("canonical temporary catalog root")
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn pinned_source_fixture() -> (
    KagemushaCatalogTestTempDir,
    PathBuf,
    KagemushaCatalogPinnedArtifactSourceV4,
) {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary pinned-source root");
    let root = canonical_temporary_root(&temporary);
    let release_directory = root.join("release");
    std::fs::create_dir(&release_directory).expect("create pinned-source release directory");
    let (authenticated, _, _) = authenticated_candidate_binding_release();
    for (index, descriptor) in authenticated
        .manifest()
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .enumerate()
    {
        let length =
            usize::try_from(descriptor.size_bytes).expect("test artifact length must fit usize");
        let tag = u8::try_from(index + 1).expect("test artifact tag");
        std::fs::write(
            release_directory.join(&descriptor.file_name),
            vec![tag; length],
        )
        .expect("write pinned-source artifact");
    }
    let pinned_directory = CatalogDirectory::open_path(&release_directory, "pinned-source release")
        .expect("pin source release directory");
    let source =
        KagemushaCatalogPinnedArtifactSourceV4::open_pinned(&pinned_directory, authenticated)
            .expect("open exact-eight pinned source fixture");
    (temporary, release_directory, source)
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn sealed_fast_source_open_never_reads_any_artifact_payload() {
    KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(true));
    let fixture = std::panic::catch_unwind(pinned_source_fixture);
    KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(false));
    let (_temporary, release_directory, source) =
        fixture.expect("sealed open must not touch any artifact payload");
    source
        .validate_snapshot()
        .expect("metadata-only pinned source remains valid");
    assert_eq!(source.artifacts.len(), KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4);
    for artifact in &source.artifacts {
        assert!(
            artifact.authenticated_inspection.is_none(),
            "the sealed fast path must not retain a content inspection"
        );
        let bytes = std::fs::read(release_directory.join(&artifact.descriptor.file_name))
            .expect("read deliberately invalid proving-key fixture");
        assert_ne!(
            <[u8; 32]>::from(Sha256::digest(bytes)),
            artifact.descriptor.sha256,
            "fixture must prove open_pinned accepted an unread digest-mismatched artifact"
        );
    }
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn sealed_catalog_source_construction_never_reads_any_artifact_payload() {
    let (temporary, release_directory, source) = pinned_source_fixture();
    let authenticated = source.release.clone();
    drop(source);
    let directory = CatalogDirectory::open_path(
        &release_directory,
        "sealed catalog source construction fixture",
    )
    .expect("pin sealed catalog release directory");
    let mut seal = qualification_seal_fixture(
        Path::new("/sealed-fixture/policy.norito"),
        Path::new("/sealed-fixture/artifacts"),
    );
    let sealed_release = seal.releases.remove(0);
    KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(true));
    let construction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        open_qualified_kagemusha_catalog_source_v4(&directory, authenticated, Some(&sealed_release))
    }));
    KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(false));
    let (pinned, qualified) = construction
        .expect("sealed source construction must not panic")
        .expect("sealed source construction must not touch any artifact payload");
    assert!(
        qualified.authenticated_release() == pinned.authenticated_release(),
        "sealed qualified and pinned sources must retain the same release"
    );
    pinned
        .validate_snapshot()
        .expect("sealed source construction retains exact read-only handles");
    assert!(
        pinned
            .artifacts
            .iter()
            .all(|artifact| artifact.authenticated_inspection.is_none()),
        "sealed source construction must not retain a content inspection"
    );
    drop(temporary);
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn qualification_seal_capture_rejects_replaced_release_after_full_qualification() {
    let (temporary, qualified_release_directory, pinned_source) = pinned_source_fixture();
    let replacement_release_directory =
        canonical_temporary_root(&temporary).join("replacement-release");
    std::fs::create_dir(&replacement_release_directory)
        .expect("create replacement release directory");
    for artifact in &pinned_source.artifacts {
        std::fs::copy(
            qualified_release_directory.join(&artifact.descriptor.file_name),
            replacement_release_directory.join(&artifact.descriptor.file_name),
        )
        .expect("copy replacement artifact");
    }
    let replacement = CatalogDirectory::open_path(
        &replacement_release_directory,
        "replacement release after full qualification",
    )
    .expect("open replacement release directory");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let mut paths = BTreeMap::new();
    let error = capture_trusted_catalog_release_inventory_v1(
        &replacement,
        "replacement-release",
        pinned_source.manifest_sha256,
        &pinned_source,
        pinned_source
            .authenticated_release()
            .manifest()
            .qualification_receipt_sha256,
        trusted_uid,
        &mut paths,
    )
    .expect_err("seal capture must reject inodes not used by full qualification");
    assert!(
        error.contains("different from the fully qualified pinned source"),
        "unexpected replacement error: {error}"
    );
    pinned_source
        .validate_snapshot()
        .expect("the originally qualified pinned source remains unchanged");
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn sealed_path_revalidation_rejects_path_stat_owner_mode_and_time_tamper() {
    let temporary = kagemusha_catalog_test_tempdir().expect("ACL-free temporary sealed-path root");
    let file_path = temporary.path().join("sealed.bin");
    std::fs::write(&file_path, b"sealed identity").expect("write sealed-path fixture");
    let canonical_file = std::fs::canonicalize(&file_path).expect("canonical fixture file");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let mut captured = BTreeMap::new();
    capture_trusted_catalog_file_v1(
        &canonical_file,
        "sealed-path fixture",
        trusted_uid,
        &mut captured,
        false,
    )
    .expect("capture trusted fixture path");
    let paths = captured.into_values().collect::<Vec<_>>();
    verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).expect("unchanged fixture path");
    let file_index = paths
        .iter()
        .position(|entry| entry.canonical_path == canonical_file.to_string_lossy())
        .expect("sealed fixture file entry");
    let mut changed_path = paths.clone();
    changed_path[file_index]
        .canonical_path
        .push_str(".replacement");
    assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_path, trusted_uid).is_err());
    let mut changed_inode = paths.clone();
    changed_inode[file_index].stat.inode = changed_inode[file_index].stat.inode.saturating_add(1);
    assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_inode, trusted_uid).is_err());
    let mut changed_owner = paths.clone();
    changed_owner[file_index].stat.owner_uid =
        changed_owner[file_index].stat.owner_uid.saturating_add(1);
    assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_owner, trusted_uid).is_err());
    let mut changed_mode = paths.clone();
    changed_mode[file_index].stat.mode ^= 0o100;
    assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_mode, trusted_uid).is_err());
    let mut changed_time = paths;
    changed_time[file_index].stat.changed_nanoseconds =
        (changed_time[file_index].stat.changed_nanoseconds + 1) % 1_000_000_000;
    assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_time, trusted_uid).is_err());
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn sealed_path_revalidation_rejects_content_replacement_and_writable_mode() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = kagemusha_catalog_test_tempdir().expect("ACL-free temporary sealed-path root");
    let file_path = temporary.path().join("sealed.bin");
    std::fs::write(&file_path, b"original bytes").expect("write sealed-path fixture");
    let canonical_file = std::fs::canonicalize(&file_path).expect("canonical fixture file");
    let trusted_uid = rustix::process::geteuid().as_raw();
    let mut captured = BTreeMap::new();
    capture_trusted_catalog_file_v1(
        &canonical_file,
        "sealed-path fixture",
        trusted_uid,
        &mut captured,
        false,
    )
    .expect("capture trusted fixture path");
    let paths = captured.into_values().collect::<Vec<_>>();
    std::fs::write(&canonical_file, b"tampered bytes").expect("tamper fixture in place");
    assert!(
        verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
        "in-place byte mutation must change the sealed stat identity"
    );
    let mut permissions = std::fs::metadata(&canonical_file)
        .expect("inspect fixture permissions")
        .permissions();
    permissions.set_mode(0o664);
    std::fs::set_permissions(&canonical_file, permissions).expect("make fixture group-writable");
    assert!(
        verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
        "a group-writable sealed file must fail root-trust validation"
    );
    std::fs::remove_file(&canonical_file).expect("remove original fixture inode");
    std::fs::write(&canonical_file, b"replacement obj").expect("replace fixture inode");
    assert!(
        verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
        "path replacement must fail the inode seal"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn write_test_policy(root: &Path) -> std::path::PathBuf {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
        KagemushaRecursiveSpendReleaseApprovalRoleV1, KagemushaRecursiveSpendReleaseRolePolicyV1,
    };
    let roles = [
        KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
        KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
    ];
    let policy = KagemushaRecursiveSpendReleasePolicyV1 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        policy_id: "catalog-loader-test-policy".to_owned(),
        internal_validation_runner_identity_sha256: [0x91; 32],
        roles: roles
            .into_iter()
            .enumerate()
            .map(|(index, role)| {
                let seed = u8::try_from(index + 1).expect("small signer index");
                KagemushaRecursiveSpendReleaseRolePolicyV1 {
                    role,
                    threshold: 1,
                    authorized_signers: vec![
                        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                            .public_key()
                            .clone(),
                    ],
                }
            })
            .collect(),
    };
    policy.validate().expect("valid catalog test policy");
    let path = root.join("policy.norito");
    std::fs::write(
        &path,
        norito::to_bytes(&policy).expect("canonical catalog test policy"),
    )
    .expect("write catalog test policy");
    path
}
fn verifier_record_for_manifest(manifest_sha256: [u8; 32]) -> VerifyingKeyRecord {
    VerifyingKeyRecord::new_with_owner(
        1,
        "catalog-test-circuit",
        Some(format!(
            "{VERIFIER_OWNER_MANIFEST_PREFIX_V4}{}",
            hex::encode(manifest_sha256)
        )),
        KAGEMUSHA_VERIFIER_NAMESPACE,
        BackendTag::Halo2IpaPasta,
        STEP_EQ_VERIFIER_CURVE_V4,
        [0x31; 32],
        [0x32; 32],
    )
}
#[test]
fn empty_catalog_is_explicitly_unconfigured() {
    let catalog = KagemushaReleaseCatalogV4::empty();
    assert!(!catalog.is_configured());
    assert_eq!(catalog.configured_policy_sha256(), None);
    assert_eq!(catalog.consensus_policy_digest(), None);
    assert!(catalog.is_empty());
}
#[test]
fn catalog_consensus_identity_binds_inventory_with_stable_ordering() {
    let release_identity = |seed| KagemushaCatalogReleaseConsensusIdentityV1 {
        manifest_sha256: [seed; 32],
        release_record_sha256: [seed.wrapping_add(1); 32],
        qualification_receipt_sha256: [seed.wrapping_add(2); 32],
        qualified_candidate_sha256: [seed.wrapping_add(3); 32],
    };
    let policy_sha256 = [0x41; 32];
    let release_a = release_identity(0x10);
    let release_b = release_identity(0x20);
    let ordered = kagemusha_catalog_consensus_policy_digest_from_identities_v1(
        policy_sha256,
        vec![release_a.clone(), release_b.clone()],
    );
    let reordered = kagemusha_catalog_consensus_policy_digest_from_identities_v1(
        policy_sha256,
        vec![release_b.clone(), release_a.clone()],
    );
    assert_eq!(ordered, reordered, "catalog iteration order must be stable");
    assert_ne!(
        ordered,
        kagemusha_catalog_consensus_policy_digest_from_identities_v1(
            policy_sha256,
            vec![release_a.clone()],
        ),
        "different authenticated release inventories must not share an identity"
    );
    let mut changed_release = release_a;
    changed_release.release_record_sha256[0] ^= 1;
    assert_ne!(
        kagemusha_catalog_consensus_policy_digest_from_identities_v1(
            policy_sha256,
            vec![release_b, changed_release],
        ),
        ordered,
        "consensus-relevant cached release identity must be bound"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn staged_genesis_authenticates_only_policy_while_runtime_requires_nonempty_catalog() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let policy_bytes = std::fs::read(&policy).expect("read test release policy");
    let expected_policy_sha256: [u8; 32] = Sha256::digest(policy_bytes).into();
    let mut config = iroha_config::parameters::actual::Offline::default();
    config.kagemusha_release_policy_path = Some(policy);
    config.kagemusha_artifact_dir = Some(root.join("not-yet-generated-catalog"));
    config.kagemusha_catalog_qualification_seal_path = None;

    let trusted_uid = rustix::process::geteuid().as_raw();
    let staged = KagemushaReleaseCatalogV4::load_policy_only_for_genesis_staging_for_trusted_uid(
        config
            .kagemusha_release_policy_path
            .as_deref()
            .expect("configured test policy"),
        config
            .kagemusha_artifact_dir
            .as_deref()
            .expect("configured future artifact directory"),
        trusted_uid,
    )
    .expect("staged genesis must bind the authenticated policy before release generation");
    assert!(staged.is_configured());
    assert!(staged.is_empty());
    assert_eq!(
        staged.configured_policy_sha256(),
        Some(expected_policy_sha256)
    );

    let artifact_dir = config
        .kagemusha_artifact_dir
        .as_deref()
        .expect("configured future artifact directory");
    std::fs::create_dir(artifact_dir).expect("create stale empty artifact directory");
    let stale_artifact_error =
        KagemushaReleaseCatalogV4::load_policy_only_for_genesis_staging_for_trusted_uid(
            config
                .kagemusha_release_policy_path
                .as_deref()
                .expect("configured test policy"),
            artifact_dir,
            trusted_uid,
        )
        .err()
        .expect("staged genesis must reject even an empty stale artifact directory");
    assert!(stale_artifact_error.contains("must not exist"));
    std::fs::remove_dir(artifact_dir).expect("remove stale empty artifact directory");

    let mut sealed_staging = config.clone();
    sealed_staging.kagemusha_catalog_qualification_seal_path =
        Some(root.join("not-yet-generated-qualification-seal.norito"));
    let sealed_staging_error =
        KagemushaReleaseCatalogV4::from_offline_config_for_genesis_staging(&sealed_staging)
            .err()
            .expect("staged genesis must reject a future seal path");
    assert!(sealed_staging_error.contains("must omit"));

    std::fs::create_dir(artifact_dir).expect("create empty runtime artifact directory");
    let runtime = KagemushaReleaseCatalogV4::from_offline_config(&config)
        .err()
        .expect("runtime must not accept a policy-only staged catalog");
    assert!(
        runtime.contains("contains no releases"),
        "unexpected runtime error: {runtime}"
    );
}
#[test]
fn production_catalog_has_no_eager_artifact_materializer_path() {
    let module = include_str!("../kagemusha_terminal_registry_v4.rs");
    let owner_body = |name: &str| {
        let start = module
            .find(name)
            .unwrap_or_else(|| panic!("missing production owner `{name}`"));
        let tail = &module[start..];
        let end = tail
            .find("\n}")
            .unwrap_or_else(|| panic!("unterminated production owner `{name}`"));
        &tail[..end]
    };
    let resolved = owner_body("pub(crate) struct ResolvedKagemushaTerminalVerifierV4");
    let cached = owner_body("pub(crate) struct KagemushaCachedReleaseV4");
    let catalog = owner_body("pub struct KagemushaReleaseCatalogV4");
    for owner in [resolved, cached, catalog] {
        assert!(!owner.contains(concat!("KagemushaPastaCycleVerifier", "ArtifactsV4")));
    }
    assert!(resolved.contains("qualified_source: Arc<KagemushaQualifiedArtifactSourceV4>"));
    assert!(resolved.contains("verifier: Arc<KagemushaPastaCycleOpaqueVerifierV4>"));
    assert!(module.contains("from_qualified_artifact_source"));
    assert!(module.contains("verify_catalog_qualification_receipt_v4"));
    assert!(module.contains("verify_candidate_recursive_step_two_receipt_v4"));
    for forbidden in [
        concat!("KagemushaPastaCycleVerifier", "ArtifactsV4"),
        concat!("KagemushaValidatedArtifact", "PayloadV4"),
        concat!("from_", "authenticated_artifacts"),
    ] {
        assert!(
            !module.contains(forbidden),
            "production catalog contains forbidden eager symbol `{forbidden}`"
        );
    }
}
#[test]
fn production_release_inventory_requires_receipts_and_eighteen_unique_files() {
    let module = include_str!("../kagemusha_terminal_registry_v4.rs");
    let inventory = module
        .split_once("fn verify_exact_release_inventory_v4(")
        .expect("exact release inventory verifier")
        .1
        .split_once("fn open_qualified_kagemusha_catalog_source_v4(")
        .expect("release inventory verifier boundary")
        .0;
    assert!(inventory.contains("KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4"));
    assert!(
        inventory.contains("KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1")
    );
    assert!(inventory.contains("expected.len() != 18"));
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_source_is_exact_read_only_and_rewinds_once_per_callback() {
    let (_temporary, _release_directory, source) = pinned_source_fixture();
    assert_eq!(source.artifacts.len(), KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4);
    source
        .validate_snapshot()
        .expect("all exact handles remain read-only");
    let mut callback_count = 0_u8;
    for _ in 0..2 {
        let mut callback = |reader: &mut dyn KagemushaArtifactReadSeekV4| {
            callback_count = callback_count.saturating_add(1);
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() != 128 || bytes.iter().any(|byte| *byte != 1) {
                return Err("pinned source did not rewind to the original Eq params".to_owned());
            }
            Ok(())
        };
        source
            .with_framed_artifact(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                &mut callback,
            )
            .expect("lend one exact pinned file");
    }
    assert_eq!(callback_count, 2);
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_source_rejects_wrong_role_without_invoking_callback() {
    let (_temporary, _release_directory, mut source) = pinned_source_fixture();
    source.artifacts[0].parity = KagemushaPastaCycleParityV1::StepEp;
    let mut invoked = false;
    let mut callback = |_reader: &mut dyn KagemushaArtifactReadSeekV4| {
        invoked = true;
        Ok(())
    };
    let error = source
        .with_framed_artifact(
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            &mut callback,
        )
        .expect_err("a role-substituted source must fail closed");
    assert!(error.contains("no exact artifact role"));
    assert!(!invoked);
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_source_rejects_in_place_tamper_and_trailing_growth() {
    use std::io::Write as _;
    {
        let (_temporary, release_directory, source) = pinned_source_fixture();
        let file_name = source.artifacts[0].descriptor.file_name.clone();
        std::fs::write(release_directory.join(&file_name), vec![0xa5; 128])
            .expect("tamper pinned artifact in place");
        let mut invoked = false;
        let tamper_error = source
            .with_selected_file(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                |_file| {
                    invoked = true;
                    Ok(())
                },
            )
            .expect_err("in-place tamper must invalidate the pinned snapshot");
        assert!(tamper_error.contains("changed identity, bytes, or read-only"));
        assert!(!invoked);
    }
    let (_temporary, release_directory, source) = pinned_source_fixture();
    let file_name = source.artifacts[0].descriptor.file_name.clone();
    std::fs::OpenOptions::new()
        .append(true)
        .open(release_directory.join(&file_name))
        .and_then(|mut file| file.write_all(b"trailing"))
        .expect("append trailing bytes to pinned artifact");
    let trailing_error = source
        .with_selected_file(
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            |_file| Ok(()),
        )
        .expect_err("trailing growth must invalidate the pinned snapshot");
    assert!(trailing_error.contains("changed identity, bytes, or read-only"));
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_source_global_permit_serializes_all_roles() {
    use std::{
        sync::{
            Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    let (_temporary, _release_directory, source) = pinned_source_fixture();
    let source = Arc::new(source);
    let barrier = Arc::new(Barrier::new(3));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut threads = Vec::new();
    for kind in [
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    ] {
        let source = Arc::clone(&source);
        let barrier = Arc::clone(&barrier);
        let active = Arc::clone(&active);
        let maximum = Arc::clone(&maximum);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            source
                .with_selected_file(KagemushaPastaCycleParityV1::StepEq, kind, |_file| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(25));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
                .expect("serialized pinned source access");
        }));
    }
    barrier.wait();
    for thread in threads {
        thread.join().expect("pinned-source worker");
    }
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
}
#[test]
fn manifest_directory_names_are_canonical_lowercase_sha256() {
    let digest = [0xab; 32];
    let encoded = hex::encode(digest);
    assert_eq!(parse_manifest_directory_name(&encoded), Ok(digest));
    assert!(parse_manifest_directory_name(&encoded.to_uppercase()).is_err());
    assert!(parse_manifest_directory_name(&encoded[..63]).is_err());
}
#[test]
fn release_state_key_is_manifest_content_addressed() {
    let binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "release-1".to_owned(),
        manifest_sha256: [0x5a; 32],
    };
    assert_eq!(
        release_state_key(&binding)
            .expect("valid V4 release key")
            .to_string(),
        format!(
            "{TERMINAL_RELEASE_STATE_KEY_PREFIX_V4}{}",
            hex::encode(binding.manifest_sha256)
        )
    );
}
#[test]
fn runtime_promotion_validation_rejects_candidate_digest_substitution() {
    let (authenticated, promotion, _) = authenticated_candidate_binding_release();
    promotion
        .validate_against_authenticated_release(&authenticated)
        .expect("exact reconstructed candidate binding");
    let mut substituted = promotion;
    substituted.candidate_sha256[0] ^= 1;
    substituted
        .validate()
        .expect("substituted candidate digest remains structurally valid");
    assert_eq!(
        substituted.validate_against_authenticated_release(&authenticated),
        Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_malformed_policy_before_publication() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = root.join("policy.norito");
    let artifacts = root.join("artifacts");
    std::fs::write(&policy, b"not canonical norito").expect("write malformed policy");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("malformed configured policy must fail closed");
    assert!(error.contains("policy") || error.contains("malformed"));
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_empty_release_inventory() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("an empty configured catalog must fail closed");
    assert!(
        error.contains("contains no releases"),
        "unexpected error: {error}"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_release_count_above_retention_bound() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    for index in 0..=MAX_CATALOG_RELEASES_V4 {
        std::fs::create_dir(artifacts.join(format!("{index:064x}")))
            .expect("create bounded release directory");
    }
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("a catalog above the release-retention bound must fail closed");
    assert!(
        error.contains("at most") && error.contains(&MAX_CATALOG_RELEASES_V4.to_string()),
        "unexpected error: {error}"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_aggregate_byte_accounting_is_bounded() {
    const CORRECTED_EQ_EP_PROVING_KEYS_BYTES: u64 = 2 * 5_347_763_078;
    assert_eq!(MAX_CATALOG_AGGREGATE_BYTES_V4, 12 * 1024 * 1024 * 1024);
    assert!(
        CORRECTED_EQ_EP_PROVING_KEYS_BYTES < MAX_CATALOG_AGGREGATE_BYTES_V4,
        "the authenticated Eq/Ep proving keys must fit below the internal catalog ceiling"
    );
    assert_eq!(
        add_catalog_release_bytes(0, CORRECTED_EQ_EP_PROVING_KEYS_BYTES),
        Ok(CORRECTED_EQ_EP_PROVING_KEYS_BYTES)
    );
    assert_eq!(
        add_catalog_release_bytes(MAX_CATALOG_AGGREGATE_BYTES_V4 - 1, 1),
        Ok(MAX_CATALOG_AGGREGATE_BYTES_V4)
    );
    let error = add_catalog_release_bytes(MAX_CATALOG_AGGREGATE_BYTES_V4, 1)
        .expect_err("an aggregate catalog above the byte bound must fail closed");
    assert!(
        error.contains("aggregate byte limit"),
        "unexpected error: {error}"
    );
    let error = add_catalog_release_bytes(u64::MAX, 1)
        .expect_err("aggregate byte accounting overflow must fail closed");
    assert!(error.contains("overflowed"), "unexpected error: {error}");
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_manifest_directory_digest_substitution() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    let release = artifacts.join(hex::encode([0x55; 32]));
    std::fs::create_dir_all(&release).expect("create substituted release directory");
    std::fs::write(
        release.join(MANIFEST_FILE_NAME_V4),
        b"different manifest bytes",
    )
    .expect("write substituted manifest");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("a manifest under a different digest must fail closed");
    assert!(
        error.contains("manifest digest does not match its directory"),
        "unexpected error: {error}"
    );
}
#[test]
fn activation_records_require_one_exact_v4_manifest_owner() {
    let manifest_sha256 = [0x61; 32];
    let step_eq = verifier_record_for_manifest(manifest_sha256);
    let step_ep = verifier_record_for_manifest(manifest_sha256);
    assert_eq!(
        activation_manifest_sha256(&step_eq, &step_ep),
        Ok(manifest_sha256)
    );
    let other = verifier_record_for_manifest([0x62; 32]);
    let error = activation_manifest_sha256(&step_eq, &other)
        .expect_err("cross-manifest Eq/Ep records must fail closed");
    assert!(error.contains("select different releases"));
    let mut retired = step_ep;
    retired.owner_manifest_id = Some(format!("kagemusha-v3-{}", hex::encode(manifest_sha256)));
    let error = activation_manifest_sha256(&step_eq, &retired)
        .expect_err("a retired owner namespace must fail closed");
    assert!(error.contains("owner namespace is invalid"));
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_symlinked_policy_and_artifact_roots() {
    use std::os::unix::fs::symlink;
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    let policy_link = root.join("policy-link.norito");
    symlink(&policy, &policy_link).expect("create policy symlink");
    let error = read_bounded_regular_file(&policy_link, MAX_POLICY_BYTES, "release policy")
        .err()
        .expect("a symlinked policy leaf must fail before artifact scanning");
    assert!(
        error.contains("direct single-link regular file"),
        "unexpected error: {error}"
    );
    let artifact_link = root.join("artifact-link");
    symlink(&artifacts, &artifact_link).expect("create artifact-root symlink");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifact_link)
        .err()
        .expect("a symlinked artifact root must fail closed");
    assert!(error.contains("not a real directory"));
    for suffix in ["/", "/."] {
        let mut spelling = artifact_link.as_os_str().to_os_string();
        spelling.push(suffix);
        let error = CatalogDirectory::open_path(
            Path::new(&spelling),
            "non-canonical symlinked artifact root",
        )
        .err()
        .expect("a trailing component must not turn the final symlink into an intermediate");
        assert!(error.contains("canonical"), "unexpected error: {error}");
    }
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_paths_reject_intermediate_symlinks() {
    use std::os::unix::fs::symlink;
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let real_parent = root.join("real-parent");
    let artifacts = real_parent.join("artifacts");
    let policy = real_parent.join("policy.norito");
    std::fs::create_dir(&real_parent).expect("create real parent");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    std::fs::write(&policy, b"policy bytes").expect("write policy leaf");
    let intermediate = root.join("intermediate-link");
    symlink(&real_parent, &intermediate).expect("create intermediate symlink");
    let artifact_error = CatalogDirectory::open_path(
        &intermediate.join("artifacts"),
        "intermediate-symlink artifact root",
    )
    .err()
    .expect("an intermediate artifact-root symlink must fail closed");
    assert!(
        artifact_error.contains("not a real directory"),
        "unexpected error: {artifact_error}"
    );
    let policy_error = read_bounded_regular_file(
        &intermediate.join("policy.norito"),
        MAX_POLICY_BYTES,
        "intermediate-symlink release policy",
    )
    .err()
    .expect("an intermediate policy symlink must fail closed");
    assert!(
        policy_error.contains("not a real directory"),
        "unexpected error: {policy_error}"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_paths_must_be_absolute() {
    let error = CatalogDirectory::open_path(Path::new("relative-artifacts"), "artifact root")
        .err()
        .expect("a relative configured catalog path must fail closed");
    assert!(error.contains("absolute"), "unexpected error: {error}");
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn configured_catalog_rejects_symlinked_release_directory_and_manifest_leaf() {
    use std::os::unix::fs::symlink;
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = write_test_policy(&root);
    let artifacts = root.join("artifacts");
    let external_release = root.join("external-release");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    std::fs::create_dir(&external_release).expect("create external release");
    let release_name = hex::encode([0x71; 32]);
    symlink(&external_release, artifacts.join(&release_name))
        .expect("create release-directory symlink");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("a symlinked release directory must fail closed");
    assert!(
        error.contains("not a real directory"),
        "unexpected error: {error}"
    );
    std::fs::remove_file(artifacts.join(&release_name)).expect("remove release-directory symlink");
    let release = artifacts.join(&release_name);
    std::fs::create_dir(&release).expect("create real release directory");
    let external_manifest = root.join("external-manifest.norito");
    std::fs::write(&external_manifest, b"substituted manifest").expect("write external manifest");
    symlink(&external_manifest, release.join(MANIFEST_FILE_NAME_V4))
        .expect("create manifest symlink");
    let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
        .err()
        .expect("a symlinked manifest leaf must fail before decoding");
    assert!(
        error.contains("direct single-link regular file"),
        "unexpected error: {error}"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_directory_reads_original_object_and_rejects_path_replacement() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let temporary_root = canonical_temporary_root(&temporary);
    let artifacts = temporary_root.join("artifacts");
    let displaced = temporary_root.join("artifacts-displaced");
    std::fs::create_dir(&artifacts).expect("create artifact root");
    std::fs::write(artifacts.join("original.bin"), b"original").expect("write original artifact");
    let pinned = CatalogDirectory::open_path(&artifacts, "test artifact root")
        .expect("pin original artifact root");
    std::fs::rename(&artifacts, &displaced).expect("displace original artifact root");
    std::fs::create_dir(&artifacts).expect("install replacement artifact root");
    std::fs::write(artifacts.join("original.bin"), b"replacement")
        .expect("write replacement artifact");
    let mut opened = pinned
        .open_file("original.bin", "test artifact")
        .expect("open through pinned original directory");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut opened.file, &mut bytes)
        .expect("read pinned original artifact");
    opened.verify_unchanged().expect("original file is stable");
    assert_eq!(bytes, b"original");
    assert!(
        pinned.verify_path_identity().is_err(),
        "publication must reject a replaced configured path"
    );
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn retained_policy_handle_rejects_post_read_mutation() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let root = canonical_temporary_root(&temporary);
    let policy = root.join("policy.norito");
    std::fs::write(&policy, b"initial-policy").expect("write initial policy");
    let parent =
        CatalogDirectory::open_path(&root, "release policy parent").expect("pin policy parent");
    let mut opened = parent
        .open_file("policy.norito", "release policy")
        .expect("pin policy file");
    let bytes = read_bounded_opened_file(&mut opened, MAX_POLICY_BYTES, "release policy")
        .expect("read stable initial policy");
    assert_eq!(bytes, b"initial-policy");
    std::fs::write(&policy, b"changed-policy-with-a-different-length")
        .expect("mutate policy after read");
    let error = opened
        .verify_unchanged()
        .expect_err("the retained policy handle must detect post-read mutation");
    assert!(error.contains("changed"), "unexpected error: {error}");
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[test]
fn pinned_release_reads_original_object_and_rejects_entry_replacement() {
    let temporary = kagemusha_catalog_test_tempdir().expect("temporary catalog root");
    let temporary_root = canonical_temporary_root(&temporary);
    let root = temporary_root.join("artifacts");
    let release_name = hex::encode([0x72; 32]);
    let release = root.join(&release_name);
    let displaced = root.join("displaced-release");
    std::fs::create_dir_all(&release).expect("create release directory");
    std::fs::write(release.join("original.bin"), b"original").expect("write original artifact");
    let pinned_root =
        CatalogDirectory::open_path(&root, "test artifact root").expect("pin artifact root");
    let pinned_release = pinned_root
        .open_directory(&release_name, "test release")
        .expect("pin release directory");
    std::fs::rename(&release, &displaced).expect("displace original release");
    std::fs::create_dir(&release).expect("install replacement release");
    std::fs::write(release.join("original.bin"), b"replacement")
        .expect("write replacement artifact");
    let mut opened = pinned_release
        .open_file("original.bin", "test release artifact")
        .expect("open through pinned original release");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut opened.file, &mut bytes)
        .expect("read pinned original release artifact");
    opened.verify_unchanged().expect("original file is stable");
    assert_eq!(bytes, b"original");
    assert!(
        pinned_root
            .verify_directory_entry(&release_name, &pinned_release)
            .is_err(),
        "publication must reject a replaced release entry"
    );
}
