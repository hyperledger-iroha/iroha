//! Review pinning, exact bounded source custody and refusal of retained owners.

use super::*;

fn native(label: &str) -> ImplementationCodeV1 {
    ImplementationCodeV1::Native {
        module: label.into(),
        reference_closure: Sha256::digest(label.as_bytes()).into(),
    }
}

fn unit(path: &str, bytes: &[u8]) -> SourceUnitV1 {
    SourceUnitV1 {
        path: path.into(),
        bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes).into(),
        native_owners: vec![native("iroha.reviewed.handler")],
        retained_uses: Vec::new(),
    }
}

fn fixture() -> (tempfile::TempDir, SourceManifestV1, SourceRemovalRequestV1) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    let original = b"fn reviewed_handler() {}\n";
    std::fs::write(directory.path().join("src/handler.rs"), original).unwrap();
    let manifest = SourceManifestV1 {
        version: 1,
        units: vec![unit("src/handler.rs", original)],
    };
    let request = SourceRemovalRequestV1 {
        paths: vec!["src/handler.rs".into()],
        native_owners: manifest.units[0].native_owners.clone(),
    };
    (directory, manifest, request)
}

fn limits() -> SourceReadLimits {
    SourceReadLimits {
        max_file_bytes: 4096,
        max_total_bytes: 8192,
    }
}

fn check<'a>(
    root: &Path,
    manifest: &'a SourceManifestV1,
    request: &'a SourceRemovalRequestV1,
) -> Result<CheckedSourceManifest<'a>, Error> {
    check_sources(
        root,
        manifest,
        manifest_digest(manifest)?,
        request,
        limits(),
    )
}

#[test]
fn checked_scope_preserves_exact_bytes_and_all_native_owners_without_authorizing_deletion() {
    let (directory, mut manifest, mut request) = fixture();
    manifest.units[0]
        .native_owners
        .push(native("iroha.reviewed.shared"));
    request.native_owners = manifest.units[0].native_owners.clone();
    std::fs::write(directory.path().join("src/reader.rs"), b"retained decoder").unwrap();
    let mut retained = unit("src/reader.rs", b"retained decoder");
    retained.native_owners.clear();
    retained.retained_uses.push(RetainedUseV1 {
        role: RetainedRoleV1::Decoder,
        owner: "iroha.reviewed.decoder".into(),
    });
    manifest.units.push(retained);
    let original = std::fs::read(directory.path().join(&request.paths[0])).unwrap();
    let pin = manifest_digest(&manifest).unwrap();
    let checked = check_sources(directory.path(), &manifest, pin, &request, limits()).unwrap();
    assert_eq!(checked.manifest_digest(), pin);
    assert_eq!(checked.requested_paths(), request.paths);
    assert_eq!(checked.required_native_owners(), request.native_owners);
    checked.recheck_preimages().unwrap();
    assert_eq!(
        std::fs::read(directory.path().join(&request.paths[0])).unwrap(),
        original
    );
    // Even an unrequested reviewed dependency remains an exact pinned original.
    std::fs::write(directory.path().join("src/reader.rs"), b"replaced decoder").unwrap();
    assert!(checked.recheck_preimages().is_err());
}

#[test]
fn local_norito_data_roundtrips_and_review_identity_binds_every_ownership_field() {
    let (_, manifest, request) = fixture();
    assert_eq!(
        norito::decode_canonical::<SourceManifestV1>(&norito::encode_canonical(&manifest).unwrap())
            .unwrap(),
        manifest
    );
    assert_eq!(
        norito::decode_canonical::<SourceRemovalRequestV1>(
            &norito::encode_canonical(&request).unwrap()
        )
        .unwrap(),
        request
    );
    let json = norito::json::to_json(&manifest).unwrap();
    assert_eq!(
        norito::json::from_json::<SourceManifestV1>(&json).unwrap(),
        manifest
    );
    let pin = manifest_digest(&manifest).unwrap();
    for mutate in [
        |manifest: &mut SourceManifestV1| manifest.units[0].path.push('x'),
        |manifest: &mut SourceManifestV1| manifest.units[0].bytes += 1,
        |manifest: &mut SourceManifestV1| manifest.units[0].sha256[0] ^= 1,
        |manifest: &mut SourceManifestV1| manifest.units[0].native_owners[0] = native("other"),
        |manifest: &mut SourceManifestV1| {
            manifest.units[0].retained_uses.push(RetainedUseV1 {
                role: RetainedRoleV1::ProofVerifier,
                owner: "retained.proof".into(),
            })
        },
    ] {
        let mut changed = manifest.clone();
        mutate(&mut changed);
        assert_ne!(manifest_digest(&changed).unwrap(), pin);
        assert!(matches!(
            check_sources(
                Path::new("unused-no-source-open"),
                &changed,
                pin,
                &request,
                limits()
            ),
            Err(Error::ReviewMismatch)
        ));
    }
}

#[test]
fn every_retained_dependency_role_refuses_removal_before_source_io() {
    let (_, manifest, request) = fixture();
    for role in [
        RetainedRoleV1::Decoder,
        RetainedRoleV1::CheckpointReader,
        RetainedRoleV1::ProofVerifier,
        RetainedRoleV1::CertificateVerifier,
        RetainedRoleV1::StateStorage,
        RetainedRoleV1::DescriptorOrReceipt,
        RetainedRoleV1::RuntimeProfile,
        RetainedRoleV1::Migration,
        RetainedRoleV1::Other,
    ] {
        let mut changed = manifest.clone();
        changed.units[0].retained_uses.push(RetainedUseV1 {
            role,
            owner: "reviewed.retained.owner".into(),
        });
        assert!(
            matches!(check(Path::new("unused-no-source-open"), &changed, &request), Err(Error::Retained { role: observed, .. }) if observed == role)
        );
    }
}

#[test]
fn unknown_sources_shared_native_owners_and_extra_owner_claims_refuse_before_io() {
    let (_, mut manifest, mut request) = fixture();
    request.paths[0] = "src/unknown.rs".into();
    assert!(matches!(
        check(Path::new("unused"), &manifest, &request),
        Err(Error::Invalid(
            "requested source is outside the reviewed inventory"
        ))
    ));
    request.paths[0] = manifest.units[0].path.clone();
    manifest.units[0]
        .native_owners
        .push(native("iroha.reviewed.shared"));
    assert!(matches!(
        check(Path::new("unused"), &manifest, &request),
        Err(Error::Invalid("requested source has another native owner"))
    ));
    request.native_owners = manifest.units[0].native_owners.clone();
    request.native_owners.push(native("unrelated"));
    assert!(matches!(
        check(Path::new("unused"), &manifest, &request),
        Err(Error::Invalid(
            "requested native owner owns no requested source"
        ))
    ));
}

#[test]
fn path_aliases_and_unbounded_or_ambiguous_manifests_are_rejected() {
    let (_, original, _) = fixture();
    for path in [
        "",
        "/absolute.rs",
        "../escape.rs",
        "src/../escape.rs",
        "src/./handler.rs",
        "src//handler.rs",
        "src/handler.rs/",
        "src/handler.rs.",
        "C:/handler.rs",
        "src\\handler.rs",
        "src/file\n.rs",
    ] {
        let mut manifest = original.clone();
        manifest.units[0].path = path.into();
        assert!(manifest_digest(&manifest).is_err(), "{path:?}");
    }
    let mut manifest = original.clone();
    manifest.units[0].path = "a/".repeat(MAX_PATH_COMPONENTS) + "handler.rs";
    assert!(manifest_digest(&manifest).is_err());
    manifest.units[0].path = "a".repeat(MAX_SOURCE_PATH_BYTES + 1);
    assert!(manifest_digest(&manifest).is_err());
    for mutate in [
        |manifest: &mut SourceManifestV1| manifest.version = 2,
        |manifest: &mut SourceManifestV1| manifest.units.clear(),
        |manifest: &mut SourceManifestV1| manifest.units.push(manifest.units[0].clone()),
        |manifest: &mut SourceManifestV1| {
            manifest.units = vec![manifest.units[0].clone(); MAX_SOURCE_UNITS + 1]
        },
        |manifest: &mut SourceManifestV1| manifest.units[0].native_owners.clear(),
        |manifest: &mut SourceManifestV1| {
            let duplicate = manifest.units[0].native_owners[0].clone();
            manifest.units[0].native_owners.push(duplicate);
        },
        |manifest: &mut SourceManifestV1| {
            manifest.units[0].native_owners[0] = ImplementationCodeV1::Ivm {
                code_hash: [1; 32],
                entrypoint: "main".into(),
            }
        },
    ] {
        let mut manifest = original.clone();
        mutate(&mut manifest);
        assert!(manifest_digest(&manifest).is_err());
    }
}

#[test]
fn read_limits_precede_io_and_preimages_require_exact_extent_and_digest() {
    let (directory, manifest, request) = fixture();
    let pin = manifest_digest(&manifest).unwrap();
    for bounds in [
        SourceReadLimits {
            max_file_bytes: 0,
            max_total_bytes: 8192,
        },
        SourceReadLimits {
            max_file_bytes: manifest.units[0].bytes - 1,
            max_total_bytes: 8192,
        },
        SourceReadLimits {
            max_file_bytes: 4096,
            max_total_bytes: manifest.units[0].bytes - 1,
        },
    ] {
        assert!(matches!(
            check_sources(Path::new("unused"), &manifest, pin, &request, bounds),
            Err(Error::Invalid(_))
        ));
    }
    let mut changed = manifest.clone();
    changed.units[0].bytes += 1;
    assert!(matches!(
        check(directory.path(), &changed, &request),
        Err(Error::PreimageChanged(_))
    ));
    changed.units[0].bytes -= 1;
    changed.units[0].sha256[0] ^= 1;
    assert!(matches!(
        check(directory.path(), &changed, &request),
        Err(Error::PreimageChanged(_))
    ));
    check_sources(
        directory.path(),
        &manifest,
        pin,
        &request,
        SourceReadLimits {
            max_file_bytes: manifest.units[0].bytes,
            max_total_bytes: manifest.units[0].bytes,
        },
    )
    .unwrap();
}

#[test]
fn cumulative_extent_overflow_and_ambiguous_requests_refuse_before_source_io() {
    let (_, mut manifest, request) = fixture();
    manifest.units[0].bytes = u64::MAX;
    manifest.units.push(unit("src/second.rs", b"x"));
    assert!(matches!(
        check_sources(
            Path::new("unused"),
            &manifest,
            manifest_digest(&manifest).unwrap(),
            &request,
            SourceReadLimits {
                max_file_bytes: u64::MAX,
                max_total_bytes: u64::MAX
            }
        ),
        Err(Error::Invalid("total source extent overflow"))
    ));

    let (_, manifest, original) = fixture();
    for mutate in [
        |request: &mut SourceRemovalRequestV1| request.paths.clear(),
        |request: &mut SourceRemovalRequestV1| request.paths.push(request.paths[0].clone()),
        |request: &mut SourceRemovalRequestV1| request.native_owners.clear(),
        |request: &mut SourceRemovalRequestV1| {
            request.native_owners.push(request.native_owners[0].clone())
        },
        |request: &mut SourceRemovalRequestV1| {
            request.native_owners[0] = ImplementationCodeV1::Ivm {
                code_hash: [1; 32],
                entrypoint: "main".into(),
            }
        },
    ] {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert!(matches!(
            check(Path::new("unused"), &manifest, &changed),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn review_pin_refuses_stripping_a_retained_dependency_and_bounds_each_owner_list() {
    let (_, mut manifest, request) = fixture();
    manifest.units[0].retained_uses.push(RetainedUseV1 {
        role: RetainedRoleV1::StateStorage,
        owner: "retained.storage".into(),
    });
    let reviewed = manifest_digest(&manifest).unwrap();
    manifest.units[0].retained_uses.clear();
    assert!(matches!(
        check_sources(Path::new("unused"), &manifest, reviewed, &request, limits()),
        Err(Error::ReviewMismatch)
    ));
    manifest.units[0].native_owners = (0..=MAX_SOURCE_OWNERS)
        .map(|index| native(&format!("owner.{index:03}")))
        .collect();
    assert!(matches!(
        manifest_digest(&manifest),
        Err(Error::Invalid("source owner count"))
    ));
    manifest.units[0].native_owners.clear();
    manifest.units[0].retained_uses = (0..=MAX_SOURCE_OWNERS)
        .map(|index| RetainedUseV1 {
            role: RetainedRoleV1::Decoder,
            owner: format!("reader.{index:03}"),
        })
        .collect();
    assert!(matches!(
        manifest_digest(&manifest),
        Err(Error::Invalid("source owner count"))
    ));
}

#[test]
fn retained_original_refuses_content_replacement_and_namespace_replacement() {
    let (directory, manifest, request) = fixture();
    let checked = check(directory.path(), &manifest, &request).unwrap();
    let path = directory.path().join(&request.paths[0]);
    let mut different = std::fs::read(&path).unwrap();
    different[0] ^= 1;
    std::fs::write(&path, different).unwrap();
    assert!(checked.recheck_preimages().is_err());
    drop(checked);
    let (directory, manifest, request) = fixture();
    let checked = check(directory.path(), &manifest, &request).unwrap();
    let path = directory.path().join(&request.paths[0]);
    let original = std::fs::read(&path).unwrap();
    std::fs::rename(&path, directory.path().join("src/original.rs")).unwrap();
    std::fs::write(&path, original).unwrap();
    assert!(
        checked.recheck_preimages().is_err(),
        "equal bytes do not replace the original file owner"
    );
}

#[test]
fn missing_files_and_nonregular_sources_never_become_empty_preimages() {
    let (directory, manifest, request) = fixture();
    let path = directory.path().join(&request.paths[0]);
    std::fs::remove_file(&path).unwrap();
    assert!(
        matches!(check(directory.path(), &manifest, &request), Err(Error::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound)
    );
    std::fs::create_dir(&path).unwrap();
    assert!(check(directory.path(), &manifest, &request).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_ancestors_leaves_and_shared_file_links_are_refused() {
    use std::os::unix::fs::symlink;
    let (directory, manifest, request) = fixture();
    let path = directory.path().join(&request.paths[0]);
    std::fs::rename(&path, directory.path().join("src/original.rs")).unwrap();
    symlink("original.rs", &path).unwrap();
    assert!(check(directory.path(), &manifest, &request).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::hard_link(directory.path().join("src/original.rs"), &path).unwrap();
    assert!(check(directory.path(), &manifest, &request).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::rename(directory.path().join("src/original.rs"), &path).unwrap();
    std::fs::rename(
        directory.path().join("src"),
        directory.path().join("actual"),
    )
    .unwrap();
    symlink("actual", directory.path().join("src")).unwrap();
    assert!(check(directory.path(), &manifest, &request).is_err());
}
