/// Initial PoP registry admission preserves governed issuer, signature and monotonic state checks.
mod sorafs_pop_registry_admission {
    use super::*;
    use crate::{executor::Executor, smartcontracts::ValidSingularQuery};
    use iroha_data_model::{
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                CommitSorafsPopCredentialBatch, PublishSorafsPopRevocationList,
                SetSorafsPopIssuerPolicy,
            },
        },
        query::sorafs::prelude::{
            FindSorafsPopAuditDigestBySequence, FindSorafsPopCredentialCommitmentByDigest,
            FindSorafsPopIssuerPolicy, FindSorafsPopRegistryStatus,
            FindSorafsPopRevocationByNonceCommitment,
        },
        sorafs::pop_registry::{
            POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1, POP_ISSUER_POLICY_VERSION_V1,
            PopCredentialCommitmentBatchV1, PopCredentialCommitmentV1, PopIssuerPolicyV1,
            PopRegistryAuditEventKindV1, pop_revocation_nonce_commitment_v1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer,
    };
    use iroha_test_samples::BOB_KEYPAIR;
    use sorafs_manifest::pop_credentials::{
        POP_COMMITMENT_ROOT_VERSION_V1, POP_CREDENTIAL_TREE_DEPTH_V1,
        POP_REVOCATION_LIST_VERSION_V1, POP_REVOCATION_TREE_DEPTH_V1, PopCommitmentRootV1,
        PopRevocationEntryV1, PopRevocationListV1, PopRevocationReasonV1, PopSignatureAlgorithmV1,
        PopSignatureV1, pop_commitment_root_signature_digest_v1,
        pop_revocation_list_signature_digest_v1, pop_revocation_root_v1,
        verify_pop_commitment_root_signature_v1, verify_pop_revocation_list_signature_v1,
    };

    const NOW: u64 = 10_000;
    const ROOT: [u8; 32] = {
        let mut bytes = [0; 32];
        bytes[0] = 0xA1;
        bytes
    };
    const CREDENTIAL: [u8; 32] = [0xA2; 32];
    const NONCE: [u8; 32] = {
        let mut bytes = [0; 32];
        bytes[0] = 0xA3;
        bytes
    };

    fn fixture() -> State {
        state_after_genesis(World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
            ],
            [],
        ))
    }

    fn public_key(keys: &KeyPair) -> [u8; 32] {
        let (algorithm, bytes) = keys.public_key().try_to_bytes().unwrap();
        assert_eq!(algorithm, Algorithm::Ed25519);
        bytes.try_into().unwrap()
    }

    fn policy() -> PopIssuerPolicyV1 {
        PopIssuerPolicyV1 {
            version: POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-initial".to_owned(),
            issuer_account: ALICE_ID.clone(),
            issuer_public_key: public_key(&ALICE_KEYPAIR),
            max_credentials_per_batch: 2,
            max_revocations_per_publication: 2,
            max_credential_lifetime_secs: 10_000,
            max_future_clock_skew_secs: 5,
            paused: false,
        }
    }

    fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::encode_canonical(value).expect("canonical Initial registry fixture")
    }

    fn empty_signature(keys: &KeyPair) -> PopSignatureV1 {
        PopSignatureV1 {
            algorithm: PopSignatureAlgorithmV1::Ed25519,
            public_key: public_key(keys).to_vec(),
            signature: Vec::new(),
        }
    }

    fn signed_root(keys: &KeyPair) -> PopCommitmentRootV1 {
        let mut root = PopCommitmentRootV1 {
            version: POP_COMMITMENT_ROOT_VERSION_V1,
            root_digest: ROOT,
            tree_size: 1,
            tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
            tree_version: 1,
            issuer_id: policy().issuer_id,
            published_at_epoch: NOW - 1,
            previous_root_digest: None,
            governance_event_digest: [0xA4; 32],
            publisher_signature: empty_signature(keys),
        };
        root.publisher_signature.signature = Signature::try_new(
            keys.private_key(),
            &pop_commitment_root_signature_digest_v1(&root).unwrap(),
        )
        .unwrap()
        .payload()
        .to_vec();
        verify_pop_commitment_root_signature_v1(&root).unwrap();
        root
    }

    fn signed_revocations(
        keys: &KeyPair,
        version: u64,
        entries: Vec<PopRevocationEntryV1>,
    ) -> PopRevocationListV1 {
        let mut revocations = PopRevocationListV1 {
            version: POP_REVOCATION_LIST_VERSION_V1,
            list_version: version,
            commitment_root: ROOT,
            revocation_root: pop_revocation_root_v1(&entries).unwrap(),
            revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
            issuer_id: policy().issuer_id,
            published_at_epoch: if version == 1 { NOW - 1 } else { NOW },
            entries,
            publisher_signature: empty_signature(keys),
        };
        revocations.publisher_signature.signature = Signature::try_new(
            keys.private_key(),
            &pop_revocation_list_signature_digest_v1(&revocations).unwrap(),
        )
        .unwrap()
        .payload()
        .to_vec();
        verify_pop_revocation_list_signature_v1(&revocations).unwrap();
        revocations
    }

    fn batch(keys: &KeyPair) -> PopCredentialCommitmentBatchV1 {
        PopCredentialCommitmentBatchV1 {
            version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
            issuer_policy_digest: policy().digest().unwrap(),
            commitment_root_payload: encode(&signed_root(keys)),
            revocation_list_payload: encode(&signed_revocations(keys, 1, Vec::new())),
            commitments: vec![PopCredentialCommitmentV1 {
                credential_commitment: CREDENTIAL,
                revocation_nonce_commitment: pop_revocation_nonce_commitment_v1(NONCE),
                commitment_root: ROOT,
                commitment_tree_version: 1,
                revocation_list_version: 1,
                issued_at_epoch: NOW - 100,
                expires_at_epoch: NOW + 1_000,
            }],
        }
    }

    fn entry() -> PopRevocationEntryV1 {
        PopRevocationEntryV1 {
            nonce: NONCE,
            revoked_at_epoch: NOW,
            reason: PopRevocationReasonV1::GovernanceSuspension,
        }
    }

    fn publication(
        keys: &KeyPair,
        version: u64,
        entries: Vec<PopRevocationEntryV1>,
    ) -> InstructionBox {
        PublishSorafsPopRevocationList::new(
            encode(&signed_revocations(keys, version, entries)),
            policy().digest().unwrap(),
        )
        .into()
    }

    fn grant(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        permissions: Vec<Permission>,
        through_role: bool,
    ) {
        transaction
            .world
            .account_permissions
            .remove(authority.clone());
        let role_id: RoleId = "pop_initial_permissions".parse().unwrap();
        if through_role {
            let mut role = Role::new(role_id.clone(), authority.clone());
            for permission in permissions {
                role = role.add_permission(permission);
            }
            transaction
                .world
                .roles
                .insert(role_id.clone(), role.build(authority));
            transaction.world.account_roles.insert(
                crate::role::RoleIdWithOwner::new(authority.clone(), role_id),
                (),
            );
        } else {
            transaction
                .world
                .account_roles
                .remove(crate::role::RoleIdWithOwner::new(
                    authority.clone(),
                    role_id,
                ));
            transaction
                .world
                .account_permissions
                .insert(authority.clone(), permissions.into_iter().collect());
        }
    }

    fn apply(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
    ) {
        assert!(initial_native_instruction_is_explicitly_admitted(
            &instruction
        ));
        Executor::Initial
            .execute_instruction(transaction, authority, instruction)
            .expect("exact governed PoP operation reaches native validation");
    }

    fn stored(transaction: &StateTransaction<'_, '_>) -> Vec<(String, Vec<u8>)> {
        transaction
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }

    fn reject_unchanged(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        marker: &str,
    ) {
        assert!(initial_native_instruction_is_explicitly_admitted(
            &instruction
        ));
        let before = stored(transaction);
        let error = Executor::Initial
            .execute_instruction(transaction, authority, instruction)
            .expect_err("Initial must preserve native PoP rejection");
        assert!(
            matches!(error, ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(ref message)))
            if message.contains(marker)),
            "expected native {marker:?}, got {error:?}"
        );
        assert_eq!(
            stored(transaction),
            before,
            "rejected PoP mutation altered registry or audit state"
        );
    }

    #[test]
    fn initial_executor_pop_registry_requires_exact_direct_and_role_permissions() {
        for through_role in [false, true] {
            let state = fixture();
            let mut block = state.block(BlockHeader::new(
                nonzero!(2_u64),
                None,
                None,
                None,
                NOW * 1_000,
                0,
            ));
            let mut transaction = block.transaction();
            let management = Permission::from(CanManageSorafsPopRegistry);
            let operation = Permission::from(CanOperateSorafsPopIssuer);
            let policy_instruction: InstructionBox = SetSorafsPopIssuerPolicy::new(policy()).into();
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                policy_instruction.clone(),
                management.name(),
            );
            let malformed = Permission::new(management.name().to_owned(), Json::new(false));
            grant(&mut transaction, &ALICE_ID, vec![malformed], through_role);
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                policy_instruction.clone(),
                management.name(),
            );
            grant(&mut transaction, &ALICE_ID, vec![management], through_role);
            apply(&mut transaction, &ALICE_ID, policy_instruction);

            let batch_instruction: InstructionBox =
                CommitSorafsPopCredentialBatch::new(encode(&batch(&ALICE_KEYPAIR))).into();
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                batch_instruction.clone(),
                operation.name(),
            );
            let malformed = Permission::new(operation.name().to_owned(), Json::new(false));
            grant(
                &mut transaction,
                &ALICE_ID,
                vec![malformed.clone()],
                through_role,
            );
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                batch_instruction.clone(),
                operation.name(),
            );
            grant(
                &mut transaction,
                &ALICE_ID,
                vec![operation.clone()],
                through_role,
            );
            apply(&mut transaction, &ALICE_ID, batch_instruction);

            let revocation = publication(&ALICE_KEYPAIR, 2, vec![entry()]);
            grant(&mut transaction, &ALICE_ID, vec![malformed], through_role);
            reject_unchanged(
                &mut transaction,
                &ALICE_ID,
                revocation.clone(),
                operation.name(),
            );
            grant(&mut transaction, &ALICE_ID, vec![operation], through_role);
            apply(&mut transaction, &ALICE_ID, revocation);
            let status = FindSorafsPopRegistryStatus.execute(&transaction).unwrap();
            assert_eq!(status.credential_commitment_count, 1);
            assert_eq!(status.revoked_credential_count, 1);
            assert_eq!(status.active_tree_version, 1);
            assert_eq!(status.active_revocation_list_version, 2);
        }
    }

    #[test]
    fn initial_executor_pop_registry_preserves_issuer_signature_and_policy_binding() {
        let state = fixture();
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            None,
            None,
            None,
            NOW * 1_000,
            0,
        ));
        let mut transaction = block.transaction();
        grant(
            &mut transaction,
            &ALICE_ID,
            vec![
                CanManageSorafsPopRegistry.into(),
                CanOperateSorafsPopIssuer.into(),
            ],
            false,
        );
        grant(
            &mut transaction,
            &BOB_ID,
            vec![CanOperateSorafsPopIssuer.into()],
            false,
        );
        let missing_keys = KeyPair::try_from_seed(vec![0xAF; 32], Algorithm::Ed25519).unwrap();
        let mut missing_issuer = policy();
        missing_issuer.issuer_account = AccountId::new(missing_keys.public_key().clone());
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            SetSorafsPopIssuerPolicy::new(missing_issuer).into(),
            "issuer policy account is not registered",
        );
        apply(
            &mut transaction,
            &ALICE_ID,
            SetSorafsPopIssuerPolicy::new(policy()).into(),
        );
        let canonical = batch(&ALICE_KEYPAIR);
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&canonical).unwrap()
        };
        assert_ne!(alternate, encode(&canonical));
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(alternate).into(),
            "is not exact canonical Norito",
        );
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            CommitSorafsPopCredentialBatch::new(encode(&canonical)).into(),
            "authority does not match the active PoP issuer account",
        );
        let mut wrong_policy = canonical.clone();
        wrong_policy.issuer_policy_digest = [0xAA; 32];
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(encode(&wrong_policy)).into(),
            "issuer-policy digest does not match",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(encode(&batch(&BOB_KEYPAIR))).into(),
            "publisher key does not match",
        );
        let mut bad_signature = canonical.clone();
        let mut root = signed_root(&ALICE_KEYPAIR);
        root.root_digest[0] ^= 1;
        bad_signature.commitment_root_payload = encode(&root);
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(encode(&bad_signature)).into(),
            "invalid signed PoP commitment root",
        );
        apply(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(encode(&canonical)).into(),
        );
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            publication(&ALICE_KEYPAIR, 2, vec![entry()]),
            "authority does not match the active PoP issuer account",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            publication(&BOB_KEYPAIR, 2, vec![entry()]),
            "publisher key does not match",
        );
        let mut paused = policy();
        paused.revision = 2;
        paused.predecessor_policy_digest = Some(policy().digest().unwrap());
        paused.paused = true;
        apply(
            &mut transaction,
            &ALICE_ID,
            SetSorafsPopIssuerPolicy::new(paused).into(),
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            CommitSorafsPopCredentialBatch::new(encode(&canonical)).into(),
            "paused by governance",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            publication(&ALICE_KEYPAIR, 2, vec![entry()]),
            "paused by governance",
        );
    }

    #[test]
    fn initial_executor_pop_registry_commits_once_and_rejects_version_or_revocation_rollback() {
        let state = fixture();
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            None,
            None,
            None,
            NOW * 1_000,
            0,
        ));
        let mut transaction = block.transaction();
        grant(
            &mut transaction,
            &ALICE_ID,
            vec![
                CanManageSorafsPopRegistry.into(),
                CanOperateSorafsPopIssuer.into(),
            ],
            false,
        );
        let mut invalid_first = policy();
        invalid_first.revision = 2;
        invalid_first.predecessor_policy_digest = Some([0xAD; 32]);
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            SetSorafsPopIssuerPolicy::new(invalid_first).into(),
            "first PoP issuer policy must be revision one",
        );
        let policy_instruction: InstructionBox = SetSorafsPopIssuerPolicy::new(policy()).into();
        apply(&mut transaction, &ALICE_ID, policy_instruction.clone());
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            policy_instruction,
            "must exactly follow active revision",
        );
        let mut wrong_predecessor = policy();
        wrong_predecessor.revision = 2;
        wrong_predecessor.predecessor_policy_digest = Some([0xAC; 32]);
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            SetSorafsPopIssuerPolicy::new(wrong_predecessor).into(),
            "predecessor does not match",
        );
        let batch_instruction: InstructionBox =
            CommitSorafsPopCredentialBatch::new(encode(&batch(&ALICE_KEYPAIR))).into();
        apply(&mut transaction, &ALICE_ID, batch_instruction.clone());
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            batch_instruction,
            "rolls back or skips the active root/list version",
        );
        let mut unknown = entry();
        unknown.nonce[0] ^= 1;
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            publication(&ALICE_KEYPAIR, 2, vec![unknown]),
            "nonce is not bound to an authoritative credential commitment",
        );
        let revocation = publication(&ALICE_KEYPAIR, 2, vec![entry()]);
        apply(&mut transaction, &ALICE_ID, revocation.clone());
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            revocation,
            "must exactly follow active version",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            publication(&ALICE_KEYPAIR, 3, Vec::new()),
            "rolls back or mutates an existing revocation",
        );
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            publication(&ALICE_KEYPAIR, 3, vec![entry()]),
            "must add at least one new nonce",
        );
        let status = FindSorafsPopRegistryStatus.execute(&transaction).unwrap();
        assert_eq!(status.credential_commitment_count, 1);
        assert_eq!(status.revoked_credential_count, 1);
        let issuer = FindSorafsPopIssuerPolicy.execute(&transaction).unwrap();
        assert_eq!(issuer.policy, policy());
        assert_eq!(issuer.activated_by, *ALICE_ID);
        let credential = FindSorafsPopCredentialCommitmentByDigest::new(CREDENTIAL)
            .execute(&transaction)
            .unwrap();
        assert_eq!(credential.committed_by, *ALICE_ID);
        assert_eq!(credential.admitted_policy_digest, issuer.policy_digest);
        let revoked = FindSorafsPopRevocationByNonceCommitment::new(
            pop_revocation_nonce_commitment_v1(NONCE),
        )
        .execute(&transaction)
        .unwrap();
        assert_eq!(revoked.credential_commitment, CREDENTIAL);
        assert_eq!(revoked.recorded_by, *ALICE_ID);
        let mut previous = None;
        for (sequence, kind) in [
            (1, PopRegistryAuditEventKindV1::PolicyActivated),
            (2, PopRegistryAuditEventKindV1::CredentialBatchCommitted),
            (3, PopRegistryAuditEventKindV1::RevocationListPublished),
        ] {
            let audit = FindSorafsPopAuditDigestBySequence::new(sequence)
                .execute(&transaction)
                .unwrap();
            assert_eq!(audit.kind, kind);
            assert_eq!(audit.recorded_by, *ALICE_ID);
            assert_eq!(audit.previous_audit_digest, previous);
            previous = Some(audit.audit_digest);
        }
        assert_eq!(status.audit_sequence, 3);
        assert_eq!(status.audit_head, previous);
    }
}
