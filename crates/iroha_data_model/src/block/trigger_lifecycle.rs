//! Explicit ordered trigger lifecycle coverage for one execution owner.
//!
//! These public model values and their structural checks do not authenticate
//! execution, original registration, action hashes or completeness. Core must
//! produce the entire trace from its charged, rollback-aware mutation journal;
//! consumers must authenticate its containing executed output before using it.
//! TODO: integrate native journal production, output budgets and verification
//! before activating the new explicit trace-bearing carriers. There is deliberately
//! no Default, partial coverage or conversion from absent historical evidence.

// Detached preparation only: no validator trait implementation or activation.
#[path = "trigger_action_scanner.rs"]
pub mod action_frame_scanner;

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use norito::codec::{Decode, Encode};

use super::{InvocationCompletionV1, TriggerUseV1};
use crate::{
    events::trigger_completed::TriggerCompletedOutcome,
    trigger::{TriggerId, action::Action},
};

/// Persistent trigger-store kind, independent of loaded/cache representation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleStateKindV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecycleStateKindV1 {
    /// Trigger matched by a data event.
    Data,
    /// Trigger explicitly invoked by an ExecuteTrigger instruction.
    ByCall,
    /// Trigger matched by a pipeline event.
    Pipeline,
    /// Trigger matched by a scheduled time occurrence.
    Time,
}

/// Closed codec for a complete persistent Action frame in the unreleased V2 carrier.
///
/// V1 means the exact uncompressed bytes emitted by `norito::encode_canonical`
/// for `Action`, including its typed schema identity, padding and checksum. It
/// never means a bare payload, ambient-layout encoding, JSON, hash or loaded IVM
/// handle. A future Action schema/layout change requires an explicit protocol
/// decision; an unknown schema must not be accepted by guessing another decoder.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleActionCodecV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecycleActionCodecV1 {
    /// Exact canonical Norito V1 Action frame, independently validated in full.
    NoritoCanonicalV1,
}

/// Full original Action evidence; decoded model bytes are untrusted, not a proof.
///
/// Core must count and reserve the frame, journal and serializer scratch before
/// copying, then retain the original physical allocation charges until disposal.
/// Moving a prepaid buffer into this field is permitted; `Vec` itself establishes
/// no funding. There is deliberately no owned-Action decode, capture constructor,
/// partial representation, or implicit conversion from historical evidence here.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleActionFrameV1")]
#[norito(deny_unknown_fields)]
pub struct TriggerLifecycleActionFrameV1 {
    /// Closed protocol codec; not chosen from untrusted schema discovery.
    pub codec: TriggerLifecycleActionCodecV1,
    /// Entire canonical Action frame, including every executable/filter/metadata byte.
    pub bytes: Vec<u8>,
}

/// Exact persisted retry values, with a closed schema and required JSON fields.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleRetryStateV1")]
#[norito(deny_unknown_fields)]
pub struct TriggerLifecycleRetryStateV1 {
    /// Actual failed-attempt count; zero is rejected by lifecycle validation.
    pub attempt: u32,
    /// Actual next eligible invocation time in milliseconds.
    pub next_time_ms: u64,
}

/// Caller-selected logical work bounds for a complete validation range.
///
/// These are not physical allocation ownership and do not fund outer wire decode,
/// structural indexes, parser scratch or serializer internals. Those need their
/// original resource owners before use. Zero permits no corresponding work.
#[derive(Debug, Clone, Copy)]
pub struct TriggerLifecycleActionFrameLimitsV1 {
    /// Maximum complete frame length of one stored state.
    pub max_frame_bytes: u64,
    /// Maximum sum of stored state-frame lengths, including repeated snapshots.
    pub max_total_frame_bytes: u64,
    /// Maximum number of stored state frames visited in this range.
    pub max_frames: u64,
}

/// Cumulative frame accounting, shared across all rows of one admitted range.
#[derive(Debug)]
#[allow(
    missing_copy_implementations,
    reason = "Duplicating a range owner would fork cumulative charges and sticky refusal"
)]
pub struct TriggerLifecycleActionFrameBudgetV1 {
    limits: TriggerLifecycleActionFrameLimitsV1,
    used_bytes: u64,
    used_frames: u64,
    refused: bool,
}

impl TriggerLifecycleActionFrameBudgetV1 {
    /// Begin one explicit range; never reset this counter between its rows.
    pub fn new(limits: TriggerLifecycleActionFrameLimitsV1) -> Self {
        Self {
            limits,
            used_bytes: 0,
            used_frames: 0,
            refused: false,
        }
    }

    fn charge(&mut self, frame: &TriggerLifecycleActionFrameV1) -> Result<(), String> {
        if self.refused {
            return Err("lifecycle Action frame validation range already refused".into());
        }
        let attempted: Result<(), String> = (|| {
            let len =
                u64::try_from(frame.bytes.len()).map_err(|_| "Action frame length exceeds u64")?;
            let total = self
                .used_bytes
                .checked_add(len)
                .ok_or("Action frame byte sum overflow")?;
            let count = self
                .used_frames
                .checked_add(1)
                .ok_or("Action frame count overflow")?;
            if len == 0
                || len > self.limits.max_frame_bytes
                || total > self.limits.max_total_frame_bytes
                || count > self.limits.max_frames
            {
                return Err("lifecycle Action frame validation limit exceeded".into());
            }
            self.used_bytes = total;
            self.used_frames = count;
            Ok(())
        })();
        if attempted.is_err() {
            self.refused = true;
        }
        attempted
    }
}

/// Small non-wire projection returned only after validating the ENTIRE Action.
/// It is not independently authoritative and cannot replace the retained frame.
#[derive(Debug, Clone, Copy)]
pub struct TriggerLifecycleActionSemanticsV1 {
    /// Kind proven from the actual filter, not copied from lifecycle state.kind.
    pub filter_kind: TriggerLifecycleStateKindV1,
    /// Presence proven from the actual policy after all Action invariants pass.
    pub has_retry_policy: bool,
}

/// Required integration boundary, with no production implementation or fallback.
///
/// A concrete verifier must traverse all six Action fields and every nested value
/// with bounded byte/work/depth and physically owned scratch, prove exact canonical
/// consumption/normalization and Action constructor invariants, and derive this
/// projection from those bytes. Header/schema/CRC validation alone is insufficient.
/// Neither eager unbudgeted Action decode nor PreparedCanonicalDecode (which funds
/// only counter controls) fulfills this contract. The original frame and its
/// borrowed envelope payload are supplied together without an owned graph.
///
/// No activation or native admission is available until such an implementation is
/// integrated and qualified. Test-only codec oracles are not that implementation.
pub trait TriggerLifecycleActionValidatorV1 {
    /// Validate every payload byte, including executable, authority, repeats,
    /// filter, retry policy and metadata. Refuse unsupported nested kinds.
    fn validate_canonical_action(
        &mut self,
        frame: &[u8],
        payload: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, String>;
}

/// Semantic persistent state, not a transaction-local generation or cache hash.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleStateV1")]
#[norito(deny_unknown_fields)]
pub struct TriggerLifecycleStateV1 {
    /// Actual persistent store kind.
    pub kind: TriggerLifecycleStateKindV1,
    /// Existing registration metadata, required for Time and Pipeline actions.
    /// Absence on other kinds must not be replaced by an invented zero height.
    #[norito(required)]
    pub registered_at_height: Option<u64>,
    /// Canonical semantic action hash, including authority and retry state.
    /// Existing Time/Pipeline use-time action hash; required for those kinds.
    /// Explicitly absent for Data/ByCall, whose complete native Action is retained.
    /// This model does not invent a different hash recipe for those store kinds.
    #[norito(required)]
    pub action_hash: Option<Hash>,
    /// Complete canonical persistent Action, including executable, authority,
    /// filter, repetitions, retry policy and normalized metadata. Opaque roots
    /// cannot be reconstructed from an independently readable Register input.
    /// Frame envelope and all nested semantics require bounded validation.
    pub action: TriggerLifecycleActionFrameV1,
    /// Actual native retry tuple: failed-attempt count and next eligible time in
    /// milliseconds. This is runtime state separate from the action's policy.
    #[norito(required)]
    pub retry_state: Option<TriggerLifecycleRetryStateV1>,
}

/// Automatic policy transition, distinct from ordinary body mutations.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecyclePolicyV1 {
    /// Consume an actually completed invocation's finite repetition.
    ConsumeRepeat,
    /// Remove a depleted entry, including another entry visited by a full sweep.
    RemoveDepleted,
    /// Clear retry state after a successful Time occurrence.
    ClearTimeRetry,
    /// Advance retry state after a genuine rejected Time occurrence.
    AdvanceTimeRetry,
    /// Remove a Time action whose genuine rejection exhausted its retry policy.
    ExhaustTimeRetry,
    /// Quarantine a genuinely rejected Pipeline action.
    QuarantinePipeline,
}

/// Host-owned origin; no guest instruction or contract may supply this claim.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleOriginV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecycleOriginV1 {
    /// Exact top-level native instruction position in the original Network input.
    /// Batch positions include contract-call items; this is not a compressed index.
    DirectNetworkInstruction {
        /// Original input item position, joined independently by Core and verifier.
        item_index: u32,
    },
    /// Nested execution, custom executor work or an opaque root body.
    Derived {
        /// Actual callback ordinal, or explicit absence for the Network root.
        #[norito(required)]
        callback_index: Option<u32>,
    },
    /// Committed automatic transition owned by an actual callback.
    InvocationPolicy {
        /// Causal callback; a depleted sweep may remove a different trigger ID.
        callback_index: u32,
        /// Exact policy class; its state transition still needs native validation.
        policy: TriggerLifecyclePolicyV1,
    },
}

/// One committed mutation or use; order must not be coalesced to a final diff.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleChangeV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecycleChangeV1 {
    /// A new persistent entry, including a later same-ID reincarnation.
    Insert {
        /// State after insertion.
        after: TriggerLifecycleStateV1,
    },
    /// An applied mutation; equal before/after hashes do not hide a successful touch.
    Update {
        /// State immediately before this mutation.
        before: TriggerLifecycleStateV1,
        /// State immediately after this mutation.
        after: TriggerLifecycleStateV1,
    },
    /// Remove the exact prior entry, including implicit owner retirement.
    Remove {
        /// State immediately before deletion.
        before: TriggerLifecycleStateV1,
    },
    /// Pre-body use retained only when its enclosing business work commits.
    Invoke {
        /// Actual persistent use-time state.
        state: TriggerLifecycleStateV1,
        /// Existing callback ordinal; it is not a new top-level execution owner.
        callback_index: u32,
    },
}

/// One member of a complete execution owner's ordered lifecycle journal.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleEffectV1")]
#[norito(deny_unknown_fields)]
pub struct TriggerLifecycleEffectV1 {
    /// Contiguous zero-based encounter position within this owner's journal.
    pub ordinal: u32,
    /// Exact entry affected or invoked.
    pub trigger_id: TriggerId,
    /// Host execution scope at the actual mutation/use point.
    pub origin: TriggerLifecycleOriginV1,
    /// Applied transition or use.
    pub change: TriggerLifecycleChangeV1,
}

/// Explicit complete coverage, including the meaningful empty list.
///
/// Canonical decode/allocation limits and captured output byte policy must be
/// enforced separately. This allocated model is not an unbounded capture API.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::block::execution_output::TriggerLifecycleEffectsV1")]
#[norito(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TriggerLifecycleEffectsV1 {
    /// The entire owner's applied lifecycle trace, never a partial capture.
    Complete {
        /// Encounter-ordered effects; an empty vector explicitly covers no effects.
        effects: Vec<TriggerLifecycleEffectV1>,
    },
}

/// Borrowed containing-output context for shape checks, never a wire authority.
#[derive(Debug, Clone, Copy)]
pub enum TriggerLifecycleOwnerV1<'a> {
    /// An ordinary Network input; callback zero is still a nested invocation.
    Network,
    /// Exact containing Pipeline invocation.
    Pipeline(&'a TriggerUseV1),
    /// Exact containing Time invocation.
    Time(&'a TriggerUseV1),
}

impl TriggerLifecycleEffectsV1 {
    /// Borrow the complete ordered trace without duplicating its charged storage.
    pub fn effects(&self) -> &[TriggerLifecycleEffectV1] {
        match self {
            Self::Complete { effects } => effects,
        }
    }

    /// Validate every complete stored Action frame under one cumulative budget.
    ///
    /// Call structural validation as well. The activated block entry point does
    /// both and shares a budget across rows. Envelope checks are necessary but
    /// never sufficient: the required validator must inspect the complete payload.
    /// This method never constructs an owned Action graph or substitutes an empty trace.
    ///
    /// # Errors
    /// Refuses exhausted limits, invalid envelopes, validator refusals and semantic
    /// kind/retry mismatches. A refusal latches the range closed, including for a
    /// later empty trace; it cannot be reused to retry a different interpretation.
    pub fn validate_action_frames<V: TriggerLifecycleActionValidatorV1 + ?Sized>(
        &self,
        budget: &mut TriggerLifecycleActionFrameBudgetV1,
        validator: &mut V,
    ) -> Result<(), String> {
        if budget.refused {
            return Err("lifecycle Action frame validation range already refused".into());
        }
        for effect in self.effects() {
            let states = match &effect.change {
                TriggerLifecycleChangeV1::Insert { after } => [Some(after), None],
                TriggerLifecycleChangeV1::Update { before, after } => [Some(before), Some(after)],
                TriggerLifecycleChangeV1::Remove { before } => [Some(before), None],
                TriggerLifecycleChangeV1::Invoke { state, .. } => [Some(state), None],
            };
            for state in states.into_iter().flatten() {
                budget.charge(&state.action)?;
                let validated: Result<(), String> = (|| {
                    let payload = match state.action.codec {
                        TriggerLifecycleActionCodecV1::NoritoCanonicalV1 => {
                            // A header-only check cannot establish canonical payload flags.
                            // The full validator must prove the fixed canonical encoding;
                            // this borrowed envelope check admits no compression/reserved bits.
                            let header = norito::core::Header::read(std::io::Cursor::new(
                                &state.action.bytes,
                            ))
                            .map_err(|error| error.to_string())?;
                            norito::canonical_frame_payload::<Action>(
                                &state.action.bytes,
                                header.flags,
                            )
                            .map_err(|error| error.to_string())?
                        }
                    };
                    let semantic =
                        validator.validate_canonical_action(&state.action.bytes, payload)?;
                    if semantic.filter_kind != state.kind
                        || (state.retry_state.is_some() && !semantic.has_retry_policy)
                    {
                        return Err(
                            "lifecycle state differs from complete Action frame semantics".into(),
                        );
                    }
                    Ok(())
                })();
                if validated.is_err() {
                    budget.refused = true;
                }
                validated?;
            }
        }
        Ok(())
    }

    /// Validate allocated-value shape and the supplied owner's local joins.
    ///
    /// This does not establish capture completeness, an original input's exact
    /// instruction position, semantic action hashes, or retry arithmetic. Native
    /// execution and an authenticated verifier must establish those facts. The
    /// caller must first enforce canonical decode/allocation/output-byte limits.
    /// This checks outer state only; it does not interpret Action frame bytes.
    /// `validate_action_frames` is additionally mandatory before lifecycle admission.
    ///
    /// # Errors
    /// Rejects invalid order, contradictory transitions, missing callback joins,
    /// foreign owner scopes and rolled-back body effects.
    pub fn validate_structure(
        &self,
        owner: TriggerLifecycleOwnerV1<'_>,
        proposal_height: u64,
        completions: &[InvocationCompletionV1],
        rejected: bool,
    ) -> Result<(), String> {
        if proposal_height == 0 || u32::try_from(self.effects().len()).is_err() {
            return Err("lifecycle height is zero or effect count exceeds u32".into());
        }
        if completions
            .windows(2)
            .any(|pair| pair[0].callback_index >= pair[1].callback_index)
        {
            return Err("lifecycle callback positions are duplicated or unordered".into());
        }
        let completion = |index: u32| {
            completions
                .binary_search_by_key(&index, |row| row.callback_index)
                .ok()
                .map(|position| &completions[position])
                .ok_or_else(|| "lifecycle callback is absent from its owner".to_owned())
        };
        let mut states: BTreeMap<&TriggerId, Option<&TriggerLifecycleStateV1>> = BTreeMap::new();
        let mut invokes = BTreeSet::new();
        for (position, effect) in self.effects().iter().enumerate() {
            if usize::try_from(effect.ordinal).ok() != Some(position) {
                return Err("lifecycle effect positions are not contiguous".into());
            }
            let (before, after) = match &effect.change {
                TriggerLifecycleChangeV1::Insert { after } => (None, Some(after)),
                TriggerLifecycleChangeV1::Update { before, after } => {
                    if before.kind != after.kind {
                        return Err("lifecycle update changes persistent store kind".into());
                    }
                    (Some(before), Some(after))
                }
                TriggerLifecycleChangeV1::Remove { before } => (Some(before), None),
                TriggerLifecycleChangeV1::Invoke { state, .. } => (Some(state), Some(state)),
            };
            for state in before.into_iter().chain(after) {
                validate_state(state, proposal_height)?;
            }
            if states
                .get(&effect.trigger_id)
                .is_some_and(|known| *known != before)
            {
                return Err("lifecycle transition contradicts its preceding state".into());
            }
            if rejected
                && !matches!(
                    &effect.origin,
                    TriggerLifecycleOriginV1::InvocationPolicy { .. }
                )
            {
                return Err("rejected output retains a body lifecycle effect".into());
            }
            match &effect.origin {
                TriggerLifecycleOriginV1::DirectNetworkInstruction { .. } => {
                    if !matches!(owner, TriggerLifecycleOwnerV1::Network)
                        || matches!(&effect.change, TriggerLifecycleChangeV1::Invoke { .. })
                    {
                        return Err(
                            "direct lifecycle origin is not a Network instruction mutation".into(),
                        );
                    }
                }
                TriggerLifecycleOriginV1::Derived { callback_index } => {
                    if let Some(index) = callback_index {
                        // Invoke is the pre-body boundary itself. Every other
                        // callback-owned mutation needs an earlier boundary,
                        // not merely a matching completion at the end.
                        if !matches!(&effect.change, TriggerLifecycleChangeV1::Invoke { .. })
                            && !invokes.contains(index)
                        {
                            return Err("callback lifecycle effect precedes its Invoke".into());
                        }
                        if !matches!(
                            completion(*index)?.outcome,
                            TriggerCompletedOutcome::Success
                        ) {
                            return Err("applied lifecycle effect has a failed callback".into());
                        }
                    } else if !matches!(owner, TriggerLifecycleOwnerV1::Network) {
                        return Err("internal lifecycle origin omits its callback".into());
                    }
                }
                TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index,
                    policy,
                } => {
                    let callback = completion(*callback_index)?;
                    validate_policy(*policy, &effect.change, owner, rejected)?;
                    if rejected {
                        let root = match owner {
                            TriggerLifecycleOwnerV1::Pipeline(root)
                            | TriggerLifecycleOwnerV1::Time(root) => root,
                            TriggerLifecycleOwnerV1::Network => {
                                return Err("Network rejection has lifecycle policy effects".into());
                            }
                        };
                        if *callback_index != 0
                            || callback.trigger_id != root.trigger_id
                            || effect.trigger_id != root.trigger_id
                            || !matches!(callback.outcome, TriggerCompletedOutcome::Failure(_))
                        {
                            return Err("rejection policy is not owned by its failed root".into());
                        }
                        let state = before.ok_or("rejection policy has no prior state")?;
                        if !states.contains_key(&effect.trigger_id)
                            && (state.registered_at_height != Some(root.registered_at_height)
                                || state.action_hash != Some(root.action_hash))
                        {
                            return Err(
                                "rejection policy starts from a foreign use-time state".into()
                            );
                        }
                    } else {
                        // Rejected policy-only traces deliberately have no
                        // committed Invoke. A successful policy does have one.
                        if !invokes.contains(callback_index) {
                            return Err("successful lifecycle policy precedes its Invoke".into());
                        }
                        if !matches!(callback.outcome, TriggerCompletedOutcome::Success) {
                            return Err("success policy has a failed callback".into());
                        }
                        // Only the explicit depleted-entry sweep may retire a
                        // different entry than the callback that caused it.
                        if *policy != TriggerLifecyclePolicyV1::RemoveDepleted
                            && effect.trigger_id != callback.trigger_id
                        {
                            return Err("lifecycle policy mutates a foreign callback entry".into());
                        }
                        if *policy == TriggerLifecyclePolicyV1::ClearTimeRetry {
                            let TriggerLifecycleOwnerV1::Time(root) = owner else {
                                return Err("retry clearing has no Time owner".into());
                            };
                            if *callback_index != 0 || effect.trigger_id != root.trigger_id {
                                return Err("retry clearing is not owned by its Time root".into());
                            }
                        }
                    }
                }
            }
            if let TriggerLifecycleChangeV1::Invoke {
                state,
                callback_index,
            } = &effect.change
            {
                // InvocationCompletionV1 declares ordinals allocated before
                // execution. Encounter order is increasing, but gaps remain
                // valid; no new callback numbering scheme is introduced.
                if invokes
                    .last()
                    .is_some_and(|previous| previous >= callback_index)
                {
                    return Err("lifecycle Invoke order differs from callback allocation".into());
                }
                if !matches!(&effect.origin, TriggerLifecycleOriginV1::Derived { callback_index: Some(index) } if index == callback_index)
                    || !invokes.insert(*callback_index)
                {
                    return Err(
                        "lifecycle invocation has a foreign or duplicate callback origin".into(),
                    );
                }
                let callback = completion(*callback_index)?;
                if callback.trigger_id != effect.trigger_id
                    || !matches!(callback.outcome, TriggerCompletedOutcome::Success)
                {
                    return Err(
                        "lifecycle invocation differs from its successful completion".into(),
                    );
                }
                if matches!(
                    state.kind,
                    TriggerLifecycleStateKindV1::Time | TriggerLifecycleStateKindV1::Pipeline
                ) && state
                    .registered_at_height
                    .is_some_and(|height| height >= proposal_height)
                {
                    return Err(
                        "lifecycle invocation was not registered before applying height".into(),
                    );
                }
                if *callback_index == 0 {
                    let root = match owner {
                        TriggerLifecycleOwnerV1::Network => None,
                        TriggerLifecycleOwnerV1::Pipeline(root) => {
                            Some((root, TriggerLifecycleStateKindV1::Pipeline))
                        }
                        TriggerLifecycleOwnerV1::Time(root) => {
                            Some((root, TriggerLifecycleStateKindV1::Time))
                        }
                    };
                    if let Some((root, kind)) = root
                        && (effect.trigger_id != root.trigger_id
                            || state.kind != kind
                            || state.action_hash != Some(root.action_hash)
                            || state.registered_at_height != Some(root.registered_at_height))
                    {
                        return Err(
                            "lifecycle root invocation differs from its containing owner".into(),
                        );
                    }
                }
            }
            states.insert(&effect.trigger_id, after);
        }
        if !rejected && !matches!(owner, TriggerLifecycleOwnerV1::Network) && !invokes.contains(&0)
        {
            return Err("complete internal lifecycle trace omits its root invocation".into());
        }
        if invokes.len()
            != completions
                .iter()
                .filter(|row| matches!(row.outcome, TriggerCompletedOutcome::Success))
                .count()
        {
            return Err("complete lifecycle trace omits a successful callback invocation".into());
        }
        Ok(())
    }
}

fn validate_state(state: &TriggerLifecycleStateV1, proposal_height: u64) -> Result<(), String> {
    if state.action.bytes.is_empty()
        || state.retry_state.is_some_and(|retry| retry.attempt == 0)
        || (state.retry_state.is_some() && state.kind != TriggerLifecycleStateKindV1::Time)
        || state.action_hash == Some(Hash::prehashed([0; Hash::LENGTH]))
        || (matches!(
            state.kind,
            TriggerLifecycleStateKindV1::Time | TriggerLifecycleStateKindV1::Pipeline
        ) != state.action_hash.is_some())
        || state
            .registered_at_height
            .is_some_and(|height| height > proposal_height)
        || (matches!(
            state.kind,
            TriggerLifecycleStateKindV1::Pipeline | TriggerLifecycleStateKindV1::Time
        ) && state.registered_at_height.is_none())
    {
        return Err("lifecycle state has an invalid action or registration identity".into());
    }
    Ok(())
}

fn validate_policy(
    policy: TriggerLifecyclePolicyV1,
    change: &TriggerLifecycleChangeV1,
    owner: TriggerLifecycleOwnerV1<'_>,
    rejected: bool,
) -> Result<(), String> {
    use TriggerLifecycleChangeV1::{Remove, Update};
    use TriggerLifecyclePolicyV1 as Policy;
    use TriggerLifecycleStateKindV1 as Kind;
    let valid = match (policy, change) {
        (Policy::ConsumeRepeat, Update { .. }) | (Policy::RemoveDepleted, Remove { .. }) => {
            !rejected
        }
        (Policy::ClearTimeRetry, Update { before, .. }) => {
            !rejected
                && matches!(owner, TriggerLifecycleOwnerV1::Time(_))
                && before.kind == Kind::Time
        }
        (Policy::AdvanceTimeRetry, Update { before, .. }) => {
            rejected
                && matches!(owner, TriggerLifecycleOwnerV1::Time(_))
                && before.kind == Kind::Time
        }
        (Policy::ExhaustTimeRetry, Remove { before }) => {
            rejected
                && matches!(owner, TriggerLifecycleOwnerV1::Time(_))
                && before.kind == Kind::Time
        }
        (Policy::QuarantinePipeline, Update { before, .. } | Remove { before }) => {
            rejected
                && matches!(owner, TriggerLifecycleOwnerV1::Pipeline(_))
                && before.kind == Kind::Pipeline
        }
        _ => false,
    };
    if !valid {
        return Err("lifecycle policy kind, transition or owner disposition differs".into());
    }
    Ok(())
}
