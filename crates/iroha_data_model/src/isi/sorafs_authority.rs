//! Closed Parliament proposals for nondelegable `SoraFS` operating authorities.

use crate::{
    NetworkId,
    account::AccountId,
    governance::types::{GovernanceAttemptId, ProposalContentId},
    runtime::lifecycle::RootScopeV1,
    sorafs::pin_registry::ProviderIngestCompletionAuthorityV1,
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Fixed number of native consensus epochs in one authority term.
pub const SORAFS_AUTHORITY_TERM_EPOCHS_V1: u64 = 64;
/// Aggregate fresh-randomness allowance for each decision role in one term family.
pub const SORAFS_AUTHORITY_ROLE_RANDOMNESS_UNITS_V1: u32 = 17;

/// Canonical fixed-term grid bound to the root's committed native epoch cadence.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityTermGridV1 {
    /// Exact committed native epoch length; Core rejects caller-substituted cadence.
    #[norito(json = "crate::json_helpers::u64_string")]
    pub epoch_length_blocks: u64,
}

/// Checked committed-height interval and admission boundaries of one authority term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SorafsAuthorityTermWindowV1 {
    /// Inclusive first height at which the term may hold authority.
    pub start_height: u64,
    /// Exclusive authority height; its retained terminal mandate is due here.
    pub end_height: u64,
    /// Exclusive approval cutoff, one native epoch before the start.
    pub cutoff_height: u64,
    /// Inclusive admission opening, exactly one term before the start.
    pub admission_height: u64,
}

impl SorafsAuthorityTermGridV1 {
    /// Derive a nonzero term on the height-one anchored grid using checked arithmetic.
    /// Returns `None` for term zero, an invalid native cadence, or height overflow.
    #[must_use]
    pub fn window(self, term: u64) -> Option<SorafsAuthorityTermWindowV1> {
        if term == 0 || self.epoch_length_blocks < 3 {
            return None;
        }
        let length = self
            .epoch_length_blocks
            .checked_mul(SORAFS_AUTHORITY_TERM_EPOCHS_V1)?;
        let start_height = term.checked_mul(length)?.checked_add(1)?;
        Some(SorafsAuthorityTermWindowV1 {
            start_height,
            end_height: start_height.checked_add(length)?,
            cutoff_height: start_height.checked_sub(self.epoch_length_blocks)?,
            admission_height: start_height.checked_sub(length)?,
        })
    }
}

/// The four operating authorities governed by this release addition.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::sorafs_authority::SorafsOperatingAuthorityV1")]
#[norito(tag = "kind", content = "scope", deny_unknown_fields)]
pub enum SorafsOperatingAuthorityV1 {
    /// Complete assignments for one exact nonzero provider.
    #[codec(index = 0)]
    CompleteReplicationOrder {
        /// Governed provider identity.
        provider_id: crate::sorafs::capacity::ProviderId,
    },
    /// Change the network-wide reserve policy.
    #[codec(index = 1)]
    SetReservePolicy,
    /// Update network-wide provider credit.
    #[codec(index = 2)]
    UpsertProviderCredit,
    /// Operate the gateway compliance controls.
    #[codec(index = 3)]
    GatewayComplianceOperator,
}

/// One voted change to one exact operating authority.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::sorafs_authority::SorafsAuthorityActionV1")]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum SorafsAuthorityActionV1 {
    /// Approve one exact term's conditional grant without conferring delegation rights.
    #[codec(index = 0)]
    Grant,
    /// Remove the exact direct authority and retain its revision tombstone.
    #[codec(index = 1)]
    Revoke,
    /// Irrevocably approve removal of the same plan's grant at its fixed term end.
    #[codec(index = 2)]
    AuthorizeTerminalRevoke,
}

/// Complete immutable content of a Parliament operating-authority proposal.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::isi::sorafs_authority::SorafsAuthorityProposalV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityProposalV1 {
    /// Genesis-derived network security domain.
    pub network: NetworkId,
    /// Universal root governed by SORA Parliament.
    pub root: RootScopeV1,
    /// Canonical domainless account receiving or losing the authority.
    pub account: AccountId,
    /// Closed operating authority and its exact scope.
    pub authority: SorafsOperatingAuthorityV1,
    /// Exact independently voted decision about this plan.
    pub action: SorafsAuthorityActionV1,
    /// Immutable native consensus grid; every action for a plan carries the same grid.
    pub grid: SorafsAuthorityTermGridV1,
    /// Nonzero canonical term number; callers cannot choose arbitrary start or end heights.
    #[norito(json = "crate::json_helpers::u64_string")]
    pub term: u64,
    /// Exact subject revision before this plan's grant, identical for all three actions.
    /// Early revocation compares the active head with this revision plus one.
    #[norito(json = "crate::json_helpers::u64_string")]
    pub expected_revision: u64,
    /// Exact captured owner-approved signer policy for every completion-plan decision.
    /// It identifies the plan; early revocation does not require it to remain current.
    #[norito(required)]
    pub completion_policy: Option<ProviderIngestCompletionAuthorityV1>,
}

impl SorafsAuthorityProposalV1 {
    /// Reject invalid scopes and noncanonical action/precondition combinations.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.root != RootScopeV1::Universal {
            return Some("SoraFS operating authorities require the universal root");
        }
        if self.grid.window(self.term).is_none() {
            return Some("SoraFS authority term grid is invalid or overflows");
        }
        if self.expected_revision > u64::MAX - 3 {
            return Some(
                "SoraFS authority plan must retain revisions for grant and terminal revocation",
            );
        }
        if let SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id } = self.authority
            && provider_id.as_bytes() == &[0; 32]
        {
            return Some("completion authority requires a nonzero provider");
        }
        // The embedded existing policy uses a JSON number for its revision. Keep
        // this new host's public vote content exact in every SDK without changing it.
        if self.completion_policy.as_ref().is_some_and(|policy| {
            policy.signer_policy.revision
                > crate::governance::types::FIRST_RELEASE_MAX_EXACT_JSON_U64
        }) {
            return Some("completion signer policy revision exceeds the exact JSON integer range");
        }
        match (self.authority, self.completion_policy.as_ref()) {
            (SorafsOperatingAuthorityV1::CompleteReplicationOrder { .. }, Some(policy))
                if policy.is_valid() && policy.completion_signer == self.account =>
            {
                None
            }
            (SorafsOperatingAuthorityV1::CompleteReplicationOrder { .. }, _) => {
                Some("completion plan requires the exact valid captured signer policy")
            }
            (_, None) => None,
            (_, Some(_)) => Some("only completion plans carry a signer-policy precondition"),
        }
    }

    /// Exact plan shared by grant, terminal authorization and early revocation decisions.
    /// Only the action is normalized; no certificate or decision hash is nested recursively.
    #[must_use]
    pub fn plan_id(&self) -> [u8; 32] {
        let mut plan = self.clone();
        plan.action = SorafsAuthorityActionV1::Grant;
        crate::governance_fingerprint::fingerprint(b"iroha.sorafs.authority.plan.v1", &plan)
    }

    /// Term-wide decision family shared by corrected plans and all decision roles.
    /// Grid, predecessor, provider policy, proposer and wrapper identities cannot reset it.
    #[must_use]
    pub fn family_id(&self) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(
            b"iroha.sorafs.authority.family.v1",
            &(self.governed_subject_id_v1(), self.term),
        )
    }

    /// Network/root-bound content identity; changing a predecessor creates a new decision.
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(b"iroha.sorafs.authority.proposal.v1", self)
    }

    /// Independently domain-separated commitment to the complete effect.
    #[must_use]
    pub fn effect_preimage_hash_v1(&self) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(b"iroha.sorafs.authority.effect.v1", self)
    }

    /// Stable subject shared by grants and revocations, independent of their revision.
    #[must_use]
    pub fn governed_subject_id_v1(&self) -> [u8; 32] {
        Self::subject_id(&self.network, self.root, self.authority, &self.account)
    }

    /// Derive the exact subject key from the network, root, authority and account.
    #[must_use]
    pub fn subject_id(
        network: &NetworkId,
        root: RootScopeV1,
        authority: SorafsOperatingAuthorityV1,
        account: &AccountId,
    ) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(
            b"iroha.sorafs.authority.subject.v1",
            &(*network, root, authority, account.clone()),
        )
    }

    /// Content identity consumed by the shared Parliament attempt engine.
    #[must_use]
    pub fn proposal_content_id(&self) -> ProposalContentId {
        ProposalContentId::new(self.fingerprint())
    }
}

/// Submit one closed operating-authority proposal through the selected addition.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::isi::sorafs_authority::ProposeSorafsAuthorityGovernanceV1"
)]
#[norito(deny_unknown_fields)]
pub struct ProposeSorafsAuthorityGovernanceV1 {
    /// Exact immutable proposal.
    pub proposal: SorafsAuthorityProposalV1,
}
impl crate::seal::Instruction for ProposeSorafsAuthorityGovernanceV1 {}
impl ProposeSorafsAuthorityGovernanceV1 {
    /// Stable strict wire identifier owned by the release addition.
    pub const WIRE_ID: &'static str = "iroha.sorafs.authority.propose.v1";
}

/// Start a shared Parliament attempt for one exact hosted proposal.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::isi::sorafs_authority::CreateSorafsAuthorityParliamentAttemptV1"
)]
#[norito(deny_unknown_fields)]
pub struct CreateSorafsAuthorityParliamentAttemptV1 {
    /// Exact retained typed proposal.
    pub proposal: SorafsAuthorityProposalV1,
    /// Zero-based retry sequence owned by the existing Parliament reducer.
    pub attempt_sequence: u32,
}
impl crate::seal::Instruction for CreateSorafsAuthorityParliamentAttemptV1 {}
impl CreateSorafsAuthorityParliamentAttemptV1 {
    /// Stable strict wire identifier owned by the release addition.
    pub const WIRE_ID: &'static str = "iroha.sorafs.authority.attempt.create.v1";

    /// Exact shared-engine attempt identity; retries never replace proposal content.
    #[must_use]
    pub fn governance_attempt_id(&self) -> GovernanceAttemptId {
        GovernanceAttemptId::derive_v1(self.proposal.proposal_content_id(), self.attempt_sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf, KeyPair};

    fn proposal() -> SorafsAuthorityProposalV1 {
        SorafsAuthorityProposalV1 {
            network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [1; 32],
            ))),
            root: RootScopeV1::Universal,
            account: AccountId::new(KeyPair::random().public_key().clone()),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: SorafsAuthorityActionV1::Grant,
            grid: SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 10,
            },
            term: 1,
            expected_revision: 0,
            completion_policy: None,
        }
    }

    #[test]
    fn term_grid_has_exact_nonoverlapping_windows_and_checked_boundaries() {
        let grid = SorafsAuthorityTermGridV1 {
            epoch_length_blocks: 10,
        };
        assert_eq!(
            grid.window(1),
            Some(SorafsAuthorityTermWindowV1 {
                start_height: 641,
                end_height: 1281,
                cutoff_height: 631,
                admission_height: 1,
            })
        );
        let second = grid.window(2).unwrap();
        assert_eq!(
            second.admission_height,
            grid.window(1).unwrap().start_height
        );
        assert_eq!(second.start_height, grid.window(1).unwrap().end_height);
        assert!(grid.window(0).is_none());
        assert!(grid.window(u64::MAX).is_none());
        for length in [0, 1, 2, u64::MAX] {
            assert!(
                SorafsAuthorityTermGridV1 {
                    epoch_length_blocks: length
                }
                .window(1)
                .is_none()
            );
        }
        assert_eq!(SORAFS_AUTHORITY_ROLE_RANDOMNESS_UNITS_V1, 17);
    }

    #[test]
    fn paired_decisions_share_exact_plan_and_corrected_plans_share_family_budget() {
        let proposal = proposal();
        let mut ids = std::collections::BTreeSet::new();
        for action in [
            SorafsAuthorityActionV1::Grant,
            SorafsAuthorityActionV1::Revoke,
            SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
        ] {
            let mut decision = proposal.clone();
            decision.action = action;
            assert!(decision.invariant_error().is_none());
            assert_eq!(decision.plan_id(), proposal.plan_id());
            assert_eq!(decision.family_id(), proposal.family_id());
            assert!(ids.insert(decision.fingerprint()));
            let json = norito::json::to_json(&decision).unwrap();
            assert_eq!(
                norito::json::from_str::<SorafsAuthorityProposalV1>(&json).unwrap(),
                decision
            );
            assert_eq!(
                norito::decode_from_bytes::<SorafsAuthorityProposalV1>(
                    &norito::to_bytes(&decision).unwrap()
                )
                .unwrap(),
                decision
            );
        }
        let mut corrected = proposal.clone();
        corrected.expected_revision = 2;
        assert_ne!(corrected.plan_id(), proposal.plan_id());
        assert_eq!(corrected.family_id(), proposal.family_id());
        corrected = proposal.clone();
        corrected.grid.epoch_length_blocks = 20;
        assert_ne!(corrected.plan_id(), proposal.plan_id());
        assert_eq!(corrected.family_id(), proposal.family_id());
        corrected = proposal.clone();
        corrected.term += 1;
        assert_ne!(corrected.plan_id(), proposal.plan_id());
        assert_ne!(corrected.family_id(), proposal.family_id());
        let mut missing = norito::json::to_value(&proposal).unwrap();
        missing.as_object_mut().unwrap().remove("grid");
        assert!(norito::json::from_value::<SorafsAuthorityProposalV1>(missing).is_err());
    }

    #[test]
    fn identities_separate_subject_decision_effect_network_and_retry() {
        let p = proposal();
        assert!(p.invariant_error().is_none());
        assert_ne!(p.fingerprint(), p.effect_preimage_hash_v1());
        assert_ne!(p.fingerprint(), p.governed_subject_id_v1());
        let mut next = p.clone();
        next.expected_revision = 2;
        assert_ne!(next.fingerprint(), p.fingerprint());
        assert_eq!(next.governed_subject_id_v1(), p.governed_subject_id_v1());
        next.action = SorafsAuthorityActionV1::Revoke;
        assert_ne!(next.fingerprint(), p.fingerprint());
        assert_eq!(next.governed_subject_id_v1(), p.governed_subject_id_v1());
        next.network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([2; 32])));
        assert_ne!(next.governed_subject_id_v1(), p.governed_subject_id_v1());
        let mut attempt = CreateSorafsAuthorityParliamentAttemptV1 {
            proposal: p,
            attempt_sequence: 0,
        };
        let first = attempt.governance_attempt_id();
        attempt.attempt_sequence = 1;
        assert_ne!(first, attempt.governance_attempt_id());
    }

    #[test]
    fn closed_instructions_roundtrip_norito_and_json() {
        let p = proposal();
        for authority in [
            SorafsOperatingAuthorityV1::SetReservePolicy,
            SorafsOperatingAuthorityV1::UpsertProviderCredit,
            SorafsOperatingAuthorityV1::GatewayComplianceOperator,
        ] {
            let mut p = p.clone();
            p.authority = authority;
            p.expected_revision = u64::MAX;
            let instruction = ProposeSorafsAuthorityGovernanceV1 { proposal: p };
            let bytes = norito::to_bytes(&instruction).unwrap();
            assert_eq!(
                norito::decode_from_bytes::<ProposeSorafsAuthorityGovernanceV1>(&bytes).unwrap(),
                instruction
            );
            let json = norito::json::to_json(&instruction).unwrap();
            assert!(json.contains("\"18446744073709551615\""));
            assert_eq!(
                norito::json::from_str::<ProposeSorafsAuthorityGovernanceV1>(&json).unwrap(),
                instruction
            );
            let attempt = CreateSorafsAuthorityParliamentAttemptV1 {
                proposal: instruction.proposal,
                attempt_sequence: 1,
            };
            assert_eq!(
                norito::decode_from_bytes::<CreateSorafsAuthorityParliamentAttemptV1>(
                    &norito::to_bytes(&attempt).unwrap()
                )
                .unwrap(),
                attempt
            );
        }
        let json = norito::json::to_json(&p).unwrap();
        assert!(
            norito::json::from_str::<SorafsAuthorityProposalV1>(
                &json.replace("\"SetReservePolicy\"", "\"ArbitraryPermission\"")
            )
            .is_err()
        );
    }

    #[test]
    fn authority_instructions_box_and_roundtrip_their_registered_wire_envelopes() {
        let proposal = proposal();
        let propose: crate::isi::InstructionBox = ProposeSorafsAuthorityGovernanceV1 {
            proposal: proposal.clone(),
        }
        .into();
        let attempt: crate::isi::InstructionBox = CreateSorafsAuthorityParliamentAttemptV1 {
            proposal,
            attempt_sequence: 0,
        }
        .into();
        for (instruction, wire) in [
            (propose, ProposeSorafsAuthorityGovernanceV1::WIRE_ID),
            (attempt, CreateSorafsAuthorityParliamentAttemptV1::WIRE_ID),
        ] {
            assert_eq!(crate::isi::instruction_wire_id(&instruction), Some(wire));
            let encoded = norito::encode_canonical(&instruction).unwrap();
            let decoded: crate::isi::InstructionBox = norito::decode_canonical(&encoded).unwrap();
            assert_eq!(decoded, instruction);
            assert!(decoded.as_opaque().is_none());
        }
    }

    #[test]
    fn authority_additions_preserve_malformed_envelopes_for_unselected_peers() {
        for wire in [
            ProposeSorafsAuthorityGovernanceV1::WIRE_ID,
            CreateSorafsAuthorityParliamentAttemptV1::WIRE_ID,
        ] {
            let schema = crate::isi::registry::built_in_instruction_schemas()
                .find(|entry| entry.wire_id == wire)
                .expect("authority interface is registered");
            assert!(schema.tolerant, "a new interface cannot tighten decoding");
            for frame in [b"".as_slice(), b"not a canonical instruction".as_slice()] {
                let instruction = crate::isi::decode_instruction_envelope(wire, frame)
                    .expect("malformed addition payload remains an opaque envelope");
                let opaque = instruction
                    .as_opaque()
                    .expect("malformed addition is opaque");
                assert_eq!(opaque.wire_id(), wire);
                assert_eq!(opaque.frame(), frame);
                let encoded = norito::encode_canonical(&instruction).unwrap();
                let decoded: crate::isi::InstructionBox =
                    norito::decode_canonical(&encoded).unwrap();
                assert_eq!(decoded, instruction);
            }
        }
    }

    #[test]
    fn revision_exhaustion_never_admits_an_irrevocable_grant() {
        let mut proposal = proposal();
        proposal.expected_revision = u64::MAX - 3;
        assert!(proposal.invariant_error().is_none());
        proposal.expected_revision = u64::MAX - 1;
        assert!(proposal.invariant_error().is_some());
        proposal.action = SorafsAuthorityActionV1::Revoke;
        proposal.expected_revision = u64::MAX - 3;
        assert!(proposal.invariant_error().is_none());
        proposal.expected_revision = u64::MAX - 2;
        assert!(proposal.invariant_error().is_some());
        proposal.expected_revision = u64::MAX;
        assert!(proposal.invariant_error().is_some());
        proposal.action = SorafsAuthorityActionV1::Grant;
        assert!(proposal.invariant_error().is_some());
    }

    #[test]
    fn completion_plan_requires_nonzero_scope_and_exact_captured_policy_for_each_action() {
        let mut p = proposal();
        p.authority = SorafsOperatingAuthorityV1::CompleteReplicationOrder {
            provider_id: crate::sorafs::capacity::ProviderId::new([0; 32]),
        };
        assert!(p.invariant_error().is_some());
        p.authority = SorafsOperatingAuthorityV1::CompleteReplicationOrder {
            provider_id: crate::sorafs::capacity::ProviderId::new([1; 32]),
        };
        assert!(p.invariant_error().is_some());
        p.completion_policy = Some(ProviderIngestCompletionAuthorityV1 {
            provider_owner: p.account.clone(),
            completion_signer: p.account.clone(),
            signer_policy: crate::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [1; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [2; 32],
            },
        });
        assert!(p.invariant_error().is_none());
        for action in [
            SorafsAuthorityActionV1::Grant,
            SorafsAuthorityActionV1::Revoke,
            SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
        ] {
            let mut decision = p.clone();
            decision.action = action;
            assert!(decision.invariant_error().is_none());
            assert_eq!(decision.plan_id(), p.plan_id());
        }
        let mut rotated = p.clone();
        rotated
            .completion_policy
            .as_mut()
            .unwrap()
            .signer_policy
            .policy_digest = [4; 32];
        assert_ne!(rotated.plan_id(), p.plan_id());
        assert_eq!(rotated.family_id(), p.family_id());
        let mut unsafe_json_policy = p.clone();
        let signer_policy = &mut unsafe_json_policy
            .completion_policy
            .as_mut()
            .unwrap()
            .signer_policy;
        signer_policy.revision = crate::governance::types::FIRST_RELEASE_MAX_EXACT_JSON_U64;
        signer_policy.predecessor_digest = Some([3; 32]);
        assert!(unsafe_json_policy.invariant_error().is_none());
        unsafe_json_policy
            .completion_policy
            .as_mut()
            .unwrap()
            .signer_policy
            .revision += 1;
        assert!(unsafe_json_policy.invariant_error().is_some());
        let bytes = norito::to_bytes(&p).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<SorafsAuthorityProposalV1>(&bytes).unwrap(),
            p
        );
        assert_eq!(
            norito::json::from_str::<SorafsAuthorityProposalV1>(
                &norito::json::to_json(&p).unwrap()
            )
            .unwrap(),
            p
        );
        p.completion_policy.as_mut().unwrap().completion_signer =
            AccountId::new(KeyPair::random().public_key().clone());
        assert!(p.invariant_error().is_some());
        p.action = SorafsAuthorityActionV1::Revoke;
        assert!(p.invariant_error().is_some());
        p.completion_policy = None;
        assert!(p.invariant_error().is_some());
        p.root = RootScopeV1::DataSpace(iroha_model_base::topology::DataSpaceId::new(1));
        assert!(p.invariant_error().is_some());
    }
}
