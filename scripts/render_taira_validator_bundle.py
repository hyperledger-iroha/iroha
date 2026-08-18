#!/usr/bin/env python3
"""Render per-validator Taira config bundles from Taira roster material."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
from dataclasses import dataclass
from ipaddress import ip_address
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

try:
    from scripts import taira_constants
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_constants


DEFAULT_NETWORK_ADDRESS = "0.0.0.0:1337"
DEFAULT_TORII_ADDRESS = "0.0.0.0:18080"
DEFAULT_INSTALL_ROOT = Path("/etc/iroha/taira-validator")
KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH = Path("policy/release-policy-v1.norito")
KAGEMUSHA_ARTIFACT_RELATIVE_PATH = Path("catalog")
KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH = Path(
    "seals/catalog-qualification-v1.norito"
)
KAGEMUSHA_MAX_DECODED_BYTES = 256 * 1024 * 1024
KAGEMUSHA_MANAGED_CONFIG_KEYS = (
    "kagemusha_release_policy_path",
    "kagemusha_artifact_dir",
    "kagemusha_catalog_qualification_seal_path",
)
MIN_VALIDATORS = 4
# Mirrors `iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT`.
MAX_VALIDATORS = 31
# Rust's TOML representation and the canonical config parser admit signed
# 64-bit integers only.
TOML_I64_MAX = (1 << 63) - 1
# Mirrors the Rust default, which reserves five positions per protocol-maximum
# validator, three per default authenticated non-validator source, and two for
# anonymous traffic.
SUMERAGI_DEFAULT_BODY_CAPACITY = 5 * MAX_VALIDATORS + 3 * 2 + 2
# A syntactically valid, marker-bearing Iroha hash used only by the private
# pre-signing render. The external signer replaces it with the signed genesis
# hash before any runtime bundle is published.
GENESIS_EXPECTED_HASH_PLACEHOLDER = (
    "7a5823b7ebd34d7599807390890cf20c1d37072949641dca62f83c14fb4347cd"
)
GENESIS_EXPECTED_HASH_RE = re.compile(r"[0-9a-f]{64}")
TAIRA_CHAIN_DISCRIMINANT = taira_constants.CHAIN_DISCRIMINANT
MIB = 1024 * 1024
# First-release privacy admission permits one 9 MiB action per 10 MiB
# transaction. Revision 4 caps the complete canonical consensus payload at
# 16 MiB, leaving 6 MiB for canonical block framing and context attachments
# when one maximum transaction is present.
TAIRA_PRIVACY_MAX_ACTION_BYTES = 9 * MIB
TAIRA_TRANSACTION_MAX_BYTES = 10 * MIB
TAIRA_BLOCK_MAX_PAYLOAD_BYTES = 16 * MIB
TAIRA_PRIVACY_ISSUER_DESIGNATED_VALIDATOR = "taira-validator-1"
TAIRA_PRIVACY_ISSUER_SECTION = "[torii.privacy_bootle_lantern_issuer]"
TAIRA_PRIVACY_ISSUER_BASE_FIELDS = {
    "enabled",
    "state_dir",
    "max_inflight",
    "authorization_lifetime_blocks",
    "max_records",
    "max_total_bytes",
    "terminal_retention_blocks",
}
TAIRA_PRIVACY_ISSUER_BINDING_FIELDS = {
    "issuer_id_hex",
    "policy_id_hex",
    "runtime_provider_registry_handle",
    "runtime_provider_registry_revision",
    "runtime_provider_registry_policy_digest_hex",
}
# Sumeragi isolates an ordinary body envelope, a completion envelope with the
# recommended 1,024-hash manifest, and one timeout vote for every source.
SUMERAGI_BODY_ENVELOPE_HEADROOM_BYTES = 64 * 1024
SUMERAGI_RECOMMENDED_MANIFEST_WIRE_BYTES = 8 + 1024 * 33
SUMERAGI_TIMEOUT_VOTE_RESERVE_BYTES = 64 * 1024
TAIRA_BODY_SOURCE_MIN_BYTES = (
    2 * (TAIRA_BLOCK_MAX_PAYLOAD_BYTES + SUMERAGI_BODY_ENVELOPE_HEADROOM_BYTES)
    + SUMERAGI_RECOMMENDED_MANIFEST_WIRE_BYTES
    + SUMERAGI_TIMEOUT_VOTE_RESERVE_BYTES
)
# Preserve the reviewed whole-MiB deployment margin above the exact minimum.
TAIRA_BODY_SOURCE_BYTES = ((TAIRA_BODY_SOURCE_MIN_BYTES + MIB - 1) // MIB) * MIB
# Exact completion/P2P geometry is checked by the node at height activation.
# The deployment rounds its maximum block-sync plaintext frame to the next MiB
# and its maximum 10 MiB transaction frame to the next MiB. The global cap adds
# the first-release ChaCha20-Poly1305 nonce/tag to the block-sync ceiling.
TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES = 11 * MIB
TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES = 22 * MIB
TAIRA_AEAD_FRAME_OVERHEAD_BYTES = 12 + 16
TAIRA_MAX_FRAME_BYTES = (
    TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES + TAIRA_AEAD_FRAME_OVERHEAD_BYTES
)
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
I105_ALPHABET = tuple(BASE58_ALPHABET) + (
    "ｲ",
    "ﾛ",
    "ﾊ",
    "ﾆ",
    "ﾎ",
    "ﾍ",
    "ﾄ",
    "ﾁ",
    "ﾘ",
    "ﾇ",
    "ﾙ",
    "ｦ",
    "ﾜ",
    "ｶ",
    "ﾖ",
    "ﾀ",
    "ﾚ",
    "ｿ",
    "ﾂ",
    "ﾈ",
    "ﾅ",
    "ﾗ",
    "ﾑ",
    "ｳ",
    "ヰ",
    "ﾉ",
    "ｵ",
    "ｸ",
    "ﾔ",
    "ﾏ",
    "ｹ",
    "ﾌ",
    "ｺ",
    "ｴ",
    "ﾃ",
    "ｱ",
    "ｻ",
    "ｷ",
    "ﾕ",
    "ﾒ",
    "ﾐ",
    "ｼ",
    "ヱ",
    "ﾋ",
    "ﾓ",
    "ｾ",
    "ｽ",
)
I105_INDEX = {symbol: index for index, symbol in enumerate(I105_ALPHABET)}
I105_CHECKSUM_LEN = 6
I105_BECH32M_CONST = 0x2BC830A3
RECEIPT_PUBLIC_KEY_RE = re.compile(r"e70121(?:02|03)[0-9A-F]{64}")
RECEIPT_PRIVATE_KEY_RE = re.compile(r"812620[0-9A-F]{64}")
RECEIPT_PUBLIC_KEY_PREFIX = "e70121"
RECEIPT_PRIVATE_KEY_PREFIX = "812620"
RECEIPT_NODE_ID_DOMAIN = b"iroha.taira.receipt-signer.node-id.v1\x00"
RECEIPT_NODE_ID_PREFIX = "taira-node:receipt-signer:secp256k1:sha256:"
SECP256K1_FIELD_MODULUS = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
SECP256K1_GROUP_ORDER = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
SECP256K1_GENERATOR = (
    0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
    0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8,
)


@dataclass(frozen=True)
class ValidatorEntry:
    """Validator-specific material for a rendered Taira config."""

    slug: str
    account_id: str
    public_key: str
    private_key: str
    soranet_transport_public_key: str
    soranet_transport_private_key: str
    receipt_public_key: str | None
    receipt_private_key: str | None
    receipt_node_id: str | None
    pop_hex: str
    public_address: str
    network_address: str
    torii_address: str
    torii_public_address: str


@dataclass(frozen=True)
class RosterDefaults:
    """Shared defaults applied to validator entries."""

    network_address: str
    torii_address: str
    torii_public_address: str | None


@dataclass(frozen=True)
class SharedSecrets:
    """Runtime-only shared secret material injected into rendered configs."""

    account_onboarding_authority: str | None = None
    account_onboarding_private_key: str | None = None
    account_onboarding_api_token: str | None = None
    account_onboarding_credential_id: str | None = None
    account_onboarding_scope_domain: str | None = None
    account_onboarding_scope_dataspace: str | None = None
    torii_faucet_authority: str | None = None
    torii_faucet_private_key: str | None = None
    kagemusha_commands_private_key: str | None = None
    soracloud_runtime_signer_handle: str | None = None
    soracloud_runtime_signer_authority: str | None = None
    soracloud_runtime_signer_algorithm: str | None = None
    soracloud_runtime_signer_public_key_hex: str | None = None
    soracloud_runtime_signer_revision: int | None = None
    soracloud_runtime_signer_policy_digest_hex: str | None = None
    streaming_identity_public_key: str | None = None
    streaming_identity_private_key: str | None = None
    sorafs_council_public_keys: tuple[str, ...] = ()
    sorafs_council_signature_threshold: int | None = None


@dataclass(frozen=True)
class ValidatorSecrets:
    """Runtime-only, validator-local signing material."""

    private_key: str
    soranet_transport_public_key: str
    soranet_transport_private_key: str
    receipt_public_key: str
    receipt_private_key: str
    receipt_node_id: str


@dataclass(frozen=True)
class SecretMaterial:
    """User-local validator and shared secrets used during rendering."""

    validators: dict[str, ValidatorSecrets]
    shared: SharedSecrets


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        import tomllib
    except ModuleNotFoundError:
        try:
            import tomli as tomllib
        except ModuleNotFoundError as error:  # pragma: no cover - environment specific
            raise SystemExit(
                "python3 must provide tomllib (Python 3.11+) or tomli to load roster TOML"
            ) from error

    with path.open("rb") as handle:
        payload = tomllib.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a top-level TOML table")
    return payload


def _is_strong_lower_hex_digest(value: Any) -> bool:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        return False
    return len(set(bytes.fromhex(value))) >= 8


def _validate_privacy_issuer_template(
    template: dict[str, Any], validators: list[ValidatorEntry]
) -> None:
    """Reject partial or fleet-wide first-release issuer configuration."""

    torii = template.get("torii")
    if not isinstance(torii, dict):
        return
    issuer = torii.get("privacy_bootle_lantern_issuer")
    if issuer is None:
        return
    if not isinstance(issuer, dict):
        raise ValueError(
            "config template `[torii.privacy_bootle_lantern_issuer]` must be a table"
        )
    enabled = issuer.get("enabled")
    if type(enabled) is not bool:
        raise ValueError(
            "config template privacy issuer `enabled` must be exactly boolean"
        )
    expected_fields = TAIRA_PRIVACY_ISSUER_BASE_FIELDS | (
        TAIRA_PRIVACY_ISSUER_BINDING_FIELDS if enabled else set()
    )
    if set(issuer) != expected_fields:
        raise ValueError(
            "config template privacy issuer contains partial, dormant, or unknown bindings"
        )
    expected_state_dir = (
        "/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer"
    )
    expected_bounds = {
        "state_dir": expected_state_dir,
        "max_inflight": 2,
        "authorization_lifetime_blocks": 300,
        "max_records": 4096,
        "max_total_bytes": 13_557_760,
        "terminal_retention_blocks": 4096,
    }
    for field, expected in expected_bounds.items():
        if type(issuer.get(field)) is not type(expected) or issuer[field] != expected:
            raise ValueError(
                f"config template privacy issuer `{field}` must be exactly {expected!r}"
            )
    if not enabled:
        return
    if (
        sum(
            validator.slug == TAIRA_PRIVACY_ISSUER_DESIGNATED_VALIDATOR
            for validator in validators
        )
        != 1
    ):
        raise ValueError(
            "enabled privacy issuer requires exactly one taira-validator-1 roster entry"
        )
    for field in (
        "issuer_id_hex",
        "policy_id_hex",
        "runtime_provider_registry_policy_digest_hex",
    ):
        if not _is_strong_lower_hex_digest(issuer.get(field)):
            raise ValueError(
                f"config template privacy issuer `{field}` must be a strong lowercase digest"
            )
    if (
        issuer.get("runtime_provider_registry_handle")
        != "runtime://privacy/bootle-lantern/taira-primary"
        or type(issuer.get("runtime_provider_registry_revision")) is not int
        or issuer["runtime_provider_registry_revision"] != 1
    ):
        raise ValueError(
            "config template privacy issuer provider binding differs from Taira V1"
        )


def _validate_receipt_signer_template(template: dict[str, Any]) -> None:
    """Keep validator-specific receipt material out of the shared template."""

    torii = template.get("torii")
    if not isinstance(torii, dict):
        raise ValueError("config template must define a `[torii]` table")
    planted = sorted(
        {"receipt_public_key", "receipt_private_key"}.intersection(torii)
    )
    if planted:
        raise ValueError(
            "config template must not contain validator receipt signer material: "
            + ", ".join(planted)
        )


def _require_string(payload: dict[str, Any], key: str, context: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} field `{key}` must be a non-empty string")
    return value.strip()


def _require_positive_integer(payload: dict[str, Any], key: str, context: str) -> int:
    value = payload.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ValueError(f"{context} field `{key}` must be a positive integer")
    if value > TOML_I64_MAX:
        raise ValueError(
            f"{context} field `{key}` must not exceed {TOML_I64_MAX}, "
            "the Rust/TOML signed 64-bit integer maximum"
        )
    return value


def _scaled_sumeragi_body_bytes(template: dict[str, Any], validator_count: int) -> int:
    """Return an aggregate ingress budget isolating every configured source."""

    sumeragi = template.get("sumeragi")
    if not isinstance(sumeragi, dict):
        raise ValueError("config template must define a `[sumeragi]` table")
    block = sumeragi.get("block")
    if not isinstance(block, dict):
        raise ValueError("config template must define a `[sumeragi.block]` table")
    block_context = "config template `[sumeragi.block]`"
    max_payload_bytes = _require_positive_integer(
        block, "max_payload_bytes", block_context
    )
    if max_payload_bytes != TAIRA_BLOCK_MAX_PAYLOAD_BYTES:
        raise ValueError(
            f"{block_context} field `max_payload_bytes` must equal the "
            f"revision-4 protocol ceiling of {TAIRA_BLOCK_MAX_PAYLOAD_BYTES} bytes"
        )
    queues = sumeragi.get("queues")
    if not isinstance(queues, dict):
        raise ValueError("config template must define a `[sumeragi.queues]` table")
    context = "config template `[sumeragi.queues]`"
    configured = _require_positive_integer(queues, "body_bytes", context)
    source_bytes = _require_positive_integer(queues, "body_source_bytes", context)
    authenticated_non_validator_sources = _require_positive_integer(
        queues, "authenticated_non_validator_sources", context
    )
    minimum_source_bytes = (
        2 * (max_payload_bytes + SUMERAGI_BODY_ENVELOPE_HEADROOM_BYTES)
        + SUMERAGI_RECOMMENDED_MANIFEST_WIRE_BYTES
        + SUMERAGI_TIMEOUT_VOTE_RESERVE_BYTES
    )
    if source_bytes < minimum_source_bytes:
        raise ValueError(
            f"{context} field `body_source_bytes` must be at least "
            f"{minimum_source_bytes} bytes for the configured block payload"
        )
    network = template.get("network")
    if not isinstance(network, dict):
        raise ValueError("config template must define a `[network]` table")
    network_context = "config template `[network]`"
    max_frame_bytes = _require_positive_integer(
        network, "max_frame_bytes", network_context
    )
    max_frame_bytes_block_sync = _require_positive_integer(
        network, "max_frame_bytes_block_sync", network_context
    )
    max_frame_bytes_tx_gossip = _require_positive_integer(
        network, "max_frame_bytes_tx_gossip", network_context
    )
    if max_frame_bytes_tx_gossip < TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES:
        raise ValueError(
            f"{network_context} field `max_frame_bytes_tx_gossip` must be at "
            f"least {TAIRA_TX_GOSSIP_PLAINTEXT_FRAME_BYTES} bytes"
        )
    if max_frame_bytes_block_sync < TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES:
        raise ValueError(
            f"{network_context} field `max_frame_bytes_block_sync` must be at "
            f"least {TAIRA_BLOCK_SYNC_PLAINTEXT_FRAME_BYTES} bytes"
        )
    if max_frame_bytes < max_frame_bytes_block_sync + TAIRA_AEAD_FRAME_OVERHEAD_BYTES:
        raise ValueError(
            f"{network_context} field `max_frame_bytes` must include "
            f"{TAIRA_AEAD_FRAME_OVERHEAD_BYTES} AEAD bytes beyond "
            "`max_frame_bytes_block_sync`"
        )
    source_count = validator_count + authenticated_non_validator_sources + 1
    if source_count > TOML_I64_MAX:
        raise ValueError(
            "derived Sumeragi body source partition count "
            f"{source_count} exceeds the Rust/TOML signed 64-bit integer maximum "
            f"of {TOML_I64_MAX}"
        )
    if source_bytes > TOML_I64_MAX // source_count:
        raise ValueError(
            "derived `sumeragi.queues.body_bytes` exceeds the Rust/TOML signed "
            f"64-bit integer maximum of {TOML_I64_MAX}; reduce "
            "`authenticated_non_validator_sources` or `body_source_bytes`"
        )
    minimum = (validator_count + authenticated_non_validator_sources + 1) * source_bytes
    return max(configured, minimum)


def _scaled_sumeragi_bodies(template: dict[str, Any], validator_count: int) -> int:
    """Return a roster-aware canonical body-message queue capacity."""

    if (
        isinstance(validator_count, bool)
        or not isinstance(validator_count, int)
        or validator_count < 0
    ):
        raise ValueError("validator count must be a non-negative integer")
    sumeragi = template.get("sumeragi")
    if not isinstance(sumeragi, dict):
        raise ValueError("config template must define a `[sumeragi]` table")
    queues = sumeragi.get("queues")
    if not isinstance(queues, dict):
        raise ValueError("config template must define a `[sumeragi.queues]` table")
    context = "config template `[sumeragi.queues]`"
    configured = (
        SUMERAGI_DEFAULT_BODY_CAPACITY
        if "bodies" not in queues
        else _require_positive_integer(queues, "bodies", context)
    )
    authenticated_non_validator_sources = _require_positive_integer(
        queues, "authenticated_non_validator_sources", context
    )
    anonymous_slots = 1 if validator_count == 0 else 2
    if validator_count > TOML_I64_MAX // 5:
        raise ValueError(
            "derived `sumeragi.queues.bodies` exceeds the Rust/TOML signed "
            f"64-bit integer maximum of {TOML_I64_MAX}; reduce the validator roster"
        )
    validator_slots = validator_count * 5
    if authenticated_non_validator_sources > (TOML_I64_MAX - validator_slots) // 3:
        raise ValueError(
            "derived `sumeragi.queues.bodies` exceeds the Rust/TOML signed "
            f"64-bit integer maximum of {TOML_I64_MAX}; reduce the validator "
            "roster or `authenticated_non_validator_sources`"
        )
    authenticated_slots = authenticated_non_validator_sources * 3
    if validator_slots + authenticated_slots > TOML_I64_MAX - anonymous_slots:
        raise ValueError(
            "derived `sumeragi.queues.bodies` exceeds the Rust/TOML signed "
            f"64-bit integer maximum of {TOML_I64_MAX}; reduce the validator "
            "roster or `authenticated_non_validator_sources`"
        )
    minimum = (
        5 * validator_count
        + 3 * authenticated_non_validator_sources
        + (1 if validator_count == 0 else 2)
    )
    return max(configured, minimum)


def _quote_toml(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def _decode_base_digits(digits: list[int], base: int) -> bytes:
    value = 0
    for digit in digits:
        value = value * base + digit
    decoded = value.to_bytes((value.bit_length() + 7) // 8, "big") if value else b""
    leading_zeroes = 0
    for digit in digits:
        if digit != 0:
            break
        leading_zeroes += 1
    return b"\0" * leading_zeroes + decoded


def _convert_to_base32(data: bytes) -> list[int]:
    accumulator = 0
    bits = 0
    result: list[int] = []
    for byte in data:
        accumulator = (accumulator << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            result.append((accumulator >> bits) & 0x1F)
    if bits:
        result.append((accumulator << (5 - bits)) & 0x1F)
    return result


def _bech32_polymod(values: list[int]) -> int:
    generators = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
    checksum = 1
    for value in values:
        top = checksum >> 25
        checksum = ((checksum & 0x1FF_FFFF) << 5) ^ value
        for index, generator in enumerate(generators):
            if (top >> index) & 1:
                checksum ^= generator
    return checksum


def _i105_checksum_digits(canonical: bytes) -> list[int]:
    values = [ord(character) >> 5 for character in "snx"]
    values.append(0)
    values.extend(ord(character) & 0x1F for character in "snx")
    values.extend(_convert_to_base32(canonical))
    values.extend([0] * I105_CHECKSUM_LEN)
    polymod = _bech32_polymod(values) ^ I105_BECH32M_CONST
    return [
        (polymod >> (5 * (I105_CHECKSUM_LEN - 1 - index))) & 0x1F
        for index in range(I105_CHECKSUM_LEN)
    ]


def _encode_taira_i105(canonical: bytes) -> str:
    leading_zeroes = len(canonical) - len(canonical.lstrip(b"\0"))
    value = int.from_bytes(canonical, "big")
    digits: list[int] = []
    while value:
        value, remainder = divmod(value, len(I105_ALPHABET))
        digits.append(remainder)
    encoded_digits = [0] * leading_zeroes + list(reversed(digits))
    if not encoded_digits:
        encoded_digits = [0]
    return "test" + "".join(
        I105_ALPHABET[digit]
        for digit in (*encoded_digits, *_i105_checksum_digits(canonical))
    )


def _decode_taira_i105_account(value: str, context: str) -> bytes:
    if not value.startswith("test") or value != value.strip() or "@" in value:
        raise ValueError(f"{context} must be an exact canonical Taira I105 account id")
    payload = value[len("test") :]
    try:
        digits = [I105_INDEX[symbol] for symbol in payload]
    except KeyError as error:
        raise ValueError(
            f"{context} must be an exact canonical Taira I105 account id"
        ) from error
    if len(digits) <= I105_CHECKSUM_LEN:
        raise ValueError(f"{context} must be an exact canonical Taira I105 account id")
    canonical = _decode_base_digits(digits[:-I105_CHECKSUM_LEN], len(I105_ALPHABET))
    if digits[-I105_CHECKSUM_LEN:] != _i105_checksum_digits(canonical):
        raise ValueError(f"{context} must be an exact canonical Taira I105 account id")
    if _encode_taira_i105(canonical) != value:
        raise ValueError(f"{context} must be an exact canonical Taira I105 account id")
    return canonical


def _validate_taira_i105_account(value: str, context: str) -> None:
    _decode_taira_i105_account(value, context)


def _validate_asset_definition_id(value: str, context: str) -> None:
    try:
        index = {symbol: offset for offset, symbol in enumerate(BASE58_ALPHABET)}
        digits = [index[symbol] for symbol in value]
    except KeyError as error:
        raise ValueError(
            f"{context} must be an exact canonical asset definition id"
        ) from error
    decoded = _decode_base_digits(digits, len(BASE58_ALPHABET))
    if len(decoded) != 21 or decoded[0] != 1:
        raise ValueError(f"{context} must be an exact canonical asset definition id")
    aid_bytes = decoded[1:17]
    if aid_bytes[6] >> 4 != 4 or aid_bytes[8] & 0xC0 != 0x80:
        raise ValueError(f"{context} must encode UUIDv4 asset-definition bytes")
    try:
        import blake3
    except ModuleNotFoundError as error:  # pragma: no cover - environment specific
        raise SystemExit(
            "install scripts/requirements.txt before rendering Taira bundles"
        ) from error
    if decoded[17:] != blake3.blake3(decoded[:17]).digest()[:4]:
        raise ValueError(f"{context} asset definition checksum is invalid")
    canonical_digits: list[int] = []
    integer = int.from_bytes(decoded, "big")
    while integer:
        integer, remainder = divmod(integer, len(BASE58_ALPHABET))
        canonical_digits.append(remainder)
    leading_zeroes = len(decoded) - len(decoded.lstrip(b"\0"))
    canonical = "1" * leading_zeroes + "".join(
        BASE58_ALPHABET[digit] for digit in reversed(canonical_digits)
    )
    if canonical != value:
        raise ValueError(f"{context} must be an exact canonical asset definition id")


def _format_literal(tag: str, body: str) -> str:
    """Return the canonical CRC-bound Norito literal for one UTF-8 body."""

    crc = 0xFFFF
    for byte in tag.encode("utf-8") + b":" + body.encode("utf-8"):
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return f"{tag}:{body}#{crc:04X}"


def _canonical_socket_address(value: str, context: str) -> str:
    """Normalize one host/port into the strict `addr:<body>#<crc16>` form."""

    raw = value.strip()
    literal_match = re.fullmatch(r"addr:(.+)#([0-9A-F]{4})", raw)
    if raw.startswith("addr:"):
        if literal_match is None:
            raise ValueError(
                f"{context} must be a canonical addr:<host>:<port>#<CRC16> literal"
            )
        body = literal_match.group(1)
    else:
        if "#" in raw:
            raise ValueError(
                f"{context} must not contain a checksum without the `addr:` tag"
            )
        body = raw

    if body.startswith("["):
        close = body.find("]")
        if close <= 1 or body[close + 1 : close + 2] != ":":
            raise ValueError(f"{context} contains an invalid bracketed IPv6 address")
        host = body[1:close]
        port_text = body[close + 2 :]
        if "]" in port_text:
            raise ValueError(f"{context} contains an invalid bracketed IPv6 address")
        canonical_host = f"[{host.lower()}]"
    else:
        host, separator, port_text = body.rpartition(":")
        if (
            not separator
            or not host
            or ":" in host
            or any(character in host for character in "[]/@#")
        ):
            raise ValueError(f"{context} must contain one host and one port")
        canonical_host = host.lower()

    if (
        not port_text
        or not port_text.isascii()
        or not port_text.isdecimal()
        or any(character.isspace() for character in body)
    ):
        raise ValueError(f"{context} contains an invalid decimal port")
    port = int(port_text, 10)
    if port > 65535:
        raise ValueError(f"{context} port must fit in u16")

    canonical = _format_literal("addr", f"{canonical_host}:{port}")
    if literal_match is not None and canonical != raw:
        raise ValueError(f"{context} is not canonical; expected `{canonical}`")
    return canonical


def _canonical_torii_origin(value: str, context: str) -> str:
    """Return one canonical public Taira HTTPS origin."""

    if value != value.strip():
        raise ValueError(f"{context} must not contain leading or trailing whitespace")
    try:
        parsed = urlsplit(value)
        hostname = parsed.hostname
        port = parsed.port
    except ValueError as error:
        raise ValueError(f"{context} is not a valid absolute HTTPS origin: {error}") from error
    if parsed.scheme != "https" or not parsed.netloc or hostname is None:
        raise ValueError(f"{context} must be an absolute HTTPS origin")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError(f"{context} must not contain credentials")
    if parsed.path not in ("", "/") or parsed.query or parsed.fragment:
        raise ValueError(f"{context} must not contain a path, query, or fragment")
    canonical_hostname = hostname.lower().rstrip(".")
    if not canonical_hostname:
        raise ValueError(f"{context} must contain a hostname")
    try:
        canonical_hostname = str(ip_address(canonical_hostname))
    except ValueError:
        labels = canonical_hostname.split(".")
        if (
            len(canonical_hostname) > 253
            or (len(labels) == 4 and all(label.isdecimal() for label in labels))
            or any(
                not label
                or len(label) > 63
                or re.fullmatch(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?", label)
                is None
                for label in labels
            )
        ):
            raise ValueError(f"{context} contains a non-canonical hostname") from None
    canonical_host = (
        f"[{canonical_hostname}]" if ":" in canonical_hostname else canonical_hostname
    )
    effective_port = 443 if port is None else port
    if effective_port <= 0:
        raise ValueError(f"{context} contains an invalid port")
    if effective_port != 443:
        canonical_host = f"{canonical_host}:{effective_port}"
    return f"https://{canonical_host}"


def _blake3_token_hash(token: str, native_tool: Path | None = None) -> str:
    """Return the canonical digest stored in account-onboarding config."""

    try:
        token_bytes = token.encode("ascii")
    except UnicodeEncodeError as error:
        raise ValueError(
            "onboarding token must contain only printable ASCII bytes"
        ) from error
    if not 32 <= len(token_bytes) <= 256:
        raise ValueError("onboarding token must contain 32 through 256 bytes")
    if any(byte < 0x21 or byte > 0x7E for byte in token_bytes):
        raise ValueError(
            "onboarding token must contain only non-whitespace printable ASCII bytes"
        )
    if native_tool is not None:
        if not native_tool.is_absolute():
            raise ValueError("native onboarding-token hash tool must be absolute")
        try:
            resolved = native_tool.resolve(strict=True)
            info = native_tool.lstat()
        except OSError as error:
            raise ValueError(
                "cannot inspect native onboarding-token hash tool"
            ) from error
        if (
            resolved != native_tool
            or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            or not os.access(native_tool, os.X_OK)
        ):
            raise ValueError(
                "native onboarding-token hash tool must be a canonical, "
                "single-link, non-writable executable"
            )
        try:
            result = subprocess.run(
                [str(native_tool)],
                input=token_bytes,
                capture_output=True,
                check=False,
                timeout=30,
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                umask=0o077,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise ValueError(
                "native onboarding-token hash tool could not run"
            ) from error
        if (
            result.returncode != 0
            or result.stderr
            or re.fullmatch(rb"[0-9a-f]{64}\n", result.stdout) is None
        ):
            raise ValueError(
                "native onboarding-token hash tool refused canonical derivation"
            )
        after = native_tool.lstat()
        if (
            info.st_dev,
            info.st_ino,
            info.st_size,
            info.st_mtime_ns,
            info.st_ctime_ns,
            info.st_mode,
            info.st_nlink,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            after.st_mode,
            after.st_nlink,
        ):
            raise ValueError("native onboarding-token hash tool changed while running")
        return f"blake3:{result.stdout[:-1].decode('ascii')}"

    try:
        import blake3
    except ModuleNotFoundError as error:  # pragma: no cover - environment specific
        raise SystemExit(
            "install scripts/requirements.txt before rendering Taira bundles"
        ) from error
    return f"blake3:{blake3.blake3(token.encode('utf-8')).hexdigest()}"


def _write_private_text(path: Path, value: str) -> None:
    """Atomically replace one private regular file without following planted links."""

    parent_descriptor = _open_private_directory(path.parent, "private output parent")
    temporary_name = f".{path.name}.{os.urandom(16).hex()}.tmp"
    descriptor = -1
    try:
        try:
            existing = os.stat(
                path.name,
                dir_fd=parent_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            existing = None
        if existing is not None and (
            not stat.S_ISREG(existing.st_mode)
            or existing.st_nlink != 1
            or existing.st_uid != os.getuid()
            or existing.st_gid != os.getgid()
            or stat.S_IMODE(existing.st_mode) & 0o077
        ):
            raise ValueError(f"private output path is not a safe regular file: {path}")
        descriptor = os.open(
            temporary_name,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
            dir_fd=parent_descriptor,
        )
        os.fchmod(descriptor, 0o600)
        payload = (value if value.endswith("\n") else f"{value}\n").encode("utf-8")
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:  # pragma: no cover - defensive kernel contract
                raise OSError("short write while publishing private output")
            view = view[written:]
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        os.replace(
            temporary_name,
            path.name,
            src_dir_fd=parent_descriptor,
            dst_dir_fd=parent_descriptor,
        )
        final = os.stat(path.name, dir_fd=parent_descriptor, follow_symlinks=False)
        if (
            not stat.S_ISREG(final.st_mode)
            or final.st_nlink != 1
            or final.st_uid != os.getuid()
            or final.st_gid != os.getgid()
            or stat.S_IMODE(final.st_mode) != 0o600
        ):
            raise ValueError(f"published private output has an unsafe identity: {path}")
        os.fsync(parent_descriptor)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary_name, dir_fd=parent_descriptor)
        except FileNotFoundError:
            pass
        os.close(parent_descriptor)


def _open_private_directory(path: Path, label: str) -> int:
    """Open one canonical owner-private directory without following links."""

    if not path.is_absolute():
        raise ValueError(f"{label} must be an absolute path")
    try:
        if path.resolve(strict=True) != path:
            raise ValueError(
                f"{label} must be canonical and contain no symlink components"
            )
        descriptor = os.open(
            path,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as error:
        raise ValueError(f"cannot open {label}: {path}") from error
    info = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
        or stat.S_IMODE(info.st_mode) & 0o077
    ):
        os.close(descriptor)
        raise ValueError(f"{label} must be owner-controlled and mode 0700: {path}")
    return descriptor


def _ensure_private_directory(path: Path, label: str) -> None:
    """Create one direct child of a private directory, or validate it in place."""

    if not path.is_absolute() or path.name in {"", ".", ".."}:
        raise ValueError(f"{label} must be a canonical absolute child path")
    parent_descriptor = _open_private_directory(path.parent, f"{label} parent")
    try:
        try:
            os.mkdir(path.name, 0o700, dir_fd=parent_descriptor)
            os.fsync(parent_descriptor)
        except FileExistsError:
            pass
    finally:
        os.close(parent_descriptor)
    descriptor = _open_private_directory(path, label)
    os.close(descriptor)


def _validate_account_onboarding_secrets(shared: SharedSecrets, context: str) -> None:
    fields = {
        "account_onboarding_authority": shared.account_onboarding_authority,
        "account_onboarding_private_key": shared.account_onboarding_private_key,
        "account_onboarding_api_token": shared.account_onboarding_api_token,
        "account_onboarding_credential_id": shared.account_onboarding_credential_id,
    }
    scopes = [
        shared.account_onboarding_scope_domain,
        shared.account_onboarding_scope_dataspace,
    ]
    if any(value is not None for value in (*fields.values(), *scopes)):
        missing = [key for key, value in fields.items() if value is None]
        if missing:
            raise ValueError(
                f"{context} account onboarding is incomplete; missing "
                + ", ".join(missing)
            )
        if sum(value is not None for value in scopes) != 1:
            raise ValueError(
                f"{context} account onboarding must set exactly one of "
                "account_onboarding_scope_domain or account_onboarding_scope_dataspace"
            )
        if shared.account_onboarding_scope_domain is not None:
            raise ValueError(
                f"{context} BOI/Taira onboarding must use a deployed Taira dataspace, not a domain"
            )
        if shared.account_onboarding_scope_dataspace not in {"is", "is2"}:
            raise ValueError(
                f"{context} BOI/Taira onboarding dataspace must be exactly `is` or `is2`"
            )


def _validate_mandatory_soracloud_runtime_signer(
    shared: SharedSecrets, context: str
) -> None:
    required = {
        "soracloud_runtime_signer_handle": shared.soracloud_runtime_signer_handle,
        "soracloud_runtime_signer_authority": shared.soracloud_runtime_signer_authority,
        "soracloud_runtime_signer_algorithm": shared.soracloud_runtime_signer_algorithm,
        "soracloud_runtime_signer_public_key_hex": (
            shared.soracloud_runtime_signer_public_key_hex
        ),
        "soracloud_runtime_signer_revision": shared.soracloud_runtime_signer_revision,
        "soracloud_runtime_signer_policy_digest_hex": (
            shared.soracloud_runtime_signer_policy_digest_hex
        ),
    }
    missing = [key for key, value in required.items() if value is None]
    if missing:
        raise ValueError(
            f"{context} production Soracloud runtime signer is mandatory; missing "
            + ", ".join(missing)
        )
    placeholders = [
        key
        for key, value in required.items()
        if isinstance(value, str) and value.startswith("REPLACE_WITH_")
    ]
    if placeholders:
        raise ValueError(
            f"{context} Soracloud runtime signer still contains placeholders: "
            + ", ".join(placeholders)
        )

    handle = shared.soracloud_runtime_signer_handle or ""
    allowed_handle_bytes = set(
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._:/-"
    )
    rejected_components = {
        "null",
        "mock",
        "test",
        "dev",
        "fake",
        "dummy",
        "placeholder",
    }
    if (
        not handle
        or len(handle.encode("utf-8")) > 256
        or not handle.isascii()
        or any(character not in allowed_handle_bytes for character in handle)
        or any(
            component in rejected_components
            for component in re.split(r"[^a-z0-9]+", handle.lower())
        )
    ):
        raise ValueError(
            f"{context} soracloud_runtime_signer_handle must be a credential-free "
            "production provider handle"
        )

    authority = shared.soracloud_runtime_signer_authority or ""
    canonical_authority = _decode_taira_i105_account(
        authority,
        f"{context} soracloud_runtime_signer_authority",
    )
    algorithm = shared.soracloud_runtime_signer_algorithm
    public_key_hex = shared.soracloud_runtime_signer_public_key_hex or ""
    if (
        not public_key_hex
        or len(public_key_hex) > 16 * 1024
        or len(public_key_hex) % 2
        or public_key_hex != public_key_hex.lower()
        or any(character not in "0123456789abcdef" for character in public_key_hex)
    ):
        raise ValueError(
            f"{context} soracloud_runtime_signer_public_key_hex must be bounded "
            "canonical lowercase hexadecimal"
        )
    public_key = bytes.fromhex(public_key_hex)
    if not any(public_key):
        raise ValueError(
            f"{context} soracloud_runtime_signer_public_key_hex must be nonzero"
        )
    if algorithm == "ed25519":
        if len(public_key) != 32:
            raise ValueError(
                f"{context} Ed25519 Soracloud runtime signer public key must be 32 bytes"
            )
        expected_authority = bytes((2, 0, 1, len(public_key))) + public_key
    elif algorithm == "ml_dsa":
        if len(public_key) != 1_952:
            raise ValueError(
                f"{context} ML-DSA Soracloud runtime signer public key must be 1952 bytes"
            )
        expected_authority = (
            bytes((2, 2, 4)) + len(public_key).to_bytes(2, "big") + public_key
        )
    else:
        raise ValueError(
            f"{context} soracloud_runtime_signer_algorithm must be exactly "
            "`ed25519` or `ml_dsa`"
        )
    if canonical_authority != expected_authority:
        raise ValueError(
            f"{context} soracloud_runtime_signer_authority must be derived from "
            "soracloud_runtime_signer_public_key_hex"
        )

    revision = shared.soracloud_runtime_signer_revision
    if isinstance(revision, bool) or not isinstance(revision, int) or revision <= 0:
        raise ValueError(
            f"{context} soracloud_runtime_signer_revision must be a positive integer"
        )
    policy_digest = shared.soracloud_runtime_signer_policy_digest_hex or ""
    if (
        len(policy_digest) != 64
        or policy_digest != policy_digest.lower()
        or any(character not in "0123456789abcdef" for character in policy_digest)
        or set(policy_digest) == {"0"}
    ):
        raise ValueError(
            f"{context} soracloud_runtime_signer_policy_digest_hex must be a "
            "canonical nonzero 32-byte lowercase digest"
        )


def _validate_kagemusha_command_submitter(shared: SharedSecrets, context: str) -> None:
    """Validate Taira's optional online command-submitter credential.

    This application-service signer is used by hosted top-up/redemption
    routes. It is not an offline capability switch, asset enrollment record,
    or validator-readiness requirement.
    """

    private_key = shared.kagemusha_commands_private_key
    if private_key is None:
        return
    if private_key.startswith("REPLACE_WITH_"):
        raise ValueError(
            f"{context} kagemusha_commands_private_key still contains a placeholder"
        )


def _load_validator_tables(
    payload: dict[str, Any], context: str
) -> list[dict[str, Any]]:
    validators_raw = payload.get("validators")
    if not isinstance(validators_raw, list):
        raise ValueError(f"{context} must define a `validators` array of tables")
    validators: list[dict[str, Any]] = []
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"{context} validator entry #{index} must be a TOML table")
        validators.append(raw)
    return validators


def _load_optional_validator_tables(
    payload: dict[str, Any], context: str
) -> list[dict[str, Any]]:
    validators_raw = payload.get("validators")
    if validators_raw is None:
        return []
    if not isinstance(validators_raw, list):
        raise ValueError(f"{context} must define a `validators` array of tables")
    validators: list[dict[str, Any]] = []
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"{context} validator entry #{index} must be a TOML table")
        validators.append(raw)
    return validators


def _load_defaults(payload: dict[str, Any]) -> RosterDefaults:
    values = {
        "network_address": payload.get("network_address", DEFAULT_NETWORK_ADDRESS),
        "torii_address": payload.get("torii_address", DEFAULT_TORII_ADDRESS),
    }
    for key, value in values.items():
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"roster default `{key}` must be a non-empty string")
    torii_public_address = payload.get("torii_public_address")
    if torii_public_address is not None:
        if (
            not isinstance(torii_public_address, str)
            or not torii_public_address.strip()
        ):
            raise ValueError(
                "roster default `torii_public_address` must be a non-empty string"
            )
        torii_public_address = _canonical_torii_origin(
            torii_public_address,
            "roster default `torii_public_address`",
        )
    return RosterDefaults(
        network_address=_canonical_socket_address(
            values["network_address"], "roster default `network_address`"
        ),
        torii_address=_canonical_socket_address(
            values["torii_address"], "roster default `torii_address`"
        ),
        torii_public_address=torii_public_address,
    )


def _optional_string(payload: dict[str, Any], key: str, context: str) -> str | None:
    value = payload.get(key)
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} field `{key}` must be a non-empty string")
    return value.strip()


def _optional_string_list(
    payload: dict[str, Any], key: str, context: str
) -> tuple[str, ...]:
    value = payload.get(key)
    if value is None:
        return ()
    if not isinstance(value, list) or not value:
        raise ValueError(f"{context} field `{key}` must be a non-empty array")
    normalized: list[str] = []
    for index, entry in enumerate(value, start=1):
        if not isinstance(entry, str) or not entry.strip():
            raise ValueError(
                f"{context} field `{key}` entry #{index} must be a non-empty string"
            )
        normalized.append(entry.strip())
    if len(set(normalized)) != len(normalized):
        raise ValueError(f"{context} field `{key}` must not contain duplicates")
    return tuple(normalized)


def _validate_ed25519_public_key(value: str, context: str) -> None:
    prefix = "ed0120"
    payload = value[len(prefix) :] if value.startswith(prefix) else ""
    if (
        len(payload) != 64
        or payload != payload.upper()
        or any(character not in "0123456789ABCDEF" for character in payload)
        or set(payload) == {"0"}
    ):
        raise ValueError(
            f"{context} must be a canonical non-zero Ed25519 multihash key "
            f"(`{prefix}` plus 64 uppercase hex characters)"
        )


def _secp256k1_add(
    left: tuple[int, int] | None,
    right: tuple[int, int] | None,
) -> tuple[int, int] | None:
    """Add two affine secp256k1 points for receipt-key validation."""

    if left is None:
        return right
    if right is None:
        return left
    x1, y1 = left
    x2, y2 = right
    if x1 == x2 and (y1 + y2) % SECP256K1_FIELD_MODULUS == 0:
        return None
    if left == right:
        slope = (
            3
            * x1
            * x1
            * pow(2 * y1, -1, SECP256K1_FIELD_MODULUS)
        ) % SECP256K1_FIELD_MODULUS
    else:
        slope = (
            (y2 - y1)
            * pow(x2 - x1, -1, SECP256K1_FIELD_MODULUS)
        ) % SECP256K1_FIELD_MODULUS
    x3 = (slope * slope - x1 - x2) % SECP256K1_FIELD_MODULUS
    y3 = (slope * (x1 - x3) - y1) % SECP256K1_FIELD_MODULUS
    return x3, y3


def _secp256k1_public_payload(private_payload: bytes) -> bytes:
    """Derive one compressed SEC1 public key from an exact secret scalar."""

    scalar = int.from_bytes(private_payload, "big")
    if not 1 <= scalar < SECP256K1_GROUP_ORDER:
        raise ValueError("receipt private key scalar is outside the secp256k1 group")
    point: tuple[int, int] | None = None
    addend: tuple[int, int] | None = SECP256K1_GENERATOR
    while scalar:
        if scalar & 1:
            point = _secp256k1_add(point, addend)
        addend = _secp256k1_add(addend, addend)
        scalar >>= 1
    if point is None:  # pragma: no cover - excluded by the scalar bound
        raise ValueError("receipt private key maps to the point at infinity")
    x, y = point
    return bytes((2 | (y & 1),)) + x.to_bytes(32, "big")


def receipt_node_id(receipt_public_key: str) -> str:
    """Derive the canonical public lifecycle node ID from a receipt key."""

    if RECEIPT_PUBLIC_KEY_RE.fullmatch(receipt_public_key) is None:
        raise ValueError(
            "receipt public key must be one canonical compressed secp256k1 "
            "multihash"
        )
    payload = bytes.fromhex(receipt_public_key[len(RECEIPT_PUBLIC_KEY_PREFIX) :])
    x = int.from_bytes(payload[1:], "big")
    if x >= SECP256K1_FIELD_MODULUS:
        raise ValueError("receipt public key x-coordinate is outside secp256k1")
    curve_y_squared = (pow(x, 3, SECP256K1_FIELD_MODULUS) + 7) % (
        SECP256K1_FIELD_MODULUS
    )
    curve_y = pow(
        curve_y_squared,
        (SECP256K1_FIELD_MODULUS + 1) // 4,
        SECP256K1_FIELD_MODULUS,
    )
    if pow(curve_y, 2, SECP256K1_FIELD_MODULUS) != curve_y_squared:
        raise ValueError("receipt public key is not a secp256k1 curve point")
    digest = hashlib.sha256(
        RECEIPT_NODE_ID_DOMAIN + receipt_public_key.encode("ascii")
    ).hexdigest()
    return RECEIPT_NODE_ID_PREFIX + digest


def validate_receipt_keypair(
    public_key: str,
    private_key: str,
    context: str,
) -> str:
    """Validate one canonical non-BLS Torii receipt keypair."""

    try:
        node_id = receipt_node_id(public_key)
    except ValueError as error:
        raise ValueError(f"{context} {error}") from error
    if RECEIPT_PRIVATE_KEY_RE.fullmatch(private_key) is None:
        raise ValueError(
            f"{context} receipt private key must be one canonical secp256k1 "
            "private multihash"
        )
    private_payload = bytes.fromhex(
        private_key[len(RECEIPT_PRIVATE_KEY_PREFIX) :]
    )
    try:
        derived_payload = _secp256k1_public_payload(private_payload)
    except ValueError as error:
        raise ValueError(f"{context} {error}") from error
    configured_payload = bytes.fromhex(
        public_key[len(RECEIPT_PUBLIC_KEY_PREFIX) :]
    )
    if derived_payload != configured_payload:
        raise ValueError(
            f"{context} receipt public/private keys do not form one secp256k1 keypair"
        )
    return node_id


def receipt_signer_map(
    validators: list[ValidatorEntry],
) -> dict[str, dict[str, object]]:
    """Return the ordered secret-free receipt-signer identity projection."""

    result: dict[str, dict[str, object]] = {}
    seen_nodes: set[str] = set()
    seen_keys: set[str] = set()
    for validator in validators:
        public_key = validator.receipt_public_key
        private_key = validator.receipt_private_key
        if public_key is None or private_key is None:
            raise ValueError(
                f"validator `{validator.slug}` is missing its runtime-only Torii "
                "receipt keypair"
            )
        node_id = validate_receipt_keypair(
            public_key,
            private_key,
            f"validator `{validator.slug}`",
        )
        if node_id != validator.receipt_node_id:
            raise ValueError(
                f"validator `{validator.slug}` receipt node ID changed after loading"
            )
        if node_id in seen_nodes or public_key in seen_keys:
            raise ValueError("validator Torii receipt signer identities are duplicated")
        seen_nodes.add(node_id)
        seen_keys.add(public_key)
        result[validator.slug] = {
            "node_id": node_id,
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": public_key[
                    len(RECEIPT_PUBLIC_KEY_PREFIX) :
                ].lower(),
            },
        }
    return result


def load_secret_material(path: Path) -> SecretMaterial:
    """Load per-validator private keys plus shared runtime-only secret material."""

    payload = _load_toml(path)
    validators_raw = _load_optional_validator_tables(payload, f"secrets file {path}")
    secrets: dict[str, ValidatorSecrets] = {}
    seen_receipt_public_keys: set[str] = set()
    seen_receipt_private_keys: set[str] = set()
    seen_receipt_node_ids: set[str] = set()
    for raw in validators_raw:
        slug = _require_string(raw, "slug", f"secrets file `{path}`")
        private_key = _require_string(raw, "private_key", f"secrets file `{slug}`")
        soranet_transport_public_key = _require_string(
            raw,
            "soranet_transport_public_key",
            f"secrets file `{slug}`",
        )
        soranet_transport_private_key = _require_string(
            raw,
            "soranet_transport_private_key",
            f"secrets file `{slug}`",
        )
        receipt_public_key = _require_string(
            raw,
            "receipt_public_key",
            f"secrets file `{slug}`",
        )
        receipt_private_key = _require_string(
            raw,
            "receipt_private_key",
            f"secrets file `{slug}`",
        )
        receipt_node_id = validate_receipt_keypair(
            receipt_public_key,
            receipt_private_key,
            f"secrets file `{slug}`",
        )
        if slug in secrets:
            raise ValueError(
                f"secrets file `{path}` duplicates validator slug `{slug}`"
            )
        if soranet_transport_public_key == private_key:
            raise ValueError(
                f"secrets file `{slug}` must not reuse the BLS validator private key "
                "as its SoraNet transport public key"
            )
        if soranet_transport_private_key == private_key:
            raise ValueError(
                f"secrets file `{slug}` must not reuse the BLS validator private key "
                "as its SoraNet transport private key"
            )
        if receipt_public_key in seen_receipt_public_keys:
            raise ValueError(
                f"secrets file `{path}` duplicates a Torii receipt public key"
            )
        if receipt_private_key in seen_receipt_private_keys:
            raise ValueError(
                f"secrets file `{path}` duplicates a Torii receipt private key"
            )
        if receipt_node_id in seen_receipt_node_ids:
            raise ValueError(
                f"secrets file `{path}` aliases a Torii receipt node ID"
            )
        if receipt_private_key in {private_key, soranet_transport_private_key}:
            raise ValueError(
                f"secrets file `{slug}` must use a distinct Torii receipt private key"
            )
        if receipt_public_key == soranet_transport_public_key:
            raise ValueError(
                f"secrets file `{slug}` must use a distinct Torii receipt public key"
            )
        seen_receipt_public_keys.add(receipt_public_key)
        seen_receipt_private_keys.add(receipt_private_key)
        seen_receipt_node_ids.add(receipt_node_id)
        secrets[slug] = ValidatorSecrets(
            private_key=private_key,
            soranet_transport_public_key=soranet_transport_public_key,
            soranet_transport_private_key=soranet_transport_private_key,
            receipt_public_key=receipt_public_key,
            receipt_private_key=receipt_private_key,
            receipt_node_id=receipt_node_id,
        )
    shared_raw = payload.get("shared", {})
    if not isinstance(shared_raw, dict):
        raise ValueError(f"secrets file `{path}` field `shared` must be a TOML table")
    legacy_onboarding_fields = sorted(
        field
        for field in ("torii_onboarding_authority", "torii_onboarding_private_key")
        if field in shared_raw
    )
    if legacy_onboarding_fields:
        raise ValueError(
            f"secrets file `{path}` uses removed onboarding fields: "
            + ", ".join(legacy_onboarding_fields)
            + "; use account_onboarding_* fields"
        )
    removed_offline_enrollment_fields = sorted(
        field
        for field in (
            "offline_asset_alias",
            "offline_asset_definition_id",
            "offline_asset_scale",
            "offline_escrow_account",
        )
        if field in shared_raw
    )
    if removed_offline_enrollment_fields:
        raise ValueError(
            f"secrets file `{path}` uses removed offline enrollment fields: "
            + ", ".join(removed_offline_enrollment_fields)
            + "; offline capability is universal and has no asset or escrow catalog"
        )
    sorafs_council_public_keys = _optional_string_list(
        shared_raw,
        "sorafs_council_public_keys",
        f"secrets file `{path}`",
    )
    for index, key in enumerate(sorafs_council_public_keys, start=1):
        _validate_ed25519_public_key(
            key,
            f"secrets file `{path}` SoraFS council key #{index}",
        )
    sorafs_council_signature_threshold = shared_raw.get(
        "sorafs_council_signature_threshold"
    )
    if sorafs_council_signature_threshold is not None and (
        isinstance(sorafs_council_signature_threshold, bool)
        or not isinstance(sorafs_council_signature_threshold, int)
        or sorafs_council_signature_threshold <= 0
    ):
        raise ValueError(
            f"secrets file `{path}` field `sorafs_council_signature_threshold` "
            "must be a positive integer"
        )
    if bool(sorafs_council_public_keys) != (
        sorafs_council_signature_threshold is not None
    ):
        raise ValueError(
            f"secrets file `{path}` must configure both sorafs_council_public_keys "
            "and sorafs_council_signature_threshold"
        )
    if (
        sorafs_council_signature_threshold is not None
        and sorafs_council_signature_threshold > len(sorafs_council_public_keys)
    ):
        raise ValueError(
            f"secrets file `{path}` SoraFS council threshold exceeds the trusted key count"
        )

    shared = SharedSecrets(
        account_onboarding_authority=_optional_string(
            shared_raw, "account_onboarding_authority", f"secrets file `{path}`"
        ),
        account_onboarding_private_key=_optional_string(
            shared_raw, "account_onboarding_private_key", f"secrets file `{path}`"
        ),
        account_onboarding_api_token=_optional_string(
            shared_raw, "account_onboarding_api_token", f"secrets file `{path}`"
        ),
        account_onboarding_credential_id=_optional_string(
            shared_raw, "account_onboarding_credential_id", f"secrets file `{path}`"
        ),
        account_onboarding_scope_domain=_optional_string(
            shared_raw, "account_onboarding_scope_domain", f"secrets file `{path}`"
        ),
        account_onboarding_scope_dataspace=_optional_string(
            shared_raw,
            "account_onboarding_scope_dataspace",
            f"secrets file `{path}`",
        ),
        torii_faucet_authority=_optional_string(
            shared_raw, "torii_faucet_authority", f"secrets file `{path}`"
        ),
        torii_faucet_private_key=_optional_string(
            shared_raw, "torii_faucet_private_key", f"secrets file `{path}`"
        ),
        kagemusha_commands_private_key=_optional_string(
            shared_raw, "kagemusha_commands_private_key", f"secrets file `{path}`"
        ),
        soracloud_runtime_signer_handle=_optional_string(
            shared_raw,
            "soracloud_runtime_signer_handle",
            f"secrets file `{path}`",
        ),
        soracloud_runtime_signer_authority=_optional_string(
            shared_raw,
            "soracloud_runtime_signer_authority",
            f"secrets file `{path}`",
        ),
        soracloud_runtime_signer_algorithm=_optional_string(
            shared_raw,
            "soracloud_runtime_signer_algorithm",
            f"secrets file `{path}`",
        ),
        soracloud_runtime_signer_public_key_hex=_optional_string(
            shared_raw,
            "soracloud_runtime_signer_public_key_hex",
            f"secrets file `{path}`",
        ),
        soracloud_runtime_signer_revision=shared_raw.get(
            "soracloud_runtime_signer_revision"
        ),
        soracloud_runtime_signer_policy_digest_hex=_optional_string(
            shared_raw,
            "soracloud_runtime_signer_policy_digest_hex",
            f"secrets file `{path}`",
        ),
        streaming_identity_public_key=_optional_string(
            shared_raw, "streaming_identity_public_key", f"secrets file `{path}`"
        ),
        streaming_identity_private_key=_optional_string(
            shared_raw, "streaming_identity_private_key", f"secrets file `{path}`"
        ),
        sorafs_council_public_keys=sorafs_council_public_keys,
        sorafs_council_signature_threshold=sorafs_council_signature_threshold,
    )
    _validate_account_onboarding_secrets(shared, f"secrets file `{path}`")
    if bool(shared.torii_faucet_authority) != bool(shared.torii_faucet_private_key):
        raise ValueError(
            f"secrets file `{path}` must configure both torii_faucet_authority "
            "and torii_faucet_private_key"
        )
    _validate_mandatory_soracloud_runtime_signer(shared, f"secrets file `{path}`")
    _validate_kagemusha_command_submitter(shared, f"secrets file `{path}`")
    return SecretMaterial(
        validators=secrets,
        shared=shared,
    )


def load_secret_keys(path: Path) -> dict[str, str]:
    """Load per-validator private keys from a user-local secrets file."""

    return load_secret_material(path).validators


def _render_trusted_peers(validators: list[ValidatorEntry]) -> list[str]:
    lines = ["trusted_peers = ["]
    for validator in validators:
        lines.append(
            f"  {_quote_toml(f'{validator.public_key}@{validator.public_address}')},"
        )
    lines.append("]")
    return lines


def _render_trusted_peers_pop(validators: list[ValidatorEntry]) -> list[str]:
    lines = ["trusted_peers_pop = ["]
    for validator in validators:
        lines.append(
            "  { public_key = "
            f"{_quote_toml(validator.public_key)}, "
            f"pop_hex = {_quote_toml(validator.pop_hex)} }},"
        )
    lines.append("]")
    return lines


def _render_governance_manifest(validators: list[ValidatorEntry]) -> str:
    """Render the Parliament lane manifest used for authoritative routing."""

    payload = {
        "lane": "governance",
        "governance": "parliament",
        "version": 1,
        "validators": [
            {
                "validator": validator.account_id,
                "peer_id": validator.public_key,
                "torii_url": validator.torii_public_address,
            }
            for validator in validators
        ],
        "quorum": max(1, (len(validators) * 2 // 3) + 1),
        "protected_namespaces": [
            "apps",
            "governance",
        ],
        "hooks": {
            "runtime_upgrade": {
                "allow": True,
                "require_metadata": True,
                "metadata_key": "gov_upgrade_id",
            },
        },
    }
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def render_genesis_template(
    base_genesis_path: Path,
    validators: list[ValidatorEntry],
    output_dir: Path,
) -> Path:
    """Render the unsigned shared genesis template with the exact public BLS roster.

    The matching private validator keys are intentionally absent. ``kagami
    genesis sign --config`` stages this template, derives the signed Nexus/AMX
    and execution-policy height-context commitments from the chosen validator
    config, persists that exact bound manifest, and only then emits the final
    Norito genesis block.
    """

    payload = json.loads(base_genesis_path.read_text(encoding="utf-8"))
    transactions = payload.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        raise ValueError(
            f"base genesis {base_genesis_path} must contain a non-empty transactions array"
        )
    if not isinstance(payload.get("sumeragi_v2"), dict):
        raise ValueError(
            f"base genesis {base_genesis_path} is missing required sumeragi_v2 parameters"
        )
    da_layout = payload["sumeragi_v2"].get("da_layout")
    if not isinstance(da_layout, dict):
        raise ValueError(
            f"base genesis {base_genesis_path} is missing required "
            "sumeragi_v2.da_layout parameters"
        )
    max_payload_size_bytes = da_layout.get("max_payload_size_bytes")
    if (
        isinstance(max_payload_size_bytes, bool)
        or not isinstance(max_payload_size_bytes, int)
        or max_payload_size_bytes != TAIRA_BLOCK_MAX_PAYLOAD_BYTES
    ):
        raise ValueError(
            f"base genesis {base_genesis_path} sumeragi_v2.da_layout."
            f"max_payload_size_bytes must equal the revision-4 protocol "
            f"ceiling of {TAIRA_BLOCK_MAX_PAYLOAD_BYTES}"
        )
    transaction_parameter_tables = [
        transaction["parameters"]["transaction"]
        for transaction in transactions
        if isinstance(transaction, dict)
        and isinstance(transaction.get("parameters"), dict)
        and isinstance(transaction["parameters"].get("transaction"), dict)
    ]
    if len(transaction_parameter_tables) != 1:
        raise ValueError(
            f"base genesis {base_genesis_path} must define exactly one "
            "transaction admission parameter table"
        )
    transaction_parameters = transaction_parameter_tables[0]
    transaction_context = "base genesis transaction admission parameters"
    max_tx_bytes = _require_positive_integer(
        transaction_parameters, "max_tx_bytes", transaction_context
    )
    if max_tx_bytes < TAIRA_TRANSACTION_MAX_BYTES:
        raise ValueError(
            f"{transaction_context} field `max_tx_bytes` must be at least "
            f"{TAIRA_TRANSACTION_MAX_BYTES} bytes to carry one maximum "
            "first-release privacy action and transaction framing"
        )
    max_decompressed_bytes = _require_positive_integer(
        transaction_parameters, "max_decompressed_bytes", transaction_context
    )
    if max_decompressed_bytes < max_tx_bytes:
        raise ValueError(
            f"{transaction_context} field `max_decompressed_bytes` must be at "
            "least `max_tx_bytes`"
        )
    for transaction in transactions:
        if not isinstance(transaction, dict):
            raise ValueError(
                f"base genesis {base_genesis_path} contains a non-object transaction"
            )
        transaction["topology"] = []

    registered_accounts: set[str] = set()
    for transaction_index, transaction in enumerate(transactions):
        instructions = transaction.get("instructions", [])
        if not isinstance(instructions, list):
            raise ValueError(
                f"base genesis {base_genesis_path} contains a non-array instructions field"
            )
        for instruction_index, instruction in enumerate(instructions):
            if isinstance(instruction, str) and instruction:
                continue
            if not isinstance(instruction, dict) or len(instruction) != 1:
                raise ValueError(
                    f"base genesis {base_genesis_path} transaction "
                    f"{transaction_index} instruction {instruction_index} must be "
                    "a single-key structured instruction object or a non-empty "
                    "canonical base64 instruction string"
                )
            account = instruction.get("Register", {}).get("Account")
            if isinstance(account, dict) and isinstance(account.get("id"), str):
                registered_accounts.add(account["id"])

    validator_account_instructions = [
        {
            "Register": {
                "Account": {
                    "id": validator.account_id,
                    "metadata": {
                        "purpose": "taira_validator_payout_recipient",
                        "validator_slug": validator.slug,
                    },
                }
            }
        }
        for validator in validators
        if validator.account_id not in registered_accounts
    ]
    transactions.append(
        {
            "instructions": validator_account_instructions,
            "ivm_triggers": [],
            "topology": [
                {"peer": validator.public_key, "pop_hex": validator.pop_hex}
                for validator in validators
            ],
        }
    )

    target = output_dir / "genesis.json"
    _write_private_text(target, json.dumps(payload, ensure_ascii=False, indent=2))
    signing_command = output_dir / "genesis-signing-command.txt"
    _write_private_text(
        signing_command,
        " ".join(
            [
                '"$TAIRA_GENESIS_EXTERNAL_SIGNER"',
                "--unsigned-genesis",
                str(target),
                "--peer-config",
                str(output_dir / validators[0].slug / "config.toml"),
                "--bound-manifest-out",
                str(target),
                "--signed-genesis-out",
                str(output_dir / "genesis.signed.nrt"),
                "--expected-hash-out",
                str(output_dir / "genesis.expected_hash"),
            ]
        )
        + "\n",
    )
    return target


def load_roster(
    path: Path,
    secrets_path: Path | None = None,
    secrets: SecretMaterial | None = None,
) -> list[ValidatorEntry]:
    """Load and validate Taira validator material."""

    payload = _load_toml(path)
    defaults = _load_defaults(payload)
    validators_raw = _load_validator_tables(payload, "roster")
    if len(validators_raw) < MIN_VALIDATORS:
        raise ValueError(
            f"roster must define at least {MIN_VALIDATORS} validators for Taira"
        )
    if len(validators_raw) > MAX_VALIDATORS:
        raise ValueError(
            f"roster must define at most {MAX_VALIDATORS} validators for the "
            "Sumeragi v2 protocol"
        )
    if (len(validators_raw) - 1) % 3 != 0:
        raise ValueError(
            "roster must define an exact 3f + 1 validator committee "
            "(4, 7, 10, ..., 31)"
        )
    if secrets is None and secrets_path is not None:
        secrets = load_secret_material(secrets_path)
    secrets_by_slug = secrets.validators if secrets is not None else {}

    validators: list[ValidatorEntry] = []
    seen_slugs: set[str] = set()
    seen_account_ids: set[str] = set()
    seen_public_keys: set[str] = set()
    seen_soranet_transport_public_keys: set[str] = set()
    seen_receipt_public_keys: set[str] = set()
    seen_receipt_node_ids: set[str] = set()
    seen_public_addresses: set[str] = set()
    seen_torii_public_addresses: set[str] = set()
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"validator entry #{index} must be a TOML table")
        slug = _require_string(raw, "slug", f"validator `{index}`")
        expected_slug = f"taira-validator-{index}"
        if slug != expected_slug:
            raise ValueError(
                f"validator entry #{index} slug must be exactly `{expected_slug}`"
            )
        public_key = _require_string(raw, "public_key", f"validator `{slug}`")
        if "receipt_public_key" in raw or "receipt_private_key" in raw:
            raise ValueError(
                f"validator `{slug}` Torii receipt keys must come only from the "
                "runtime-only --secrets file"
            )
        validator_secrets = secrets_by_slug.get(slug)
        private_key_value = raw.get(
            "private_key",
            validator_secrets.private_key if validator_secrets is not None else None,
        )
        if not isinstance(private_key_value, str) or not private_key_value.strip():
            raise ValueError(
                f"validator `{slug}` is missing `private_key`; provide it inline or via --secrets"
            )
        private_key = private_key_value.strip()
        soranet_transport_public_key_value = raw.get(
            "soranet_transport_public_key",
            validator_secrets.soranet_transport_public_key
            if validator_secrets is not None
            else None,
        )
        if (
            not isinstance(soranet_transport_public_key_value, str)
            or not soranet_transport_public_key_value.strip()
        ):
            raise ValueError(
                f"validator `{slug}` is missing `soranet_transport_public_key`; "
                "provide it inline or via --secrets"
            )
        soranet_transport_public_key = soranet_transport_public_key_value.strip()
        soranet_transport_private_key_value = raw.get(
            "soranet_transport_private_key",
            validator_secrets.soranet_transport_private_key
            if validator_secrets is not None
            else None,
        )
        if (
            not isinstance(soranet_transport_private_key_value, str)
            or not soranet_transport_private_key_value.strip()
        ):
            raise ValueError(
                f"validator `{slug}` is missing `soranet_transport_private_key`; "
                "provide it inline or via --secrets"
            )
        soranet_transport_private_key = soranet_transport_private_key_value.strip()
        receipt_public_key = (
            validator_secrets.receipt_public_key
            if validator_secrets is not None
            else None
        )
        receipt_private_key = (
            validator_secrets.receipt_private_key
            if validator_secrets is not None
            else None
        )
        receipt_node_id = (
            validator_secrets.receipt_node_id
            if validator_secrets is not None
            else None
        )
        pop_hex = _require_string(raw, "pop_hex", f"validator `{slug}`")
        public_address = _canonical_socket_address(
            _require_string(raw, "public_address", f"validator `{slug}`"),
            f"validator `{slug}` field `public_address`",
        )
        network_address = raw.get("network_address", defaults.network_address)
        torii_address = raw.get("torii_address", defaults.torii_address)
        torii_public_address = raw.get(
            "torii_public_address", defaults.torii_public_address
        )
        if not isinstance(network_address, str) or not network_address.strip():
            raise ValueError(f"validator `{slug}` field `network_address` is invalid")
        if not isinstance(torii_address, str) or not torii_address.strip():
            raise ValueError(f"validator `{slug}` field `torii_address` is invalid")
        if (
            not isinstance(torii_public_address, str)
            or not torii_public_address.strip()
        ):
            raise ValueError(
                f"validator `{slug}` must set `torii_public_address` explicitly; "
                "public Taira deploys use direct per-node Torii hostnames"
            )
        torii_public_address = _canonical_torii_origin(
            torii_public_address,
            f"validator `{slug}` field `torii_public_address`",
        )
        if slug in seen_slugs:
            raise ValueError(f"validator slug `{slug}` is duplicated")
        account_id = _require_string(raw, "account_id", f"validator `{slug}`")
        if account_id in seen_account_ids:
            raise ValueError(f"validator account_id `{account_id}` is duplicated")
        if public_key in seen_public_keys:
            raise ValueError(f"validator public_key `{public_key}` is duplicated")
        if soranet_transport_public_key == public_key:
            raise ValueError(
                f"validator `{slug}` must use distinct node and SoraNet transport public keys"
            )
        if soranet_transport_public_key in seen_soranet_transport_public_keys:
            raise ValueError(
                "validator soranet_transport_public_key "
                f"`{soranet_transport_public_key}` is duplicated"
            )
        if (
            receipt_public_key is not None
            and receipt_public_key in seen_receipt_public_keys
        ):
            raise ValueError(
                f"validator receipt_public_key `{receipt_public_key}` is duplicated"
            )
        if receipt_node_id is not None and receipt_node_id in seen_receipt_node_ids:
            raise ValueError(f"validator receipt node_id `{receipt_node_id}` is duplicated")
        if public_address in seen_public_addresses:
            raise ValueError(
                f"validator public_address `{public_address}` is duplicated"
            )
        if torii_public_address in seen_torii_public_addresses:
            raise ValueError(
                f"validator torii_public_address `{torii_public_address}` is duplicated; "
                "each public validator must expose its own direct Torii hostname"
            )
        seen_slugs.add(slug)
        seen_account_ids.add(account_id)
        seen_public_keys.add(public_key)
        seen_soranet_transport_public_keys.add(soranet_transport_public_key)
        if receipt_public_key is not None:
            seen_receipt_public_keys.add(receipt_public_key)
        if receipt_node_id is not None:
            seen_receipt_node_ids.add(receipt_node_id)
        seen_public_addresses.add(public_address)
        seen_torii_public_addresses.add(torii_public_address)
        validators.append(
            ValidatorEntry(
                slug=slug,
                account_id=account_id,
                public_key=public_key,
                private_key=private_key,
                soranet_transport_public_key=soranet_transport_public_key,
                soranet_transport_private_key=soranet_transport_private_key,
                receipt_public_key=receipt_public_key,
                receipt_private_key=receipt_private_key,
                receipt_node_id=receipt_node_id,
                pop_hex=pop_hex,
                public_address=public_address,
                network_address=_canonical_socket_address(
                    network_address,
                    f"validator `{slug}` field `network_address`",
                ),
                torii_address=_canonical_socket_address(
                    torii_address,
                    f"validator `{slug}` field `torii_address`",
                ),
                torii_public_address=torii_public_address,
            )
        )

    unknown_secret_slugs = sorted(set(secrets_by_slug).difference(seen_slugs))
    if unknown_secret_slugs:
        raise ValueError(
            "secrets file contains validators not present in the public roster: "
            + ", ".join(unknown_secret_slugs)
        )

    if secrets is not None and secrets.shared.streaming_identity_public_key is not None:
        reused_streaming = sorted(
            validator.slug
            for validator in validators
            if validator.soranet_transport_public_key
            == secrets.shared.streaming_identity_public_key
        )
        if reused_streaming:
            raise ValueError(
                "SoraNet transport identities must not reuse the shared streaming identity; "
                "conflicting validators: " + ", ".join(reused_streaming)
            )

    return validators


def render_validator_config(
    template_text: str,
    validator: ValidatorEntry,
    validators: list[ValidatorEntry],
    validator_private_key_file: Path,
    soranet_transport_private_key_file: Path,
    shared_secrets: SharedSecrets | None = None,
    onboarding_private_key_file: Path | None = None,
    onboarding_token_hash: str | None = None,
    faucet_private_key_file: Path | None = None,
    kagemusha_commands_private_key_file: Path | None = None,
    streaming_identity_private_key_file: Path | None = None,
    manifest_directory: Path = DEFAULT_INSTALL_ROOT / "manifests",
    sorafs_admission_directory: Path = DEFAULT_INSTALL_ROOT / "sorafs_admission",
    kagemusha_release_policy_path: Path | None = None,
    kagemusha_artifact_dir: Path | None = None,
    kagemusha_catalog_qualification_seal_path: Path | None = None,
    sumeragi_bodies: int | None = None,
    sumeragi_body_bytes: int | None = None,
    genesis_expected_hash: str | None = None,
    genesis_file: Path | None = None,
    privacy_issuer_state_dir: Path | None = None,
) -> str:
    """Rewrite the checked-in peer-1 baseline for one validator."""

    current_section: str | None = None
    skipping_array: str | None = None
    bodies_rewritten = False
    body_bytes_rewritten = False
    genesis_expected_hash_rewritten = False
    genesis_file_rewritten = False
    kagemusha_offline_section_seen = False
    receipt_signer_written = False
    rendered: list[str] = []
    trusted_peers_lines = _render_trusted_peers(validators)
    trusted_peers_pop_lines = _render_trusted_peers_pop(validators)
    shared = shared_secrets or SharedSecrets()
    if (kagemusha_release_policy_path is None) != (kagemusha_artifact_dir is None):
        raise ValueError(
            "Kagemusha release policy and artifact directory must be supplied together"
        )
    if (
        kagemusha_catalog_qualification_seal_path is not None
        and kagemusha_release_policy_path is None
    ):
        raise ValueError(
            "Kagemusha qualification seal requires the release policy and artifact directory"
        )

    for raw_line in template_text.splitlines():
        stripped = raw_line.strip()

        if skipping_array is not None:
            if stripped == "]":
                skipping_array = None
            continue

        if stripped.startswith("[[") or stripped.startswith("["):
            current_section = stripped
            rendered.append(raw_line)
            if current_section == "[sumeragi.queues]" and sumeragi_bodies is not None:
                rendered.append(f"bodies = {sumeragi_bodies}")
                bodies_rewritten = True
            if current_section == "[torii]":
                if (
                    validator.receipt_public_key is None
                    or validator.receipt_private_key is None
                ):
                    raise ValueError(
                        f"validator `{validator.slug}` lacks its runtime-only Torii "
                        "receipt keypair"
                    )
                validate_receipt_keypair(
                    validator.receipt_public_key,
                    validator.receipt_private_key,
                    f"validator `{validator.slug}`",
                )
                rendered.extend(
                    [
                        "receipt_public_key = "
                        + _quote_toml(validator.receipt_public_key),
                        "receipt_private_key = "
                        + _quote_toml(validator.receipt_private_key),
                    ]
                )
                receipt_signer_written = True
            if current_section == "[settlement.offline]":
                kagemusha_offline_section_seen = True
                if kagemusha_release_policy_path is not None:
                    rendered.extend(
                        [
                            "kagemusha_release_policy_path = "
                            + _quote_toml(str(kagemusha_release_policy_path)),
                            "kagemusha_artifact_dir = "
                            + _quote_toml(str(kagemusha_artifact_dir)),
                        ]
                    )
                    if kagemusha_catalog_qualification_seal_path is not None:
                        rendered.append(
                            "kagemusha_catalog_qualification_seal_path = "
                            + _quote_toml(
                                str(kagemusha_catalog_qualification_seal_path)
                            )
                        )
                    rendered.append(
                        f"kagemusha_max_decoded_bytes = {KAGEMUSHA_MAX_DECODED_BYTES}"
                    )
            if current_section == "[genesis]" and genesis_file is not None:
                rendered.append(f"file = {_quote_toml(str(genesis_file))}")
                genesis_file_rewritten = True
            continue

        if current_section is None and stripped.startswith("public_key = "):
            rendered.append(f"public_key = {_quote_toml(validator.public_key)}")
            continue
        if current_section is None and (
            stripped.startswith("private_key = ")
            or stripped.startswith("private_key_file = ")
        ):
            rendered.append(
                "private_key_file = "
                + _quote_toml(str(validator_private_key_file))
            )
            continue
        if current_section is None and stripped.startswith(
            "soranet_transport_public_key = "
        ):
            rendered.append(
                "soranet_transport_public_key = "
                + _quote_toml(validator.soranet_transport_public_key)
            )
            continue
        if current_section is None and (
            stripped.startswith("soranet_transport_private_key = ")
            or stripped.startswith("soranet_transport_private_key_file = ")
        ):
            rendered.append(
                "soranet_transport_private_key_file = "
                + _quote_toml(str(soranet_transport_private_key_file))
            )
            continue
        if current_section is None and stripped == "trusted_peers = [":
            rendered.extend(trusted_peers_lines)
            skipping_array = "trusted_peers"
            continue
        if current_section is None and stripped == "trusted_peers_pop = [":
            rendered.extend(trusted_peers_pop_lines)
            skipping_array = "trusted_peers_pop"
            continue

        if current_section == TAIRA_PRIVACY_ISSUER_SECTION:
            field = stripped.partition("=")[0].strip()
            if field == "enabled" and (
                validator.slug != TAIRA_PRIVACY_ISSUER_DESIGNATED_VALIDATOR
            ):
                rendered.append("enabled = false")
                continue
            if field == "state_dir":
                state_dir = privacy_issuer_state_dir or Path(
                    f"/var/lib/iroha/{validator.slug}/privacy/bootle-lantern/issuer"
                )
                rendered.append("state_dir = " + _quote_toml(str(state_dir)))
                continue
            if (
                validator.slug != TAIRA_PRIVACY_ISSUER_DESIGNATED_VALIDATOR
                and field in TAIRA_PRIVACY_ISSUER_BINDING_FIELDS
            ):
                continue

        if (
            current_section == "[genesis]"
            and stripped.startswith("file = ")
            and genesis_file is not None
        ):
            continue
        if current_section == "[genesis]" and (
            stripped.startswith("expected_hash = ")
            or stripped.startswith("expected_hash_file = ")
        ):
            genesis_expected_hash_rewritten = True
            if genesis_expected_hash is None:
                rendered.append(
                    'expected_hash_file = "/run/iroha/genesis.expected_hash"'
                )
            else:
                # Inline config hashes use the Norito JSON literal grammar; the
                # signer/file contract remains raw lowercase hexadecimal.
                expected_hash_literal = _format_literal(
                    "hash", genesis_expected_hash.upper()
                )
                rendered.append(f'expected_hash = "{expected_hash_literal}"')
            continue

        if current_section == "[network]" and stripped.startswith("address = "):
            rendered.append(f"address = {_quote_toml(validator.network_address)}")
            continue
        if current_section == "[network]" and stripped.startswith("public_address = "):
            rendered.append(f"public_address = {_quote_toml(validator.public_address)}")
            continue
        if (
            current_section == "[sumeragi.queues]"
            and stripped.partition("=")[0].strip() == "bodies"
            and sumeragi_bodies is not None
        ):
            continue
        if (
            current_section == "[sumeragi.queues]"
            and stripped.partition("=")[0].strip() == "body_bytes"
            and sumeragi_body_bytes is not None
        ):
            rendered.append(f"body_bytes = {sumeragi_body_bytes}")
            body_bytes_rewritten = True
            continue
        if current_section == "[torii]" and stripped.startswith("address = "):
            rendered.append(f"address = {_quote_toml(validator.torii_address)}")
            continue
        if current_section == "[torii]" and stripped.startswith("public_address = "):
            rendered.append(
                f"public_address = {_quote_toml(validator.torii_public_address)}"
            )
            continue
        if (
            current_section == "[torii.account_onboarding]"
            and stripped.startswith("authority = ")
            and shared.account_onboarding_authority is not None
        ):
            rendered.append(
                f"authority = {_quote_toml(shared.account_onboarding_authority)}"
            )
            continue
        if (
            current_section == "[torii.account_onboarding]"
            and stripped.startswith("private_key_file = ")
            and onboarding_private_key_file is not None
        ):
            rendered.append(
                f"private_key_file = {_quote_toml(str(onboarding_private_key_file))}"
            )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("id = ")
            and shared.account_onboarding_credential_id is not None
        ):
            rendered.append(
                f"id = {_quote_toml(shared.account_onboarding_credential_id)}"
            )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("scope = ")
            and (
                shared.account_onboarding_scope_domain is not None
                or shared.account_onboarding_scope_dataspace is not None
            )
        ):
            if shared.account_onboarding_scope_domain is not None:
                rendered.append(
                    "scope = { domain = "
                    f"{_quote_toml(shared.account_onboarding_scope_domain)} }}"
                )
            else:
                rendered.append(
                    "scope = { dataspace = "
                    f"{_quote_toml(shared.account_onboarding_scope_dataspace or '')} }}"
                )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("token_hash = ")
            and onboarding_token_hash is not None
        ):
            rendered.append(f"token_hash = {_quote_toml(onboarding_token_hash)}")
            continue
        if (
            current_section == "[torii.faucet]"
            and stripped.startswith("authority = ")
            and shared.torii_faucet_authority is not None
        ):
            rendered.append(f"authority = {_quote_toml(shared.torii_faucet_authority)}")
            continue
        if (
            current_section == "[torii.kagemusha_commands]"
            and (
                stripped.startswith("private_key = ")
                or stripped.startswith("private_key_file = ")
            )
            and kagemusha_commands_private_key_file is not None
        ):
            rendered.append(
                "private_key_file = "
                + _quote_toml(str(kagemusha_commands_private_key_file))
            )
            continue
        if (
            current_section == "[settlement.offline]"
            and kagemusha_release_policy_path is not None
            and stripped.partition("=")[0].strip()
            in {
                "kagemusha_release_policy_path",
                "kagemusha_artifact_dir",
                "kagemusha_catalog_qualification_seal_path",
                "kagemusha_max_decoded_bytes",
            }
        ):
            continue
        if current_section == "[soracloud_runtime.submission.signer]":
            signer_values = {
                "handle": shared.soracloud_runtime_signer_handle,
                "authority": shared.soracloud_runtime_signer_authority,
                "algorithm": shared.soracloud_runtime_signer_algorithm,
                "public_key_hex": shared.soracloud_runtime_signer_public_key_hex,
                "policy_digest_hex": (
                    shared.soracloud_runtime_signer_policy_digest_hex
                ),
            }
            field = stripped.partition("=")[0].strip()
            if field == "revision":
                rendered.append(
                    f"revision = {shared.soracloud_runtime_signer_revision}"
                )
                continue
            if field in signer_values:
                rendered.append(f"{field} = {_quote_toml(signer_values[field] or '')}")
                continue
        if (
            current_section == "[torii.faucet]"
            and stripped.startswith("private_key_file = ")
            and faucet_private_key_file is not None
        ):
            rendered.append(
                f"private_key_file = {_quote_toml(str(faucet_private_key_file))}"
            )
            continue
        if (
            current_section == "[streaming]"
            and stripped.startswith("identity_public_key = ")
            and shared.streaming_identity_public_key is not None
        ):
            rendered.append(
                f"identity_public_key = {_quote_toml(shared.streaming_identity_public_key)}"
            )
            continue
        if current_section == "[sorafs.discovery.admission]" and stripped.startswith(
            "envelopes_dir = "
        ):
            rendered.append(
                f"envelopes_dir = {_quote_toml(str(sorafs_admission_directory))}"
            )
            continue
        if (
            current_section == "[sorafs.discovery.admission]"
            and stripped.startswith("trusted_council_keys = ")
            and shared.sorafs_council_public_keys
        ):
            rendered_keys = ", ".join(
                _quote_toml(key) for key in shared.sorafs_council_public_keys
            )
            rendered.append(f"trusted_council_keys = [{rendered_keys}]")
            continue
        if (
            current_section == "[sorafs.discovery.admission]"
            and stripped.startswith("signature_threshold = ")
            and shared.sorafs_council_signature_threshold is not None
        ):
            rendered.append(
                f"signature_threshold = {shared.sorafs_council_signature_threshold}"
            )
            continue
        if (
            current_section == "[streaming]"
            and (
                stripped.startswith("identity_private_key = ")
                or stripped.startswith("identity_private_key_file = ")
            )
            and streaming_identity_private_key_file is not None
        ):
            rendered.append(
                "identity_private_key_file = "
                + _quote_toml(str(streaming_identity_private_key_file))
            )
            continue
        if current_section == "[nexus.registry]" and stripped.startswith(
            "manifest_directory = "
        ):
            rendered.append(
                f"manifest_directory = {_quote_toml(str(manifest_directory))}"
            )
            continue
        if current_section == "[nexus.registry]" and stripped.startswith(
            "cache_directory = "
        ):
            rendered.append(f"cache_directory = {_quote_toml(str(manifest_directory))}")
            continue

        rendered.append(raw_line)

    if kagemusha_release_policy_path is not None and not kagemusha_offline_section_seen:
        if rendered and rendered[-1]:
            rendered.append("")
        rendered.extend(
            [
                "[settlement.offline]",
                "kagemusha_release_policy_path = "
                + _quote_toml(str(kagemusha_release_policy_path)),
                "kagemusha_artifact_dir = " + _quote_toml(str(kagemusha_artifact_dir)),
            ]
        )
        if kagemusha_catalog_qualification_seal_path is not None:
            rendered.append(
                "kagemusha_catalog_qualification_seal_path = "
                + _quote_toml(str(kagemusha_catalog_qualification_seal_path))
            )
        rendered.append(
            f"kagemusha_max_decoded_bytes = {KAGEMUSHA_MAX_DECODED_BYTES}"
        )

    rendered_text = "\n".join(rendered)
    if not rendered_text.endswith("\n"):
        rendered_text += "\n"
    if sumeragi_bodies is not None and not bodies_rewritten:
        raise ValueError(
            f"rendered config for `{validator.slug}` could not rewrite the "
            "`[sumeragi.queues] bodies` assignment"
        )
    if sumeragi_body_bytes is not None and not body_bytes_rewritten:
        raise ValueError(
            f"rendered config for `{validator.slug}` could not rewrite the "
            "`[sumeragi.queues] body_bytes` assignment"
        )
    if not genesis_expected_hash_rewritten:
        raise ValueError(
            f"rendered config for `{validator.slug}` lacks the mandatory "
            "`[genesis] expected_hash` or `expected_hash_file` assignment"
        )
    if not receipt_signer_written:
        raise ValueError(
            f"rendered config for `{validator.slug}` lacks the mandatory `[torii]` table"
        )
    if genesis_file is not None and not genesis_file_rewritten:
        raise ValueError(
            f"rendered config for `{validator.slug}` lacks the mandatory "
            "`[genesis]` table needed for its bundle-local file"
        )
    if "REPLACE_WITH_" in rendered_text:
        raise ValueError(
            f"rendered config for `{validator.slug}` still contains template placeholder "
            "values; provide the matching validator/shared secrets in the roster or "
            "--secrets file before rendering"
        )
    return rendered_text


def render_bundle(
    base_config_path: Path,
    roster_path: Path,
    output_dir: Path,
    secrets_path: Path | None = None,
    only: str | None = None,
    base_genesis_path: Path | None = None,
    install_root: Path = DEFAULT_INSTALL_ROOT,
    genesis_expected_hash: str | None = None,
    bundle_root: Path | None = None,
    onboarding_token_hash_tool: Path | None = None,
    kagemusha_release_root: Path | None = None,
    include_kagemusha_qualification_seal: bool = True,
) -> list[Path]:
    """Render one config.toml per validator into output_dir."""

    if genesis_expected_hash is not None and (
        GENESIS_EXPECTED_HASH_RE.fullmatch(genesis_expected_hash) is None
        or int(genesis_expected_hash[-2:], 16) & 1 == 0
    ):
        raise ValueError(
            "genesis_expected_hash must be a lowercase 32-byte Iroha hash with its marker bit set"
        )
    secret_material = (
        load_secret_material(secrets_path) if secrets_path is not None else None
    )
    validators = load_roster(roster_path, secrets=secret_material)
    if only is not None and only not in {validator.slug for validator in validators}:
        raise ValueError(
            "only must identify one validator in the canonical Taira roster"
        )
    resolved_onboarding_token_hash: str | None = None
    if (
        secret_material is not None
        and secret_material.shared.account_onboarding_api_token is not None
    ):
        resolved_onboarding_token_hash = _blake3_token_hash(
            secret_material.shared.account_onboarding_api_token,
            onboarding_token_hash_tool,
        )
    template = _load_toml(base_config_path)
    settlement = template.get("settlement", {})
    offline = settlement.get("offline", {}) if isinstance(settlement, dict) else {}
    managed_kagemusha_keys = (
        set(KAGEMUSHA_MANAGED_CONFIG_KEYS).intersection(offline)
        if isinstance(offline, dict)
        else set()
    )
    if kagemusha_release_root is None and managed_kagemusha_keys:
        listed = ", ".join(sorted(managed_kagemusha_keys))
        raise ValueError(
            "base config contains managed Kagemusha release paths without "
            f"--kagemusha-release-root: {listed}"
        )
    _validate_privacy_issuer_template(template, validators)
    _validate_receipt_signer_template(template)
    sumeragi_body_bytes = _scaled_sumeragi_body_bytes(template, len(validators))
    sumeragi_bodies = _scaled_sumeragi_bodies(template, len(validators))
    template_text = base_config_path.read_text(encoding="utf-8")
    path_root = bundle_root if bundle_root is not None else install_root
    install_root_text = str(path_root)
    if (
        not path_root.is_absolute()
        or path_root == Path("/")
        or install_root_text.startswith("//")
        or os.path.normpath(install_root_text) != install_root_text
        or any(ord(character) < 0x20 for character in install_root_text)
    ):
        root_label = "bundle_root" if bundle_root is not None else "install_root"
        raise ValueError(f"{root_label} must be a canonical, non-root absolute path")
    if bundle_root is not None:
        if output_dir != bundle_root / "rendered":
            raise ValueError(
                "bundle-local rendering requires output_dir to equal bundle_root/rendered"
            )
        if not bundle_root.exists() or bundle_root.resolve(strict=True) != bundle_root:
            raise ValueError("bundle_root must be an existing canonical directory")
    output_dir_text = str(output_dir)
    if (
        not output_dir.is_absolute()
        or output_dir == Path("/")
        or output_dir_text.startswith("//")
        or os.path.normpath(output_dir_text) != output_dir_text
        or any(ord(character) < 0x20 for character in output_dir_text)
    ):
        raise ValueError("output_dir must be a canonical, non-root absolute path")
    if kagemusha_release_root is not None:
        release_root_text = str(kagemusha_release_root)
        if (
            not kagemusha_release_root.is_absolute()
            or kagemusha_release_root == Path("/")
            or release_root_text.startswith("//")
            or os.path.normpath(release_root_text) != release_root_text
            or any(ord(character) < 0x20 for character in release_root_text)
        ):
            raise ValueError(
                "kagemusha_release_root must be a canonical, non-root absolute path"
            )
        if (
            kagemusha_release_root == path_root
            or kagemusha_release_root.is_relative_to(path_root)
            or path_root.is_relative_to(kagemusha_release_root)
            or kagemusha_release_root == output_dir
            or kagemusha_release_root.is_relative_to(output_dir)
            or output_dir.is_relative_to(kagemusha_release_root)
        ):
            raise ValueError(
                "kagemusha_release_root and the validator-writable install, bundle, and render roots must be disjoint"
            )
    receipt_signer_map(validators)
    _ensure_private_directory(output_dir, "render output directory")
    if bundle_root is None:
        _write_private_text(output_dir / ".gitignore", "*\n!.gitignore")

    written: list[Path] = []
    for validator in validators:
        if only is not None and validator.slug != only:
            continue
        target_dir = output_dir / validator.slug
        _ensure_private_directory(target_dir, f"{validator.slug} output directory")
        runtime_dir = target_dir / "runtime"
        _ensure_private_directory(runtime_dir, f"{validator.slug} runtime directory")
        manifest_dir = target_dir / "manifests"
        _ensure_private_directory(manifest_dir, f"{validator.slug} manifest directory")
        if bundle_root is None:
            sorafs_admission_dir = target_dir / "sorafs_admission"
            _ensure_private_directory(
                sorafs_admission_dir,
                f"{validator.slug} SoraFS admission directory",
            )
        onboarding_private_key_file: Path | None = None
        faucet_private_key_file: Path | None = None
        kagemusha_commands_private_key_file: Path | None = None
        streaming_identity_private_key_file: Path | None = None
        if bundle_root is None:
            installed_runtime_dir = install_root / "runtime"
            installed_manifest_dir = install_root / "manifests"
            installed_sorafs_admission_dir = install_root / "sorafs_admission"
            genesis_file = None
            privacy_issuer_state_dir = None
        else:
            installed_runtime_dir = target_dir / "runtime"
            installed_manifest_dir = target_dir / "manifests"
            installed_sorafs_admission_dir = (
                target_dir / "configs/soranexus/taira/sorafs_admission"
            )
            genesis_file = bundle_root / "genesis.signed.nrt"
            privacy_issuer_state_dir = (
                target_dir / "runtime/privacy/bootle-lantern/issuer"
            )
        validator_private_key_file = installed_runtime_dir / "validator-signer.key"
        soranet_transport_private_key_file = (
            installed_runtime_dir / "soranet-transport.key"
        )
        _write_private_text(
            runtime_dir / "validator-signer.key",
            validator.private_key,
        )
        _write_private_text(
            runtime_dir / "soranet-transport.key",
            validator.soranet_transport_private_key,
        )
        if secret_material is not None:
            shared = secret_material.shared
            if shared.account_onboarding_private_key is not None:
                onboarding_private_key_file = (
                    installed_runtime_dir / "onboarding-signer.key"
                )
                _write_private_text(
                    runtime_dir / "onboarding-signer.key",
                    shared.account_onboarding_private_key,
                )
            if shared.account_onboarding_api_token is not None:
                _write_private_text(
                    runtime_dir / "onboarding-token",
                    shared.account_onboarding_api_token,
                )
            if shared.torii_faucet_private_key is not None:
                faucet_private_key_file = installed_runtime_dir / "faucet-signer.key"
                _write_private_text(
                    runtime_dir / "faucet-signer.key",
                    shared.torii_faucet_private_key,
                )
            if shared.kagemusha_commands_private_key is not None:
                kagemusha_commands_private_key_file = (
                    installed_runtime_dir / "kagemusha-command-signer.key"
                )
                _write_private_text(
                    runtime_dir / "kagemusha-command-signer.key",
                    shared.kagemusha_commands_private_key,
                )
            if shared.streaming_identity_private_key is not None:
                streaming_identity_private_key_file = (
                    installed_runtime_dir / "streaming-identity.key"
                )
                _write_private_text(
                    runtime_dir / "streaming-identity.key",
                    shared.streaming_identity_private_key,
                )

        target_path = target_dir / "config.toml"
        _write_private_text(
            target_path,
            render_validator_config(
                template_text,
                validator,
                validators,
                validator_private_key_file=validator_private_key_file,
                soranet_transport_private_key_file=soranet_transport_private_key_file,
                shared_secrets=secret_material.shared if secret_material else None,
                onboarding_private_key_file=onboarding_private_key_file,
                onboarding_token_hash=resolved_onboarding_token_hash,
                faucet_private_key_file=faucet_private_key_file,
                kagemusha_commands_private_key_file=kagemusha_commands_private_key_file,
                streaming_identity_private_key_file=streaming_identity_private_key_file,
                manifest_directory=installed_manifest_dir,
                sorafs_admission_directory=installed_sorafs_admission_dir,
                kagemusha_release_policy_path=(
                    kagemusha_release_root / KAGEMUSHA_RELEASE_POLICY_RELATIVE_PATH
                    if kagemusha_release_root is not None
                    else None
                ),
                kagemusha_artifact_dir=(
                    kagemusha_release_root / KAGEMUSHA_ARTIFACT_RELATIVE_PATH
                    if kagemusha_release_root is not None
                    else None
                ),
                kagemusha_catalog_qualification_seal_path=(
                    kagemusha_release_root / KAGEMUSHA_QUALIFICATION_SEAL_RELATIVE_PATH
                    if kagemusha_release_root is not None
                    and include_kagemusha_qualification_seal
                    else None
                ),
                sumeragi_bodies=sumeragi_bodies,
                sumeragi_body_bytes=sumeragi_body_bytes,
                genesis_expected_hash=genesis_expected_hash,
                genesis_file=genesis_file,
                privacy_issuer_state_dir=privacy_issuer_state_dir,
            ),
        )
        _write_private_text(
            manifest_dir / "governance.manifest.json",
            _render_governance_manifest(validators),
        )
        written.append(target_path)

    if only is not None and not written:
        raise ValueError(f"validator `{only}` is not present in {roster_path}")
    if base_genesis_path is not None:
        render_genesis_template(base_genesis_path, validators, output_dir)
    return written


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint."""

    parser = argparse.ArgumentParser(
        description="Render per-validator Taira config.toml files from a roster."
    )
    parser.add_argument(
        "--base-config",
        default="configs/soranexus/taira/config.toml",
        help="checked-in peer-1 baseline config to rewrite",
    )
    parser.add_argument(
        "--base-genesis",
        default="configs/soranexus/taira/genesis.json",
        help="checked-in unsigned Taira genesis template to populate with the public roster",
    )
    parser.add_argument(
        "--roster",
        required=True,
        help="TOML roster with validator public addresses, public keys, and PoPs",
    )
    parser.add_argument(
        "--secrets",
        help="optional user-local TOML with per-validator private keys",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="directory where <validator-slug>/config.toml files will be written",
    )
    parser.add_argument(
        "--install-root",
        default=str(DEFAULT_INSTALL_ROOT),
        help=(
            "canonical absolute directory where one rendered validator directory "
            "will be installed on its host"
        ),
    )
    parser.add_argument(
        "--only",
        help="render only one validator slug instead of the full bundle",
    )
    parser.add_argument(
        "--genesis-expected-hash",
        help=(
            "exact lowercase consensus-header hash printed by `kagami genesis sign`; "
            "the inline config value is emitted as an uppercase CRC-bound hash literal; "
            "omit only for the non-runnable pre-signing bundle"
        ),
    )
    parser.add_argument(
        "--bundle-root",
        help=(
            "existing canonical private reset root; when set, --output-dir must "
            "be its rendered/ child and every runtime/genesis/privacy path is "
            "bound inside that reset"
        ),
    )
    parser.add_argument(
        "--onboarding-token-hash-tool",
        help=(
            "optional exact-source native helper that derives the onboarding "
            "token BLAKE3 digest from stdin"
        ),
    )
    parser.add_argument(
        "--kagemusha-release-root",
        help=(
            "opt in with an absolute root-controlled directory outside the "
            "validator install/bundle root; settlement.offline policy, catalog, "
            "and qualification-seal paths are derived beneath it"
        ),
    )
    args = parser.parse_args(argv)

    written = render_bundle(
        Path(args.base_config),
        Path(args.roster),
        Path(args.output_dir),
        secrets_path=Path(args.secrets) if args.secrets else None,
        only=args.only,
        base_genesis_path=Path(args.base_genesis),
        install_root=Path(args.install_root),
        genesis_expected_hash=args.genesis_expected_hash,
        bundle_root=Path(args.bundle_root) if args.bundle_root else None,
        onboarding_token_hash_tool=(
            Path(args.onboarding_token_hash_tool)
            if args.onboarding_token_hash_tool
            else None
        ),
        kagemusha_release_root=(
            Path(args.kagemusha_release_root) if args.kagemusha_release_root else None
        ),
    )
    for path in written:
        print(f"config: {path}")
        runtime_dir = path.parent / "runtime"
        for filename in (
            "onboarding-signer.key",
            "onboarding-token",
            "faucet-signer.key",
        ):
            sidecar = runtime_dir / filename
            if sidecar.exists():
                print(f"sidecar: {sidecar}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
