//! Governed adoption of a running network (`specs/live_runtime_upgrades.md` §13).
//!
//! The predecessor release (iroha f31b64d8c5) governs runtime upgrades through ABI-v1 manifests
//! recorded in its `world.runtime_upgrades` table by the account holding
//! `CanManageRuntimeUpgrades`. It admits only manifests it can activate, so it cannot stop itself
//! at a governed height. Adoption is therefore authorized by one such manifest that opens an
//! adoption window instead: [`ADOPTION_MANIFEST_NAME`], an ABI-v1 surface the predecessor accepts,
//! and an SBOM digest [`ADOPTION_SELECTION_DIGEST`] naming this release's baseline selection. The
//! predecessor activates it at its start height `A`; validators stop at a coordinated checkpoint
//! `H` with `A < H < end_height`, and this release adopts the predecessor's World at `H` only
//! with that authorization in it.
//!
//! The authorization is part of the predecessor's certified World: the import adds its entries
//! back when it recomputes the predecessor's root, which must equal block `H`'s certified World
//! root ([`crate::state::world_projection::world_state_accumulator`]). Authority therefore comes
//! from the certified governance record, never from node configuration, which only locates the
//! checkpoint.
//!
//! The types here reproduce the predecessor's encoding exactly so its entries hash as they did.
//! They exist only for the import and are deleted with it once every validator has adopted.

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    runtime::{
        definition::SelectionId,
        lifecycle::{AdoptionRecordV1, committee_digest},
    },
    smart_contract::manifest::ManifestProvenance,
};
use iroha_model_base::peer::PeerId;
use norito::{
    codec::{Decode, Encode},
    json::{self, JsonKeyCodec as _, Value},
};

/// Name of the predecessor runtime-upgrade manifest that authorizes adoption.
pub const ADOPTION_MANIFEST_NAME: &str = "iroha:runtime:adoption:v1";
/// SBOM-digest algorithm whose digest names the selection the adopting release founds.
pub const ADOPTION_SELECTION_DIGEST: &str = "iroha:runtime:adoption-selection:v1";

/// Identity of a predecessor runtime-upgrade record: the hash of its manifest's canonical bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct PredecessorUpgradeId(pub [u8; 32]);

/// One SBOM digest of a predecessor manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PredecessorSbomDigest {
    /// Digest algorithm label.
    pub algorithm: String,
    /// Digest bytes.
    pub digest: Vec<u8>,
}

/// A predecessor runtime-upgrade manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PredecessorUpgradeManifest {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// ABI version; the predecessor admits only `1`.
    pub abi_version: u16,
    /// ABI hash; the predecessor admits only its own ABI-v1 hash.
    pub abi_hash: [u8; 32],
    /// Added syscalls; the predecessor admits none.
    pub added_syscalls: Vec<u16>,
    /// Added pointer types; the predecessor admits none.
    pub added_pointer_types: Vec<u16>,
    /// First height of the window (inclusive): the predecessor activates the manifest here.
    pub start_height: u64,
    /// End of the window (exclusive).
    pub end_height: u64,
    /// SBOM digests.
    pub sbom_digests: Vec<PredecessorSbomDigest>,
    /// SLSA attestation bytes.
    pub slsa_attestation: Vec<u8>,
    /// Provenance signatures over the manifest's signature payload.
    pub provenance: Vec<ManifestProvenance>,
}

/// Lifecycle status of a predecessor runtime-upgrade record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PredecessorUpgradeStatus {
    /// Recorded, not activated.
    Proposed,
    /// Activated at the height.
    ActivatedAt(u64),
    /// Cancelled before activation.
    Canceled,
}

/// A predecessor runtime-upgrade record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PredecessorUpgradeRecord {
    /// Manifest.
    pub manifest: PredecessorUpgradeManifest,
    /// Status.
    pub status: PredecessorUpgradeStatus,
    /// Account that proposed it.
    pub proposer: AccountId,
    /// Height of the block that recorded it.
    pub created_height: u64,
}

/// One entry of the predecessor's retired `world.runtime_upgrades` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredecessorRecord {
    /// Record identity (the table key).
    pub id: PredecessorUpgradeId,
    /// Record.
    pub record: PredecessorUpgradeRecord,
}

impl PredecessorRecord {
    /// The key and value hashes of this entry in the predecessor's World root.
    ///
    /// # Errors
    /// A value cannot be encoded.
    pub(crate) fn element_hashes(&self) -> Result<(Hash, Hash), String> {
        Ok((
            crate::state::world_projection::hash_value(&self.id)?,
            crate::state::world_projection::hash_value(&self.record)?,
        ))
    }
}

/// Why a predecessor World cannot be adopted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdoptionError {
    /// The retired table holds a record of another runtime upgrade, which an import would drop.
    #[error(
        "the predecessor's runtime_upgrades holds record {0}, which is no adoption authorization"
    )]
    NotAnAuthorization(String),
    /// No record authorizes this release to adopt at the checkpoint.
    #[error(
        "no governed adoption authorization names selection {selection} for checkpoint {height}"
    )]
    Unauthorized {
        /// The selection this release founds.
        selection: SelectionId,
        /// The checkpoint height.
        height: u64,
    },
    /// More than one record authorizes the same adoption.
    #[error("{0} records authorize this adoption; exactly one must")]
    Ambiguous(usize),
    /// The table's snapshot JSON is malformed.
    #[error("malformed predecessor runtime_upgrades: {0}")]
    Malformed(String),
}

/// The selection an authorization's SBOM digests name, if exactly one names one.
fn named_selection(manifest: &PredecessorUpgradeManifest) -> Option<SelectionId> {
    let mut named = manifest
        .sbom_digests
        .iter()
        .filter(|digest| digest.algorithm == ADOPTION_SELECTION_DIGEST);
    let first = named.next()?;
    if named.next().is_some() {
        return None;
    }
    <[u8; 32]>::try_from(first.digest.as_slice())
        .ok()
        .map(SelectionId)
}

/// Whether `record` authorizes adopting `selection` at checkpoint `height`: it is an adoption
/// manifest naming `selection`, the predecessor activated it at its start height `A`, and
/// `A < height < end_height`.
fn authorizes(record: &PredecessorUpgradeRecord, height: u64, selection: SelectionId) -> bool {
    let manifest = &record.manifest;
    manifest.name == ADOPTION_MANIFEST_NAME
        && named_selection(manifest) == Some(selection)
        && record.status == PredecessorUpgradeStatus::ActivatedAt(manifest.start_height)
        && manifest.start_height < height
        && height < manifest.end_height
}

/// The record that authorizes this release, whose baseline is `selection`, to adopt the
/// predecessor's World at checkpoint `height`.
///
/// Every record of the retired table must be an adoption manifest, so the import drops no other
/// governance state; earlier, cancelled or expired authorizations are dropped with the table.
///
/// # Errors
/// A record is not an adoption manifest, or not exactly one record authorizes this adoption.
pub fn authorization(
    records: &[PredecessorRecord],
    height: u64,
    selection: SelectionId,
) -> Result<&PredecessorRecord, AdoptionError> {
    if let Some(other) = records
        .iter()
        .find(|entry| entry.record.manifest.name != ADOPTION_MANIFEST_NAME)
    {
        return Err(AdoptionError::NotAnAuthorization(hex::encode(other.id.0)));
    }
    let mut authorizing = records
        .iter()
        .filter(|entry| authorizes(&entry.record, height, selection));
    let found = authorizing
        .next()
        .ok_or(AdoptionError::Unauthorized { selection, height })?;
    let more = authorizing.count();
    if more > 0 {
        return Err(AdoptionError::Ambiguous(more + 1));
    }
    Ok(found)
}

fn malformed(message: impl Into<String>) -> AdoptionError {
    AdoptionError::Malformed(message.into())
}

/// The fields of a JSON object, refusing any other than `names`.
fn object<'v>(
    value: &'v Value,
    what: &str,
    names: &[&str],
) -> Result<&'v json::Map, AdoptionError> {
    let map = value
        .as_object()
        .ok_or_else(|| malformed(format!("{what} is not an object")))?;
    if let Some(unknown) = map.keys().find(|key| !names.contains(&key.as_str())) {
        return Err(malformed(format!("{what} has unknown field {unknown}")));
    }
    if map.len() != names.len() {
        return Err(malformed(format!("{what} lacks a field")));
    }
    Ok(map)
}

fn field<'v>(map: &'v json::Map, name: &str) -> Result<&'v Value, AdoptionError> {
    map.get(name)
        .ok_or_else(|| malformed(format!("missing field {name}")))
}

fn string(value: &Value, what: &str) -> Result<String, AdoptionError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| malformed(format!("{what} is not a string")))
}

fn unsigned(value: &Value, what: &str) -> Result<u64, AdoptionError> {
    value
        .as_u64()
        .ok_or_else(|| malformed(format!("{what} is not an unsigned integer")))
}

fn base64(value: &Value, what: &str) -> Result<Vec<u8>, AdoptionError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(string(value, what)?.as_bytes())
        .map_err(|error| malformed(format!("{what}: {error}")))
}

fn array<'v>(value: &'v Value, what: &str) -> Result<&'v Vec<Value>, AdoptionError> {
    value
        .as_array()
        .ok_or_else(|| malformed(format!("{what} is not an array")))
}

fn u16_list(value: &Value, what: &str) -> Result<Vec<u16>, AdoptionError> {
    array(value, what)?
        .iter()
        .map(|item| {
            u16::try_from(unsigned(item, what)?)
                .map_err(|_| malformed(format!("{what} exceeds u16")))
        })
        .collect()
}

fn manifest(value: &Value) -> Result<PredecessorUpgradeManifest, AdoptionError> {
    let map = object(
        value,
        "manifest",
        &[
            "name",
            "description",
            "abi_version",
            "abi_hash",
            "added_syscalls",
            "added_pointer_types",
            "start_height",
            "end_height",
            "sbom_digests",
            "slsa_attestation",
            "provenance",
        ],
    )?;
    let abi_hash = array(field(map, "abi_hash")?, "abi_hash")?
        .iter()
        .map(|byte| {
            u8::try_from(unsigned(byte, "abi_hash")?)
                .map_err(|_| malformed("abi_hash byte exceeds u8"))
        })
        .collect::<Result<Vec<u8>, _>>()?;
    let sbom_digests = array(field(map, "sbom_digests")?, "sbom_digests")?
        .iter()
        .map(|digest| {
            let digest = object(digest, "sbom digest", &["algorithm", "digest"])?;
            Ok(PredecessorSbomDigest {
                algorithm: string(field(digest, "algorithm")?, "algorithm")?,
                digest: base64(field(digest, "digest")?, "digest")?,
            })
        })
        .collect::<Result<Vec<_>, AdoptionError>>()?;
    let provenance = array(field(map, "provenance")?, "provenance")?
        .iter()
        .map(|entry| {
            json::from_value::<ManifestProvenance>(entry.clone())
                .map_err(|error| malformed(format!("provenance: {error}")))
        })
        .collect::<Result<Vec<_>, AdoptionError>>()?;
    Ok(PredecessorUpgradeManifest {
        name: string(field(map, "name")?, "name")?,
        description: string(field(map, "description")?, "description")?,
        abi_version: u16::try_from(unsigned(field(map, "abi_version")?, "abi_version")?)
            .map_err(|_| malformed("abi_version exceeds u16"))?,
        abi_hash: abi_hash
            .try_into()
            .map_err(|_| malformed("abi_hash is not 32 bytes"))?,
        added_syscalls: u16_list(field(map, "added_syscalls")?, "added_syscalls")?,
        added_pointer_types: u16_list(field(map, "added_pointer_types")?, "added_pointer_types")?,
        start_height: unsigned(field(map, "start_height")?, "start_height")?,
        end_height: unsigned(field(map, "end_height")?, "end_height")?,
        sbom_digests,
        slsa_attestation: base64(field(map, "slsa_attestation")?, "slsa_attestation")?,
        provenance,
    })
}

fn status(value: &Value) -> Result<PredecessorUpgradeStatus, AdoptionError> {
    let map = value
        .as_object()
        .filter(|map| map.len() == 1)
        .ok_or_else(|| malformed("status is not a single-key object"))?;
    match map.iter().next() {
        Some((name, Value::Null)) if name == "Proposed" => Ok(PredecessorUpgradeStatus::Proposed),
        Some((name, Value::Null)) if name == "Canceled" => Ok(PredecessorUpgradeStatus::Canceled),
        Some((name, height)) if name == "ActivatedAt" => Ok(PredecessorUpgradeStatus::ActivatedAt(
            unsigned(height, "ActivatedAt")?,
        )),
        _ => Err(malformed("unknown status")),
    }
}

fn record(value: &Value) -> Result<PredecessorUpgradeRecord, AdoptionError> {
    let map = object(
        value,
        "record",
        &["manifest", "status", "proposer", "created_height"],
    )?;
    Ok(PredecessorUpgradeRecord {
        manifest: manifest(field(map, "manifest")?)?,
        status: status(field(map, "status")?)?,
        proposer: json::from_value::<AccountId>(field(map, "proposer")?.clone())
            .map_err(|error| malformed(format!("proposer: {error}")))?,
        created_height: unsigned(field(map, "created_height")?, "created_height")?,
    })
}

/// The entries of the predecessor's `world.runtime_upgrades` from its snapshot JSON
/// (`{"revert":{…},"blocks":{"<id>":<record>,…}}`).
///
/// The checkpoint block must not have changed the table: its undo log is empty, since nothing of
/// the table survives the import to undo.
///
/// # Errors
/// The JSON is not the predecessor's table layout, or the checkpoint block changed the table.
pub fn parse_runtime_upgrades(raw: &str) -> Result<Vec<PredecessorRecord>, AdoptionError> {
    let value: Value =
        json::from_str(raw).map_err(|error| malformed(format!("not JSON: {error}")))?;
    let map = object(&value, "runtime_upgrades", &["revert", "blocks"])?;
    if !field(map, "revert")?
        .as_object()
        .is_some_and(json::Map::is_empty)
    {
        return Err(malformed(
            "the checkpoint block changed the table; adopt at a later checkpoint",
        ));
    }
    let blocks = field(map, "blocks")?
        .as_object()
        .ok_or_else(|| malformed("blocks is not an object"))?;
    blocks
        .iter()
        .map(|(key, value)| {
            let id = <[u8; 32]>::decode_json_key(key)
                .map(PredecessorUpgradeId)
                .map_err(|error| malformed(format!("record key {key}: {error}")))?;
            Ok(PredecessorRecord {
                id,
                record: record(value)?,
            })
        })
        .collect()
}

/// What the import knows when it anchors an adoption.
#[derive(Clone, Copy, Debug)]
pub struct AnchorInputs<'a> {
    /// The network.
    pub network: NetworkId,
    /// The checkpoint height: the last block the predecessor executed.
    pub height: u64,
    /// Hash of that block.
    pub block_hash: HashOf<BlockHeader>,
    /// The predecessor's World root that block certifies.
    pub predecessor_world_root: Hash,
    /// Root of the imported World under this release's commitment profile.
    pub imported_state_root: Hash,
    /// The committee at the checkpoint.
    pub committee: &'a [PeerId],
}

/// The trust anchor of adopting the predecessor's World at a checkpoint (§13, "Anchor"):
/// the network, the checkpoint block, both roots, the committee, the selection this release
/// founds and the governance record that authorized it.
///
/// # Errors
/// No record of `records` authorizes this release to adopt at the checkpoint, or a value
/// cannot be encoded.
pub fn anchor(
    records: &[PredecessorRecord],
    inputs: AnchorInputs<'_>,
) -> Result<AdoptionRecordV1, String> {
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    let authorization = authorization(records, inputs.height, catalog.baseline_id)
        .map_err(|error| error.to_string())?;
    let committee: Vec<PublicKey> = inputs
        .committee
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    Ok(AdoptionRecordV1 {
        network: inputs.network,
        height: inputs.height,
        block_hash: Hash::from(inputs.block_hash).into(),
        predecessor_world_root: inputs.predecessor_world_root.into(),
        imported_state_root: inputs.imported_state_root.into(),
        committee: committee_digest(&committee).map_err(|error| error.to_string())?,
        selection: catalog.baseline_id,
        authorization: authorization.id.0,
    })
}

/// The predecessor manifest, in the JSON its `iroha runtime upgrade propose --file` takes, that
/// authorizes adopting `selection` at a checkpoint after `start_height` and before
/// `end_height`: an ABI-v1 surface the predecessor accepts, named [`ADOPTION_MANIFEST_NAME`],
/// whose SBOM digest [`ADOPTION_SELECTION_DIGEST`] names `selection`.
///
/// Provenance the predecessor's configured policy requires is added with the predecessor's own
/// tooling; signatures do not change what the manifest authorizes.
///
/// # Errors
/// The window is empty.
pub fn authorization_manifest_json(
    selection: SelectionId,
    start_height: u64,
    end_height: u64,
    description: &str,
) -> Result<String, String> {
    use base64::Engine as _;
    if end_height <= start_height.saturating_add(1) {
        return Err("the window must leave a checkpoint between its start and its end".into());
    }
    let abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let mut digest = json::Map::new();
    digest.insert(
        "algorithm".into(),
        Value::String(ADOPTION_SELECTION_DIGEST.into()),
    );
    digest.insert(
        "digest".into(),
        Value::String(base64::engine::general_purpose::STANDARD.encode(selection.0)),
    );
    let mut manifest = json::Map::new();
    manifest.insert("name".into(), Value::String(ADOPTION_MANIFEST_NAME.into()));
    manifest.insert("description".into(), Value::String(description.into()));
    manifest.insert("abi_version".into(), Value::from(1_u64));
    manifest.insert(
        "abi_hash".into(),
        Value::Array(
            abi_hash
                .iter()
                .map(|byte| Value::from(u64::from(*byte)))
                .collect(),
        ),
    );
    manifest.insert("added_syscalls".into(), Value::Array(Vec::new()));
    manifest.insert("added_pointer_types".into(), Value::Array(Vec::new()));
    manifest.insert("start_height".into(), Value::from(start_height));
    manifest.insert("end_height".into(), Value::from(end_height));
    manifest.insert(
        "sbom_digests".into(),
        Value::Array(vec![Value::Object(digest)]),
    );
    manifest.insert("slsa_attestation".into(), Value::String(String::new()));
    manifest.insert("provenance".into(), Value::Array(Vec::new()));
    json::to_json(&Value::Object(manifest)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
