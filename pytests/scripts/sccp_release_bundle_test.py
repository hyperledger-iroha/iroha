"""Tests for the SCCP release-note attachment bundle builder."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "sccp_release_bundle.py"
VERIFY_SCRIPT = ROOT / "scripts" / "sccp_verify_release_bundle.py"
ALL_LANES_TESTS = ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py"
PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "contract-smoke",
    "core-admission",
)


def load_all_lanes_helpers():
    """Load all-lanes fixture helpers without importing pytest collection state."""

    spec = spec_from_file_location(
        "sccp_all_lanes_evidence_bundle_helpers",
        ALL_LANES_TESTS,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_verify_helpers():
    """Load release-bundle verifier helpers without running its CLI."""

    spec = spec_from_file_location(
        "sccp_release_bundle_verify_helpers",
        VERIFY_SCRIPT,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def write_complete_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write a complete synthetic all-lanes evidence bundle."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    evidence = tmp_path / "complete.toml"
    payload = helpers.render_records(helpers.complete_bundle(evidence_module))
    evidence.write_text(payload, encoding="utf-8")
    return evidence, payload


def write_phase_artifacts(tmp_path: Path) -> dict[str, str]:
    """Write downloaded GitHub Actions-style per-phase log artifacts."""

    root = tmp_path / "phase-artifacts"
    payloads: dict[str, str] = {}
    for phase in PHASES:
        payload = (
            f"==> SCCP production corridor: {phase}\n"
            f"phase {phase} passed\n"
            "SCCP production corridor completed.\n"
        )
        path = root / f"sccp-production-corridor-{phase}" / f"{phase}.log"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(payload, encoding="utf-8")
        payloads[phase] = payload
    return payloads


def build_ready_bundle(tmp_path: Path) -> Path:
    evidence, _ = write_complete_evidence(tmp_path)
    write_phase_artifacts(tmp_path)
    output_dir = tmp_path / "bundle"
    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--output-dir",
            str(output_dir),
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(tmp_path / "phase-artifacts"),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    return output_dir


def rewrite_manifest_artifact(output_dir: Path, relative_path: str) -> None:
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    artifact_path = output_dir / relative_path
    payload = artifact_path.read_bytes()
    for artifact in manifest["artifacts"]:
        if artifact["path"] == relative_path:
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            break
    else:
        raise AssertionError(f"manifest artifact not found: {relative_path}")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def append_manifest_artifact(output_dir: Path, relative_path: str) -> None:
    """Append a newly-created bundle artifact to the manifest."""

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    artifact_path = output_dir / relative_path
    payload = artifact_path.read_bytes()
    manifest["artifacts"].append(
        {
            "path": relative_path,
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        }
    )
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def remove_manifest_artifact(output_dir: Path, relative_path: str) -> None:
    """Remove a bundle artifact from the manifest."""

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"] = [
        artifact
        for artifact in manifest["artifacts"]
        if artifact["path"] != relative_path
    ]
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def manifest_artifact_hash(output_dir: Path, relative_path: str) -> str:
    """Return the manifest hash for an artifact path."""

    manifest = json.loads((output_dir / "manifest.json").read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == relative_path:
            return artifact["sha256"]
    raise AssertionError(f"manifest artifact not found: {relative_path}")


def rewrite_report_phase_artifact(output_dir: Path, phase: str) -> None:
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    relative_path = f"corridor/{phase}.log"
    payload = (output_dir / relative_path).read_bytes()
    report["corridor"]["evidence_artifacts"][phase]["bytes"] = len(payload)
    report["corridor"]["evidence_artifacts"][phase]["sha256"] = hashlib.sha256(
        payload
    ).hexdigest()
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, relative_path)
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")


def rewrite_report_input_artifact(output_dir: Path, relative_path: str) -> None:
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    payload = (output_dir / relative_path).read_bytes()
    for artifact in report["input_artifacts"]:
        if artifact["path"] == relative_path:
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            break
    else:
        raise AssertionError(f"report input artifact not found: {relative_path}")
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, relative_path)
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")


def rewrite_canonical_report_and_notes(output_dir: Path) -> None:
    verifier = load_verify_helpers()
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    (output_dir / "sccp-release-readiness.md").write_text(
        verifier._expected_readiness_markdown(report),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.md")

    manifest = json.loads((output_dir / "manifest.json").read_text(encoding="utf-8"))
    (output_dir / "sccp-release-notes-attachment.md").write_text(
        verifier._expected_release_notes_attachment(report, manifest["artifacts"]),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")


def test_release_bundle_requires_hashed_phase_evidence(tmp_path: Path) -> None:
    """The production bundle must not pass with declared-only corridor status."""

    evidence, _ = write_complete_evidence(tmp_path)
    output_dir = tmp_path / "bundle"

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--output-dir",
            str(output_dir),
            "--phase-result",
            "all=passed",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 1
    assert "SCCP release bundle is not production ready" in completed.stderr
    assert "production corridor phase rust-sccp has no hashed evidence artifact" in (
        completed.stderr
    )
    assert not output_dir.exists()


def test_release_bundle_force_rejects_output_containing_inputs(
    tmp_path: Path,
) -> None:
    """Forced output replacement must not delete evidence inputs or phase logs."""

    evidence, evidence_payload = write_complete_evidence(tmp_path)
    write_phase_artifacts(tmp_path)

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--force",
            "--output-dir",
            str(tmp_path),
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(tmp_path / "phase-artifacts"),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "refusing --force output directory that contains input evidence" in (
        completed.stderr
    )
    assert evidence.read_text(encoding="utf-8") == evidence_payload
    assert (
        tmp_path
        / "phase-artifacts"
        / "sccp-production-corridor-contract-smoke"
        / "contract-smoke.log"
    ).is_file()


def test_release_bundle_writes_hash_bound_public_artifacts(tmp_path: Path) -> None:
    """A ready release produces report, summary, copied inputs, logs, and manifest."""

    evidence, evidence_payload = write_complete_evidence(tmp_path)
    phase_payloads = write_phase_artifacts(tmp_path)
    output_dir = tmp_path / "bundle"

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--output-dir",
            str(output_dir),
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(tmp_path / "phase-artifacts"),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr
    assert f"Wrote SCCP release bundle to {output_dir}" in completed.stdout

    report_md = output_dir / "sccp-release-readiness.md"
    report_json = output_dir / "sccp-release-readiness.json"
    summary_json = output_dir / "sccp-all-lanes-summary.json"
    notes_md = output_dir / "sccp-release-notes-attachment.md"
    manifest_json = output_dir / "manifest.json"
    for path in (report_md, report_json, summary_json, notes_md, manifest_json):
        assert path.is_file()

    manifest = json.loads(manifest_json.read_text(encoding="utf-8"))
    assert manifest["schema"] == "sccp-release-bundle-v1"
    assert manifest["production_ready"] is True
    assert manifest["release_checklist_ready"] is True
    assert manifest["corridor_ready"] is True
    assert manifest["blockers"] == []
    artifact_by_path = {
        artifact["path"]: artifact for artifact in manifest["artifacts"]
    }
    assert "sccp-release-readiness.md" in artifact_by_path
    assert "sccp-release-readiness.json" in artifact_by_path
    assert "sccp-all-lanes-summary.json" in artifact_by_path
    assert "sccp-release-notes-attachment.md" in artifact_by_path
    assert "evidence/00-complete.toml" in artifact_by_path
    assert artifact_by_path["evidence/00-complete.toml"]["sha256"] == hashlib.sha256(
        evidence_payload.encode("utf-8")
    ).hexdigest()
    for phase, payload in phase_payloads.items():
        artifact = artifact_by_path[f"corridor/{phase}.log"]
        assert artifact["sha256"] == hashlib.sha256(
            payload.encode("utf-8")
        ).hexdigest()

    report = json.loads(report_json.read_text(encoding="utf-8"))
    assert report["production_ready"] is True
    assert report["corridor"]["require_phase_evidence"] is True
    assert report["corridor"]["phases"]["contract-smoke"] == "passed"
    assert report["corridor"]["evidence_artifacts"]["rust-sccp"]["sha256"] == (
        artifact_by_path["corridor/rust-sccp.log"]["sha256"]
    )

    summary = json.loads(summary_json.read_text(encoding="utf-8"))
    assert summary["production_ready"] is True
    assert summary["release_checklist"]["ready"] is True

    notes = notes_md.read_text(encoding="utf-8")
    assert "Status: READY" in notes
    assert "`manifest.json` is the verifier root" in notes
    assert "`sccp-release-readiness.md`" in notes
    assert "`sccp-all-lanes-summary.json`" in notes
    assert "`corridor/contract-smoke.log`" in notes

    verified = subprocess.run(
        [
            "python3",
            str(VERIFY_SCRIPT),
            "--json",
            str(output_dir),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 0, verified.stdout + verified.stderr
    verification = json.loads(verified.stdout)
    assert verification["verified"] is True
    assert verification["errors"] == []


def test_release_bundle_verifier_rejects_tampered_artifact(tmp_path: Path) -> None:
    """Published bundle verification must fail if a copied log changes."""

    evidence, _ = write_complete_evidence(tmp_path)
    write_phase_artifacts(tmp_path)
    output_dir = tmp_path / "bundle"
    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--output-dir",
            str(output_dir),
            "--phase-result",
            "all=passed",
            "--phase-evidence-dir",
            str(tmp_path / "phase-artifacts"),
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr

    (output_dir / "corridor" / "contract-smoke.log").write_text(
        "tampered\n",
        encoding="utf-8",
    )
    verified = subprocess.run(
        [
            "python3",
            str(VERIFY_SCRIPT),
            str(output_dir),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "SCCP release bundle verification failed" in verified.stdout
    assert "corridor/contract-smoke.log sha256 mismatch" in verified.stdout


def test_release_bundle_verifier_rejects_manifest_path_escape(tmp_path: Path) -> None:
    """The manifest must not be able to prove files outside the bundle."""

    output_dir = build_ready_bundle(tmp_path)
    outside = tmp_path / "outside.md"
    outside.write_bytes((output_dir / "sccp-release-readiness.md").read_bytes())
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == "sccp-release-readiness.md":
            artifact["path"] = "../outside.md"
            artifact["bytes"] = outside.stat().st_size
            artifact["sha256"] = hashlib.sha256(outside.read_bytes()).hexdigest()
            break
    else:
        raise AssertionError("readiness report artifact not found")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "manifest artifact path escapes bundle: ../outside.md" in verified.stdout


def test_release_bundle_verifier_rejects_symlinked_manifest(
    tmp_path: Path,
) -> None:
    """The verifier root must be an attached file, not a symlink."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    outside = tmp_path / "manifest-copy.json"
    outside.write_bytes(manifest_path.read_bytes())
    manifest_path.unlink()
    manifest_path.symlink_to(outside)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "manifest is a symlink: manifest.json" in verified.stdout


def test_release_bundle_verifier_rejects_unmanifested_artifact(
    tmp_path: Path,
) -> None:
    """A verified release bundle must not carry files outside the manifest."""

    output_dir = build_ready_bundle(tmp_path)
    unexpected = output_dir / "evidence" / "operator-side-note.txt"
    unexpected.write_text("not hash-bound\n", encoding="utf-8")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "bundle contains unmanifested artifact: evidence/operator-side-note.txt"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_extra_manifested_artifact(
    tmp_path: Path,
) -> None:
    """Hash-bound extra artifacts must still be tied to the readiness report."""

    output_dir = build_ready_bundle(tmp_path)
    extra_path = output_dir / "operator-extra.md"
    extra_path.write_text("unreviewed operator appendix\n", encoding="utf-8")
    append_manifest_artifact(output_dir, "operator-extra.md")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "manifest contains artifact not referenced by readiness report: "
        "operator-extra.md"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_unknown_phase_artifact_reference(
    tmp_path: Path,
) -> None:
    """Only known passed corridor phases may justify manifested phase logs."""

    output_dir = build_ready_bundle(tmp_path)
    extra_log = output_dir / "corridor" / "operator-extra.log"
    extra_log.write_text(
        "==> SCCP production corridor: operator-extra\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    payload = extra_log.read_bytes()
    report["corridor"]["evidence_artifacts"]["operator-extra"] = {
        "path": "corridor/operator-extra.log",
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    append_manifest_artifact(output_dir, "corridor/operator-extra.log")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report corridor has evidence artifact for unknown phase: "
        "operator-extra"
    ) in verified.stdout
    assert (
        "manifest contains artifact not referenced by readiness report: "
        "corridor/operator-extra.log"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_symlinked_artifact(
    tmp_path: Path,
) -> None:
    """Manifested artifacts must be ordinary bundle files, not symlinks."""

    output_dir = build_ready_bundle(tmp_path)
    artifact = output_dir / "evidence" / "00-complete.toml"
    outside = tmp_path / "outside-complete.toml"
    outside.write_bytes(artifact.read_bytes())
    artifact.unlink()
    artifact.symlink_to(outside)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "bundle artifact path uses symlink: evidence/00-complete.toml"
        in verified.stdout
    )
    assert "bundle contains symlink: evidence/00-complete.toml" in verified.stdout


def test_release_bundle_verifier_requires_manifest_handoff_note(
    tmp_path: Path,
) -> None:
    """Release notes must tell operators to attach the verifier manifest."""

    output_dir = build_ready_bundle(tmp_path)
    notes_path = output_dir / "sccp-release-notes-attachment.md"
    notes_path.write_text(
        notes_path.read_text(encoding="utf-8").replace("`manifest.json`", "`manifest`"),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "release notes attachment does not list manifest.json" in verified.stdout


def test_release_bundle_verifier_rejects_release_notes_drift(
    tmp_path: Path,
) -> None:
    """The release-notes attachment must be the canonical manifest/report table."""

    output_dir = build_ready_bundle(tmp_path)
    notes_path = output_dir / "sccp-release-notes-attachment.md"
    notes_path.write_text(
        notes_path.read_text(encoding="utf-8")
        + "\nUnreviewed release-manager note.\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "release notes attachment does not match manifest and report" in (
        verified.stdout
    )


def test_release_bundle_verifier_rejects_omitted_phase_artifact(
    tmp_path: Path,
) -> None:
    """Every phase artifact referenced by the readiness report must be in the manifest."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"] = [
        artifact
        for artifact in manifest["artifacts"]
        if artifact["path"] != "corridor/contract-smoke.log"
    ]
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase contract-smoke artifact is missing from manifest: "
        "corridor/contract-smoke.log"
    ) in verified.stdout


def test_release_bundle_verifier_recomputes_required_corridor_phases(
    tmp_path: Path,
) -> None:
    """Ready flags cannot hide a skipped or unbound corridor phase."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["corridor"]["phases"]["swift-sdk"] = "skipped"
    report["corridor"]["evidence_artifacts"].pop("swift-sdk")
    report["corridor"]["production_ready"] = True
    report["corridor"]["blockers"] = []
    report["production_ready"] = True
    report["blockers"] = []
    report["user_prover_submission_surfaces"] = verifier._expected_submission_surfaces(
        report
    )
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    (output_dir / "corridor" / "swift-sdk.log").unlink()
    remove_manifest_artifact(output_dir, "corridor/swift-sdk.log")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report corridor phase swift-sdk is not passed: 'skipped'"
        in verified.stdout
    )
    assert (
        "readiness report corridor phase swift-sdk has no hashed evidence artifact"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_noncanonical_phase_log_path(
    tmp_path: Path,
) -> None:
    """Passed phase evidence must stay at the canonical corridor log path."""

    output_dir = build_ready_bundle(tmp_path)
    phase = "swift-sdk"
    original = output_dir / "corridor" / f"{phase}.log"
    alternate = output_dir / "phase-logs" / f"{phase}.log"
    alternate.parent.mkdir(parents=True, exist_ok=True)
    alternate.write_bytes(original.read_bytes())
    original.unlink()

    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["corridor"]["evidence_artifacts"][phase]["path"] = (
        f"phase-logs/{phase}.log"
    )
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    remove_manifest_artifact(output_dir, f"corridor/{phase}.log")
    append_manifest_artifact(output_dir, f"phase-logs/{phase}.log")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase swift-sdk evidence artifact path must be "
        "corridor/swift-sdk.log"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_summary_report_mismatch(
    tmp_path: Path,
) -> None:
    """The standalone all-lanes summary must match the report's embedded evidence."""

    output_dir = build_ready_bundle(tmp_path)
    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["required_domains"] = [*summary["required_domains"], 999]
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "all-lanes summary does not match readiness report evidence" in (
        verified.stdout
    )


def test_release_bundle_verifier_rejects_unknown_root_json_fields(
    tmp_path: Path,
) -> None:
    """Published manifest and readiness JSON roots must not carry extra claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["operator_attestation"] = "production approved outside the evidence set"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["operator_attestation"] = "production approved outside the manifest"
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report contains unknown top-level field: operator_attestation"
        in verified.stdout
    )
    assert (
        "manifest contains unknown top-level field: operator_attestation"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_missing_report_root_json_fields(
    tmp_path: Path,
) -> None:
    """Published readiness JSON roots must keep every canonical field."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report.pop("inputs")
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report missing top-level field: inputs" in verified.stdout


def test_release_bundle_verifier_rejects_manifest_readiness_claim_drift(
    tmp_path: Path,
) -> None:
    """The manifest readiness header must be derived from the report roots."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest.pop("blockers")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "manifest missing top-level field: blockers" in verified.stdout
    assert (
        "manifest blockers do not match readiness report blockers"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_unknown_artifact_fields(
    tmp_path: Path,
) -> None:
    """Manifest and report artifact objects must not carry extra claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["input_artifacts"][0]["operator_attestation"] = "reviewed elsewhere"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == "sccp-all-lanes-summary.json":
            artifact["operator_attestation"] = "reviewed elsewhere"
            break
    else:
        raise AssertionError("summary artifact not found")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "manifest artifact sccp-all-lanes-summary.json contains unknown field: "
        "operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report input artifact evidence/00-complete.toml contains "
        "unknown field: operator_attestation"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_artifact_field_type_drift(
    tmp_path: Path,
) -> None:
    """Manifest and report artifact hashes must keep canonical JSON types."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["input_artifacts"][0]["bytes"] = True
    report["input_artifacts"][0]["sha256"] = {
        "sha256": report["input_artifacts"][0]["sha256"],
    }
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == "sccp-all-lanes-summary.json":
            artifact["bytes"] = str(artifact["bytes"])
            artifact["sha256"] = ["not-a-canonical-hash"]
            break
    else:
        raise AssertionError("summary artifact not found")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "sccp-all-lanes-summary.json bytes must be a non-negative integer"
        in verified.stdout
    )
    assert "sccp-all-lanes-summary.json sha256 must be a string" in verified.stdout
    assert (
        "readiness report input artifact bytes must be a non-negative integer "
        "for evidence/00-complete.toml"
    ) in verified.stdout
    assert (
        "readiness report input artifact sha256 must be a string for "
        "evidence/00-complete.toml"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_release_checklist_drift(
    tmp_path: Path,
) -> None:
    """The public checklist table must match the embedded all-lanes evidence."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["release_checklist"]["items"][0]["id"] = "forged_release_gate"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report release_checklist does not match embedded evidence"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_release_checklist_unknown_fields(
    tmp_path: Path,
) -> None:
    """Release checklist rows must not carry extra operator claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["release_checklist"]["items"][0]["operator_attestation"] = (
        "reviewed elsewhere"
    )
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report release_checklist item all_required_lane_records "
        "contains unknown field: operator_attestation"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_release_checklist_field_type_drift(
    tmp_path: Path,
) -> None:
    """Release checklist rows must keep canonical string and blocker shapes."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["release_checklist"]["items"][0]["id"] = ""
    report["release_checklist"]["items"][0]["title"] = 1
    report["release_checklist"]["items"][0]["blockers"] = [""]
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["release_checklist"]["items"][0]["id"] = ""
    summary["release_checklist"]["items"][0]["title"] = False
    summary["release_checklist"]["items"][0]["blockers"] = [""]
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report release_checklist item id must be a non-empty string"
        in verified.stdout
    )
    assert (
        "readiness report release_checklist item title must be a non-empty string"
        in verified.stdout
    )
    assert (
        "readiness report release_checklist item blockers must be a list of "
        "non-empty strings"
    ) in verified.stdout
    assert (
        "all-lanes summary release_checklist item id must be a non-empty string"
        in verified.stdout
    )
    assert (
        "all-lanes summary release_checklist item title must be a non-empty string"
        in verified.stdout
    )
    assert (
        "all-lanes summary release_checklist item blockers must be a list of "
        "non-empty strings"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_corridor_unknown_fields(
    tmp_path: Path,
) -> None:
    """The production corridor section must not carry side-channel claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["corridor"]["operator_attestation"] = "reviewed elsewhere"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report corridor contains unknown field: operator_attestation"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_input_path_drift(
    tmp_path: Path,
) -> None:
    """The report's provenance list must match the copied TOML artifacts."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["inputs"] = ["operator/local/complete.toml"]
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report inputs do not match copied input artifacts"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_input_provenance_schema_drift(
    tmp_path: Path,
) -> None:
    """Copied input provenance must use unique canonical bundle paths."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["inputs"] = [
        "evidence/00-complete.toml",
        "evidence/00-complete.toml",
        "../operator/complete.toml",
        "",
    ]
    report["input_artifacts"].append(dict(report["input_artifacts"][0]))
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report inputs contains duplicate path: "
        "evidence/00-complete.toml"
    ) in verified.stdout
    assert (
        "readiness report inputs path escapes bundle: ../operator/complete.toml"
        in verified.stdout
    )
    assert (
        "readiness report inputs item must be a non-empty string"
        in verified.stdout
    )
    assert (
        "readiness report input_artifacts contains duplicate path: "
        "evidence/00-complete.toml"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_report_artifact_path_drift(
    tmp_path: Path,
) -> None:
    """Report artifact records must use canonical bundle-relative paths."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["input_artifacts"][0]["path"] = "../evidence/00-complete.toml"
    report["corridor"]["evidence_artifacts"]["swift-sdk"]["path"] = (
        "corridor\\swift-sdk.log"
    )
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report input artifact path escapes bundle: "
        "../evidence/00-complete.toml"
    ) in verified.stdout
    assert (
        "readiness report phase swift-sdk artifact path is not canonical: "
        "corridor\\swift-sdk.log"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_copied_input_layout_drift(
    tmp_path: Path,
) -> None:
    """Copied evidence inputs must keep the builder's evidence/NN-*.toml layout."""

    output_dir = build_ready_bundle(tmp_path)
    original = output_dir / "evidence" / "00-complete.toml"
    renamed = output_dir / "evidence" / "renamed.toml"
    original.rename(renamed)

    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["inputs"] = ["evidence/renamed.toml"]
    report["input_artifacts"][0]["path"] = "evidence/renamed.toml"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == "evidence/00-complete.toml":
            artifact["path"] = "evidence/renamed.toml"
            break
    else:
        raise AssertionError("copied evidence artifact not found")
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report inputs path must use copied evidence layout "
        "evidence/00-*.toml: evidence/renamed.toml"
    ) in verified.stdout
    assert (
        "readiness report input_artifacts path must use copied evidence layout "
        "evidence/00-*.toml: evidence/renamed.toml"
    ) in verified.stdout


def test_release_bundle_verifier_requires_non_empty_report_and_summary_json(
    tmp_path: Path,
) -> None:
    """Empty JSON evidence roots must not skip production readiness checks."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "sccp-release-readiness.json").write_text("{}\n", encoding="utf-8")
    (output_dir / "sccp-all-lanes-summary.json").write_text("{}\n", encoding="utf-8")
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report JSON must be a non-empty object" in verified.stdout
    assert "all-lanes summary JSON must be a non-empty object" in verified.stdout


def test_release_bundle_verifier_rejects_malformed_report_sections(
    tmp_path: Path,
) -> None:
    """Malformed nested readiness sections must produce verifier errors."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["evidence"] = []
    report["release_checklist"] = []
    report["corridor"] = []
    report["input_artifacts"] = None
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["release_checklist"] = []
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report evidence is not an object" in verified.stdout
    assert "readiness report release_checklist is not an object" in verified.stdout
    assert "readiness report corridor is not an object" in verified.stdout
    assert "readiness report input_artifacts must be a non-empty list" in (
        verified.stdout
    )
    assert "all-lanes summary release_checklist is not an object" in verified.stdout
    assert "Traceback" not in verified.stderr


def test_release_bundle_verifier_rejects_readiness_boolean_type_drift(
    tmp_path: Path,
) -> None:
    """Readiness flags must be real JSON booleans, not truthy scalars."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report_item_id = report["release_checklist"]["items"][0]["id"]
    report["production_ready"] = "true"
    report["evidence"]["production_ready"] = "true"
    report["release_checklist"]["ready"] = "true"
    report["release_checklist"]["items"][0]["ready"] = "true"
    report["corridor"]["production_ready"] = 1
    report["corridor"]["require_phase_evidence"] = "true"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary_item_id = summary["release_checklist"]["items"][0]["id"]
    summary["production_ready"] = "true"
    summary["release_checklist"]["ready"] = "true"
    summary["release_checklist"]["items"][0]["ready"] = 1
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["production_ready"] = "true"
    manifest["release_checklist_ready"] = "true"
    manifest["corridor_ready"] = 1
    manifest["blockers"] = ""
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "manifest production_ready must be a boolean" in verified.stdout
    assert "manifest release_checklist_ready must be a boolean" in verified.stdout
    assert "manifest corridor_ready must be a boolean" in verified.stdout
    assert (
        "manifest blockers must be a list of non-empty strings"
        in verified.stdout
    )
    assert "readiness report production_ready must be a boolean" in verified.stdout
    assert (
        "readiness report embedded evidence production_ready must be a boolean"
        in verified.stdout
    )
    assert (
        "readiness report release_checklist ready must be a boolean"
        in verified.stdout
    )
    assert (
        f"readiness report release_checklist item {report_item_id} "
        "ready must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report corridor production_ready is not a boolean"
        in verified.stdout
    )
    assert (
        "readiness report corridor require_phase_evidence is not a boolean"
        in verified.stdout
    )
    assert "all-lanes summary production_ready must be a boolean" in verified.stdout
    assert (
        "all-lanes summary release_checklist ready must be a boolean"
        in verified.stdout
    )
    assert (
        f"all-lanes summary release_checklist item {summary_item_id} "
        "ready must be a boolean"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_summary_unknown_fields(
    tmp_path: Path,
) -> None:
    """Embedded and standalone all-lanes summaries must not carry extra claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["evidence"]["operator_attestation"] = "reviewed elsewhere"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["operator_attestation"] = "reviewed elsewhere"
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report embedded evidence contains unknown field: "
        "operator_attestation"
    ) in verified.stdout
    assert (
        "all-lanes summary contains unknown field: operator_attestation"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_all_lanes_lane_unknown_fields(
    tmp_path: Path,
) -> None:
    """Embedded and standalone all-lanes lane rows must not carry extra claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["evidence"]["lanes"][0]["operator_attestation"] = "reviewed elsewhere"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["lanes"][0]["operator_attestation"] = "reviewed elsewhere"
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert any(
        "readiness report embedded evidence lane domain " in line
        and "contains unknown field: operator_attestation" in line
        for line in verified.stdout.splitlines()
    )
    assert any(
        "all-lanes summary lane domain " in line
        and "contains unknown field: operator_attestation" in line
        for line in verified.stdout.splitlines()
    )


def test_release_bundle_verifier_rejects_all_lanes_list_scalar_type_drift(
    tmp_path: Path,
) -> None:
    """All-lanes domain and blocker lists must keep canonical scalar types."""

    def mutate_summary(summary: dict) -> None:
        summary["required_domains"] = ["1"]
        summary["blockers"] = [""]
        summary["lanes"][0]["blockers"] = [""]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_summary(report["evidence"])
    report["blockers"] = [""]
    report["corridor"]["blockers"] = [""]
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_summary(summary)
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["blockers"] = [""]
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "manifest blockers must be a list of non-empty strings"
        in verified.stdout
    )
    assert (
        "readiness report blockers must be a list of non-empty strings"
        in verified.stdout
    )
    assert (
        "readiness report corridor blockers must be a list of non-empty strings"
        in verified.stdout
    )
    assert (
        "readiness report embedded evidence required_domains must be a list "
        "of integers"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence blockers must be a list of "
        "non-empty strings"
    ) in verified.stdout
    assert any(
        "readiness report embedded evidence lane domain " in line
        and "blockers must be a list of non-empty strings" in line
        for line in verified.stdout.splitlines()
    )
    assert "all-lanes summary required_domains must be a list of integers" in (
        verified.stdout
    )
    assert (
        "all-lanes summary blockers must be a list of non-empty strings"
        in verified.stdout
    )
    assert any(
        "all-lanes summary lane domain " in line
        and "blockers must be a list of non-empty strings" in line
        for line in verified.stdout.splitlines()
    )


def test_release_bundle_verifier_rejects_all_lanes_required_domain_drift(
    tmp_path: Path,
) -> None:
    """All-lanes required_domains must bind to the published lane domains."""

    def mutate_summary(summary: dict) -> None:
        domains = list(summary["required_domains"])
        summary["required_domains"] = [domains[0], *domains]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_summary(report["evidence"])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_summary(summary)
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report embedded evidence required_domains contains "
        "duplicate domains"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence required_domains must match lane "
        "domains"
    ) in verified.stdout
    assert "all-lanes summary required_domains contains duplicate domains" in (
        verified.stdout
    )
    assert "all-lanes summary required_domains must match lane domains" in (
        verified.stdout
    )


def test_release_bundle_verifier_rejects_all_lanes_nested_crypto_field_drift(
    tmp_path: Path,
) -> None:
    """Nested all-lanes lane crypto sections must keep canonical field shapes."""

    def mutate_lane(lane: dict) -> None:
        lane["records"]["route_allowlist"] = "true"
        lane["source_record_hashes"]["operator_attestation"] = "reviewed elsewhere"
        lane["source_record_hashes"]["source_verifier_material_hash"] = (
            "0X" + "aa" * 32
        )
        lane["source_adapter_gate"]["operator_attestation"] = "reviewed elsewhere"
        lane["source_adapter_gate"]["required"] = "true"
        lane["source_adapter_gate"]["gate_hash"] = "0X" + "aa" * 32
        lane["source_adapter_gate"]["audit_hashes"] = {"audit": True}
        lane["source_adapter_gate"]["blockers"] = "none"
        lane["destination_binding"]["operator_attestation"] = "reviewed elsewhere"
        lane["destination_binding"]["destination_binding_key"] = ""
        lane["destination_binding"]["destination_binding_hash"] = "0X" + "aa" * 32
        lane["destination_binding"]["expected_destination_binding_hash_matches"] = (
            "true"
        )
        lane["destination_binding"]["recomputed"] = "true"
        lane["destination_binding"]["destination_bridge_address"] = "0X" + "aa" * 20
        lane["destination_binding"]["destination_network_id"] = "0X" + "aa" * 32
        lane["route_allowlist"]["operator_attestation"] = "reviewed elsewhere"
        lane["route_allowlist"]["route_allowlist_hash"] = True
        lane["route_allowlist"]["expected_route_allowlist_hash_matches"] = "true"
        route_canary = lane["route_allowlist"]["route_canary"]
        route_canary["evidence_hash"] = "0x" + "gg" * 32
        route_canary["evidence_bound"] = "true"
        route_canary.pop("destination_binding_hash")

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lane(report["evidence"]["lanes"][0])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_lane(summary["lanes"][0])
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report embedded evidence lane domain "
        "1 records route_allowlist must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_record_hashes contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_record_hashes source_verifier_material_hash must be a "
        "canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate required must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate gate_hash must be empty or a canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate audit_hashes audit must be a canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate blockers must be a list of non-empty strings"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding destination_binding_key must be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding destination_binding_hash must be a canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding expected_destination_binding_hash_matches "
        "must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding recomputed must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding destination_bridge_address must be a canonical "
        "20-byte hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 destination_binding destination_network_id must be a canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist route_allowlist_hash must be a canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist expected_route_allowlist_hash_matches must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist route_canary missing field: destination_binding_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist route_canary evidence_hash must be a canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 route_allowlist route_canary evidence_bound must be a boolean"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 source_record_hashes contains unknown "
        "field: operator_attestation"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 source_adapter_gate contains unknown "
        "field: operator_attestation"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 destination_binding contains unknown "
        "field: operator_attestation"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_route_canary_field_drift(
    tmp_path: Path,
) -> None:
    """All-lanes route canaries must keep lane-specific transcript schemas."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}

        evm_canary = by_domain[1]["route_allowlist"]["route_canary"]
        evm_canary["operator_attestation"] = "reviewed elsewhere"
        evm_canary["evidence_source"] = "operator_attestation"
        evm_canary["transaction_hash"] = "0X" + "aa" * 32
        evm_canary["log_index"] = "0"
        evm_canary["target_domain"] = 2
        evm_canary["proof_version"] = 2
        evm_canary["proof_source_domain"] = 1
        evm_canary["message_proof_used"] = False

        solana_canary = by_domain[3]["route_allowlist"]["route_canary"]
        solana_canary["solana_programdata_address"] = ""
        solana_canary["solana_programdata_slot"] = 1080

        ton_canary = by_domain[4]["route_allowlist"]["route_canary"]
        ton_canary["ton_account_state_hash"] = "0X" + "89" * 32
        ton_canary["ton_last_transaction_lt"] = "0"

        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        tron_canary["transaction_owner_address"] = "0x42" + "11" * 20
        tron_canary["raw_data_owner_matches_transaction"] = False
        tron_canary["signature_recovers_to_owner"] = "true"

        substrate_canary = by_domain[6]["route_allowlist"]["route_canary"]
        substrate_canary["substrate_finalized_head"] = True
        substrate_canary["substrate_runtime_spec_version"] = "01"

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lanes(report["evidence"]["lanes"])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_lanes(summary["lanes"])
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary evidence_source must be "
        "evm_message_proof_accepted_transaction"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary transaction_hash must be a canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary log_index must be a u32 integer"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary target_domain must be the lane domain"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary proof_version must be 1"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary proof_source_domain must be SORA"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary message_proof_used must be true"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 3 route_allowlist "
        "route_canary solana_programdata_address must be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 3 route_allowlist "
        "route_canary solana_programdata_slot must be a canonical positive "
        "decimal string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 4 route_allowlist "
        "route_canary ton_account_state_hash must be a canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 4 route_allowlist "
        "route_canary ton_last_transaction_lt must be a canonical positive "
        "decimal string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary transaction_owner_address must be a canonical "
        "0x41-prefixed 21-byte hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary raw_data_owner_matches_transaction must be true"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary signature_recovers_to_owner must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 6 route_allowlist "
        "route_canary substrate_finalized_head must be a canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 6 route_allowlist "
        "route_canary substrate_runtime_spec_version must be a canonical "
        "decimal string"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "contains unknown field: operator_attestation"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "transaction_owner_address must be a canonical 0x41-prefixed "
        "21-byte hex string"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_route_canary_hash_drift(
    tmp_path: Path,
) -> None:
    """Route-canary hashes must match the sibling route and destination hashes."""

    def replacement_hash(current: str) -> str:
        candidate = "0x" + "11" * 32
        if current == candidate:
            return "0x" + "22" * 32
        return candidate

    def mutate_lane(lane: dict) -> None:
        route_canary = lane["route_allowlist"]["route_canary"]
        route_canary["route_allowlist_hash"] = replacement_hash(
            lane["route_allowlist"]["route_allowlist_hash"]
        )
        route_canary["destination_binding_hash"] = replacement_hash(
            lane["destination_binding"]["destination_binding_hash"]
        )

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lane(report["evidence"]["lanes"][0])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_lane(summary["lanes"][0])
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert any(
        "readiness report embedded evidence lane domain " in line
        and "route_canary route_allowlist_hash must match lane route_allowlist_hash"
        in line
        for line in verified.stdout.splitlines()
    )
    assert any(
        "readiness report embedded evidence lane domain " in line
        and (
            "route_canary destination_binding_hash must match lane "
            "destination_binding_hash"
        )
        in line
        for line in verified.stdout.splitlines()
    )
    assert any(
        "all-lanes summary lane domain " in line
        and "route_canary route_allowlist_hash must match lane route_allowlist_hash"
        in line
        for line in verified.stdout.splitlines()
    )
    assert any(
        "all-lanes summary lane domain " in line
        and (
            "route_canary destination_binding_hash must match lane "
            "destination_binding_hash"
        )
        in line
        for line in verified.stdout.splitlines()
    )


def test_release_bundle_verifier_rejects_all_lanes_expected_hash_drift(
    tmp_path: Path,
) -> None:
    """Expected route and destination hash pins must match their actual hashes."""

    def replacement_hash(current: str) -> str:
        candidate = "0x" + "33" * 32
        if current == candidate:
            return "0x" + "44" * 32
        return candidate

    def mutate_lane(lane: dict) -> None:
        destination = lane["destination_binding"]
        destination["expected_destination_binding_hash"] = replacement_hash(
            destination["destination_binding_hash"]
        )
        destination["expected_destination_binding_hash_matches"] = False
        destination["recomputed"] = False

        route = lane["route_allowlist"]
        route["expected_route_allowlist_hash"] = replacement_hash(
            route["route_allowlist_hash"]
        )
        route["expected_route_allowlist_hash_matches"] = False

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lane(report["evidence"]["lanes"][0])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    mutate_lane(summary["lanes"][0])
    summary_path.write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report embedded evidence lane domain 1 destination_binding "
        "expected_destination_binding_hash must match destination_binding_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 destination_binding "
        "expected_destination_binding_hash_matches must be true"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 destination_binding "
        "recomputed must be true"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "expected_route_allowlist_hash must match route_allowlist_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "expected_route_allowlist_hash_matches must be true"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 destination_binding "
        "expected_destination_binding_hash must match destination_binding_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist "
        "expected_route_allowlist_hash must match route_allowlist_hash"
    ) in verified.stdout


def test_release_bundle_verifier_recomputes_summary_from_copied_evidence(
    tmp_path: Path,
) -> None:
    """The standalone report cannot hide tampered copied TOML evidence."""

    output_dir = build_ready_bundle(tmp_path)
    evidence_path = output_dir / "evidence" / "00-complete.toml"
    evidence_path.write_text("", encoding="utf-8")
    rewrite_report_input_artifact(output_dir, "evidence/00-complete.toml")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "all-lanes summary does not match copied evidence inputs" in (
        verified.stdout
    )
    assert "readiness report evidence does not match copied evidence inputs" in (
        verified.stdout
    )


def test_release_bundle_verifier_requires_copied_evidence_inputs(
    tmp_path: Path,
) -> None:
    """The report must keep hash-bound copied TOML inputs for recomputation."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["input_artifacts"] = []
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report input_artifacts must be a non-empty list" in (
        verified.stdout
    )


def test_release_bundle_verifier_rejects_markdown_report_drift(
    tmp_path: Path,
) -> None:
    """The public Markdown report must be the canonical render of the JSON report."""

    output_dir = build_ready_bundle(tmp_path)
    report_md = output_dir / "sccp-release-readiness.md"
    notes_md = output_dir / "sccp-release-notes-attachment.md"
    old_report_hash = manifest_artifact_hash(output_dir, "sccp-release-readiness.md")
    report_md.write_text(
        report_md.read_text(encoding="utf-8").replace(
            "Status: READY\n",
            "Status: READY\n\nTampered reviewer-facing report text.\n",
        ),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.md")
    new_report_hash = manifest_artifact_hash(output_dir, "sccp-release-readiness.md")
    notes_md.write_text(
        notes_md.read_text(encoding="utf-8").replace(
            old_report_hash,
            new_report_hash,
        ),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report Markdown does not match readiness report JSON" in (
        verified.stdout
    )


def test_release_bundle_verifier_rejects_unbound_crypto_evidence(
    tmp_path: Path,
) -> None:
    """Every release-report crypto row must bind governed and canary evidence."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["cryptographic_evidence"][0]["route_canary_evidence_bound"] = False
    report["cryptographic_evidence"][0].pop("route_canary_evidence_hash")
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report cryptographic evidence row has unbound route canary" in (
        verified.stdout
    )
    assert (
        "readiness report cryptographic evidence row missing "
        "route_canary_evidence_hash"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_crypto_evidence_hash_drift(
    tmp_path: Path,
) -> None:
    """The public crypto table must match the embedded all-lanes lane evidence."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["cryptographic_evidence"][0]["destination_binding_hash"] = "0x" + "11" * 32
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report cryptographic_evidence does not match embedded lane evidence"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_crypto_evidence_lane_binding_drift(
    tmp_path: Path,
) -> None:
    """The public crypto table must stay aligned to embedded lane rows."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["cryptographic_evidence"][0]["domain"] = 999
    report["cryptographic_evidence"][0]["chain"] = "forged-chain"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report cryptographic evidence row 0 domain must match "
        "lane domain"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row 0 chain must match lane chain"
        in verified.stdout
    )
    assert (
        "readiness report cryptographic_evidence does not match embedded lane evidence"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_crypto_evidence_field_type_drift(
    tmp_path: Path,
) -> None:
    """Cryptographic evidence rows must carry exact JSON types and bytes32 text."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["cryptographic_evidence"][0]
    row["domain"] = "3"
    row["chain"] = 3
    row["source_verifier_material_hash"] = "0X" + "aa" * 32
    row["source_adapter_engine_deployment_hash"] = "0x" + "bb" * 31
    row["destination_binding_hash"] = True
    row["route_allowlist_hash"] = "0x" + "cc" * 33
    row["route_canary_evidence_hash"] = "0x" + "gg" * 32
    row["route_canary_evidence_source"] = False
    row["route_canary_evidence_bound"] = "true"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report cryptographic evidence row domain must be an integer"
        in verified.stdout
    )
    assert (
        "readiness report cryptographic evidence row chain must be a non-empty string"
        in verified.stdout
    )
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        assert (
            "readiness report cryptographic evidence row "
            f"{field} must be a canonical bytes32 hex string"
        ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "route_canary_evidence_source must be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "route_canary_evidence_bound must be a boolean"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_crypto_evidence_unknown_fields(
    tmp_path: Path,
) -> None:
    """Cryptographic evidence rows must not carry extra release claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["cryptographic_evidence"][0]["operator_attestation"] = "reviewed elsewhere"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report cryptographic evidence row contains unknown field: "
        "operator_attestation"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_drift(
    tmp_path: Path,
) -> None:
    """Portal/mobile submission rows must be derived from corridor phase results."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["user_prover_submission_surfaces"][0]["sdk_helpers"] = "forgedUiProver()"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_unknown_fields(
    tmp_path: Path,
) -> None:
    """Portal/mobile submission rows must not carry extra release claims."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["user_prover_submission_surfaces"][0]["operator_attestation"] = (
        "reviewed elsewhere"
    )
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report user prover submission surface row contains unknown "
        "field: operator_attestation"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_field_type_drift(
    tmp_path: Path,
) -> None:
    """Portal/mobile submission rows must keep canonical JSON field types."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["lanes"] = []
    row["proof_backend"] = ""
    row["sdk_helpers"] = ["forgedUiProver"]
    row["on_chain_submission"] = 1
    row["required_phases"] = ["js-sdk", ""]
    row["validation_status"] = "reviewed"
    row["validation_blockers"] = [""]
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    rewrite_canonical_report_and_notes(output_dir)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report user prover submission surface row lanes must be a "
        "non-empty string"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row proof_backend "
        "must be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row sdk_helpers must "
        "be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row on_chain_submission "
        "must be a non-empty string"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row required_phases "
        "must be a list of non-empty strings"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row validation_status "
        "must be passed or blocked"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row validation_blockers "
        "must be a list of non-empty strings"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_forged_phase_log(
    tmp_path: Path,
) -> None:
    """Published phase artifacts must still be real corridor transcripts."""

    output_dir = build_ready_bundle(tmp_path)
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text("SCCP production corridor completed.\n", encoding="utf-8")
    rewrite_report_phase_artifact(output_dir, "contract-smoke")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase contract-smoke evidence artifact is missing "
        "the phase marker"
    ) in verified.stdout
