#!/usr/bin/env python3
"""Compose an unsigned, public-input-only NEVO overlay for a fresh Taira genesis.

The composer accepts only four canonical public account identifiers and two
BLAKE3 token hashes. It never accepts raw bearer tokens, private keys, seeds,
or signing commands. The checked-in generic Taira genesis is read-only: every
successful run writes a distinct unsigned genesis plus a deterministic review
manifest to new files.

Signing remains a separate operator-controlled step performed by the
independently provisioned, digest-pinned external genesis signer.
"""

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
from typing import Any, NoReturn

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from scripts import taira_constants
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_constants


PUBLIC_INPUT_SCHEMA = "iroha.taira.nevo-reset-public-inputs.v2"
REVIEW_SCHEMA = "iroha.taira.nevo-reset-review.v3"
EXPECTED_PUBLIC_INPUT_FIELDS = frozenset(
    {
        "schema",
        "onboarding_authority_account_id",
        "api_signer_account_id",
        "dpn_inori_account_id",
        "dpn_epr_guard_account_id",
        "is2_onboarding_token_hash",
        "dpn_onboarding_token_hash",
    }
)
EXPECTED_REVIEW_FIELDS = frozenset(
    {
        "schema",
        "chain",
        "chain_discriminant",
        "state",
        "public_inputs_sha256",
        "base_genesis_sha256",
        "base_config_sha256",
        "unsigned_genesis_sha256",
        "public_identities",
        "credential_hash_bindings",
        "genesis_overlay",
        "secret_boundary",
        "required_next_steps",
    }
)
CHAIN_ID = taira_constants.CHAIN_ID
CHAIN_DISCRIMINANT = taira_constants.CHAIN_DISCRIMINANT
DPN_DATASPACE_ALIAS = "dpn"
DPN_DATASPACE_ID = 10
IS2_DATASPACE_ALIAS = "is2"
IS2_DATASPACE_ID = 8_477_022_798_449_861_195
NEVO_DOMAIN = "nevo.dpn"
UNIVERSAL_DATASPACE_ALIAS = "universal"
UNIVERSAL_DATASPACE_ID = 0
ADMIN_ACCOUNT_ALIAS = "admin@universal"
INORI_ACCOUNT_ALIAS = "inori@universal"
EPR_GUARD_ACCOUNT_ALIAS = "source_guard@universal"
CONTRACT_DEPLOYMENT_PERMISSION = "CanRegisterSmartContractCode"
GENESIS_ALIAS_BOOTSTRAP_ROLE_ID = "nevo_taira_alias_bootstrap"
FEE_ASSET_ALIAS = "xor#universal"
ACCOUNT_FUNDING_AMOUNT = "1000000"
ALIAS_POLICY_VERSION = 2
ALIAS_MAX_AMOUNT = "0.5"
MAX_U64 = (1 << 64) - 1
MAX_PUBLIC_INPUT_BYTES = 64 * 1024
MAX_BASE_CONFIG_BYTES = 4 * 1024 * 1024
MAX_BASE_GENESIS_BYTES = 16 * 1024 * 1024
MAX_OUTPUT_BYTES = 32 * 1024 * 1024
TOKEN_HASH_RE = re.compile(r"blake3:[0-9a-f]{64}\Z")
PROGRAM_NAME_RE = re.compile(r"[a-z0-9](?:[a-z0-9_-]{0,126}[a-z0-9])?\Z")
ED25519_PUBLIC_KEY_RE = re.compile(r"ed0120([0-9A-F]{64})\Z")

# Detect the retired sample namespace without retaining its literal in this
# Taira source path. This is SHA-256(lowercase namespace), with a fixed length
# so substrings embedded in aliases, domains, comments, or JSON are rejected.
RETIRED_NAMESPACE_LENGTH = 10
RETIRED_NAMESPACE_SHA256 = (
    "a71a7c7011f53a1bab3642ec2ce12593f05230ace8de1e3e7645f69efac1443d"
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
I105_CHECKSUM_LENGTH = 6
I105_BECH32M_CONSTANT = 0x2BC830A3
TAIRA_ACCOUNT_PREFIX = "test"
ED25519_SINGLE_CONTROLLER_PREFIX = bytes((2, 0, 1, 32))
ED25519_FIELD_MODULUS = (1 << 255) - 19
ED25519_CURVE_D = (
    -121665 * pow(121666, ED25519_FIELD_MODULUS - 2, ED25519_FIELD_MODULUS)
) % ED25519_FIELD_MODULUS
ED25519_SQRT_MINUS_ONE = pow(
    2, (ED25519_FIELD_MODULUS - 1) // 4, ED25519_FIELD_MODULUS
)
ED25519_SUBGROUP_ORDER = (1 << 252) + 27742317777372353535851937790883648493

REPO_ROOT = Path(__file__).resolve().parent.parent
CHECKED_IN_TAIRA_GENESIS = (
    REPO_ROOT / "configs" / "soranexus" / "taira" / "genesis.json"
).resolve()


class CompositionError(ValueError):
    """Public-input or source-template refusal."""


@dataclass(frozen=True)
class PublicInputs:
    """Exact public identities and credential hashes accepted by the composer."""

    onboarding_authority_account_id: str
    api_signer_account_id: str
    dpn_inori_account_id: str
    dpn_epr_guard_account_id: str
    is2_onboarding_token_hash: str
    dpn_onboarding_token_hash: str

    def as_dict(self) -> dict[str, str]:
        """Return the canonical public input document."""

        return {
            "schema": PUBLIC_INPUT_SCHEMA,
            "onboarding_authority_account_id": self.onboarding_authority_account_id,
            "api_signer_account_id": self.api_signer_account_id,
            "dpn_inori_account_id": self.dpn_inori_account_id,
            "dpn_epr_guard_account_id": self.dpn_epr_guard_account_id,
            "is2_onboarding_token_hash": self.is2_onboarding_token_hash,
            "dpn_onboarding_token_hash": self.dpn_onboarding_token_hash,
        }


@dataclass(frozen=True)
class BaseConfig:
    """Public Taira values bound into the unsigned reset overlay."""

    fee_asset_definition_id: str
    fee_sponsor_program_id: str
    fee_sponsor_account_id: str
    fee_sponsor_program_name: str
    genesis_authority_account_id: str


def fail(message: str) -> NoReturn:
    """Raise one stable composition refusal."""

    raise CompositionError(message)


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
    values.extend([0] * I105_CHECKSUM_LENGTH)
    polymod = _bech32_polymod(values) ^ I105_BECH32M_CONSTANT
    return [
        (polymod >> (5 * (I105_CHECKSUM_LENGTH - 1 - index))) & 0x1F
        for index in range(I105_CHECKSUM_LENGTH)
    ]


def _encode_taira_i105_account(canonical: bytes) -> str:
    """Encode canonical bytes for tests and canonicality revalidation."""

    leading_zeroes = len(canonical) - len(canonical.lstrip(b"\0"))
    value = int.from_bytes(canonical, "big")
    digits: list[int] = []
    while value:
        value, remainder = divmod(value, len(I105_ALPHABET))
        digits.append(remainder)
    encoded_digits = [0] * leading_zeroes + list(reversed(digits))
    if not encoded_digits:
        encoded_digits = [0]
    return TAIRA_ACCOUNT_PREFIX + "".join(
        I105_ALPHABET[digit]
        for digit in (*encoded_digits, *_i105_checksum_digits(canonical))
    )


def _ed25519_add(
    left: tuple[int, int], right: tuple[int, int]
) -> tuple[int, int]:
    """Add two affine Edwards25519 points for public-key admission."""

    modulus = ED25519_FIELD_MODULUS
    x1, y1 = left
    x2, y2 = right
    product = ED25519_CURVE_D * x1 * x2 * y1 * y2 % modulus
    x_denominator = (1 + product) % modulus
    y_denominator = (1 - product) % modulus
    if x_denominator == 0 or y_denominator == 0:
        fail("invalid Ed25519 public key point")
    return (
        (x1 * y2 + y1 * x2) * pow(x_denominator, modulus - 2, modulus)
        % modulus,
        (y1 * y2 + x1 * x2) * pow(y_denominator, modulus - 2, modulus)
        % modulus,
    )


def _ed25519_multiply(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    """Multiply one public Edwards25519 point without accepting secret inputs."""

    result = (0, 1)
    addend = point
    while scalar:
        if scalar & 1:
            result = _ed25519_add(result, addend)
        addend = _ed25519_add(addend, addend)
        scalar >>= 1
    return result


def _validate_ed25519_public_key(encoded: bytes, context: str) -> None:
    """Match Iroha's canonical, non-weak, prime-subgroup key admission."""

    if len(encoded) != 32 or not any(encoded):
        fail(f"{context} contains invalid Ed25519 public key material")
    compressed = int.from_bytes(encoded, "little")
    sign = compressed >> 255
    y = compressed & ((1 << 255) - 1)
    modulus = ED25519_FIELD_MODULUS
    if y >= modulus:
        fail(f"{context} contains a non-canonical Ed25519 public key")
    y_squared = y * y % modulus
    numerator = (y_squared - 1) % modulus
    denominator = (ED25519_CURVE_D * y_squared + 1) % modulus
    if denominator == 0:
        fail(f"{context} contains invalid Ed25519 public key material")
    x_squared = numerator * pow(denominator, modulus - 2, modulus) % modulus
    x = pow(x_squared, (modulus + 3) // 8, modulus)
    if x * x % modulus != x_squared:
        x = x * ED25519_SQRT_MINUS_ONE % modulus
    if x * x % modulus != x_squared:
        fail(f"{context} contains invalid Ed25519 public key material")
    if x == 0 and sign == 1:
        fail(f"{context} contains a non-canonical Ed25519 public key")
    if x & 1 != sign:
        x = modulus - x
    point = (x, y)
    if point == (0, 1) or _ed25519_multiply(point, ED25519_SUBGROUP_ORDER) != (0, 1):
        fail(f"{context} contains a weak or mixed-torsion Ed25519 public key")


def _decode_taira_ed25519_account(value: Any, context: str) -> bytes:
    if (
        not isinstance(value, str)
        or not value.startswith(TAIRA_ACCOUNT_PREFIX)
        or value != value.strip()
        or "@" in value
    ):
        fail(f"{context} must be one canonical domainless Taira I105 account id")
    payload = value[len(TAIRA_ACCOUNT_PREFIX) :]
    try:
        digits = [I105_INDEX[symbol] for symbol in payload]
    except KeyError:
        fail(f"{context} must be one canonical domainless Taira I105 account id")
    if len(digits) <= I105_CHECKSUM_LENGTH:
        fail(f"{context} must be one canonical domainless Taira I105 account id")
    canonical = _decode_base_digits(
        digits[:-I105_CHECKSUM_LENGTH], len(I105_ALPHABET)
    )
    if digits[-I105_CHECKSUM_LENGTH:] != _i105_checksum_digits(canonical):
        fail(f"{context} has an invalid I105 checksum")
    if _encode_taira_i105_account(canonical) != value:
        fail(f"{context} is not canonical")
    if len(canonical) != len(ED25519_SINGLE_CONTROLLER_PREFIX) + 32 or not canonical.startswith(
        ED25519_SINGLE_CONTROLLER_PREFIX
    ):
        fail(f"{context} must identify one Ed25519 single-controller account")
    _validate_ed25519_public_key(
        canonical[len(ED25519_SINGLE_CONTROLLER_PREFIX) :], context
    )
    return canonical


def _contains_retired_namespace(text: str) -> bool:
    normalized = text.casefold()
    if len(normalized) < RETIRED_NAMESPACE_LENGTH:
        return False
    for offset in range(len(normalized) - RETIRED_NAMESPACE_LENGTH + 1):
        candidate = normalized[offset : offset + RETIRED_NAMESPACE_LENGTH]
        if hashlib.sha256(candidate.encode("utf-8")).hexdigest() == RETIRED_NAMESPACE_SHA256:
            return True
    return False


def _reject_retired_namespace(text: str, context: str) -> None:
    if _contains_retired_namespace(text):
        fail(f"{context} contains the retired sample namespace")


def _reject_secret_fields(value: Any, context: str = "public inputs") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            lowered = key.casefold().replace("-", "_")
            if (
                "private" in lowered
                or "secret" in lowered
                or "mnemonic" in lowered
                or "seed" in lowered
                or "raw_token" in lowered
                or ("token" in lowered and not lowered.endswith("token_hash"))
            ):
                fail(f"{context} contains forbidden secret-bearing field `{key}`")
            _reject_secret_fields(child, f"{context}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_secret_fields(child, f"{context}[{index}]")
    elif isinstance(value, str):
        folded = value.casefold()
        if "begin private key" in folded or "begin encrypted private key" in folded:
            fail(f"{context} contains private-key material")


def _read_bounded_regular(path: Path, maximum_bytes: int, context: str) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise CompositionError(f"cannot inspect {context} `{path}`: {error}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        fail(f"{context} must be a direct, single-link regular file of at most {maximum_bytes} bytes")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            opened.st_dev != before.st_dev
            or opened.st_ino != before.st_ino
            or opened.st_size != before.st_size
        ):
            fail(f"{context} changed while opening")
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1024 * 1024))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        if len(payload) > maximum_bytes:
            fail(f"{context} exceeds the {maximum_bytes}-byte limit")
        after = os.fstat(descriptor)
        if (
            after.st_dev != opened.st_dev
            or after.st_ino != opened.st_ino
            or after.st_size != opened.st_size
            or after.st_mtime_ns != opened.st_mtime_ns
            or after.st_ctime_ns != opened.st_ctime_ns
        ):
            fail(f"{context} changed while reading")
        return payload
    finally:
        os.close(descriptor)


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"JSON object contains duplicate key `{key}`")
        result[key] = value
    return result


def _parse_json(payload: bytes, context: str) -> Any:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CompositionError(f"{context} is not UTF-8") from error
    _reject_retired_namespace(text, context)
    try:
        return json.loads(text, object_pairs_hook=_unique_object)
    except (json.JSONDecodeError, UnicodeError) as error:
        raise CompositionError(f"{context} is not valid strict JSON: {error}") from error


def _validate_token_hash(value: Any, context: str) -> str:
    if not isinstance(value, str) or TOKEN_HASH_RE.fullmatch(value) is None:
        fail(f"{context} must be exactly `blake3:` plus 64 lowercase hexadecimal characters")
    digest = bytes.fromhex(value.removeprefix("blake3:"))
    if not any(digest) or len(set(digest)) < 8:
        fail(f"{context} is too weak to bind a runtime credential")
    return value


def _public_inputs_from_payload(payload: Any, context: str) -> PublicInputs:
    if not isinstance(payload, dict):
        fail(f"{context} must contain one JSON object")
    _reject_secret_fields(payload)
    fields = set(payload)
    if fields != EXPECTED_PUBLIC_INPUT_FIELDS:
        missing = sorted(EXPECTED_PUBLIC_INPUT_FIELDS - fields)
        unknown = sorted(fields - EXPECTED_PUBLIC_INPUT_FIELDS)
        detail = []
        if missing:
            detail.append("missing " + ", ".join(missing))
        if unknown:
            detail.append("unknown " + ", ".join(unknown))
        fail(f"{context} fields differ from the closed schema: " + "; ".join(detail))
    if payload["schema"] != PUBLIC_INPUT_SCHEMA:
        fail(f"public input schema must be exactly `{PUBLIC_INPUT_SCHEMA}`")
    onboarding = payload["onboarding_authority_account_id"]
    api_signer = payload["api_signer_account_id"]
    dpn_inori = payload["dpn_inori_account_id"]
    dpn_epr_guard = payload["dpn_epr_guard_account_id"]
    _decode_taira_ed25519_account(onboarding, "onboarding authority account")
    _decode_taira_ed25519_account(api_signer, "API signer account")
    _decode_taira_ed25519_account(dpn_inori, "DPN Inori account")
    _decode_taira_ed25519_account(dpn_epr_guard, "DPN EPR guard account")
    public_accounts = (onboarding, api_signer, dpn_inori, dpn_epr_guard)
    if len(set(public_accounts)) != len(public_accounts):
        fail("all four public NEVO accounts must be pairwise distinct")
    is2_hash = _validate_token_hash(
        payload["is2_onboarding_token_hash"], "is2 onboarding token hash"
    )
    dpn_hash = _validate_token_hash(
        payload["dpn_onboarding_token_hash"], "DPN onboarding token hash"
    )
    if is2_hash == dpn_hash:
        fail("is2 and DPN onboarding credentials must use distinct token hashes")
    return PublicInputs(
        onboarding_authority_account_id=onboarding,
        api_signer_account_id=api_signer,
        dpn_inori_account_id=dpn_inori,
        dpn_epr_guard_account_id=dpn_epr_guard,
        is2_onboarding_token_hash=is2_hash,
        dpn_onboarding_token_hash=dpn_hash,
    )


def load_public_inputs(path: Path) -> tuple[PublicInputs, bytes]:
    raw = _read_bounded_regular(path, MAX_PUBLIC_INPUT_BYTES, "public input file")
    payload = _parse_json(raw, "public input file")
    inputs = _public_inputs_from_payload(payload, "public input file")
    return inputs, _canonical_json_bytes(inputs.as_dict())


def _load_toml(payload: bytes, context: str) -> dict[str, Any]:
    try:
        import tomllib
    except ModuleNotFoundError:
        try:
            import tomli as tomllib
        except ModuleNotFoundError as error:  # pragma: no cover - environment specific
            raise SystemExit("Python 3.11+ or tomli is required to parse Taira config") from error
    try:
        value = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise CompositionError(f"{context} is not valid UTF-8 TOML: {error}") from error
    if not isinstance(value, dict):
        fail(f"{context} must contain a top-level TOML table")
    return value


def _catalog_id(config: dict[str, Any], alias: str) -> int:
    nexus = config.get("nexus")
    if not isinstance(nexus, dict):
        fail("base config lacks `[nexus]`")
    catalog = nexus.get("dataspace_catalog")
    if not isinstance(catalog, list):
        fail("base config lacks `[[nexus.dataspace_catalog]]`")
    matches = [entry for entry in catalog if isinstance(entry, dict) and entry.get("alias") == alias]
    if len(matches) != 1 or type(matches[0].get("id")) is not int:
        fail(f"base config must map dataspace alias `{alias}` exactly once")
    return matches[0]["id"]


def _parse_base_config(raw: bytes, base_genesis: dict[str, Any]) -> BaseConfig:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CompositionError("base Taira config is not UTF-8") from error
    _reject_retired_namespace(text, "base Taira config")
    config = _load_toml(raw, "base Taira config")
    if config.get("chain") != CHAIN_ID:
        fail(f"base config chain must be exactly `{CHAIN_ID}`")
    if config.get("chain_discriminant") != CHAIN_DISCRIMINANT:
        fail(f"base config chain_discriminant must be exactly {CHAIN_DISCRIMINANT}")
    genesis = config.get("genesis")
    public_key = genesis.get("public_key") if isinstance(genesis, dict) else None
    public_key_match = (
        ED25519_PUBLIC_KEY_RE.fullmatch(public_key)
        if isinstance(public_key, str)
        else None
    )
    if public_key_match is None:
        fail("base config `[genesis] public_key` must be one canonical Ed25519 public key")
    genesis_public_key = bytes.fromhex(public_key_match.group(1))
    _validate_ed25519_public_key(genesis_public_key, "genesis authority public key")
    genesis_authority_account_id = _encode_taira_i105_account(
        ED25519_SINGLE_CONTROLLER_PREFIX + genesis_public_key
    )
    if _catalog_id(config, DPN_DATASPACE_ALIAS) != DPN_DATASPACE_ID:
        fail(f"base config must map `dpn` to dataspace {DPN_DATASPACE_ID}")
    if _catalog_id(config, IS2_DATASPACE_ALIAS) != IS2_DATASPACE_ID:
        fail(f"base config must map `is2` to dataspace {IS2_DATASPACE_ID}")
    nexus = config["nexus"]
    fees = nexus.get("fees")
    if not isinstance(fees, dict) or fees.get("fee_asset_id") != FEE_ASSET_ALIAS:
        fail(f"base config `[nexus.fees] fee_asset_id` must be `{FEE_ASSET_ALIAS}`")
    torii = config.get("torii")
    onboarding = torii.get("account_onboarding") if isinstance(torii, dict) else None
    if not isinstance(onboarding, dict):
        fail("base config lacks `[torii.account_onboarding]`")
    credential_rows = onboarding.get("credentials")
    if not isinstance(credential_rows, list):
        fail("base config lacks onboarding credential templates")
    scopes: list[str] = []
    for row in credential_rows:
        scope = row.get("scope") if isinstance(row, dict) else None
        if not isinstance(scope, dict) or set(scope) != {"dataspace"}:
            fail("base config onboarding credentials must use exact dataspace scopes")
        value = scope.get("dataspace")
        if not isinstance(value, str):
            fail("base config onboarding credential dataspace must be a string")
        scopes.append(value)
    if sorted(scopes) != [DPN_DATASPACE_ALIAS, IS2_DATASPACE_ALIAS]:
        fail("base config onboarding credentials must contain exactly one `dpn` and one `is2` scope")
    program = onboarding.get("fee_sponsor_program_id")
    if not isinstance(program, str) or program.count("/") != 1:
        fail("base config onboarding fee sponsor program id is invalid")
    sponsor, program_name = program.split("/", 1)
    _decode_taira_ed25519_account(sponsor, "fee sponsor account")
    if PROGRAM_NAME_RE.fullmatch(program_name) is None:
        fail("base config onboarding fee sponsor program name is invalid")
    fee_asset_definition_id = _resolve_genesis_asset_alias(base_genesis, FEE_ASSET_ALIAS)
    return BaseConfig(
        fee_asset_definition_id=fee_asset_definition_id,
        fee_sponsor_program_id=program,
        fee_sponsor_account_id=sponsor,
        fee_sponsor_program_name=program_name,
        genesis_authority_account_id=genesis_authority_account_id,
    )


def load_base_config(path: Path, base_genesis: dict[str, Any]) -> tuple[BaseConfig, bytes]:
    raw = _read_bounded_regular(path, MAX_BASE_CONFIG_BYTES, "base Taira config")
    return _parse_base_config(raw, base_genesis), raw


def _parse_base_genesis(raw: bytes) -> dict[str, Any]:
    payload = _parse_json(raw, "base Taira genesis")
    if not isinstance(payload, dict):
        fail("base Taira genesis must contain one JSON object")
    if payload.get("chain") != CHAIN_ID:
        fail(f"base genesis chain must be exactly `{CHAIN_ID}`")
    if payload.get("chain_discriminant") != CHAIN_DISCRIMINANT:
        fail(f"base genesis chain_discriminant must be exactly {CHAIN_DISCRIMINANT}")
    transactions = payload.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        fail("base Taira genesis must contain a non-empty transactions array")
    for index, transaction in enumerate(transactions):
        if not isinstance(transaction, dict) or not isinstance(
            transaction.get("instructions"), list
        ):
            fail(f"base genesis transaction {index} lacks an instructions array")
    return payload


def load_base_genesis(path: Path) -> tuple[dict[str, Any], bytes]:
    raw = _read_bounded_regular(path, MAX_BASE_GENESIS_BYTES, "base Taira genesis")
    return _parse_base_genesis(raw), raw


def _instructions(genesis: dict[str, Any]) -> list[Any]:
    return [
        instruction
        for transaction in genesis["transactions"]
        for instruction in transaction["instructions"]
    ]


def _resolve_genesis_asset_alias(genesis: dict[str, Any], alias: str) -> str:
    bindings: list[str] = []
    registered: set[str] = set()
    for instruction in _instructions(genesis):
        if not isinstance(instruction, dict):
            continue
        registration = instruction.get("Register")
        definition = registration.get("AssetDefinition") if isinstance(registration, dict) else None
        if isinstance(definition, dict) and isinstance(definition.get("id"), str):
            registered.add(definition["id"])
        binding = instruction.get("SetAssetDefinitionAlias")
        if isinstance(binding, dict) and binding.get("alias") == alias:
            asset_id = binding.get("asset_definition_id")
            if isinstance(asset_id, str):
                bindings.append(asset_id)
    if len(bindings) != 1 or bindings[0] not in registered:
        fail(f"base genesis must register and bind `{alias}` exactly once")
    return bindings[0]


def _program_object(config: BaseConfig) -> dict[str, str]:
    return {
        "sponsor": config.fee_sponsor_account_id,
        "name": config.fee_sponsor_program_name,
    }


def _validate_base_program(genesis: dict[str, Any], config: BaseConfig) -> None:
    program = _program_object(config)
    registrations = 0
    sponsor_registered = False
    for instruction in _instructions(genesis):
        if not isinstance(instruction, dict):
            continue
        register = instruction.get("Register")
        account = register.get("Account") if isinstance(register, dict) else None
        if isinstance(account, dict) and account.get("id") == config.fee_sponsor_account_id:
            sponsor_registered = True
        created = instruction.get("CreateFeeSponsorProgram")
        candidate = created.get("program") if isinstance(created, dict) else None
        if isinstance(candidate, dict) and candidate.get("id") == program:
            registrations += 1
    if not sponsor_registered or registrations != 1:
        fail(
            "base genesis must register the configured sponsor account and fee sponsor program exactly once"
        )


def _ensure_alias_target(instruction: dict[str, Any]) -> tuple[str, str] | None:
    ensure = instruction.get("EnsureAlias")
    if not isinstance(ensure, dict):
        return None
    intent = ensure.get("intent")
    if not isinstance(intent, dict):
        return ("invalid", "invalid")
    kind = intent.get("kind")
    body = intent.get("intent")
    if not isinstance(kind, str) or not isinstance(body, dict):
        return ("invalid", "invalid")
    if kind == "dataspace":
        dataspace = body.get("dataspace")
        return (
            kind,
            str(dataspace.get("canonical_name")) if isinstance(dataspace, dict) else "invalid",
        )
    if kind == "domain":
        domain = body.get("domain")
        return (
            kind,
            str(domain.get("canonical_name")) if isinstance(domain, dict) else "invalid",
        )
    if kind == "account_alias":
        alias = body.get("alias")
        canonical = alias.get("canonical_name") if isinstance(alias, dict) else None
        if not isinstance(canonical, dict):
            return (kind, "invalid")
        label = canonical.get("label")
        domain = canonical.get("domain")
        dataspace = canonical.get("dataspace")
        if (
            not isinstance(label, str)
            or (domain is not None and not isinstance(domain, str))
            or not isinstance(dataspace, str)
        ):
            return (kind, "invalid")
        parent = f"{domain}.{dataspace}" if domain is not None else dataspace
        return (kind, f"{label}@{parent}")
    return (kind, "unknown")


def _validate_pristine_overlay_target(genesis: dict[str, Any], inputs: PublicInputs) -> None:
    target_accounts = {
        inputs.onboarding_authority_account_id,
        inputs.api_signer_account_id,
        inputs.dpn_inori_account_id,
        inputs.dpn_epr_guard_account_id,
    }
    forbidden_alias_targets = {
        ("dataspace", DPN_DATASPACE_ALIAS),
        ("dataspace", IS2_DATASPACE_ALIAS),
        ("domain", NEVO_DOMAIN),
        ("account_alias", ADMIN_ACCOUNT_ALIAS),
        ("account_alias", INORI_ACCOUNT_ALIAS),
        ("account_alias", EPR_GUARD_ACCOUNT_ALIAS),
    }
    for instruction in _instructions(genesis):
        if not isinstance(instruction, dict):
            continue
        register = instruction.get("Register")
        if isinstance(register, dict):
            account = register.get("Account")
            if isinstance(account, dict) and account.get("id") in target_accounts:
                fail("base genesis already registers an operator-selected NEVO account")
            domain = register.get("Domain")
            if isinstance(domain, dict) and domain.get("id") == NEVO_DOMAIN:
                fail(f"base genesis already registers `{NEVO_DOMAIN}`")
            role = register.get("Role")
            if isinstance(role, dict) and role.get("id") == GENESIS_ALIAS_BOOTSTRAP_ROLE_ID:
                fail("base genesis already registers the NEVO alias-bootstrap role")
        if _ensure_alias_target(instruction) in forbidden_alias_targets:
            fail("base genesis already contains a NEVO reset alias target")
        grant = instruction.get("Grant")
        permission = grant.get("Permission") if isinstance(grant, dict) else None
        if isinstance(permission, dict) and permission.get("destination") in target_accounts:
            fail("base genesis already grants permissions to an operator-selected NEVO account")
        enrollment = instruction.get("EnrollFeeSponsorBeneficiary")
        if isinstance(enrollment, dict) and enrollment.get("beneficiary") in target_accounts:
            fail("base genesis already enrolls an operator-selected NEVO account")


def _register_account(account_id: str, purpose: str) -> dict[str, Any]:
    return {
        "Register": {
            "Account": {
                "id": account_id,
                "metadata": {"purpose": purpose},
            }
        }
    }


def _mint_fee_asset(asset_definition_id: str, account_id: str) -> dict[str, Any]:
    return {
        "Mint": {
            "Asset": {
                "destination": f"{asset_definition_id}#{account_id}",
                "object": ACCOUNT_FUNDING_AMOUNT,
            }
        }
    }


def _quote_guard(asset_definition_id: str) -> dict[str, Any]:
    return {
        "expected_policy_version": ALIAS_POLICY_VERSION,
        "expected_payment_asset": asset_definition_id,
        "max_amount": ALIAS_MAX_AMOUNT,
        "valid_until_ms": MAX_U64,
    }


def _ensure_dataspace(
    alias: str, dataspace_id: int, owner: str, asset_definition_id: str
) -> dict[str, Any]:
    return {
        "EnsureAlias": {
            "intent": {
                "kind": "dataspace",
                "intent": {
                    "dataspace": {
                        "canonical_name": alias,
                        "dataspace_id": dataspace_id,
                    },
                    "owner": owner,
                },
            },
            "acquisition": {"term_years": 1, "pricing_class_hint": None},
            "quote_guard": _quote_guard(asset_definition_id),
        }
    }


def _ensure_domain(owner: str, asset_definition_id: str) -> dict[str, Any]:
    return {
        "EnsureAlias": {
            "intent": {
                "kind": "domain",
                "intent": {
                    "domain": {
                        "canonical_name": NEVO_DOMAIN,
                        "dataspace_id": DPN_DATASPACE_ID,
                    },
                    "owner": owner,
                },
            },
            "acquisition": {"term_years": 1, "pricing_class_hint": None},
            "quote_guard": _quote_guard(asset_definition_id),
        }
    }


def _ensure_account_alias(
    alias_literal: str, target_account: str, asset_definition_id: str
) -> dict[str, Any]:
    label, separator, dataspace = alias_literal.partition("@")
    if not separator or not label or dataspace != UNIVERSAL_DATASPACE_ALIAS:
        fail("internal NEVO account alias must be a two-segment universal alias")
    return {
        "EnsureAlias": {
            "intent": {
                "kind": "account_alias",
                "intent": {
                    "alias": {
                        "canonical_name": {
                            "label": label,
                            "domain": None,
                            "dataspace": dataspace,
                        },
                        "dataspace_id": UNIVERSAL_DATASPACE_ID,
                    },
                    "target_account": target_account,
                    "provision": {"kind": "existing", "value": None},
                    "role": {"kind": "primary", "value": None},
                },
            },
            "acquisition": {"term_years": 1, "pricing_class_hint": None},
            "quote_guard": _quote_guard(asset_definition_id),
        }
    }


def _exact_account_alias_scope(alias_literal: str) -> dict[str, Any]:
    label, separator, dataspace = alias_literal.partition("@")
    if not separator or not label or dataspace != UNIVERSAL_DATASPACE_ALIAS:
        fail("internal NEVO account alias scope must be a two-segment universal alias")
    return {
        "scope": "alias",
        "value": {
            "canonical_name": {
                "label": label,
                "domain": None,
                "dataspace": dataspace,
            },
            "dataspace_id": UNIVERSAL_DATASPACE_ID,
        },
    }


def _genesis_alias_bootstrap_scopes() -> list[dict[str, Any]]:
    scopes = [
        {"scope": "dataspace", "value": DPN_DATASPACE_ID},
        {"scope": "dataspace", "value": IS2_DATASPACE_ID},
        {"scope": "domain", "value": NEVO_DOMAIN},
        _exact_account_alias_scope(ADMIN_ACCOUNT_ALIAS),
        _exact_account_alias_scope(INORI_ACCOUNT_ALIAS),
        _exact_account_alias_scope(EPR_GUARD_ACCOUNT_ALIAS),
    ]
    # `Role.permissions` is a BTreeSet ordered by the permission name and its
    # canonical JSON payload. Emit that order up front so signing does not
    # reorder the reviewed role payload while binding the genesis manifest.
    return sorted(
        scopes,
        key=lambda scope: json.dumps(
            {"scope": scope}, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ),
    )


def _register_genesis_alias_bootstrap_role(config: BaseConfig) -> dict[str, Any]:
    return {
        "Register": {
            "Role": {
                "id": GENESIS_ALIAS_BOOTSTRAP_ROLE_ID,
                "permissions": [
                    {
                        "name": "CanManageAccountAlias",
                        "payload": {"scope": scope},
                    }
                    for scope in _genesis_alias_bootstrap_scopes()
                ],
                "grant_to": config.genesis_authority_account_id,
            }
        }
    }


def _unregister_genesis_alias_bootstrap_role() -> dict[str, Any]:
    return {
        "Unregister": {
            "Role": {
                "object": GENESIS_ALIAS_BOOTSTRAP_ROLE_ID,
            }
        }
    }


def _grant_permission(destination: str, name: str, payload: Any) -> dict[str, Any]:
    return {
        "Grant": {
            "Permission": {
                "destination": destination,
                "object": {"name": name, "payload": payload},
            }
        }
    }


def _enroll_beneficiary(config: BaseConfig, beneficiary: str) -> dict[str, Any]:
    return {
        "EnrollFeeSponsorBeneficiary": {
            "program_id": _program_object(config),
            "beneficiary": beneficiary,
        }
    }


def _dpn_permission_grants(
    inputs: PublicInputs,
) -> tuple[tuple[str, tuple[str, ...]], ...]:
    return (
        (inputs.api_signer_account_id, ("DpnAdmin", "DpnUser")),
        (
            inputs.dpn_inori_account_id,
            ("DpnInori", "DpnSettlement", "DpnUser"),
        ),
        (inputs.dpn_epr_guard_account_id, ("DpnEprGuard",)),
    )


def nevo_overlay_instructions(inputs: PublicInputs, config: BaseConfig) -> list[dict[str, Any]]:
    """Return the exact deterministic height-1 NEVO overlay instruction list."""

    authority = inputs.onboarding_authority_account_id
    api_signer = inputs.api_signer_account_id
    dpn_inori = inputs.dpn_inori_account_id
    dpn_epr_guard = inputs.dpn_epr_guard_account_id
    asset = config.fee_asset_definition_id
    instructions = [
        _register_account(authority, "nevo_taira_onboarding_authority"),
        _register_account(api_signer, "nevo_dpn_contract_admin_api_signer"),
        _register_account(dpn_inori, "nevo_dpn_inori_controller"),
        _register_account(dpn_epr_guard, "nevo_dpn_epr_source_guard"),
        _mint_fee_asset(asset, authority),
        _mint_fee_asset(asset, api_signer),
        _mint_fee_asset(asset, dpn_inori),
        _mint_fee_asset(asset, dpn_epr_guard),
        _register_genesis_alias_bootstrap_role(config),
        _ensure_dataspace(DPN_DATASPACE_ALIAS, DPN_DATASPACE_ID, authority, asset),
        _ensure_dataspace(IS2_DATASPACE_ALIAS, IS2_DATASPACE_ID, authority, asset),
        _ensure_domain(authority, asset),
        _ensure_account_alias(ADMIN_ACCOUNT_ALIAS, api_signer, asset),
        _ensure_account_alias(INORI_ACCOUNT_ALIAS, dpn_inori, asset),
        _ensure_account_alias(EPR_GUARD_ACCOUNT_ALIAS, dpn_epr_guard, asset),
        _unregister_genesis_alias_bootstrap_role(),
    ]
    instructions.extend(
        [
            _grant_permission(
                authority,
                "CanRegisterAccount",
                {"domain": NEVO_DOMAIN},
            ),
            _grant_permission(
                authority,
                "CanEnrollFeeSponsorProgram",
                {"program_id": _program_object(config)},
            ),
            _grant_permission(api_signer, CONTRACT_DEPLOYMENT_PERMISSION, None),
        ]
    )
    for account_id, permissions in _dpn_permission_grants(inputs):
        instructions.extend(
            _grant_permission(account_id, permission, None)
            for permission in permissions
        )
    instructions.extend(
        _enroll_beneficiary(config, account_id)
        for account_id in (authority, api_signer, dpn_inori, dpn_epr_guard)
    )
    return instructions


def compose_genesis(
    base_genesis: dict[str, Any], inputs: PublicInputs, config: BaseConfig
) -> dict[str, Any]:
    """Return a new unsigned genesis without mutating the caller's object."""

    if config.genesis_authority_account_id in {
        inputs.onboarding_authority_account_id,
        inputs.api_signer_account_id,
        inputs.dpn_inori_account_id,
        inputs.dpn_epr_guard_account_id,
    }:
        fail("a public NEVO account must not reuse the implicit genesis authority account")
    _validate_base_program(base_genesis, config)
    _validate_pristine_overlay_target(base_genesis, inputs)
    # A JSON round trip provides a bounded, type-preserving deep copy while
    # retaining object insertion order on supported Python versions.
    composed = json.loads(json.dumps(base_genesis, ensure_ascii=False))
    composed["transactions"].append(
        {
            "instructions": nevo_overlay_instructions(inputs, config),
            "ivm_triggers": [],
            "topology": [],
        }
    )
    return composed


def _canonical_json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")


def _pretty_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def build_review_manifest(
    *,
    inputs: PublicInputs,
    canonical_inputs: bytes,
    config: BaseConfig,
    base_genesis_bytes: bytes,
    base_config_bytes: bytes,
    unsigned_genesis_bytes: bytes,
    instruction_count: int,
) -> dict[str, Any]:
    """Return the deterministic public review record for one unsigned output."""

    return {
        "schema": REVIEW_SCHEMA,
        "chain": CHAIN_ID,
        "chain_discriminant": CHAIN_DISCRIMINANT,
        "state": "unsigned_operator_review_required",
        "public_inputs_sha256": _sha256(canonical_inputs),
        "base_genesis_sha256": _sha256(base_genesis_bytes),
        "base_config_sha256": _sha256(base_config_bytes),
        "unsigned_genesis_sha256": _sha256(unsigned_genesis_bytes),
        "public_identities": {
            "onboarding_authority_account_id": inputs.onboarding_authority_account_id,
            "api_signer_account_id": inputs.api_signer_account_id,
            "dpn_inori_account_id": inputs.dpn_inori_account_id,
            "dpn_epr_guard_account_id": inputs.dpn_epr_guard_account_id,
        },
        "credential_hash_bindings": [
            {
                "scope": {"dataspace": IS2_DATASPACE_ALIAS},
                "token_hash": inputs.is2_onboarding_token_hash,
            },
            {
                "scope": {"dataspace": DPN_DATASPACE_ALIAS},
                "token_hash": inputs.dpn_onboarding_token_hash,
            },
        ],
        "genesis_overlay": {
            "transaction_count": 1,
            "instruction_count": instruction_count,
            "dataspace_roots": [
                {"alias": DPN_DATASPACE_ALIAS, "id": DPN_DATASPACE_ID},
                {"alias": IS2_DATASPACE_ALIAS, "id": IS2_DATASPACE_ID},
            ],
            "domain": NEVO_DOMAIN,
            "fee_asset_definition_id": config.fee_asset_definition_id,
            "account_funding_amount": ACCOUNT_FUNDING_AMOUNT,
            "fee_sponsor_program_id": config.fee_sponsor_program_id,
            "fee_sponsor_beneficiaries": [
                inputs.onboarding_authority_account_id,
                inputs.api_signer_account_id,
                inputs.dpn_inori_account_id,
                inputs.dpn_epr_guard_account_id,
            ],
            "account_aliases": [
                {
                    "alias": ADMIN_ACCOUNT_ALIAS,
                    "target_account_id": inputs.api_signer_account_id,
                    "role": "primary",
                },
                {
                    "alias": INORI_ACCOUNT_ALIAS,
                    "target_account_id": inputs.dpn_inori_account_id,
                    "role": "primary",
                },
                {
                    "alias": EPR_GUARD_ACCOUNT_ALIAS,
                    "target_account_id": inputs.dpn_epr_guard_account_id,
                    "role": "primary",
                },
            ],
            "genesis_alias_bootstrap": {
                "authority_account_id": config.genesis_authority_account_id,
                "authority_source": "base_config.genesis.public_key",
                "role_id": GENESIS_ALIAS_BOOTSTRAP_ROLE_ID,
                "permissions": [
                    {
                        "name": "CanManageAccountAlias",
                        "payload": {"scope": scope},
                    }
                    for scope in _genesis_alias_bootstrap_scopes()
                ],
                "registered_before_alias_intents": True,
                "unregistered_after_alias_intents": True,
            },
            "contract_deployment_permission_grant": {
                "account_id": inputs.api_signer_account_id,
                "permission": CONTRACT_DEPLOYMENT_PERMISSION,
                "payload": None,
            },
            "dpn_permission_grants": [
                {"account_id": account_id, "permissions": list(permissions)}
                for account_id, permissions in _dpn_permission_grants(inputs)
            ],
            "dpn_settlement_holder_account_id": inputs.dpn_inori_account_id,
            "ensure_alias_derived_owner_permissions": [
                "CanManageAccountAlias",
                "CanDelegateAccountAliasResolution",
                "CanResolveAccountAlias",
            ],
        },
        "secret_boundary": {
            "raw_tokens_accepted": False,
            "private_keys_accepted": False,
            "genesis_signed": False,
        },
        "required_next_steps": [
            "review the exact unsigned genesis and this digest record",
            "bind validator runtime credentials to these exact token hashes",
            "validate the unsigned manifest with the source-matched Kagami binary",
            "sign only through the independently provisioned digest-pinned external signer",
            "deploy and pin the reviewed NEVO contract after the reset finalizes",
            "initialize the application factoring policy after API deployment",
        ],
    }


def _public_inputs_from_review(review: Any) -> PublicInputs:
    if not isinstance(review, dict) or set(review) != EXPECTED_REVIEW_FIELDS:
        fail("NEVO review fields differ from the closed review schema")
    identities = review.get("public_identities")
    if not isinstance(identities, dict) or set(identities) != {
        "onboarding_authority_account_id",
        "api_signer_account_id",
        "dpn_inori_account_id",
        "dpn_epr_guard_account_id",
    }:
        fail("NEVO review public identities are not exact")
    bindings = review.get("credential_hash_bindings")
    if not isinstance(bindings, list) or len(bindings) != 2:
        fail("NEVO review must bind exactly the is2 and DPN credential hashes")
    expected_scopes = (IS2_DATASPACE_ALIAS, DPN_DATASPACE_ALIAS)
    token_hashes: list[str] = []
    for index, (binding, expected_scope) in enumerate(
        zip(bindings, expected_scopes, strict=True)
    ):
        if not isinstance(binding, dict) or set(binding) != {"scope", "token_hash"}:
            fail(f"NEVO review credential binding {index} is not exact")
        if binding.get("scope") != {"dataspace": expected_scope}:
            fail(f"NEVO review credential binding {index} has the wrong scope")
        token_hashes.append(binding["token_hash"])
    payload = {
        "schema": PUBLIC_INPUT_SCHEMA,
        "onboarding_authority_account_id": identities[
            "onboarding_authority_account_id"
        ],
        "api_signer_account_id": identities["api_signer_account_id"],
        "dpn_inori_account_id": identities["dpn_inori_account_id"],
        "dpn_epr_guard_account_id": identities["dpn_epr_guard_account_id"],
        "is2_onboarding_token_hash": token_hashes[0],
        "dpn_onboarding_token_hash": token_hashes[1],
    }
    return _public_inputs_from_payload(payload, "NEVO review public inputs")


def verify_reviewed_payloads(
    *,
    unsigned_genesis_bytes: bytes,
    review_bytes: bytes,
    base_genesis_bytes: bytes,
    base_config_bytes: bytes,
) -> dict[str, Any]:
    """Recompose and byte-verify one standalone NEVO genesis/review pair."""

    review = _parse_json(review_bytes, "NEVO reset review")
    inputs = _public_inputs_from_review(review)
    base_genesis = _parse_base_genesis(base_genesis_bytes)
    config = _parse_base_config(base_config_bytes, base_genesis)
    composed = compose_genesis(base_genesis, inputs, config)
    expected_unsigned = _pretty_json_bytes(composed)
    if unsigned_genesis_bytes != expected_unsigned:
        fail("reviewed unsigned genesis differs from deterministic NEVO recomposition")
    canonical_inputs = _canonical_json_bytes(inputs.as_dict())
    expected_review = build_review_manifest(
        inputs=inputs,
        canonical_inputs=canonical_inputs,
        config=config,
        base_genesis_bytes=base_genesis_bytes,
        base_config_bytes=base_config_bytes,
        unsigned_genesis_bytes=expected_unsigned,
        instruction_count=len(composed["transactions"][-1]["instructions"]),
    )
    if review != expected_review or review_bytes != _pretty_json_bytes(expected_review):
        fail("NEVO reset review differs from the deterministic closed review record")
    return expected_review


def verify_reviewed_files(
    *,
    unsigned_genesis: Path,
    review: Path,
    base_genesis: Path = CHECKED_IN_TAIRA_GENESIS,
    base_config: Path = REPO_ROOT / "configs/soranexus/taira/config.toml",
) -> dict[str, Any]:
    return verify_reviewed_payloads(
        unsigned_genesis_bytes=_read_bounded_regular(
            unsigned_genesis, MAX_OUTPUT_BYTES, "reviewed unsigned NEVO genesis"
        ),
        review_bytes=_read_bounded_regular(
            review, MAX_OUTPUT_BYTES, "NEVO reset review"
        ),
        base_genesis_bytes=_read_bounded_regular(
            base_genesis, MAX_BASE_GENESIS_BYTES, "base Taira genesis"
        ),
        base_config_bytes=_read_bounded_regular(
            base_config, MAX_BASE_CONFIG_BYTES, "base Taira config"
        ),
    )


def _normalized_output_path(path: Path) -> Path:
    try:
        parent = path.parent.resolve(strict=True)
    except OSError as error:
        raise CompositionError(f"output parent does not exist: {path.parent}") from error
    if not parent.is_dir():
        fail(f"output parent is not a directory: {parent}")
    return parent / path.name


def validate_output_paths(
    *,
    public_inputs: Path,
    base_genesis: Path,
    base_config: Path,
    output_genesis: Path,
    review_out: Path,
) -> tuple[Path, Path]:
    """Reject aliasing, existing targets, and any checked-in genesis overwrite."""

    output = _normalized_output_path(output_genesis)
    review = _normalized_output_path(review_out)
    sources = {
        public_inputs.resolve(strict=True),
        base_genesis.resolve(strict=True),
        base_config.resolve(strict=True),
        CHECKED_IN_TAIRA_GENESIS,
    }
    if output in sources or review in sources:
        fail("output paths must not overwrite an input or the checked-in Taira genesis")
    if output == review:
        fail("unsigned genesis and review manifest must use distinct output paths")
    for path in (output, review):
        if path.exists() or path.is_symlink():
            fail(f"refusing to overwrite existing output `{path}`")
    return output, review


def _write_exclusive(path: Path, payload: bytes) -> None:
    if len(payload) > MAX_OUTPUT_BYTES:
        fail(f"output `{path}` exceeds the {MAX_OUTPUT_BYTES}-byte limit")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise
    finally:
        os.close(descriptor)


def _publish_outputs(
    output_genesis: Path,
    genesis_bytes: bytes,
    review_out: Path,
    review_bytes: bytes,
) -> None:
    _write_exclusive(output_genesis, genesis_bytes)
    try:
        _write_exclusive(review_out, review_bytes)
    except BaseException:
        output_genesis.unlink(missing_ok=True)
        raise
    directory = os.open(output_genesis.parent, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def run(args: argparse.Namespace) -> dict[str, Any]:
    """Compose, validate, and optionally publish one unsigned reset candidate."""

    verify_unsigned_genesis = getattr(args, "verify_unsigned_genesis", None)
    verify_review = getattr(args, "verify_review", None)
    if verify_unsigned_genesis is not None or verify_review is not None:
        if verify_unsigned_genesis is None or verify_review is None:
            fail("review verification requires both --verify-unsigned-genesis and --verify-review")
        if (
            args.public_inputs is not None
            or args.output_genesis is not None
            or args.review_out is not None
            or args.dry_run
        ):
            fail("review verification cannot be combined with composition arguments")
        return verify_reviewed_files(
            unsigned_genesis=verify_unsigned_genesis,
            review=verify_review,
            base_genesis=args.base_genesis,
            base_config=args.base_config,
        )
    if args.public_inputs is None:
        fail("composition requires --public-inputs")
    inputs, canonical_inputs = load_public_inputs(args.public_inputs)
    base_genesis, base_genesis_bytes = load_base_genesis(args.base_genesis)
    config, base_config_bytes = load_base_config(args.base_config, base_genesis)
    composed = compose_genesis(base_genesis, inputs, config)
    unsigned_genesis_bytes = _pretty_json_bytes(composed)
    try:
        unsigned_text = unsigned_genesis_bytes.decode("utf-8")
    except UnicodeDecodeError as error:  # pragma: no cover - json always emits UTF-8
        raise CompositionError("generated unsigned genesis is not UTF-8") from error
    _reject_retired_namespace(unsigned_text, "generated unsigned genesis")
    if inputs.is2_onboarding_token_hash in unsigned_text or inputs.dpn_onboarding_token_hash in unsigned_text:
        fail("generated genesis unexpectedly contains onboarding credential hashes")
    instruction_count = len(composed["transactions"][-1]["instructions"])
    review = build_review_manifest(
        inputs=inputs,
        canonical_inputs=canonical_inputs,
        config=config,
        base_genesis_bytes=base_genesis_bytes,
        base_config_bytes=base_config_bytes,
        unsigned_genesis_bytes=unsigned_genesis_bytes,
        instruction_count=instruction_count,
    )
    review_bytes = _pretty_json_bytes(review)
    _reject_retired_namespace(review_bytes.decode("utf-8"), "generated review manifest")
    if args.dry_run:
        if args.output_genesis is not None or args.review_out is not None:
            if args.output_genesis is None or args.review_out is None:
                fail("dry-run output checks require both --output-genesis and --review-out")
            validate_output_paths(
                public_inputs=args.public_inputs,
                base_genesis=args.base_genesis,
                base_config=args.base_config,
                output_genesis=args.output_genesis,
                review_out=args.review_out,
            )
        return review
    if args.output_genesis is None or args.review_out is None:
        fail("non-dry-run composition requires --output-genesis and --review-out")
    output, review_path = validate_output_paths(
        public_inputs=args.public_inputs,
        base_genesis=args.base_genesis,
        base_config=args.base_config,
        output_genesis=args.output_genesis,
        review_out=args.review_out,
    )
    _publish_outputs(output, unsigned_genesis_bytes, review_path, review_bytes)
    return review


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    argument_parser = argparse.ArgumentParser(
        description=(
            "Compose a separate unsigned NEVO-only Taira reset genesis from "
            "public account IDs and BLAKE3 token hashes."
        )
    )
    argument_parser.add_argument(
        "--public-inputs",
        type=Path,
        help="strict public-only JSON input document",
    )
    argument_parser.add_argument(
        "--verify-unsigned-genesis",
        type=Path,
        help="existing standalone NEVO unsigned genesis to verify",
    )
    argument_parser.add_argument(
        "--verify-review",
        type=Path,
        help="existing deterministic NEVO review manifest to verify",
    )
    argument_parser.add_argument(
        "--base-genesis",
        type=Path,
        default=CHECKED_IN_TAIRA_GENESIS,
        help="read-only generic Taira genesis template",
    )
    argument_parser.add_argument(
        "--base-config",
        type=Path,
        default=REPO_ROOT / "configs/soranexus/taira/config.toml",
        help="read-only Taira config used to verify catalog and sponsor invariants",
    )
    argument_parser.add_argument(
        "--output-genesis",
        type=Path,
        help="new path for the unsigned composed genesis; existing paths are refused",
    )
    argument_parser.add_argument(
        "--review-out",
        type=Path,
        help="new path for the deterministic public review manifest",
    )
    argument_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="validate and print the review manifest without writing output files",
    )
    return argument_parser


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint."""

    args = parser().parse_args(argv)
    try:
        review = run(args)
    except (CompositionError, OSError) as error:
        print(f"refused: {error}", file=sys.stderr)
        return 2
    if args.verify_review is not None:
        print(f"verified_nevo_review_sha256={_sha256(_read_bounded_regular(args.verify_review, MAX_OUTPUT_BYTES, 'NEVO reset review'))}")
    elif args.dry_run:
        print(json.dumps(review, ensure_ascii=False, indent=2))
    else:
        print(f"unsigned_genesis_sha256={review['unsigned_genesis_sha256']}")
        print(f"review_schema={review['schema']}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI boundary
    raise SystemExit(main())
