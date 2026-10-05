//! Native file-publication tests for explicit bootstrap; fixture images never execute.

use super::*;
use clap::Parser as _;

#[test]
fn initializer_requires_both_independent_pins_and_has_no_custody_or_image_override() {
    let command = [
        "iroha",
        "taira",
        "public-reset",
        "initialize-native-edge-custody",
        "--expected-executable-sha256",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "--trusted-public-key",
        "/Users/operator/reset-owner-public.json",
        "--expected-trusted-public-key-sha256",
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
    ];
    assert!(crate::Args::try_parse_from(command).is_ok());
    for range in [4..6, 6..8, 8..10] {
        let missing: Vec<_> = command
            .iter()
            .enumerate()
            .filter_map(|(index, value)| (!range.contains(&index)).then_some(*value))
            .collect();
        assert!(crate::Args::try_parse_from(missing).is_err());
    }
    for flag in [
        "--home",
        "--custody-root",
        "--dispatcher",
        "--executable",
        "--signing-key-fd",
    ] {
        let mut changed = command.to_vec();
        changed.extend([flag, "/other"]);
        assert!(crate::Args::try_parse_from(changed).is_err(), "{flag}");
    }
}

#[cfg(unix)]
struct Fixture {
    temporary: tempfile::TempDir,
    account: NativeAccount,
    image: PathBuf,
    args: InitializeNativeEdgeCustody,
}

#[cfg(unix)]
impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let base = temporary.path().canonicalize().unwrap();
        let home = base.join("home");
        PrivateDirectory::open_or_create(&home).unwrap();
        let input = PrivateDirectory::open_or_create(base.join("inputs")).unwrap();
        let account = NativeAccount {
            uid: rustix::process::geteuid().as_raw(),
            gid: rustix::process::getegid().as_raw(),
            user: "isolated-native-fixture".into(),
            home,
        };
        let mut header = [0_u8; 128];
        // Native admission fixtures only; no child process, service or live home is used.
        match (std::env::consts::OS, std::env::consts::ARCH) {
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
            _ => panic!("unsupported native Unix test host"),
        }
        input
            .write_atomic("iroha", &header, PublishMode::CreateNew)
            .unwrap();
        let image = input.path().join("iroha");
        fs::set_permissions(&image, fs::Permissions::from_mode(0o700)).unwrap();
        let pair = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let trusted = TrustedKeyV1 {
            schema: TRUSTED_KEY_SCHEMA_V1.into(),
            algorithm: "ed25519".into(),
            public_key: pair.public_key().to_string(),
        };
        let mut key = json::to_json(&trusted).unwrap().into_bytes();
        key.push(b'\n');
        input
            .write_atomic("owner-public.json", &key, PublishMode::CreateNew)
            .unwrap();
        let args = InitializeNativeEdgeCustody {
            expected_executable_sha256: sha256_hex(&header),
            trusted_public_key: input.path().join("owner-public.json"),
            expected_trusted_public_key_sha256: sha256_hex(&key),
        };
        Self {
            temporary,
            account,
            image,
            args,
        }
    }

    fn initialize(&self) -> Result<InitializationReceiptV1> {
        initialize_bound(
            &self.account,
            &self.image,
            "0123456789abcdef0123456789abcdef0123456789",
            &self.args,
            &mut |_| Ok(()),
        )
    }

    fn root(&self) -> PathBuf {
        self.account.custody()
    }

    fn parent(&self) -> PathBuf {
        self.root().parent().unwrap().into()
    }
}

#[cfg(unix)]
#[test]
fn bootstrap_atomically_publishes_exact_public_authority_and_created_native_inodes() {
    let fixture = Fixture::new();
    let receipt = initialize_bound(
        &fixture.account,
        &fixture.image,
        "0123456789abcdef0123456789abcdef0123456789",
        &fixture.args,
        &mut |stage| {
            assert!(!fixture.root().exists());
            assert_eq!(
                fs::read(stage.join("dispatcher/iroha"))?,
                fs::read(&fixture.image)?
            );
            assert_eq!(
                fs::read(stage.join("taira-edge").join(TRUSTED_NAME))?,
                fs::read(&fixture.args.trusted_public_key)?
            );
            Ok(())
        },
    )
    .unwrap();
    assert!(!receipt.qualified);
    assert_eq!(receipt.outcome, "initialized");
    assert_eq!(receipt.custody_root, fixture.root().to_str().unwrap());
    assert_eq!(
        receipt.dispatcher.sha256,
        fixture.args.expected_executable_sha256
    );
    assert_eq!(
        receipt.trusted_public_key.sha256,
        fixture.args.expected_trusted_public_key_sha256
    );
    for (reference, mode) in [
        (&receipt.dispatcher, 0o755),
        (&receipt.guard, 0o600),
        (&receipt.trusted_public_key, 0o600),
    ] {
        assert_eq!(reference.file.identity.mode, mode);
        assert_eq!(reference.file.identity.links, 1);
        assert_eq!(reference.file.identity.uid, fixture.account.uid);
        assert_eq!(reference.file.identity.gid, fixture.account.gid);
        assert_eq!(
            protocol::native_file_identity(&fs::symlink_metadata(&reference.file.path).unwrap())
                .unwrap(),
            reference.file.identity
        );
        assert_eq!(
            sha256_hex(&fs::read(&reference.file.path).unwrap()),
            reference.sha256
        );
    }
    let guard: HostGuardV1 =
        json::from_slice(&fs::read(&receipt.guard.file.path).unwrap()).unwrap();
    assert_eq!(guard.schema, HOST_GUARD_SCHEMA_V1);
    assert_eq!(guard.host_slug, "taira-edge");
    assert_eq!(guard.dispatcher_path, receipt.dispatcher.file.path);
    assert_eq!(guard.dispatcher_sha256, receipt.dispatcher.sha256);
    assert_eq!(guard.trusted_key_sha256, receipt.trusted_public_key.sha256);
    assert!(!Path::new(&guard.service_root).exists());
    assert!(!Path::new(&guard.state_root).exists());
    assert!(!Path::new(&guard.upload_parent).exists());
    assert_eq!(
        fs::symlink_metadata(fixture.root()).unwrap().mode() & 0o7777,
        0o700
    );
    assert_eq!(
        PrivateDirectory::open(fixture.parent())
            .unwrap()
            .entries(4)
            .unwrap(),
        vec![OsStr::new(ROOT_NAME).to_owned()]
    );
    let wire = json::to_json(&receipt).unwrap();
    assert!(wire.len() < 16 * 1024);
    assert!(!wire.contains("private_key"));
}

#[cfg(unix)]
#[test]
fn dispatcher_copy_refuses_existing_empty_or_nonempty_child_without_append_or_chmod() {
    for body in [b"".as_slice(), b"retained foreign dispatcher".as_slice()] {
        let fixture = Fixture::new();
        let directory = PrivateDirectory::open_or_create(
            fixture
                .temporary
                .path()
                .canonicalize()
                .unwrap()
                .join("fresh-dispatcher"),
        )
        .unwrap();
        directory
            .write_atomic("iroha", body, PublishMode::CreateNew)
            .unwrap();
        let path = directory.path().join("iroha");
        let before = protocol::native_file_identity(&fs::symlink_metadata(&path).unwrap()).unwrap();
        let mut image = PublicPin::open(&fixture.image, MAX_IMAGE, false).unwrap();
        assert!(copy_dispatcher_new(&directory, &fixture.account, &mut image).is_err());
        assert_eq!(fs::read(&path).unwrap(), body);
        assert_eq!(
            protocol::native_file_identity(&fs::symlink_metadata(&path).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            directory.entries(2).unwrap(),
            vec![OsStr::new("iroha").to_owned()]
        );
        assert!(!fixture.root().exists());
    }
}

#[cfg(unix)]
#[test]
fn lost_receipt_replay_returns_read_only_exact_same_custody() {
    let fixture = Fixture::new();
    let first = fixture.initialize().unwrap();
    let root_identity =
        protocol::native_file_identity(&fs::symlink_metadata(fixture.root()).unwrap()).unwrap();
    let parent_identity =
        protocol::native_file_identity(&fs::symlink_metadata(fixture.parent()).unwrap()).unwrap();
    let second = fixture.initialize().unwrap();
    assert_eq!(second.outcome, "already_initialized");
    assert!(!second.qualified);
    assert_eq!(first.dispatcher, second.dispatcher);
    assert_eq!(first.guard, second.guard);
    assert_eq!(first.trusted_public_key, second.trusted_public_key);
    assert_eq!(
        root_identity,
        protocol::native_file_identity(&fs::symlink_metadata(fixture.root()).unwrap()).unwrap()
    );
    assert_eq!(
        parent_identity,
        protocol::native_file_identity(&fs::symlink_metadata(fixture.parent()).unwrap()).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn replay_after_normal_use_inspects_only_anchors_and_leaves_operational_children_untouched() {
    let fixture = Fixture::new();
    let first = fixture.initialize().unwrap();
    let root = PrivateDirectory::open(fixture.root()).unwrap();
    let edge = root.open_child("taira-edge").unwrap();
    let operations = root.create_child("operations").unwrap();
    let operation = operations.create_child("operation-opaque").unwrap();
    let inputs = root.create_child("inputs").unwrap();
    let helpers = edge.create_child("helpers").unwrap();
    let mut preserved = Vec::new();
    for (directory, name, body) in [
        (
            &operation,
            "progress.json",
            b"opaque operation state".as_slice(),
        ),
        (
            &inputs,
            "inventory.json",
            b"uninterpreted input state".as_slice(),
        ),
        (
            &helpers,
            "receiver.py",
            b"uninterpreted helper state".as_slice(),
        ),
        (&edge, "host-operation.lock", b"".as_slice()),
        (
            &root,
            "unknown-child",
            b"uninterpreted unknown child".as_slice(),
        ),
    ] {
        directory
            .write_atomic(name, body, PublishMode::CreateNew)
            .unwrap();
        let path = directory.path().join(name);
        preserved.push((
            path.clone(),
            protocol::native_file_identity(&fs::symlink_metadata(&path).unwrap()).unwrap(),
            body.to_vec(),
        ));
    }
    let directory_snapshots: Vec<_> = [&root, &edge, &operations, &operation, &inputs, &helpers]
        .into_iter()
        .map(|directory| {
            (
                directory.path().to_owned(),
                protocol::native_file_identity(&fs::symlink_metadata(directory.path()).unwrap())
                    .unwrap(),
                directory.entries(16).unwrap(),
            )
        })
        .collect();
    let second = fixture.initialize().unwrap();
    assert_eq!(second.outcome, "already_initialized");
    assert!(!second.qualified);
    assert_eq!(first.dispatcher, second.dispatcher);
    assert_eq!(first.guard, second.guard);
    assert_eq!(first.trusted_public_key, second.trusted_public_key);
    for (path, identity, body) in preserved {
        assert_eq!(fs::read(&path).unwrap(), body);
        assert_eq!(
            protocol::native_file_identity(&fs::symlink_metadata(&path).unwrap()).unwrap(),
            identity
        );
    }
    for (path, identity, names) in directory_snapshots {
        assert_eq!(
            protocol::native_file_identity(&fs::symlink_metadata(&path).unwrap()).unwrap(),
            identity
        );
        assert_eq!(
            PrivateDirectory::open(path).unwrap().entries(16).unwrap(),
            names
        );
    }
}

#[cfg(unix)]
#[test]
fn executable_and_public_key_digest_fail_before_any_custody_creation() {
    for executable in [true, false] {
        let mut fixture = Fixture::new();
        if executable {
            fixture.args.expected_executable_sha256 = "0".repeat(64)
        } else {
            fixture.args.expected_trusted_public_key_sha256 = "0".repeat(64)
        }
        assert!(fixture.initialize().is_err());
        assert!(!fixture.account.home.join(".local").exists());
    }
}

#[cfg(unix)]
#[test]
fn malformed_or_non_ed25519_authority_and_non_native_image_fail_before_creation() {
    for fault in [
        "invalid_json",
        "unknown_field",
        "wrong_schema",
        "wrong_algorithm",
        "wrong_key",
        "non_native",
    ] {
        let mut fixture = Fixture::new();
        if fault == "non_native" {
            fs::write(&fixture.image, [0_u8; 128]).unwrap();
            fixture.args.expected_executable_sha256 = sha256_hex(&[0_u8; 128]);
        } else {
            let body = if fault == "invalid_json" {
                b"closed-secret-marker invalid JSON".to_vec()
            } else {
                let mut value: json::Value =
                    json::from_slice(&fs::read(&fixture.args.trusted_public_key).unwrap()).unwrap();
                let map = value.as_object_mut().unwrap();
                let (field, text) = match fault {
                    "unknown_field" => ("private_key", "closed-secret-marker"),
                    "wrong_schema" => ("schema", "other"),
                    "wrong_algorithm" => ("algorithm", "secp256k1"),
                    _ => ("public_key", "closed-secret-marker"),
                };
                map.insert(field.into(), json::Value::String(text.into()));
                json::to_vec(&value).unwrap()
            };
            fs::write(&fixture.args.trusted_public_key, &body).unwrap();
            fixture.args.expected_trusted_public_key_sha256 = sha256_hex(&body);
        }
        let error = fixture.initialize().unwrap_err();
        assert!(!format!("{error:#}").contains("closed-secret-marker"));
        assert!(!fixture.account.home.join(".local").exists(), "{fault}");
    }
}

#[cfg(unix)]
#[test]
fn occupied_partial_unknown_and_indirect_custody_is_preserved() {
    for fault in ["file", "empty", "unknown", "symlink"] {
        let fixture = Fixture::new();
        let parent = PrivateDirectory::open_or_create(fixture.parent()).unwrap();
        match fault {
            "file" => parent
                .write_atomic(ROOT_NAME, b"foreign-state", PublishMode::CreateNew)
                .unwrap(),
            "symlink" => std::os::unix::fs::symlink(&fixture.image, fixture.root()).unwrap(),
            _ => {
                let root = parent.create_child(ROOT_NAME).unwrap();
                if fault == "unknown" {
                    root.write_atomic("foreign", b"foreign-state", PublishMode::CreateNew)
                        .unwrap();
                }
            }
        }
        let before =
            protocol::native_file_identity(&fs::symlink_metadata(fixture.root()).unwrap()).unwrap();
        assert!(fixture.initialize().is_err(), "{fault}");
        assert_eq!(
            before,
            protocol::native_file_identity(&fs::symlink_metadata(fixture.root()).unwrap()).unwrap()
        );
        assert_eq!(
            parent.entries(4).unwrap(),
            vec![OsStr::new(ROOT_NAME).to_owned()]
        );
        if fault == "file" {
            assert_eq!(fs::read(fixture.root()).unwrap(), b"foreign-state");
        }
        if fault == "unknown" {
            assert_eq!(
                fs::read(fixture.root().join("foreign")).unwrap(),
                b"foreign-state"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn interrupted_unpublished_custody_is_refused_without_retry_or_deletion() {
    let fixture = Fixture::new();
    let parent = PrivateDirectory::open_or_create(fixture.parent()).unwrap();
    let stage = parent
        .create_child(format!("{STAGE_PREFIX}{}", "0".repeat(32)))
        .unwrap();
    stage
        .write_atomic("partial", b"retained-partial", PublishMode::CreateNew)
        .unwrap();
    let before = parent.entries(4).unwrap();
    assert!(fixture.initialize().is_err());
    assert!(!fixture.root().exists());
    assert_eq!(parent.entries(4).unwrap(), before);
    assert_eq!(
        fs::read(stage.path().join("partial")).unwrap(),
        b"retained-partial"
    );
}

#[cfg(unix)]
#[test]
fn same_bytes_source_and_staged_inode_substitutions_refuse_before_root_visibility() {
    for fault in ["image", "dispatcher", "guard", "key", "directory"] {
        let fixture = Fixture::new();
        let result = initialize_bound(
            &fixture.account,
            &fixture.image,
            "0123456789abcdef0123456789abcdef0123456789",
            &fixture.args,
            &mut |stage| {
                let selected = match fault {
                    "image" => fixture.image.clone(),
                    "dispatcher" => stage.join("dispatcher/iroha"),
                    "guard" => stage.join("taira-edge/guard.json"),
                    "key" => stage.join("taira-edge").join(TRUSTED_NAME),
                    _ => stage.join("dispatcher"),
                };
                let displaced = fixture.temporary.path().join("displaced");
                if fault == "directory" {
                    let original = fs::read(selected.join("iroha"))?;
                    fs::rename(&selected, &displaced)?;
                    let recreated = PrivateDirectory::open_or_create(&selected)?;
                    recreated.write_atomic("iroha", &original, PublishMode::CreateNew)?;
                    fs::set_permissions(selected.join("iroha"), fs::Permissions::from_mode(0o755))?;
                } else {
                    let body = fs::read(&selected)?;
                    let mode = fs::symlink_metadata(&selected)?.mode() & 0o7777;
                    fs::rename(&selected, &displaced)?;
                    fs::write(&selected, &body)?;
                    fs::set_permissions(&selected, fs::Permissions::from_mode(mode))?;
                }
                Ok(())
            },
        );
        assert!(result.is_err(), "{fault}");
        assert!(!fixture.root().exists(), "{fault}");
        assert!(fixture.temporary.path().join("displaced").exists());
    }
}

#[cfg(unix)]
#[test]
fn atomic_root_collision_preserves_foreign_custody_and_unpublished_complete_stage() {
    let fixture = Fixture::new();
    let mut foreign_identity = None;
    let result = initialize_bound(
        &fixture.account,
        &fixture.image,
        "0123456789abcdef0123456789abcdef0123456789",
        &fixture.args,
        &mut |_| {
            let root = PrivateDirectory::open_or_create(fixture.root())?;
            root.write_atomic("foreign", b"retained-foreign", PublishMode::CreateNew)?;
            foreign_identity = Some(protocol::native_file_identity(&fs::symlink_metadata(
                fixture.root(),
            )?)?);
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(
        foreign_identity.unwrap(),
        protocol::native_file_identity(&fs::symlink_metadata(fixture.root()).unwrap()).unwrap()
    );
    assert_eq!(
        fs::read(fixture.root().join("foreign")).unwrap(),
        b"retained-foreign"
    );
    assert!(
        PrivateDirectory::open(fixture.parent())
            .unwrap()
            .entries(4)
            .unwrap()
            .iter()
            .any(|name| name.to_string_lossy().starts_with(STAGE_PREFIX))
    );
}

#[cfg(unix)]
#[test]
fn different_or_mutated_initialized_authority_never_rewrites_the_incumbent() {
    for fault in ["guard", "key", "dispatcher"] {
        let fixture = Fixture::new();
        let receipt = fixture.initialize().unwrap();
        let selected: PathBuf = match fault {
            "guard" => receipt.guard.file.path.into(),
            "key" => receipt.trusted_public_key.file.path.into(),
            _ => receipt.dispatcher.file.path.into(),
        };
        fs::write(&selected, b"changed-owned-state").unwrap();
        let before =
            protocol::native_file_identity(&fs::symlink_metadata(&selected).unwrap()).unwrap();
        assert!(fixture.initialize().is_err(), "{fault}");
        assert_eq!(fs::read(&selected).unwrap(), b"changed-owned-state");
        assert_eq!(
            before,
            protocol::native_file_identity(&fs::symlink_metadata(&selected).unwrap()).unwrap()
        );
    }
}

#[cfg(unix)]
#[test]
fn indirect_or_multiple_link_bootstrap_inputs_fail_before_custody_creation() {
    for (image, hardlink) in [(true, true), (true, false), (false, true), (false, false)] {
        let fixture = Fixture::new();
        let selected = if image {
            &fixture.image
        } else {
            &fixture.args.trusted_public_key
        };
        let other = fixture.temporary.path().join("other");
        if hardlink {
            fs::hard_link(selected, &other).unwrap();
        } else {
            fs::rename(selected, &other).unwrap();
            std::os::unix::fs::symlink(&other, selected).unwrap();
        }
        assert!(fixture.initialize().is_err());
        assert!(!fixture.account.home.join(".local").exists());
    }
}
