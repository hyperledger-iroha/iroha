#!/usr/bin/env python3
"""Generate and independently replay canonical release publication plans."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Mapping
from urllib import parse as urllib_parse
from urllib import request as urllib_request
from urllib.error import HTTPError, URLError

if __package__:
    from .release_artifact_contract import (
        ALLOWED_PROFILES,
        MAX_RELEASE_MANIFEST_SIZE,
        ReleaseArtifactError,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_write_bytes,
        format_source_date_epoch,
        load_canonical_release_manifest,
        load_json_object,
        parse_sha256sums,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_read_path,
        validate_artifact_descriptor,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )
else:
    from release_artifact_contract import (
        ALLOWED_PROFILES,
        MAX_RELEASE_MANIFEST_SIZE,
        ReleaseArtifactError,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_write_bytes,
        format_source_date_epoch,
        load_canonical_release_manifest,
        load_json_object,
        parse_sha256sums,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_read_path,
        validate_artifact_descriptor,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )


PUBLISH_PLAN_SCHEMA = "iroha.publish_plan"
PUBLISH_PLAN_SCHEMA_VERSION = 1
MAX_PUBLISH_PLAN_SIZE = 16 * 1024 * 1024
MAX_TARGET_URI_BYTES = 2048
_HEX_SHA256_RE = re.compile(r"[0-9a-f]{64}")
_URI_SCHEME_RE = re.compile(r"[a-z][a-z0-9+.-]{0,31}")
_PLAN_FIELDS = {
    "schema",
    "schema_version",
    "generated_at",
    "source_date_epoch",
    "manifest",
    "manifest_sha256",
    "manifest_signature",
    "manifest_public_key",
    "manifest_signature_algorithm",
    "manifest_public_key_format",
    "manifest_signer_fingerprint_sha256",
    "manifest_signature_verified",
    "manifest_verification_mode",
    "manifest_native_verifier_protocol",
    "manifest_native_verifier",
    "manifest_native_verifier_sha256",
    "artifacts_root",
    "target_map",
    "version",
    "commit",
    "os",
    "arch",
    "artifacts",
}
_PLAN_ARTIFACT_FIELDS = {
    "profile",
    "target",
    "kind",
    "format",
    "relative_path",
    "source",
    "destination",
    "sha256",
    "size",
}


class PublishPlanError(RuntimeError):
    """Raised when a plan violates the closed publication contract."""


@dataclass(frozen=True)
class Artifact:
    profile: str
    target: str
    kind: str
    format: str
    relative_path: str
    source: str
    destination: str
    sha256: str
    size: int


def _contains_control(value: str) -> bool:
    return any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def _safe_absolute_path(path: Path, label: str) -> Path:
    absolute = Path(os.path.abspath(path))
    rendered = str(absolute)
    if _contains_control(rendered):
        raise PublishPlanError(f"{label} must not contain control characters")
    return absolute


def _canonical_target_uri(raw: str) -> str:
    if (
        not isinstance(raw, str)
        or not raw
        or raw != raw.strip()
        or len(raw.encode("utf-8")) > MAX_TARGET_URI_BYTES
        or _contains_control(raw)
        or "\\" in raw
    ):
        raise PublishPlanError("publish target must be a bounded canonical URI")
    try:
        parsed = urllib_parse.urlsplit(raw)
        port = parsed.port
    except ValueError as exc:
        raise PublishPlanError(f"publish target URI is malformed: {raw!r}") from exc
    if (
        _URI_SCHEME_RE.fullmatch(parsed.scheme) is None
        or parsed.scheme not in {"https", "sorafs"}
        or not parsed.netloc
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or port is not None
        and not (1 <= port <= 65535)
        or raw.endswith("/")
    ):
        raise PublishPlanError(
            "publish target must be a canonical HTTPS or SoraFS base URI "
            "without credentials, query, fragment, or trailing slash"
        )
    try:
        raw.encode("ascii")
    except UnicodeEncodeError as exc:
        raise PublishPlanError("publish target URI must be ASCII") from exc
    decoded_path = urllib_parse.unquote(parsed.path)
    if (
        _contains_control(decoded_path)
        or "\\" in decoded_path
        or any(part in {".", ".."} for part in decoded_path.split("/"))
        or re.search(r"%(?:2[fF]|5[cC])", parsed.path) is not None
    ):
        raise PublishPlanError("publish target URI path is not canonical")
    canonical_path = urllib_parse.quote(
        decoded_path,
        safe="/-._~",
        encoding="utf-8",
        errors="strict",
    )
    if canonical_path != parsed.path:
        raise PublishPlanError("publish target URI uses a noncanonical path encoding")
    canonical = urllib_parse.urlunsplit(parsed)
    if canonical != raw:
        raise PublishPlanError("publish target URI is not in canonical form")
    return canonical


def _destination(base_uri: str, relative_path: str) -> str:
    encoded = "/".join(
        urllib_parse.quote(part, safe="-._~")
        for part in canonical_relative_path(relative_path).split("/")
    )
    return f"{base_uri}/{encoded}"


def parse_target_map(values: Iterable[str]) -> dict[str, str]:
    """Parse a default URI or exact per-profile URI mappings."""

    target_map: dict[str, str] = {}
    default_target: str | None = None
    saw_value = False
    for supplied in values:
        if supplied is None:
            continue
        if not isinstance(supplied, str) or not supplied:
            raise PublishPlanError("publish target values must be non-empty strings")
        saw_value = True
        if "=" in supplied:
            profile, uri = supplied.split("=", 1)
            if profile not in ALLOWED_PROFILES:
                raise PublishPlanError(f"unsupported publish target profile: {profile!r}")
            if profile in target_map:
                raise PublishPlanError(
                    f"duplicate publish target mapping for profile {profile}"
                )
            target_map[profile] = _canonical_target_uri(uri)
        else:
            if default_target is not None:
                raise PublishPlanError("multiple default publish targets are not allowed")
            default_target = _canonical_target_uri(supplied)
    if not saw_value:
        raise PublishPlanError("at least one publish target URI is required")
    if default_target is not None:
        for profile in sorted(ALLOWED_PROFILES):
            target_map.setdefault(profile, default_target)
    return dict(sorted(target_map.items()))


def _normalize_target_map(
    target_map: Mapping[str, str],
    required_profiles: set[str],
) -> dict[str, str]:
    if not isinstance(target_map, Mapping):
        raise PublishPlanError("publish target map must be a mapping")
    normalized: dict[str, str] = {}
    for profile, uri in target_map.items():
        if profile not in ALLOWED_PROFILES:
            raise PublishPlanError(f"unsupported publish target profile: {profile!r}")
        normalized[profile] = _canonical_target_uri(uri)
    if set(normalized) != required_profiles:
        missing = sorted(required_profiles - set(normalized))
        extra = sorted(set(normalized) - required_profiles)
        raise PublishPlanError(
            f"publish target map must exactly cover manifest profiles: "
            f"missing={missing}, extra={extra}"
        )
    return dict(sorted(normalized.items()))


def _stable_payload(path: Path, label: str, max_size: int) -> tuple[object, bytes]:
    try:
        info, payload = stable_read_path(path, max_size=max_size)
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"{label} is unsafe or unstable: {exc}") from exc
    return info, payload


def _manifest_inputs(
    manifest_path: Path,
    *,
    manifest_signature_path: Path | None,
    manifest_public_key_path: Path | None,
    trusted_signing_fingerprint: str | None,
    release_manifest_verifier_path: Path | None,
    trusted_release_manifest_verifier_sha256: str | None,
    development_allow_unsigned_manifest: bool,
) -> tuple[dict[str, object], bytes, dict[str, object]]:
    signed_values = (
        manifest_signature_path,
        manifest_public_key_path,
        trusted_signing_fingerprint,
        release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256,
    )
    supplied = sum(value is not None for value in signed_values)
    if development_allow_unsigned_manifest and supplied:
        raise PublishPlanError(
            "signed manifest inputs cannot be combined with "
            "development_allow_unsigned_manifest"
        )
    if not development_allow_unsigned_manifest and supplied != len(signed_values):
        if supplied:
            raise PublishPlanError(
                "manifest signature, raw public key, trusted fingerprint, native "
                "verifier path, and reviewed verifier SHA256 must be supplied together"
            )
        raise PublishPlanError(
            "production publish plans require a verified aggregate Ed25519 "
            "release-manifest signature"
        )

    manifest_path = _safe_absolute_path(manifest_path, "release manifest path")
    if development_allow_unsigned_manifest:
        _, payload = _stable_payload(
            manifest_path,
            "aggregate release manifest",
            MAX_RELEASE_MANIFEST_SIZE,
        )
        verification = {
            "manifest_sha256": hashlib.sha256(payload).hexdigest(),
            "signature_algorithm": None,
            "public_key_format": None,
            "signer_fingerprint_sha256": None,
            "signature_verified": False,
            "native_verifier_protocol": None,
            "native_verifier_path": None,
            "native_verifier_sha256": None,
            "verification_mode": "development-unsigned",
            "signature_path": None,
            "public_key_path": None,
        }
    else:
        assert manifest_signature_path is not None
        assert manifest_public_key_path is not None
        assert trusted_signing_fingerprint is not None
        assert release_manifest_verifier_path is not None
        assert trusted_release_manifest_verifier_sha256 is not None
        signature_path = _safe_absolute_path(
            manifest_signature_path, "manifest signature path"
        )
        public_key_path = _safe_absolute_path(
            manifest_public_key_path, "manifest public key path"
        )
        native_verifier = _safe_absolute_path(
            release_manifest_verifier_path, "native verifier path"
        )
        try:
            verified = verify_release_manifest(
                manifest_path,
                signature_path,
                public_key_path,
                trusted_signing_fingerprint,
                native_verifier,
                trusted_release_manifest_verifier_sha256,
            )
        except ReleaseManifestSignatureError as exc:
            raise PublishPlanError(
                f"aggregate release-manifest signature is invalid: {exc}"
            ) from exc
        _, payload = _stable_payload(
            manifest_path,
            "verified aggregate release manifest",
            MAX_RELEASE_MANIFEST_SIZE,
        )
        if hashlib.sha256(payload).hexdigest() != verified["manifest_sha256"]:
            raise PublishPlanError(
                "aggregate release manifest changed after signature verification"
            )
        verification = {
            **verified,
            "verification_mode": "ed25519",
            "signature_path": str(signature_path),
            "public_key_path": str(public_key_path),
        }
    try:
        manifest = load_canonical_release_manifest(payload)
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"aggregate release manifest is invalid: {exc}") from exc
    return manifest, payload, verification


def build_publish_plan(
    manifest_path: Path,
    artifacts_dir: Path,
    target_map: Mapping[str, str],
    *,
    manifest_signature_path: Path | None = None,
    manifest_public_key_path: Path | None = None,
    trusted_signing_fingerprint: str | None = None,
    release_manifest_verifier_path: Path | None = None,
    trusted_release_manifest_verifier_sha256: str | None = None,
    development_allow_unsigned_manifest: bool = False,
) -> dict[str, object]:
    """Build a deterministic plan from a closed canonical artifact inventory."""

    manifest_path = _safe_absolute_path(manifest_path, "release manifest path")
    artifacts_root = _safe_absolute_path(artifacts_dir, "release artifact root")
    manifest, _, verification = _manifest_inputs(
        manifest_path,
        manifest_signature_path=manifest_signature_path,
        manifest_public_key_path=manifest_public_key_path,
        trusted_signing_fingerprint=trusted_signing_fingerprint,
        release_manifest_verifier_path=release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=(
            trusted_release_manifest_verifier_sha256
        ),
        development_allow_unsigned_manifest=development_allow_unsigned_manifest,
    )
    manifest_rows = manifest["artifacts"]
    assert isinstance(manifest_rows, list)
    required_profiles = {str(row["profile"]) for row in manifest_rows}
    targets = _normalize_target_map(target_map, required_profiles)
    expected_paths = [str(row["path"]) for row in manifest_rows]
    try:
        scanned_before = scan_inventory_paths(
            artifacts_root,
            ignored={"SHA256SUMS"},
        )
        if scanned_before != sorted(expected_paths):
            missing = sorted(set(expected_paths) - set(scanned_before))
            extra = sorted(set(scanned_before) - set(expected_paths))
            raise PublishPlanError(
                f"closed publish artifact inventory mismatch: "
                f"missing={missing}, extra={extra}"
            )
        checksums = parse_sha256sums(artifacts_root)
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"release artifact inventory is invalid: {exc}") from exc
    if set(checksums) != set(expected_paths):
        missing = sorted(set(expected_paths) - set(checksums))
        extra = sorted(set(checksums) - set(expected_paths))
        raise PublishPlanError(
            f"canonical SHA256SUMS inventory mismatch: "
            f"missing={missing}, extra={extra}"
        )

    artifacts: list[Artifact] = []
    captured_files: dict[str, object] = {}
    for row in manifest_rows:
        path = str(row["path"])
        try:
            info = stable_hash_relative(artifacts_root, path)
        except ReleaseArtifactError as exc:
            raise PublishPlanError(f"release artifact {path!r} is unsafe: {exc}") from exc
        if info.sha256 != row["sha256"] or info.sha256 != checksums[path]:
            raise PublishPlanError(
                f"SHA256 mismatch for release artifact {path!r}"
            )
        if info.size != row["size"]:
            raise PublishPlanError(f"size mismatch for release artifact {path!r}")
        captured_files[path] = info
        profile = str(row["profile"])
        source = str(artifacts_root / Path(*path.split("/")))
        artifacts.append(
            Artifact(
                profile=profile,
                target=str(row["target"]),
                kind=str(row["kind"]),
                format=str(row["format"]),
                relative_path=path,
                source=source,
                destination=_destination(targets[profile], path),
                sha256=info.sha256,
                size=info.size,
            )
        )
    try:
        scanned_after = scan_inventory_paths(
            artifacts_root,
            ignored={"SHA256SUMS"},
        )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"release artifact inventory is invalid: {exc}") from exc
    if scanned_after != scanned_before:
        raise PublishPlanError(
            "release artifact inventory changed while the publish plan was built"
        )
    try:
        if parse_sha256sums(artifacts_root) != checksums:
            raise PublishPlanError(
                "canonical SHA256SUMS changed while the publish plan was built"
            )
        for path, before in captured_files.items():
            if stable_hash_relative(artifacts_root, path) != before:
                raise PublishPlanError(
                    f"release artifact {path!r} changed while the publish plan "
                    "was built"
                )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"release artifact inventory is invalid: {exc}") from exc

    return {
        "schema": PUBLISH_PLAN_SCHEMA,
        "schema_version": PUBLISH_PLAN_SCHEMA_VERSION,
        "generated_at": manifest["built_at"],
        "source_date_epoch": manifest["source_date_epoch"],
        "manifest": str(manifest_path),
        "manifest_sha256": verification["manifest_sha256"],
        "manifest_signature": verification["signature_path"],
        "manifest_public_key": verification["public_key_path"],
        "manifest_signature_algorithm": verification["signature_algorithm"],
        "manifest_public_key_format": verification["public_key_format"],
        "manifest_signer_fingerprint_sha256": verification[
            "signer_fingerprint_sha256"
        ],
        "manifest_signature_verified": verification["signature_verified"],
        "manifest_verification_mode": verification["verification_mode"],
        "manifest_native_verifier_protocol": verification[
            "native_verifier_protocol"
        ],
        "manifest_native_verifier": verification["native_verifier_path"],
        "manifest_native_verifier_sha256": verification["native_verifier_sha256"],
        "artifacts_root": str(artifacts_root),
        "target_map": targets,
        "version": manifest["version"],
        "commit": manifest["commit"],
        "os": manifest["os"],
        "arch": manifest["arch"],
        "artifacts": [
            {
                "profile": artifact.profile,
                "target": artifact.target,
                "kind": artifact.kind,
                "format": artifact.format,
                "relative_path": artifact.relative_path,
                "source": artifact.source,
                "destination": artifact.destination,
                "sha256": artifact.sha256,
                "size": artifact.size,
            }
            for artifact in artifacts
        ],
    }


def _validate_publish_plan_schema(plan: Mapping[str, object]) -> dict[str, object]:
    if set(plan) != _PLAN_FIELDS:
        raise PublishPlanError(
            "publish plan fields must be exactly " + ", ".join(sorted(_PLAN_FIELDS))
        )
    if (
        plan["schema"] != PUBLISH_PLAN_SCHEMA
        or plan["schema_version"] != PUBLISH_PLAN_SCHEMA_VERSION
    ):
        raise PublishPlanError("publish plan schema is unsupported")
    epoch = plan["source_date_epoch"]
    if isinstance(epoch, bool) or not isinstance(epoch, int):
        raise PublishPlanError("publish plan source_date_epoch must be an integer")
    if plan["generated_at"] != format_source_date_epoch(epoch):
        raise PublishPlanError(
            "publish plan generated_at must equal the manifest release epoch"
        )
    for field in ("manifest", "artifacts_root"):
        value = plan[field]
        if not isinstance(value, str) or not value or _contains_control(value):
            raise PublishPlanError(f"publish plan {field} path is invalid")
    if _HEX_SHA256_RE.fullmatch(str(plan["manifest_sha256"])) is None:
        raise PublishPlanError("publish plan manifest SHA256 is invalid")
    targets = plan["target_map"]
    if not isinstance(targets, dict) or not targets:
        raise PublishPlanError("publish plan target_map must be a non-empty object")
    normalized_targets = {
        profile: _canonical_target_uri(uri) for profile, uri in targets.items()
    }
    if normalized_targets != targets:
        raise PublishPlanError("publish plan target_map is not canonical")
    rows = plan["artifacts"]
    if not isinstance(rows, list) or not rows:
        raise PublishPlanError("publish plan artifacts must be a non-empty array")
    normalized_rows: list[dict[str, object]] = []
    for raw in rows:
        if not isinstance(raw, dict) or set(raw) != _PLAN_ARTIFACT_FIELDS:
            raise PublishPlanError(
                "publish plan artifact rows have an invalid closed schema"
            )
        descriptor = validate_artifact_descriptor(
            {
                "profile": raw["profile"],
                "target": raw["target"],
                "kind": raw["kind"],
                "format": raw["format"],
                "path": raw["relative_path"],
                "sha256": raw["sha256"],
                "size": raw["size"],
            },
            require_digest=True,
        )
        for field in ("source", "destination"):
            value = raw[field]
            if not isinstance(value, str) or not value or _contains_control(value):
                raise PublishPlanError(
                    f"publish plan artifact {field} is invalid"
                )
        profile = str(descriptor["profile"])
        if profile not in normalized_targets:
            raise PublishPlanError(
                f"publish plan has no target for profile {profile}"
            )
        if raw["destination"] != _destination(
            normalized_targets[profile],
            str(descriptor["path"]),
        ):
            raise PublishPlanError(
                "publish plan artifact destination is not canonically derived"
            )
        normalized_rows.append(dict(raw))
    if normalized_rows != sorted(
        normalized_rows,
        key=lambda row: (
            str(row["relative_path"]),
            str(row["profile"]),
            str(row["target"]),
            str(row["kind"]),
            str(row["format"]),
        ),
    ):
        raise PublishPlanError("publish plan artifact rows are not canonically sorted")
    mode = plan["manifest_verification_mode"]
    if mode == "ed25519":
        required_signed = {
            "manifest_signature": str,
            "manifest_public_key": str,
            "manifest_signer_fingerprint_sha256": str,
            "manifest_native_verifier": str,
            "manifest_native_verifier_sha256": str,
        }
        if (
            plan["manifest_signature_algorithm"] != "ed25519"
            or plan["manifest_public_key_format"] != "raw-ed25519-32"
            or plan["manifest_signature_verified"] is not True
            or plan["manifest_native_verifier_protocol"]
            != "sorafs-validate-release-manifest-v1"
            or any(
                not isinstance(plan[field], expected) or not plan[field]
                for field, expected in required_signed.items()
            )
        ):
            raise PublishPlanError(
                "publish plan has incomplete aggregate Ed25519 metadata"
            )
    elif mode == "development-unsigned":
        if (
            plan["manifest_signature"] is not None
            or plan["manifest_public_key"] is not None
            or plan["manifest_signature_algorithm"] is not None
            or plan["manifest_public_key_format"] is not None
            or plan["manifest_signer_fingerprint_sha256"] is not None
            or plan["manifest_signature_verified"] is not False
            or plan["manifest_native_verifier_protocol"] is not None
            or plan["manifest_native_verifier"] is not None
            or plan["manifest_native_verifier_sha256"] is not None
        ):
            raise PublishPlanError(
                "development unsigned plan contains signature metadata"
            )
    else:
        raise PublishPlanError(
            "publish plan verification mode must be ed25519 or development-unsigned"
        )
    return dict(plan)


def _load_canonical_plan(path: Path, label: str) -> tuple[object, dict[str, object]]:
    info, payload = _stable_payload(path, label, MAX_PUBLISH_PLAN_SIZE)
    try:
        plan = _validate_publish_plan_schema(
            load_json_object(payload, label)
        )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"{label} is invalid: {exc}") from exc
    if canonical_json_bytes(plan) != payload:
        raise PublishPlanError(f"{label} is not canonical deterministic JSON")
    return info, plan


def _render_messages(plan: Mapping[str, object]) -> list[str]:
    rows = plan["artifacts"]
    assert isinstance(rows, list)
    return [
        f"upload {row['source']} -> {row['destination']}"
        for row in rows
        if isinstance(row, dict)
    ]


def write_plan_files(
    plan: Mapping[str, object],
    output_dir: Path,
) -> dict[str, Path]:
    """Write all plan views exclusively; existing paths are never replaced."""

    normalized = _validate_publish_plan_schema(plan)
    output_root = _safe_absolute_path(output_dir, "publish plan output directory")
    if not output_root.is_dir() or output_root.is_symlink():
        raise PublishPlanError(
            "publish plan output directory must already exist as a direct directory"
        )
    paths = {
        "json": output_root / "publish_plan.json",
        "sh": output_root / "publish_plan.sh",
        "txt": output_root / "publish_plan.txt",
    }
    if any(path.exists() or path.is_symlink() for path in paths.values()):
        raise PublishPlanError("publish plan output paths must all be new")
    messages = _render_messages(normalized)
    shell_lines = [
        "#!/usr/bin/env bash",
        "set -euo pipefail",
        "",
        *[f"printf '%s\\n' {shlex.quote(message)}" for message in messages],
        "",
    ]
    try:
        exclusive_write_bytes(
            paths["json"],
            canonical_json_bytes(normalized),
            mode=0o644,
        )
        exclusive_write_bytes(
            paths["sh"],
            "\n".join(shell_lines).encode("utf-8"),
            mode=0o755,
        )
        exclusive_write_bytes(
            paths["txt"],
            ("\n".join(messages) + "\n").encode("utf-8"),
            mode=0o644,
        )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"failed to write publish plan: {exc}") from exc
    return paths


def _http_head(url: str) -> int | None:
    request = urllib_request.Request(url, method="HEAD")
    try:
        with urllib_request.urlopen(request, timeout=10) as response:
            length = response.headers.get("Content-Length")
            return int(length) if length is not None else None
    except (HTTPError, URLError, OSError, ValueError):
        return None


def _probe_with_command(template: str, destination: str) -> int | None:
    try:
        tokens = shlex.split(template)
    except ValueError:
        return None
    if sum(token.count("{destination}") for token in tokens) != 1:
        return None
    command = [
        token.replace("{destination}", destination) for token in tokens
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
    except (subprocess.SubprocessError, OSError):
        return None
    if len(completed.stdout.encode("utf-8")) > 64 * 1024:
        return None
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return None
    if not isinstance(payload, dict):
        return None
    size = payload.get("size", payload.get("content_length"))
    if isinstance(size, bool):
        return None
    try:
        parsed = int(size)
    except (TypeError, ValueError):
        return None
    return parsed if parsed >= 0 else None


def validate_publish_plan(
    plan_path: Path,
    previous_plan_path: Path | None = None,
    probe_remote: bool = False,
    probe_command: str | None = None,
    *,
    manifest_path: Path | None = None,
    artifacts_dir: Path | None = None,
    target_map: Mapping[str, str] | None = None,
    manifest_signature_path: Path | None = None,
    manifest_public_key_path: Path | None = None,
    trusted_signing_fingerprint: str | None = None,
    release_manifest_verifier_path: Path | None = None,
    trusted_release_manifest_verifier_sha256: str | None = None,
    development_allow_unsigned_manifest: bool = False,
) -> dict[str, object]:
    """Replay a plan only from independently supplied roots, targets, and tuple."""

    if manifest_path is None or artifacts_dir is None or target_map is None:
        raise PublishPlanError(
            "publish-plan validation requires independent manifest_path, "
            "artifacts_dir, and target_map inputs"
        )
    plan_path = _safe_absolute_path(plan_path, "publish plan path")
    pinned_plan, plan = _load_canonical_plan(plan_path, "publish plan")
    expected = build_publish_plan(
        manifest_path,
        artifacts_dir,
        target_map,
        manifest_signature_path=manifest_signature_path,
        manifest_public_key_path=manifest_public_key_path,
        trusted_signing_fingerprint=trusted_signing_fingerprint,
        release_manifest_verifier_path=release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=(
            trusted_release_manifest_verifier_sha256
        ),
        development_allow_unsigned_manifest=development_allow_unsigned_manifest,
    )
    if canonical_json_bytes(plan) != canonical_json_bytes(expected):
        raise PublishPlanError(
            "publish plan does not match independently reconstructed release inputs"
        )

    results: list[dict[str, object]] = []
    remote_failures: list[str] = []
    rows = plan["artifacts"]
    assert isinstance(rows, list)
    for row in rows:
        assert isinstance(row, dict)
        destination = str(row["destination"])
        expected_size = int(row["size"])
        remote_status = "skipped"
        remote_error: str | None = None
        remote_size: int | None = None
        if probe_remote and destination.startswith("https://"):
            remote_size = _http_head(destination)
        elif probe_remote and probe_command:
            remote_size = _probe_with_command(probe_command, destination)
        if probe_remote and (
            destination.startswith("https://") or probe_command is not None
        ):
            if remote_size is None:
                remote_status = "error"
                remote_error = f"remote size probe failed for {destination}"
            elif remote_size != expected_size:
                remote_status = "size-mismatch"
                remote_error = (
                    f"remote size mismatch for {destination}: "
                    f"expected {expected_size} got {remote_size}"
                )
            else:
                remote_status = "ok"
            if remote_error is not None:
                remote_failures.append(remote_error)
        results.append(
            {
                "relative_path": row["relative_path"],
                "destination": destination,
                "local_status": "ok",
                "local_error": None,
                "remote_status": remote_status,
                "remote_error": remote_error,
                "remote_size": remote_size,
            }
        )

    diff: dict[str, list[str]] | None = None
    if previous_plan_path is not None:
        _, previous = _load_canonical_plan(
            _safe_absolute_path(previous_plan_path, "previous publish plan path"),
            "previous publish plan",
        )
        previous_rows = previous["artifacts"]
        assert isinstance(previous_rows, list)
        previous_map = {
            str(row["relative_path"]): row
            for row in previous_rows
            if isinstance(row, dict)
        }
        current_map = {
            str(row["relative_path"]): row
            for row in rows
            if isinstance(row, dict)
        }
        diff = {
            "added": sorted(set(current_map) - set(previous_map)),
            "removed": sorted(set(previous_map) - set(current_map)),
            "changed": sorted(
                path
                for path in set(current_map) & set(previous_map)
                if current_map[path] != previous_map[path]
            ),
        }

    try:
        if stable_hash_path(plan_path) != pinned_plan:
            raise PublishPlanError(
                "publish plan path changed while validation was running"
            )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"publish plan became unsafe: {exc}") from exc
    return {
        "plan": str(plan_path),
        "plan_sha256": pinned_plan.sha256,
        "generated_at": plan["generated_at"],
        "status": "ok" if not remote_failures else "failed",
        "local_failures": [],
        "remote_failures": remote_failures,
        "results": results,
        "diff": diff,
    }


def _write_report(report: Mapping[str, object], output: Path) -> None:
    try:
        exclusive_write_bytes(
            _safe_absolute_path(output, "publish report output"),
            canonical_json_bytes(report),
            mode=0o644,
        )
    except ReleaseArtifactError as exc:
        raise PublishPlanError(f"failed to write publish report: {exc}") from exc


def _cmd_generate(args: argparse.Namespace) -> int:
    plan = build_publish_plan(
        manifest_path=Path(args.manifest),
        artifacts_dir=Path(args.artifacts_dir),
        target_map=parse_target_map(args.target),
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
        manifest_path=Path(args.manifest),
        artifacts_dir=Path(args.artifacts_dir),
        target_map=parse_target_map(args.target),
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
    if args.output:
        _write_report(report, Path(args.output))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "ok" else 1


def _add_manifest_tuple_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--artifacts-dir", required=True)
    parser.add_argument("--target", action="append", required=True)
    parser.add_argument("--manifest-signature")
    parser.add_argument("--manifest-public-key")
    parser.add_argument("--trusted-signing-fingerprint")
    parser.add_argument("--release-manifest-verifier")
    parser.add_argument("--trusted-release-manifest-verifier-sha256")
    parser.add_argument(
        "--development-allow-unsigned-manifest",
        action="store_true",
        help="TEST/DEVELOPMENT ONLY; never valid for promotion",
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate = subparsers.add_parser("generate")
    _add_manifest_tuple_arguments(generate)
    generate.add_argument("--output-dir", required=True)
    generate.set_defaults(func=_cmd_generate)

    validate = subparsers.add_parser("validate")
    validate.add_argument("--plan", required=True)
    validate.add_argument("--previous-plan")
    validate.add_argument("--probe-remote", action="store_true")
    validate.add_argument("--probe-command")
    validate.add_argument("--output")
    _add_manifest_tuple_arguments(validate)
    validate.set_defaults(func=_cmd_validate)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (PublishPlanError, ReleaseArtifactError) as exc:
        print(f"publish plan error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
