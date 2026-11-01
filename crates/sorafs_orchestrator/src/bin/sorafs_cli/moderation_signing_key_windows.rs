//! Bounded ordinary software signing-key input through retained Windows private custody.
//!
//! The original file and ancestry remain held through parsing and the final custody check.
//! This does not enable the separate Unix-only external software-signer transport, create
//! inputs, repair permissions, or substitute another credential.

use std::{io::Read as _, path::Path};

use zeroize::Zeroizing;

use super::{MODERATION_SIGNING_KEY_MAX_BYTES, PrivateKey, parse_private_key_inline};

pub(super) fn load(path: &Path, context: &str) -> Result<PrivateKey, String> {
    load_observed(path, context, || Ok(()))
}

// The private seam observes the real original immediately before final custody checks.
// Production supplies no operation; it cannot select a different reader or policy.
fn load_observed(
    path: &Path,
    context: &str,
    before_final: impl FnOnce() -> Result<(), String>,
) -> Result<PrivateKey, String> {
    // Native admission rejects nonlocal/reparse names before any pathname metadata probe.
    let mut original = iroha_fs::RetainedFile::open_private(path).map_err(|_| {
        "moderation result-signing key requires private current-owner regular-file custody"
            .to_string()
    })?;
    let before = original
        .snapshot()
        .map_err(|_| "failed to inspect moderation signing-key custody".to_string())?;
    // Both plaintext buffers outlive every ordinary read/parser failure and the final check.
    let mut bytes = Zeroizing::new(Vec::new());
    let mut boundary = Zeroizing::new([0_u8; 1]);
    let result = (|| {
        let length = original
            .file()
            .metadata()
            .map_err(|_| "failed to inspect moderation signing-key length".to_string())?
            .len();
        if length > MODERATION_SIGNING_KEY_MAX_BYTES {
            return Err(format!(
                "moderation result-signing key exceeds {MODERATION_SIGNING_KEY_MAX_BYTES} bytes"
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| "moderation signing-key length exceeds host width".to_string())?;
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "moderation signing-key allocation failed".to_string())?;
        bytes.resize(length, 0);
        original
            .file_mut()
            .read_exact(&mut bytes)
            .map_err(|_| "failed to read complete moderation signing key".to_string())?;
        loop {
            match original.file_mut().read(&mut boundary[..]) {
                Ok(0) => break,
                Ok(_) => return Err("moderation signing-key extent changed".to_string()),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err("failed to verify moderation signing-key extent".to_string()),
            }
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| format!("moderation result-signing key for `{context}` is not UTF-8"))?;
        parse_private_key_inline(text)
    })();
    let observed = before_final();
    // Evaluate eagerly even after a read/parser/observer failure. Preserve the initiating
    // error while refusing a parsed key if the original or its ancestors changed.
    let final_check = (|| {
        let after = original
            .snapshot()
            .map_err(|_| "moderation signing-key custody changed".to_string())?;
        if before != after {
            return Err("moderation signing-key original changed".to_string());
        }
        original
            .revalidate()
            .map_err(|_| "moderation signing-key custody changed".to_string())
    })();
    let key = result?;
    observed?;
    final_check?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write as _, path::PathBuf, process::Command};

    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};

    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        (temp, path)
    }

    fn key() -> (KeyPair, String) {
        let key = KeyPair::try_from_seed(vec![0x7C; 32], Algorithm::Ed25519).unwrap();
        let text = ExposedPrivateKey(key.private_key().clone()).to_string();
        (key, text)
    }

    fn write(path: &Path, bytes: &[u8]) {
        let mut owner = iroha_fs::RetainedFile::create_new_private(path).unwrap();
        owner.file_mut().write_all(bytes).unwrap();
        owner.file().sync_all().unwrap();
        owner.revalidate().unwrap();
    }

    fn acl(path: &Path) -> Vec<u8> {
        let result = Command::new("icacls.exe").arg(path).output().unwrap();
        assert!(
            result.status.success(),
            "actual ACL observation must succeed"
        );
        result.stdout
    }

    fn grant(path: &Path, rights: &str) {
        let result = Command::new("icacls.exe")
            .arg(path)
            .args(["/grant", rights])
            .output()
            .unwrap();
        assert!(result.status.success(), "actual ACL mutation must succeed");
    }

    #[test]
    fn native_moderation_key_preserves_original_bytes_identity_and_permissions() {
        let (_temp, root) = root();
        let (expected, text) = key();
        let path = root.join("signing.key");
        write(&path, text.as_bytes());
        let before = iroha_fs::RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap();
        let security = (acl(&root), acl(&path));
        assert_eq!(
            &load(&path, "native regression").unwrap(),
            expected.private_key()
        );
        assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
        assert_eq!((acl(&root), acl(&path)), security);
        assert_eq!(
            iroha_fs::RetainedFile::open_private(&path)
                .unwrap()
                .snapshot()
                .unwrap(),
            before
        );
    }

    #[test]
    fn native_moderation_key_preserves_exact_bound_and_shared_parse_errors() {
        let (_temp, root) = root();
        let (expected, text) = key();
        let mut at_limit = text.as_bytes().to_vec();
        at_limit.resize(MODERATION_SIGNING_KEY_MAX_BYTES as usize, b' ');
        let path = root.join("limit.key");
        write(&path, &at_limit);
        assert_eq!(&load(&path, "limit").unwrap(), expected.private_key());
        at_limit.push(b' ');
        let oversized = root.join("oversized.key");
        write(&oversized, &at_limit);
        assert!(load(&oversized, "limit").unwrap_err().contains("exceeds"));
        for (index, invalid) in ["", "MODERATION_SECRET_MUST_NOT_LEAK", "invalid-key"]
            .iter()
            .enumerate()
        {
            let path = root.join(format!("invalid-{index}.key"));
            write(&path, invalid.as_bytes());
            let error = load(&path, "parse").unwrap_err();
            assert_eq!(error, parse_private_key_inline(invalid).unwrap_err());
            assert!(!error.contains("MODERATION_SECRET_MUST_NOT_LEAK"));
            assert_eq!(fs::read(&path).unwrap(), invalid.as_bytes());
            assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        }
        let invalid_utf8 = root.join("utf8.key");
        write(&invalid_utf8, &[0xff]);
        assert!(
            load(&invalid_utf8, "parse")
                .unwrap_err()
                .contains("not UTF-8")
        );
    }

    #[test]
    fn native_moderation_key_refuses_hardlinks_missing_and_directory_inputs() {
        let (_temp, root) = root();
        let (_, text) = key();
        let path = root.join("signing.key");
        write(&path, text.as_bytes());
        let alias = root.join("alias.key");
        fs::hard_link(&path, &alias).unwrap();
        for p in [&path, &alias, &root, &root.join("missing.key")] {
            assert!(load(p, "custody").is_err());
        }
        assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
        fs::remove_file(alias).unwrap();
        assert!(load(&path, "custody").is_ok());
    }

    #[test]
    fn native_moderation_key_refuses_unsafe_acl_without_repair() {
        let (_temp, root) = root();
        let (_, text) = key();
        let inherited = root.join("inherited.key");
        fs::write(&inherited, text.as_bytes()).unwrap();
        let foreign = root.join("foreign.key");
        write(&foreign, text.as_bytes());
        grant(&foreign, "*S-1-1-0:(R)");
        for path in [&inherited, &foreign] {
            let before = acl(path);
            assert!(load(path, "custody").is_err());
            assert_eq!(acl(path), before);
            assert_eq!(fs::read(path).unwrap(), text.as_bytes());
        }
        let private = root.join("private.key");
        write(&private, text.as_bytes());
        grant(&root, "*S-1-1-0:(W)");
        let before = acl(&root);
        assert!(load(&private, "custody").is_err());
        assert_eq!(acl(&root), before);
    }

    #[test]
    fn native_moderation_key_rejects_writer_and_holds_original_through_parse() {
        let (_temp, root) = root();
        let (expected, text) = key();
        let path = root.join("held.key");
        let mut writer = iroha_fs::RetainedFile::create_new_private(&path).unwrap();
        writer.file_mut().write_all(text.as_bytes()).unwrap();
        writer.file().sync_all().unwrap();
        assert!(load(&path, "writer").is_err());
        drop(writer);
        let mut called = false;
        let observed = load_observed(&path, "held", || {
            called = true;
            assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
            assert!(fs::remove_file(&path).is_err());
            assert!(fs::rename(&path, root.join("replaced.key")).is_err());
            assert!(fs::rename(&root, root.with_extension("moved")).is_err());
            Ok(())
        })
        .unwrap();
        assert!(called);
        assert_eq!(&observed, expected.private_key());
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
    }

    #[test]
    fn native_moderation_key_refuses_late_file_and_ancestor_acl_changes() {
        for parent in [false, true] {
            let (_temp, root) = root();
            let (_, text) = key();
            let path = root.join("changing.key");
            write(&path, text.as_bytes());
            let error = load_observed(&path, "final custody", || {
                if parent {
                    grant(&root, "*S-1-1-0:(W)");
                } else {
                    grant(&path, "*S-1-1-0:(R)");
                }
                Ok(())
            })
            .unwrap_err();
            assert!(error.contains("custody changed"));
            assert!(load(&path, "no repair").is_err());
            assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
        }
    }

    #[test]
    fn native_moderation_key_keeps_parser_and_observer_errors_before_final_refusal() {
        for valid in [false, true] {
            let (_temp, root) = root();
            let (_, text) = key();
            let bytes = if valid { text.as_str() } else { "invalid-key" };
            let path = root.join("first.key");
            write(&path, bytes.as_bytes());
            let expected = if valid {
                "first observation failure".to_string()
            } else {
                parse_private_key_inline(bytes).unwrap_err()
            };
            let error = load_observed(&path, "first error", || {
                grant(&path, "*S-1-1-0:(R)");
                Err("first observation failure".to_string())
            })
            .unwrap_err();
            assert_eq!(error, expected);
            assert!(iroha_fs::RetainedFile::open_private(&path).is_err());
            assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
            assert_eq!(fs::read(&path).unwrap(), bytes.as_bytes());
        }
    }

    #[test]
    fn native_moderation_key_refuses_junction_ancestry_and_nonlocal_names() {
        let (_temp, root) = root();
        let (_, text) = key();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        let path = real.join("signing.key");
        write(&path, text.as_bytes());
        let nested = real.join("nested");
        fs::create_dir(&nested).unwrap();
        write(&nested.join("signing.key"), text.as_bytes());
        let junction = root.join("junction");
        let command = Command::new("cmd.exe")
            .current_dir(&root)
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&real)
            .output()
            .unwrap();
        assert!(command.status.success(), "real junction fixture must exist");
        for path in [
            junction.join("signing.key"),
            junction.join("nested/signing.key"),
            PathBuf::from(r"\\server\share\key"),
            PathBuf::from(r"\\?\UNC\server\share\key"),
            PathBuf::from(r"\\.\C:\key"),
        ] {
            let mut called = false;
            assert!(
                load_observed(&path, "namespace", || {
                    called = true;
                    Ok(())
                })
                .is_err()
            );
            assert!(!called);
        }
        assert_eq!(fs::read(path).unwrap(), text.as_bytes());
        fs::remove_dir(junction).unwrap();
    }

    #[test]
    fn native_moderation_key_unwind_releases_original_without_mutation() {
        let (_temp, root) = root();
        let (_, text) = key();
        let path = root.join("unwind.key");
        write(&path, text.as_bytes());
        let result = std::panic::catch_unwind(|| {
            let _ = load_observed(&path, "unwind", || panic!("actual retained boundary"));
        });
        assert!(result.is_err());
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok());
        assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
        assert!(load(&path, "after unwind").is_ok());
    }
}
