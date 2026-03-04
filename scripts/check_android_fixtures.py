#!/usr/bin/env python3
"""Verify Android Norito fixtures match the committed manifest and payload sources."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, TextIO, Union

DEFAULT_RESOURCES_DIR = Path("java/iroha_android/src/test/resources")
DEFAULT_FIXTURES_PATH = DEFAULT_RESOURCES_DIR / "transaction_payloads.json"
DEFAULT_MANIFEST_PATH = DEFAULT_RESOURCES_DIR / "transaction_fixtures.manifest.json"
DEFAULT_STATE_PATH = Path("artifacts/android_fixture_regen_state.json")


def decode_base64(value: str, context: str) -> bytes:
    try:
        return base64.b64decode(value)
    except Exception as exc:  # pragma: no cover - defensive conversion
        raise ValueError(f"invalid base64 for {context}: {exc}") from exc


def iroha_hash(data: bytes) -> str:
    digest = bytearray(hashlib.blake2b(data, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()

def normalize_authority(value: str) -> str:
    if not isinstance(value, str):
        return value
    trimmed = value.strip()
    if not trimmed:
        return trimmed
    at_index = trimmed.rfind("@")
    if at_index > 0:
        return trimmed[:at_index]
    return trimmed


@dataclass(frozen=True)
class PayloadFixture:
    encoded: str
    chain: str
    authority: str
    creation_time_ms: int
    time_to_live_ms: Optional[int]
    nonce: Optional[int]


def load_payload_fixtures(path: Path) -> Dict[str, PayloadFixture]:
    try:
        payloads = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc

    if not isinstance(payloads, list):
        raise ValueError(f"fixtures JSON at {path} must be a list")

    mapping: Dict[str, PayloadFixture] = {}
    for entry in payloads:
        if not isinstance(entry, dict):
            raise ValueError(f"fixture entry in {path} is not an object")
        name = entry.get("name")
        encoded = entry.get("encoded")
        if not isinstance(name, str):
            raise ValueError(f"fixture entry in {path} missing name string: {entry!r}")
        if isinstance(encoded, str):
            chain = entry.get("chain")
            authority = entry.get("authority")
            creation_time_ms = entry.get("creation_time_ms")
            if "time_to_live_ms" not in entry:
                raise ValueError(
                    f"fixture entry {name} in {path} missing time_to_live_ms field"
                )
            if "nonce" not in entry:
                raise ValueError(
                    f"fixture entry {name} in {path} missing nonce field"
                )
            time_to_live_ms = entry.get("time_to_live_ms")
            nonce = entry.get("nonce")
            if not isinstance(chain, str) or not chain.strip():
                raise ValueError(
                    f"fixture entry {name} in {path} missing chain string"
                )
            if not isinstance(authority, str) or not authority.strip():
                raise ValueError(
                    f"fixture entry {name} in {path} missing authority string"
                )
            if not isinstance(creation_time_ms, int) or isinstance(creation_time_ms, bool):
                raise ValueError(
                    f"fixture entry {name} in {path} missing creation_time_ms integer"
                )
            if time_to_live_ms is not None and (
                not isinstance(time_to_live_ms, int) or isinstance(time_to_live_ms, bool)
            ):
                raise ValueError(
                    f"fixture entry {name} in {path} has invalid time_to_live_ms"
                )
            if nonce is not None and (not isinstance(nonce, int) or isinstance(nonce, bool)):
                raise ValueError(f"fixture entry {name} in {path} has invalid nonce")
            mapping[name] = PayloadFixture(
                encoded=encoded,
                chain=chain,
                authority=authority,
                creation_time_ms=creation_time_ms,
                time_to_live_ms=time_to_live_ms,
                nonce=nonce,
            )
            continue
        # Some fixtures intentionally embed raw payload JSON for doc/test coverage.
        # Skip them because they do not participate in the canonical manifest.
        if "payload" in entry:
            continue
        raise ValueError(f"fixture entry in {path} missing encoded string: {entry!r}")
    return mapping


def load_manifest(path: Path) -> Dict[str, object]:
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def compare(
    resources_dir: Path,
    manifest: Dict[str, object],
    payload_map: Dict[str, PayloadFixture],
) -> List[str]:
    errors: List[str] = []

    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list):
        errors.append("manifest missing 'fixtures' array")
        return errors

    seen_names: Set[str] = set()
    seen_files: Set[str] = set()

    for entry in fixtures:
        if not isinstance(entry, dict):
            errors.append(f"manifest fixture entry is not an object: {entry!r}")
            continue

        name = entry.get("name")
        encoded_file = entry.get("encoded_file")
        payload_base64 = entry.get("payload_base64")
        payload_hash = entry.get("payload_hash")
        encoded_len = entry.get("encoded_len")
        signed_base64 = entry.get("signed_base64")
        signed_hash = entry.get("signed_hash")
        signed_len = entry.get("signed_len")
        chain = entry.get("chain")
        authority = entry.get("authority")
        creation_time_ms = entry.get("creation_time_ms")
        if "time_to_live_ms" not in entry:
            errors.append(f"manifest fixture missing time_to_live_ms field: {entry}")
            continue
        if "nonce" not in entry:
            errors.append(f"manifest fixture missing nonce field: {entry}")
            continue
        time_to_live_ms = entry.get("time_to_live_ms")
        nonce = entry.get("nonce")

        if not all(
            isinstance(v, str)
            for v in (
                name,
                encoded_file,
                payload_base64,
                payload_hash,
                signed_base64,
                signed_hash,
                chain,
                authority,
            )
        ):
            errors.append(f"manifest fixture missing required string fields: {entry}")
            continue
        if not isinstance(encoded_len, int) or not isinstance(signed_len, int):
            errors.append(f"manifest fixture missing encoded_len/signed_len integers: {entry}")
            continue
        if not isinstance(creation_time_ms, int) or isinstance(creation_time_ms, bool):
            errors.append(f"manifest fixture missing creation_time_ms integer: {entry}")
            continue
        if time_to_live_ms is not None and (
            not isinstance(time_to_live_ms, int) or isinstance(time_to_live_ms, bool)
        ):
            errors.append(f"manifest fixture has invalid time_to_live_ms: {entry}")
            continue
        if nonce is not None and (not isinstance(nonce, int) or isinstance(nonce, bool)):
            errors.append(f"manifest fixture has invalid nonce: {entry}")
            continue

        seen_names.add(name)
        seen_files.add(encoded_file)

        expected_payload_bytes = decode_base64(payload_base64, f"{name} payload")
        expected_payload_hash = iroha_hash(expected_payload_bytes)
        if expected_payload_hash != payload_hash:
            errors.append(
                f"manifest payload_hash mismatch for {name}: manifest={payload_hash} computed={expected_payload_hash}"
            )

        payload_entry = payload_map.get(name)
        if payload_entry is None:
            errors.append(f"fixtures JSON missing entry for {name}")
        else:
            if payload_entry.encoded != payload_base64:
                errors.append(
                    f"payload JSON for {name} does not match manifest payload_base64"
                )
            if payload_entry.chain != chain:
                errors.append(
                    f"payload JSON chain mismatch for {name}: "
                    f"payloads={payload_entry.chain} manifest={chain}"
                )
            normalized_payload_authority = normalize_authority(payload_entry.authority)
            normalized_manifest_authority = normalize_authority(authority)
            if normalized_payload_authority != normalized_manifest_authority:
                errors.append(
                    f"payload JSON authority mismatch for {name}: "
                    f"payloads={payload_entry.authority} manifest={authority}"
                )
            if payload_entry.creation_time_ms != creation_time_ms:
                errors.append(
                    f"payload JSON creation_time_ms mismatch for {name}: "
                    f"payloads={payload_entry.creation_time_ms} manifest={creation_time_ms}"
                )
            if payload_entry.time_to_live_ms != time_to_live_ms:
                errors.append(
                    f"payload JSON time_to_live_ms mismatch for {name}: "
                    f"payloads={payload_entry.time_to_live_ms} manifest={time_to_live_ms}"
                )
            if payload_entry.nonce != nonce:
                errors.append(
                    f"payload JSON nonce mismatch for {name}: "
                    f"payloads={payload_entry.nonce} manifest={nonce}"
                )

        fixture_path = resources_dir / encoded_file
        if not fixture_path.exists():
            errors.append(f"fixture file missing: {fixture_path}")
        else:
            actual_bytes = fixture_path.read_bytes()
            if actual_bytes != expected_payload_bytes:
                errors.append(f"fixture file differs from manifest payload for {encoded_file}")
            if len(actual_bytes) != encoded_len:
                errors.append(
                    f"fixture length mismatch for {encoded_file}: manifest={encoded_len} actual={len(actual_bytes)}"
                )
            actual_hash = iroha_hash(actual_bytes)
            if actual_hash != payload_hash:
                errors.append(
                    f"fixture hash mismatch for {encoded_file}: manifest={payload_hash} actual={actual_hash}"
                )

        signed_bytes = decode_base64(signed_base64, f"{name} signed")
        if len(signed_bytes) != signed_len:
            errors.append(
                f"signed transaction length mismatch for {name}: manifest={signed_len} actual={len(signed_bytes)}"
            )
        signed_digest = iroha_hash(signed_bytes)
        if signed_digest != signed_hash:
            errors.append(
                f"signed transaction hash mismatch for {name}: manifest={signed_hash} actual={signed_digest}"
            )

    extra_payloads = sorted(set(payload_map) - seen_names)
    for name in extra_payloads:
        errors.append(f"fixtures JSON entry without manifest counterpart: {name}")

    for extra in sorted(p.name for p in resources_dir.glob("*.norito") if p.name not in seen_files):
        errors.append(f"unexpected fixture file present: {extra}")

    return errors


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check Android Norito fixtures against the committed manifest and payload definitions",
    )
    parser.add_argument(
        "--resources",
        type=Path,
        default=DEFAULT_RESOURCES_DIR,
        help=f"Directory containing Android fixture artifacts (default: {DEFAULT_RESOURCES_DIR})",
    )
    parser.add_argument(
        "--fixtures",
        type=Path,
        default=DEFAULT_FIXTURES_PATH,
        help=f"Path to transaction_payloads.json (default: {DEFAULT_FIXTURES_PATH})",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST_PATH,
        help=f"Path to transaction_fixtures.manifest.json (default: {DEFAULT_MANIFEST_PATH})",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a parity summary JSON file (for dashboards/gates).",
    )
    parser.add_argument(
        "--state",
        type=Path,
        default=DEFAULT_STATE_PATH,
        help=f"Path to the cadence state file recorded by scripts/android_fixture_regen.sh (default: {DEFAULT_STATE_PATH})",
    )
    parser.add_argument(
        "--pipeline-metadata",
        type=str,
        help="Optional JSON file describing pipeline/test metadata (use '-' to read from stdin).",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def load_state_metadata(path: Path) -> Optional[dict]:
    if not path:
        return None
    try:
        payload = json.loads(path.read_text())
    except FileNotFoundError:
        return None
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise ValueError(f"invalid JSON in state file {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"state file {path} must contain a JSON object")
    return payload


def load_pipeline_metadata(
    source: Optional[Union[Path, str]],
    *,
    stdin: TextIO | None = None,
) -> Optional[dict]:
    if source is None:
        return None
    if isinstance(source, Path):
        try:
            raw = source.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise ValueError(f"pipeline metadata file not found: {source}") from None
        except OSError as exc:  # pragma: no cover - defensive guard
            raise ValueError(f"failed to read pipeline metadata file {source}: {exc}") from exc
        source_label: Union[str, Path] = source
    else:
        if source != "-":
            raise ValueError("pipeline metadata source must be a path or '-' for stdin")
        stream = stdin if stdin is not None else sys.stdin
        raw = stream.read()
        if not raw.strip():
            raise ValueError("pipeline metadata from stdin was empty")
        source_label = "<stdin>"
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        location = source_label if isinstance(source_label, str) else str(source_label)
        raise ValueError(f"invalid JSON in pipeline metadata file {location}: {exc}") from exc
    if not isinstance(payload, dict):
        location = source_label if isinstance(source_label, str) else str(source_label)
        raise ValueError(f"pipeline metadata in {location} must be a JSON object")
    return payload


def build_summary_payload(
    *,
    resources_dir: Path,
    fixtures_path: Path,
    manifest_path: Path,
    state: Optional[dict],
    errors: Sequence[str],
    pipeline_metadata: Optional[dict],
    pipeline_source: Optional[str],
    manifest: Optional[dict],
    payload_map: Dict[str, PayloadFixture],
) -> dict:
    timestamp = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    payload: Dict[str, object] = {
        "generated_at": timestamp,
        "resources_dir": str(resources_dir),
        "fixtures_path": str(fixtures_path),
        "manifest_path": str(manifest_path),
        "result": {
            "status": "ok" if not errors else "error",
            "error_count": len(errors),
        },
    }
    if errors:
        payload["result"]["errors"] = list(errors)
    payload["artifacts"] = build_artifact_metadata(
        resources_dir=resources_dir,
        fixtures_path=fixtures_path,
        manifest_path=manifest_path,
        manifest=manifest,
        payload_map=payload_map,
    )
    if state is not None:
        payload["state"] = state
    if pipeline_metadata is not None:
        pipeline_block: Dict[str, object] = {"metadata": pipeline_metadata}
        if pipeline_source is not None:
            pipeline_block["source_path"] = pipeline_source
        payload["pipeline"] = pipeline_block
    return payload


def write_summary(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def hash_encoded_directory(resources_dir: Path) -> str:
    """Compute a deterministic hash across encoded fixture files (.norito)."""
    digest = hashlib.sha256()
    for fixture_path in sorted(resources_dir.glob("*.norito")):
        digest.update(fixture_path.name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(fixture_path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def build_artifact_metadata(
    *,
    resources_dir: Path,
    fixtures_path: Path,
    manifest_path: Path,
    manifest: Optional[dict],
    payload_map: Dict[str, PayloadFixture],
) -> Dict[str, object]:
    encoded_files = sorted(resources_dir.glob("*.norito"))
    manifest_fixtures = manifest.get("fixtures") if isinstance(manifest, dict) else None
    return {
        "manifest": {
          "path": str(manifest_path),
          "sha256": sha256_file(manifest_path),
          "fixture_count": len(manifest_fixtures) if isinstance(manifest_fixtures, list) else 0,
        },
        "payloads": {
            "path": str(fixtures_path),
            "sha256": sha256_file(fixtures_path),
            "entry_count": len(payload_map),
        },
        "encoded": {
          "dir": str(resources_dir),
          "file_count": len(encoded_files),
          "aggregate_sha256": hash_encoded_directory(resources_dir),
        },
    }


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)

    resources_dir = args.resources.resolve()
    fixtures_path = args.fixtures.resolve()
    manifest_path = args.manifest.resolve()
    summary_path: Optional[Path] = args.json_out.resolve() if args.json_out else None
    pipeline_arg = args.pipeline_metadata
    pipeline_source_label: Optional[str] = None

    try:
        state_metadata = load_state_metadata(args.state.resolve() if args.state else DEFAULT_STATE_PATH)
        pipeline_metadata = None
        if pipeline_arg:
            if pipeline_arg == "-":
                pipeline_source_label = "<stdin>"
                pipeline_metadata = load_pipeline_metadata("-", stdin=sys.stdin)
            else:
                pipeline_path = Path(pipeline_arg).resolve()
                pipeline_source_label = str(pipeline_path)
                pipeline_metadata = load_pipeline_metadata(pipeline_path)
    except ValueError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    missing_paths = [
        (resources_dir.exists(), f"[error] missing resources directory: {resources_dir}"),
        (fixtures_path.exists(), f"[error] missing fixtures JSON: {fixtures_path}"),
        (manifest_path.exists(), f"[error] missing manifest JSON: {manifest_path}"),
    ]
    has_missing = False
    for ok, message in missing_paths:
        if not ok:
            has_missing = True
            print(message, file=sys.stderr)
    if has_missing:
        return 1

    try:
        payload_map = load_payload_fixtures(fixtures_path)
        manifest = load_manifest(manifest_path)
    except ValueError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    errors = compare(resources_dir, manifest, payload_map)
    exit_code = 0
    if errors:
        for message in errors:
            print(f"[error] {message}", file=sys.stderr)
        exit_code = 1
    else:
        if not args.quiet:
            print(f"[ok] Android fixtures match manifest and payload JSON ({resources_dir})")

    if summary_path is not None:
        try:
            summary = build_summary_payload(
                resources_dir=resources_dir,
                fixtures_path=fixtures_path,
                manifest_path=manifest_path,
                state=state_metadata,
                errors=errors,
                pipeline_metadata=pipeline_metadata,
                pipeline_source=pipeline_source_label,
                manifest=manifest,
                payload_map=payload_map,
            )
            write_summary(summary_path, summary)
            if not args.quiet:
                print(f"[ok] wrote parity summary to {summary_path}")
        except Exception as exc:  # pragma: no cover - defensive guard
            print(f"[error] failed to write parity summary: {exc}", file=sys.stderr)
            exit_code = 1

    return exit_code


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
