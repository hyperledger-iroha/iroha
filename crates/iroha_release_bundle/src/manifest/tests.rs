//! Exact manifest inventory, source binding and canonical encoding controls.

use super::*;

fn fixture(windows: bool) -> Manifest {
    let (directory, executable, manifest) = custody_policies(windows);
    Manifest {
        schema: SCHEMA.into(),
        source_manifest_sha256: "a".repeat(64),
        cargo_lock_sha256: "b".repeat(64),
        cargo_version_sha256: "c".repeat(64),
        rustc_version_sha256: "d".repeat(64),
        host_triple: "x86_64-pc-windows-msvc".into(),
        target_triple: "x86_64-pc-windows-msvc".into(),
        profile: "release".into(),
        bundle_dir: std::env::temp_dir()
            .join("invocation.fixture")
            .to_str()
            .unwrap()
            .into(),
        directory_custody: directory.into(),
        file_custody: manifest.into(),
        directory_identity: NativeIdentity {
            platform: if windows { 2 } else { 1 },
            volume: 7,
            object: "0".repeat(32),
        },
        file_identity: NativeIdentity {
            platform: if windows { 2 } else { 1 },
            volume: 7,
            object: format!("{:032x}", 1),
        },
        binaries: BinaryKind::ALL
            .into_iter()
            .map(|kind| BinaryAttestation {
                kind,
                filename: kind.filename(windows).into(),
                sha256: "e".repeat(64),
                size_bytes: 37,
                custody: executable.into(),
                identity: NativeIdentity {
                    platform: if windows { 2 } else { 1 },
                    volume: 7,
                    object: format!("{:032x}", kind.index() + 2),
                },
            })
            .collect(),
    }
}

fn expected(manifest: &Manifest, windows: bool) -> ExpectedManifest<'_> {
    ExpectedManifest {
        source_sha256: &manifest.source_manifest_sha256,
        cargo_lock_sha256: &manifest.cargo_lock_sha256,
        bundle_dir: Path::new(&manifest.bundle_dir),
        native_target: &manifest.target_triple,
        windows,
    }
}

#[test]
fn native_inventory_has_four_distinct_exact_basenames() {
    for windows in [false, true] {
        let names = BinaryKind::ALL.map(|kind| kind.filename(windows));
        for (index, kind) in BinaryKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(), index);
            assert_eq!(
                Path::new(names[index]).extension() == Some(std::ffi::OsStr::new("exe")),
                windows
            );
            assert!(!names[..index].contains(&names[index]));
            assert_eq!(Path::new(names[index]).components().count(), 1);
        }
        assert!(names.iter().all(|name| *name != MANIFEST_NAME));
    }
}

#[test]
fn canonical_manifest_roundtrips_both_native_custody_inventories() {
    for windows in [false, true] {
        let manifest = fixture(windows);
        let expected = expected(&manifest, windows);
        let bytes = manifest.encode(expected).unwrap();
        assert_eq!(Manifest::decode(&bytes, expected).unwrap(), manifest);
        assert_eq!(bytes.last(), Some(&b'\n'));
        let other = ExpectedManifest {
            windows: !windows,
            ..expected
        };
        assert!(Manifest::decode(&bytes, other).is_err());
    }
}

#[test]
fn every_identity_and_inventory_mutation_is_refused() {
    let manifest = fixture(true);
    let expected = expected(&manifest, true);
    for change in 0..26 {
        let mut changed = manifest.clone();
        match change {
            0 => changed.schema = "2".into(),
            1 => changed.source_manifest_sha256 = "f".repeat(64),
            2 => changed.cargo_lock_sha256 = "f".repeat(64),
            3 => changed.cargo_version_sha256 = "C".repeat(64),
            4 => changed.rustc_version_sha256.push('0'),
            5 => changed.host_triple = "x86_64-unknown-linux-gnu".into(),
            6 => changed.target_triple = "x86_64-unknown-linux-gnu".into(),
            7 => changed.profile = "debug".into(),
            8 => changed.bundle_dir.push('x'),
            9 => changed.directory_custody = UNIX_DIRECTORY.into(),
            10 => changed.file_custody = WINDOWS_EXECUTABLE.into(),
            11 => changed.binaries.swap(0, 1),
            12 => changed.binaries[3].kind = BinaryKind::Irohad,
            13 => changed.binaries[3].filename = "iroha3d_taira".into(),
            14 => changed.binaries[0].filename = "release/iroha3d.exe".into(),
            15 => changed.binaries[0].sha256 = "not a digest".into(),
            16 => changed.binaries[0].size_bytes = 0,
            17 => changed.binaries[0].size_bytes = MAX_BINARY_BYTES + 1,
            18 => {
                changed.binaries.pop();
            }
            19 => changed.binaries.push(changed.binaries[0].clone()),
            20 => changed.directory_identity.platform = 1,
            21 => changed.file_identity.object = "A".repeat(32),
            22 => changed.binaries[0].identity.volume = 8,
            23 => changed.binaries[1].identity = changed.binaries[0].identity.clone(),
            24 => changed.file_identity = changed.directory_identity.clone(),
            25 => changed.file_identity.object.push('0'),
            _ => unreachable!(),
        }
        assert!(changed.validate(expected).is_err(), "mutation {change}");
    }
    let mut changed = manifest.clone();
    changed.binaries[0].custody = WINDOWS_MANIFEST.into();
    assert!(changed.validate(expected).is_err());
}

#[test]
fn parser_refuses_extra_fields_retired_tsv_whitespace_and_trailing_bytes() {
    let manifest = fixture(true);
    let expected = expected(&manifest, true);
    let bytes = manifest.encode(expected).unwrap();
    let mut unknown = bytes.clone();
    unknown.splice(1..1, b"\"unknown\":true,".iter().copied());
    let mut duplicate = bytes.clone();
    duplicate.splice(1..1, format!("\"schema\":\"{SCHEMA}\",").bytes());
    for changed in [
        Vec::new(),
        b"schema_version\t2\n".to_vec(),
        [b" ".as_slice(), &bytes].concat(),
        [bytes.as_slice(), b"\n"].concat(),
        [bytes.as_slice(), b"{}\n"].concat(),
        bytes[..bytes.len() - 1].to_vec(),
        unknown,
        duplicate,
        vec![b' '; MAX_MANIFEST_BYTES + 1],
    ] {
        assert!(Manifest::decode(&changed, expected).is_err());
    }
}

#[test]
fn independent_anchor_and_target_inputs_must_be_canonical() {
    assert!(is_digest(&"0".repeat(64)));
    for value in ["", "abc", &"A".repeat(64), &"g".repeat(64)] {
        assert!(!is_digest(value));
    }
    let manifest = fixture(false);
    let valid = expected(&manifest, false);
    for target in ["", "bad target", "target\n", &"a".repeat(129)] {
        assert!(!is_target(target));
        assert!(
            manifest
                .validate(ExpectedManifest {
                    native_target: target,
                    ..valid
                })
                .is_err()
        );
    }
    assert!(is_target("aarch64-apple-darwin"));
    assert!(
        manifest
            .validate(ExpectedManifest {
                source_sha256: "a",
                ..valid
            })
            .is_err()
    );
    assert!(
        manifest
            .validate(ExpectedManifest {
                bundle_dir: Path::new("relative"),
                ..valid
            })
            .is_err()
    );
}
