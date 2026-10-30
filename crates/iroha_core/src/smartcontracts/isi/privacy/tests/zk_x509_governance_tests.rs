//! X.509 governance lifecycle and atomic root-state regression coverage.

use super::*;

struct X509GovernanceRecords {
    anchor_origin: PrivacyZkX509TrustAnchorRecordV1,
    policy_origin: PrivacyZkX509CertificatePolicyRecordV1,
    crl_origin: PrivacyZkX509CrlRecordV1,
    anchor_rotation: PrivacyZkX509TrustAnchorRecordV1,
    policy_rotation: PrivacyZkX509CertificatePolicyRecordV1,
    crl_rotation: PrivacyZkX509CrlRecordV1,
    anchor_revoked: PrivacyZkX509TrustAnchorRecordV1,
    policy_revoked: PrivacyZkX509CertificatePolicyRecordV1,
    crl_revoked: PrivacyZkX509CrlRecordV1,
}

impl X509GovernanceRecords {
    fn new() -> Self {
        let anchor_origin =
            x509_trust_anchor(1, 0xD1, None, PrivacyZkX509RecordLifecycleV1::Active);
        let policy_origin = x509_policy(
            1,
            0xD2,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let crl_origin = x509_crl(
            1,
            1,
            0xE2,
            1_799_999_900,
            1_800_000_300,
            0xD4,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let anchor_rotation = x509_trust_anchor(
            2,
            0xD5,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let policy_rotation = x509_policy(
            2,
            0xD6,
            vec![0, 2, 3],
            Some(policy_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let crl_rotation = x509_crl(
            2,
            2,
            0xE3,
            1_799_999_950,
            1_800_000_300,
            0xD8,
            Some(crl_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let anchor_revoked = x509_trust_anchor(
            3,
            0xD5,
            Some(anchor_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        let policy_revoked = x509_policy(
            3,
            0xD6,
            vec![0, 2, 3],
            Some(policy_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        let crl_revoked = x509_crl(
            3,
            2,
            0xE3,
            1_799_999_950,
            1_800_000_300,
            0xD8,
            Some(crl_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        Self {
            anchor_origin,
            policy_origin,
            crl_origin,
            anchor_rotation,
            policy_rotation,
            crl_rotation,
            anchor_revoked,
            policy_revoked,
            crl_revoked,
        }
    }

    // Each lifecycle stage owns one block. Separate calls bound the live stack
    // frame: putting every block/transaction overlay in one test function also
    // reserves all of their debug-build stack slots during state construction.
    #[inline(never)]
    fn register_anchor_and_policy(&self, state: &State, height: u64) {
        let mut block = state.block(test_header_at(height));
        let anchor_origin = self.anchor_origin;
        let policy_origin = self.policy_origin.clone();
        {
            let mut transaction = block.transaction();
            let error = RegisterPrivacyZkX509TrustAnchorV1::new(anchor_origin)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("X.509 governance permission is mandatory");
            assert!(error.to_string().contains("CanEnactGovernance"), "{error}");
            assert_eq!(transaction.world.privacy_commitments.iter().count(), 0);
            assert_empty_and_unbudgeted(&transaction);
        }
        {
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            RegisterPrivacyZkX509TrustAnchorV1::new(anchor_origin)
                .execute(&ALICE_ID, &mut transaction)
                .expect(
                    "trust-anchor and CA root register atomically without a protocol policy row",
                );
            assert_eq!(transaction.world.privacy_roots.iter().count(), 1);
            assert_eq!(transaction.world.privacy_root_heads.iter().count(), 1);
            let ca_head = transaction
                .world
                .privacy_root_heads
                .get(
                    &PrivacyRootHeadKeyV1::new(
                        x509_ca_namespace(),
                        PrivacyRootRoleV1::CertificateAuthorityMembership,
                    )
                    .expect("CA root-head key"),
                )
                .expect("atomic CA root head");
            assert_eq!(ca_head.epoch(), 1);
            assert_eq!(ca_head.root(), anchor_origin.ca_membership_root);
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        {
            let mut transaction = block.transaction();
            RegisterPrivacyZkX509CertificatePolicyV1::new(policy_origin.clone())
                .execute(&ALICE_ID, &mut transaction)
                .expect("certificate-policy registration needs no protocol activation");
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        block
            .commit_world_overlay_for_testing()
            .expect("commit X.509 anchor and policy block");
    }

    #[inline(never)]
    fn register_crl_and_rotate_anchor(&self, state: &State, height: u64) {
        let mut block = state.block(test_header_at(height));
        let anchor_origin = self.anchor_origin;
        let policy_origin = self.policy_origin.clone();
        let crl_origin = self.crl_origin;
        let anchor_rotation = self.anchor_rotation;
        {
            let mut transaction = block.transaction();
            RegisterPrivacyZkX509CrlV1::new(crl_origin)
                .execute(&ALICE_ID, &mut transaction)
                .expect("signed CRL record registers without a secondary revocation root");
            assert_eq!(
                privacy_zk_x509_governance_record_counts_v1(&transaction.world.privacy_commitments)
                    .expect("canonical X.509 origins"),
                (1, 1)
            );
            assert_eq!(transaction.world.privacy_commitments.iter().count(), 3);
            assert_eq!(transaction.world.privacy_roots.iter().count(), 1);
            assert_eq!(transaction.world.privacy_root_heads.iter().count(), 1);
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            let snapshot = load_privacy_zk_x509_authoritative_state_v1(
                x509_trust_anchor_id(),
                x509_policy_id(),
                8,
                &transaction.world.privacy_commitments,
                &transaction.world.privacy_roots,
                &transaction.world.privacy_root_heads,
            )
            .expect("complete typed origin authoritative state");
            assert_eq!(snapshot.trust_anchor(), anchor_origin);
            assert_eq!(snapshot.certificate_policy(), &policy_origin);
            assert_eq!(snapshot.crl_record(), crl_origin);
            transaction.apply();
        }
        {
            let mut transaction = block.transaction();
            let budget_before = transaction.privacy_budget_for_testing();
            let count_before = transaction.world.privacy_commitments.iter().count();
            let roots_before = transaction.world.privacy_roots.iter().count();
            let heads_before = transaction.world.privacy_root_heads.iter().count();
            let error = RegisterPrivacyZkX509TrustAnchorV1::new(anchor_origin)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("duplicate trust-anchor origin");
            assert!(
                smart_contract_parameter_message(&error).contains("already registered"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                count_before
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), roots_before);
            assert_eq!(
                transaction.world.privacy_root_heads.iter().count(),
                heads_before
            );
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
        {
            let mut transaction = block.transaction();
            let budget_before = transaction.privacy_budget_for_testing();
            let roots_before = transaction.world.privacy_roots.iter().count();
            let heads_before = transaction.world.privacy_root_heads.iter().count();
            let publication = PrivacyRootPublicationV1 {
                namespace: x509_namespace(),
                role: PrivacyRootRoleV1::CertificateAuthorityMembership,
                epoch: 2,
                root: PrivacyRootV1::new([0xA2; 32]),
            };
            let error = PublishPrivacyRootV1::new(publication)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("generic and cross-scope X.509 roots must fail closed");
            assert!(
                smart_contract_parameter_message(&error).contains("derived atomically")
                    || smart_contract_parameter_message(&error).contains("incompatible"),
                "{error:?}"
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), roots_before);
            assert_eq!(
                transaction.world.privacy_root_heads.iter().count(),
                heads_before
            );
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
        {
            let mut transaction = block.transaction();
            let budget_before = transaction.privacy_budget_for_testing();
            let commitment_count = transaction.world.privacy_commitments.iter().count();
            let root_count = transaction.world.privacy_roots.iter().count();
            let outdated = x509_crl(
                1,
                1,
                0xEA,
                1_799_999_699,
                1_800_000_300,
                0xEA,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            let error = RegisterPrivacyZkX509CrlV1::new(outdated)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("CRL older than the consensus freshness limit");
            assert!(
                smart_contract_parameter_message(&error).contains("freshness limit"),
                "{error:?}"
            );
            let future = x509_crl(
                1,
                1,
                0xEB,
                1_800_000_001,
                1_800_000_301,
                0xEB,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            let error = RegisterPrivacyZkX509CrlV1::new(future)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("future CRL must not be accepted");
            assert!(
                smart_contract_parameter_message(&error).contains("not current"),
                "{error:?}"
            );
            let mut digest_substitution = crl_origin;
            digest_substitution.crl_der_digest = PrivacyX509CrlDerDigestV1::new([0xEC; 32]);
            let error = RegisterPrivacyZkX509CrlV1::new(digest_substitution)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("record/root substitution must invalidate the self-digest");
            assert!(
                smart_contract_parameter_message(&error).contains("digest"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                commitment_count
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), root_count);
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
        {
            let mut transaction = block.transaction();
            let count_before = transaction.world.privacy_commitments.iter().count();
            let root_count_before = transaction.world.privacy_roots.iter().count();
            let budget_before = transaction.privacy_budget_for_testing();
            let error = RotatePrivacyZkX509TrustAnchorV1::new(
                PrivacyZkX509TrustAnchorRecordDigestV1::new([0xEF; 32]),
                anchor_rotation,
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("stale trust-anchor digest");
            assert!(
                smart_contract_parameter_message(&error).contains("stale or substituted"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                count_before
            );
            assert_eq!(
                transaction.world.privacy_roots.iter().count(),
                root_count_before
            );
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
        {
            let mut transaction = block.transaction();
            RotatePrivacyZkX509TrustAnchorV1::new(anchor_origin.record_digest, anchor_rotation)
                .execute(&ALICE_ID, &mut transaction)
                .expect("exact trust-anchor and CA-root rotation");
            assert_eq!(transaction.world.privacy_roots.iter().count(), 2);
            let ca_head = transaction
                .world
                .privacy_root_heads
                .get(
                    &PrivacyRootHeadKeyV1::new(
                        x509_ca_namespace(),
                        PrivacyRootRoleV1::CertificateAuthorityMembership,
                    )
                    .expect("CA head"),
                )
                .expect("rotated CA head");
            assert_eq!(ca_head.epoch(), 2);
            assert_eq!(ca_head.root(), anchor_rotation.ca_membership_root);
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        block
            .commit_world_overlay_for_testing()
            .expect("commit X.509 CRL and anchor-rotation block");
    }

    #[inline(never)]
    fn rotate_policy_and_crl(&self, state: &State, height: u64) {
        let mut block = state.block(test_header_at(height));
        let policy_origin = self.policy_origin.clone();
        let crl_origin = self.crl_origin;
        let anchor_rotation = self.anchor_rotation;
        let policy_rotation = self.policy_rotation.clone();
        let crl_rotation = self.crl_rotation;
        {
            let mut transaction = block.transaction();
            RotatePrivacyZkX509CertificatePolicyV1::new(
                policy_origin.record_digest,
                policy_rotation.clone(),
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect("exact one-epoch certificate-policy rotation");
            assert_eq!(transaction.world.privacy_roots.iter().count(), 2);
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        {
            let mut transaction = block.transaction();
            let budget_before = transaction.privacy_budget_for_testing();
            let commitment_count = transaction.world.privacy_commitments.iter().count();
            let root_count = transaction.world.privacy_roots.iter().count();
            let error = RotatePrivacyZkX509CrlV1::new(
                PrivacyZkX509CrlRecordDigestV1::new([0xEE; 32]),
                crl_rotation,
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("stale CRL compare-and-swap digest");
            assert!(
                smart_contract_parameter_message(&error).contains("stale or substituted"),
                "{error:?}"
            );
            let mut record_substitution = crl_rotation;
            record_substitution.next_update_unix_seconds += 1;
            let error =
                RotatePrivacyZkX509CrlV1::new(crl_origin.record_digest, record_substitution)
                    .execute(&ALICE_ID, &mut transaction)
                    .expect_err("tampered CRL record must fail its self-digest");
            assert!(
                smart_contract_parameter_message(&error).contains("digest"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                commitment_count
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), root_count);
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
        {
            let mut transaction = block.transaction();
            RotatePrivacyZkX509CrlV1::new(crl_origin.record_digest, crl_rotation)
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete signed CRL rotates atomically");
            assert_eq!(transaction.world.privacy_roots.iter().count(), 2);
            assert_eq!(transaction.world.privacy_root_heads.iter().count(), 1);
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            let snapshot = load_privacy_zk_x509_authoritative_state_v1(
                x509_trust_anchor_id(),
                x509_policy_id(),
                8,
                &transaction.world.privacy_commitments,
                &transaction.world.privacy_roots,
                &transaction.world.privacy_root_heads,
            )
            .expect("fully rotated authoritative X.509 state");
            assert_eq!(snapshot.trust_anchor(), anchor_rotation);
            assert_eq!(snapshot.certificate_policy(), &policy_rotation);
            assert_eq!(snapshot.crl_record(), crl_rotation);
            transaction.apply();
        }
        block
            .commit_world_overlay_for_testing()
            .expect("commit X.509 policy and CRL-rotation block");
    }

    #[inline(never)]
    fn revoke_crl_and_policy(&self, state: &State, height: u64) {
        let mut block = state.block(test_header_at(height));
        let anchor_rotation = self.anchor_rotation;
        let policy_rotation = self.policy_rotation.clone();
        let crl_rotation = self.crl_rotation;
        let anchor_revoked = self.anchor_revoked;
        let policy_revoked = self.policy_revoked.clone();
        let crl_revoked = self.crl_revoked;
        {
            let mut transaction = block.transaction();
            let commitment_count = transaction.world.privacy_commitments.iter().count();
            let root_count = transaction.world.privacy_roots.iter().count();
            let error = RevokePrivacyZkX509TrustAnchorV1::new(
                anchor_rotation.record_digest,
                anchor_revoked,
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("active policy must block parent-anchor revocation");
            assert!(
                smart_contract_parameter_message(&error).contains("active certificate policy"),
                "{error:?}"
            );
            let error = RevokePrivacyZkX509CertificatePolicyV1::new(
                policy_rotation.record_digest,
                policy_revoked.clone(),
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("active CRL must block parent-policy revocation");
            assert!(
                smart_contract_parameter_message(&error).contains("active signed CRL"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                commitment_count
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), root_count);
            assert_eq!(transaction.privacy_budget_for_testing(), (0, 0, 0, 0));
        }
        {
            let mut transaction = block.transaction();
            RevokePrivacyZkX509CrlV1::new(crl_rotation.record_digest, crl_revoked)
                .execute(&ALICE_ID, &mut transaction)
                .expect("leaf CRL lineage revokes first");
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        {
            let mut transaction = block.transaction();
            RevokePrivacyZkX509CertificatePolicyV1::new(
                policy_rotation.record_digest,
                policy_revoked.clone(),
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect("policy revokes only after its CRL");
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            transaction.apply();
        }
        block
            .commit_world_overlay_for_testing()
            .expect("commit X.509 CRL and policy-revocation block");
    }

    #[inline(never)]
    fn revoke_anchor_and_reject_terminal_rotation(&self, state: &State, height: u64) {
        let mut block = state.block(test_header_at(height));
        let anchor_rotation = self.anchor_rotation;
        let anchor_revoked = self.anchor_revoked;
        let policy_revoked = self.policy_revoked.clone();
        let crl_revoked = self.crl_revoked;
        {
            let mut transaction = block.transaction();
            RevokePrivacyZkX509TrustAnchorV1::new(anchor_rotation.record_digest, anchor_revoked)
                .execute(&ALICE_ID, &mut transaction)
                .expect("trust anchor revokes only after every child");
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            assert_eq!(transaction.world.privacy_roots.iter().count(), 2);
            assert_eq!(transaction.world.privacy_root_heads.iter().count(), 1);
            let current_crl = crate::privacy_state::load_privacy_zk_x509_crl_v1(
                x509_trust_anchor_id(),
                x509_policy_id(),
                &transaction.world.privacy_commitments,
            )
            .expect("terminal CRL remains authoritative");
            assert_eq!(current_crl, crl_revoked);
            assert_eq!(
                load_privacy_zk_x509_certificate_policy_v1(
                    x509_trust_anchor_id(),
                    x509_policy_id(),
                    &transaction.world.privacy_commitments,
                )
                .expect("terminal certificate policy remains authoritative"),
                policy_revoked
            );
            assert_eq!(
                load_privacy_zk_x509_trust_anchor_v1(
                    x509_trust_anchor_id(),
                    &transaction.world.privacy_commitments,
                )
                .expect("terminal trust anchor remains authoritative"),
                anchor_revoked
            );
            transaction.apply();
        }
        {
            let mut transaction = block.transaction();
            let budget_before = transaction.privacy_budget_for_testing();
            let roots_before = transaction.world.privacy_roots.iter().count();
            let commitment_count = transaction.world.privacy_commitments.iter().count();
            let error = RotatePrivacyZkX509CertificatePolicyV1::new(
                policy_revoked.record_digest,
                x509_policy(
                    4,
                    0xDB,
                    vec![0, 2, 3],
                    Some(policy_revoked.record_digest),
                    PrivacyZkX509RecordLifecycleV1::Active,
                ),
            )
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("terminal policy cannot rotate");
            assert!(
                smart_contract_parameter_message(&error).contains("active trust anchor")
                    || smart_contract_parameter_message(&error).contains("not active"),
                "{error:?}"
            );
            let after_terminal = x509_crl(
                4,
                3,
                0xE4,
                1_799_999_975,
                1_800_000_300,
                0xD9,
                Some(crl_revoked.record_digest),
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            let error = RotatePrivacyZkX509CrlV1::new(crl_revoked.record_digest, after_terminal)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("terminal CRL cannot rotate");
            assert!(
                smart_contract_parameter_message(&error).contains("active")
                    || smart_contract_parameter_message(&error).contains("revoked"),
                "{error:?}"
            );
            assert_eq!(
                transaction.world.privacy_commitments.iter().count(),
                commitment_count
            );
            assert_eq!(transaction.world.privacy_roots.iter().count(), roots_before);
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
    }
}

#[test]
fn zk_x509_governance_and_roots_are_exact_failure_atomic_without_protocol_registration() {
    let state = state_with_default_privacy_policy();
    let records = X509GovernanceRecords::new();
    records.register_anchor_and_policy(&state, TEST_BLOCK_HEIGHT);
    records.register_crl_and_rotate_anchor(&state, TEST_BLOCK_HEIGHT + 1);
    records.rotate_policy_and_crl(&state, TEST_BLOCK_HEIGHT + 2);
    records.revoke_crl_and_policy(&state, TEST_BLOCK_HEIGHT + 3);
    records.revoke_anchor_and_reject_terminal_rotation(&state, TEST_BLOCK_HEIGHT + 4);
}

fn x509_statement_for_snapshot(
    snapshot: &crate::privacy_state::PrivacyZkX509AuthoritativeStateV1,
) -> iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1 {
    use iroha_data_model::privacy::{
        IrohaZkX509StarkP256StatementV1, PrivacyAttributeDigestV1, PrivacyCertificateKeyDigestV1,
        PrivacyChallengeV1, PrivacyEngineManifestDigestV1, PrivacyNullifierV1,
        PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
        PrivacyZkX509DisclosedAttributeV1,
    };

    let anchor = snapshot.trust_anchor();
    let policy = snapshot.certificate_policy();
    let crl = snapshot.crl_record();
    IrohaZkX509StarkP256StatementV1 {
        context: PrivacyStatementContextV1 {
            network_id: test_network_id(),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x91; 32]),
            parameter_id: PrivacyParameterIdV1::new([0x92; 32]),
            parameter_digest: PrivacyParameterDigestV1::new([0x93; 32]),
            verifier_digest: PrivacyVerifierDigestV1::new([0x94; 32]),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0x95; 32]),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0x96; 32]),
        },
        trust_anchor_id: anchor.trust_anchor_id,
        certificate_policy_id: policy.policy_id,
        trust_anchor_record_digest: anchor.record_digest,
        trust_anchor_record_epoch: anchor.record_epoch,
        certificate_policy_record_digest: policy.record_digest,
        certificate_policy_record_epoch: policy.record_epoch,
        crl_record_digest: crl.record_digest,
        crl_record_epoch: crl.record_epoch,
        subject_public_key_digest: PrivacyCertificateKeyDigestV1::new([0x97; 32]),
        ca_membership_root: snapshot.ca_membership_root(),
        ca_membership_root_epoch: snapshot.ca_membership_root_epoch(),
        key_usage: policy.required_key_usage,
        extended_key_usages: policy.required_extended_key_usages.clone(),
        disclosed_attributes: policy
            .required_disclosed_attribute_indices
            .iter()
            .map(|index| PrivacyZkX509DisclosedAttributeV1 {
                index: *index,
                attribute_digest: PrivacyAttributeDigestV1::new([index.wrapping_add(1); 32]),
            })
            .collect(),
        // Fixed public window common to both existing signed CRL fixtures. Do
        // not regenerate it from successor times when checking a stale attempt.
        presentation_not_before_unix_seconds: 1_800_000_000,
        presentation_not_after_unix_seconds: 1_800_000_001,
        wallet_account: iroha_test_samples::ALICE_ID.clone(),
        wallet_challenge: PrivacyChallengeV1::new([0x98; 32]),
        certificate_nullifier: PrivacyNullifierV1::new([0x99; 32]),
    }
}

fn assert_x509_snapshot_accepts_its_statement(
    snapshot: &crate::privacy_state::PrivacyZkX509AuthoritativeStateV1,
) -> iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1 {
    let statement = x509_statement_for_snapshot(snapshot);
    crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
        &statement,
        snapshot,
        1_800_000_000_000,
        &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
    )
    .expect("new statement binds the exact ordered authoritative snapshot");
    statement
}

fn temporal_snapshot(
    transaction: &StateTransaction<'_, '_>,
) -> crate::privacy_state::PrivacyZkX509AuthoritativeStateV1 {
    load_privacy_zk_x509_authoritative_state_v1(
        x509_trust_anchor_id(),
        x509_policy_id(),
        8,
        &transaction.world.privacy_commitments,
        &transaction.world.privacy_roots,
        &transaction.world.privacy_root_heads,
    )
    .expect("current active authoritative X.509 State")
}

#[inline(never)]
fn register_temporal_crl(state: &State, record: PrivacyZkX509CrlRecordV1) {
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 1));
    let mut transaction = block.transaction();
    RegisterPrivacyZkX509CrlV1::new(record)
        .execute(&ALICE_ID, &mut transaction)
        .expect("eligible CRL origin");
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit initial CRL");
}

#[inline(never)]
fn check_temporal_event_order(order: [u8; 3], first_block_events: usize) {
    assert!((1..=2).contains(&first_block_events));
    let state = state_with_default_privacy_policy();
    let records = X509GovernanceRecords::new();
    records.register_anchor_and_policy(&state, TEST_BLOCK_HEIGHT);
    register_temporal_crl(&state, records.crl_origin);
    // Issuer CRL numbers may skip; the governed revision may not. Both signed
    // intervals overlap at the executing block, so recency cannot be decided
    // by whichever interval happens to contain the presentation timestamp.
    let successor = x509_crl(
        2,
        9,
        0xE3,
        1_799_999_950,
        1_800_000_300,
        0xD8,
        Some(records.crl_origin.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let mut seen = [false; 3];
    let mut prior_statements = Vec::new();
    let mut expected_current = None;
    // The admission ceiling permits two privacy actions per block. Exercise
    // each same-block pair and its adjacent-block boundary without raising it.
    let (first, second) = order.split_at(first_block_events);
    for (offset, events) in [(0, first), (1, second)] {
        let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 2 + offset));
        for &event in events {
            let mut transaction = block.transaction();
            let before = temporal_snapshot(&transaction);
            let original = assert_x509_snapshot_accepts_its_statement(&before);
            if let Some(expected) = &expected_current {
                assert_eq!(
                    &original, expected,
                    "the next transaction retains the exact preceding State, including across blocks"
                );
            }
            prior_statements.push(original);
            match event {
                0 => RotatePrivacyZkX509TrustAnchorV1::new(
                    records.anchor_origin.record_digest,
                    records.anchor_rotation,
                )
                .execute(&ALICE_ID, &mut transaction),
                1 => RotatePrivacyZkX509CertificatePolicyV1::new(
                    records.policy_origin.record_digest,
                    records.policy_rotation.clone(),
                )
                .execute(&ALICE_ID, &mut transaction),
                2 => RotatePrivacyZkX509CrlV1::new(records.crl_origin.record_digest, successor)
                    .execute(&ALICE_ID, &mut transaction),
                _ => unreachable!("three explicit event types"),
            }
            .expect("ordered governance event");
            seen[usize::from(event)] = true;
            let after = temporal_snapshot(&transaction);
            assert_eq!(
                after.trust_anchor(),
                if seen[0] {
                    records.anchor_rotation
                } else {
                    records.anchor_origin
                }
            );
            assert_eq!(
                after.certificate_policy(),
                if seen[1] {
                    &records.policy_rotation
                } else {
                    &records.policy_origin
                }
            );
            assert_eq!(
                after.crl_record(),
                if seen[2] {
                    successor
                } else {
                    records.crl_origin
                }
            );
            let current = assert_x509_snapshot_accepts_its_statement(&after);
            for original in &prior_statements {
                assert_ne!(original, &current);
                let error = crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
                    original,
                    &after,
                    1_800_000_000_000,
                    &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
                )
                .expect_err("every superseded original statement remains invalid");
                assert!(error.contains("stale or substituted"), "{error}");
            }
            assert_eq!(transaction.privacy_budget_for_testing().0, 1);
            expected_current = Some(current);
            // Transaction application preserves actual order inside this block.
            transaction.apply();
        }
        // The next block must acquire the committed authoritative State; no
        // synthetic event ordering, manual budget reset, or secondary State.
        block
            .commit_world_overlay_for_testing()
            .expect("commit admitted governance event block");
    }
    assert_eq!(seen, [true; 3]);
    assert_eq!(prior_statements.len(), 3);
}

#[test]
fn zk_x509_merged_events_respect_block_budget_and_supersede_originals() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for first_block_events in [1, 2] {
            check_temporal_event_order(order, first_block_events);
        }
    }
}

#[test]
fn zk_x509_failed_crl_publications_leave_the_exact_current_selection() {
    let state = state_with_default_privacy_policy();
    let records = X509GovernanceRecords::new();
    records.register_anchor_and_policy(&state, TEST_BLOCK_HEIGHT);
    register_temporal_crl(&state, records.crl_origin);
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 2));
    let mut transaction = block.transaction();
    let before = temporal_snapshot(&transaction);
    let original = assert_x509_snapshot_accepts_its_statement(&before);
    let current_key =
        PrivacyCommitmentKeyV1::zk_x509_crl_current(x509_trust_anchor_id(), x509_policy_id())
            .unwrap();
    let state_before = proof_managed_state_snapshot(&transaction, current_key);
    for (epoch, this_update, next_update, authorized) in [
        (2, 1_800_000_001, 1_800_000_400, true),  // not yet eligible
        (2, 1_799_999_950, 1_800_000_000, true),  // nextUpdate is exclusive
        (3, 1_799_999_950, 1_800_000_300, true),  // missing governed revision
        (2, 1_799_999_950, 1_800_000_300, false), // no governance permission
    ] {
        let candidate = x509_crl(
            epoch,
            9,
            0xE3,
            this_update,
            next_update,
            0xD8,
            Some(records.crl_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let error = RotatePrivacyZkX509CrlV1::new(records.crl_origin.record_digest, candidate)
            .execute(
                if authorized {
                    &ALICE_ID
                } else {
                    &iroha_test_samples::BOB_ID
                },
                &mut transaction,
            )
            .expect_err("ineligible or unauthorized publication must fail atomically");
        if !authorized {
            assert!(error.to_string().contains("CanEnactGovernance"), "{error}");
        }
        assert_eq!(temporal_snapshot(&transaction), before);
        assert_eq!(
            proof_managed_state_snapshot(&transaction, current_key),
            state_before
        );
        crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
            &original,
            &before,
            1_800_000_000_000,
            &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
        )
        .expect("failed publication did not supersede the original");
    }
}

#[test]
fn zk_x509_expired_crl_revocation_is_immediate_without_older_fallback() {
    let state = state_with_default_privacy_policy();
    let records = X509GovernanceRecords::new();
    records.register_anchor_and_policy(&state, TEST_BLOCK_HEIGHT);
    register_temporal_crl(&state, records.crl_origin);
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(TEST_BLOCK_HEIGHT + 2).unwrap(),
        None,
        None,
        1_800_000_500_000,
        0,
    ));
    let mut transaction = block.transaction();
    let before = temporal_snapshot(&transaction);
    let original = assert_x509_snapshot_accepts_its_statement(&before);
    assert!(
        crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
            &original,
            &before,
            1_800_000_500_000,
            &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
        )
        .is_err(),
        "a quiet chain cannot extend a previously observed window"
    );
    let revoked = x509_crl(
        2,
        records.crl_origin.crl_number,
        0xE2,
        records.crl_origin.this_update_unix_seconds,
        records.crl_origin.next_update_unix_seconds,
        0xD4,
        Some(records.crl_origin.record_digest),
        PrivacyZkX509RecordLifecycleV1::Revoked,
    );
    RevokePrivacyZkX509CrlV1::new(records.crl_origin.record_digest, revoked)
        .execute(&ALICE_ID, &mut transaction)
        .expect("revocation does not wait for a fresh CRL");
    let error = load_privacy_zk_x509_authoritative_state_v1(
        x509_trust_anchor_id(),
        x509_policy_id(),
        8,
        &transaction.world.privacy_commitments,
        &transaction.world.privacy_roots,
        &transaction.world.privacy_root_heads,
    )
    .expect_err("revocation prevents any active selection immediately");
    assert!(
        error.contains("revoked") || error.contains("not active"),
        "{error}"
    );
    let resurrection = x509_crl(
        3,
        9,
        0xE3,
        1_800_000_499,
        1_800_000_800,
        0xD8,
        Some(revoked.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let budget = transaction.privacy_budget_for_testing();
    RotatePrivacyZkX509CrlV1::new(revoked.record_digest, resurrection)
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("a fresh successor cannot revive a revoked lineage");
    assert_eq!(transaction.privacy_budget_for_testing(), budget);
    assert_eq!(
        crate::privacy_state::load_privacy_zk_x509_crl_v1(
            x509_trust_anchor_id(),
            x509_policy_id(),
            &transaction.world.privacy_commitments,
        )
        .unwrap(),
        revoked
    );
}

#[test]
fn zk_x509_publication_freshness_uses_inclusive_seconds_and_calendar_ceiling() {
    use iroha_data_model::privacy::ZK_X509_MAX_UNIX_SECONDS_V1 as MAX;
    for (this_update, next_update, timestamp_ms, expected) in [
        (1_800_000_000, 1_800_000_400, 1_799_999_999_999, false),
        (1_800_000_000, 1_800_000_400, 1_800_000_000_000, true),
        (1_800_000_000, 1_800_000_400, 1_800_000_300_999, true),
        (1_800_000_000, 1_800_000_400, 1_800_000_301_000, false),
        (1_800_000_000, 1_800_000_100, 1_800_000_099_999, true),
        (1_800_000_000, 1_800_000_100, 1_800_000_100_000, false),
        (MAX - 10, MAX + 100, MAX * 1_000 + 999, true),
        (MAX - 10, MAX + 100, (MAX + 1) * 1_000, false),
        (MAX + 1, MAX + 100, (MAX + 1) * 1_000, false),
    ] {
        let record = x509_crl(
            1,
            1,
            0xE2,
            this_update,
            next_update,
            0xD4,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_crl_freshness_v1(record, timestamp_ms).is_ok(),
            expected,
            "CRL [{this_update}, {next_update}) at {timestamp_ms} ms"
        );
    }
}

#[inline(never)]
fn check_calendar_publication(rotation: bool, beyond_calendar: bool) {
    use iroha_data_model::privacy::ZK_X509_MAX_UNIX_SECONDS_V1 as MAX;
    let state = state_with_default_privacy_policy();
    let records = X509GovernanceRecords::new();
    records.register_anchor_and_policy(&state, TEST_BLOCK_HEIGHT);
    if rotation {
        register_temporal_crl(&state, records.crl_origin);
    }
    let timestamp = if beyond_calendar {
        (MAX + 1) * 1_000
    } else {
        MAX * 1_000 + 999
    };
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(TEST_BLOCK_HEIGHT + 2).unwrap(),
        None,
        None,
        timestamp,
        0,
    ));
    let mut transaction = block.transaction();
    let current_key =
        PrivacyCommitmentKeyV1::zk_x509_crl_current(x509_trust_anchor_id(), x509_policy_id())
            .unwrap();
    let state_before = proof_managed_state_snapshot(&transaction, current_key);
    let record = x509_crl(
        if rotation { 2 } else { 1 },
        9,
        0xE3,
        MAX - 10,
        MAX + 100,
        0xD8,
        rotation.then_some(records.crl_origin.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let outcome = if rotation {
        RotatePrivacyZkX509CrlV1::new(records.crl_origin.record_digest, record)
            .execute(&ALICE_ID, &mut transaction)
    } else {
        RegisterPrivacyZkX509CrlV1::new(record).execute(&ALICE_ID, &mut transaction)
    };
    if beyond_calendar {
        let error = outcome.expect_err("publications after the RFC calendar are ineligible");
        assert!(
            smart_contract_parameter_message(&error).contains("not current"),
            "{error:?}"
        );
        assert_eq!(
            proof_managed_state_snapshot(&transaction, current_key),
            state_before
        );
        if rotation {
            assert_eq!(
                temporal_snapshot(&transaction).crl_record(),
                records.crl_origin
            );
        }
    } else {
        outcome.expect("the last calendar millisecond remains eligible");
        assert_eq!(temporal_snapshot(&transaction).crl_record(), record);
        assert_eq!(transaction.privacy_budget_for_testing().0, 1);
    }
}

#[test]
fn zk_x509_register_and_rotate_enforce_calendar_boundary_atomically() {
    for rotation in [false, true] {
        for beyond_calendar in [false, true] {
            check_calendar_publication(rotation, beyond_calendar);
        }
    }
}
