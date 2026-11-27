//! Domain-separated governance proposal fingerprints.
use iroha_crypto::blake2::{
    Blake2b,
    digest::{Digest, consts::U32},
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
/// Owner-consented contract lifecycle proposal domain.
pub const CONTRACT_LIFECYCLE_GOVERNANCE_V1: &[u8] =
    b"iroha.governance.proposal.contract_lifecycle.v1";
/// Emergency contract-containment proposal domain.
pub const CONTRACT_EMERGENCY_HOLD_V1: &[u8] =
    b"iroha.governance.proposal.contract_emergency_hold.v1";
/// Exact-account global data-trigger permission proposal domain.
pub const GLOBAL_DATA_TRIGGER_PERMISSION_GOVERNANCE_V1: &[u8] =
    b"iroha.governance.proposal.global_data_trigger_permission.v1";
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
/// Existing domain of a Parliament compare-and-set predecessor head.
pub const PARLIAMENT_EXPECTED_HEAD_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.expected_head.root.v1";
pub const GOVERNANCE_CERTIFICATE_ID_V1: &[u8] = b"iroha.governance.certificate.id.v1";
/// Attempt identifier of the anonymous Parliament definition family.
pub const ANONYMOUS_GOVERNANCE_ATTEMPT_ID_V1: &[u8] =
    b"iroha.governance.parliament.anonymous.attempt.id.v1";
/// Body-instance identifier of the anonymous Parliament definition family, over the 64-byte
/// roster root.
pub const ANONYMOUS_BODY_INSTANCE_ID_V1: &[u8] =
    b"iroha.governance.parliament.anonymous.body_instance.id.v1";
/// Digest of an exact transition of the anonymous Parliament definition family.
pub const ANONYMOUS_PARLIAMENT_TRANSITION_DIGEST_V1: &[u8] =
    b"iroha.governance.parliament.anonymous.transition.digest.v1";
/// Certificate identifier of the anonymous Parliament definition family.
pub const ANONYMOUS_GOVERNANCE_CERTIFICATE_ID_V1: &[u8] =
    b"iroha.governance.parliament.anonymous.certificate.id.v1";
/// Execution-failure root of the anonymous Parliament definition family.
pub const ANONYMOUS_PARLIAMENT_EXECUTION_FAILURE_ROOT_V1: &[u8] =
    b"iroha.governance.parliament.anonymous.execution_failure.root.v1";
pub fn fingerprint(domain: &[u8], proposal: &impl Encode) -> [u8; 32] {
    let encoded = proposal.encode();
    let domain_len = u64::try_from(domain.len())
        .expect("protocol-defined digest domains fit in u64")
        .to_le_bytes();
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(domain_len);
    hasher.update(domain);
    hasher.update(&encoded);
    hasher.finalize().into()
}

/// Stream the existing fingerprint preimage while retaining the original codec refusal.
///
/// The domain length, domain and bare Norito payload are exactly those of
/// [`fingerprint`]. This checked entrypoint does not allocate an output frame or change
/// the installed fingerprint function.
///
/// # Errors
/// The original bare payload encoder error, including its local resource classification.
pub fn checked_fingerprint(
    domain: &[u8],
    proposal: &impl Encode,
) -> Result<[u8; 32], norito::core::DecodeAttemptError> {
    struct DigestWriter(Blake2b<U32>);

    impl std::io::Write for DigestWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    norito::core::classify_decode_attempt(|| {
        let domain_len = u64::try_from(domain.len())
            .map_err(|_| norito::Error::LengthMismatch)?
            .to_le_bytes();
        let mut writer = DigestWriter(Blake2b::<U32>::new());
        writer.0.update(domain_len);
        writer.0.update(domain);
        norito::codec::encode_adaptive_into(proposal, &mut writer)?;
        Ok(writer.0.finalize().into())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn checked_fingerprint_preserves_the_exact_bare_payload_preimage() {
        #[derive(Encode)]
        struct Payload {
            label: String,
            values: Vec<Vec<u64>>,
        }
        for value in [
            Payload {
                label: String::new(),
                values: Vec::new(),
            },
            Payload {
                label: "runtime proposal".into(),
                values: vec![Vec::new(), vec![0, 127, 256, u64::MAX]],
            },
        ] {
            let bare = value.encode();
            for domain in [
                b"".as_slice(),
                RUNTIME_UPGRADE_V1,
                GOVERNANCE_EFFECT_PREIMAGE_V1,
            ] {
                let mut original = Blake2b::<U32>::new();
                original.update(u64::try_from(domain.len()).unwrap().to_le_bytes());
                original.update(domain);
                original.update(&bare);
                let original: [u8; 32] = original.finalize().into();
                assert_eq!(checked_fingerprint(domain, &value).unwrap(), original);
                assert_eq!(fingerprint(domain, &value), original);
            }
        }
    }

    #[test]
    fn checked_fingerprint_preserves_a_serializer_error() {
        struct RefusedPayload;
        impl norito::core::SerializePayload for RefusedPayload {
            fn serialize(&self, _: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
                Err(norito::Error::NonCanonicalEncoding)
            }
        }
        let error = checked_fingerprint(RUNTIME_UPGRADE_V1, &RefusedPayload).unwrap_err();
        assert!(error.into_local_refusal().is_err());
        let error = checked_fingerprint(RUNTIME_UPGRADE_V1, &RefusedPayload).unwrap_err();
        assert!(matches!(error.error(), norito::Error::NonCanonicalEncoding));
    }

    #[test]
    fn checked_fingerprint_retains_original_serializer_scratch_refusal() {
        // Norito sorts the actual map entries in admitted scratch before encoding them.
        // Streaming the fingerprint must not hide that serializer-owned allocation.
        let value = std::collections::HashMap::from([(3_u64, 7_u64), (1, 9)]);
        let limits =
            |bytes| norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, 64);
        let (result, usage) = norito::core::with_decode_limits_measured(limits(usize::MAX), || {
            checked_fingerprint(RUNTIME_UPGRADE_V1, &value)
        });
        let expected = result.unwrap();
        assert_eq!(expected, fingerprint(RUNTIME_UPGRADE_V1, &value));
        let required = usage.total_allocated_bytes();
        assert!(
            required > 0,
            "the native map serializer retains ordering scratch"
        );
        assert_eq!(
            norito::core::with_decode_limits_scope(limits(required), || {
                checked_fingerprint(RUNTIME_UPGRADE_V1, &value)
            })
            .unwrap(),
            expected
        );
        let error = norito::core::with_decode_limits_scope(limits(required - 1), || {
            checked_fingerprint(RUNTIME_UPGRADE_V1, &value)
        })
        .unwrap_err();
        let original = error.into_local_refusal().unwrap();
        assert_eq!(
            original.kind(),
            norito::core::DecodeLocalRefusalKind::EnclosingLimit
        );
    }

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
            CONTRACT_LIFECYCLE_GOVERNANCE_V1,
            CONTRACT_EMERGENCY_HOLD_V1,
            GLOBAL_DATA_TRIGGER_PERMISSION_GOVERNANCE_V1,
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
            PARLIAMENT_EXPECTED_HEAD_ROOT_V1,
            GOVERNANCE_CERTIFICATE_ID_V1,
            ANONYMOUS_GOVERNANCE_ATTEMPT_ID_V1,
            ANONYMOUS_BODY_INSTANCE_ID_V1,
            ANONYMOUS_PARLIAMENT_TRANSITION_DIGEST_V1,
            ANONYMOUS_GOVERNANCE_CERTIFICATE_ID_V1,
            ANONYMOUS_PARLIAMENT_EXECUTION_FAILURE_ROOT_V1,
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
