use std::{
    fmt,
    os::unix::fs::PermissionsExt as _,
    sync::{Arc, Mutex},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_torii::sorafs::{PotrProviderSignerV1 as _, StreamTokenRuntimeSigner as _};
use sorafs_manifest::StreamTokenBodyV1;
use sorafs_node::{
    GovernanceDagRuntimeSigner as _, evidence_viewer::EvidenceViewerReceiptSignerV1 as _,
    hedging_billing_service::BillingStatementRuntimeSigner as _,
    pop_credentials::PopIssuerSigner as _,
};
use super::{
    ExternalSoftwareSignerAdapterErrorV1, ExternalSoftwareSignerBackendsV1,
    ExternalSoftwareSignerBillingStatementAdapterV1, ExternalSoftwareSignerEvidenceViewerAdapterV1,
    ExternalSoftwareSignerGovernanceDagAdapterV1, ExternalSoftwareSignerPopIssuerAdapterV1,
    ExternalSoftwareSignerPotrProviderAdapterV1, ExternalSoftwareSignerStreamTokenAdapterV1,
    SoftwareSignerClientV1, SoftwareSignerKeyAlgorithmV1, SoftwareSignerProvisioningV1,
    SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1, SoftwareSignerServiceV1,
    SoftwareSignerWrappingKeyV1,
};
const TEST_WRAP_KEY: [u8; 32] = [0xD1; 32];
fn direct_signer(
    role: SoftwareSignerRoleV1,
    purpose_binding: SoftwareSignerPurposeBindingV1,
    algorithm: SoftwareSignerKeyAlgorithmV1,
) -> (tempfile::TempDir, Arc<SoftwareSignerServiceV1>) {
    let parent = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("secure temporary parent");
    std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o700))
        .expect("secure temporary parent permissions");
    let service_uid = rustix::process::geteuid().as_raw();
    let role_name = match role {
        SoftwareSignerRoleV1::GovernanceDag => "governance-dag",
        SoftwareSignerRoleV1::PotrProvider => "potr",
        SoftwareSignerRoleV1::BillingStatement => "billing",
        SoftwareSignerRoleV1::EvidenceViewer => "evidence-viewer",
        SoftwareSignerRoleV1::StreamToken => "stream-token",
        SoftwareSignerRoleV1::PopCredentials => "pop-credentials",
        _ => panic!("unsupported direct adapter fixture role"),
    };
    let instance = if role == SoftwareSignerRoleV1::PotrProvider {
        "provider-primary"
    } else {
        "primary"
    };
    let service = SoftwareSignerServiceV1::provision(
        parent.path().join("state"),
        SoftwareSignerProvisioningV1 {
            handle: format!("software://sorafs/{role_name}/{instance}"),
            service_id: format!("{}-service-primary", role.as_str()),
            administrator_id: format!("{}-admin-primary", role.as_str()),
            service_uid,
            client_uid: service_uid.checked_add(1).expect("client uid"),
            administrator_uid: service_uid.checked_add(2).expect("administrator uid"),
            role,
            purpose_binding,
            algorithm,
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0xD2; 32],
            max_request_bytes: 1024 * 1024,
        },
        SoftwareSignerWrappingKeyV1::try_from_bytes(TEST_WRAP_KEY).expect("wrapping key"),
    )
    .expect("provision direct signer");
    (parent, Arc::new(service))
}
fn direct_client(service: &Arc<SoftwareSignerServiceV1>) -> SoftwareSignerClientV1 {
    SoftwareSignerClientV1::new_direct(Arc::clone(service)).expect("direct test client")
}
fn unsigned_potr_payload(provider_id: [u8; 32]) -> Vec<u8> {
    sorafs_manifest::PotrReceiptV1 {
        version: sorafs_manifest::POTR_RECEIPT_VERSION_V1,
        manifest_digest: [0x21; 32],
        provider_id,
        tier: sorafs_manifest::proof_stream::ProofStreamTier::Hot,
        deadline_ms: 90_000,
        latency_ms: 42_000,
        status: sorafs_manifest::PotrStatus::Success,
        requested_at_ms: 1_700_000_000_000,
        responded_at_ms: 1_700_000_042_000,
        recorded_at_ms: 1_700_000_042_100,
        range_start: 0,
        range_end: 1_048_575,
        request_id: Some([0x22; 16]),
        trace_id: Some([0x23; 16]),
        note: None,
        gateway_signature: None,
        provider_signature: None,
    }
    .signing_payload_bytes()
    .expect("PoTR signing payload")
}
fn evidence_checkpoint_anchor_message(binding: &super::SoftwareSignerPublicBindingV1) -> Vec<u8> {
    let checkpoint_handle = "runtime://sorafs/evidence-viewer/checkpoint-store/primary";
    let (_, public_key) = binding
        .public_key
        .try_to_bytes()
        .expect("fixture public key bytes");
    let mut message = b"sorafs.evidence-viewer.checkpoint-signature.v1".to_vec();
    message.extend_from_slice(
        &sorafs_node::evidence_viewer::EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1.to_le_bytes(),
    );
    message.extend_from_slice(&1_u64.to_le_bytes());
    message.extend_from_slice(&[0, 0]);
    message.extend_from_slice(&[0x61; 32]);
    message.extend_from_slice(&0_u64.to_le_bytes());
    message.extend_from_slice(&[0, 0]);
    message.extend_from_slice(
        &u64::try_from(checkpoint_handle.len())
            .expect("checkpoint handle length")
            .to_le_bytes(),
    );
    message.extend_from_slice(checkpoint_handle.as_bytes());
    message.extend_from_slice(&1_u64.to_le_bytes());
    message.extend_from_slice(&[0x62; 32]);
    message.extend_from_slice(
        &u64::try_from(binding.handle.len())
            .expect("signer handle length")
            .to_le_bytes(),
    );
    message.extend_from_slice(binding.handle.as_bytes());
    message.extend_from_slice(public_key);
    message
}
#[test]
fn typed_adapters_bind_identity_algorithm_and_exact_purpose() {
    let peer = b"12D3KooWPhaseOneGovernancePublisher".to_vec();
    let (_parent, governance_service) = direct_signer(
        SoftwareSignerRoleV1::GovernanceDag,
        SoftwareSignerPurposeBindingV1::GovernanceDag {
            publisher_peer_id: peer.clone(),
        },
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    assert_eq!(
        ExternalSoftwareSignerGovernanceDagAdapterV1::try_new(
            direct_client(&governance_service),
            b"12D3KooWSubstitutedPublisher".to_vec(),
        )
        .expect_err("substituted Governance publisher must fail"),
        ExternalSoftwareSignerAdapterErrorV1::BindingMismatch
    );
    let governance = ExternalSoftwareSignerGovernanceDagAdapterV1::try_new(
        direct_client(&governance_service),
        peer,
    )
    .expect("exact Governance adapter");
    let transition =
        sorafs_node::governance_dag_key_transition_signing_payload_v1(1, 2, [0x31; 32])
            .expect("Governance transition payload");
    governance
        .sign(
            sorafs_node::GovernanceDagSigningPurposeV1::KeyTransition,
            &transition,
        )
        .expect("sign exact Governance transition");
    assert!(
        governance
            .sign(
                sorafs_node::GovernanceDagSigningPurposeV1::DagHead,
                &transition,
            )
            .is_err(),
        "cross-purpose Governance bytes must fail before signing"
    );
    assert!(
        governance
            .sign(
                sorafs_node::GovernanceDagSigningPurposeV1::QualificationArchive,
                &transition,
            )
            .is_err(),
        "key-transition bytes must not substitute for a qualification archive"
    );
    let signer_id = [0x41; 32];
    let provider_id = [0x42; 32];
    let (_parent, potr_service) = direct_signer(
        SoftwareSignerRoleV1::PotrProvider,
        SoftwareSignerPurposeBindingV1::PotrProvider {
            signer_id,
            provider_id,
        },
        SoftwareSignerKeyAlgorithmV1::MlDsa,
    );
    assert_eq!(
        ExternalSoftwareSignerPotrProviderAdapterV1::try_new(
            direct_client(&potr_service),
            signer_id,
            [0x43; 32],
        )
        .expect_err("substituted PoTR provider must fail"),
        ExternalSoftwareSignerAdapterErrorV1::BindingMismatch
    );
    let potr = ExternalSoftwareSignerPotrProviderAdapterV1::try_new(
        direct_client(&potr_service),
        signer_id,
        provider_id,
    )
    .expect("exact PoTR provider adapter");
    assert!(
        !potr
            .sign(&unsigned_potr_payload(provider_id))
            .unwrap()
            .is_empty()
    );
    assert!(potr.sign(&unsigned_potr_payload([0x44; 32])).is_err());
    let billing_id = "billing-signer-primary".to_owned();
    let (_parent, billing_service) = direct_signer(
        SoftwareSignerRoleV1::BillingStatement,
        SoftwareSignerPurposeBindingV1::BillingStatement {
            signer_id: billing_id.clone(),
        },
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    assert_eq!(
        ExternalSoftwareSignerBillingStatementAdapterV1::try_new(
            direct_client(&billing_service),
            "billing-signer-substituted".to_owned(),
        )
        .expect_err("substituted billing signer must fail"),
        ExternalSoftwareSignerAdapterErrorV1::BindingMismatch
    );
    ExternalSoftwareSignerBillingStatementAdapterV1::try_new(
        direct_client(&billing_service),
        billing_id,
    )
    .expect("exact billing adapter")
    .sign_digest([0x51; 32])
    .expect("sign governed billing digest");
    let (_parent, evidence_service) = direct_signer(
        SoftwareSignerRoleV1::EvidenceViewer,
        SoftwareSignerPurposeBindingV1::EvidenceViewer,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let evidence =
        ExternalSoftwareSignerEvidenceViewerAdapterV1::try_new(direct_client(&evidence_service))
            .expect("exact evidence-viewer adapter");
    let mut receipt = b"sorafs.evidence-viewer.receipt-signature.v1".to_vec();
    receipt.extend_from_slice(&[0x52; 32]);
    let checkpoint_anchor = evidence_checkpoint_anchor_message(evidence.signer_binding());
    for (purpose, message) in [
        (
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::Receipt,
            receipt.as_slice(),
        ),
        (
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointStoreRecord,
            &[0x53; 32],
        ),
        (
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointAnchor,
            checkpoint_anchor.as_slice(),
        ),
        (
            sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CompactionArchive,
            &[0x54; 32],
        ),
    ] {
        evidence
            .sign(purpose, message)
            .expect("sign exact evidence-viewer purpose");
    }
    assert!(
        evidence
            .sign(
                sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointAnchor,
                &[0x54; 32],
            )
            .is_err(),
        "digest-only evidence bytes must not substitute for a checkpoint anchor"
    );
    let (_parent, stream_service) = direct_signer(
        SoftwareSignerRoleV1::StreamToken,
        SoftwareSignerPurposeBindingV1::StreamToken,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let stream =
        ExternalSoftwareSignerStreamTokenAdapterV1::try_new(direct_client(&stream_service))
            .expect("exact stream-token adapter");
    let stream_payload = StreamTokenBodyV1 {
        token_id: "11".repeat(16),
        manifest_cid: vec![0x61; 32],
        provider_id: [0x62; 32],
        profile_handle: "sorafs.standard".to_owned(),
        max_streams: 1,
        ttl_epoch: 1_060,
        rate_limit_bytes: 1_024,
        issued_at: 1_000,
        requests_per_minute: 1,
        token_pk_version: 1,
    }
    .signing_payload_bytes()
    .expect("stream-token signing payload");
    stream
        .sign(&stream_payload)
        .expect("sign canonical stream-token payload");
    let mut malformed_stream_payload = stream_payload;
    malformed_stream_payload.push(0);
    assert!(stream.sign(&malformed_stream_payload).is_err());
    let issuer_id = "pop-issuer-primary".to_owned();
    let (_parent, pop_service) = direct_signer(
        SoftwareSignerRoleV1::PopCredentials,
        SoftwareSignerPurposeBindingV1::PopCredentials {
            issuer_id: issuer_id.clone(),
        },
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    assert_eq!(
        ExternalSoftwareSignerPopIssuerAdapterV1::try_new(
            direct_client(&pop_service),
            "pop-issuer-substituted".to_owned(),
        )
        .expect_err("substituted PoP issuer must fail"),
        ExternalSoftwareSignerAdapterErrorV1::BindingMismatch
    );
    let pop =
        ExternalSoftwareSignerPopIssuerAdapterV1::try_new(direct_client(&pop_service), issuer_id)
            .expect("exact PoP issuer adapter");
    for purpose in [
        sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::Credential,
        sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::CommitmentRoot,
        sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::RevocationList,
    ] {
        pop.sign_digest(purpose, [purpose.wire_id(); 32])
            .expect("sign exact PoP purpose");
    }
    assert!(
        ExternalSoftwareSignerBillingStatementAdapterV1::try_new(
            direct_client(&pop_service),
            "billing-signer-primary".to_owned(),
        )
        .is_err(),
        "cross-role adapter construction must fail"
    );
}
#[derive(Debug)]
struct OverlapGovernanceSigner;
impl sorafs_node::GovernanceDagRuntimeSigner for OverlapGovernanceSigner {
    fn handle(&self) -> &str {
        "software://sorafs/governance-dag/overlap"
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(1, [0x71; 32]))
    }
    fn publisher_peer_id(&self) -> &[u8] {
        b"12D3KooWOverlapPublisher"
    }
    fn public_key(&self) -> [u8; 32] {
        [0x72; 32]
    }
    fn sign(
        &self,
        _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        _payload: &[u8],
    ) -> Result<[u8; 64], String> {
        Ok([0x73; 64])
    }
}
struct RecordingBaseRegistry {
    overlap: bool,
    observed: Mutex<Vec<crate::IrohaRuntimeProviderSlotV1>>,
}
impl fmt::Debug for RecordingBaseRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordingBaseRegistry")
            .field("overlap", &self.overlap)
            .finish_non_exhaustive()
    }
}
impl crate::RuntimeProviderBrokerBackendRegistryV1 for RecordingBaseRegistry {
    fn resolve(
        &self,
        bindings: &crate::IrohaRuntimeProviderBindingsV1,
    ) -> Result<crate::RuntimeProviderBrokerBackendsV1, crate::IrohaRuntimeProviderRegistryErrorV1>
    {
        *self.observed.lock().expect("observed catalog lock") =
            bindings.iter().map(|binding| binding.slot()).collect();
        let backends = crate::RuntimeProviderBrokerBackendsV1::new();
        Ok(if self.overlap {
            backends.with_governance_dag_signer(Arc::new(OverlapGovernanceSigner))
        } else {
            backends
        })
    }
}
#[test]
fn composite_registry_partitions_exactly_and_rejects_overlap_or_missing_base() {
    let base_catalog = crate::IrohaRuntimeProviderBindingsV1::qualified_for_test(
        "software-signer-composite-test",
        crate::IrohaRuntimeProviderSlotV1::BillingFinalizedQuery,
        "ledger://billing/finalized-query/primary",
        1,
        [0x81; 32],
    );
    assert!(matches!(
        crate::RuntimeProviderBrokerBackendRegistryV1::resolve(
            &ExternalSoftwareSignerBackendsV1::new(),
            &base_catalog,
        ),
        Err(crate::IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
    ));
    let base = Arc::new(RecordingBaseRegistry {
        overlap: false,
        observed: Mutex::new(Vec::new()),
    });
    let composite = ExternalSoftwareSignerBackendsV1::new().with_base_registry(base.clone());
    crate::RuntimeProviderBrokerBackendRegistryV1::resolve(&composite, &base_catalog)
        .expect("delegate the exact non-signer partition");
    assert_eq!(
        *base.observed.lock().expect("observed catalog lock"),
        vec![crate::IrohaRuntimeProviderSlotV1::BillingFinalizedQuery]
    );
    let overlapping = ExternalSoftwareSignerBackendsV1::new().with_base_registry(Arc::new(
        RecordingBaseRegistry {
            overlap: true,
            observed: Mutex::new(Vec::new()),
        },
    ));
    assert!(matches!(
        crate::RuntimeProviderBrokerBackendRegistryV1::resolve(&overlapping, &base_catalog),
        Err(crate::IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
    ));
    let keypair = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
        .expect("Governance fixture keypair");
    let public_key: [u8; 32] = keypair
        .public_key()
        .try_to_bytes()
        .expect("public key bytes")
        .1
        .try_into()
        .expect("Ed25519 public key width");
    let signer_catalog =
        crate::IrohaRuntimeProviderBindingsV1::qualified_governance_dag_signer_for_test(
            "software-signer-composite-test",
            "software://sorafs/governance-dag/primary",
            1,
            [0x92; 32],
            "12D3KooWPhaseOneGovernancePublisher",
            &hex::encode(public_key),
        );
    let (signers, base) = signer_catalog.partition_external_software_signers_v1();
    assert_eq!(signers.len(), 1);
    assert!(base.is_empty());
    assert_eq!(signers.network_id(), signer_catalog.network_id());
    assert_eq!(base.network_id(), signer_catalog.network_id());
}
