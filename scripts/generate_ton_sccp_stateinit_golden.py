#!/usr/bin/env python3
"""Generate and verify the Tolk-owned SCCP TON StateInit final-V1 golden.

The semantic values come only from the Tolk script executed by exact Acton
1.1.0 (embedded Tolk 1.4.1).  This wrapper authenticates the supplied Acton
archive, parses a closed line protocol, records the complete contract source
closure, and publishes canonical JSON.  It never reconstructs TON cells in
Python.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "contracts" / "ton" / "sccp"
EMITTER = PROJECT / "scripts" / "generate-stateinit-golden.tolk"
FIXTURE = PROJECT / "tests" / "stateinit-golden-fixture.tolk"
DEFAULT_OUTPUT = ROOT / "fixtures" / "sccp" / "ton_stateinit_golden_v1.json"
ACTON_VERSION_FILE = PROJECT / ".acton" / ".version"
GENERATED_SOURCE_PATHS = (
    ACTON_VERSION_FILE,
    PROJECT / "gen" / "TairaXorJettonMaster.code.tolk",
    PROJECT / "gen" / "TairaXorJettonWallet.code.tolk",
)

SCHEMA = "iroha.sccp.ton-stateinit-golden.final-v1"
SOURCE_CLOSURE_DOMAIN = b"iroha:sccp:ton-stateinit-source-closure:final-v1\x00"
ACTON_VERSION = "acton 1.1.0 (9cf4d1f 2026-05-22)"
TOLK_VERSION = "1.4.1"
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024
MAX_TOOL_OUTPUT_BYTES = 2 * 1024 * 1024
HEX_RE = re.compile(r"^[0-9a-f]{1,64}$")

# The Linux archive is the release-corridor identity.  The macOS identity is
# accepted solely to reproduce/check this non-release test vector on the
# repository's supported development host.  Both archives contain the same
# Acton/Tolk implementation and the JSON deliberately records semantic
# toolchain identity rather than platform packaging identity.
APPROVED_ACTON_ARCHIVES: Mapping[str, str] = {
    "c2e640eacbb5b6ece1c343cab2ab6d2db74643d0706777aad181ed7e6e1bfc16": ("linux/amd64"),
    "44b0fcd928f196ae9ba7eb088e8ac51b155e93fe250488453b59427c3d63c216": (
        "darwin/arm64-development-only"
    ),
}

DECIMAL_FIELDS = (
    "sora_network_profile",
    "ton_network_profile",
    "sora_domain",
    "ton_domain",
    "expected_global_id",
    "workchain",
    "storage_version",
    "route_revision",
    "taira_to_ton_multiplier",
    "max_wrapped_supply",
    "route_code_depth",
    "route_initial_data_cell_depth",
    "master_code_depth",
    "master_initial_data_cell_depth",
)
HEX_FIELDS = (
    "sora_taira_chain_id",
    "ton_zero_state_root_hash",
    "ton_zero_state_file_hash",
    "source_lane_cell_hash",
    "source_lane_hash",
    "destination_lane_cell_hash",
    "destination_lane_hash",
    "jetton_master_code_hash",
    "jetton_wallet_code_hash",
    "route_code_hash",
    "semantic_proof_profile_hash",
    "sora_finality_anchor_hash",
    "verifier_circuit_hash",
    "verifying_key_hash",
    "proof_profile_commitment",
    "embedded_verifier_code_hash",
    "verifying_key_cell_hash",
    "master_metadata_cell_hash",
    "guardian_0",
    "guardian_1",
    "guardian_2",
    "guardian_3",
    "guardian_4",
    "destination_binding_hash",
    "route_configuration_hash",
    "route_config_cell_hash",
    "route_replay_cell_hash",
    "route_pending_cell_hash",
    "master_replay_cell_hash",
    "route_initial_data_cell_hash",
    "route_state_init_account_hash",
    "master_initial_data_cell_hash",
    "master_state_init_account_hash",
)
HEX_WIDTHS: Mapping[str, int] = {
    **{name: 64 for name in HEX_FIELDS},
    "sora_taira_chain_id": 32,
}
LINE_FIELDS = (
    "schema",
    "sora_network_profile",
    "ton_network_profile",
    "sora_domain",
    "ton_domain",
    "sora_taira_chain_id",
    "ton_zero_state_root_hash",
    "ton_zero_state_file_hash",
    "expected_global_id",
    "workchain",
    "storage_version",
    "route_revision",
    "taira_to_ton_multiplier",
    "max_wrapped_supply",
    *HEX_FIELDS[3:30],
    "route_code_depth",
    "route_initial_data_cell_depth",
    "route_state_init_account_hash",
    "master_initial_data_cell_hash",
    "master_code_depth",
    "master_initial_data_cell_depth",
    "master_state_init_account_hash",
)


class GoldenError(RuntimeError):
    """A bounded failure safe to show in a build log."""


def _fail(message: str) -> None:
    raise GoldenError(message)


def _read_regular(path: Path, *, label: str, maximum: int) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise GoldenError(f"{label} is unavailable") from error
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        _fail(f"{label} must be one direct regular file")
    if not 0 < before.st_size <= maximum:
        _fail(f"{label} has an invalid size")
    try:
        data = path.read_bytes()
        after = path.lstat()
    except OSError as error:
        raise GoldenError(f"{label} could not be read") from error
    if (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    ) or len(data) != before.st_size:
        _fail(f"{label} changed while it was read")
    return data


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _source_paths() -> tuple[Path, ...]:
    paths = {
        PROJECT / "Acton.toml",
        EMITTER,
        FIXTURE,
        *PROJECT.joinpath("contracts").glob("*.tolk"),
        *GENERATED_SOURCE_PATHS,
    }
    ordered = tuple(sorted(paths, key=lambda path: path.relative_to(ROOT).as_posix()))
    if not ordered or EMITTER not in ordered or FIXTURE not in ordered:
        _fail("TON StateInit source closure is incomplete")
    return ordered


def _recorded_source_inventory(value: object) -> dict[str, dict[str, Any]]:
    expected_paths = {
        path.relative_to(ROOT).as_posix()
        for path in _source_paths()
    }
    if type(value) is not list:
        _fail("TON StateInit golden source inventory is malformed")
    recorded: dict[str, dict[str, Any]] = {}
    for item in value:
        if type(item) is not dict or set(item) != {"path", "sha256", "size_bytes"}:
            _fail("TON StateInit golden source inventory is malformed")
        path = item.get("path")
        sha256 = item.get("sha256")
        size_bytes = item.get("size_bytes")
        if (
            type(path) is not str
            or path not in expected_paths
            or path in recorded
            or type(sha256) is not str
            or re.fullmatch(r"[0-9a-f]{64}", sha256) is None
            or type(size_bytes) is not int
            or not 0 < size_bytes <= 8 * 1024 * 1024
        ):
            _fail("TON StateInit golden source inventory is malformed")
        recorded[path] = item
    if set(recorded) != expected_paths:
        _fail("TON StateInit golden source inventory is incomplete")
    return recorded


def source_closure(
    recorded_inventory: object | None = None,
) -> tuple[list[dict[str, Any]], str]:
    """Return the exact source inventory and its domain-separated digest.

    A clean checkout does not contain Acton's ignored dependency marker or
    emitted code. Golden validation may therefore supply their authenticated
    recorded inventory entries; every tracked source is still read and hashed.
    Generation calls this without a recorded inventory and requires the full
    materialized closure.
    """

    inventory: list[dict[str, Any]] = []
    digest = hashlib.sha256()
    digest.update(SOURCE_CLOSURE_DOMAIN)
    recorded = (
        None
        if recorded_inventory is None
        else _recorded_source_inventory(recorded_inventory)
    )
    for path in _source_paths():
        relative = path.relative_to(ROOT).as_posix()
        generated_is_absent = False
        if recorded is not None and path in GENERATED_SOURCE_PATHS:
            try:
                path.lstat()
            except FileNotFoundError:
                generated_is_absent = True
            except OSError as error:
                raise GoldenError("TON StateInit source is unavailable") from error
        if generated_is_absent:
            item = recorded[relative]
            item_hash_hex = item["sha256"]
            size_bytes = item["size_bytes"]
        else:
            data = _read_regular(
                path, label="TON StateInit source", maximum=8 * 1024 * 1024
            )
            item_hash_hex = _sha256(data)
            size_bytes = len(data)
            item = {
                "path": relative,
                "sha256": item_hash_hex,
                "size_bytes": size_bytes,
            }
            if recorded is not None and recorded[relative] != item:
                _fail("TON StateInit golden source inventory is stale")
        encoded_path = relative.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(4, "little"))
        digest.update(encoded_path)
        digest.update(size_bytes.to_bytes(8, "little"))
        digest.update(bytes.fromhex(item_hash_hex))
        inventory.append(item)
    return inventory, digest.hexdigest()


def _authenticated_acton(executable: Path, archive: Path) -> tuple[Path, str, str]:
    if not executable.is_absolute() or not archive.is_absolute():
        _fail("Acton executable and archive paths must be absolute")
    executable_bytes = _read_regular(
        executable, label="Acton executable", maximum=MAX_ARCHIVE_BYTES
    )
    executable_mode = executable.lstat().st_mode
    if executable_mode & 0o111 == 0:
        _fail("Acton executable is not executable")
    archive_bytes = _read_regular(
        archive, label="Acton archive", maximum=MAX_ARCHIVE_BYTES
    )
    archive_sha256 = _sha256(archive_bytes)
    platform = APPROVED_ACTON_ARCHIVES.get(archive_sha256)
    if platform is None:
        _fail("Acton archive is not an approved exact 1.1.0 package")

    try:
        with tarfile.open(archive, mode="r:gz") as bundle:
            regular_members = []
            for member in bundle.getmembers():
                normalized = PurePosixPath(member.name)
                if normalized.is_absolute() or ".." in normalized.parts:
                    _fail("Acton archive contains an unsafe member")
                if member.isdir():
                    continue
                if not member.isfile():
                    _fail("Acton archive contains a non-regular member")
                regular_members.append(member)
            if (
                len(regular_members) != 1
                or regular_members[0].name.lstrip("./") != "acton"
            ):
                _fail("Acton archive does not have the exact one-binary shape")
            stream = bundle.extractfile(regular_members[0])
            if stream is None or stream.read(MAX_ARCHIVE_BYTES + 1) != executable_bytes:
                _fail("Acton executable does not match its authenticated archive")
    except (OSError, tarfile.TarError) as error:
        raise GoldenError("Acton archive could not be authenticated") from error
    return executable, archive_sha256, platform


def _run(
    executable: Path,
    arguments: Sequence[str],
    *,
    home: Path,
    timeout: int,
    label: str,
) -> tuple[bytes, bytes]:
    environment = {
        "HOME": os.fspath(home),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": os.fspath(home / "tmp"),
    }
    (home / "tmp").mkdir(mode=0o700, exist_ok=True)
    try:
        completed = subprocess.run(
            [os.fspath(executable), *arguments],
            cwd=PROJECT,
            env=environment,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise GoldenError(f"{label} could not complete") from error
    if (
        len(completed.stdout) > MAX_TOOL_OUTPUT_BYTES
        or len(completed.stderr) > MAX_TOOL_OUTPUT_BYTES
    ):
        _fail(f"{label} exceeded its output bound")
    if completed.returncode != 0:
        _fail(f"{label} failed")
    return completed.stdout, completed.stderr


def _run_emitter(executable: Path) -> bytes:
    with tempfile.TemporaryDirectory(prefix="sccp-ton-stateinit-") as temporary:
        home = Path(temporary)
        version, version_stderr = _run(
            executable,
            ("--version",),
            home=home,
            timeout=30,
            label="Acton version probe",
        )
        if version_stderr or version.decode("utf-8", "strict").strip() != ACTON_VERSION:
            _fail("Acton does not report the exact 1.1.0 identity")
        doctor, doctor_stderr = _run(
            executable,
            ("doctor", "--project-root", os.fspath(PROJECT)),
            home=home,
            timeout=60,
            label="Acton doctor",
        )
        if (
            doctor_stderr
            or len(re.findall(rb"(?m)^tolk\.version:[ \t]+1\.4\.1[ \t]*$", doctor)) != 1
        ):
            _fail("Acton does not expose exact embedded Tolk 1.4.1")
        _run(
            executable,
            ("build", "--color", "never"),
            home=home,
            timeout=1_800,
            label="TON contract dependency build",
        )
        output, stderr = _run(
            executable,
            ("script", "--color", "never", os.fspath(EMITTER.relative_to(PROJECT))),
            home=home,
            timeout=1_800,
            label="Tolk StateInit golden emitter",
        )
        if stderr:
            _fail("Tolk StateInit golden emitter wrote to stderr")
        repeated, repeated_stderr = _run(
            executable,
            ("script", "--color", "never", os.fspath(EMITTER.relative_to(PROJECT))),
            home=home,
            timeout=1_800,
            label="repeated Tolk StateInit golden emitter",
        )
        if repeated_stderr or repeated != output:
            _fail("Tolk StateInit golden emitter is not deterministic")
        return output


def parse_line_protocol(output: bytes) -> dict[str, str | int]:
    """Parse the exact emitter protocol and normalize all hashes to 32 bytes."""

    try:
        text = output.decode("ascii")
    except UnicodeDecodeError as error:
        raise GoldenError("Tolk emitter output is not ASCII") from error
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        _fail("Tolk emitter output is not canonical line text")
    values: dict[str, str] = {}
    for line in text.splitlines():
        if line.count("=") != 1:
            _fail("Tolk emitter output contains a malformed line")
        name, value = line.split("=", 1)
        if name not in LINE_FIELDS or name in values or not value:
            _fail("Tolk emitter output contains an unknown or duplicate field")
        values[name] = value
    if tuple(values) != LINE_FIELDS:
        _fail("Tolk emitter output fields are missing or out of order")
    if values["schema"] != SCHEMA:
        _fail("Tolk emitter selected the wrong schema")

    normalized: dict[str, str | int] = {"schema": SCHEMA}
    for name in DECIMAL_FIELDS:
        value = values[name]
        if not re.fullmatch(r"(?:0|[1-9][0-9]{0,38}|-[1-9][0-9]{0,38})", value):
            _fail("Tolk emitter produced a noncanonical integer")
        normalized[name] = int(value)
    for name in HEX_FIELDS:
        value = values[name]
        width = HEX_WIDTHS[name]
        if HEX_RE.fullmatch(value) is None or len(value) > width:
            _fail("Tolk emitter produced a noncanonical hash")
        normalized[name] = value.rjust(width, "0")

    if (
        normalized["sora_network_profile"] != 0x40
        or normalized["ton_network_profile"] != 0x44
    ):
        _fail("Tolk emitter did not use the final-V1 profile tags")
    if normalized["sora_domain"] != 0 or normalized["ton_domain"] != 4:
        _fail("Tolk emitter did not use the governed transfer domains")
    if normalized["expected_global_id"] != -239 or normalized["workchain"] != 0:
        _fail("Tolk emitter did not select TON mainnet/basechain")
    if normalized["storage_version"] != 1 or normalized["route_revision"] <= 0:
        _fail("Tolk emitter produced an invalid deployment version")
    if normalized["taira_to_ton_multiplier"] != 1:
        _fail("Tolk emitter produced an invalid scale-9 multiplier")
    if not 0 < int(normalized["max_wrapped_supply"]) < 2**120:
        _fail("Tolk emitter produced an invalid governed supply cap")
    for name in (
        "route_code_depth",
        "route_initial_data_cell_depth",
        "master_code_depth",
        "master_initial_data_cell_depth",
    ):
        if not 0 <= int(normalized[name]) <= 0xFFFF:
            _fail("Tolk emitter produced a cell depth outside the u16 domain")

    hashes = [
        str(normalized[name]) for name in HEX_FIELDS if name != "sora_taira_chain_id"
    ]
    if any(value == "0" * 64 for value in hashes):
        _fail("Tolk emitter produced a zero commitment")
    guardians = [str(normalized[f"guardian_{index}"]) for index in range(5)]
    if guardians != sorted(set(guardians)):
        _fail("Tolk emitter produced noncanonical breaker guardians")
    if (
        normalized["route_state_init_account_hash"]
        == normalized["master_state_init_account_hash"]
    ):
        _fail("Tolk emitter aliased route and master StateInit identities")
    return normalized


def build_golden(output: bytes) -> dict[str, Any]:
    """Build canonical JSON only from authenticated Tolk output and source bytes."""

    values = parse_line_protocol(output)
    inventory, closure_sha256 = source_closure()
    workchain = int(values["workchain"])
    route_account_hash = str(values["route_state_init_account_hash"])
    master_account_hash = str(values["master_state_init_account_hash"])
    return {
        "schema": SCHEMA,
        "provenance": {
            "generator_kind": "tolk-script",
            "generator_path": EMITTER.relative_to(ROOT).as_posix(),
            "fixture_path": FIXTURE.relative_to(ROOT).as_posix(),
            "acton_version": ACTON_VERSION,
            "tolk_version": TOLK_VERSION,
            "tolk_output_sha256": _sha256(output),
            "source_closure_sha256": closure_sha256,
            "source_inventory": inventory,
        },
        "network": {
            "sora_profile": int(values["sora_network_profile"]),
            "ton_profile": int(values["ton_network_profile"]),
            "sora_domain": int(values["sora_domain"]),
            "ton_domain": int(values["ton_domain"]),
            "sora_taira_chain_id": str(values["sora_taira_chain_id"]),
            "ton_global_id": int(values["expected_global_id"]),
            "ton_workchain": workchain,
            "ton_zero_state_root_hash": str(values["ton_zero_state_root_hash"]),
            "ton_zero_state_file_hash": str(values["ton_zero_state_file_hash"]),
        },
        "configuration": {
            "storage_version": int(values["storage_version"]),
            "route_revision": int(values["route_revision"]),
            "taira_to_ton_multiplier": int(values["taira_to_ton_multiplier"]),
            "max_wrapped_supply": str(values["max_wrapped_supply"]),
            "source_lane_cell_hash": str(values["source_lane_cell_hash"]),
            "source_lane_hash": str(values["source_lane_hash"]),
            "destination_lane_cell_hash": str(values["destination_lane_cell_hash"]),
            "destination_lane_hash": str(values["destination_lane_hash"]),
            "destination_binding_hash": str(values["destination_binding_hash"]),
            "route_configuration_hash": str(values["route_configuration_hash"]),
            "route_config_cell_hash": str(values["route_config_cell_hash"]),
            "semantic_proof_profile_hash": str(values["semantic_proof_profile_hash"]),
            "sora_finality_anchor_hash": str(values["sora_finality_anchor_hash"]),
            "verifier_circuit_hash": str(values["verifier_circuit_hash"]),
            "verifying_key_hash": str(values["verifying_key_hash"]),
            "proof_profile_commitment": str(values["proof_profile_commitment"]),
            "embedded_verifier_code_hash": str(values["embedded_verifier_code_hash"]),
            "verifying_key_cell_hash": str(values["verifying_key_cell_hash"]),
            "master_metadata_cell_hash": str(values["master_metadata_cell_hash"]),
            "mint_breaker_guardians": [
                str(values[f"guardian_{index}"]) for index in range(5)
            ],
        },
        "canonical_empty_state": {
            "route_replay_cell_hash": str(values["route_replay_cell_hash"]),
            "route_pending_cell_hash": str(values["route_pending_cell_hash"]),
            "master_replay_cell_hash": str(values["master_replay_cell_hash"]),
            "route_minting_disabled": False,
            "master_minting_disabled": False,
            "master_total_supply": "0",
        },
        "route": {
            "code_hash": str(values["route_code_hash"]),
            "code_depth": int(values["route_code_depth"]),
            "initial_data_cell_hash": str(values["route_initial_data_cell_hash"]),
            "initial_data_cell_depth": int(values["route_initial_data_cell_depth"]),
            "state_init_hash": route_account_hash,
            "address": {
                "workchain": workchain,
                "account_hash": route_account_hash,
                "raw": f"{workchain}:{route_account_hash}",
            },
        },
        "master": {
            "code_hash": str(values["jetton_master_code_hash"]),
            "code_depth": int(values["master_code_depth"]),
            "wallet_code_hash": str(values["jetton_wallet_code_hash"]),
            "initial_data_cell_hash": str(values["master_initial_data_cell_hash"]),
            "initial_data_cell_depth": int(values["master_initial_data_cell_depth"]),
            "state_init_hash": master_account_hash,
            "address": {
                "workchain": workchain,
                "account_hash": master_account_hash,
                "raw": f"{workchain}:{master_account_hash}",
            },
        },
    }


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def _line_protocol_from_golden(value: Mapping[str, Any]) -> bytes:
    """Reconstruct the exact Tolk line protocol for its bound output digest."""

    network = value["network"]
    configuration = value["configuration"]
    empty = value["canonical_empty_state"]
    route = value["route"]
    master = value["master"]
    guardians = configuration["mint_breaker_guardians"]
    fields: Mapping[str, Any] = {
        "schema": value["schema"],
        "sora_network_profile": network["sora_profile"],
        "ton_network_profile": network["ton_profile"],
        "sora_domain": network["sora_domain"],
        "ton_domain": network["ton_domain"],
        "sora_taira_chain_id": network["sora_taira_chain_id"],
        "ton_zero_state_root_hash": network["ton_zero_state_root_hash"],
        "ton_zero_state_file_hash": network["ton_zero_state_file_hash"],
        "expected_global_id": network["ton_global_id"],
        "workchain": network["ton_workchain"],
        "storage_version": configuration["storage_version"],
        "route_revision": configuration["route_revision"],
        "taira_to_ton_multiplier": configuration["taira_to_ton_multiplier"],
        "max_wrapped_supply": configuration["max_wrapped_supply"],
        "source_lane_cell_hash": configuration["source_lane_cell_hash"],
        "source_lane_hash": configuration["source_lane_hash"],
        "destination_lane_cell_hash": configuration["destination_lane_cell_hash"],
        "destination_lane_hash": configuration["destination_lane_hash"],
        "jetton_master_code_hash": master["code_hash"],
        "jetton_wallet_code_hash": master["wallet_code_hash"],
        "route_code_hash": route["code_hash"],
        "semantic_proof_profile_hash": configuration["semantic_proof_profile_hash"],
        "sora_finality_anchor_hash": configuration["sora_finality_anchor_hash"],
        "verifier_circuit_hash": configuration["verifier_circuit_hash"],
        "verifying_key_hash": configuration["verifying_key_hash"],
        "proof_profile_commitment": configuration["proof_profile_commitment"],
        "embedded_verifier_code_hash": configuration["embedded_verifier_code_hash"],
        "verifying_key_cell_hash": configuration["verifying_key_cell_hash"],
        "master_metadata_cell_hash": configuration["master_metadata_cell_hash"],
        **{f"guardian_{index}": guardians[index] for index in range(5)},
        "destination_binding_hash": configuration["destination_binding_hash"],
        "route_configuration_hash": configuration["route_configuration_hash"],
        "route_config_cell_hash": configuration["route_config_cell_hash"],
        "route_replay_cell_hash": empty["route_replay_cell_hash"],
        "route_pending_cell_hash": empty["route_pending_cell_hash"],
        "master_replay_cell_hash": empty["master_replay_cell_hash"],
        "route_initial_data_cell_hash": route["initial_data_cell_hash"],
        "route_code_depth": route["code_depth"],
        "route_initial_data_cell_depth": route["initial_data_cell_depth"],
        "route_state_init_account_hash": route["state_init_hash"],
        "master_initial_data_cell_hash": master["initial_data_cell_hash"],
        "master_code_depth": master["code_depth"],
        "master_initial_data_cell_depth": master["initial_data_cell_depth"],
        "master_state_init_account_hash": master["state_init_hash"],
    }
    lines = []
    for name in LINE_FIELDS:
        rendered = str(fields[name])
        if name in HEX_FIELDS:
            rendered = rendered.lstrip("0") or "0"
        lines.append(f"{name}={rendered}")
    return ("\n".join(lines) + "\n").encode("ascii")


def validate_checked_in_golden(data: bytes) -> dict[str, Any]:
    """Validate canonical shape and current Tolk source provenance without Acton."""

    try:
        value = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise GoldenError("TON StateInit golden is not canonical JSON") from error
    if type(value) is not dict or canonical_json(value) != data:
        _fail("TON StateInit golden is not canonical JSON")
    if set(value) != {
        "schema",
        "provenance",
        "network",
        "configuration",
        "canonical_empty_state",
        "route",
        "master",
    }:
        _fail("TON StateInit golden has an unknown final-V1 field")
    provenance = value.get("provenance")
    if type(provenance) is not dict or set(provenance) != {
        "generator_kind",
        "generator_path",
        "fixture_path",
        "acton_version",
        "tolk_version",
        "tolk_output_sha256",
        "source_closure_sha256",
        "source_inventory",
    }:
        _fail("TON StateInit golden lacks provenance")
    inventory, closure_sha256 = source_closure(provenance.get("source_inventory"))
    if provenance.get("source_inventory") != inventory:
        _fail("TON StateInit golden source inventory is stale")
    if provenance.get("source_closure_sha256") != closure_sha256:
        _fail("TON StateInit golden source closure is stale")
    if (
        provenance.get("acton_version") != ACTON_VERSION
        or provenance.get("tolk_version") != TOLK_VERSION
    ):
        _fail("TON StateInit golden has the wrong toolchain provenance")
    if (
        provenance.get("generator_kind") != "tolk-script"
        or provenance.get("generator_path") != EMITTER.relative_to(ROOT).as_posix()
        or provenance.get("fixture_path") != FIXTURE.relative_to(ROOT).as_posix()
        or type(provenance.get("tolk_output_sha256")) is not str
        or re.fullmatch(r"[0-9a-f]{64}", provenance["tolk_output_sha256"]) is None
    ):
        _fail("TON StateInit golden has invalid Tolk provenance")
    if value.get("schema") != SCHEMA:
        _fail("TON StateInit golden has the wrong schema")

    network = value.get("network")
    if type(network) is not dict or set(network) != {
        "sora_profile",
        "ton_profile",
        "sora_domain",
        "ton_domain",
        "sora_taira_chain_id",
        "ton_global_id",
        "ton_workchain",
        "ton_zero_state_root_hash",
        "ton_zero_state_file_hash",
    }:
        _fail("TON StateInit golden has an invalid network projection")
    if (
        network["sora_profile"] != 0x40
        or network["ton_profile"] != 0x44
        or network["sora_domain"] != 0
        or network["ton_domain"] != 4
        or network["ton_global_id"] != -239
        or network["ton_workchain"] != 0
        or type(network["sora_taira_chain_id"]) is not str
        or re.fullmatch(r"[0-9a-f]{32}", network["sora_taira_chain_id"]) is None
    ):
        _fail("TON StateInit golden has the wrong final-V1 network")

    configuration = value.get("configuration")
    if type(configuration) is not dict or set(configuration) != {
        "storage_version",
        "route_revision",
        "taira_to_ton_multiplier",
        "max_wrapped_supply",
        "source_lane_cell_hash",
        "source_lane_hash",
        "destination_lane_cell_hash",
        "destination_lane_hash",
        "destination_binding_hash",
        "route_configuration_hash",
        "route_config_cell_hash",
        "semantic_proof_profile_hash",
        "sora_finality_anchor_hash",
        "verifier_circuit_hash",
        "verifying_key_hash",
        "proof_profile_commitment",
        "embedded_verifier_code_hash",
        "verifying_key_cell_hash",
        "master_metadata_cell_hash",
        "mint_breaker_guardians",
    }:
        _fail("TON StateInit golden has an invalid configuration projection")
    cap = configuration["max_wrapped_supply"]
    guardians = configuration["mint_breaker_guardians"]
    if (
        configuration["storage_version"] != 1
        or type(configuration["route_revision"]) is not int
        or configuration["route_revision"] <= 0
        or configuration["taira_to_ton_multiplier"] != 1
        or type(cap) is not str
        or re.fullmatch(r"[1-9][0-9]{0,38}", cap) is None
        or int(cap) >= 2**120
        or type(guardians) is not list
        or len(guardians) != 5
        or guardians != sorted(set(guardians))
    ):
        _fail("TON StateInit golden configuration is not canonical")

    empty = value.get("canonical_empty_state")
    if type(empty) is not dict or set(empty) != {
        "route_replay_cell_hash",
        "route_pending_cell_hash",
        "master_replay_cell_hash",
        "route_minting_disabled",
        "master_minting_disabled",
        "master_total_supply",
    }:
        _fail("TON StateInit golden has an invalid empty-state projection")
    if (
        empty["route_minting_disabled"] is not False
        or empty["master_minting_disabled"] is not False
        or empty["master_total_supply"] != "0"
    ):
        _fail("TON StateInit golden is not a zero-state deployment")

    hex_values = [
        network["ton_zero_state_root_hash"],
        network["ton_zero_state_file_hash"],
        *(
            configuration[name]
            for name in configuration
            if name
            not in {
                "storage_version",
                "route_revision",
                "taira_to_ton_multiplier",
                "max_wrapped_supply",
                "mint_breaker_guardians",
            }
        ),
        *guardians,
        empty["route_replay_cell_hash"],
        empty["route_pending_cell_hash"],
        empty["master_replay_cell_hash"],
    ]
    if any(
        type(item) is not str
        or re.fullmatch(r"[0-9a-f]{64}", item) is None
        or item == "0" * 64
        for item in hex_values
    ):
        _fail("TON StateInit golden contains an invalid commitment")

    for role in ("route", "master"):
        item = value.get(role)
        if type(item) is not dict or type(item.get("address")) is not dict:
            _fail("TON StateInit golden lacks a StateInit role")
        expected_role_fields = (
            {
                "code_hash",
                "code_depth",
                "initial_data_cell_hash",
                "initial_data_cell_depth",
                "state_init_hash",
                "address",
            }
            if role == "route"
            else {
                "code_hash",
                "code_depth",
                "wallet_code_hash",
                "initial_data_cell_hash",
                "initial_data_cell_depth",
                "state_init_hash",
                "address",
            }
        )
        if set(item) != expected_role_fields or set(item["address"]) != {
            "workchain",
            "account_hash",
            "raw",
        }:
            _fail("TON StateInit golden has an invalid StateInit role")
        state_hash = item.get("state_init_hash")
        address = item["address"]
        depths = (item.get("code_depth"), item.get("initial_data_cell_depth"))
        if (
            type(state_hash) is not str
            or re.fullmatch(r"[0-9a-f]{64}", state_hash) is None
            or state_hash == "0" * 64
            or address.get("workchain") != 0
            or address.get("account_hash") != state_hash
            or address.get("raw") != f"{address.get('workchain')}:{state_hash}"
            or any(
                type(depth) is not int or not 0 <= depth <= 0xFFFF for depth in depths
            )
            or any(
                type(item.get(name)) is not str
                or re.fullmatch(r"[0-9a-f]{64}", item[name]) is None
                or item[name] == "0" * 64
                for name in (
                    ["code_hash", "initial_data_cell_hash"]
                    if role == "route"
                    else ["code_hash", "wallet_code_hash", "initial_data_cell_hash"]
                )
            )
        ):
            _fail("TON StateInit golden has an inconsistent derived address")
    if value["route"]["state_init_hash"] == value["master"]["state_init_hash"]:
        _fail("TON StateInit golden aliases route and master")
    if _sha256(_line_protocol_from_golden(value)) != provenance["tolk_output_sha256"]:
        _fail("TON StateInit golden differs from its Tolk output commitment")
    return value


def _publish(path: Path, data: bytes) -> None:
    path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp.{os.getpid()}")
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acton", required=True, type=Path)
    parser.add_argument("--acton-archive", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    try:
        parsed = _parser().parse_args(arguments)
        executable, archive_sha256, platform = _authenticated_acton(
            parsed.acton, parsed.acton_archive
        )
        generated = canonical_json(build_golden(_run_emitter(executable)))
        _, after_sha256, after_platform = _authenticated_acton(
            parsed.acton, parsed.acton_archive
        )
        if (after_sha256, after_platform) != (archive_sha256, platform):
            _fail("Acton toolchain changed during generation")
        output = parsed.output.resolve()
        if parsed.check:
            current = _read_regular(
                output, label="checked-in TON StateInit golden", maximum=4 * 1024 * 1024
            )
            validate_checked_in_golden(current)
            if current != generated:
                _fail("checked-in TON StateInit golden differs from Tolk output")
            print("TON SCCP StateInit golden matches exact Tolk output.")
        else:
            _publish(output, generated)
            validate_checked_in_golden(
                _read_regular(
                    output,
                    label="published TON StateInit golden",
                    maximum=4 * 1024 * 1024,
                )
            )
            print(output)
        return 0
    except (GoldenError, OSError, UnicodeError, ValueError, tarfile.TarError) as error:
        print(f"TON SCCP StateInit golden failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
