"""Contract tests for the strict SoraFS first-release documentation."""

from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
ARCHITECTURE = REPO_ROOT / "docs/source/sorafs_architecture_rfc.md"
ROLLOUT = REPO_ROOT / "docs/source/sorafs/migration_roadmap.md"
MANIFEST = REPO_ROOT / "crates/sorafs_manifest/src/lib.rs"


def test_architecture_does_not_reintroduce_pre_release_fallbacks_or_fake_evidence() -> None:
    source = ARCHITECTURE.read_text(encoding="utf-8")
    normalized = " ".join(source.split())
    forbidden = (
        "Status: Ratified by council",
        "council_minutes_2025-10-29.md",
        "Storage nodes accept pins if either",
        "manual envelope approval becomes a fallback",
        "Older manifests stay grandfathered",
        "The manifest contains `pin_policy.retention_epoch` and `alias_version`",
        'The seed for the digest is `"sorafs-manifest-v1"',
    )
    assert [phrase for phrase in forbidden if phrase in normalized] == []
    for required in (
        "Pre-release envelope-only or caller-summary paths are not accepted.",
        "There is no grace window, envelope-only fallback, or grandfathered pre-release",
        "This repository deliberately does not invent or backfill council minutes",
        "Missing lanes or evidence keep promotion blocked.",
    ):
        assert required in normalized


def test_manifest_digest_documentation_matches_the_canonical_implementation() -> None:
    architecture = ARCHITECTURE.read_text(encoding="utf-8")
    implementation = MANIFEST.read_text(encoding="utf-8")
    digest_start = implementation.index("pub fn digest(&self)")
    digest_end = implementation.index("\n    }", digest_start)
    digest_function = implementation[digest_start:digest_end]
    assert "let bytes = self.encode()?;" in digest_function
    assert "blake3::hash(&bytes)" in digest_function
    assert "sorafs-manifest-v1" not in digest_function
    assert "digest input is the exact canonical Norito `ManifestV1` byte sequence" in architecture
    assert "validators must not prepend" in architecture


def test_rollout_roadmap_keeps_local_checks_separate_from_production_evidence() -> None:
    source = ROLLOUT.read_text(encoding="utf-8")
    normalized = " ".join(source.split())
    for required in (
        "it is a deployment roadmap, not a backward-compatibility plan",
        "Repository checks establish local conformance; only reviewed, deployment-bound evidence can",
        "No milestone can be waived by changing a repository status document.",
        "the envelope is an audit artifact, not an admission fallback.",
        "Promotion is allowed only when the resulting aggregate reports `status=ready`",
    ):
        assert required in normalized
