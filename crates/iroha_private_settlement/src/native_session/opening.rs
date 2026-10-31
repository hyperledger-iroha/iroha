//! Original native authority and concrete custody construction under session admission.
//!
//! This owner has no decoder for verified state, no network bootstrap and no wallet
//! provisioning side effect. The caller selects the genesis/schema pins independently.

use super::*;
use crate::{
    FilePrivateSettlementArtifacts, FilePrivateSettlementWallet,
    MAX_PRIVATE_SETTLEMENT_WALLET_BYTES, OneCallPrivateSettlementRequest,
};
use iroha::{
    client::{AccountClient, AccountWorkflowContext},
    workflow::{FileWorkflowStorage, WorkflowAccount as _, WorkflowIdentityV1},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{
        MAX_SIGNED_GENESIS_BYTES_V1, SignedGenesisPinsV1, SumeragiFinalityVerifier,
        authenticate_signed_genesis_v1,
    },
};
use iroha_fs::PrivateDirectory;
use std::path::{Component, PathBuf};
use zeroize::Zeroizing;

/// Concrete file locations and caller-held encryption key for one native payment host.
///
/// Construction performs no filesystem access. The original root contains `workflows`,
/// `artifacts` and `wallet` owner-private directories. The wallet is opened without
/// provisioning or reserving inputs; use its existing explicit `stage_authorized` API
/// separately. An opener, a recovery call or a directory's existence grants no note authority.
pub struct NativePrivateSettlementStores {
    root: PathBuf,
    key: Zeroizing<[u8; 32]>,
    maximum_wallet_bytes: NonZeroUsize,
}

impl NativePrivateSettlementStores {
    /// Retain the original directory selection and wipe-on-drop key without opening files.
    #[must_use]
    pub fn new(
        root: PathBuf,
        key: Zeroizing<[u8; 32]>,
        maximum_wallet_bytes: NonZeroUsize,
    ) -> Self {
        Self {
            root,
            key,
            maximum_wallet_bytes,
        }
    }

    /// Exact original wallet directory used for separate, explicit authorization staging.
    #[must_use]
    pub fn wallet_directory(&self) -> PathBuf {
        self.root.join("wallet")
    }
}

impl std::fmt::Debug for NativePrivateSettlementStores {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativePrivateSettlementStores(<private>)")
    }
}

/// Caller-selected native originals; no file or remote response supplies their authority.
///
/// Supply an already validated, owned `AccountClient`. Its signing key and listener
/// credentials remain in that owner. `genesis_pins` and `world_schema` must come from
/// an independently authenticated source, never the supplied genesis or Torii response.
/// The selected schema is checked against the exact request here and against certified
/// World evidence by the existing preparation/recovery verifier before wallet effects.
/// No unsigned manifest parser or self-authenticating schema document is introduced.
pub struct NativePrivateSettlementOriginals {
    pub(super) account: AccountClient,
    pub(super) request: OneCallPrivateSettlementRequest,
    pub(super) genesis_pins: SignedGenesisPinsV1,
    pub(super) world_schema: Hash,
    stores: NativePrivateSettlementStores,
}

impl NativePrivateSettlementOriginals {
    /// Retain original typed inputs. This does not authenticate, decode, copy genesis or open files.
    ///
    /// Acquire session admission before constructing/copying boundary inputs. Language adapters
    /// must check their raw lengths before decoding into these existing typed native owners.
    #[must_use]
    pub fn new(
        account: AccountClient,
        request: OneCallPrivateSettlementRequest,
        genesis_pins: SignedGenesisPinsV1,
        world_schema: Hash,
        stores: NativePrivateSettlementStores,
    ) -> Self {
        Self {
            account,
            request,
            genesis_pins,
            world_schema,
            stores,
        }
    }
}

impl std::fmt::Debug for NativePrivateSettlementOriginals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativePrivateSettlementOriginals(<restricted>)")
    }
}

// Only an admitted token can capture the borrowed genesis. Both the retained original
// frame and the typed request share its original-value allowance; no fresh budget is made.
pub(super) struct OpenSource {
    originals: NativePrivateSettlementOriginals,
    genesis_wire: Zeroizing<Vec<u8>>,
}

impl OpenSource {
    pub(super) fn from_owned(
        originals: NativePrivateSettlementOriginals,
        genesis_wire: Zeroizing<Vec<u8>>,
        limits: NativePrivateSettlementLimits,
    ) -> Result<Self, OneCallPrivateSettlementError> {
        if genesis_wire.is_empty() || genesis_wire.len() > MAX_SIGNED_GENESIS_BYTES_V1 {
            return Err(OneCallPrivateSettlementError::Capacity);
        }
        originals.check_capacity(genesis_wire.len(), limits)?;
        Ok(Self {
            originals,
            genesis_wire,
        })
    }
    pub(super) fn capture(
        originals: NativePrivateSettlementOriginals,
        genesis_wire: &[u8],
        limits: NativePrivateSettlementLimits,
    ) -> Result<Self, NativePrivateSettlementSessionError> {
        if genesis_wire.is_empty()
            || genesis_wire.len() > MAX_SIGNED_GENESIS_BYTES_V1
            || genesis_wire.len() > limits.maximum_original_bytes.get()
        {
            return Err(NativePrivateSettlementSessionError::Capacity);
        }
        originals
            .check_capacity(genesis_wire.len(), limits)
            .map_err(|_| NativePrivateSettlementSessionError::Capacity)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(genesis_wire.len())
            .map_err(|_| NativePrivateSettlementSessionError::Capacity)?;
        owned.extend_from_slice(genesis_wire);
        Ok(Self {
            originals,
            genesis_wire: Zeroizing::new(owned),
        })
    }
}

impl NativePrivateSettlementOriginals {
    fn original_bytes(&self, wire_len: usize) -> Result<usize, OneCallPrivateSettlementError> {
        let originals = self;
        let pins = &originals.genesis_pins;
        let mut bytes = bounds::request_bytes(&originals.request)?
            .checked_add(wire_len)
            .and_then(|n| n.checked_add(originals.stores.root.as_os_str().as_encoded_bytes().len()))
            .and_then(|n| n.checked_add(pins.chain_id.len()))
            // The retained AEAD key and canonical wallet-capacity descriptor are source
            // originals too. Fixed queue/control/Arc overhead is not canonical value storage.
            .and_then(|n| n.checked_add(32 + std::mem::size_of::<u64>()))
            .ok_or(OneCallPrivateSettlementError::Capacity)?;
        // Count each native pin in its actual canonical framing; the separate frames are a
        // conservative retained-value allowance, not a new wire format or authority decoder.
        for length in [
            norito::canonical_frame_len(&pins.network_id)?,
            norito::canonical_frame_len(&pins.genesis_hash)?,
            norito::canonical_frame_len(&pins.signed_genesis_sha256)?,
            norito::canonical_frame_len(&pins.genesis_public_key)?,
            norito::canonical_frame_len(&pins.roster)?,
            norito::canonical_frame_len(&pins.mode)?,
            norito::canonical_frame_len(&originals.world_schema)?,
        ] {
            bytes = bytes
                .checked_add(length)
                .ok_or(OneCallPrivateSettlementError::Capacity)?;
        }
        Ok(bytes)
    }

    fn check_capacity(
        &self,
        wire_len: usize,
        limits: NativePrivateSettlementLimits,
    ) -> Result<(), OneCallPrivateSettlementError> {
        let custody_bytes = self
            .request
            .0
            .max_total_artifact_bytes
            .checked_add(self.stores.maximum_wallet_bytes.get())
            .ok_or(OneCallPrivateSettlementError::Capacity)?;
        if self.original_bytes(wire_len)? > limits.maximum_original_bytes.get()
            || custody_bytes > limits.maximum_artifact_bytes.get()
            || self.stores.maximum_wallet_bytes.get() > MAX_PRIVATE_SETTLEMENT_WALLET_BYTES
        {
            return Err(OneCallPrivateSettlementError::Capacity);
        }
        Ok(())
    }
}

impl OpenSource {
    pub(super) fn construct_with_staging(
        self,
        limits: NativePrivateSettlementLimits,
    ) -> Result<Opened, OneCallPrivateSettlementError> {
        // Count before native decode and every filesystem open. These are canonical original
        // values plus retained source bytes, not a claim about the decoder's peak live heap.
        self.originals
            .check_capacity(self.genesis_wire.len(), limits)?;
        let original_bytes = self.originals.original_bytes(self.genesis_wire.len())?;
        let Self {
            originals,
            genesis_wire,
        } = self;
        let NativePrivateSettlementOriginals {
            account,
            request,
            genesis_pins,
            world_schema,
            stores,
        } = originals;
        if stores.key.iter().all(|byte| *byte == 0)
            || !stores.root.is_absolute()
            || stores
                .root
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
            || world_schema.as_ref().iter().all(|byte| *byte == 0)
            || request.0.schema != world_schema
            || request.intent().network_id != genesis_pins.network_id
            || account.network_id() != &genesis_pins.network_id
            || account.chain_id().as_ref() != genesis_pins.chain_id
            || account.authority() != &request.intent().sponsor
        {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        let authenticated = authenticate_signed_genesis_v1(&genesis_wire, &genesis_pins)?;
        let verifier = SumeragiFinalityVerifier::new(
            authenticated.block(),
            &genesis_pins.chain_id,
            authenticated.validators().to_vec(),
        )?;
        if verifier.root_scope() != SumeragiRootScope::Global {
            return Err(OneCallPrivateSettlementError::Binding);
        }
        let account_binding = AccountWorkflowContext::new(account.clone())?.binding();
        WorkflowIdentityV1::new(&request.plan(), &account_binding)?;
        // Independently pinned authority and all local input limits now hold. Keep native
        // directory identities throughout child creation and revalidate before handoff.
        let root = PrivateDirectory::open_or_create(&stores.root)
            .map_err(OneCallPrivateSettlementError::Storage)?;
        let workflows = root
            .ensure_child("workflows")
            .map_err(OneCallPrivateSettlementError::Storage)?;
        let artifacts = root
            .ensure_child("artifacts")
            .map_err(OneCallPrivateSettlementError::Storage)?;
        let wallet = root
            .ensure_child("wallet")
            .map_err(OneCallPrivateSettlementError::Storage)?;
        let storage = FileWorkflowStorage::open_or_create(workflows.path())
            .map_err(|error| OneCallPrivateSettlementError::Provider(error.into()))?;
        let artifacts = FilePrivateSettlementArtifacts::open(artifacts.path(), &stores.key)?;
        let wallet = FilePrivateSettlementWallet::open(
            wallet.path(),
            &stores.key,
            stores.maximum_wallet_bytes.get(),
        )?;
        root.revalidate()
            .map_err(OneCallPrivateSettlementError::Storage)?;
        let wallet = Arc::new(wallet);
        let staging = Arc::new(Staging {
            request: request.clone(),
            wallet: wallet.clone(),
            original_bytes,
        });
        let operation = OneCallPrivateSettlementOperation::new(
            account,
            request,
            Arc::new(storage),
            Arc::new(artifacts),
            wallet,
            verifier,
        )?;
        Ok(Opened {
            operation,
            staging: Some(staging),
        })
    }
}

pub(super) struct Opened {
    pub(super) operation: OneCallPrivateSettlementOperation,
    pub(super) staging: Option<Arc<Staging>>,
}
pub(super) struct Staging {
    pub(super) request: OneCallPrivateSettlementRequest,
    pub(super) wallet: Arc<FilePrivateSettlementWallet>,
    pub(super) original_bytes: usize,
}
#[cfg(test)]
impl OpenSource {
    fn construct(
        self,
        limits: NativePrivateSettlementLimits,
    ) -> Result<OneCallPrivateSettlementOperation, OneCallPrivateSettlementError> {
        Ok(self.construct_with_staging(limits)?.operation)
    }
}

#[cfg(test)]
#[path = "opening_tests.rs"]
pub(super) mod tests;
