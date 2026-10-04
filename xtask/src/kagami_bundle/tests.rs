//! Native CLI packaging contract tests; no build or validator is started.

use super::*;

#[test]
fn locked_build_selects_both_native_cli_programs_without_gui_or_feature_changes() {
    for profile in ["debug", "release"] {
        let args = build_args(profile);
        assert_eq!(args[0], "build");
        assert!(args.iter().any(|arg| arg == "--locked"));
        for selected in ["iroha_kagami", "irohad", "kagami", "iroha3d"] {
            assert!(args.iter().any(|arg| arg == selected));
        }
        for forbidden in ["mochi", "mochi-ui", "--features", "--no-default-features"] {
            assert!(!args.iter().any(|arg| arg == forbidden));
        }
        assert_eq!(
            args.iter().any(|arg| arg == "--release"),
            profile == "release"
        );
    }
    assert!(validate_profile("local-release").is_err());
    assert!(validate_profile("../other").is_err());
}

fn source(root: &Path) -> BTreeMap<String, PathBuf> {
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    let mut programs = BTreeMap::new();
    // Unit format fixtures exercise admission only; these bytes never start a process.
    for program in PROGRAMS {
        let mut header = [0_u8; 128];
        match (env::consts::OS, env::consts::ARCH) {
            ("macos", arch) => {
                header[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
                let cpu: u32 = if arch == "aarch64" {
                    0x0100_000c
                } else {
                    0x0100_0007
                };
                header[4..8].copy_from_slice(&cpu.to_le_bytes());
                header[12..16].copy_from_slice(&2_u32.to_le_bytes());
            }
            ("linux", arch) => {
                header[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
                header[16..18].copy_from_slice(&2_u16.to_le_bytes());
                let cpu: u16 = if arch == "aarch64" { 183 } else { 62 };
                header[18..20].copy_from_slice(&cpu.to_le_bytes());
                header[24..32].copy_from_slice(&4096_u64.to_le_bytes());
            }
            ("windows", arch) => {
                header[..2].copy_from_slice(b"MZ");
                header[60..64].copy_from_slice(&64_u32.to_le_bytes());
                header[64..68].copy_from_slice(b"PE\0\0");
                let cpu: u16 = if arch == "aarch64" { 0xaa64 } else { 0x8664 };
                header[68..70].copy_from_slice(&cpu.to_le_bytes());
                header[86..88].copy_from_slice(&2_u16.to_le_bytes());
                header[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
            }
            _ => panic!("unsupported native test host"),
        }
        let path = source.join(format!("{program}{}", env::consts::EXE_SUFFIX));
        fs::write(&path, header).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        programs.insert(program.into(), path);
    }
    programs
}

#[test]
fn two_program_package_has_exact_runtime_profile_location_and_sorted_hash_inventory() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let package = temporary.path().join("native-cli");
    let profiles = InstalledNetworkProfiles::new(Vec::new()).unwrap();
    publish(&source, &package, "debug", Some(&profiles)).unwrap();
    let runtime = KagamiBundleLayout::runtime_directory(&package);
    let installed = InstalledRuntime::from_directory(&runtime).unwrap();
    assert_eq!(installed.network_profiles().unwrap().names().len(), 0);
    assert_eq!(
        KagamiBundleLayout::profiles_path(&package).parent(),
        Some(runtime.as_path())
    );
    assert!(!runtime.join("mochi").exists());
    assert!(!package.join("Mochi.app").exists());
    let manifest: Value =
        json::from_slice(&fs::read(package.join("manifest.json")).unwrap()).unwrap();
    let files = manifest.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 3);
    let mut previous = "";
    for file in files {
        let path = file.get("path").unwrap().as_str().unwrap();
        assert!(path > previous);
        assert_eq!(
            file.get("sha256").unwrap().as_str().unwrap(),
            digest(&mut RetainedFile::open_regular(package.join(path)).unwrap()).unwrap()
        );
        previous = path;
    }
    assert!(publish(&source, &package, "debug", None).is_err());
}

#[test]
fn packaging_refuses_missing_or_indirect_programs_before_creating_the_package() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let daemon = source.get("iroha3d").unwrap();
    fs::remove_file(&daemon).unwrap();
    let package = temporary.path().join("native-cli");
    assert!(publish(&source, &package, "debug", None).is_err());
    assert!(!package.exists());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source.get("kagami").unwrap(), &daemon).unwrap();
        assert!(publish(&source, &package, "debug", None).is_err());
        assert!(!package.exists());
    }
}

#[test]
fn exact_cargo_records_refuse_missing_duplicate_test_or_relative_executables() {
    let record = |name: &str, path: &str, kind: &str| {
        Value::Object(Map::from([
            ("reason".into(), Value::String("compiler-artifact".into())),
            ("executable".into(), Value::String(path.into())),
            (
                "target".into(),
                Value::Object(Map::from([
                    ("name".into(), Value::String(name.into())),
                    (
                        "kind".into(),
                        Value::Array(vec![Value::String(kind.into())]),
                    ),
                ])),
            ),
        ]))
    };
    let read = |records: Vec<Value>| {
        let mut bytes = Vec::new();
        for record in records {
            bytes.extend(json::to_vec(&record).unwrap());
            bytes.push(b'\n');
        }
        collect_programs(io::Cursor::new(bytes))
    };
    let native_path = std::env::temp_dir().join("exact build/native/kagami");
    let daemon_path = std::env::temp_dir().join("other target/native/iroha3d");
    let native = record("kagami", native_path.to_str().unwrap(), "bin");
    let daemon = record("iroha3d", daemon_path.to_str().unwrap(), "bin");
    let programs = read(vec![native.clone(), daemon.clone()]).unwrap();
    assert_eq!(programs.get("kagami").unwrap(), &native_path);
    for records in [
        vec![native.clone()],
        vec![native.clone(), native.clone(), daemon.clone()],
        vec![native.clone(), record("iroha3d", "relative/iroha3d", "bin")],
        vec![
            native,
            record("iroha3d", daemon_path.to_str().unwrap(), "test"),
        ],
    ] {
        assert!(read(records).is_err());
    }
}

#[test]
fn nonexecutables_cross_host_and_multiple_link_inputs_fail_before_visibility() {
    let temporary = tempfile::tempdir().unwrap();
    let programs = source(temporary.path());
    let kagami = programs.get("kagami").unwrap();
    let original = fs::read(kagami).unwrap();
    let package = temporary.path().join("native-cli");
    fs::write(kagami, b"not an executable").unwrap();
    assert!(publish(&programs, &package, "debug", None).is_err());
    assert!(!package.exists());
    fs::write(kagami, &original).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(kagami, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(publish(&programs, &package, "debug", None).is_err());
        assert!(!package.exists());
        fs::set_permissions(kagami, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut cross_host = original.clone();
    cross_host[..4].fill(0);
    fs::write(kagami, cross_host).unwrap();
    assert!(publish(&programs, &package, "debug", None).is_err());
    assert!(!package.exists());
    fs::write(kagami, original).unwrap();
    fs::hard_link(kagami, temporary.path().join("shared-link")).unwrap();
    assert!(publish(&programs, &package, "debug", None).is_err());
    assert!(!package.exists());
}

#[test]
fn complete_staging_and_source_substitution_never_publish_a_partial_installation() {
    let temporary = tempfile::tempdir().unwrap();
    let programs = source(temporary.path());
    let package = temporary.path().join("native-cli");
    assert!(
        publish_checked(&programs, &package, "debug", None, &mut || {
            assert!(!package.exists());
            Err("injected failure before atomic publication".into())
        })
        .is_err()
    );
    assert!(!package.exists());
    assert!(
        publish_checked(&programs, &package, "debug", None, &mut || {
            let kagami = programs.get("kagami").unwrap();
            fs::rename(kagami, temporary.path().join("original-kagami"))?;
            fs::write(kagami, b"substituted source")?;
            Ok(())
        })
        .is_err()
    );
    assert!(!package.exists());
}

#[test]
fn staged_program_profile_manifest_or_inventory_drift_refuses_atomic_publication() {
    for relative in [
        format!("bin/kagami{}", env::consts::EXE_SUFFIX),
        format!("bin/iroha3d{}", env::consts::EXE_SUFFIX),
        format!("bin/{}", iroha_deploy::bootstrap::NETWORK_PROFILES_FILENAME),
        "manifest.json".into(),
        "bin/uninventoried-file".into(),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let programs = source(temporary.path());
        let package = temporary.path().join("native-cli");
        let profiles = InstalledNetworkProfiles::new(Vec::new()).unwrap();
        assert!(
            publish_checked(&programs, &package, "debug", Some(&profiles), &mut || {
                assert!(!package.exists());
                let staging = fs::read_dir(temporary.path())?
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .find(|path| {
                        path.file_name().is_some_and(|name| {
                            name.to_string_lossy().starts_with(".kagami-stage-")
                        })
                    })
                    .ok_or("private package staging is absent")?;
                fs::write(
                    staging.join(&relative),
                    b"changed after completed validation",
                )?;
                Ok(())
            })
            .is_err(),
            "changed {relative} must not publish a stale inventory"
        );
        assert!(!package.exists());
    }
}
