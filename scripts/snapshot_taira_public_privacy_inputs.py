#!/usr/bin/env python3
"""Copy the exact secret-free Taira privacy inputs through stable file handles."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any, NoReturn

try:
    from . import compose_taira_nevo_reset_genesis as nevo_composer
except ImportError:
    import compose_taira_nevo_reset_genesis as nevo_composer

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.9/3.10 development and operator hosts.
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:  # The installed controller fails closed below.
        tomllib = None  # type: ignore[assignment]

EXPECTED = {
    "bootle_lantern_broker_public.json": 4 * 1024 * 1024,
    "config.toml": 8 * 1024 * 1024,
    "genesis.json": 16 * 1024 * 1024,
    "nevo-reset.review.json": 4 * 1024 * 1024,
    "privacy_bootstrap_plan.json": 8 * 1024 * 1024,
}
HANDOFF_MANIFEST = "handoff-inventory-v1.json"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
NETWORK_ID_RE = re.compile(r"hash:([0-9A-F]{64})#([0-9A-F]{4})")

# These constants mirror the first-release release-mode contract in
# configs/soranexus/taira/validate_privacy_bootstrap.py.  The installed
# controller cannot import checkout code, so it carries the public validation
# closure needed to reject secret-bearing operator input before upload.
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
CHAIN_DISCRIMINANT = 369
GENESIS_AUTHORITY = (
    "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"
)
MATRIX_FILE_SHA256 = "f75eeba824067aaf903fd8060c967190e37073dc07e487c81c265018a1c00f38"
ROLLOUT_PLAN_SHA256 = "6db9c54ebfa147a57199b02f476929389b0751d59c8387da2e0104d16200ce65"
REGISTRY_SHA256 = "734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51"
ISSUER_ID = hashlib.sha256(
    b"iroha.taira.privacy.bootle-lantern.issuer.v1"
).hexdigest()
POLICY_ID = hashlib.sha256(
    b"iroha.taira.privacy.bootle-lantern.policy.v1"
).hexdigest()
PROVIDER_HANDLE = "runtime://privacy/bootle-lantern/taira-primary"
PUBLIC_ONBOARDING_CREDENTIALS = [
    {
        "id": "REPLACE_WITH_TAIRA_BOI_ONBOARDING_CREDENTIAL_ID",
        "scope": {"dataspace": "is2"},
        "token_hash": "REPLACE_WITH_TAIRA_BOI_ONBOARDING_TOKEN_HASH",
    },
    {
        "id": "REPLACE_WITH_TAIRA_DPN_ONBOARDING_CREDENTIAL_ID",
        "scope": {"dataspace": "dpn"},
        "token_hash": "REPLACE_WITH_TAIRA_DPN_ONBOARDING_TOKEN_HASH",
    },
]
CONFIG_PUBLIC_BASE_SHA256 = (
    "ba0f21fddf6806bc2a40ff3743f3111476141884f8ef104e8392c7fe4760f5e5"
)
PROTOCOLS = (
    (0, "zk-ace-pq-authorization-v0", "ZkAcePqAuthorizationV0"),
    (1, "anonymous-pgc-k-out-of-n-v1", "AnonymousPgcKOutOfNV1"),
    (2, "verange-transparent-range-v1", "VeRangeTransparentRangeV1"),
    (3, "iroha-zk-ams-v1", "IrohaZkAmsV1"),
    (4, "vega-existing-credential-zk-v0", "VegaExistingCredentialZkV0"),
    (5, "iroha-zk-x509-stark-p256-v0", "IrohaZkX509StarkP256V0"),
    (6, "iroha-jindo-polynomial-commitment-v0", "IrohaJindoPolynomialCommitmentV0"),
    (7, "iroha-bootle-lantern-anoncred-v1", "IrohaBootleLanternAnoncredV1"),
    (8, "orchard-halo2-actions-v1", "OrchardHalo2ActionsV1"),
    (9, "monero-fcmp-plus-plus-v1", "MoneroFcmpPlusPlusV1"),
    (10, "iroha-ivm-private-note-stark-v1", "IrohaIvmPrivateNoteStarkV1"),
    (11, "pq-masp-stark-v0", "PqMaspStarkV0"),
)
RETIRED_LABELS = (
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
)


class SnapshotError(RuntimeError):
    """The protected public-input snapshot could not be made safely."""


def _fail(message: str) -> NoReturn:
    raise SnapshotError(message)


def _crc16_ccitt_false(payload: bytes) -> int:
    checksum = 0xFFFF
    for byte in payload:
        checksum ^= byte << 8
        for _ in range(8):
            checksum = (
                ((checksum << 1) ^ 0x1021) & 0xFFFF
                if checksum & 0x8000
                else (checksum << 1) & 0xFFFF
            )
    return checksum


def _canonical_network_id(value: Any, label: str) -> str:
    if not isinstance(value, str):
        _fail(f"{label} must be one canonical checksummed NetworkId")
    match = NETWORK_ID_RE.fullmatch(value)
    if match is None:
        _fail(f"{label} must be one canonical checksummed NetworkId")
    identity = bytes.fromhex(match.group(1))
    prefix = value[:69]
    if identity[-1] & 1 == 0 or _crc16_ccitt_false(prefix.encode("ascii")) != int(
        match.group(2), 16
    ):
        _fail(f"{label} must be one canonical checksummed NetworkId")
    return value


def _file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
    )


def _canonical(path: Path, label: str, *, exists: bool = True) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must use one absolute lexical path")
    if exists:
        try:
            resolved = path.resolve(strict=True)
        except OSError as exc:
            raise SnapshotError(f"cannot resolve {label}: {exc}") from exc
        if resolved != path:
            _fail(f"{label} must use one canonical physical path")
    else:
        parent = path.parent.resolve(strict=True)
        if parent != path.parent:
            _fail(f"{label} parent must use one canonical physical path")
    return path


def _same_inode(left: os.stat_result, right: os.stat_result) -> bool:
    return (left.st_dev, left.st_ino) == (right.st_dev, right.st_ino)


def _write_all(descriptor: int, payload: bytes | bytearray, label: str) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            _fail(f"short {label} write")
        view = view[written:]


def _read_exact(descriptor: int, size: int, label: str) -> bytes:
    payload = bytearray()
    while len(payload) < size:
        chunk = os.read(descriptor, min(64 * 1024, size - len(payload)))
        if not chunk:
            _fail(f"{label} was truncated")
        payload.extend(chunk)
    if os.read(descriptor, 1):
        _fail(f"{label} grew while it was read")
    return bytes(payload)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"public privacy JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _reject_nonfinite_json(value: str) -> NoReturn:
    _fail(f"public privacy JSON contains non-finite number {value!r}")


def _strict_json_object(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonfinite_json,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SnapshotError(f"{label} is not strict UTF-8 JSON") from exc
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def _exact_object(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    if set(value) != expected:
        missing = sorted(expected - set(value))
        extra = sorted(set(value) - expected)
        _fail(f"{label} fields differ: missing={missing}, extra={extra}")
    return value


def _fixed_sha256(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _exact_integer(value: Any, expected: int, label: str) -> None:
    if type(value) is not int or value != expected:
        _fail(f"{label} must be exactly {expected}")


def _semantic_bytes(value: Any, label: str) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise SnapshotError(f"{label} contains a noncanonical value") from exc


def _validate_release_plan(payload: bytes) -> dict[str, Any]:
    plan = _strict_json_object(payload, "privacy bootstrap plan")
    canonical = (
        json.dumps(
            plan,
            ensure_ascii=False,
            sort_keys=True,
            indent=2,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")
    if payload != canonical:
        _fail("privacy bootstrap plan is not in canonical emitted form")
    _exact_object(
        plan,
        {
            "bootle_lantern_issuer",
            "chain_discriminant",
            "chain_id",
            "network_id",
            "genesis_authority",
            "governance_rollout",
            "governance_permission",
            "privacy_catalog",
            "schema",
            "schema_version",
        },
        "privacy bootstrap plan",
    )
    if (
        plan["schema"] != "iroha.taira.privacy_bootstrap_plan.v1"
        or plan["chain_id"] != CHAIN_ID
        or plan["genesis_authority"] != GENESIS_AUTHORITY
        or plan["governance_permission"] != "CanEnactGovernance"
    ):
        _fail("privacy bootstrap plan identity differs from canonical Taira V1")
    _canonical_network_id(plan["network_id"], "privacy bootstrap plan network_id")
    _exact_integer(plan["schema_version"], 1, "privacy plan schema version")
    _exact_integer(
        plan["chain_discriminant"],
        CHAIN_DISCRIMINANT,
        "privacy plan chain discriminant",
    )

    rollout = _exact_object(
        plan["governance_rollout"],
        {
            "activation_state",
            "controller_observation_required",
            "genesis_activation_forbidden",
            "mode",
            "notice_interval_blocks",
            "observation_interval_blocks",
            "rollout_plan_path",
            "rollout_plan_sha256",
        },
        "privacy governance rollout",
    )
    if (
        rollout["activation_state"] != "not-executed"
        or rollout["controller_observation_required"] is not True
        or rollout["genesis_activation_forbidden"] is not True
        or rollout["mode"] != "governance-four-wave"
        or rollout["rollout_plan_path"]
        != "configs/soranexus/taira/privacy_rollout_plan_v1.json"
        or rollout["rollout_plan_sha256"] != ROLLOUT_PLAN_SHA256
    ):
        _fail("privacy governance rollout differs from canonical release V1")
    for field, expected in (
        ("notice_interval_blocks", 300),
        ("observation_interval_blocks", 300),
    ):
        _exact_integer(rollout[field], expected, f"privacy rollout {field}")

    catalog = _exact_object(
        plan["privacy_catalog"],
        {
            "matrix_file_sha256",
            "matrix_version",
            "protocols",
            "registry_sha256",
            "retired_labels",
        },
        "privacy catalog",
    )
    _exact_integer(catalog["matrix_version"], 1, "privacy catalog matrix version")
    expected_protocols = [
        {"index": index, "label": label, "statement_type": statement_type}
        for index, label, statement_type in PROTOCOLS
    ]
    if (
        catalog["matrix_file_sha256"] != MATRIX_FILE_SHA256
        or catalog["registry_sha256"] != REGISTRY_SHA256
        or catalog["protocols"] != expected_protocols
        or catalog["retired_labels"] != list(RETIRED_LABELS)
    ):
        _fail("privacy catalog differs from the exact first-release inventory")
    for index, row in enumerate(catalog["protocols"]):
        _exact_object(row, {"index", "label", "statement_type"}, f"protocol {index}")
        _exact_integer(row["index"], index, f"protocol {index} index")

    bootle = _exact_object(
        plan["bootle_lantern_issuer"],
        {
            "authorization_lifetime_blocks",
            "designated_validator",
            "edge_routing",
            "governed_issuer_policy",
            "issuer_id_hex",
            "max_inflight",
            "policy_id_hex",
            "protocol_label",
            "provider_operations",
            "public_export_sha256",
            "runtime_provider",
            "secret_material_permitted",
            "state_authority",
        },
        "Bootle/Lantern issuer plan",
    )
    if (
        bootle["protocol_label"] != "iroha-bootle-lantern-anoncred-v1"
        or bootle["designated_validator"] != "taira-validator-1"
        or bootle["edge_routing"] != "designated-validator-only"
        or bootle["issuer_id_hex"] != ISSUER_ID
        or bootle["policy_id_hex"] != POLICY_ID
        or bootle["state_authority"] != "torii-local-one-shot-store"
        or bootle["provider_operations"]
        != ["bearer-principal-authentication-v1", "falcon512-native-crypto-v1"]
        or bootle["secret_material_permitted"] is not False
    ):
        _fail("Bootle/Lantern issuer plan is not the exact secret-free V1 contract")
    _exact_integer(
        bootle["authorization_lifetime_blocks"],
        300,
        "Bootle/Lantern authorization lifetime",
    )
    _exact_integer(bootle["max_inflight"], 2, "Bootle/Lantern max inflight")
    _fixed_sha256(bootle["public_export_sha256"], "broker public export digest")

    provider = _exact_object(
        bootle["runtime_provider"],
        {
            "handle",
            "qualification_policy_digest_hex",
            "revision",
            "slot_wire_id",
            "transport",
        },
        "Bootle/Lantern runtime provider",
    )
    if (
        provider["transport"] != "stock-local-runtime-provider-broker-v1"
        or provider["handle"] != PROVIDER_HANDLE
    ):
        _fail("Bootle/Lantern provider identity differs from canonical V1")
    _exact_integer(provider["slot_wire_id"], 56, "runtime provider slot")
    _exact_integer(provider["revision"], 1, "runtime provider revision")
    _fixed_sha256(
        provider["qualification_policy_digest_hex"],
        "runtime provider qualification digest",
    )
    policy = _exact_object(
        bootle["governed_issuer_policy"],
        {
            "instruction_norito_sha256",
            "issuer_parameter_digest_hex",
            "issuer_parameter_id_hex",
            "record_digest_hex",
        },
        "Bootle/Lantern governed issuer policy",
    )
    for field, value in policy.items():
        _fixed_sha256(value, f"governed issuer policy {field}")
    return plan


def _fixed_byte_array_hex(value: Any, label: str) -> str:
    if (
        not isinstance(value, list)
        or len(value) != 32
        or any(type(item) is not int or not 0 <= item <= 255 for item in value)
    ):
        _fail(f"{label} must be an exact 32-byte public array")
    return bytes(value).hex()


def _validate_broker_public(payload: bytes, plan: dict[str, Any]) -> dict[str, Any]:
    broker = _strict_json_object(payload, "Bootle/Lantern broker public export")
    canonical = (
        json.dumps(
            broker,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        + b"\n"
    )
    if payload != canonical:
        _fail("broker public export is not in canonical compact emitted form")
    _exact_object(
        broker,
        {
            "authorization_lifetime_blocks",
            "broker_contract_digest_hex",
            "chain_id",
            "network_id",
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
        },
        "broker public export",
    )
    bootle = plan["bootle_lantern_issuer"]
    provider = bootle["runtime_provider"]
    policy_plan = bootle["governed_issuer_policy"]
    if (
        broker["schema"]
        != "iroha.taira.privacy.bootle-lantern-broker-public.v1"
        or broker["chain_id"] != CHAIN_ID
        or broker["network_id"] != plan["network_id"]
        or broker["runtime_provider_handle"] != PROVIDER_HANDLE
        or broker["issuer_id_hex"] != ISSUER_ID
        or broker["policy_id_hex"] != POLICY_ID
        or broker["runtime_provider_policy_digest_hex"]
        != provider["qualification_policy_digest_hex"]
        or broker["issuer_parameter_id_hex"] != policy_plan["issuer_parameter_id_hex"]
        or broker["issuer_parameter_digest_hex"]
        != policy_plan["issuer_parameter_digest_hex"]
        or broker["policy_record_digest_hex"] != policy_plan["record_digest_hex"]
        or broker["registration_instruction_norito_sha256"]
        != policy_plan["instruction_norito_sha256"]
        or hashlib.sha256(payload).hexdigest() != bootle["public_export_sha256"]
    ):
        _fail("broker public export differs from the checked release plan")
    _canonical_network_id(broker["network_id"], "broker public export network_id")
    _exact_integer(
        broker["runtime_provider_revision"], 1, "broker runtime provider revision"
    )
    _exact_integer(
        broker["authorization_lifetime_blocks"], 300, "broker authorization lifetime"
    )
    for field in (
        "broker_contract_digest_hex",
        "issuer_parameter_digest_hex",
        "issuer_parameter_id_hex",
        "issuer_profile_digest_hex",
        "policy_record_digest_hex",
        "registration_instruction_norito_sha256",
        "runtime_provider_policy_digest_hex",
        "stable_principal_digest_hex",
    ):
        _fixed_sha256(broker[field], f"broker public export {field}")
    encoded = broker["registration_instruction_norito_hex"]
    if (
        not isinstance(encoded, str)
        or not encoded
        or len(encoded) % 2
        or re.fullmatch(r"[0-9a-f]+", encoded) is None
    ):
        _fail("broker registration instruction must be canonical lowercase hex")
    instruction = bytes.fromhex(encoded)
    if hashlib.sha256(instruction).hexdigest() != broker[
        "registration_instruction_norito_sha256"
    ]:
        _fail("broker registration instruction digest differs")

    registration = _exact_object(
        broker["registration_instruction"], {"policy"}, "broker registration"
    )
    policy = _exact_object(
        registration["policy"],
        {
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
        },
        "broker issuer policy",
    )
    if (
        _fixed_byte_array_hex(policy["issuer_id"], "broker issuer id") != ISSUER_ID
        or _fixed_byte_array_hex(policy["policy_id"], "broker policy id") != POLICY_ID
        or _fixed_byte_array_hex(
            policy["issuer_parameter_id"], "broker issuer parameter id"
        )
        != broker["issuer_parameter_id_hex"]
        or _fixed_byte_array_hex(
            policy["issuer_parameter_digest"], "broker issuer parameter digest"
        )
        != broker["issuer_parameter_digest_hex"]
        or _fixed_byte_array_hex(policy["record_digest"], "broker record digest")
        != broker["policy_record_digest_hex"]
        or policy["lifecycle"] != {"state": "active", "value": None}
    ):
        _fail("broker structured policy differs from its public bindings")
    _exact_integer(policy["epoch"], 1, "broker policy epoch")
    _exact_integer(
        policy["required_disclosure_bitmap"], 0, "broker disclosure bitmap"
    )
    allowed = policy["allowed_values"]
    if (
        not isinstance(allowed, list)
        or len(allowed) != 8
        or any(value != {"values": []} for value in allowed)
    ):
        _fail("broker disclosure allowlists differ from first release")
    matrix = _exact_object(
        policy["issuer_public_matrix"], {"entries"}, "broker issuer public matrix"
    )
    entries = matrix["entries"]
    if not isinstance(entries, list) or len(entries) != 64:
        _fail("broker issuer public matrix must contain exactly 64 polynomials")
    for index, entry in enumerate(entries):
        polynomial = _exact_object(
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
            _fail(f"broker issuer polynomial {index} is not canonical public data")
    return broker


def _toml_contains_comment(text: str) -> bool:
    """Return whether TOML contains a comment token outside a quoted string."""

    index = 0
    state = "plain"
    while index < len(text):
        if state == "plain":
            if text.startswith('"""', index):
                state = "multiline-basic"
                index += 3
                continue
            if text.startswith("'''", index):
                state = "multiline-literal"
                index += 3
                continue
            if text[index] == '"':
                state = "basic"
            elif text[index] == "'":
                state = "literal"
            elif text[index] == "#":
                return True
        elif state == "basic":
            if text[index] == "\\":
                index += 2
                continue
            if text[index] == '"':
                state = "plain"
        elif state == "literal":
            if text[index] == "'":
                state = "plain"
        elif state == "multiline-basic":
            if text[index] == "\\":
                index += 2
                continue
            if text.startswith('"""', index):
                state = "plain"
                index += 3
                continue
        elif text.startswith("'''", index):
            state = "plain"
            index += 3
            continue
        index += 1
    return False


def _validate_public_config(payload: bytes, plan: dict[str, Any]) -> None:
    if tomllib is None:
        _fail("secret-free config validation requires Python tomllib support")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise SnapshotError("public Taira config is not strict UTF-8 TOML") from exc
    if (
        "\r" in text
        or "\x00" in text
        or _toml_contains_comment(text)
        or not text.endswith("\n")
        or text.endswith("\n\n")
        or any(line.endswith((" ", "\t")) for line in text.splitlines())
    ):
        _fail("public Taira config is not in canonical comment-free emitted form")
    try:
        config = tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:  # type: ignore[union-attr]
        raise SnapshotError("public Taira config is not strict TOML") from exc
    if not isinstance(config, dict):
        _fail("public Taira config root must be a table")
    torii = config.get("torii")
    streaming = config.get("streaming")
    if not isinstance(torii, dict) or not isinstance(streaming, dict):
        _fail("public Taira config lacks canonical torii/streaming tables")
    kagemusha = torii.get("kagemusha_commands")
    onboarding = torii.get("account_onboarding")
    faucet = torii.get("faucet")
    credentials = onboarding.get("credentials") if isinstance(onboarding, dict) else None
    if (
        "private_key" in config
        or config.get("private_key_file")
        != "/run/secrets/iroha/taira-validator-private-key"
        or config.get("soranet_transport_public_key")
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
        or credentials != PUBLIC_ONBOARDING_CREDENTIALS
        or not isinstance(faucet, dict)
        or faucet.get("private_key_file")
        != "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE"
        or "identity_private_key" in streaming
        or streaming.get("identity_private_key_file")
        != "/run/secrets/iroha/taira-streaming-identity-private-key"
    ):
        _fail(
            "public Taira config materializes or replaces a required runtime-identity, "
            "private-key, token, or reviewed runtime key-file handle"
        )
    base = {
        "root_without_torii": {
            key: value for key, value in config.items() if key != "torii"
        },
        "torii_without_privacy_issuer": {
            key: value
            for key, value in torii.items()
            if key != "privacy_bootle_lantern_issuer"
        },
    }
    if hashlib.sha256(_semantic_bytes(base, "public config projection")).hexdigest() != (
        CONFIG_PUBLIC_BASE_SHA256
    ):
        _fail("public Taira config differs from the canonical secret-free template")
    issuer = torii.get("privacy_bootle_lantern_issuer")
    bootle = plan["bootle_lantern_issuer"]
    provider = bootle["runtime_provider"]
    expected_issuer = {
        "authorization_lifetime_blocks": 300,
        "enabled": True,
        "issuer_id_hex": ISSUER_ID,
        "max_inflight": 2,
        "max_records": 4096,
        "max_total_bytes": 13_557_760,
        "policy_id_hex": POLICY_ID,
        "runtime_provider_registry_handle": PROVIDER_HANDLE,
        "runtime_provider_registry_policy_digest_hex": provider[
            "qualification_policy_digest_hex"
        ],
        "runtime_provider_registry_revision": 1,
        "state_dir": "/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer",
        "terminal_retention_blocks": 4096,
    }
    if _semantic_bytes(issuer, "public issuer config") != _semantic_bytes(
        expected_issuer, "expected public issuer config"
    ):
        _fail("public Taira issuer config has unknown, secret, or noncanonical fields")


def _validate_public_genesis(
    payload: bytes, _plan: dict[str, Any], _broker: dict[str, Any]
) -> None:
    genesis = _strict_json_object(payload, "public Taira genesis")
    if (
        b"\r" in payload
        or not payload.startswith(b"{\n")
        or not payload.endswith(b"\n")
    ):
        _fail("public Taira genesis is not in canonical emitted envelope form")
    if genesis.get("chain") != CHAIN_ID:
        _fail("public Taira genesis targets the wrong chain")
    _exact_integer(
        genesis.get("chain_discriminant"),
        CHAIN_DISCRIMINANT,
        "genesis chain discriminant",
    )
    transactions = genesis.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        _fail("public Taira genesis has no transaction inventory")
    for index, transaction in enumerate(transactions):
        if not isinstance(transaction, dict) or not isinstance(
            transaction.get("instructions"), list
        ):
            _fail(f"public genesis transaction {index} has no instruction array")
    if any(
        isinstance(instruction, str)
        for transaction in transactions
        for instruction in transaction["instructions"]
    ):
        _fail(
            "public release genesis must not schedule encoded privacy activation or "
            "issuer-policy instructions"
        )
def _validate_secret_free_projection(payloads: dict[str, bytes]) -> None:
    if set(payloads) != set(EXPECTED):
        _fail("public privacy projection inventory differs")
    plan = _validate_release_plan(payloads["privacy_bootstrap_plan.json"])
    broker = _validate_broker_public(
        payloads["bootle_lantern_broker_public.json"], plan
    )
    _validate_public_config(payloads["config.toml"], plan)
    _validate_public_genesis(payloads["genesis.json"], plan, broker)
    base_genesis = nevo_composer._read_bounded_regular(
        nevo_composer.CHECKED_IN_TAIRA_GENESIS,
        nevo_composer.MAX_BASE_GENESIS_BYTES,
        "sealed canonical Taira genesis",
    )
    base_config_path = (
        nevo_composer.REPO_ROOT / "configs/soranexus/taira/config.toml"
    )
    base_config = nevo_composer._read_bounded_regular(
        base_config_path,
        nevo_composer.MAX_BASE_CONFIG_BYTES,
        "sealed canonical Taira config",
    )
    try:
        nevo_composer.verify_reviewed_payloads(
            unsigned_genesis_bytes=payloads["genesis.json"],
            review_bytes=payloads["nevo-reset.review.json"],
            base_genesis_bytes=base_genesis,
            base_config_bytes=base_config,
        )
    except nevo_composer.CompositionError as exc:
        raise SnapshotError(f"reviewed NEVO genesis is invalid: {exc}") from exc


def snapshot(source: Path, output: Path, forbidden_root: Path) -> dict[str, object]:
    source = _canonical(source, "public privacy source")
    output = _canonical(output, "public privacy output", exists=False)
    forbidden_root = _canonical(forbidden_root, "untrusted workspace")
    if source == forbidden_root or forbidden_root in source.parents:
        _fail("public privacy source must be provisioned outside the checkout")
    if output == forbidden_root or forbidden_root in output.parents:
        _fail("public privacy output must be staged outside the checkout")
    expected_uid = os.geteuid()
    expected_gid = os.getegid()
    source_before = source.lstat()
    if (
        not stat.S_ISDIR(source_before.st_mode)
        or stat.S_ISLNK(source_before.st_mode)
        or source_before.st_uid != expected_uid
        or source_before.st_gid != expected_gid
        or stat.S_IMODE(source_before.st_mode) != 0o700
    ):
        _fail("public privacy source must be owner-held exact mode 0700")

    read_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    directory_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    source_fd = -1
    parent_fd = -1
    output_fd = -1
    inputs_fd = -1
    committed = False
    rows: list[dict[str, object]] = []
    captured_sources: dict[str, tuple[int, ...]] = {}
    captured_payloads: dict[str, bytes] = {}
    captured_outputs: dict[str, tuple[tuple[int, ...], str, int]] = {}
    try:
        source_fd = os.open(source, directory_flags)
        if _directory_identity(os.fstat(source_fd)) != _directory_identity(source_before):
            _fail("public privacy source changed while opening")
        if sorted(os.listdir(source_fd)) != sorted(EXPECTED):
            _fail("public privacy source inventory differs")

        for name, maximum in sorted(EXPECTED.items()):
            before = os.stat(name, dir_fd=source_fd, follow_symlinks=False)
            if (
                not stat.S_ISREG(before.st_mode)
                or before.st_nlink != 1
                or before.st_uid != expected_uid
                or before.st_gid != expected_gid
                or stat.S_IMODE(before.st_mode) != 0o600
                or before.st_size <= 0
                or before.st_size > maximum
            ):
                _fail(f"public privacy input identity is unsafe: {name}")
            descriptor = os.open(name, read_flags, dir_fd=source_fd)
            try:
                opened = os.fstat(descriptor)
                if _file_identity(opened) != _file_identity(before):
                    _fail(f"public privacy input changed while opening: {name}")
                payload = _read_exact(
                    descriptor,
                    before.st_size,
                    f"public privacy input {name}",
                )
                if _file_identity(os.fstat(descriptor)) != _file_identity(before):
                    _fail(f"public privacy input changed while reading: {name}")
            finally:
                os.close(descriptor)
            if _file_identity(
                os.stat(name, dir_fd=source_fd, follow_symlinks=False)
            ) != _file_identity(before):
                _fail(f"public privacy input path changed: {name}")
            captured_sources[name] = _file_identity(before)
            captured_payloads[name] = payload

        if sorted(os.listdir(source_fd)) != sorted(EXPECTED):
            _fail("public privacy source inventory changed before validation")
        for name, expected_identity in captured_sources.items():
            if _file_identity(
                os.stat(name, dir_fd=source_fd, follow_symlinks=False)
            ) != expected_identity:
                _fail(f"public privacy input changed before validation: {name}")

        # This is deliberately before opening the output parent or creating a
        # handoff inode.  Secret-bearing or malformed input must leave no
        # publishable partial path at all.
        _validate_secret_free_projection(captured_payloads)
        if sorted(os.listdir(source_fd)) != sorted(EXPECTED):
            _fail("public privacy source inventory changed during validation")
        for name, expected_identity in captured_sources.items():
            if _file_identity(
                os.stat(name, dir_fd=source_fd, follow_symlinks=False)
            ) != expected_identity:
                _fail(f"public privacy input changed during validation: {name}")

        parent_before = output.parent.lstat()
        parent_fd = os.open(output.parent, directory_flags)
        if _directory_identity(os.fstat(parent_fd)) != _directory_identity(parent_before):
            _fail("public privacy output parent changed while opening")
        os.mkdir(output.name, mode=0o700, dir_fd=parent_fd)
        os.fsync(parent_fd)
        output_before = os.stat(output.name, dir_fd=parent_fd, follow_symlinks=False)
        output_fd = os.open(output.name, directory_flags, dir_fd=parent_fd)
        output_opened = os.fstat(output_fd)
        if (
            not stat.S_ISDIR(output_opened.st_mode)
            or not _same_inode(output_before, output_opened)
            or output_opened.st_uid != expected_uid
            or output_opened.st_gid != expected_gid
        ):
            _fail("public privacy handoff root changed while opening")
        os.fchmod(output_fd, 0o755)
        os.mkdir("inputs", mode=0o700, dir_fd=output_fd)
        inputs_before = os.stat("inputs", dir_fd=output_fd, follow_symlinks=False)
        inputs_fd = os.open("inputs", directory_flags, dir_fd=output_fd)
        inputs_opened = os.fstat(inputs_fd)
        if (
            not stat.S_ISDIR(inputs_opened.st_mode)
            or not _same_inode(inputs_before, inputs_opened)
            or inputs_opened.st_uid != expected_uid
            or inputs_opened.st_gid != expected_gid
        ):
            _fail("public privacy handoff inputs changed while opening")
        os.fchmod(inputs_fd, 0o755)
        parent_after_create = os.fstat(parent_fd)

        for name in sorted(EXPECTED):
            payload = captured_payloads[name]
            output_descriptor = os.open(
                name,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | os.O_CLOEXEC
                | getattr(os, "O_NOFOLLOW", 0),
                0o444,
                dir_fd=inputs_fd,
            )
            try:
                _write_all(output_descriptor, payload, f"public privacy output {name}")
                os.fchmod(output_descriptor, 0o444)
                os.fsync(output_descriptor)
                installed = os.fstat(output_descriptor)
                if (
                    not stat.S_ISREG(installed.st_mode)
                    or installed.st_nlink != 1
                    or installed.st_uid != expected_uid
                    or installed.st_gid != expected_gid
                    or stat.S_IMODE(installed.st_mode) != 0o444
                    or installed.st_size != len(payload)
                ):
                    _fail(f"public privacy output identity is unsafe: {name}")
            finally:
                os.close(output_descriptor)
            named_output = os.stat(name, dir_fd=inputs_fd, follow_symlinks=False)
            if _file_identity(named_output) != _file_identity(installed):
                _fail(f"public privacy output path changed: {name}")
            digest = hashlib.sha256(payload).hexdigest()
            captured_outputs[name] = (_file_identity(installed), digest, len(payload))
            rows.append(
                {
                    "path": f"inputs/{name}",
                    "sha256": digest,
                    "size": len(payload),
                }
            )
        os.fsync(inputs_fd)
        handoff = {
            "files": rows,
            "kind": "public-privacy-input",
            "schema": "iroha.taira.release_handoff",
            "schema_version": 1,
        }
        manifest_payload = (
            json.dumps(
                handoff,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
        manifest_descriptor = os.open(
            HANDOFF_MANIFEST,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | os.O_CLOEXEC
            | getattr(os, "O_NOFOLLOW", 0),
            0o444,
            dir_fd=output_fd,
        )
        try:
            _write_all(
                manifest_descriptor,
                manifest_payload,
                "public privacy handoff manifest",
            )
            os.fchmod(manifest_descriptor, 0o444)
            os.fsync(manifest_descriptor)
            manifest_installed = os.fstat(manifest_descriptor)
        finally:
            os.close(manifest_descriptor)
        if (
            not stat.S_ISREG(manifest_installed.st_mode)
            or manifest_installed.st_nlink != 1
            or manifest_installed.st_uid != expected_uid
            or manifest_installed.st_gid != expected_gid
            or stat.S_IMODE(manifest_installed.st_mode) != 0o444
            or manifest_installed.st_size != len(manifest_payload)
            or _file_identity(
                os.stat(HANDOFF_MANIFEST, dir_fd=output_fd, follow_symlinks=False)
            )
            != _file_identity(manifest_installed)
        ):
            _fail("public privacy handoff manifest identity is unsafe")
        if sorted(os.listdir(inputs_fd)) != sorted(EXPECTED):
            _fail("public privacy output inventory changed during snapshot")
        if sorted(os.listdir(output_fd)) != [HANDOFF_MANIFEST, "inputs"]:
            _fail("public privacy handoff inventory is not exactly closed")
        if sorted(os.listdir(source_fd)) != sorted(EXPECTED):
            _fail("public privacy source inventory changed during snapshot")
        for name, expected_identity in captured_sources.items():
            if _file_identity(
                os.stat(name, dir_fd=source_fd, follow_symlinks=False)
            ) != expected_identity:
                _fail(f"public privacy input changed after snapshot: {name}")

        # Freeze the two root-owned directories through their pinned
        # descriptors, then prove that the public path still names those exact
        # inodes.  A same-UID process controlling RUNNER_TEMP may rename a
        # root-owned child entry, so path-based chmod alone is not sufficient.
        os.fchmod(inputs_fd, 0o555)
        os.fsync(inputs_fd)
        os.fchmod(output_fd, 0o555)
        os.fsync(output_fd)
        os.fsync(parent_fd)
        output_final = os.fstat(output_fd)
        inputs_final = os.fstat(inputs_fd)
        named_output_final = os.stat(
            output.name, dir_fd=parent_fd, follow_symlinks=False
        )
        named_inputs_final = os.stat(
            "inputs", dir_fd=output_fd, follow_symlinks=False
        )
        if (
            not _same_inode(output_final, named_output_final)
            or output_final.st_uid != expected_uid
            or output_final.st_gid != expected_gid
            or stat.S_IMODE(output_final.st_mode) != 0o555
            or not _same_inode(inputs_final, named_inputs_final)
            or inputs_final.st_uid != expected_uid
            or inputs_final.st_gid != expected_gid
            or stat.S_IMODE(inputs_final.st_mode) != 0o555
            or _directory_identity(os.fstat(parent_fd))
            != _directory_identity(parent_after_create)
        ):
            _fail("public privacy handoff root changed before freeze")
        reopened_output = os.open(output.name, directory_flags, dir_fd=parent_fd)
        reopened_inputs = os.open("inputs", directory_flags, dir_fd=output_fd)
        try:
            if (
                _directory_identity(os.fstat(reopened_output))
                != _directory_identity(output_final)
                or _directory_identity(os.fstat(reopened_inputs))
                != _directory_identity(inputs_final)
            ):
                _fail("public privacy handoff path changed after freeze")
        finally:
            os.close(reopened_inputs)
            os.close(reopened_output)

        for name, (expected_identity, expected_digest, expected_size) in (
            captured_outputs.items()
        ):
            named = os.stat(name, dir_fd=inputs_fd, follow_symlinks=False)
            if _file_identity(named) != expected_identity:
                _fail(f"public privacy output changed before replay: {name}")
            descriptor = os.open(name, read_flags, dir_fd=inputs_fd)
            try:
                opened = os.fstat(descriptor)
                if _file_identity(opened) != expected_identity:
                    _fail(f"public privacy output changed while reopening: {name}")
                replay = _read_exact(
                    descriptor,
                    expected_size,
                    f"public privacy output {name}",
                )
                if (
                    _file_identity(os.fstat(descriptor)) != expected_identity
                    or hashlib.sha256(replay).hexdigest() != expected_digest
                ):
                    _fail(f"public privacy output changed during replay: {name}")
            finally:
                os.close(descriptor)
            if _file_identity(
                os.stat(name, dir_fd=inputs_fd, follow_symlinks=False)
            ) != expected_identity:
                _fail(f"public privacy output path changed after replay: {name}")
        committed = True
        return handoff
    finally:
        if not committed:
            for descriptor in (inputs_fd, output_fd):
                if descriptor >= 0:
                    try:
                        os.fchmod(descriptor, 0o700)
                    except OSError:
                        pass
        for descriptor in (inputs_fd, output_fd, parent_fd, source_fd):
            if descriptor >= 0:
                os.close(descriptor)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--source", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--forbidden-root", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = snapshot(
            Path(args.source), Path(args.output), Path(args.forbidden_root)
        )
    except (OSError, SnapshotError) as exc:
        print(f"Taira public privacy snapshot error: {exc}", file=sys.stderr)
        return 1
    sys.stdout.write(
        json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
