use super::*;
use iroha_crypto::{HybridKeyPair, KeyPair};
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;
use sorafs_manifest::pop_credentials::{
    POP_CREDENTIAL_TREE_DEPTH_V1, POP_REVOCATION_TREE_DEPTH_V1, derive_pop_holder_commitment_v1,
};
use std::{io, sync::Arc};
use tempfile::TempDir;
#[derive(Debug)]
struct TestAuthenticator {
    request_authority: PopRequestAuthorityV1,
}
impl PopCredentialApiAuthenticator for TestAuthenticator {
    fn authenticate(
        &self,
        _opaque_credential: &[u8],
        _action: PopCredentialApiActionV1,
        _request_binding: [u8; 32],
        _now_epoch: u64,
    ) -> Result<PopAuthenticatedPrincipalV1, String> {
        Ok(PopAuthenticatedPrincipalV1 {
            principal_digest: [0x31; 32],
            expires_at_epoch: 101,
            request_authority: self.request_authority,
        })
    }
}
#[test]
fn mutation_actions_require_exact_caller_signed_authority() {
    let authenticated = PopCredentialApiV1::new(Arc::new(TestAuthenticator {
        request_authority: PopRequestAuthorityV1::AuthenticatedRequest,
    }));
    for action in [
        PopCredentialApiActionV1::ReadEnrollmentStatus,
        PopCredentialApiActionV1::SubmitRegistryOutbox,
        PopCredentialApiActionV1::ReconcileRegistry,
        PopCredentialApiActionV1::ReadRegistryProjection,
        PopCredentialApiActionV1::FetchWalletDelivery,
        PopCredentialApiActionV1::ProveMembership,
    ] {
        assert!(
            authenticated
                .authorize(b"credential", action, [0x32; 32], 100)
                .is_ok(),
            "authenticated read or durable work action {action:?}"
        );
    }
    for action in [
        PopCredentialApiActionV1::SubmitEnrollment,
        PopCredentialApiActionV1::ApproveEnrollment,
        PopCredentialApiActionV1::IssueCredential,
        PopCredentialApiActionV1::TriggerCredentialIssuance,
        PopCredentialApiActionV1::EnqueueRevocation,
        PopCredentialApiActionV1::AcknowledgeWalletDelivery,
        PopCredentialApiActionV1::ImportWalletDelivery,
        PopCredentialApiActionV1::SynchronizeWalletWitness,
        PopCredentialApiActionV1::VerifyMembership,
    ] {
        assert_eq!(
            authenticated.authorize(b"credential", action, [0x33; 32], 100),
            Err(PopCredentialServiceError::Unauthorized),
            "unsigned mutation action {action:?}"
        );
    }
    let caller_signed = PopCredentialApiV1::new(Arc::new(TestAuthenticator {
        request_authority: PopRequestAuthorityV1::CallerSignedTransaction,
    }));
    for action in [
        PopCredentialApiActionV1::SubmitEnrollment,
        PopCredentialApiActionV1::ApproveEnrollment,
        PopCredentialApiActionV1::IssueCredential,
        PopCredentialApiActionV1::TriggerCredentialIssuance,
        PopCredentialApiActionV1::EnqueueRevocation,
        PopCredentialApiActionV1::AcknowledgeWalletDelivery,
        PopCredentialApiActionV1::ImportWalletDelivery,
        PopCredentialApiActionV1::SynchronizeWalletWitness,
        PopCredentialApiActionV1::VerifyMembership,
    ] {
        assert!(
            caller_signed
                .authorize(b"credential", action, [0x34; 32], 100)
                .is_ok(),
            "caller-signed mutation action {action:?}"
        );
    }
}
#[derive(Debug)]
struct TestSigner {
    key_id: String,
    keypair: KeyPair,
}
impl PopIssuerSigner for TestSigner {
    fn key_id(&self) -> &str {
        &self.key_id
    }
    fn public_key(&self) -> [u8; 32] {
        let (_, bytes) = self
            .keypair
            .public_key()
            .try_to_bytes()
            .expect("public key");
        bytes.try_into().expect("ed25519")
    }
    fn sign_digest(
        &self,
        _purpose: PopIssuerSigningPurposeV1,
        digest: [u8; 32],
    ) -> Result<[u8; 64], String> {
        Signature::try_new(self.keypair.private_key(), &digest)
            .map_err(|error| error.to_string())?
            .payload()
            .try_into()
            .map_err(|_| "signature length".to_owned())
    }
}
#[derive(Debug)]
struct TestWrapper {
    key_id: String,
    key: [u8; 32],
}
impl PopWalletKeyWrapper for TestWrapper {
    fn active_key_id(&self) -> &str {
        &self.key_id
    }
    fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
        Ok(dek
            .iter()
            .zip(self.key)
            .zip(context)
            .map(|((&byte, key), aad)| byte ^ key ^ aad)
            .collect())
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], String> {
        if key_id != self.key_id || wrapped_dek.len() != 32 {
            return Err("wrong key".to_owned());
        }
        let mut dek = [0; 32];
        for (index, output) in dek.iter_mut().enumerate() {
            *output = wrapped_dek[index] ^ self.key[index] ^ context[index];
        }
        Ok(dek)
    }
}
struct TestRecipient {
    key_id: String,
    secret: HybridSecretKey,
    public_key_digest: [u8; 32],
}
impl fmt::Debug for TestRecipient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestRecipient")
            .field("key_id", &self.key_id)
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}
impl PopEnrollmentRecipientV1 for TestRecipient {
    fn key_id(&self) -> &str {
        &self.key_id
    }
    fn public_key_digest(&self) -> [u8; 32] {
        self.public_key_digest
    }
    fn open_enrollment(
        &self,
        encrypted_payload: &HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
        decrypt_payload(encrypted_payload, aad, &self.secret)
            .map_err(|_| PopRecipientOpenErrorV1::Rejected)
    }
}
impl PopWalletRecipientV1 for TestRecipient {
    fn key_id(&self) -> &str {
        &self.key_id
    }
    fn public_key_digest(&self) -> [u8; 32] {
        self.public_key_digest
    }
    fn open_wallet_delivery(
        &self,
        encrypted_payload: &HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
        decrypt_payload(encrypted_payload, aad, &self.secret)
            .map_err(|_| PopRecipientOpenErrorV1::Rejected)
    }
}
fn test_recipient(key_id: &str, keypair: &HybridKeyPair) -> Arc<TestRecipient> {
    Arc::new(TestRecipient {
        key_id: key_id.to_owned(),
        secret: keypair.secret().clone(),
        public_key_digest: pop_enrollment_recipient_public_key_digest_v1(keypair.public()),
    })
}
fn ed25519(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("keypair")
}
#[test]
fn enrollment_recipient_public_key_digest_binds_both_hybrid_components() {
    let mut first_rng = ChaCha20Rng::from_seed([0x31; 32]);
    let first = HybridKeyPair::generate(&mut first_rng).expect("first hybrid key");
    let mut second_rng = ChaCha20Rng::from_seed([0x32; 32]);
    let second = HybridKeyPair::generate(&mut second_rng).expect("second hybrid key");
    let first_digest = pop_enrollment_recipient_public_key_digest_v1(first.public());
    assert_ne!(first_digest, [0; 32]);
    assert_eq!(
        first_digest,
        pop_enrollment_recipient_public_key_digest_v1(first.secret().public())
    );
    assert_ne!(
        first_digest,
        pop_enrollment_recipient_public_key_digest_v1(second.public())
    );
}
#[test]
fn recipient_capability_failures_map_without_provider_details() {
    assert_eq!(
        map_enrollment_recipient_error(PopRecipientOpenErrorV1::Unavailable),
        PopCredentialServiceError::RuntimeProviderUnavailable
    );
    assert_eq!(
        map_enrollment_recipient_error(PopRecipientOpenErrorV1::Rejected),
        PopCredentialServiceError::InvalidEnrollment
    );
    assert_eq!(
        map_wallet_recipient_error(PopRecipientOpenErrorV1::Unavailable),
        PopCredentialServiceError::RuntimeProviderUnavailable
    );
    assert_eq!(
        map_wallet_recipient_error(PopRecipientOpenErrorV1::Rejected),
        PopCredentialServiceError::Encryption
    );
}
fn public_key(keypair: &KeyPair) -> [u8; 32] {
    let (_, bytes) = keypair.public_key().try_to_bytes().expect("key");
    bytes.try_into().expect("ed25519")
}
fn policy(signer: &TestSigner, approvers: &[KeyPair]) -> PopCredentialServicePolicyV1 {
    PopCredentialServicePolicyV1 {
        version: POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1,
        issuer_policy_digest: [0x41; 32],
        issuer_id: "pop-issuer-sora-foundation".to_owned(),
        issuer_signer_handle: signer.key_id.clone(),
        issuer_public_key: signer.public_key(),
        enrollment_recipient_key_id: "kms://pop/enrollment/primary".to_owned(),
        approval_quorum: 2,
        approval_signers: approvers
            .iter()
            .enumerate()
            .map(|(index, key)| PopApprovalSignerV1 {
                signer_id: format!("approver-{index}"),
                public_key: public_key(key),
                revoked_at_epoch: None,
            })
            .collect(),
        max_pending_enrollments: 16,
        max_outbox_entries: 16,
        max_dead_letters: 16,
        max_seen_nullifiers: 16,
        max_submission_attempts: 2,
    }
}
fn scalar(value: u64) -> [u8; 32] {
    let mut output = [0; 32];
    output[..8].copy_from_slice(&value.to_le_bytes());
    output
}
fn nonce(value: u128) -> [u8; 32] {
    let mut output = [0; 32];
    output[..16].copy_from_slice(&value.to_le_bytes());
    output
}
fn private_enrollment(wallet: &HybridKeyPair) -> PopPrivateEnrollmentV1 {
    let attestation_payload = b"private biometric attestation".to_vec();
    let holder_commitment =
        derive_pop_holder_commitment_v1(scalar(0x1234), scalar(0x5678)).unwrap();
    PopPrivateEnrollmentV1 {
        request: PopEnrollmentRequestV1 {
            version: sorafs_manifest::POP_ENROLLMENT_REQUEST_VERSION_V1,
            request_id: [0x11; 32],
            applicant_id: "private-applicant-alias".to_owned(),
            requested_class: sorafs_manifest::PopEligibilityClassV1::General,
            requested_attributes: vec!["residency".to_owned()],
            attestation_digest: pop_enrollment_attestation_digest_v1(&attestation_payload),
            submitted_at_epoch: 10,
            expires_at_epoch: 100,
        },
        holder_commitment,
        wallet_x25519_public_key: wallet.public().x25519_bytes(),
        wallet_mlkem_public_key: wallet.public().kyber_bytes().to_vec(),
        attestation_payload,
    }
}
fn canonical_temp_root(temp: &TempDir) -> PathBuf {
    fs::canonicalize(temp.path()).expect("canonical temporary directory")
}
fn injected_sensitive_failure(bytes: &mut [u8]) -> Result<(), PopCredentialServiceError> {
    let _guard = SensitiveBytesGuard::new(bytes);
    Err(PopCredentialServiceError::Encryption)
}
#[test]
fn production_runtime_handles_use_canonical_grammar() {
    for handle in [
        "software://sorafs/pop-credentials/primary",
        "kms://sorafs/pop/wallet-primary",
    ] {
        assert_eq!(
            bounded_production_runtime_handle("runtime_handle", handle),
            Ok(())
        );
    }
    for handle in [
        "software://sorafs/pop-credentials/test",
        "kms://pop/mock/wallet",
        "kms://pop/placeholder/enrollment",
        "kms://pop/private key",
        "kms://pop/ключ",
        "software://sorafs/pop-credentials/operator@issuer",
        "software://sorafs/pop-credentials/primary?token",
        "software://sorafs/pop-credentials/primary#fragment",
        "software://sorafs/pop-credentials/%70rimary",
        "software://sorafs/pop-credentials/primary\\substituted",
    ] {
        assert!(matches!(
            bounded_production_runtime_handle("runtime_handle", handle),
            Err(PopCredentialServiceError::InvalidInput {
                field: "runtime_handle"
            })
        ));
    }
}
#[test]
fn sensitive_guard_scrubs_on_early_error() {
    let mut secret = vec![0xA5; 64];
    assert_eq!(
        injected_sensitive_failure(&mut secret),
        Err(PopCredentialServiceError::Encryption)
    );
    assert_eq!(secret, vec![0; 64]);
}
#[test]
fn private_membership_witness_guard_scrubs_all_secret_material() {
    let mut guard = PrivateMembershipWitnessGuard::new(PopMembershipWitnessV1 {
        holder_secret: [0xA1; 32],
        credential_path: PopCredentialMerklePathV1 {
            siblings: vec![[0xB2; 32], [0xC3; 32]],
            directions: vec![true, true],
        },
        revocation_path: PopRevocationNonMembershipPathV1 {
            siblings: vec![[0xD4; 32]],
        },
    });
    guard.zeroize();
    assert_eq!(guard.witness.holder_secret, [0; 32]);
    assert!(
        guard
            .witness
            .credential_path
            .siblings
            .iter()
            .flatten()
            .all(|byte| *byte == 0)
    );
    assert!(
        guard
            .witness
            .credential_path
            .directions
            .iter()
            .all(|direction| !direction)
    );
    assert!(
        guard
            .witness
            .revocation_path
            .siblings
            .iter()
            .flatten()
            .all(|byte| *byte == 0)
    );
}
fn approval(
    signer_id: &str,
    keypair: &KeyPair,
    envelope: &PopEncryptedEnrollmentV1,
    policy: &PopCredentialServicePolicyV1,
    decision: PopApprovalDecisionV1,
) -> PopApprovalV1 {
    let mut approval = PopApprovalV1 {
        version: POP_APPROVAL_VERSION_V1,
        request_id: envelope.request_id,
        enrollment_envelope_digest: envelope.digest().expect("digest"),
        issuer_policy_digest: policy.issuer_policy_digest,
        decision,
        decided_at_epoch: 20,
        signer_id: signer_id.to_owned(),
        signature: Vec::new(),
    };
    approval.signature = Signature::try_new(
        keypair.private_key(),
        &approval.signature_digest().expect("digest"),
    )
    .expect("sign")
    .payload()
    .to_vec();
    approval
}
fn service_fixture() -> (
    TempDir,
    PopCredentialService,
    PopCredentialServicePolicyV1,
    HybridKeyPair,
    Vec<KeyPair>,
    PopEncryptedEnrollmentV1,
) {
    let temp = TempDir::new().expect("temp");
    let signer = Arc::new(TestSigner {
        key_id: "software://sorafs/pop-credentials/primary".to_owned(),
        keypair: ed25519(1),
    });
    let approvers = vec![ed25519(2), ed25519(3), ed25519(4)];
    let policy = policy(&signer, &approvers);
    let mut rng = ChaCha20Rng::from_seed([0x21; 32]);
    let issuer_encryption = HybridKeyPair::generate(&mut rng).expect("issuer encryption");
    let wallet = HybridKeyPair::generate(&mut rng).expect("wallet encryption");
    let enrollment = encrypt_pop_enrollment_v1(
        &private_enrollment(&wallet),
        &policy,
        issuer_encryption.public(),
        &mut rng,
    )
    .expect("encrypt");
    let service = PopCredentialService::open(
        canonical_temp_root(&temp),
        policy.clone(),
        test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
        signer,
    )
    .expect("service");
    (temp, service, policy, wallet, approvers, enrollment)
}
include!("pop_credentials/issuer_checkpoint_security_tests.rs");
include!("pop_credentials/canonical_checkpoint_tests.rs");
fn empty_signature(key: [u8; 32]) -> PopSignatureV1 {
    PopSignatureV1 {
        algorithm: PopSignatureAlgorithmV1::Ed25519,
        public_key: key.to_vec(),
        signature: vec![1; 64],
    }
}
fn unsigned_root(key: [u8; 32], version: u64, previous: Option<[u8; 32]>) -> PopCommitmentRootV1 {
    PopCommitmentRootV1 {
        version: sorafs_manifest::POP_COMMITMENT_ROOT_VERSION_V1,
        root_digest: scalar(100 + version),
        tree_size: version,
        tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
        tree_version: version,
        issuer_id: "pop-issuer-sora-foundation".to_owned(),
        published_at_epoch: 30 + version,
        previous_root_digest: previous,
        governance_event_digest: [0x61; 32],
        publisher_signature: empty_signature(key),
    }
}
fn unsigned_revocations(key: [u8; 32], root: [u8; 32], version: u64) -> PopRevocationListV1 {
    PopRevocationListV1 {
        version: sorafs_manifest::POP_REVOCATION_LIST_VERSION_V1,
        list_version: version,
        commitment_root: root,
        revocation_root: sorafs_manifest::pop_credentials::pop_revocation_root_v1(&[])
            .expect("root"),
        revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
        issuer_id: "pop-issuer-sora-foundation".to_owned(),
        published_at_epoch: 30 + version,
        entries: Vec::new(),
        publisher_signature: empty_signature(key),
    }
}
fn projection(
    signer: &TestSigner,
    height: u64,
    previous_block_hash: Option<[u8; 32]>,
    root_version: u64,
    previous_root: Option<[u8; 32]>,
    policy_digest: [u8; 32],
) -> PopFinalizedRegistryProjectionV1 {
    let root = unsigned_root(signer.public_key(), root_version, previous_root);
    let revocations = unsigned_revocations(signer.public_key(), root.root_digest, root_version);
    let bundle = sign_bundle_with_signer(
        PopCredentialV1 {
            version: sorafs_manifest::POP_CREDENTIAL_VERSION_V1,
            credential_id: scalar(1),
            holder_commitment: scalar(2),
            eligibility_class: sorafs_manifest::PopEligibilityClassV1::General,
            attributes: Vec::new(),
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issued_at_epoch: 1,
            expires_at_epoch: 1000,
            renewal_at_epoch: 500,
            revocation_nonce: nonce(1),
            commitment_root: root.root_digest,
            commitment_tree_version: root.tree_version,
            revocation_list_version: revocations.list_version,
            issuer_signature: empty_signature(signer.public_key()),
        },
        root,
        revocations,
        signer,
    )
    .expect("sign");
    PopFinalizedRegistryProjectionV1 {
        version: POP_FINALIZED_REGISTRY_PROJECTION_VERSION_V1,
        cursor: PopFinalizedCursorV1 {
            block_height: height,
            block_hash: [height as u8; 32],
        },
        previous_block_hash,
        issuer_policy_digest: policy_digest,
        canonical_commitment_root: encode_canonical(&bundle.commitment_root).unwrap(),
        canonical_revocation_list: encode_canonical(&bundle.revocation_list).unwrap(),
        committed_operation_digests: Vec::new(),
        rejected_operation_digests: Vec::new(),
        revoked_issuer_public_keys: Vec::new(),
    }
}
#[test]
fn finalized_sync_rejects_cursor_root_rollback_and_wrong_policy() {
    let signer = TestSigner {
        key_id: "software://sorafs/pop-credentials/primary".to_owned(),
        keypair: ed25519(1),
    };
    let policy = policy(&signer, &[ed25519(2), ed25519(3)]);
    let first = projection(&signer, 1, None, 2, None, policy.issuer_policy_digest);
    validate_projection(None, &first, &policy).expect("first");
    let mut wrong_policy = first.clone();
    wrong_policy.issuer_policy_digest = [0x99; 32];
    assert_eq!(
        validate_projection(None, &wrong_policy, &policy),
        Err(PopCredentialServiceError::WrongPolicy)
    );
    let rollback = projection(
        &signer,
        2,
        Some(first.cursor.block_hash),
        1,
        None,
        policy.issuer_policy_digest,
    );
    assert_eq!(
        validate_projection(Some(&first), &rollback, &policy),
        Err(PopCredentialServiceError::RootRollback)
    );
    let fork = projection(
        &signer,
        2,
        Some([0xFF; 32]),
        3,
        Some(
            decode_canonical::<PopCommitmentRootV1>(
                &first.canonical_commitment_root,
                1_000_000,
                100,
            )
            .unwrap()
            .root_digest,
        ),
        policy.issuer_policy_digest,
    );
    assert_eq!(
        validate_projection(Some(&first), &fork, &policy),
        Err(PopCredentialServiceError::RootRollback)
    );
}
#[test]
fn wallet_vault_rejects_symlink_and_wrong_wrapping_key() {
    let temp = TempDir::new().unwrap();
    let mut recipient_rng = ChaCha20Rng::from_seed([0x77; 32]);
    let recipient_key = HybridKeyPair::generate(&mut recipient_rng).expect("wallet recipient key");
    let recipient = test_recipient("kms://wallet/recipient-one", &recipient_key);
    let wrapper = Arc::new(TestWrapper {
        key_id: "kms://wallet/one".to_owned(),
        key: [7; 32],
    });
    let vault =
        PopWalletVault::open(canonical_temp_root(&temp), recipient.clone(), wrapper).unwrap();
    let target = temp.path().join("outside");
    fs::write(&target, b"sentinel").unwrap();
    let credential = [0xAB; 32];
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, vault.credential_path(credential)).unwrap();
    let private = PopWalletVaultPlaintextV1 {
        bundle: PopIssuedCredentialBundleV1 {
            version: sorafs_manifest::POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1,
            credential: PopCredentialV1 {
                version: 0,
                credential_id: [0; 32],
                holder_commitment: [0; 32],
                eligibility_class: sorafs_manifest::PopEligibilityClassV1::General,
                attributes: Vec::new(),
                issuer_id: String::new(),
                issued_at_epoch: 0,
                expires_at_epoch: 0,
                renewal_at_epoch: 0,
                revocation_nonce: [0; 32],
                commitment_root: [0; 32],
                commitment_tree_version: 0,
                revocation_list_version: 0,
                issuer_signature: empty_signature([1; 32]),
            },
            commitment_root: unsigned_root([1; 32], 1, None),
            revocation_list: unsigned_revocations([1; 32], scalar(101), 1),
        },
        witness: PopPrivateWitnessEnvelopeV1 {
            holder_secret: [1; 32],
            credential_siblings: vec![[0; 32]; usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
            credential_directions: vec![false; usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
            revocation_siblings: vec![[0; 32]; usize::from(POP_REVOCATION_TREE_DEPTH_V1)],
        },
        finalized_operation_digest: [1; 32],
        witness_commitment_root: scalar(101),
        witness_commitment_tree_version: 1,
        active_revocation_list: unsigned_revocations([1; 32], scalar(101), 1),
    };
    #[cfg(unix)]
    assert_eq!(
        vault.persist_credential(credential, &private),
        Err(PopCredentialServiceError::CheckpointIo)
    );
    assert_eq!(fs::read(target).unwrap(), b"sentinel");
    #[cfg(unix)]
    fs::remove_file(vault.credential_path(credential)).unwrap();
    vault
        .persist_credential(credential, &private)
        .expect("encrypted vault");
    let wrong_wrapper = Arc::new(TestWrapper {
        key_id: "kms://wallet/two".to_owned(),
        key: [8; 32],
    });
    let wrong_vault =
        PopWalletVault::open(canonical_temp_root(&temp), recipient, wrong_wrapper).unwrap();
    assert_eq!(
        wrong_vault.load_credential(credential),
        Err(PopCredentialServiceError::KeyWrapping)
    );
}
#[test]
fn sensitive_struct_debug_is_redacted() {
    let mut rng = ChaCha20Rng::from_seed([0x23; 32]);
    let wallet = HybridKeyPair::generate(&mut rng).unwrap();
    let private = private_enrollment(&wallet);
    let rendered = format!("{private:?}");
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains("private-applicant-alias"));
    assert!(!rendered.contains("biometric"));
}
