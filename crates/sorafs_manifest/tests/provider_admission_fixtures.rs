use std::{env, fs};

use assert_cmd::Command;
use norito::json::Value;
use sorafs_manifest::{
    ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdmissionRenewalV1,
    ProviderAdvertV1,
};
use tempfile::{Builder, TempDir};

fn tempdir() -> Result<TempDir, std::io::Error> {
    Builder::new()
        .prefix("sorafs-provider-admission-fixtures-")
        .tempdir_in(env::temp_dir().canonicalize()?)
}

#[test]
fn provider_admission_fixture_generator_outputs_digests() {
    let tempdir = tempdir().expect("tempdir");
    let out_dir = tempdir.path().join("fixtures");

    let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("run")
        .arg("--locked")
        .arg("-p")
        .arg("sorafs_car")
        .arg("--features")
        .arg("cli")
        .arg("--bin")
        .arg("provider_admission_fixtures")
        .arg("--")
        .arg(format!("--out-dir={}", out_dir.display()))
        .env("NORITO_SKIP_BINDINGS_SYNC", "1");
    cmd.assert().success();

    for name in [
        "proposal_v1.to",
        "advert_v1.to",
        "envelope_v1.to",
        "proposal_renewed_v1.to",
        "advert_renewed_v1.to",
        "envelope_renewed_v1.to",
        "renewal_v1.to",
        "revocation_v1.to",
        "metadata.json",
    ] {
        assert!(
            out_dir.join(name).exists(),
            "{name} missing from fixture output"
        );
    }

    let metadata_text =
        fs::read_to_string(out_dir.join("metadata.json")).expect("read metadata.json");
    let metadata: Value = norito::json::from_str(&metadata_text).expect("parse metadata");
    assert_eq!(
        metadata
            .get("proposal_digest_hex")
            .and_then(Value::as_str)
            .expect("proposal digest"),
        "69f5a339695ce00ce917b94865ce38ea67d11d53bdf1c0583768c18ee13b2daa"
    );
    assert_eq!(
        metadata
            .get("envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("envelope digest"),
        "f6dac4124c10bfc9905d58ec1b880ec85f747120c8d27fb0d15328f96095a4d4"
    );
    assert_eq!(
        metadata
            .get("renewal_envelope_digest_hex")
            .and_then(Value::as_str)
            .expect("renewal envelope digest"),
        "b703263247bdd59c285f699931f57e1e8e5f33d1b26607951bf1ba3ce891b8f1"
    );
    assert_eq!(
        metadata
            .get("revocation_digest_hex")
            .and_then(Value::as_str)
            .expect("revocation digest"),
        "eafe0515b73971ea5aefb4fa3c6680936a1dae1843a0f05b09e0fbde29809d5b"
    );

    let renewal_text =
        fs::read_to_string(out_dir.join("renewal_v1.json")).expect("read renewal_v1.json");
    let renewal: Value = norito::json::from_str(&renewal_text).expect("parse renewal_v1.json");
    assert_eq!(
        renewal
            .get("previous_envelope_digest_hex")
            .and_then(Value::as_str),
        Some("f6dac4124c10bfc9905d58ec1b880ec85f747120c8d27fb0d15328f96095a4d4")
    );
    assert_eq!(
        renewal.get("retention_epoch").and_then(Value::as_u64),
        Some(900)
    );

    let revocation_text =
        fs::read_to_string(out_dir.join("revocation_v1.json")).expect("read revocation_v1.json");
    let revocation: Value =
        norito::json::from_str(&revocation_text).expect("parse revocation_v1.json");
    assert_eq!(
        revocation
            .get("envelope_digest_hex")
            .and_then(Value::as_str),
        Some("f6dac4124c10bfc9905d58ec1b880ec85f747120c8d27fb0d15328f96095a4d4")
    );
    assert_eq!(
        revocation
            .get("revocation_digest_hex")
            .and_then(Value::as_str),
        Some("eafe0515b73971ea5aefb4fa3c6680936a1dae1843a0f05b09e0fbde29809d5b")
    );
    assert_eq!(
        revocation.get("revoked_at").and_then(Value::as_u64),
        Some(970)
    );

    let renewal: ProviderAdmissionRenewalV1 = norito::decode_from_bytes(
        &fs::read(out_dir.join("renewal_v1.to")).expect("read renewal fixture"),
    )
    .expect("decode renewal fixture");
    let renewed_envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(
        &fs::read(out_dir.join("envelope_renewed_v1.to")).expect("read renewed envelope fixture"),
    )
    .expect("decode renewed envelope fixture");
    let renewed_proposal: ProviderAdmissionProposalV1 = norito::decode_from_bytes(
        &fs::read(out_dir.join("proposal_renewed_v1.to")).expect("read renewed proposal fixture"),
    )
    .expect("decode renewed proposal fixture");
    let renewed_advert: ProviderAdvertV1 = norito::decode_from_bytes(
        &fs::read(out_dir.join("advert_renewed_v1.to")).expect("read renewed advert fixture"),
    )
    .expect("decode renewed advert fixture");

    assert_eq!(renewal.version, 1);
    assert_eq!(renewed_envelope.version, 1);
    assert_eq!(renewed_proposal.version, 1);
    assert_eq!(renewed_advert.version, 1);
    assert_eq!(renewal.envelope, renewed_envelope);
    assert_eq!(renewed_envelope.proposal, renewed_proposal);
    assert_eq!(renewed_envelope.advert_body, renewed_advert.body);

    for retired in [
        "proposal_legacy_v1.json",
        "proposal_legacy_v1.to",
        "advert_legacy_v1.json",
        "advert_legacy_v1.to",
        "envelope_legacy_v1.json",
        "envelope_legacy_v1.to",
        "proposal_v2.json",
        "proposal_v2.to",
        "advert_v2.json",
        "advert_v2.to",
        "envelope_v2.json",
        "envelope_v2.to",
    ] {
        assert!(
            !out_dir.join(retired).exists(),
            "retired fixture {retired} must not be emitted"
        );
    }
}
