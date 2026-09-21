//! One-use native Check execution joined to the same node's authenticated applied State.
//!
//! Independent floor/context, distinct accounts, custody, payload commitment and clock expectations enter
//! explicitly. This consumer owns the actual State and reuses canonical consensus continuity;
//! a decoded Check, isolated QC or successful submission acknowledgement cannot mint authority.
//! TODO: wire the production purpose-bound account signer, independent observer signer and qualified
//! clock to this consumer, and qualify live phase latency and physical custody independently.

use std::{sync::Arc, time::Duration};

use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::HeightContextId,
    isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
    sorafs::final_promotion_account_custody::{
        FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1,
        FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1, FinalPromotionAccountCustodyActionV1,
        FinalPromotionAccountCustodyCheckV1,
    },
    transaction::{SignedTransaction, TransactionEntrypoint, TransactionPayload},
};
use sorafs_manifest::signer::{
    custody::SignerCustodyBindingV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};

use super::{
    FinalPromotionAccountCustodySnapshotV1,
    check::{check_applied_snapshot_v1, check_snapshot_eligibility_v1},
};
use crate::{
    query::signer_check::{
        BoundNativeCheckV1, NativeCheckErrorV1, NativeCheckFloorV1, NativeCheckRoundV1,
        NativeCustodyCheckPurposeV1, NativeCustodyCheckRefV1, authenticate_applied_check_v1,
        bind_signed_check_v1, validate_native_signatory_v1,
    },
    state::State,
};

/// Shared canonical signed-External byte ceiling for observer and protected-account transactions.
/// The value bounds storage and envelope structure; it grants no native action or fee authority.
pub use crate::query::signer_check::FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1;

/// Independently retained chain/committee floor; candidate proofs must not select these pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalPromotionAccountCheckFloorV1 {
    /// One-based canonical height already trusted by the caller across restart.
    pub height: u64,
    /// Exact trusted canonical block-header hash at that height.
    pub block_hash: [u8; 32],
    /// Independently trusted height-context identity, including its committee authority.
    pub context_id: HeightContextId,
}

/// Independent, immutable expectations selected before creating a pending Check.
///
/// These are caller trust inputs, not decoded verification output. The source factory must pin
/// the complete reviewed account transaction before any provider or observer I/O.
pub struct FinalPromotionAccountCheckExpectedV1 {
    /// Exact role-15 signer authorization binding for the reviewed account transaction.
    pub binding: SignerCustodyBindingV1,
    /// Independently qualified single Ed25519 observation account, distinct from the target.
    pub observer: AccountId,
    /// Account derived independently from this exact role-15 Ed25519 custody key.
    pub expected_account: AccountId,
    /// Exact reviewed complete payload commitment selected by the prepared-account owner.
    /// This consumer neither computes the digest nor substitutes it for retained payload bytes.
    pub transaction_payload_digest: [u8; 32],
    /// Exact governed custody control revision.
    pub control_revision: u64,
    /// Exact governed custody control record digest.
    pub control_digest: [u8; 32],
    /// Independently retained canonical chain and committee floor.
    pub floor: FinalPromotionAccountCheckFloorV1,
}

/// Independently supplied closed UTC interval for one eligibility observation.
///
/// This runtime-only value is a caller expectation, not qualified clock evidence. The production
/// source must independently establish its source, uncertainty and sampling lifetime. Both
/// endpoints must satisfy current account custody at one applied State cut. The prepared-account
/// signer separately retains the complete reviewed payload and enforces its exact action scope.
/// Equal endpoints retain exact-time semantics. No wire representation or scalar fallback exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalPromotionAccountEligibilityTimeIntervalV1 {
    /// Earliest possible UTC time in milliseconds since the UNIX epoch, strictly greater than zero.
    pub earliest_unix_ms: u64,
    /// Latest possible UTC time, at least the earliest value and strictly less than `u64::MAX`.
    pub latest_unix_ms: u64,
}

/// Payload-free terminal failures; none leaves a reusable pending observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FinalPromotionAccountObservationErrorV1 {
    /// Independent expectations or canonical bounds are invalid.
    #[error("invalid final-promotion account-custody Check expectations")]
    Invalid,
    /// The local one-use observation interval has elapsed.
    #[error("final-promotion account-custody Check observation expired")]
    Expired,
    /// Unpredictable local challenge generation failed.
    #[error("final-promotion account-custody Check entropy unavailable")]
    Entropy,
    /// The signed envelope differs from the exact prepared Check.
    #[error("final-promotion account-custody Check signed transaction mismatch")]
    Transaction,
    /// The exact transaction has not been applied to this State cut.
    #[error("final-promotion account-custody Check is not applied")]
    NotApplied,
    /// Exact durable finality or independently anchored committee continuity failed.
    #[error("final-promotion account-custody Check finality unavailable")]
    Finality,
    /// Exact signed entry, executed wire or aligned successful result could not be proven.
    #[error("final-promotion account-custody Check execution proof rejected")]
    Execution,
    /// Current same-cut account permissions, control or custody is no longer eligible.
    #[error("final-promotion account-custody Check current authority rejected")]
    Authority,
    /// The independently supplied eligibility interval is unavailable or malformed.
    #[error("final-promotion account-custody Check eligibility clock unavailable")]
    Clock,
}
use FinalPromotionAccountObservationErrorV1 as Error;

/// Move-only challenge prepared before signing or submitting its exact native transaction.
#[must_use = "a prepared Check must be signed within its original observation interval"]
pub struct PreparedFinalPromotionAccountCheckV1 {
    state: Arc<State>,
    expected: FinalPromotionAccountCheckExpectedV1,
    instruction: MutateSorafsFinalPromotionAccountCustody,
    round: NativeCheckRoundV1,
}

/// Move-only exact signed attempt. Dropping or terminal verification retires its challenge.
#[must_use = "submit the exact transaction and consume the pending Check once applied"]
pub struct PendingFinalPromotionAccountCheckV1 {
    prepared: PreparedFinalPromotionAccountCheckV1,
    bound: BoundNativeCheckV1,
}

/// Move-only success scoped to one exact native Check and one authenticated applied cut.
///
/// It has no wire representation. Use it immediately for its original account transaction; it cannot certify
/// that revocation will not race a later key call or authorize a different observation phase.
#[must_use = "use the verified Check only for its original live observation phase"]
pub struct VerifiedFinalPromotionAccountCheckV1 {
    observer: AccountId,
    instruction: MutateSorafsFinalPromotionAccountCustody,
    snapshot: FinalPromotionAccountCustodySnapshotV1,
    check_height: u64,
    original_floor: FinalPromotionAccountCheckFloorV1,
    applied_floor: FinalPromotionAccountCheckFloorV1,
    entry_hash: HashOf<TransactionEntrypoint>,
    canonical_external: Vec<u8>,
    check_block_hash: [u8; 32],
    eligibility_time_interval: FinalPromotionAccountEligibilityTimeIntervalV1,
    round: NativeCheckRoundV1,
}

impl From<NativeCheckErrorV1> for Error {
    fn from(error: NativeCheckErrorV1) -> Self {
        match error {
            NativeCheckErrorV1::Invalid => Self::Invalid,
            NativeCheckErrorV1::Expired => Self::Expired,
            NativeCheckErrorV1::Entropy => Self::Entropy,
            NativeCheckErrorV1::Transaction => Self::Transaction,
            NativeCheckErrorV1::NotApplied => Self::NotApplied,
            NativeCheckErrorV1::Finality => Self::Finality,
            NativeCheckErrorV1::Execution => Self::Execution,
        }
    }
}
impl FinalPromotionAccountCheckFloorV1 {
    fn coordinates(self) -> NativeCheckFloorV1 {
        NativeCheckFloorV1 {
            height: self.height,
            block_hash: self.block_hash,
            context_id: self.context_id,
        }
    }
}

/// Validate the structural envelope profile for one reviewed final-promotion account payload.
///
/// This checks the common canonical signed-External byte ceiling, a single Ed25519 account,
/// absent proof attachments and the ordinary transaction builder's domain, lifetime and fee
/// intent syntax. It preserves the complete payload, including fee intent, timing and metadata.
/// A private fixed-size signature placeholder is used only to count the exact envelope layout;
/// no synthetic signed transaction or encoded bytes are returned.
///
/// Success establishes no authority, approved fees, native action or phase, custody, currentness
/// or valid signature. The prepared account owner must independently validate all those inputs,
/// and the actual returned transaction must still pass signature verification and exact binding.
///
/// # Errors
/// Returns [`FinalPromotionAccountObservationErrorV1::Transaction`] for invalid structure or
/// an oversized canonical payload or complete signed External envelope.
pub fn validate_final_promotion_account_transaction_envelope_v1(
    payload: &TransactionPayload,
) -> Result<(), Error> {
    crate::query::signer_check::validate_account_transaction_envelope_v1(payload)
        .map_err(Into::into)
}

/// Return the exact canonical External frame for the first-release native signed profile.
///
/// Both observer and protected-account envelopes require a single Ed25519 account, no proof
/// attachments or multisig authorization sidecars, ordinary domain/lifetime/fee syntax, the
/// shared complete-frame bound, and a valid signature over the complete payload. This preserves
/// all signed payload fields. It does not approve fees, actions, routing, custody or currentness.
/// Callers replaying retained bytes must use bounded canonical decoding and compare this exact
/// returned frame with the original bytes; no alternate wire encoding is accepted by that replay.
///
/// # Errors
/// Returns [`FinalPromotionAccountObservationErrorV1::Transaction`] for another entry kind,
/// unsupported profile, invalid signature or oversized canonical envelope.
pub fn final_promotion_native_signed_entry_frame_v1(
    entry: &TransactionEntrypoint,
) -> Result<Vec<u8>, Error> {
    crate::query::signer_check::native_signed_entry_frame_v1(entry).map_err(Into::into)
}

/// Begin a purpose-native round with fresh OS entropy before any signing or State/proof I/O.
///
/// # Errors
/// Rejects invalid independent expectations, a zero or greater-than-60-second interval, failed
/// entropy, or expiry. The duration is an observation bound, not signed logical block time.
pub fn begin_final_promotion_account_check_v1(
    state: Arc<State>,
    expected: FinalPromotionAccountCheckExpectedV1,
    max_elapsed: Duration,
) -> Result<PreparedFinalPromotionAccountCheckV1, Error> {
    let mut round = NativeCheckRoundV1::start(max_elapsed)?;
    let binding = &expected.binding;
    binding.validate().map_err(|_| Error::Invalid)?;
    let SignerPurposeBindingV1::FinalPromotionAccountTransaction { deployment_id } =
        &binding.purpose
    else {
        return Err(Error::Invalid);
    };
    expected.floor.coordinates().validate()?;
    let target = AccountId::new(binding.public_key.clone());
    if binding.role != SignerRoleV1::FinalPromotionAccountTransaction
        || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        || validate_native_signatory_v1(&expected.observer).is_err()
        || expected.expected_account != target
        || expected.observer == target
        || expected.transaction_payload_digest == [0; 32]
        || expected.control_revision == 0
        || expected.control_revision > FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1
        || expected.control_digest == [0; 32]
    {
        return Err(Error::Invalid);
    }
    let challenge = round.issue_challenge()?;
    let instruction = MutateSorafsFinalPromotionAccountCustody {
        deployment_id: deployment_id.clone(),
        expected_control_revision: expected.control_revision,
        expected_control_digest: expected.control_digest,
        action: FinalPromotionAccountCustodyActionV1::Check(FinalPromotionAccountCustodyCheckV1 {
            challenge,
            network_id: binding.network_id,
            minimum_height: expected.floor.height,
            minimum_block_hash: expected.floor.block_hash,
            expected_account: expected.expected_account.clone(),
            transaction_payload_digest: expected.transaction_payload_digest,
        }),
    };
    if norito::canonical_frame_len(&instruction).map_err(|_| Error::Invalid)?
        > FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1
    {
        return Err(Error::Invalid);
    }
    norito::encode_canonical(&instruction).map_err(|_| Error::Invalid)?;
    round.ensure_live()?;
    Ok(PreparedFinalPromotionAccountCheckV1 {
        state,
        expected,
        instruction,
        round,
    })
}

impl PreparedFinalPromotionAccountCheckV1 {
    /// Original independently supplied role-15 binding, including its provider and policy pins.
    /// This immutable input is not verified custody or current authorization.
    #[must_use]
    pub const fn binding(&self) -> &SignerCustodyBindingV1 {
        &self.expected.binding
    }

    /// Exact independently pinned observer to compare before invoking a signing credential.
    /// This immutable input is not verified custody or current authorization.
    #[must_use]
    pub const fn observer(&self) -> &AccountId {
        &self.expected.observer
    }

    /// Check this original account observation round before source-owned runtime work.
    ///
    /// This neither observes custody nor renews the challenge, deadline or trust inputs.
    ///
    /// # Errors
    /// Returns expiry without changing this one-use prepared capability.
    pub fn ensure_live(&self) -> Result<(), Error> {
        self.round.ensure_live().map_err(Into::into)
    }

    /// Exact native instruction to sign; the challenge is already fixed and cannot be replaced.
    #[must_use]
    pub const fn instruction(&self) -> &MutateSorafsFinalPromotionAccountCustody {
        &self.instruction
    }

    /// Bind exactly one ordinary direct Check and retain its complete signed External bytes.
    ///
    /// # Errors
    /// Consumes the prepared challenge on expiry, size, account/network, instruction or signature
    /// mismatch. Signing and transport waits never reset the original interval.
    pub fn bind_signed_transaction(
        mut self,
        signed: SignedTransaction,
    ) -> Result<PendingFinalPromotionAccountCheckV1, Error> {
        let bound = bind_signed_check_v1(
            &mut self.round,
            NativeCustodyCheckRefV1::FinalPromotionAccount(&self.instruction),
            &self.expected.binding.chain_id,
            self.expected.binding.network_id,
            &self.expected.observer,
            self.expected.floor.coordinates(),
            signed,
        )?;
        Ok(PendingFinalPromotionAccountCheckV1 {
            prepared: self,
            bound,
        })
    }
}

impl PendingFinalPromotionAccountCheckV1 {
    /// Exact signed envelope for ordinary native submission or reconciliation; no replacement API.
    #[must_use]
    pub const fn signed_transaction(&self) -> &SignedTransaction {
        self.bound.signed_transaction()
    }

    /// Check the unchanged local interval while awaiting native application or ambiguous transport.
    ///
    /// # Errors
    /// Returns expiry; the caller must retire the attempt instead of creating a new deadline.
    pub fn ensure_live(&self) -> Result<(), Error> {
        self.prepared.round.ensure_live().map_err(Into::into)
    }

    /// Consume the attempt against its retained actual State, after native application is observed.
    ///
    /// The callback supplies one independently established UTC interval after expensive proof work.
    /// Both endpoints must satisfy the shared custody predicates at this same applied cut;
    /// this consumer does not qualify the clock source or derive its uncertainty from the candidate.
    /// All terminal outcomes consume the capability, including absence of application. Polling and
    /// transport reconciliation therefore occur before this call within the same original interval.
    ///
    /// # Errors
    /// Rejects expiry, missing exact application/finality, foreign committee continuity, changed
    /// signed envelope, rejected or misaligned result, unavailable clock, or current ineligibility.
    pub fn verify_finalized(
        self,
        sample_eligibility_time: impl FnOnce() -> Result<
            FinalPromotionAccountEligibilityTimeIntervalV1,
            Error,
        >,
    ) -> Result<VerifiedFinalPromotionAccountCheckV1, Error> {
        self.ensure_live()?;
        let p = &self.prepared;
        let cut = authenticate_applied_check_v1(
            &p.state,
            NativeCustodyCheckPurposeV1::FinalPromotionAccount,
            self.bound,
            &p.round,
        )?;
        let view = cut.view();
        let eligibility_time_interval = sample_eligibility_time().map_err(|_| Error::Clock)?;
        let FinalPromotionAccountEligibilityTimeIntervalV1 {
            earliest_unix_ms,
            latest_unix_ms,
        } = eligibility_time_interval;
        if earliest_unix_ms == 0 || earliest_unix_ms > latest_unix_ms || latest_unix_ms == u64::MAX
        {
            return Err(Error::Clock);
        }
        p.round.ensure_live()?;
        let snapshot = check_applied_snapshot_v1(
            view,
            &p.instruction,
            &p.expected.binding,
            &p.expected.observer,
            earliest_unix_ms,
        )
        .map_err(|_| Error::Authority)?;
        p.round.ensure_live()?;
        // Reuse this exact native snapshot; do not repeat history traversal or capture a newer cut.
        // The shared owner rejects not-yet-valid lower bounds and expired upper bounds alike.
        // The earliest bound is also the observation time; uncertainty consumes anchor age.
        check_snapshot_eligibility_v1(
            &snapshot,
            &p.instruction,
            &p.expected.binding,
            &p.expected.observer,
            latest_unix_ms,
            earliest_unix_ms,
        )
        .map_err(|_| Error::Authority)?;
        p.round.ensure_live()?;
        let check_height = cut.check_height();
        let native_floor = cut.applied_floor();
        let applied_floor = FinalPromotionAccountCheckFloorV1 {
            height: native_floor.height,
            block_hash: native_floor.block_hash,
            context_id: native_floor.context_id,
        };
        let entry_hash = cut.entry_hash();
        let (canonical_external, check_block_hash) = cut.into_verified_entry();
        Ok(VerifiedFinalPromotionAccountCheckV1 {
            observer: p.expected.observer.clone(),
            instruction: p.instruction.clone(),
            snapshot,
            check_height,
            original_floor: p.expected.floor,
            applied_floor,
            entry_hash,
            canonical_external,
            check_block_hash,
            eligibility_time_interval,
            round: self.prepared.round,
        })
    }
}

impl VerifiedFinalPromotionAccountCheckV1 {
    /// Independent observer whose current Check permission was rechecked.
    #[must_use]
    pub const fn observer(&self) -> &AccountId {
        &self.observer
    }
    /// Exact one-use challenge, target, payload commitment and control CAS authenticated by this success.
    #[must_use]
    pub const fn instruction(&self) -> &MutateSorafsFinalPromotionAccountCustody {
        &self.instruction
    }
    /// Raw snapshot from the exact authenticated applied cut, not a renewable freshness certificate.
    /// Production phase consumers must call `ensure_live` immediately before using this result.
    #[must_use]
    pub const fn snapshot(&self) -> &FinalPromotionAccountCustodySnapshotV1 {
        &self.snapshot
    }
    /// Height of the exact signed Check and its aligned successful execution result.
    #[must_use]
    pub const fn check_height(&self) -> u64 {
        self.check_height
    }
    /// Exact canonical signed External bytes whose aligned successful result was authenticated.
    /// Borrowing historical execution material grants no renewed eligibility or authority.
    #[must_use]
    pub fn canonical_external(&self) -> &[u8] {
        &self.canonical_external
    }
    /// Exact authenticated block hash at `check_height`, distinct from a later applied floor.
    #[must_use]
    pub const fn check_block_hash(&self) -> [u8; 32] {
        self.check_block_hash
    }
    /// Exact independent trust floor used for this original proof, including its committee pin.
    /// Retention does not authenticate how the caller provisioned that floor or renew this round.
    #[must_use]
    pub const fn original_floor(&self) -> FinalPromotionAccountCheckFloorV1 {
        self.original_floor
    }
    /// Authenticated applied descendant floor, suitable for retaining continuity across rounds.
    #[must_use]
    pub const fn applied_floor(&self) -> FinalPromotionAccountCheckFloorV1 {
        self.applied_floor
    }
    /// Exact signed intent's native entrypoint membership identity.
    #[must_use]
    pub const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.entry_hash
    }
    /// Exact independently supplied UTC interval checked after proof work at this same State cut.
    /// This preserves both uncertainty endpoints; success does not qualify the clock source.
    /// A production source must retain the earliest observation bound for later age checks and
    /// resample after blocking persistence. A later sample must not refresh this observation time.
    #[must_use]
    pub const fn eligibility_time_interval(
        &self,
    ) -> FinalPromotionAccountEligibilityTimeIntervalV1 {
        self.eligibility_time_interval
    }
    /// Recheck time eligibility against this exact retained authenticated snapshot.
    ///
    /// Both independently supplied UTC endpoints must remain eligible under the original
    /// observation's earliest bound and unchanged local deadline. This neither samples or
    /// qualifies a clock, observes newer state, nor proves absence of a later revocation.
    /// Callers must still obtain a fresh Check for each required observation phase.
    ///
    /// # Errors
    /// Rejects expiry, malformed or backward intervals, excess original observation age,
    /// and custody or phase ineligibility. No success or failure renews this capability.
    pub fn recheck_use_interval(
        &self,
        interval: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<(), Error> {
        self.ensure_live()?;
        let FinalPromotionAccountEligibilityTimeIntervalV1 {
            earliest_unix_ms,
            latest_unix_ms,
        } = interval;
        let observed_at = self.eligibility_time_interval.earliest_unix_ms;
        if earliest_unix_ms == 0
            || earliest_unix_ms < observed_at
            || earliest_unix_ms > latest_unix_ms
            || latest_unix_ms == u64::MAX
        {
            return Err(Error::Clock);
        }
        for now in [earliest_unix_ms, latest_unix_ms] {
            self.ensure_live()?;
            check_snapshot_eligibility_v1(
                &self.snapshot,
                &self.instruction,
                &self.snapshot.control.policy.binding,
                &self.observer,
                now,
                observed_at,
            )
            .map_err(|_| Error::Authority)?;
        }
        self.ensure_live()
    }

    /// Check the original interval immediately before using this success for its pinned account transaction.
    ///
    /// # Errors
    /// Returns expiry; this never renews authority or proves absence of a subsequent revocation.
    pub fn ensure_live(&self) -> Result<(), Error> {
        self.round.ensure_live().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
