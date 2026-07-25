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

if __package__:
    from .release_manifest_signing import (
        MAX_MANIFEST_SIZE,
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )
else:
    from release_manifest_signing import (
        MAX_MANIFEST_SIZE,
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )


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


def _read_manifest_payload(path: Path) -> bytes:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise PublishPlanError(f"Cannot read release manifest {path}: {exc}") from exc
    if len(payload) > MAX_MANIFEST_SIZE:
        raise PublishPlanError(
            f"Release manifest exceeds the {MAX_MANIFEST_SIZE}-byte size limit"
        )
    return payload


def _load_manifest_payload(payload: bytes) -> Dict[str, object]:
    try:
        manifest = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PublishPlanError(f"Release manifest is not valid UTF-8 JSON: {exc}") from exc
    if not isinstance(manifest, dict):
        raise PublishPlanError("Release manifest root must be an object")
    if "artifacts" not in manifest or not isinstance(manifest["artifacts"], list):
        raise PublishPlanError("Release manifest is missing an 'artifacts' array")
    return manifest


def build_publish_plan(
    manifest_path: Path,
    artifacts_dir: Path,
    target_map: Dict[str, str],
    *,
    manifest_signature_path: Optional[Path] = None,
    manifest_public_key_path: Optional[Path] = None,
    trusted_signing_fingerprint: Optional[str] = None,
    release_manifest_verifier_path: Optional[Path] = None,
    trusted_release_manifest_verifier_sha256: Optional[str] = None,
    development_allow_unsigned_manifest: bool = False,
) -> Dict[str, object]:
    signed_values = (
        manifest_signature_path,
        manifest_public_key_path,
        trusted_signing_fingerprint,
        release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256,
    )
    signed_value_count = sum(value is not None for value in signed_values)
    if development_allow_unsigned_manifest and signed_value_count:
        raise PublishPlanError(
            "signed manifest inputs cannot be combined with "
            "development_allow_unsigned_manifest"
        )
    if not development_allow_unsigned_manifest and signed_value_count != len(signed_values):
        if signed_value_count:
            raise PublishPlanError(
                "manifest signature, raw public key, trusted fingerprint, native "
                "verifier path, and reviewed verifier SHA256 must be supplied together"
            )
        raise PublishPlanError(
            "production publish plans require a verified aggregate Ed25519 "
            "release-manifest signature"
        )

    manifest_path = manifest_path.absolute()
    if development_allow_unsigned_manifest:
        manifest_payload = _read_manifest_payload(manifest_path)
        manifest_verification = {
            "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
            "signature_algorithm": None,
            "public_key_format": None,
            "signer_fingerprint_sha256": None,
            "signature_verified": False,
            "native_verifier_protocol": None,
            "native_verifier_path": None,
            "native_verifier_sha256": None,
        }
        signature_path_value = None
        public_key_path_value = None
        verification_mode = "development-unsigned"
    else:
        assert manifest_signature_path is not None
        assert manifest_public_key_path is not None
        assert trusted_signing_fingerprint is not None
        assert release_manifest_verifier_path is not None
        assert trusted_release_manifest_verifier_sha256 is not None
        signature_path = manifest_signature_path.absolute()
        public_key_path = manifest_public_key_path.absolute()
        native_verifier_path = release_manifest_verifier_path.absolute()
        try:
            manifest_verification = verify_release_manifest(
                manifest_path,
                signature_path,
                public_key_path,
                trusted_signing_fingerprint,
                native_verifier_path,
                trusted_release_manifest_verifier_sha256,
            )
        except ReleaseManifestSignatureError as exc:
            raise PublishPlanError(
                f"aggregate release-manifest signature is invalid: {exc}"
            ) from exc
        # Parse only bytes bound to the verified digest. A path replacement after
        # verification therefore cannot feed unverified content into the plan.
        manifest_payload = _read_manifest_payload(manifest_path)
        if (
            hashlib.sha256(manifest_payload).hexdigest()
            != manifest_verification["manifest_sha256"]
        ):
            raise PublishPlanError(
                "aggregate release manifest changed after signature verification"
            )
        signature_path_value = str(signature_path)
        public_key_path_value = str(public_key_path)
        verification_mode = "ed25519"

    manifest = _load_manifest_payload(manifest_payload)
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
        "manifest_sha256": manifest_verification["manifest_sha256"],
        "manifest_signature": signature_path_value,
        "manifest_public_key": public_key_path_value,
        "manifest_signature_algorithm": manifest_verification["signature_algorithm"],
        "manifest_public_key_format": manifest_verification["public_key_format"],
        "manifest_signer_fingerprint_sha256": manifest_verification[
            "signer_fingerprint_sha256"
        ],
        "manifest_signature_verified": manifest_verification["signature_verified"],
        "manifest_verification_mode": verification_mode,
        "manifest_native_verifier_protocol": manifest_verification[
            "native_verifier_protocol"
        ],
        "manifest_native_verifier": manifest_verification["native_verifier_path"],
        "manifest_native_verifier_sha256": manifest_verification[
            "native_verifier_sha256"
        ],
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
    *,
    trusted_signing_fingerprint: Optional[str] = None,
    release_manifest_verifier_path: Optional[Path] = None,
    trusted_release_manifest_verifier_sha256: Optional[str] = None,
    development_allow_unsigned_manifest: bool = False,
) -> Dict[str, object]:
    with plan_path.open("r", encoding="utf-8") as fh:
        try:
            plan = json.load(fh)
        except json.JSONDecodeError as exc:
            raise PublishPlanError(f"Publish plan is not valid JSON: {exc}") from exc

    artifacts = plan.get("artifacts", [])
    if not isinstance(artifacts, list):
        raise PublishPlanError("Publish plan is missing an 'artifacts' array")
    results = []
    local_failures: List[str] = []
    remote_failures: List[str] = []

    manifest_value = plan.get("manifest")
    if not isinstance(manifest_value, str) or not manifest_value:
        raise PublishPlanError("Publish plan is missing the aggregate manifest path")
    manifest_path = Path(manifest_value)
    expected_manifest_sha = plan.get("manifest_sha256")
    if not isinstance(expected_manifest_sha, str):
        raise PublishPlanError("Publish plan is missing the aggregate manifest SHA256")
    verification_mode = plan.get("manifest_verification_mode")
    if verification_mode == "ed25519":
        if trusted_signing_fingerprint is None:
            raise PublishPlanError(
                "production publish-plan validation requires an independently "
                "trusted signing fingerprint"
            )
        if (
            release_manifest_verifier_path is None
            or trusted_release_manifest_verifier_sha256 is None
        ):
            raise PublishPlanError(
                "production publish-plan validation requires the pinned native "
                "release-manifest verifier path and reviewed SHA256"
            )
        signature_value = plan.get("manifest_signature")
        public_key_value = plan.get("manifest_public_key")
        fingerprint = plan.get("manifest_signer_fingerprint_sha256")
        if (
            not isinstance(signature_value, str)
            or not signature_value
            or not isinstance(public_key_value, str)
            or not public_key_value
            or not isinstance(fingerprint, str)
            or not fingerprint
            or plan.get("manifest_signature_algorithm") != "ed25519"
            or plan.get("manifest_public_key_format") != "raw-ed25519-32"
            or plan.get("manifest_signature_verified") is not True
            or plan.get("manifest_native_verifier_protocol")
            != "sorafs-validate-release-manifest-v1"
        ):
            raise PublishPlanError(
                "Publish plan has incomplete aggregate Ed25519 verification metadata"
            )
        if fingerprint != trusted_signing_fingerprint:
            raise PublishPlanError(
                "Publish plan signer fingerprint does not match the independently "
                "trusted signing fingerprint"
            )
        native_verifier = release_manifest_verifier_path.absolute()
        if plan.get("manifest_native_verifier") != str(native_verifier):
            raise PublishPlanError(
                "Publish plan native verifier path does not match the independently "
                "supplied verifier path"
            )
        if (
            plan.get("manifest_native_verifier_sha256")
            != trusted_release_manifest_verifier_sha256
        ):
            raise PublishPlanError(
                "Publish plan native verifier digest does not match the independently "
                "reviewed SHA256"
            )
        try:
            verification = verify_release_manifest(
                manifest_path,
                Path(signature_value),
                Path(public_key_value),
                trusted_signing_fingerprint,
                native_verifier,
                trusted_release_manifest_verifier_sha256,
            )
        except ReleaseManifestSignatureError as exc:
            local_failures.append(
                f"aggregate release-manifest signature verification failed: {exc}"
            )
        else:
            if verification["manifest_sha256"] != expected_manifest_sha:
                local_failures.append(
                    "aggregate release-manifest digest no longer matches the publish plan"
                )
    elif verification_mode == "development-unsigned":
        if trusted_signing_fingerprint is not None:
            raise PublishPlanError(
                "Development unsigned validation cannot accept a trusted signing "
                "fingerprint"
            )
        if (
            release_manifest_verifier_path is not None
            or trusted_release_manifest_verifier_sha256 is not None
        ):
            raise PublishPlanError(
                "Development unsigned validation cannot accept native verifier inputs"
            )
        if not development_allow_unsigned_manifest:
            raise PublishPlanError(
                "unsigned publish plans are test/development-only; validation requires "
                "development_allow_unsigned_manifest"
            )
        if (
            plan.get("manifest_signature") is not None
            or plan.get("manifest_public_key") is not None
            or plan.get("manifest_signature_algorithm") is not None
            or plan.get("manifest_public_key_format") is not None
            or plan.get("manifest_signer_fingerprint_sha256") is not None
            or plan.get("manifest_signature_verified") is not False
            or plan.get("manifest_native_verifier_protocol") is not None
            or plan.get("manifest_native_verifier") is not None
            or plan.get("manifest_native_verifier_sha256") is not None
        ):
            raise PublishPlanError(
                "Development unsigned publish plan contains conflicting signature metadata"
            )
        try:
            manifest_payload = _read_manifest_payload(manifest_path)
        except PublishPlanError as exc:
            local_failures.append(str(exc))
        else:
            if hashlib.sha256(manifest_payload).hexdigest() != expected_manifest_sha:
                local_failures.append(
                    "aggregate release-manifest digest no longer matches the publish plan"
                )
    else:
        raise PublishPlanError(
            "Publish plan must declare ed25519 or development-unsigned manifest verification"
        )

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
        manifest_signature_path=Path(args.manifest_signature)
        if args.manifest_signature
        else None,
        manifest_public_key_path=Path(args.manifest_public_key)
        if args.manifest_public_key
        else None,
        trusted_signing_fingerprint=args.trusted_signing_fingerprint,
        release_manifest_verifier_path=Path(args.release_manifest_verifier)
        if args.release_manifest_verifier
        else None,
        trusted_release_manifest_verifier_sha256=(
            args.trusted_release_manifest_verifier_sha256
        ),
        development_allow_unsigned_manifest=args.development_allow_unsigned_manifest,
    )
    write_plan_files(plan, Path(args.output_dir))
    return 0


def _cmd_validate(args: argparse.Namespace) -> int:
    report = validate_publish_plan(
        plan_path=Path(args.plan),
        previous_plan_path=Path(args.previous_plan) if args.previous_plan else None,
        probe_remote=args.probe_remote,
        probe_command=args.probe_command,
        trusted_signing_fingerprint=args.trusted_signing_fingerprint,
        release_manifest_verifier_path=Path(args.release_manifest_verifier)
        if args.release_manifest_verifier
        else None,
        trusted_release_manifest_verifier_sha256=(
            args.trusted_release_manifest_verifier_sha256
        ),
        development_allow_unsigned_manifest=args.development_allow_unsigned_manifest,
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
    gen.add_argument(
        "--manifest-signature",
        help="Raw 64-byte Ed25519 signature for the aggregate release manifest",
    )
    gen.add_argument(
        "--manifest-public-key",
        help="Raw 32-byte Ed25519 public key for the aggregate manifest",
    )
    gen.add_argument(
        "--trusted-signing-fingerprint",
        help="Reviewed lowercase SHA256 fingerprint of the raw Ed25519 public key",
    )
    gen.add_argument(
        "--release-manifest-verifier",
        help="Direct path to the reviewed sorafs-validate native verifier",
    )
    gen.add_argument(
        "--trusted-release-manifest-verifier-sha256",
        help="Reviewed lowercase SHA256 of the exact native verifier executable",
    )
    gen.add_argument(
        "--development-allow-unsigned-manifest",
        action="store_true",
        help=(
            "TEST/DEVELOPMENT ONLY: permit an unsigned aggregate manifest; "
            "never use this mode for promotion"
        ),
    )
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
    val.add_argument(
        "--trusted-signing-fingerprint",
        help=(
            "Independently reviewed lowercase SHA256 fingerprint of the raw "
            "Ed25519 public key; required for production validation"
        ),
    )
    val.add_argument(
        "--release-manifest-verifier",
        help="Direct path to the reviewed sorafs-validate native verifier",
    )
    val.add_argument(
        "--trusted-release-manifest-verifier-sha256",
        help="Reviewed lowercase SHA256 of the exact native verifier executable",
    )
    val.add_argument(
        "--development-allow-unsigned-manifest",
        action="store_true",
        help=(
            "TEST/DEVELOPMENT ONLY: validate a plan explicitly marked "
            "development-unsigned"
        ),
    )
    val.set_defaults(func=_cmd_validate)
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except PublishPlanError as exc:
        print(f"publish plan error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    raise SystemExit(main())
