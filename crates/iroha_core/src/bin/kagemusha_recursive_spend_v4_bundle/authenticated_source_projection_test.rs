#[test]
fn authenticated_source_projection_decodes_once_and_binds_the_exact_closure() {
    let commit = "a".repeat(40);
    let source_tree = [0x11; 32];
    let source_tree_hex = hex::encode(source_tree);
    let empty_sha256: [u8; 32] = Sha256::digest([]).into();
    let mut combined = Sha256::new();
    combined.update(b"iroha-source-diff-v1\0");
    combined.update(b"tracked-binary-diff-sha256\0");
    combined.update(empty_sha256);
    combined.update(b"untracked-path-blob-manifest-sha256\0");
    combined.update(empty_sha256);
    let combined_sha256: [u8; 32] = combined.finalize().into();
    let closure = KagemushaReviewedSourceClosureV1 {
        schema: "iroha.reviewed-source-closure.v1".to_owned(),
        base_commit: commit.clone(),
        source_commit: commit.clone(),
        source_repo_dirty: false,
        source_tree_sha256: source_tree,
        tracked_binary_diff_sha256: empty_sha256,
        untracked_file_count: 0,
        untracked_path_mode_blob_oid_manifest: Vec::new(),
        untracked_path_mode_blob_oid_manifest_sha256: empty_sha256,
        ignored_cargo_lock_size_bytes: 1,
        ignored_cargo_lock_sha256: [0x22; 32],
        combined_source_fingerprint_sha256: combined_sha256,
    };
    closure.validate().expect("valid clean closure fixture");
    let closure_bytes = format!(
        concat!(
            "{{\"base_commit\":\"{}\",",
            "\"combined_source_fingerprint_sha256\":\"{}\",",
            "\"ignored_cargo_lock_sha256\":\"{}\",",
            "\"ignored_cargo_lock_size_bytes\":1,",
            "\"schema\":\"iroha.reviewed-source-closure.v1\",",
            "\"source_commit\":\"{}\",\"source_repo_dirty\":false,",
            "\"source_tree_sha256\":\"{}\",",
            "\"tracked_binary_diff_sha256\":\"{}\",",
            "\"untracked_file_count\":0,",
            "\"untracked_path_mode_blob_oid_manifest\":[],",
            "\"untracked_path_mode_blob_oid_manifest_sha256\":\"{}\"}}\n"
        ),
        commit,
        hex::encode(combined_sha256),
        hex::encode(closure.ignored_cargo_lock_sha256),
        commit,
        source_tree_hex,
        hex::encode(empty_sha256),
        hex::encode(empty_sha256),
    )
    .into_bytes();
    let closure_sha256 = hex::encode(Sha256::digest(&closure_bytes));
    assert_eq!(
        hex::encode(
            closure
                .canonical_descriptor_sha256()
                .expect("closure descriptor digest")
        ),
        closure_sha256
    );
    let source_date_epoch = AUTHORIZED_SOURCE_PARENT_EPOCH + 1;
    let reviewed_cargo_binary_sha256 = "9".repeat(64);
    let reviewed_rustc_binary_sha256 = "a".repeat(64);
    let projection = AuthenticatedSourceSealProjectionV1 {
        build_script_observed: SourceSealBuildScriptObservedV1 {
            debug_assertions: false,
            features: SOURCE_SEAL_RESOLVED_FEATURES
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            host: "aarch64-apple-darwin".to_owned(),
            num_jobs: 1,
            opt_level: "3".to_owned(),
            profile: "release".to_owned(),
            schema: SOURCE_SEAL_BUILD_SCRIPT_OBSERVED_SCHEMA.to_owned(),
            target: "aarch64-apple-darwin".to_owned(),
        },
        outer_policy: SourceSealOuterPolicyV1 {
            cargo: SourceSealCargoPolicyV1 {
                binary: "kagemusha_recursive_spend_v4_bundle".to_owned(),
                explicit_features: SOURCE_SEAL_EXPLICIT_FEATURES
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                package: "iroha_core".to_owned(),
                profile: "release".to_owned(),
                semantic_argv: SOURCE_SEAL_SEMANTIC_ARGV
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                target: "aarch64-apple-darwin".to_owned(),
                unit_graph: SourceSealUnitGraphV1 {
                    custom_build_packages: 1,
                    custom_build_units: 1,
                    iroha_core_units: 1,
                    normalization: SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION.to_owned(),
                    packages: 1,
                    raw_sha256: "2".repeat(64),
                    raw_size_bytes: 1,
                    sha256: "3".repeat(64),
                    size_bytes: 1,
                    units: 1,
                },
            },
            execution_policy_sha256: "4".repeat(64),
            schema: SOURCE_SEAL_OUTER_POLICY_SCHEMA.to_owned(),
            toolchain: SourceSealToolchainV1 {
                cargo: SourceSealToolIdentityV1 {
                    binary_sha256: reviewed_cargo_binary_sha256.clone(),
                    binary_size_bytes: 1,
                },
                rustc: SourceSealToolIdentityV1 {
                    binary_sha256: reviewed_rustc_binary_sha256.clone(),
                    binary_size_bytes: 1,
                },
            },
        },
        reviewed_source_closure_hex: hex::encode(&closure_bytes),
        reviewed_source_closure_sha256: closure_sha256.clone(),
        schema: AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA.to_owned(),
        source_authority: SourceSealAuthorityV1 {
            commit: commit.clone(),
            commit_object_sha256: "5".repeat(64),
            commit_object_size: 1,
            committer_epoch: source_date_epoch,
            git_tree: "b".repeat(40),
            ordered_parents: vec![AUTHORIZED_SOURCE_PARENT_COMMIT.to_owned()],
            parent_commit: AUTHORIZED_SOURCE_PARENT_COMMIT.to_owned(),
            parent_tree: AUTHORIZED_SOURCE_PARENT_TREE.to_owned(),
            signature: SourceSealSshSignatureV1 {
                allowed_signers_sha256: "6".repeat(64),
                mechanism: "git-commit-ssh-signature-v1".to_owned(),
                principal: "release@example.org".to_owned(),
                public_key_sha256: "7".repeat(64),
                revocation_sha256: "8".repeat(64),
                signature_namespace: "git".to_owned(),
            },
        },
        source_commit: commit.clone(),
        source_date_epoch,
        source_repo_dirty: false,
        source_tree_sha256: source_tree_hex.clone(),
    };
    let mut projection_bytes = norito::json::to_json(&projection)
        .expect("serialize projection")
        .into_bytes();
    projection_bytes.push(b'\n');
    let projection_hex = hex::encode(&projection_bytes);
    let projection_sha256 = hex::encode(Sha256::digest(&projection_bytes));
    let decoded = decode_embedded_source_seal(
        Some(&projection_hex),
        Some(&projection_sha256),
        Some(&reviewed_cargo_binary_sha256),
        Some(&reviewed_rustc_binary_sha256),
        Some(&commit),
        Some(&source_tree_hex),
        Some(&source_date_epoch.to_string()),
    )
    .expect("decode exact authenticated projection");
    assert_eq!(decoded.identity.reviewed_source_closure, closure);
    assert_eq!(
        decoded.identity.authenticated_source_seal_projection_sha256,
        projection_sha256
    );
    assert_eq!(
        decoded.identity.reviewed_cargo_binary_sha256,
        reviewed_cargo_binary_sha256
    );
    assert_eq!(
        decoded.identity.reviewed_rustc_binary_sha256,
        reviewed_rustc_binary_sha256
    );
    assert!(
        decode_embedded_source_seal(
            Some(&projection_hex),
            Some(&"0".repeat(64)),
            Some(&reviewed_cargo_binary_sha256),
            Some(&reviewed_rustc_binary_sha256),
            Some(&commit),
            Some(&source_tree_hex),
            Some(&source_date_epoch.to_string()),
        )
        .is_err()
    );
    assert!(
        decode_embedded_source_seal(
            Some(&projection_hex),
            Some(&projection_sha256),
            Some(&"0".repeat(64)),
            Some(&reviewed_rustc_binary_sha256),
            Some(&commit),
            Some(&source_tree_hex),
            Some(&source_date_epoch.to_string()),
        )
        .is_err()
    );
    assert!(
        decode_embedded_source_seal(
            Some(&projection_hex),
            Some(&projection_sha256),
            Some(&"b".repeat(64)),
            Some(&reviewed_rustc_binary_sha256),
            Some(&commit),
            Some(&source_tree_hex),
            Some(&source_date_epoch.to_string()),
        )
        .is_err()
    );
    assert!(
        decode_embedded_source_seal(
            Some(&projection_hex),
            Some(&projection_sha256),
            Some(&reviewed_cargo_binary_sha256),
            Some(&"0".repeat(64)),
            Some(&commit),
            Some(&source_tree_hex),
            Some(&source_date_epoch.to_string()),
        )
        .is_err()
    );
    let mut substituted_projection_bytes = projection_bytes;
    substituted_projection_bytes[0] ^= 1;
    assert!(
        decode_embedded_source_seal(
            Some(&hex::encode(substituted_projection_bytes)),
            Some(&projection_sha256),
            Some(&reviewed_cargo_binary_sha256),
            Some(&reviewed_rustc_binary_sha256),
            Some(&commit),
            Some(&source_tree_hex),
            Some(&source_date_epoch.to_string()),
        )
        .is_err()
    );
}
