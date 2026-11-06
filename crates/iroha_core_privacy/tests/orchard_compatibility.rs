//! Immutable Orchard profile and capability-row regressions for additive protocol work.

use iroha_core_privacy::{
    privacy_engines::orchard::{
        ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1, OrchardActionPublicV1, OrchardBundlePublicV1,
        derive_orchard_bundle_sighash_v1, orchard_authorization_wire_size_v1,
        verify_orchard_bundle_v1,
    },
    privacy_profiles::{compiled_privacy_profile_catalog_v1, compiled_privacy_profile_v1},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::privacy::{
    OrchardLimitsV1, PrivacyCompiledProfileCatalogRowV1, PrivacyCompiledProfileResultV1,
    PrivacyConsensusLimitsV1, PrivacyEngineIdV1, PrivacyEngineManifestDigestV1,
    PrivacyNativeConsensusBindingV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
    PrivacyProofSystemIdV1, PrivacyRuntimeProtocolIdV1, PrivacyRuntimeProtocolLimitsV1,
    PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
    PrivacyVerifierDigestV1,
};
use iroha_data_model::{NetworkId, block::BlockHeader};
use orchard::{
    Anchor,
    builder::{Builder, BundleType},
    bundle::{BundleVersion, Flags},
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::{FullViewingKey, Scope, SpendingKey},
    note::NoteVersion,
    value::NoteValue,
};
use rand::{SeedableRng as _, rngs::StdRng};
use sha2::{Digest as _, Sha256};

#[test]
fn orchard_compatibility_v1_catalog_row_preserves_profile_identity() {
    let catalog = compiled_privacy_profile_catalog_v1().expect("historical v1 catalog");
    assert_eq!(catalog.version, 1);
    assert_eq!(catalog.protocols.len(), 12);
    // Captured before the Ironwood extension from the deployed-equivalent source.
    assert_eq!(
        hex::encode(Sha256::digest(norito::encode_canonical(&catalog).unwrap())),
        "796ee6ea8e9e4810f10e4b38259f09861bd3b3da094cd2c2eb959ad259aee0bd"
    );
    let row = catalog.protocols[8];
    assert_eq!(
        row.protocol_id,
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1
    );
    let PrivacyCompiledProfileResultV1::Available(profile) = row.compiled_profile else {
        panic!("the existing Orchard engine must remain available");
    };
    assert_eq!(
        profile,
        compiled_privacy_profile_v1(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("historical Orchard profile")
            .snapshot(),
    );
    assert_eq!(
        [
            hex::encode(profile.parameter_id.as_bytes()),
            hex::encode(profile.parameter_digest.as_bytes()),
            hex::encode(profile.verifier_digest.as_bytes()),
            hex::encode(profile.statement_schema_digest.as_bytes()),
            hex::encode(profile.engine_manifest_digest.as_bytes()),
        ],
        [
            // Deployed baseline evidence supplied by Claude on 2026-10-31: Taira
            // a660d601 (local tree cade8d5a) matches 89b71821 privacy sources and
            // Cargo.lock by blob OID: Orchard 0.16.0 / Halo2 0.4.1. Earlier goldens
            // predated those dependency upgrades; these values freeze deployed identities.
            "d53bdf99bb4a112d018efccfd949579d1bd917b5fef086b918594033b29eee43",
            "2c2d3d4f1901a1067a5653b1e3770c9e99b05dd1ebbdddb1e69cc321174ff02a",
            "0c1d3ba20ea6ae8f3c49474342d3db39d9bc13edb7f1d849be220ae1312d6688",
            "2141dd88d579b2460cfb0f79f230bfb2f71663364d8700c8e7e6b3e8f57c5a2a",
            "9f981ac374079e1411925f2795cc10c76b71b27dc52536fa56a1a5634754ac91",
        ],
        "existing Orchard profile identities must remain byte-for-byte unchanged",
    );
    assert_eq!(
        profile.proof_system_id,
        PrivacyProofSystemIdV1::Halo2IpaPasta
    );
    assert_eq!(profile.engine_id, PrivacyEngineIdV1::NativeHalo2Orchard);
    assert_eq!(
        profile.protocol_limits,
        PrivacyRuntimeProtocolLimitsV1::OrchardHalo2ActionsV1(OrchardLimitsV1 {
            max_action_count: 2
        }),
    );
    let bytes = norito::encode_canonical(&row).expect("historical capability row");
    assert_eq!(
        norito::decode_canonical::<PrivacyCompiledProfileCatalogRowV1>(&bytes)
            .expect("restore historical capability row"),
        row,
    );
}

#[test]
fn orchard_compatibility_v3_flags_and_recipient_restriction_are_frozen() {
    let version = BundleVersion::orchard_v3();
    assert_eq!(version.default_flags(), Flags::CROSS_ADDRESS_DISABLED);
    assert_eq!(version.note_version(), NoteVersion::V2);
    assert_eq!(version.circuit_version(), OrchardCircuitVersion::PostNu6_3);
    assert_eq!(ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1, *b"ORC1");
    assert_eq!(orchard_authorization_wire_size_v1(1), Some(5125));
    assert_eq!(orchard_authorization_wire_size_v1(2), Some(7461));
    let bundle_type = BundleType::Transactional {
        bundle_required: true,
        pad_to_minimum: Some(1),
    };
    assert!(Builder::new(bundle_type, version, Flags::ENABLED, Anchor::empty_tree()).is_err());
    let mut builder = Builder::new(
        bundle_type,
        version,
        Flags::CROSS_ADDRESS_DISABLED,
        Anchor::empty_tree(),
    )
    .expect("original Orchard flags");
    let recipient = FullViewingKey::from(
        &Option::<SpendingKey>::from(SpendingKey::from_bytes([0x71; 32]))
            .expect("fixed valid spending key"),
    )
    .address_at(0_u32, Scope::External);
    assert!(matches!(
        builder.add_output(None, recipient, NoteValue::from_raw(17), [0; 512]),
        Err(orchard::builder::OutputError::CrossAddressDisabled)
    ));
}

/// Reproduce the existing internal Orchard known-answer vector using only public APIs.
#[test]
fn orchard_compatibility_prover_and_verifier_known_answer_is_frozen() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let seed = 0x44_u8;
    let genesis_hash = Hash::prehashed([seed.wrapping_add(6); 32]);
    let context = PrivacyStatementContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            genesis_hash,
        )),
        action_index: 0,
        transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([seed; 32]),
        parameter_id: PrivacyParameterIdV1::new([seed.wrapping_add(1); 32]),
        parameter_digest: PrivacyParameterDigestV1::new([seed.wrapping_add(2); 32]),
        verifier_digest: PrivacyVerifierDigestV1::new([seed.wrapping_add(3); 32]),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new([seed.wrapping_add(4); 32]),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new([seed.wrapping_add(5); 32]),
    };
    let consensus_binding =
        PrivacyNativeConsensusBindingV1::new(&context, genesis_hash.into(), &limits)
            .expect("original canonical consensus binding");
    let version = BundleVersion::orchard_v3();
    let mut rng = StdRng::from_seed([0xA7; 32]);
    let unsigned = Builder::new(
        BundleType::Transactional {
            bundle_required: true,
            pad_to_minimum: Some(1),
        },
        version,
        version.default_flags(),
        Anchor::empty_tree(),
    )
    .expect("original Orchard builder")
    .build::<i64>(&mut rng)
    .expect("build original dummy action")
    .expect("bundle required")
    .0;
    let proven = unsigned
        .create_proof(
            &ProvingKey::build(OrchardCircuitVersion::PostNu6_3),
            &mut rng,
        )
        .expect("prove original PostNu6_3 action");
    let public = OrchardBundlePublicV1 {
        consensus_binding,
        anchor: proven.anchor().to_bytes(),
        value_balance: *proven.value_balance(),
        actions: proven
            .actions()
            .iter()
            .map(|action| OrchardActionPublicV1 {
                nullifier: action.nullifier().to_bytes(),
                randomized_key: <[u8; 32]>::from(action.rk()),
                note_commitment: action.cmx().to_bytes(),
                ephemeral_key: action.encrypted_note().epk_bytes,
                encrypted_note: action.encrypted_note().enc_ciphertext.0,
                outgoing_ciphertext: action.encrypted_note().out_ciphertext,
                value_commitment: action.cv_net().to_bytes(),
            })
            .collect(),
    };
    let sighash =
        derive_orchard_bundle_sighash_v1(&public, &limits).expect("original bound sighash");
    let authorized = proven
        .apply_signatures(&mut rng, sighash, &[])
        .expect("authorize original bundle");
    let mut authorization = ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1.to_vec();
    authorization.push(1);
    authorization.extend_from_slice(authorized.authorization().proof().as_ref());
    for action in authorized.actions().iter() {
        authorization.extend_from_slice(&<[u8; 64]>::from(action.authorization()));
    }
    authorization.extend_from_slice(&<[u8; 64]>::from(
        authorized.authorization().binding_signature(),
    ));
    assert_eq!(authorization.len(), 5125);

    let mut public_bytes = consensus_binding
        .digest()
        .expect("binding digest")
        .as_bytes()
        .to_vec();
    public_bytes.extend_from_slice(&public.anchor);
    public_bytes.extend_from_slice(&public.value_balance.to_be_bytes());
    public_bytes.push(1);
    for action in &public.actions {
        public_bytes.extend_from_slice(&action.nullifier);
        public_bytes.extend_from_slice(&action.randomized_key);
        public_bytes.extend_from_slice(&action.note_commitment);
        public_bytes.extend_from_slice(&action.ephemeral_key);
        public_bytes.extend_from_slice(&action.encrypted_note);
        public_bytes.extend_from_slice(&action.outgoing_ciphertext);
        public_bytes.extend_from_slice(&action.value_commitment);
    }
    assert_eq!(
        [
            hex::encode(sighash),
            hex::encode(Sha256::digest(public_bytes)),
            hex::encode(Sha256::digest(&authorization)),
        ],
        [
            "6e6c49cd734cb573fb7a506816dc8ca09febd4b219279b9e68d68ef11320543e",
            "9707670460bcc99e65052e24b5919c8edcdd2822ba360b1eda20430095cd320b",
            "96091d8cc928d74fd94778b6a293d3b58b7961f40e99dbb1d0d061c1b1738fca",
        ],
        "original prover bytes and signature framing must not drift",
    );
    verify_orchard_bundle_v1(&public, &authorization, &limits)
        .expect("original proof independently verifies");
    let mut changed_public = public.clone();
    changed_public.value_balance = 1;
    assert!(verify_orchard_bundle_v1(&changed_public, &authorization, &limits).is_err());
    let mut changed_public = public.clone();
    changed_public.anchor[0] ^= 1;
    assert!(verify_orchard_bundle_v1(&changed_public, &authorization, &limits).is_err());
    let mut changed_public = public.clone();
    changed_public.actions[0].encrypted_note[317] ^= 0x80;
    assert!(verify_orchard_bundle_v1(&changed_public, &authorization, &limits).is_err());
    let mut changed_authorization = authorization.clone();
    changed_authorization[4] = 2;
    assert!(verify_orchard_bundle_v1(&public, &changed_authorization, &limits).is_err());
    changed_authorization = authorization;
    changed_authorization[0] ^= 1;
    assert!(verify_orchard_bundle_v1(&public, &changed_authorization, &limits).is_err());
}
