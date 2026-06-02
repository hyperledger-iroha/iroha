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
REPORT_SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
ALL_LANES_TESTS = ROOT / "pytests" / "scripts" / "sccp_all_lanes_evidence_test.py"
PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
)


def phase_command_lines(fragments) -> list[str]:
    """Render required fragments as production-corridor traced commands."""

    return [f"+ {fragment}" for fragment in fragments]


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


def load_bundle_module():
    """Load release-bundle builder helpers without running its CLI."""

    spec = spec_from_file_location(
        "sccp_release_bundle_builder_helpers",
        SCRIPT,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def load_report_module():
    """Load release-readiness helpers without importing CLI state."""

    spec = spec_from_file_location(
        "sccp_release_readiness_report_bundle_helpers",
        REPORT_SCRIPT,
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


def test_release_bundle_active_launch_policy_is_ethereum_mainnet() -> None:
    """Readiness and verifier constants must pin the Ethereum launch lane."""

    report = load_report_module()
    verifier = load_verify_helpers()

    for module in (report, verifier):
        assert module.ACTIVE_LAUNCH_DOMAIN == 1
        assert module.ACTIVE_LAUNCH_CHAIN == "eth"
        assert module.ACTIVE_LAUNCH_POLICY == "EthereumMainnetLane"
        assert module.ACTIVE_LAUNCH_DISPLAY == "ETH mainnet"


def write_active_launch_evidence(tmp_path: Path) -> tuple[Path, str]:
    """Write only the active launch-lane evidence bundle."""

    helpers = load_all_lanes_helpers()
    evidence_module = helpers.load_evidence_module()
    report = load_report_module()
    active_domain = report.ACTIVE_LAUNCH_DOMAIN
    records = helpers.complete_bundle(evidence_module)
    for section, domain_key in {
        "sccp_source_verifier_materials": "source_domain",
        "sccp_source_adapter_engine_deployments": "source_domain",
        "sccp_destination_rollouts": "domain",
        "sccp_route_allowlists": "domain",
    }.items():
        records[section] = [
            record
            for record in records[section]
            if record.get(domain_key) == active_domain
        ]
    evidence = tmp_path / f"{report.ACTIVE_LAUNCH_CHAIN}-launch.toml"
    payload = helpers.render_records(records)
    evidence.write_text(payload, encoding="utf-8")
    return evidence, payload


def write_phase_artifacts(tmp_path: Path) -> dict[str, str]:
    """Write downloaded GitHub Actions-style per-phase log artifacts."""

    root = tmp_path / "phase-artifacts"
    payloads: dict[str, str] = {}
    report = load_report_module()
    for phase in PHASES:
        payload = "\n".join(
            (
                f"==> SCCP production corridor: {phase}",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase],
                "SCCP production corridor completed.",
                "",
            )
        )
        path = root / f"sccp-production-corridor-{phase}" / f"{phase}.log"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(payload, encoding="utf-8")
        payloads[phase] = payload
    return payloads


def complete_corridor_log() -> str:
    """Return a synthetic successful full SCCP production-corridor transcript."""

    report = load_report_module()
    lines: list[str] = []
    for phase in PHASES:
        lines.append(f"==> SCCP production corridor: {phase}")
        lines.extend(
            phase_command_lines(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase])
        )
        lines.extend(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS[phase])
    return "\n".join(
        [*lines, ""]
    ) + "SCCP production corridor completed.\n"


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


def test_generated_bundle_self_verifier_reports_strict_errors(tmp_path: Path) -> None:
    """Production bundle generation must surface strict verifier failures."""

    module = load_bundle_module()
    invalid_bundle = tmp_path / "invalid-bundle"
    invalid_bundle.mkdir()

    try:
        module._verify_generated_bundle(invalid_bundle)
    except RuntimeError as exc:
        message = str(exc)
    else:
        raise AssertionError("invalid bundle passed strict self-verification")

    assert "generated SCCP release bundle failed strict verification" in message
    assert "missing manifest" in message


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
    assert f"Verified SCCP release bundle at {output_dir}" in completed.stdout

    report_md = output_dir / "sccp-release-readiness.md"
    report_json = output_dir / "sccp-release-readiness.json"
    summary_json = output_dir / "sccp-all-lanes-summary.json"
    notes_md = output_dir / "sccp-release-notes-attachment.md"
    manifest_json = output_dir / "manifest.json"
    for path in (report_md, report_json, summary_json, notes_md, manifest_json):
        assert path.is_file()
    manifest_sha256 = hashlib.sha256(manifest_json.read_bytes()).hexdigest()
    assert (
        f"SCCP release bundle manifest_sha256: {manifest_sha256}"
        in completed.stdout
    )

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
    assert verification["manifest_sha256"] == hashlib.sha256(
        manifest_json.read_bytes()
    ).hexdigest()


def test_release_bundle_accepts_hash_bound_full_corridor_log(
    tmp_path: Path,
) -> None:
    """A single full corridor transcript can be hash-bound for every phase."""

    evidence, _ = write_complete_evidence(tmp_path)
    corridor_log = tmp_path / "sccp-production-corridor.log"
    corridor_payload = complete_corridor_log()
    corridor_log.write_text(corridor_payload, encoding="utf-8")
    output_dir = tmp_path / "bundle"

    completed = subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--output-dir",
            str(output_dir),
            "--phase-result",
            "all=passed",
            "--phase-evidence",
            f"all={corridor_log}",
            str(evidence),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
    manifest = json.loads((output_dir / "manifest.json").read_text(encoding="utf-8"))
    artifact_by_path = {
        artifact["path"]: artifact for artifact in manifest["artifacts"]
    }
    expected_hash = hashlib.sha256(corridor_payload.encode("utf-8")).hexdigest()
    for phase in PHASES:
        artifact = artifact_by_path[f"corridor/{phase}.log"]
        assert artifact["sha256"] == expected_hash

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 0, verified.stdout + verified.stderr


def test_release_bundle_accepts_active_launch_lane_without_future_lanes(
    tmp_path: Path,
) -> None:
    """A release bundle can be ready when only the active lane is complete."""

    evidence, _ = write_active_launch_evidence(tmp_path)
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

    assert completed.returncode == 0, completed.stdout + completed.stderr
    report = json.loads(
        (output_dir / "sccp-release-readiness.json").read_text(encoding="utf-8")
    )
    summary = json.loads(
        (output_dir / "sccp-all-lanes-summary.json").read_text(encoding="utf-8")
    )
    manifest = json.loads((output_dir / "manifest.json").read_text(encoding="utf-8"))
    assert report["production_ready"] is True
    assert report["release_checklist"]["ready"] is True
    assert summary["production_ready"] is False
    assert summary["release_checklist"]["ready"] is False
    assert manifest["production_ready"] is True
    assert manifest["release_checklist_ready"] is True

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert verified.returncode == 0, verified.stdout + verified.stderr


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


def test_release_bundle_verifier_rejects_manifest_root_self_listing(
    tmp_path: Path,
) -> None:
    """The verifier root must stay outside the manifest artifact table."""

    output_dir = build_ready_bundle(tmp_path)
    append_manifest_artifact(output_dir, "manifest.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "manifest must not list verifier root as an artifact: manifest.json"
        in verified.stdout
    )


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


def test_release_bundle_verifier_rejects_symlinked_bundle_root(
    tmp_path: Path,
) -> None:
    """The reviewed bundle root itself must be an ordinary directory."""

    output_dir = build_ready_bundle(tmp_path)
    bundle_link = tmp_path / "bundle-link"
    bundle_link.symlink_to(output_dir, target_is_directory=True)

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(bundle_link)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert f"bundle root is a symlink: {bundle_link}" in verified.stdout


def test_release_bundle_verifier_rejects_non_directory_bundle_root(
    tmp_path: Path,
) -> None:
    """The verifier input must be the extracted release bundle directory."""

    bundle_file = tmp_path / "bundle.tar"
    bundle_file.write_text("not a directory\n", encoding="utf-8")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(bundle_file)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert f"bundle root is not a directory: {bundle_file}" in verified.stdout


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


def test_release_bundle_verifier_rejects_unmanifested_directory(
    tmp_path: Path,
) -> None:
    """A verified release bundle must not carry empty operator-side folders."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "operator-notes").mkdir()

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "bundle contains unmanifested directory: operator-notes" in verified.stdout


def test_release_bundle_rejects_control_character_artifact_paths(
    tmp_path: Path,
) -> None:
    """Generated public artifact paths must not contain control characters."""

    module = load_bundle_module()
    output_dir = tmp_path / "bundle"
    artifact = output_dir / "evidence" / "00-complete\noperator.toml"
    artifact.parent.mkdir(parents=True)
    artifact.write_text("release evidence\n", encoding="utf-8")

    try:
        module._artifact(artifact, output_dir)
    except ValueError as exc:
        message = str(exc)
    else:
        raise AssertionError("control-character artifact path was accepted")

    assert (
        "release artifact path contains control character '\\n': "
        "'evidence/00-complete\\noperator.toml'"
    ) in message


def test_release_bundle_verifier_rejects_control_character_manifest_paths(
    tmp_path: Path,
) -> None:
    """Manifest artifact paths must be printable canonical paths."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"][0]["path"] = "sccp-release-readiness\n.md"
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
        "manifest artifact path contains control character '\\n': "
        "'sccp-release-readiness\\n.md'"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_control_character_report_paths(
    tmp_path: Path,
) -> None:
    """Readiness report paths must not smuggle control characters into review."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["inputs"][0] = "evidence/00-complete\n.toml"
    report["input_artifacts"][0]["path"] = "evidence/00-complete\n.toml"
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
        "readiness report inputs path contains control character '\\n': "
        "'evidence/00-complete\\n.toml'"
    ) in verified.stdout
    assert (
        "readiness report input artifact path contains control character '\\n': "
        "'evidence/00-complete\\n.toml'"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_control_character_filesystem_entries(
    tmp_path: Path,
) -> None:
    """Extracted bundle entries with control characters must be rejected."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "operator\nnotes.txt").write_text(
        "unreviewed operator note\n",
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
        "bundle contains entry path with control character '\\n': "
        "'operator\\nnotes.txt'"
    ) in verified.stdout


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


def test_release_bundle_verifier_release_notes_renderer_is_independent(
    tmp_path: Path,
) -> None:
    """A weakened bundle builder must not relax public attachment rendering."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    manifest = json.loads((output_dir / "manifest.json").read_text(encoding="utf-8"))

    def weak_attachment(_report, artifacts):
        lines = ["# Weak SCCP Notes", "", "manifest.json"]
        for artifact in artifacts:
            if artifact["path"] == "sccp-release-notes-attachment.md":
                continue
            lines.append(f"{artifact['path']} {artifact['sha256']}")
        return "\n".join(lines) + "\n"

    assert not hasattr(verifier, "_bundle_module")
    (output_dir / "sccp-release-notes-attachment.md").write_text(
        weak_attachment({}, manifest["artifacts"]),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")

    summary = verifier.verify_bundle(output_dir)

    assert summary["verified"] is False
    assert "release notes attachment does not match manifest and report" in (
        summary["errors"]
    )


def test_release_bundle_verifier_rejects_manifest_artifact_order_drift(
    tmp_path: Path,
) -> None:
    """The manifest artifact table must keep the bundle builder's public order."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"][0], manifest["artifacts"][1] = (
        manifest["artifacts"][1],
        manifest["artifacts"][0],
    )
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
        "manifest artifact order does not match canonical release bundle order"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_noncanonical_json_serialization(
    tmp_path: Path,
) -> None:
    """Public JSON roots must keep the bundle builder's canonical byte form."""

    output_dir = build_ready_bundle(tmp_path)
    for relative_path in (
        "sccp-release-readiness.json",
        "sccp-all-lanes-summary.json",
    ):
        path = output_dir / relative_path
        payload = json.loads(path.read_text(encoding="utf-8"))
        path.write_text(
            json.dumps(payload, separators=(",", ":")),
            encoding="utf-8",
        )
        rewrite_manifest_artifact(output_dir, relative_path)
    rewrite_canonical_report_and_notes(output_dir)

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest_path.write_text(
        json.dumps(manifest, separators=(",", ":")),
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
        "manifest JSON is not canonical release-bundle serialization"
        in verified.stdout
    )
    assert (
        "readiness report JSON is not canonical release-bundle serialization"
        in verified.stdout
    )
    assert (
        "all-lanes summary JSON is not canonical release-bundle serialization"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_duplicate_json_keys(
    tmp_path: Path,
) -> None:
    """Public JSON roots must reject duplicate keys before semantic review."""

    output_dir = build_ready_bundle(tmp_path)
    manifest_path = output_dir / "manifest.json"
    manifest_text = manifest_path.read_text(encoding="utf-8")
    manifest_path.write_text(
        manifest_text.replace(
            '  "schema": "sccp-release-bundle-v1"\n',
            (
                '  "schema": "sccp-release-bundle-v1",\n'
                '  "schema": "sccp-release-bundle-v1"\n'
            ),
            1,
        ),
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
    assert "manifest JSON contains duplicate key: schema" in verified.stdout


def test_release_bundle_verifier_rejects_report_summary_duplicate_json_keys(
    tmp_path: Path,
) -> None:
    """Readiness report and all-lanes summary JSON roots reject duplicate keys."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report_text = report_path.read_text(encoding="utf-8")
    report_path.write_text(
        report_text.replace(
            '  "production_ready": true,\n',
            '  "production_ready": true,\n  "production_ready": true,\n',
            1,
        ),
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary_text = summary_path.read_text(encoding="utf-8")
    summary_path.write_text(
        summary_text.replace(
            '  "production_ready": true,\n',
            '  "production_ready": true,\n  "production_ready": true,\n',
            1,
        ),
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
    assert (
        "readiness report JSON contains duplicate key: production_ready"
        in verified.stdout
    )
    assert (
        "all-lanes summary JSON contains duplicate key: production_ready"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_non_utf8_manifest_json(
    tmp_path: Path,
) -> None:
    """The verifier must fail closed instead of crashing on non-UTF-8 manifest JSON."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "manifest.json").write_bytes(b"\xff")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "manifest JSON is not UTF-8 text:" in verified.stdout
    assert "Traceback" not in verified.stderr


def test_release_bundle_verifier_rejects_non_utf8_report_summary_json(
    tmp_path: Path,
) -> None:
    """Published report JSON roots must be UTF-8 text."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "sccp-release-readiness.json").write_bytes(b"\xff")
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")
    (output_dir / "sccp-all-lanes-summary.json").write_bytes(b"\xfe")
    rewrite_manifest_artifact(output_dir, "sccp-all-lanes-summary.json")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report JSON is not UTF-8 text:" in verified.stdout
    assert "all-lanes summary JSON is not UTF-8 text:" in verified.stdout
    assert "Traceback" not in verified.stderr


def test_release_bundle_verifier_rejects_non_utf8_public_markdown(
    tmp_path: Path,
) -> None:
    """Published Markdown artifacts must be UTF-8 text."""

    output_dir = build_ready_bundle(tmp_path)
    (output_dir / "sccp-release-readiness.md").write_bytes(b"\xff")
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.md")
    (output_dir / "sccp-release-notes-attachment.md").write_bytes(b"\xfe")
    rewrite_manifest_artifact(output_dir, "sccp-release-notes-attachment.md")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert "readiness report Markdown is not UTF-8 text:" in verified.stdout
    assert "release-notes attachment is not UTF-8 text:" in verified.stdout
    assert "Traceback" not in verified.stderr


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


def test_release_bundle_verifier_corridor_phase_inventory_matches_runner() -> None:
    """Verifier-owned corridor phases must stay aligned with the runner plan."""

    verifier = load_verify_helpers()
    report = load_report_module()

    assert verifier.CORRIDOR_PHASES == tuple(report._corridor_phases())
    assert set(verifier.CORRIDOR_PHASES) == set(
        verifier.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS
    )
    assert set(verifier.CORRIDOR_PHASES) == set(
        verifier.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS
    )


def test_release_bundle_verifier_corridor_phase_inventory_is_independent(
) -> None:
    """A weakened report module must not shrink required corridor phases."""

    verifier = load_verify_helpers()
    assert not hasattr(verifier, "_report_module")
    omitted_phase = "swift-sdk"
    weak_phases = [
        phase for phase in verifier.CORRIDOR_PHASES if phase != omitted_phase
    ]
    corridor = {
        "phases": {phase: "passed" for phase in weak_phases},
        "evidence_artifacts": {
            phase: {"path": f"corridor/{phase}.log", "bytes": 1, "sha256": "0" * 64}
            for phase in weak_phases
        },
    }

    errors = verifier._corridor_phase_errors(corridor)

    assert f"readiness report corridor missing phase status: {omitted_phase}" in errors


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
    assert (
        "sccp-all-lanes-summary.json sha256 must be a canonical SHA-256 hex string"
        in verified.stdout
    )
    assert (
        "readiness report input artifact bytes must be a non-negative integer "
        "for evidence/00-complete.toml"
    ) in verified.stdout
    assert (
        "readiness report input artifact sha256 must be a canonical SHA-256 hex string for "
        "evidence/00-complete.toml"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_artifact_digest_text_drift(
    tmp_path: Path,
) -> None:
    """Manifest and report artifacts must carry canonical SHA-256 text."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["input_artifacts"][0]["sha256"] = "A" * 64
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rewrite_manifest_artifact(output_dir, "sccp-release-readiness.json")

    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest["artifacts"]:
        if artifact["path"] == "sccp-all-lanes-summary.json":
            artifact["sha256"] = "0" * 63
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
        "sccp-all-lanes-summary.json sha256 must be a canonical SHA-256 hex string"
        in verified.stdout
    )
    assert (
        "readiness report input artifact sha256 must be a canonical SHA-256 hex string for "
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


def test_release_bundle_verifier_rejects_release_checklist_blocked_items(
    tmp_path: Path,
) -> None:
    """Published release checklist rows must be ready and blocker-free."""

    def mutate_checklist(checklist: dict) -> str:
        item = checklist["items"][0]
        item["ready"] = False
        item["blockers"] = ["manual approval pending"]
        return item["id"]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report_item_id = mutate_checklist(report["release_checklist"])
    mutate_checklist(report["evidence"]["release_checklist"])
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    summary_path = output_dir / "sccp-all-lanes-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary_item_id = mutate_checklist(summary["release_checklist"])
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
        f"readiness report release_checklist item {report_item_id} ready must be true"
    ) in verified.stdout
    assert (
        f"readiness report release_checklist item {report_item_id} blockers must be empty"
    ) in verified.stdout
    assert summary_item_id == "all_required_lane_records"
    assert "all-lanes summary does not match copied evidence inputs" in verified.stdout


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


def test_release_bundle_verifier_rejects_corridor_blockers(
    tmp_path: Path,
) -> None:
    """Published production-corridor roots must be blocker-free."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["corridor"]["blockers"] = ["manual corridor blocker"]
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
    assert "readiness report corridor blockers must be empty" in verified.stdout
    assert (
        "readiness report production corridor contains blockers"
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


def test_release_bundle_verifier_rejects_all_lanes_lane_not_ready(
    tmp_path: Path,
) -> None:
    """Published all-lanes lane rows must be ready and blocker-free."""

    def mutate_summary(summary: dict) -> None:
        active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
        active_lane = next(
            lane for lane in summary["lanes"] if lane["domain"] == active_domain
        )
        active_lane["production_ready"] = False
        active_lane["blockers"] = ["manual blocker"]

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
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    for label in (
        f"readiness report embedded evidence lane domain {active_domain}",
        f"all-lanes summary lane domain {active_domain}",
    ):
        assert f"{label} production_ready must be true" in verified.stdout
        assert f"{label} blockers must be empty" in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_root_blockers(
    tmp_path: Path,
) -> None:
    """Published all-lanes roots must be blocker-free when release-ready."""

    def mutate_summary(summary: dict) -> None:
        summary["blockers"] = ["manual root blocker"]

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
    report = load_report_module()
    active_label = f"{report.ACTIVE_LAUNCH_CHAIN.upper()} mainnet"
    assert (
        f"readiness report embedded evidence active {active_label} launch blockers must be empty"
        in verified.stdout
    )
    assert (
        f"all-lanes summary active {active_label} launch blockers must be empty"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_all_lanes_missing_record_flags(
    tmp_path: Path,
) -> None:
    """Published ready lane rows must not advertise missing evidence records."""

    def mutate_summary(summary: dict) -> None:
        active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
        active_lane = next(
            lane for lane in summary["lanes"] if lane["domain"] == active_domain
        )
        records = active_lane["records"]
        for field in (
            "source_verifier_material",
            "source_adapter_deployment",
            "destination_rollout",
            "route_allowlist",
        ):
            records[field] = False

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
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    for label in (
        f"readiness report embedded evidence lane domain {active_domain} records",
        f"all-lanes summary lane domain {active_domain} records",
    ):
        for field in (
            "source_verifier_material",
            "source_adapter_deployment",
            "destination_rollout",
            "route_allowlist",
        ):
            assert f"{label} {field} must be true" in verified.stdout


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


def test_release_bundle_verifier_rejects_all_lanes_unknown_domain_and_chain_drift(
    tmp_path: Path,
) -> None:
    """All-lanes lane domains and chain labels must match production lanes."""

    def mutate_summary(summary: dict) -> None:
        summary["required_domains"][0] = 99
        summary["lanes"][0]["domain"] = 99
        summary["lanes"][0]["chain"] = "operator"
        summary["lanes"][1]["chain"] = "eth"

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
    for label in (
        "readiness report embedded evidence",
        "all-lanes summary",
    ):
        assert (
            f"{label} required_domains must be the production remote domains"
        ) in verified.stdout
        assert f"{label} lane domains must be the production remote domains" in (
            verified.stdout
        )
        assert f"{label} lane domain 99 domain must be a production remote domain" in (
            verified.stdout
        )
        assert f"{label} lane domain 2 chain must be bsc" in verified.stdout


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
        "1 source_adapter_gate gate_hash must be empty or a non-zero canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain "
        "1 source_adapter_gate audit_hashes audit must be a non-zero canonical "
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


def test_release_bundle_verifier_rejects_all_lanes_destination_binding_field_shape(
    tmp_path: Path,
) -> None:
    """Lane-specific destination binding fields must match the destination family."""

    def lane_for_domain(lanes: list[dict], domain: int) -> dict:
        for lane in lanes:
            if lane["domain"] == domain:
                return lane
        raise AssertionError(f"lane domain {domain} not found")

    def mutate_lanes(lanes: list[dict]) -> None:
        eth_destination = lane_for_domain(lanes, 1)["destination_binding"]
        eth_destination.pop("destination_network_id", None)
        eth_destination.pop("destination_bridge_address", None)

        ton_destination = lane_for_domain(lanes, 4)["destination_binding"]
        ton_destination["destination_network_id"] = "0x" + "22" * 32

        tron_destination = lane_for_domain(lanes, 5)["destination_binding"]
        tron_destination.pop("destination_network_id", None)
        tron_destination["destination_bridge_address"] = "0x" + "11" * 20

        solana_destination = lane_for_domain(lanes, 3)["destination_binding"]
        solana_destination["destination_bridge_address"] = "0x" + "33" * 20

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
    for label in (
        "readiness report embedded evidence",
        "all-lanes summary",
    ):
        assert (
            f"{label} lane domain 1 destination_binding destination_network_id "
            "is required for EVM-family lanes"
        ) in verified.stdout
        assert (
            f"{label} lane domain 1 destination_binding "
            "destination_bridge_address is required for EVM-family lanes"
        ) in verified.stdout
        assert (
            f"{label} lane domain 5 destination_binding destination_network_id "
            "is required for TRON lanes"
        ) in verified.stdout
        assert (
            f"{label} lane domain 5 destination_binding "
            "destination_bridge_address is only valid for EVM-family lanes"
        ) in verified.stdout
        assert (
            f"{label} lane domain 4 destination_binding destination_network_id "
            "is only valid for EVM-family or TRON lanes"
        ) in verified.stdout
        assert (
            f"{label} lane domain 3 destination_binding "
            "destination_bridge_address is only valid for EVM-family lanes"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_zero_governed_hashes(
    tmp_path: Path,
) -> None:
    """Governed lane hashes in public all-lanes JSON must be non-zero."""

    zero_hash = "0x" + "00" * 32

    def mutate_lane(lane: dict) -> None:
        lane["source_record_hashes"]["source_verifier_material_hash"] = zero_hash
        lane["source_record_hashes"][
            "source_adapter_engine_deployment_hash"
        ] = zero_hash
        lane["destination_binding"]["destination_binding_hash"] = zero_hash
        lane["destination_binding"]["expected_destination_binding_hash"] = zero_hash
        lane["destination_binding"]["destination_bridge_address"] = "0x" + "00" * 20
        lane["destination_binding"]["destination_network_id"] = zero_hash
        lane["route_allowlist"]["route_allowlist_hash"] = zero_hash
        lane["route_allowlist"]["expected_route_allowlist_hash"] = zero_hash

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
    for label in (
        "readiness report embedded evidence lane domain 1",
        "all-lanes summary lane domain 1",
    ):
        assert (
            f"{label} source_record_hashes source_verifier_material_hash must be "
            "a non-zero canonical bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} source_record_hashes "
            "source_adapter_engine_deployment_hash must be a non-zero canonical "
            "bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} destination_binding destination_binding_hash must be a "
            "non-zero canonical bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} destination_binding expected_destination_binding_hash must be "
            "a non-zero canonical bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} destination_binding destination_bridge_address must be a "
            "non-zero canonical 20-byte hex string"
        ) in verified.stdout
        assert (
            f"{label} destination_binding destination_network_id must be a "
            "non-zero canonical bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} route_allowlist route_allowlist_hash must be a non-zero "
            "canonical bytes32 hex string"
        ) in verified.stdout
        assert (
            f"{label} route_allowlist expected_route_allowlist_hash must be a "
            "non-zero canonical bytes32 hex string"
        ) in verified.stdout


def test_release_bundle_verifier_recomputes_all_lanes_route_allowlist_hash(
    tmp_path: Path,
) -> None:
    """Public bundle review must not trust self-attested route allowlist hashes."""

    def replacement_hash(current: str) -> str:
        replacement = "0x" + "ab" * 32
        return "0x" + "cd" * 32 if current == replacement else replacement

    def mutate_lanes(lanes: list[dict]) -> None:
        eth_lane = next(lane for lane in lanes if lane["domain"] == 1)
        eth_lane["source_record_hashes"][
            "source_adapter_engine_deployment_hash"
        ] = eth_lane["source_record_hashes"]["source_verifier_material_hash"]

        bsc_lane = next(lane for lane in lanes if lane["domain"] == 2)
        bsc_route = bsc_lane["route_allowlist"]
        forged_route_hash = replacement_hash(bsc_route["route_allowlist_hash"])
        bsc_route["route_allowlist_hash"] = forged_route_hash
        bsc_route["expected_route_allowlist_hash"] = forged_route_hash
        bsc_route["route_canary"]["route_allowlist_hash"] = forged_route_hash

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
    for label in (
        "readiness report embedded evidence",
        "all-lanes summary",
    ):
        assert (
            f"{label} lane domain 1 route_allowlist governed hash role "
            "source_adapter_engine_deployment_hash must not reuse "
            "source_verifier_material_hash"
        ) in verified.stdout
        assert (
            f"{label} lane domain 2 route_allowlist route_allowlist_hash "
            "must recompute from source material, source adapter deployment, "
            "and destination binding hashes"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_all_lanes_zero_source_gate_hashes(
    tmp_path: Path,
) -> None:
    """Required source-adapter gate hashes in public JSON must be non-zero."""

    zero_hash = "0x" + "00" * 32

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        source_gate = by_domain[3]["source_adapter_gate"]
        source_gate["gate_hash"] = zero_hash
        for field in tuple(source_gate["audit_hashes"]):
            source_gate["audit_hashes"][field] = zero_hash

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
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert (
            f"{label} gate_hash must be empty or a non-zero canonical bytes32 "
            "hex string"
        ) in verified.stdout
        for field in (
            "solana_tower_replay_verifier_hash",
            "solana_full_accountsdb_lattice_verifier_hash",
            "solana_bank_fork_choice_verifier_hash",
        ):
            assert (
                f"{label} audit_hashes {field} must be a non-zero canonical "
                "bytes32 hex string"
            ) in verified.stdout


def test_release_bundle_verifier_rejects_required_source_gate_ready_without_audits(
    tmp_path: Path,
) -> None:
    """Required ready source-adapter gates must carry audited gate evidence."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        source_gate = by_domain[3]["source_adapter_gate"]
        source_gate["gate_hash"] = ""
        source_gate["audit_hashes"] = {}
        source_gate["ready"] = True
        source_gate["blockers"] = ["manual override pending"]

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
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert (
            f"{label} gate_hash must be a non-zero canonical bytes32 hex string "
            "when required"
        ) in verified.stdout
        assert (
            f"{label} audit_hashes must not be empty when required"
        ) in verified.stdout
        assert f"{label} blockers must be empty when ready" in verified.stdout


def test_release_bundle_verifier_rejects_required_source_gate_not_ready(
    tmp_path: Path,
) -> None:
    """Required source-adapter gates must be ready in public release bundles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        source_gate = by_domain[3]["source_adapter_gate"]
        source_gate["ready"] = False
        source_gate["blockers"] = ["source gate audit pending"]

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
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert f"{label} ready must be true when gate is required" in verified.stdout
        assert f"{label} blockers must be empty when gate is required" in (
            verified.stdout
        )


def test_release_bundle_verifier_rejects_required_source_gate_unbacked_hash(
    tmp_path: Path,
) -> None:
    """Required source-adapter gate hashes must be backed by a named audit hash."""

    forged_gate_hash = "0x" + "ef" * 32

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        source_gate = by_domain[3]["source_adapter_gate"]
        assert forged_gate_hash not in set(source_gate["audit_hashes"].values())
        source_gate["gate_hash"] = forged_gate_hash

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
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert f"{label} gate_hash must match one audit_hashes value" in (
            verified.stdout
        )


def test_release_bundle_verifier_rejects_source_gate_hash_named_role_drift(
    tmp_path: Path,
) -> None:
    """Required source-adapter gate hashes must point at the named gate transcript."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        sol_gate = by_domain[3]["source_adapter_gate"]
        sol_gate["gate_hash"] = sol_gate["audit_hashes"][
            "solana_tower_replay_verifier_hash"
        ]
        ton_gate = by_domain[4]["source_adapter_gate"]
        ton_gate["gate_hash"] = ton_gate["audit_hashes"][
            "ton_masterchain_config_verifier_hash"
        ]

    def mutate_crypto_rows(rows: list[dict]) -> None:
        by_domain = {row["domain"]: row for row in rows}
        sol_row = by_domain[3]
        sol_row["source_adapter_gate_hash"] = sol_row[
            "source_adapter_gate_audit_hashes"
        ]["solana_tower_replay_verifier_hash"]
        ton_row = by_domain[4]
        ton_row["source_adapter_gate_hash"] = ton_row[
            "source_adapter_gate_audit_hashes"
        ]["ton_masterchain_config_verifier_hash"]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lanes(report["evidence"]["lanes"])
    mutate_crypto_rows(report["cryptographic_evidence"])
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
    for label, gate_field in (
        (
            "readiness report embedded evidence lane domain 3 source_adapter_gate",
            "solana_full_light_client_gate_hash",
        ),
        (
            "all-lanes summary lane domain 3 source_adapter_gate",
            "solana_full_light_client_gate_hash",
        ),
        (
            "readiness report embedded evidence lane domain 4 source_adapter_gate",
            "ton_full_light_client_gate_hash",
        ),
        (
            "all-lanes summary lane domain 4 source_adapter_gate",
            "ton_full_light_client_gate_hash",
        ),
    ):
        assert (
            f"{label} gate_hash must match audit_hashes.{gate_field}"
            in verified.stdout
        )


def test_release_bundle_verifier_rejects_source_gate_domain_policy_drift(
    tmp_path: Path,
) -> None:
    """Source-adapter gate policy must match the lane domain."""

    forged_gate_hash = "0x" + "12" * 32

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        open_gate = by_domain[1]["source_adapter_gate"]
        open_gate["ready"] = False
        open_gate["gate_hash"] = forged_gate_hash
        open_gate["audit_hashes"] = {"operator_override": forged_gate_hash}
        open_gate["blockers"] = ["operator override pending"]
        by_domain[3]["source_adapter_gate"]["required"] = False

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
    for label in (
        "readiness report embedded evidence lane domain 1 source_adapter_gate",
        "all-lanes summary lane domain 1 source_adapter_gate",
    ):
        assert f"{label} ready must be true when gate is not required" in (
            verified.stdout
        )
        assert f"{label} audit_hashes must be empty when gate is not required" in (
            verified.stdout
        )
        assert f"{label} gate_hash must be empty when gate is not required" in (
            verified.stdout
        )
        assert f"{label} blockers must be empty when gate is not required" in (
            verified.stdout
        )
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert f"{label} required must be true for this lane domain" in verified.stdout


def test_release_bundle_verifier_rejects_required_source_gate_audit_key_drift(
    tmp_path: Path,
) -> None:
    """Required source-adapter gates must carry the expected named audit hashes."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        source_gate = by_domain[3]["source_adapter_gate"]
        gate_hash = source_gate["audit_hashes"].pop(
            "solana_full_light_client_gate_hash"
        )
        source_gate["audit_hashes"]["operator_override"] = gate_hash
        substrate_gate = by_domain[6]["source_adapter_gate"]
        substrate_gate_hash = substrate_gate["audit_hashes"].pop(
            "substrate_runtime_storage_gate_hash"
        )
        substrate_gate["audit_hashes"]["runtime_storage_operator_note"] = (
            substrate_gate_hash
        )

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
    for label in (
        "readiness report embedded evidence lane domain 3 source_adapter_gate",
        "all-lanes summary lane domain 3 source_adapter_gate",
    ):
        assert (
            f"{label} audit_hashes contains unexpected field: operator_override"
        ) in verified.stdout
        assert (
            f"{label} audit_hashes missing field: solana_full_light_client_gate_hash"
        ) in verified.stdout
    for label in (
        "readiness report embedded evidence lane domain 6 source_adapter_gate",
        "all-lanes summary lane domain 6 source_adapter_gate",
    ):
        assert (
            f"{label} audit_hashes contains unexpected field: "
            "runtime_storage_operator_note"
        ) in verified.stdout
        assert (
            f"{label} audit_hashes missing field: "
            "substrate_runtime_storage_gate_hash"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_source_gate_audit_hash_role_reuse(
    tmp_path: Path,
) -> None:
    """Source-adapter gate audit hashes must not replay governed lane hashes."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        sol_lane = by_domain[3]
        source_gate = sol_lane["source_adapter_gate"]
        canary_hash = sol_lane["route_allowlist"]["route_canary"]["evidence_hash"]
        source_gate["gate_hash"] = canary_hash
        source_gate["audit_hashes"]["solana_full_light_client_gate_hash"] = canary_hash

        ton_lane = by_domain[4]
        ton_gate = ton_lane["source_adapter_gate"]
        duplicated_audit_hash = ton_gate["audit_hashes"][
            "ton_masterchain_config_verifier_hash"
        ]
        ton_gate["audit_hashes"][
            "ton_validator_set_transition_verifier_hash"
        ] = duplicated_audit_hash

        substrate_lane = by_domain[8]
        substrate_gate = substrate_lane["source_adapter_gate"]
        destination_hash = substrate_lane["destination_binding"][
            "destination_binding_hash"
        ]
        substrate_gate["gate_hash"] = destination_hash
        substrate_gate["audit_hashes"]["substrate_runtime_storage_gate_hash"] = (
            destination_hash
        )

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
    for label in (
        "readiness report embedded evidence",
        "all-lanes summary",
    ):
        assert (
            f"{label} lane domain 3 source_adapter_gate hash role "
            "audit_hashes.solana_full_light_client_gate_hash must not reuse "
            "route_canary_evidence_hash"
        ) in verified.stdout
        assert (
            f"{label} lane domain 4 source_adapter_gate hash role "
            "audit_hashes.ton_validator_set_transition_verifier_hash must not "
            "reuse audit_hashes.ton_masterchain_config_verifier_hash"
        ) in verified.stdout
        assert (
            f"{label} lane domain 8 source_adapter_gate hash role "
            "audit_hashes.substrate_runtime_storage_gate_hash must not reuse "
            "destination_binding_hash"
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
        evm_canary["receipt_block_number"] = 0
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
        tron_canary["block_number"] = 0
        tron_canary["block_timestamp"] = -1
        tron_canary["raw_data_owner_matches_transaction"] = False
        tron_canary["signature_recovers_to_owner"] = "true"

        substrate_canary = by_domain[6]["route_allowlist"]["route_canary"]
        substrate_canary["substrate_finalized_head"] = True
        substrate_canary["substrate_runtime_code_hash"] = "0X" + "55" * 32
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
        "route_canary receipt_block_number must be a positive integer"
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
        "route_canary solana_programdata_address must be a non-zero canonical "
        "base58 Solana address"
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
        "route_canary transaction_owner_address must be a non-zero canonical "
        "0x41-prefixed 21-byte hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary block_number must be a positive integer"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary block_timestamp must be a non-negative integer"
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
        "route_canary substrate_runtime_code_hash must be a canonical bytes32 hex string"
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
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "receipt_block_number must be a positive integer"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "transaction_owner_address must be a non-zero canonical 0x41-prefixed "
        "21-byte hex string"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "block_number must be a positive integer"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_tron_route_canary_zero_addresses(
    tmp_path: Path,
) -> None:
    """TRON route-canary owner and recovered signer addresses must be non-zero."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        zero_address = "0x41" + "00" * 20
        tron_canary["transaction_owner_address"] = zero_address
        tron_canary["signature_recovered_address"] = zero_address

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
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary transaction_owner_address must be a non-zero canonical "
        "0x41-prefixed 21-byte hex string"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary signature_recovered_address must be a non-zero canonical "
        "0x41-prefixed 21-byte hex string"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "transaction_owner_address must be a non-zero canonical 0x41-prefixed "
        "21-byte hex string"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "signature_recovered_address must be a non-zero canonical "
        "0x41-prefixed 21-byte hex string"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_tron_route_canary_zero_binding_hashes(
    tmp_path: Path,
) -> None:
    """TRON route-canary common binding hashes must be non-zero."""

    common_hash_fields = (
        "evidence_hash",
        "route_allowlist_hash",
        "destination_binding_hash",
    )

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        for field in common_hash_fields:
            tron_canary[field] = "0x" + "00" * 32

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
    for label in (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 5 route_allowlist route_canary",
    ):
        for field in common_hash_fields:
            assert (
                f"{label} {field} must be a non-zero canonical bytes32 hex string"
                in verified.stdout
            )


def test_release_bundle_verifier_rejects_tron_route_canary_zero_transcript_words(
    tmp_path: Path,
) -> None:
    """TRON route-canary transcript words must match runtime non-zero policy."""

    transcript_fields = (
        "transaction_id",
        "message_id",
        "call_data_sha256",
        "payload_hash",
        "statement_hash",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
        "signature_sha256",
    )

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        for field in transcript_fields:
            tron_canary[field] = "0x" + "00" * 32

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
    for label in (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 5 route_allowlist route_canary",
    ):
        for field in transcript_fields:
            assert (
                f"{label} {field} must be a non-zero canonical bytes32 hex string"
                in verified.stdout
            )


def test_release_bundle_verifier_rejects_evm_route_canary_zero_transcript_words(
    tmp_path: Path,
) -> None:
    """EVM route-canary transcript words must match runtime non-zero policy."""

    transcript_fields = (
        "transaction_hash",
        "receipt_block_hash",
        "block_receipts_root",
        "call_data_sha256",
        "message_id",
        "payload_hash",
        "statement_hash",
        "commitment_root",
        "finality_height",
        "finality_block_hash",
    )

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        for domain in (1, 2):
            evm_canary = by_domain[domain]["route_allowlist"]["route_canary"]
            for field in transcript_fields:
                evm_canary[field] = "0x" + "00" * 32

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
    for domain in (1, 2):
        for label in (
            f"readiness report embedded evidence lane domain {domain} "
            "route_allowlist route_canary",
            f"all-lanes summary lane domain {domain} route_allowlist route_canary",
        ):
            for field in transcript_fields:
                assert (
                    f"{label} {field} must be a non-zero canonical bytes32 "
                    "hex string"
                in verified.stdout
            )


def test_release_bundle_verifier_rejects_evm_route_canary_transcript_hash_reuse(
    tmp_path: Path,
) -> None:
    """EVM route-canary transcript hash roles must stay distinct."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        evm_canary = by_domain[1]["route_allowlist"]["route_canary"]
        evm_canary["receipt_block_hash"] = evm_canary["transaction_hash"]
        evm_canary["block_receipts_root"] = evm_canary["transaction_hash"]
        evm_canary["payload_hash"] = evm_canary["call_data_sha256"]
        evm_canary["finality_height"] = evm_canary["transaction_hash"]

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
        "route_canary transcript hash receipt_block_hash must not reuse "
        "transaction_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary transcript hash block_receipts_root must not reuse "
        "transaction_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary transcript hash payload_hash must not reuse "
        "call_data_sha256"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 1 route_allowlist "
        "route_canary transcript hash finality_height must not reuse "
        "transaction_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "transcript hash receipt_block_hash must not reuse transaction_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "transcript hash block_receipts_root must not reuse transaction_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "transcript hash payload_hash must not reuse call_data_sha256"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "transcript hash finality_height must not reuse transaction_hash"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_evm_route_canary_governed_hash_reuse(
    tmp_path: Path,
) -> None:
    """EVM route-canary hashes must not reuse governed lane hash roles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        evm_lane = by_domain[1]
        evm_canary = evm_lane["route_allowlist"]["route_canary"]
        evm_canary["message_id"] = evm_lane["source_record_hashes"][
            "source_verifier_material_hash"
        ]

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
        "route_canary hash role message_id must not reuse "
        "source_verifier_material_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 1 route_allowlist route_canary "
        "hash role message_id must not reuse source_verifier_material_hash"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_solana_route_canary_zero_programdata_address(
    tmp_path: Path,
) -> None:
    """Solana route-canary ProgramData address must be a non-zero pubkey."""

    zero_solana_pubkey = "1" * 32

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        solana_canary = by_domain[3]["route_allowlist"]["route_canary"]
        solana_canary["solana_programdata_address"] = zero_solana_pubkey

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
    for label in (
        "readiness report embedded evidence lane domain 3 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 3 route_allowlist route_canary",
    ):
        assert (
            f"{label} solana_programdata_address must be a non-zero canonical "
            "base58 Solana address"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_ton_route_canary_zero_live_hashes(
    tmp_path: Path,
) -> None:
    """TON route-canary live-account hashes must match runtime non-zero policy."""

    live_hash_fields = ("ton_account_state_hash", "ton_last_transaction_hash")

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        ton_canary = by_domain[4]["route_allowlist"]["route_canary"]
        for field in live_hash_fields:
            ton_canary[field] = "0x" + "00" * 32

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
    for label in (
        "readiness report embedded evidence lane domain 4 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 4 route_allowlist route_canary",
    ):
        for field in live_hash_fields:
            assert (
                f"{label} {field} must be a non-zero canonical bytes32 hex string"
                in verified.stdout
            )


def test_release_bundle_verifier_rejects_ton_route_canary_hash_role_reuse(
    tmp_path: Path,
) -> None:
    """TON route-canary live-account hashes must not replay governed hashes."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        ton_lane = by_domain[4]
        ton_canary = ton_lane["route_allowlist"]["route_canary"]
        ton_canary["ton_account_state_hash"] = ton_lane["destination_binding"][
            "destination_binding_hash"
        ]
        ton_canary["ton_last_transaction_hash"] = ton_lane["route_allowlist"][
            "route_allowlist_hash"
        ]

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
    for label in (
        "readiness report embedded evidence lane domain 4 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 4 route_allowlist route_canary",
    ):
        assert (
            f"{label} hash role ton_account_state_hash must not reuse "
            "destination_binding_hash"
        ) in verified.stdout
        assert (
            f"{label} hash role ton_last_transaction_hash must not reuse "
            "route_allowlist_hash"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_substrate_route_canary_zero_runtime_hashes(
    tmp_path: Path,
) -> None:
    """Substrate route-canary runtime hashes must be non-zero bytes32 values."""

    substrate_domains = (6, 7, 8)
    runtime_hash_fields = ("substrate_finalized_head", "substrate_runtime_code_hash")

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        for domain in substrate_domains:
            substrate_canary = by_domain[domain]["route_allowlist"]["route_canary"]
            for field in runtime_hash_fields:
                substrate_canary[field] = "0x" + "00" * 32

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
    for domain in substrate_domains:
        for label in (
            f"readiness report embedded evidence lane domain {domain} "
            "route_allowlist route_canary",
            f"all-lanes summary lane domain {domain} route_allowlist route_canary",
        ):
            for field in runtime_hash_fields:
                assert (
                    f"{label} {field} must be a non-zero canonical "
                    "bytes32 hex string"
                ) in verified.stdout


def test_release_bundle_verifier_rejects_substrate_route_canary_hash_role_reuse(
    tmp_path: Path,
) -> None:
    """Substrate route-canary hashes must not replay governed lane hash roles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        substrate_lane = by_domain[6]
        substrate_canary = substrate_lane["route_allowlist"]["route_canary"]
        substrate_canary["substrate_finalized_head"] = substrate_lane[
            "source_record_hashes"
        ]["source_verifier_material_hash"]
        substrate_canary["substrate_runtime_code_hash"] = substrate_lane[
            "destination_binding"
        ]["destination_binding_hash"]

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
    for label in (
        "readiness report embedded evidence lane domain 6 route_allowlist "
        "route_canary",
        "all-lanes summary lane domain 6 route_allowlist route_canary",
    ):
        assert (
            f"{label} hash role substrate_finalized_head must not reuse "
            "source_verifier_material_hash"
        ) in verified.stdout
        assert (
            f"{label} hash role substrate_runtime_code_hash must not reuse "
            "destination_binding_hash"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_tron_route_canary_transcript_hash_reuse(
    tmp_path: Path,
) -> None:
    """TRON route-canary transcript hash roles must stay distinct."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        tron_canary["finality_height"] = tron_canary["transaction_id"]
        tron_canary["signature_sha256"] = tron_canary["finality_block_hash"]

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
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary transcript hash finality_height must not reuse "
        "transaction_id"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary transcript hash signature_sha256 must not reuse "
        "finality_block_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "transcript hash finality_height must not reuse transaction_id"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "transcript hash signature_sha256 must not reuse finality_block_hash"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_tron_route_canary_governed_hash_reuse(
    tmp_path: Path,
) -> None:
    """TRON route-canary hashes must not reuse governed lane hash roles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_lane = by_domain[5]
        tron_canary = tron_lane["route_allowlist"]["route_canary"]
        tron_canary["message_id"] = tron_lane["source_record_hashes"][
            "source_adapter_engine_deployment_hash"
        ]

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
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary hash role message_id must not reuse "
        "source_adapter_engine_deployment_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "hash role message_id must not reuse "
        "source_adapter_engine_deployment_hash"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_tron_route_canary_recovered_owner_drift(
    tmp_path: Path,
) -> None:
    """TRON route-canary signer evidence must bind recovered signer to owner."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        tron_canary = by_domain[5]["route_allowlist"]["route_canary"]
        replacement = "0x41" + "22" * 20
        if tron_canary["transaction_owner_address"] == replacement:
            replacement = "0x41" + "33" * 20
        tron_canary["signature_recovered_address"] = replacement

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
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary signature_recovered_address must match "
        "transaction_owner_address"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "signature_recovered_address must match transaction_owner_address"
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


def test_release_bundle_verifier_rejects_route_canary_evidence_hash_role_reuse(
    tmp_path: Path,
) -> None:
    """Route-canary evidence hashes must not replay governed or canary roles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        by_domain[1]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            1
        ]["route_allowlist"]["route_allowlist_hash"]
        by_domain[2]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            2
        ]["source_record_hashes"]["source_verifier_material_hash"]
        by_domain[3]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            3
        ]["destination_binding"]["destination_binding_hash"]
        by_domain[4]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            4
        ]["route_allowlist"]["route_canary"]["ton_account_state_hash"]
        by_domain[6]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            6
        ]["route_allowlist"]["route_canary"]["substrate_runtime_code_hash"]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lanes(report["evidence"]["lanes"])
    evidence_by_domain = {lane["domain"]: lane for lane in report["evidence"]["lanes"]}
    for row in report["cryptographic_evidence"]:
        row["route_canary_evidence_hash"] = evidence_by_domain[row["domain"]][
            "route_allowlist"
        ]["route_canary"]["evidence_hash"]
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
    expected_failures = (
        (
            1,
            "hash role evidence_hash must not reuse route_allowlist_hash",
        ),
        (
            2,
            "hash role evidence_hash must not reuse source_verifier_material_hash",
        ),
        (
            3,
            "hash role evidence_hash must not reuse destination_binding_hash",
        ),
        (
            4,
            "hash role evidence_hash must not reuse ton_account_state_hash",
        ),
        (
            6,
            "hash role evidence_hash must not reuse substrate_runtime_code_hash",
        ),
    )
    for domain, failure in expected_failures:
        assert (
            "readiness report embedded evidence lane domain "
            f"{domain} route_allowlist route_canary {failure}"
        ) in verified.stdout
        assert (
            f"all-lanes summary lane domain {domain} route_allowlist "
            f"route_canary {failure}"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_cross_lane_route_canary_evidence_replay(
    tmp_path: Path,
) -> None:
    """Route-canary evidence hashes must not replay another lane's canary roles."""

    def mutate_lanes(lanes: list[dict]) -> None:
        by_domain = {lane["domain"]: lane for lane in lanes}
        by_domain[2]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            1
        ]["route_allowlist"]["route_canary"]["evidence_hash"]
        by_domain[5]["route_allowlist"]["route_canary"]["evidence_hash"] = by_domain[
            4
        ]["source_record_hashes"]["source_adapter_engine_deployment_hash"]

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    mutate_lanes(report["evidence"]["lanes"])
    evidence_by_domain = {lane["domain"]: lane for lane in report["evidence"]["lanes"]}
    for row in report["cryptographic_evidence"]:
        row["route_canary_evidence_hash"] = evidence_by_domain[row["domain"]][
            "route_allowlist"
        ]["route_canary"]["evidence_hash"]
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
        "readiness report embedded evidence lane domain 2 route_allowlist "
        "route_canary evidence_hash must be distinct from readiness report "
        "embedded evidence lane domain 1 route_allowlist route_canary "
        "evidence_hash"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 2 route_allowlist route_canary "
        "evidence_hash must be distinct from all-lanes summary lane domain 1 "
        "route_allowlist route_canary evidence_hash"
    ) in verified.stdout
    assert (
        "readiness report embedded evidence lane domain 5 route_allowlist "
        "route_canary evidence_hash must not reuse "
        "source_adapter_engine_deployment_hash from readiness report embedded "
        "evidence lane domain 4"
    ) in verified.stdout
    assert (
        "all-lanes summary lane domain 5 route_allowlist route_canary "
        "evidence_hash must not reuse source_adapter_engine_deployment_hash "
        "from all-lanes summary lane domain 4"
    ) in verified.stdout


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


def test_release_bundle_verifier_copied_evidence_recompute_is_independent(
    tmp_path: Path,
) -> None:
    """A weakened bundle builder must not recompute copied TOML evidence."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    assert not hasattr(verifier, "_bundle_module")

    summary = verifier.verify_bundle(output_dir)

    assert summary["verified"] is True


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


def test_release_bundle_verifier_readiness_markdown_renderer_matches_report(
    tmp_path: Path,
) -> None:
    """Verifier-owned readiness Markdown rendering must match the report renderer."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    report = json.loads(
        (output_dir / "sccp-release-readiness.json").read_text(encoding="utf-8")
    )

    assert verifier._expected_readiness_markdown(report) == (
        output_dir / "sccp-release-readiness.md"
    ).read_text(encoding="utf-8")


def test_release_bundle_verifier_readiness_markdown_renderer_is_independent(
    tmp_path: Path,
) -> None:
    """A weakened report renderer must not define canonical public Markdown."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    assert not hasattr(verifier, "_report_module")

    summary = verifier.verify_bundle(output_dir)

    assert summary["verified"] is True


def test_release_bundle_verifier_markdown_invariants_require_public_sections(
    tmp_path: Path,
) -> None:
    """Verifier-owned Markdown checks must require the public evidence sections."""

    output_dir = build_ready_bundle(tmp_path)
    verifier = load_verify_helpers()
    report = json.loads(
        (output_dir / "sccp-release-readiness.json").read_text(encoding="utf-8")
    )
    markdown = (output_dir / "sccp-release-readiness.md").read_text(
        encoding="utf-8"
    )

    assert verifier._readiness_markdown_invariant_errors(report, markdown) == []

    errors = verifier._readiness_markdown_invariant_errors(
        report,
        markdown.replace("## Cryptographic Evidence", "## Crypto Summary", 1),
    )

    assert (
        "readiness report Markdown missing section: ## Cryptographic Evidence"
        in errors
    )


def test_release_bundle_verifier_rejects_markdown_crypto_evidence_omission(
    tmp_path: Path,
) -> None:
    """Public Markdown must independently carry cryptographic evidence hashes."""

    output_dir = build_ready_bundle(tmp_path)
    report = json.loads(
        (output_dir / "sccp-release-readiness.json").read_text(encoding="utf-8")
    )
    crypto_hash = report["cryptographic_evidence"][0][
        "source_verifier_material_hash"
    ]
    report_md = output_dir / "sccp-release-readiness.md"
    notes_md = output_dir / "sccp-release-notes-attachment.md"
    old_report_hash = manifest_artifact_hash(output_dir, "sccp-release-readiness.md")
    report_md.write_text(
        report_md.read_text(encoding="utf-8").replace(crypto_hash, "0" * 64, 1),
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
    assert (
        "readiness report Markdown Cryptographic Evidence section missing "
        "source_verifier_material_hash for domain 1"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_unbound_crypto_evidence(
    tmp_path: Path,
) -> None:
    """Every release-report crypto row must bind governed and canary evidence."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    row = next(
        row for row in report["cryptographic_evidence"] if row["domain"] == active_domain
    )
    row["route_canary_evidence_bound"] = False
    row.pop("route_canary_evidence_hash")
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


def test_release_bundle_verifier_rejects_crypto_evidence_zero_hashes(
    tmp_path: Path,
) -> None:
    """The public crypto table must not advertise zero governed hashes."""

    zero_hash = "0x" + "00" * 32

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    row = next(
        row for row in report["cryptographic_evidence"] if row["domain"] == active_domain
    )
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        row[field] = zero_hash
    row["source_adapter_gate_hash"] = zero_hash
    row["source_adapter_gate_audit_hashes"] = {"operator_override": zero_hash}
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
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        assert (
            "readiness report cryptographic evidence row "
            f"{field} must be a non-zero canonical bytes32 hex string"
        ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row source_adapter_gate_hash "
        "must be empty or a non-zero canonical bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_audit_hashes operator_override must be a "
        "non-zero canonical bytes32 hex string"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_crypto_evidence_field_binding_drift(
    tmp_path: Path,
) -> None:
    """Every public crypto field must bind to the embedded lane field it names."""

    def replacement_hash(current: str) -> str:
        candidate = "0x" + "55" * 32
        if current == candidate:
            return "0x" + "66" * 32
        return candidate

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["cryptographic_evidence"][0]
    tron_index, tron_row = next(
        (index, row)
        for index, row in enumerate(report["cryptographic_evidence"])
        if row["domain"] == 5
    )
    source_gate_index, source_gate_row = next(
        (index, row)
        for index, row in enumerate(report["cryptographic_evidence"])
        if row["domain"] == 3
    )
    for field in (
        "source_verifier_material_hash",
        "source_adapter_engine_deployment_hash",
        "destination_binding_hash",
        "route_allowlist_hash",
        "route_canary_evidence_hash",
    ):
        row[field] = replacement_hash(row[field])
    row["route_canary_evidence_source"] = "forged-route-canary"
    row["route_canary_evidence_bound"] = False
    tron_row["route_canary_block_number"] += 1
    tron_row["route_canary_block_timestamp"] += 1
    source_gate_row["source_adapter_gate_required"] = False
    source_gate_row["source_adapter_gate_hash"] = replacement_hash(
        source_gate_row["source_adapter_gate_hash"]
    )
    source_gate_row["source_adapter_gate_audit_hashes"] = {
        field: replacement_hash(value)
        for field, value in source_gate_row[
            "source_adapter_gate_audit_hashes"
        ].items()
    }
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
    for field, lane_field in (
        (
            "source_verifier_material_hash",
            "source_record_hashes.source_verifier_material_hash",
        ),
        (
            "source_adapter_engine_deployment_hash",
            "source_record_hashes.source_adapter_engine_deployment_hash",
        ),
        ("destination_binding_hash", "destination_binding.destination_binding_hash"),
        ("route_allowlist_hash", "route_allowlist.route_allowlist_hash"),
        (
            "route_canary_evidence_hash",
            "route_allowlist.route_canary.evidence_hash",
        ),
        (
            "route_canary_evidence_source",
            "route_allowlist.route_canary.evidence_source",
        ),
        (
            "route_canary_evidence_bound",
            "route_allowlist.route_canary.evidence_bound",
        ),
    ):
        assert (
            "readiness report cryptographic evidence row 0 "
            f"{field} must match embedded lane {lane_field}"
        ) in verified.stdout
    for field, lane_field in (
        ("route_canary_block_number", "route_allowlist.route_canary.block_number"),
        (
            "route_canary_block_timestamp",
            "route_allowlist.route_canary.block_timestamp",
        ),
    ):
        assert (
            "readiness report cryptographic evidence row "
            f"{tron_index} {field} must match embedded lane {lane_field}"
        ) in verified.stdout
    for field, lane_field in (
        ("source_adapter_gate_required", "source_adapter_gate.required"),
        ("source_adapter_gate_hash", "source_adapter_gate.gate_hash"),
        ("source_adapter_gate_audit_hashes", "source_adapter_gate.audit_hashes"),
    ):
        assert (
            "readiness report cryptographic evidence row "
            f"{source_gate_index} {field} must match embedded lane {lane_field}"
        ) in verified.stdout
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
    assert "readiness report cryptographic_evidence contains unknown domain: 999" in (
        verified.stdout
    )
    assert "readiness report cryptographic_evidence missing required domain: 1" in (
        verified.stdout
    )
    assert (
        "readiness report cryptographic evidence row 0 chain must match lane chain"
        in verified.stdout
    )
    assert (
        "readiness report cryptographic_evidence does not match embedded lane evidence"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_crypto_evidence_inventory_drift(
    tmp_path: Path,
) -> None:
    """The public crypto table must cover each production domain exactly once."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    eth_row = report["cryptographic_evidence"][0]
    assert eth_row["domain"] == 1
    eth_row["domain"] = 2
    eth_row["chain"] = "eth"
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
    assert "readiness report cryptographic_evidence contains duplicate domain: 2" in (
        verified.stdout
    )
    assert (
        "readiness report cryptographic_evidence chain mismatch for domain 2: "
        "expected bsc, got 'eth'"
    ) in verified.stdout
    assert "readiness report cryptographic_evidence missing required domain: 1" in (
        verified.stdout
    )
    assert (
        "readiness report cryptographic_evidence does not match embedded lane evidence"
        in verified.stdout
    )


def test_release_bundle_verifier_rejects_crypto_evidence_domain_policy_drift(
    tmp_path: Path,
) -> None:
    """Public crypto rows must obey route-canary and source-gate domain policy."""

    forged_hash = "0x" + "12" * 32

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    active_row = next(
        row for row in report["cryptographic_evidence"] if row["domain"] == active_domain
    )
    active_row["route_canary_evidence_source"] = "operator_review_note"
    active_row["source_adapter_gate_required"] = True
    active_row["source_adapter_gate_hash"] = forged_hash
    active_row["source_adapter_gate_audit_hashes"] = {"operator_override": forged_hash}
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
        "readiness report cryptographic evidence row "
        "route_canary_evidence_source must be evm_message_proof_accepted_transaction"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_required must be false for this domain"
    ) in verified.stdout
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
    domain_row = next(row for row in report["cryptographic_evidence"] if row["domain"] == 3)
    domain_row["domain"] = "3"
    domain_row["chain"] = 3
    domain_row["source_adapter_gate_audit_hashes"] = ["audit"]
    active_domain = load_report_module().ACTIVE_LAUNCH_DOMAIN
    row = next(
        row for row in report["cryptographic_evidence"] if row["domain"] == active_domain
    )
    row["source_verifier_material_hash"] = "0X" + "aa" * 32
    row["source_adapter_engine_deployment_hash"] = "0x" + "bb" * 31
    row["destination_binding_hash"] = True
    row["route_allowlist_hash"] = "0x" + "cc" * 33
    row["route_canary_evidence_hash"] = "0x" + "gg" * 32
    row["route_canary_evidence_source"] = False
    row["route_canary_evidence_bound"] = "true"
    row["route_canary_block_number"] = 1
    row["route_canary_block_timestamp"] = 2
    row["source_adapter_gate_required"] = "true"
    row["source_adapter_gate_hash"] = "0X" + "dd" * 32
    row["source_adapter_gate_audit_hashes"] = {"": "0x" + "ee" * 32}
    row["source_adapter_gate_audit_hashes"]["operator_override"] = "0X" + "ff" * 32
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
    assert (
        "readiness report cryptographic evidence row "
        "route_canary_block_number must be null for non-TRON lanes"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "route_canary_block_timestamp must be null for non-TRON lanes"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_required must be a boolean"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_hash must be empty or a non-zero canonical "
        "bytes32 hex string"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_audit_hashes must be an object"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_audit_hashes contains an empty key"
    ) in verified.stdout
    assert (
        "readiness report cryptographic evidence row "
        "source_adapter_gate_audit_hashes operator_override must be a canonical "
        "bytes32 hex string"
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
    row["sdk_helper_symbols"] = ["buildEvmSccpProofRequest", ""]
    row["sdk_helper_symbols_by_sdk"] = {
        "js-sdk": ["buildEvmSccpProofRequest", ""],
        "swift-sdk": "buildEvmSccpProofRequest",
    }
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
        "readiness report user prover submission surface row "
        "sdk_helper_symbols must be a list of non-empty strings"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk[js-sdk] must be a list of non-empty strings"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk[swift-sdk] must be a list of non-empty strings"
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


def test_release_bundle_verifier_rejects_submission_surface_helper_symbol_drift(
    tmp_path: Path,
) -> None:
    """Portal/mobile SDK helper strings must be backed by structured symbols."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["sdk_helper_symbols"] = list(row["sdk_helper_symbols"][:-1])
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
        "readiness report user prover submission surface row "
        "sdk_helpers must match sdk_helper_symbols"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk[js-sdk] must match sdk_helper_symbols"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_per_sdk_helper_symbol_drift(
    tmp_path: Path,
) -> None:
    """Portal/mobile SDK helper maps must stay derived from the readiness report."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["sdk_helper_symbols_by_sdk"]["python-sdk"] = list(
        row["sdk_helper_symbols_by_sdk"]["python-sdk"][:-1]
    )
    row["sdk_helper_symbols_by_sdk"]["unknown-sdk"] = ["forgedUiProver"]
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
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk contains unknown SDK: unknown-sdk"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_sdk_phase_inventory_matches_report() -> None:
    """Verifier-owned SDK inventory must mirror release-readiness rows."""

    verifier = load_verify_helpers()
    report = load_report_module()

    assert verifier.USER_PROVER_SDK_PHASES == report.USER_PROVER_SDK_PHASES


def test_release_bundle_verifier_sdk_phase_inventory_is_independent(
) -> None:
    """A weakened report module must not relax per-SDK helper-map checks."""

    verifier = load_verify_helpers()
    assert not hasattr(verifier, "_report_module")
    row = {
        "lanes": "eth,bsc",
        "proof_backend": "evm-groth16-bn254-v1",
        "sdk_helper_symbols": [
            "buildEvmSccpProofRequest",
            "witnessProvider",
            "proveFn",
        ],
        "sdk_helper_symbols_by_sdk": {
            "js-sdk": [
                "buildEvmSccpProofRequest",
                "witnessProvider",
                "proveFn",
            ],
        },
        "sdk_helpers": "buildEvmSccpProofRequest, witnessProvider, proveFn",
        "on_chain_submission": "Torii bridge-proof submit payload",
        "required_phases": [
            "js-sdk",
            "python-sdk",
            "swift-sdk",
            "kotlin-sdk",
            "java-android",
            "contract-smoke",
            "core-admission",
        ],
        "validation_status": "passed",
        "validation_blockers": [],
    }

    errors = verifier._submission_surface_row_schema_errors(row)

    for sdk in ("python-sdk", "swift-sdk", "kotlin-sdk", "java-android"):
        assert (
            "readiness report user prover submission surface row "
            f"sdk_helper_symbols_by_sdk[{sdk}] must be a list of non-empty strings"
        ) in errors


def test_release_bundle_verifier_rejects_duplicate_submission_surface_helpers(
    tmp_path: Path,
) -> None:
    """Verified user-prover rows must not count duplicate helpers as coverage."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["sdk_helper_symbols"] = [
        *row["sdk_helper_symbols"],
        row["sdk_helper_symbols"][0],
    ]
    row["sdk_helpers"] = ", ".join(row["sdk_helper_symbols"])
    row["sdk_helper_symbols_by_sdk"]["js-sdk"] = list(row["sdk_helper_symbols"])
    row["sdk_helper_symbols_by_sdk"]["python-sdk"] = [
        *row["sdk_helper_symbols_by_sdk"]["python-sdk"],
        row["sdk_helper_symbols_by_sdk"]["python-sdk"][0],
    ]
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
        "readiness report user prover submission surface row "
        "sdk_helper_symbols contains duplicate symbols"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk[js-sdk] contains duplicate symbols"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "sdk_helper_symbols_by_sdk[python-sdk] contains duplicate symbols"
    ) in verified.stdout


def test_release_bundle_verifier_requires_submission_surface_sdk_core_phases(
    tmp_path: Path,
) -> None:
    """Verified user-prover rows must remain gated by every SDK and core phase."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["required_phases"] = [
        phase
        for phase in row["required_phases"]
        if phase not in {"python-sdk", "core-admission"}
    ]
    row["required_phases"].extend(["js-sdk", "portal-review"])
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
        "readiness report user prover submission surface row "
        "required_phases contains duplicate phases"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "required_phases contains unknown phase: portal-review"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "required_phases missing required phase: python-sdk"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "required_phases missing required phase: core-admission"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_requires_contract_smoke_for_contract_backends(
    tmp_path: Path,
) -> None:
    """EVM/TRON submission rows must stay gated by contract smoke evidence."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    assert row["proof_backend"] == "evm-groth16-bn254-v1"
    row["required_phases"] = [
        phase for phase in row["required_phases"] if phase != "contract-smoke"
    ]
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
        "readiness report user prover submission surface row "
        "required_phases missing required phase: contract-smoke"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_duplicate_lanes(
    tmp_path: Path,
) -> None:
    """Published user-prover rows must cover each production lane once."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["lanes"] = "sol"
    row["proof_backend"] = "sccp-solana-recursive-mainnet-v1"
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
        "readiness report user_prover_submission_surfaces contains duplicate "
        "lanes row: sol"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces missing required "
        "lanes row: eth,bsc"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_unknown_lanes(
    tmp_path: Path,
) -> None:
    """Published user-prover rows must not advertise unknown production lanes."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["lanes"] = "unknown-mainnet"
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
        "readiness report user_prover_submission_surfaces contains unknown "
        "lanes row: unknown-mainnet"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces missing required "
        "lanes row: eth,bsc"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_submission_surface_backend_mismatch(
    tmp_path: Path,
) -> None:
    """Published user-prover lane rows must keep their production backend ids."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = next(
        row
        for row in report["user_prover_submission_surfaces"]
        if row["lanes"] == "ton"
    )
    row["proof_backend"] = "sccp-solana-recursive-mainnet-v1"
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
        "readiness report user_prover_submission_surfaces proof_backend "
        "mismatch for lanes ton: expected ton-contract-v1, got "
        "'sccp-solana-recursive-mainnet-v1'"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_required_helper_inventory_matches_report() -> None:
    """Verifier-owned user-prover rows must match the generated rows exactly."""

    verifier = load_verify_helpers()
    report = load_report_module()
    phase_status = {phase: "passed" for phase in PHASES}

    assert verifier._expected_submission_surfaces(
        {"corridor": {"phases": phase_status}}
    ) == report._submission_surfaces(phase_status)


def test_release_bundle_verifier_expected_submission_surfaces_are_independent(
) -> None:
    """A weakened report module must not define expected user-prover rows."""

    verifier = load_verify_helpers()
    phase_status = {phase: "passed" for phase in PHASES}
    assert not hasattr(verifier, "_report_module")

    surfaces = verifier._expected_submission_surfaces(
        {"corridor": {"phases": phase_status}}
    )
    ton_surface = next(surface for surface in surfaces if surface["lanes"] == "ton")

    assert ton_surface["validation_status"] == "passed"
    assert ton_surface["validation_blockers"] == []
    assert (
        "buildTonSccpValidatorSetTransitionProofRequest"
        in ton_surface["sdk_helper_symbols_by_sdk"]["js-sdk"]
    )


def test_release_bundle_verifier_helper_inventory_is_independent() -> None:
    """Verifier-owned helper inventory must reject weakened copied rows."""

    verifier = load_verify_helpers()
    report = load_report_module()
    phase_status = {phase: "passed" for phase in PHASES}
    surfaces = report._submission_surfaces(phase_status)
    evm_surface = next(surface for surface in surfaces if surface["lanes"] == "eth,bsc")
    sol_surface = next(surface for surface in surfaces if surface["lanes"] == "sol")
    missing_swift_receipt_proof_helper = "EthereumMainnetReceiptProof"
    missing_swift_outbound_helper = "EthereumMainnetSccp.buildEthereumCalldata"
    missing_kotlin_receipt_proof_helper = "EthereumMainnetReceiptProof"
    missing_kotlin_outbound_helper = "EthereumMainnetSccp.proveOutboundToEthereum"
    missing_java_receipt_proof_helper = "EthereumMainnetSccp.ReceiptProof"
    missing_java_outbound_helper = "EthereumMainnetSccp.buildOutboundProofRequest"
    missing_dotnet_receipt_proof_helper = "EthereumMainnetReceiptProof"
    missing_dotnet_helper = "IEthereumMainnetOutboundProver"
    missing_bsc_dotnet_helper = "IBscMainnetOutboundSubmitter"
    missing_evm_java_helper = "BscMainnetSccp.submitOutboundToBsc"
    missing_js_helper = "buildSolanaSccpFullLightClientAuditProofRequests"
    missing_java_helper = "SolanaSccpProver.FullLightClientAuditProofEngine"
    evm_surface["sdk_helper_symbols_by_sdk"]["swift-sdk"] = [
        helper
        for helper in evm_surface["sdk_helper_symbols_by_sdk"]["swift-sdk"]
        if helper
        not in {
            missing_swift_receipt_proof_helper,
            missing_swift_outbound_helper,
        }
    ]
    evm_surface["sdk_helper_symbols_by_sdk"]["kotlin-sdk"] = [
        helper
        for helper in evm_surface["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
        if helper
        not in {
            missing_kotlin_receipt_proof_helper,
            missing_kotlin_outbound_helper,
        }
    ]
    evm_surface["sdk_helper_symbols_by_sdk"]["dotnet-sdk"] = [
        helper
        for helper in evm_surface["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
        if helper not in {
            missing_dotnet_helper,
            missing_bsc_dotnet_helper,
            missing_dotnet_receipt_proof_helper,
        }
    ]
    evm_surface["sdk_helper_symbols_by_sdk"]["java-android"] = [
        helper
        for helper in evm_surface["sdk_helper_symbols_by_sdk"]["java-android"]
        if helper
        not in {
            missing_evm_java_helper,
            missing_java_receipt_proof_helper,
            missing_java_outbound_helper,
        }
    ]
    sol_surface["sdk_helper_symbols"] = [
        helper
        for helper in sol_surface["sdk_helper_symbols"]
        if helper != missing_js_helper
    ]
    sol_surface["sdk_helpers"] = ", ".join(sol_surface["sdk_helper_symbols"])
    sol_surface["sdk_helper_symbols_by_sdk"]["js-sdk"] = [
        helper
        for helper in sol_surface["sdk_helper_symbols_by_sdk"]["js-sdk"]
        if helper != missing_js_helper
    ]
    sol_surface["sdk_helper_symbols_by_sdk"]["java-android"] = [
        helper
        for helper in sol_surface["sdk_helper_symbols_by_sdk"]["java-android"]
        if helper != missing_java_helper
    ]
    errors = verifier._submission_surface_inventory_errors(surfaces)

    assert (
        "readiness report user_prover_submission_surfaces lanes sol "
        "sdk_helper_symbols missing required helper: "
        f"{missing_js_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes sol "
        "sdk_helper_symbols_by_sdk[js-sdk] missing required helper: "
        f"{missing_js_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes sol "
        "sdk_helper_symbols_by_sdk[java-android] missing required helper: "
        f"{missing_java_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[swift-sdk] missing required helper: "
        f"{missing_swift_receipt_proof_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[swift-sdk] missing required helper: "
        f"{missing_swift_outbound_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[kotlin-sdk] missing required helper: "
        f"{missing_kotlin_receipt_proof_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[kotlin-sdk] missing required helper: "
        f"{missing_kotlin_outbound_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[java-android] missing required helper: "
        f"{missing_java_receipt_proof_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[java-android] missing required helper: "
        f"{missing_java_outbound_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[java-android] missing required helper: "
        f"{missing_evm_java_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing required helper: "
        f"{missing_dotnet_receipt_proof_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing required helper: "
        f"{missing_dotnet_helper}"
    ) in errors
    assert (
        "readiness report user_prover_submission_surfaces lanes eth,bsc "
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing required helper: "
        f"{missing_bsc_dotnet_helper}"
    ) in errors


def test_release_bundle_verifier_rejects_missing_required_submission_surface_helper(
    tmp_path: Path,
) -> None:
    """Public bundles must retain lane-critical portal/mobile prover helpers."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = next(
        row
        for row in report["user_prover_submission_surfaces"]
        if row["lanes"] == "ton"
    )
    missing_helper = "buildTonSccpFullLightClientAuditProofRequests"
    row["sdk_helper_symbols"] = [
        helper for helper in row["sdk_helper_symbols"] if helper != missing_helper
    ]
    row["sdk_helpers"] = ", ".join(row["sdk_helper_symbols"])
    row["sdk_helper_symbols_by_sdk"]["js-sdk"] = [
        helper
        for helper in row["sdk_helper_symbols_by_sdk"]["js-sdk"]
        if helper != missing_helper
    ]
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
        "readiness report user_prover_submission_surfaces lanes ton "
        "sdk_helper_symbols missing required helper: "
        f"{missing_helper}"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces lanes ton "
        "sdk_helper_symbols_by_sdk[js-sdk] missing required helper: "
        f"{missing_helper}"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
    ) in verified.stdout


def test_release_bundle_verifier_requires_submission_surface_ui_hooks(
    tmp_path: Path,
) -> None:
    """Verified user-prover rows must keep explicit UI witness/prover hooks."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["sdk_helper_symbols"] = [
        "portalWitnessResolver" if symbol == "witnessProvider" else symbol
        for symbol in row["sdk_helper_symbols"]
    ]
    row["sdk_helpers"] = ", ".join(row["sdk_helper_symbols"])
    row["sdk_helper_symbols_by_sdk"]["js-sdk"] = list(row["sdk_helper_symbols"])
    row["sdk_helper_symbols_by_sdk"]["python-sdk"] = [
        "portal_proof_callback" if symbol == "prove" else symbol
        for symbol in row["sdk_helper_symbols_by_sdk"]["python-sdk"]
    ]
    row["sdk_helper_symbols_by_sdk"]["swift-sdk"] = [
        "EvmSccpWitnessResolver"
        if "WitnessProvider" in symbol
        else symbol
        for symbol in row["sdk_helper_symbols_by_sdk"]["swift-sdk"]
    ]
    row["sdk_helper_symbols_by_sdk"]["kotlin-sdk"] = [
        "EvmSccpProofCallback"
        if "ProofEngine" in symbol
        else symbol
        for symbol in row["sdk_helper_symbols_by_sdk"]["kotlin-sdk"]
    ]
    row["sdk_helper_symbols_by_sdk"]["java-android"] = [
        "EvmSccpProver.WitnessResolver"
        if "WitnessProvider" in symbol
        else symbol
        for symbol in row["sdk_helper_symbols_by_sdk"]["java-android"]
    ]
    row["sdk_helper_symbols_by_sdk"]["dotnet-sdk"] = [
        "IEthereumMainnetProofCallback"
        if "InboundProver" in symbol
        else "IEthereumMainnetSubmitCallback"
        if "InboundSubmitter" in symbol
        else "IEthereumMainnetProofCallback"
        if "OutboundProver" in symbol
        else "IEthereumMainnetSubmitCallback"
        if "OutboundSubmitter" in symbol
        else symbol
        for symbol in row["sdk_helper_symbols_by_sdk"]["dotnet-sdk"]
    ]
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
    for expected in (
        "sdk_helper_symbols_by_sdk[js-sdk] missing UI-owned hook marker: "
        "witnessProvider",
        "sdk_helper_symbols_by_sdk[python-sdk] missing UI-owned hook marker: prove",
        "sdk_helper_symbols_by_sdk[swift-sdk] missing UI-owned hook marker: "
        "WitnessProvider",
        "sdk_helper_symbols_by_sdk[kotlin-sdk] missing UI-owned hook marker: "
        "ProofEngine",
        "sdk_helper_symbols_by_sdk[java-android] missing UI-owned hook marker: "
        "WitnessProvider",
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing UI-owned hook marker: "
        "InboundProver",
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing UI-owned hook marker: "
        "InboundSubmitter",
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing UI-owned hook marker: "
        "OutboundProver",
        "sdk_helper_symbols_by_sdk[dotnet-sdk] missing UI-owned hook marker: "
        "OutboundSubmitter",
    ):
        assert expected in verified.stdout


def test_release_bundle_verifier_rejects_blocked_submission_surface(
    tmp_path: Path,
) -> None:
    """Portal/mobile submission rows in a release bundle must be validated."""

    output_dir = build_ready_bundle(tmp_path)
    report_path = output_dir / "sccp-release-readiness.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    row = report["user_prover_submission_surfaces"][0]
    row["validation_status"] = "blocked"
    row["validation_blockers"] = ["phase evidence missing"]
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
        "readiness report user prover submission surface row "
        "validation_status must be passed"
    ) in verified.stdout
    assert (
        "readiness report user prover submission surface row "
        "validation_blockers must be empty"
    ) in verified.stdout
    assert (
        "readiness report user_prover_submission_surfaces does not match "
        "corridor phases"
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


def test_release_bundle_verifier_rejects_phase_log_without_expected_command(
    tmp_path: Path,
) -> None:
    """Published phase evidence must show the command for the claimed phase."""

    output_dir = build_ready_bundle(tmp_path)
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "==> SCCP production corridor: contract-smoke\n"
        "phase contract-smoke passed\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )
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
        "expected phase-block command: node --check "
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_output_only_phase_command_fragment(
    tmp_path: Path,
) -> None:
    """Published phase command fragments must come from traced command lines."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke",
                *report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"],
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["contract-smoke"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
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
        "expected phase-block command: node --check "
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js"
    ) in verified.stdout


def test_release_bundle_verifier_phase_transcript_inventory_matches_report() -> None:
    """Verifier-owned public phase transcript inventory must mirror the report."""

    verifier = load_verify_helpers()
    report = load_report_module()

    assert verifier.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS == (
        report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS
    )
    assert verifier.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS == (
        report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS
    )


def test_release_bundle_verifier_phase_transcript_inventory_is_independent(
    tmp_path: Path,
) -> None:
    """A weakened report module must not relax release-bundle log checks."""

    verifier = load_verify_helpers()
    assert not hasattr(verifier, "_report_module")
    required_export_test = "javascript/iroha_js/test/sccpPackageExports.test.js"
    weak_required_fragments = tuple(
        fragment
        for fragment in verifier.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != required_export_test
    )
    phase_log = tmp_path / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(weak_required_fragments),
                *verifier.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )

    errors = verifier._phase_transcript_errors(
        tmp_path,
        "js-sdk",
        {"path": "js-sdk.log"},
    )

    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {required_export_test}"
    ) in errors


def test_release_bundle_verifier_requires_js_package_export_transcript(
    tmp_path: Path,
) -> None:
    """Published JS phase evidence must prove package-root SCCP exports were tested."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    required_export_test = "javascript/iroha_js/test/sccpPackageExports.test.js"
    assert required_export_test in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
    required_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        if fragment != required_export_test
    ]
    phase_log = output_dir / "corridor" / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(required_fragments),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
    rewrite_report_phase_artifact(output_dir, "js-sdk")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block command: {required_export_test}"
    ) in verified.stdout


def test_release_bundle_verifier_requires_bsc_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """Published JS evidence must prove the browser BSC path stayed native JS."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    bsc_no_wasm_marker = (
        "browser BSC mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert bsc_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != bsc_no_wasm_marker
    ]
    phase_log = output_dir / "corridor" / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
    rewrite_report_phase_artifact(output_dir, "js-sdk")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {bsc_no_wasm_marker}"
    ) in verified.stdout


def test_release_bundle_verifier_requires_ethereum_browser_no_wasm_marker(
    tmp_path: Path,
) -> None:
    """Published JS evidence must prove the browser Ethereum path stayed native JS."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    ethereum_no_wasm_marker = (
        "browser Ethereum mainnet SCCP artifacts stay JS-only and local-prover owned"
    )
    assert ethereum_no_wasm_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != ethereum_no_wasm_marker
    ]
    phase_log = output_dir / "corridor" / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
    rewrite_report_phase_artifact(output_dir, "js-sdk")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {ethereum_no_wasm_marker}"
    ) in verified.stdout


def test_release_bundle_verifier_requires_bsc_parlia_declaration_marker(
    tmp_path: Path,
) -> None:
    """Published JS evidence must prove the BSC Parlia declarations were tested."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    declaration_marker = (
        "package declarations expose BSC mainnet Parlia finality evidence hooks"
    )
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    phase_log = output_dir / "corridor" / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
    rewrite_report_phase_artifact(output_dir, "js-sdk")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {declaration_marker}"
    ) in verified.stdout


def test_release_bundle_verifier_requires_ethereum_facade_declaration_marker(
    tmp_path: Path,
) -> None:
    """Published JS evidence must prove the Ethereum facade declarations were tested."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    declaration_marker = "package declarations expose Ethereum mainnet SCCP facade methods"
    assert declaration_marker in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
    success_fragments = [
        fragment
        for fragment in report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"]
        if fragment != declaration_marker
    ]
    phase_log = output_dir / "corridor" / "js-sdk.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: js-sdk",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
                ),
                *success_fragments,
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
    rewrite_report_phase_artifact(output_dir, "js-sdk")

    verified = subprocess.run(
        ["python3", str(VERIFY_SCRIPT), str(output_dir)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert verified.returncode == 1
    assert (
        "readiness report phase js-sdk evidence artifact is missing "
        f"expected phase-block success marker: {declaration_marker}"
    ) in verified.stdout


def test_release_bundle_verifier_requires_js_mainnet_facade_transcripts(
    tmp_path: Path,
) -> None:
    """Published JS phase evidence must prove ETH/BSC facade tests were run."""

    required_facade_tests = (
        "javascript/iroha_js/test/sccpEthereumMainnet.test.js",
        "javascript/iroha_js/test/sccpBscMainnet.test.js",
    )
    report = load_report_module()
    for required_facade_test in required_facade_tests:
        case_dir = tmp_path / Path(required_facade_test).stem
        case_dir.mkdir()
        output_dir = build_ready_bundle(case_dir)
        assert (
            required_facade_test
            in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
        )
        required_fragments = [
            fragment
            for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["js-sdk"]
            if fragment != required_facade_test
        ]
        phase_log = output_dir / "corridor" / "js-sdk.log"
        phase_log.write_text(
            "\n".join(
                (
                    "==> SCCP production corridor: js-sdk",
                    *phase_command_lines(required_fragments),
                    *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["js-sdk"],
                    "SCCP production corridor completed.",
                    "",
                )
            ),
            encoding="utf-8",
        )
        rewrite_report_phase_artifact(output_dir, "js-sdk")

        verified = subprocess.run(
            ["python3", str(VERIFY_SCRIPT), str(output_dir)],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        assert verified.returncode == 1
        assert (
            "readiness report phase js-sdk evidence artifact is missing "
            f"expected phase-block command: {required_facade_test}"
        ) in verified.stdout


def test_release_bundle_verifier_rejects_phase_command_outside_claimed_block(
    tmp_path: Path,
) -> None:
    """Published phase evidence must bind commands to their phase block."""

    output_dir = build_ready_bundle(tmp_path)
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "==> SCCP production corridor: contract-smoke\n"
        "phase contract-smoke passed\n"
        "==> SCCP production corridor: core-admission\n"
        "+ node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js\n"
        "+ bash scripts/sccp_evm_contract_smoke.sh\n"
        "SCCP production corridor completed.\n",
        encoding="utf-8",
    )
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
        "expected phase-block command: node --check "
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_phase_log_without_success_marker(
    tmp_path: Path,
) -> None:
    """Published phase evidence must show a phase-local success marker."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
                ),
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
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
        "expected phase-block success marker: sccp_message_bridge_smoke: ok"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_prefix_alias_phase_marker(
    tmp_path: Path,
) -> None:
    """Published phase evidence must use the exact claimed phase marker."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke-forged",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["contract-smoke"],
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
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


def test_release_bundle_verifier_rejects_completion_outside_claimed_phase_block(
    tmp_path: Path,
) -> None:
    """Published phase evidence must show completion in the claimed phase block."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
                ),
                *report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS["contract-smoke"],
                "==> SCCP production corridor: core-admission",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
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
        "the phase-block completion sentinel"
    ) in verified.stdout


def test_release_bundle_verifier_rejects_command_line_only_success_marker(
    tmp_path: Path,
) -> None:
    """Published phase evidence must show success outside traced commands."""

    output_dir = build_ready_bundle(tmp_path)
    report = load_report_module()
    phase_log = output_dir / "corridor" / "contract-smoke.log"
    phase_log.write_text(
        "\n".join(
            (
                "==> SCCP production corridor: contract-smoke",
                *phase_command_lines(
                    report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["contract-smoke"]
                ),
                "+ echo sccp_message_bridge_smoke: ok",
                "SCCP production corridor completed.",
                "",
            )
        ),
        encoding="utf-8",
    )
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
        "expected phase-block success marker: sccp_message_bridge_smoke: ok"
    ) in verified.stdout
