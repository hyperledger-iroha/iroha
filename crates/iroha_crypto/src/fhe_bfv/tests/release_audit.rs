//! Default-stack release-audit validation, grouped by the protocol boundary under test.

use super::*;

#[path = "release_audit/archive_commitments.rs"]
mod archive_commitments;
#[path = "release_audit/artifact_bodies.rs"]
mod artifact_bodies;
#[path = "release_audit/artifact_digests.rs"]
mod artifact_digests;
#[path = "release_audit/artifact_fixture.rs"]
mod artifact_fixture;
#[path = "release_audit/evidence_binding.rs"]
mod evidence_binding;
#[path = "release_audit/evidence_codec.rs"]
mod evidence_codec;
#[path = "release_audit/generated_inventory.rs"]
mod generated_inventory;
#[path = "release_audit/manifest_codec.rs"]
mod manifest_codec;
#[path = "release_audit/manifest_commitments.rs"]
mod manifest_commitments;
#[path = "release_audit/package_codec.rs"]
mod package_codec;
#[path = "release_audit/record_codec.rs"]
mod record_codec;
#[path = "release_audit/report_commitments.rs"]
mod report_commitments;
#[path = "release_audit/review_fixture.rs"]
mod review_fixture;
#[path = "release_audit/review_markers.rs"]
mod review_markers;
#[path = "release_audit/signoff_authority.rs"]
mod signoff_authority;
#[path = "release_audit/signoff_codec.rs"]
mod signoff_codec;
use artifact_digests::MalformedArtifactBytes;
use artifact_fixture::ArtifactFixture;
use evidence_codec::EvidenceFixtures;
use generated_inventory::GeneratedPackages;
use manifest_codec::ManifestFrames;
use package_codec::PackageAuthorities;
use record_codec::RecordFrames;
use report_commitments::ReportCommitments;
use review_fixture::ReviewDocuments;
use signoff_authority::SignoffFixtures;
use signoff_codec::SignedRecords;

/// Borrowed generated evidence and signed-review fixtures shared by admission cases.
struct SignedAuditInputs<'a> {
    artifact_fixture: &'a ArtifactFixture,
    evidence_codec: &'a EvidenceFixtures,
    review_fixture: &'a ReviewDocuments,
    signoff_authority: &'a SignoffFixtures,
    signoff_codec: &'a SignedRecords,
    record_codec: &'a RecordFrames,
}

pub(super) fn run() {
    let artifact_fixture = artifact_fixture::check();
    let evidence_codec = evidence_codec::check(&artifact_fixture);
    let review_fixture = review_fixture::check(&artifact_fixture, &evidence_codec);
    let signoff_authority =
        signoff_authority::check(&artifact_fixture, &evidence_codec, &review_fixture);
    let signoff_codec =
        signoff_codec::check(&artifact_fixture, &review_fixture, &signoff_authority);
    let record_codec = record_codec::check(
        &artifact_fixture,
        &evidence_codec,
        &review_fixture,
        &signoff_authority,
        &signoff_codec,
    );
    let signed = SignedAuditInputs {
        artifact_fixture: &artifact_fixture,
        evidence_codec: &evidence_codec,
        review_fixture: &review_fixture,
        signoff_authority: &signoff_authority,
        signoff_codec: &signoff_codec,
        record_codec: &record_codec,
    };
    let generated_inventory = generated_inventory::check(&signed);
    review_markers::check(&signed, &generated_inventory);
    let report_commitments = report_commitments::check(&signed);
    archive_commitments::check(&signed, &report_commitments);
    let manifest_codec = manifest_codec::check(&signed, &generated_inventory);
    let package_codec = package_codec::check(&signed, &generated_inventory, &manifest_codec);
    manifest_commitments::check(
        &signed,
        &generated_inventory,
        &manifest_codec,
        &package_codec,
    );
    let artifact_digests = artifact_digests::check(&signed, &manifest_codec);
    artifact_bodies::check(&signed, &generated_inventory, artifact_digests);
    evidence_binding::check(&signed, &generated_inventory, &package_codec);
}

fn replace_ascii_once(
    diagnostics: &BfvTestDiagnostics,
    bytes: &[u8],
    from: &str,
    to: &str,
) -> Vec<u8> {
    let source = std::str::from_utf8(bytes).expect("release-audit fixture bytes are ASCII text");
    assert_eq_row! { source.matches(from).count(), 1, "{}", diagnostics.static_context_at(300) };
    source.replacen(from, to, 1).into_bytes()
}
