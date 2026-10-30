//! Node-side support attestation and release support (`specs/live_runtime_upgrades.md` §6,
//! "Attest" and "Stop the unsupported").
//!
//! Whether a validator supports a change set is a property of its release, so the node decides
//! it, not its operator: it resolves the successor selection the change set produces from
//! committed State and checks that this release implements every definition the selection
//! relies on. Only then does it sign the attestation with its own validator key. An operator
//! relays the signed attestation on chain in `AttestRuntimeSupportV1`; the key never leaves the
//! node. The same check tells an operator, before the boundary, which scheduled selection this
//! release cannot execute and which definition it lacks.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, KeyPair, PublicKey};
use iroha_data_model::runtime::{
    definition::{DefinitionId, RootSelectionV1, SelectionId},
    lifecycle::{
        ChangeSetId, ChangeSetRecordV1, ChangeSetStatusV1, LifecycleSignatureError,
        SignedSupportAttestationV1, SupportAttestationV1, committee_digest,
    },
};
use iroha_torii_shared::status::BuildStatus;
use mv::storage::StorageReadOnly;

use super::{
    CatalogError, ReleaseCatalog,
    change_set::{PublishedDefinitions, successor},
    release_catalog,
};
use crate::state::WorldReadOnly;

/// Domain separator of [`release_digest`].
pub const RELEASE_DOMAIN: &[u8] = b"iroha:runtime:release:v1\0";

/// Digest of a release that support attestations bind: `H(RELEASE_DOMAIN ‖ canonical build
/// status ‖ canonical identities of every definition the release implements, in identity
/// order)`.
///
/// The build status names the version, source revision, Cargo features, target and wire-schema
/// identity. Two builds of one source and feature set implement the same definitions and share
/// the digest; a release that changes its build or the definitions it implements gets another.
///
/// # Errors
/// The build status or the definition list cannot be encoded.
pub fn release_digest(
    build: &BuildStatus,
    catalog: &ReleaseCatalog,
) -> Result<[u8; 32], LifecycleSignatureError> {
    // TODO(live_runtime_upgrades.md §3.1): bind the binary digest and toolchain once the build
    // identity carries them.
    let encoding =
        |error: norito::core::Error| LifecycleSignatureError::Encoding(error.to_string());
    let build = norito::encode_canonical(build).map_err(encoding)?;
    let definitions: Vec<DefinitionId> = catalog.definitions.keys().copied().collect();
    let definitions = norito::encode_canonical(&definitions).map_err(encoding)?;
    Ok(Hash::new_from_chunks(&[RELEASE_DOMAIN, &build, &definitions]).into())
}

/// Why this node does not attest a change set, or cannot tell whether it supports it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NodeAttestationError {
    /// The change set is not recorded.
    #[error("change set {0} is not recorded")]
    NotRecorded(ChangeSetId),
    /// The change set is no longer scheduled; attestations count only before it activates.
    #[error("change set {0} is not scheduled")]
    NotScheduled(ChangeSetId),
    /// This node's key is not a member of the current committee.
    #[error("this node's key is not a member of the current committee")]
    NotInCommittee,
    /// The change set no longer applies to the selection it will meet.
    #[error("{0}")]
    NoLongerApplies(String),
    /// This release cannot execute the successor selection.
    #[error(
        "this release cannot execute successor selection {selection}: it does not implement definition {definition}"
    )]
    Unsupported {
        /// The successor selection.
        selection: SelectionId,
        /// The first definition the selection relies on that this release lacks.
        definition: DefinitionId,
    },
    /// The release catalog cannot be built (a build defect).
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// The attestation or the committee cannot be encoded, or the key cannot sign.
    #[error(transparent)]
    Signature(#[from] LifecycleSignatureError),
}

/// Whether this release executes one selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseSupport {
    /// The release implements every definition the selection relies on.
    Supported,
    /// The first definition the selection relies on that the release lacks.
    Missing(DefinitionId),
}

/// Whether `catalog` executes `selection`.
#[must_use]
pub fn release_support(catalog: &ReleaseCatalog, selection: &RootSelectionV1) -> ReleaseSupport {
    catalog
        .unsupported(selection)
        .map_or(ReleaseSupport::Supported, ReleaseSupport::Missing)
}

/// The scheduled change set `id` and the successor selection it produces, resolved from the
/// committed `world` the way admission resolves it: applied to the selection it will meet.
///
/// # Errors
/// The change set is not recorded or not scheduled, or no longer applies to the selection it
/// will meet.
pub fn successor_selection(
    world: &impl WorldReadOnly,
    id: ChangeSetId,
) -> Result<(ChangeSetRecordV1, RootSelectionV1), NodeAttestationError> {
    let change_sets = world.runtime_change_sets();
    let record = change_sets
        .get(&id)
        .cloned()
        .ok_or(NodeAttestationError::NotRecorded(id))?;
    if record.status != ChangeSetStatusV1::Scheduled {
        return Err(NodeAttestationError::NotScheduled(id));
    }
    let scheduled: BTreeMap<ChangeSetId, ChangeSetRecordV1> = change_sets
        .iter()
        .filter(|(_, record)| record.status == ChangeSetStatusV1::Scheduled)
        .map(|(id, record)| (*id, record.clone()))
        .collect();
    let selection = successor(
        &record.change_set,
        id,
        world.runtime_selection(),
        &scheduled,
        &PublishedDefinitions(world.runtime_definitions()),
    )
    .map_err(|error| NodeAttestationError::NoLongerApplies(error.to_string()))?;
    Ok((record, selection))
}

/// Sign this node's support for the scheduled change set `id` (§6, "Attest").
///
/// The node attests only a change set whose successor selection this release executes, only
/// while its key is a member of `committee`, and binds the attestation to that committee. It
/// counts exactly at the effective height, the only height activation reads it.
///
/// # Errors
/// The change set is not scheduled or no longer applies, this node is not in the committee,
/// this release cannot execute the successor, or the attestation cannot be signed.
pub fn sign_support(
    world: &impl WorldReadOnly,
    committee: &[PublicKey],
    committed_height: u64,
    id: ChangeSetId,
    key_pair: &KeyPair,
    release: [u8; 32],
) -> Result<SignedSupportAttestationV1, NodeAttestationError> {
    let (record, successor) = successor_selection(world, id)?;
    let validator = key_pair.public_key().clone();
    if !committee.contains(&validator) {
        return Err(NodeAttestationError::NotInCommittee);
    }
    let change_set = &record.change_set;
    if let ReleaseSupport::Missing(definition) = release_support(release_catalog()?, &successor) {
        return Err(NodeAttestationError::Unsupported {
            selection: change_set.successor,
            definition,
        });
    }
    let effective_height = change_set.effective_height;
    SupportAttestationV1 {
        network: change_set.network,
        root: change_set.root,
        change_set: id,
        predecessor: change_set.predecessor,
        successor: change_set.successor,
        committee: committee_digest(committee)?,
        // TODO(live_runtime_upgrades.md §7.3): name the checkpoint the release was qualified on
        // once checkpoints exist; until then, the committed height whose State the node resolved
        // the successor against.
        tested_checkpoint: committed_height,
        valid_from: effective_height,
        valid_until: effective_height.saturating_add(1),
        release,
        validator,
    }
    .sign(key_pair.private_key())
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
