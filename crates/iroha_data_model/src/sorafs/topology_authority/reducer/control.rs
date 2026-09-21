//! Topology control planning reuses the sole Manifest policy and enrollment validators.
use super::*;
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyEnrollmentContextV1,
        verify_signer_custody_enrollment_v1,
    },
    custody_control::{
        SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1, SignerCustodyPolicyTransitionErrorV1,
        SignerCustodyPolicyV1, configure_signer_custody_policy_v1,
    },
};
pub(super) struct PreparedControl {
    pub(super) state: SignerCustodyControlStateV1,
    pub(super) record: TopologyControlRecordV1,
    pub(super) signer_key: Option<[u8; 32]>,
    pub(super) attester_key: Option<[u8; 32]>,
}

pub(super) fn prepare<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    transition: &TopologyTransitionV1,
    context: &TopologyContextClaimV1,
    request_digest: [u8; 32],
) -> Result<Option<PreparedControl>, TopologyPreparationErrorV1<L::Error>> {
    if !matches!(
        transition.action,
        TopologyActionV1::Configure(_)
            | TopologyActionV1::Enroll(_)
            | TopologyActionV1::Revoke { .. }
    ) {
        return Ok(None);
    }
    let revision = model
        .root
        .control_head
        .revision
        .checked_add(1)
        .ok_or(Error::Capacity)?;
    if revision > TOPOLOGY_CONTROL_LIMIT_V1
        || (revision > TOPOLOGY_CONTROL_NORMAL_LIMIT_V1
            && !matches!(transition.action, TopologyActionV1::Revoke { .. }))
    {
        return Err(Error::Capacity.into());
    }
    let mut keys = (None, None);
    let (state, enrollment) = match &transition.action {
        TopologyActionV1::Configure(bytes) => {
            let policy: SignerCustodyPolicyV1 = decode(bytes, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)?;
            model.validate_binding(&policy.binding)?;
            let state = configure_signer_custody_policy_v1(model.state, policy).map_err(
                |error| match error {
                    SignerCustodyPolicyTransitionErrorV1::Invalid => Error::Invalid,
                    SignerCustodyPolicyTransitionErrorV1::Unchanged => Error::Conflict,
                    SignerCustodyPolicyTransitionErrorV1::BindingMismatch => Error::Binding,
                    SignerCustodyPolicyTransitionErrorV1::Generation => Error::Generation,
                },
            )?;
            keys = new_keys(model, &state.policy)?;
            (state, None)
        }
        TopologyActionV1::Enroll(bytes) => enroll(model, bytes, context)?,
        TopologyActionV1::Revoke { signer, attester } => {
            let mut next = model.state.cloned().ok_or(Error::Conflict)?;
            if !(*signer && !next.signer_revoked || *attester && !next.attester_revoked) {
                return Err(Error::Conflict.into());
            }
            next.signer_revoked |= signer;
            next.attester_revoked |= attester;
            (next, model.control.and_then(|row| row.enrollment.clone()))
        }
        _ => return Err(Error::Invalid.into()),
    };
    state.validate().map_err(|_| Error::Custody)?;
    model.validate_binding(&state.policy.binding)?;
    let record = TopologyControlRecordV1 {
        deployment_id: model.deployment.to_owned(),
        revision,
        predecessor_digest: model.root.control_head.digest,
        request_digest,
        execution: context.execution.clone(),
        control_state: encode(&state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)?,
        enrollment,
    };
    encode(&record, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
    Ok(Some(PreparedControl {
        state,
        record,
        signer_key: keys.0,
        attester_key: keys.1,
    }))
}
fn new_keys<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    policy: &SignerCustodyPolicyV1,
) -> Result<(Option<[u8; 32]>, Option<[u8; 32]>), TopologyPreparationErrorV1<L::Error>> {
    let current = model.state.map(|state| &state.policy);
    let mut result = [None, None];
    for (index, key, previous, count) in [
        (
            0,
            &policy.binding.public_key,
            current.map(|old| &old.binding.public_key),
            model.root.signer_key_count,
        ),
        (
            1,
            &policy.attester_public_key,
            current.map(|old| &old.attester_public_key),
            model.root.attester_key_count,
        ),
    ] {
        let key_digest = digest(b"iroha.sorafs.topology.key-tombstone.v1\0", key)?;
        let seen = if index == 0 {
            model.index.signer_key_seen(&key_digest)
        } else {
            model.index.attester_key_seen(&key_digest)
        }
        .map_err(TopologyPreparationErrorV1::Lookup)?;
        if previous == Some(key) {
            if !seen {
                return Err(Error::History.into());
            }
        } else {
            if seen {
                return Err(Error::Generation.into());
            }
            if count >= TOPOLOGY_CONTROL_NORMAL_LIMIT_V1 {
                return Err(Error::Capacity.into());
            }
            result[index] = Some(key_digest);
        }
    }
    Ok((result[0], result[1]))
}
fn enroll<L: TopologyIndexedReadV1 + ?Sized>(
    model: &TopologyStateViewV1<'_, L>,
    bytes: &[u8],
    context: &TopologyContextClaimV1,
) -> Result<(SignerCustodyControlStateV1, Option<Vec<u8>>), TopologyPreparationErrorV1<L::Error>> {
    let state = model.state.ok_or(Error::Custody)?;
    let anchor = model.anchor(context)?;
    let verified = verify_signer_custody_enrollment_v1(
        bytes,
        &state.policy.binding,
        &state.policy.custody_trust(),
        &SignerCustodyEnrollmentContextV1 {
            now_unix_ms: context.execution.recorded_at_unix_ms,
            anchor_observed_at_unix_ms: context.execution.recorded_at_unix_ms,
            current_anchor: anchor,
            next_sequence: state.next_sequence,
            predecessor_digest: state.predecessor_digest,
            signer_revoked: state.signer_revoked,
            attester_revoked: state.attester_revoked,
        },
    )
    .map_err(|_| Error::Custody)?;
    let mut next = state.clone();
    next.next_sequence = next.next_sequence.checked_add(1).ok_or(Error::Capacity)?;
    next.predecessor_digest = verified.record_digest();
    next.active_head = Some(SignerCustodyActiveHeadV1 {
        record_digest: verified.record_digest(),
        sequence: verified.statement().sequence,
        approved_anchor: anchor,
        key_revision: state.policy.binding.key_revision,
        policy_revision: state.policy.binding.policy_revision,
        policy_digest: state.policy.binding.policy_digest,
    });
    Ok((next, Some(bytes.to_vec())))
}
