//! Actual native instructions and exact result-bearing blocks with real test-only BLS finality.
//!
//! World/frontier test commits are deliberately not a consensus application-state-root proof.
//! Custody attestations are software-signed fixtures and do not qualify a physical device.

use super::*;
use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    permission::{Permission, Permissions},
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::{
    self as manifest,
    signer::{
        custody::{
            SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
            SignerCustodyRecordV1, SignerCustodyStatementV1, SignerCustodyUseContextV1,
            verify_signer_custody_use_v1,
        },
        custody_control::SignerCustodyPolicyV1,
        final_promotion::{
            SignerFinalPromotionExpectedV1, signer_final_promotion_digest_v1,
            statement::prepare_final_promotion_statement_v1,
        },
    },
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sorafs_manifest/src/signer/final_promotion/tests/statement_fixture_support.rs"
));

pub(super) const NOW: u64 = 3_000;
pub(super) const DEPLOYMENT: &str = "promotion-primary";

pub(super) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}

pub(super) struct Fixture {
    pub(super) state: Arc<State>,
    pub(super) policy: SignerCustodyPolicyV1,
    pub(super) finalized: Vec<SccpFinalizedBlockTestFixtureV1>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        Self::with_observer_permissions([])
    }

    pub(super) fn with_observer_permissions(
        extra_permissions: impl IntoIterator<Item = Permission>,
    ) -> Self {
        let manager = AccountId::new(key(1).public_key().clone());
        let operator = AccountId::new(key(2).public_key().clone());
        let observer = AccountId::new(key(3).public_key().clone());
        let mut extra_permissions = Some(extra_permissions);
        let mut world = World::new();
        for authority in [&manager, &operator, &observer] {
            let (id, account) = Account::new(authority.clone())
                .build(&manager)
                .into_key_value();
            world.accounts.insert(id, account);
        }
        for (authority, permission) in [
            (
                observer.clone(),
                Permission::from(CanCheckSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }),
            ),
            (
                manager,
                Permission::from(CanManageSorafsFinalPromotionCustody {
                    deployment_id: DEPLOYMENT.into(),
                }),
            ),
            (
                operator,
                Permission::from(CanOperateSorafsFinalPromotion {
                    deployment_id: DEPLOYMENT.into(),
                }),
            ),
        ] {
            let mut permissions = Permissions::new();
            permissions.insert(permission);
            if authority == observer {
                permissions.extend(extra_permissions.take().unwrap());
            }
            world.account_permissions.insert(authority, permissions);
        }
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().unwrap(),
            sccp_taira_finality_network_id_v1(),
        ));
        let policy = SignerCustodyPolicyV1 {
            binding: SignerCustodyBindingV1 {
                chain_id: SCCP_TAIRA_CHAIN_ID_V1.into(),
                network_id: *sccp_taira_finality_network_id_v1().as_bytes(),
                runtime_handle: "software://sorafs/final-promotion-provenance/primary".into(),
                key_handle: "software://sorafs/final-promotion-provenance/key-1".into(),
                service_id: "promotion-service".into(),
                administrator_id: "promotion-admin".into(),
                role: SignerRoleV1::FinalPromotionProvenance,
                purpose: SignerPurposeBindingV1::FinalPromotionProvenance {
                    deployment_id: DEPLOYMENT.into(),
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: key(4).public_key().clone(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [5; 32],
            },
            attester_authority: SignerCustodyAuthorityV1 {
                service_id: "custody-service".into(),
                administrator_id: "custody-admin".into(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [6; 32],
            },
            attester_public_key: key(7).public_key().clone(),
            active_from_unix_ms: 100,
            active_until_unix_ms: 500_000,
            max_validity_ms: 120_000,
            max_anchor_age_ms: 60_000,
        };
        let mut f = Self {
            state,
            policy,
            finalized: Vec::new(),
        };
        let configure = MutateSorafsFinalPromotionAuthority {
            deployment_id: DEPLOYMENT.into(),
            expected_control_revision: 0,
            expected_control_digest: [0; 32],
            action: FinalPromotionAuthorityActionV1::Configure(
                norito::encode_canonical(&f.policy).unwrap(),
            ),
        };
        assert_eq!(
            f.commit(1_000, vec![f.sign(configure.into(), 1, 1_000)], true, true),
            [true]
        );
        let current = f.snapshot();
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: f.policy.binding.clone(),
            authority: f.policy.attester_authority.clone(),
            anchor: current.custody_anchor,
            sequence: current.control.next_sequence,
            predecessor_digest: current.control.predecessor_digest,
            issued_at_unix_ms: 1_500,
            expires_at_unix_ms: 100_000,
            evidence_digest: [12; 32],
            revoked: false,
        };
        let signature =
            Signature::try_new(key(7).private_key(), &statement.signing_payload().unwrap())
                .unwrap();
        let record = SignerCustodyRecordV1 {
            statement,
            attestation: signature.payload().try_into().unwrap(),
        };
        let enroll = f.instruction(FinalPromotionAuthorityActionV1::Enroll(
            norito::encode_canonical(&record).unwrap(),
        ));
        assert_eq!(
            f.commit(1_500, vec![f.sign(enroll.into(), 1, 1_500)], true, true),
            [true]
        );
        f
    }

    pub(super) fn snapshot(&self) -> FinalPromotionAuthoritySnapshotV1 {
        super::super::super::read_final_promotion_authority_at_v1(
            &self.state.view(),
            &self.policy.binding,
            self.state.view().height() as u64,
            None,
        )
        .unwrap()
        .unwrap()
    }

    pub(super) fn instruction(
        &self,
        action: FinalPromotionAuthorityActionV1,
    ) -> MutateSorafsFinalPromotionAuthority {
        let current = self.snapshot();
        MutateSorafsFinalPromotionAuthority {
            deployment_id: DEPLOYMENT.into(),
            expected_control_revision: current.control_record.revision,
            expected_control_digest: current.custody_anchor.state_digest,
            action,
        }
    }

    pub(super) fn expected(&self) -> FinalPromotionCheckExpectedV1 {
        let current = self.snapshot();
        let binding = self.policy.binding.clone();
        let custody = verify_signer_custody_use_v1(
            current.control_record.enrollment.as_deref().unwrap(),
            &binding,
            &self.policy.custody_trust(),
            &SignerCustodyUseContextV1 {
                now_unix_ms: NOW,
                anchor_observed_at_unix_ms: NOW,
                current_anchor: current.custody_anchor,
                active_head: current.control.active_head.unwrap(),
                signer_revoked: false,
                attester_revoked: false,
            },
        )
        .unwrap();
        let message = statement_message(&binding);
        let prepared = prepare_final_promotion_statement_v1(&message, &binding).unwrap();
        let request = SignerFinalPromotionRequestV1::new(
            &custody,
            &SignerFinalPromotionExpectedV1 {
                operation_id: [31; 32],
                statement_digest: signer_final_promotion_digest_v1(&message),
                statement_size: message.len() as u64,
            },
            &prepared,
        )
        .unwrap();
        let proof = &self.finalized.last().unwrap().proof().finality_artifact;
        FinalPromotionCheckExpectedV1 {
            binding,
            observer: AccountId::new(key(3).public_key().clone()),
            expected_operator: AccountId::new(key(2).public_key().clone()),
            request,
            subject: FinalPromotionCheckSubjectV1::Current(current.operations.audit),
            control_revision: current.control_record.revision,
            control_digest: current.custody_anchor.state_digest,
            floor: FinalPromotionCheckFloorV1 {
                height: proof.height,
                block_hash: *proof.block_hash.as_ref(),
                context_id: proof.context_id(),
            },
        }
    }

    pub(super) fn prepared(&self) -> PreparedFinalPromotionCheckV1 {
        begin_final_promotion_check_v1(
            Arc::clone(&self.state),
            self.expected(),
            Duration::from_secs(60),
        )
        .unwrap()
    }

    pub(super) fn sign(
        &self,
        instruction: InstructionBox,
        seed: u8,
        now: u64,
    ) -> SignedTransaction {
        crate::query::signer_check::fixture::sign(&self.state, instruction, seed, now)
    }

    pub(super) fn pending(&self) -> PendingFinalPromotionCheckV1 {
        let prepared = self.prepared();
        let signed = self.sign(prepared.instruction().clone().into(), 3, NOW);
        prepared.bind_signed_transaction(signed).unwrap()
    }

    pub(super) fn commit(
        &mut self,
        now: u64,
        transactions: Vec<SignedTransaction>,
        membership: bool,
        finality: bool,
    ) -> Vec<bool> {
        crate::query::signer_check::fixture::commit(
            &self.state,
            &mut self.finalized,
            now,
            transactions,
            membership,
            finality,
        )
    }
}
