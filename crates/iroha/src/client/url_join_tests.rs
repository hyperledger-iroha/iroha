use url::Url;
use super::{
    ZkProofsFilter, is_verifier_backend_registry_label_v1, join_torii_url,
    join_torii_url_with_path_segments, normalize_hex32_lower,
    require_verifier_backend_registry_label_v1, validate_zk_proofs_filter,
};
#[test]
fn join_prover_reports_paths() {
    let base = Url::parse("http://localhost:8080/api/").unwrap();
    let u = join_torii_url(&base, "v1/zk/prover/reports");
    assert_eq!(u.as_str(), "http://localhost:8080/api/v1/zk/prover/reports");
    let u2 = join_torii_url(&base, "v1/zk/prover/reports/abcd");
    assert_eq!(
        u2.as_str(),
        "http://localhost:8080/api/v1/zk/prover/reports/abcd"
    );
}
#[test]
fn join_vote_tally_path() {
    let base = Url::parse("http://localhost:8080/api/").unwrap();
    let u = join_torii_url(&base, "v1/zk/vote/tally");
    assert_eq!(u.as_str(), "http://localhost:8080/api/v1/zk/vote/tally");
}
#[test]
fn join_vk_path_encodes_slash_containing_backend_segment() {
    let base = Url::parse("http://localhost:8080/api/").unwrap();
    let u = join_torii_url_with_path_segments(&base, "v1/zk/vk", &["halo2/ipa", "ivm_execution"]);
    assert_eq!(
        u.as_str(),
        "http://localhost:8080/api/v1/zk/vk/halo2%2Fipa/ivm_execution"
    );
}
#[test]
fn join_proof_path_encodes_slash_containing_backend_segment() {
    let base = Url::parse("http://localhost:8080/api/").unwrap();
    let proof_hash = "aa".repeat(32);
    let u = join_torii_url_with_path_segments(
        &base,
        "v1/zk/proof",
        &["halo2/ipa", proof_hash.as_str()],
    );
    let expected = format!("http://localhost:8080/api/v1/zk/proof/halo2%2Fipa/{proof_hash}");
    assert_eq!(u.as_str(), expected.as_str());
}
#[test]
fn zk_client_backend_guard_accepts_exact_registry_labels() {
    for &backend in iroha_data_model::zk::ZK_VERIFIER_BACKEND_REGISTRY_LABELS_V1 {
        assert!(
            is_verifier_backend_registry_label_v1(backend),
            "registry label {backend} must be admitted"
        );
        assert_eq!(
            require_verifier_backend_registry_label_v1(backend, "backend")
                .expect("exact registry label"),
            backend
        );
    }
}
#[test]
fn zk_client_backend_guard_rejects_protocol_trusted_setup_and_path_labels() {
    for backend in [
        "unknown/privacy/backend",
        "halo2/ipa-pasta-cycle-v1",
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2/ipa\0",
        "HALO2/IPA",
        "stark/FRI",
        "halo2/ipa:ivm-execution-v1",
        "halo2/ipa::ivm-execution-v1",
        "halo2/ipa/ivm-execution-v1",
        "halo2/pasta/ipa/ivm-execution-v1",
        "halo2//ipa",
        "halo2/ipa:",
        "halo2/ipa.",
        "halo2/ipa/.ivm-execution-v1",
        "halo2/ipa:ivm..execution-v1",
        "../halo2/ipa",
        "halo2/ipa/../tiny-add",
        "halo2/ipa/orchard",
        "fcmp++",
        "groth16/bls12-377",
        "stark/fri/miden",
        "stark/fri/pq-masp-stark-fri",
        "stark/fri/latest",
        "stark/fri/random-profile",
        "stark/fri/poseidon2-goldilocks/extra",
        "stark/fri/sha256_goldilocks.v2",
        "stark/fri/sha512-goldilocks",
        "stark/fri/boi-audited",
        "stark/fri/external-security-review",
        "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/placeholder",
        "stark/fri/kzg",
        "stark/fri/debug",
        "halo2/kzg",
        "halo2/ipa:release-ready",
        "halo2/ipa:certified-mainnet",
        "halo2/ipa:third-party-audited",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/tiny-anon-transfer-2x2",
        "halo2/pasta/tiny-commit-open",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2",
        "halo2/pasta/anon-transfer-2x2-merkle2",
        "halo2/ipa/anon-transfer-2x2-merkle8",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa/vote-bool-commit",
        "halo2/ipa:vote-bool-commit",
        "halo2/pasta/vote-bool-commit-merkle2",
        "halo2/ipa/vote-bool-commit-merkle8",
        "halo2/ipa:vote-bool-commit-merkle16",
        "mock/dev",
    ] {
        assert!(
            !is_verifier_backend_registry_label_v1(backend),
            "unsupported backend {backend:?} must stay fail-closed"
        );
        let err = require_verifier_backend_registry_label_v1(backend, "backend")
            .expect_err("unsupported backend rejected");
        assert!(
            format!("{err}").contains("exact supported verifier-registry label"),
            "unexpected backend error for {backend:?}: {err}"
        );
    }
}
#[test]
fn zk_client_proof_filter_rejects_unsupported_backends_before_request() {
    for backend in [
        " halo2/ipa",
        "halo2/ipa/orchard",
        "stark/fri/miden",
        "stark/fri/latest",
        "stark/fri/boi-audited",
        "halo2/ipa:release-ready",
        "halo2/ipa:tiny-add",
        "halo2/kzg",
        "mock/dev",
    ] {
        let filter = ZkProofsFilter {
            backend: Some(backend),
            ..ZkProofsFilter::default()
        };
        let err =
            validate_zk_proofs_filter(&filter).expect_err("unsupported filter backend rejected");
        assert!(format!("{err}").contains("exact supported verifier-registry label"));
    }
}
#[test]
fn zk_client_proof_hash_canonicalizes_and_rejects_malformed_values() {
    assert_eq!(
        normalize_hex32_lower(&format!("0x{}", "AA".repeat(32)), "proof hash").expect("hash"),
        "aa".repeat(32)
    );
    for hash in [
        String::new(),
        "abc".to_string(),
        "z".repeat(64),
        "a".repeat(63),
        format!("0x0x{}", "aa".repeat(32)),
    ] {
        assert!(
            normalize_hex32_lower(&hash, "proof hash").is_err(),
            "malformed proof hash {hash:?} must be rejected"
        );
    }
}
