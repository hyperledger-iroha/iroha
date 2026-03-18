use std::{fs, path::PathBuf};

use norito::decode_from_bytes;
use sorafs_car::{TrustlessVerifier, TrustlessVerifierConfig};
use sorafs_manifest::ManifestV1;

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(relative)
}

#[test]
fn trustless_verifier_reports_gateway_fixture_digests() {
    let config_path = workspace_path("configs/soranet/gateway_m0/gateway_trustless_verifier.toml");
    let config =
        TrustlessVerifierConfig::from_file(&config_path).expect("gateway config parses cleanly");

    let manifest_bytes = fs::read(workspace_path(
        "fixtures/sorafs_gateway/1.0.0/manifest_v1.to",
    ))
    .expect("manifest bytes");
    let manifest: ManifestV1 =
        decode_from_bytes(&manifest_bytes).expect("manifest Norito decoding succeeds");

    let car_bytes = fs::read(workspace_path("fixtures/sorafs_gateway/1.0.0/gateway.car"))
        .expect("gateway CAR bytes");

    let outcome = TrustlessVerifier::new(config)
        .verify_full(&manifest, &car_bytes)
        .expect("trustless verification succeeds");

    assert_eq!(
        outcome.manifest_digest_hex(),
        "ecc2e8564dda27834b8bd53a3eebdc56055d3e2cbdd30b0f96938fb9f04b216e",
        "manifest digest should match published fixture metadata"
    );
    assert_eq!(
        outcome.car_digest_hex(),
        "ce50a9aadf84e57559208d39201621262fd1b1887ae490ca54470e2a00153f27",
        "CAR digest should match gateway helper file"
    );
    assert_eq!(
        outcome.payload_digest_hex(),
        "91275991d58858bdc7ce3eb4472b61c5289dec3ecc6cf43c6411db772c1888a8",
        "payload digest should match gateway helper file"
    );

    // Chunk plan digest and PoR root should be fully populated.
    assert_eq!(outcome.chunk_plan_digest_hex().len(), 64);
    assert_ne!(outcome.chunk_plan_digest_hex(), outcome.car_digest_hex());
    assert_eq!(outcome.por_root_hex().len(), 64);
    assert!(
        outcome.por_root_hex().chars().any(|ch| ch != '0'),
        "PoR root must not be all zeros"
    );

    assert_eq!(outcome.profile_handle(), "sorafs.sf1@1.0.0");
    assert_eq!(
        outcome.report.stats.payload_bytes, manifest.content_length,
        "payload length should come from the manifest"
    );
    assert!(
        !outcome.report.chunk_store.chunks().is_empty(),
        "chunk store should carry the rebuilt plan"
    );
}
