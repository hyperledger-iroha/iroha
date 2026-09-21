//! Deterministic claimed execution history for reducer tests, never native authorization evidence.
use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use sorafs_manifest::signer::{
    custody::*, custody_control::*, protocol::*, topology::subject::prepare_topology_approval_v1,
};
pub(super) struct Fixture {
    pub(super) model: TopologyTransitionModelV1,
    pub(super) policy: SignerCustodyPolicyV1,
    pub(super) attester: KeyPair,
    pub(super) frames: Vec<Vec<u8>>,
}
pub(super) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}
pub(super) fn actor(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}
fn policy() -> SignerCustodyPolicyV1 {
    SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: "topology-chain".into(),
            network_id: [1; 32],
            runtime_handle: "software://sorafs/topology-approval/primary".into(),
            key_handle: "software://sorafs/topology-approval/key-1".into(),
            service_id: "topology-primary".into(),
            administrator_id: "topology-security".into(),
            role: SignerRoleV1::TopologyApproval,
            purpose: SignerPurposeBindingV1::TopologyApproval {
                deployment_id: "production-primary".into(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key(21).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [2; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-primary".into(),
            administrator_id: "custody-security".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [3; 32],
        },
        attester_public_key: key(22).public_key().clone(),
        active_from_unix_ms: 90_000,
        active_until_unix_ms: 400_000,
        max_validity_ms: 300_000,
        max_anchor_age_ms: 10_000,
    }
}
impl Fixture {
    pub(super) fn new() -> Self {
        let mut fixture = Self {
            model: TopologyTransitionModelV1::new(
                "production-primary".into(),
                [1; 32],
                "topology-chain".into(),
                369,
            )
            .unwrap(),
            policy: policy(),
            attester: key(22),
            frames: Vec::new(),
        };
        fixture
            .apply(
                TopologyActionV1::Configure(norito::encode_canonical(&fixture.policy).unwrap()),
                1,
                100_000,
                31,
            )
            .unwrap();
        let enrollment = fixture.enrollment(2, 110_000);
        fixture
            .apply(TopologyActionV1::Enroll(enrollment), 2, 110_000, 31)
            .unwrap();
        fixture
    }
    pub(super) fn context(&self, height: u64, time: u64, authority: u8) -> TopologyContextClaimV1 {
        TopologyContextClaimV1 {
            execution: TopologyExecutionClaimV1 {
                height,
                ordinal: 0,
                recorded_at_unix_ms: time,
                authority: actor(authority),
            },
            custody_anchor: self.model.control().map(|_| SignerCustodyAnchorV1 {
                height: height - 1,
                block_hash: [height as u8; 32],
                state_digest: self.model.control_head().digest,
            }),
            floor: None,
        }
    }
    pub(super) fn transition(&self, action: TopologyActionV1) -> TopologyTransitionV1 {
        TopologyTransitionV1 {
            deployment_id: "production-primary".into(),
            control: self.model.control_head(),
            operations: self.model.operation_head(),
            action,
        }
    }
    pub(super) fn apply(
        &mut self,
        action: TopologyActionV1,
        height: u64,
        time: u64,
        authority: u8,
    ) -> Result<TopologyTransitionDeltaV1, Error> {
        let transition = self.transition(action);
        let context = self.context(height, time, authority);
        let delta = self.model.apply_claimed(&transition, &context)?;
        if let Some(entry) = &delta.history {
            self.frames.push(norito::encode_canonical(entry).unwrap());
        }
        Ok(delta)
    }
    pub(super) fn enrollment(&self, height: u64, time: u64) -> Vec<u8> {
        let state = self.model.state.as_ref().unwrap();
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: 1,
            binding: state.policy.binding.clone(),
            authority: state.policy.attester_authority.clone(),
            anchor: self.context(height, time, 31).custody_anchor.unwrap(),
            sequence: state.next_sequence,
            predecessor_digest: state.predecessor_digest,
            issued_at_unix_ms: time,
            expires_at_unix_ms: 300_000,
            evidence_digest: [4; 32],
            revoked: false,
        };
        let attestation = Signature::new(
            self.attester.private_key(),
            &statement.signing_payload().unwrap(),
        )
        .payload()
        .try_into()
        .unwrap();
        norito::encode_canonical(&SignerCustodyRecordV1 {
            statement,
            attestation,
        })
        .unwrap()
    }
    pub(super) fn reviewed(&self, id: u8, height: u64, time: u64) -> TopologyReserveV1 {
        let context = self.context(height, time, 32);
        let custody = self.model.view().use_current(&context).unwrap();
        let subject = TopologyApprovalSubjectV1 {
            deployment_id: "production-primary".into(),
            network_id: [1; 32],
            chain_id: "topology-chain".into(),
            chain_discriminant: 369,
            release_manifest_sha256: [5; 32],
            qualification_summary_sha256: [6; 32],
            manifest_sha256: [7; 32],
            canonical_manifest_sha256: [8; 32],
            validator_ids_sha256: [9; 32],
            reviewed_at_unix_ms: 115_000,
            expires_at_unix_ms: 250_000,
        };
        let prepared =
            prepare_topology_approval_v1(&subject, &custody.statement().binding).unwrap();
        let request = SignerTopologyRequestV1::new(&custody, [id; 32], &prepared).unwrap();
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: [id; 32],
            request_digest: request.digest().unwrap(),
            previous_audit: self.model.audit(),
        };
        TopologyReserveV1 {
            subject,
            request,
            intent,
        }
    }
    pub(super) fn reserve(&mut self, id: u8, height: u64, time: u64) -> TopologyOperationRecordV1 {
        let request = self.reviewed(id, height, time);
        self.apply(
            TopologyActionV1::Reserve(Box::new(request)),
            height,
            time,
            32,
        )
        .unwrap()
        .operation
        .unwrap()
    }
    pub(super) fn completion(&self, row: &TopologyOperationRecordV1) -> TopologyCompleteV1 {
        TopologyCompleteV1 {
            request: row.reviewed.request,
            intent: row.reviewed.intent,
            reservation: row.reservation,
            commitment: SignerOperationCommitmentV1 {
                audit: SignerOperationAuditHeadV1 {
                    sequence: self.model.audit().sequence + 1,
                    digest: [15; 32],
                },
                response_digest: [16; 32],
            },
            signatures_digest: [17; 32],
        }
    }
    pub(super) fn restore(&self, frames: Vec<Vec<u8>>) -> Result<TopologyTransitionModelV1, Error> {
        TopologyTransitionModelV1::restore_claimed(
            "production-primary".into(),
            [1; 32],
            "topology-chain".into(),
            369,
            self.model.retained(),
            frames,
        )
    }
}
