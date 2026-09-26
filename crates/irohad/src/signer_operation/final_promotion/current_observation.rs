//! Finalized role-14 Current Checks yield custody and audit from one authenticated State cut.
//!
//! The observer owner signs the exact challenged transaction. The runtime driver submits or
//! reconciles only that original signed envelope, then Core consumes its pending Check against
//! retained State and Kura. Submission, query results and decoded Check bytes cannot establish
//! the result. Configuration owns the separate observer key, qualified UTC and durable rollback
//! floor providers. This module does not implement the signing/reservation state-source trait.
//! TODO: Construct those deployment providers from reviewed daemon configuration and qualify
//! their clock, spending, finality and restart behavior before enabling a production caller.

use std::{sync::Arc, time::Duration};

use iroha_core::query::final_promotion_authority::observation::{
    FinalPromotionCheckExpectedV1, FinalPromotionCheckFloorV1, FinalPromotionCheckSourceV1,
    FinalPromotionEligibilityTimeIntervalV1, FinalPromotionObservationErrorV1,
    PendingFinalPromotionCheckV1, VerifiedFinalPromotionCheckV1, begin_final_promotion_check_v1,
};
use iroha_core::{
    query::final_promotion_authority::read_final_promotion_authority_at_v1, state::State,
};
use iroha_crypto::Signature;
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{
        FinalPromotionAuthorityActionV1, FinalPromotionCheckSubjectV1,
    },
    transaction::{SignedTransaction, TransactionPayload},
};
use sorafs_manifest::signer::{
    custody::{SignerCustodyBindingV1, SignerCustodyUseContextV1, verify_signer_custody_use_v1},
    final_promotion::SignerFinalPromotionRequestV1,
};

use super::{
    super::SignerOperationSigningStateV1,
    observer_transaction::{
        FinalPromotionObserverKeyRequestV1, FinalPromotionObserverTransactionErrorV1,
        FinalPromotionObserverTransactionsV1,
    },
};

/// Fixed failures of finalized final-promotion Check handoffs; no candidate payload escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalPromotionCheckObservationErrorV1 {
    /// Exact signed Check execution, finality or same-cut current authority did not verify.
    Check,
    /// The observer, protected binding, operator or subject was not the configured Check.
    Binding,
    /// Independently qualified UTC was unavailable or failed the original phase interval.
    Clock,
    /// The independent rollback floor was unavailable, changed or not durably advanced.
    Floor,
    /// Enrolled custody could not be reconstructed from the verified snapshot.
    Custody,
    /// A configured builder supplied no exact approved observer transaction.
    Payload,
    /// The configured independent observer failed to sign the exact transaction.
    Provider,
    /// Exact submission or same-envelope reconciliation was unavailable or unresolved.
    Submission,
    /// The private pending-Reserve journal is unavailable, changed or noncanonical.
    Journal,
    /// Local inventory resources are busy; reconcile the original signed operation.
    LocalCapacity,
}
impl std::fmt::Display for FinalPromotionCheckObservationErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Check => "final-promotion Check rejected",
            Self::Binding => "final-promotion Check binding rejected",
            Self::Clock => "final-promotion Check clock unavailable",
            Self::Floor => "final-promotion Check floor unavailable",
            Self::Custody => "final-promotion Check custody rejected",
            Self::Payload => "final-promotion Check payload rejected",
            Self::Provider => "final-promotion Check observer unavailable",
            Self::Submission => "final-promotion Check submission unavailable",
            Self::Journal => "final-promotion pending Reserve journal unavailable",
            Self::LocalCapacity => "final-promotion pending Reserve journal capacity unavailable",
        })
    }
}
impl std::error::Error for FinalPromotionCheckObservationErrorV1 {}
type Error = FinalPromotionCheckObservationErrorV1;

/// Independently qualified UTC for one finalized Check and its post-persistence use.
///
/// The implementation must establish its source, uncertainty and sampling lifetime independently
/// of the candidate transaction or native block time. Each call must take a fresh sample; returning
/// a cached interval or silently widening uncertainty violates this contract.
pub trait FinalPromotionQualifiedUtcV1 {
    /// Return the current closed UTC uncertainty interval.
    ///
    /// # Errors
    /// Fails closed if the qualified source or uncertainty bound is unavailable.
    fn sample(&mut self) -> Result<FinalPromotionEligibilityTimeIntervalV1, Error>;
}

/// Independently retained finalized chain/committee floor with durable compare-and-advance.
///
/// The implementation must survive restart and prevent a stale process from lowering the floor.
/// `advance_and_readback` must atomically compare the exact `previous` floor, retain `applied`
/// durably and read back that same value before returning. The Check proof cannot select the floor.
pub trait FinalPromotionRetainedFloorV1 {
    /// Read the floor pinned before this Check was prepared.
    ///
    /// # Errors
    /// Fails closed if the independently retained floor cannot be authenticated.
    fn read(&mut self) -> Result<FinalPromotionCheckFloorV1, Error>;

    /// Compare, advance and read back the exact authenticated applied descendant floor.
    ///
    /// # Errors
    /// Fails closed on persistence, concurrent change or readback failure.
    fn advance_and_readback(
        &mut self,
        previous: FinalPromotionCheckFloorV1,
        applied: FinalPromotionCheckFloorV1,
    ) -> Result<FinalPromotionCheckFloorV1, Error>;
}

/// Configuration-owned builder of one complete observer transaction for the original Check.
///
/// The existing observer owner checks the exact instruction, account, network and approved fee
/// intent before key I/O. This builder must independently approve timing and spending.
pub trait FinalPromotionObserverCheckPayloadV1 {
    /// Prepare the complete unsigned transaction around the exact challenged instruction.
    ///
    /// # Errors
    /// Fails closed when approved timing, admission or spending is unavailable.
    fn prepare(
        &mut self,
        instruction: &MutateSorafsFinalPromotionAuthority,
    ) -> Result<TransactionPayload, FinalPromotionCheckObservationErrorV1>;
}

/// Configuration-owned independent observer key; the protected role-14 key cannot implement it.
pub trait FinalPromotionObserverCheckSignerV1 {
    /// Sign the exact ordinary transaction prehash selected by the observer owner.
    ///
    /// # Errors
    /// Fails without returning alternate signed bytes or a replacement Check.
    fn sign(
        &mut self,
        request: &FinalPromotionObserverKeyRequestV1<'_>,
    ) -> Result<Signature, FinalPromotionObserverTransactionErrorV1>;
}

/// Result of submitting one original signed native transaction, without claiming finality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalPromotionNativeSubmitOutcomeV1 {
    /// The transport accepted or already recognized the exact signed transaction.
    Accepted,
    /// The result is unknown; reconcile this same signed transaction within the original round.
    Ambiguous,
}

/// Deployment-owned native submission and same-envelope reconciliation.
///
/// A successful transport result is never finality proof. Core must still establish exact
/// successful execution in the retained State and Kura before the driver returns anything.
pub trait FinalPromotionNativeSubmissionV1 {
    /// Submit only the original signed transaction, once.
    ///
    /// # Errors
    /// Fails closed when transport cannot establish even an ambiguous result.
    fn submit_exact(
        &mut self,
        transaction: &SignedTransaction,
    ) -> Result<FinalPromotionNativeSubmitOutcomeV1, FinalPromotionCheckObservationErrorV1>;

    /// Reconcile an ambiguous submission using the unchanged original signed transaction.
    ///
    /// # Errors
    /// Fails closed on unknown, conflicting or unavailable reconciliation; it must never re-sign.
    fn reconcile_exact(
        &mut self,
        transaction: &SignedTransaction,
    ) -> Result<(), FinalPromotionCheckObservationErrorV1>;
}

/// Non-signing runtime owner for one independently reviewed role-14 request.
///
/// Construction pins the request and configured observer policy before any coordinator exists.
/// The candidate custody and audit head come from one State view and are admitted only by a fresh
/// finalized Current Check. This owner has no Reserve, Complete or receipt-release method.
pub struct FinalPromotionCurrentCheckRuntimeV1 {
    state: Arc<State>,
    observer: FinalPromotionObserverTransactionsV1,
    request: SignerFinalPromotionRequestV1,
    max_elapsed: Duration,
}

impl FinalPromotionCurrentCheckRuntimeV1 {
    /// Pin the reviewed request, exact node State and configured independent observer.
    ///
    /// # Errors
    /// Rejects an invalid role/deployment/network binding or observation lifetime.
    pub fn new(
        state: Arc<State>,
        observer: FinalPromotionObserverTransactionsV1,
        request: SignerFinalPromotionRequestV1,
        max_elapsed: Duration,
    ) -> Result<Self, FinalPromotionCheckObservationErrorV1> {
        if max_elapsed.is_zero()
            || max_elapsed > Duration::from_secs(60)
            || state.network_id_ref().as_bytes() != &observer.receipt_binding().network_id
            || state.chain_id_ref().to_string() != observer.receipt_binding().chain_id
            || request
                .validate_binding(observer.receipt_binding())
                .is_err()
        {
            return Err(Error::Binding);
        }
        Ok(Self {
            state,
            observer,
            request,
            max_elapsed,
        })
    }

    /// Run one fresh challenged Current Check and return only its same-cut custody/audit view.
    ///
    /// The source first reads the independently retained floor, then selects the candidate native
    /// custody and audit from one State view. A signer gets only the observer payload after exact
    /// validation. Ambiguous submission can reconcile only the retained signed envelope and
    /// original challenge; every failure consumes that attempt. Core independently checks actual
    /// applied execution and durable finality before the floor is advanced.
    ///
    /// # Errors
    /// Fails closed on changed custody/audit, invalid observer payload or signature, unknown
    /// submission, missing finalized application, unqualified time or rollback.
    pub fn observe_current_with(
        &self,
        payload: &mut impl FinalPromotionObserverCheckPayloadV1,
        signer: &mut impl FinalPromotionObserverCheckSignerV1,
        submission: &mut impl FinalPromotionNativeSubmissionV1,
        clock: &mut impl FinalPromotionQualifiedUtcV1,
        floor: &mut impl FinalPromotionRetainedFloorV1,
    ) -> Result<FinalPromotionCurrentObservationV1, FinalPromotionCheckObservationErrorV1> {
        let original_floor = floor.read().map_err(|_| Error::Floor)?;
        let view = self.state.view();
        let height = u64::try_from(view.height()).map_err(|_| Error::Check)?;
        let snapshot = read_final_promotion_authority_at_v1(
            &view,
            self.observer.receipt_binding(),
            height,
            None,
        )
        .map_err(|_| Error::Check)?
        .ok_or(Error::Custody)?;
        if snapshot.custody_anchor.state_digest
            != self.request.original_custody.control_state_digest
            || snapshot.control.active_head.is_none_or(|head| {
                head.record_digest != self.request.original_custody.record_digest
            })
        {
            return Err(Error::Custody);
        }
        let expected = FinalPromotionCheckExpectedV1 {
            binding: self.observer.receipt_binding().clone(),
            observer: self.observer.observer().clone(),
            expected_operator: AccountId::new(self.observer.account_binding().public_key.clone()),
            request: self.request,
            subject: FinalPromotionCheckSubjectV1::Current(snapshot.operations.audit),
            control_revision: snapshot.control_record.revision,
            control_digest: snapshot.custody_anchor.state_digest,
            floor: original_floor,
        };
        drop(view);
        let prepared =
            begin_final_promotion_check_v1(Arc::clone(&self.state), expected, self.max_elapsed)
                .map_err(|_| Error::Check)?;
        let unsigned = payload
            .prepare(prepared.instruction())
            .map_err(|_| Error::Payload)?;
        let pending = self
            .observer
            .sign_receipt_with(prepared, unsigned, |request| signer.sign(request))
            .map_err(|error| match error {
                FinalPromotionObserverTransactionErrorV1::Provider => Error::Provider,
                FinalPromotionObserverTransactionErrorV1::Payload => Error::Payload,
                _ => Error::Check,
            })?;
        pending.ensure_live().map_err(|_| Error::Check)?;
        let outcome = submission
            .submit_exact(pending.signed_transaction())
            .map_err(|_| Error::Submission)?;
        if outcome == FinalPromotionNativeSubmitOutcomeV1::Ambiguous {
            pending.ensure_live().map_err(|_| Error::Check)?;
            submission
                .reconcile_exact(pending.signed_transaction())
                .map_err(|_| Error::Submission)?;
        }
        pending.ensure_live().map_err(|_| Error::Check)?;
        self.observer.finish_current_with(pending, clock, floor)
    }
}

/// One current custody and audit predecessor from the same finalized Check cut.
///
/// This is neither a reservation nor signing permission. The original Check lifetime remains
/// finite, and every later source phase must issue its own fresh challenged Check.
#[must_use = "consume the finalized Current Check observation within its original interval"]
pub struct FinalPromotionCurrentObservationV1 {
    verified: VerifiedFinalPromotionCheckV1,
    signing_state: SignerOperationSigningStateV1,
}
impl FinalPromotionCurrentObservationV1 {
    /// Authenticated applied descendant floor already retained durably by the floor provider.
    #[must_use]
    pub const fn applied_floor(&self) -> FinalPromotionCheckFloorV1 {
        self.verified.applied_floor()
    }

    /// Consume the same-cut signing state together with its exact verified Current Check.
    ///
    /// The Check is move-only and is required by role-15 Reserve preparation. Returning only
    /// custody and audit here would discard the authenticated source of that preparation.
    ///
    /// # Errors
    /// Rejects a Check whose original monotonic lifetime has elapsed before its consumer uses it.
    pub fn into_reserve_context(
        self,
    ) -> Result<(VerifiedFinalPromotionCheckV1, SignerOperationSigningStateV1), Error> {
        self.verified.ensure_live().map_err(|_| Error::Check)?;
        Ok((self.verified, self.signing_state))
    }
}

impl FinalPromotionObserverTransactionsV1 {
    /// Complete a signed role-14 Current Check only after its exact transaction has finalized.
    ///
    /// Core proves successful aligned execution, finality and current same-cut authority using
    /// the original State/Kura owner. This adapter then compares the independent rollback floor,
    /// durably advances it, resamples qualified UTC, and returns custody plus audit from that one
    /// snapshot. An acknowledgement, query, foreign subject or later clock sample alone cannot
    /// produce this value. The pending challenge is consumed on every outcome.
    ///
    /// # Errors
    /// Fails closed on unavailable application/finality, wrong purpose, clock, custody or floor.
    pub fn finish_current_with(
        &self,
        pending: PendingFinalPromotionCheckV1,
        clock: &mut impl FinalPromotionQualifiedUtcV1,
        floor: &mut impl FinalPromotionRetainedFloorV1,
    ) -> Result<FinalPromotionCurrentObservationV1, Error> {
        let retained = floor.read().map_err(|_| Error::Floor)?;
        let verified = pending
            .verify_finalized(FinalPromotionCheckSourceV1::Current, || {
                clock
                    .sample()
                    .map_err(|_| FinalPromotionObservationErrorV1::Clock)
            })
            .map_err(|error| match error {
                FinalPromotionObservationErrorV1::Clock => Error::Clock,
                _ => Error::Check,
            })?;
        let FinalPromotionAuthorityActionV1::Check(check) = &verified.instruction().action else {
            return Err(Error::Binding);
        };
        let FinalPromotionCheckSubjectV1::Current(audit) = &check.subject else {
            return Err(Error::Binding);
        };
        let snapshot = verified.snapshot();
        if verified.observer() != self.observer()
            || verified.expected_operator()
                != &AccountId::new(self.account_binding().public_key.clone())
            || &check.expected_operator != verified.expected_operator()
            || snapshot.control.policy.binding != *self.receipt_binding()
            || *audit != snapshot.operations.audit
        {
            return Err(Error::Binding);
        }
        if retained != verified.original_floor()
            || verified.applied_floor().height <= retained.height
        {
            return Err(Error::Floor);
        }
        // Reject ineligible enrollment before advancing durable floor state. Core checked this
        // same snapshot already; this explicit derivation protects the daemon handoff contract.
        signing_context(
            &verified,
            self.receipt_binding(),
            verified.eligibility_time_interval(),
        )?;
        let applied = verified.applied_floor();
        if floor
            .advance_and_readback(retained, applied)
            .map_err(|_| Error::Floor)?
            != applied
        {
            return Err(Error::Floor);
        }
        let after_persistence = clock.sample().map_err(|_| Error::Clock)?;
        verified
            .recheck_use_interval(after_persistence)
            .map_err(|_| Error::Clock)?;
        let snapshot = verified.snapshot();
        let custody = signing_context(&verified, self.receipt_binding(), after_persistence)?;
        Ok(FinalPromotionCurrentObservationV1 {
            signing_state: SignerOperationSigningStateV1 {
                custody,
                audit_head: snapshot.operations.audit,
            },
            verified,
        })
    }
}

pub(super) fn signing_context(
    verified: &VerifiedFinalPromotionCheckV1,
    binding: &SignerCustodyBindingV1,
    interval: FinalPromotionEligibilityTimeIntervalV1,
) -> Result<SignerCustodyUseContextV1, Error> {
    let snapshot = verified.snapshot();
    let custody = SignerCustodyUseContextV1 {
        now_unix_ms: interval.latest_unix_ms,
        anchor_observed_at_unix_ms: verified.eligibility_time_interval().earliest_unix_ms,
        current_anchor: snapshot.custody_anchor,
        active_head: snapshot.control.active_head.ok_or(Error::Custody)?,
        signer_revoked: snapshot.control.signer_revoked,
        attester_revoked: snapshot.control.attester_revoked,
    };
    verify_signer_custody_use_v1(
        snapshot
            .control_record
            .enrollment
            .as_deref()
            .ok_or(Error::Custody)?,
        binding,
        &snapshot.control.policy.custody_trust(),
        &custody,
    )
    .map_err(|_| Error::Custody)?;
    Ok(custody)
}
