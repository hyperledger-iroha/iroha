#[cfg(feature = "transparent_api")]
use crate::{
    Level,
    account::{MultisigMember, MultisigPolicy},
    block::{
        BlockHeader, BlockSignature, SignedBlock,
        consensus_v2::{
            BlockSubject, ConsensusRound, ExecutionCommitment, Vote, finality::V2FinalityArtifact,
        },
    },
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
    metadata::Metadata,
    peer::PeerId,
    prelude::Log,
    query::CommittedTransaction,
    transaction::{
        DataTriggerSequence, FeePaymentIntent, SignedTransaction, TransactionBuilder,
        TransactionEntrypoint, TransactionResult, TransactionResultInner,
    },
};
#[cfg(feature = "transparent_api")]
use iroha_crypto::{HashOf, MerkleTree, Signature};
#[cfg(feature = "transparent_api")]
use iroha_primitives::json::Json;
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
    promotion_controller: PublicKey,
) -> KagemushaV4PromotionBindingV1 {
    let manifest = &activation.release_record.manifest;
    KagemushaV4PromotionBindingV1 {
        promotion_controller,
        promotion_reservation: exact_receipt_bytes(b"pending signed promotion reservation"),
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
    let validators = keys
        .iter()
        .enumerate()
        .map(|(index, key)| KagemushaV4RuntimeValidatorProjectionV1 {
            validator_id: PeerId::new(key.public_key().clone()),
            public_address: format!("127.0.0.1:{}", 15_000 + index)
                .parse()
                .expect("fixture validator address"),
            bls_pop: iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator PoP"),
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("exactly four runtime validators");
    let runtime_effective_config = KagemushaV4RuntimeEffectiveConfigProjectionV1 {
        chain: crate::ChainId::from("kagemusha-complete-receipt-test"),
        chain_discriminant: 42,
        is_validator: true,
        genesis_public_key: KeyPair::from_seed(vec![0x80; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
        genesis_expected_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"genesis header",
        )),
        validators,
        sumeragi_config_fingerprint: Hash::new(b"effective Sumeragi V2 config"),
        genesis_context: crate::block::consensus_v2::SumeragiV2GenesisContextParameters {
            execution_policy_hash: *binding.execution_policy_hash.as_ref(),
            ..crate::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended()
        },
        kagemusha_max_decoded_bytes: 64 * 1024 * 1024,
    };
    runtime_effective_config
        .validate()
        .expect("valid runtime-effective config");
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
            runtime_effective_config: runtime_effective_config.clone(),
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
    parent: Option<&BridgeFinalityProof>,
) -> BridgeFinalityProof {
    let genesis_context =
        crate::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended();
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
        parent_commit_qc: parent.map(|proof| proof.finality_artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("four-validator quorum"),
        roster,
        nexus_amx_context_hash: Hash::prehashed(genesis_context.nexus_amx_context_hash),
        execution_policy_hash,
        da_layout: genesis_context.da_layout,
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
    expectations_artifact: KagemushaV4ActivationReceiptExpectationsArtifactV1,
    expectations_artifact_bytes: Vec<u8>,
    promotion_reservation: KagemushaV4PromotionReservationV1,
    promotion_reservation_bytes: Vec<u8>,
    promotion_controller: KeyPair,
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
    height: u64,
    previous_block_hash: Option<HashOf<BlockHeader>>,
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
        NonZeroU64::new(height).expect("non-zero fixture height"),
        previous_block_hash,
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
fn receipt_anchor_block(block_signer: &KeyPair) -> SignedBlock {
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero anchor height"),
        None,
        None,
        None,
        0,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_signer.private_key(), header.hash())
            .expect("roster-validator anchor block signature"),
    );
    SignedBlock::presigned(signature, header, Vec::new())
}

#[cfg(feature = "transparent_api")]
struct CompleteReceiptOptions {
    direct_activation: bool,
    alternate_authorization: bool,
    failed_result: bool,
    instruction_promotion_id: Option<[u8; 32]>,
    expires_at_height: Option<u64>,
}

#[cfg(feature = "transparent_api")]
impl Default for CompleteReceiptOptions {
    fn default() -> Self {
        Self {
            direct_activation: true,
            alternate_authorization: false,
            failed_result: false,
            instruction_promotion_id: None,
            expires_at_height: Some(3),
        }
    }
}

#[cfg(feature = "transparent_api")]
fn sign_expectations_body_unchecked(
    body: KagemushaV4ActivationReceiptExpectationsBodyV1,
    controller: &KeyPair,
) -> KagemushaV4ActivationReceiptExpectationsArtifactV1 {
    let signature = SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
        .expect("test controller can sign expectations body");
    KagemushaV4ActivationReceiptExpectationsArtifactV1 {
        schema: KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        body,
        signature,
    }
}

#[cfg(feature = "transparent_api")]
fn complete_receipt_fixture_with_options(
    options: CompleteReceiptOptions,
) -> CompleteReceiptFixture {
    let (activation, policy, release_policy_source) = valid_release_activation_fixture();
    let promotion_controller = KeyPair::from_seed(vec![0x90; 32], Algorithm::Ed25519);
    let mut binding = receipt_binding(
        &activation,
        &policy,
        &release_policy_source,
        promotion_controller.public_key().clone(),
    );
    let github_run = KagemushaV4GitHubPromotionRunV1 {
        repository: "hyperledger-iroha/iroha".to_owned(),
        workflow_ref:
            "hyperledger-iroha/iroha/.github/workflows/promote_kagemusha_v4.yml@refs/heads/main"
                .to_owned(),
        workflow_sha: [0x4a; 20],
        run_id: 1_234_567,
        run_attempt: 2,
    };
    binding.promotion_id = github_run.promotion_id();
    let source_descriptor = activation
        .release_record
        .manifest
        .reviewed_source_closure
        .canonical_descriptor_bytes()
        .expect("reviewed source descriptor bytes");
    let promotion_record_norito =
        norito::encode_canonical(&activation.release_record.promotion_record)
            .expect("canonical promotion record");
    let promotion_reservation = KagemushaV4PromotionReservationV1::try_sign(
        KagemushaV4PromotionReservationBodyV1 {
            schema: KAGEMUSHA_V4_PROMOTION_RESERVATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_controller: promotion_controller.public_key().clone(),
            github_run,
            promotion_id: binding.promotion_id,
            network_id: binding.network_id,
            reviewed_source_closure_descriptor: exact_receipt_bytes(&source_descriptor),
            manifest_sha256: binding.manifest_sha256,
            release_record_sha256: binding.release_record_sha256,
            promotion_record_norito: exact_receipt_bytes(&promotion_record_norito),
            release_policy_source: binding.release_policy_source,
            signed_genesis: binding.signed_genesis,
            catalog_revalidation_receipt_json: exact_receipt_bytes(
                b"{\"schema\":\"fixture.catalog_revalidation.v1\",\"valid\":true}\n",
            ),
            catalog_revalidation_catalog_sha256: digest(
                b"fixture App-Attest catalog revalidation bindings",
            ),
            catalog_consensus_policy_digest: binding.catalog_consensus_policy_digest,
            execution_policy_hash: binding.execution_policy_hash,
            device_attestation_policy: policy.clone(),
            policy_evaluation_time_ms: 1_700_000_000_000,
            validator_qualification_expires_at_unix_ms: 1_700_000_300_000,
        },
        &promotion_controller,
    )
    .expect("signed root promotion reservation");
    let promotion_reservation_bytes = norito::encode_canonical(&promotion_reservation)
        .expect("canonical signed promotion reservation");
    binding.promotion_reservation = exact_receipt_bytes(&promotion_reservation_bytes);
    let (_validator_bodies, validator_seals, validator_keys) = qualified_receipt_hosts(&binding);
    let (governance_keys, governance_policy, governance_authority) = governance_receipt_fixture();
    let mut metadata = Metadata::default();
    if let Some(expiry) = options.expires_at_height {
        metadata.insert(
            "expires_at_height"
                .parse()
                .expect("valid expiry metadata key"),
            Json::from(expiry),
        );
    }
    let transaction_builder = (if options.direct_activation {
        TransactionBuilder::new(
            binding.network_id,
            governance_authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([ActivateKagemushaRecursiveReleaseV4::new(
            {
                let mut instruction_binding = binding.clone();
                instruction_binding.promotion_id = options
                    .instruction_promotion_id
                    .unwrap_or(binding.promotion_id);
                instruction_binding
            },
            activation,
            policy,
        )])
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
    })
    .with_metadata(metadata);
    let approved_transaction = transaction_builder.clone().sign_multisig([
        governance_keys[0].private_key(),
        governance_keys[1].private_key(),
    ]);
    let carrier_transaction = if options.alternate_authorization {
        transaction_builder.sign_multisig([
            governance_keys[0].private_key(),
            governance_keys[2].private_key(),
        ])
    } else {
        approved_transaction.clone()
    };
    let result_inner = if options.failed_result {
        TransactionResultInner::Err(
            crate::transaction::error::TransactionRejectionReason::Validation(
                crate::ValidationFail::InternalError("receipt fixture rejection".into()),
            ),
        )
    } else {
        TransactionResultInner::Ok(DataTriggerSequence::default())
    };
    let anchor_block = receipt_anchor_block(&validator_keys[0]);
    let trusted_finality_anchor = finalized_receipt_proof(
        &anchor_block,
        &binding,
        &validator_keys,
        binding.execution_policy_hash,
        None,
    );
    let (block, committed_transaction) = receipt_block(
        &carrier_transaction,
        &approved_transaction,
        &validator_keys[0],
        result_inner,
        2,
        Some(anchor_block.hash()),
        0,
    );
    let finality_proof = finalized_receipt_proof(
        &block,
        &binding,
        &validator_keys,
        binding.execution_policy_hash,
        Some(&trusted_finality_anchor),
    );
    let block_wire = block.encode_wire().expect("result-bearing block wire");
    let transaction_wire = approved_transaction
        .encode_wire_v1()
        .expect("authorization-bearing transaction wire");
    let issuer = KeyPair::from_seed(vec![0x93; 32], Algorithm::Ed25519);
    let expectations_artifact = sign_expectations_body_unchecked(
        KagemushaV4ActivationReceiptExpectationsBodyV1 {
            schema: KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            promotion_controller: promotion_controller.public_key().clone(),
            promotion_reservation: exact_receipt_bytes(&promotion_reservation_bytes),
            binding: binding.clone(),
            receipt_issuer: issuer.public_key().clone(),
            governance_authority: governance_authority.clone(),
            governance_multisig_policy: governance_policy,
            validator_seals: validator_seals.clone(),
            activation_transaction: approved_transaction.clone(),
            trusted_finality_anchor: trusted_finality_anchor.clone(),
        },
        &promotion_controller,
    );
    let expectations_artifact_bytes = norito::encode_canonical(&expectations_artifact)
        .expect("canonical signed activation expectations");
    let expectations =
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &expectations_artifact_bytes,
            promotion_controller.public_key(),
            &promotion_reservation_bytes,
        )
        .unwrap_or_else(|_| {
            KagemushaV4ActivationReceiptExpectationsV1::from_unverified_artifact_for_test(
                &expectations_artifact.body,
                &expectations_artifact_bytes,
            )
            .expect("negative receipt fixture capability")
        });
    let body = KagemushaV4ActivationFinalityReceiptBodyV1 {
        schema: KAGEMUSHA_V4_ACTIVATION_FINALITY_RECEIPT_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        promotion_reservation: expectations.promotion_reservation(),
        activation_expectations_artifact: expectations.activation_expectations_artifact(),
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
        finality_proof_chain: vec![finality_proof]
            .try_into()
            .expect("one bounded successor proof"),
    };
    let receipt = KagemushaV4ActivationFinalityReceiptV1::try_sign(body, &issuer)
        .expect("signed complete receipt");
    CompleteReceiptFixture {
        receipt,
        expectations,
        expectations_artifact,
        expectations_artifact_bytes,
        promotion_reservation,
        promotion_reservation_bytes,
        promotion_controller,
        block,
        issuer,
        validator_keys,
        approved_transaction,
    }
}

#[cfg(feature = "transparent_api")]
fn complete_receipt_fixture(direct_activation: bool) -> CompleteReceiptFixture {
    complete_receipt_fixture_with_options(CompleteReceiptOptions {
        direct_activation,
        ..CompleteReceiptOptions::default()
    })
}

#[cfg(feature = "transparent_api")]
#[test]
fn github_promotion_id_derivation_matches_known_vector() {
    let run = KagemushaV4GitHubPromotionRunV1 {
        repository: "hyperledger-iroha/iroha".to_owned(),
        workflow_ref:
            "hyperledger-iroha/iroha/.github/workflows/promote_kagemusha_v4.yml@refs/heads/main"
                .to_owned(),
        workflow_sha: [0x4a; 20],
        run_id: 1_234_567,
        run_attempt: 2,
    };
    assert_eq!(
        hex::encode(run.promotion_id()),
        "a0ab5de0bf87e740b555f41b9eba3dd5b3cc14f2f811ad45ef87220713245a2d",
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn promotion_reservation_enforces_receipt_size_and_strict_signing_lifetime() {
    let fixture = complete_receipt_fixture(true);
    let evaluation = fixture.promotion_reservation.body.policy_evaluation_time_ms;

    let mut oversized_receipt = fixture.promotion_reservation.body.clone();
    oversized_receipt.catalog_revalidation_receipt_json.byte_len =
        u64::try_from(KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES)
            .expect("receipt bound fits u64")
            + 1;
    assert!(
        KagemushaV4PromotionReservationV1::try_sign(
            oversized_receipt,
            &fixture.promotion_controller,
        )
        .is_err(),
        "the signed receipt descriptor cannot bypass the 256 KiB structural ceiling"
    );

    let mut zero_lifetime = fixture.promotion_reservation.body.clone();
    zero_lifetime.validator_qualification_expires_at_unix_ms = evaluation;
    assert!(
        KagemushaV4PromotionReservationV1::try_sign(zero_lifetime, &fixture.promotion_controller,)
            .is_err(),
        "policy evaluation must strictly precede validator-signing expiry"
    );

    let mut excessive_lifetime = fixture.promotion_reservation.body.clone();
    excessive_lifetime.validator_qualification_expires_at_unix_ms =
        evaluation + KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS + 1;
    assert!(
        KagemushaV4PromotionReservationV1::try_sign(
            excessive_lifetime,
            &fixture.promotion_controller,
        )
        .is_err(),
        "the signed validator-qualification lifetime cannot exceed five minutes"
    );

    let mut zero_catalog = fixture.promotion_reservation.body.clone();
    zero_catalog.catalog_revalidation_catalog_sha256 = [0; 32];
    assert!(
        KagemushaV4PromotionReservationV1::try_sign(zero_catalog, &fixture.promotion_controller,)
            .is_err(),
        "the App-Attest catalog digest must be nonzero independently of consensus policy"
    );

    let oversized_wire = vec![0_u8; KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES + 1];
    assert!(
        KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            &oversized_wire,
            fixture.promotion_controller.public_key(),
        )
        .is_err(),
        "the public exact-byte decoder rejects oversized input before decoding"
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn validator_seals_reject_mixed_exact_reservation_generations() {
    let fixture = complete_receipt_fixture(true);
    let mut reservation_two_body = fixture.promotion_reservation.body.clone();
    reservation_two_body.catalog_revalidation_receipt_json = exact_receipt_bytes(
        b"{\"schema\":\"fixture.catalog_revalidation.v1\",\"valid\":true,\"generation\":2}\n",
    );
    let reservation_two = KagemushaV4PromotionReservationV1::try_sign(
        reservation_two_body,
        &fixture.promotion_controller,
    )
    .expect("controller can sign a second exact reservation generation");
    let reservation_two_bytes =
        norito::encode_canonical(&reservation_two).expect("canonical second reservation");
    assert_ne!(
        reservation_two_bytes, fixture.promotion_reservation_bytes,
        "R1 and R2 must differ in their exact signed bytes"
    );

    let mut binding_two = fixture.expectations_artifact.body.binding.clone();
    binding_two.promotion_reservation = exact_receipt_bytes(&reservation_two_bytes);
    let (_, seals_two, _) = qualified_receipt_hosts(&binding_two);

    let mut coherent_two_body = fixture.expectations_artifact.body.clone();
    coherent_two_body.promotion_reservation = exact_receipt_bytes(&reservation_two_bytes);
    coherent_two_body.binding = binding_two.clone();
    coherent_two_body.validator_seals = seals_two.clone();
    let coherent_two =
        sign_expectations_body_unchecked(coherent_two_body, &fixture.promotion_controller);
    let coherent_two_bytes =
        norito::encode_canonical(&coherent_two).expect("canonical coherent R2 expectations");
    KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
        &coherent_two_bytes,
        fixture.promotion_controller.public_key(),
        &reservation_two_bytes,
    )
    .expect("four R2 seals and exact R2 reservation remain a coherent generation");

    for use_second_generation_binding in [false, true] {
        let mut mixed_body = fixture.expectations_artifact.body.clone();
        if use_second_generation_binding {
            mixed_body.promotion_reservation = exact_receipt_bytes(&reservation_two_bytes);
            mixed_body.binding = binding_two.clone();
            mixed_body.validator_seals = seals_two.clone();
            mixed_body.validator_seals[0] =
                fixture.expectations_artifact.body.validator_seals[0].clone();
        } else {
            mixed_body.validator_seals[0] = seals_two[0].clone();
        }
        let mixed = sign_expectations_body_unchecked(mixed_body, &fixture.promotion_controller);
        let mixed_bytes =
            norito::encode_canonical(&mixed).expect("canonical mixed-generation expectations");
        let reservation_bytes = if use_second_generation_binding {
            &reservation_two_bytes
        } else {
            &fixture.promotion_reservation_bytes
        };
        assert_eq!(
            KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
                &mixed_bytes,
                fixture.promotion_controller.public_key(),
                reservation_bytes,
            ),
            Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
            "one R1/R2 seal splice must fail under either claimed reservation generation"
        );
    }
}

#[cfg(feature = "transparent_api")]
#[test]
fn root_expectations_authenticate_key_domain_provenance_seals_transaction_anchor_and_digests() {
    let fixture = complete_receipt_fixture(true);
    let verified = KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
        &fixture.expectations_artifact_bytes,
        fixture.promotion_controller.public_key(),
        &fixture.promotion_reservation_bytes,
    )
    .expect("valid signed reservation and expectations mint a capability");
    assert_eq!(
        verified.activation_transaction_wire(),
        exact_receipt_bytes(
            &fixture
                .approved_transaction
                .encode_wire_v1()
                .expect("approved transaction wire"),
        ),
    );

    let wrong_controller = KeyPair::from_seed(vec![0xD0; 32], Algorithm::Ed25519);
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &fixture.expectations_artifact_bytes,
            wrong_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::PromotionController),
    );

    let mut stale_signature = fixture.expectations_artifact.clone();
    stale_signature.body.binding.catalog_consensus_policy_digest[0] ^= 0x80;
    let stale_signature_bytes =
        norito::encode_canonical(&stale_signature).expect("canonical stale-signature artifact");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &stale_signature_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
            "activation_expectations",
        )),
    );

    let mut no_domain_signature = fixture.expectations_artifact.clone();
    no_domain_signature.signature = SignatureOf::try_from_hash(
        fixture.promotion_controller.private_key(),
        HashOf::new(&no_domain_signature.body),
    )
    .expect("test controller can sign an undomained body hash");
    let no_domain_signature_bytes = norito::encode_canonical(&no_domain_signature)
        .expect("canonical undomained-signature artifact");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &no_domain_signature_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::InvalidSignature(
            "activation_expectations",
        )),
    );

    let mut replay_body = fixture.promotion_reservation.body.clone();
    replay_body.github_run.run_attempt += 1;
    replay_body.promotion_id = replay_body.github_run.promotion_id();
    let replay_reservation =
        KagemushaV4PromotionReservationV1::try_sign(replay_body, &fixture.promotion_controller)
            .expect("controller can sign a different valid run reservation");
    let replay_reservation_bytes =
        norito::encode_canonical(&replay_reservation).expect("canonical replay reservation");
    let mut replay_expectations_body = fixture.expectations_artifact.body.clone();
    replay_expectations_body.promotion_reservation = exact_receipt_bytes(&replay_reservation_bytes);
    let replay_expectations =
        sign_expectations_body_unchecked(replay_expectations_body, &fixture.promotion_controller);
    let replay_expectations_bytes =
        norito::encode_canonical(&replay_expectations).expect("canonical replay expectations");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &replay_expectations_bytes,
            fixture.promotion_controller.public_key(),
            &replay_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::PromotionProvenance),
    );

    let mut reordered_seals_body = fixture.expectations_artifact.body.clone();
    reordered_seals_body.validator_seals.swap(0, 1);
    let reordered_seals =
        sign_expectations_body_unchecked(reordered_seals_body, &fixture.promotion_controller);
    let reordered_seals_bytes =
        norito::encode_canonical(&reordered_seals).expect("canonical reordered-seal artifact");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &reordered_seals_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ValidatorSet),
    );

    let log_fixture = complete_receipt_fixture(false);
    let mut transaction_splice_body = fixture.expectations_artifact.body.clone();
    transaction_splice_body.activation_transaction = log_fixture
        .expectations_artifact
        .body
        .activation_transaction;
    let transaction_splice =
        sign_expectations_body_unchecked(transaction_splice_body, &fixture.promotion_controller);
    let transaction_splice_bytes = norito::encode_canonical(&transaction_splice)
        .expect("canonical transaction-splice artifact");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &transaction_splice_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ActivationTransaction),
    );

    let mut anchor_splice_body = fixture.expectations_artifact.body.clone();
    anchor_splice_body
        .trusted_finality_anchor
        .finality_artifact
        .commit_qc
        .aggregate_signature[0] ^= 0x80;
    let anchor_splice =
        sign_expectations_body_unchecked(anchor_splice_body, &fixture.promotion_controller);
    let anchor_splice_bytes =
        norito::encode_canonical(&anchor_splice).expect("canonical anchor-splice artifact");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &anchor_splice_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::Finality),
    );

    let mut reservation_digest_splice_body = fixture.expectations_artifact.body.clone();
    reservation_digest_splice_body.promotion_reservation.sha256[0] ^= 0x80;
    let reservation_digest_splice = sign_expectations_body_unchecked(
        reservation_digest_splice_body,
        &fixture.promotion_controller,
    );
    let reservation_digest_splice_bytes = norito::encode_canonical(&reservation_digest_splice)
        .expect("canonical reservation-digest splice");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &reservation_digest_splice_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ReservationDigest),
    );

    let mut wrong_exact_artifact_bytes = fixture.expectations_artifact_bytes.clone();
    wrong_exact_artifact_bytes.push(0);
    assert_eq!(
        fixture.expectations_artifact.verify_exact(
            &wrong_exact_artifact_bytes,
            fixture.promotion_controller.public_key(),
            &fixture.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ExpectationsDigest),
    );

    let mut receipt_digest_splice_body = fixture.receipt.body.clone();
    receipt_digest_splice_body
        .activation_expectations_artifact
        .sha256[0] ^= 0x80;
    let receipt_digest_splice = KagemushaV4ActivationFinalityReceiptV1::try_sign(
        receipt_digest_splice_body,
        &fixture.issuer,
    )
    .expect("issuer can sign a hostile digest for verifier regression");
    assert_eq!(
        receipt_digest_splice.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::ExpectationMismatch),
    );
    let mut receipt_reservation_splice_body = fixture.receipt.body.clone();
    receipt_reservation_splice_body.promotion_reservation.sha256[0] ^= 0x80;
    let receipt_reservation_splice = KagemushaV4ActivationFinalityReceiptV1::try_sign(
        receipt_reservation_splice_body,
        &fixture.issuer,
    )
    .expect("issuer can sign a hostile reservation digest for verifier regression");
    assert_eq!(
        receipt_reservation_splice.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::ExpectationMismatch),
    );

    let mut overlap_reservation_body = fixture.promotion_reservation.body.clone();
    overlap_reservation_body.promotion_controller = fixture.issuer.public_key().clone();
    let overlap_reservation =
        KagemushaV4PromotionReservationV1::try_sign(overlap_reservation_body, &fixture.issuer)
            .expect("independent reservation checks do not know downstream roles");
    let overlap_reservation_bytes = norito::encode_canonical(&overlap_reservation)
        .expect("canonical overlapping-controller reservation");
    let mut overlap_binding = fixture.expectations_artifact.body.binding.clone();
    overlap_binding.promotion_controller = fixture.issuer.public_key().clone();
    overlap_binding.promotion_reservation = exact_receipt_bytes(&overlap_reservation_bytes);
    let overlap_validator_seals = fixture
        .expectations_artifact
        .body
        .validator_seals
        .iter()
        .zip(&fixture.validator_keys)
        .map(|(seal, validator_key)| {
            let mut body = seal.body.clone();
            body.binding = overlap_binding.clone();
            KagemushaV4ValidatorQualificationSealV1::try_sign(body, validator_key)
                .expect("validator can requalify the coherent hostile binding")
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("exactly four hostile validator seals");
    let mut overlap_expectations_body = fixture.expectations_artifact.body.clone();
    overlap_expectations_body.promotion_controller = fixture.issuer.public_key().clone();
    overlap_expectations_body.promotion_reservation =
        exact_receipt_bytes(&overlap_reservation_bytes);
    overlap_expectations_body.binding = overlap_binding;
    overlap_expectations_body.validator_seals = overlap_validator_seals;
    let overlap_expectations =
        sign_expectations_body_unchecked(overlap_expectations_body, &fixture.issuer);
    let overlap_expectations_bytes = norito::encode_canonical(&overlap_expectations)
        .expect("canonical overlapping-controller expectations");
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &overlap_expectations_bytes,
            fixture.issuer.public_key(),
            &overlap_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ControllerRoleOverlap),
    );

    let no_expiry = complete_receipt_fixture_with_options(CompleteReceiptOptions {
        expires_at_height: None,
        ..CompleteReceiptOptions::default()
    });
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &no_expiry.expectations_artifact_bytes,
            no_expiry.promotion_controller.public_key(),
            &no_expiry.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ActivationExpiry),
    );

    let promotion_splice = complete_receipt_fixture_with_options(CompleteReceiptOptions {
        instruction_promotion_id: Some([0xEE; 32]),
        ..CompleteReceiptOptions::default()
    });
    assert_eq!(
        KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical(
            &promotion_splice.expectations_artifact_bytes,
            promotion_splice.promotion_controller.public_key(),
            &promotion_splice.promotion_reservation_bytes,
        ),
        Err(KagemushaPromotionReceiptValidationError::ActivationPayload),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn complete_activation_receipt_verifies_and_rejects_block_finality_policy_and_tx_splices() {
    let fixture = complete_receipt_fixture(true);
    let verified = fixture
        .receipt
        .verify(&fixture.expectations)
        .expect("complete activation receipt verifies");
    assert_eq!(verified.finalized_block_hash(), fixture.block.hash());
    assert_eq!(
        verified.activation_transaction_intent(),
        fixture.expectations.activation_transaction_intent()
    );
    assert_eq!(verified.finalized_height(), 2);

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
    let mut bad_finality_proofs = bad_finality.finality_proof_chain.into_vec();
    bad_finality_proofs
        .last_mut()
        .expect("one successor proof")
        .finality_artifact
        .commit_qc
        .aggregate_signature[0] ^= 0x80;
    bad_finality.finality_proof_chain = bad_finality_proofs
        .try_into()
        .expect("mutated signature preserves the bounded chain shape");
    let bad_finality =
        KagemushaV4ActivationFinalityReceiptV1::try_sign(bad_finality, &fixture.issuer)
            .expect("receipt issuer may attest hostile finality for verifier regression");
    assert_eq!(
        bad_finality.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::Finality),
    );

    let mut policy_splice = fixture.receipt.body.clone();
    let mut policy_splice_proofs = policy_splice.finality_proof_chain.into_vec();
    *policy_splice_proofs
        .last_mut()
        .expect("one successor proof") = finalized_receipt_proof(
        &fixture.block,
        fixture.expectations.binding(),
        &fixture.validator_keys,
        Hash::new(b"different aggregate execution policy"),
        Some(fixture.expectations.trusted_finality_anchor()),
    );
    policy_splice.finality_proof_chain = policy_splice_proofs
        .try_into()
        .expect("policy splice preserves the bounded chain shape");
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
fn activation_receipt_rejects_finality_da_layout_and_pop_splices() {
    let fixture = complete_receipt_fixture(true);

    let mut da_body = fixture.receipt.body.clone();
    let mut da_proofs = da_body.finality_proof_chain.into_vec();
    da_proofs[0]
        .finality_artifact
        .height_context
        .da_layout
        .chunk_size_bytes /= 2;
    da_body.finality_proof_chain = da_proofs
        .try_into()
        .expect("DA splice preserves the bounded chain shape");
    let da_receipt = KagemushaV4ActivationFinalityReceiptV1::try_sign(da_body, &fixture.issuer)
        .expect("issuer may sign hostile DA evidence for verifier regression");
    assert_eq!(
        da_receipt.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::FinalityRoster),
    );

    let mut pop_body = fixture.receipt.body.clone();
    let mut pop_proofs = pop_body.finality_proof_chain.into_vec();
    pop_proofs[0].finality_artifact.validator_set_pops[0][0] ^= 0x80;
    pop_body.finality_proof_chain = pop_proofs
        .try_into()
        .expect("PoP splice preserves the bounded chain shape");
    let pop_receipt = KagemushaV4ActivationFinalityReceiptV1::try_sign(pop_body, &fixture.issuer)
        .expect("issuer may sign hostile PoP evidence for verifier regression");
    assert_eq!(
        pop_receipt.verify(&fixture.expectations),
        Err(KagemushaPromotionReceiptValidationError::FinalityRoster),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn activation_receipt_rejects_authorization_result_context_roster_and_block_splices() {
    let fixture = complete_receipt_fixture(true);

    let authorization_splice = complete_receipt_fixture_with_options(CompleteReceiptOptions {
        alternate_authorization: true,
        ..CompleteReceiptOptions::default()
    });
    assert_eq!(
        authorization_splice
            .receipt
            .verify(&authorization_splice.expectations),
        Err(KagemushaPromotionReceiptValidationError::ActivationAuthorizationWire),
        "the finalized block must carry the exact approved multisig bundle, not only its intent",
    );

    let failed_result = complete_receipt_fixture_with_options(CompleteReceiptOptions {
        failed_result: true,
        ..CompleteReceiptOptions::default()
    });
    assert_eq!(
        failed_result.receipt.verify(&failed_result.expectations),
        Err(KagemushaPromotionReceiptValidationError::CommittedTransaction),
    );

    let mut stale_context = fixture.expectations.clone();
    stale_context
        .trusted_finality_anchor_mut_for_test()
        .finality_artifact
        .height_context
        .leader_seed[0] ^= 0x80;
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
    let mut substituted_roster_proofs = substituted_roster_body.finality_proof_chain.into_vec();
    *substituted_roster_proofs
        .last_mut()
        .expect("one successor proof") = finalized_receipt_proof(
        &fixture.block,
        fixture.expectations.binding(),
        &substituted_validator_keys,
        fixture.expectations.binding().execution_policy_hash,
        Some(fixture.expectations.trusted_finality_anchor()),
    );
    substituted_roster_body.finality_proof_chain = substituted_roster_proofs
        .try_into()
        .expect("roster splice preserves the bounded chain shape");
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
        2,
        Some(
            fixture
                .expectations
                .trusted_finality_anchor()
                .block_header
                .hash(),
        ),
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
fn activation_receipt_binds_promotion_id_expiry_chain_and_independent_issuer() {
    let promotion_splice = complete_receipt_fixture_with_options(CompleteReceiptOptions {
        instruction_promotion_id: Some([0xEE; 32]),
        ..CompleteReceiptOptions::default()
    });
    assert_eq!(
        promotion_splice
            .receipt
            .verify(&promotion_splice.expectations),
        Err(KagemushaPromotionReceiptValidationError::ActivationPayload),
    );

    let maximum_expiry = 1_u64
        + u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
            .expect("proof-count bound fits u64")
        + 1;
    for expires_at_height in [None, Some(2), Some(maximum_expiry + 1)] {
        let fixture = complete_receipt_fixture_with_options(CompleteReceiptOptions {
            expires_at_height,
            ..CompleteReceiptOptions::default()
        });
        assert_eq!(
            fixture.receipt.verify(&fixture.expectations),
            Err(KagemushaPromotionReceiptValidationError::ActivationExpiry),
        );
    }
    for expires_at_height in [3, maximum_expiry] {
        let fixture = complete_receipt_fixture_with_options(CompleteReceiptOptions {
            expires_at_height: Some(expires_at_height),
            ..CompleteReceiptOptions::default()
        });
        let verified = fixture
            .receipt
            .verify(&fixture.expectations)
            .unwrap_or_else(|error| {
                panic!(
                    "exclusive expiry {expires_at_height} above finalized height 2 and at or below anchor 1 + 4097 must verify: {error}"
                )
            });
        assert_eq!(verified.finalized_height(), 2);
    }

    let fixture = complete_receipt_fixture(true);
    let mut empty_chain = fixture.receipt.body.clone();
    empty_chain.finality_proof_chain =
        KagemushaV4ActivationFinalityProofChainV1::from_proofs_unchecked(Vec::new());
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(empty_chain, &fixture.issuer),
        Err(KagemushaPromotionReceiptValidationError::FinalityChain),
    );
    let mut noncontiguous_chain = fixture.receipt.body.clone();
    let repeated = noncontiguous_chain.finality_proof_chain.as_slice()[0].clone();
    let mut noncontiguous_proofs = noncontiguous_chain.finality_proof_chain.into_vec();
    noncontiguous_proofs.push(repeated);
    noncontiguous_chain.finality_proof_chain =
        KagemushaV4ActivationFinalityProofChainV1::from_proofs_unchecked(noncontiguous_proofs);
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(noncontiguous_chain, &fixture.issuer,),
        Err(KagemushaPromotionReceiptValidationError::FinalityChain),
    );

    let governance_issuer = KeyPair::from_seed(vec![0x91; 32], Algorithm::Ed25519);
    let mut governance_overlap = fixture.receipt.body.clone();
    governance_overlap.issuer = governance_issuer.public_key().clone();
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(governance_overlap, &governance_issuer,),
        Err(KagemushaPromotionReceiptValidationError::IssuerRoleOverlap),
    );
    let mut overlapping_expectations = fixture.expectations.clone();
    overlapping_expectations.set_receipt_issuer_for_test(governance_issuer.public_key().clone());
    assert_eq!(
        fixture.receipt.verify(&overlapping_expectations),
        Err(KagemushaPromotionReceiptValidationError::IssuerRoleOverlap),
    );
    let mut validator_overlap = fixture.receipt.body;
    validator_overlap.issuer = fixture.validator_keys[0].public_key().clone();
    assert_eq!(
        KagemushaV4ActivationFinalityReceiptV1::try_sign(
            validator_overlap,
            &fixture.validator_keys[0],
        ),
        Err(KagemushaPromotionReceiptValidationError::IssuerRoleOverlap),
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
    single_expectations.set_governance_authority_for_test(single_authority);
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
    weak_expectations
        .set_governance_authority_for_test(AccountId::new_multisig(weak_policy.clone()));
    weak_expectations.set_governance_multisig_policy_for_test(weak_policy);
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
    substituted_policy.set_governance_multisig_policy_for_test(different_policy);
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

#[cfg(feature = "transparent_api")]
struct CompleteCanaryFixture {
    receipt: CompleteReceiptFixture,
    canary_transaction: SignedTransaction,
    authorization: KagemushaV4TairaCanaryAuthorizationV1,
    authorization_bytes: Vec<u8>,
    evidence: KagemushaV4TairaCanaryEvidenceV1,
    evidence_bytes: Vec<u8>,
}

#[cfg(feature = "transparent_api")]
fn canary_transaction_fixture(
    permit: &KagemushaV4TairaCanaryPermitV1,
    authority: &KeyPair,
    nonce: u32,
) -> SignedTransaction {
    let body = &permit.body;
    let metadata = kagemusha_v4_taira_canary_transaction_metadata(
        body.binding.promotion_id,
        body.activation_finality_receipt,
        &body.canonical_torii_origin,
        body.expires_at_height,
    );
    let mut builder = TransactionBuilder::new(
        body.binding.network_id.clone(),
        body.canary_authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([RecordKagemushaTairaCanaryV4::new(permit.clone())])
    .with_metadata(metadata);
    builder.set_creation_time(std::time::Duration::from_millis(1_700_000_001_000));
    builder.set_ttl(std::time::Duration::from_millis(60_000));
    builder.set_nonce(std::num::NonZeroU32::new(nonce).expect("non-zero canary nonce"));
    builder.sign(authority.private_key())
}

#[cfg(feature = "transparent_api")]
fn complete_canary_fixture() -> CompleteCanaryFixture {
    let receipt = complete_receipt_fixture(true);
    let receipt_bytes = norito::encode_canonical(&receipt.receipt).expect("canonical receipt");
    let receipt_identity = exact_receipt_bytes(&receipt_bytes);
    let authorization_body = KagemushaV4TairaCanaryAuthorizationBodyV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        binding: receipt.expectations.binding().clone(),
        activation_expectations_artifact: receipt.expectations.activation_expectations_artifact(),
        activation_finality_receipt: receipt_identity,
        canary_authority: AccountId::new(receipt.promotion_controller.public_key().clone()),
        canonical_torii_origin: "https://taira.example".to_owned(),
        authorized_at_unix_ms: 1_700_000_000_000,
        expires_at_unix_ms: 1_700_000_300_000,
        expires_at_height: NonZeroU64::new(4).expect("non-zero canary height expiry"),
    };
    let permit = KagemushaV4TairaCanaryPermitV1::try_sign(
        authorization_body,
        &receipt.promotion_controller,
        &receipt.expectations,
        &receipt.receipt,
        &receipt_bytes,
    )
    .expect("controller-signed post-receipt canary permit");
    let canary_transaction = canary_transaction_fixture(&permit, &receipt.promotion_controller, 7);
    let canary_transaction_wire = canary_transaction
        .encode_wire_v1()
        .expect("canonical canary transaction wire");
    let authorization = KagemushaV4TairaCanaryAuthorizationV1::try_sign(
        permit,
        canary_transaction.clone(),
        &receipt.promotion_controller,
        &receipt.expectations,
        &receipt.receipt,
        &receipt_bytes,
    )
    .expect("controller-signed exact post-receipt canary authorization");
    let authorization_bytes =
        norito::encode_canonical(&authorization).expect("canonical canary authorization");
    authorization
        .verify_exact(
            &authorization_bytes,
            &receipt.expectations,
            &receipt.receipt,
            &receipt_bytes,
            1_700_000_001_500,
        )
        .expect("exact canary authorization verifies");

    let (block, committed_transaction) = receipt_block(
        &canary_transaction,
        &canary_transaction,
        &receipt.validator_keys[0],
        TransactionResultInner::Ok(DataTriggerSequence::default()),
        3,
        Some(receipt.block.hash()),
        1_700_000_002_000,
    );
    let receipt_terminal = receipt
        .receipt
        .body
        .finality_proof_chain
        .last()
        .expect("receipt terminal proof");
    let finality_proof = finalized_receipt_proof(
        &block,
        receipt.expectations.binding(),
        &receipt.validator_keys,
        receipt.expectations.binding().execution_policy_hash,
        Some(receipt_terminal),
    );
    let finality_proof_chain =
        KagemushaV4ActivationFinalityProofChainV1::try_from(vec![finality_proof])
            .expect("one receipt-rooted canary successor");
    let block_bytes = block.encode_wire().expect("canonical canary block wire");
    let query_started_at_unix_ms = 1_700_000_003_000;
    let committed_bytes =
        norito::encode_canonical(&committed_transaction).expect("canonical committed canary");
    let proof_bytes =
        norito::encode_canonical(&finality_proof_chain).expect("canonical canary proof extension");
    let evidence_body = KagemushaV4TairaCanaryEvidenceBodyV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        promotion_controller: receipt.promotion_controller.public_key().clone(),
        promotion_id: receipt.expectations.binding().promotion_id,
        network_id: receipt.expectations.binding().network_id,
        promotion_reservation: receipt.expectations.promotion_reservation(),
        activation_expectations_artifact: receipt.expectations.activation_expectations_artifact(),
        activation_finality_receipt: receipt_identity,
        canary_authorization: exact_receipt_bytes(&authorization_bytes),
        issuer: receipt.issuer.public_key().clone(),
        activation_transaction_intent: receipt.approved_transaction.hash(),
        activation_finalized_height: receipt.block.header().height().get(),
        activation_finalized_block_hash: receipt.block.hash(),
        canary_transaction_intent: canary_transaction.hash(),
        canary_transaction_wire: exact_receipt_bytes(&canary_transaction_wire),
        committed_transaction,
        finalized_block_wire: KagemushaFinalizedBlockWireV1::try_from_bytes(block_bytes.clone())
            .expect("bounded canary block wire"),
        finalized_block_wire_digest: exact_receipt_bytes(&block_bytes),
        finality_proof_chain,
        finalized_height: block.header().height().get(),
        finalized_block_hash: block.hash(),
        query: KagemushaV4TairaCanaryQueryObservationV1 {
            query_started_at_unix_ms,
            query_completed_at_unix_ms: query_started_at_unix_ms + 1_000,
            pipeline_status_response_norito: exact_receipt_bytes(b"Applied pipeline response"),
            pipeline_status_scope: "global".to_owned(),
            pipeline_status_resolved_from: "state".to_owned(),
            pipeline_transaction_intent: canary_transaction.hash(),
            pipeline_status_kind: "Applied".to_owned(),
            pipeline_status_block_height: 3,
            transaction_details_response_norito: exact_receipt_bytes(
                b"canary transaction details response",
            ),
            transaction_details_trigger_completion_count: 0,
            node_status_before_norito: exact_receipt_bytes(b"node status before"),
            node_status_before_observed_at_ms: query_started_at_unix_ms + 100,
            node_status_before_height: 3,
            node_status_after_norito: exact_receipt_bytes(b"node status after"),
            node_status_after_observed_at_ms: query_started_at_unix_ms + 900,
            node_status_after_height: 3,
            committed_transaction_norito: exact_receipt_bytes(&committed_bytes),
            finalized_block_wire: exact_receipt_bytes(&block_bytes),
            finality_proof_chain_norito: exact_receipt_bytes(&proof_bytes),
            finality_proof_count: 1,
        },
    };
    let evidence = KagemushaV4TairaCanaryEvidenceV1::try_sign(
        evidence_body,
        &receipt.issuer,
        &authorization,
        &authorization_bytes,
        &receipt.expectations,
        &receipt.receipt,
        &receipt_bytes,
    )
    .expect("issuer-signed production canary evidence");
    let evidence_bytes =
        norito::encode_canonical(&evidence).expect("canonical production canary evidence");
    CompleteCanaryFixture {
        receipt,
        canary_transaction,
        authorization,
        authorization_bytes,
        evidence,
        evidence_bytes,
    }
}

#[cfg(feature = "transparent_api")]
#[test]
fn canary_permit_expiry_matches_the_bounded_finality_corridor() {
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical receipt");
    let finalized_height = fixture
        .receipt
        .receipt
        .verify(&fixture.receipt.expectations)
        .expect("valid activation receipt")
        .finalized_height();
    let maximum_expiry = finalized_height
        .checked_add(
            u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
                .expect("proof-count bound fits u64"),
        )
        .and_then(|height| height.checked_add(1))
        .expect("bounded canary expiry");
    let mut body = fixture.authorization.permit().body.clone();
    body.expires_at_height = NonZeroU64::new(maximum_expiry).expect("non-zero maximum expiry");
    KagemushaV4TairaCanaryPermitV1::try_sign(
        body.clone(),
        &fixture.receipt.promotion_controller,
        &fixture.receipt.expectations,
        &fixture.receipt.receipt,
        &receipt_bytes,
    )
    .expect("the exclusive finalized-plus-proof-bound-plus-one expiry must be admitted");

    body.expires_at_height = NonZeroU64::new(maximum_expiry + 1).expect("non-zero excess expiry");
    assert_eq!(
        KagemushaV4TairaCanaryPermitV1::try_sign(
            body,
            &fixture.receipt.promotion_controller,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
        ),
        Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn production_canary_authorization_and_finality_evidence_verify_exactly() {
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical receipt");
    let decoded_authorization =
        KagemushaV4TairaCanaryAuthorizationV1::decode_canonical(&fixture.authorization_bytes)
            .expect("canonical authorization decodes");
    assert_eq!(decoded_authorization, fixture.authorization);
    let decoded_evidence =
        KagemushaV4TairaCanaryEvidenceV1::decode_canonical(&fixture.evidence_bytes)
            .expect("canonical evidence decodes");
    let verified = decoded_evidence
        .verify_exact(
            &fixture.evidence_bytes,
            &fixture.authorization,
            &fixture.authorization_bytes,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
        )
        .expect("production canary evidence verifies");
    assert_eq!(verified.finalized_height(), 3);
    assert_eq!(
        verified.canary_transaction_intent(),
        fixture.canary_transaction.hash()
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn canary_rejects_valid_transaction_spliced_under_an_old_authorization_signature() {
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical receipt");
    let replacement = canary_transaction_fixture(
        fixture.authorization.permit(),
        &fixture.receipt.promotion_controller,
        8,
    );
    let mut spliced = fixture.authorization.clone();
    spliced.canary_transaction = replacement;
    let spliced_bytes = norito::encode_canonical(&spliced).expect("canonical spliced artifact");
    assert_eq!(
        spliced.verify_exact(
            &spliced_bytes,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
            1_700_000_001_500,
        ),
        Err(KagemushaV4TairaCanaryEvidenceValidationError::Signature),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn canary_rejects_retrospectively_resigned_permit_not_embedded_in_transaction() {
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical receipt");
    let mut retrospective = fixture.authorization;
    retrospective
        .reservation
        .body
        .permit
        .body
        .canonical_torii_origin = "https://other.example".to_owned();
    retrospective.reservation.body.permit.signature = SignatureOf::try_from_hash(
        fixture.receipt.promotion_controller.private_key(),
        retrospective.reservation.body.permit.body.signing_hash(),
    )
    .expect("controller can sign the altered permit");
    retrospective.reservation.signature = SignatureOf::try_from_hash(
        fixture.receipt.promotion_controller.private_key(),
        retrospective.reservation.body.signing_hash(),
    )
    .expect("controller can sign the altered reservation");
    let package = KagemushaV4TairaCanaryAuthorizationPackageV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_PACKAGE_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        reservation: retrospective.reservation.clone(),
        canary_transaction: retrospective.canary_transaction.clone(),
    };
    retrospective.signature = SignatureOf::try_from_hash(
        fixture.receipt.promotion_controller.private_key(),
        package.signing_hash(),
    )
    .expect("controller can sign the altered outer package");
    let bytes = norito::encode_canonical(&retrospective).expect("canonical retrospective package");
    assert_eq!(
        retrospective.verify_exact(
            &bytes,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
            1_700_000_001_500,
        ),
        Err(KagemushaV4TairaCanaryEvidenceValidationError::CanaryTransaction),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn canary_rejects_issuer_assertion_that_only_repackages_receipt_finality() {
    let fixture = complete_canary_fixture();
    let receipt_bytes =
        norito::encode_canonical(&fixture.receipt.receipt).expect("canonical receipt");
    let mut forged_body = fixture.evidence.body.clone();
    forged_body.finality_proof_chain = fixture.receipt.receipt.body.finality_proof_chain.clone();
    let proof_bytes = norito::encode_canonical(&forged_body.finality_proof_chain)
        .expect("canonical forged proof chain");
    forged_body.query.finality_proof_chain_norito = exact_receipt_bytes(&proof_bytes);
    assert_eq!(
        KagemushaV4TairaCanaryEvidenceV1::try_sign(
            forged_body,
            &fixture.receipt.issuer,
            &fixture.authorization,
            &fixture.authorization_bytes,
            &fixture.receipt.expectations,
            &fixture.receipt.receipt,
            &receipt_bytes,
        ),
        Err(KagemushaV4TairaCanaryEvidenceValidationError::Finality),
    );
}

#[cfg(feature = "transparent_api")]
#[test]
fn canary_origin_requires_canonical_https_dns_authority() {
    assert!(validate_kagemusha_v4_taira_canary_torii_origin("https://taira.example").is_ok());
    assert!(validate_kagemusha_v4_taira_canary_torii_origin("https://internal:8443").is_ok());
    for invalid in [
        "http://taira.example",
        "https://Taira.example",
        "https://127.0.0.1",
        "https://taira.example/",
        "https://taira.example:443",
        "https://taira.example:0844",
    ] {
        assert!(
            validate_kagemusha_v4_taira_canary_torii_origin(invalid).is_err(),
            "origin `{invalid}` must be rejected",
        );
    }
}
