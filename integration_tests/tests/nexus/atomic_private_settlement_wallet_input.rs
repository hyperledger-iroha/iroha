//! Construct authorized inputs for the genuine note funded by the network fixture.
use super::*;
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    AtomicPrivateSettlementAuthorizedWalletLegV1, plan_atomic_private_settlement_initial_inputs_v1,
    prepare_atomic_private_settlement_authorization_inputs_v1,
};
use iroha_data_model::nexus::PrivateSettlementProofIntentV1;

pub(super) fn authorize_wallet_leg(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: usize,
    governance: &PrivateSettlementPoolGovernanceV1,
    data: &PrivateSettlementLegPrivateData,
    funding: PrivateSettlementFunding,
) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1> {
    let material = |index, purpose: &[u8]| {
        private_settlement_leg_private_material_for_bundle(
            intent.bundle_id,
            governance,
            ordinal,
            index,
            purpose,
        )
    };
    let note = |purpose: &[u8], index: usize, active, value, spending_authority| {
        let field = |offset| material(index * 6 + offset, purpose);
        PrivateSettlementAuditNoteOpeningV1 {
            active,
            // This is private note construction state. The canonical input/output owner
            // replaces the commitment before it can enter a signed body or proof statement.
            commitment: PrivacyCommitmentV1::new([0; 32]),
            value,
            spending_authority,
            rho: field(2),
            blinding: field(3),
            memo_digest: field(4),
            dummy_domain: (!active).then(|| Hash::prehashed(field(5))),
        }
    };
    let spending_secrets = [
        funding.spending_secret,
        material(1, b"input-spending-secret"),
    ];
    let mut input_openings = vec![
        funding.opening,
        note(
            b"input-note",
            1,
            false,
            0,
            derive_note_authority_v1(&spending_secrets[1])?,
        ),
    ];
    let nullifiers = prepare_atomic_private_settlement_authorization_inputs_v1(
        intent,
        ordinal as u8,
        &mut input_openings,
        &spending_secrets,
    )?;
    ensure!(
        input_openings[0].commitment == funding.input_commitments[0],
        "cold wallet replaced the already funded positive note"
    );
    let payer = AccountId::new(data.payer.public_key().clone());
    let recipient = AccountId::new(data.recipient.public_key().clone());
    let payer_body = PrivateSettlementAuditPayerAuthorizationBodyV1::from_openings(
        intent.network_id,
        intent.bundle_id,
        ordinal as u8,
        governance.body.route,
        &payer,
        intent.expiry_height,
        &input_openings,
        &nullifiers,
    )?;
    let payer_authorization = PrivateSettlementAuditPayerAuthorizationV1::new(
        payer_body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            data.payer.public_key().clone(),
            SignatureOf::try_new(data.payer.private_key(), &payer_body)?,
        )],
    );
    let reimbursement = if ordinal == usize::from(intent.reimbursement_leg_ordinal) {
        5
    } else {
        0
    };
    let mut outputs = Vec::with_capacity(3);
    for (index, (signer, active, amount, role)) in [
        (
            &data.recipient,
            true,
            data.amount,
            PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
        ),
        (
            &data.payer,
            true,
            7,
            PrivateSettlementAuditOutputRoleV1::PayerChange,
        ),
        (
            &*ALICE_KEYPAIR,
            reimbursement != 0,
            reimbursement,
            PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let opening = note(
            b"output-note",
            index,
            active,
            amount,
            derive_note_authority_v1(&material(index, b"output-spending-secret"))?,
        );
        let view = ivm_private_recipient_public_key_v1(&material(index, b"output-view-secret"))?;
        let body = PrivateSettlementAuditViewKeyAuthorizationBodyV1::from_output(
            intent.network_id,
            intent.bundle_id,
            ordinal as u8,
            governance.body.route,
            intent.expiry_height,
            &recipient,
            &payer,
            &intent.sponsor,
            index,
            view,
            &opening,
        )?;
        outputs.push(PrivateSettlementAuditOutputV1 {
            role,
            recipient_view_key: view,
            view_key_authorization: PrivateSettlementAuditViewKeyAuthorizationV1::new(
                body.clone(),
                vec![PrivateSettlementAuditViewKeySignatureV1::new(
                    signer.public_key().clone(),
                    SignatureOf::try_new(signer.private_key(), &body)?,
                )],
            ),
            encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                ephemeral_secret: material(index, b"output-ephemeral-secret"),
            },
            note: opening,
        });
    }
    let plaintext = PrivateSettlementAuditPlaintextV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: intent.network_id,
        bundle_id: intent.bundle_id,
        leg_ordinal: ordinal as u8,
        route: governance.body.route,
        pool_id: governance.body.pool_id,
        payer,
        payer_authorization,
        recipient,
        sponsor: intent.sponsor.clone(),
        asset_definition_id: governance.body.asset_definition_id.clone(),
        asset_binding_salt: governance.body.asset_binding_salt,
        amount: data.amount,
        sponsor_reimbursement_amount: reimbursement,
        fee_intent_digest: intent.fee_intent_digest,
        settlement_expiry_height: intent.expiry_height,
        reimbursement_terms_salt: private_settlement_reimbursement_terms_salt_for_terms(
            intent.network_id,
            intent.authority_context_height,
            intent.expiry_height,
            intent.fee_intent_digest,
            governance,
        ),
        memo: data.memo.clone(),
        policy_references: vec![governance.governance_digest],
        inputs: input_openings,
        outputs,
    };
    let inputs = plan_atomic_private_settlement_initial_inputs_v1(
        governance.body.pool_id,
        funding.input_commitments,
        spending_secrets,
    )?;
    Ok(AtomicPrivateSettlementAuthorizedWalletLegV1::new(
        plaintext, inputs,
    ))
}
