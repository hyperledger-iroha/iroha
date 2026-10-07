//! Genuine test-network publisher custody, projected for parsing and bound once to genesis.
//!
//! The public scheme vector supplies only its current relation identity. Fresh P-256 keys
//! and an actual root signature satisfy the production LoadAuthorization keyring validator.
//! This fixture registers no wallet scheme, grants no permission, and mints no money.

#[cfg(unix)]
mod platform {
    use color_eyre::eyre::{Result, eyre};
    use iroha_core::kagemusha_wallet_v1::{
        LOAD_AUTHORIZER_KEYRING_MAX_BYTES, LoadAuthorizerKeyV1, LoadAuthorizerKeyringV1,
        PublicationWorker,
    };
    use iroha_crypto::{ExposedPrivateKey, Hash, HashOf, KeyPair, PrivateKey, sha256};
    use iroha_data_model::{NetworkId, kagemusha::*};
    use iroha_test_samples::ALICE_KEYPAIR;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
    use rand::{TryRngCore as _, rngs::OsRng};
    use std::{
        collections::BTreeSet,
        fmt, fs,
        fs::OpenOptions,
        io::{Read as _, Write as _},
        os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _},
        path::{Path, PathBuf},
        sync::Mutex,
    };
    use zeroize::Zeroizing;

    const PRIVATE_KEY_MAX_BYTES: usize = 4_096;
    const KEYRING_NAME: &str = "keyring.nrt";
    const SUBMITTER_NAME: &str = "submitter.key";
    const PROJECTION_NAME: &str = "pre-genesis";
    const NATIVE_NAME: &str = "native";

    /// One shared fixture owner. NetworkPeer clones retain the same Arc<Fixture>.
    /// The pre-genesis files are parser inputs only; runtime admission requires bind.
    pub(crate) struct Fixture {
        root: Directory,
        projection: CustodyFiles,
        projection_network: NetworkId,
        native_paths: (PathBuf, PathBuf),
        root_key: SigningKey,
        load_key: SigningKey,
        relation_id: [u8; 32],
        state: Mutex<Binding>,
    }

    struct Binding {
        native: Option<(NetworkId, CustodyFiles)>,
        refused: bool,
    }

    impl fmt::Debug for Fixture {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut debug = formatter.debug_struct("LoadAuthorizerFixture");
            match self.state.try_lock() {
                Ok(state) => {
                    debug.field("runtime_bound", &state.native.is_some());
                    debug.field("custody_refused", &state.refused);
                }
                Err(_) => {
                    debug.field("state", &"unavailable");
                }
            }
            debug.finish_non_exhaustive()
        }
    }

    impl Fixture {
        /// Create genuine private parser inputs exactly once, without runtime admission.
        pub(crate) fn new(peer_dir: &Path) -> Result<Self> {
            let peer_metadata = fs::symlink_metadata(peer_dir)?;
            if !peer_metadata.is_dir()
                || peer_metadata.file_type().is_symlink()
                || peer_metadata.uid() != nix::unistd::Uid::effective().as_raw()
            {
                return Err(eyre!(
                    "publisher peer directory is not an original owned directory"
                ));
            }
            let peer_dir = peer_dir.canonicalize()?;
            let root = Directory::create(&peer_dir.join("kagemusha-load-authorizer"))?;
            let root_key = new_p256_key()?;
            let load_key = new_p256_key()?;
            let relation_id = fixture_relation_id()?;
            // This domain-separated identity is never accepted by bind/require_runtime. It
            // exists only because canonical Root parsing precedes signed genesis assembly.
            let projection_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new_from_chunks(&[
                    b"iroha-test-network:load-authorizer:parser-only:v1\0",
                    root_key.verifying_key().to_encoded_point(false).as_bytes(),
                ]),
            ));
            let keyring = keyring_bytes(&root_key, &load_key, relation_id, projection_network)?;
            let projection = CustodyFiles::create(&root.path.join(PROJECTION_NAME), &keyring)?;
            projection.verify(projection_network)?;
            root.require_entries(&[PROJECTION_NAME])?;
            let native_dir = root.path.join(NATIVE_NAME);
            Ok(Self {
                root,
                projection,
                projection_network,
                native_paths: (
                    native_dir.join(KEYRING_NAME),
                    native_dir.join(SUBMITTER_NAME),
                ),
                root_key,
                load_key,
                relation_id,
                state: Mutex::new(Binding {
                    native: None,
                    refused: false,
                }),
            })
        }

        /// Bind once to the exact signed genesis. Retrying the same binding verifies originals.
        /// Partial, changed or unsafe custody is refused permanently; it is never regenerated.
        pub(crate) fn bind(&self, network: NetworkId) -> Result<()> {
            let mut state = self
                .state
                .lock()
                .map_err(|_| eyre!("publisher fixture owner poisoned"))?;
            if state.refused {
                return Err(eyre!("publisher fixture custody already refused"));
            }
            if network == self.projection_network {
                return Err(eyre!(
                    "parser-only publisher identity cannot become runtime custody"
                ));
            }
            if let Some((original_network, _)) = &state.native {
                if network != *original_network {
                    return Err(eyre!(
                        "publisher fixture is already bound to another network"
                    ));
                }
                let result = self.verify_bound(&state);
                if result.is_err() {
                    state.refused = true;
                }
                return result;
            }
            let result = (|| {
                self.root.require_entries(&[PROJECTION_NAME])?;
                self.projection.verify(self.projection_network)?;
                // Validate the actual new certificate before publishing anything. No original
                // file is rewritten. create_new below refuses an existing/partial native set.
                let bytes =
                    keyring_bytes(&self.root_key, &self.load_key, self.relation_id, network)?;
                let files = CustodyFiles::create(&self.root.path.join(NATIVE_NAME), &bytes)?;
                files.verify(network)?;
                self.root.require_entries(&[PROJECTION_NAME, NATIVE_NAME])?;
                self.root.sync()?;
                Ok(files)
            })();
            match result {
                Ok(files) => {
                    state.native = Some((network, files));
                    Ok(())
                }
                Err(error) => {
                    state.refused = true;
                    Err(error)
                }
            }
        }

        /// Select the generated base-layer paths; explicit caller config overrides remain later.
        pub(crate) fn paths(&self) -> (PathBuf, PathBuf) {
            // A poisoned lock remains a runtime refusal. Reading the retained selection does
            // not clear the poison or grant admission and does not introduce a panic in Debug.
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.native.is_some() {
                self.native_paths.clone()
            } else {
                self.projection.paths()
            }
        }

        /// Require original native custody before a process/storage lifecycle can start.
        pub(crate) fn require_runtime(&self) -> Result<()> {
            let mut state = self
                .state
                .lock()
                .map_err(|_| eyre!("publisher fixture owner poisoned"))?;
            if state.refused {
                return Err(eyre!("publisher fixture custody already refused"));
            }
            let result = self.verify_bound(&state);
            if result.is_err() && state.native.is_some() {
                state.refused = true;
            }
            result
        }

        fn verify_bound(&self, state: &Binding) -> Result<()> {
            let (network, files) = state.native.as_ref().ok_or_else(|| {
                eyre!("publisher fixture has no exact signed-genesis runtime binding")
            })?;
            self.root.require_entries(&[PROJECTION_NAME, NATIVE_NAME])?;
            self.projection.verify(self.projection_network)?;
            files.verify(*network)
        }
    }

    fn new_p256_key() -> Result<SigningKey> {
        let mut scalar = Zeroizing::new([0_u8; 32]);
        for _ in 0..32 {
            OsRng
                .try_fill_bytes(&mut *scalar)
                .map_err(|_| eyre!("publisher entropy unavailable"))?;
            if let Ok(key) = SigningKey::from_slice(&*scalar) {
                return Ok(key);
            }
        }
        Err(eyre!(
            "publisher entropy did not yield a canonical P-256 scalar"
        ))
    }

    fn fixture_relation_id() -> Result<[u8; 32]> {
        // The canonical fixture's relation is useful for startup shape validation, not a
        // claim that these local keys or that relation have qualified production G3 artifacts.
        let value: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))?;
        let rows = value["objects"]
            .as_array()
            .ok_or_else(|| eyre!("wallet vector objects absent"))?;
        let mut schemes = rows
            .iter()
            .filter(|row| row["type"].as_str() == Some("KagemushaWalletSchemeV1"));
        let row = schemes
            .next()
            .ok_or_else(|| eyre!("wallet scheme vector absent"))?;
        if schemes.next().is_some() {
            return Err(eyre!("wallet scheme vector is not unique"));
        }
        let encoded = row["canonical_hex"]
            .as_str()
            .ok_or_else(|| eyre!("wallet scheme bytes absent"))?;
        if encoded.len() % 2 != 0 || encoded.len() > KAGEMUSHA_WALLET_SCHEME_MAX_BYTES_V1 * 2 {
            return Err(eyre!("wallet scheme vector encoding is invalid"));
        }
        let bytes = encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let nibble = |byte: u8| match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => Err(eyre!(
                        "wallet scheme vector encoding is not canonical lowercase hex"
                    )),
                };
                Ok((nibble(pair[0])? << 4) | nibble(pair[1])?)
            })
            .collect::<Result<Vec<u8>>>()?;
        let scheme: KagemushaWalletSchemeV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )?;
        scheme
            .validate()
            .map_err(|_| eyre!("wallet scheme vector fields invalid"))?;
        Ok(scheme.relation_id)
    }

    fn keyring_bytes(
        root: &SigningKey,
        load: &SigningKey,
        relation_id: [u8; 32],
        network: NetworkId,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let public = |key: &SigningKey| {
            KagemushaDevicePublicKeyV1::from_sec1_bytes(
                key.verifying_key().to_encoded_point(false).as_bytes(),
            )
            .map_err(|_| eyre!("publisher public key invalid"))
        };
        let scheme = KagemushaWalletSchemeV1 {
            version: 1,
            network_id: *network.as_bytes(),
            scheme_root_key: public(root)?,
            relation_id,
            provider_contract: kagemusha_wallet_provider_contract_v1(),
        };
        scheme
            .validate()
            .map_err(|_| eyre!("publisher scheme fields invalid"))?;
        let body = KagemushaWalletSignerCertificateBodyV1 {
            version: 1,
            scheme_id: scheme.scheme_id(),
            role: KagemushaWalletSignerRoleV1::LoadAuthorization,
            key: public(load)?,
            serial: 1,
        };
        let signature: Signature = root.sign(&body.signing_message());
        let certificate = KagemushaWalletSignerCertificateV1::sign(
            body,
            &scheme,
            KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
        )
        .map_err(|_| eyre!("publisher root certificate invalid"))?;
        let secret = Zeroizing::new(<[u8; 32]>::from(load.to_bytes()));
        let ring = LoadAuthorizerKeyringV1 {
            version: 1,
            keys: vec![LoadAuthorizerKeyV1 {
                scheme,
                certificate,
                secret: *secret,
            }],
        };
        let bytes = Zeroizing::new(norito::encode_canonical(&ring)?);
        PublicationWorker::from_canonical_keyring(&bytes)
            .map_err(|_| eyre!("publisher keyring admission failed"))?
            .require_network(*network.as_bytes())
            .map_err(|_| eyre!("publisher keyring network admission failed"))?;
        Ok(bytes)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct FileIdentity {
        dev: u64,
        ino: u64,
        uid: u32,
        mode: u32,
        len: u64,
        links: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    }
    impl FileIdentity {
        fn checked(metadata: &fs::Metadata, maximum: usize) -> Result<Self> {
            if !metadata.is_file()
                || metadata.uid() != nix::unistd::Uid::effective().as_raw()
                || metadata.mode() & 0o7777 != 0o600
                || metadata.nlink() != 1
                || metadata.len() == 0
                || metadata.len() > u64::try_from(maximum)?
            {
                return Err(eyre!(
                    "publisher custody file is not private, regular, single-link and bounded"
                ));
            }
            Ok(Self {
                dev: metadata.dev(),
                ino: metadata.ino(),
                uid: metadata.uid(),
                mode: metadata.mode(),
                len: metadata.len(),
                links: metadata.nlink(),
                modified: (metadata.mtime(), metadata.mtime_nsec()),
                changed: (metadata.ctime(), metadata.ctime_nsec()),
            })
        }
    }

    struct StoredFile {
        path: PathBuf,
        original: FileIdentity,
        digest: [u8; 32],
        maximum: usize,
    }
    impl StoredFile {
        fn create(path: PathBuf, bytes: &[u8], maximum: usize) -> Result<Self> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
                .open(&path)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            let stored = Self {
                path,
                original: FileIdentity::checked(&file.metadata()?, maximum)?,
                digest: sha256(bytes),
                maximum,
            };
            stored.read_original()?;
            Ok(stored)
        }

        fn read_original(&self) -> Result<Zeroizing<Vec<u8>>> {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK)
                .open(&self.path)?;
            if FileIdentity::checked(&file.metadata()?, self.maximum)? != self.original
                || FileIdentity::checked(&fs::symlink_metadata(&self.path)?, self.maximum)?
                    != self.original
            {
                return Err(eyre!("publisher custody file identity changed"));
            }
            let mut bytes = Zeroizing::new(Vec::new());
            let bound = u64::try_from(self.maximum)?
                .checked_add(1)
                .ok_or_else(|| eyre!("publisher custody bound overflow"))?;
            (&mut file).take(bound).read_to_end(&mut bytes)?;
            if u64::try_from(bytes.len())? != self.original.len
                || sha256(&bytes) != self.digest
                || FileIdentity::checked(&file.metadata()?, self.maximum)? != self.original
                || FileIdentity::checked(&fs::symlink_metadata(&self.path)?, self.maximum)?
                    != self.original
            {
                return Err(eyre!(
                    "publisher custody contents or identity changed during read"
                ));
            }
            Ok(bytes)
        }
    }

    struct Directory {
        path: PathBuf,
        dev: u64,
        ino: u64,
        uid: u32,
    }
    impl Directory {
        fn create(path: &Path) -> Result<Self> {
            fs::DirBuilder::new().mode(0o700).create(path)?;
            let metadata = Self::checked(path)?;
            Ok(Self {
                path: path.to_path_buf(),
                dev: metadata.dev(),
                ino: metadata.ino(),
                uid: metadata.uid(),
            })
        }
        fn checked(path: &Path) -> Result<fs::Metadata> {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != nix::unistd::Uid::effective().as_raw()
                || metadata.mode() & 0o7777 != 0o700
            {
                return Err(eyre!(
                    "publisher custody directory is not original private custody"
                ));
            }
            Ok(metadata)
        }
        fn require_original(&self) -> Result<()> {
            let metadata = Self::checked(&self.path)?;
            if metadata.dev() != self.dev
                || metadata.ino() != self.ino
                || metadata.uid() != self.uid
            {
                return Err(eyre!("publisher custody directory identity changed"));
            }
            Ok(())
        }
        fn require_entries(&self, names: &[&str]) -> Result<()> {
            self.require_original()?;
            let actual = fs::read_dir(&self.path)?
                .map(|entry| Ok(entry?.file_name()))
                .collect::<Result<BTreeSet<_>>>()?;
            let expected = names
                .iter()
                .map(|name| std::ffi::OsString::from(*name))
                .collect::<BTreeSet<_>>();
            if actual != expected {
                return Err(eyre!(
                    "publisher custody directory is partial or has unexpected files"
                ));
            }
            self.require_original()
        }
        fn sync(&self) -> Result<()> {
            self.require_original()?;
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_DIRECTORY)
                .open(&self.path)?;
            let metadata = file.metadata()?;
            if metadata.dev() != self.dev || metadata.ino() != self.ino {
                return Err(eyre!(
                    "publisher custody directory changed before synchronization"
                ));
            }
            file.sync_all()?;
            self.require_original()
        }
    }

    struct CustodyFiles {
        directory: Directory,
        keyring: StoredFile,
        submitter: StoredFile,
    }
    impl CustodyFiles {
        fn create(path: &Path, bytes: &[u8]) -> Result<Self> {
            let directory = Directory::create(path)?;
            let keyring = StoredFile::create(
                directory.path.join(KEYRING_NAME),
                bytes,
                LOAD_AUTHORIZER_KEYRING_MAX_BYTES,
            )?;
            // ALICE is already the ordinary fixture account. Files alone grant no on-chain
            // LoadAuthorization registration, CanPublish permission, or monetary authority.
            let submitter =
                Zeroizing::new(ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string());
            let submitter = StoredFile::create(
                directory.path.join(SUBMITTER_NAME),
                submitter.as_bytes(),
                PRIVATE_KEY_MAX_BYTES,
            )?;
            directory.require_entries(&[KEYRING_NAME, SUBMITTER_NAME])?;
            directory.sync()?;
            Ok(Self {
                directory,
                keyring,
                submitter,
            })
        }
        fn paths(&self) -> (PathBuf, PathBuf) {
            (self.keyring.path.clone(), self.submitter.path.clone())
        }
        fn verify(&self, network: NetworkId) -> Result<()> {
            self.directory
                .require_entries(&[KEYRING_NAME, SUBMITTER_NAME])?;
            let bytes = self.keyring.read_original()?;
            PublicationWorker::from_canonical_keyring(&bytes)
                .map_err(|_| eyre!("publisher original keyring admission failed"))?
                .require_network(*network.as_bytes())
                .map_err(|_| eyre!("publisher original keyring network mismatch"))?;
            let submitter = self.submitter.read_original()?;
            let encoded = std::str::from_utf8(&submitter)
                .map_err(|_| eyre!("publisher submitter encoding invalid"))?;
            let key: PrivateKey = encoded
                .parse()
                .map_err(|_| eyre!("publisher submitter key invalid"))?;
            let pair = KeyPair::from_private_key(key)
                .map_err(|_| eyre!("publisher submitter public key invalid"))?;
            if pair.public_key() != ALICE_KEYPAIR.public_key() {
                return Err(eyre!(
                    "publisher submitter is not the original ordinary fixture account"
                ));
            }
            self.directory
                .require_entries(&[KEYRING_NAME, SUBMITTER_NAME])
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        use std::sync::{Arc, Barrier};

        fn network(label: &[u8]) -> NetworkId {
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
        }
        fn decode(path: &Path) -> LoadAuthorizerKeyringV1 {
            let bytes = Zeroizing::new(fs::read(path).unwrap());
            norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .unwrap()
        }

        #[test]
        fn parser_projection_is_genuine_but_never_runtime_custody() {
            let peer = tempfile::tempdir().unwrap();
            let fixture = Fixture::new(peer.path()).unwrap();
            assert!(fixture.require_runtime().is_err());
            let paths = fixture.paths();
            assert!(paths.0.starts_with(fixture.root.path.join(PROJECTION_NAME)));
            let ring = decode(&paths.0);
            let worker = PublicationWorker::from_canonical_keyring(&Zeroizing::new(
                fs::read(&paths.0).unwrap(),
            ))
            .unwrap();
            assert!(
                worker
                    .require_network(*fixture.projection_network.as_bytes())
                    .is_ok()
            );
            assert!(
                worker
                    .require_network(*network(b"actual-genesis").as_bytes())
                    .is_err()
            );
            assert_eq!(ring.version, 1);
            assert_eq!(ring.keys.len(), 1);
            assert_eq!(
                ring.keys[0].certificate.body.role,
                KagemushaWalletSignerRoleV1::LoadAuthorization
            );
            assert_eq!(
                ring.keys[0].scheme.provider_contract,
                kagemusha_wallet_provider_contract_v1()
            );
            assert_eq!(
                ring.keys[0].scheme.relation_id,
                fixture_relation_id().unwrap()
            );
            assert!(fixture.bind(fixture.projection_network).is_err());
            assert!(fixture.require_runtime().is_err());
            assert!(!fixture.root.path.join(NATIVE_NAME).exists());
            assert_eq!(
                fs::symlink_metadata(&paths.0).unwrap().mode() & 0o7777,
                0o600
            );
            assert_eq!(
                fs::symlink_metadata(paths.0.parent().unwrap())
                    .unwrap()
                    .mode()
                    & 0o7777,
                0o700
            );
        }

        #[test]
        fn exact_signed_network_binding_preserves_original_files_on_retry_and_restart() {
            let peer = tempfile::tempdir().unwrap();
            let fixture = Fixture::new(peer.path()).unwrap();
            let projection_paths = fixture.paths();
            let projection = decode(&projection_paths.0);
            let real = network(b"exact-signed-genesis");
            fixture.bind(real).unwrap();
            fixture.require_runtime().unwrap();
            let paths = fixture.paths();
            assert_ne!(paths, projection_paths);
            let original_keyring = Zeroizing::new(fs::read(&paths.0).unwrap());
            let original_submitter = Zeroizing::new(fs::read(&paths.1).unwrap());
            let identity = FileIdentity::checked(
                &fs::symlink_metadata(&paths.0).unwrap(),
                LOAD_AUTHORIZER_KEYRING_MAX_BYTES,
            )
            .unwrap();
            let submitter_identity = FileIdentity::checked(
                &fs::symlink_metadata(&paths.1).unwrap(),
                PRIVATE_KEY_MAX_BYTES,
            )
            .unwrap();
            let runtime = decode(&paths.0);
            assert_eq!(runtime.keys[0].scheme.network_id, *real.as_bytes());
            assert_eq!(
                runtime.keys[0].scheme.scheme_root_key,
                projection.keys[0].scheme.scheme_root_key
            );
            assert_eq!(
                runtime.keys[0].certificate.body.key,
                projection.keys[0].certificate.body.key
            );
            assert!(runtime.keys[0].secret == projection.keys[0].secret);
            assert_ne!(
                runtime.keys[0].certificate.body.scheme_id,
                projection.keys[0].certificate.body.scheme_id
            );
            runtime.keys[0]
                .certificate
                .verify_role(
                    &runtime.keys[0].scheme,
                    KagemushaWalletSignerRoleV1::LoadAuthorization,
                )
                .unwrap();
            assert!(
                PublicationWorker::from_canonical_keyring(&original_keyring)
                    .unwrap()
                    .require_network(*real.as_bytes())
                    .is_ok()
            );
            assert!(
                PublicationWorker::from_canonical_keyring(&original_keyring)
                    .unwrap()
                    .require_network(*network(b"foreign-genesis").as_bytes())
                    .is_err()
            );
            for _ in 0..3 {
                fixture.bind(real).unwrap();
                fixture.require_runtime().unwrap();
                assert_eq!(fixture.paths(), paths);
                assert!(*Zeroizing::new(fs::read(&paths.0).unwrap()) == *original_keyring);
                assert!(*Zeroizing::new(fs::read(&paths.1).unwrap()) == *original_submitter);
                assert!(
                    FileIdentity::checked(
                        &fs::symlink_metadata(&paths.0).unwrap(),
                        LOAD_AUTHORIZER_KEYRING_MAX_BYTES
                    )
                    .unwrap()
                        == identity
                );
                assert!(
                    FileIdentity::checked(
                        &fs::symlink_metadata(&paths.1).unwrap(),
                        PRIVATE_KEY_MAX_BYTES
                    )
                    .unwrap()
                        == submitter_identity
                );
            }
            assert!(fixture.bind(network(b"another-genesis")).is_err());
            fixture.require_runtime().unwrap();
            let debug = format!("{fixture:?}");
            assert!(!debug.contains(std::str::from_utf8(&original_submitter).unwrap()));
            assert!(!debug.contains("secret"));
        }

        #[test]
        fn shared_peer_clones_bind_once_under_real_concurrent_retries() {
            let peer = tempfile::tempdir().unwrap();
            let fixture = Arc::new(Fixture::new(peer.path()).unwrap());
            let barrier = Arc::new(Barrier::new(2));
            let clone = Arc::clone(&fixture);
            let other_barrier = Arc::clone(&barrier);
            let real = network(b"shared-signed-genesis");
            let other = std::thread::spawn(move || {
                other_barrier.wait();
                clone.bind(real).unwrap();
                clone.require_runtime().unwrap();
                clone.paths()
            });
            barrier.wait();
            fixture.bind(real).unwrap();
            fixture.require_runtime().unwrap();
            assert_eq!(fixture.paths(), other.join().unwrap());
            assert_eq!(
                fs::read_dir(fixture.root.path.join(NATIVE_NAME))
                    .unwrap()
                    .count(),
                2
            );
        }

        #[test]
        fn changed_removed_replaced_unsafe_and_partial_custody_never_regenerates() {
            for attack in 0..8 {
                let peer = tempfile::tempdir().unwrap();
                let fixture = Fixture::new(peer.path()).unwrap();
                let real = network(b"refused-signed-genesis");
                fixture.bind(real).unwrap();
                let paths = fixture.paths();
                match attack {
                    0 => fs::remove_file(&paths.0).unwrap(),
                    1 => {
                        let bytes = Zeroizing::new(fs::read(&paths.0).unwrap());
                        let renamed = paths.0.with_extension("old");
                        fs::rename(&paths.0, &renamed).unwrap();
                        let mut file = OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o600)
                            .open(&paths.0)
                            .unwrap();
                        file.write_all(&bytes).unwrap();
                        // Identical bytes in a replacement inode are not the original source.
                        fs::remove_file(renamed).unwrap();
                    }
                    2 => fs::set_permissions(&paths.1, fs::Permissions::from_mode(0o640)).unwrap(),
                    3 => fs::write(&paths.0, b"malformed canonical keyring").unwrap(),
                    4 => fs::remove_file(&paths.1).unwrap(),
                    5 => {
                        fs::remove_file(&paths.0).unwrap();
                        symlink(&fixture.projection.keyring.path, &paths.0).unwrap();
                    }
                    6 => fs::set_permissions(
                        paths.0.parent().unwrap(),
                        fs::Permissions::from_mode(0o750),
                    )
                    .unwrap(),
                    7 => fs::write(
                        paths.0.parent().unwrap().join("unexpected"),
                        b"partial foreign custody",
                    )
                    .unwrap(),
                    _ => unreachable!(),
                }
                assert!(fixture.require_runtime().is_err(), "attack {attack}");
                assert!(fixture.bind(real).is_err(), "attack {attack}");
                assert!(fixture.require_runtime().is_err(), "attack {attack}");
                if attack == 0 {
                    assert!(!paths.0.exists());
                }
                if attack == 4 {
                    assert!(!paths.1.exists());
                }
            }
        }

        #[test]
        fn partial_native_publication_and_changed_projection_refuse_before_binding() {
            for attack in 0..4 {
                let peer = tempfile::tempdir().unwrap();
                let fixture = Fixture::new(peer.path()).unwrap();
                let projection = fixture.paths();
                match attack {
                    0 => {
                        Directory::create(&fixture.root.path.join(NATIVE_NAME)).unwrap();
                    }
                    1 => {
                        fs::remove_file(&projection.1).unwrap();
                    }
                    2 => {
                        fs::set_permissions(&projection.0, fs::Permissions::from_mode(0o644))
                            .unwrap();
                    }
                    3 => {
                        let native =
                            Directory::create(&fixture.root.path.join(NATIVE_NAME)).unwrap();
                        StoredFile::create(
                            native.path.join(KEYRING_NAME),
                            &fs::read(&projection.0).unwrap(),
                            LOAD_AUTHORIZER_KEYRING_MAX_BYTES,
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
                let real = network(b"not-admitted-genesis");
                assert!(fixture.bind(real).is_err());
                assert!(fixture.require_runtime().is_err());
                assert_eq!(fixture.paths(), projection);
                assert!(fixture.bind(real).is_err());
            }
        }

        #[test]
        fn production_keyring_validator_rejects_changed_role_scalar_and_root_certificate() {
            let peer = tempfile::tempdir().unwrap();
            let fixture = Fixture::new(peer.path()).unwrap();
            let real = network(b"validator-signed-genesis");
            fixture.bind(real).unwrap();
            for attack in 0..3 {
                let mut ring = decode(&fixture.paths().0);
                match attack {
                    0 => {
                        ring.keys[0].certificate.body.role = KagemushaWalletSignerRoleV1::Enrollment
                    }
                    1 => ring.keys[0].secret = [0; 32],
                    2 => ring.keys[0].certificate.body.serial += 1,
                    _ => unreachable!(),
                }
                let bytes = Zeroizing::new(norito::encode_canonical(&ring).unwrap());
                assert!(PublicationWorker::from_canonical_keyring(&bytes).is_err());
                fixture.require_runtime().unwrap();
            }
        }
    }
}

#[cfg(unix)]
pub(crate) use platform::Fixture;

/// Secure native private-file custody is unavailable on this platform.
#[cfg(not(unix))]
#[derive(Debug)]
pub(crate) struct Fixture {
    paths: (std::path::PathBuf, std::path::PathBuf),
}

#[cfg(not(unix))]
impl Fixture {
    /// Refuse unsupported custody rather than publishing an insecure fixture.
    pub(crate) fn new(_peer_dir: &std::path::Path) -> color_eyre::eyre::Result<Self> {
        Err(color_eyre::eyre::eyre!(
            "secure test publisher custody requires Unix private-file controls"
        ))
    }
    /// No unsupported fixture can acquire a runtime binding.
    pub(crate) fn bind(
        &self,
        _network: iroha_data_model::NetworkId,
    ) -> color_eyre::eyre::Result<()> {
        Err(color_eyre::eyre::eyre!(
            "secure test publisher custody requires Unix private-file controls"
        ))
    }
    /// Preserve the common API; new refuses before an instance or any paths are published.
    pub(crate) fn paths(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        self.paths.clone()
    }
    /// Never admit unsupported custody to runtime startup.
    pub(crate) fn require_runtime(&self) -> color_eyre::eyre::Result<()> {
        Err(color_eyre::eyre::eyre!(
            "secure test publisher custody requires Unix private-file controls"
        ))
    }
}
