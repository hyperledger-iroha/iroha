//! Exact paid alias requests using the shared transaction budget and immutable native journal.

use super::*;
use private_root::BoundedOperationExpectation;

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum AliasFeeBounds {
    Quoted,
    Bounded(BoundedTerms),
}

impl AccountService {
    /// Prepare one paid alias request within explicit aggregate fee limits and one deadline.
    ///
    /// The verified alias request and plan separately bind lease rent and ownership terms.
    /// These bounds cap the transaction's execution fees; they do not replace lease price guards.
    /// The ordinary alias planner, quote, signer and journal are reused without another transaction.
    ///
    /// # Errors
    /// Rejects changed request or quote terms, insufficient funds, increased fee limits, expired
    /// budgets and unsafe journals before any transaction is submitted.
    pub fn prepare_alias_bounded(
        &self,
        request: &AliasSetupPlanRequestV1,
        options: &BoundedTransactionOptions,
        journal: &Path,
    ) -> Result<OperationReport> {
        let terms = BoundedTerms::new(options)?;
        self.with_deadline(options.deadline)?
            .prepare_alias_with_bounds(
                request,
                options.fee_payment.clone(),
                AliasFeeBounds::Bounded(terms),
                journal,
            )
    }

    /// Verify the original paid alias request and fee limits against an immutable saved journal.
    ///
    /// Keep the original request's lease generation and quote guards on retry. Only the options'
    /// monotonic I/O deadline may be refreshed; the signed transaction lifetime stays unchanged.
    ///
    /// # Errors
    /// Rejects substituted alias/owner/rent terms, changed fee authorization or unsafe evidence.
    pub fn verify_alias_journal(
        &self,
        journal: &Path,
        request: &AliasSetupPlanRequestV1,
        options: &BoundedTransactionOptions,
    ) -> Result<()> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(journal)?;
        let record: TransactionJournal = journal.read_operation()?;
        record.verify(&self.config)?;
        BoundedOperationExpectation::Alias(request, options).verify(&record)
    }

    /// Submit the original bounded alias transaction once after comparing its request under lock.
    ///
    /// # Errors
    /// Rejects changed request/fee authorization or an expired I/O budget before dispatch.
    pub fn submit_alias_bounded(
        &self,
        journal: &Path,
        request: &AliasSetupPlanRequestV1,
        options: &BoundedTransactionOptions,
    ) -> Result<OperationReport> {
        self.with_deadline(options.deadline)?.run_transaction(
            journal,
            NativeOperationKind::AliasSetup,
            true,
            Some(BoundedOperationExpectation::Alias(request, options)),
        )
    }

    /// Reconcile the exact bounded alias transaction without signing or submitting again.
    ///
    /// # Errors
    /// Rejects changed request/fee authorization, expired I/O or untrusted observations.
    pub fn resume_alias_bounded(
        &self,
        journal: &Path,
        request: &AliasSetupPlanRequestV1,
        options: &BoundedTransactionOptions,
    ) -> Result<OperationReport> {
        self.with_deadline(options.deadline)?.run_transaction(
            journal,
            NativeOperationKind::AliasSetup,
            false,
            Some(BoundedOperationExpectation::Alias(request, options)),
        )
    }
}
