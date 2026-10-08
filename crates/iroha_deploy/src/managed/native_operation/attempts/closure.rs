//! Terminal unsigned closure under one exact reserved custody-body successor.
use super::*;
use crate::managed::native_operation::{Fees, authorization::DispatchAuthorization};
use std::time::Instant;

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::ClosureTail")]
pub(super) enum ClosureTail {
    Empty,
    Missing {
        authorization: [u8; 32],
    },
    Request {
        authorization: [u8; 32],
        request_sha256: String,
    },
}
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::ClosurePlan")]
pub(super) struct ClosurePlan {
    purpose: Purpose,
    semantic: [u8; 32],
    scope: DispatchScope,
    dispatch: Option<Dispatch>,
    successor: [u8; 32],
    tail: ClosureTail,
    cumulative_reserved: u8,
}
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::ClosureRecord")]
pub(super) struct ClosureRecord {
    plan: [u8; 32],
    tail: ClosureTail,
}

pub(in crate::managed) struct PendingUnsignedClosure {
    history: History,
}
pub(in crate::managed) struct VerifiedUnsignedClosure {
    history: History,
    digest: [u8; 32],
    successor: [u8; 32],
    cumulative_reserved: usize,
    outer_intent: [u8; 32],
    root_identity: iroha_fs::FileIdentity,
    fees: Fees,
}
impl VerifiedUnsignedClosure {
    pub(in crate::managed) fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub(in crate::managed) fn purpose(&self) -> Purpose {
        self.history.purpose
    }
    pub(in crate::managed) fn closed_semantic(&self) -> [u8; 32] {
        self.history.semantic
    }
    pub(in crate::managed) fn successor_selection(&self) -> [u8; 32] {
        self.successor
    }
    pub(in crate::managed) fn cumulative_reserved_count(&self) -> usize {
        self.cumulative_reserved
    }
    pub(super) fn outer_intent(&self) -> [u8; 32] {
        self.outer_intent
    }
    pub(super) fn root_identity(&self) -> iroha_fs::FileIdentity {
        self.root_identity
    }
    pub(super) fn fees(&self) -> &Fees {
        &self.fees
    }
    pub(super) fn require_retained(&self) -> Result<()> {
        self.history.require_current()?;
        self.require_receipt()
    }
    pub(in crate::managed) fn retained_history(&self) -> &History {
        &self.history
    }
    // The bounded graph owner invokes this only as part of its complete before/after passes.
    pub(super) fn require_retained_local(
        &self,
        pass: Option<&crate::managed::stream_token_custody::body_history::SnapshotReadPass<'_>>,
    ) -> Result<()> {
        self.history.require_current_local(pass)?;
        self.require_receipt()
    }
    pub(super) fn require_receipt(&self) -> Result<()> {
        let evidence = self.history.scope.enrollment()?;
        let plan = self
            .history
            .closing
            .as_ref()
            .ok_or_else(|| invalid("verified unsigned closure lost its closing plan"))?;
        if self.history.closed.as_ref().map(digest).transpose()? != Some(self.digest)
            || plan.successor != self.successor
            || usize::from(plan.cumulative_reserved) != self.cumulative_reserved
            || self.history.cumulative_reserved != self.cumulative_reserved
            || evidence.binding().outer_intent != self.outer_intent
            || evidence.root().identity()? != self.root_identity
            || evidence.fees() != &self.fees
        {
            return Err(invalid("verified unsigned closure was lost or changed"));
        }
        self.history.require_fees(&self.fees)?;
        self.history.validate_closure_records()
    }
}
impl History {
    pub(super) fn validate_closure_records(&self) -> Result<()> {
        let Some(plan) = &self.closing else {
            if self.closed.is_some() {
                return Err(invalid("unsigned closure lost its closing plan"));
            }
            return Ok(());
        };
        self.scope.enrollment()?;
        if self.reservation_pending()
            || self.empty_tail
            || plan.purpose != self.purpose
            || plan.semantic != self.semantic
            || plan.scope != self.scope.commitment()?
            || plan.dispatch != self.dispatch
            || plan.successor == [0; 32]
            || usize::from(plan.cumulative_reserved)
                != self
                    .scope
                    .prior_total()
                    .checked_add(self.reserved_attempt_count())
                    .ok_or_else(|| invalid("body dispatch count overflow"))?
            || usize::from(plan.cumulative_reserved) > MAX_ATTEMPTS
        {
            return Err(invalid(
                "unsigned closing plan changed original dispatch custody",
            ));
        }
        match (&plan.tail, self.attempts.last()) {
            (ClosureTail::Empty, None) if self.dispatch.is_none() && self.root.is_none() => {}
            (ClosureTail::Missing { authorization }, Some(last))
                if *authorization == last.digest()?
                    && last.commit.is_none()
                    && last.retirement.is_none() => {}
            (
                ClosureTail::Request {
                    authorization,
                    request_sha256,
                },
                Some(last),
            ) if *authorization == last.digest()?
                && last.observation.is_some()
                && last.retirement.is_none()
                && valid_sha256(request_sha256)
                && last
                    .commit
                    .as_ref()
                    .is_none_or(|value| value.request_sha256 == *request_sha256) => {}
            _ => return Err(invalid("unsigned closing plan selected another exact tail")),
        }
        if self
            .attempts
            .iter()
            .take(self.attempts.len().saturating_sub(1))
            .any(|attempt| attempt.retirement.is_none())
        {
            return Err(ManagedBootstrapFailure::TransitionPending.into());
        }
        if let Some(closed) = &self.closed {
            if closed.plan != digest(plan)? || closed.tail != plan.tail {
                return Err(invalid(
                    "unsigned closure changed its reserved closing plan",
                ));
            }
        }
        Ok(())
    }
    fn check_successor(&self, successor: &dyn SemanticSuccessor) -> Result<()> {
        self.require_current()?;
        successor.revalidate()?;
        let target = successor.target();
        target.validate_target()?;
        let evidence = self.scope.enrollment()?;
        let binding = evidence.binding();
        if target.purpose() != self.purpose
            || target.predecessor_semantic() != Some(self.semantic)
            || target.outer_intent() != binding.outer_intent
            || target.predecessor_body() != binding.body_selection
            || target.successor_selection() == [0; 32]
            || target.fees() != evidence.fees()
            || self
                .closing
                .as_ref()
                .is_some_and(|plan| plan.successor != target.successor_selection())
        {
            return Err(invalid("unsigned closure selected another body successor"));
        }
        self.require_fees(evidence.fees())?;
        Ok(())
    }
    fn verify_unsigned_wallets(
        &self,
        inspect: &mut impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<()> {
        self.verify_wallets(|attempt| {
            let value = inspect(attempt)?;
            if matches!(
                value.phase(),
                NativePreparationPhase::PayloadRetained | NativePreparationPhase::Signed
            ) {
                return Err(invalid(
                    "body closure cannot replace retained paid material",
                ));
            }
            Ok(value)
        })?;
        for attempt in &self.attempts {
            require_no_native_effects(attempt)?;
        }
        Ok(())
    }
    fn inspect_unsigned_tail(
        &self,
        inspect: &mut impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<ClosureTail> {
        self.verify_unsigned_wallets(inspect)?;
        if self
            .attempts
            .iter()
            .take(self.attempts.len().saturating_sub(1))
            .any(|attempt| attempt.retirement.is_none())
        {
            return Err(ManagedBootstrapFailure::TransitionPending.into());
        }
        let Some(last) = self.attempts.last() else {
            return Ok(ClosureTail::Empty);
        };
        let authorization = last.digest()?;
        if last.observation.is_none() {
            require_missing_wallet(last)?;
            return Ok(ClosureTail::Missing { authorization });
        }
        let value = inspect(last)?;
        match value.phase() {
            NativePreparationPhase::Missing => {
                require_missing_wallet(last)?;
                Ok(ClosureTail::Missing { authorization })
            }
            NativePreparationPhase::RequestOnly => {
                let request_sha256 = value
                    .request_sha256()
                    .filter(|value| valid_sha256(value))
                    .ok_or_else(|| invalid("unsigned closure lost its exact request commitment"))?
                    .to_owned();
                Ok(ClosureTail::Request {
                    authorization,
                    request_sha256,
                })
            }
            NativePreparationPhase::Retired => {
                Err(invalid("retired tail lacks its exact closing plan"))
            }
            NativePreparationPhase::PayloadRetained | NativePreparationPhase::Signed => Err(
                invalid("body closure cannot replace retained paid material"),
            ),
        }
    }
    pub(in crate::managed) fn prepare_unsigned_closure(
        self,
        successor: &dyn SemanticSuccessor,
        authorization: &dyn DispatchAuthorization,
        deadline: Instant,
        mut inspect: impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
        mut retire: impl FnMut(&Attempt) -> Result<RetiredNativeRequest>,
    ) -> Result<PendingUnsignedClosure> {
        self.check_successor(successor)?;
        authorization.check(self.purpose, deadline)?;
        self.require_fees(authorization.fees())?;
        authorization.claim_body_replacement(successor.target(), deadline)?;
        self.verify_wallets_for_pending_close(&mut inspect)?;
        // This is only publication of the exact anchored original authorization, never new Terms.
        if self.reservation_pending() {
            self.finish_reserved(&self.operation, true)?;
        }
        let history = self.reread()?;
        history.check_successor(successor)?;
        if history.closing.is_none() {
            // Finish only an existing same-body retirement prefix. The exact Published successor
            // already consumes its original slot; this neither selects Terms nor appends an attempt.
            history.verify_unsigned_wallets(&mut inspect)?;
            if let (Some(previous), Some(last)) = (history.predecessor(), history.last()) {
                if previous.retirement.is_none() {
                    authorization.check(history.purpose, deadline)?;
                    successor.revalidate()?;
                    history.require_current()?;
                    selection::retire_predecessor(previous, last, &mut inspect, &mut retire)?;
                }
            }
        }
        let history = history.reread()?;
        history.check_successor(successor)?;
        authorization.check(history.purpose, deadline)?;
        if history.closing.is_none() {
            let tail = history.inspect_unsigned_tail(&mut inspect)?;
            let cumulative = history
                .scope
                .prior_total()
                .checked_add(history.reserved_attempt_count())
                .ok_or_else(|| invalid("body dispatch count overflow"))?;
            let plan = ClosurePlan {
                purpose: history.purpose,
                semantic: history.semantic,
                scope: history.scope.commitment()?,
                dispatch: history.dispatch.clone(),
                successor: successor.target().successor_selection(),
                tail,
                cumulative_reserved: u8::try_from(cumulative)
                    .map_err(|_| invalid("body dispatch count overflow"))?,
            };
            authorization.check(history.purpose, deadline)?;
            successor.revalidate()?;
            history.require_current()?;
            write_record(&history.operation, "closing.nrt", &plan)?;
        } else {
            history.verify_unsigned_closure(successor, &mut inspect)?;
        }
        let history = history.reread()?;
        authorization.check(history.purpose, deadline)?;
        Ok(PendingUnsignedClosure { history })
    }
    fn verify_wallets_for_pending_close(
        &self,
        inspect: &mut impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<()> {
        match &self.closing {
            Some(plan) => self.verify_wallets_inner(inspect, Some(plan)),
            None => self.verify_unsigned_wallets(inspect),
        }
    }
    /// Verify the complete pending or closed transition. None means a valid incomplete closure,
    /// never permission to ignore an existing retired wallet or to resume ordinary dispatch.
    pub(in crate::managed) fn verify_unsigned_closure(
        &self,
        successor: &dyn SemanticSuccessor,
        mut inspect: impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<Option<VerifiedUnsignedClosure>> {
        self.check_successor(successor)?;
        let Some(plan) = &self.closing else {
            // A reserved outer successor may coexist with an interrupted same-body unsigned
            // retirement. Validate that exact prefix read-only; prepare closes it through the
            // sole canonical retire_predecessor owner before publishing a closing plan.
            self.verify_unsigned_wallets(&mut inspect)?;
            return Ok(None);
        };
        self.verify_wallets_inner(&mut inspect, Some(plan))?;
        for attempt in &self.attempts {
            require_no_native_effects(attempt)?;
        }
        let Some(closed) = &self.closed else {
            return Ok(None);
        };
        if let (ClosureTail::Request { request_sha256, .. }, Some(last)) =
            (&plan.tail, self.attempts.last())
        {
            let value = inspect(last)?;
            if value.phase() != NativePreparationPhase::Retired
                || value.request_sha256() != Some(request_sha256.as_str())
            {
                return Err(invalid(
                    "closed body lacks canonical exact unsigned retirement",
                ));
            }
        }
        let evidence = self.scope.enrollment()?;
        let value = VerifiedUnsignedClosure {
            history: Self::read_retained(
                &self.operation,
                self.purpose,
                self.semantic,
                &self.scope,
                self,
            )?,
            digest: digest(closed)?,
            successor: plan.successor,
            cumulative_reserved: usize::from(plan.cumulative_reserved),
            outer_intent: evidence.binding().outer_intent,
            root_identity: evidence.root().identity()?,
            fees: evidence.fees().clone(),
        };
        value.require_retained()?;
        successor.revalidate()?;
        Ok(Some(value))
    }
}
impl PendingUnsignedClosure {
    pub(in crate::managed) fn finish(
        self,
        successor: &dyn SemanticSuccessor,
        authorization: &dyn DispatchAuthorization,
        deadline: Instant,
        mut inspect: impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
        mut retire: impl FnMut(&Attempt) -> Result<RetiredNativeRequest>,
    ) -> Result<VerifiedUnsignedClosure> {
        authorization.check(self.history.purpose, deadline)?;
        self.history.require_fees(authorization.fees())?;
        authorization.claim_body_replacement(successor.target(), deadline)?;
        if let Some(value) = self
            .history
            .verify_unsigned_closure(successor, &mut inspect)?
        {
            return Ok(value);
        }
        let plan = self
            .history
            .closing
            .as_ref()
            .ok_or_else(|| invalid("unsigned closing plan absent"))?;
        if let ClosureTail::Request { request_sha256, .. } = &plan.tail {
            let last = self
                .history
                .attempts
                .last()
                .ok_or_else(|| invalid("unsigned closing tail absent"))?;
            authorization.check(self.history.purpose, deadline)?;
            successor.revalidate()?;
            let receipt = retire(last)?;
            if receipt.journal_path() != last.wallet_path()
                || receipt.request_sha256() != request_sha256
            {
                return Err(invalid("body retirement selected another original wallet"));
            }
            let value = inspect(last)?;
            if value.phase() != NativePreparationPhase::Retired
                || value.request_sha256() != Some(request_sha256.as_str())
            {
                return Err(invalid(
                    "body retirement is not retained at the original wallet",
                ));
            }
        }
        authorization.check(self.history.purpose, deadline)?;
        self.history.check_successor(successor)?;
        self.history
            .verify_wallets_inner(&mut inspect, Some(plan))?;
        write_record(
            &self.history.operation,
            "closed.nrt",
            &ClosureRecord {
                plan: digest(plan)?,
                tail: plan.tail.clone(),
            },
        )?;
        let history = self.history.reread()?;
        authorization.check(history.purpose, deadline)?;
        history
            .verify_unsigned_closure(successor, &mut inspect)?
            .ok_or_else(|| invalid("unsigned body closure was not retained"))
    }
}
impl ClosurePlan {
    pub(super) fn permits_retired_tail(
        &self,
        attempt: &Attempt,
        preparation: &VerifiedNativePreparation,
    ) -> Result<bool> {
        Ok(
            matches!(&self.tail, ClosureTail::Request { authorization, request_sha256 }
            if *authorization == attempt.digest()? && preparation.request_sha256() == Some(request_sha256.as_str())),
        )
    }
    pub(super) fn validate_tail_wallet(
        &self,
        attempt: &Attempt,
        preparation: &VerifiedNativePreparation,
    ) -> Result<()> {
        let valid = match &self.tail {
            ClosureTail::Missing { authorization } => {
                *authorization == attempt.digest()?
                    && preparation.phase() == NativePreparationPhase::Missing
            }
            ClosureTail::Request {
                authorization,
                request_sha256,
            } => {
                *authorization == attempt.digest()?
                    && matches!(
                        preparation.phase(),
                        NativePreparationPhase::RequestOnly | NativePreparationPhase::Retired
                    )
                    && preparation.request_sha256() == Some(request_sha256.as_str())
            }
            ClosureTail::Empty => false,
        };
        if valid {
            Ok(())
        } else {
            Err(invalid(
                "closing wallet changed its exact unsigned phase or request",
            ))
        }
    }
}
fn require_no_native_effects(attempt: &Attempt) -> Result<()> {
    if attempt
        .directory
        .entries(7)?
        .iter()
        .any(|name| name == "carrier.nrt" || name == "replay.nrt")
    {
        return Err(invalid("native execution custody cannot close as unsigned"));
    }
    Ok(())
}
fn require_missing_wallet(attempt: &Attempt) -> Result<()> {
    if attempt.is_committed() {
        return Err(invalid("committed request absence cannot close a body"));
    }
    if attempt.directory.entries(7)?.iter().any(|name| {
        !["authorization.nrt", "observation.nrt", "retired.nrt"]
            .iter()
            .any(|allowed| name == *allowed)
    }) {
        return Err(invalid(
            "missing-wallet body closure found later or unknown custody",
        ));
    }
    Ok(())
}
