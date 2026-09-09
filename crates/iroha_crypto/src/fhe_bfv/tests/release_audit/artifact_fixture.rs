//! Release-audit artifact fixture assertions and fixture ownership.

use super::*;

pub(super) struct ArtifactFixture {
    pub(super) diagnostics: BfvTestDiagnostics,
    pub(super) params: BfvParameters,
    pub(super) artifacts: BfvFullBootstrapCircuitArtifactBundleV1,
    pub(super) material: BfvFullBootstrapCircuitMaterialV1,
    pub(super) evidence: BfvFullBootstrapReleaseAuditEvidenceV1,
    pub(super) artifact_bundle_bytes: Vec<u8>,
    pub(super) evidence_bytes: Vec<u8>,
    pub(super) artifact_bundle_digest: Hash,
    pub(super) evidence_digest: Hash,
}

pub(super) fn check() -> ArtifactFixture {
    let_row! { diagnostics = BfvTestDiagnostics::new( include_str!("../../testdata/evidence_diagnostics_v1.tsv"), 799, [ 0x74, 0xe7, 0xa3, 0xbf, 0x80, 0xc4, 0x76, 0xb0, 0xc6, 0x8c, 0xed, 0xe5, 0x6a, 0x78, 0x08, 0xf7, 0xb1, 0x41, 0xe3, 0xa5, 0xe2, 0x2f, 0x96, 0xeb, 0xb4, 0x70, 0x53, 0xf6, 0xe0, 0xe7, 0x71, 0x8a, ], ) };
    let params = ram_lfe_bfv_parameters_v1();
    let artifacts = sample_full_bootstrap_circuit_artifacts(&params);
    let material = sample_full_bootstrap_circuit_material_for_artifacts(&params, &artifacts);
    let_row! { evidence = release_evidence_v1(&params, &material, &artifacts) .expect("derive full-bootstrap release audit evidence") };
    validate_release_evidence_v1(&evidence).expect("release audit evidence validates");
    let_row! { artifact_bundle_bytes = norito::to_bytes(&artifacts).expect("encode release audit source artifact bundle") };
    let evidence_bytes = norito::to_bytes(&evidence).expect("encode release audit evidence");
    let_row! { artifact_bundle_digest = circuit_artifact_bundle_digest(&params, &material, &artifacts) .expect("digest release audit source artifact bundle") };
    let_row! { evidence_digest = release_evidence_digest_v1(&evidence).expect("digest typed release audit evidence") };
    ArtifactFixture {
        diagnostics,
        params,
        artifacts,
        material,
        evidence,
        artifact_bundle_bytes,
        evidence_bytes,
        artifact_bundle_digest,
        evidence_digest,
    }
}
