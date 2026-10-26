//! Native network discovery and durable identity pins for developer-network bootstrap.
//!
//! An installed root supplies initial discovery over HTTPS or explicit numeric-loopback HTTP.
//! Signed genesis and native consensus proofs
//! authenticate its checkpoint; owner-private custody retains the exact network and prevents a
//! later response from replacing it. No separately signed publication or expiring release is used.
//! A retained checkpoint proves its recorded decision, not that the parent is currently reachable.

use std::{fs::File, path::Path, sync::Mutex};

use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint},
};
use iroha_fs::{FileIdentity, OwnerDirectory, PrivateDirectory, PublishMode};
use norito::{Decode, Encode};

use crate::verify::finality::FinalityVerifier;

mod runtime;
pub use runtime::ParentFinalityStore;
mod amx_sources;
pub(crate) use amx_sources::{AmxSourceSelection, ParentBootstrapSources};
mod native;
pub use native::{NativeBootstrap, discover_native_bootstrap};
mod profile;
mod transport;
pub use profile::{
    InstalledNetworkProfile, InstalledNetworkProfiles, MAX_INSTALLED_NETWORK_PROFILES,
    MAX_INSTALLED_PROFILE_BYTES, NETWORK_PROFILES_FILENAME,
};
pub use transport::{CheckpointReadError, CheckpointTransport};
mod provisioning;
pub use provisioning::{BootstrapPeer, BuildRegistryPolicy, FaucetPolicy};
mod registry;

/// Maximum canonical retained native checkpoint and its bounded discovery metadata.
pub const MAX_BOOTSTRAP_BYTES: usize = MAX_FINALITY_CHECKPOINT_BYTES + 160 * 1024;

/// Failure to authenticate or durably retain a selected network.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// Native private-file custody or publication failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Another live operation holds bootstrap custody; retry within the existing deadline.
    #[error("network bootstrap custody is already in use")]
    Busy,
    /// Bounded input, network identity or monotonicity validation failed.
    #[error("network bootstrap: {0}")]
    Invalid(&'static str),
    /// The checkpoint failed native consensus verification.
    #[error(transparent)]
    Finality(#[from] crate::verify::finality::FinalityError),
}

type Result<T> = std::result::Result<T, BootstrapError>;

/// Exact discovered network and retained client policy, separate from consensus authority.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::NetworkMetadataV1")]
pub struct NetworkMetadata {
    /// Installed human-facing selection, such as `taira`.
    pub network_name: String,
    /// Genesis-derived network identity, retained after initial discovery.
    pub network_id: NetworkId,
    /// Consensus chain label authenticated with the selected genesis.
    pub chain_id: String,
    /// Installed account-address prefix checked against public discovery.
    pub account_chain_discriminant: u16,
    /// Compiled native World schema for typed current-state proof interpretation.
    pub native_world_schema: Hash,
    /// Selected credential-free roots for native reads and operations.
    pub torii_roots: Vec<String>,
    /// Optional exact validator endpoint bindings; empty for native public-root discovery.
    pub peers: Vec<BootstrapPeer>,
    /// Retained testnet faucet terms and finite client spending allowances.
    pub faucet: Option<FaucetPolicy>,
    /// Explicit public build registry; absence disables remote resolution.
    pub build_registry: Option<BuildRegistryPolicy>,
}

/// Network identity and a checkpoint authenticated by native consensus proofs.
#[derive(Debug)]
pub struct AuthenticatedBootstrap {
    metadata: NetworkMetadata,
    verifier: FinalityVerifier,
}

impl AuthenticatedBootstrap {
    /// Exact selected network, endpoints and client policy.
    pub fn metadata(&self) -> &NetworkMetadata {
        &self.metadata
    }

    /// Bind native HTTP clients to the selected network's retained endpoints.
    /// Endpoint hints cannot grant committee membership, and credentials are not retargeted.
    ///
    /// # Errors
    /// Unselected endpoint, inconsistent network or scope, invalid peers, or expired deadline.
    pub fn http_source(
        &self,
        proof_clients: Vec<iroha::client::Client>,
        peer_clients: Vec<(iroha_model_base::peer::PeerId, iroha::client::Client)>,
        deadline: std::time::Instant,
    ) -> std::result::Result<
        crate::verify::http::HttpFinalitySource,
        crate::verify::http::HttpFinalityError,
    > {
        for client in proof_clients
            .iter()
            .chain(peer_clients.iter().map(|(_, client)| client))
        {
            if !self
                .metadata
                .torii_roots
                .iter()
                .any(|root| root == client.endpoint().as_str())
            {
                return Err(crate::verify::http::HttpFinalityError::Invalid(
                    "endpoint absent from selected network",
                ));
            }
        }
        crate::verify::http::HttpFinalitySource::new(
            self.metadata.network_id,
            std::num::NonZeroU64::new(self.verifier.checkpoint().height()).ok_or(
                crate::verify::http::HttpFinalityError::Invalid("checkpoint height"),
            )?,
            proof_clients,
            peer_clients,
            deadline,
            self.verifier.root_scope().map_err(|_| {
                crate::verify::http::HttpFinalityError::Invalid("authenticated checkpoint scope")
            })?,
        )
    }

    /// Consume the bootstrap and continue native finality verification.
    pub fn into_verifier(self) -> FinalityVerifier {
        self.verifier
    }
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::RetainedBootstrapV1")]
struct RetainedBootstrap {
    profile: Vec<u8>,
    metadata: NetworkMetadata,
    checkpoint: Vec<u8>,
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::BootstrapRecordV1")]
struct BootstrapRecord {
    accepted: Option<RetainedBootstrap>,
}

/// Exclusive owner-private custody of one installed network's identity and verified checkpoint.
/// Keep this directory outside resettable developer generations.
pub struct BootstrapStore {
    directory: PrivateDirectory,
    lock: File,
    state: Mutex<StoreState>,
}

struct StoreState {
    publication_uncertain: bool,
}

impl BootstrapStore {
    /// Open or atomically initialize owner-private custody and acquire its exclusive lock.
    ///
    /// # Errors
    /// Unsafe or incomplete custody, another owner, or filesystem durability failure.
    pub fn open(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .ok_or(BootstrapError::Invalid("bootstrap custody path"))?;
        let parent = OwnerDirectory::open_or_create(
            path.parent()
                .ok_or(BootstrapError::Invalid("bootstrap custody path"))?,
        )?;
        // Publish the empty record with its lock. Lost accepted custody must never look like an
        // uninitialized store merely because one file is absent after a crash or replacement.
        let initial = norito::encode_canonical(&BootstrapRecord { accepted: None })
            .map_err(|_| BootstrapError::Invalid("cannot initialize bootstrap custody"))?;
        let directory = match parent
            .publish_private_child(name, &[("bootstrap.lock", &[]), ("accepted.nrt", &initial)])
        {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                PrivateDirectory::open(parent.path().join(name))?
            }
            Err(error) => return Err(error.into()),
        };
        let lock = directory.open_existing_lock("bootstrap.lock")?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => BootstrapError::Busy,
            std::fs::TryLockError::Error(error) => BootstrapError::Io(error),
        })?;
        lock.sync_all()?;
        directory.sync()?;
        let store = Self {
            directory,
            lock,
            state: Mutex::new(StoreState {
                publication_uncertain: false,
            }),
        };
        store.read_record()?;
        Ok(store)
    }

    /// Load a previously verified checkpoint for the exact installed profile without network I/O.
    /// `None` means this store has never accepted a network, not that accepted state was lost.
    ///
    /// # Errors
    /// Changed profile, malformed checkpoint, unsafe or uncertain custody, or missing state.
    pub fn load(
        &self,
        profile: &InstalledNetworkProfile,
    ) -> Result<Option<AuthenticatedBootstrap>> {
        let state = self
            .state
            .lock()
            .map_err(|_| BootstrapError::Invalid("bootstrap custody update was interrupted"))?;
        self.check_custody(&state)?;
        self.read_record()?
            .accepted
            .map(|retained| authenticate_retained(profile, retained))
            .transpose()
    }

    /// Retain native discovery after exact profile, identity and monotonic checkpoint checks.
    /// A changed genesis never replaces an existing context, even if the public endpoint resets.
    ///
    /// # Errors
    /// Inconsistent discovery, changed identity or policy, regressed/equivocating checkpoint,
    /// invalid native verification, unsafe custody, or durable publication failure.
    pub fn accept(
        &self,
        profile: &InstalledNetworkProfile,
        native: NativeBootstrap,
    ) -> Result<AuthenticatedBootstrap> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| BootstrapError::Invalid("bootstrap custody update was interrupted"))?;
        self.check_custody(&state)?;
        let metadata = native_metadata(profile, &native)?;
        let checkpoint = native
            .verifier
            .checkpoint()
            .encode_canonical()
            .map_err(|_| BootstrapError::Invalid("cannot encode native checkpoint"))?;
        let verifier = verify_checkpoint(&metadata, &checkpoint)?;
        if let Some(previous) = self.read_record()?.accepted {
            let previous = authenticate_retained(profile, previous)?;
            if previous.metadata != metadata {
                return Err(BootstrapError::Invalid(
                    "retained network identity or policy changed",
                ));
            }
            let old = previous.verifier.checkpoint();
            let next = verifier.checkpoint();
            if next.height() < old.height() {
                return Err(BootstrapError::Invalid("native checkpoint regressed"));
            }
            if next.height() == old.height() {
                let trusted = iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier::from_trusted_checkpoint(
                    old,
                    &metadata.network_id,
                    &metadata.chain_id,
                )
                .map_err(crate::verify::finality::FinalityError::from)?;
                trusted
                    .verify_same_decision(old.tip(), next.tip())
                    .map_err(crate::verify::finality::FinalityError::from)?;
                // Alternate exact-quorum certificates can witness the same decision. Keep the
                // original retained witness rather than treating valid signer variation as a fork.
                return Ok(previous);
            }
        }
        self.publish(
            &mut state,
            RetainedBootstrap {
                profile: profile_image(profile)?,
                metadata: metadata.clone(),
                checkpoint,
            },
        )?;
        Ok(AuthenticatedBootstrap { metadata, verifier })
    }

    fn check_custody(&self, state: &StoreState) -> Result<()> {
        if state.publication_uncertain {
            return Err(BootstrapError::Invalid(
                "bootstrap publication uncertain; reopen custody",
            ));
        }
        self.directory.revalidate()?;
        if FileIdentity::of(&self.directory.open_read("bootstrap.lock")?)?
            != FileIdentity::of(&self.lock)?
        {
            return Err(BootstrapError::Invalid(
                "bootstrap custody lock was replaced",
            ));
        }
        Ok(())
    }

    fn read_record(&self) -> Result<BootstrapRecord> {
        decode(
            &self.directory.read("accepted.nrt", MAX_BOOTSTRAP_BYTES)?,
            MAX_BOOTSTRAP_BYTES,
        )
    }

    fn publish(&self, state: &mut StoreState, retained: RetainedBootstrap) -> Result<()> {
        self.check_custody(state)?;
        let encoded = norito::encode_canonical(&BootstrapRecord {
            accepted: Some(retained),
        })
        .map_err(|_| BootstrapError::Invalid("cannot encode retained bootstrap"))?;
        let _: BootstrapRecord = decode(&encoded, MAX_BOOTSTRAP_BYTES)?;
        if let Err(error) =
            self.directory
                .write_atomic("accepted.nrt", &encoded, PublishMode::Replace)
        {
            state.publication_uncertain = true;
            return Err(error.into());
        }
        Ok(())
    }
}

fn profile_image(profile: &InstalledNetworkProfile) -> Result<Vec<u8>> {
    InstalledNetworkProfiles::new(vec![profile.clone()])?.encode_installation()
}

fn native_metadata(
    profile: &InstalledNetworkProfile,
    native: &NativeBootstrap,
) -> Result<NetworkMetadata> {
    if native.network_id != native.verifier.checkpoint().network_id()
        || native.verifier.checkpoint().chain_id() != profile.chain_id()
        || native.capabilities.schema_version != 1
        || native.capabilities.network_id != native.network_id
        || native.capabilities.network_prefix != profile.account_chain_discriminant()
        || native.faucet.schema_version != 1
        || native.faucet.network_id != native.network_id
        || native.faucet.network_prefix != profile.account_chain_discriminant()
    {
        return Err(BootstrapError::Invalid(
            "native discovery differs from selected network",
        ));
    }
    if !native
        .capabilities
        .allowed_signing
        .iter()
        .any(|algorithm| algorithm == "ed25519")
    {
        return Err(BootstrapError::Invalid(
            "selected network does not admit the managed Ed25519 owner",
        ));
    }
    iroha::client::AccountFaucetPolicyV1::try_new(
        native.faucet.authority.clone(),
        native.faucet.asset_definition_id.clone(),
        native.faucet.amount.clone(),
    )
    .map_err(|_| BootstrapError::Invalid("invalid discovered faucet policy"))?;
    let maximum: iroha_primitives::numeric::Quantity = 1000_u64.into();
    let allowance = native.faucet.amount.clone().min(maximum);
    let root = profile.torii_root().to_string();
    let metadata = NetworkMetadata {
        network_name: profile.network_name().into(),
        network_id: native.network_id,
        chain_id: profile.chain_id().into(),
        account_chain_discriminant: profile.account_chain_discriminant(),
        native_world_schema: iroha_core::state::State::native_world_schema_hash_v1()
            .map_err(|_| BootstrapError::Invalid("cannot derive native World schema"))?,
        torii_roots: vec![root.clone()],
        peers: Vec::new(),
        faucet: Some(FaucetPolicy {
            torii_root: root,
            issuer: native.faucet.authority.clone(),
            asset_definition_id: native.faucet.asset_definition_id.clone(),
            amount: native.faucet.amount.clone(),
            max_operation_fee: allowance.clone(),
            max_namespace_rent: allowance,
        }),
        build_registry: None,
    };
    validate_metadata(&metadata)?;
    Ok(metadata)
}

fn authenticate_retained(
    profile: &InstalledNetworkProfile,
    retained: RetainedBootstrap,
) -> Result<AuthenticatedBootstrap> {
    if retained.profile != profile_image(profile)?
        || retained.metadata.network_name != profile.network_name()
        || retained.metadata.chain_id != profile.chain_id()
        || retained.metadata.account_chain_discriminant != profile.account_chain_discriminant()
        || retained.metadata.torii_roots.first().map(String::as_str)
            != Some(profile.torii_root().as_str())
    {
        return Err(BootstrapError::Invalid(
            "retained bootstrap differs from installed profile",
        ));
    }
    let verifier = verify_checkpoint(&retained.metadata, &retained.checkpoint)?;
    Ok(AuthenticatedBootstrap {
        metadata: retained.metadata,
        verifier,
    })
}

fn verify_checkpoint(metadata: &NetworkMetadata, bytes: &[u8]) -> Result<FinalityVerifier> {
    validate_metadata(metadata)?;
    if bytes.is_empty() || bytes.len() > MAX_FINALITY_CHECKPOINT_BYTES {
        return Err(BootstrapError::Invalid(
            "native checkpoint exceeds byte bound",
        ));
    }
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)
        .map_err(|_| BootstrapError::Invalid("invalid canonical native checkpoint"))?;
    if checkpoint.height() < 2 {
        return Err(BootstrapError::Invalid(
            "public parent checkpoint lacks a certified successor",
        ));
    }
    let verifier =
        FinalityVerifier::from_checkpoint(checkpoint, metadata.network_id, &metadata.chain_id)?;
    if verifier.root_scope()? != SumeragiRootScope::Global {
        return Err(BootstrapError::Invalid(
            "public parent requires a committed global root",
        ));
    }
    Ok(verifier)
}

fn decode<T: norito::codec::Decode + norito::NoritoSchema>(
    bytes: &[u8],
    limit: usize,
) -> Result<T> {
    if bytes.is_empty() || bytes.len() > limit {
        return Err(BootstrapError::Invalid(
            "bootstrap material exceeds byte bound",
        ));
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| BootstrapError::Invalid("bootstrap material is not canonical"))
}

fn validate_network_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || !name.as_bytes()[0].is_ascii_lowercase()
    {
        return Err(BootstrapError::Invalid("invalid installed network name"));
    }
    Ok(())
}

fn validate_metadata(metadata: &NetworkMetadata) -> Result<()> {
    validate_network_name(&metadata.network_name)?;
    metadata
        .chain_id
        .parse::<iroha_model_base::chain::ChainId>()
        .map_err(|_| BootstrapError::Invalid("invalid native chain identity"))?;
    if metadata.torii_roots.is_empty() || metadata.torii_roots.len() > 32 {
        return Err(BootstrapError::Invalid("invalid bounded network endpoints"));
    }
    let mut roots = std::collections::BTreeSet::new();
    for root in &metadata.torii_roots {
        InstalledNetworkProfile::new(
            metadata.network_name.clone(),
            root,
            metadata.chain_id.clone(),
            metadata.account_chain_discriminant,
        )?;
        if !roots.insert(root) {
            return Err(BootstrapError::Invalid("duplicate network endpoint"));
        }
    }
    provisioning::validate(metadata)
}

#[cfg(test)]
pub(crate) mod tests;
