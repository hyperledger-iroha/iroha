//! Domain-separated governance proposal fingerprints.
use iroha_crypto::blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use norito::codec::Encode;
pub const DEPLOY_CONTRACT_V1: &[u8] = b"iroha.governance.proposal.deploy_contract.v1";
pub const RUNTIME_UPGRADE_V1: &[u8] = b"iroha.governance.proposal.runtime_upgrade.v1";
pub const SCCP_ROUTE_GOVERNANCE_V1: &[u8] = b"iroha.governance.proposal.sccp_route_governance.v1";
pub const VALIDATION_FEE_POLICY_V1: &[u8] = b"iroha.governance.proposal.validation_fee_policy.v1";
pub const VALIDATION_FEE_PAYOUT_LIFECYCLE_V1: &[u8] =
    b"iroha.governance.proposal.validation_fee_payout_lifecycle.v1";
pub const MUSUBI_REGISTRY_GOVERNANCE_V1: &[u8] =
    b"iroha.governance.proposal.musubi_registry_governance.v1";
pub const SORAFS_PROVIDER_GOVERNANCE_V1: &[u8] =
    b"iroha.governance.proposal.sorafs_provider_governance.v1";
pub const GOVERNANCE_EFFECT_PREIMAGE_V1: &[u8] = b"iroha.governance.effect_preimage.v1";
pub const GOVERNANCE_SUBJECT_ID_V1: &[u8] = b"iroha.governance.subject.id.v1";
pub const GOVERNANCE_ATTEMPT_ID_V1: &[u8] = b"iroha.governance.attempt.id.v1";
pub const BODY_ELECTION_ATTEMPT_ID_V1: &[u8] =
    b"iroha.governance.parliament.body_election_attempt.id.v1";
pub const SORTITION_REQUEST_ID_V1: &[u8] = b"iroha.governance.parliament.sortition_request.id.v1";
pub const LOGICAL_BEACON_SESSION_ID_V1: &[u8] =
    b"iroha.governance.parliament.logical_beacon_session.id.v1";
pub const CANDIDATE_ROOT_V1: &[u8] = b"iroha.governance.parliament.candidate.root.v1";
pub const BODY_INSTANCE_ID_V1: &[u8] = b"iroha.governance.parliament.body_instance.id.v1";
pub const ASSIGNMENT_ID_V1: &[u8] = b"iroha.governance.parliament.assignment.id.v1";
pub const ASSIGNMENT_PLAN_ROOT_V1: &[u8] = b"iroha.governance.parliament.assignment_plan.root.v1";
pub const ROSTER_ROOT_V1: &[u8] = b"iroha.governance.parliament.roster.root.v1";
pub const BALLOT_ATTEMPT_ID_V1: &[u8] = b"iroha.governance.parliament.ballot_attempt.id.v1";
pub const BALLOT_PARTICIPANT_HASH_V1: &[u8] =
    b"iroha.governance.parliament.ballot_participant.hash.v1";
pub const TLE_SESSION_ID_V1: &[u8] = b"iroha.governance.parliament.tle_session.id.v1";
pub const PARLIAMENT_BALLOT_FAILURE_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.ballot_failure.root.v1";
pub const PARLIAMENT_BALLOT_RESULT_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.ballot_result.root.v1";
pub const PARLIAMENT_PUBLIC_FINDING_ENDORSEMENT_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.public_finding_endorsement.root.v1";
pub const PARLIAMENT_EXECUTION_FAILURE_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.execution_failure.root.v1";
pub const GOVERNANCE_CERTIFICATE_ID_V1: &[u8] = b"iroha.governance.certificate.id.v1";
pub fn fingerprint(domain: &[u8], proposal: &impl Encode) -> [u8; 32] {
    let encoded = proposal.encode();
    let domain_len = u64::try_from(domain.len())
        .expect("protocol-defined digest domains fit in u64")
        .to_le_bytes();
    let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length is fixed and valid");
    hasher.update(&domain_len);
    hasher.update(domain);
    hasher.update(&encoded);
    let mut fingerprint = [0_u8; 32];
    hasher
        .finalize_variable(&mut fingerprint)
        .expect("fingerprint output has the configured Blake2b length");
    fingerprint
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn proposal_domains_are_unique() {
        let domains = [
            DEPLOY_CONTRACT_V1,
            RUNTIME_UPGRADE_V1,
            SCCP_ROUTE_GOVERNANCE_V1,
            VALIDATION_FEE_POLICY_V1,
            VALIDATION_FEE_PAYOUT_LIFECYCLE_V1,
            MUSUBI_REGISTRY_GOVERNANCE_V1,
            SORAFS_PROVIDER_GOVERNANCE_V1,
            GOVERNANCE_EFFECT_PREIMAGE_V1,
            GOVERNANCE_SUBJECT_ID_V1,
            GOVERNANCE_ATTEMPT_ID_V1,
            BODY_ELECTION_ATTEMPT_ID_V1,
            SORTITION_REQUEST_ID_V1,
            LOGICAL_BEACON_SESSION_ID_V1,
            CANDIDATE_ROOT_V1,
            BODY_INSTANCE_ID_V1,
            ASSIGNMENT_ID_V1,
            ASSIGNMENT_PLAN_ROOT_V1,
            ROSTER_ROOT_V1,
            BALLOT_ATTEMPT_ID_V1,
            BALLOT_PARTICIPANT_HASH_V1,
            TLE_SESSION_ID_V1,
            PARLIAMENT_BALLOT_FAILURE_ROOT_V1,
            PARLIAMENT_BALLOT_RESULT_ROOT_V1,
            PARLIAMENT_PUBLIC_FINDING_ENDORSEMENT_ROOT_V1,
            PARLIAMENT_EXECUTION_FAILURE_ROOT_V1,
            GOVERNANCE_CERTIFICATE_ID_V1,
        ];
        assert_eq!(
            domains.into_iter().collect::<BTreeSet<_>>().len(),
            domains.len()
        );
    }
    #[test]
    fn fingerprint_binds_the_proposal_domain() {
        let proposal = 7_u32;
        assert_ne!(
            fingerprint(DEPLOY_CONTRACT_V1, &proposal),
            fingerprint(RUNTIME_UPGRADE_V1, &proposal)
        );
    }
}
