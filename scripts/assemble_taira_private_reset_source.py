#!/usr/bin/env python3
"""Assemble the exact owner-private source input accepted by Taira reset prep.

Only the matched validator roster/secrets and each peer's ``codec``/``configs``
trees are semantically consumed by reset preparation.  The remaining files are
explicit inert placeholders required by the legacy source-bundle envelope.

This helper also performs the one-way migration from the former single is2
onboarding credential to the required independent ``boi-mobile``/is2 and
``dpn-api``/dpn credentials.  It never prints either token.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
from typing import Any, NoReturn, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.9 operator hosts carry the backport.
    import tomli as tomllib  # type: ignore[no-redef]

try:
    from . import prepare_taira_empty_reset_bundle as reset_prepare
    from . import render_taira_validator_bundle as renderer
except ImportError:
    import prepare_taira_empty_reset_bundle as reset_prepare
    import render_taira_validator_bundle as renderer


SLUGS = reset_prepare.SLUGS
MAX_PRIVATE_INPUT = 64 * 1024 * 1024
MAX_STATIC_TOTAL = 512 * 1024 * 1024
KEY_RE = re.compile(r"[A-Za-z0-9_-]+")
RETIRED_NAMESPACE_WINDOW = 10
RETIRED_NAMESPACE_ROLLING = 0xD8F50D9988183A2E
RETIRED_NAMESPACE_SHA256 = (
    "a71a7c7011f53a1bab3642ec2ce12593f05230ace8de1e3e7645f69efac1443d"
)
ROLLING_BASE = 257
ROLLING_MASK = (1 << 64) - 1
ROLLING_REMOVE_FACTOR = 2_617_856_364_451_727_617
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")


class AssemblyError(RuntimeError):
    """A private source input is unsafe, ambiguous, or inconsistent."""


def fail(message: str) -> NoReturn:
    raise AssemblyError(message)


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
        info.st_nlink,
    )


def _sha256(payload: bytes) -> str:
    import hashlib

    return hashlib.sha256(payload).hexdigest()


def _contains_retired_namespace(body: bytes) -> bool:
    lowered = body.lower()
    window = RETIRED_NAMESPACE_WINDOW
    if len(lowered) < window:
        return False
    rolling = 0
    for value in lowered[:window]:
        rolling = ((rolling * ROLLING_BASE) + value) & ROLLING_MASK
    for offset in range(0, len(lowered) - window + 1):
        candidate = lowered[offset : offset + window]
        if (
            rolling == RETIRED_NAMESPACE_ROLLING
            and _sha256(candidate) == RETIRED_NAMESPACE_SHA256
        ):
            return True
        if offset + window < len(lowered):
            rolling = (
                (
                    rolling
                    - ((lowered[offset] * ROLLING_REMOVE_FACTOR) & ROLLING_MASK)
                )
                * ROLLING_BASE
                + lowered[offset + window]
            ) & ROLLING_MASK
    return False


def _canonical_json(payload: object) -> bytes:
    return (json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _secure_directory(path: Path, label: str, *, exact_private: bool) -> None:
    if not path.is_absolute():
        fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as error:
        raise AssemblyError(f"cannot inspect {label}: {error}") from error
    mode = stat.S_IMODE(info.st_mode)
    if (
        resolved != path
        or stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
        or (mode != 0o700 if exact_private else bool(mode & 0o022))
    ):
        requirement = "mode 0700" if exact_private else "non-group/world-writable"
        fail(f"{label} must be one canonical owner-controlled {requirement} directory")


def _secure_ancestry(path: Path, label: str) -> None:
    parent = path.parent
    while True:
        try:
            resolved = parent.resolve(strict=True)
            info = parent.lstat()
        except OSError as error:
            raise AssemblyError(f"cannot inspect {label} ancestry: {error}") from error
        if (
            resolved != parent
            or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid not in {0, os.getuid()}
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"{label} has unsafe replaceable ancestry: {parent}")
        if parent == parent.parent:
            return
        parent = parent.parent


def _read_private(path: Path, label: str, maximum: int = MAX_PRIVATE_INPUT) -> bytes:
    if not path.is_absolute():
        fail(f"{label} must be an absolute path")
    try:
        if path.resolve(strict=True) != path:
            fail(f"{label} must be a canonical path")
    except OSError as error:
        raise AssemblyError(f"cannot resolve {label}: {error}") from error
    _secure_ancestry(path, label)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise AssemblyError(f"cannot open {label}: {error}") from error
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.getuid()
            or before.st_gid != os.getgid()
            or stat.S_IMODE(before.st_mode) != 0o600
            or before.st_size <= 0
            or before.st_size > maximum
        ):
            fail(f"{label} must be one non-empty owner-private mode-0600 file")
        payload = bytearray()
        while True:
            block = os.read(descriptor, min(1024 * 1024, maximum + 1 - len(payload)))
            if not block:
                break
            payload.extend(block)
            if len(payload) > maximum:
                fail(f"{label} exceeds {maximum} bytes")
        after = os.fstat(descriptor)
        if _identity(before) != _identity(after):
            fail(f"{label} changed while it was read")
        return bytes(payload)
    finally:
        os.close(descriptor)


def _write_private(path: Path, payload: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        pending = memoryview(payload)
        while pending:
            written = os.write(descriptor, pending)
            if written <= 0:
                fail(f"short write while assembling {path.name}")
            pending = pending[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.chmod(path, 0o600)


def _toml_key(value: str) -> str:
    return value if KEY_RE.fullmatch(value) else json.dumps(value, ensure_ascii=False)


def _toml_scalar(value: object) -> str:
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, list) and all(not isinstance(item, dict) for item in value):
        return "[" + ", ".join(_toml_scalar(item) for item in value) + "]"
    fail(f"unsupported private TOML value type: {type(value).__name__}")


def _toml_bytes(payload: dict[str, Any]) -> bytes:
    lines: list[str] = []

    def table(path: tuple[str, ...], values: dict[str, Any], *, array: bool) -> None:
        if path:
            dotted = ".".join(_toml_key(part) for part in path)
            lines.append(f"[[{dotted}]]" if array else f"[{dotted}]")
        for key, value in values.items():
            if not isinstance(value, dict) and not (
                isinstance(value, list) and value and all(isinstance(item, dict) for item in value)
            ):
                lines.append(f"{_toml_key(key)} = {_toml_scalar(value)}")
        if path:
            lines.append("")
        for key, value in values.items():
            if isinstance(value, dict):
                table((*path, key), value, array=False)
            elif isinstance(value, list) and value and all(isinstance(item, dict) for item in value):
                for item in value:
                    table((*path, key), item, array=True)

    table((), payload, array=False)
    return ("\n".join(lines).rstrip() + "\n").encode("utf-8")


def _printable_token(payload: bytes, label: str) -> str:
    try:
        value = payload.decode("ascii")
    except UnicodeDecodeError:
        fail(f"{label} must contain printable ASCII")
    if not 32 <= len(value) <= 512 or any(not 0x21 <= ord(char) <= 0x7E for char in value):
        fail(f"{label} must contain 32-512 non-whitespace printable ASCII bytes")
    return value


def _migrate_secrets(legacy_payload: bytes, dpn_token_payload: bytes) -> bytes:
    try:
        parsed = tomllib.loads(legacy_payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        fail("legacy validator secrets are not valid UTF-8 TOML")
    shared = parsed.get("shared") if isinstance(parsed, dict) else None
    if not isinstance(shared, dict):
        fail("legacy validator secrets lack the shared table")
    if "account_onboarding_credentials" in shared:
        fail("validator secrets are already migrated; refusing an ambiguous rewrite")
    required = {
        "account_onboarding_api_token",
        "account_onboarding_credential_id",
        "account_onboarding_scope_dataspace",
    }
    if not required.issubset(shared):
        fail("legacy validator secrets lack the exact single onboarding credential")
    if shared.get("account_onboarding_scope_dataspace") != "is2":
        fail("legacy onboarding credential must be scoped exactly to is2")
    is2_token = shared.get("account_onboarding_api_token")
    if not isinstance(is2_token, str):
        fail("legacy is2 onboarding token is malformed")
    try:
        is2_token_bytes = is2_token.encode("ascii", "strict")
    except UnicodeEncodeError:
        fail("legacy is2 onboarding token must contain printable ASCII")
    _printable_token(is2_token_bytes, "legacy is2 onboarding token")
    dpn_token = _printable_token(dpn_token_payload, "DPN onboarding token")
    if is2_token == dpn_token:
        fail("is2 and dpn onboarding tokens must be distinct")
    for key in required:
        shared.pop(key)
    shared["account_onboarding_credentials"] = [
        {"id": "boi-mobile", "api_token": is2_token, "scope_dataspace": "is2"},
        {"id": "dpn-api", "api_token": dpn_token, "scope_dataspace": "dpn"},
    ]
    return _toml_bytes(parsed)


def _stage_pair_digest(roster: bytes, secrets: bytes) -> str:
    return _sha256(
        _canonical_json(
            {
                "schema": "iroha.taira.matched-validator-stage-pair.v1",
                "files": [
                    {
                        "path": "runtime/validator-roster.toml",
                        "sha256": _sha256(roster),
                        "size": len(roster),
                    },
                    {
                        "path": "runtime/validator-secrets.toml",
                        "sha256": _sha256(secrets),
                        "size": len(secrets),
                    },
                ],
            }
        )
    )


def _copy_static_tree(
    source: Path, destination: Path | None, inventory_prefix: str
) -> tuple[list[dict[str, object]], int]:
    _secure_directory(source, f"static tree {source}", exact_private=False)
    if _contains_retired_namespace(inventory_prefix.encode("utf-8")):
        fail("static source path retains the retired test namespace")
    if destination is not None:
        destination.mkdir(mode=0o700)
    rows: list[dict[str, object]] = [
        {
            "path": inventory_prefix,
            "type": "directory",
            "source_mode": stat.S_IMODE(source.lstat().st_mode),
            "output_mode": 0o700,
        }
    ]
    total_bytes = 0
    for root, directories, files in os.walk(source, topdown=True, followlinks=False):
        root_path = Path(root)
        for name in sorted(directories):
            child = root_path / name
            info = child.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode) or info.st_mode & 0o022:
                fail(f"unsafe static source directory: {child}")
            relative = (child.relative_to(source)).as_posix()
            inventory_path = f"{inventory_prefix}/{relative}"
            if _contains_retired_namespace(inventory_path.encode("utf-8")):
                fail("static source path retains the retired test namespace")
            rows.append(
                {
                    "path": inventory_path,
                    "type": "directory",
                    "source_mode": stat.S_IMODE(info.st_mode),
                    "output_mode": 0o700,
                }
            )
        relative_root = root_path.relative_to(source)
        output_root = destination / relative_root if destination is not None else None
        if output_root is not None:
            output_root.mkdir(mode=0o700, parents=True, exist_ok=True)
            os.chmod(output_root, 0o700)
        for name in sorted(files):
            child = root_path / name
            info = child.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1
                or info.st_uid != os.getuid()
                or info.st_mode & 0o022
                or info.st_size > MAX_PRIVATE_INPUT
            ):
                fail(f"unsafe static source file: {child}")
            flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
            descriptor = os.open(child, flags)
            try:
                before = os.fstat(descriptor)
                if (
                    _identity(info) != _identity(before)
                    or not stat.S_ISREG(before.st_mode)
                    or before.st_nlink != 1
                    or before.st_uid != os.getuid()
                    or before.st_gid != os.getgid()
                    or stat.S_IMODE(before.st_mode) & 0o022
                ):
                    fail(f"static source changed before secure open: {child}")
                body = bytearray()
                while block := os.read(descriptor, 1024 * 1024):
                    body.extend(block)
                    if len(body) > MAX_PRIVATE_INPUT:
                        fail(f"static source file exceeds its bound: {child}")
                if _identity(before) != _identity(os.fstat(descriptor)):
                    fail(f"static source file changed while read: {child}")
            finally:
                os.close(descriptor)
            if _contains_retired_namespace(bytes(body)):
                fail(f"static source retains the retired test namespace: {child}")
            if output_root is not None:
                _write_private(output_root / name, bytes(body))
            relative = (relative_root / name).as_posix()
            inventory_path = f"{inventory_prefix}/{relative}"
            if _contains_retired_namespace(inventory_path.encode("utf-8")):
                fail("static source path retains the retired test namespace")
            rows.append(
                {
                    "path": inventory_path,
                    "type": "file",
                    "source_mode": stat.S_IMODE(before.st_mode),
                    "output_mode": 0o600,
                    "sha256": _sha256(bytes(body)),
                    "size": len(body),
                }
            )
            total_bytes += len(body)
    return rows, total_bytes


def inspect_inputs(matched_stage_root: Path, static_rendered_root: Path) -> dict[str, object]:
    _secure_directory(matched_stage_root, "matched validator stage", exact_private=True)
    roster = _read_private(
        matched_stage_root / "runtime/validator-roster.toml", "validator roster"
    )
    secrets = _read_private(
        matched_stage_root / "runtime/validator-secrets.toml", "validator secrets"
    )
    if _contains_retired_namespace(roster) or _contains_retired_namespace(secrets):
        fail("matched roster/secrets retain the retired test namespace")
    _secure_directory(static_rendered_root, "static rendered root", exact_private=True)
    rows: list[dict[str, object]] = []
    total = 0
    for slug in SLUGS:
        for name in ("codec", "configs"):
            tree_rows, tree_bytes = _copy_static_tree(
                static_rendered_root / slug / name,
                None,
                f"{slug}/{name}",
            )
            rows.extend(tree_rows)
            total += tree_bytes
            if total > MAX_STATIC_TOTAL:
                fail("static source trees exceed the aggregate size bound")
    static_digest = _sha256(
        _canonical_json(
            {
                "schema": "iroha.taira.private-reset-static-inventory.v1",
                "files": sorted(rows, key=lambda row: str(row["path"])),
            }
        )
    )
    return {
        "schema": "iroha.taira.private-reset-source-inspection.v1",
        "matched_stage_pair_sha256": _stage_pair_digest(roster, secrets),
        "static_inventory_sha256": static_digest,
        "static_file_count": sum(row["type"] == "file" for row in rows),
        "static_directory_count": sum(row["type"] == "directory" for row in rows),
        "static_bytes": total,
    }


def assemble(args: argparse.Namespace) -> dict[str, object]:
    for name in (
        "output",
        "dpn_token_file",
        "trusted_stage_pair_sha256",
        "trusted_static_inventory_sha256",
    ):
        if getattr(args, name, None) is None:
            fail(f"--{name.replace('_', '-')} is required unless --inspect-only is used")
    _secure_directory(args.output.parent, "output parent", exact_private=True)
    if args.output.exists() or args.output.is_symlink():
        fail("output source bundle already exists")
    if SHA256_RE.fullmatch(args.trusted_stage_pair_sha256) is None:
        fail("trusted stage-pair SHA-256 is malformed")
    if SHA256_RE.fullmatch(args.trusted_static_inventory_sha256) is None:
        fail("trusted static inventory SHA-256 is malformed")
    inspection = inspect_inputs(args.matched_stage_root, args.static_rendered_root)
    if inspection["matched_stage_pair_sha256"] != args.trusted_stage_pair_sha256:
        fail("matched validator stage differs from its trusted roster/secrets digest")
    if inspection["static_inventory_sha256"] != args.trusted_static_inventory_sha256:
        fail("static source differs from its trusted admitted inventory digest")
    roster_path = args.matched_stage_root / "runtime/validator-roster.toml"
    secrets_path = args.matched_stage_root / "runtime/validator-secrets.toml"
    roster = _read_private(roster_path, "validator roster")
    legacy_secrets = _read_private(secrets_path, "legacy validator secrets")
    stage_pair_sha256 = _stage_pair_digest(roster, legacy_secrets)
    if stage_pair_sha256 != args.trusted_stage_pair_sha256:
        fail("matched validator stage changed after inspection")
    dpn_token = _read_private(args.dpn_token_file, "DPN onboarding token", 1024)
    migrated_secrets = _migrate_secrets(legacy_secrets, dpn_token)
    if _contains_retired_namespace(roster) or _contains_retired_namespace(migrated_secrets):
        fail("matched roster/secrets retain the retired test namespace")

    args.output.mkdir(mode=0o700)
    try:
        for name, body in (
            ("genesis.signed.nrt", b"inert-source-envelope-placeholder\n"),
            ("genesis.json", b"{}\n"),
            ("base-config.toml", b"# inert source envelope; final config is reviewed separately\n"),
            ("validator-roster.toml", roster),
            ("validator-secrets.toml", migrated_secrets),
        ):
            _write_private(args.output / name, body)
        manifest = {
            "schema": "taira-exact2f-reset-bundle",
            "peer_count": 4,
            "chain_id": reset_prepare.CHAIN_ID,
            "chain_discriminant": reset_prepare.CHAIN_DISCRIMINANT,
            "node_storage_budget_bytes": 68_719_476_736,
            "node_storage_budget_weights": {
                "kura_blocks_bps": 7499,
                "wsv_snapshots_bps": 2000,
                "sorafs_bps": 1,
                "soranet_spool_bps": 250,
                "soravpn_spool_bps": 250,
            },
            "nexus_storage_budget_policy": "bounded-64-gib-per-validator",
            "matched_stage_pair_sha256": stage_pair_sha256,
            "static_inventory_sha256": args.trusted_static_inventory_sha256,
            "source_envelope_note": (
                "only validator-roster.toml, validator-secrets.toml, and each "
                "peer codec/configs tree are consumed"
            ),
        }
        _write_private(
            args.output / "reset-manifest.json",
            (json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n").encode(),
        )
        rendered = args.output / "rendered"
        rendered.mkdir(mode=0o700)
        _write_private(rendered / "genesis.json", b"{}\n")
        static_rows: list[dict[str, object]] = []
        static_bytes = 0
        for slug in SLUGS:
            peer = rendered / slug
            peer.mkdir(mode=0o700)
            _write_private(peer / "config.toml", b"# inert source envelope\n")
            for name in ("manifests", "runtime", "storage"):
                (peer / name).mkdir(mode=0o700)
            for name in ("codec", "configs"):
                rows, size = _copy_static_tree(
                    args.static_rendered_root / slug / name,
                    peer / name,
                    f"{slug}/{name}",
                )
                static_rows.extend(rows)
                static_bytes += size
                if static_bytes > MAX_STATIC_TOTAL:
                    fail("static source trees exceed the aggregate size bound")

        static_inventory_sha256 = _sha256(
            _canonical_json(
                {
                    "schema": "iroha.taira.private-reset-static-inventory.v1",
                    "files": sorted(static_rows, key=lambda row: str(row["path"])),
                }
            )
        )
        if static_inventory_sha256 != args.trusted_static_inventory_sha256:
            fail("static source differs from its trusted admitted inventory digest")

        try:
            renderer.load_roster(
                args.output / "validator-roster.toml",
                secrets_path=args.output / "validator-secrets.toml",
            )
        except Exception:
            fail("migrated roster/secrets fail the current renderer contract")
        reset_prepare._load_source_manifest(args.output)
        digest = reset_prepare.source_bundle_sha256(args.output)
        return {
            "schema": "iroha.taira.private-reset-source-assembly.v1",
            "source_bundle": str(args.output),
            "source_bundle_sha256": digest,
            "matched_stage_pair_sha256": stage_pair_sha256,
            "static_inventory_sha256": static_inventory_sha256,
            "static_file_count": sum(row["type"] == "file" for row in static_rows),
            "static_directory_count": sum(
                row["type"] == "directory" for row in static_rows
            ),
            "static_bytes": static_bytes,
            "credential_scopes": ["is2", "dpn"],
        }
    except BaseException:
        shutil.rmtree(args.output)
        raise


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(allow_abbrev=False)
    value.add_argument("--matched-stage-root", type=Path, required=True)
    value.add_argument("--trusted-stage-pair-sha256")
    value.add_argument("--dpn-token-file", type=Path)
    value.add_argument("--static-rendered-root", type=Path, required=True)
    value.add_argument("--trusted-static-inventory-sha256")
    value.add_argument("--output", type=Path)
    value.add_argument("--inspect-only", action="store_true")
    return value


def main(argv: Sequence[str] | None = None) -> int:
    try:
        args = parser().parse_args(argv)
        receipt = (
            inspect_inputs(args.matched_stage_root, args.static_rendered_root)
            if args.inspect_only
            else assemble(args)
        )
    except AssemblyError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    print(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
