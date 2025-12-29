#!/usr/bin/env python3
"""
Generate and validate publish plans for Iroha dual-track releases.

Targets are storage-agnostic: pass one or more base URIs (e.g.,
`sorafs://cluster/releases/iroha2/v<ver>` or `https://gateway/sorafs/iroha3/v<ver>`)
and the tool will:
- read the release manifest,
- verify local hash/size for each artifact,
- emit `publish_plan.{json,sh,txt}` with fully qualified destinations.

Validation can optionally probe HTTP(S) destinations via HEAD requests; other
schemes are skipped, keeping the tool compatible with SoraFS/SoraNet gateways
or offline flows.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import shlex
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional
from urllib import request as urllib_request
from urllib.error import HTTPError, URLError


class PublishPlanError(RuntimeError):
    """Raised when plan generation or validation fails."""


@dataclass(frozen=True)
class Artifact:
    profile: str
    kind: str
    format: str
    relative_path: str
    source: Path
    destination: str
    sha256: str
    size: int


def parse_target_map(values: Iterable[str]) -> Dict[str, str]:
    """
    Accepts either a single base URI or per-profile mappings in the form
    `profile=uri`. When only a single URI is supplied it is used for both
    `iroha2` and `iroha3`.
    """
    target_map: Dict[str, str] = {}
    default_target: Optional[str] = None
    for raw in values:
        if raw is None:
            continue
        raw = raw.strip()
        if not raw:
            continue
        if "=" in raw:
            profile, uri = raw.split("=", 1)
            profile = profile.strip()
            if not profile or not uri.strip():
                raise PublishPlanError(f"Invalid target mapping '{raw}'")
            target_map[profile] = uri.strip().rstrip("/")
        else:
            if default_target is not None and default_target != raw:
                raise PublishPlanError("Multiple default targets provided; use profile=uri form")
            default_target = raw.rstrip("/")

    if not target_map and not default_target:
        raise PublishPlanError("At least one publish target URI is required")

    for profile in ("iroha2", "iroha3"):
        if profile not in target_map:
            if default_target is None:
                raise PublishPlanError(f"Missing publish target for profile {profile}")
            target_map[profile] = default_target
    return target_map


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_manifest(path: Path) -> Dict[str, object]:
    with path.open("r", encoding="utf-8") as fh:
        try:
            manifest = json.load(fh)
        except json.JSONDecodeError as exc:
            raise PublishPlanError(f"Release manifest is not valid JSON: {exc}") from exc
    if "artifacts" not in manifest or not isinstance(manifest["artifacts"], list):
        raise PublishPlanError("Release manifest is missing an 'artifacts' array")
    return manifest


def build_publish_plan(
    manifest_path: Path,
    artifacts_dir: Path,
    target_map: Dict[str, str],
) -> Dict[str, object]:
    manifest = _load_manifest(manifest_path)
    artifacts_dir = artifacts_dir.resolve()

    artifacts: List[Artifact] = []
    for entry in manifest["artifacts"]:
        if not isinstance(entry, dict):
            raise PublishPlanError("Release manifest contains a non-object artifact entry")
        profile = entry.get("profile")
        path_value = entry.get("path")
        sha256 = entry.get("sha256")
        kind = entry.get("kind", "")
        fmt = entry.get("format", "")
        if not profile or not path_value or not sha256:
            raise PublishPlanError(f"Artifact entry is missing required fields: {entry}")
        base_uri = target_map.get(profile)
        if not base_uri:
            raise PublishPlanError(f"No publish target configured for profile '{profile}'")
        rel_path = os.path.normpath(str(path_value))
        source_path = artifacts_dir / rel_path
        if not source_path.exists():
            raise PublishPlanError(f"Artifact path does not exist: {source_path}")
        size = source_path.stat().st_size
        computed_sha = _sha256_file(source_path)
        if computed_sha != sha256:
            raise PublishPlanError(
                f"SHA256 mismatch for {source_path}: manifest={sha256} computed={computed_sha}"
            )
        destination = f"{base_uri}/{rel_path}"
        artifacts.append(
            Artifact(
                profile=str(profile),
                kind=str(kind),
                format=str(fmt),
                relative_path=rel_path,
                source=source_path,
                destination=destination,
                sha256=sha256,
                size=size,
            )
        )

    plan = {
        "generated_at": _dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z",
        "manifest": str(manifest_path),
        "artifacts_dir": str(artifacts_dir),
        "target_map": target_map,
        "version": manifest.get("version"),
        "commit": manifest.get("commit"),
        "os": manifest.get("os"),
        "arch": manifest.get("arch"),
        "artifacts": [
            {
                "profile": a.profile,
                "kind": a.kind,
                "format": a.format,
                "relative_path": a.relative_path,
                "source": str(a.source),
                "destination": a.destination,
                "sha256": a.sha256,
                "size": a.size,
            }
            for a in artifacts
        ],
    }
    return plan


def _render_commands(plan: Dict[str, object]) -> List[str]:
    artifacts = plan.get("artifacts", [])
    commands: List[str] = []
    for entry in artifacts:
        if not isinstance(entry, dict):
            continue
        src = entry.get("source")
        dest = entry.get("destination")
        if not src or not dest:
            continue
        # Storage-agnostic: default to curl PUT/POST is not safe; emit a cp-style command for gateways.
        commands.append(f"# upload {src} -> {dest}")
    return commands


def write_plan_files(plan: Dict[str, object], output_dir: Path) -> Dict[str, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "publish_plan.json"
    with json_path.open("w", encoding="utf-8") as fh:
        json.dump(plan, fh, indent=2)
        fh.write("\n")

    commands = _render_commands(plan)
    shell_path = output_dir / "publish_plan.sh"
    shell_body = "#!/usr/bin/env bash\nset -euo pipefail\n\n" + "\n".join(commands) + "\n"
    shell_path.write_text(shell_body, encoding="utf-8")
    shell_path.chmod(0o750)

    txt_path = output_dir / "publish_plan.txt"
    txt_path.write_text("\n".join(commands) + "\n", encoding="utf-8")
    return {"json": json_path, "sh": shell_path, "txt": txt_path}


def _http_head(url: str) -> Optional[int]:
    req = urllib_request.Request(url, method="HEAD")
    try:
        with urllib_request.urlopen(req) as resp:
            length = resp.headers.get("Content-Length")
            return int(length) if length is not None else None
    except (HTTPError, URLError, ValueError):
        return None


def _probe_with_command(cmd_template: str, destination: str) -> Optional[int]:
    rendered = cmd_template.format(destination=destination)
    cmd = shlex.split(rendered)
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=30)
    except (subprocess.SubprocessError, OSError):
        return None
    try:
        payload = json.loads(proc.stdout.strip() or "{}")
    except json.JSONDecodeError:
        return None
    size = payload.get("size") or payload.get("content_length")
    try:
        return int(size) if size is not None else None
    except (TypeError, ValueError):
        return None


def validate_publish_plan(
    plan_path: Path,
    previous_plan_path: Optional[Path] = None,
    probe_remote: bool = False,
    probe_command: Optional[str] = None,
) -> Dict[str, object]:
    with plan_path.open("r", encoding="utf-8") as fh:
        try:
            plan = json.load(fh)
        except json.JSONDecodeError as exc:
            raise PublishPlanError(f"Publish plan is not valid JSON: {exc}") from exc

    artifacts = plan.get("artifacts", [])
    results = []
    local_failures: List[str] = []
    remote_failures: List[str] = []

    for entry in artifacts:
        if not isinstance(entry, dict):
            continue
        source = Path(entry.get("source", ""))
        expected_sha = entry.get("sha256")
        expected_size = entry.get("size")
        destination = entry.get("destination")
        rel_path = entry.get("relative_path")
        local_status = "ok"
        local_error = None

        if not source.exists():
            local_status = "missing"
            local_error = f"missing source {source}"
            local_failures.append(local_error)
        else:
            size = source.stat().st_size
            if expected_size is not None and size != expected_size:
                local_status = "size-mismatch"
                local_error = f"size mismatch for {source}: expected {expected_size} got {size}"
                local_failures.append(local_error)
            computed_sha = _sha256_file(source)
            if expected_sha and computed_sha != expected_sha:
                local_status = "sha-mismatch"
                local_error = (
                    f"sha mismatch for {source}: expected {expected_sha} computed {computed_sha}"
                )
                local_failures.append(local_error)

        remote_status = "skipped"
        remote_error = None
        remote_size = None
        if probe_remote and destination and destination.startswith(("http://", "https://")):
            remote_size = _http_head(destination)
            if remote_size is None:
                remote_status = "error"
                remote_error = f"HEAD failed for {destination}"
                remote_failures.append(remote_error)
            elif expected_size is not None and remote_size != expected_size:
                remote_status = "size-mismatch"
                remote_error = (
                    f"remote size mismatch for {destination}: expected {expected_size} got {remote_size}"
                )
                remote_failures.append(remote_error)
            else:
                remote_status = "ok"
        elif probe_remote and probe_command and destination:
            remote_size = _probe_with_command(probe_command, destination)
            if remote_size is None:
                remote_status = "error"
                remote_error = f"probe command failed for {destination}"
                remote_failures.append(remote_error)
            elif expected_size is not None and remote_size != expected_size:
                remote_status = "size-mismatch"
                remote_error = (
                    f"probe size mismatch for {destination}: expected {expected_size} got {remote_size}"
                )
                remote_failures.append(remote_error)
            else:
                remote_status = "ok"

        results.append(
            {
                "relative_path": rel_path,
                "destination": destination,
                "local_status": local_status,
                "local_error": local_error,
                "remote_status": remote_status,
                "remote_error": remote_error,
                "remote_size": remote_size,
            }
        )

    diff = None
    if previous_plan_path:
        with previous_plan_path.open("r", encoding="utf-8") as fh:
            previous = json.load(fh)
        prev_map = {a["relative_path"]: a for a in previous.get("artifacts", []) if isinstance(a, dict)}
        curr_map = {a["relative_path"]: a for a in artifacts if isinstance(a, dict)}
        added = sorted(set(curr_map) - set(prev_map))
        removed = sorted(set(prev_map) - set(curr_map))
        changed = sorted(
            path
            for path in set(curr_map) & set(prev_map)
            if (
                curr_map[path].get("sha256") != prev_map[path].get("sha256")
                or curr_map[path].get("destination") != prev_map[path].get("destination")
            )
        )
        diff = {"added": added, "removed": removed, "changed": changed}

    report = {
        "plan": str(plan_path),
        "generated_at": plan.get("generated_at"),
        "status": "ok" if not local_failures and not remote_failures else "failed",
        "local_failures": local_failures,
        "remote_failures": remote_failures,
        "results": results,
        "diff": diff,
    }
    return report


def _write_report(report: Dict[str, object], output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")


def _cmd_generate(args: argparse.Namespace) -> int:
    target_map = parse_target_map(args.target)
    plan = build_publish_plan(
        manifest_path=Path(args.manifest),
        artifacts_dir=Path(args.artifacts_dir),
        target_map=target_map,
    )
    write_plan_files(plan, Path(args.output_dir))
    return 0


def _cmd_validate(args: argparse.Namespace) -> int:
    report = validate_publish_plan(
        plan_path=Path(args.plan),
        previous_plan_path=Path(args.previous_plan) if args.previous_plan else None,
        probe_remote=args.probe_remote,
        probe_command=args.probe_command,
    )
    if args.output:
        _write_report(report, Path(args.output))
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "ok" else 1


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    gen = subparsers.add_parser("generate", help="Create publish_plan.* artefacts")
    gen.add_argument("--manifest", required=True, help="Path to release_manifest.json")
    gen.add_argument("--artifacts-dir", required=True, help="Directory containing release artifacts")
    gen.add_argument(
        "--target",
        action="append",
        required=True,
        help="Publish target URI (e.g., sorafs://... or https://...). Repeat as profile=uri to target specific tracks.",
    )
    gen.add_argument("--output-dir", required=True, help="Directory to write plan files into")
    gen.set_defaults(func=_cmd_generate)

    val = subparsers.add_parser("validate", help="Validate a publish plan (local and optional HTTP remote)")
    val.add_argument("--plan", required=True, help="Path to publish_plan.json")
    val.add_argument("--previous-plan", help="Optional previous plan to diff against")
    val.add_argument("--probe-remote", action="store_true", help="Probe HTTP(S) destinations via HEAD")
    val.add_argument(
        "--probe-command",
        help="Optional external probe command; use {destination} placeholder (expects JSON with size/content_length).",
    )
    val.add_argument("--output", help="Optional path to write the validation report JSON")
    val.set_defaults(func=_cmd_validate)
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    raise SystemExit(main())
