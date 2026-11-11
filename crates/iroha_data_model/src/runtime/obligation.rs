//! Obligations registered through the kernel and migration cursors
//! (`specs/live_runtime_upgrades.md` §5.4, §7.2).
//!
//! An effect that leaves work behind registers it with the kernel of its root. A registration
//! commits to the exact instructions that complete the obligation, its terminal effects, and
//! captures their completion closure from the selection the root executes at that moment
//! ([`CompletionCaptureV1`]): the implementation each terminal effect resolves to, the
//! implementation every interface those implementations invoke resolves to, and the kernel,
//! profiles and host profiles they run under.
//!
//! While the obligation is live, the kernel keeps that settlement authority:
//!
//! - when its owner issues one of the committed instructions, it runs although the definitions
//!   behind it have admission closed or are completion-only, once;
//! - a change set that would change a captured binding or stop executing a captured definition
//!   lapses.
//!
//! A capability is narrow. It covers only the exact instructions committed at registration, each
//! once, only for its owner and until it expires, so it creates no new work. An owner holds one
//! obligation, and registering again replaces it under the bindings the root then executes.
//! Registration never changes the selection, so it cannot invalidate a scheduled change set.
//!
//! A contract cannot call the kernel under the first-release ABI, whose descriptor fixes every
//! syscall and operation tag. The account that controls a contract's lifecycle therefore
//! registers the obligation for it, the contract's subject account owns it, and the committed
//! instructions the contract itself queues complete it.
//!
//! A migration is the kernel's own obligation. Its committed [`MigrationRecordV1`] holds the
//! cursor that survives restarts and suspension until the old storage is retired.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use norito::codec::{Decode, Encode};

use super::{
    definition::{
        BindingV1, DefinitionDescriptorV1, DefinitionId, DefinitionLookup, EntrypointV1,
        RestrictionV1, RootSelectionV1,
    },
    lifecycle::ObligationKindV1,
};
use crate::{
    account::AccountId,
    isi::{InstructionBox, instruction_wire_id},
};

/// Domain separator of [`ObligationId`].
pub const OBLIGATION_ID_DOMAIN: &[u8] = b"iroha:runtime:obligation:v1\0";
/// Most terminal effects one obligation registers.
pub const MAX_TERMINAL_EFFECTS: usize = 16;
/// Domain separator of a terminal effect's envelope identity.
pub const TERMINAL_EFFECT_DOMAIN: &[u8] = b"iroha:runtime:terminal-effect:v1\0";
/// Most nested bindings a capture lists besides its default implementation.
pub const MAX_NESTED_BINDINGS: usize = 64;
/// Longest committed time an obligation stays live after its registration: 30 days, the longest
/// ordinary-disable notice of the design. Closing admission to registration therefore drains
/// every capability within one such notice; an obligation that needs longer registers again,
/// under the bindings the root then executes.
pub const MAX_OBLIGATION_LIFETIME_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// Wire-ID prefix of the lifecycle's own instructions, which no obligation registers as a
/// terminal effect.
pub const LIFECYCLE_WIRE_PREFIX: &str = "iroha.runtime.";

/// Identity of an obligation registered through the kernel.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::ObligationId")]
pub struct ObligationId(pub [u8; 32]);

super::definition::hex_identity_json!(ObligationId);

impl core::fmt::Display for ObligationId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl ObligationId {
    /// The identity of the obligation `owner` holds: `H(OBLIGATION_ID_DOMAIN ‖ canonical
    /// owner)`. An owner holds one obligation on a root, so the kernel finds it from the owner
    /// alone.
    ///
    /// # Errors
    /// The owner cannot be encoded.
    pub fn derive(owner: &AccountId) -> Result<Self, ObligationError> {
        let owner = norito::encode_canonical(owner)
            .map_err(|error| ObligationError::Encoding(error.to_string()))?;
        Ok(Self(
            Hash::new_from_chunks(&[OBLIGATION_ID_DOMAIN, &owner]).into(),
        ))
    }
}

/// An obligation or capture that breaks the kernel's rules.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ObligationError {
    /// The terminal effects are empty, too many or repeated, or one of them is an envelope this
    /// release kept opaque, has no wire ID or is one of the lifecycle's own instructions.
    #[error(
        "terminal effects must be 1 to {MAX_TERMINAL_EFFECTS} distinct decoded instructions outside the lifecycle's own"
    )]
    TerminalEffects,
    /// The expiry is not after the registration or exceeds [`MAX_OBLIGATION_LIFETIME_MS`].
    #[error("an obligation expires after its registration and within 30 days of it")]
    Expiry,
    /// The kind is one only the kernel itself registers.
    #[error("{0:?} obligations are registered by the kernel, not by an instruction")]
    Kind(ObligationKindV1),
    /// The selection binds no implementation to a terminal effect.
    #[error("no active interface answers terminal effect {0}")]
    Unbound(String),
    /// A captured definition is not published.
    #[error("captured definition {0} is not published")]
    Unpublished(DefinitionId),
    /// The interface or implementation of a terminal effect has admission closed, is
    /// completion-only or is disabled: registering is new work.
    #[error("definition {0} of a terminal effect is restricted, and registering is new work")]
    Restricted(DefinitionId),
    /// The closure binds more nested interfaces to other implementations than a capture lists.
    #[error("the completion closure lists more than {MAX_NESTED_BINDINGS} nested bindings")]
    TooWide,
    /// A capture's lists are unsorted or repeated.
    #[error("capture {0} must be sorted and unique")]
    NonCanonical(&'static str),
    /// The value cannot be encoded canonically.
    #[error("obligation encoding failed: {0}")]
    Encoding(String),
}

/// The identity of a terminal effect: `H(TERMINAL_EFFECT_DOMAIN ‖ canonical instruction)`. It
/// binds the exact envelope, wire ID and payload, so only that instruction completes.
///
/// # Errors
/// The instruction cannot be encoded.
pub fn terminal_effect_id(instruction: &InstructionBox) -> Result<[u8; 32], ObligationError> {
    let bytes = norito::encode_canonical(instruction)
        .map_err(|error| ObligationError::Encoding(error.to_string()))?;
    Ok(Hash::new_from_chunks(&[TERMINAL_EFFECT_DOMAIN, &bytes]).into())
}

/// One instruction a registered obligation committed to, and whether it has completed.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::TerminalEffectV1")]
pub struct TerminalEffectV1 {
    /// [`terminal_effect_id`] of the committed instruction.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub envelope: [u8; 32],
    /// Its wire ID.
    pub wire_id: String,
    /// Whether the owner has performed it. A performed effect lends no capability again.
    pub performed: bool,
}

/// The terminal effects of a registration, sorted by envelope identity, and their wire IDs,
/// sorted and unique.
///
/// # Errors
/// [`ObligationError::TerminalEffects`], or an instruction cannot be encoded.
pub fn terminal_effects(
    instructions: &[InstructionBox],
) -> Result<(Vec<TerminalEffectV1>, Vec<String>), ObligationError> {
    if instructions.is_empty() || instructions.len() > MAX_TERMINAL_EFFECTS {
        return Err(ObligationError::TerminalEffects);
    }
    let mut effects = Vec::with_capacity(instructions.len());
    let mut wire_ids = BTreeSet::new();
    for instruction in instructions {
        let wire_id = instruction_wire_id(instruction)
            .filter(|wire_id| {
                instruction.as_opaque().is_none() && !wire_id.starts_with(LIFECYCLE_WIRE_PREFIX)
            })
            .ok_or(ObligationError::TerminalEffects)?;
        wire_ids.insert(wire_id.to_owned());
        effects.push(TerminalEffectV1 {
            envelope: terminal_effect_id(instruction)?,
            wire_id: wire_id.to_owned(),
            performed: false,
        });
    }
    effects.sort_unstable();
    if effects
        .windows(2)
        .any(|pair| pair[0].envelope == pair[1].envelope)
    {
        return Err(ObligationError::TerminalEffects);
    }
    Ok((effects, wire_ids.into_iter().collect()))
}

/// The completion closure of an obligation, captured from the selection it was registered under.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::CompletionCaptureV1")]
pub struct CompletionCaptureV1 {
    /// The kernel the completion is sequenced by.
    pub kernel: DefinitionId,
    /// The commitment profile its effects are committed under.
    pub commitment_profile: DefinitionId,
    /// The execution services profile it runs under.
    pub services_profile: DefinitionId,
    /// The host profiles the selection activated, sorted and unique: a terminal effect may run a
    /// contract, and the contract that owns the obligation runs under one.
    pub host_profiles: Vec<DefinitionId>,
    /// The captured binding of every terminal effect, sorted by wire ID.
    pub terminal: Vec<BindingV1>,
    /// The implementation most nested interfaces of the closure were bound to: every interface
    /// it answers that neither `terminal` nor `nested` lists resolves to it.
    pub default_implementation: Option<DefinitionId>,
    /// The captured binding of every other interface the closure reaches that was bound to
    /// another implementation than `default_implementation`, sorted by wire ID.
    pub nested: Vec<BindingV1>,
}

fn binding<'a>(bindings: &'a [BindingV1], wire_id: &str) -> Option<&'a BindingV1> {
    bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .ok()
        .map(|index| &bindings[index])
}

/// The restriction of `definition` that stops new work at `height`, if any.
fn blocking(selection: &RootSelectionV1, definition: &DefinitionId, height: u64) -> bool {
    selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(definition))
        .is_ok_and(|index| match selection.restrictions[index].restriction {
            RestrictionV1::Deprecated(_) => false,
            RestrictionV1::AdmissionClosed(from)
            | RestrictionV1::CompletionOnly(from)
            | RestrictionV1::Disabled(from) => from <= height,
        })
}

impl CompletionCaptureV1 {
    /// Capture the completion closure of `terminal_effects` from `selection` at `height`.
    ///
    /// The closure is what the selection's bindings of the terminal effects reach: their
    /// interfaces and implementations, every definition those declare, and the bindings of the
    /// interfaces they declare, transitively; with the kernel, both profiles and every host
    /// profile the selection activates.
    ///
    /// Registering is new work, so the interface and implementation of a terminal effect may not
    /// be restricted at `height`. A restriction elsewhere in the closure does not refuse the
    /// registration: the bundle that answers one instruction declares every instruction it may
    /// invoke, and one contained definition must not stop every registration.
    ///
    /// # Errors
    /// A terminal effect is unbound or restricted, a reached definition is unpublished, or the
    /// closure lists too many nested bindings.
    pub fn capture(
        selection: &RootSelectionV1,
        lookup: &impl DefinitionLookup,
        terminal_effects: &[String],
        height: u64,
    ) -> Result<Self, ObligationError> {
        let mut terminal = Vec::with_capacity(terminal_effects.len());
        let mut reached: BTreeMap<&str, &BindingV1> = BTreeMap::new();
        let mut pending = Vec::new();
        for wire_id in terminal_effects {
            let bound = binding(&selection.bindings, wire_id)
                .ok_or_else(|| ObligationError::Unbound(wire_id.clone()))?;
            if let Some(restricted) = [bound.interface, bound.implementation]
                .into_iter()
                .find(|definition| blocking(selection, definition, height))
            {
                return Err(ObligationError::Restricted(restricted));
            }
            terminal.push(bound.clone());
            reached.insert(bound.wire_id.as_str(), bound);
            pending.extend([bound.interface, bound.implementation]);
        }
        let host_profiles: Vec<DefinitionId> = selection
            .active
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    lookup.definition(id),
                    Some(DefinitionDescriptorV1::HostProfile(_))
                )
            })
            .collect();
        pending.extend(host_profiles.iter().copied());
        let mut visited = BTreeSet::new();
        while let Some(definition) = pending.pop() {
            if !visited.insert(definition) {
                continue;
            }
            let descriptor = lookup
                .definition(&definition)
                .ok_or(ObligationError::Unpublished(definition))?;
            if let DefinitionDescriptorV1::Interface(interface) = descriptor
                && let Some(bound) = binding(&selection.bindings, &interface.wire_id)
                && bound.interface == definition
            {
                reached.insert(bound.wire_id.as_str(), bound);
                pending.push(bound.implementation);
            }
            pending.extend(
                descriptor
                    .typed_dependencies()
                    .into_iter()
                    .map(|(_, id)| id),
            );
        }
        // The implementation most reached interfaces are bound to answers them by default; the
        // rest are listed. Ties go to the smaller identity.
        let mut answered: BTreeMap<DefinitionId, usize> = BTreeMap::new();
        for bound in reached.values() {
            *answered.entry(bound.implementation).or_default() += 1;
        }
        let default_implementation = answered
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then(right.0.cmp(left.0)))
            .map(|(implementation, _)| *implementation);
        let nested: Vec<BindingV1> = reached
            .values()
            .filter(|bound| {
                Some(bound.implementation) != default_implementation
                    && binding(&terminal, &bound.wire_id).is_none()
            })
            .map(|bound| (*bound).clone())
            .collect();
        if nested.len() > MAX_NESTED_BINDINGS {
            return Err(ObligationError::TooWide);
        }
        Ok(Self {
            kernel: selection.kernel,
            commitment_profile: selection.commitment_profile,
            services_profile: selection.services_profile,
            host_profiles,
            terminal,
            default_implementation,
            nested,
        })
    }

    /// Check the capture's own format rules.
    ///
    /// # Errors
    /// A list is unsorted, repeated or over its bound.
    pub fn validate(&self) -> Result<(), ObligationError> {
        let sorted = |bindings: &[BindingV1]| {
            bindings
                .windows(2)
                .all(|pair| pair[0].wire_id < pair[1].wire_id)
        };
        if self.terminal.is_empty()
            || self.terminal.len() > MAX_TERMINAL_EFFECTS
            || !sorted(&self.terminal)
        {
            return Err(ObligationError::NonCanonical("terminal bindings"));
        }
        if self.nested.len() > MAX_NESTED_BINDINGS
            || !sorted(&self.nested)
            || self
                .nested
                .iter()
                .any(|nested| binding(&self.terminal, &nested.wire_id).is_some())
        {
            return Err(ObligationError::NonCanonical("nested bindings"));
        }
        if !self.host_profiles.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(ObligationError::NonCanonical("host profiles"));
        }
        Ok(())
    }

    /// The wire IDs of the terminal effects, sorted.
    pub fn terminal_effects(&self) -> impl Iterator<Item = &str> {
        self.terminal.iter().map(|bound| bound.wire_id.as_str())
    }

    /// The implementation the capture resolves `wire_id` to, whose active interface is
    /// `interface`: its terminal or nested binding, or the default implementation when that
    /// answers the interface. `None` when the closure does not reach the interface, which then
    /// resolves against the root's current selection.
    #[must_use]
    pub fn implementation_for(
        &self,
        wire_id: &str,
        interface: &DefinitionId,
        lookup: &impl DefinitionLookup,
    ) -> Option<DefinitionId> {
        if let Some(bound) =
            binding(&self.terminal, wire_id).or_else(|| binding(&self.nested, wire_id))
        {
            return (bound.interface == *interface).then_some(bound.implementation);
        }
        let default = self.default_implementation?;
        match lookup.definition(&default) {
            Some(DefinitionDescriptorV1::Implementation(implementation)) => {
                let EntrypointV1::Instructions(interfaces) = &implementation.entrypoint else {
                    return None;
                };
                interfaces
                    .binary_search(interface)
                    .is_ok()
                    .then_some(default)
            }
            _ => None,
        }
    }

    /// Every definition the captured closure needs to stay executable: the kernel, both
    /// profiles, the host profiles, the captured interfaces and implementations, and everything
    /// those declare, transitively.
    ///
    /// # Errors
    /// A reached definition is not published. Definitions are never unpublished, so this is a
    /// corrupted record.
    pub fn closure(
        &self,
        lookup: &impl DefinitionLookup,
    ) -> Result<BTreeSet<DefinitionId>, ObligationError> {
        let mut pending: Vec<DefinitionId> =
            vec![self.kernel, self.commitment_profile, self.services_profile];
        pending.extend(self.host_profiles.iter().copied());
        pending.extend(self.default_implementation);
        for bound in self.terminal.iter().chain(&self.nested) {
            pending.extend([bound.interface, bound.implementation]);
        }
        let mut closure = BTreeSet::new();
        while let Some(definition) = pending.pop() {
            if !closure.insert(definition) {
                continue;
            }
            let descriptor = lookup
                .definition(&definition)
                .ok_or(ObligationError::Unpublished(definition))?;
            pending.extend(
                descriptor
                    .typed_dependencies()
                    .into_iter()
                    .map(|(_, id)| id),
            );
        }
        Ok(closure)
    }

    /// Why `successor` does not preserve this closure, if it does not (§7.2): it stops executing
    /// a captured definition that `predecessor` executes, or binds an interface the closure
    /// reaches to another implementation than the obligation captured.
    ///
    /// A release answers a wire ID from the selection it executes, so a registered completion
    /// runs the code the root binds now. Until dispatch can run a captured implementation beside
    /// its replacement, the only way to keep a completion as it was registered is to keep its
    /// bindings: a change set that rebinds what a live obligation reaches lapses, as it does for
    /// escrows and AMX participations.
    ///
    /// # Errors
    /// A captured definition is not published.
    pub fn broken_by(
        &self,
        predecessor: &RootSelectionV1,
        successor: &RootSelectionV1,
        lookup: &impl DefinitionLookup,
    ) -> Result<Option<String>, ObligationError> {
        for definition in self.closure(lookup)? {
            if executes(predecessor, &definition) && !executes(successor, &definition) {
                return Ok(Some(format!(
                    "needs {definition}, which is inactive or disabled"
                )));
            }
            let Some(DefinitionDescriptorV1::Interface(interface)) = lookup.definition(&definition)
            else {
                continue;
            };
            let Some(captured) = self.implementation_for(&interface.wire_id, &definition, lookup)
            else {
                continue;
            };
            let bound = binding(&successor.bindings, &interface.wire_id);
            if bound.is_none_or(|bound| {
                bound.interface != definition || bound.implementation != captured
            }) {
                return Ok(Some(format!(
                    "{} is no longer bound as the obligation captured it",
                    interface.wire_id
                )));
            }
        }
        Ok(None)
    }
}

/// Whether `selection` retains `definition` in its execution closure and does not disable it.
fn executes(selection: &RootSelectionV1, definition: &DefinitionId) -> bool {
    let disabled = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(definition))
        .is_ok_and(|index| {
            matches!(
                selection.restrictions[index].restriction,
                RestrictionV1::Disabled(_)
            )
        });
    let retained = *definition == selection.kernel
        || *definition == selection.commitment_profile
        || *definition == selection.services_profile
        || selection.active.binary_search(definition).is_ok()
        || selection
            .installed_storage
            .binary_search(definition)
            .is_ok();
    retained && !disabled
}

/// An obligation registered through the kernel, keyed by [`ObligationId`] in State.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::ObligationRecordV1")]
pub struct ObligationRecordV1 {
    /// The account that owns the obligation and alone exercises it: the signer that registered
    /// it, or the subject account of `contract`.
    pub owner: AccountId,
    /// The contract whose subject account owns the obligation, if the account that controls the
    /// contract's lifecycle registered it for the contract.
    pub contract: Option<crate::smart_contract::ContractAddress>,
    /// What kind of work it is.
    pub kind: ObligationKindV1,
    /// Height of the block that registered it.
    pub created_at: u64,
    /// Committed time from which it is no longer live.
    pub expires_ms: u64,
    /// Its completion closure.
    pub capture: CompletionCaptureV1,
    /// The instructions it committed to, sorted by unique envelope identity.
    pub terminal_effects: Vec<TerminalEffectV1>,
}

impl ObligationRecordV1 {
    /// Whether the obligation still holds a capability at committed time `now_ms`: it has not
    /// expired and a committed instruction is still to be performed. Only a live obligation
    /// keeps its closure.
    #[must_use]
    pub fn is_live(&self, now_ms: u64) -> bool {
        now_ms < self.expires_ms && self.terminal_effects.iter().any(|effect| !effect.performed)
    }

    /// The position of the committed instruction `envelope` that is still to be performed.
    #[must_use]
    pub fn pending_effect(&self, envelope: &[u8; 32]) -> Option<usize> {
        self.terminal_effects
            .binary_search_by(|effect| effect.envelope.cmp(envelope))
            .ok()
            .filter(|index| !self.terminal_effects[*index].performed)
    }
}

/// Check the kind and the expiry a registration asks for, at committed time `now_ms`.
///
/// # Errors
/// The first rule the registration violates.
pub fn check_registration(
    kind: ObligationKindV1,
    expires_ms: u64,
    now_ms: u64,
) -> Result<(), ObligationError> {
    match kind {
        ObligationKindV1::Escrow
        | ObligationKindV1::PendingConsent
        | ObligationKindV1::ScheduledTrigger => {}
        ObligationKindV1::MigrationCursor | ObligationKindV1::AmxParticipation => {
            return Err(ObligationError::Kind(kind));
        }
    }
    if expires_ms <= now_ms || expires_ms - now_ms > MAX_OBLIGATION_LIFETIME_MS {
        return Err(ObligationError::Expiry);
    }
    Ok(())
}

/// Where a migration stands (§5.4).
///
/// A migration copies the old storage into its successor in bounded stages, behind a write
/// barrier, while the old storage stays authoritative. The cutover is the change set that leaves
/// no active implementation declaring the old storage. Further bounded stages then drain it, and
/// a last change set retires the empty old storage and deactivates the migration.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::MigrationStatusV1")]
#[norito(tag = "kind", content = "content")]
pub enum MigrationStatusV1 {
    /// Stages still copy the old storage into its successor. A restriction of the migration
    /// suspends them; the cursor and the write barrier stay in place.
    Running,
    /// Every old row was staged at this height. The write barrier keeps the successor
    /// consistent until the cutover, and the successor may now be read.
    Complete(u64),
    /// At this height no active implementation declared the old storage any more: the successor
    /// is authoritative, the barrier is lifted, and stages drain the old storage.
    CutOver(u64),
    /// The old storage was empty at this height and can be retired.
    Drained(u64),
    /// A stage failed at this height, before it wrote anything. No further stage runs; the
    /// cursor and the write barrier stay in place until a replacement migration takes the
    /// cursor over.
    Faulted(u64),
    /// A write reached the old storage at this height, after the cutover. The drain stops and
    /// the old storage is not retired: the release still has a writer the cutover left behind.
    StaleWrite(u64),
    /// A replacement migration took this cursor over at this height.
    Replaced(u64),
}

/// The committed cursor of one migration, keyed by the migration's definition in State.
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
#[norito_schema(name = "iroha_data_model::runtime::obligation::MigrationRecordV1")]
pub struct MigrationRecordV1 {
    /// Where the migration stands.
    pub status: MigrationStatusV1,
    /// Height at which its change set activated it.
    pub started_at: u64,
    /// The cursor under the migration's cursor schema: every old row at or before it is staged.
    /// Empty before the first stage.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub cursor: Vec<u8>,
    /// Stages run so far.
    pub stages: u64,
    /// Old rows staged so far, counting each row once.
    pub rows: u64,
}

impl MigrationRecordV1 {
    /// The record of a migration activated at `height`.
    #[must_use]
    pub const fn started(height: u64) -> Self {
        Self {
            status: MigrationStatusV1::Running,
            started_at: height,
            cursor: Vec::new(),
            stages: 0,
            rows: 0,
        }
    }

    /// Whether the write barrier still mirrors writes of the old storage into its successor:
    /// from activation until the cutover, also while the migration is suspended or faulted.
    #[must_use]
    pub const fn barrier_holds(&self) -> bool {
        matches!(
            self.status,
            MigrationStatusV1::Running
                | MigrationStatusV1::Complete(_)
                | MigrationStatusV1::Faulted(_)
        )
    }

    /// Whether every old row is staged, so an implementation may read the successor.
    #[must_use]
    pub const fn staged(&self) -> bool {
        matches!(
            self.status,
            MigrationStatusV1::Complete(_)
                | MigrationStatusV1::CutOver(_)
                | MigrationStatusV1::Drained(_)
                | MigrationStatusV1::StaleWrite(_)
        )
    }

    /// The record of a replacement migration that takes this cursor over: it resumes staging
    /// where this one stopped, or keeps a completed copy.
    #[must_use]
    pub fn taken_over(&self) -> Self {
        Self {
            status: match self.status {
                MigrationStatusV1::Complete(height) => MigrationStatusV1::Complete(height),
                _ => MigrationStatusV1::Running,
            },
            started_at: self.started_at,
            cursor: self.cursor.clone(),
            stages: self.stages,
            rows: self.rows,
        }
    }
}

#[cfg(test)]
mod tests;
