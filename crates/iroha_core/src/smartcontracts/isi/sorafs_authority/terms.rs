//! Irrevocable, separately voted term mandates and their native execution receipts.

use super::*;
use iroha_data_model::runtime::definition::DefinitionId;

/// Retained certificate whose approved effect has already enacted.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityApprovalV1")]
pub struct SorafsAuthorityApprovalV1 {
    /// Exact immutable decision.
    pub proposal_content_id: ProposalContentId,
    /// Shared Parliament attempt owning the certificate.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Content identity of the retained certificate.
    pub certificate_id: GovernanceCertificateId,
    /// Approval enactment height, distinct from authority installation or removal.
    pub approved_at_height: u64,
}

impl SorafsAuthorityApprovalV1 {
    fn from_certificate(certificate: &GovernanceCertificateV1) -> Self {
        Self {
            proposal_content_id: certificate.proposal_content_id,
            governance_attempt_id: certificate.governance_attempt_id,
            certificate_id: GovernanceCertificateId::derive_v1(certificate),
            approved_at_height: certificate.enact_at_height,
        }
    }
}

/// Actual projection change, under an exact retained native completion owner.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityExecutionV1")]
pub struct SorafsAuthorityExecutionV1 {
    /// Successful subject revision.
    pub revision: u64,
    /// Actual committed effect height.
    pub at_height: u64,
    /// Separately retained authorization for this effect.
    pub approval: SorafsAuthorityApprovalV1,
    /// Captured selected services semantics.
    pub services_profile: DefinitionId,
    /// Captured native authority implementation.
    pub implementation: DefinitionId,
}

/// One exact plan's approvals and one-use grant/termination capabilities.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityLeaseV1")]
pub struct SorafsAuthorityLeaseV1 {
    /// Canonical plan content, normalized to the Grant action.
    pub plan: SorafsAuthorityProposalV1,
    /// Enacted approval of the grant.
    pub grant_approval: Option<SorafsAuthorityApprovalV1>,
    /// Enacted, irrevocable approval of terminal revocation.
    pub terminal_approval: Option<SorafsAuthorityApprovalV1>,
    /// Actual successful grant and captured completion owner.
    pub activation: Option<SorafsAuthorityExecutionV1>,
    /// Terminal effect, either voted early revocation or scheduled termination.
    pub completion: Option<SorafsAuthorityExecutionV1>,
    /// Whether the one permitted start has been processed, including refusal.
    pub start_processed: bool,
}

fn owner(
    world: &impl WorldReadOnly,
) -> Result<(DefinitionId, DefinitionId), InstructionExecutionError> {
    require_active(world)?;
    let selection = world.runtime_selection();
    let binding = selection
        .bindings
        .iter()
        .find(|b| b.wire_id == "iroha.sorafs.authority.propose.v1")
        .ok_or_else(|| invalid("selected authority completion owner is missing"))?;
    Ok((selection.services_profile, binding.implementation))
}

fn validate_approval(
    world: &impl WorldReadOnly,
    plan: &SorafsAuthorityProposalV1,
    approval: &SorafsAuthorityApprovalV1,
    action: SorafsAuthorityActionV1,
) -> Result<(), String> {
    let mut content = plan.clone();
    content.action = action;
    let record = world
        .sorafs_authority_proposals()
        .get(approval.proposal_content_id.as_bytes())
        .ok_or("lease approval has no proposal")?;
    let attempt = world
        .parliament_attempts()
        .get(&approval.governance_attempt_id)
        .ok_or("lease approval has no shared attempt")?;
    let certificate = attempt
        .certificate()
        .ok_or("lease approval has no certificate")?;
    if record.proposal != content
        || content.proposal_content_id() != approval.proposal_content_id
        || record.status != GovernanceProposalStatus::Enacted
        || attempt.attempt().status != GovernanceAttemptStatusV1::Enacted
        || certificate.proposal_content_id != approval.proposal_content_id
        || certificate.governance_attempt_id != approval.governance_attempt_id
        || certificate.effect_preimage_hash != content.effect_preimage_hash_v1()
        || certificate.expected_head != admitted_expected_head(&content)
        || certificate.enact_at_height != approval.approved_at_height
        || GovernanceCertificateId::derive_v1(certificate) != approval.certificate_id
        || attempt.validate().is_err()
        || certificate.validate().is_err()
    {
        return Err("lease approval differs from enacted exact decision".into());
    }
    Ok(())
}

pub(super) fn validate_lease(
    world: &impl WorldReadOnly,
    id: [u8; 32],
    lease: &SorafsAuthorityLeaseV1,
) -> Result<(), String> {
    let plan = &lease.plan;
    let window = plan.grid.window(plan.term).ok_or("lease term overflow")?;
    if id != plan.plan_id()
        || plan.invariant_error().is_some()
        || plan.action != SorafsAuthorityActionV1::Grant
        || (lease.grant_approval.is_none() && lease.terminal_approval.is_none())
    {
        return Err("noncanonical authority lease plan".into());
    }
    for (approval, action) in [
        (&lease.grant_approval, SorafsAuthorityActionV1::Grant),
        (
            &lease.terminal_approval,
            SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
        ),
    ] {
        if let Some(approval) = approval {
            validate_approval(world, plan, approval, action)?;
            if approval.approved_at_height < window.admission_height
                || approval.approved_at_height >= window.cutoff_height
            {
                return Err("lease approval missed its canonical cutoff".into());
            }
        }
    }
    if let Some(activation) = &lease.activation {
        if !lease.start_processed
            || lease.grant_approval.as_ref() != Some(&activation.approval)
            || lease.terminal_approval.is_none()
            || activation.revision != plan.expected_revision + 1
            || activation.at_height != window.start_height
        {
            return Err("authority grant lacks its exact paired approvals or start receipt".into());
        }
        let Some(iroha_data_model::runtime::definition::DefinitionDescriptorV1::Implementation(
            implementation,
        )) = world.runtime_definitions().get(&activation.implementation)
        else {
            return Err("authority completion implementation has no published definition".into());
        };
        if implementation.services_profile != activation.services_profile
            || !matches!(&implementation.code, iroha_data_model::runtime::definition::ImplementationCodeV1::Native { module, .. } if module == "iroha.sorafs.authority_governance.v1")
        {
            return Err(
                "authority captured completion owner differs from selected semantics".into(),
            );
        }
        if world
            .sorafs_authority_leases()
            .iter()
            .any(|(other_id, other)| {
                *other_id != id
                    && other.plan.family_id() == plan.family_id()
                    && other.activation.is_some()
            })
        {
            return Err("more than one authority grant activated in a term".into());
        }
        if lease.completion.is_none()
            && owner(world).map_err(|error| format!("{error:?}"))?
                != (activation.services_profile, activation.implementation)
        {
            return Err("active lease lost its exact completion owner".into());
        }
    }
    if let Some(completion) = &lease.completion {
        let activation = lease
            .activation
            .as_ref()
            .ok_or("terminated lease was never active")?;
        if completion.revision != activation.revision + 1
            || completion.at_height < activation.at_height
            || completion.services_profile != activation.services_profile
            || completion.implementation != activation.implementation
        {
            return Err("lease termination changed its captured authority".into());
        }
        if lease.terminal_approval.as_ref() == Some(&completion.approval) {
            if completion.at_height != window.end_height {
                return Err("scheduled termination missed its exact height".into());
            }
        } else {
            validate_approval(
                world,
                plan,
                &completion.approval,
                SorafsAuthorityActionV1::Revoke,
            )?;
            if completion.at_height != completion.approval.approved_at_height
                || completion.at_height >= window.end_height
            {
                return Err("early revocation is outside its original term".into());
            }
        }
    }
    Ok(())
}

pub(super) fn approve(
    proposal: &SorafsAuthorityProposalV1,
    certificate: &GovernanceCertificateV1,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let id = proposal.plan_id();
    let mut plan = proposal.clone();
    plan.action = SorafsAuthorityActionV1::Grant;
    let mut lease = tx
        .world
        .sorafs_authority_leases
        .get(&id)
        .cloned()
        .unwrap_or(SorafsAuthorityLeaseV1 {
            plan,
            grant_approval: None,
            terminal_approval: None,
            activation: None,
            completion: None,
            start_processed: false,
        });
    if lease.plan.plan_id() != id || lease.start_processed {
        return Err(invalid(
            "authority approval cannot replace a processed lease",
        ));
    }
    let slot = match proposal.action {
        SorafsAuthorityActionV1::Grant => &mut lease.grant_approval,
        SorafsAuthorityActionV1::AuthorizeTerminalRevoke => &mut lease.terminal_approval,
        SorafsAuthorityActionV1::Revoke => {
            return Err(invalid("early revocation is not a dormant approval"));
        }
    };
    if slot.is_some() {
        return Err(invalid("authority approval already enacted"));
    }
    *slot = Some(SorafsAuthorityApprovalV1::from_certificate(certificate));
    tx.world.sorafs_authority_leases.insert(id, lease);
    Ok(())
}

pub(super) fn early_revoke(
    proposal: &SorafsAuthorityProposalV1,
    certificate: &GovernanceCertificateV1,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let id = proposal.plan_id();
    let mut lease = tx
        .world
        .sorafs_authority_leases
        .get(&id)
        .cloned()
        .ok_or_else(|| invalid("early revocation has no exact lease"))?;
    finish(
        &mut lease,
        SorafsAuthorityApprovalV1::from_certificate(certificate),
        tx,
    )?;
    tx.world.sorafs_authority_leases.insert(id, lease);
    Ok(())
}

fn finish(
    lease: &mut SorafsAuthorityLeaseV1,
    approval: SorafsAuthorityApprovalV1,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let activation = lease
        .activation
        .as_ref()
        .ok_or_else(|| invalid("termination has no active grant"))?;
    if lease.completion.is_some() {
        return Err(invalid("lease already terminated"));
    }
    if owner(&tx.world)? != (activation.services_profile, activation.implementation) {
        return Err(invalid(
            "lease completion owner differs from its captured definition",
        ));
    }
    let subject = lease.plan.governed_subject_id_v1();
    let head = tx
        .world
        .sorafs_authority_heads
        .get(&subject)
        .ok_or_else(|| invalid("lease termination has no authority head"))?;
    if !head.granted || head.plan_id != lease.plan.plan_id() || head.revision != activation.revision
    {
        return Err(invalid(
            "lease termination does not own the active incarnation",
        ));
    }
    let revision = activation
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("authority revision overflow"))?;
    project(&lease.plan, &lease.plan.account, false, tx)?;
    let execution = SorafsAuthorityExecutionV1 {
        revision,
        at_height: tx.block_height(),
        approval,
        services_profile: activation.services_profile,
        implementation: activation.implementation,
    };
    write_head(&lease.plan, &execution, false, tx)?;
    lease.completion = Some(execution);
    Ok(())
}

fn write_head(
    plan: &SorafsAuthorityProposalV1,
    execution: &SorafsAuthorityExecutionV1,
    granted: bool,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let window = plan
        .grid
        .window(plan.term)
        .ok_or_else(|| invalid("invalid authority term"))?;
    tx.world.sorafs_authority_heads.insert(
        plan.governed_subject_id_v1(),
        SorafsAuthorityHeadV1 {
            plan_id: plan.plan_id(),
            term: plan.term,
            expires_at_height: window.end_height,
            authority: plan.authority,
            account: plan.account.clone(),
            revision: execution.revision,
            granted,
            proposal_content_id: execution.approval.proposal_content_id,
            governance_attempt_id: execution.approval.governance_attempt_id,
            certificate_id: execution.approval.certificate_id,
            enacted_at_height: execution.at_height,
        },
    );
    Ok(())
}

/// Execute terminal capabilities before starts; caller commits the complete transaction or none.
pub(super) fn advance(tx: &mut StateTransaction<'_, '_>) -> Result<(), InstructionExecutionError> {
    super::close_expired_proposals(tx)?;
    if !tx.world.sorafs_authority_leases.iter().any(|(_, lease)| {
        !lease.start_processed || (lease.activation.is_some() && lease.completion.is_none())
    }) {
        return Ok(());
    }
    require_active(&tx.world)?;
    let height = tx.block_height();
    loop {
        let due = tx
            .world
            .sorafs_authority_leases
            .iter()
            .find(|(_, lease)| {
                lease.activation.is_some()
                    && lease.completion.is_none()
                    && lease
                        .plan
                        .grid
                        .window(lease.plan.term)
                        .is_some_and(|w| w.end_height <= height)
            })
            .map(|(id, lease)| (*id, lease.clone()));
        let Some((id, mut lease)) = due else {
            break;
        };
        let approval = lease
            .terminal_approval
            .ok_or_else(|| invalid("active lease lost its terminal mandate"))?;
        finish(&mut lease, approval, tx)?;
        tx.world.sorafs_authority_leases.insert(id, lease);
    }
    loop {
        let due = tx
            .world
            .sorafs_authority_leases
            .iter()
            .find(|(_, lease)| {
                !lease.start_processed
                    && lease
                        .plan
                        .grid
                        .window(lease.plan.term)
                        .is_some_and(|w| w.start_height <= height)
            })
            .map(|(id, lease)| (*id, lease.clone()));
        let Some((id, mut lease)) = due else {
            break;
        };
        let window = lease
            .plan
            .grid
            .window(lease.plan.term)
            .ok_or_else(|| invalid("invalid authority term"))?;
        let ready = height == window.start_height
            && lease
                .grant_approval
                .as_ref()
                .is_some_and(|a| a.approved_at_height < window.cutoff_height)
            && lease
                .terminal_approval
                .as_ref()
                .is_some_and(|a| a.approved_at_height < window.cutoff_height)
            && !tx.world.sorafs_authority_leases.iter().any(|(_, other)| {
                other.plan.family_id() == lease.plan.family_id() && other.activation.is_some()
            })
            && validate_start(&lease.plan, tx).is_ok();
        if ready {
            let (services_profile, implementation) = owner(&tx.world)?;
            let approval = lease
                .grant_approval
                .ok_or_else(|| invalid("missing grant approval"))?;
            let execution = SorafsAuthorityExecutionV1 {
                revision: lease.plan.expected_revision + 1,
                at_height: height,
                approval,
                services_profile,
                implementation,
            };
            project(&lease.plan, &lease.plan.account, true, tx)?;
            write_head(&lease.plan, &execution, true, tx)?;
            lease.activation = Some(execution);
        }
        lease.start_processed = true;
        tx.world.sorafs_authority_leases.insert(id, lease);
    }
    Ok(())
}
