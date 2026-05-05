#!/usr/bin/env python3
"""
Orchestrate the dual-track Iroha release pipeline.

This helper generates release notes via git-cliff, invokes the existing bundle
and image builders for the iroha2/iroha3 profiles, aggregates checksums,
produces the canonical manifest, and optionally stages publication commands
for release targets (SoraFS/SoraNet gateways or other URIs).

Example:
    ./scripts/run_release_pipeline.py \\
        --version <release-version> \\
        --previous-tag <previous-release-tag> \\
        --output-dir artifacts/releases/<release-version> \\
        --signing-key ~/.keys/iroha_signing.pem \\
        --publish-target iroha2=sorafs://releases/iroha2/v<release-version> \\
        --publish-target iroha3=sorafs://releases/iroha3/v<release-version>
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Dict, Iterable, List, Tuple
from urllib import error as urllib_error
from urllib import request as urllib_request

from publish_plan import build_publish_plan, parse_target_map, validate_publish_plan, write_plan_files
REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_PROFILES = ("iroha2=single", "iroha3=nexus")


class PipelineError(RuntimeError):
    """Fatal error raised during the release pipeline."""


def ensure_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise PipelineError(f"Required tool '{name}' not found in PATH")


def run(cmd: List[str], *, cwd: Path | None = None, env: Dict[str, str] | None = None) -> None:
    display = " ".join(cmd)
    print(f"[release-pipeline] $ {display}", flush=True)
    subprocess.run(cmd, check=True, cwd=cwd or REPO_ROOT, env=env)


def repo_version() -> str:
    cargo_toml = REPO_ROOT / "Cargo.toml"
    for line in cargo_toml.read_text(encoding="utf-8").splitlines():
        if line.strip().startswith("version"):
            return line.split('"')[1]
    raise PipelineError("Unable to read version from Cargo.toml")


def parse_profiles(specs: Iterable[str]) -> List[Tuple[str, str]]:
    pairs: List[Tuple[str, str]] = []
    for spec in specs:
        if "=" not in spec:
            raise PipelineError(f"Invalid profile spec '{spec}'. Expected format <name>=<config>.")
        name, config = spec.split("=", 1)
        name = name.strip()
        config = config.strip()
        if not name or not config:
            raise PipelineError(f"Invalid profile spec '{spec}'.")
        pairs.append((name, config))
    return pairs


def detect_os_tag() -> str:
    sysname = platform.system().lower()
    if sysname.startswith("darwin"):
        return "mac"
    if sysname.startswith(("cygwin", "mingw", "msys", "windows")):
        return "win"
    return "linux"


def collect_sha256(artifact_dir: Path) -> Dict[str, str]:
    hashes: Dict[str, str] = {}
    for sha_file in sorted(artifact_dir.glob("*.sha256")):
        content = sha_file.read_text(encoding="utf-8").strip()
        if not content:
            continue
        parts = content.split()
        if len(parts) < 2:
            raise PipelineError(f"Malformed sha256 file: {sha_file}")
        sha = parts[0]
        filename = parts[-1]
        # sha256sum writes "./file", strip leading "./"
        filename = filename.lstrip("./")
        hashes[filename] = sha
    return hashes


def write_sha256sums(hashes: Dict[str, str], output: Path) -> None:
    with output.open("w", encoding="utf-8") as fh:
        for filename, sha in sorted(hashes.items()):
            fh.write(f"{sha}  {filename}\n")


def copytree_clean(src: Path, dest: Path) -> None:
    if not src.exists():
        return
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src, dest)


def archive_android_release_artifacts(version: str, release_root: Path, dry_run: bool) -> None:
    archive_root = REPO_ROOT / "artifacts" / "android" / "releases" / version
    if dry_run:
        print(f"[release-pipeline] (dry-run) archive Android checklist artifacts -> {archive_root}")
        return

    archive_root.mkdir(parents=True, exist_ok=True)

    lint_src = REPO_ROOT / "artifacts" / "android" / "lint" / "latest"
    copytree_clean(lint_src, archive_root / "lint")

    telemetry_src = REPO_ROOT / "artifacts" / "android" / "telemetry" / "latest"
    copytree_clean(telemetry_src, archive_root / "telemetry")

    attestation_src = REPO_ROOT / "artifacts" / "android" / "attestation" / "latest"
    copytree_clean(attestation_src, archive_root / "attestation")

    docs_to_copy = [
        (REPO_ROOT / "docs" / "source" / "android_release_checklist.md", archive_root / "android_release_checklist.md"),
        (REPO_ROOT / "docs" / "source" / "compliance" / "android" / "evidence_log.csv", archive_root / "evidence_log.csv"),
    ]
    for src, dest in docs_to_copy:
        if src.exists():
            shutil.copy2(src, dest)

    summary_src = release_root / "android" / "maven" / "publish_summary.json"
    if summary_src.exists():
        shutil.copy2(summary_src, archive_root / "publish_summary.json")
    checksum_src = release_root / "android" / "maven" / "checksums.txt"
    if checksum_src.exists():
        shutil.copy2(checksum_src, archive_root / "checksums.txt")


def export_fastpq_dashboard(grafana_url: str, token: str, destination: Path) -> None:
    api_url = grafana_url.rstrip("/") + "/api/dashboards/uid/fastpq-acceleration"
    req = urllib_request.Request(
        api_url,
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/json",
        },
    )
    try:
        with urllib_request.urlopen(req) as resp:
            payload = json.load(resp)
    except urllib_error.HTTPError as exc:
        raise PipelineError(f"Grafana export failed ({exc.code}): {exc.reason}") from exc
    except urllib_error.URLError as exc:
        raise PipelineError(f"Grafana export failed: {exc.reason}") from exc
    except json.JSONDecodeError as exc:
        raise PipelineError(f"Grafana response was not valid JSON: {exc}") from exc

    dashboard = payload.get("dashboard", payload)
    destination.write_text(json.dumps(dashboard, indent=2) + "\n", encoding="utf-8")


def summarize_fastpq_rollout_bundle(bundle_dir: Path, *, dry_run: bool) -> List[Dict[str, str]]:
    """Write reviewer-facing FASTPQ rollout summaries for each archived manifest."""

    manifests = sorted(bundle_dir.rglob("fastpq_bench_manifest.json"))
    summaries: List[Dict[str, str]] = []
    helper = REPO_ROOT / "scripts" / "fastpq" / "rollout_manifest_summary.py"
    for manifest in manifests:
        json_out = manifest.parent / "fastpq_rollout_summary.json"
        markdown_out = manifest.parent / "fastpq_rollout_summary.md"
        if dry_run:
            print(
                "[release-pipeline] (dry-run) summarize FASTPQ rollout manifest "
                f"{manifest} -> {json_out}, {markdown_out}"
            )
            continue
        run(
            [
                sys.executable,
                str(helper),
                "--manifest",
                str(manifest),
                "--bundle-dir",
                str(manifest.parent),
                "--repo-root",
                str(REPO_ROOT),
                "--json-out",
                str(json_out),
                "--markdown-out",
                str(markdown_out),
            ]
        )
        summary_entry: Dict[str, str] = {}
        try:
            summary_entry["manifest"] = str(manifest.relative_to(REPO_ROOT))
        except ValueError:
            summary_entry["manifest"] = str(manifest)
        try:
            summary_entry["json"] = str(json_out.relative_to(REPO_ROOT))
        except ValueError:
            summary_entry["json"] = str(json_out)
        try:
            summary_entry["markdown"] = str(markdown_out.relative_to(REPO_ROOT))
        except ValueError:
            summary_entry["markdown"] = str(markdown_out)
        summaries.append(summary_entry)
    return summaries


def update_release_manifest_evidence(
    manifest_path: Path,
    *,
    fastpq_grafana_rel: str | None,
    archived_fastpq: List[Dict[str, object]],
    cbdc_validation_rel: str | None,
) -> None:
    """Attach archived rollout evidence paths to the release manifest."""

    with manifest_path.open("r", encoding="utf-8") as fh:
        manifest = json.load(fh)

    evidence = manifest.get("evidence")
    if not isinstance(evidence, dict):
        evidence = {}

    if fastpq_grafana_rel or archived_fastpq:
        fastpq_evidence: Dict[str, object] = {}
        if fastpq_grafana_rel:
            fastpq_evidence["grafana_export"] = fastpq_grafana_rel
        if archived_fastpq:
            fastpq_evidence["rollout_bundles"] = archived_fastpq
        evidence["fastpq"] = fastpq_evidence

    if cbdc_validation_rel:
        evidence["cbdc"] = {"validated_bundle": cbdc_validation_rel}

    if evidence:
        manifest["evidence"] = evidence

    with manifest_path.open("w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2)
        fh.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Target release version (e.g., 2.0.0-rc.3 or v2.0.0-rc.3)")
    parser.add_argument(
        "--previous-tag",
        help="Previous release tag for changelog generation (optional; should match the release line being prepared).",
    )
    parser.add_argument(
        "--output-dir",
        default="artifacts/releases",
        help="Base directory for release artifacts (default: artifacts/releases)",
    )
    parser.add_argument(
        "--profile",
        action="append",
        dest="profiles",
        help="Profile spec in the form name=config (default: iroha2=single, iroha3=nexus). Can be repeated.",
    )
    parser.add_argument("--signing-key", help="PEM-encoded private key used for bundle/image signing.")
    parser.add_argument("--skip-bundles", action="store_true", help="Skip building tar.zst bundles.")
    parser.add_argument("--skip-images", action="store_true", help="Skip building Docker images.")
    parser.add_argument(
        "--skip-privacy-dp",
        action="store_true",
        help="Skip regenerating the SoraNet privacy differential privacy artefacts.",
    )
    parser.add_argument(
        "--skip-nexus-lane-smoke",
        action="store_true",
        help="Skip ci/check_nexus_lane_smoke.sh + NX-18 evidence bundling.",
    )
    parser.add_argument(
        "--skip-nexus-cross-dataspace-proof",
        action="store_true",
        help="Skip ci/check_nexus_cross_dataspace_localnet.sh cross-dataspace atomic swap proof gate.",
    )
    parser.add_argument(
        "--publish-target",
        action="append",
        help=(
            "Publish target URI for publication plan (optional). "
            "Repeat as profile=uri to target specific tracks; a single URI is used for both profiles."
        ),
    )
    parser.add_argument(
        "--previous-publish-plan",
        help="Optional previous publish_plan.json used for diffing during validation.",
    )
    parser.add_argument(
        "--publish-probe-remote",
        action="store_true",
        help="Issue HTTP(S) HEAD requests during publish_plan validation for destinations with http/https scheme.",
    )
    parser.add_argument(
        "--publish-probe-command",
        help="Optional external probe command (use {destination} placeholder) to validate non-HTTP publish targets.",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print commands without executing.")
    parser.add_argument(
        "--export-fastpq-grafana",
        action="store_true",
        help="Capture the fastpq-acceleration Grafana dashboard into the rollout bundle.",
    )
    parser.add_argument(
        "--fastpq-rollout-dir",
        default="artifacts/fastpq_rollouts",
        help="Base directory for FASTPQ rollout artefacts (default: artifacts/fastpq_rollouts).",
    )
    parser.add_argument(
        "--fastpq-rollout-stamp",
        help="Override timestamp folder for FASTPQ rollout artefacts (default combines timestamp + version).",
    )
    parser.add_argument(
        "--grafana-url",
        help="Grafana base URL used when exporting FASTPQ dashboards (required with --export-fastpq-grafana unless GRAFANA_URL is set).",
    )
    parser.add_argument(
        "--grafana-token-env",
        default="GRAFANA_TOKEN",
        help="Environment variable holding the Grafana API token (default: GRAFANA_TOKEN).",
    )
    parser.add_argument(
        "--fastpq-rollout-bundle",
        action="append",
        dest="fastpq_bundles",
        help="Path to a FASTPQ rollout bundle directory to archive alongside the release output (can be repeated).",
    )
    parser.add_argument(
        "--skip-fastpq-rollout-check",
        action="store_true",
        help="Skip validating FASTPQ rollout bundles even if provided.",
    )
    parser.add_argument(
        "--cbdc-rollout-dir",
        default="artifacts/nexus/cbdc_rollouts",
        help="Base directory for CBDC rollout artefacts validated via ci/check_cbdc_rollout.sh (default: artifacts/nexus/cbdc_rollouts).",
    )
    parser.add_argument(
        "--skip-cbdc-rollout-check",
        action="store_true",
        help="Skip validating CBDC rollout bundles even if present.",
    )
    parser.add_argument(
        "--publish-android-sdk",
        action="store_true",
        help="Run scripts/publish_android_sdk.sh to include the Android Maven artifacts in the release bundle.",
    )
    parser.add_argument(
        "--android-sdk-repo-url",
        help="Remote Maven repository URL passed to scripts/publish_android_sdk.sh when publishing the Android SDK.",
    )
    parser.add_argument(
        "--android-sdk-username",
        help="Repository username/token used with --android-sdk-repo-url.",
    )
    parser.add_argument(
        "--android-sdk-password",
        help="Repository password used with --android-sdk-repo-url.",
    )
    parser.add_argument(
        "--android-sdk-skip-sbom",
        action="store_true",
        help="Skip SBOM/provenance generation in scripts/publish_android_sdk.sh (tests still run).",
    )

    args = parser.parse_args()

    provided_version = args.version.lstrip("v")
    repo_ver = repo_version()
    if repo_ver != provided_version:
        raise PipelineError(
            f"Version mismatch: repository declares {repo_ver} but --version {provided_version} was provided."
        )

    profiles = parse_profiles(args.profiles or DEFAULT_PROFILES)
    release_root = Path(args.output_dir).expanduser().resolve() / provided_version
    artifact_dir = release_root / "artifacts"
    artifact_dir.mkdir(parents=True, exist_ok=True)

    ensure_tool("git")
    ensure_tool("git-cliff")
    if not args.skip_images:
        ensure_tool("docker")

    dp_script = REPO_ROOT / "scripts" / "telemetry" / "run_privacy_dp_notebook.sh"
    if not args.skip_privacy_dp:
        if not dp_script.is_file():
            raise PipelineError(f"Privacy DP script missing: {dp_script}")
        dp_cmd = [str(dp_script)]
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) {' '.join(dp_cmd)}")
        else:
            run(dp_cmd)

    if args.skip_nexus_lane_smoke:
        print("[release-pipeline] skipping Nexus lane smoke evidence step")
    else:
        smoke_cmd = ["bash", str(REPO_ROOT / "ci" / "check_nexus_lane_smoke.sh")]
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) {' '.join(smoke_cmd)}")
        else:
            run(smoke_cmd)
        nx_source = REPO_ROOT / "artifacts" / "nx18"
        if not nx_source.exists():
            raise PipelineError("NX-18 evidence directory artifacts/nx18 was not created")
        nx_dest = release_root / "nx18"
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) copy {nx_source} -> {nx_dest}")
        else:
            copytree_clean(nx_source, nx_dest)

    if args.skip_nexus_cross_dataspace_proof:
        print("[release-pipeline] skipping Nexus cross-dataspace proof gate")
    else:
        cross_ds_cmd = ["bash", str(REPO_ROOT / "ci" / "check_nexus_cross_dataspace_localnet.sh")]
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) {' '.join(cross_ds_cmd)}")
        else:
            run(cross_ds_cmd)

    built_at = dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z"
    commit = subprocess.check_output(
        ["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT, text=True
    ).strip()
    os_tag = detect_os_tag()
    arch = platform.machine()

    changelog_path = release_root / f"CHANGELOG-{provided_version}.md"
    cliff_cmd = [
        "git",
        "cliff",
        "-c",
        str(REPO_ROOT / "cliff.toml"),
        "--tag",
        f"v{provided_version}",
        "--output",
        str(changelog_path),
    ]
    if args.previous_tag:
        cliff_cmd.extend(["--previous-tag", args.previous_tag])

    if args.dry_run:
        print(f"[release-pipeline] (dry-run) {' '.join(cliff_cmd)}")
    else:
        run(cliff_cmd)

    bundle_paths: List[Path] = []
    for profile, config in profiles:
        if not args.skip_bundles:
            bundle_cmd = [
                str(REPO_ROOT / "scripts" / "build_release_bundle.sh"),
                "--profile",
                profile,
                "--config",
                config,
                "--artifacts-dir",
                str(artifact_dir),
            ]
            if args.signing_key:
                bundle_cmd.extend(["--signing-key", args.signing_key])
            if args.dry_run:
                print(f"[release-pipeline] (dry-run) {' '.join(bundle_cmd)}")
            else:
                run(bundle_cmd)
            bundle_paths.append(
                artifact_dir / f"{profile}-{provided_version}-{os_tag}.tar.zst"
            )

        if not args.skip_images:
            image_cmd = [
                str(REPO_ROOT / "scripts" / "build_release_image.sh"),
                "--profile",
                profile,
                "--config",
                config,
                "--artifacts-dir",
                str(artifact_dir),
            ]
            if args.signing_key:
                image_cmd.extend(["--signing-key", args.signing_key])
            if args.dry_run:
                print(f"[release-pipeline] (dry-run) {' '.join(image_cmd)}")
            else:
                run(image_cmd)

    if not args.skip_bundles and bundle_paths:
        matrix_output = artifact_dir / "dual_profile_matrix.json"
        matrix_cmd = [
            str(REPO_ROOT / "ci" / "dual_profile_matrix.sh"),
            "--output",
            str(matrix_output),
            "--expect-version",
            provided_version,
            *[str(path) for path in bundle_paths],
        ]
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) {' '.join(matrix_cmd)}")
        else:
            run(matrix_cmd)

    hashes = collect_sha256(artifact_dir)
    sha_path = artifact_dir / "SHA256SUMS"
    write_sha256sums(hashes, sha_path)

    manifest_args = [
        str(REPO_ROOT / "scripts" / "generate_release_manifest.py"),
        "--artifacts-dir",
        str(artifact_dir),
        "--version",
        provided_version,
        "--commit",
        commit,
        "--built-at",
        built_at,
        "--os-tag",
        os_tag,
        "--arch",
        arch,
        "--output",
        str(release_root / "release_manifest.json"),
    ]

    if args.dry_run:
        print(f"[release-pipeline] (dry-run) {' '.join(manifest_args)}")
    else:
        run(manifest_args)

    publish_target_map = None
    if args.publish_target:
        target_map = parse_target_map(args.publish_target)
        publish_target_map = target_map
        if args.dry_run:
            print(
                f"[release-pipeline] (dry-run) publish plan for targets {target_map} would be written to {release_root}"
            )
        else:
            plan = build_publish_plan(
                manifest_path=release_root / "release_manifest.json",
                artifacts_dir=artifact_dir,
                target_map=target_map,
            )
            plan_paths = write_plan_files(plan, release_root)
            report = validate_publish_plan(
                plan_path=plan_paths["json"],
                previous_plan_path=Path(args.previous_publish_plan)
                if args.previous_publish_plan
                else None,
                probe_remote=args.publish_probe_remote,
                probe_command=args.publish_probe_command,
            )
            report_path = release_root / "publish_plan_report.json"
            with report_path.open("w", encoding="utf-8") as fh:
                json.dump(report, fh, indent=2)
                fh.write("\n")
            report_txt = release_root / "publish_plan_report.txt"
            lines = [
                f"status: {report.get('status')}",
                f"local failures: {len(report.get('local_failures', []))}",
                f"remote failures: {len(report.get('remote_failures', []))}",
                f"diff: {report.get('diff') or {}}",
            ]
            report_txt.write_text("\n".join(lines) + "\n", encoding="utf-8")
            if report.get("status") != "ok":
                raise PipelineError("Publish plan validation failed; see publish_plan_report.json")

    if args.publish_android_sdk:
        android_dir = release_root / "android"
        android_repo_dir = android_dir / "maven"
        publish_cmd = [
            str(REPO_ROOT / "scripts" / "publish_android_sdk.sh"),
            "--version",
            provided_version,
            "--repo-dir",
            str(android_repo_dir),
        ]
        if args.android_sdk_repo_url:
            publish_cmd.extend(["--repo-url", args.android_sdk_repo_url])
        if args.android_sdk_username:
            publish_cmd.extend(["--username", args.android_sdk_username])
        if args.android_sdk_password:
            publish_cmd.extend(["--password", args.android_sdk_password])
        if args.android_sdk_skip_sbom:
            publish_cmd.append("--skip-sbom")
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) {' '.join(publish_cmd)}")
        else:
            run(publish_cmd)

        sbom_src = REPO_ROOT / "artifacts" / "android" / "sbom" / provided_version
        sbom_dest = android_dir / "sbom"
        if args.dry_run:
            print(f"[release-pipeline] (dry-run) copy Android SBOM bundle {sbom_src} -> {sbom_dest}")
        elif sbom_src.is_dir():
            if sbom_dest.exists():
                shutil.rmtree(sbom_dest)
            shutil.copytree(sbom_src, sbom_dest)
        else:
            print(f"[release-pipeline] warning: Android SBOM bundle not found at {sbom_src}")

        readme_path = android_dir / "README.txt"
        summary_path = android_repo_dir / "publish_summary.json"
        checksum_path = android_repo_dir / "checksums.txt"
        if not args.dry_run:
            android_dir.mkdir(parents=True, exist_ok=True)
            with readme_path.open("w", encoding="utf-8") as fh:
                fh.write("Android SDK release bundle\n")
                fh.write(f"Version: {provided_version}\n")
                fh.write(f"Generated: {built_at}\n")
                fh.write(f"Local Maven repo: {android_repo_dir}\n")
                if args.android_sdk_repo_url:
                    fh.write(f"Remote Maven repo: {args.android_sdk_repo_url}\n")
                if sbom_dest.is_dir():
                    fh.write(f"SBOM bundle: {sbom_dest}\n")
                if summary_path.is_file():
                    fh.write(f"Summary: {summary_path}\n")
                if checksum_path.is_file():
                    fh.write(f"Checksums: {checksum_path}\n")
        else:
            print(f"[release-pipeline] (dry-run) write Android README to {readme_path}")

        archive_android_release_artifacts(provided_version, release_root, args.dry_run)

    fastpq_grafana_rel: str | None = None
    fastpq_rollout_base = Path(args.fastpq_rollout_dir).expanduser().resolve()
    if args.export_fastpq_grafana:
        grafana_url = args.grafana_url or os.environ.get("GRAFANA_URL")
        if not grafana_url:
            raise PipelineError("Grafana URL not provided (use --grafana-url or set GRAFANA_URL).")
        token = os.environ.get(args.grafana_token_env)
        if not token:
            raise PipelineError(
                f"Grafana token not found in environment variable {args.grafana_token_env}."
            )
        stamp = args.fastpq_rollout_stamp or f"{dt.datetime.utcnow():%Y%m%dT%H%MZ}_{provided_version}"
        rollout_dir = fastpq_rollout_base / stamp / "release_pipeline"
        rollout_dir.mkdir(parents=True, exist_ok=True)
        dashboard_path = rollout_dir / "grafana_fastpq_acceleration.json"
        alerts_dst = rollout_dir / "alerts"
        tests_dst = alerts_dst / "tests"
        repo_alert = REPO_ROOT / "dashboards" / "alerts" / "fastpq_acceleration_rules.yml"
        repo_test = (
            REPO_ROOT
            / "dashboards"
            / "alerts"
            / "tests"
            / "fastpq_acceleration_rules.test.yml"
        )
        if args.dry_run:
            print(
                f"[release-pipeline] (dry-run) export Grafana dashboard to {dashboard_path} "
                f"and copy alert pack into {alerts_dst}"
            )
        else:
            export_fastpq_dashboard(grafana_url, token, dashboard_path)
            alerts_dst.mkdir(parents=True, exist_ok=True)
            shutil.copy2(repo_alert, alerts_dst / "fastpq_acceleration_rules.yml")
            tests_dst.mkdir(parents=True, exist_ok=True)
            shutil.copy2(
                repo_test,
                tests_dst / "fastpq_acceleration_rules.test.yml",
            )
        try:
            fastpq_grafana_rel = str(rollout_dir.relative_to(REPO_ROOT))
        except ValueError:
            fastpq_grafana_rel = str(rollout_dir)

    archived_fastpq: List[Dict[str, object]] = []
    if args.fastpq_bundles:
        script = REPO_ROOT / "ci" / "check_fastpq_rollout.sh"
        release_rollout_root = release_root / "fastpq_rollouts"
        release_rollout_root.mkdir(parents=True, exist_ok=True)
        for bundle in args.fastpq_bundles:
            bundle_path = Path(bundle).expanduser().resolve()
            if not bundle_path.exists():
                raise PipelineError(f"FASTPQ rollout bundle not found: {bundle_path}")
            if not args.skip_fastpq_rollout_check and not args.dry_run:
                env = os.environ.copy()
                env["FASTPQ_ROLLOUT_BUNDLE"] = str(bundle_path)
                run([str(script)], env=env)
            try:
                rel_bundle = bundle_path.relative_to(fastpq_rollout_base)
            except ValueError:
                rel_bundle = Path(bundle_path.name)
            dest_dir = release_rollout_root / rel_bundle
            if args.dry_run:
                print(
                    f"[release-pipeline] (dry-run) archive FASTPQ rollout bundle {bundle_path} -> {dest_dir}"
                )
            else:
                if dest_dir.exists():
                    shutil.rmtree(dest_dir)
                dest_dir.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(bundle_path, dest_dir)
            summary_paths = summarize_fastpq_rollout_bundle(dest_dir, dry_run=args.dry_run)
            try:
                bundle_label = str(dest_dir.relative_to(REPO_ROOT))
            except ValueError:
                bundle_label = str(dest_dir)
            archived_fastpq.append(
                {
                    "bundle": bundle_label,
                    "summaries": summary_paths,
                }
            )

    cbdc_validation_rel: str | None = None
    if not args.skip_cbdc_rollout_check:
        cbdc_dir = Path(args.cbdc_rollout_dir).expanduser().resolve()
        manifests = (
            list(cbdc_dir.rglob("cbdc.manifest.json")) if cbdc_dir.exists() else []
        )
        if manifests:
            script = REPO_ROOT / "ci" / "check_cbdc_rollout.sh"
            if args.dry_run:
                print(
                    f"[release-pipeline] (dry-run) CBDC_ROLLOUT_BUNDLE={cbdc_dir} {script}"
                )
            else:
                env = os.environ.copy()
                env["CBDC_ROLLOUT_BUNDLE"] = str(cbdc_dir)
                run([str(script)], env=env)
            try:
                cbdc_validation_rel = str(cbdc_dir.relative_to(REPO_ROOT))
            except ValueError:
                cbdc_validation_rel = str(cbdc_dir)

    if not args.dry_run:
        update_release_manifest_evidence(
            release_root / "release_manifest.json",
            fastpq_grafana_rel=fastpq_grafana_rel,
            archived_fastpq=archived_fastpq,
            cbdc_validation_rel=cbdc_validation_rel,
        )

    summary = release_root / "SUMMARY.txt"
    lines = [
        f"Version: {provided_version}",
        f"Commit: {commit}",
        f"Built at: {built_at}",
        f"Artifacts dir: {artifact_dir}",
        f"Changelog: {changelog_path.name}",
        f"Manifest: release_manifest.json",
        f"Profiles: {', '.join(f'{p}={c}' for p, c in profiles)}",
    ]
    if args.skip_privacy_dp:
        lines.append("Privacy DP notebook: skipped (--skip-privacy-dp)")
    else:
        lines.append("Privacy DP notebook: refreshed")
    if args.skip_nexus_cross_dataspace_proof:
        lines.append("Nexus cross-dataspace proof gate: skipped (--skip-nexus-cross-dataspace-proof)")
    else:
        lines.append("Nexus cross-dataspace proof gate: passed")
    if publish_target_map:
        lines.append("Publish targets:")
        for profile, target in sorted(publish_target_map.items()):
            lines.append(f"  - {profile}: {target}")
        lines.append("Publish plan: publish_plan.{json,sh,txt}")
        lines.append("Publish plan report: publish_plan_report.json")
        lines.append("Publish plan report (text): publish_plan_report.txt")
    if fastpq_grafana_rel:
        lines.append(f"FASTPQ Grafana export: {fastpq_grafana_rel}")
    if archived_fastpq:
        lines.append("FASTPQ rollout bundles archived:")
        for entry in archived_fastpq:
            bundle = entry.get("bundle", "")
            lines.append(f"  - {bundle}")
            for summary_path in entry.get("summaries", []):
                if isinstance(summary_path, dict):
                    markdown_path = summary_path.get("markdown")
                    if markdown_path:
                        lines.append(f"    Summary: {markdown_path}")
    if cbdc_validation_rel:
        lines.append(f"CBDC rollout bundles validated: {cbdc_validation_rel}")
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print("[release-pipeline] Completed. Artifacts staged under", release_root)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except PipelineError as exc:
        print(f"[release-pipeline] error: {exc}", file=sys.stderr)
        sys.exit(1)
    except subprocess.CalledProcessError as exc:
        print(f"[release-pipeline] command failed with exit code {exc.returncode}", file=sys.stderr)
        sys.exit(exc.returncode)
