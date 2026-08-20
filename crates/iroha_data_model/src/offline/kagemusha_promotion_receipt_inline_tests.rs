#[cfg(feature = "transparent_api")]
use crate::{
    Level,
    account::{MultisigMember, MultisigPolicy},
    block::{
        BlockHeader, BlockSignature, SignedBlock,
        consensus_v2::{
            BlockSubject, ConsensusRound, ExecutionCommitment, PayloadEncoding, Vote,
            finality::V2FinalityArtifact,
        },
    },
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
    peer::PeerId,
    prelude::Log,
    query::CommittedTransaction,
    transaction::{
        DataTriggerSequence, FeePaymentIntent, SignedTransaction, TransactionBuilder,
        TransactionEntrypoint, TransactionResult, TransactionResultInner,
    },
};
#[cfg(feature = "transparent_api")]
use iroha_crypto::{MerkleTree, Signature};
#[cfg(feature = "transparent_api")]
use std::num::NonZeroU64;

#[cfg(feature = "transparent_api")]
fn exact_receipt_bytes(bytes: &[u8]) -> KagemushaExactBytesDigestV1 {
    KagemushaExactBytesDigestV1::from_bytes(bytes).expect("non-empty exact byte identity")
}

#[cfg(feature = "transparent_api")]
fn release_bound_verifier_record(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
    key: VerifyingKeyBox,
) -> (VerifyingKeyId, VerifyingKeyRecord) {
    let manifest_sha256 = manifest.canonical_sha256().expect("manifest identity");
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .expect("requested verifier profile");
    let curve = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4,
    };
    let key_id = kagemusha_recursive_spend_verifier_key_id_v4(parity, manifest_sha256);
    let mut record = VerifyingKeyRecord::new_with_owner(
        7,
        profile.circuit_id.clone(),
        Some(kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
            manifest_sha256,
        )),
        KAGEMUSHA_VERIFIER_NAMESPACE,
        BackendTag::Halo2IpaPasta,
        curve,
        kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4(manifest, parity)
            .expect("public-input schema identity"),
        verifying_key_commitment_v1(&key).expect("verifier commitment"),
    );
    record.vk_len = u32::try_from(key.bytes.len()).expect("small verifier fixture");
    record.max_proof_bytes = manifest.max_proof_bytes;
    record.activation_height = Some(manifest.activation_height);
    record.key = Some(key);
    record.status = ConfidentialStatus::Active;
    (key_id, record)
}

#[cfg(feature = "transparent_api")]
fn valid_release_activation_fixture() -> (
    KagemushaRecursiveSpendReleaseActivationV4,
    OfflineDeviceAttestationPolicy,
    Vec<u8>,
) {
    let mut activation = release_activation_wire_fixture();
    let policy = device_attestation_policy_wire_fixture();
    let release_policy_source = b"wire-bound release policy".to_vec();
    let eq_key = VerifyingKeyBox::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        b"receipt Eq verifier key".to_vec(),
    );
    let ep_key = VerifyingKeyBox::new(
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        b"receipt Ep verifier key".to_vec(),
    );
    for (parity, key) in [
        (KagemushaPastaCycleParityV1::StepEq, &eq_key),
        (KagemushaPastaCycleParityV1::StepEp, &ep_key),
    ] {
        let profile = activation
            .release_record
            .manifest
            .profiles
            .iter_mut()
            .find(|profile| profile.parity == parity)
            .expect("verifier profile");
        let descriptor = profile
            .artifacts
            .get_mut(2)
            .expect("verifier artifact descriptor");
        descriptor.payload_size_bytes =
            u64::try_from(key.bytes.len()).expect("small verifier fixture");
        descriptor.size_bytes = descriptor.payload_size_bytes + 256;
        descriptor.payload_sha256 = digest(&key.bytes);
    }

    let candidate = unsigned_candidate(&activation.release_record.manifest);
    let candidate_sha256 = candidate.sha256().expect("candidate identity");
    activation
        .release_record
        .manifest
        .qualified_candidate_sha256 = kagemusha_recursive_spend_qualified_candidate_sha256_v4(
        candidate_sha256,
        activation
            .release_record
            .manifest
            .qualification_receipt_sha256,
    );
    let reviewer = KeyPair::from_seed(vec![0x79; 32], Algorithm::Ed25519);
    activation.release_record.cryptographic_review_summary =
        signed_review_bytes(&candidate, &[&reviewer]);
    activation.release_record.manifest.benchmark_evidence_sha256 =
        digest(&activation.release_record.physical_device_benchmark_summary);
    activation
        .release_record
        .manifest
        .cryptographic_review_sha256 =
        digest(&activation.release_record.cryptographic_review_summary);
    activation.release_record.release_attestation.subject = activation
        .release_record
        .manifest
        .release_attestation_subject()
        .expect("release attestation subject");
    activation
        .release_record
        .manifest
        .release_attestation_sha256 = digest(
        &norito::encode_canonical(&activation.release_record.release_attestation)
            .expect("canonical release attestation"),
    );
    activation.release_record.promotion_record.candidate_sha256 = candidate_sha256;
    activation
        .release_record
        .promotion_record
        .qualified_candidate_sha256 = activation
        .release_record
        .manifest
        .qualified_candidate_sha256;
    activation.release_record.promotion_record.manifest_sha256 = activation
        .release_record
        .manifest
        .canonical_sha256()
        .expect("final manifest identity");
    activation
        .release_record
        .promotion_record
        .release_attestation_sha256 = activation
        .release_record
        .manifest
        .release_attestation_sha256;
    activation
        .release_record
        .promotion_record
        .release_policy_sha256 = digest(&release_policy_source);
    activation.configured_policy_sha256 = digest(&release_policy_source);
    let (eq_id, eq_record) = release_bound_verifier_record(
        &activation.release_record.manifest,
        KagemushaPastaCycleParityV1::StepEq,
        eq_key,
    );
    let (ep_id, ep_record) = release_bound_verifier_record(
        &activation.release_record.manifest,
        KagemushaPastaCycleParityV1::StepEp,
        ep_key,
    );
    activation.step_eq_verifier_key_id = eq_id;
    activation.step_eq_verifier_record = eq_record;
    activation.step_ep_verifier_key_id = ep_id;
    activation.step_ep_verifier_record = ep_record;
    activation
        .validate_structure()
        .expect("complete release activation fixture");
    (activation, policy, release_policy_source)
}

#[cfg(feature = "transparent_api")]
fn receipt_binding(
    activation: &KagemushaRecursiveSpendReleaseActivationV4,
    policy: &OfflineDeviceAttestationPolicy,
    release_policy_source: &[u8],
) -> KagemushaV4PromotionBindingV1 {
    let manifest = &activation.release_record.manifest;
    KagemushaV4PromotionBindingV1 {
        promotion_id: digest(b"complete receipt promotion id"),
        network_id: manifest.network_id,
        reviewed_source_closure_descriptor_sha256: manifest
            .reviewed_source_closure_descriptor_sha256,
        manifest_sha256: manifest.canonical_sha256().expect("manifest identity"),
        release_record_sha256: digest(
            &norito::encode_canonical(&activation.release_record)
                .expect("canonical release record"),
        ),
        release_policy_source: exact_receipt_bytes(release_policy_source),
        device_attestation_policy_norito: exact_receipt_bytes(
            &norito::encode_canonical(policy).expect("canonical device policy"),
        ),
        signed_genesis: exact_receipt_bytes(b"ordinary genesis-rooted signed genesis body"),
        catalog_consensus_policy_digest: digest(b"complete ordered Kagemusha catalog"),
        execution_policy_hash: Hash::new(b"complete aggregate execution policy"),
    }
}

#[cfg(feature = "transparent_api")]
fn qualified_receipt_hosts(
    binding: &KagemushaV4PromotionBindingV1,
) -> (
    [KagemushaV4ValidatorQualificationSealBodyV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    Vec<KeyPair>,
) {
    let mut keys = [0x81_u8, 0x82, 0x83, 0x84]
        .into_iter()
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let bodies: [KagemushaV4ValidatorQualificationSealBodyV1;
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] = keys
        .iter()
        .enumerate()
        .map(|(index, key)| KagemushaV4ValidatorQualificationSealBodyV1 {
            schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            binding: binding.clone(),
            validator_id: PeerId::new(key.public_key().clone()),
            iroha3d_executable: exact_receipt_bytes(
                format!("validator-{index}-iroha3d").as_bytes(),
            ),
            flattened_toml_config_source: exact_receipt_bytes(
                format!("validator-{index}-flattened-toml-source").as_bytes(),
            ),
            catalog_qualification_seal: exact_receipt_bytes(
                format!("validator-{index}-catalog-qualification-seal").as_bytes(),
            ),
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("exactly four validator bodies");
    let seals: [KagemushaV4ValidatorQualificationSealV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT] =
        bodies
            .iter()
            .zip(&keys)
            .map(
                |(body, key): (&KagemushaV4ValidatorQualificationSealBodyV1, &KeyPair)| {
                    KagemushaV4ValidatorQualificationSealV1::try_sign(body.clone(), key)
                        .expect("validator qualification signature")
                },
            )
            .collect::<Vec<_>>()
            .try_into()
            .expect("exactly four validator seals");
    (bodies, seals, keys)
}

#[cfg(feature = "transparent_api")]
fn finalized_receipt_proof(
    block: &SignedBlock,
    binding: &KagemushaV4PromotionBindingV1,
    validator_keys: &[KeyPair],
    execution_policy_hash: Hash,
) -> BridgeFinalityProof {
    let roster = validator_keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = HeightContext {
        network_id: binding.network_id,
        protocol_version: PROTOCOL_VERSION,
        height: block.header().height().get(),
        epoch: 0,
        epoch_end_height: u64::MAX,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("four-validator quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"complete receipt nexus context"),
        execution_policy_hash,
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xB4; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("canonical proposal wire identity"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: block.header().view_change_index(),
    };
    let block_wire = block.encode_wire().expect("result-bearing block wire");
    let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"complete receipt parent state"),
        Hash::new(b"complete receipt post state"),
        Hash::new(b"complete receipt ordinary writes"),
        u64::try_from(block_wire.len()).expect("small block wire"),
        Hash::new(&block_wire),
    );
    let preimage = Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = validator_keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .expect("CommitQC signature")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate CommitQC signatures"),
    };
    let pops = validator_keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP"))
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, pops);
    artifact.verify().expect("valid finality artifact");
    artifact
        .validate_for_header(&block.header())
        .expect("finality authenticates block header");
    BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: block.header(),
        finality_artifact: artifact,
    }
}

#[cfg(feature = "transparent_api")]
struct CompleteReceiptFixture {
    receipt: KagemushaV4ActivationFinalityReceiptV1,
    expectations: KagemushaV4ActivationReceiptExpectationsV1,
    block: SignedBlock,
    issuer: KeyPair,
    validator_keys: Vec<KeyPair>,
    approved_transaction: SignedTransaction,
}

#[cfg(feature = "transparent_api")]
fn governance_receipt_fixture() -> ([KeyPair; 3], MultisigPolicy, AccountId) {
    let keys =
        [0x91_u8, 0x92, 0x94].map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
    let policy = MultisigPolicy::new(
        2,
        keys.iter()
            .map(|key| {
                MultisigMember::new(key.public_key().clone(), 1).expect("valid governance member")
            })
            .collect(),
    )
    .expect("canonical two-of-three governance policy");
    let authority = AccountId::new_multisig(policy.clone());
    (keys, policy, authority)
}

#[cfg(feature = "transparent_api")]
fn receipt_block(
    carrier_transaction: &SignedTransaction,
    committed_transaction: &SignedTransaction,
    block_signer: &KeyPair,
    result_inner: TransactionResultInner,
    creation_time_ms: u64,
) -> (SignedBlock, CommittedTransaction) {
    let entrypoint_hash = carrier_transaction.hash_as_entrypoint();
    assert_eq!(
        entrypoint_hash,
        committed_transaction.hash_as_entrypoint(),
        "carrier and committed transaction must have the same approved intent",
    );
    let entrypoint_tree = MerkleTree::from_iter([entrypoint_hash]);
    let result_tree = MerkleTree::from_iter([TransactionResult::hash_from_inner(&result_inner)]);
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero fixture height"),
        None,
        entrypoint_tree.root(),
        result_tree.root(),
        creation_time_ms,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_signer.private_key(), header.hash())
            .expect("roster-validator block signature"),
    );
    let mut block = SignedBlock::presigned(signature, header, vec![carrier_transaction.clone()]);
    block
        .set_transaction_results(Vec::new(), &[entrypoint_hash], vec![result_inner])
        .expect("aligned result-bearing block with precomputed roots");
    let proofs = block
        .proofs_for_entry_hash(&entrypoint_hash)
        .expect("entrypoint and result proofs");
    let result = block
        .results()
        .next()
        .expect("one transaction result")
        .clone();
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash,
        entrypoint_proof: proofs.entry_proof.proof().clone(),
        entrypoint: TransactionEntrypoint::External(committed_transaction.clone()),
        result_hash: result.hash(),
        result_proof: proofs.result_proof.proof().clone(),
        result,
        merge_inclusion: None,
    };
    (block, committed)
}

#[cfg(feature = "transparent_api")]
fn complete_receipt_fixture_with_options(
    direct_activation: bool,
    alternate_authorization: bool,
    failed_result: bool,
) -> CompleteReceiptFixture {
    let (activation, policy, release_policy_source) = valid_release_activation_fixture();
    let binding = receipt_binding(&activation, &policy, &release_policy_source);
    let (validator_bodies, validator_seals, validator_keys) = qualified_receipt_hosts(&binding);
    let (governance_keys, governance_policy, governance_authority) = governance_receipt_fixture();
    let transaction_builder = if direct_activation {
        TransactionBuilder::new(
            binding.network_id,
            governance_authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([ActivateKagemushaRecursiveReleaseV4::new(activation, policy)])
    } else {
        TransactionBuilder::new(
            binding.network_id,
            governance_authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "not a governed Kagemusha activation".into(),
        )])
    };
    let approved_transaction = transaction_builder.clone().sign_multisig([
        governance_keys[0].private_key(),
        governance_keys[1].private_key(),
    ]);
    let carrier_transaction = if alternate_authorization {
        transaction_builder.sign_multisig([
            governance_keys[0].private_key(),
            governance_keys[2].private_key(),
        ])
    } else {
        approved_transaction.clone()
    };
    let result_inner = if failed_result {
        TransactionResultInner::Err(
            crate::transaction::error::TransactionRejectionReason::Validation(
                crate::ValidationFail::InternalError("receipt fixture rejection".into()),
            ),
        )
    } else {
        TransactionResultInner::Ok(DataTriggerSequence::default())
    };
    let (block, committed_transaction) = receipt_block(
        &carrier_transaction,
        &approved_transaction,
        &validator_keys[0],
        result_inner,
        0,
    );
    let finality_proof = finalized_receipt_proof(
        &block,
        &binding,
        &validator_keys,
        binding.execution_policy_hash,
    );
    let block_wire = block.encode_wire().expect("result-bearing block wire");
    let transaction_wire = approved_transaction
        .encode_wire_v1()
        .expect("authorization-bearing transaction wire");
    let issuer = KeyPair::from_seed(vec![0x93; 32], Algorithm::Ed25519);
    let body = KagemushaV4ActivationFinalityReceiptBodyV1 {
        schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        binding: binding.clone(),
        issuer: issuer.public_key().clone(),
        governance_authority: governance_authority.clone(),
        validator_seals,
        activation_transaction_intent: approved_transaction.hash(),
        activation_transaction_wire: exact_receipt_bytes(&transaction_wire),
        committed_transaction,
        finalized_block_wire: KagemushaFinalizedBlockWireV1::try_from_bytes(block_wire.clone())
            .expect("bounded block wire"),
        finalized_block_wire_digest: exact_receipt_bytes(&block_wire),
        finality_proof,
    };
    let receipt = KagemushaV4ActivationFinalityReceiptV1::try_sign(body, &issuer)
        .expect("signed complete receipt");
    let expectations = KagemushaV4ActivationReceiptExpectationsV1 {
        binding,
        receipt_issuer: issuer.public_key().clone(),
        governance_authority,
        governance_multisig_policy: governance_policy,
        validator_bodies,
        activation_transaction_intent: approved_transaction.hash(),
        activation_transaction_wire: exact_receipt_bytes(&transaction_wire),
        activation_height_context: receipt.body.finality_proof.finality_artifact.context_id(),
    };
    CompleteReceiptFixture {
        receipt,
        expectations,
        block,
        issuer,
        validator_keys,
        approved_transaction,
    }
}

#[cfg(feature = "transparent_api")]
fn complete_receipt_fixture(direct_activation: bool) -> CompleteReceiptFixture {
    complete_receipt_fixture_with_options(direct_activation, false, false)
}

#[cfg(feature = "transparent_api")]
#[test]
fn complete_activation_receipt_verifies_and_rejects_block_finality_policy_and_tx_splices() {
    let fixture = complete_receipt_fixture(true);
    let verified = fixture
        .receipt
        .verify(&fixture.expectations)
        .expect("complete activation receipt verifies");
    assert_eq!(verified.finalized_block_hash, fixture.block.hash());
    assert_eq!(
        verified.activation_transaction_intent,
        fixture.expectations.activation_transaction_intent
    );

    let canonical = norito::encode_canonical(&fixture.receipt).expect("canonical receipt");
    let decoded = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(&canonical)
        .expect("bounded canonical receipt");
    decoded
        .verify(&fixture.expectations)
        .expect("decoded complete receipt verifies");

    let log_fixture = complete_receipt_fixture(false);
    let mut substituted_block = fixture.receipt.body.clone();
    substituted_block.finalized_block_wire = log_fixture.receipt.body.finalized_block_wire.clone();
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(substituted_block, &fixture.issuer),
        Err(KagemushaPromotionReceiptValidationError::BlockWireDigest),
    );

    let mut bad_finality = fixture.receipt.body.clone();
    bad_finality
        .finality_proof
        .finality_artifact
        .commit_qc
        .aggregate_signature[0] ^= 0x80;
    let bad_finality =
        KagemushaV4ActivationFinalityReceiptV1::try_sign(bad_finality, &fixture.issuer)
            .expect("receipt issuer may attest hostile finality for verifier regression");
    assert_eq!(
        bad_finality.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::Finality),
    );

    let mut policy_splice = fixture.receipt.body.clone();
    policy_splice.finality_proof = finalized_receipt_proof(
        &fixture.block,
        &fixture.expectations.binding,
        &fixture.validator_keys,
        Hash::new(b"different aggregate execution policy"),
    );
    let policy_splice =
        KagemushaV4ActivationFinalityReceiptV1::try_sign(policy_splice, &fixture.issuer)
            .expect("receipt issuer may attest hostile policy context for verifier regression");
    assert_eq!(
        policy_splice.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::FinalityRoster),
    );

    assert_eq!(
        log_fixture.receipt.verify(&log_fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::ActivationTransaction),
        "a fully finalized successful non-activation transaction must still fail closed",
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn activation_receipt_rejects_authorization_result_context_roster_and_block_splices() {
    let fixture = complete_receipt_fixture(true);

    let authorization_splice = complete_receipt_fixture_with_options(true, true, false);
    assert_eq!(
        authorization_splice
            .receipt
            .verify(&authorization_splice.expectations),
        Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire),
        "the finalized block must carry the exact approved multisig bundle, not only its intent",
    );

    let failed_result = complete_receipt_fixture_with_options(true, false, true);
    assert_eq!(
        failed_result.receipt.verify(&failed_result.expectations),
        Err(KagemushaPromotionReceiptValidationError::CommittedTransaction),
    );

    let mut stale_context = fixture.expectations.clone();
    stale_context.activation_height_context =
        HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"stale externally trusted height context",
        )));
    assert_eq!(
        fixture.receipt.verify(&stale_context),
        Err(KagemushaPromotionReceiptValidationError::Finality),
    );

    let mut substituted_validator_keys = [0xB1_u8, 0xB2, 0xB3, 0xB4]
        .into_iter()
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    substituted_validator_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let mut substituted_roster_body = fixture.receipt.body.clone();
    substituted_roster_body.finality_proof = finalized_receipt_proof(
        &fixture.block,
        &fixture.expectations.binding,
        &substituted_validator_keys,
        fixture.expectations.binding.execution_policy_hash,
    );
    let substituted_roster =
        KagemushaV4ActivationFinalityReceiptV1::try_sign(substituted_roster_body, &fixture.issuer)
            .expect("receipt issuer may attest a hostile roster for verifier regression");
    assert_eq!(
        substituted_roster.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::FinalityRoster),
    );

    let (different_block, different_committed) = receipt_block(
        &fixture.approved_transaction,
        &fixture.approved_transaction,
        &fixture.validator_keys[0],
        TransactionResultInner::Ok(DataTriggerSequence::default()),
        1,
    );
    assert_ne!(different_block.hash(), fixture.block.hash());
    let different_block_wire = different_block
        .encode_wire()
        .expect("alternate coherent result-bearing block wire");
    let mut mismatched_block_body = fixture.receipt.body.clone();
    mismatched_block_body.committed_transaction = different_committed;
    mismatched_block_body.finalized_block_wire =
        KagemushaFinalizedBlockWireV1::try_from_bytes(different_block_wire.clone())
            .expect("bounded alternate block wire");
    mismatched_block_body.finalized_block_wire_digest = exact_receipt_bytes(&different_block_wire);
    let mismatched_block =
        KagemushaV4ActivationFinalityReceiptV1::try_sign(mismatched_block_body, &fixture.issuer)
            .expect("receipt issuer may attest a block/finality splice for verifier regression");
    assert_eq!(
        mismatched_block.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::FinalityBlockBinding),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn activation_receipt_requires_the_exact_strong_governance_multisig() {
    let fixture = complete_receipt_fixture(true);
    let single_key = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
    let single_authority = AccountId::new(single_key.public_key().clone());

    let mut single_body = fixture.receipt.body.clone();
    single_body.governance_authority = single_authority.clone();
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(single_body, &fixture.issuer),
        Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority),
    );
    let mut single_expectations = fixture.expectations.clone();
    single_expectations.governance_authority = single_authority;
    assert_eq!(
        fixture.receipt.verify(&single_expectations),
        Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority),
    );

    let weak_keys =
        [0xC2_u8, 0xC3].map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
    let weak_policy = MultisigPolicy::new(
        1,
        weak_keys
            .iter()
            .map(|key| {
                MultisigMember::new(key.public_key().clone(), 1).expect("valid weak-policy member")
            })
            .collect(),
    )
    .expect("canonical but insufficient one-of-two policy");
    let mut weak_expectations = fixture.expectations.clone();
    weak_expectations.governance_authority = AccountId::new_multisig(weak_policy.clone());
    weak_expectations.governance_multisig_policy = weak_policy;
    assert_eq!(
        fixture.receipt.verify(&weak_expectations),
        Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority),
    );

    let (_, different_policy, _) = {
        let keys = [0xC4_u8, 0xC5, 0xC6]
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
        let policy = MultisigPolicy::new(
            2,
            keys.iter()
                .map(|key| {
                    MultisigMember::new(key.public_key().clone(), 1)
                        .expect("valid alternate governance member")
                })
                .collect(),
        )
        .expect("alternate strong policy");
        (keys, policy.clone(), AccountId::new_multisig(policy))
    };
    let mut substituted_policy = fixture.expectations.clone();
    substituted_policy.governance_multisig_policy = different_policy;
    assert_eq!(
        fixture.receipt.verify(&substituted_policy),
        Err(KagemushaPromotionReceiptValidationError::GovernanceAuthority),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn receipt_constructor_rejects_signature_algorithms_the_verifier_cannot_admit() {
    let fixture = complete_receipt_fixture(true);
    let unsupported = KeyPair::from_seed(vec![0xA1; 32], Algorithm::Secp256k1);
    let mut body = fixture.receipt.body;
    body.issuer = unsupported.public_key().clone();
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(body, &unsupported),
        Err(KagemushaPromotionReceiptValidationError::InvalidField(
            "activation_receipt.issuer",
        )),
    );
}
