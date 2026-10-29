//! Live-upgrade lifecycle records (`specs/live_runtime_upgrades.md` §6, §7 and §13).
//!
//! A [`ChangeSetV1`] moves one execution root from its predecessor selection to an exact
//! successor selection. [`ChangeSetV1::apply`] is the deterministic application every node
//! performs: it adds the published definitions, applies the transitions in order, and requires the
//! result to be the declared successor and to satisfy the closure rules. Validators state that
//! their release executes that successor with a [`SupportAttestationV1`]. Effects that leave work
//! behind register it as an [`ObligationRecordV1`], emergency containment is a [`ContainmentV1`],
//! and a running network that adopts the foundation records its trust anchor as an
//! [`AdoptionRecordV1`].

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, PrivateKey, PublicKey, Signature};
use iroha_model_base::topology::DataSpaceId;
use norito::codec::{Decode, Encode};

use super::definition::{
    BindingV1, CompletionClosureV1, DefinitionDescriptorV1, DefinitionError, DefinitionId,
    DefinitionKind, DefinitionLookup, ResourceLimitsV1, RestrictionEntryV1, RestrictionV1,
    RootSelectionV1, SelectionError, SelectionId, StorageShapeV1,
};
use crate::NetworkId;

/// Domain separator of [`ChangeSetId`].
pub const CHANGE_SET_ID_DOMAIN: &[u8] = b"iroha:runtime:change-set:v1\0";
/// Domain separator of a support attestation signature.
pub const SUPPORT_ATTESTATION_DOMAIN: &[u8] = b"iroha:runtime:support-attestation:v1\0";
/// Domain separator of [`committee_digest`].
pub const COMMITTEE_DOMAIN: &[u8] = b"iroha:runtime:committee:v1\0";
/// Domain separator of [`AdoptionRecordV1::id`].
pub const ADOPTION_RECORD_DOMAIN: &[u8] = b"iroha:runtime:adoption:v1\0";
/// Domain separator of [`ContainmentV1::id`].
pub const CONTAINMENT_DOMAIN: &[u8] = b"iroha:runtime:containment:v1\0";
/// Domain separator of a containment endorsement signature.
pub const ENDORSEMENT_DOMAIN: &[u8] = b"iroha:runtime:containment-endorsement:v1\0";
/// How long an emergency endorsement stays valid (§7.1).
pub const ENDORSEMENT_LIFETIME_MS: u64 = 30 * 60 * 1000;

/// The execution root a lifecycle record applies to.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::RootScopeV1")]
#[norito(tag = "kind", content = "content")]
pub enum RootScopeV1 {
    /// The universal (global) root, governed by SORA Parliament.
    Universal,
    /// An independently rooted permissioned data space, governed by its admins.
    DataSpace(DataSpaceId),
}

/// A restriction a transition applies; its height is the change set's effective height.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::RestrictionKindV1")]
#[norito(tag = "kind", content = "content")]
pub enum RestrictionKindV1 {
    /// No new deployments, bindings or dependents.
    Deprecated,
    /// No new work.
    AdmissionClosed,
    /// Only completion capabilities run.
    CompletionOnly,
    /// No execution.
    Disabled,
}

impl RestrictionKindV1 {
    /// The restriction effective from `height`.
    #[must_use]
    pub const fn at(self, height: u64) -> RestrictionV1 {
        match self {
            Self::Deprecated => RestrictionV1::Deprecated(height),
            Self::AdmissionClosed => RestrictionV1::AdmissionClosed(height),
            Self::CompletionOnly => RestrictionV1::CompletionOnly(height),
            Self::Disabled => RestrictionV1::Disabled(height),
        }
    }
}

/// One change to a root's selection.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::SelectionTransitionV1")]
#[norito(tag = "kind", content = "content")]
pub enum SelectionTransitionV1 {
    /// Install a storage definition; its initial bytes are written.
    InstallStorage(DefinitionId),
    /// Retire installed storage; only a migration cutover may do this.
    RetireStorage(DefinitionId),
    /// Activate an interface, implementation, event, permission, parameter, host profile or
    /// migration.
    Activate(DefinitionId),
    /// Remove a disabled definition that no obligation needs from the active set.
    Deactivate(DefinitionId),
    /// Bind a wire ID to an implementation, replacing any earlier binding.
    Bind(BindingV1),
    /// Remove a wire ID's binding.
    Unbind(String),
    /// Restrict a definition from the effective height.
    Restrict {
        /// The definition.
        definition: DefinitionId,
        /// The restriction.
        restriction: RestrictionKindV1,
    },
    /// Lift a definition's restriction (re-enable it).
    Lift(DefinitionId),
    /// Replace the kernel.
    SetKernel(DefinitionId),
    /// Replace the commitment profile; the parent root keeps the predecessor profile.
    SetCommitmentProfile(DefinitionId),
    /// Replace the execution services profile.
    SetServicesProfile(DefinitionId),
    /// Retain the completion closure of a newly registered obligation.
    RetainCompletion(CompletionClosureV1),
    /// Release the completion closure of an obligation that completed or was transferred.
    ReleaseCompletion([u8; 32]),
}

/// Identity of a change set.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ChangeSetId")]
pub struct ChangeSetId(pub [u8; 32]);

crate::runtime::definition::hex_identity_json!(ChangeSetId);

impl core::fmt::Display for ChangeSetId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// A governed change from one selection of an execution root to the next.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ChangeSetV1")]
pub struct ChangeSetV1 {
    /// Network the change applies to.
    pub network: NetworkId,
    /// Execution root the change applies to.
    pub root: RootScopeV1,
    /// The selection the change starts from.
    pub predecessor: SelectionId,
    /// The exact selection the change produces.
    pub successor: SelectionId,
    /// First height executed under the successor.
    pub effective_height: u64,
    /// Definitions published by this change, sorted by unique identity.
    pub publish: Vec<DefinitionDescriptorV1>,
    /// Transitions, applied in order.
    pub transitions: Vec<SelectionTransitionV1>,
    /// Digest of the qualification evidence for the successor composition.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub qualification: [u8; 32],
}

/// A change set that cannot be applied.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChangeSetError {
    /// The change set does not start from the given selection.
    #[error("the change set does not start from the current selection")]
    PredecessorMismatch,
    /// The change set produces another selection than it declares.
    #[error("the change set does not produce its declared successor")]
    SuccessorMismatch,
    /// A published definition is invalid.
    #[error("published definition is invalid: {0}")]
    Definition(#[from] DefinitionError),
    /// Published definitions are unsorted, repeated or already published.
    #[error("published definitions must be new, sorted and unique")]
    NonCanonicalPublication,
    /// A transition does not apply to the selection it meets.
    #[error("transition {index} does not apply: {reason}")]
    Transition {
        /// Position of the transition.
        index: usize,
        /// Why it does not apply.
        reason: &'static str,
    },
    /// A transition would strand a live obligation.
    #[error("definition {0} is still needed by a live obligation")]
    ObligationStranded(DefinitionId),
    /// The successor breaks the closure rules.
    #[error("successor selection: {0}")]
    Selection(#[from] SelectionError),
    /// The change set cannot be encoded canonically.
    #[error("change set encoding failed: {0}")]
    Encoding(String),
}

/// Published definitions overlaid on an existing lookup.
struct Overlay<'a, L> {
    base: &'a L,
    added: &'a BTreeMap<DefinitionId, DefinitionDescriptorV1>,
}

impl<L: DefinitionLookup> DefinitionLookup for Overlay<'_, L> {
    fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
        self.added.get(id).or_else(|| self.base.definition(id))
    }
}

/// The successor selection and the definitions a change set published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedChangeSet {
    /// The successor selection.
    pub selection: RootSelectionV1,
    /// The newly published definitions by identity.
    pub published: BTreeMap<DefinitionId, DefinitionDescriptorV1>,
}

impl ChangeSetV1 {
    /// Content-addressed identity: `H(CHANGE_SET_ID_DOMAIN ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The change set cannot be encoded.
    pub fn id(&self) -> Result<ChangeSetId, ChangeSetError> {
        let bytes = norito::encode_canonical(self)
            .map_err(|error| ChangeSetError::Encoding(error.to_string()))?;
        Ok(ChangeSetId(
            Hash::new_from_chunks(&[CHANGE_SET_ID_DOMAIN, &bytes]).into(),
        ))
    }

    /// Apply the change to `predecessor`, whose definitions `published` resolves.
    ///
    /// # Errors
    /// The predecessor does not match, a publication or transition is invalid, a live obligation
    /// would be stranded, or the result is not the declared, closed successor.
    pub fn apply(
        &self,
        predecessor: &RootSelectionV1,
        published: &impl DefinitionLookup,
    ) -> Result<AppliedChangeSet, ChangeSetError> {
        if predecessor.id()? != self.predecessor {
            return Err(ChangeSetError::PredecessorMismatch);
        }
        let mut added = BTreeMap::new();
        let mut previous = None;
        for descriptor in &self.publish {
            let id = descriptor.id()?;
            if previous.is_some_and(|previous| previous >= id)
                || published.definition(&id).is_some()
            {
                return Err(ChangeSetError::NonCanonicalPublication);
            }
            previous = Some(id);
            added.insert(id, descriptor.clone());
        }
        let lookup = Overlay {
            base: published,
            added: &added,
        };

        let mut installed: BTreeSet<DefinitionId> =
            predecessor.installed_storage.iter().copied().collect();
        let mut active: BTreeSet<DefinitionId> = predecessor.active.iter().copied().collect();
        let mut bindings: BTreeMap<String, BindingV1> = predecessor
            .bindings
            .iter()
            .map(|binding| (binding.wire_id.clone(), binding.clone()))
            .collect();
        let mut restrictions: BTreeMap<DefinitionId, RestrictionV1> = predecessor
            .restrictions
            .iter()
            .map(|entry| (entry.definition, entry.restriction))
            .collect();
        let mut closures: BTreeMap<[u8; 32], CompletionClosureV1> = predecessor
            .completion_closures
            .iter()
            .map(|closure| (closure.obligation, closure.clone()))
            .collect();
        let mut kernel = predecessor.kernel;
        let mut commitment_profile = predecessor.commitment_profile;
        let mut services_profile = predecessor.services_profile;

        for (index, transition) in self.transitions.iter().enumerate() {
            let fail = |reason| ChangeSetError::Transition { index, reason };
            let needed = |id: &DefinitionId, closures: &BTreeMap<[u8; 32], CompletionClosureV1>| {
                closures
                    .values()
                    .any(|closure| closure.definitions.binary_search(id).is_ok())
            };
            match transition {
                SelectionTransitionV1::InstallStorage(id) => {
                    let Some(DefinitionDescriptorV1::Storage(storage)) = lookup.definition(id)
                    else {
                        return Err(fail("only a published storage definition can be installed"));
                    };
                    if matches!(storage.shape, StorageShapeV1::Cell) && storage.initial.is_none() {
                        return Err(fail(
                            "a cell without initial bytes is installed only by the act that founds its root",
                        ));
                    }
                    if !installed.insert(*id) {
                        return Err(fail("storage is already installed"));
                    }
                }
                SelectionTransitionV1::RetireStorage(id) => {
                    if needed(id, &closures) {
                        return Err(ChangeSetError::ObligationStranded(*id));
                    }
                    if !installed.remove(id) {
                        return Err(fail("storage is not installed"));
                    }
                }
                SelectionTransitionV1::Activate(id) => {
                    let Some(descriptor) = lookup.definition(id) else {
                        return Err(fail("the definition is not published"));
                    };
                    if !activatable(descriptor.kind()) {
                        return Err(fail("this kind is installed or set, not activated"));
                    }
                    if !active.insert(*id) {
                        return Err(fail("the definition is already active"));
                    }
                }
                SelectionTransitionV1::Deactivate(id) => {
                    if !matches!(restrictions.get(id), Some(RestrictionV1::Disabled(_))) {
                        return Err(fail("only a disabled definition can be deactivated"));
                    }
                    if needed(id, &closures) {
                        return Err(ChangeSetError::ObligationStranded(*id));
                    }
                    active.remove(id);
                    restrictions.remove(id);
                }
                SelectionTransitionV1::Bind(binding) => {
                    bindings.insert(binding.wire_id.clone(), binding.clone());
                }
                SelectionTransitionV1::Unbind(wire_id) => {
                    if bindings.remove(wire_id).is_none() {
                        return Err(fail("the wire ID is not bound"));
                    }
                }
                SelectionTransitionV1::Restrict {
                    definition,
                    restriction,
                } => {
                    if matches!(restriction, RestrictionKindV1::Disabled)
                        && needed(definition, &closures)
                    {
                        return Err(ChangeSetError::ObligationStranded(*definition));
                    }
                    restrictions.insert(*definition, restriction.at(self.effective_height));
                }
                SelectionTransitionV1::Lift(id) => {
                    if restrictions.remove(id).is_none() {
                        return Err(fail("the definition is not restricted"));
                    }
                }
                SelectionTransitionV1::SetKernel(id) => kernel = *id,
                SelectionTransitionV1::SetCommitmentProfile(id) => commitment_profile = *id,
                SelectionTransitionV1::SetServicesProfile(id) => services_profile = *id,
                SelectionTransitionV1::RetainCompletion(closure) => {
                    if closures
                        .insert(closure.obligation, closure.clone())
                        .is_some()
                    {
                        return Err(fail("the obligation is already retained"));
                    }
                }
                SelectionTransitionV1::ReleaseCompletion(obligation) => {
                    if closures.remove(obligation).is_none() {
                        return Err(fail("the obligation is not retained"));
                    }
                }
            }
        }

        let selection = RootSelectionV1 {
            kernel,
            commitment_profile,
            services_profile,
            installed_storage: installed.into_iter().collect(),
            active: active.into_iter().collect(),
            bindings: bindings.into_values().collect(),
            restrictions: restrictions
                .into_iter()
                .map(|(definition, restriction)| RestrictionEntryV1 {
                    definition,
                    restriction,
                })
                .collect(),
            completion_closures: closures.into_values().collect(),
        };
        selection.check_closure(&lookup)?;
        if selection.id()? != self.successor {
            return Err(ChangeSetError::SuccessorMismatch);
        }
        Ok(AppliedChangeSet {
            selection,
            published: added,
        })
    }
}

/// A validator's statement that its release executes a change set's exact successor selection.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::SupportAttestationV1")]
pub struct SupportAttestationV1 {
    /// Network.
    pub network: NetworkId,
    /// Execution root.
    pub root: RootScopeV1,
    /// The change set whose successor is supported.
    pub change_set: ChangeSetId,
    /// Expected predecessor selection.
    pub predecessor: SelectionId,
    /// Exact successor selection.
    pub successor: SelectionId,
    /// [`committee_digest`] of the committee the attestation is bound to; a committee change
    /// requires fresh attestations.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub committee: [u8; 32],
    /// Height of the checkpoint the validator tested the release on.
    pub tested_checkpoint: u64,
    /// First height at which the attestation counts.
    pub valid_from: u64,
    /// Height from which the attestation no longer counts.
    pub valid_until: u64,
    /// Digest of the release attestation (binary digest, target, toolchain, features,
    /// provenance and implemented definitions).
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub release: [u8; 32],
    /// The attesting validator.
    pub validator: PublicKey,
}

/// A support attestation with the validator's signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::SignedSupportAttestationV1")]
pub struct SignedSupportAttestationV1 {
    /// The attestation.
    pub attestation: SupportAttestationV1,
    /// Signature over the domain-separated canonical attestation.
    pub signature: Signature,
}

/// A lifecycle record whose signature or encoding is invalid.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleSignatureError {
    /// The record cannot be encoded canonically.
    #[error("record encoding failed: {0}")]
    Encoding(String),
    /// The signature does not verify.
    #[error("signature does not verify")]
    BadSignature,
    /// The key cannot sign.
    #[error("signing failed: {0}")]
    Signing(String),
}

fn signing_payload(
    domain: &[u8],
    value: &impl norito::core::NoritoSerialize,
) -> Result<Vec<u8>, LifecycleSignatureError> {
    let bytes = norito::encode_canonical(value)
        .map_err(|error| LifecycleSignatureError::Encoding(error.to_string()))?;
    let mut payload = Vec::with_capacity(domain.len() + bytes.len());
    payload.extend_from_slice(domain);
    payload.extend_from_slice(&bytes);
    Ok(payload)
}

impl SupportAttestationV1 {
    /// Sign with the validator's key.
    ///
    /// # Errors
    /// The attestation cannot be encoded or the key cannot sign.
    pub fn sign(
        self,
        key: &PrivateKey,
    ) -> Result<SignedSupportAttestationV1, LifecycleSignatureError> {
        let payload = signing_payload(SUPPORT_ATTESTATION_DOMAIN, &self)?;
        let signature = Signature::try_new(key, &payload)
            .map_err(|error| LifecycleSignatureError::Signing(error.to_string()))?;
        Ok(SignedSupportAttestationV1 {
            attestation: self,
            signature,
        })
    }

    /// Whether the attestation counts for `change_set` at `height` under the committee whose
    /// digest is `committee`.
    #[must_use]
    pub fn counts_for(&self, change_set: ChangeSetId, height: u64, committee: [u8; 32]) -> bool {
        self.change_set == change_set
            && self.committee == committee
            && self.valid_from <= height
            && height < self.valid_until
    }
}

impl SignedSupportAttestationV1 {
    /// Verify the signature against the attesting validator's key.
    ///
    /// # Errors
    /// The attestation cannot be encoded or the signature does not verify.
    pub fn verify(&self) -> Result<(), LifecycleSignatureError> {
        let payload = signing_payload(SUPPORT_ATTESTATION_DOMAIN, &self.attestation)?;
        self.signature
            .verify(&self.attestation.validator, &payload)
            .map_err(|_| LifecycleSignatureError::BadSignature)
    }
}

/// Identity of a committee as a set of members: `H(COMMITTEE_DOMAIN ‖ canonical member keys in
/// key order)`. The leader order the consensus derives within one committee does not change it;
/// any change of membership does, so attestations are given again (§6, "Activate").
///
/// # Errors
/// The keys cannot be encoded.
pub fn committee_digest(committee: &[PublicKey]) -> Result<[u8; 32], LifecycleSignatureError> {
    let mut members = committee.to_vec();
    members.sort();
    members.dedup();
    let bytes = norito::encode_canonical(&members)
        .map_err(|error| LifecycleSignatureError::Encoding(error.to_string()))?;
    Ok(Hash::new_from_chunks(&[COMMITTEE_DOMAIN, &bytes]).into())
}

/// Whether every member of `committee` has a valid attestation for `change_set` that counts at
/// `height` and is bound to exactly this committee.
///
/// Readiness comes from all `n` validators, never a quorum (§6.4).
#[must_use]
pub fn committee_supports(
    committee: &[PublicKey],
    attestations: &[SignedSupportAttestationV1],
    change_set: ChangeSetId,
    height: u64,
) -> bool {
    let Ok(digest) = committee_digest(committee) else {
        return false;
    };
    committee.iter().all(|member| {
        attestations.iter().any(|signed| {
            signed.attestation.validator == *member
                && signed.attestation.counts_for(change_set, height, digest)
                && signed.verify().is_ok()
        })
    })
}

/// The trust anchor of a network that adopted the foundation at a checkpoint (§13).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::AdoptionRecordV1")]
pub struct AdoptionRecordV1 {
    /// Network that adopted.
    pub network: NetworkId,
    /// Height of the last predecessor block.
    pub height: u64,
    /// Hash of that block.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub block_hash: [u8; 32],
    /// The predecessor's certified World root at that block.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub predecessor_world_root: [u8; 32],
    /// Root of the imported State under the foundation's commitment profile.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub imported_state_root: [u8; 32],
    /// [`committee_digest`] of the committee at the adoption height.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub committee: [u8; 32],
    /// The foundation's initial selection.
    pub selection: SelectionId,
    /// Hash of the certified transaction that authorized the adoption.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub authorization: [u8; 32],
}

impl AdoptionRecordV1 {
    /// Identity of the trust anchor: `H(ADOPTION_RECORD_DOMAIN ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The record cannot be encoded.
    pub fn id(&self) -> Result<[u8; 32], LifecycleSignatureError> {
        let bytes = norito::encode_canonical(self)
            .map_err(|error| LifecycleSignatureError::Encoding(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[ADOPTION_RECORD_DOMAIN, &bytes]).into())
    }
}

/// What kind of work an obligation leaves behind (§7.2).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ObligationKindV1")]
#[norito(tag = "kind", content = "content")]
pub enum ObligationKindV1 {
    /// Value held in escrow.
    Escrow,
    /// Consent collected but not yet executed, such as a multisig proposal.
    PendingConsent,
    /// A scheduled or repeating trigger.
    ScheduledTrigger,
    /// A migration cursor.
    MigrationCursor,
    /// An AMX participation awaiting decision or settlement.
    AmxParticipation,
}

/// An obligation registered through the kernel, with what its completion needs.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ObligationRecordV1")]
pub struct ObligationRecordV1 {
    /// What kind of work it is.
    pub kind: ObligationKindV1,
    /// The implementation that created it.
    pub creator: DefinitionId,
    /// Height at which it was created.
    pub created_at: u64,
    /// Definitions its completion needs; `closure.obligation` is the obligation's identity.
    pub closure: CompletionClosureV1,
    /// Terminal effects the completion may perform, sorted and unique.
    pub terminal_effects: Vec<String>,
    /// Resource budget reserved for completion.
    pub budget: ResourceLimitsV1,
}

/// What emergency containment may impose (§7.1).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ContainmentKindV1")]
#[norito(tag = "kind", content = "content")]
pub enum ContainmentKindV1 {
    /// Close admission of new work.
    AdmissionClosed,
    /// Keep only completion capabilities.
    CompletionOnly,
}

/// Emergency containment of definitions, pending ratification (§7.1).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ContainmentV1")]
pub struct ContainmentV1 {
    /// Network.
    pub network: NetworkId,
    /// Execution root.
    pub root: RootScopeV1,
    /// Identity of the incident.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub incident: [u8; 32],
    /// Contained definitions, sorted and unique.
    pub targets: Vec<DefinitionId>,
    /// What containment imposes.
    pub kind: ContainmentKindV1,
    /// End of the lease in committed milliseconds; unratified containment expires then.
    pub lease_until_ms: u64,
}

/// One panel member's endorsement of a containment.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::EndorsementV1")]
pub struct EndorsementV1 {
    /// The endorsed containment.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub containment: [u8; 32],
    /// The endorsing member.
    pub member: PublicKey,
    /// Committed milliseconds at which the member endorsed.
    pub endorsed_at_ms: u64,
    /// Signature over the domain-separated containment identity and time.
    pub signature: Signature,
}

/// Containment that is not authorized.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ContainmentError {
    /// The containment cannot be encoded canonically.
    #[error("containment encoding failed: {0}")]
    Encoding(String),
    /// The containment names no target, or its targets are unsorted or repeated.
    #[error("containment targets must be non-empty, sorted and unique")]
    Targets,
    /// Fewer distinct panel members endorsed it in time than the threshold requires.
    #[error("{have} valid endorsements, {need} required")]
    Threshold {
        /// Valid endorsements found.
        have: usize,
        /// Endorsements required.
        need: usize,
    },
}

impl ContainmentV1 {
    /// Identity: `H(CONTAINMENT_DOMAIN ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The containment cannot be encoded.
    pub fn id(&self) -> Result<[u8; 32], ContainmentError> {
        let bytes = norito::encode_canonical(self)
            .map_err(|error| ContainmentError::Encoding(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[CONTAINMENT_DOMAIN, &bytes]).into())
    }

    /// Check that at least `threshold` distinct members of `panel` endorsed this containment
    /// within [`ENDORSEMENT_LIFETIME_MS`] before `now_ms`.
    ///
    /// # Errors
    /// The targets are malformed or the threshold is not met.
    pub fn check_endorsements(
        &self,
        panel: &[PublicKey],
        threshold: usize,
        now_ms: u64,
        endorsements: &[EndorsementV1],
    ) -> Result<(), ContainmentError> {
        if self.targets.is_empty() || self.targets.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ContainmentError::Targets);
        }
        let id = self.id()?;
        let mut members = BTreeSet::new();
        for endorsement in endorsements {
            let fresh = endorsement.endorsed_at_ms <= now_ms
                && now_ms - endorsement.endorsed_at_ms <= ENDORSEMENT_LIFETIME_MS;
            if endorsement.containment == id
                && fresh
                && panel.contains(&endorsement.member)
                && endorsement.verify().is_ok()
            {
                members.insert(endorsement.member.clone());
            }
        }
        if members.len() >= threshold {
            Ok(())
        } else {
            Err(ContainmentError::Threshold {
                have: members.len(),
                need: threshold,
            })
        }
    }
}

impl EndorsementV1 {
    fn payload(containment: &[u8; 32], endorsed_at_ms: u64) -> Vec<u8> {
        let mut payload = Vec::with_capacity(ENDORSEMENT_DOMAIN.len() + 40);
        payload.extend_from_slice(ENDORSEMENT_DOMAIN);
        payload.extend_from_slice(containment);
        payload.extend_from_slice(&endorsed_at_ms.to_le_bytes());
        payload
    }

    /// Endorse `containment` at `endorsed_at_ms` with a panel member's key.
    ///
    /// # Errors
    /// The key cannot sign.
    pub fn sign(
        containment: [u8; 32],
        endorsed_at_ms: u64,
        member: PublicKey,
        key: &PrivateKey,
    ) -> Result<Self, LifecycleSignatureError> {
        let signature = Signature::try_new(key, &Self::payload(&containment, endorsed_at_ms))
            .map_err(|error| LifecycleSignatureError::Signing(error.to_string()))?;
        Ok(Self {
            containment,
            member,
            endorsed_at_ms,
            signature,
        })
    }

    /// Verify the member's signature.
    ///
    /// # Errors
    /// The signature does not verify.
    pub fn verify(&self) -> Result<(), LifecycleSignatureError> {
        self.signature
            .verify(
                &self.member,
                &Self::payload(&self.containment, self.endorsed_at_ms),
            )
            .map_err(|_| LifecycleSignatureError::BadSignature)
    }
}

/// One definition a containment restricted, with what it replaced.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ContainedDefinitionV1")]
pub struct ContainedDefinitionV1 {
    /// The contained definition.
    pub definition: DefinitionId,
    /// Its restriction before containment, restored when the containment expires while it still
    /// holds the restriction it imposed.
    pub previous: Option<RestrictionV1>,
    /// The restriction the containment imposed.
    pub imposed: RestrictionV1,
}

/// Where an authorized containment stands.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ContainmentStatusV1")]
#[norito(tag = "kind", content = "content")]
pub enum ContainmentStatusV1 {
    /// In force until its lease ends.
    Active,
    /// A renewal of the same incident took it over at this height.
    Renewed(u64),
    /// Its lease ended at this height; restrictions it still held were restored.
    Expired(u64),
}

/// An authorized containment and its effect, keyed by containment identity in State.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ContainmentRecordV1")]
pub struct ContainmentRecordV1 {
    /// The containment.
    pub containment: ContainmentV1,
    /// Height of the block that authorized it; it restricts from the next height.
    pub authorized_height: u64,
    /// Committed time of that block.
    pub authorized_ms: u64,
    /// The definitions it restricted, sorted by definition.
    pub contained: Vec<ContainedDefinitionV1>,
    /// Where it stands.
    pub status: ContainmentStatusV1,
}

impl ContainmentRecordV1 {
    /// Committed time from which a renewal with fresh endorsements may replace it: halfway
    /// through its lease (§7.1, "Expiry").
    #[must_use]
    pub fn renewal_opens_ms(&self) -> u64 {
        let lease = self
            .containment
            .lease_until_ms
            .saturating_sub(self.authorized_ms);
        self.authorized_ms.saturating_add(lease / 2)
    }
}

/// What a transaction expects behind the wire IDs it uses (§8.1).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ExpectationV1")]
pub struct ExpectationV1 {
    /// Expected bindings, including those of interfaces called transitively, sorted by wire ID.
    pub bindings: Vec<BindingV1>,
    /// Expected execution services profile.
    pub services_profile: DefinitionId,
}

/// Stable rejection of the live-upgrade mechanism, committed in transaction results.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    PartialOrd,
    Ord,
    displaydoc::Display,
    thiserror::Error,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::RuntimeRejectionV1")]
#[norito(tag = "kind", content = "content")]
#[ignore_extra_doc_attributes]
pub enum RuntimeRejectionV1 {
    /// No active interface answers wire ID {wire_id}
    ///
    /// The first envelope in canonical pre-order whose wire ID is unknown or not active.
    DefinitionInactive {
        /// Its wire ID.
        wire_id: String,
    },
    /// Definition {definition} is restricted: {restriction:?}
    DefinitionRestricted {
        /// The definition.
        definition: DefinitionId,
        /// Its restriction.
        restriction: RestrictionV1,
    },
    /// A binding or the services profile differs from the expectation (wire ID {wire_id:?})
    ExpectationMismatch {
        /// The wire ID whose binding differs, or `None` for the services profile.
        wire_id: Option<String>,
    },
}

impl RuntimeRejectionV1 {
    /// Stable code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DefinitionInactive { .. } => "definition_inactive",
            Self::DefinitionRestricted { .. } => "definition_restricted",
            Self::ExpectationMismatch { .. } => "expectation_mismatch",
        }
    }
}

impl ExpectationV1 {
    /// Check the expectation against a selection.
    ///
    /// # Errors
    /// The stable rejection for the first difference.
    pub fn check(&self, selection: &RootSelectionV1) -> Result<(), RuntimeRejectionV1> {
        if self.services_profile != selection.services_profile {
            return Err(RuntimeRejectionV1::ExpectationMismatch { wire_id: None });
        }
        for expected in &self.bindings {
            let actual = selection
                .bindings
                .binary_search_by(|binding| binding.wire_id.cmp(&expected.wire_id))
                .ok()
                .map(|index| &selection.bindings[index]);
            if actual != Some(expected) {
                return Err(RuntimeRejectionV1::ExpectationMismatch {
                    wire_id: Some(expected.wire_id.clone()),
                });
            }
        }
        Ok(())
    }
}

/// Whether `kind` may appear in [`RootSelectionV1::active`].
#[must_use]
pub const fn activatable(kind: DefinitionKind) -> bool {
    !matches!(
        kind,
        DefinitionKind::Storage
            | DefinitionKind::ServicesProfile
            | DefinitionKind::CommitmentProfile
            | DefinitionKind::Kernel
    )
}

/// Where a governed change set stands.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ChangeSetStatusV1")]
#[norito(tag = "kind", content = "content")]
pub enum ChangeSetStatusV1 {
    /// Approved; it applies at its effective height once every validator supports it.
    Scheduled,
    /// Applied at this height.
    Activated(u64),
    /// Its effective height passed without complete, current support or notice; it never applies.
    Lapsed(u64),
    /// Governance withdrew it at this height, before activation.
    Cancelled(u64),
}

/// A governed change set and its progress, keyed by [`ChangeSetId`] in State.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::ChangeSetRecordV1")]
pub struct ChangeSetRecordV1 {
    /// The change set.
    pub change_set: ChangeSetV1,
    /// Where it stands.
    pub status: ChangeSetStatusV1,
    /// Committed time of the block that admitted its proposal.
    pub proposed_ms: u64,
    /// Height of the block in which governance approved it.
    pub approved_height: u64,
    /// Committed time of that block.
    pub approved_ms: u64,
    /// The notice policy in force when the proposal was admitted (§6, "Notice").
    pub notice: RuntimeNoticeV1,
    /// The latest attestation of each validator, sorted by validator key.
    pub attestations: Vec<SignedSupportAttestationV1>,
}

/// Minimum notice of one kind of change: blocks and committed time (§6, "Notice").
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::RuntimeNoticeV1")]
pub struct RuntimeNoticeV1 {
    /// Fewest blocks between approval and the effective height.
    pub blocks: u64,
    /// Fewest committed milliseconds between proposal and activation.
    pub ms: u64,
    /// Fewest committed milliseconds between approval and activation.
    pub approval_cushion_ms: u64,
}

impl ChangeSetRecordV1 {
    /// Whether the committed block time `now_ms` satisfies the frozen notice.
    #[must_use]
    pub fn notice_elapsed(&self, now_ms: u64) -> bool {
        now_ms >= self.proposed_ms.saturating_add(self.notice.ms)
            && now_ms
                >= self
                    .approved_ms
                    .saturating_add(self.notice.approval_cushion_ms)
    }
}

/// What a runtime-upgrade proposal asks SORA Parliament to enact (§6).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::lifecycle::RuntimeUpgradeActionV1")]
#[norito(tag = "kind", content = "content")]
pub enum RuntimeUpgradeActionV1 {
    /// Schedule a change set at its effective height.
    Schedule(ChangeSetV1),
    /// Withdraw a scheduled change set before it takes effect.
    Cancel(ChangeSetId),
}

/// A minimal scheduling action for codec fixtures: no publication and no transitions.
#[cfg(test)]
pub(crate) fn test_schedule(effective_height: u64) -> RuntimeUpgradeActionV1 {
    use iroha_crypto::HashOf;
    RuntimeUpgradeActionV1::Schedule(ChangeSetV1 {
        network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0x11; 32],
        ))),
        root: RootScopeV1::Universal,
        predecessor: SelectionId([1; 32]),
        successor: SelectionId([2; 32]),
        effective_height,
        publish: Vec::new(),
        transitions: Vec::new(),
        qualification: [3; 32],
    })
}
