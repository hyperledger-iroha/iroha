//! Signed, byte-exact admission of the ten bundled CUDA PTX families.

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{fs, io::Read as _, path::Path};

const MANIFEST_NAME: &str = "provenance.v1";
const MANIFEST_SIGNATURE_NAME: &str = "provenance.v1.sig";
const MANIFEST_PUBLIC_KEY_NAME: &str = "provenance.v1.pub";
const MANIFEST_HEADER: &str = "ivm-cuda-ptx-provenance-v1";
const GENERATION_DOMAIN: &[u8] = b"ivm-cuda-ptx-generation-v1\0";
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_PTX_BYTES: usize = 8 * 1024 * 1024;

/// Bundle bytes that passed signed provenance and exact source/PTX hashing.
#[derive(Debug)]
pub(super) struct VerifiedCudaBundle {
    /// PTX bytes in the pinned family order, kept live through installation.
    pub artifacts: Vec<Vec<u8>>,
    /// Signed CUDA toolkit-image digest for independent release-runner comparison.
    pub cuda_image_sha256: String,
    /// Exact signed `nvcc --version` output digest.
    pub nvcc_version_sha256: String,
    /// Exact signed compiler flags, excluding source and output paths.
    pub nvcc_flags: String,
    /// Signed GPU code-generation target.
    pub target_profile: String,
}

fn same_regular_inode(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    if !before.file_type().is_file()
        || !after.file_type().is_file()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "linux",
    target_os = "android"
))]
fn no_follow_nonblocking_flags() -> i32 {
    // Match the platform file-admission flags in state_overlay_fs. This module
    // also belongs to build.rs, so it cannot depend on the runtime overlay owner.
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    {
        0x100 | 0x4
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let no_follow = if cfg!(any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )) {
            0x8000
        } else {
            0x0002_0000
        };
        let nonblocking = if cfg!(any(target_arch = "mips", target_arch = "mips64")) {
            0x80
        } else if cfg!(any(target_arch = "sparc", target_arch = "sparc64")) {
            0x4000
        } else {
            0x800
        };
        no_follow | nonblocking
    }
}

#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "linux",
        target_os = "android"
    ))
))]
fn open_regular_file(_path: &Path) -> std::io::Result<fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "CUDA provenance requires no-follow nonblocking file admission",
    ))
}

#[cfg(any(
    not(unix),
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "linux",
    target_os = "android"
))]
fn open_regular_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(no_follow_nonblocking_flags());
    }
    options.open(path)
}

fn read_regular_file(path: &Path, maximum: Option<usize>) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    if maximum.is_some_and(|maximum| metadata.len() > maximum as u64) {
        return Err(format!("{} exceeds its byte limit", path.display()));
    }
    let mut file = open_regular_file(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !same_regular_inode(&metadata, &opened) {
        return Err(format!("{} changed before read", path.display()));
    }
    let limit = opened
        .len()
        .checked_add(1)
        .ok_or_else(|| format!("{} has an invalid inode size", path.display()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if maximum.is_some_and(|maximum| bytes.len() > maximum) {
        return Err(format!("{} grew beyond its byte limit", path.display()));
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot recheck {}: {error}", path.display()))?;
    let after_path = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot recheck {}: {error}", path.display()))?;
    if bytes.len() as u64 != opened.len()
        || !same_regular_inode(&opened, &after)
        || !same_regular_inode(&opened, &after_path)
    {
        return Err(format!("{} changed while read", path.display()));
    }
    Ok(bytes)
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn canonical_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value.starts_with(' ')
        && !value.ends_with(' ')
        && value
            .bytes()
            .all(|byte| byte == b' ' || byte.is_ascii_graphic())
}

fn field<'a>(lines: &mut impl Iterator<Item = &'a str>, name: &str) -> Result<&'a str, String> {
    let line = lines
        .next()
        .ok_or_else(|| format!("CUDA provenance missing {name}"))?;
    line.strip_prefix(name)
        .and_then(|value| value.strip_prefix('='))
        .ok_or_else(|| format!("CUDA provenance expected {name} in fixed order"))
}

fn digest_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<&'a str, String> {
    let value = field(lines, name)?;
    if !is_lower_sha256(value) {
        return Err(format!(
            "CUDA provenance {name} must be a nonzero lowercase SHA-256"
        ));
    }
    Ok(value)
}

fn signed_manifest(cuda_dir: &Path, trusted_key_sha256: &str) -> Result<Vec<u8>, String> {
    if !is_lower_sha256(trusted_key_sha256) {
        return Err(
            "CUDA trusted public-key fingerprint must be a reviewed lowercase SHA-256".into(),
        );
    }
    let manifest = read_regular_file(&cuda_dir.join(MANIFEST_NAME), Some(MAX_MANIFEST_BYTES))?;
    let public_key = read_regular_file(&cuda_dir.join(MANIFEST_PUBLIC_KEY_NAME), Some(32))?;
    let signature = read_regular_file(&cuda_dir.join(MANIFEST_SIGNATURE_NAME), Some(64))?;
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| "CUDA provenance public key must be 32 raw bytes")?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| "CUDA provenance signature must be 64 raw bytes")?;
    if sha256_hex(&public_key) != trusted_key_sha256 {
        return Err("CUDA provenance public key differs from the reviewed fingerprint".into());
    }
    let verifier = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| "CUDA provenance public key is not canonical Ed25519")?;
    verifier
        .verify_strict(&manifest, &Signature::from_bytes(&signature))
        .map_err(|_| "CUDA provenance signature does not verify")?;
    Ok(manifest)
}

fn generation_sha256(stems: &[&str], artifacts: &[Vec<u8>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(GENERATION_DOMAIN);
    for (stem, bytes) in stems.iter().zip(artifacts) {
        hasher.update((stem.len() as u16).to_le_bytes());
        hasher.update(stem.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

/// Verify the signed manifest and all ten source/PTX byte pairs before installation.
///
/// The trusted public-key fingerprint is an explicit reviewed build input, not
/// a value supplied by the artifact being admitted. The manifest's two
/// generation digests attest independent clean runs; the release runner must
/// separately verify those runs against its pinned toolkit image.
pub(super) fn verify_bundle(
    cuda_dir: &Path,
    stems: &[&str],
    trusted_key_sha256: &str,
) -> Result<VerifiedCudaBundle, String> {
    let manifest = signed_manifest(cuda_dir, trusted_key_sha256)?;
    if !manifest.ends_with(b"\n") || manifest.contains(&b'\r') || manifest.contains(&b'\0') {
        return Err("CUDA provenance must be canonical LF text with a final LF".into());
    }
    let text = std::str::from_utf8(&manifest).map_err(|_| "CUDA provenance must be UTF-8")?;
    let mut lines = text.split_terminator('\n');
    if lines.next() != Some(MANIFEST_HEADER) {
        return Err("CUDA provenance has an unknown V1 header".into());
    }
    let cuda_image_sha256 = digest_field(&mut lines, "cuda_image_sha256")?.to_owned();
    let nvcc_version_sha256 = digest_field(&mut lines, "nvcc_version_sha256")?.to_owned();
    let nvcc_flags = field(&mut lines, "nvcc_flags")?;
    if !canonical_text(nvcc_flags, 512) || !nvcc_flags.starts_with("-ptx -std=c++14 ") {
        return Err("CUDA provenance has invalid exact nvcc flags".into());
    }
    let target_profile = field(&mut lines, "target_profile")?;
    if !canonical_text(target_profile, 128)
        || !target_profile.starts_with("arch=compute_")
        || !target_profile.contains(",code=sm_")
        || !nvcc_flags.contains(target_profile)
    {
        return Err("CUDA provenance has invalid or unbound target profile".into());
    }
    let first_generation = digest_field(&mut lines, "generation_1_sha256")?;
    let second_generation = digest_field(&mut lines, "generation_2_sha256")?;
    let mut artifacts = Vec::with_capacity(stems.len());
    for stem in stems {
        let source_digest = digest_field(&mut lines, &format!("artifact.{stem}.source_sha256"))?;
        let ptx_digest = digest_field(&mut lines, &format!("artifact.{stem}.ptx_sha256"))?;
        let source = read_regular_file(&cuda_dir.join(format!("{stem}.cu")), None)?;
        let ptx = read_regular_file(&cuda_dir.join(format!("{stem}.ptx")), Some(MAX_PTX_BYTES))?;
        if sha256_hex(&source) != source_digest || sha256_hex(&ptx) != ptx_digest {
            return Err(format!(
                "CUDA provenance source/PTX digest mismatch for {stem}"
            ));
        }
        artifacts.push(ptx);
    }
    if lines.next().is_some() {
        return Err("CUDA provenance has an unexpected trailing field".into());
    }
    let actual_generation = generation_sha256(stems, &artifacts);
    if first_generation != second_generation || first_generation != actual_generation {
        return Err("CUDA provenance two-run generation digests differ from bundled PTX".into());
    }
    Ok(VerifiedCudaBundle {
        artifacts,
        cuda_image_sha256,
        nvcc_version_sha256,
        nvcc_flags: nvcc_flags.to_owned(),
        target_profile: target_profile.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::{
        fmt::Write as _,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    const STEMS: [&str; 2] = ["aes", "vector"];
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        directory: PathBuf,
        trusted_fingerprint: String,
        signing_key: SigningKey,
        manifest: String,
        artifacts: Vec<Vec<u8>>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "ivm-cuda-provenance-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).expect("unique private fixture directory");
            let signing_key = SigningKey::from_bytes(&[0x42; 32]);
            let public_key = signing_key.verifying_key().to_bytes();
            let trusted_fingerprint = sha256_hex(&public_key);
            let artifacts = STEMS
                .iter()
                .map(|stem| {
                    format!(
                        ".version 7.8\n.target sm_86\n.address_size 64\n.visible .entry {stem}() {{ ret; }}\n"
                    )
                    .into_bytes()
                })
                .collect::<Vec<_>>();
            let generation = generation_sha256(&STEMS, &artifacts);
            let mut manifest = format!(
                "{MANIFEST_HEADER}\n\
                 cuda_image_sha256={}\n\
                 nvcc_version_sha256={}\n\
                 nvcc_flags=-ptx -std=c++14 -gencode arch=compute_86,code=sm_86\n\
                 target_profile=arch=compute_86,code=sm_86\n\
                 generation_1_sha256={generation}\n\
                 generation_2_sha256={generation}\n",
                sha256_hex(b"pinned CUDA image"),
                sha256_hex(b"nvcc --version output"),
            );
            for (stem, ptx) in STEMS.iter().zip(&artifacts) {
                let source = format!("// source for {stem}\n");
                fs::write(directory.join(format!("{stem}.cu")), &source).expect("source fixture");
                fs::write(directory.join(format!("{stem}.ptx")), ptx).expect("PTX fixture");
                writeln!(
                    &mut manifest,
                    "artifact.{stem}.source_sha256={}",
                    sha256_hex(source.as_bytes())
                )
                .expect("manifest string");
                writeln!(
                    &mut manifest,
                    "artifact.{stem}.ptx_sha256={}",
                    sha256_hex(ptx)
                )
                .expect("manifest string");
            }
            let mut fixture = Self {
                directory,
                trusted_fingerprint,
                signing_key,
                manifest,
                artifacts,
            };
            fixture.resign();
            fixture
        }
        fn resign(&mut self) {
            fs::write(self.directory.join(MANIFEST_NAME), &self.manifest)
                .expect("manifest fixture");
            fs::write(
                self.directory.join(MANIFEST_PUBLIC_KEY_NAME),
                self.signing_key.verifying_key().to_bytes(),
            )
            .expect("public key fixture");
            fs::write(
                self.directory.join(MANIFEST_SIGNATURE_NAME),
                self.signing_key.sign(self.manifest.as_bytes()).to_bytes(),
            )
            .expect("signature fixture");
        }
        fn verify(&self) -> Result<VerifiedCudaBundle, String> {
            verify_bundle(&self.directory, &STEMS, &self.trusted_fingerprint)
        }
    }

    #[test]
    fn signed_bundle_binds_exact_source_ptx_and_toolchain_fields() {
        let fixture = Fixture::new();
        let verified = fixture.verify().expect("signed fixture");
        assert_eq!(verified.artifacts, fixture.artifacts);
        assert_eq!(verified.cuda_image_sha256, sha256_hex(b"pinned CUDA image"));
        assert_eq!(
            verified.nvcc_version_sha256,
            sha256_hex(b"nvcc --version output")
        );
        assert_eq!(verified.target_profile, "arch=compute_86,code=sm_86");
        assert_eq!(
            verified.nvcc_flags,
            "-ptx -std=c++14 -gencode arch=compute_86,code=sm_86"
        );
    }

    #[test]
    fn signed_bundle_accepts_large_source_trivia_and_preserves_exact_hashes() {
        let mut fixture = Fixture::new();
        let source = format!("//{}\n", "x".repeat(1024 * 1024));
        let path = fixture.directory.join("vector.cu");
        fs::write(&path, &source).expect("large source fixture");
        let original = format!(
            "artifact.vector.source_sha256={}",
            sha256_hex(b"// source for vector\n")
        );
        assert_eq!(fixture.manifest.matches(&original).count(), 1);
        fixture.manifest = fixture.manifest.replacen(
            &original,
            &format!(
                "artifact.vector.source_sha256={}",
                sha256_hex(source.as_bytes())
            ),
            1,
        );
        fixture.resign();
        assert_eq!(
            fixture.verify().expect("large signed source").artifacts,
            fixture.artifacts
        );
        assert_eq!(
            read_regular_file(&path, None).expect("exact source"),
            source.as_bytes()
        );
        assert!(
            read_regular_file(&path, Some(64))
                .unwrap_err()
                .contains("byte limit")
        );
        fs::write(path, format!("{source}// mutation\n")).expect("source mutation");
        assert!(fixture.verify().unwrap_err().contains("digest mismatch"));
    }

    #[cfg(unix)]
    #[test]
    fn source_reader_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let source = fixture.directory.join("vector.cu");
        let alias = fixture.directory.join("alias.cu");
        symlink(source, &alias).expect("source alias fixture");
        assert!(open_regular_file(&alias).is_err());
        assert!(
            read_regular_file(&alias, None)
                .unwrap_err()
                .contains("regular file")
        );
    }

    #[test]
    fn source_reader_requires_the_original_regular_inode() {
        let fixture = Fixture::new();
        let source = fixture.directory.join("vector.cu");
        let before = fs::symlink_metadata(&source).expect("source metadata");
        let opened = open_regular_file(&source).expect("direct source");
        assert!(same_regular_inode(
            &before,
            &opened.metadata().expect("retained inode")
        ));
        fs::rename(&source, fixture.directory.join("original.cu")).expect("move source");
        fs::write(&source, b"// source for vector\n").expect("same-byte replacement");
        #[cfg(unix)]
        assert!(!same_regular_inode(
            &before,
            &fs::symlink_metadata(&source).expect("replacement metadata")
        ));
        let retained = opened.metadata().expect("original retained inode");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            assert_eq!(before.dev(), retained.dev());
            assert_eq!(before.ino(), retained.ino());
        }
        assert_eq!(before.len(), retained.len());
    }

    #[test]
    fn bundle_rejects_unsigned_or_unreviewed_mutations() {
        let fixture = Fixture::new();
        let ptx_path = fixture.directory.join("aes.ptx");
        fs::write(&ptx_path, b"different PTX").expect("mutate PTX");
        assert!(fixture.verify().unwrap_err().contains("digest mismatch"));
        fs::write(&ptx_path, &fixture.artifacts[0]).expect("restore PTX");

        let source_path = fixture.directory.join("vector.cu");
        fs::write(&source_path, b"different source").expect("mutate source");
        assert!(fixture.verify().unwrap_err().contains("digest mismatch"));
        fs::write(&source_path, b"// source for vector\n").expect("restore source");

        let signature_path = fixture.directory.join(MANIFEST_SIGNATURE_NAME);
        fs::write(&signature_path, [0_u8; 64]).expect("mutate signature");
        assert!(
            fixture
                .verify()
                .unwrap_err()
                .contains("signature does not verify")
        );
        assert!(
            verify_bundle(&fixture.directory, &STEMS, &sha256_hex(b"unreviewed key"))
                .unwrap_err()
                .contains("reviewed fingerprint")
        );
    }

    #[test]
    fn signed_manifest_rejects_false_two_run_claim_and_extra_fields() {
        let mut fixture = Fixture::new();
        let recorded = format!(
            "generation_2_sha256={}",
            generation_sha256(&STEMS, &fixture.artifacts)
        );
        fixture.manifest = fixture.manifest.replacen(
            &recorded,
            &format!("generation_2_sha256={}", sha256_hex(b"different run")),
            1,
        );
        fixture.resign();
        assert!(
            fixture
                .verify()
                .unwrap_err()
                .contains("generation digests differ")
        );

        let mut fixture = Fixture::new();
        fixture.manifest.push_str("unexpected=field\n");
        fixture.resign();
        assert!(
            fixture
                .verify()
                .unwrap_err()
                .contains("unexpected trailing field")
        );
    }
}
