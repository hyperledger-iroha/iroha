//! Real native handshake-file and authenticated-cache controls, pending execution.

use std::{
    fs,
    io::Write as _,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use base64::Engine as _;
use iroha_crypto::soranet::token::{
    AdmissionTokenVerifier, InMemoryTokenStore, TokenStore, TokenStoreLimits,
};
use rand::{SeedableRng as _, rngs::StdRng};
use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os};

use super::super::{
    HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1, HandshakeTokenFingerprintArgs, HandshakeTokenIdArgs,
    HandshakeTokenIssueArgs, MlDsaSuiteArg, URL_SAFE_NO_PAD, load_guard_cache_key_file,
    persist_guard_set, tests::TestContext,
};
use super::*;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().expect("native fixture");
    let root = temporary.path().join("state");
    drop(iroha_fs::PrivateDirectory::open_or_create(&root).unwrap());
    (temporary, root)
}

fn private_file(parent: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = parent.join(name);
    let mut writer = RetainedFile::create_new_private(&path).unwrap();
    writer.file_mut().write_all(bytes).unwrap();
    writer.file().sync_all().unwrap();
    writer.revalidate().unwrap();
    drop(writer);
    path
}

fn acl(path: &Path) -> Vec<u8> {
    let result = Command::new("icacls.exe").arg(path).output().unwrap();
    assert!(result.status.success(), "fixture ACL inspection failed");
    result.stdout
}

fn grant(path: &Path, permission: &str) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", permission])
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture ACL mutation failed");
}

fn junction(path: &Path, target: &Path) {
    let result = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture junction creation failed");
}

#[test]
fn native_handshake_private_and_public_reads_preserve_originals_and_acl() {
    let (_temporary, root) = fixture();
    let secret = private_file(&root, "secret", &[0xA5; 32]);
    let public = private_file(&root, "public", &[0xB6; 32]);
    grant(&public, "*S-1-1-0:(R)");
    let before = (acl(&root), acl(&secret), acl(&public));
    let secret_snapshot = RetainedFile::open_private(&secret)
        .unwrap()
        .snapshot()
        .unwrap();
    let public_snapshot = RetainedFile::open_regular(&public)
        .unwrap()
        .snapshot()
        .unwrap();
    for path in [&secret, &fs::canonicalize(&secret).unwrap()] {
        assert_eq!(
            read_private(path, 32, Some(32), "secret")
                .unwrap()
                .as_slice(),
            &[0xA5; 32]
        );
    }
    assert_eq!(
        read_public(&public, 32, Some(32), "public").unwrap(),
        [0xB6; 32]
    );
    assert!(read_private(&public, 32, Some(32), "secret").is_err());
    assert_eq!((acl(&root), acl(&secret), acl(&public)), before);
    assert_eq!(
        RetainedFile::open_private(&secret)
            .unwrap()
            .snapshot()
            .unwrap(),
        secret_snapshot
    );
    assert_eq!(
        RetainedFile::open_regular(&public)
            .unwrap()
            .snapshot()
            .unwrap(),
        public_snapshot
    );
    let expected_key = GuardCacheKey::from_bytes([0xA5; 32]).unwrap();
    let encoded = GuardSet::new(Vec::new())
        .encode_authenticated(&expected_key)
        .unwrap();
    let loaded_key = load_guard_cache_key_file(&secret).unwrap();
    assert!(
        GuardSet::decode_authenticated(&encoded, &loaded_key)
            .unwrap()
            .guards()
            .is_empty()
    );
}

#[test]
fn native_handshake_refuses_inherited_private_acl_and_unsafe_public_acl_without_repair() {
    let temporary = tempfile::tempdir().unwrap();
    let inherited = temporary.path().join("inherited");
    fs::write(&inherited, [0xA5; 32]).unwrap();
    let inherited_acl = acl(&inherited);
    assert!(read_private(&inherited, 32, Some(32), "secret").is_err());
    assert_eq!(acl(&inherited), inherited_acl);
    let public = private_file(temporary.path(), "public", &[0xB6; 32]);
    grant(&public, "*S-1-1-0:(W)");
    let before = acl(&public);
    assert!(read_public(&public, 32, Some(32), "public").is_err());
    assert_eq!(acl(&public), before);
    assert_eq!(fs::read(public).unwrap(), [0xB6; 32]);
}

#[test]
fn native_handshake_refuses_hardlinks_junctions_and_nonfiles() {
    let (temporary, root) = fixture();
    let path = private_file(&root, "key", &[0xA5; 32]);
    fs::hard_link(&path, root.join("alias")).unwrap();
    assert!(read_private(&path, 32, None, "key").is_err());
    assert!(read_public(&path, 32, None, "key").is_err());
    fs::remove_file(root.join("alias")).unwrap();
    let link = temporary.path().join("junction");
    junction(&link, &root);
    for private in [false, true] {
        assert!(
            read_observed(&link.join("key"), 32, None, "key", private, |_| panic!(
                "junction must not reach read"
            ))
            .is_err()
        );
        assert!(
            read_observed(&root, 32, None, "key", private, |_| panic!(
                "directory must not reach read"
            ))
            .is_err()
        );
    }
    assert_eq!(fs::read(path).unwrap(), [0xA5; 32]);
}

#[test]
fn native_handshake_refuses_nonlocal_namespaces_before_read_or_creation() {
    let key = GuardCacheKey::from_bytes([7; 32]).unwrap();
    for path in [
        r"\\server\share\key",
        r"\\?\UNC\server\share\key",
        r"\\.\C:\key",
    ] {
        for private in [false, true] {
            assert!(
                read_observed(Path::new(path), 32, None, "key", private, |_| panic!(
                    "namespace reached read"
                ))
                .is_err()
            );
        }
        assert!(
            load_guard_set_observed(Path::new(path), &key, |_| panic!(
                "namespace reached cache read"
            ))
            .is_err()
        );
        assert!(write_token(Path::new(path), TokenOutputFormat::Binary, b"secret").is_err());
        assert!(persist_guard_cache(Path::new(path), b"cache").is_err());
    }
}

#[test]
fn native_handshake_bounds_precede_payload_read_and_preserve_oversized_extent() {
    let (_temporary, root) = fixture();
    for (name, length) in [("empty", 0), ("short", 31), ("long", 33)] {
        let path = private_file(&root, name, &vec![0xA5; length]);
        assert!(
            read_observed(&path, 32, Some(32), "key", true, |_| panic!(
                "invalid extent reached read callback"
            ))
            .is_err()
        );
    }
    let oversized = root.join("oversized");
    let original = RetainedFile::create_new_private(&oversized).unwrap();
    original
        .file()
        .set_len((HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1 + 1) as u64)
        .unwrap();
    original.file().sync_all().unwrap();
    drop(original);
    let before = RetainedFile::open_private(&oversized)
        .unwrap()
        .snapshot()
        .unwrap();
    assert!(
        read_observed(
            &oversized,
            HANDSHAKE_TOKEN_FILE_MAX_BYTES_V1,
            None,
            "token",
            true,
            |_| panic!("oversized payload must be refused before read")
        )
        .is_err()
    );
    assert_eq!(
        RetainedFile::open_private(&oversized)
            .unwrap()
            .snapshot()
            .unwrap(),
        before
    );
}

#[test]
fn native_handshake_retains_writer_delete_and_rename_exclusion() {
    let (_temporary, root) = fixture();
    let path = private_file(&root, "key", &[0xA5; 32]);
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(read_private(&path, 32, Some(32), "key").is_err());
    drop(writer);
    let before = RetainedFile::open_private(&path)
        .unwrap()
        .snapshot()
        .unwrap();
    read_observed(&path, 32, Some(32), "key", true, |_| {
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert!(fs::rename(&path, root.join("moved")).is_err());
        Ok(())
    })
    .unwrap();
    assert_eq!(
        RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap(),
        before
    );
}

#[test]
fn native_handshake_final_custody_refuses_changed_acl_and_preserves_first_error() {
    let (_temporary, root) = fixture();
    let path = private_file(&root, "key", &[0xA5; 32]);
    assert!(
        read_observed(&path, 32, Some(32), "key", true, |_| {
            grant(&path, "*S-1-1-0:(R)");
            Ok(())
        })
        .is_err()
    );
    let other = private_file(&root, "other", &[0xB6; 32]);
    let error = read_observed(&other, 32, Some(32), "key", true, |_| {
        grant(&other, "*S-1-1-0:(R)");
        Err(eyre!("original read observation refusal"))
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("original read observation refusal")
    );
    assert_eq!(fs::read(other).unwrap(), [0xB6; 32]);
}

#[test]
fn native_token_create_new_formats_preserve_private_bytes_and_refuse_overwrite() {
    let (_temporary, root) = fixture();
    let bytes = b"\0token\xffbytes";
    for (name, format) in [
        ("binary", TokenOutputFormat::Binary),
        ("hex", TokenOutputFormat::Hex),
        ("base64", TokenOutputFormat::Base64),
    ] {
        let path = root.join(name);
        write_token(&path, format, bytes).unwrap();
        let stored = read_private(&path, 128, None, "token").unwrap();
        let decoded = match format {
            TokenOutputFormat::Binary => stored.to_vec(),
            TokenOutputFormat::Hex => {
                hex::decode(std::str::from_utf8(&stored).unwrap().trim_end()).unwrap()
            }
            TokenOutputFormat::Base64 => URL_SAFE_NO_PAD
                .decode(std::str::from_utf8(&stored).unwrap().trim_end())
                .unwrap(),
        };
        assert_eq!(decoded, bytes);
        let before = RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap();
        assert!(write_token(&path, format, b"clobber").is_err());
        assert_eq!(
            RetainedFile::open_private(&path)
                .unwrap()
                .snapshot()
                .unwrap(),
            before
        );
        assert_eq!(fs::read(path).unwrap(), *stored);
    }
}

#[test]
fn native_token_write_keeps_original_and_refuses_post_write_custody_loss() {
    let (_temporary, root) = fixture();
    let path = root.join("token");
    let error = write_token_observed(&path, TokenOutputFormat::Binary, b"secret", |_| {
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
        grant(&path, "*S-1-1-0:(R)");
        Ok(())
    })
    .unwrap_err();
    assert!(!error.to_string().is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"secret");
    assert!(write_token(&path, TokenOutputFormat::Binary, b"retry").is_err());
}

#[test]
fn native_guard_cache_absence_never_creates_or_accepts_parent_traversal() {
    let (_temporary, root) = fixture();
    let key = GuardCacheKey::from_bytes([7; 32]).unwrap();
    assert!(load_guard_set(&root.join("cache"), &key).unwrap().is_none());
    let nested = root.join("missing/nested/cache");
    assert!(load_guard_set(&nested, &key).unwrap().is_none());
    assert!(!root.join("missing").exists());
    assert!(load_guard_set(&root.join("missing/../cache"), &key).is_err());
    assert!(persist_guard_cache(&root.join("missing/../cache"), b"cache").is_err());
    assert!(!root.join("missing").exists());
}

#[test]
fn native_guard_cache_round_trip_replacement_keeps_safe_public_parent() {
    for public in [false, true] {
        let (_temporary, root) = fixture();
        if public {
            grant(&root, "*S-1-1-0:(R)");
        }
        let before = acl(&root);
        let path = root.join("nested/guards.norito");
        let key = GuardCacheKey::from_bytes([7; 32]).unwrap();
        let guards = GuardSet::new(Vec::new());
        persist_guard_set(&path, &guards, Some(&key)).unwrap();
        let first = RetainedFile::open_private(&path)
            .unwrap()
            .identity()
            .unwrap();
        persist_guard_set(&path, &guards, Some(&key)).unwrap();
        assert_ne!(
            RetainedFile::open_private(&path)
                .unwrap()
                .identity()
                .unwrap(),
            first
        );
        assert!(
            load_guard_set(&path, &key)
                .unwrap()
                .unwrap()
                .guards()
                .is_empty()
        );
        assert_eq!(acl(&root), before);
        assert_eq!(fs::read_dir(root.join("nested")).unwrap().count(), 1);
    }
}

#[test]
fn native_guard_cache_authentication_bounds_and_private_leaf_policy_remain_closed() {
    let (_temporary, root) = fixture();
    let path = root.join("cache");
    let key = GuardCacheKey::from_bytes([7; 32]).unwrap();
    let other = GuardCacheKey::from_bytes([8; 32]).unwrap();
    let guards = GuardSet::new(Vec::new());
    persist_guard_set(&path, &guards, Some(&key)).unwrap();
    assert!(super::super::load_guard_set(&path, None).is_err());
    assert!(persist_guard_set(&path, &guards, None).is_err());
    assert!(load_guard_set(&path, &other).is_err());
    let before = fs::read(&path).unwrap();
    grant(&path, "*S-1-1-0:(R)");
    let before_acl = acl(&path);
    assert!(load_guard_set(&path, &key).is_err());
    assert!(persist_guard_set(&path, &guards, Some(&key)).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(acl(&path), before_acl);
    let oversized = root.join("oversized");
    let writer = RetainedFile::create_new_private(&oversized).unwrap();
    writer
        .file()
        .set_len((GUARD_CACHE_MAX_BYTES_V1 + 1) as u64)
        .unwrap();
    drop(writer);
    assert!(load_guard_set(&oversized, &key).is_err());
    let bad = private_file(&root, "bad", b"not an authenticated envelope");
    assert!(load_guard_set(&bad, &key).is_err());
}

#[test]
fn native_guard_cache_rejects_aliases_writers_and_changed_parent_without_repair() {
    let (temporary, root) = fixture();
    let path = root.join("cache");
    let key = GuardCacheKey::from_bytes([7; 32]).unwrap();
    let guards = GuardSet::new(Vec::new());
    persist_guard_set(&path, &guards, Some(&key)).unwrap();
    let before = fs::read(&path).unwrap();
    fs::hard_link(&path, root.join("alias")).unwrap();
    assert!(load_guard_set(&path, &key).is_err());
    assert!(persist_guard_set(&path, &guards, Some(&key)).is_err());
    fs::remove_file(root.join("alias")).unwrap();
    let link = temporary.path().join("junction");
    junction(&link, &root);
    assert!(load_guard_set(&link.join("cache"), &key).is_err());
    assert!(persist_guard_set(&link.join("cache"), &guards, Some(&key)).is_err());
    let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    assert!(load_guard_set(&path, &key).is_err());
    drop(writer);
    assert!(
        load_guard_set_observed(&path, &key, |_| {
            grant(&root, "*S-1-1-0:(W)");
            Ok(())
        })
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn native_handshake_real_issue_verify_and_id_keep_bearer_off_summary() {
    let (_temporary, root) = fixture();
    let keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa44).unwrap();
    let secret = private_file(&root, "issuer.secret", keys.secret_key());
    let public = private_file(&root, "issuer.public", keys.public_key());
    grant(&public, "*S-1-1-0:(R)");
    let output = root.join("token");
    let mut context = TestContext::new();
    let mut args = HandshakeTokenIssueArgs {
        suite: MlDsaSuiteArg::MlDsa44,
        issuer_secret_key: secret,
        issuer_public_key: Some(public.clone()),
        issuer_public_hex: None,
        relay_id: "11".repeat(32),
        transcript_hash: "22".repeat(32),
        issued_at: Some("2026-01-01T00:00:00Z".to_owned()),
        expires_at: None,
        ttl_secs: Some(900),
        flags: Some(0),
        output: output.clone(),
        token_encoding: TokenOutputFormat::Binary,
    };
    let mut rng = StdRng::seed_from_u64(0x5eed);
    let mut artifacts = args
        .issue_with_rng(&mut context, &mut rng, SystemTime::UNIX_EPOCH)
        .unwrap();
    let limits = TokenStoreLimits::new(4, Duration::from_secs(1800)).unwrap();
    let replay: Arc<Mutex<dyn TokenStore + Send>> =
        Arc::new(Mutex::new(InMemoryTokenStore::new(limits).unwrap()));
    let verifier = AdmissionTokenVerifier::try_new(
        MlDsaSuite::MlDsa44,
        keys.public_key().to_vec(),
        Duration::from_secs(900),
        Duration::from_secs(5),
        replay,
    )
    .unwrap();
    verifier
        .verify(
            &artifacts.token,
            &artifacts.relay_id,
            &artifacts.transcript_hash,
            SystemTime::UNIX_EPOCH + Duration::from_secs(artifacts.token.issued_at() + 1),
        )
        .unwrap();
    HandshakeTokenIssueArgs::emit(&mut context, &artifacts, &output, TokenOutputFormat::Binary)
        .unwrap();
    let issued: norito::json::Value =
        norito::json::from_str(context.outputs().last().unwrap()).unwrap();
    assert!(issued.get("token_base64url").is_none());
    assert!(issued.get("token_hex").is_none());
    HandshakeTokenIdArgs {
        path: output.clone(),
    }
    .run(&mut context)
    .unwrap();
    let id: norito::json::Value =
        norito::json::from_str(context.outputs().last().unwrap()).unwrap();
    assert_eq!(id["token_id_hex"], issued["token_id_hex"]);
    HandshakeTokenFingerprintArgs {
        public_key: Some(public),
        public_key_hex: None,
    }
    .run(&mut context)
    .unwrap();
    let fingerprint: norito::json::Value =
        norito::json::from_str(context.outputs().last().unwrap()).unwrap();
    assert_eq!(
        fingerprint["issuer_fingerprint_hex"],
        issued["issuer_fingerprint_hex"]
    );
    let summaries = context.outputs().len();
    assert!(
        HandshakeTokenIssueArgs::emit(&mut context, &artifacts, &output, TokenOutputFormat::Binary)
            .is_err()
    );
    assert_eq!(context.outputs().len(), summaries);
    artifacts.zeroize_encoded_token();
    assert!(artifacts.token_bytes.is_empty());
    args.issued_at = Some("2026-01-01T00:00:00.123Z".to_owned());
    let error = args
        .issue_with_rng(&mut context, &mut rng, SystemTime::UNIX_EPOCH)
        .err()
        .expect("explicit fractional timestamp must refuse");
    assert!(
        error
            .to_string()
            .contains("--issued-at must use whole-second")
    );
    args.issued_at = None;
    let default_time = SystemTime::UNIX_EPOCH + Duration::from_millis(10_987);
    let default_token = args
        .issue_with_rng(&mut context, &mut rng, default_time)
        .unwrap();
    assert_eq!(default_token.token.issued_at(), 10);
    assert_eq!(context.outputs().len(), summaries);
}
