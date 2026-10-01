//! Typed bounded parameter writes over the shared native exact-operation journal.

use super::*;
use iroha_data_model::parameter::Parameter;
use private_root::BoundedOperationExpectation;

/// The exact parameter and original spending authorization selected before preparation.
#[derive(Clone, Debug)]
pub struct ParameterUpdateRequest {
    /// Native parameter; catalog requests retain every owner generation and expected state root.
    pub parameter: Parameter,
    /// Explicit fee ceilings and one current I/O deadline.
    pub options: BoundedTransactionOptions,
}

impl AccountService {
    /// Quote, sign and atomically retain one exact parameter write without dispatching.
    ///
    /// # Errors
    /// Rejects unsafe/existing custody, invalid fees, an expired deadline and incompatible SDK/node.
    pub fn prepare_parameter_update(
        &self,
        request: &ParameterUpdateRequest,
        journal: &Path,
    ) -> Result<OperationReport> {
        let operation = NativeOperation::ParameterUpdate {
            parameter: request.parameter.clone(),
            terms: BoundedTerms::new(&request.options)?,
        };
        let account = self.with_deadline(request.options.deadline)?;
        account.prepare_native(operation, request.options.fee_payment.clone(), journal)?;
        account.inspect_parameter_update(journal, request)
    }

    /// Authenticate original signed intent before an external finality owner records it.
    ///
    /// This does no network I/O and never repairs or replaces journal evidence. The returned
    /// signed wire is public evidence, not a grant of independent parent finality.
    ///
    /// # Errors
    /// Rejects changed parameter, fee limits, signer/network or unsafe/substituted journal bytes.
    pub fn inspect_parameter_update(
        &self,
        path: &Path,
        expected: &ParameterUpdateRequest,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(path)?;
        let record: TransactionJournal = journal.read_operation()?;
        let transaction = record.verify(&self.config)?;
        BoundedOperationExpectation::Parameter(expected).verify(&record)?;
        let status = if journal.submission_recorded(&record)? {
            OperationStatus::Pending
        } else {
            OperationStatus::Prepared
        };
        let mut report = transfer_report(&journal, &record, status, None);
        let fields = report
            .data
            .as_object_mut()
            .ok_or_else(|| eyre!("parameter operation projection is not an object"))?;
        fields.insert(
            "native_signed_transaction_hex".into(),
            Value::String(hex::encode(transaction.encode_versioned())),
        );
        Ok(report)
    }

    /// Dispatch only this original unattempted transaction; an existing attempt is never replayed.
    ///
    /// # Errors
    /// Rejects changed request/fees, elapsed I/O budget or malformed custody/observations.
    pub fn submit_parameter_update(
        &self,
        path: &Path,
        expected: &ParameterUpdateRequest,
    ) -> Result<OperationReport> {
        self.with_deadline(expected.options.deadline)?
            .run_transaction(
                path,
                NativeOperationKind::ParameterUpdate,
                true,
                Some(BoundedOperationExpectation::Parameter(expected)),
            )
    }

    /// Reconcile only the retained hash without generating, signing or submitting another write.
    ///
    /// # Errors
    /// Rejects changed request/fees, elapsed I/O budget or malformed custody/observations.
    pub fn resume_parameter_update(
        &self,
        path: &Path,
        expected: &ParameterUpdateRequest,
    ) -> Result<OperationReport> {
        self.with_deadline(expected.options.deadline)?
            .run_transaction(
                path,
                NativeOperationKind::ParameterUpdate,
                false,
                Some(BoundedOperationExpectation::Parameter(expected)),
            )
    }
}
