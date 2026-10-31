//! Real native QCs and controller/committee signatures over synthetic proof bytes/execution.
//! This fixture qualifies wallet custody and receipt binding, never STARK proving or World execution.

use super::*;
use iroha::client::Client;
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    self as native, AtomicPrivateSettlementInputSecretV1,
    test_fixtures::{
        aps_sidecar_material_fixture_with_threshold_and_ordinal_v1, authorize_output_view_keys,
        authorize_payer_inputs,
    },
};
use iroha_core_privacy::privacy_engines::ivm_private_note::ivm_private_recipient_public_key_v1;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    block::{
        BlockSignatures,
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        output_budget::ExecutionOutputLimits,
    },
    isi::private_settlement::{AbortAtomicPrivateSettlementV1, FinalizeAtomicPrivateSettlementV1},
    nexus::*,
    privacy::{PrivacyCommitmentV1, PrivacyPoolIdV1},
    query::CommittedTransaction,
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
    transaction::{TransactionBuilder, TransactionResult},
};
use std::{collections::BTreeMap, time::Duration};

pub(crate) struct Fixture {
    pub intent: PrivateSettlementProofIntentV1,
    pub terms: Vec<PrivateSettlementBusinessTerms>,
    pub openings: Vec<PrivateSettlementRestrictedOpening>,
    pub authorized: Vec<AtomicPrivateSettlementAuthorizedWalletLegV1>,
    pub prepared: PrivateSettlementRequest,
    pub receipt: VerifiedPrivateSettlementReceipt,
}

fn aggregate(keys: &[KeyPair], bytes: &[u8]) -> Vec<u8> {
    let signatures = keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), bytes)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap()
}
fn phase(
    manifest: &AtomicPrivateSettlementV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    keys: &[KeyPair],
    ordinal: usize,
    kind: PrivateSettlementPhaseV1,
    prepared: Hash,
) -> PrivateSettlementPhaseCertificateV1 {
    let leg = &manifest.legs[ordinal];
    let body = PrivateSettlementPhaseBodyV1 {
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        manifest_digest: manifest.manifest_digest().unwrap(),
        leg_ordinal: ordinal as u8,
        route: leg.route,
        delta_digest: leg.delta_digest,
        authority_digest: authority.digest().unwrap(),
        prepared_bundle_digest: prepared,
        phase: kind,
        authority_context_height: manifest.authority_context_height,
        expiry_height: manifest.expiry_height,
    };
    PrivateSettlementPhaseCertificateV1 {
        authority_catalog_index: ordinal as u8,
        signers_bitmap: 0b0111,
        aggregate_signature: aggregate(keys, &body.signature_preimage().unwrap()),
        body,
    }
}

pub(crate) fn complete() -> Fixture {
    complete_for("private-wallet-all-leg-cleanup")
}

pub(super) fn complete_for(label: &str) -> Fixture {
    // A chain label alone does not change signed genesis or its network identity.
    // Bind this fixture's distinct context into the actual signed genesis metadata.
    let mut chain = NativeFinalityFixture::start_with_genesis_extension(
        label,
        *Hash::new(label.as_bytes()).as_ref(),
        Vec::new(),
    );
    let mut independent = chain.verifier();
    let mut originals = (0..2)
        .map(|ordinal| aps_sidecar_material_fixture_with_threshold_and_ordinal_v1(1, ordinal))
        .collect::<Vec<_>>();
    let payer = KeyPair::from_seed(vec![0x38; 32], Algorithm::Ed25519);
    let recipient = KeyPair::from_seed(vec![0x39; 32], Algorithm::Ed25519);
    let sponsor = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    let mut manifest = originals[0].sidecar.manifest.clone();
    manifest.network_id = chain.network_id();
    manifest.authority_context_height = 1;
    manifest.reimbursement_leg_ordinal = 0;
    manifest.fee_intent_digest = manifest.computed_fee_intent_digest().unwrap();
    for (index, original) in originals.iter_mut().enumerate() {
        let mut policy = original.sidecar.policy.body.clone();
        policy.activation_height = 1;
        original.sidecar.policy = PrivateSettlementAuditPolicyV1::new(policy).unwrap();
        let governance = &original.pool_governance.body;
        original.pool_governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
            governance.route,
            PrivacyPoolIdV1::new([0x27 + index as u8; 32]),
            governance.asset_definition_id.clone(),
            governance.asset_binding_salt,
            &original.sidecar.policy,
            PrivateSettlementPoolGovernanceLifecycleV1 {
                activation_height: 1,
                ..governance.lifecycle
            },
        )
        .unwrap();
        let plaintext = &mut original.plaintext;
        plaintext.network_id = manifest.network_id;
        plaintext.fee_intent_digest = manifest.fee_intent_digest;
        plaintext.pool_id = original.pool_governance.body.pool_id;
        plaintext.policy_references = vec![original.pool_governance.governance_digest];
        // Every output has its own one-time view key across the whole bundle,
        // including the inactive cover output. Re-sign these exact keys below.
        for (slot, output) in plaintext.outputs.iter_mut().enumerate() {
            let view_secret = Zeroizing::new([0xA1 + (index * 3 + slot) as u8; 32]);
            output.recipient_view_key = ivm_private_recipient_public_key_v1(&view_secret).unwrap();
        }
        if index == 1 {
            plaintext.sponsor_reimbursement_amount = 0;
            plaintext.outputs[1].note.active = true;
            plaintext.outputs[1].note.value = 5;
            plaintext.outputs[1].note.dummy_domain = None;
            plaintext.outputs[2].note.active = false;
            plaintext.outputs[2].note.value = 0;
            plaintext.outputs[2].note.dummy_domain =
                Some(Hash::new(b"wallet fixture inactive sponsor"));
        }
        let mut leg = original.sidecar.manifest.legs[index];
        leg.pool_id = plaintext.pool_id;
        leg.asset_binding_commitment = plaintext.asset_binding_commitment().unwrap();
        leg.audit_policy_digest = original.sidecar.policy.policy_digest;
        manifest.legs[index] = leg;
    }
    manifest.reimbursement_terms_commitment = originals[0]
        .plaintext
        .reimbursement_terms_commitment()
        .unwrap();
    manifest.bundle_id = manifest.computed_bundle_id().unwrap();
    let intent = manifest.proof_intent();
    let mut terms = Vec::new();
    let mut openings = Vec::new();
    let mut authorized = Vec::new();
    let mut payloads = Vec::new();
    for (index, original) in originals.iter_mut().enumerate() {
        let plaintext = &mut original.plaintext;
        plaintext.bundle_id = manifest.bundle_id;
        let spending = Zeroizing::new([[0x81; 32], [0x82; 32]]);
        let nullifiers = native::prepare_atomic_private_settlement_authorization_inputs_v1(
            &intent,
            index as u8,
            &mut plaintext.inputs,
            &spending,
        )
        .unwrap();
        authorize_payer_inputs(plaintext, &nullifiers, &payer);
        authorize_output_view_keys(plaintext, [&recipient, &payer, &sponsor]);
        for output in &mut plaintext.outputs {
            output.note.commitment = PrivacyCommitmentV1::new([0; 32]);
            output.note.memo_digest = [0; 32];
        }
        let inputs = core::array::from_fn(|slot| {
            AtomicPrivateSettlementInputSecretV1::new(
                spending[slot],
                slot as u32,
                &core::array::from_fn(|level| [0x40 + level as u8; 32]),
            )
            .unwrap()
        });
        let owner = AtomicPrivateSettlementAuthorizedWalletLegV1::new(plaintext.clone(), inputs);
        let (_, inspected) =
            native::encode_atomic_private_settlement_reservation_v1(&intent, index as u8, &owner)
                .unwrap();
        terms.push(PrivateSettlementBusinessTerms {
            asset: plaintext.asset_definition_id.clone(),
            payer: plaintext.payer.clone(),
            recipient: plaintext.recipient.clone(),
            amount: plaintext.amount,
            reimbursement: plaintext.sponsor_reimbursement_amount,
            memo: plaintext.memo.clone(),
        });
        openings.push(PrivateSettlementRestrictedOpening {
            governance: original.pool_governance.clone(),
            policy: original.sidecar.policy.clone(),
        });
        authorized.push(owner);
        let mut payload = original.sidecar.payload.clone();
        let statement = &mut payload.statement;
        statement.network_id = manifest.network_id;
        statement.bundle_id = manifest.bundle_id;
        statement.authority_context_height = manifest.authority_context_height;
        statement.pool_id = manifest.legs[index].pool_id;
        statement.asset_binding_commitment = manifest.legs[index].asset_binding_commitment;
        statement.audit_policy_digest = manifest.legs[index].audit_policy_digest;
        statement.fee_intent_digest = manifest.fee_intent_digest;
        statement.reimbursement_terms_commitment = manifest.reimbursement_terms_commitment;
        statement.reimbursement_leg_ordinal = manifest.reimbursement_leg_ordinal;
        statement.nullifiers = nullifiers.to_vec();
        statement.audit_plaintext_commitment = inspected.audit_plaintext_commitment;
        statement.audit_input_commitment = inspected.audit_input_commitment;
        let mut rng = iroha_crypto::rng_from_seed_slice(&[0x50 + index as u8; 32]);
        statement.encrypted_outputs = native::prepare_atomic_private_settlement_outputs_v1(
            &mut rng,
            &manifest,
            statement,
            &mut plaintext.outputs,
        )
        .unwrap();
        statement.output_commitments = plaintext
            .outputs
            .iter()
            .map(|output| output.note.commitment)
            .collect();
        assert_eq!(
            plaintext.commitment().unwrap(),
            inspected.audit_plaintext_commitment
        );
        let bytes = Zeroizing::new(norito::encode_canonical(plaintext).unwrap());
        payload.audit_capsule = native::audit::seal_private_settlement_audit_capsule_v1_with_rng(
            &bytes,
            PrivateSettlementAuditAadV1 {
                network_id: manifest.network_id,
                bundle_id: manifest.bundle_id,
                leg_ordinal: index as u8,
                route: statement.route,
                authority_digest: original.sidecar.authority.digest().unwrap(),
                authority_context_height: manifest.authority_context_height,
                audit_policy_digest: statement.audit_policy_digest,
                audit_key_epoch: statement.audit_key_epoch,
                plaintext_commitment: statement.audit_plaintext_commitment,
            },
            PrivateSettlementCapsulePaddingV1::KiB16,
            &original.sidecar.policy,
            &mut rng,
        )
        .unwrap();
        statement.audit_capsule_digest = payload.audit_capsule.digest().unwrap();
        // Proof bytes and execution below are explicitly synthetic. Every cryptographic
        // ownership, encryption and finality check exercised by wallet cleanup remains real.
        payload.delta.bundle_id = manifest.bundle_id;
        payload.delta.pool_id = statement.pool_id;
        payload.delta.asset_binding_commitment = statement.asset_binding_commitment;
        payload.delta.statement_digest = statement.digest().unwrap();
        payload.delta.nullifiers = statement.nullifiers.clone();
        payload.delta.output_commitments = statement.output_commitments.clone();
        payload.delta.encrypted_outputs = statement.encrypted_outputs.clone();
        payload.delta.capsule_digest = statement.audit_capsule_digest;
        payload.delta.audit_policy_digest = statement.audit_policy_digest;
        payload.delta.audit_key_epoch = statement.audit_key_epoch;
        payload.delta.validate_against(statement).unwrap();
        payload.availability.body.network_id = manifest.network_id;
        payload.availability.body.bundle_id = manifest.bundle_id;
        payload.availability.body.authority_context_height = manifest.authority_context_height;
        payload.availability.body.payload_digest = payload.payload_digest().unwrap();
        payload.availability.body.payload_bytes = payload
            .sidecar_material_bytes_len()
            .unwrap()
            .try_into()
            .unwrap();
        payload.availability.aggregate_signature = aggregate(
            &original.validator_keys,
            &payload.availability.signature_preimage().unwrap(),
        );
        manifest.legs[index].delta_digest = payload.delta.digest().unwrap();
        manifest.legs[index].payload_digest = payload.payload_digest().unwrap();
        manifest.legs[index].availability_certificate_digest =
            payload.availability.digest().unwrap();
        payloads.push(payload);
    }
    assert_eq!(
        payloads
            .iter()
            .flat_map(|payload| payload
                .delta
                .encrypted_outputs
                .iter()
                .map(|output| output.recipient))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        6,
        "every real leg and cover output requires a distinct one-time recipient"
    );
    manifest.validate().unwrap();
    let materials = originals
        .iter()
        .zip(&payloads)
        .map(|(original, payload)| {
            native::test_fixtures::aps_provisional_material_fixture_v1(
                &manifest,
                &original.sidecar.policy,
                &original.sidecar.authority,
                payload,
            )
        })
        .collect::<Vec<_>>();
    let endpoints = vec![
        (0..4)
            .map(|index| format!("http://wallet-fixture-{index}.invalid/")
                .parse()
                .unwrap())
            .collect();
        2
    ];
    let prepared = PrivateSettlementRequest::new(
        materials,
        endpoints,
        PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
    )
    .unwrap();
    let authorities = originals
        .iter()
        .map(|original| original.sidecar.authority.clone())
        .collect::<Vec<_>>();
    let prepares = originals
        .iter()
        .enumerate()
        .map(|(index, original)| {
            phase(
                &manifest,
                &original.sidecar.authority,
                &original.validator_keys,
                index,
                PrivateSettlementPhaseV1::Prepare,
                Hash::prehashed([0; 32]),
            )
        })
        .collect::<Vec<_>>();
    let barrier = Client::build_private_settlement_prepare_barrier_v1(
        manifest.clone(),
        &authorities,
        payloads.iter().map(|p| p.delta.clone()).collect(),
        prepares.clone(),
    )
    .unwrap();
    let legs = originals
        .iter()
        .enumerate()
        .map(|(index, original)| PrivateSettlementLegReceiptV1 {
            delta: payloads[index].delta.clone(),
            prepare: prepares[index].clone(),
            commit: phase(
                &manifest,
                &original.sidecar.authority,
                &original.validator_keys,
                index,
                PrivateSettlementPhaseV1::Commit,
                barrier.prepared_bundle_digest,
            ),
        })
        .collect();
    let bundle = PrivateSettlementCommitBundleV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: manifest.clone(),
        authority_catalog: barrier.authority_catalog.clone(),
        legs,
    };
    let header = chain.next_header();
    let mut tx = TransactionBuilder::new(
        manifest.network_id,
        manifest.sponsor.clone(),
        manifest.public_fee_intent.clone(),
    )
    .with_instructions([FinalizeAtomicPrivateSettlementV1::new(bundle)]);
    tx.set_creation_time(Duration::from_millis(header.creation_time_ms - 1));
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(tx.sign(sponsor.private_key()));
    let mut block = builder.build(BlockSignatures::default());
    block
        .set_execution_outputs(
            vec![ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: 0,
                result: TransactionResult::new(Ok(Vec::new())),
                completions: Vec::new(),
            })],
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            &ExecutionOutputLimits {
                max_outputs: 16,
                max_output_bytes: 16 * 1024 * 1024,
                max_total_output_bytes: 32 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    let authenticated = independent.verify(&chain.certify(block)).unwrap();
    let block = authenticated.block();
    let entrypoint = block.network_entrypoint_at(0).unwrap().clone();
    let (index, _) = block.network_output_at(0).unwrap();
    let output = block.execution_outputs()[index as usize].clone();
    let evidence = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block.network_input_proof(0).unwrap(),
        entrypoint,
        output_hash: iroha_crypto::HashOf::new(&output),
        output_proof: block.output_proof(index).unwrap(),
        output,
    };
    let receipt =
        VerifiedPrivateSettlementReceipt::verify_recovered(&authenticated, &barrier, evidence)
            .unwrap();
    Fixture {
        intent,
        terms,
        openings,
        authorized,
        prepared,
        receipt,
    }
}

pub(crate) fn verified_abort(fixture: &Fixture, label: &str) -> VerifiedPrivateSettlementAbort {
    // Alternative fixture branch at the same independently pinned genesis, not an Abort
    // after a successful settlement. The synthetic outputs do not claim World execution.
    let mut chain = NativeFinalityFixture::start_with_genesis_extension(
        label,
        *Hash::new(label.as_bytes()).as_ref(),
        Vec::new(),
    );
    let mut independent = chain.verifier();
    let manifest = &fixture.receipt.receipt().manifest;
    assert_eq!(chain.network_id(), manifest.network_id);
    let sponsor = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    let header = chain.next_header();
    let mut transaction = TransactionBuilder::new(
        manifest.network_id,
        manifest.sponsor.clone(),
        manifest.public_fee_intent.clone(),
    )
    .with_instructions([AbortAtomicPrivateSettlementV1::new(
        manifest.clone(),
        PrivateSettlementAbortReasonV1::ParticipantRejected,
    )]);
    transaction.set_creation_time(Duration::from_millis(header.creation_time_ms - 1));
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction.sign(sponsor.private_key()));
    let mut block = builder.build(BlockSignatures::default());
    block
        .set_execution_outputs(
            vec![ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: 0,
                result: TransactionResult::new(Ok(Vec::new())),
                completions: Vec::new(),
            })],
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            &ExecutionOutputLimits {
                max_outputs: 16,
                max_output_bytes: 16 * 1024 * 1024,
                max_total_output_bytes: 32 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    let authenticated = independent.verify(&chain.certify(block)).unwrap();
    let block = authenticated.block();
    let entrypoint = block.network_entrypoint_at(0).unwrap().clone();
    let (index, _) = block.network_output_at(0).unwrap();
    let output = block.execution_outputs()[index as usize].clone();
    let evidence = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block.network_input_proof(0).unwrap(),
        entrypoint,
        output_hash: iroha_crypto::HashOf::new(&output),
        output_proof: block.output_proof(index).unwrap(),
        output,
    };
    VerifiedPrivateSettlementAbort::verify_recovered(&authenticated, manifest, evidence).unwrap()
}
