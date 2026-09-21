//! Constant-row custody history reconstruction and generation-index validation.
use super::*;

pub(crate) fn control_digest<P: CustodyPurpose>(
    record: &P::Record,
) -> Result<[u8; 32], HistoryError> {
    digest(P::RECORD_DOMAIN, record)
}
fn validate_keys<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    record: &P::Record,
    state: &SignerCustodyControlStateV1,
) -> Result<(), HistoryError> {
    let fields = P::record_view(record);
    for (signer, key, generation) in [
        (
            true,
            &state.policy.binding.public_key,
            state.policy.binding.key_revision,
        ),
        (
            false,
            &state.policy.attester_public_key,
            state.policy.attester_authority.key_revision,
        ),
    ] {
        let first: ControlIndexV1 = decode(
            world
                .smart_contract_state()
                .get(&key_path::<P>(fields.deployment, signer, key)?)
                .ok_or(HistoryError::CorruptHistory)?,
        )
        .map_err(|_| HistoryError::CorruptHistory)?;
        if first.revision == 0 || first.revision > fields.revision {
            return Err(HistoryError::CorruptHistory);
        }
        // Validate the referenced first-use row with the same bounded local rules as
        // selected history, without recursively following its own key references.
        let original = read_record_value::<P>(world, fields.deployment, first.revision)?;
        let policy = &original.state;
        let (first_key, first_generation) = if signer {
            (
                &policy.policy.binding.public_key,
                policy.policy.binding.key_revision,
            )
        } else {
            (
                &policy.policy.attester_public_key,
                policy.policy.attester_authority.key_revision,
            )
        };
        if original.index != first
            || policy.policy.binding.chain_id != state.policy.binding.chain_id
            || policy.policy.binding.network_id != state.policy.binding.network_id
            || first_key != key
            || first_generation != generation
        {
            return Err(HistoryError::CorruptHistory);
        }
    }
    Ok(())
}
pub(crate) fn read_control_record<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    deployment: &str,
    revision: u64,
) -> Result<NativeControl<P>, HistoryError> {
    let control = read_record_value::<P>(world, deployment, revision)?;
    validate_keys::<P>(world, &control.record, &control.state)?;
    Ok(control)
}

// This bounded local validator deliberately does not follow key references. Both selected
// rows and their first-use rows must satisfy the same record, enrollment and height index rules.
fn read_record_value<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    deployment: &str,
    revision: u64,
) -> Result<NativeControl<P>, HistoryError> {
    let record: P::Record = decode(
        world
            .smart_contract_state()
            .get(&control_record_key::<P>(deployment, revision)?)
            .ok_or(HistoryError::CorruptHistory)?,
    )
    .map_err(|_| HistoryError::CorruptHistory)?;
    let fields = P::record_view(&record);
    let execution = fields.execution.view();
    let state: SignerCustodyControlStateV1 =
        decode(fields.control_state).map_err(|_| HistoryError::CorruptHistory)?;
    state.validate().map_err(|_| HistoryError::CorruptHistory)?;
    if fields.deployment != deployment
        || fields.revision != revision
        || revision == 0
        || revision > P::MAX_REVISIONS
        || (revision == 1) != (fields.predecessor_digest == [0; 32])
        || fields.request_digest == [0; 32]
        || !valid_execution(fields.execution)
        || state.policy.binding.role != P::ROLE
        || state.policy.binding.purpose != P::purpose(deployment.to_owned())
        || state.active_head.is_some() != fields.enrollment.is_some()
        || state
            .active_head
            .is_some_and(|head| head.approved_anchor.height >= execution.height)
        || encode(&state)?.as_slice() != fields.control_state
    {
        return Err(HistoryError::CorruptHistory);
    }
    if let Some(enrollment) = fields.enrollment {
        use sorafs_manifest::signer::custody::{
            SignerCustodyEnrollmentContextV1, verify_signer_custody_enrollment_v1,
        };
        let enrolled: sorafs_manifest::signer::custody::SignerCustodyRecordV1 =
            decode(enrollment).map_err(|_| HistoryError::CorruptHistory)?;
        let head = state.active_head.ok_or(HistoryError::CorruptHistory)?;
        if enrolled.statement.binding != state.policy.binding
            || enrolled.statement.sequence != head.sequence
            || enrolled.statement.anchor != head.approved_anchor
            || encode(&enrolled)?.as_slice() != enrollment
        {
            return Err(HistoryError::CorruptHistory);
        }
        // Authenticate retained history at its original issuance, including the exact signed
        // frame digest. Live use separately checks current native time and revocation state.
        let verified = verify_signer_custody_enrollment_v1(
            enrollment,
            &state.policy.binding,
            &state.policy.custody_trust(),
            &SignerCustodyEnrollmentContextV1 {
                now_unix_ms: enrolled.statement.issued_at_unix_ms,
                anchor_observed_at_unix_ms: enrolled.statement.issued_at_unix_ms,
                current_anchor: head.approved_anchor,
                next_sequence: head.sequence,
                predecessor_digest: enrolled.statement.predecessor_digest,
                signer_revoked: false,
                attester_revoked: false,
            },
        )
        .map_err(|_| HistoryError::CorruptHistory)?;
        if verified.record_digest() != head.record_digest {
            return Err(HistoryError::CorruptHistory);
        }
    }
    let index = ControlIndexV1 {
        revision,
        digest: control_digest::<P>(&record)?,
        height: execution.height,
        ordinal: execution.ordinal,
    };
    let indexed: ControlIndexV1 = decode(
        world
            .smart_contract_state()
            .get(&control_height_key::<P>(
                deployment,
                index.height,
                index.ordinal,
            )?)
            .ok_or(HistoryError::CorruptHistory)?,
    )
    .map_err(|_| HistoryError::CorruptHistory)?;
    if indexed != index {
        return Err(HistoryError::CorruptHistory);
    }
    Ok(NativeControl {
        record,
        state,
        index,
    })
}
fn adjacent<P: CustodyPurpose>(
    previous: &NativeControl<P>,
    next: &NativeControl<P>,
) -> Result<(), HistoryError> {
    adjacent_execution(
        P::record_view(&previous.record).execution,
        P::record_view(&next.record).execution,
    )?;
    let old = &previous.state;
    let new = &next.state;
    if previous.index.revision.checked_add(1) != Some(next.index.revision)
        || P::record_view(&next.record).predecessor_digest != previous.index.digest
        || new.policy.binding.key_revision < old.policy.binding.key_revision
        || new.policy.binding.policy_revision < old.policy.binding.policy_revision
        || new.policy.attester_authority.key_revision < old.policy.attester_authority.key_revision
        || new.policy.attester_authority.policy_revision
            < old.policy.attester_authority.policy_revision
        || new.next_sequence < old.next_sequence
        || new.next_sequence
            > old
                .next_sequence
                .checked_add(1)
                .ok_or(HistoryError::CorruptHistory)?
        || (old.signer_revoked
            && !new.signer_revoked
            && new.policy.binding.key_revision == old.policy.binding.key_revision)
        || (old.attester_revoked
            && !new.attester_revoked
            && new.policy.attester_authority.key_revision
                == old.policy.attester_authority.key_revision)
    {
        return Err(HistoryError::CorruptHistory);
    }
    Ok(())
}
pub(crate) fn read_control<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    deployment: &str,
) -> Result<Option<NativeControl<P>>, HistoryError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&control_head_key::<P>(deployment)?)
    else {
        if prefix_has_any::<P>(world, deployment, "control_revision_")?
            || prefix_has_any::<P>(world, deployment, "control_height_")?
            || prefix_has_any::<P>(world, deployment, "signer_key_")?
            || prefix_has_any::<P>(world, deployment, "attester_key_")?
        {
            return Err(HistoryError::CorruptHistory);
        }
        return Ok(None);
    };
    let head: ControlIndexV1 = decode(bytes).map_err(|_| HistoryError::CorruptHistory)?;
    let active = read_control_record::<P>(world, deployment, head.revision)?;
    if head != active.index {
        return Err(HistoryError::CorruptHistory);
    }
    if head.revision > 1 {
        adjacent::<P>(
            &read_control_record::<P>(world, deployment, head.revision - 1)?,
            &active,
        )?;
    } else if head.ordinal != 0 {
        return Err(HistoryError::CorruptHistory);
    }
    let latest_revision = world
        .smart_contract_state()
        .range(
            control_record_key::<P>(deployment, 0)?
                ..=control_record_key::<P>(deployment, u64::MAX)?,
        )
        .next_back();
    let latest_height = world
        .smart_contract_state()
        .range(
            control_height_key::<P>(deployment, 0, 0)?
                ..=control_height_key::<P>(deployment, u64::MAX, u32::MAX)?,
        )
        .next_back();
    if latest_revision.map(|(key, _)| key)
        != Some(&control_record_key::<P>(deployment, head.revision)?)
        || latest_height.map(|(key, _)| key)
            != Some(&control_height_key::<P>(
                deployment,
                head.height,
                head.ordinal,
            )?)
    {
        return Err(HistoryError::CorruptHistory);
    }
    Ok(Some(active))
}
pub(crate) fn read_control_at<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    deployment: &str,
    height: u64,
) -> Result<Option<NativeControl<P>>, HistoryError> {
    let Some(active) = read_control::<P>(world, deployment)? else {
        return Ok(None);
    };
    let entry = world
        .smart_contract_state()
        .range(
            control_height_key::<P>(deployment, 0, 0)?
                ..=control_height_key::<P>(deployment, height, u32::MAX)?,
        )
        .next_back();
    let Some((key, bytes)) = entry else {
        if P::record_view(&read_control_record::<P>(world, deployment, 1)?.record)
            .execution
            .view()
            .height
            <= height
        {
            return Err(HistoryError::CorruptHistory);
        }
        return Ok(None);
    };
    let index: ControlIndexV1 = decode(bytes).map_err(|_| HistoryError::CorruptHistory)?;
    if key != &control_height_key::<P>(deployment, index.height, index.ordinal)?
        || index.height > height
        || index.revision > active.index.revision
    {
        return Err(HistoryError::CorruptHistory);
    }
    let selected = read_control_record::<P>(world, deployment, index.revision)?;
    if selected.index != index {
        return Err(HistoryError::CorruptHistory);
    }
    if index.revision > 1 {
        adjacent::<P>(
            &read_control_record::<P>(world, deployment, index.revision - 1)?,
            &selected,
        )?;
    }
    if index.revision < active.index.revision {
        let next = read_control_record::<P>(world, deployment, index.revision + 1)?;
        adjacent::<P>(&selected, &next)?;
        if next.index.height <= height {
            return Err(HistoryError::CorruptHistory);
        }
    }
    Ok(Some(selected))
}
