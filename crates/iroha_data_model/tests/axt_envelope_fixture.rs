//! Regression guard for the AXT envelope fixtures (handles/proofs/touches).

use hex::encode;
use iroha_data_model::nexus::{
    AxtDescriptor, AxtHandleFragment, AxtProofFragment, TouchManifest, compute_descriptor_binding,
    proof_matches_manifest, validate_descriptor,
};
use norito::{json, to_bytes};

#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct DescriptorFixture {
    descriptor: AxtDescriptor,
    touch_manifest: Vec<iroha_data_model::nexus::AxtTouchFragment>,
    binding_hex: String,
    descriptor_hex: String,
}

#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct HandleFixtures {
    happy: Vec<AxtHandleFragment>,
    rejects: Vec<AxtHandleFragment>,
}

#[derive(Debug, Clone, norito::json::JsonDeserialize)]
struct EnvelopeFixture {
    descriptor_hex: String,
    binding_hex: String,
    proofs: Vec<AxtProofFragment>,
    handles: HandleFixtures,
}

#[test]
fn envelope_fixtures_align_with_descriptor_binding() {
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
            proof_matches_manifest(&proof.proof, proof.dsid, manifest_root_bytes),
            "proof should bind to manifest root for dsid {}",
            proof.dsid.as_u64()
        );
    }

    for handle in &envelope.handles.happy {
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
    }

    let mut reject_seen = false;
    for handle in &envelope.handles.rejects {
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
