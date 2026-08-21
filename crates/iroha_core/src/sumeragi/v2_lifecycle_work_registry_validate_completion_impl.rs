impl ConcreteLifecycleWorkRegistry {
    /// Reattach the complete executed dispatch and its exact wake authority.
    ///
    /// This is the sole registry entry to volatile Validate completion. Every
    /// failure returns the original move-only dispatch and leaves the map
    /// untouched; success retains the exclusive borrow in a sealed preflight.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_executed_durable_validate_completion(
        &mut self,
        dispatch: ExecutedDurableValidateDispatch,
    ) -> Result<
        PreparedExecutedDurableValidateCompletion<'_>,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        let ExecutedDurableValidateDispatch { executed, wake } = dispatch;
        let prepared = match self.reattach_durable_validate_execution(executed) {
            Ok(prepared) => prepared,
            Err((error, executed)) => {
                return Err((
                    DurableValidateCompletionPublicationError::Registry(
                        DurableValidateCompletionConversionError::Execution(error),
                    ),
                    ExecutedDurableValidateDispatch { executed, wake },
                ));
            }
        };
        let PreparedDurableValidateCompletion {
            _registry: registry,
            executed,
        } = prepared;
        let dispatch = ExecutedDurableValidateDispatch { executed, wake };
        let request = &dispatch.executed.request;
        let expected_source = durable_validation_wait_source_for_request(request);
        if dispatch.wake.wait_token.source() != expected_source
            || dispatch.wake.wait_token.observed_generation() == u64::MAX
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidWakeAuthority,
                ),
                dispatch,
            ));
        }
        let Some(outcome_kind) = durable_validate_outcome_kind(dispatch.outcome()) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        let replacement_digest = durable_validate_completion_digest(
            request.incumbent_digest,
            request.expected_manifest_hash,
            dispatch.outcome(),
        );
        if matches!(
            outcome_kind,
            DurableValidateOutcomeKind::Validated | DurableValidateOutcomeKind::Rejected
        ) && replacement_digest.is_none_or(|digest| digest == request.incumbent_digest)
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidReplacementDigest,
                ),
                dispatch,
            ));
        }
        if outcome_kind == DurableValidateOutcomeKind::DeferredMergeSidecar
            && replacement_digest.is_some()
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let Some(payload) = durable_validate_body_payload(&request.durable_receipt) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        if !super::body_pipeline_transition::durable_validate_payload_is_exact(
            request.lifecycle_key,
            payload,
        ) {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let authority = DurableValidateCompletionAuthority {
            address: request.address,
            incumbent_digest: request.incumbent_digest,
            replacement_digest,
            wait_token: dispatch.wake.wait_token,
            outcome_kind,
            lifecycle_key: request.lifecycle_key,
            lifecycle_stage: request.lifecycle_stage,
            payload,
        };
        Ok(PreparedExecutedDurableValidateCompletion {
            registry,
            dispatch,
            authority,
        })
    }
}
