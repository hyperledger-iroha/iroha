//! Fresh observations from the native owner of an exact private generation.
//!
//! The serializable receipt is a public projection, not a transferable process capability.
//! Consumers must invoke the admitted installed launcher with a new independently generated
//! challenge, then authenticate the operating-system processes before and after publication.

use super::*;
use iroha_data_model::{NetworkId, block::consensus::SumeragiRootScope};
use iroha_fs::PrivateDirectory;
use std::path::Path;

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

fn retained_file(
    directory: &PrivateDirectory,
    name: &str,
    maximum: usize,
) -> Result<ManagedStartupFile> {
    let bytes = directory.read(name, maximum)?;
    Ok(ManagedStartupFile {
        path: directory.path().join(name),
        blake3: blake3::hash(&bytes).to_hex().to_string(),
        size: u64::try_from(bytes.len())
            .map_err(|_| Error::Invalid("retained file length overflow".into()))?,
    })
}

fn executable(pin: &BinaryPin) -> Result<ManagedStartupFile> {
    store::verify_binary(pin)?;
    Ok(ManagedStartupFile {
        path: pin.path.clone(),
        blake3: pin.blake3.clone(),
        size: std::fs::metadata(&pin.path)?.len(),
    })
}

pub(super) fn capture(
    store_root: &Path,
    directory: &PrivateDirectory,
    retained: &RetainedLocalnet,
    challenge: &str,
    children: &[(u32, Vec<String>)],
) -> Result<ManagedStartupReceipt> {
    validate_challenge(challenge)?;
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
    let generation = directory.open_child(generation::DIRECTORY)?;
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
            let config = retained_file(&generation, &format!("peer{index}.toml"), MAX_METADATA)?;
            if config.path != peer.config_path || *pid <= 1 {
                return Err(Error::Invalid(
                    "live startup child differs from its retained config".into(),
                ));
            }
            Ok(ManagedStartupPeer {
                index,
                pid: *pid,
                argv: argv.clone(),
                config,
                torii_url: peer.torii_url.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ManagedStartupReceipt {
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
        generation_manifest: retained_file(&generation, MANIFEST, MAX_METADATA)?,
        private_root_manifest: retained_file(
            &generation,
            "private-root-prepared.json",
            MAX_METADATA,
        )?,
        genesis_manifest: retained_file(
            &generation,
            "genesis.json",
            iroha_genesis::GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
        )?,
        signed_genesis: retained_file(
            &generation,
            "genesis.signed.nrt",
            iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1,
        )?,
        launcher: executable(&retained.launcher)?,
        daemon: executable(&retained.daemon)?,
        peers,
    })
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
}
