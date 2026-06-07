"""Build and sign Kagemusha Android device-lab evidence artifacts."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


DEFAULT_SIGNED_EVIDENCE_PATH = device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH


def _secret_key_path_error(path: Path, label: str) -> str | None:
    if device_lab.SECRET_RE.search(str(path)):
        return f"{label} path must not contain secret-looking material"
    return None


def _validate_json_output_path(path: Path, label: str) -> list[str]:
    """Validate a signer-controlled output immediately before writing."""

    if device_lab.SECRET_RE.search(str(path)):
        return [f"{label} must not contain secret-looking material"]
    errors: list[str] = []
    parent = path.parent
    if parent.exists():
        if parent.is_symlink():
            errors.append(f"{label} parent directory must not be a symlink")
        elif not parent.is_dir():
            errors.append(f"{label} parent must be a directory")
    if errors:
        return errors
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    )
    if errors:
        return errors
    if not parent.exists():
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            errors.append(f"{label} parent directory could not be created")
    if errors:
        return errors

    if path.exists():
        if path.is_symlink():
            errors.append(f"{label} must not be a symlink")
        elif not path.is_file():
            errors.append(f"{label} must be a regular file")
        else:
            try:
                link_count = path.stat().st_nlink
            except OSError:
                errors.append(f"{label} hardlink metadata could not be read")
            else:
                if link_count > 1:
                    errors.append(f"{label} must not be hardlinked")
    return errors


def _write_json(path: Path, payload: dict[str, Any], label: str) -> list[str]:
    errors = _validate_json_output_path(path, label)
    if errors:
        return errors
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return []


def _write_text(path: Path, text: str, label: str) -> list[str]:
    errors = _validate_json_output_path(path, label)
    if errors:
        return errors
    path.write_text(text, encoding="utf-8")
    return []


def _preflight_slot_metadata_reads(slot_path: Path) -> list[str]:
    """Validate slot paths before any signer-controlled metadata is parsed."""

    if device_lab.SECRET_RE.search(str(slot_path)):
        return ["slot path must not contain secret-looking material"]
    if slot_path.is_symlink():
        return ["slot directory must not be a symlink"]
    if slot_path.parent.is_symlink():
        return ["slot parent directory must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        slot_path,
        "slot ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if not slot_path.is_dir():
        return ["slot directory missing"]

    errors: list[str] = []
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    return errors


def _require_slot_metadata(slot_path: Path) -> tuple[dict[str, Any] | None, list[str]]:
    errors = _preflight_slot_metadata_reads(slot_path)
    if errors:
        return None, errors
    metadata = device_lab._load_json(slot_path / "slot.json", "slot.json", errors)
    if metadata is None:
        return None, errors
    device_lab.validate_slot_metadata_fields(metadata, errors)
    if metadata.get("schema") != "iroha.android.device_lab.kagemusha.v1":
        errors.append("slot.json schema must be iroha.android.device_lab.kagemusha.v1")
    return metadata, errors


def _slot_string(metadata: dict[str, Any], key: str, errors: list[str]) -> str | None:
    value = metadata.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"slot.json {key} must be a non-empty string")
        return None
    if device_lab.SECRET_RE.search(value):
        errors.append(f"slot.json {key} must not contain secret-looking material")
        return None
    return value.strip()


def _slot_sha256(metadata: dict[str, Any], key: str, errors: list[str]) -> str | None:
    value = metadata.get(key)
    if not isinstance(value, str) or not device_lab.SHA256_HEX_RE.fullmatch(value):
        errors.append(f"slot.json {key} must be lowercase sha256 hex")
        return None
    return value


def _slot_true(metadata: dict[str, Any], key: str, errors: list[str]) -> bool | None:
    if metadata.get(key) is not True:
        errors.append(f"slot.json {key} must be true")
        return None
    return True


def _slot_int(metadata: dict[str, Any], key: str, errors: list[str]) -> int | None:
    value = metadata.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        if key == "native_bridge_abi_version":
            errors.append("slot.json native_bridge_abi_version must be an integer")
        else:
            errors.append(f"slot.json {key} must be an integer")
        return None
    return value


def _slot_raw_test_commands(metadata: dict[str, Any], errors: list[str]) -> list[str] | None:
    commands = metadata.get("raw_test_commands")
    if not isinstance(commands, list) or not commands:
        errors.append("slot.json raw_test_commands must be a non-empty array")
        return None
    accepted: list[str] = []
    for index, command in enumerate(commands):
        if not isinstance(command, str) or not command.strip():
            errors.append(f"slot.json raw_test_commands[{index}] must be a non-empty string")
            continue
        if device_lab.SECRET_RE.search(command):
            errors.append(
                f"slot.json raw_test_commands[{index}] must not contain secret-looking material"
            )
            continue
        accepted.append(command)
    if len(accepted) == len(commands):
        device_lab._validate_raw_test_command_markers(
            commands,
            label="slot.json raw_test_commands",
            errors=errors,
        )
    return accepted if len(accepted) == len(commands) else None


def _signer_public_key_sha256(public_key_path: Path, errors: list[str]) -> str | None:
    der = device_lab._openssl_public_key_der(
        public_key_path,
        errors=errors,
        label="signer public key",
    )
    if der is None:
        return None
    return hashlib.sha256(der).hexdigest()


def _sign_ed25519(private_key_path: Path, payload: bytes, errors: list[str]) -> bytes | None:
    secret_error = _secret_key_path_error(private_key_path, "private key")
    if secret_error is not None:
        errors.append(secret_error)
        return None
    if private_key_path.is_symlink():
        errors.append("private key must not be a symlink")
        return None
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        private_key_path,
        "private key ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return None
    if private_key_path.exists() and not private_key_path.is_file():
        errors.append("private key must be a regular file")
        return None
    if not private_key_path.is_file():
        errors.append("private key must point to an existing file")
        return None
    try:
        link_count = private_key_path.stat().st_nlink
    except OSError:
        errors.append("private key hardlink metadata could not be read")
        return None
    if link_count > 1:
        errors.append("private key must not be hardlinked")
        return None
    openssl = device_lab._require_openssl(errors)
    if openssl is None:
        return None
    with tempfile.TemporaryDirectory(prefix="iroha-kagemusha-evidence-sign-") as temp:
        temp_path = Path(temp)
        payload_path = temp_path / "payload.bin"
        signature_path = temp_path / "signature.bin"
        payload_path.write_bytes(payload)
        try:
            subprocess.run(
                [
                    openssl,
                    "pkeyutl",
                    "-sign",
                    "-inkey",
                    str(private_key_path),
                    "-rawin",
                    "-in",
                    str(payload_path),
                    "-out",
                    str(signature_path),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except subprocess.CalledProcessError:
            errors.append("private key must be a valid OpenSSL Ed25519 private key")
            return None
        return signature_path.read_bytes()


def _validate_private_public_pair(
    public_key_path: Path,
    payload: bytes,
    signature: bytes,
    errors: list[str],
) -> None:
    verify_errors: list[str] = []
    device_lab._verify_ed25519_signature(
        public_key_path=public_key_path,
        payload=payload,
        signature=signature,
        errors=verify_errors,
        label="signer public key",
    )
    if verify_errors:
        errors.append(
            "private key did not produce a signature accepted by the signer public key"
        )


def _normalise_output_path(
    slot_path: Path,
    metadata: dict[str, Any],
    output: str | None,
    errors: list[str],
) -> tuple[Path, str] | None:
    raw_output = output
    if raw_output is None:
        metadata_output = metadata.get("signed_evidence_artifact_path")
        raw_output = (
            metadata_output.strip()
            if isinstance(metadata_output, str) and metadata_output.strip()
            else DEFAULT_SIGNED_EVIDENCE_PATH
        )
    if device_lab.SECRET_RE.search(raw_output):
        errors.append("signed evidence output path must not contain secret-looking material")
        return None

    candidate = Path(raw_output)
    if candidate.is_absolute():
        try:
            relative = candidate.resolve().relative_to(slot_path.resolve()).as_posix()
        except ValueError:
            errors.append("signed evidence output path must stay inside the slot directory")
            return None
    else:
        relative = device_lab._normalise_safe_relative_path(
            raw_output,
            errors,
            "signed evidence output path",
        )
        if relative is None:
            return None
    if relative in {"slot.json", "sha256sum.txt"}:
        errors.append("signed evidence output path must not overwrite slot metadata")
        return None
    if relative.split("/", 1)[0] != "evidence":
        errors.append("signed evidence output path must stay under evidence/")
        return None
    if relative != DEFAULT_SIGNED_EVIDENCE_PATH:
        errors.append(f"signed evidence output path must be {DEFAULT_SIGNED_EVIDENCE_PATH}")
        return None
    return slot_path / relative, relative


def _artifact_digests(slot_path: Path, errors: list[str]) -> dict[str, str] | None:
    digests: dict[str, str] = {}
    initial_error_count = len(errors)
    preflight_errors = _preflight_slot_metadata_reads(slot_path)
    if preflight_errors:
        errors.extend(preflight_errors)
        return None
    device_lab.validate_required_kagemusha_slot_artifact_shapes(slot_path, errors)
    if len(errors) != initial_error_count:
        return None
    for relative in device_lab._required_signed_evidence_digest_paths(slot_path):
        if device_lab.SECRET_RE.search(relative):
            errors.append("slot artifacts must not contain secret-looking material")
            return None
        artifact_path = slot_path / relative
        if not artifact_path.is_file():
            errors.append(f"slot artifact {relative} is missing")
            return None
        digests[relative] = hashlib.sha256(artifact_path.read_bytes()).hexdigest()
    return digests


def build_signed_evidence(
    slot_path: Path,
    metadata: dict[str, Any],
    *,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
    signed_at_utc: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build, sign, and return the signed evidence JSON object."""

    if not signer_key_id.strip() or device_lab.SECRET_RE.search(signer_key_id):
        errors.append("signer key id must be non-empty and must not contain secret-looking material")
        return None
    signed_at_errors: list[str] = []
    device_lab._validate_signed_at_utc(signed_at_utc, signed_at_errors)
    if signed_at_errors:
        errors.extend(signed_at_errors)
        return None

    evidence: dict[str, Any] = {"schema": device_lab.SIGNED_EVIDENCE_SCHEMA}
    for key in device_lab.SIGNED_EVIDENCE_SLOT_STRING_FIELDS:
        value = _slot_string(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_SHA256_FIELDS:
        value = _slot_sha256(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_INT_FIELDS:
        value = _slot_int(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_TRUE_FIELDS:
        value = _slot_true(metadata, key, errors)
        if value is not None:
            evidence[key] = value

    commands = _slot_raw_test_commands(metadata, errors)
    if commands is not None:
        evidence["raw_test_commands"] = commands
    evidence["signed_at_utc"] = signed_at_utc
    evidence["signer_key_id"] = signer_key_id.strip()
    signer_public_key_sha256 = _signer_public_key_sha256(public_key_path, errors)
    if signer_public_key_sha256 is not None:
        evidence["signer_public_key_sha256"] = signer_public_key_sha256
    evidence["signature_algorithm"] = "ed25519"
    artifact_digests = _artifact_digests(slot_path, errors)
    if artifact_digests is not None:
        evidence["artifact_digests"] = artifact_digests

    if errors:
        return None

    payload = device_lab._canonical_signed_evidence_payload(evidence)
    signature = _sign_ed25519(private_key_path, payload, errors)
    if signature is None:
        return None
    _validate_private_public_pair(public_key_path, payload, signature, errors)
    if errors:
        return None
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = signature.hex()
    return evidence


def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:
    """Validate a slot immediately before rewriting its SHA-256 manifest."""

    if device_lab.SECRET_RE.search(str(slot_path)):
        return ["slot path must not contain secret-looking material"]
    if slot_path.is_symlink():
        return ["slot directory must not be a symlink"]
    if slot_path.parent.is_symlink():
        return ["slot parent directory must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        slot_path,
        "slot ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if not slot_path.is_dir():
        return ["slot directory missing"]
    errors: list[str] = []
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    if errors:
        return errors
    for relative in device_lab._slot_files(slot_path):
        if device_lab.SECRET_RE.search(relative):
            errors.append("slot artifacts must not contain secret-looking material")
            return errors
    return []


def rewrite_sha256_manifest(slot_path: Path) -> list[str]:
    """Rewrite sha256sum.txt so it exactly covers current slot artifacts."""

    errors = _validate_slot_for_manifest_rewrite(slot_path)
    if errors:
        return errors
    lines = []
    for relative in sorted(device_lab._slot_files(slot_path)):
        digest = hashlib.sha256((slot_path / relative).read_bytes()).hexdigest()
        lines.append(f"{digest}  {relative}")
    return _write_text(slot_path / "sha256sum.txt", "\n".join(lines) + "\n", "sha256sum.txt")


def sign_slot_evidence(
    *,
    slot_path: Path,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
    signed_at_utc: str,
    output: str | None,
    update_slot_json: bool,
    update_sha256sum: bool,
) -> tuple[int, str | None, list[str]]:
    """Sign one slot and return status, artifact path, and errors."""

    runtime_arg_errors = [
        error
        for error in (
            (
                "slot path must not contain secret-looking material"
                if device_lab.SECRET_RE.search(str(slot_path))
                else None
            ),
            _secret_key_path_error(private_key_path, "private key"),
            _secret_key_path_error(public_key_path, "signer public key"),
            (
                "signed evidence output path must not contain secret-looking material"
                if output is not None and device_lab.SECRET_RE.search(output)
                else None
            ),
            (
                "signer key id must be non-empty and must not contain secret-looking material"
                if not signer_key_id.strip() or device_lab.SECRET_RE.search(signer_key_id)
                else None
            ),
        )
        if error is not None
    ]
    if runtime_arg_errors:
        return 1, None, runtime_arg_errors

    metadata, errors = _require_slot_metadata(slot_path)
    if metadata is None:
        return 1, None, errors
    output_pair = _normalise_output_path(slot_path, metadata, output, errors)
    if output_pair is None:
        return 1, None, errors
    output_path, output_relative = output_pair
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    device_lab.validate_attestation_result(slot_path, metadata, errors)
    device_lab.validate_d2d_payment_transcript_binding(slot_path, metadata, errors)
    device_lab.validate_wallet_integrity_transcript_binding(slot_path, metadata, errors)
    if errors:
        return 1, None, errors
    evidence = build_signed_evidence(
        slot_path,
        metadata,
        private_key_path=private_key_path,
        public_key_path=public_key_path,
        signer_key_id=signer_key_id,
        signed_at_utc=signed_at_utc,
        errors=errors,
    )
    if evidence is None:
        return 1, None, errors

    write_errors = _write_json(output_path, evidence, "signed evidence output path")
    if write_errors:
        return 1, None, write_errors
    artifact_digest = hashlib.sha256(output_path.read_bytes()).hexdigest()
    if update_slot_json:
        metadata["signed_evidence_artifact_path"] = output_relative
        metadata["signed_evidence_artifact_sha256"] = artifact_digest
        write_errors = _write_json(slot_path / "slot.json", metadata, "slot.json")
        if write_errors:
            return 1, output_relative, write_errors
    if update_sha256sum:
        write_errors = rewrite_sha256_manifest(slot_path)
        if write_errors:
            return 1, output_relative, write_errors

    trusted = {}
    trusted_errors: list[str] = []
    public_der = device_lab._openssl_public_key_der(
        public_key_path,
        errors=trusted_errors,
        label="signer public key",
    )
    if public_der is not None:
        trusted[hashlib.sha256(public_der).hexdigest()] = public_key_path
    validation_errors, _details = device_lab.validate_kagemusha_production_metadata(
        slot_path,
        trusted,
    )
    if trusted_errors or validation_errors:
        return 1, output_relative, trusted_errors + validation_errors
    return 0, output_relative, []


def default_signed_at_utc() -> str:
    """Return a canonical UTC timestamp for signatures."""

    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00",
        "Z",
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build and sign a Kagemusha Android device-lab evidence artifact."
    )
    parser.add_argument("--slot", required=True, help="Device-lab slot directory.")
    parser.add_argument(
        "--private-key",
        required=True,
        help="Runtime-only OpenSSL Ed25519 private key used for signing.",
    )
    parser.add_argument(
        "--public-key",
        required=True,
        help="OpenSSL Ed25519 public key pinned by production validation.",
    )
    parser.add_argument("--signer-key-id", required=True, help="Stable lab signer key id.")
    parser.add_argument(
        "--signed-at-utc",
        default=None,
        help="ISO-8601 UTC signing timestamp. Defaults to the current UTC second.",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="Signed evidence output path, relative to the slot by default.",
    )
    parser.add_argument(
        "--no-update-slot-json",
        action="store_true",
        help="Do not refresh signed_evidence_artifact_* fields in slot.json.",
    )
    parser.add_argument(
        "--no-update-sha256sum",
        action="store_true",
        help="Do not rewrite sha256sum.txt after writing evidence.",
    )
    args = parser.parse_args(argv)

    status, output_relative, errors = sign_slot_evidence(
        slot_path=Path(args.slot),
        private_key_path=Path(args.private_key),
        public_key_path=Path(args.public_key),
        signer_key_id=args.signer_key_id,
        signed_at_utc=args.signed_at_utc or default_signed_at_utc(),
        output=args.output,
        update_slot_json=not args.no_update_slot_json,
        update_sha256sum=not args.no_update_sha256sum,
    )
    if status != 0:
        for error in errors:
            print(f"[device-lab-sign] {error}", file=sys.stderr)
        return status
    print(f"[device-lab-sign] wrote signed evidence {output_relative}")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
