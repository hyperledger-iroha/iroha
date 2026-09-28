//! Regression guard for the final AXT anchored-spend fixtures and descriptor binding.
use hex::encode;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::nexus::{
    AxtAnchoredSpendV1, AxtDescriptor, AxtHandleReplayKey, AxtProofEnvelope, AxtProofFragment,
    AxtRemoteSpendClaimV1, TouchManifest, compute_descriptor_binding,
    compute_remote_spend_claim_commitment_v1, proof_envelope_shape_matches_manifest,
    validate_descriptor,
};
use norito::{decode_from_bytes, json, to_bytes};
#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct DescriptorFixture {
    descriptor: AxtDescriptor,
    touch_manifest: Vec<iroha_data_model::nexus::AxtTouchFragment>,
    binding_hex: String,
    descriptor_hex: String,
}
#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct AnchoredSpendFixtures {
    happy: Vec<AxtAnchoredSpendV1>,
    rejects: Vec<AxtAnchoredSpendV1>,
}
#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct EnvelopeFixture {
    descriptor_hex: String,
    binding_hex: String,
    proofs: Vec<AxtProofFragment>,
    spends: AnchoredSpendFixtures,
}
#[test]
fn envelope_fixtures_align_with_descriptor_binding() {
    let fixture_json = include_str!("fixtures/axt_envelope_multi_ds.json");
    let retired_transport = fixture_json.replacen("\"spends\":", "\"handles\":", 1);
    assert_ne!(
        retired_transport, fixture_json,
        "fixture uses final V1 spends"
    );
    assert!(
        json::from_str::<EnvelopeFixture>(&retired_transport).is_err(),
        "retired handle-fragment envelope must not decode as anchored V1"
    );
    let descriptor: DescriptorFixture =
        json::from_slice(include_bytes!("fixtures/axt_descriptor_multi_ds.json"))
            .expect("descriptor fixture decodes");
    let envelope: EnvelopeFixture =
        json::from_slice(include_bytes!("fixtures/axt_envelope_multi_ds.json"))
            .expect("envelope fixture decodes");
    validate_descriptor(&descriptor.descriptor).expect("fixture descriptor is valid");
    let binding = compute_descriptor_binding(&descriptor.descriptor).expect("binding computed");
    assert_eq!(encode(binding), descriptor.binding_hex);
    assert_eq!(descriptor.binding_hex, envelope.binding_hex);
    assert_eq!(descriptor.descriptor_hex, envelope.descriptor_hex);
    let manifest_for = |dsid| -> TouchManifest {
        descriptor
            .touch_manifest
            .iter()
            .find(|fragment| fragment.dsid == dsid)
            .map(|fragment| fragment.manifest.clone())
            .expect("manifest for dataspace present")
    };
    for proof in &envelope.proofs {
        let manifest = manifest_for(proof.dsid);
        let manifest_root = iroha_crypto::Hash::new(to_bytes(&manifest).expect("manifest encodes"));
        let manifest_root_bytes: [u8; 32] = *manifest_root.as_ref();
        assert!(
            proof_envelope_shape_matches_manifest(&proof.proof, proof.dsid, manifest_root_bytes),
            "proof should bind to manifest root for dsid {}",
            proof.dsid.as_u64()
        );
    }
    let issuer = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
    for spend in &envelope.spends.happy {
        let handle = &spend.draft;
        spend
            .verify_issuer_signatures_v1(
                handle.handle.issuer_context,
                spend.authorization.anchor,
                issuer.public_key(),
            )
            .expect("fixture spend has both valid issuer signatures");
        assert_eq!(
            handle.handle.axt_binding.as_bytes(),
            &binding,
            "handle binding must match descriptor"
        );
        let manifest = manifest_for(handle.intent.asset_dsid);
        let manifest_root = iroha_crypto::Hash::new(to_bytes(&manifest).expect("manifest encodes"));
        let manifest_root_bytes: [u8; 32] = *manifest_root.as_ref();
        assert_eq!(
            handle.handle.manifest_view_root, manifest_root_bytes,
            "handle manifest root must reflect fixture manifest"
        );
        assert_eq!(
            handle.handle.asset_definition_id, handle.intent.op.asset_definition_id,
            "the issuer-signed handle asset must match the remote-spend intent"
        );
        let effective_amount = handle.amount.as_ref().expect("happy fixture amount");
        assert_eq!(
            handle.intent.op.amount.as_ref(),
            Some(effective_amount),
            "fixture clear amount must agree with the intent"
        );
        let attached_proof = handle.proof.as_ref().expect("happy fixture proof");
        let proof_fragment = envelope
            .proofs
            .iter()
            .find(|proof| proof.dsid == handle.intent.asset_dsid)
            .expect("proof fragment for handle dataspace");
        assert_eq!(
            attached_proof, &proof_fragment.proof,
            "handle must carry the canonical proof fragment for its dataspace"
        );
        let proof_envelope: AxtProofEnvelope = decode_from_bytes(&attached_proof.payload)
            .expect("fixture proof payload decodes canonically");
        assert_eq!(
            effective_amount.scale(),
            0,
            "clear fixture uses integer units"
        );
        assert_eq!(
            proof_envelope.committed_amount,
            effective_amount.as_numeric().try_mantissa_u128(),
            "proof scalar must exactly bind the signed clear amount"
        );
        assert_eq!(
            proof_envelope.amount_commitment, None,
            "clear fixture must not carry a hidden-amount commitment"
        );
        assert_eq!(proof_envelope.dsid, handle.intent.asset_dsid);
        assert_eq!(proof_envelope.manifest_root, manifest_root_bytes);
        assert_eq!(
            proof_envelope.da_commitment,
            Some(spend.authorization.anchor.da_manifest_digest.into()),
            "proof DA commitment must equal the signed finalized anchor"
        );
        assert_eq!(
            to_bytes(&proof_envelope).expect("re-encode proof envelope"),
            attached_proof.payload,
            "decoded proof envelope must round-trip byte-identically"
        );
        let fastpq_binding = proof_envelope
            .fastpq_binding
            .as_ref()
            .expect("happy fixture FASTPQ binding");
        let effect_binding = fastpq_binding
            .effect_binding
            .as_ref()
            .expect("happy fixture effect binding");
        let signed_asset_literal = handle.handle.asset_definition_id.to_string();
        assert_eq!(
            effect_binding.source_asset_definition_id.as_deref(),
            Some(signed_asset_literal.as_str()),
            "decoded proof effect must name the exact issuer-signed asset"
        );
        let claim = AxtRemoteSpendClaimV1::new(
            AxtHandleReplayKey::from_handle(handle.intent.asset_dsid, &handle.handle),
            handle.handle.asset_definition_id.clone(),
            &handle.intent.op.kind,
            &handle.intent.op.from,
            &handle.intent.op.to,
            effective_amount.clone(),
        );
        assert_eq!(
            fastpq_binding.remote_spend_intent_commitments,
            vec![compute_remote_spend_claim_commitment_v1(&claim)],
            "decoded proof must commit to the exact canonical remote-spend preimage"
        );
    }
    let mut reject_seen = false;
    for spend in &envelope.spends.rejects {
        let handle = &spend.draft;
        assert!(
            spend
                .verify_issuer_signatures_v1(
                    handle.handle.issuer_context,
                    spend.authorization.anchor,
                    issuer.public_key(),
                )
                .is_err(),
            "reject fixture cannot be issuer authorized"
        );
        if handle.handle.axt_binding.as_bytes() != &binding {
            reject_seen = true;
        }
        if handle
            .handle
            .manifest_view_root
            .iter()
            .all(|byte| *byte == 0)
        {
            reject_seen = true;
        }
    }
    assert!(reject_seen, "reject fixtures should include mismatches");
}
