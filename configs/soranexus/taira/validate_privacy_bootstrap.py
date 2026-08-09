#!/usr/bin/env python3
"""Validate the fail-closed first-release Taira privacy bootstrap contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.9/3.10 operator hosts.
    import tomli as tomllib


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import taira_constants

DEFAULT_PLAN = HERE / "privacy_bootstrap_plan.json"
DEFAULT_CONFIG = HERE / "config.toml"
DEFAULT_GENESIS = HERE / "genesis.json"
DEFAULT_MATRIX = REPO_ROOT / "fixtures/privacy/exact12_v1.tsv"
MAX_INPUT_BYTES = 8 * 1024 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
EXPECTED_MATRIX_FILE_SHA256 = "7336d0221fddc51486ee53d4203f5a92d560d0ec9104a49de25896a8b10673d0"
EXPECTED_ROLLOUT_PLAN_SHA256 = "63f3d331b25e5b240b3e8ac291b1fa64c6901b52f88b2ea2bb7bdb8af0889aa2"
EXPECTED_GENESIS_AUTHORITY = (
    "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"
)

EXPECTED_PLAN_KEYS = {
    "schema",
    "schema_version",
    "chain_id",
    "chain_discriminant",
    "genesis_authority",
    "governance_permission",
    "governance_rollout",
    "privacy_catalog",
    "bootle_lantern_issuer",
}
EXPECTED_ROLLOUT_KEYS = {
    "activation_state",
    "controller_observation_required",
    "genesis_activation_forbidden",
    "mode",
    "notice_interval_blocks",
    "observation_interval_blocks",
    "rollout_plan_path",
    "rollout_plan_sha256",
}
EXPECTED_CATALOG_KEYS = {
    "matrix_version",
    "matrix_file_sha256",
    "registry_sha256",
    "protocols",
    "retired_labels",
}
EXPECTED_PROTOCOL_KEYS = {"index", "label", "statement_type"}
EXPECTED_BOOTLE_KEYS = {
    "protocol_label",
    "public_export_sha256",
    "designated_validator",
    "edge_routing",
    "issuer_id_hex",
    "policy_id_hex",
    "authorization_lifetime_blocks",
    "max_inflight",
    "state_authority",
    "provider_operations",
    "runtime_provider",
    "governed_issuer_policy",
    "secret_material_permitted",
}
EXPECTED_PROVIDER_KEYS = {
    "transport",
    "slot_wire_id",
    "handle",
    "revision",
    "qualification_policy_digest_hex",
}
EXPECTED_ISSUER_POLICY_KEYS = {
    "instruction_norito_sha256",
    "issuer_parameter_id_hex",
    "issuer_parameter_digest_hex",
    "record_digest_hex",
}
DISABLED_CONFIG_KEYS = {
    "enabled",
    "state_dir",
    "max_inflight",
    "authorization_lifetime_blocks",
    "max_records",
    "max_total_bytes",
    "terminal_retention_blocks",
}
RELEASE_CONFIG_BINDING_KEYS = {
    "issuer_id_hex",
    "policy_id_hex",
    "runtime_provider_registry_handle",
    "runtime_provider_registry_revision",
    "runtime_provider_registry_policy_digest_hex",
}
EXPECTED_BROKER_PUBLIC_KEYS = {
    "authorization_lifetime_blocks",
    "broker_contract_digest_hex",
    "chain_id",
    "issuer_id_hex",
    "issuer_parameter_digest_hex",
    "issuer_parameter_id_hex",
    "issuer_profile_digest_hex",
    "policy_id_hex",
    "policy_record_digest_hex",
    "registration_instruction",
    "registration_instruction_norito_hex",
    "registration_instruction_norito_sha256",
    "runtime_provider_handle",
    "runtime_provider_policy_digest_hex",
    "runtime_provider_revision",
    "schema",
    "stable_principal_digest_hex",
}
EXPECTED_BROKER_POLICY_KEYS = {
    "allowed_values",
    "epoch",
    "issuer_id",
    "issuer_parameter_digest",
    "issuer_parameter_id",
    "issuer_public_matrix",
    "lifecycle",
    "policy_id",
    "record_digest",
    "required_disclosure_bitmap",
}
EXPECTED_BROKER_SCHEMA = "iroha.taira.privacy.bootle-lantern-broker-public.v1"
EXPECTED_CHAIN_ID = taira_constants.CHAIN_ID
EXPECTED_PROVIDER_HANDLE = "runtime://privacy/bootle-lantern/taira-primary"


class PrivacyBootstrapValidationError(RuntimeError):
    """The checked bootstrap is incomplete, ambiguous, or non-canonical."""


@dataclass(frozen=True)
class MatrixContract:
    version: int
    file_sha256: str
    registry_sha256: str
    protocols: tuple[tuple[int, str, str], ...]
    retired_labels: tuple[str, ...]


@dataclass(frozen=True)
class ValidationPaths:
    plan: Path = DEFAULT_PLAN
    config: Path = DEFAULT_CONFIG
    genesis: Path = DEFAULT_GENESIS
    matrix: Path = DEFAULT_MATRIX
    broker_public: Path | None = None


def _fail(message: str) -> None:
    raise PrivacyBootstrapValidationError(message)


def _read_regular(path: Path, label: str) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise PrivacyBootstrapValidationError(f"cannot stat {label}: {path}") from exc
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        _fail(f"{label} must be one non-symlink regular file")
    if metadata.st_size > MAX_INPUT_BYTES:
        _fail(f"{label} exceeds the {MAX_INPUT_BYTES}-byte input bound")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
        try:
            opened = os.fstat(descriptor)
            if (
                (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino)
                or opened.st_size != metadata.st_size
                or opened.st_nlink != 1
                or opened.st_mtime_ns != metadata.st_mtime_ns
                or opened.st_ctime_ns != metadata.st_ctime_ns
            ):
                _fail(f"{label} changed before it could be read")
            payload = bytearray()
            while len(payload) <= MAX_INPUT_BYTES:
                chunk = os.read(
                    descriptor,
                    min(1024 * 1024, MAX_INPUT_BYTES + 1 - len(payload)),
                )
                if not chunk:
                    break
                payload.extend(chunk)
            closed = os.fstat(descriptor)
            if (
                (closed.st_dev, closed.st_ino) != (opened.st_dev, opened.st_ino)
                or closed.st_size != opened.st_size
                or closed.st_mtime_ns != opened.st_mtime_ns
                or closed.st_ctime_ns != opened.st_ctime_ns
            ):
                _fail(f"{label} changed while it was read")
        finally:
            os.close(descriptor)
    except OSError as exc:
        raise PrivacyBootstrapValidationError(f"cannot read {label}: {path}") from exc
    if len(payload) > MAX_INPUT_BYTES:
        _fail(f"{label} exceeds the {MAX_INPUT_BYTES}-byte input bound")
    return bytes(payload)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _reject_nonfinite_json(value: str) -> None:
    _fail(f"JSON contains non-finite number {value!r}")


def _decode_json_object(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonfinite_json,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PrivacyBootstrapValidationError(f"{label} is not strict UTF-8 JSON") from exc
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def _load_json(path: Path, label: str) -> dict[str, Any]:
    return _decode_json_object(_read_regular(path, label), label)


def _load_canonical_compact_json(path: Path, label: str) -> tuple[dict[str, Any], bytes]:
    payload = _read_regular(path, label)
    value = _decode_json_object(payload, label)
    canonical = (
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        + b"\n"
    )
    if payload != canonical:
        _fail(f"{label} is not in canonical compact emitted form")
    return value, payload


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        value = tomllib.loads(_read_regular(path, "Taira config").decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise PrivacyBootstrapValidationError("Taira config is not strict UTF-8 TOML") from exc
    if not isinstance(value, dict):
        _fail("Taira config root must be a table")
    return value


def _exact_keys(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        _fail(f"{label} fields differ: missing={missing}, extra={extra}")
    return value


def _fixed_sha256(value: Any, label: str, *, required: bool) -> str | None:
    if value is None and not required:
        return None
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be exactly 64 lowercase hexadecimal characters")
    if value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _exact_integer(value: Any, expected: int) -> bool:
    return type(value) is int and value == expected


def _parse_matrix(path: Path) -> MatrixContract:
    payload = _read_regular(path, "exact-12 matrix")
    file_sha256 = hashlib.sha256(payload).hexdigest()
    if file_sha256 != EXPECTED_MATRIX_FILE_SHA256:
        _fail("exact-12 matrix file digest differs from the frozen Taira V1 contract")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise PrivacyBootstrapValidationError("exact-12 matrix is not UTF-8") from exc
    if "\r" in text or not text.endswith("\n"):
        _fail("exact-12 matrix must use LF and end with one newline")
    version: int | None = None
    registry_sha256: str | None = None
    protocols: list[tuple[int, str, str]] = []
    typed_envelopes: list[tuple[str, str, str, str, str]] = []
    retired: list[str] = []
    for line_number, line in enumerate(text.splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if parts[0] == "matrix-version" and len(parts) == 2:
            try:
                version = int(parts[1])
            except ValueError:
                _fail(f"exact-12 matrix version at line {line_number} is not an integer")
        elif parts[0] == "registry-sha256" and len(parts) == 2:
            registry_sha256 = _fixed_sha256(
                parts[1], "exact-12 registry digest", required=True
            )
        elif parts[0] == "protocol" and len(parts) == 5:
            try:
                index = int(parts[1])
            except ValueError:
                _fail(f"exact-12 protocol index at line {line_number} is not an integer")
            if parts[3] != parts[4]:
                _fail(f"exact-12 row {line_number} changes statement/proof type")
            protocols.append((index, parts[2], parts[3]))
        elif parts[0] == "typed-envelope" and len(parts) == 6:
            _fixed_sha256(parts[4], f"typed statement digest at line {line_number}", required=True)
            _fixed_sha256(parts[5], f"typed envelope digest at line {line_number}", required=True)
            if parts[2] != parts[3]:
                _fail(f"typed-envelope row {line_number} changes statement/proof type")
            typed_envelopes.append((parts[1], parts[2], parts[3], parts[4], parts[5]))
        elif parts[0] == "retired" and len(parts) == 2:
            retired.append(parts[1])
        else:
            _fail(f"unknown exact-12 row at line {line_number}")
    if version != 1 or registry_sha256 is None:
        _fail("exact-12 matrix header is incomplete or not version 1")
    if tuple(index for index, _, _ in protocols) != tuple(range(12)):
        _fail("exact-12 matrix must contain canonical contiguous indices 0..11")
    labels = [label for _, label, _ in protocols]
    if len(set(labels)) != 12 or len(set(retired)) != len(retired):
        _fail("exact-12 matrix contains duplicate active or retired labels")
    computed = hashlib.sha256("".join(f"{label}\n" for label in labels).encode()).hexdigest()
    if computed != registry_sha256:
        _fail("exact-12 registry digest does not match the ordered active labels")
    if set(labels) & set(retired):
        _fail("exact-12 active and retired labels overlap")
    if len(typed_envelopes) != 12:
        _fail("exact-12 matrix must contain exactly 12 typed-envelope rows")
    for protocol, envelope in zip(protocols, typed_envelopes):
        _, label, statement_type = protocol
        if envelope[:3] != (label, statement_type, statement_type):
            _fail("typed-envelope rows must match exact-12 protocol order and types")
    statement_digests = [row[3] for row in typed_envelopes]
    envelope_digests = [row[4] for row in typed_envelopes]
    if len(set(statement_digests)) != 12 or len(set(envelope_digests)) != 12:
        _fail("typed statement and envelope digests must each be unique")
    return MatrixContract(
        version, file_sha256, registry_sha256, tuple(protocols), tuple(retired)
    )


def _validate_plan(plan: dict[str, Any], matrix: MatrixContract, *, release: bool) -> None:
    _exact_keys(plan, EXPECTED_PLAN_KEYS, "privacy bootstrap plan")
    if (
        plan["schema"] != "iroha.taira.privacy_bootstrap_plan.v1"
        or not _exact_integer(plan["schema_version"], 1)
    ):
        _fail("privacy bootstrap plan schema must be exact V1")
    if plan["chain_id"] != EXPECTED_CHAIN_ID:
        _fail("privacy bootstrap plan targets the wrong chain")
    if not _exact_integer(plan["chain_discriminant"], 369):
        _fail("privacy bootstrap plan targets the wrong chain discriminant")
    if plan["genesis_authority"] != EXPECTED_GENESIS_AUTHORITY:
        _fail("privacy bootstrap plan targets the wrong Taira governance authority")
    if plan["governance_permission"] != "CanEnactGovernance":
        _fail("privacy bootstrap authority must use CanEnactGovernance")

    rollout = _exact_keys(
        plan["governance_rollout"], EXPECTED_ROLLOUT_KEYS, "governance rollout"
    )
    if (
        rollout["activation_state"] != "not-executed"
        or rollout["controller_observation_required"] is not True
        or rollout["genesis_activation_forbidden"] is not True
        or rollout["mode"] != "governance-four-wave"
        or not _exact_integer(rollout["notice_interval_blocks"], 300)
        or not _exact_integer(rollout["observation_interval_blocks"], 300)
        or rollout["rollout_plan_path"]
        != "configs/soranexus/taira/privacy_rollout_plan_v1.json"
        or rollout["rollout_plan_sha256"] != EXPECTED_ROLLOUT_PLAN_SHA256
    ):
        _fail(
            "privacy activation must remain an unexecuted four-wave governance rollout "
            "with exact notice and observation intervals"
        )

    catalog = _exact_keys(plan["privacy_catalog"], EXPECTED_CATALOG_KEYS, "privacy catalog")
    if not _exact_integer(catalog["matrix_version"], matrix.version):
        _fail("privacy catalog matrix version differs from exact-12")
    if catalog["matrix_file_sha256"] != matrix.file_sha256:
        _fail("privacy catalog matrix file digest differs from exact-12")
    if catalog["registry_sha256"] != matrix.registry_sha256:
        _fail("privacy catalog registry digest differs from exact-12")
    expected_protocols = [
        {"index": index, "label": label, "statement_type": statement_type}
        for index, label, statement_type in matrix.protocols
    ]
    if not isinstance(catalog["protocols"], list):
        _fail("privacy catalog protocols must be an array")
    for index, protocol in enumerate(catalog["protocols"]):
        _exact_keys(protocol, EXPECTED_PROTOCOL_KEYS, f"privacy protocol {index}")
        if not _exact_integer(protocol["index"], index):
            _fail(f"privacy protocol {index} index differs from canonical order")
    if catalog["protocols"] != expected_protocols:
        _fail("privacy catalog must equal exact-12 in canonical order with no aliases")
    if catalog["retired_labels"] != list(matrix.retired_labels):
        _fail("retired privacy labels must equal the exact first-release retirement set")
    active_labels = {row["label"] for row in catalog["protocols"]}
    if "sis-with-hints" in active_labels or "sis-hints-anoncred-pq-v0" in active_labels:
        _fail("SIS-with-hints is retired and must never be activated")

    bootle = _exact_keys(
        plan["bootle_lantern_issuer"], EXPECTED_BOOTLE_KEYS, "Bootle/Lantern issuer"
    )
    expected_issuer_id = hashlib.sha256(
        b"iroha.taira.privacy.bootle-lantern.issuer.v1"
    ).hexdigest()
    expected_policy_id = hashlib.sha256(
        b"iroha.taira.privacy.bootle-lantern.policy.v1"
    ).hexdigest()
    if (
        bootle["protocol_label"] != "iroha-bootle-lantern-anoncred-v1"
        or bootle["designated_validator"] != "taira-validator-1"
        or bootle["edge_routing"] != "designated-validator-only"
        or bootle["issuer_id_hex"] != expected_issuer_id
        or bootle["policy_id_hex"] != expected_policy_id
        or not _exact_integer(bootle["authorization_lifetime_blocks"], 300)
        or not _exact_integer(bootle["max_inflight"], 2)
        or bootle["state_authority"] != "torii-local-one-shot-store"
        or bootle["provider_operations"]
        != ["bearer-principal-authentication-v1", "falcon512-native-crypto-v1"]
        or bootle["secret_material_permitted"] is not False
    ):
        _fail("Bootle/Lantern public issuer contract differs from canonical Taira V1")
    _fixed_sha256(
        bootle["public_export_sha256"],
        "Bootle/Lantern broker public export digest",
        required=release,
    )
    if not release and bootle["public_export_sha256"] is not None:
        _fail("staging plan must not bind a broker public export")
    provider = _exact_keys(
        bootle["runtime_provider"], EXPECTED_PROVIDER_KEYS, "Bootle/Lantern runtime provider"
    )
    if (
        provider["transport"] != "stock-local-runtime-provider-broker-v1"
        or not _exact_integer(provider["slot_wire_id"], 56)
        or provider["handle"] != EXPECTED_PROVIDER_HANDLE
        or not _exact_integer(provider["revision"], 1)
    ):
        _fail("Bootle/Lantern must use the exact stock broker slot-56 public binding")
    _fixed_sha256(
        provider["qualification_policy_digest_hex"],
        "Bootle/Lantern provider qualification digest",
        required=release,
    )
    policy = _exact_keys(
        bootle["governed_issuer_policy"],
        EXPECTED_ISSUER_POLICY_KEYS,
        "Bootle/Lantern governed issuer policy",
    )
    for field in EXPECTED_ISSUER_POLICY_KEYS:
        _fixed_sha256(policy[field], f"Bootle/Lantern {field}", required=release)
    if not release and any(policy[field] is not None for field in EXPECTED_ISSUER_POLICY_KEYS):
        _fail("staging plan must not contain a partial governed issuer policy")


def _fixed_byte_array_hex(value: Any, label: str, *, length: int = 32) -> str:
    if (
        not isinstance(value, list)
        or len(value) != length
        or any(type(byte) is not int or not 0 <= byte <= 255 for byte in value)
    ):
        _fail(f"{label} must be an exact {length}-byte JSON array")
    return bytes(value).hex()


def _validate_broker_public(
    path: Path | None, plan: dict[str, Any]
) -> dict[str, Any]:
    if path is None:
        _fail("release mode requires --broker-public")
    broker, payload = _load_canonical_compact_json(
        path, "Taira Bootle/Lantern broker public export"
    )
    _exact_keys(broker, EXPECTED_BROKER_PUBLIC_KEYS, "broker public export")
    bootle = plan["bootle_lantern_issuer"]
    provider = bootle["runtime_provider"]
    policy_plan = bootle["governed_issuer_policy"]
    if hashlib.sha256(payload).hexdigest() != bootle["public_export_sha256"]:
        _fail("broker public export digest differs from the checked release plan")
    if (
        broker["schema"] != EXPECTED_BROKER_SCHEMA
        or broker["chain_id"] != EXPECTED_CHAIN_ID
        or broker["runtime_provider_handle"] != EXPECTED_PROVIDER_HANDLE
        or not _exact_integer(broker["runtime_provider_revision"], 1)
        or not _exact_integer(broker["authorization_lifetime_blocks"], 300)
        or broker["issuer_id_hex"] != bootle["issuer_id_hex"]
        or broker["policy_id_hex"] != bootle["policy_id_hex"]
        or broker["runtime_provider_policy_digest_hex"]
        != provider["qualification_policy_digest_hex"]
        or broker["issuer_parameter_id_hex"]
        != policy_plan["issuer_parameter_id_hex"]
        or broker["issuer_parameter_digest_hex"]
        != policy_plan["issuer_parameter_digest_hex"]
        or broker["policy_record_digest_hex"] != policy_plan["record_digest_hex"]
        or broker["registration_instruction_norito_sha256"]
        != policy_plan["instruction_norito_sha256"]
    ):
        _fail("broker public export differs from the checked Taira plan bindings")
    for field in (
        "runtime_provider_policy_digest_hex",
        "issuer_parameter_id_hex",
        "issuer_parameter_digest_hex",
        "policy_record_digest_hex",
        "stable_principal_digest_hex",
        "issuer_profile_digest_hex",
        "broker_contract_digest_hex",
        "registration_instruction_norito_sha256",
    ):
        _fixed_sha256(broker[field], f"broker public export {field}", required=True)

    encoded = broker["registration_instruction_norito_hex"]
    if (
        not isinstance(encoded, str)
        or not encoded
        or len(encoded) % 2
        or re.fullmatch(r"[0-9a-f]+", encoded) is None
    ):
        _fail("broker registration instruction must be nonempty canonical lowercase hex")
    instruction = bytes.fromhex(encoded)
    if hashlib.sha256(instruction).hexdigest() != broker[
        "registration_instruction_norito_sha256"
    ]:
        _fail("broker registration instruction digest does not match its Norito bytes")

    registration = _exact_keys(
        broker["registration_instruction"],
        {"policy"},
        "broker structured registration",
    )
    policy = _exact_keys(
        registration["policy"], EXPECTED_BROKER_POLICY_KEYS, "broker issuer policy"
    )
    if (
        _fixed_byte_array_hex(policy["issuer_id"], "broker issuer id")
        != broker["issuer_id_hex"]
        or _fixed_byte_array_hex(policy["policy_id"], "broker policy id")
        != broker["policy_id_hex"]
        or _fixed_byte_array_hex(
            policy["issuer_parameter_id"], "broker issuer parameter id"
        )
        != broker["issuer_parameter_id_hex"]
        or _fixed_byte_array_hex(
            policy["issuer_parameter_digest"], "broker issuer parameter digest"
        )
        != broker["issuer_parameter_digest_hex"]
        or _fixed_byte_array_hex(policy["record_digest"], "broker policy record digest")
        != broker["policy_record_digest_hex"]
        or not _exact_integer(policy["epoch"], 1)
        or policy["lifecycle"] != {"state": "active", "value": None}
        or not _exact_integer(policy["required_disclosure_bitmap"], 0)
    ):
        _fail("broker structured issuer policy differs from its public bindings")
    allowed_values = policy["allowed_values"]
    if (
        not isinstance(allowed_values, list)
        or len(allowed_values) != 8
        or any(rule != {"values": []} for rule in allowed_values)
    ):
        _fail("broker issuer policy disclosure allowlists differ from first release")
    matrix = _exact_keys(
        policy["issuer_public_matrix"], {"entries"}, "broker issuer public matrix"
    )
    entries = matrix["entries"]
    if not isinstance(entries, list) or len(entries) != 64:
        _fail("broker issuer public matrix must contain exactly 64 polynomials")
    for index, entry in enumerate(entries):
        polynomial = _exact_keys(
            entry, {"coefficients"}, f"broker issuer polynomial {index}"
        )
        coefficients = polynomial["coefficients"]
        if (
            not isinstance(coefficients, list)
            or len(coefficients) != 64
            or any(
                type(coefficient) is not int or not 0 <= coefficient < 12_289
                for coefficient in coefficients
            )
        ):
            _fail(
                f"broker issuer polynomial {index} must contain 64 canonical coefficients"
            )
    return broker


def _validate_config(config: dict[str, Any], plan: dict[str, Any], *, release: bool) -> None:
    if config.get("chain") != plan["chain_id"] or not _exact_integer(
        config.get("chain_discriminant"), 369
    ):
        _fail("Taira config chain identity differs from the privacy plan")
    if (
        "private_key" in config
        or config.get("private_key_file")
        != "/run/secrets/iroha/taira-validator-private-key"
    ):
        _fail("public Taira config must use the validator runtime key file")
    torii = config.get("torii")
    if not isinstance(torii, dict):
        _fail("Taira config is missing [torii]")
    kagemusha = torii.get("kagemusha_commands")
    onboarding = torii.get("account_onboarding")
    faucet = torii.get("faucet")
    credentials = onboarding.get("credentials") if isinstance(onboarding, dict) else None
    streaming = config.get("streaming")
    if (
        config.get("soranet_transport_public_key")
        != "REPLACE_WITH_SORANET_TRANSPORT_PUBLIC_KEY"
        or "soranet_transport_private_key" in config
        or config.get("soranet_transport_private_key_file")
        != "/run/secrets/iroha/taira-soranet-transport-private-key"
        or not isinstance(kagemusha, dict)
        or "private_key" in kagemusha
        or kagemusha.get("private_key_file")
        != "/run/secrets/iroha/taira-kagemusha-commands-private-key"
        or not isinstance(onboarding, dict)
        or onboarding.get("private_key_file")
        != "REPLACE_WITH_TAIRA_ONBOARDING_PRIVATE_KEY_FILE"
        or not isinstance(credentials, list)
        or len(credentials) != 1
        or not isinstance(credentials[0], dict)
        or credentials[0].get("token_hash")
        != "REPLACE_WITH_TAIRA_ONBOARDING_TOKEN_HASH"
        or not isinstance(faucet, dict)
        or faucet.get("private_key_file")
        != "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE"
        or not isinstance(streaming, dict)
        or "identity_private_key" in streaming
        or streaming.get("identity_private_key_file")
        != "/run/secrets/iroha/taira-streaming-identity-private-key"
    ):
        _fail("public Taira config must retain public identities and runtime key-file handles")

    canonical = _load_toml(DEFAULT_CONFIG)
    canonical_torii = canonical.get("torii")
    if not isinstance(canonical_torii, dict):
        _fail("canonical Taira config is missing [torii]")
    if (
        {key: value for key, value in config.items() if key != "torii"}
        != {key: value for key, value in canonical.items() if key != "torii"}
        or {
            key: value
            for key, value in torii.items()
            if key != "privacy_bootle_lantern_issuer"
        }
        != {
            key: value
            for key, value in canonical_torii.items()
            if key != "privacy_bootle_lantern_issuer"
        }
    ):
        _fail(
            "Taira config differs from the canonical first-release template outside "
            "the privacy issuer section"
        )
    issuer = torii.get("privacy_bootle_lantern_issuer")
    if not isinstance(issuer, dict):
        _fail("Taira config is missing [torii.privacy_bootle_lantern_issuer]")
    expected_keys = DISABLED_CONFIG_KEYS | (RELEASE_CONFIG_BINDING_KEYS if release else set())
    if set(issuer) != expected_keys:
        _fail("Bootle/Lantern config contains incomplete, dormant, or unknown bindings")
    if issuer["enabled"] is not release:
        _fail("Bootle/Lantern config must be disabled in staging and enabled in release mode")
    bootle = plan["bootle_lantern_issuer"]
    expected_path = "/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer"
    if (
        issuer["state_dir"] != expected_path
        or not Path(issuer["state_dir"]).is_absolute()
        or not _exact_integer(issuer["max_inflight"], bootle["max_inflight"])
        or not _exact_integer(issuer["authorization_lifetime_blocks"], 300)
        or not _exact_integer(issuer["max_records"], 4096)
        or not _exact_integer(issuer["max_total_bytes"], 13_557_760)
        or issuer["max_total_bytes"] != issuer["max_records"] * 3_310
        or not _exact_integer(issuer["terminal_retention_blocks"], 4096)
    ):
        _fail("Bootle/Lantern store/lifetime bounds differ from canonical Taira V1")
    if not release:
        return
    provider = bootle["runtime_provider"]
    if (
        issuer["issuer_id_hex"] != bootle["issuer_id_hex"]
        or issuer["policy_id_hex"] != bootle["policy_id_hex"]
        or issuer["runtime_provider_registry_handle"] != provider["handle"]
        or issuer["runtime_provider_registry_revision"] != provider["revision"]
        or issuer["runtime_provider_registry_policy_digest_hex"]
        != provider["qualification_policy_digest_hex"]
    ):
        _fail("enabled Bootle/Lantern config does not exactly match the checked public plan")


def _instruction_list(genesis: dict[str, Any]) -> list[Any]:
    transactions = genesis.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        _fail("genesis must contain transactions")
    result: list[Any] = []
    for tx_index, transaction in enumerate(transactions):
        if not isinstance(transaction, dict) or not isinstance(transaction.get("instructions"), list):
            _fail(f"genesis transaction {tx_index} has no instruction array")
        result.extend(transaction["instructions"])
    return result


def _validate_canonical_genesis_base(genesis: dict[str, Any]) -> None:
    canonical = _load_json(DEFAULT_GENESIS, "canonical Taira genesis template")
    if genesis != canonical:
        _fail("genesis differs from the canonical first-release base template")


def _validate_genesis(genesis: dict[str, Any], plan: dict[str, Any], *, release: bool) -> None:
    if genesis.get("chain") != plan["chain_id"] or not _exact_integer(
        genesis.get("chain_discriminant"), 369
    ):
        _fail("genesis chain identity differs from the privacy plan")
    instructions = _instruction_list(genesis)
    authority = plan["genesis_authority"]
    authority_registration_indices: list[int] = []
    governance_grant_indices: list[int] = []
    encoded: list[tuple[int, str]] = []
    for index, instruction in enumerate(instructions):
        if isinstance(instruction, str):
            encoded.append((index, instruction))
            continue
        if not isinstance(instruction, dict):
            _fail(f"genesis instruction {index} must be an object or canonical base64 string")
        if (
            "RegisterPrivacyProtocolActivationV1" in instruction
            or "RegisterPrivacyBootleLanternIssuerPolicyV1" in instruction
        ):
            _fail("genesis must not contain decoded privacy bootstrap rows")
        register = instruction.get("Register")
        if isinstance(register, dict):
            account = register.get("Account")
            if isinstance(account, dict) and account.get("id") == authority:
                authority_registration_indices.append(index)
        grant = instruction.get("Grant")
        if isinstance(grant, dict):
            permission = grant.get("Permission")
            if isinstance(permission, dict):
                obj = permission.get("object")
                if isinstance(obj, dict) and obj.get("name") == "CanEnactGovernance":
                    if set(obj) != {"name"}:
                        _fail("CanEnactGovernance genesis grant must be unscoped")
                    if permission.get("destination") != authority:
                        _fail("CanEnactGovernance genesis grant targets the wrong authority")
                    governance_grant_indices.append(index)
    if len(authority_registration_indices) != 1:
        _fail("genesis must register the privacy governance authority exactly once")
    if len(governance_grant_indices) != 1:
        _fail("genesis must grant CanEnactGovernance to the authority exactly once")
    if authority_registration_indices[0] >= governance_grant_indices[0]:
        _fail("privacy governance authority must be registered before its permission grant")

    if encoded:
        _fail(
            "privacy activation and issuer-policy instructions are forbidden in genesis; "
            "the sealed controller must execute the bound four-wave governance rollout"
        )
    _validate_canonical_genesis_base(genesis)


def validate(paths: ValidationPaths = ValidationPaths(), *, release: bool = False) -> None:
    matrix = _parse_matrix(paths.matrix)
    plan = _load_json(paths.plan, "privacy bootstrap plan")
    config = _load_toml(paths.config)
    genesis = _load_json(paths.genesis, "Taira genesis")
    _validate_plan(plan, matrix, release=release)
    if release:
        _validate_broker_public(paths.broker_public, plan)
    _validate_config(config, plan, release=release)
    _validate_genesis(genesis, plan, release=release)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode", choices=("staging", "release", "auto"), default="staging"
    )
    parser.add_argument("--plan", type=Path, default=DEFAULT_PLAN)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--genesis", type=Path, default=DEFAULT_GENESIS)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument(
        "--broker-public",
        type=Path,
        help="verified canonical broker public export (mandatory in release mode)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    paths = ValidationPaths(
        args.plan, args.config, args.genesis, args.matrix, args.broker_public
    )
    selected_mode = args.mode
    if args.mode == "auto":
        try:
            validate(paths, release=False)
        except PrivacyBootstrapValidationError as staging_error:
            try:
                validate(paths, release=True)
            except PrivacyBootstrapValidationError as release_error:
                print(
                    "Taira privacy bootstrap validation failed in both modes: "
                    f"staging={staging_error}; release={release_error}",
                    file=sys.stderr,
                )
                return 1
            selected_mode = "release"
        else:
            selected_mode = "staging"
    try:
        validate(paths, release=selected_mode == "release")
    except PrivacyBootstrapValidationError as exc:
        print(f"Taira privacy bootstrap validation failed: {exc}", file=sys.stderr)
        return 1
    if selected_mode == "release":
        print(
            "Taira privacy bootstrap public-input inventory is release-bound; "
            "native genesis/profile validation remains mandatory"
        )
    else:
        print("Taira privacy bootstrap staging contract is canonical and remains disabled")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
