//! Checked ordinary/mandatory source ledgers prepared from one frozen policy.
//!
//! State freezes the authenticated profile before block-start work. Its execution
//! producer retains ordinary E; the retained governance sweep owns the disjoint
//! mandatory pool. Caller-supplied hashes alone never grant invocation authority.

use iroha_crypto::Hash;
use iroha_data_model::parameter::{
    ExecutionOutputPolicyV1, FastpqSourceLimitsV1, FastpqSourcePolicyV1,
};

use super::ReservationError;
use super::entry_bundle::{
    EntryBundleOwner, EntryBundleReservationError, EntryBundleReservationLedger,
    EntryBundleReservationTransaction,
};
use super::{ReservationContext, ReservationPolicy, SourceUsage};
use crate::fastpq::FastpqSourceStatementBuildLimits;
use iroha_data_model::fastpq::TransferTranscript;
use std::collections::{BTreeMap, BTreeSet};

/// Two disjoint pools whose combined ceilings fit the authenticated block policy.
/// Ordinary work cannot consume retained mandatory capacity, even before a sweep.
pub(crate) struct PreparedSourceQuota {
    ordinary: EntryBundleReservationLedger,
    mandatory: EntryBundleReservationLedger,
    ordinary_context: ReservationContext,
    mandatory_context: ReservationContext,
    #[cfg(test)]
    ordinary_ceiling: SourceUsage,
    #[cfg(test)]
    mandatory_ceiling: SourceUsage,
    profile: FastpqSourcePolicyV1,
}

/// Disposable physical contribution whose failed preparation cannot be ignored.
/// State and witness owners must remain disposable until `commit_after` succeeds.
pub(crate) struct PreparedSourceFragment<'a> {
    inner: EntryBundleReservationTransaction<'a>,
    failed: bool,
}

impl PreparedSourceFragment<'_> {
    /// Open an applied protocol-purpose owner inside this disposable attempt.
    /// Ordinary E must instead be retained by the producer before its attempt.
    pub(crate) fn open_entry(&mut self, hash: Hash) -> Result<EntryBundleOwner, ReservationError> {
        let result = self.inner.open_entry(hash);
        self.failed |= result.is_err();
        result
    }

    #[cfg(test)]
    /// Replace one complete logical bundle before the matching movement.
    /// A failure permanently refuses this physical attempt, even if ignored.
    pub(crate) fn replace_bundle<'a, I>(
        &mut self,
        owner: &EntryBundleOwner,
        bundle: I,
    ) -> Result<SourceUsage, EntryBundleReservationError>
    where
        I: IntoIterator<Item = &'a TransferTranscript>,
    {
        if self.failed {
            return Err(EntryBundleReservationError::Preparation(
                "source attempt already failed preparation".into(),
            ));
        }
        let result = self.inner.replace_bundle(owner, bundle);
        self.failed |= result.is_err();
        result
    }

    /// Publish counters only after the paired State/witness commit succeeds.
    ///
    /// The closure must own its disposable overlays and perform a typed preflight
    /// before an infallible application. On refusal it is dropped without running,
    /// so ignored preparation errors still drop State, witness and source together.
    /// Host panics/allocation failures during publication remain the enclosing
    /// carrier's poison responsibility; this does not invent an allocator rollback.
    pub(crate) fn commit_after(
        self,
        apply: impl FnOnce() -> Result<(), String>,
    ) -> Result<SourceUsage, String> {
        if self.failed {
            return Err("FASTPQ source attempt failed preparation".into());
        }
        apply()?;
        Ok(self.inner.commit())
    }
}

/// Canonical diagnostic used by actual execution owners after an intrinsic refusal.
pub(crate) const SOURCE_INTRINSIC_REJECTION: &str = "FASTPQ source intrinsic limit exceeded";

/// Typed source result; invariant failures must never become ordinary rejected rows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SourceQuotaFailure {
    Intrinsic,
    Invariant(String),
}

/// Both pool journals stay owned by the same physical State overlay.
/// Logical ordinary E was retained by the producer before opening this scope.
pub(crate) struct SourceQuotaTransaction<'a> {
    ordinary: Option<EntryBundleReservationTransaction<'a>>,
    mandatory: Option<EntryBundleReservationTransaction<'a>>,
    allow_governance_purposes: bool,
    failed: bool,
    failure: Option<SourceQuotaFailure>,
}

impl SourceQuotaTransaction<'_> {
    /// Retained construction refusal: no fallback ledger or source capacity exists.
    pub(crate) fn unavailable(error: String) -> Self {
        Self {
            ordinary: None,
            mandatory: None,
            allow_governance_purposes: false,
            failed: true,
            failure: Some(SourceQuotaFailure::Invariant(error)),
        }
    }

    /// Require a fresh producer invocation before starting fee or effect meters.
    /// This inspection neither grants an owner nor changes either source journal.
    pub(crate) fn require_empty_ordinary_entry(&self, hash: Hash) -> Result<(), String> {
        if self.failed {
            return Err("FASTPQ source transaction was poisoned".into());
        }
        let ordinary = self
            .ordinary
            .as_ref()
            .ok_or_else(|| "FASTPQ source journal is unavailable".to_owned())?;
        let owner = ordinary
            .existing_entry(hash)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "FASTPQ source has no retained producer invocation".to_owned())?;
        let actual = ordinary
            .owner_usage(&owner)
            .map_err(|error| error.to_string())?;
        if actual
            != (SourceUsage {
                executed_entries: 1,
                ..SourceUsage::ZERO
            })
        {
            return Err("FASTPQ fee admission requires an empty retained source entry".into());
        }
        Ok(())
    }

    /// Inspect the already retained invocation; diagnostic capture cannot mint E.
    pub(crate) fn require_existing_quantity_capture_entry(
        &self,
        hash: Hash,
        protocol_purpose: bool,
    ) -> Result<(), String> {
        if self.failed {
            return Err("FASTPQ source transaction was poisoned".into());
        }
        let journal = if protocol_purpose {
            if !self.allow_governance_purposes {
                return Err("quantity capture has no retained mandatory owner".into());
            }
            &self.mandatory
        } else {
            &self.ordinary
        };
        journal
            .as_ref()
            .ok_or("quantity capture source journal is unavailable")?
            .existing_entry(hash)
            .map_err(|error| error.to_string())?
            .ok_or("quantity capture has no retained invocation")?;
        Ok(())
    }

    /// Allow the private block-start governance sweep's retained-purpose entries.
    pub(crate) fn authorize_governance_purposes(&mut self) {
        self.allow_governance_purposes = true;
    }

    /// Measure the exact whole committed/pending/candidate bundle before movement.
    pub(crate) fn replace_entry<'a, I>(
        &mut self,
        hash: Hash,
        protocol_purpose: bool,
        bundle: I,
    ) -> Result<(), String>
    where
        I: IntoIterator<Item = &'a TransferTranscript>,
    {
        if self.failed {
            return Err("FASTPQ source transaction was poisoned".into());
        }
        let result = (|| {
            let transaction = if protocol_purpose {
                if !self.allow_governance_purposes {
                    return Err(SourceQuotaFailure::Invariant(
                        "protocol source has no authenticated mandatory owner".into(),
                    ));
                }
                &mut self.mandatory
            } else {
                &mut self.ordinary
            };
            let transaction = transaction.as_mut().ok_or_else(|| {
                SourceQuotaFailure::Invariant("FASTPQ source journal is unavailable".into())
            })?;
            let invariant =
                |error: ReservationError| SourceQuotaFailure::Invariant(error.to_string());
            let owner = if protocol_purpose {
                transaction.open_entry(hash).map_err(invariant)?
            } else {
                transaction
                    .existing_entry(hash)
                    .map_err(invariant)?
                    .ok_or_else(|| {
                        SourceQuotaFailure::Invariant(
                            "FASTPQ source has no retained producer invocation".into(),
                        )
                    })?
            };
            transaction
                .replace_bundle(&owner, bundle)
                .map_err(|error| match &error {
                    EntryBundleReservationError::Capacity(_)
                    | EntryBundleReservationError::Reservation(ReservationError::Intrinsic {
                        ..
                    }) if !protocol_purpose => SourceQuotaFailure::Intrinsic,
                    _ => SourceQuotaFailure::Invariant(error.to_string()),
                })?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(failure) => {
                let message = match &failure {
                    SourceQuotaFailure::Intrinsic => SOURCE_INTRINSIC_REJECTION.to_owned(),
                    SourceQuotaFailure::Invariant(error) => error.clone(),
                };
                self.failed = true;
                self.failure = Some(failure);
                Err(message)
            }
        }
    }

    /// Record a capture, allocation or ownership failure before any movement.
    pub(crate) fn fail_preparation(&mut self, error: String) {
        self.failed = true;
        if self.failure.is_none() {
            self.failure = Some(SourceQuotaFailure::Invariant(error));
        }
    }

    /// An authenticated producer selects a canonical rejection or aborts the carrier.
    pub(crate) fn intrinsic_rejected(&self) -> Result<bool, String> {
        match &self.failure {
            None => Ok(false),
            Some(SourceQuotaFailure::Intrinsic) => Ok(true),
            Some(SourceQuotaFailure::Invariant(error)) => Err(error.clone()),
        }
    }

    /// A failed movement/preparation cannot be followed by State application.
    pub(crate) fn poison(&mut self) {
        self.failed = true;
    }

    /// Commit preflight; no reservation error may be ignored by an executor.
    pub(crate) fn allows_apply(&self) -> bool {
        !self.failed
    }

    /// Publish both already checked journals with their State/source owner.
    pub(crate) fn commit(self) {
        debug_assert!(!self.failed, "State source preflight precedes publication");
        if let Some(ordinary) = self.ordinary {
            ordinary.commit();
        }
        if let Some(mandatory) = self.mandatory {
            mandatory.commit();
        }
    }
}

fn usage(value: FastpqSourceLimitsV1) -> SourceUsage {
    SourceUsage {
        executed_entries: u64::from(value.max_executed_entries),
        transcripts: u64::from(value.max_transcripts),
        deltas: u64::from(value.max_deltas),
        input_transcript_bytes: value.max_input_transcript_bytes,
        max_statement_bytes: value.max_statement_bytes,
        total_statement_bytes: value.max_total_statement_bytes,
    }
}

fn construction(value: FastpqSourceLimitsV1) -> Result<FastpqSourceStatementBuildLimits, String> {
    let bytes = |value| {
        usize::try_from(value)
            .map_err(|_| "FASTPQ source policy exceeds host length width".to_owned())
    };
    Ok(FastpqSourceStatementBuildLimits {
        max_executed_entries: value.max_executed_entries,
        max_transcripts: bytes(u64::from(value.max_transcripts))?,
        max_deltas: bytes(u64::from(value.max_deltas))?,
        max_input_transcript_bytes: bytes(value.max_input_transcript_bytes)?,
        max_statement_bytes: bytes(value.max_statement_bytes)?,
        max_total_statement_bytes: bytes(value.max_total_statement_bytes)?,
    })
}

impl PreparedSourceQuota {
    /// Prepare conservative ceilings from an already frozen canonical policy.
    ///
    /// Reserve every potential Pipeline/Time call and the complete retained
    /// mandatory pool before any business attempt. No unused mandatory capacity
    /// is lent to ordinary work. `scope` is selected by State from its frozen
    /// network/height/proposal; the private ledger identity also rejects foreign
    /// capabilities, including a second ledger with identical public inputs.
    ///
    /// Framing is bounded by each validated intrinsic profile. Typed framing-cap
    /// failures become ordinary intrinsic refusals only in the ordinary pool;
    /// a mandatory overflow contradicts the retained custody admission invariant.
    pub(crate) fn new(
        profile: FastpqSourcePolicyV1,
        output: ExecutionOutputPolicyV1,
        height: u64,
        scope: Hash,
        network_inputs: u32,
    ) -> Result<Self, String> {
        profile.validate(output)?;
        if network_inputs > profile.maximum_network_inputs(output)? {
            return Err("Network input count exceeds the frozen FASTPQ source envelope".into());
        }
        let invocations = network_inputs
            .checked_add(1)
            .and_then(|events| events.checked_mul(output.max_pipeline_triggers))
            .and_then(|pipeline| pipeline.checked_add(network_inputs))
            .and_then(|calls| calls.checked_add(output.max_time_invocations))
            .ok_or("FASTPQ complete invocation count overflows u32")?;
        let ordinary = profile.intrinsic.checked_repeat_entries(invocations)?;
        let mandatory = profile.mandatory.reservation()?;
        if !ordinary
            .checked_add_entries(mandatory)?
            .fits_within(profile.block)
        {
            return Err("FASTPQ source pools exceed the frozen block policy".into());
        }
        let policy_digest = profile.digest(output)?;
        let context_frame = norito::encode_canonical(&(policy_digest, output, scope))
            .map_err(|error| error.to_string())?;
        let context_digest = Hash::new(context_frame);
        let ordinary_context = ReservationContext {
            height,
            policy_digest: context_digest.into(),
            scope_tag: 0,
        };
        let mandatory_context = ReservationContext {
            scope_tag: 1,
            ..ordinary_context
        };
        let ledger = |context, intrinsic, block| {
            EntryBundleReservationLedger::new(
                context,
                ReservationPolicy {
                    intrinsic: usage(intrinsic),
                    block: usage(block),
                },
                construction(intrinsic)?,
            )
            .map_err(|error| error.to_string())
        };
        Ok(Self {
            ordinary: ledger(ordinary_context, profile.intrinsic, ordinary)?,
            mandatory: ledger(
                mandatory_context,
                profile.mandatory.per_obligation,
                mandatory,
            )?,
            ordinary_context,
            mandatory_context,
            #[cfg(test)]
            ordinary_ceiling: usage(ordinary),
            #[cfg(test)]
            mandatory_ceiling: usage(mandatory),
            profile,
        })
    }

    /// Borrow both source pools for one disposable State transaction.
    pub(crate) fn transaction(&mut self) -> Result<SourceQuotaTransaction<'_>, String> {
        let ordinary = self
            .ordinary
            .transaction(self.ordinary_context)
            .map_err(|error| error.to_string())?;
        let mandatory = self
            .mandatory
            .transaction(self.mandatory_context)
            .map_err(|error| error.to_string())?;
        Ok(SourceQuotaTransaction {
            ordinary: Some(ordinary),
            mandatory: Some(mandatory),
            allow_governance_purposes: false,
            failed: false,
            failure: None,
        })
    }

    /// Remeasure the final original archive and compare all six owned dimensions.
    pub(crate) fn reconcile(
        &self,
        entries: &[iroha_data_model::fastpq::FastpqSourceExecutionEntryV1],
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
    ) -> Result<(), String> {
        use iroha_data_model::fastpq::FastpqSourceExecutionKindV1;
        let mut identities = BTreeSet::new();
        let mut ordinary = BTreeSet::new();
        let mut mandatory = BTreeSet::new();
        let mut ordinary_usage = FastpqSourceLimitsV1::ZERO;
        let mut mandatory_usage = FastpqSourceLimitsV1::ZERO;
        for entry in entries {
            if !identities.insert(entry.entry_hash) {
                return Err("FASTPQ source inventory repeats an entry".into());
            }
            let measured = crate::fastpq::source_prefix_lengths::entry::measure_fastpq_source_entry_frame_usage(
                entry.entry_hash, transcripts.get(&entry.entry_hash).into_iter().flatten(), construction(self.profile.block)?,
            ).map_err(|error| error.to_string())?;
            let measured = FastpqSourceLimitsV1 {
                max_executed_entries: 1,
                max_transcripts: measured
                    .transcripts
                    .try_into()
                    .map_err(|_| "final FASTPQ T exceeds u32")?,
                max_deltas: measured
                    .deltas
                    .try_into()
                    .map_err(|_| "final FASTPQ D exceeds u32")?,
                max_input_transcript_bytes: measured
                    .input_transcript_bytes
                    .try_into()
                    .map_err(|_| "final FASTPQ I exceeds u64")?,
                max_statement_bytes: measured
                    .max_statement_bytes
                    .try_into()
                    .map_err(|_| "final FASTPQ M exceeds u64")?,
                max_total_statement_bytes: measured
                    .total_statement_bytes
                    .try_into()
                    .map_err(|_| "final FASTPQ S exceeds u64")?,
            };
            match entry.execution_kind {
                FastpqSourceExecutionKindV1::ExecutionCall => {
                    ordinary.insert(entry.entry_hash);
                    ordinary_usage = ordinary_usage.checked_add_entries(measured)?;
                }
                FastpqSourceExecutionKindV1::ProtocolPurpose => {
                    if measured.max_transcripts == 0 {
                        return Err(
                            "mandatory source inventory contains an unapplied purpose".into()
                        );
                    }
                    mandatory.insert(entry.entry_hash);
                    mandatory_usage = mandatory_usage.checked_add_entries(measured)?;
                }
            }
        }
        if transcripts.keys().any(|hash| !identities.contains(hash))
            || !self.ordinary.entry_hashes().eq(ordinary)
            || !self.mandatory.entry_hashes().eq(mandatory)
            || usage(ordinary_usage) != self.ordinary.usage()
            || usage(mandatory_usage) != self.mandatory.usage()
        {
            return Err(
                "FASTPQ final source archive differs from its execution-owned quota journals"
                    .into(),
            );
        }
        Ok(())
    }

    /// Confirm invocation ownership against the execution producer's complete archive.
    pub(crate) fn verify_ordinary_entries(
        &self,
        entries: impl IntoIterator<Item = Hash>,
    ) -> Result<(), String> {
        let mut expected = BTreeSet::new();
        for hash in entries {
            if !expected.insert(hash) {
                return Err("FASTPQ invocation archive repeats an identity".into());
            }
        }
        if !self.ordinary.entry_hashes().eq(expected) {
            return Err(
                "FASTPQ quota invocation set differs from the complete producer archive".into(),
            );
        }
        Ok(())
    }

    /// Begin one ordinary physical fragment; dropping it restores exact usage.
    /// The producer must retain its E owner outside disposable business attempts.
    pub(crate) fn ordinary_fragment(&mut self) -> Result<PreparedSourceFragment<'_>, String> {
        let inner = self
            .ordinary
            .transaction(self.ordinary_context)
            .map_err(|error| error.to_string())?;
        Ok(PreparedSourceFragment {
            inner,
            failed: false,
        })
    }

    /// Retain E once at the producer invocation boundary, outside business rollback.
    /// The caller must derive this hash from its authenticated invocation owner.
    pub(crate) fn retain_ordinary_entry(&mut self, hash: Hash) -> Result<EntryBundleOwner, String> {
        let mut fragment = self.ordinary_fragment()?;
        let owner = fragment
            .open_entry(hash)
            .map_err(|error| error.to_string())?;
        fragment.commit_after(|| Ok(()))?;
        Ok(owner)
    }

    #[cfg(test)]
    /// Begin one applied mandatory-purpose fragment in its isolated pool.
    /// Failed/zero no-transfer purposes do not fabricate source E entries.
    pub(crate) fn mandatory_fragment(&mut self) -> Result<PreparedSourceFragment<'_>, String> {
        let inner = self
            .mandatory
            .transaction(self.mandatory_context)
            .map_err(|error| error.to_string())?;
        Ok(PreparedSourceFragment {
            inner,
            failed: false,
        })
    }

    #[cfg(test)]
    /// Exact committed ordinary usage, independent of the mandatory holdback.
    pub(crate) fn ordinary_usage(&self) -> SourceUsage {
        self.ordinary.usage()
    }

    #[cfg(test)]
    /// Exact committed applied-purpose usage; retained obligations are separate.
    pub(crate) fn mandatory_usage(&self) -> SourceUsage {
        self.mandatory.usage()
    }

    #[cfg(test)]
    /// Conservative ordinary and mandatory ceilings, including maximum semantics.
    pub(crate) fn ceilings(&self) -> (SourceUsage, SourceUsage) {
        (self.ordinary_ceiling, self.mandatory_ceiling)
    }
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod tests;
