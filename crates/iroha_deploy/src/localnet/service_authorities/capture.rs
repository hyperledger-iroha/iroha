//! Complete original service bytes and native directory custody; no current-state authority.

use super::*;
use iroha_config::parameters::{actual::NodeSecretFile, defaults::data_dir as layout};
use iroha_config_base::file_source::{ConfigFileAccess, ConfigFileRequest, ConfigFileSource};
use iroha_fs::{PrivateDirectory, PrivateFileComparison, PrivateReadScope, PrivateReadTreeScope};
use std::{
    ffi::{OsStr, OsString},
    io,
};

const MAX_CONFIG: usize = 1024 * 1024;
const MAX_IDENTITY: usize = 512;
const MAX_RANS: usize = 64 * 1024;
const MAX_TOTAL: usize = 70 * 1024 * 1024;
const INPUT_COUNT: usize = 87;
const DIRECTORY_COUNT: usize = 20;
const GENERATION: usize = 0;
const RUNTIME: usize = 1;
const AUTHORITY: usize = 2;
const NETWORK: usize = 3;
const PROVIDERS: usize = 4;
const FIRST_PROVIDER: usize = 5;
const CODEC: usize = 8;
const RANS: usize = 9;
const TABLES: usize = 10;
const NODES: usize = 11;
/// First validator's node data directory; each validator adds that directory and its `secrets`.
const FIRST_NODE: usize = 12;
/// The four original validators, whose configurations and keys the image holds.
const PEER_COUNT: usize = 4;
/// Key files a validator configuration reads from its `<data_dir>/secrets/`.
const NODE_KEY_FILES: [NodeSecretFile; 3] = [
    NodeSecretFile::Validator,
    NodeSecretFile::Transport,
    NodeSecretFile::Streaming,
];

struct Input {
    name: &'static str,
    maximum: usize,
    access: ConfigFileAccess,
    bytes: Zeroizing<Vec<u8>>,
}

/// One original directory with complete fixed input bytes, never retained file handles.
pub(super) struct CapturedDirectory {
    directory: PrivateDirectory,
    inputs: Vec<Input>,
    inventory: Option<Vec<OsString>>,
}

impl CapturedDirectory {
    fn new(
        directory: PrivateDirectory,
        inventory: Option<Vec<OsString>>,
    ) -> crate::managed::Result<Self> {
        let value = Self {
            directory,
            inputs: Vec::new(),
            inventory,
        };
        value.revalidate()?;
        Ok(value)
    }

    fn capture(
        &mut self,
        name: &'static str,
        maximum: usize,
        access: ConfigFileAccess,
        total: &mut usize,
    ) -> crate::managed::Result<()> {
        let bytes = self.directory.read(name, maximum)?;
        *total = total
            .checked_add(bytes.len())
            .filter(|value| *value <= MAX_TOTAL)
            .ok_or_else(invalid)?;
        if self.inputs.iter().any(|input| input.name == name) {
            return Err(invalid());
        }
        self.inputs.push(Input {
            name,
            maximum,
            access,
            bytes,
        });
        Ok(())
    }

    pub(super) fn read(&self, name: &str, maximum: usize) -> crate::managed::Result<&[u8]> {
        let input = self
            .inputs
            .iter()
            .find(|input| input.name == name)
            .ok_or_else(invalid)?;
        if input.bytes.len() > maximum {
            return Err(invalid());
        }
        Ok(&input.bytes)
    }

    pub(super) fn revalidate(&self) -> crate::managed::Result<()> {
        // The canonical inventory owner checks custody before and after enumeration,
        // then once more after comparing the complete expected names.
        self.inventory.as_ref().map_or_else(
            || self.directory.revalidate().map_err(Into::into),
            |expected| require_entries(&self.directory, expected.iter().cloned()),
        )
    }

    fn check_inventory(&self, reader: &PrivateReadScope<'_>) -> crate::managed::Result<()> {
        if let Some(expected) = &self.inventory {
            let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
            if reader
                .entries(expected.len())?
                .into_iter()
                .collect::<BTreeSet<_>>()
                != expected
            {
                return Err(Error::Invalid(
                    "original service directory inventory differs".into(),
                ));
            }
        }
        Ok(())
    }

    // Borrow the same fixed bounded metadata for both callers. The canonical private leaf
    // reader owns its single zeroized buffer; exact equality is lazy and stops at the first
    // mismatch. Empty directories remain owned solely by the surrounding inventory passes.
    fn compare_inputs_with(
        &self,
        compare: impl FnOnce(&[PrivateFileComparison<'_>]) -> io::Result<bool>,
    ) -> crate::managed::Result<()> {
        if self.inputs.is_empty() {
            return Ok(());
        }
        let mut comparisons = [PrivateFileComparison {
            name: OsStr::new(""),
            maximum: 0,
            expected: &[],
        }; INPUT_COUNT];
        let selected = comparisons
            .get_mut(..self.inputs.len())
            .ok_or_else(invalid)?;
        for (comparison, input) in selected.iter_mut().zip(&self.inputs) {
            *comparison = PrivateFileComparison {
                name: OsStr::new(input.name),
                maximum: input.maximum,
                expected: input.bytes.as_slice(),
            };
        }
        if !compare(selected)? {
            return Err(invalid());
        }
        Ok(())
    }
}

impl ServiceKeySource for CapturedDirectory {
    fn service_key(&self, name: &str) -> crate::managed::Result<KeyPair> {
        let key = decode_service_private_key(self.read(name, MAX_ROLE_CREDENTIAL_BYTES)?)?;
        self.revalidate()?;
        Ok(key)
    }
}

/// The twenty original links share native ancestry, with only one transient file read at a time.
///
/// Peer configurations name no private key: each reads its validator, `SoraNet` transport and
/// streaming keys from the fixed files of its `data_dir` (`nodes/peerN/secrets/`). Those twelve
/// files are therefore inputs of the image, so the canonical parser resolves every key from the
/// original captured bytes and never from the disk.
pub(super) struct CapturedProfile {
    directories: Vec<CapturedDirectory>,
}

fn invalid() -> Error {
    Error::Invalid("retained service profile input custody differs".into())
}

impl CapturedProfile {
    pub(super) fn open(generation: PrivateDirectory) -> crate::managed::Result<Self> {
        let mut directories = Vec::with_capacity(DIRECTORY_COUNT);
        directories.push(CapturedDirectory::new(generation, None)?);
        let mut append = |parent: usize,
                          name: &str,
                          inventory: Option<Vec<OsString>>|
         -> crate::managed::Result<()> {
            let directory = directories[parent].directory.open_child(name)?;
            directories.push(CapturedDirectory::new(directory, inventory)?);
            Ok(())
        };
        append(GENERATION, RUNTIME_DIRECTORY, None)?;
        append(
            RUNTIME,
            DIRECTORY,
            Some(
                [MANIFEST, NETWORK_DIRECTORY, PROVIDERS_DIRECTORY]
                    .map(OsString::from)
                    .into(),
            ),
        )?;
        append(
            AUTHORITY,
            NETWORK_DIRECTORY,
            Some(
                NETWORK_ROLES
                    .into_iter()
                    .map(super::NetworkServiceAuthorityRole::credential_filename)
                    .chain(network_material::COUNCIL_KEYS)
                    .map(OsString::from)
                    .collect(),
            ),
        )?;
        append(
            AUTHORITY,
            PROVIDERS_DIRECTORY,
            Some(
                (0..PROVIDER_COUNT)
                    .map(|slot| OsString::from(slot.to_string()))
                    .collect(),
            ),
        )?;
        for slot in 0..PROVIDER_COUNT {
            append(
                PROVIDERS,
                &slot.to_string(),
                Some(
                    ROLES
                        .into_iter()
                        .map(super::StreamTokenAuthorityRole::credential_filename)
                        .chain(provider_material::filenames())
                        .chain(compliance_material::filenames())
                        .map(OsString::from)
                        .collect(),
                ),
            )?;
        }
        append(GENERATION, "codec", None)?;
        append(CODEC, "rans", None)?;
        append(RANS, "tables", None)?;
        // A validator writes its state beside `secrets`, so only the key directory is closed.
        append(GENERATION, super::super::config::NODES_DIRECTORY, None)?;
        for index in 0..PEER_COUNT {
            let node = FIRST_NODE + 2 * index;
            append(NODES, &format!("peer{index}"), None)?;
            append(
                node,
                layout::SECRETS_DIR,
                Some(
                    NODE_KEY_FILES
                        .into_iter()
                        .map(|file| OsString::from(file.relative_path()))
                        .collect(),
                ),
            )?;
        }
        let mut total = 0;
        directories[GENERATION].capture(
            "genesis.signed.nrt",
            SIGNED_GENESIS_MAX_BYTES_V1,
            ConfigFileAccess::Public,
            &mut total,
        )?;
        for name in [
            "client.toml",
            "peer0.toml",
            "peer1.toml",
            "peer2.toml",
            "peer3.toml",
        ] {
            directories[GENERATION].capture(
                name,
                MAX_CONFIG,
                ConfigFileAccess::Private,
                &mut total,
            )?;
        }
        directories[GENERATION].capture(
            "genesis.expected_hash",
            MAX_IDENTITY,
            ConfigFileAccess::Public,
            &mut total,
        )?;
        directories[AUTHORITY].capture(
            MANIFEST,
            MAX_MANIFEST,
            ConfigFileAccess::Public,
            &mut total,
        )?;
        for name in NETWORK_ROLES
            .into_iter()
            .map(super::NetworkServiceAuthorityRole::credential_filename)
            .chain(network_material::COUNCIL_KEYS)
        {
            directories[NETWORK].capture(
                name,
                MAX_ROLE_CREDENTIAL_BYTES,
                ConfigFileAccess::Private,
                &mut total,
            )?;
        }
        for slot in 0..PROVIDER_COUNT {
            for name in ROLES
                .into_iter()
                .map(super::StreamTokenAuthorityRole::credential_filename)
                .chain(provider_material::credential_filenames())
                .chain(compliance_material::filenames())
            {
                directories[FIRST_PROVIDER + slot].capture(
                    name,
                    MAX_ROLE_CREDENTIAL_BYTES,
                    ConfigFileAccess::Private,
                    &mut total,
                )?;
            }
            for name in provider_material::tls_filenames() {
                // All TLS material remains under native private custody, including public DER.
                directories[FIRST_PROVIDER + slot].capture(
                    name,
                    provider_material::MAX_TLS_BYTES,
                    ConfigFileAccess::Private,
                    &mut total,
                )?;
            }
        }
        directories[TABLES].capture(
            "rans_seed0.toml",
            MAX_RANS,
            ConfigFileAccess::Public,
            &mut total,
        )?;
        for index in 0..PEER_COUNT {
            for file in NODE_KEY_FILES {
                directories[FIRST_NODE + 2 * index + 1].capture(
                    file.relative_path(),
                    MAX_ROLE_CREDENTIAL_BYTES,
                    ConfigFileAccess::Private,
                    &mut total,
                )?;
            }
        }
        if directories.len() != DIRECTORY_COUNT
            || directories
                .iter()
                .map(|entry| entry.inputs.len())
                .sum::<usize>()
                != INPUT_COUNT
        {
            return Err(invalid());
        }
        let value = Self { directories };
        value.revalidate()?;
        Ok(value)
    }

    pub(super) fn generation(&self) -> &CapturedDirectory {
        &self.directories[GENERATION]
    }
    pub(super) fn runtime(&self) -> &PrivateDirectory {
        &self.directories[RUNTIME].directory
    }
    pub(super) fn authority(&self) -> &CapturedDirectory {
        &self.directories[AUTHORITY]
    }
    pub(super) fn network(&self) -> &CapturedDirectory {
        &self.directories[NETWORK]
    }
    pub(super) fn provider(&self, slot: u8) -> crate::managed::Result<&CapturedDirectory> {
        if usize::from(slot) >= PROVIDER_COUNT {
            return Err(invalid());
        }
        Ok(&self.directories[FIRST_PROVIDER + usize::from(slot)])
    }

    pub(super) fn revalidate(&self) -> crate::managed::Result<()> {
        #[cfg(test)]
        IMAGE_REVALIDATIONS.with(|value| {
            if let Some(count) = value.get() {
                value.set(Some(count.checked_add(1).expect("profile image count")));
            }
        });
        let (generation, descendants) = self.directories.split_first().ok_or_else(invalid)?;
        if self.directories.len() > DIRECTORY_COUNT {
            return Err(invalid());
        }
        generation
            .directory
            .read_tree_scope_with_root(|tree, reader| {
                Self::revalidate_directory(generation, descendants, tree, reader, &mut || Ok(()))
            })
    }

    // Open in captured order and keep the original native custody of each directory
    // until its final inventory. At the last entry, run the bounded chain of file
    // comparisons in that same order, then close inventories in reverse. Every native
    // scope rechecks its directory on ordinary error as well as success. A change wholly
    // restored inside a scope may be unobserved. No reader survives this profile pass.
    fn revalidate_directory(
        directory: &CapturedDirectory,
        remaining: &[CapturedDirectory],
        tree: &mut PrivateReadTreeScope<'_>,
        reader: &mut PrivateReadScope<'_>,
        preceding_inputs: &mut dyn FnMut() -> crate::managed::Result<()>,
    ) -> crate::managed::Result<()> {
        directory.check_inventory(reader)?;
        let mut compare_inputs = || {
            preceding_inputs()?;
            directory.compare_inputs_with(|inputs| reader.compare_files(inputs))
        };
        if let Some((next, rest)) = remaining.split_first() {
            tree.read_scope_with_tree(&next.directory, |tree, reader| {
                Self::revalidate_directory(next, rest, tree, reader, &mut compare_inputs)
            })?;
        } else {
            compare_inputs()?;
        }
        directory.check_inventory(reader)
    }

    /// Reject external inheritance/profile resolution before the general SDK byte loader.
    pub(super) fn client_bytes(&self) -> crate::managed::Result<&[u8]> {
        let bytes = self.generation().read("client.toml", MAX_CONFIG)?;
        drop(flat_table(bytes)?);
        Ok(bytes)
    }

    pub(super) fn parse_peer(
        &self,
        path: &Path,
        name: &str,
    ) -> crate::managed::Result<actual::Root> {
        let bytes = self.generation().read(name, MAX_CONFIG)?;
        let mut table = flat_table(bytes)?;
        let discriminant = table
            .get("chain_discriminant")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u16::try_from(value).ok());
        let _profile = discriminant.map(ChainDiscriminantGuard::enter);
        let node = iroha_config::node_config::open_node_config(
            iroha_config::node_config::NodeFile::Verified {
                path: path.to_owned(),
                table: std::mem::take(&mut *table),
            },
            iroha_config::node_config::NodeConfigOptions::default(),
        )
        .map_err(|_| invalid())?;
        let (user, _) = node.read().map_err(|_| invalid())?;
        let config = user.parse_with_file_source(self).map_err(|_| invalid())?;
        // The generated service profile has no private lane manifests. The later canonical
        // execution-policy owner otherwise scans these directories outside this captured image.
        if config.nexus.registry.manifest_directory.is_some()
            || config.nexus.registry.cache_directory.is_some()
        {
            return Err(invalid());
        }
        Ok(config)
    }
}

// Generated originals are flat. The canonical node/SDK owners still parse their exact bytes;
// this closed preflight prevents either general loader from resolving uncaptured profile assets.
fn flat_table(bytes: &[u8]) -> crate::managed::Result<crate::secret_toml::Table> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let table = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(text, "retained service config").map_err(|_| invalid())?,
    );
    if table.contains_key(iroha_config::node_config::PROFILE_KEY) || table.contains_key("extends") {
        return Err(invalid());
    }
    Ok(table)
}

impl ConfigFileSource for CapturedProfile {
    fn read(&self, path: &Path, request: ConfigFileRequest) -> io::Result<Zeroizing<Vec<u8>>> {
        if !path.is_absolute() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        for directory in &self.directories {
            for input in &directory.inputs {
                if path == directory.directory.path().join(input.name) {
                    if request.access != input.access || input.bytes.len() > request.maximum {
                        return Err(io::Error::from(io::ErrorKind::InvalidData));
                    }
                    return Ok(Zeroizing::new(input.bytes.to_vec()));
                }
            }
        }
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
std::thread_local! {
    static SEMANTIC_VALIDATIONS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn record_semantic_validation() {
    SEMANTIC_VALIDATIONS.with(|value| {
        if let Some(count) = value.get() {
            value.set(Some(count.checked_add(1).expect("test validation count")));
        }
    });
}

#[cfg(test)]
pub(super) fn count_semantic_validations<T>(action: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SEMANTIC_VALIDATIONS.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(SEMANTIC_VALIDATIONS.with(|value| value.replace(Some(0))));
    let result = action();
    let count = SEMANTIC_VALIDATIONS.with(|value| value.get().expect("test counter active"));
    (result, count)
}

#[cfg(test)]
std::thread_local! {
    static IMAGE_REVALIDATIONS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn count_revalidations<T>(action: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            IMAGE_REVALIDATIONS.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(IMAGE_REVALIDATIONS.with(|value| value.replace(Some(0))));
    let result = action();
    (
        result,
        IMAGE_REVALIDATIONS.with(|value| value.get().unwrap()),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "capture/batch_comparison_tests.rs"]
mod batch_comparison_tests;

#[cfg(test)]
#[path = "capture/tree_comparison_tests.rs"]
mod tree_comparison_tests;
