// Exact registry authentication for lifecycle-owned Validate sidecar waits.
impl ConcreteLifecycleWorkRegistry {
    pub(super) fn exactly_matches_validate_sidecar_registration(
        &self,
        identity: &super::validate_sidecar::LifecycleValidateSidecarRegistrationIdentityV1,
    ) -> bool {
        let key = identity.dispatch_key();
        let Some(address) =
            ConcreteWorkAddress::new(key.owner(), key.lifecycle_ordinal(), key.slot())
        else {
            return false;
        };
        let Some(work) = self.entries.get(&address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return false;
        };
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &validate.effect
        else {
            return false;
        };
        work.digest == key.digest()
            && work.validates_at(address)
            && validate.validates(work.digest)
            && *round == identity.round()
            && *subject == identity.subject()
            && validate.durable_receipt.round() == identity.round()
            && validate.durable_receipt.subject() == identity.subject()
            && validate.durable_receipt.context_id().0.as_ref()
                == identity.lifecycle_key().context().as_bytes()
            && identity.lifecycle_key().round().height() == identity.round().height
            && identity.lifecycle_key().phase() == LifecyclePhase::Validate
            && identity.lifecycle_stage().kind() == LifecycleStageKind::ValidateBody
            && identity.lifecycle_stage().predecessor_scope() == PredecessorScope::Independent
            && durable_validation_wait_source_from_exact_parts(
                address,
                work.digest,
                validate.pending.causal_lifecycle_key(),
                validate.pending.candidate_statement(),
                &validate.durable_receipt,
                validate.expected_manifest_hash,
                identity.lifecycle_key(),
                identity.lifecycle_stage(),
            ) == identity.wait_token().source()
    }
}
