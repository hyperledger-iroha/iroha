//! Fresh observations from the native owner of an exact private generation.
//!
//! The serializable receipt is a public projection, not a transferable process capability.
//! Consumers must invoke the admitted installed launcher with a new independently generated
//! challenge, then authenticate the operating-system processes before and after publication.

use super::*;
use iroha_data_model::{NetworkId, block::consensus::SumeragiRootScope};
use iroha_fs::{FileSnapshot, PrivateDirectory, RetainedFile};
use std::path::Path;
use zeroize::Zeroizing;

/// One original retained file, without secret-bearing contents.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedStartupFile {
    /// Canonical absolute source path.
    pub path: PathBuf,
    /// Digest checked against the native retained generation.
    pub blake3: String,
    /// Exact source length.
    pub size: u64,
}

/// A live validator whose unreaped child handle is held by this native supervisor.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedStartupPeer {
    /// Stable generation order.
    pub index: usize,
    /// Kernel process identifier from the actual owned child handle.
    pub pid: u32,
    /// Actual command selected before the native spawn.
    pub argv: Vec<String>,
    /// Original generated configuration.
    pub config: ManagedStartupFile,
    /// Endpoint validated against that exact generation.
    pub torii_url: String,
}

/// Challenge-bound public observation from a reachable authenticated ready supervisor.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedStartupReceipt {
    /// Closed receipt schema.
    pub schema: String,
    /// First-release schema version.
    pub schema_version: u8,
    /// Fresh caller-selected challenge; retained metadata cannot supply this value.
    pub challenge: String,
    /// Independently selected private managed store.
    pub store_root: PathBuf,
    /// Exact original generation directory.
    pub generation_path: PathBuf,
    /// Current native supervisor, which retains the runtime ownership lock.
    pub worker_pid: u32,
    /// Original validated context.
    pub context: ManagedContext,
    /// Independently selected parent and full-width private child identity.
    pub private_root: crate::localnet::PrivateRootSpec,
    /// Scope decoded and verified from the signed original private genesis.
    pub root_scope: SumeragiRootScope,
    /// Network derived from that signed genesis.
    pub network_id: NetworkId,
    /// Explicit generated account address discriminator.
    pub chain_discriminant: u16,
    /// Signed-genesis verification key retained by the native generation.
    pub genesis_public_key: String,
    /// Immutable native generation manifest.
    pub generation_manifest: ManagedStartupFile,
    /// Original retained private-root manifest.
    pub private_root_manifest: ManagedStartupFile,
    /// Original genesis manifest.
    pub genesis_manifest: ManagedStartupFile,
    /// Original signed genesis.
    pub signed_genesis: ManagedStartupFile,
    /// Exact installed supervisor binary.
    pub launcher: ManagedStartupFile,
    /// Exact installed validator binary.
    pub daemon: ManagedStartupFile,
    /// Exactly four currently live owned validator children.
    pub peers: Vec<ManagedStartupPeer>,
}

pub(super) fn validate_challenge(challenge: &str) -> Result<()> {
    if challenge.len() != 64
        || !challenge
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || challenge.bytes().all(|byte| byte == b'0')
    {
        return Err(Error::Invalid(
            "startup receipt requires a fresh 32-byte lowercase hex challenge".into(),
        ));
    }
    Ok(())
}

/// Retained launch-time authority; its projection never substitutes a later pathname read.
struct LaunchFile {
    original: RetainedFile,
    snapshot: FileSnapshot,
    pin: ManagedStartupFile,
}

impl LaunchFile {
    fn retain(directory: &PrivateDirectory, name: &str, maximum: usize) -> Result<Self> {
        directory.revalidate()?;
        let path = directory.path().join(name);
        let original = RetainedFile::open_private(&path)?;
        let snapshot = original.snapshot()?;
        let size = original.file().metadata()?.len();
        if size == 0 || size > maximum as u64 {
            return Err(Error::Invalid(
                "startup source has an invalid bounded length".into(),
            ));
        }
        let digest = Self::digest(&original, size)?;
        if original.snapshot()? != snapshot {
            return Err(Error::Invalid(
                "startup source changed while being retained".into(),
            ));
        }
        directory.revalidate()?;
        Ok(Self {
            original,
            snapshot,
            pin: ManagedStartupFile {
                path,
                blake3: digest,
                size,
            },
        })
    }

    fn digest(original: &RetainedFile, size: u64) -> Result<String> {
        original.revalidate()?;
        let before = original.snapshot()?;
        if original.file().metadata()?.len() != size {
            return Err(Error::Invalid("startup source length changed".into()));
        }
        let mut buffer = Zeroizing::new([0_u8; 64 * 1024]);
        let mut digest = blake3::Hasher::new();
        let mut offset = 0_u64;
        while offset < size {
            let count = usize::try_from((size - offset).min(buffer.len() as u64))
                .map_err(|_| Error::Invalid("startup source length overflow".into()))?;
            iroha_fs::read_exact_at(original.file(), &mut buffer[..count], offset)?;
            digest.update(&buffer[..count]);
            offset += count as u64;
        }
        if iroha_fs::read_at(original.file(), &mut buffer[..1], size)? != 0
            || original.snapshot()? != before
        {
            return Err(Error::Invalid(
                "startup source changed during hashing".into(),
            ));
        }
        Ok(digest.finalize().to_hex().to_string())
    }

    fn validate(&self) -> Result<()> {
        // Compare both the held inode and an independently opened current name. Even replacing
        // a file with the same bytes cannot promote the replacement into the original launch.
        if self.original.snapshot()? != self.snapshot
            || Self::digest(&self.original, self.pin.size)? != self.pin.blake3
        {
            return Err(Error::Invalid(
                "original startup source changed after selection".into(),
            ));
        }
        let named = RetainedFile::open_private(&self.pin.path)?;
        if named.snapshot()? != self.snapshot
            || Self::digest(&named, self.pin.size)? != self.pin.blake3
            || self.original.snapshot()? != self.snapshot
        {
            return Err(Error::Invalid(
                "named startup source differs from the original launch".into(),
            ));
        }
        Ok(())
    }
}

/// Original config and manifest descriptors held for the lifetime of the owned children.
pub(super) struct LaunchSnapshot {
    generation: PrivateDirectory,
    configs: Vec<LaunchFile>,
    generation_manifest: LaunchFile,
    private_root_manifest: LaunchFile,
    genesis_manifest: LaunchFile,
    signed_genesis: LaunchFile,
}

impl LaunchSnapshot {
    pub(super) fn retain(
        directory: &PrivateDirectory,
        retained: &RetainedLocalnet,
    ) -> Result<Self> {
        if !matches!(retained.root_kind, RootKind::Private { .. })
            || retained.prepared.peers.len() != 4
        {
            return Err(Error::Invalid(
                "private launch requires four original configurations".into(),
            ));
        }
        let snapshot = Self::retain_sources(
            directory.open_child(generation::DIRECTORY)?,
            &retained.prepared.peers,
        )?;
        let current = generation::read(directory)?;
        if encode(&current)? != encode(retained)? {
            return Err(Error::Invalid(
                "private generation changed before its original launch".into(),
            ));
        }
        store::validate_prepared(
            &retained.prepared.context.name,
            directory.path(),
            &retained.prepared,
            &retained.root_kind,
        )?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn retain_sources(generation: PrivateDirectory, peers: &[ManagedPeer]) -> Result<Self> {
        if peers.len() != 4 {
            return Err(Error::Invalid(
                "private launch requires four original configurations".into(),
            ));
        }
        let configs = peers
            .iter()
            .enumerate()
            .map(|(index, peer)| {
                let original =
                    LaunchFile::retain(&generation, &format!("peer{index}.toml"), MAX_METADATA)?;
                if original.pin.path != peer.config_path {
                    return Err(Error::Invalid(
                        "private launch configuration is out of generation order".into(),
                    ));
                }
                Ok(original)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            generation_manifest: LaunchFile::retain(&generation, MANIFEST, MAX_METADATA)?,
            private_root_manifest: LaunchFile::retain(
                &generation,
                "private-root-prepared.json",
                MAX_METADATA,
            )?,
            genesis_manifest: LaunchFile::retain(
                &generation,
                "genesis.json",
                iroha_genesis::GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
            )?,
            signed_genesis: LaunchFile::retain(
                &generation,
                "genesis.signed.nrt",
                iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1,
            )?,
            generation,
            configs,
        })
    }

    pub(super) fn config(&self, index: usize) -> Result<&ManagedStartupFile> {
        self.generation.revalidate()?;
        let config = self
            .configs
            .get(index)
            .ok_or_else(|| Error::Invalid("private launch configuration index is absent".into()))?;
        config.validate()?;
        Ok(&config.pin)
    }

    pub(super) fn validate(&self) -> Result<()> {
        self.generation.revalidate()?;
        for config in &self.configs {
            config.validate()?;
        }
        for original in [
            &self.generation_manifest,
            &self.private_root_manifest,
            &self.genesis_manifest,
            &self.signed_genesis,
        ] {
            original.validate()?;
        }
        self.generation.revalidate()?;
        Ok(())
    }
}

fn executable(pin: &BinaryPin) -> Result<ManagedStartupFile> {
    store::verify_binary(pin)?;
    Ok(ManagedStartupFile {
        path: pin.path.clone(),
        blake3: pin.blake3.clone(),
        size: std::fs::metadata(&pin.path)?.len(),
    })
}

fn validate_child_argv(argv: &[String], expected: &[String]) -> Result<()> {
    if argv != expected
        && !(argv.len() == expected.len() + 1
            && argv[..expected.len()] == *expected
            && argv
                .last()
                .is_some_and(|argument| argument == "--sumeragi-assert-fresh-key"))
    {
        return Err(Error::Invalid(
            "live startup argv differs from its original configuration digest".into(),
        ));
    }
    Ok(())
}

pub(super) fn capture(
    store_root: &Path,
    directory: &PrivateDirectory,
    retained: &RetainedLocalnet,
    challenge: &str,
    children: &[(u32, Vec<String>)],
    launch: &LaunchSnapshot,
) -> Result<ManagedStartupReceipt> {
    validate_challenge(challenge)?;
    launch.validate()?;
    directory.revalidate()?;
    let current = generation::read(directory)?;
    if encode(&current)? != encode(retained)? || children.len() != 4 {
        return Err(Error::Invalid(
            "live startup ownership differs from the exact retained generation".into(),
        ));
    }
    let RootKind::Private { spec } = &retained.root_kind else {
        return Err(Error::Invalid(
            "private startup receipts require a native private root".into(),
        ));
    };
    store::validate_prepared(
        &retained.prepared.context.name,
        directory.path(),
        &retained.prepared,
        &retained.root_kind,
    )?;
    let generation = &launch.generation;
    if generation.path() != directory.path().join(generation::DIRECTORY) {
        return Err(Error::Invalid(
            "startup snapshot belongs to another generation".into(),
        ));
    }
    let client = retained.prepared.context.load_client_config()?;
    let network_id = retained
        .prepared
        .context
        .network_id
        .parse::<NetworkId>()
        .map_err(|_| Error::Invalid("native private context has an invalid NetworkId".into()))?;
    let public_key = generation.read(crate::localnet::GENESIS_PUBLIC_KEY_FILE, 4096)?;
    let genesis_public_key = std::str::from_utf8(&public_key)
        .map_err(|_| Error::Invalid("native genesis public key is not UTF-8".into()))?
        .trim()
        .to_owned();
    if genesis_public_key != client.key_pair.public_key().to_string() {
        return Err(Error::Invalid(
            "native private genesis signer differs from its retained owner".into(),
        ));
    }
    let peers = retained
        .prepared
        .peers
        .iter()
        .zip(children)
        .enumerate()
        .map(|(index, (peer, (pid, argv)))| {
            let config = launch.config(index)?.clone();
            if config.path != peer.config_path || *pid <= 1 {
                return Err(Error::Invalid(
                    "live startup child differs from its retained config".into(),
                ));
            }
            let command = runtime::daemon_command(
                &retained.daemon.path,
                &config.path,
                &retained.root_kind,
                Some(&config.blake3),
            )?;
            validate_child_argv(argv, &runtime::command_argv(&command)?)?;
            Ok(ManagedStartupPeer {
                index,
                pid: *pid,
                argv: argv.clone(),
                config,
                torii_url: peer.torii_url.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let receipt = ManagedStartupReceipt {
        schema: "iroha-managed-private-startup-receipt".into(),
        schema_version: 1,
        challenge: challenge.into(),
        store_root: store_root.to_path_buf(),
        generation_path: generation.path().to_path_buf(),
        worker_pid: std::process::id(),
        context: retained.prepared.context.clone(),
        private_root: spec.clone(),
        root_scope: spec.scope(),
        network_id,
        chain_discriminant: client.account_chain_discriminant,
        genesis_public_key,
        generation_manifest: launch.generation_manifest.pin.clone(),
        private_root_manifest: launch.private_root_manifest.pin.clone(),
        genesis_manifest: launch.genesis_manifest.pin.clone(),
        signed_genesis: launch.signed_genesis.pin.clone(),
        launcher: executable(&retained.launcher)?,
        daemon: executable(&retained.daemon)?,
        peers,
    };
    launch.validate()?;
    directory.revalidate()?;
    Ok(receipt)
}

impl ManagedStore {
    /// Obtain a fresh receipt only from this exact ready private generation's live native owner.
    ///
    /// Persisted status, stale receipts and unreachable or stopped workers are never accepted.
    /// The decoded projection does not replace the caller's subsequent kernel process checks.
    ///
    /// # Errors
    /// Rejects malformed challenge, changed custody or genesis, wrong independently selected
    /// private identity, absent runtime ownership, and a missing authenticated live worker.
    pub fn private_startup_receipt(
        &self,
        name: &str,
        expected: &crate::localnet::PrivateRootSpec,
        challenge: &str,
    ) -> Result<ManagedStartupReceipt> {
        validate_challenge(challenge)?;
        expected
            .validate()
            .map_err(|_| Error::Invalid("invalid independently selected private root".into()))?;
        let directory = self.directory(name)?;
        let _operation = store::acquire(&directory, "operation.lock", name)?;
        let retained = generation::read(&directory)?;
        if retained.root_kind
            != (RootKind::Private {
                spec: expected.clone(),
            })
        {
            return Err(Error::Invalid(
                "startup receipt generation differs from the selected parent and dataspace".into(),
            ));
        }
        store::validate_prepared(
            name,
            directory.path(),
            &retained.prepared,
            &retained.root_kind,
        )?;
        if !store::runtime_owned(&directory)? {
            return Err(Error::Invalid(
                "startup receipt has no live native process owner".into(),
            ));
        }
        let worker: WorkerRecord = decode(&directory.read(WORKER, MAX_METADATA)?)?;
        let receipt: ManagedStartupReceipt = transport::request_as(
            &directory,
            &ControlRequest {
                token: worker.token,
                action: format!("startup_receipt:{challenge}"),
            },
        )?;
        directory.revalidate()?;
        let current = generation::read(&directory)?;
        if encode(&current)? != encode(&retained)?
            || !store::runtime_owned(&directory)?
            || receipt.challenge != challenge
            || receipt.context != retained.prepared.context
            || receipt.private_root != *expected
            || receipt.store_root != self.root()
            || receipt.generation_path != directory.path().join(generation::DIRECTORY)
            || receipt.root_scope != expected.scope()
            || receipt.peers.len() != 4
        {
            return Err(Error::Invalid(
                "startup receipt changed its exact native generation or live owner".into(),
            ));
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_fs::PublishMode;
    use std::{fs::OpenOptions, io::Write};

    const LAUNCH_NAMES: [&str; 8] = [
        "peer0.toml",
        "peer1.toml",
        "peer2.toml",
        "peer3.toml",
        MANIFEST,
        "private-root-prepared.json",
        "genesis.json",
        "genesis.signed.nrt",
    ];

    // Public synthetic file-custody controls. These bytes are neither a prepared genesis nor
    // validator credentials; this fixture exercises only the original launch snapshot owner.
    fn snapshot_fixture() -> (tempfile::TempDir, PrivateDirectory, LaunchSnapshot) {
        let temporary = tempfile::tempdir().unwrap();
        let directory =
            PrivateDirectory::open_or_create(temporary.path().join("generation")).unwrap();
        for name in LAUNCH_NAMES {
            directory
                .write_atomic(name, b"original public test bytes", PublishMode::CreateNew)
                .unwrap();
        }
        let peers = (0..4)
            .map(|index| ManagedPeer {
                config_path: directory.path().join(format!("peer{index}.toml")),
                torii_url: format!("http://127.0.0.1:{}/", 19080 + index),
                log_name: format!("peer{index}.log"),
            })
            .collect::<Vec<_>>();
        let snapshot = LaunchSnapshot::retain_sources(
            PrivateDirectory::open(directory.path()).unwrap(),
            &peers,
        )
        .unwrap();
        snapshot.validate().unwrap();
        (temporary, directory, snapshot)
    }

    #[test]
    fn startup_challenge_is_exact_and_cannot_be_a_status_marker() {
        assert!(validate_challenge(&"a".repeat(64)).is_ok());
        for invalid in [
            "".to_owned(),
            "ready".into(),
            "0".repeat(64),
            "A".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
        ] {
            assert!(validate_challenge(&invalid).is_err());
        }
    }

    #[test]
    fn original_launch_sources_have_bounded_exact_streamed_pins() {
        let _resources = super::super::native_test_guard();
        let (_temporary, directory, snapshot) = snapshot_fixture();
        for index in 0..4 {
            let config = snapshot.config(index).unwrap();
            assert_eq!(
                config.path,
                directory.path().join(format!("peer{index}.toml"))
            );
            assert_eq!(config.size, b"original public test bytes".len() as u64);
            assert_eq!(
                config.blake3,
                blake3::hash(b"original public test bytes")
                    .to_hex()
                    .to_string()
            );
        }
        assert!(snapshot.config(4).is_err());
        let bytes = vec![0x5a; 64 * 1024 + 17];
        directory
            .write_atomic("streamed", &bytes, PublishMode::CreateNew)
            .unwrap();
        let streamed = LaunchFile::retain(&directory, "streamed", bytes.len()).unwrap();
        assert_eq!(
            streamed.pin.blake3,
            blake3::hash(&bytes).to_hex().to_string()
        );
        streamed.validate().unwrap();
        assert!(LaunchFile::retain(&directory, "streamed", bytes.len() - 1).is_err());
        directory
            .write_atomic("empty", b"", PublishMode::CreateNew)
            .unwrap();
        assert!(LaunchFile::retain(&directory, "empty", 1).is_err());
    }

    #[test]
    fn every_original_launch_config_and_manifest_rejects_in_place_edits() {
        let _resources = super::super::native_test_guard();
        for name in LAUNCH_NAMES {
            let (_temporary, directory, snapshot) = snapshot_fixture();
            let path = directory.path().join(name);
            let mut writer = OpenOptions::new().write(true).open(&path).unwrap();
            writer.write_all(b"X").unwrap();
            writer.sync_all().unwrap();
            assert!(snapshot.validate().is_err(), "same-size mutation: {name}");
            if let Some(index) = LAUNCH_NAMES[..4]
                .iter()
                .position(|candidate| *candidate == name)
            {
                assert!(snapshot.config(index).is_err());
            }
        }
        for bytes in [
            b"".as_slice(),
            b"original public test bytes with growth".as_slice(),
        ] {
            let (_temporary, directory, snapshot) = snapshot_fixture();
            let mut writer = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(directory.path().join("peer0.toml"))
                .unwrap();
            writer.write_all(bytes).unwrap();
            writer.sync_all().unwrap();
            assert!(snapshot.validate().is_err());
        }
    }

    #[test]
    fn every_original_launch_config_and_manifest_rejects_equal_bytes_replacement() {
        let _resources = super::super::native_test_guard();
        for name in LAUNCH_NAMES {
            let (_temporary, directory, snapshot) = snapshot_fixture();
            directory
                .write_atomic(name, b"original public test bytes", PublishMode::Replace)
                .unwrap();
            assert!(
                snapshot.validate().is_err(),
                "new inode with equal bytes: {name}"
            );
        }
    }

    #[test]
    fn receipt_child_argv_binds_original_config_and_digest() {
        let original = vec![
            "/native/iroha3d".into(),
            "--sora".into(),
            "--config".into(),
            "/private/peer0.toml".into(),
            "--config-blake3".into(),
            "a".repeat(64),
        ];
        validate_child_argv(&original, &original).unwrap();
        let mut fresh = original.clone();
        fresh.push("--sumeragi-assert-fresh-key".into());
        validate_child_argv(&fresh, &original).unwrap();
        for index in [0, 3, 5] {
            let mut mutated = original.clone();
            mutated[index].push('x');
            assert!(validate_child_argv(&mutated, &original).is_err());
        }
        fresh.push("--sumeragi-assert-fresh-key".into());
        assert!(validate_child_argv(&fresh, &original).is_err());
    }
}
