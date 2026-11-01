//! Native admission controls retained from the former script and `TestNetwork` validators.

use super::*;
use std::{io::Write as _, path::Path};

use iroha_fs::{FileIdentity, FileMetadata, SealedFlatBatch};

use crate::{
    bundle::{MANIFEST_INDEX, inventory, native_identity},
    manifest::{ExpectedManifest, MAX_MANIFEST_BYTES, Manifest},
};

const INVOCATION: &str = "invocation.admission";

struct ClosedPublication {
    fixture: Fixture,
    manifest: Manifest,
    anchor: String,
    path: PathBuf,
    directory: FileIdentity,
    children: [FileIdentity; 5],
}

impl ClosedPublication {
    fn new() -> Self {
        let fixture = Fixture::new();
        let bundle = publish(&fixture.owner, INVOCATION, fixture.inputs()).unwrap();
        let publication = Self {
            manifest: bundle.manifest().clone(),
            anchor: bundle.manifest_sha256().to_owned(),
            path: bundle.path().to_owned(),
            directory: bundle.batch.identity().unwrap(),
            children: std::array::from_fn(|index| bundle.batch.entry_identity(index).unwrap()),
            fixture,
        };
        bundle.revalidate().unwrap();
        drop(bundle);
        publication
    }

    fn expected(&self) -> ExpectedManifest<'_> {
        ExpectedManifest {
            source_sha256: &self.fixture.source,
            cargo_lock_sha256: &self.manifest.cargo_lock_sha256,
            bundle_dir: &self.path,
            native_target: NATIVE_TARGET,
            windows: cfg!(windows),
        }
    }

    fn anchors(&self) -> BundleAnchors<'_> {
        BundleAnchors {
            source: &self.fixture.source,
            cargo_lock: &self.manifest.cargo_lock_sha256,
            manifest: &self.anchor,
        }
    }

    fn open(&self) -> io::Result<Arc<Bundle>> {
        let parent = ReaderDirectory::open(self.fixture.owner.path())?;
        Bundle::open(&parent, INVOCATION, self.anchors())
    }

    fn lower(&self) -> io::Result<SealedFlatBatch<5>> {
        ReaderDirectory::open(self.fixture.owner.path())?
            .open_sealed_flat_batch(INVOCATION, inventory()?)
    }

    fn require_originals(&self, lower: &SealedFlatBatch<5>) {
        assert_eq!(lower.identity().unwrap(), self.directory);
        for (index, original) in self.children.iter().enumerate() {
            assert_eq!(&lower.entry_identity(index).unwrap(), original);
        }
        lower.revalidate().unwrap();
    }

    fn rewrite_manifest(&mut self, bytes: &[u8]) {
        rewrite_original(&self.path.join(MANIFEST_NAME), bytes, "R");
        self.anchor = digest(bytes);
        assert_eq!(
            FileMetadata::symlink_metadata(self.path.join(MANIFEST_NAME))
                .unwrap()
                .identity(),
            self.children[MANIFEST_INDEX]
        );
    }

    fn reject_content_with_originals_admitted(&self) {
        let lower = self
            .lower()
            .expect("native policies and all original IDs remain valid");
        self.require_originals(&lower);
        let error = self.open().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "release executable differs from its manifest"
        );
        lower.revalidate().unwrap();
    }

    fn displace_leaf(&self, name: &str) -> PathBuf {
        set_access(&self.path, "F");
        let displaced = self.fixture.owner.path().join("displaced-original");
        fs::rename(self.path.join(name), &displaced).unwrap();
        displaced
    }
}

// These permissions changes are adversarial setup on disposable, closed fixtures.
// They are never admission repairs. Recovered policy is independently checked by
// the actual lower owner before any intended content-authentication refusal.
#[cfg(unix)]
fn set_access(path: &Path, access: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = match access {
        "F" => 0o700,
        "RX" => 0o500,
        "R" => 0o400,
        _ => panic!("unknown fixture access"),
    };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(windows)]
fn set_access(path: &Path, access: &str) {
    use std::{process::Command, sync::OnceLock};
    static SID: OnceLock<String> = OnceLock::new();
    // These tests do not impersonate. The fixture and its child command use the
    // same process owner; effective-token admission is tested by iroha_fs.
    let sid = SID.get_or_init(|| {
        let result = Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        assert!(result.status.success());
        let text = String::from_utf8(result.stdout).unwrap();
        let sid = text.rsplit(',').next().unwrap().trim().trim_matches('"');
        assert!(
            sid.starts_with("S-1-")
                && sid
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
        );
        sid.to_owned()
    });
    let output = Command::new("icacls")
        .arg(path)
        .arg("/grant:r")
        .arg(format!("*{sid}:{access}"))
        .arg("/Q")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rewrite_original(path: &Path, bytes: &[u8], final_access: &str) {
    let identity = FileMetadata::symlink_metadata(path).unwrap().identity();
    set_access(path, "F");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    assert_eq!(FileMetadata::of(&file).unwrap().identity(), identity);
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    set_access(path, final_access);
    assert_eq!(
        FileMetadata::symlink_metadata(path).unwrap().identity(),
        identity
    );
}

#[cfg(unix)]
fn symlink_file(source: &Path, target: &Path) {
    std::os::unix::fs::symlink(source, target).expect("native symlink fixture must be available");
}

#[cfg(windows)]
fn symlink_file(source: &Path, target: &Path) {
    std::os::windows::fs::symlink_file(source, target).expect(
        "native symlink fixture requires Windows symbolic-link privilege; no skipped assertion",
    );
}

#[cfg(unix)]
fn symlink_directory(source: &Path, target: &Path) {
    std::os::unix::fs::symlink(source, target).expect("native directory symlink must be available");
}

#[cfg(windows)]
fn symlink_directory(source: &Path, target: &Path) {
    std::os::windows::fs::symlink_dir(source, target).expect(
        "native directory symlink requires Windows symbolic-link privilege; no skipped assertion",
    );
}

fn reordered_fields(bytes: &[u8]) -> Vec<u8> {
    assert_eq!(bytes[0], b'{');
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    let mut commas = Vec::new();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                b',' if depth == 1 => {
                    commas.push(index);
                    if commas.len() == 2 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    let [first, second] = commas.as_slice() else {
        panic!("canonical object needs two fields")
    };
    [
        b"{".as_slice(),
        &bytes[first + 1..*second],
        b",",
        &bytes[1..*first],
        &bytes[*second..],
    ]
    .concat()
}

#[test]
fn canonical_parser_refuses_control_bytes_field_reordering_and_alternate_numbers() {
    let closed = ClosedPublication::new();
    let bytes = closed.manifest.encode(closed.expected()).unwrap();
    assert_eq!(
        Manifest::decode(&bytes, closed.expected()).unwrap(),
        closed.manifest
    );
    let reordered = reordered_fields(&bytes);
    let decoded: Manifest = norito::json::from_slice(&reordered).unwrap();
    assert_eq!(
        decoded, closed.manifest,
        "reordering must change spelling only"
    );
    let mut nul = bytes.clone();
    nul.insert(1, 0);
    let mut non_utf8 = bytes.clone();
    non_utf8.insert(1, 0xff);
    let crlf = [&bytes[..bytes.len() - 1], b"\r\n"].concat();
    let needle = format!("\"size_bytes\":{}", closed.manifest.binaries[0].size_bytes);
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains(&needle));
    let mut cases = vec![
        ("crlf", crlf),
        ("nul", nul),
        ("invalid UTF8", non_utf8),
        ("field order", reordered),
    ];
    let required_field = "\"profile\":\"release\",";
    assert!(text.contains(required_field));
    cases.push((
        "missing required field",
        text.replacen(required_field, "", 1).into_bytes(),
    ));
    for spelling in [
        format!("0{}", closed.manifest.binaries[0].size_bytes),
        format!("{}.0", closed.manifest.binaries[0].size_bytes),
        format!("{}e0", closed.manifest.binaries[0].size_bytes),
        format!("+{}", closed.manifest.binaries[0].size_bytes),
        format!("\"{}\"", closed.manifest.binaries[0].size_bytes),
    ] {
        cases.push((
            "noncanonical number",
            text.replacen(&needle, &format!("\"size_bytes\":{spelling}"), 1)
                .into_bytes(),
        ));
    }
    for (name, changed) in cases {
        assert_ne!(bytes, changed);
        assert!(
            Manifest::decode(&changed, closed.expected()).is_err(),
            "{name}"
        );
    }
    // The actual manifest and bundle remain valid after pure decoder controls.
    closed.open().unwrap().revalidate().unwrap();
}

#[test]
fn reanchored_canonical_wrong_digest_and_size_reach_independent_content_authentication() {
    for index in 0..4 {
        for wrong_size in [false, true] {
            let mut closed = ClosedPublication::new();
            let mut forged = closed.manifest.clone();
            if wrong_size {
                forged.binaries[index].size_bytes += 1;
            } else {
                forged.binaries[index].sha256 = "0".repeat(64);
                assert_ne!(
                    forged.binaries[index].sha256,
                    closed.manifest.binaries[index].sha256
                );
            }
            let encoded = forged.encode(closed.expected()).unwrap();
            assert_eq!(
                Manifest::decode(&encoded, closed.expected()).unwrap(),
                forged
            );
            closed.rewrite_manifest(&encoded);
            closed.reject_content_with_originals_admitted();
        }
    }
}

#[test]
fn closed_same_original_executable_byte_mutations_are_rejected_after_exact_resealing() {
    for kind in BinaryKind::ALL {
        let closed = ClosedPublication::new();
        let path = closed.path.join(kind.filename(cfg!(windows)));
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        rewrite_original(&path, &bytes, "RX");
        assert_eq!(
            bytes.len() as u64,
            closed.manifest.binaries[kind.index()].size_bytes
        );
        closed.reject_content_with_originals_admitted();
    }
}

#[test]
fn recovery_rejects_extra_files_directories_missing_programs_and_directory_substitutes() {
    for mutation in 0..8 {
        let closed = ClosedPublication::new();
        set_access(&closed.path, "F");
        if mutation < 2 {
            let owner = OwnerDirectory::open(&closed.path).unwrap();
            if mutation == 0 {
                let mut extra = owner.create_retained_private("unexpected", 8).unwrap();
                extra.write_all(b"extra").unwrap();
                drop(extra.seal_read_only().unwrap());
            } else {
                drop(owner.create_child("unexpected").unwrap());
            }
        } else {
            let name = if mutation == 6 {
                MANIFEST_NAME
            } else {
                BinaryKind::ALL[(mutation - 2).min(3)].filename(cfg!(windows))
            };
            closed.displace_leaf(name);
            if mutation == 7 {
                drop(
                    OwnerDirectory::open(&closed.path)
                        .unwrap()
                        .create_child(name)
                        .unwrap(),
                );
            }
        }
        set_access(&closed.path, "RX");
        assert!(closed.lower().is_err(), "inventory mutation {mutation}");
        assert!(closed.open().is_err(), "inventory mutation {mutation}");
    }
}

#[test]
fn recovery_rejects_symlinked_expected_executable() {
    for kind in [BinaryKind::Irohad, BinaryKind::IrohadTaira] {
        let closed = ClosedPublication::new();
        let name = kind.filename(cfg!(windows));
        let displaced = closed.displace_leaf(name);
        symlink_file(&displaced, &closed.path.join(name));
        set_access(&closed.path, "RX");
        assert!(closed.lower().is_err());
        assert!(closed.open().is_err());
    }
}

#[test]
fn recovery_rejects_symlinked_invocation_directory() {
    let closed = ClosedPublication::new();
    let displaced = closed.fixture.owner.path().join("displaced-directory");
    fs::rename(&closed.path, &displaced).unwrap();
    symlink_directory(&displaced, &closed.path);
    assert!(closed.lower().is_err());
    assert!(closed.open().is_err());
}

#[test]
fn recovery_rejects_shared_original_identity_with_alias_outside_inventory() {
    for kind in [BinaryKind::Irohad, BinaryKind::IrohadTaira] {
        let closed = ClosedPublication::new();
        let path = closed.path.join(kind.filename(cfg!(windows)));
        let alias = closed.fixture.owner.path().join("outside-alias");
        set_access(&path, "F");
        fs::hard_link(&path, &alias).unwrap();
        set_access(&path, "RX");
        let actual = FileMetadata::symlink_metadata(&path).unwrap();
        assert_eq!(actual.links(), 2);
        assert_eq!(actual.identity(), closed.children[kind.index()]);
        assert_eq!(
            FileMetadata::symlink_metadata(&alias).unwrap().identity(),
            actual.identity()
        );
        assert_eq!(fs::read_dir(&closed.path).unwrap().count(), 5);
        assert!(closed.lower().is_err());
        assert!(closed.open().is_err());
    }
}

#[test]
fn recovery_rejects_writable_parent_each_program_and_manifest() {
    for index in 0..6 {
        let closed = ClosedPublication::new();
        let path = if index == 0 {
            closed.path.clone()
        } else if index == 5 {
            closed.path.join(MANIFEST_NAME)
        } else {
            closed
                .path
                .join(BinaryKind::ALL[index - 1].filename(cfg!(windows)))
        };
        set_access(&path, "F");
        assert!(closed.lower().is_err(), "writable slot {index}");
        assert!(closed.open().is_err(), "writable slot {index}");
    }
}

#[test]
fn both_tool_transcripts_reject_empty_oversized_and_noncanonical_original_bytes() {
    for rustc in [false, true] {
        for mutation in 0..6 {
            let fixture = Fixture::new();
            let path = if rustc {
                &fixture.rustc
            } else {
                &fixture.cargo
            };
            let original = fs::read(path).unwrap();
            let changed = match mutation {
                0 => Vec::new(),
                1 => [original.as_slice(), &vec![b'x'; 64 * 1024], b"\n"].concat(),
                2 => original[..original.len() - 1].to_vec(),
                3 => [&original[..original.len() - 1], b"\r\n"].concat(),
                4 => [original.as_slice(), b"\0\n"].concat(),
                _ => [original.as_slice(), b"\xff\n"].concat(),
            };
            fs::write(path, changed).unwrap();
            assert!(
                publish(&fixture.owner, INVOCATION, fixture.inputs()).is_err(),
                "rustc={rustc} mutation={mutation}"
            );
            assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
        }
    }
}

#[test]
fn both_tool_transcripts_reject_multiple_links_before_destination_creation() {
    for rustc in [false, true] {
        let fixture = Fixture::new();
        let path = if rustc {
            &fixture.rustc
        } else {
            &fixture.cargo
        };
        fs::hard_link(path, fixture.root.path().join("tool-alias")).unwrap();
        assert_eq!(FileMetadata::symlink_metadata(path).unwrap().links(), 2);
        assert!(publish(&fixture.owner, INVOCATION, fixture.inputs()).is_err());
        assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
    }
}

#[test]
fn both_tool_transcripts_reject_foreign_read_access_before_destination_creation() {
    for rustc in [false, true] {
        let fixture = Fixture::new();
        let path = if rustc {
            &fixture.rustc
        } else {
            &fixture.cargo
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        #[cfg(windows)]
        {
            let output = std::process::Command::new("icacls")
                .arg(path)
                .args(["/grant", "*S-1-1-0:R", "/Q"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(publish(&fixture.owner, INVOCATION, fixture.inputs()).is_err());
        assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
    }
}

#[test]
fn both_tool_transcripts_reject_symlinks_before_destination_creation() {
    for rustc in [false, true] {
        let fixture = Fixture::new();
        let path = if rustc {
            &fixture.rustc
        } else {
            &fixture.cargo
        };
        let displaced = fixture.root.path().join("original-tool-transcript");
        fs::rename(path, &displaced).unwrap();
        symlink_file(&displaced, path);
        assert!(publish(&fixture.owner, INVOCATION, fixture.inputs()).is_err());
        assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
    }
}

#[test]
fn build_output_symlinks_are_refused_before_destination_creation() {
    for kind in BinaryKind::ALL {
        let fixture = Fixture::new();
        let path = fixture.build.join(kind.filename(cfg!(windows)));
        let displaced = fixture.build.join("original-build-output");
        fs::rename(&path, &displaced).unwrap();
        symlink_file(&displaced, &path);
        assert!(publish(&fixture.owner, INVOCATION, fixture.inputs()).is_err());
        assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
    }
}

#[test]
fn reanchored_cross_invocation_manifest_replay_keeps_second_originals_but_fails_binding() {
    let first = ClosedPublication::new();
    let mut second = ClosedPublication::new();
    let bytes = first.manifest.encode(first.expected()).unwrap();
    assert_ne!(first.path, second.path);
    assert_ne!(first.directory, second.directory);
    second.rewrite_manifest(&bytes);
    let lower = second.lower().unwrap();
    second.require_originals(&lower);
    assert_ne!(
        native_identity(lower.entry_identity(MANIFEST_INDEX).unwrap()),
        first.manifest.file_identity
    );
    let error = second.open().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(error.to_string(), "release invocation directory differs");
    first.open().unwrap().revalidate().unwrap();
    lower.revalidate().unwrap();
}

#[test]
fn reanchored_malformed_manifest_bytes_do_not_bypass_the_canonical_parser() {
    for mutation in 0..3 {
        let mut closed = ClosedPublication::new();
        let mut bytes = closed.manifest.encode(closed.expected()).unwrap();
        match mutation {
            0 => bytes.insert(bytes.len() - 1, b'\r'),
            1 => bytes.insert(1, 0),
            _ => bytes = reordered_fields(&bytes),
        }
        assert!(bytes.len() < MAX_MANIFEST_BYTES);
        closed.rewrite_manifest(&bytes);
        let lower = closed.lower().unwrap();
        closed.require_originals(&lower);
        let error = closed.open().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("anchor"));
        lower.revalidate().unwrap();
    }
}
