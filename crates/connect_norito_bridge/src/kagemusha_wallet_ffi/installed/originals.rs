//! Read-only actual source directories routed only by the authenticated complete catalog.

use super::*;
use iroha_core_zk::kagemusha_wallet_artifacts_v1::producer_inventory::{
    AuthenticatedProducerInventoryV1, DirectoryOriginalsV1, PROVING_KEY_MAX_BYTES_V1,
    ProducerInventoryV1,
};
use iroha_core_zk::kagemusha_wallet_proofs_v1::Error as OriginalError;
use iroha_fs::PrivateDirectory;
use std::{io::Read, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Store {
    Wallet,
    Finality,
}

/// Clones share retained native ancestry and immutable metadata; never PK buffers.
#[derive(Clone)]
pub(super) struct CatalogOriginals {
    _selected: Arc<Selection>,
    metadata: Arc<PrivateDirectory>,
    wallet: Arc<DirectoryOriginalsV1>,
    finality: Arc<DirectoryOriginalsV1>,
    roles: Arc<BTreeMap<[u8; 32], SelectedOriginal>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedOriginal {
    blob: BlobV1,
    store: Store,
}

fn insert(
    roles: &mut BTreeMap<[u8; 32], SelectedOriginal>,
    blob: BlobV1,
    role: Store,
) -> Result<()> {
    if blob.bytes == 0 || blob.sha256 == [0; 32] || blob.bytes > PROVING_KEY_MAX_BYTES_V1 as u64 {
        return Err(Failure::code(INVALID));
    }
    if let Some(previous) = roles.get_mut(&blob.sha256) {
        if previous.blob != blob {
            return Err(Failure::code(INVALID));
        }
        // A shared exact original selects one store deterministically. A read never
        // falls through from a missing/invalid original into another directory.
        if role == Store::Wallet {
            previous.store = Store::Wallet;
        }
    } else {
        roles.insert(blob.sha256, SelectedOriginal { blob, store: role });
    }
    Ok(())
}

impl CatalogOriginals {
    pub(super) fn from_authenticated(
        authenticated: &AuthenticatedProducerInventoryV1,
        root: &[u8],
        pack: &[u8],
        catalog: &[u8],
        selected: Arc<Selection>,
    ) -> Result<Self> {
        let inventory = authenticated.inventory();
        if root.is_empty() || root.len() > ROOT_MAX || root.contains(&0) {
            return Err(Failure::code(INVALID));
        }
        let root = Path::new(std::str::from_utf8(root).map_err(|_| Failure::code(INVALID))?);
        if !root.is_absolute() {
            return Err(Failure::code(INVALID));
        }
        let metadata = PrivateDirectory::open_exact(root).map_err(storage)?;
        // All five financial offering paths have fixed current Native roles.
        // A signed transport row cannot substitute a different path.
        for (name, bytes, cap) in [
            ("verifier-pack.norito", pack, VERIFIER_PACK_MAX_BYTES_V1),
            ("producer-inventory.norito", catalog, CATALOG_MAX_BYTES_V1),
        ] {
            let actual = metadata.read(name, cap).map_err(storage)?;
            if actual.as_slice() != bytes {
                return Err(Failure::code(INVALID));
            }
        }
        let transport = metadata
            .read("transport.json", 32 * 1024 * 1024)
            .map_err(storage)?;
        if BlobV1::of(&transport) != selected.transport_identity {
            return Err(Failure::code(INVALID));
        }
        require_transport(inventory, &transport)?;
        // The exact current financial roles are the only source directories. This
        // method creates no files and replaces no authority or old missing original.
        let wallet = DirectoryOriginalsV1::open_existing(
            root.join("wallet-originals"),
            PROVING_KEY_MAX_BYTES_V1,
        )
        .map_err(storage)?;
        let finality = DirectoryOriginalsV1::open_existing(
            root.join("finality-originals"),
            PROVING_KEY_MAX_BYTES_V1,
        )
        .map_err(storage)?;
        // Reauthenticate every whole offered original, including server finality PK
        // tables transported as DATA. The native finality verifier imports only D/VK;
        // this bounded pass establishes that the offered closed carrier is complete.
        for record in &inventory.originals {
            for blob in [record.descriptor, record.verifying_key, record.proving_key] {
                wallet.verify_original(blob).map_err(storage)?;
            }
        }
        for record in &inventory.finality.originals {
            for index in 0..3 {
                finality
                    .verify_original(BlobV1 {
                        bytes: record.lengths[index],
                        sha256: record.sha256[index],
                    })
                    .map_err(storage)?;
            }
        }
        let mut roles = BTreeMap::new();
        for record in &inventory.originals {
            for blob in [record.descriptor, record.verifying_key, record.proving_key] {
                insert(&mut roles, blob, Store::Wallet)?;
            }
        }
        for record in &inventory.finality.originals {
            // Finality's native wallet graph mounts only the descriptor/VK source; it
            // never imports server receipt prover tables or stores those tables in RAM.
            for i in 0..2 {
                insert(
                    &mut roles,
                    BlobV1 {
                        bytes: record.lengths[i],
                        sha256: record.sha256[i],
                    },
                    Store::Finality,
                )?;
            }
        }
        metadata.revalidate().map_err(storage)?;
        Ok(Self {
            _selected: selected,
            metadata: Arc::new(metadata),
            wallet: Arc::new(wallet),
            finality: Arc::new(finality),
            roles: Arc::new(roles),
        })
    }
}
impl OriginalSourceV1 for CatalogOriginals {
    fn open(&mut self, sha256: [u8; 32]) -> std::result::Result<Box<dyn Read + '_>, OriginalError> {
        self.metadata
            .revalidate()
            .map_err(|_| OriginalError::Unavailable)?;
        match self
            .roles
            .get(&sha256)
            .ok_or(OriginalError::Inventory)?
            .store
        {
            Store::Wallet => self.wallet.open_original(sha256),
            Store::Finality => self.finality.open_original(sha256),
        }
    }
}
fn storage(error: std::io::Error) -> Failure {
    if matches!(
        error.kind(),
        std::io::ErrorKind::InvalidData
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::NotFound
    ) {
        Failure::code(INVALID)
    } else {
        Failure::unavailable(
            UNAVAILABLE,
            advance::KagemushaWalletUnavailableV1::from_io(&error),
        )
    }
}
fn closed(blobs: impl IntoIterator<Item = BlobV1>) -> Result<Vec<BlobV1>> {
    let mut originals = BTreeMap::new();
    for blob in blobs {
        if originals
            .insert(blob.sha256, blob)
            .is_some_and(|previous| previous != blob)
        {
            return Err(Failure::code(INVALID));
        }
    }
    Ok(originals.into_values().collect())
}
fn require_transport(inventory: &ProducerInventoryV1, bytes: &[u8]) -> Result<()> {
    // Exact catalog maxima are 4096 wallet records and 65536 finality records;
    // each contributes three original rows with two fixed scalar fields. Account
    // for that whole legitimate closed transport graph before allocating JSON.
    const ROWS: usize =
        (iroha_core_zk::kagemusha_wallet_artifacts_v1::producer_inventory::ARTIFACT_MAX_COUNT_V1
            + 65_536)
            * 3;
    let limits = norito::json::JsonPreflightLimits::new(
        32 * 1024 * 1024,
        ROWS * 3 + 4,
        512,
        128,
        32 * 1024 * 1024,
        ROWS,
        ROWS,
        ROWS * 2 + 3,
        ROWS * 3 + 3,
        4,
    );
    let value = selection::json_limits(bytes, 32 * 1024 * 1024, false, limits)?;
    let object = selection::exact(&value, &["schema", "walletOriginals", "finalityOriginals"])?;
    if selection::text(object, "schema")? != "iroha.kagemusha.wallet-artifact-original-transport.v1"
    {
        return Err(Failure::code(INVALID));
    }
    let wallet = closed(
        inventory
            .originals
            .iter()
            .flat_map(|r| [r.descriptor, r.verifying_key, r.proving_key]),
    )?;
    let finality = closed(inventory.finality.originals.iter().flat_map(|r| {
        (0..3).map(|i| BlobV1 {
            bytes: r.lengths[i],
            sha256: r.sha256[i],
        })
    }))?;
    for (name, expected) in [("walletOriginals", wallet), ("finalityOriginals", finality)] {
        let actual = object
            .get(name)
            .and_then(norito::json::Value::as_array)
            .ok_or(Failure::code(INVALID))?;
        if actual.len() != expected.len() {
            return Err(Failure::code(INVALID));
        }
        for (actual, expected) in actual.iter().zip(expected) {
            let row = selection::exact(actual, &["bytes", "sha256"])?;
            if row.get("bytes").and_then(norito::json::Value::as_u64) != Some(expected.bytes)
                || selection::sha(selection::text(row, "sha256")?)? != expected.sha256
            {
                return Err(Failure::code(INVALID));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
