//! Shared canonical atomic private settlement material fixtures.
use super::audit::{
    private_settlement_audit_plaintext_commitment_v1,
    seal_private_settlement_audit_capsule_v1_with_rng,
};
use crate::privacy_engines::{
    atomic_private_settlement::*,
    ivm_private_note::{
        PrivateNotePlaintextV1, PrivateNoteRelationProfileV1, derive_note_authority_v1,
        derive_profiled_input_commitment_v1, derive_profiled_output_commitment_v1,
        encrypt_ivm_private_wallet_note_for_commitment_with_opening_v1,
        ivm_private_recipient_public_key_v1,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, HybridKeyPair, KeyPair, Signature, SignatureOf};
use iroha_data_model::transaction::FeePaymentIntent;
use iroha_data_model::{
    NetworkId, account::AccountId, asset::AssetDefinitionId, block::BlockHeader, nexus::*,
    privacy::*,
};
use iroha_model_base::{
    domain::DomainId,
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use rand_08::{SeedableRng as _, rngs::StdRng};
#[doc(hidden)]
pub struct ApsSidecarPartsV1 {
    pub manifest: AtomicPrivateSettlementV1,
    pub policy: PrivateSettlementAuditPolicyV1,
    pub authority: PrivateSettlementCommitteeAuthorityV1,
    pub payload: PrivateSettlementLegPayloadV1,
}
#[doc(hidden)]
pub struct ApsSidecarMaterialFixtureV1 {
    pub sidecar: ApsSidecarPartsV1,
    pub validator: PeerId,
    pub auditor: AccountId,
    pub signing: KeyPair,
    pub hybrid: HybridKeyPair,
    pub additional_auditors: Vec<ApsAuditorCredentialV1>,
    pub validator_keys: Vec<KeyPair>,
    pub pool_governance: PrivateSettlementPoolGovernanceV1,
    pub plaintext: PrivateSettlementAuditPlaintextV1,
}

#[doc(hidden)]
pub struct ApsAuditorCredentialV1 {
    pub auditor: AccountId,
    pub signing: KeyPair,
    pub hybrid: HybridKeyPair,
}

#[doc(hidden)]
pub fn hash(seed: u8) -> Hash {
    Hash::new([seed])
}

#[doc(hidden)]
pub fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(hash(seed)))
}

#[doc(hidden)]
pub fn route(dataspace: u64) -> PrivateSettlementRouteV1 {
    PrivateSettlementRouteV1 {
        dataspace_id: DataSpaceId::new(dataspace),
        lane_id: LaneId::new(u32::try_from(dataspace).expect("fixture lane fits")),
        lane_incarnation: hash(u8::try_from(dataspace + 20).expect("fixture seed fits")),
    }
}

#[doc(hidden)]
pub fn encrypted_outputs() -> Vec<PrivacyEncryptedOutputV1> {
    (0_u8..3)
        .map(|index| {
            let commitment = PrivacyCommitmentV1::new([0x40 + index; 32]);
            let mut ciphertext = vec![0x80 + index; PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
            ciphertext[..4].copy_from_slice(b"IPNE");
            PrivacyEncryptedOutputV1 {
                recipient: PrivacyRecipientIdV1::new([0x50 + index; 32]),
                ephemeral_public_key: PrivacyEncryptionKeyV1::new([0x60 + index; 32]),
                commitment,
                ciphertext,
            }
        })
        .collect()
}

#[doc(hidden)]
pub fn other_leg_delta_v1(
    manifest: &AtomicPrivateSettlementV1,
    first: &PrivateSettlementDeltaV1,
) -> PrivateSettlementDeltaV1 {
    let leg = manifest.legs[usize::from(first.leg_ordinal == 0)];
    let mut second = first.clone();
    second.leg_ordinal = leg.ordinal;
    second.route = leg.route;
    second.pool_id = leg.pool_id;
    second.asset_binding_commitment = leg.asset_binding_commitment;
    second.audit_policy_digest = leg.audit_policy_digest;
    for (index, output) in second.encrypted_outputs.iter_mut().enumerate() {
        output.recipient = PrivacyRecipientIdV1::new(
            [0xD0_u8 + u8::try_from(index).expect("fixed output ordinal fits u8"); 32],
        );
    }
    second
}

#[doc(hidden)]
pub fn active_opening(seed: u8, value: u128) -> PrivateSettlementAuditNoteOpeningV1 {
    PrivateSettlementAuditNoteOpeningV1 {
        active: true,
        commitment: PrivacyCommitmentV1::new([seed; 32]),
        value,
        spending_authority: [seed.wrapping_add(1); 32],
        rho: [seed.wrapping_add(2); 32],
        blinding: [seed.wrapping_add(3); 32],
        memo_digest: [seed.wrapping_add(4); 32],
        dummy_domain: None,
    }
}

#[doc(hidden)]
pub fn dummy_opening(seed: u8) -> PrivateSettlementAuditNoteOpeningV1 {
    PrivateSettlementAuditNoteOpeningV1 {
        active: false,
        commitment: PrivacyCommitmentV1::new([seed; 32]),
        value: 0,
        spending_authority: [seed.wrapping_add(2); 32],
        rho: [seed.wrapping_add(3); 32],
        blinding: [seed.wrapping_add(4); 32],
        memo_digest: [seed.wrapping_add(5); 32],
        dummy_domain: Some(hash(seed.wrapping_add(1))),
    }
}

#[doc(hidden)]
pub fn encryption_opening(seed: u8) -> PrivateSettlementAuditEncryptionOpeningV1 {
    PrivateSettlementAuditEncryptionOpeningV1 {
        ephemeral_secret: core::array::from_fn(|index| {
            seed.wrapping_add(u8::try_from(index).expect("opening index fits u8"))
        }),
    }
}

#[doc(hidden)]
pub fn placeholder_view_key_authorization(
    signing: &KeyPair,
) -> PrivateSettlementAuditViewKeyAuthorizationV1 {
    let body = PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        purpose: hash(1),
        network_id: network(1),
        bundle_id: hash(2),
        leg_ordinal: 0,
        route: route(7),
        output_ordinal: 0,
        role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
        authorized_account: AccountId::new(signing.public_key().clone()),
        recipient_view_key: [1; 32],
        output_active: true,
        note_spending_authority: [2; 32],
        expiry_height: 1,
    };
    PrivateSettlementAuditViewKeyAuthorizationV1::new(
        body.clone(),
        vec![PrivateSettlementAuditViewKeySignatureV1::new(
            signing.public_key().clone(),
            SignatureOf::try_new(signing.private_key(), &body)
                .expect("placeholder authorization signs"),
        )],
    )
}

#[doc(hidden)]
pub fn placeholder_payer_authorization(
    signing: &KeyPair,
) -> PrivateSettlementAuditPayerAuthorizationV1 {
    let body = PrivateSettlementAuditPayerAuthorizationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        purpose: hash(1),
        network_id: network(1),
        bundle_id: hash(2),
        leg_ordinal: 0,
        route: route(7),
        payer: AccountId::new(signing.public_key().clone()),
        expiry_height: 1,
        inputs: vec![
            PrivateSettlementAuditPayerInputV1 {
                input_ordinal: 0,
                active: true,
                commitment: PrivacyCommitmentV1::new([1; 32]),
                nullifier: PrivacyNullifierV1::new([2; 32]),
                note_spending_authority: [3; 32],
                dummy_domain: None,
            },
            PrivateSettlementAuditPayerInputV1 {
                input_ordinal: 1,
                active: false,
                commitment: PrivacyCommitmentV1::new([4; 32]),
                nullifier: PrivacyNullifierV1::new([5; 32]),
                note_spending_authority: [6; 32],
                dummy_domain: Some(hash(7)),
            },
        ],
    };
    PrivateSettlementAuditPayerAuthorizationV1::new(
        body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            signing.public_key().clone(),
            SignatureOf::try_new(signing.private_key(), &body)
                .expect("placeholder payer authorization signs"),
        )],
    )
}

#[doc(hidden)]
pub fn authorize_payer_inputs(
    plaintext: &mut PrivateSettlementAuditPlaintextV1,
    nullifiers: &[PrivacyNullifierV1],
    signer: &KeyPair,
) {
    let body = plaintext
        .payer_authorization_body(nullifiers)
        .expect("fixture payer authorization body");
    plaintext.payer_authorization = PrivateSettlementAuditPayerAuthorizationV1::new(
        body.clone(),
        vec![PrivateSettlementAuditPayerSignatureV1::new(
            signer.public_key().clone(),
            SignatureOf::try_new(signer.private_key(), &body)
                .expect("fixture payer authorization signs"),
        )],
    );
}

#[doc(hidden)]
pub fn authorize_output_view_keys(
    plaintext: &mut PrivateSettlementAuditPlaintextV1,
    signers: [&KeyPair; 3],
) {
    for (index, signer) in signers.into_iter().enumerate() {
        let body = plaintext
            .output_view_key_authorization_body(index)
            .expect("fixture authorization body");
        plaintext.outputs[index].view_key_authorization =
            PrivateSettlementAuditViewKeyAuthorizationV1::new(
                body.clone(),
                vec![PrivateSettlementAuditViewKeySignatureV1::new(
                    signer.public_key().clone(),
                    SignatureOf::try_new(signer.private_key(), &body)
                        .expect("fixture authorization signs"),
                )],
            );
    }
}

#[doc(hidden)]
pub fn aps_sidecar_material_fixture_with_threshold_and_ordinal_v1(
    min_approvals: u8,
    ordinal: u8,
) -> ApsSidecarMaterialFixtureV1 {
    assert!(ordinal < 2);
    let local = usize::from(ordinal);
    let other = usize::from(ordinal == 0);
    assert!((1..=2).contains(&min_approvals));
    let route = route(7 + u64::from(ordinal));
    let signing = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let auditor = AccountId::new(signing.public_key().clone());
    let mut hybrid_rng = iroha_crypto::rng_from_seed_slice(b"sidecar auditor encryption key");
    let hybrid = HybridKeyPair::generate(&mut hybrid_rng).expect("hybrid key");
    let mut additional_auditors = Vec::new();
    if min_approvals == 2 {
        let additional_signing = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
        let additional_auditor = AccountId::new(additional_signing.public_key().clone());
        let mut additional_hybrid_rng =
            iroha_crypto::rng_from_seed_slice(b"second sidecar auditor encryption key");
        let additional_hybrid =
            HybridKeyPair::generate(&mut additional_hybrid_rng).expect("second hybrid key");
        additional_auditors.push(ApsAuditorCredentialV1 {
            auditor: additional_auditor,
            signing: additional_signing,
            hybrid: additional_hybrid,
        });
    }
    let mut governed_auditors = vec![PrivateSettlementAuditorV1 {
        auditor_id: auditor.clone(),
        signing_key: signing.public_key().clone(),
        encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(hybrid.public()),
    }];
    governed_auditors.extend(additional_auditors.iter().map(|credential| {
        PrivateSettlementAuditorV1 {
            auditor_id: credential.auditor.clone(),
            signing_key: credential.signing.public_key().clone(),
            encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(
                credential.hybrid.public(),
            ),
        }
    }));
    governed_auditors.sort_by(|left, right| left.auditor_id.cmp(&right.auditor_id));
    let policy = PrivateSettlementAuditPolicyV1::new(PrivateSettlementAuditPolicyBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        dataspace_id: route.dataspace_id,
        policy_id: hash(0x22),
        revision: 1,
        key_epoch: 1,
        activation_height: 5,
        retirement_height: Some(500),
        min_approvals,
        auditors: governed_auditors,
    })
    .expect("policy");
    let validator_keys = (0_u8..4)
        .map(|index| KeyPair::from_seed(vec![0x70 + index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let validators = validator_keys
        .iter()
        .map(|key| PeerId::from(key.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_pops = validator_keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP"))
        .collect();
    let validator = validators[0].clone();
    let authority = PrivateSettlementCommitteeAuthorityV1 {
        route,
        validator_set_hash: HashOf::new(&validators),
        validators,
        validator_pops,
    };
    let authority_digest = authority.digest().expect("authority digest");
    let sponsor_key = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    let mut manifest = AtomicPrivateSettlementV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: network(1),
        bundle_id: hash(0x24),
        authority_context_height: 10,
        expiry_height: 100,
        sponsor: AccountId::new(sponsor_key.public_key().clone()),
        public_fee_intent: FeePaymentIntent::authority(Vec::new(), None),
        fee_intent_digest: hash(0x25),
        reimbursement_terms_commitment: hash(0x26),
        reimbursement_leg_ordinal: ordinal,
        legs: vec![
            PrivateSettlementLegCommitmentV1 {
                ordinal,
                route,
                pool_id: PrivacyPoolIdV1::new([0x27; 32]),
                asset_binding_commitment: hash(0x28),
                audit_policy_digest: policy.policy_digest,
                payload_digest: hash(0x29),
                availability_certificate_digest: hash(0x2A),
                delta_digest: hash(0x2A),
            },
            PrivateSettlementLegCommitmentV1 {
                ordinal: 1 - ordinal,
                route: self::route(8 - u64::from(ordinal)),
                pool_id: PrivacyPoolIdV1::new([0x2B; 32]),
                asset_binding_commitment: hash(0x2C),
                audit_policy_digest: hash(0x2D),
                payload_digest: hash(0x2E),
                availability_certificate_digest: hash(0x30),
                delta_digest: hash(0x2F),
            },
        ],
    };
    manifest.legs.sort_unstable_by_key(|leg| leg.ordinal);
    manifest.fee_intent_digest = manifest
        .computed_fee_intent_digest()
        .expect("fee intent digest");
    let payer = KeyPair::from_seed(vec![0x38; 32], Algorithm::Ed25519);
    let recipient = KeyPair::from_seed(vec![0x39; 32], Algorithm::Ed25519);
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("bank-a", "regulated").expect("fixture domain"),
        "cbdc".parse().expect("fixture asset name"),
    );
    let pool_governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
        route,
        manifest.legs[local].pool_id,
        asset_definition_id.clone(),
        [0x3A; 32],
        &policy,
        PrivateSettlementPoolGovernanceLifecycleV1 {
            governance_revision: 1,
            activation_height: 5,
            retirement_height: Some(500),
        },
    )
    .expect("restricted pool governance");
    let input_spending_secrets = [[0x81; 32], [0x82; 32]];
    let output_spending_secrets = [[0x91; 32], [0x92; 32], [0x93; 32]];
    let output_view_secrets = [[0xA1; 32], [0xA2; 32], [0xA3; 32]];
    let output_view_keys = output_view_secrets
        .map(|secret| ivm_private_recipient_public_key_v1(&secret).expect("recipient view key"));
    let mut plaintext = PrivateSettlementAuditPlaintextV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal,
        route,
        pool_id: manifest.legs[local].pool_id,
        payer: AccountId::new(payer.public_key().clone()),
        payer_authorization: placeholder_payer_authorization(&payer),
        recipient: AccountId::new(recipient.public_key().clone()),
        sponsor: manifest.sponsor.clone(),
        asset_definition_id,
        asset_binding_salt: [0x3A; 32],
        amount: 42,
        sponsor_reimbursement_amount: 5,
        fee_intent_digest: manifest.fee_intent_digest,
        settlement_expiry_height: manifest.expiry_height,
        reimbursement_terms_salt: [0x3B; 32],
        memo: b"settlement memo canary".to_vec(),
        policy_references: vec![pool_governance.governance_digest],
        inputs: vec![active_opening(0x90, 47), dummy_opening(0x91)],
        outputs: vec![
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
                recipient_view_key: output_view_keys[0],
                view_key_authorization: placeholder_view_key_authorization(&recipient),
                encryption_opening: encryption_opening(0xB1),
                note: active_opening(0x40, 42),
            },
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::PayerChange,
                recipient_view_key: output_view_keys[1],
                view_key_authorization: placeholder_view_key_authorization(&payer),
                encryption_opening: encryption_opening(0xC1),
                note: dummy_opening(0x41),
            },
            PrivateSettlementAuditOutputV1 {
                role: PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
                recipient_view_key: output_view_keys[2],
                view_key_authorization: placeholder_view_key_authorization(&sponsor_key),
                encryption_opening: encryption_opening(0xD1),
                note: active_opening(0x42, 5),
            },
        ],
    };
    manifest.legs[local].asset_binding_commitment =
        plaintext.asset_binding_commitment().expect("asset binding");
    manifest.reimbursement_terms_commitment = plaintext
        .reimbursement_terms_commitment()
        .expect("reimbursement terms");
    manifest.bundle_id = manifest.computed_bundle_id().expect("bundle id");
    plaintext.bundle_id = manifest.bundle_id;
    let profile = PrivateSettlementProofProfileV1::IvmPrivateNoteFixed2In3Out;
    let mut encrypted_outputs = encrypted_outputs();
    let mut statement = PrivateSettlementProofStatementV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        profile,
        proof_profile_digest: profile.digest(),
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal,
        route,
        authority_context_height: manifest.authority_context_height,
        pool_id: manifest.legs[local].pool_id,
        asset_binding_commitment: manifest.legs[local].asset_binding_commitment,
        old_root: PrivacyRootV1::new([0x31; 32]),
        new_root: PrivacyRootV1::new([0x34; 32]),
        old_epoch: 1,
        new_epoch: 2,
        nullifiers: vec![
            PrivacyNullifierV1::new([0x32; 32]),
            PrivacyNullifierV1::new([0x33; 32]),
        ],
        output_commitments: encrypted_outputs
            .iter()
            .map(|output| output.commitment)
            .collect(),
        encrypted_outputs: encrypted_outputs.clone(),
        audit_plaintext_commitment: hash(0x38),
        audit_input_commitment: [0x3A; 32],
        audit_capsule_digest: hash(0x39),
        audit_policy_digest: policy.policy_digest,
        audit_key_epoch: policy.body.key_epoch,
        fee_intent_digest: manifest.fee_intent_digest,
        reimbursement_terms_commitment: manifest.reimbursement_terms_commitment,
        reimbursement_leg_ordinal: manifest.reimbursement_leg_ordinal,
        expiry_height: manifest.expiry_height,
    };

    for (opening, secret) in plaintext.inputs.iter_mut().zip(input_spending_secrets) {
        opening.spending_authority = derive_note_authority_v1(&secret).expect("input authority");
    }
    for (output, secret) in plaintext.outputs.iter_mut().zip(output_spending_secrets) {
        output.note.spending_authority =
            derive_note_authority_v1(&secret).expect("output authority");
    }
    authorize_output_view_keys(&mut plaintext, [&recipient, &payer, &sponsor_key]);
    plaintext.inputs[1].memo_digest = atomic_private_settlement_dummy_input_memo_digest_v1(
        &manifest,
        &statement,
        1,
        plaintext.inputs[1]
            .dummy_domain
            .expect("dummy opening carries a domain"),
    )
    .expect("dummy input memo");
    let provisional_relation =
        PrivateNoteRelationProfileV1::exact_three_output_balanced([[0xD1; 32]; 3], [1; 32]);
    for opening in &mut plaintext.inputs {
        let note = PrivateNotePlaintextV1::new_profiled_input_v1(
            opening.value,
            opening.spending_authority,
            opening.rho,
            opening.blinding,
            opening.memo_digest,
            provisional_relation,
        )
        .expect("input note");
        opening.commitment = derive_profiled_input_commitment_v1(&note, provisional_relation)
            .expect("input commitment");
    }
    authorize_payer_inputs(&mut plaintext, &statement.nullifiers, &payer);

    let plaintext_commitment = plaintext.commitment().expect("plaintext commitment");
    statement.audit_plaintext_commitment = plaintext_commitment;
    statement.audit_input_commitment = crate::privacy_engines::atomic_private_settlement::
        atomic_private_settlement_audit_input_commitment_v1(&plaintext.inputs)
            .expect("audit input commitment");
    let output_memos = atomic_private_settlement_output_memo_digests_v1(&manifest, &statement)
        .expect("fixed output memos");
    let settlement_relation = PrivateNoteRelationProfileV1::exact_three_output_balanced(
        output_memos,
        statement.audit_input_commitment,
    );
    let program_id = atomic_private_settlement_program_id_v1().expect("settlement program");
    let mut output_rng = StdRng::seed_from_u64(0x4150_535f_4f55_5450);
    encrypted_outputs.clear();
    for (index, (output, memo)) in plaintext.outputs.iter_mut().zip(output_memos).enumerate() {
        output.note.memo_digest = memo;
        let note = PrivateNotePlaintextV1::new_profiled_output_v1(
            output.note.value,
            output.note.spending_authority,
            output.note.rho,
            output.note.blinding,
            output.note.memo_digest,
            index,
            settlement_relation,
        )
        .expect("output note");
        output.note.commitment =
            derive_profiled_output_commitment_v1(&note, index, settlement_relation)
                .expect("output commitment");
        encrypted_outputs.push(
            encrypt_ivm_private_wallet_note_for_commitment_with_opening_v1(
                &mut output_rng,
                statement.pool_id,
                program_id,
                &note,
                output.note.commitment,
                output.recipient_view_key,
                &output.encryption_opening.ephemeral_secret,
            )
            .expect("encrypted output"),
        );
    }
    statement.output_commitments = plaintext
        .outputs
        .iter()
        .map(|output| output.note.commitment)
        .collect();
    statement.encrypted_outputs.clone_from(&encrypted_outputs);
    plaintext
        .validate_against_manifest(&manifest)
        .expect("audit plaintext");
    assert_eq!(
        plaintext.commitment().expect("stable audit commitment"),
        plaintext_commitment
    );
    let audit_plaintext = norito::encode_canonical(&plaintext).expect("audit plaintext bytes");
    assert_eq!(
        private_settlement_audit_plaintext_commitment_v1(&audit_plaintext)
            .expect("plaintext commitment"),
        plaintext_commitment
    );
    let aad = PrivateSettlementAuditAadV1 {
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        leg_ordinal: ordinal,
        route,
        authority_digest,
        authority_context_height: manifest.authority_context_height,
        audit_policy_digest: policy.policy_digest,
        audit_key_epoch: policy.body.key_epoch,
        plaintext_commitment,
    };
    let mut capsule_rng = iroha_crypto::rng_from_seed_slice(b"sidecar capsule randomness");
    let audit_capsule = seal_private_settlement_audit_capsule_v1_with_rng(
        &audit_plaintext,
        aad,
        PrivateSettlementCapsulePaddingV1::KiB16,
        &policy,
        &mut capsule_rng,
    )
    .expect("capsule");
    let capsule_digest = audit_capsule.digest().expect("capsule digest");
    statement.audit_capsule_digest = capsule_digest;
    statement.validate().expect("proof statement");
    let proof = vec![0xA5; 128];
    let mut payload = PrivateSettlementLegPayloadV1 {
        statement: statement.clone(),
        proof,
        delta: PrivateSettlementDeltaV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            bundle_id: statement.bundle_id,
            leg_ordinal: statement.leg_ordinal,
            route,
            pool_id: statement.pool_id,
            asset_binding_commitment: statement.asset_binding_commitment,
            old_root: statement.old_root,
            new_root: statement.new_root,
            old_epoch: statement.old_epoch,
            new_epoch: statement.new_epoch,
            nullifiers: statement.nullifiers.clone(),
            output_commitments: statement.output_commitments.clone(),
            encrypted_outputs,
            statement_digest: statement.digest().expect("statement digest"),
            proof_digest: hash(0x35),
            capsule_digest,
            audit_policy_digest: policy.policy_digest,
            audit_key_epoch: policy.body.key_epoch,
        },
        audit_capsule,
        availability: PrivateSettlementSidecarAvailabilityV1 {
            body: PrivateSettlementSidecarAvailabilityBodyV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                network_id: manifest.network_id,
                bundle_id: manifest.bundle_id,
                leg_ordinal: statement.leg_ordinal,
                route,
                authority_digest,
                authority_context_height: manifest.authority_context_height,
                payload_digest: hash(0x36),
                payload_bytes: 1,
                retention_until_height: 120,
            },
            signers_bitmap: 0b0111,
            aggregate_signature: vec![1; 96],
        },
    };
    payload.delta.proof_digest = payload.proof_digest();
    manifest.legs[local].delta_digest = payload.delta.digest().expect("delta digest");
    let second_delta = other_leg_delta_v1(&manifest, &payload.delta);
    manifest.legs[other].delta_digest = second_delta.digest().expect("second delta digest");
    let payload_digest = payload.payload_digest().expect("payload digest");
    payload.availability.body.payload_digest = payload_digest;
    manifest.legs[local].payload_digest = payload_digest;
    payload.availability.body.payload_bytes = u32::try_from(
        payload
            .sidecar_material_bytes_len()
            .expect("canonical sidecar material length"),
    )
    .expect("payload fits u32");
    let availability_preimage = payload
        .availability
        .signature_preimage()
        .expect("availability preimage");
    let availability_signatures = validator_keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), &availability_preimage)
                .expect("availability signature")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = availability_signatures
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    payload.availability.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("availability aggregate");
    manifest.legs[local].availability_certificate_digest = payload
        .availability
        .digest()
        .expect("availability certificate digest");
    manifest.validate().expect("manifest");
    payload
        .validate_against(&manifest, &policy)
        .expect("payload");

    let sidecar = ApsSidecarPartsV1 {
        manifest,
        policy,
        authority,
        payload,
    };
    ApsSidecarMaterialFixtureV1 {
        sidecar,
        validator,
        auditor,
        signing,
        hybrid,
        additional_auditors,
        validator_keys,
        pool_governance,
        plaintext,
    }
}

/// Build one valid fixture with the given approval threshold.
#[doc(hidden)]
pub fn aps_sidecar_material_fixture_with_threshold_v1(min: u8) -> ApsSidecarMaterialFixtureV1 {
    aps_sidecar_material_fixture_with_threshold_and_ordinal_v1(min, 0)
}
/// Build the canonical single-auditor fixture.
#[doc(hidden)]
pub fn aps_sidecar_material_fixture_v1() -> ApsSidecarMaterialFixtureV1 {
    aps_sidecar_material_fixture_with_threshold_v1(1)
}
/// Derive provisional material from public fixture parts.
#[doc(hidden)]
pub fn aps_provisional_material_fixture_v1(
    manifest: &AtomicPrivateSettlementV1,
    policy: &PrivateSettlementAuditPolicyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    payload: &PrivateSettlementLegPayloadV1,
) -> PrivateSettlementProvisionalLegMaterialV1 {
    let mut manifest = manifest.clone();
    for leg in &mut manifest.legs {
        leg.availability_certificate_digest = Hash::prehashed([0; Hash::LENGTH]);
    }
    let material = PrivateSettlementProvisionalLegMaterialV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest,
        audit_policy: policy.clone(),
        committee_authority: authority.clone(),
        statement: payload.statement.clone(),
        proof: payload.proof.clone(),
        delta: payload.delta.clone(),
        audit_capsule: payload.audit_capsule.clone(),
        availability_body: payload.availability.body,
    };
    material.validate().expect("provisional material fixture");
    material
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_fixture_keeps_validated_public_parts_and_provisional_binding() {
        let fixture = aps_sidecar_material_fixture_v1();
        fixture.sidecar.manifest.validate().expect("manifest");
        fixture
            .sidecar
            .payload
            .validate_against(&fixture.sidecar.manifest, &fixture.sidecar.policy)
            .expect("payload");
        let material = aps_provisional_material_fixture_v1(
            &fixture.sidecar.manifest,
            &fixture.sidecar.policy,
            &fixture.sidecar.authority,
            &fixture.sidecar.payload,
        );
        material.validate().expect("provisional material");
        assert_eq!(material.statement, fixture.sidecar.payload.statement);
        assert_eq!(material.proof, fixture.sidecar.payload.proof);
        assert_eq!(material.committee_authority, fixture.sidecar.authority);
    }

    #[test]
    fn material_fixture_covers_both_leg_ordinals_and_auditor_thresholds() {
        let threshold = aps_sidecar_material_fixture_with_threshold_v1(2);
        assert_eq!(threshold.sidecar.policy.body.min_approvals, 2);
        assert_eq!(threshold.additional_auditors.len(), 1);
        for ordinal in 0..2 {
            let fixture = aps_sidecar_material_fixture_with_threshold_and_ordinal_v1(2, ordinal);
            assert_eq!(fixture.sidecar.payload.statement.leg_ordinal, ordinal);
            fixture
                .sidecar
                .payload
                .validate_against(&fixture.sidecar.manifest, &fixture.sidecar.policy)
                .expect("ordinal payload");
        }
    }
}
