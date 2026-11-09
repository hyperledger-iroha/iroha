// Included in world::isi: one selected resolver for the existing Parliament engine.

#[derive(Clone, Debug)]
pub(crate) enum ParliamentProposalHost {
    Baseline(crate::state::GovernanceProposalRecord),
    Sorafs(super::sorafs_authority::SorafsAuthorityProposalRecordV1),
}

impl ParliamentProposalHost {
    pub(crate) fn resolve(
        world: &impl crate::state::WorldReadOnly,
        id: [u8; 32],
    ) -> Result<Self, Error> {
        if super::sorafs_authority::active(world)? {
            let host = Self::resolve_retained(world, id)?;
            if matches!(&host, Self::Sorafs(r) if r.deadline_closure.is_some()) {
                return Err(invalid_governance_parameter(
                    "SoraFS authority decision is closed at its canonical deadline",
                ));
            }
            return Ok(host);
        }
        // The frozen baseline resolves only its original proposal table, including
        // after a voted retirement leaves historical addition-owned receipts behind.
        let host = Self::Baseline(
            world
                .governance_proposals()
                .get(&id)
                .cloned()
                .ok_or_else(|| invalid_governance_parameter("Parliament proposal is missing"))?,
        );
        if host.fingerprint() != id {
            return Err(invalid_governance_parameter(
                "Parliament proposal key differs from content",
            ));
        }
        Ok(host)
    }
    // Restore verifies retained terminal receipts even after their services have retired.
    // This resolver grants no execution authority; execution always uses `resolve` above.
    pub(crate) fn resolve_retained(
        world: &impl crate::state::WorldReadOnly,
        id: [u8; 32],
    ) -> Result<Self, Error> {
        let baseline = world.governance_proposals().get(&id).cloned();
        let hosted = world.sorafs_authority_proposals().get(&id).cloned();
        let record = match (baseline, hosted) {
            (Some(record), None) => Self::Baseline(record),
            (None, Some(record)) => Self::Sorafs(record),
            (None, None) => {
                return Err(invalid_governance_parameter(
                    "Parliament proposal is missing",
                ));
            }
            (Some(_), Some(_)) => {
                return Err(invalid_governance_parameter(
                    "ambiguous Parliament proposal identity",
                ));
            }
        };
        if record.fingerprint() != id {
            return Err(invalid_governance_parameter(
                "Parliament proposal key differs from content",
            ));
        }
        Ok(record)
    }
    fn fingerprint(&self) -> [u8; 32] {
        match self {
            Self::Baseline(r) => r.kind.fingerprint(),
            Self::Sorafs(r) => r.proposal.fingerprint(),
        }
    }
    fn effect_hash(&self) -> [u8; 32] {
        match self {
            Self::Baseline(r) => r.kind.effect_preimage_hash_v1(),
            Self::Sorafs(r) => r.proposal.effect_preimage_hash_v1(),
        }
    }
    fn status(&self) -> crate::state::GovernanceProposalStatus {
        match self {
            Self::Baseline(r) => r.status,
            Self::Sorafs(r) => r.status,
        }
    }
    fn baseline_kind(&self) -> Option<&ProposalKind> {
        match self {
            Self::Baseline(r) => Some(&r.kind),
            Self::Sorafs(_) => None,
        }
    }
    fn validate_record(
        &self,
        status: crate::state::GovernanceProposalStatus,
        tx: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Baseline(r) => {
                ensure_certificate_only_proposal_status_v1(self.fingerprint(), r, status, tx)
            }
            Self::Sorafs(r) => {
                if r.deadline_closure.is_some()
                    || super::sorafs_authority::proposal_deadline(&r.proposal)
                        .is_none_or(|deadline| tx.block_height() >= deadline)
                    || r.status != status
                    || r.proposal.invariant_error().is_some()
                    || r.proposal.network != tx.network_id
                    || r.created_height == 0
                    || standalone_governance_state_contains_proposal_id_v1(self.fingerprint(), tx)
                {
                    return Err(invalid_governance_parameter(
                        "invalid hosted certificate-only proposal",
                    ));
                }
                Ok(())
            }
        }
    }
    pub(crate) fn validate_bindings(
        &self,
        attempt: &ParliamentAttemptStateV1,
    ) -> Result<(), crate::governance::parliament::ParliamentReducerErrorV1> {
        match self {
            Self::Baseline(r) => attempt.validate_proposal_bindings_v1(&r.kind),
            Self::Sorafs(r) => {
                if attempt.expected_head()
                    != super::sorafs_authority::admitted_expected_head(&r.proposal)
                {
                    return Err(crate::governance::parliament::ParliamentReducerErrorV1::ProposalBindingMismatch);
                }
                let (risk, bodies) =
                    crate::governance::parliament::sorafs_authority_attempt_policy_v1();
                attempt.validate_hosted_proposal_bindings_v1(
                    r.proposal.proposal_content_id(),
                    r.proposal.effect_preimage_hash_v1(),
                    r.proposal.governed_subject_id_v1(),
                    risk,
                    &bodies,
                )
            }
        }
    }
    fn expected_head(
        &self,
        tx: &StateTransaction<'_, '_>,
    ) -> Result<GovernanceExpectedHeadV1, crate::execution_attempt::ExecutionAttemptError<Error>>
    {
        match self {
            Self::Baseline(r) => parliament_expected_head_v1(&r.kind, tx),
            Self::Sorafs(r) => Ok(super::sorafs_authority::expected_head(
                &r.proposal,
                &tx.world,
            )),
        }
    }
    fn attempt_expected_head(
        &self,
        tx: &StateTransaction<'_, '_>,
    ) -> Result<GovernanceExpectedHeadV1, crate::execution_attempt::ExecutionAttemptError<Error>>
    {
        match self {
            Self::Baseline(r) => parliament_expected_head_v1(&r.kind, tx),
            Self::Sorafs(r) => Ok(super::sorafs_authority::admitted_expected_head(&r.proposal)),
        }
    }
    fn apply_effect(
        &self,
        certificate: &GovernanceCertificateV1,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Baseline(r) => {
                apply_parliament_proposal_effect_v1(self.fingerprint(), r, certificate, tx)
            }
            Self::Sorafs(r) => super::sorafs_authority::apply_certified_effect(r, certificate, tx),
        }
    }
    fn write_status(
        mut self,
        status: crate::state::GovernanceProposalStatus,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let id = self.fingerprint();
        match &mut self {
            Self::Baseline(r) => {
                r.status = status;
                tx.world
                    .put_governance_proposal(id, r.clone())
                    .map_err(governance_proposal_storage_error)?;
            }
            Self::Sorafs(r) => {
                r.status = status;
                tx.world.sorafs_authority_proposals.insert(id, r.clone());
            }
        }
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs_authority::ProposeSorafsAuthorityGovernanceV1 {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !is_bonded_citizen(authority, tx) {
            return Err(invalid_governance_parameter(
                "only a bonded citizen may propose SoraFS operating authority changes",
            ));
        }
        super::sorafs_authority::validate_proposal(&self.proposal, tx)?;
        let id = self.proposal.fingerprint();
        if standalone_governance_state_contains_proposal_id_v1(id, tx) {
            return Err(invalid_governance_parameter(
                "SoraFS authority proposal aliases standalone governance state",
            ));
        }
        if tx.world.governance_proposals.get(&id).is_some() {
            return Err(invalid_governance_parameter(
                "Parliament content identity collision",
            ));
        }
        if let Some(existing) = tx.world.sorafs_authority_proposals.get(&id) {
            if existing.proposal != self.proposal {
                return Err(invalid_governance_parameter(
                    "SoraFS proposal content collision",
                ));
            }
            return Ok(());
        }
        let created_height = tx.block_height();
        tx.world.sorafs_authority_proposals.insert(
            id,
            super::sorafs_authority::SorafsAuthorityProposalRecordV1 {
                deadline_closure: None,
                proposer: authority.clone(),
                proposal: self.proposal,
                created_height,
                status: crate::state::GovernanceProposalStatus::Proposed,
            },
        );
        tx.world
            .emit_events(Some(GovernanceEvent::ProposalSubmitted(
                iroha_data_model::events::data::governance::GovernanceProposalSubmitted {
                    id,
                    proposer: authority.clone(),
                    contract_address: None,
                },
            )));
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs_authority::CreateSorafsAuthorityParliamentAttemptV1 {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_parliament_manager(authority, tx)?;
        // Accepted content remains finishable after its target changes. Its exact
        // voted predecessor is frozen in the attempt; the due effect supersedes it.
        let host = ParliamentProposalHost::resolve(&tx.world, self.proposal.fingerprint())?;
        if !matches!(&host, ParliamentProposalHost::Sorafs(r) if r.proposal == self.proposal) {
            return Err(invalid_governance_parameter(
                "attempt requires exact admitted SoraFS proposal",
            ));
        }
        create_parliament_host_attempt(host, self.attempt_sequence, tx)
    }
}
