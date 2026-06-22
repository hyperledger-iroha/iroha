#!/usr/bin/env python3
"""Collect read-only SCCP EVM-family destination evidence from JSON-RPC."""

from __future__ import annotations

import argparse
from functools import lru_cache
import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import sccp_evm_destination_evidence as evidence  # noqa: E402


Urlopen = Callable[..., Any]
EVM_JSON_RPC_MAX_RESPONSE_BYTES = 1024 * 1024
EVM_JSON_RPC_MAX_ERROR_BYTES = 4096

BRIDGE_VIEW_SIGNATURES = (
    "verifier()",
    "verifierCodeHash()",
    "verifierKeyHash()",
    "verifierBackendHash()",
    "proofFamilyHash()",
    "networkId()",
    "expectedSourceDomain()",
    "expectedTargetDomain()",
    "destinationBindingHash()",
)

EXPECTED_RPC_CHAIN_IDS = {
    evidence.SCCP_DOMAIN_ETH: 1,
    evidence.SCCP_DOMAIN_BSC: 56,
}
EVM_LIVE_ALLOWED_BLOCK_TAGS = frozenset(("latest", "safe", "finalized"))
PUBLIC_SUMMARY_FIELDS = (
    "read_only",
    "block_tag",
    "destination_bridge",
    "route_allowlist_hash",
    "source_record_hashes",
    "expected_route_allowlist_hash",
    "expected_route_allowlist_hash_matches",
    "route_canary",
    "route_canary_transaction",
    "offline_toml_sha256",
    "offline_evidence_args",
    "torii_destination_query_params",
    "torii_destination_query_proof_bytes_hex_required",
)
EVM_MESSAGE_PROOF_ACCEPTED_ABI = (
    b"MessageProofAccepted(bytes32,uint32,bytes32,bytes32,bytes32,bytes32,bytes32,bytes32)"
)
EVM_MESSAGE_PROOF_ACCEPTED_TOPIC = evidence._keccak_256(
    EVM_MESSAGE_PROOF_ACCEPTED_ABI
)
EVM_SUBMIT_MESSAGE_PROOF_SELECTOR = evidence._keccak_256(
    b"submitSccpMessageProof(bytes,bytes32[6],bytes32)"
)[:4]
EVM_GROTH16_PROOF_VERSION = 1
EVM_GROTH16_PROOF_ABI_BYTE_LENGTH = 32 * 12
BN254_BASE_FIELD_MODULUS = int(
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
    16,
)
BN254_SCALAR_FIELD_MODULUS = int(
    "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001",
    16,
)
BN254_G2_B_C0 = int(
    "2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5",
    16,
)
BN254_G2_B_C1 = int(
    "009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2",
    16,
)
BN254_SCALAR_FIELD_BITS = tuple(
    1 if symbol == "1" else 0 for symbol in bin(BN254_SCALAR_FIELD_MODULUS)[2:]
)


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _selector(signature: str) -> str:
    return "0x" + evidence._keccak_256(signature.encode("utf-8"))[:4].hex()


def _parse_hex_bytes(value: str, *, label: str, byte_length: int) -> bytes:
    return evidence.parse_hex_bytes(value, label=label, byte_length=byte_length)


def _parse_hex32(value: str, *, label: str) -> bytes:
    return _parse_hex_bytes(value, label=label, byte_length=32)


def _summary_hex_bytes(
    record: dict[str, Any],
    field: str,
    *,
    label: str,
    byte_length: int,
) -> bytes:
    value = record.get(field)
    if not isinstance(value, str):
        raise ValueError(f"{label} must be an exact hex string")
    try:
        raw = _parse_hex_bytes(value, label=label, byte_length=byte_length)
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        raise ValueError(f"{label} metadata is invalid") from None
    if value != _hex(raw):
        raise ValueError(f"{label} must be canonical lowercase 0x hex")
    return raw


def _summary_hex32(record: dict[str, Any], field: str, *, label: str) -> bytes:
    return _summary_hex_bytes(record, field, label=label, byte_length=32)


def _summary_address(record: dict[str, Any], field: str, *, label: str) -> bytes:
    return _summary_hex_bytes(record, field, label=label, byte_length=20)


def _summary_runtime_bytes(record: dict[str, Any], field: str, *, label: str) -> bytes:
    value = record.get(field)
    if not isinstance(value, str) or not value.startswith("0x"):
        raise ValueError(f"{label} must be exact 0x-prefixed hex")
    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise ValueError(f"{label} must not contain whitespace")
    invalid_metadata_errors = {
        "bridge runtime bytecode": "EVM bridge runtime bytecode metadata is invalid",
        "verifier runtime bytecode": "EVM verifier runtime bytecode metadata is invalid",
    }
    try:
        raw = evidence.parse_runtime_bytecode_hex(value, label=label)
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        raise ValueError(
            invalid_metadata_errors.get(label, f"EVM {label} metadata is invalid")
        ) from None
    if value != "0x" + raw.hex():
        raise ValueError(f"{label} must be canonical lowercase 0x hex")
    return raw


def _parse_address_text(value: str, *, label: str) -> str:
    return _hex(evidence.parse_evm_address(value, label=label))


def _parse_rpc_chain_id(value: str) -> int:
    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    parsed = int(text, 10)
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a positive u64 integer"
        )
    return parsed


def parse_block_tag(value: str) -> str:
    """Parse a stable/canonical JSON-RPC block tag for read-only evidence."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "--block-tag must not contain surrounding whitespace"
        )
    if value in EVM_LIVE_ALLOWED_BLOCK_TAGS:
        return value
    if value.startswith("0x"):
        text = value[2:]
        if (
            not text
            or any(symbol not in "0123456789abcdef" for symbol in text)
            or (len(text) > 1 and text.startswith("0"))
        ):
            raise argparse.ArgumentTypeError(
                "--block-tag must be latest, safe, finalized, or a positive "
                "canonical lowercase 0x block number"
            )
        parsed = int(text, 16)
        if parsed <= 0:
            raise argparse.ArgumentTypeError(
                "--block-tag block number must be positive"
            )
        return "0x" + format(parsed, "x")
    raise argparse.ArgumentTypeError(
        "--block-tag must be latest, safe, finalized, or a positive canonical "
        "lowercase 0x block number"
    )


def _default_rpc_chain_id_for_domain(domain: int) -> int:
    try:
        return EXPECTED_RPC_CHAIN_IDS[domain]
    except (KeyError, TypeError, ValueError, argparse.ArgumentTypeError):
        raise argparse.ArgumentTypeError(
            "domain must have a canonical RPC chain id"
        ) from None


def default_block_tag_for_domain(domain: int) -> str:
    """Return the default live-read block tag for an EVM-family destination lane."""

    return "finalized" if domain == evidence.SCCP_DOMAIN_ETH else "latest"


def _default_network_id_for_domain(domain: int) -> bytes:
    try:
        return evidence.evm_mainnet_network_id_for_domain(domain)
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        raise argparse.ArgumentTypeError(
            "domain must have a canonical EVM mainnet network id"
        ) from None


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    decoded: dict[str, Any] = {}
    for key, value in pairs:
        if key in decoded:
            raise ValueError("JSON-RPC returned duplicate JSON keys")
        decoded[key] = value
    return decoded


def _json_rpc(
    rpc_url: str,
    method: str,
    params: list[Any],
    *,
    opener: Urlopen,
    timeout: float,
) -> Any:
    request = urllib.request.Request(
        rpc_url,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params},
            separators=(",", ":"),
        ).encode("utf-8"),
        headers={"Accept": "application/json", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with opener(request, timeout=timeout) as response:
            raw = response.read(EVM_JSON_RPC_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(
            f"JSON-RPC {method} failed with HTTP {exc.code}"
        ) from None
    except urllib.error.URLError:
        raise RuntimeError(f"JSON-RPC {method} request failed") from None
    if len(raw) > EVM_JSON_RPC_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"JSON-RPC {method} response exceeds "
            f"{EVM_JSON_RPC_MAX_RESPONSE_BYTES} bytes"
        )
    try:
        decoded = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
    except UnicodeDecodeError:
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from None
    except json.JSONDecodeError:
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from None
    except ValueError as exc:
        if str(exc) == "JSON-RPC returned duplicate JSON keys":
            raise RuntimeError(f"JSON-RPC {method} returned duplicate JSON keys") from None
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from None
    if not isinstance(decoded, dict):
        raise RuntimeError(f"JSON-RPC {method} returned a non-object response")
    if decoded.get("jsonrpc") != "2.0":
        raise RuntimeError(f"JSON-RPC {method} returned an invalid protocol version")
    if decoded.get("id") != 1:
        raise RuntimeError(f"JSON-RPC {method} returned a mismatched response id")
    error = decoded.get("error")
    if error is not None:
        raise RuntimeError(f"JSON-RPC {method} returned error response")
    if "result" not in decoded:
        raise RuntimeError(f"JSON-RPC {method} returned no result")
    return decoded["result"]


def _rpc_hex_data(result: Any, *, method: str) -> bytes:
    if not isinstance(result, str):
        raise RuntimeError(f"{method} returned non-string data")
    if result != result.strip():
        raise RuntimeError(f"{method} returned non-canonical hex data")
    if not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    text = result[2:]
    if len(text) % 2 != 0:
        raise RuntimeError(f"{method} returned odd-length hex")
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    try:
        return bytes.fromhex(text)
    except (TypeError, ValueError):
        raise RuntimeError(
            f"{method} returned non-canonical lowercase 0x hex data"
        ) from None


def _rpc_quantity(result: Any, *, method: str) -> int:
    if not isinstance(result, str) or not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-quantity data")
    if result != result.strip():
        raise RuntimeError(f"{method} returned non-canonical quantity")
    text = result[2:]
    if (
        not text
        or any(symbol not in "0123456789abcdef" for symbol in text)
        or (len(text) > 1 and text.startswith("0"))
    ):
        raise RuntimeError(f"{method} returned non-canonical quantity")
    return int(text, 16)


def _parse_exact_hex_blob(value: Any, *, label: str, nonzero: bool = True) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be hex")
    if value != value.strip():
        raise RuntimeError(f"{label} must not contain surrounding whitespace")
    if not value.startswith("0x"):
        raise RuntimeError(f"{label} must be canonical lowercase 0x hex")
    text = value[2:]
    if len(text) % 2 != 0:
        raise RuntimeError(f"{label} must contain an even number of hex digits")
    if any(symbol.isspace() for symbol in text):
        raise RuntimeError(f"{label} must not contain whitespace")
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{label} must be canonical lowercase 0x hex")
    try:
        parsed = bytes.fromhex(text)
    except (TypeError, ValueError):
        raise RuntimeError(f"{label} must be canonical lowercase 0x hex") from None
    if nonzero and not any(parsed):
        raise RuntimeError(f"{label} must not be zero")
    return parsed


def _parse_exact_hex32_blob(
    value: Any,
    *,
    label: str,
    nonzero: bool = True,
) -> bytes:
    parsed = _parse_exact_hex_blob(value, label=label, nonzero=False)
    if len(parsed) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    if nonzero and not any(parsed):
        raise RuntimeError(f"{label} must not be zero")
    return parsed


def _parse_exact_address(value: Any, *, label: str) -> bytes:
    parsed = _parse_exact_hex_blob(value, label=label)
    if len(parsed) != 20:
        raise RuntimeError(f"{label} must be 20 bytes")
    return parsed


def _parse_abi_data_words(
    value: Any,
    *,
    label: str,
    word_count: int,
) -> tuple[bytes, ...]:
    data = _parse_exact_hex_blob(value, label=label, nonzero=False)
    expected_len = 32 * word_count
    if len(data) != expected_len:
        raise RuntimeError(f"{label} must contain {word_count} ABI words")
    return tuple(data[index : index + 32] for index in range(0, expected_len, 32))


def _require_nonzero_word(word: bytes, *, label: str) -> bytes:
    if len(word) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    if not any(word):
        raise RuntimeError(f"{label} must not be zero")
    return word


def _word_u256(word: bytes, *, label: str) -> int:
    if len(word) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    return int.from_bytes(word, "big")


def _word_bool(word: bytes, *, label: str) -> bool:
    value = _word_u256(word, label=label)
    if value == 0:
        return False
    if value == 1:
        return True
    raise RuntimeError(f"{label} must be an ABI bool")


def _parse_route_canary_log_index(value: str) -> int:
    return evidence.parse_u32_decimal(value, label="route canary log index")


def _eth_call_word(
    rpc_url: str,
    *,
    to_address: str,
    signature: str,
    block_tag: str,
    opener: Urlopen,
    timeout: float,
) -> bytes:
    result = _json_rpc(
        rpc_url,
        "eth_call",
        [{"to": to_address, "data": _selector(signature)}, block_tag],
        opener=opener,
        timeout=timeout,
    )
    word = _rpc_hex_data(result, method=f"eth_call {signature}")
    if len(word) != 32:
        raise RuntimeError(f"eth_call {signature} must return one ABI word")
    return word


def _word_u32(word: bytes, *, label: str) -> int:
    value = int.from_bytes(word, "big")
    if value > 0xFFFFFFFF:
        raise RuntimeError(f"{label} does not fit u32")
    return value


def _bn254_fq(value: int) -> int:
    return value % BN254_BASE_FIELD_MODULUS


def _bn254_fq2_add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    return (_bn254_fq(left[0] + right[0]), _bn254_fq(left[1] + right[1]))


def _bn254_fq2_sub(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    return (_bn254_fq(left[0] - right[0]), _bn254_fq(left[1] - right[1]))


def _bn254_fq2_scale(left: tuple[int, int], scalar: int) -> tuple[int, int]:
    return (_bn254_fq(left[0] * scalar), _bn254_fq(left[1] * scalar))


def _bn254_fq2_mul(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    return (
        _bn254_fq(left[0] * right[0] - left[1] * right[1]),
        _bn254_fq(left[0] * right[1] + left[1] * right[0]),
    )


def _bn254_fq2_is_zero(value: tuple[int, int]) -> bool:
    return value == (0, 0)


def _bn254_g2_infinity() -> tuple[
    tuple[int, int],
    tuple[int, int],
    tuple[int, int],
    bool,
]:
    return ((0, 0), (1, 0), (0, 0), True)


def _bn254_g2_projective_is_infinity(
    point: tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool],
) -> bool:
    return point[3] or _bn254_fq2_is_zero(point[2])


def _bn254_g2_affine_projective(
    x: tuple[int, int],
    y: tuple[int, int],
) -> tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool]:
    return (x, y, (1, 0), False)


def _bn254_g2_projective_double(
    point: tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool],
) -> tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool]:
    if _bn254_g2_projective_is_infinity(point) or _bn254_fq2_is_zero(point[1]):
        return _bn254_g2_infinity()
    x, y, z, _ = point
    xx = _bn254_fq2_mul(x, x)
    yy = _bn254_fq2_mul(y, y)
    yyyy = _bn254_fq2_mul(yy, yy)
    s = _bn254_fq2_scale(
        _bn254_fq2_sub(
            _bn254_fq2_sub(
                _bn254_fq2_mul(_bn254_fq2_add(x, yy), _bn254_fq2_add(x, yy)),
                xx,
            ),
            yyyy,
        ),
        2,
    )
    m = _bn254_fq2_scale(xx, 3)
    x3 = _bn254_fq2_sub(_bn254_fq2_mul(m, m), _bn254_fq2_scale(s, 2))
    y3 = _bn254_fq2_sub(
        _bn254_fq2_mul(m, _bn254_fq2_sub(s, x3)),
        _bn254_fq2_scale(yyyy, 8),
    )
    z3 = _bn254_fq2_scale(_bn254_fq2_mul(y, z), 2)
    return (x3, y3, z3, False)


def _bn254_g2_projective_add_affine(
    point: tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool],
    affine_x: tuple[int, int],
    affine_y: tuple[int, int],
) -> tuple[tuple[int, int], tuple[int, int], tuple[int, int], bool]:
    if _bn254_g2_projective_is_infinity(point):
        return _bn254_g2_affine_projective(affine_x, affine_y)
    x, y, z, _ = point
    z1z1 = _bn254_fq2_mul(z, z)
    u2 = _bn254_fq2_mul(affine_x, z1z1)
    s2 = _bn254_fq2_mul(affine_y, _bn254_fq2_mul(z, z1z1))
    h = _bn254_fq2_sub(u2, x)
    if _bn254_fq2_is_zero(h):
        if s2 == y:
            return _bn254_g2_projective_double(point)
        return _bn254_g2_infinity()
    hh = _bn254_fq2_mul(h, h)
    i = _bn254_fq2_scale(hh, 4)
    j = _bn254_fq2_mul(h, i)
    r = _bn254_fq2_scale(_bn254_fq2_sub(s2, y), 2)
    v = _bn254_fq2_mul(x, i)
    x3 = _bn254_fq2_sub(
        _bn254_fq2_sub(_bn254_fq2_mul(r, r), j),
        _bn254_fq2_scale(v, 2),
    )
    y3 = _bn254_fq2_sub(
        _bn254_fq2_mul(r, _bn254_fq2_sub(v, x3)),
        _bn254_fq2_scale(_bn254_fq2_mul(y, j), 2),
    )
    z3 = _bn254_fq2_sub(
        _bn254_fq2_sub(
            _bn254_fq2_mul(_bn254_fq2_add(z, h), _bn254_fq2_add(z, h)),
            z1z1,
        ),
        hh,
    )
    return (x3, y3, z3, False)


@lru_cache(maxsize=32)
def _bn254_g2_point_is_in_prime_subgroup(x0: int, x1: int, y0: int, y1: int) -> bool:
    x = (x0, x1)
    y = (y0, y1)
    acc = _bn254_g2_infinity()
    for bit in BN254_SCALAR_FIELD_BITS:
        acc = _bn254_g2_projective_double(acc)
        if bit:
            acc = _bn254_g2_projective_add_affine(acc, x, y)
    return _bn254_g2_projective_is_infinity(acc)


def _proof_word_u256(proof_words: tuple[bytes, ...], index: int) -> int:
    return int.from_bytes(proof_words[index], "big")


def _proof_word_is_zero(proof_words: tuple[bytes, ...], index: int) -> bool:
    return not any(proof_words[index])


def _require_route_canary_bn254_base_field_word(
    proof_words: tuple[bytes, ...],
    index: int,
    *,
    label: str,
) -> None:
    if _proof_word_u256(proof_words, index) >= BN254_BASE_FIELD_MODULUS:
        raise RuntimeError(
            f"route-canary {label} must be a BN254 base-field element"
        )


def _require_route_canary_bn254_nonzero_point(
    proof_words: tuple[bytes, ...],
    indexes: tuple[int, ...],
    *,
    label: str,
) -> None:
    if all(_proof_word_is_zero(proof_words, index) for index in indexes):
        raise RuntimeError(f"route-canary {label} must not be zero")


def _require_route_canary_bn254_g1_point(
    proof_words: tuple[bytes, ...],
    indexes: tuple[int, int],
    *,
    label: str,
) -> None:
    _require_route_canary_bn254_nonzero_point(
        proof_words,
        indexes,
        label=label,
    )
    x = _proof_word_u256(proof_words, indexes[0])
    y = _proof_word_u256(proof_words, indexes[1])
    left = _bn254_fq(y * y)
    right = _bn254_fq(x * x * x + 3)
    if left != right:
        raise RuntimeError(f"route-canary {label} must be a BN254 G1 point")


def _require_route_canary_bn254_g2_point(
    proof_words: tuple[bytes, ...],
    indexes: tuple[int, int, int, int],
    *,
    label: str,
) -> None:
    _require_route_canary_bn254_nonzero_point(
        proof_words,
        indexes,
        label=label,
    )
    x = (
        _proof_word_u256(proof_words, indexes[0]),
        _proof_word_u256(proof_words, indexes[1]),
    )
    y = (
        _proof_word_u256(proof_words, indexes[2]),
        _proof_word_u256(proof_words, indexes[3]),
    )
    left = _bn254_fq2_mul(y, y)
    x2 = _bn254_fq2_mul(x, x)
    right = _bn254_fq2_add(_bn254_fq2_mul(x2, x), (BN254_G2_B_C0, BN254_G2_B_C1))
    if left != right or not _bn254_g2_point_is_in_prime_subgroup(*x, *y):
        raise RuntimeError(f"route-canary {label} must be a BN254 G2 point")


def _require_route_canary_groth16_bn254_proof_tuple(
    proof_words: tuple[bytes, ...],
) -> None:
    for offset, field in enumerate(
        ("a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y")
    ):
        _require_route_canary_bn254_base_field_word(
            proof_words,
            4 + offset,
            label=f"proofBytes.{field}",
        )
    _require_route_canary_bn254_g1_point(proof_words, (4, 5), label="proofBytes.a")
    _require_route_canary_bn254_g2_point(
        proof_words,
        (6, 7, 8, 9),
        label="proofBytes.b",
    )
    _require_route_canary_bn254_g1_point(proof_words, (10, 11), label="proofBytes.c")


def _word_address(word: bytes, *, label: str) -> str:
    if len(word) != 32 or any(word[:12]):
        raise RuntimeError(f"{label} must be an ABI-encoded address")
    address = word[12:]
    if not any(address):
        raise RuntimeError(f"{label} must not be zero")
    return _hex(address)


def _runtime_code_hash(
    rpc_url: str,
    *,
    address: str,
    block_tag: str,
    opener: Urlopen,
    timeout: float,
    label: str,
) -> tuple[bytes, str]:
    result = _json_rpc(
        rpc_url,
        "eth_getCode",
        [address, block_tag],
        opener=opener,
        timeout=timeout,
    )
    runtime = _rpc_hex_data(result, method=f"eth_getCode {label}")
    if not runtime or not any(runtime):
        raise RuntimeError(f"{label} runtime bytecode is empty")
    return evidence.runtime_bytecode_hash(runtime), "0x" + runtime.hex()


def _expected_hashes() -> tuple[bytes, bytes]:
    return (
        evidence._keccak_256(evidence.SCCP_EVM_GROTH16_BACKEND.encode("utf-8")),
        evidence._keccak_256(evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")),
    )


def collect_destination_bridge_evidence(
    rpc_url: str,
    *,
    domain: int,
    bridge_address: str,
    block_tag: str,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect and verify read-only EVM destination bridge evidence."""

    block_tag = parse_block_tag(block_tag)
    bridge = _parse_address_text(bridge_address, label="bridge address")
    chain_id = _rpc_quantity(
        _json_rpc(
            rpc_url,
            "eth_chainId",
            [],
            opener=opener,
            timeout=timeout,
        ),
        method="eth_chainId",
    )
    expected_chain_id = _default_rpc_chain_id_for_domain(domain)
    if chain_id != expected_chain_id:
        chain = evidence.DOMAIN_PROFILES[domain]["chain"]
        raise ValueError(
            f"eth_chainId for {chain} lane must be canonical mainnet chain id "
            f"{expected_chain_id}, got {chain_id}"
        )
    bridge_code_hash, bridge_runtime_bytecode_hex = _runtime_code_hash(
        rpc_url,
        address=bridge,
        block_tag=block_tag,
        opener=opener,
        timeout=timeout,
        label="bridge",
    )
    words = {
        signature: _eth_call_word(
            rpc_url,
            to_address=bridge,
            signature=signature,
            block_tag=block_tag,
            opener=opener,
            timeout=timeout,
        )
        for signature in BRIDGE_VIEW_SIGNATURES
    }
    verifier = _word_address(words["verifier()"], label="verifier()")
    if verifier.lower() == bridge.lower():
        raise RuntimeError("destination verifier address must differ from bridge address")
    verifier_code_hash = words["verifierCodeHash()"]
    verifier_key_hash = words["verifierKeyHash()"]
    backend_hash = words["verifierBackendHash()"]
    proof_family_hash = words["proofFamilyHash()"]
    network_id = words["networkId()"]
    source_domain = _word_u32(words["expectedSourceDomain()"], label="expectedSourceDomain()")
    target_domain = _word_u32(words["expectedTargetDomain()"], label="expectedTargetDomain()")
    observed_destination_binding_hash = words["destinationBindingHash()"]

    if source_domain != evidence.SCCP_DOMAIN_SORA:
        raise RuntimeError("expectedSourceDomain() must be SORA")
    if target_domain != domain:
        raise RuntimeError("expectedTargetDomain() does not match requested lane")
    expected_backend_hash, expected_family_hash = _expected_hashes()
    if backend_hash != expected_backend_hash:
        raise RuntimeError("verifierBackendHash() is not evm-groth16-bn254-v1")
    if proof_family_hash != expected_family_hash:
        raise RuntimeError("proofFamilyHash() is not stark-fri-v1")

    live_verifier_code_hash, verifier_runtime_bytecode_hex = _runtime_code_hash(
        rpc_url,
        address=verifier,
        block_tag=block_tag,
        opener=opener,
        timeout=timeout,
        label="verifier",
    )
    if verifier_code_hash != live_verifier_code_hash:
        raise RuntimeError(
            "verifierCodeHash() does not match eth_getCode runtime bytecode: "
            f"expected {_hex(verifier_code_hash)}, got {_hex(live_verifier_code_hash)}"
        )
    live_verifier_key_hash = _eth_call_word(
        rpc_url,
        to_address=verifier,
        signature="verifyingKeyHash()",
        block_tag=block_tag,
        opener=opener,
        timeout=timeout,
    )
    if verifier_key_hash != live_verifier_key_hash:
        raise RuntimeError(
            "verifierKeyHash() does not match verifier verifyingKeyHash(): "
            f"expected {_hex(verifier_key_hash)}, got {_hex(live_verifier_key_hash)}"
        )

    destination_binding_hash = evidence.evm_destination_binding_hash(
        network_id=network_id,
        source_domain=evidence.SCCP_DOMAIN_SORA,
        target_domain=domain,
        verifier_address=evidence.parse_evm_address(verifier, label="verifier address"),
        bridge_address=evidence.parse_evm_address(bridge, label="bridge address"),
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    if observed_destination_binding_hash != destination_binding_hash:
        raise RuntimeError(
            "destinationBindingHash() does not match canonical live deployment inputs: "
            f"expected {_hex(destination_binding_hash)}, "
            f"got {_hex(observed_destination_binding_hash)}"
        )
    destination_binding_key = evidence.evm_destination_binding_key(
        network_id=network_id,
        source_domain=evidence.SCCP_DOMAIN_SORA,
        target_domain=domain,
        verifier_address=evidence.parse_evm_address(verifier, label="verifier address"),
        bridge_address=evidence.parse_evm_address(bridge, label="bridge address"),
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    return {
        "domain": domain,
        "chain": evidence.DOMAIN_PROFILES[domain]["chain"],
        "rpc_chain_id": chain_id,
        "bridge_address": bridge,
        "bridge_code_hash": _hex(bridge_code_hash),
        "bridge_runtime_bytecode_hex": bridge_runtime_bytecode_hex,
        "verifier_address": verifier,
        "network_id": _hex(network_id),
        "source_domain": source_domain,
        "target_domain": target_domain,
        "verifier_code_hash": _hex(verifier_code_hash),
        "eth_getcode_verifier_code_hash": _hex(live_verifier_code_hash),
        "verifier_runtime_bytecode_hex": verifier_runtime_bytecode_hex,
        "verifier_key_hash": _hex(verifier_key_hash),
        "verifier_verifying_key_hash": _hex(live_verifier_key_hash),
        "verifier_backend_hash": _hex(backend_hash),
        "proof_family_hash": _hex(proof_family_hash),
        "destination_binding_key": destination_binding_key,
        "destination_binding_hash": _hex(observed_destination_binding_hash),
        "destination_binding_hash_recomputed": True,
        "destination_binding_hash_matches_bridge_view": True,
    }


def _route_canary_message_proof_event_summary(
    log: dict[str, Any],
    *,
    expected_log_index: int,
    transaction_hash: bytes,
    expected_block_hash: bytes,
    expected_block_number: int,
    route_allowlist_hash: bytes,
    bridge_address: bytes,
    expected_source_domain: int,
    expected_target_domain: int,
    expected_destination_binding_hash: bytes,
    expected_verifier_backend_hash: bytes,
    expected_proof_family_hash: bytes,
    expected_network_id: bytes,
) -> dict[str, Any] | None:
    try:
        log_address = _parse_exact_address(
            log.get("address"),
            label="route-canary log address",
        )
    except RuntimeError:
        return None
    topics = log.get("topics")
    if not isinstance(topics, list) or not topics:
        return None
    if not all(isinstance(topic, str) for topic in topics):
        return None
    try:
        topic0 = _parse_exact_hex32_blob(
            topics[0],
            label="route-canary log topic0",
        )
    except RuntimeError:
        return None
    if log_address != bridge_address or topic0 != EVM_MESSAGE_PROOF_ACCEPTED_TOPIC:
        return None
    log_index = _rpc_quantity(
        log.get("logIndex"),
        method="route-canary logIndex",
    )
    if log_index != expected_log_index:
        raise RuntimeError(
            "route-canary transaction receipt contained a MessageProofAccepted "
            "event at an unexpected log index"
        )
    log_transaction_hash = _parse_exact_hex32_blob(
        log.get("transactionHash"),
        label="route-canary log transactionHash",
    )
    if log_transaction_hash != transaction_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted log transactionHash does not "
            "match receipt transactionHash"
        )
    log_block_hash = _parse_exact_hex32_blob(
        log.get("blockHash"),
        label="route-canary log blockHash",
    )
    if log_block_hash != expected_block_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted log blockHash does not match "
            "receipt blockHash"
        )
    log_block_number = _rpc_quantity(
        log.get("blockNumber"),
        method="route-canary log blockNumber",
    )
    if log_block_number != expected_block_number:
        raise RuntimeError(
            "route-canary MessageProofAccepted log blockNumber does not match "
            "receipt blockNumber"
        )
    if len(topics) != 3:
        raise RuntimeError(
            "route-canary MessageProofAccepted log must contain exactly three topics"
        )
    message_id = _require_nonzero_word(
        _parse_exact_hex32_blob(
            topics[1],
            label="route-canary messageId topic",
        ),
        label="route-canary messageId",
    )
    source_domain_word = _parse_exact_hex32_blob(
        topics[2],
        label="route-canary sourceDomain topic",
        nonzero=False,
    )
    source_domain = _word_u32(
        source_domain_word,
        label="route-canary sourceDomain topic",
    )
    if source_domain != expected_source_domain:
        raise RuntimeError(
            "route-canary MessageProofAccepted sourceDomain does not match "
            "expectedSourceDomain(): "
            f"expected {expected_source_domain}, got {source_domain}"
        )
    (
        commitment_root,
        statement_hash,
        destination_binding_hash,
        verifier_backend_hash,
        proof_family_hash,
        network_id,
    ) = _parse_abi_data_words(
        log.get("data"),
        label="route-canary MessageProofAccepted data",
        word_count=6,
    )
    commitment_root = _require_nonzero_word(
        commitment_root,
        label="route-canary commitmentRoot",
    )
    statement_hash = _require_nonzero_word(
        statement_hash,
        label="route-canary statementHash",
    )
    destination_binding_hash = _require_nonzero_word(
        destination_binding_hash,
        label="route-canary destinationBindingHash",
    )
    verifier_backend_hash = _require_nonzero_word(
        verifier_backend_hash,
        label="route-canary verifierBackendHash",
    )
    proof_family_hash = _require_nonzero_word(
        proof_family_hash,
        label="route-canary proofFamilyHash",
    )
    network_id = _require_nonzero_word(network_id, label="route-canary networkId")
    if destination_binding_hash != expected_destination_binding_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted destinationBindingHash does not "
            "match live destinationBindingHash()"
        )
    if verifier_backend_hash != expected_verifier_backend_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted verifierBackendHash does not "
            "match verifierBackendHash()"
        )
    if proof_family_hash != expected_proof_family_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted proofFamilyHash does not match "
            "proofFamilyHash()"
        )
    if network_id != expected_network_id:
        raise RuntimeError(
            "route-canary MessageProofAccepted networkId does not match networkId()"
        )
    return {
        "transaction_hash": _hex(transaction_hash),
        "log_index": log_index,
        "event_address": _hex(log_address),
        "event_topic0": _hex(topic0),
        "message_id": _hex(message_id),
        "source_domain": source_domain,
        "target_domain": expected_target_domain,
        "commitment_root": _hex(commitment_root),
        "statement_hash": _hex(statement_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "verifier_backend_hash": _hex(verifier_backend_hash),
        "proof_family_hash": _hex(proof_family_hash),
        "network_id": _hex(network_id),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "event_matches": True,
    }


def _route_canary_submit_call_data_summary(
    call_data: bytes,
    *,
    event_summary: dict[str, Any],
    expected_source_domain: int,
    expected_target_domain: int,
) -> dict[str, Any]:
    if not call_data.startswith(EVM_SUBMIT_MESSAGE_PROOF_SELECTOR):
        raise RuntimeError(
            "route-canary transaction calldata must call "
            "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
        )
    body = call_data[len(EVM_SUBMIT_MESSAGE_PROOF_SELECTOR) :]
    if len(body) < 32 * 9 or len(body) % 32 != 0:
        raise RuntimeError("route-canary submit calldata has invalid ABI length")
    offset = _word_u256(body[0:32], label="route-canary proofBytes offset")
    if offset != 32 * 8:
        raise RuntimeError(
            "route-canary submit calldata proofBytes offset must be 256 bytes"
        )
    if offset + 32 > len(body):
        raise RuntimeError("route-canary submit calldata proofBytes is truncated")
    public_inputs = tuple(body[index : index + 32] for index in range(32, 32 * 7, 32))
    statement_hash = body[32 * 7 : 32 * 8]
    proof_len = _word_u256(
        body[offset : offset + 32],
        label="route-canary proofBytes length",
    )
    proof_start = offset + 32
    proof_end = proof_start + proof_len
    if proof_end > len(body):
        raise RuntimeError("route-canary submit calldata proofBytes is truncated")
    padding_len = (32 - (proof_len % 32)) % 32
    if proof_end + padding_len != len(body):
        raise RuntimeError("route-canary submit calldata has trailing ABI data")
    if any(body[proof_end:]):
        raise RuntimeError("route-canary submit calldata proofBytes padding must be zero")
    proof_bytes = body[proof_start:proof_end]
    if proof_len != EVM_GROTH16_PROOF_ABI_BYTE_LENGTH:
        raise RuntimeError("route-canary proofBytes must be a 384-byte Groth16 tuple")
    if not any(proof_bytes):
        raise RuntimeError("route-canary proofBytes must not be all zero")
    message_id = _parse_hex32(
        str(event_summary["message_id"]),
        label="route-canary event message id",
    )
    commitment_root = _parse_hex32(
        str(event_summary["commitment_root"]),
        label="route-canary event commitment root",
    )
    event_statement_hash = _parse_hex32(
        str(event_summary["statement_hash"]),
        label="route-canary event statement hash",
    )
    if public_inputs[0] != message_id:
        raise RuntimeError(
            "route-canary submit calldata publicInputs[0] must match event messageId"
        )
    payload_hash = _require_nonzero_word(
        public_inputs[1],
        label="route-canary payloadHash",
    )
    target_domain = _word_u32(
        public_inputs[2],
        label="route-canary publicInputs targetDomain",
    )
    if target_domain != expected_target_domain:
        raise RuntimeError(
            "route-canary submit calldata targetDomain does not match "
            "expectedTargetDomain()"
        )
    if public_inputs[3] != commitment_root:
        raise RuntimeError(
            "route-canary submit calldata publicInputs[3] must match event commitmentRoot"
        )
    finality_height = _require_nonzero_word(
        public_inputs[4],
        label="route-canary finalityHeight",
    )
    finality_block_hash = _require_nonzero_word(
        public_inputs[5],
        label="route-canary finalityBlockHash",
    )
    if statement_hash != event_statement_hash:
        raise RuntimeError(
            "route-canary submit calldata statementHash must match accepted event"
        )
    proof_words = tuple(
        proof_bytes[index : index + 32]
        for index in range(0, EVM_GROTH16_PROOF_ABI_BYTE_LENGTH, 32)
    )
    version = _word_u32(proof_words[0], label="route-canary proof version")
    if version != EVM_GROTH16_PROOF_VERSION:
        raise RuntimeError("route-canary proof version must be 1")
    if proof_words[1] != message_id:
        raise RuntimeError("route-canary proof messageId must match accepted event")
    proof_source_domain = _word_u32(
        proof_words[2],
        label="route-canary proof sourceDomain",
    )
    if proof_source_domain != expected_source_domain:
        raise RuntimeError(
            "route-canary proof sourceDomain does not match expectedSourceDomain()"
        )
    if proof_words[3] != commitment_root:
        raise RuntimeError(
            "route-canary proof commitmentRoot must match accepted event"
        )
    _require_route_canary_groth16_bn254_proof_tuple(proof_words)
    return {
        "function_selector": "submitSccpMessageProof(bytes,bytes32[6],bytes32)",
        "call_data": _hex(call_data),
        "call_data_sha256": _hex(hashlib.sha256(call_data).digest()),
        "proof_bytes_length": proof_len,
        "proof_version": version,
        "proof_source_domain": proof_source_domain,
        "public_inputs_payload_hash": _hex(payload_hash),
        "public_inputs_target_domain": target_domain,
        "public_inputs_finality_height": _hex(finality_height),
        "public_inputs_finality_block_hash": _hex(finality_block_hash),
        "call_matches": True,
    }


def _route_canary_transaction_call_summary(
    response: dict[str, Any],
    *,
    transaction_hash: bytes,
    bridge_address: bytes,
    expected_block_hash: bytes,
    expected_block_number: int,
    event_summary: dict[str, Any],
    expected_source_domain: int,
    expected_target_domain: int,
) -> dict[str, Any]:
    tx_hash = _parse_exact_hex32_blob(
        response.get("hash"),
        label="route-canary transaction hash",
    )
    if tx_hash != transaction_hash:
        raise RuntimeError("route-canary transaction hash does not match request")
    tx_to = _parse_exact_address(response.get("to"), label="route-canary transaction to")
    if tx_to != bridge_address:
        raise RuntimeError("route-canary transaction to does not match destination bridge")
    tx_block_hash = _parse_exact_hex32_blob(
        response.get("blockHash"),
        label="route-canary transaction blockHash",
    )
    if tx_block_hash != expected_block_hash:
        raise RuntimeError(
            "route-canary transaction blockHash does not match receipt blockHash"
        )
    tx_block_number = _rpc_quantity(
        response.get("blockNumber"),
        method="route-canary transaction blockNumber",
    )
    if tx_block_number != expected_block_number:
        raise RuntimeError(
            "route-canary transaction blockNumber does not match receipt blockNumber"
        )
    input_value = response.get("input", response.get("data"))
    call_data = _parse_exact_hex_blob(
        input_value,
        label="route-canary transaction input",
    )
    return {
        "transaction_hash": _hex(tx_hash),
        "to": _hex(tx_to),
        "transaction_block_hash": _hex(tx_block_hash),
        "transaction_block_number": tx_block_number,
        "transaction_block_matches": True,
        **_route_canary_submit_call_data_summary(
            call_data,
            event_summary=event_summary,
            expected_source_domain=expected_source_domain,
            expected_target_domain=expected_target_domain,
        ),
    }


def _route_canary_used_message_proof_summary(
    rpc_url: str,
    *,
    bridge_address: str,
    message_id: bytes,
    block_tag: str,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    selector = evidence._keccak_256(b"usedMessageProofs(bytes32)")[:4]
    result = _json_rpc(
        rpc_url,
        "eth_call",
        [
            {
                "to": bridge_address,
                "data": "0x" + selector.hex() + message_id.hex(),
            },
            block_tag,
        ],
        opener=opener,
        timeout=timeout,
    )
    word = _rpc_hex_data(result, method="eth_call usedMessageProofs(bytes32)")
    if len(word) != 32:
        raise RuntimeError("usedMessageProofs(bytes32) must return one ABI word")
    if not _word_bool(word, label="usedMessageProofs(bytes32)"):
        raise RuntimeError(
            "route-canary bridge usedMessageProofs(bytes32) is false for "
            "the accepted messageId"
        )
    return {
        "used_message_proofs_checked": True,
        "message_proof_used": True,
        "used_message_proofs_function": "usedMessageProofs(bytes32)",
        "used_message_proofs_parameter": _hex(message_id),
    }


def _collect_route_canary_transaction_evidence(
    rpc_url: str,
    *,
    destination: dict[str, Any],
    route_allowlist_hash: bytes,
    transaction_hash: bytes,
    log_index: int,
    block_tag: str,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    block_tag = parse_block_tag(block_tag)
    transaction_hash_hex = _hex(transaction_hash)
    receipt = _json_rpc(
        rpc_url,
        "eth_getTransactionReceipt",
        [transaction_hash_hex],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(receipt, dict):
        raise RuntimeError("route-canary transaction receipt was not found")
    receipt_tx_hash = _parse_exact_hex32_blob(
        receipt.get("transactionHash"),
        label="route-canary receipt transactionHash",
    )
    if receipt_tx_hash != transaction_hash:
        raise RuntimeError("route-canary receipt transactionHash does not match request")
    if receipt.get("status") != "0x1":
        raise RuntimeError("route-canary transaction receipt status must be 0x1")
    receipt_block = _route_canary_receipt_block_summary(
        rpc_url,
        receipt,
        opener=opener,
        timeout=timeout,
    )
    finalized_block = (
        _route_canary_finalized_block_summary(
            rpc_url,
            receipt_block,
            opener=opener,
            timeout=timeout,
        )
        if block_tag == "finalized"
        else {"receipt_block_finalized": False}
    )
    bridge_address = _parse_hex_bytes(
        str(destination["bridge_address"]),
        label="destination bridge address",
        byte_length=20,
    )
    expected_source_domain = destination.get("source_domain")
    expected_target_domain = destination.get("target_domain")
    if type(expected_source_domain) is not int or type(expected_target_domain) is not int:
        raise RuntimeError("destination bridge domains must be integers")
    expected_destination_binding_hash = _parse_hex32(
        str(destination["destination_binding_hash"]),
        label="destination binding hash",
    )
    expected_verifier_backend_hash = _parse_hex32(
        str(destination["verifier_backend_hash"]),
        label="verifier backend hash",
    )
    expected_proof_family_hash = _parse_hex32(
        str(destination["proof_family_hash"]),
        label="proof family hash",
    )
    expected_network_id = _parse_hex32(
        str(destination["network_id"]),
        label="destination bridge network id",
    )
    logs = receipt.get("logs")
    if not isinstance(logs, list):
        raise RuntimeError("route-canary transaction receipt returned no logs list")
    event_summary = None
    for index, log in enumerate(logs):
        if not isinstance(log, dict):
            raise RuntimeError(
                f"route-canary transaction receipt logs[{index}] must be an object"
            )
        if log.get("removed") is True:
            raise RuntimeError(
                "route-canary transaction receipt must not contain removed logs"
            )
        candidate = _route_canary_message_proof_event_summary(
            log,
            expected_log_index=log_index,
            transaction_hash=transaction_hash,
            expected_block_hash=_parse_hex32(
                str(receipt_block["block_hash"]),
                label="route-canary receipt block hash",
            ),
            expected_block_number=int(receipt_block["block_number"]),
            route_allowlist_hash=route_allowlist_hash,
            bridge_address=bridge_address,
            expected_source_domain=expected_source_domain,
            expected_target_domain=expected_target_domain,
            expected_destination_binding_hash=expected_destination_binding_hash,
            expected_verifier_backend_hash=expected_verifier_backend_hash,
            expected_proof_family_hash=expected_proof_family_hash,
            expected_network_id=expected_network_id,
        )
        if candidate is not None:
            if event_summary is not None:
                raise RuntimeError(
                    "route-canary transaction receipt contained more than one "
                    "matching MessageProofAccepted event at the supplied log index"
                )
            event_summary = candidate
    if event_summary is None:
        raise RuntimeError(
            "route-canary transaction receipt did not contain the expected "
            "MessageProofAccepted event at the supplied log index"
        )

    transaction = _json_rpc(
        rpc_url,
        "eth_getTransactionByHash",
        [transaction_hash_hex],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(transaction, dict):
        raise RuntimeError("route-canary transaction was not found")
    call_summary = _route_canary_transaction_call_summary(
        transaction,
        transaction_hash=transaction_hash,
        bridge_address=bridge_address,
        expected_block_hash=_parse_hex32(
            str(receipt_block["block_hash"]),
            label="route-canary receipt block hash",
        ),
        expected_block_number=int(receipt_block["block_number"]),
        event_summary=event_summary,
        expected_source_domain=expected_source_domain,
        expected_target_domain=expected_target_domain,
    )
    message_id = _parse_hex32(
        str(event_summary["message_id"]),
        label="route-canary message id",
    )
    used_summary = _route_canary_used_message_proof_summary(
        rpc_url,
        bridge_address=str(destination["bridge_address"]),
        message_id=message_id,
        block_tag=block_tag,
        opener=opener,
        timeout=timeout,
    )
    canary_hash = evidence.evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=bridge_address,
        transaction_hash=transaction_hash,
        log_index=int(event_summary["log_index"]),
        receipt_block_number=int(receipt_block["block_number"]),
        receipt_block_hash=_parse_hex32(
            str(receipt_block["block_hash"]),
            label="route-canary receipt block hash",
        ),
        block_receipts_root=_parse_hex32(
            str(receipt_block["block_receipts_root"]),
            label="route-canary block receiptsRoot",
        ),
        call_data_sha256=_parse_hex32(
            str(call_summary["call_data_sha256"]),
            label="route-canary call data SHA-256",
        ),
        message_id=message_id,
        payload_hash=_parse_hex32(
            str(call_summary["public_inputs_payload_hash"]),
            label="route-canary payload hash",
        ),
        source_domain=expected_source_domain,
        target_domain=int(call_summary["public_inputs_target_domain"]),
        commitment_root=_parse_hex32(
            str(event_summary["commitment_root"]),
            label="route-canary commitment root",
        ),
        finality_height=_parse_hex32(
            str(call_summary["public_inputs_finality_height"]),
            label="route-canary finality height",
        ),
        finality_block_hash=_parse_hex32(
            str(call_summary["public_inputs_finality_block_hash"]),
            label="route-canary finality block hash",
        ),
        statement_hash=_parse_hex32(
            str(event_summary["statement_hash"]),
            label="route-canary statement hash",
        ),
        proof_version=int(call_summary["proof_version"]),
        proof_source_domain=int(call_summary["proof_source_domain"]),
        destination_binding_hash=expected_destination_binding_hash,
        verifier_backend_hash=expected_verifier_backend_hash,
        proof_family_hash=expected_proof_family_hash,
        network_id=expected_network_id,
        used_message_proof=used_summary["message_proof_used"],
        receipt_block_finalized=finalized_block["receipt_block_finalized"],
    )
    summary = {
        **event_summary,
        "receipt_status": "0x1",
        **receipt_block,
        **finalized_block,
        **call_summary,
        **used_summary,
        "route_canary_evidence_hash": _hex(canary_hash),
    }
    return summary


def _route_canary_receipt_block_summary(
    rpc_url: str,
    receipt: dict[str, Any],
    *,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    receipt_block_number_text = receipt.get("blockNumber")
    if not isinstance(receipt_block_number_text, str):
        raise RuntimeError("route-canary receipt blockNumber must be present")
    receipt_block_number = _rpc_quantity(
        receipt_block_number_text,
        method="route-canary receipt blockNumber",
    )
    if receipt_block_number == 0:
        raise RuntimeError("route-canary receipt blockNumber must be non-zero")
    receipt_block_hash = _parse_exact_hex32_blob(
        receipt.get("blockHash"),
        label="route-canary receipt blockHash",
    )
    block = _json_rpc(
        rpc_url,
        "eth_getBlockByNumber",
        [receipt_block_number_text, False],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(block, dict):
        raise RuntimeError("route-canary receipt block was not found")
    block_number = _rpc_quantity(
        block.get("number"),
        method="route-canary block number",
    )
    if block_number != receipt_block_number:
        raise RuntimeError("route-canary block number does not match receipt blockNumber")
    block_hash = _parse_exact_hex32_blob(
        block.get("hash"),
        label="route-canary block hash",
    )
    if block_hash != receipt_block_hash:
        raise RuntimeError("route-canary block hash does not match receipt blockHash")
    block_receipts_root = _parse_exact_hex32_blob(
        block.get("receiptsRoot"),
        label="route-canary block receiptsRoot",
    )
    return {
        "block_number": receipt_block_number,
        "block_hash": _hex(receipt_block_hash),
        "block_receipts_root": _hex(block_receipts_root),
        "receipt_block_matches": True,
    }


def _route_canary_finalized_block_summary(
    rpc_url: str,
    receipt_block: dict[str, Any],
    *,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    finalized = _json_rpc(
        rpc_url,
        "eth_getBlockByNumber",
        ["finalized", False],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(finalized, dict):
        raise RuntimeError("route-canary finalized block was not found")
    finalized_block_number = _rpc_quantity(
        finalized.get("number"),
        method="route-canary finalized block number",
    )
    if finalized_block_number == 0:
        raise RuntimeError("route-canary finalized block number must be non-zero")
    finalized_block_hash = _parse_exact_hex32_blob(
        finalized.get("hash"),
        label="route-canary finalized block hash",
    )
    receipt_block_number = int(receipt_block["block_number"])
    if receipt_block_number > finalized_block_number:
        raise RuntimeError(
            "route-canary receipt block is newer than the finalized execution block"
        )
    receipt_block_hash = _parse_hex32(
        str(receipt_block["block_hash"]),
        label="route-canary receipt block hash",
    )
    if (
        receipt_block_number == finalized_block_number
        and receipt_block_hash != finalized_block_hash
    ):
        raise RuntimeError(
            "route-canary receipt block hash does not match the finalized execution block"
        )
    return {
        "finalized_block_number": finalized_block_number,
        "finalized_block_hash": _hex(finalized_block_hash),
        "receipt_block_finalized": True,
    }


def _offline_args(summary: dict[str, Any]) -> list[str]:
    destination = summary["destination_bridge"]
    args = [
        "--domain",
        str(destination["chain"]),
        "--network-id",
        str(destination["network_id"]),
        "--verifier-address",
        str(destination["verifier_address"]),
        "--bridge-address",
        str(destination["bridge_address"]),
        "--bridge-code-hash",
        str(destination["bridge_code_hash"]),
        "--bridge-runtime-bytecode-hex",
        str(destination["bridge_runtime_bytecode_hex"]),
        "--verifier-code-hash",
        str(destination["verifier_code_hash"]),
        "--verifier-runtime-bytecode-hex",
        str(destination["verifier_runtime_bytecode_hex"]),
        "--verifier-key-hash",
        str(destination["verifier_key_hash"]),
    ]
    if destination.get("expected_destination_binding_hash_matches") is True:
        args.extend(
            [
                "--expected-destination-binding-hash",
                str(destination["destination_binding_hash"]),
            ]
        )
    route_hash = summary.get("route_allowlist_hash")
    if (
        isinstance(route_hash, str)
        and destination.get("expected_destination_binding_hash_matches") is True
    ):
        args.extend(["--route-allowlist-hash", route_hash])
        source_record_hashes = summary.get("source_record_hashes")
        if not isinstance(source_record_hashes, dict):
            raise ValueError("route allowlist TOML requires source record hashes")
        args.extend(
            [
                "--source-verifier-material-hash",
                str(source_record_hashes["source_verifier_material_hash"]),
                "--source-adapter-engine-deployment-hash",
                str(source_record_hashes["source_adapter_engine_deployment_hash"]),
            ]
        )
        route_canary = summary.get("route_canary")
        if isinstance(route_canary, dict):
            args.extend(
                [
                    "--route-canary-evidence-hash",
                    str(route_canary["evidence_hash"]),
                ]
            )
        route_canary_transaction = summary.get("route_canary_transaction")
        if isinstance(route_canary_transaction, dict):
            args.extend(
                [
                    "--route-canary-transaction-hash",
                    str(route_canary_transaction["transaction_hash"]),
                    "--route-canary-transaction-block-number",
                    str(route_canary_transaction["transaction_block_number"]),
                    "--route-canary-transaction-block-hash",
                    str(route_canary_transaction["transaction_block_hash"]),
                    "--route-canary-log-index",
                    str(route_canary_transaction["log_index"]),
                    "--route-canary-receipt-block-number",
                    str(route_canary_transaction["block_number"]),
                    "--route-canary-receipt-block-hash",
                    str(route_canary_transaction["block_hash"]),
                    "--route-canary-block-receipts-root",
                    str(route_canary_transaction["block_receipts_root"]),
                    "--route-canary-call-data-sha256",
                    str(route_canary_transaction["call_data_sha256"]),
                    "--route-canary-message-id",
                    str(route_canary_transaction["message_id"]),
                    "--route-canary-payload-hash",
                    str(route_canary_transaction["public_inputs_payload_hash"]),
                    "--route-canary-target-domain",
                    str(route_canary_transaction["public_inputs_target_domain"]),
                    "--route-canary-statement-hash",
                    str(route_canary_transaction["statement_hash"]),
                    "--route-canary-commitment-root",
                    str(route_canary_transaction["commitment_root"]),
                    "--route-canary-finality-height",
                    str(route_canary_transaction["public_inputs_finality_height"]),
                    "--route-canary-finality-block-hash",
                    str(route_canary_transaction["public_inputs_finality_block_hash"]),
                    "--route-canary-proof-version",
                    str(route_canary_transaction["proof_version"]),
                    "--route-canary-proof-source-domain",
                    str(route_canary_transaction["proof_source_domain"]),
                    "--route-canary-used-message-proof",
                    "true"
                    if route_canary_transaction.get("message_proof_used") is True
                    else "false",
                    "--route-canary-receipt-block-finalized",
                    "true"
                    if route_canary_transaction.get("receipt_block_finalized") is True
                    else "false",
                ]
            )
    return args


def _torii_destination_query_params(summary: dict[str, Any]) -> dict[str, str] | None:
    destination = summary["destination_bridge"]
    if destination.get("expected_destination_binding_hash_matches") is not True:
        return None
    return {
        "network_id_hex": str(destination["network_id"]),
        "verifier_address_hex": str(destination["verifier_address"]),
        "bridge_address_hex": str(destination["bridge_address"]),
        "verifier_code_hash_hex": str(destination["verifier_code_hash"]),
        "verifier_key_hash_hex": str(destination["verifier_key_hash"]),
        "expected_destination_binding_hash_hex": str(
            destination["destination_binding_hash"]
        ),
    }


def _validate_destination_summary(summary: dict[str, Any]) -> None:
    destination = summary.get("destination_bridge")
    if not isinstance(destination, dict):
        raise ValueError("destination bridge evidence is required")
    domain = destination.get("domain")
    if type(domain) is not int or domain not in EXPECTED_RPC_CHAIN_IDS:
        raise ValueError("destination domain must be an EVM-family SCCP lane")
    expected_chain = evidence.DOMAIN_PROFILES[domain]["chain"]
    if destination.get("chain") != expected_chain:
        raise ValueError("destination chain metadata must match domain")
    rpc_chain_id = destination.get("rpc_chain_id")
    if type(rpc_chain_id) is not int:
        raise ValueError("EVM RPC chain id metadata must be an integer")
    expected_rpc_chain_id = EXPECTED_RPC_CHAIN_IDS[domain]
    if rpc_chain_id != expected_rpc_chain_id:
        raise ValueError(
            f"EVM RPC chain id metadata must be {expected_rpc_chain_id} "
            f"for {expected_chain}"
        )
    if destination.get("expected_rpc_chain_id") != expected_rpc_chain_id:
        raise ValueError("expected RPC chain id metadata must match the lane")

    bridge_address = _summary_address(
        destination,
        "bridge_address",
        label="bridge address",
    )
    verifier_address = _summary_address(
        destination,
        "verifier_address",
        label="verifier address",
    )
    if bridge_address == verifier_address:
        raise ValueError("destination verifier address must differ from bridge address")
    network_id = _summary_hex32(destination, "network_id", label="network id")
    bridge_code_hash = _summary_hex32(
        destination,
        "bridge_code_hash",
        label="bridge code hash",
    )
    bridge_runtime = _summary_runtime_bytes(
        destination,
        "bridge_runtime_bytecode_hex",
        label="bridge runtime bytecode",
    )
    if evidence.runtime_bytecode_hash(bridge_runtime) != bridge_code_hash:
        raise ValueError("bridge runtime bytecode hash must match bridge_code_hash")
    if (
        destination.get("expected_bridge_code_hash_matches") is True
        and destination.get("expected_bridge_code_hash") != _hex(bridge_code_hash)
    ):
        raise ValueError("expected bridge code hash metadata must match bridge_code_hash")
    verifier_code_hash = _summary_hex32(
        destination,
        "verifier_code_hash",
        label="verifier code hash",
    )
    eth_getcode_verifier_code_hash = _summary_hex32(
        destination,
        "eth_getcode_verifier_code_hash",
        label="eth_getCode verifier code hash",
    )
    if verifier_code_hash != eth_getcode_verifier_code_hash:
        raise ValueError("verifier code hash metadata must match eth_getCode hash")
    verifier_runtime = _summary_runtime_bytes(
        destination,
        "verifier_runtime_bytecode_hex",
        label="verifier runtime bytecode",
    )
    if evidence.runtime_bytecode_hash(verifier_runtime) != verifier_code_hash:
        raise ValueError("verifier runtime bytecode hash must match verifier_code_hash")
    verifier_key_hash = _summary_hex32(
        destination,
        "verifier_key_hash",
        label="verifier key hash",
    )
    verifier_verifying_key_hash = _summary_hex32(
        destination,
        "verifier_verifying_key_hash",
        label="verifier verifyingKeyHash",
    )
    if verifier_key_hash != verifier_verifying_key_hash:
        raise ValueError("verifier key hash metadata must match verifyingKeyHash")

    expected_backend_hash, expected_family_hash = _expected_hashes()
    if (
        _summary_hex32(destination, "verifier_backend_hash", label="verifier backend hash")
        != expected_backend_hash
    ):
        raise ValueError("verifier backend hash metadata is not evm-groth16-bn254-v1")
    if (
        _summary_hex32(destination, "proof_family_hash", label="proof family hash")
        != expected_family_hash
    ):
        raise ValueError("proof family hash metadata is not stark-fri-v1")
    if destination.get("source_domain") != evidence.SCCP_DOMAIN_SORA:
        raise ValueError("destination source domain metadata must be SORA")
    if destination.get("target_domain") != domain:
        raise ValueError("destination target domain metadata must match domain")

    expected_binding_hash = evidence.evm_destination_binding_hash(
        network_id=network_id,
        source_domain=evidence.SCCP_DOMAIN_SORA,
        target_domain=domain,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    destination_binding_hash = _summary_hex32(
        destination,
        "destination_binding_hash",
        label="destination binding hash",
    )
    if destination_binding_hash != expected_binding_hash:
        raise ValueError(
            "destination binding hash metadata must match canonical live inputs"
        )
    if (
        destination.get("expected_network_id_matches") is True
        and destination.get("expected_network_id") != _hex(network_id)
    ):
        raise ValueError("expected network id metadata must match networkId()")
    if (
        destination.get("expected_destination_binding_hash_matches") is True
        and destination.get("expected_destination_binding_hash")
        != _hex(destination_binding_hash)
    ):
        raise ValueError(
            "expected destination binding hash metadata must match live binding"
        )
    expected_binding_key = evidence.evm_destination_binding_key(
        network_id=network_id,
        source_domain=evidence.SCCP_DOMAIN_SORA,
        target_domain=domain,
        verifier_address=verifier_address,
        bridge_address=bridge_address,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    if destination.get("destination_binding_key") != expected_binding_key:
        raise ValueError("destination binding key metadata must match canonical inputs")


def _route_canary_transaction_verified(summary: dict[str, Any]) -> bool:
    route_canary = summary.get("route_canary")
    transaction = summary.get("route_canary_transaction")
    if not isinstance(route_canary, dict) or not isinstance(transaction, dict):
        return False
    return (
        route_canary.get("evidence_source")
        == "evm_message_proof_accepted_transaction"
        and route_canary.get("evidence_hash")
        == transaction.get("route_canary_evidence_hash")
        and transaction.get("event_matches") is True
        and transaction.get("call_matches") is True
        and transaction.get("message_proof_used") is True
        and transaction.get("receipt_block_matches") is True
        and transaction.get("receipt_block_finalized") is True
        and transaction.get("transaction_block_matches") is True
        and type(transaction.get("block_number")) is int
        and isinstance(transaction.get("block_hash"), str)
        and type(transaction.get("transaction_block_number")) is int
        and isinstance(transaction.get("transaction_block_hash"), str)
        and isinstance(transaction.get("block_receipts_root"), str)
    )


def _full_toml_prerequisites(summary: dict[str, Any]) -> list[str]:
    missing: list[str] = []
    destination = summary.get("destination_bridge")
    if not isinstance(destination, dict):
        return ["destination bridge evidence"]
    if (
        destination.get("domain") == evidence.SCCP_DOMAIN_ETH
        and summary.get("block_tag") != "finalized"
    ):
        missing.append("--block-tag finalized")
    if destination.get("expected_rpc_chain_id_matches") is not True:
        missing.append("--expected-rpc-chain-id")
    if destination.get("expected_network_id_matches") is not True:
        missing.append("--expected-network-id")
    if destination.get("expected_bridge_code_hash_matches") is not True:
        missing.append("--expected-bridge-code-hash")
    if destination.get("expected_destination_binding_hash_matches") is not True:
        missing.append("--expected-destination-binding-hash")
    if not isinstance(summary.get("route_allowlist_hash"), str):
        missing.append("--route-allowlist-hash")
    if not isinstance(summary.get("route_canary"), dict):
        missing.append("--route-canary-evidence-hash")
    if not _route_canary_transaction_verified(summary):
        missing.append("--route-canary-transaction-hash")
    source_record_hashes = summary.get("source_record_hashes")
    if not isinstance(source_record_hashes, dict):
        missing.append("source record hashes")
    return missing


def render_offline_toml(summary: dict[str, Any]) -> str:
    """Render governed destination rollout TOML from a live-evidence summary."""

    _validate_destination_summary(summary)
    missing = _full_toml_prerequisites(summary)
    if missing:
        raise ValueError("TOML output requires " + ", ".join(missing))
    destination = summary["destination_bridge"]
    parser = evidence.build_parser()
    try:
        args = parser.parse_args([*_offline_args(summary), "--toml"])
    except SystemExit:
        raise RuntimeError(
            "generated EVM destination TOML arguments are invalid"
        ) from None
    evidence.apply_runtime_bytecode_hash(args)
    destination_binding_hash = evidence._destination_binding_hash_from_args(args)
    rendered = evidence.render_toml(args, destination_binding_hash)
    comments = [
        "# sccp_evm_block_tag = " + json.dumps(str(summary["block_tag"])),
        "# sccp_evm_rpc_chain_id = " + json.dumps(str(destination["rpc_chain_id"])),
        "# sccp_evm_bridge_runtime_code_hash = "
        + json.dumps(str(destination["bridge_code_hash"])),
        "# sccp_evm_bridge_runtime_bytecode_hex = "
        + json.dumps(str(destination["bridge_runtime_bytecode_hex"])),
        "# sccp_evm_verifier_runtime_code_hash = "
        + json.dumps(str(destination["eth_getcode_verifier_code_hash"])),
        "# sccp_evm_verifier_runtime_bytecode_hex = "
        + json.dumps(str(destination["verifier_runtime_bytecode_hex"])),
        "# sccp_evm_verifier_key_hash = "
        + json.dumps(str(destination["verifier_verifying_key_hash"])),
        "# sccp_evm_verifier_backend_hash = "
        + json.dumps(str(destination["verifier_backend_hash"])),
        "# sccp_evm_proof_family_hash = "
        + json.dumps(str(destination["proof_family_hash"])),
    ]
    existing_keys = set()
    for line in rendered.splitlines():
        stripped = line.strip()
        if not stripped.startswith("#") or "=" not in stripped:
            continue
        key, _value = stripped[1:].split("=", 1)
        existing_keys.add(key.strip())
    missing_comments = [
        comment
        for comment in comments
        if comment[1:].split("=", 1)[0].strip() not in existing_keys
    ]
    if not missing_comments:
        return rendered
    return "\n".join([*missing_comments, rendered])


def _validate_route_allowlist_hash(
    args: argparse.Namespace,
    destination: dict[str, Any],
    *,
    include_route_canary: bool = True,
) -> dict[str, Any]:
    route_allowlist_hash = getattr(args, "route_allowlist_hash", None)
    if route_allowlist_hash is None:
        return {}
    source_material_hash = getattr(args, "source_verifier_material_hash", None)
    source_deployment_hash = getattr(
        args,
        "source_adapter_engine_deployment_hash",
        None,
    )
    if source_material_hash is None or source_deployment_hash is None:
        raise ValueError(
            "--route-allowlist-hash requires --source-verifier-material-hash "
            "and --source-adapter-engine-deployment-hash"
        )
    if destination.get("expected_destination_binding_hash_matches") is not True:
        raise ValueError(
            "--route-allowlist-hash requires --expected-destination-binding-hash"
        )
    destination_binding_hash = _parse_hex32(
        destination["destination_binding_hash"],
        label="destination binding hash",
    )
    expected_hash = evidence.evm_route_allowlist_hash(
        domain=args.domain,
        source_verifier_material_hash=source_material_hash,
        source_adapter_engine_deployment_hash=source_deployment_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if route_allowlist_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, deployment, "
            "and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(route_allowlist_hash)}"
        )
    summary = {
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "source_record_hashes": {
            "source_verifier_material_hash": _hex(source_material_hash),
            "source_adapter_engine_deployment_hash": _hex(source_deployment_hash),
        },
        "expected_route_allowlist_hash": _hex(expected_hash),
        "expected_route_allowlist_hash_matches": True,
    }
    if include_route_canary:
        route_canary = evidence._route_canary_summary(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
        )
        if route_canary is not None:
            summary["route_canary"] = route_canary
    return summary


def collect_live_evidence(
    args: argparse.Namespace,
    *,
    opener: Urlopen = urllib.request.urlopen,
) -> dict[str, Any]:
    """Collect all requested evidence and return a JSON-serializable summary."""

    block_tag = parse_block_tag(
        args.block_tag
        if getattr(args, "block_tag", None) is not None
        else default_block_tag_for_domain(args.domain)
    )
    summary: dict[str, Any] = {
        "rpc_url": args.rpc_url,
        "read_only": True,
        "block_tag": block_tag,
    }
    canonical_rpc_chain_id = _default_rpc_chain_id_for_domain(args.domain)
    expected_rpc_chain_id = getattr(args, "expected_rpc_chain_id", None)
    if expected_rpc_chain_id is None:
        expected_rpc_chain_id = canonical_rpc_chain_id
    elif expected_rpc_chain_id != canonical_rpc_chain_id:
        chain = evidence.DOMAIN_PROFILES[args.domain]["chain"]
        raise ValueError(
            "--expected-rpc-chain-id must match the canonical "
            f"{chain} mainnet chain id {canonical_rpc_chain_id}"
        )
    destination = collect_destination_bridge_evidence(
        args.rpc_url,
        domain=args.domain,
        bridge_address=args.bridge_address,
        block_tag=block_tag,
        opener=opener,
        timeout=args.timeout,
    )
    if expected_rpc_chain_id != destination["rpc_chain_id"]:
        raise ValueError(
            "--expected-rpc-chain-id does not match eth_chainId for "
            f"{destination['chain']} lane: expected {expected_rpc_chain_id}, "
            f"got {destination['rpc_chain_id']}"
        )
    destination["expected_rpc_chain_id"] = expected_rpc_chain_id
    destination["expected_rpc_chain_id_matches"] = True
    canonical_network_id = _default_network_id_for_domain(args.domain)
    expected_network_id = getattr(args, "expected_network_id", None)
    if expected_network_id is None:
        expected_network_id = canonical_network_id
    elif expected_network_id != canonical_network_id:
        raise ValueError(
            "--expected-network-id must match the canonical "
            f"{destination['chain']} mainnet EIP-155 network id "
            f"{_hex(canonical_network_id)}"
        )
    if _hex(expected_network_id) != destination["network_id"]:
        raise ValueError(
            "--expected-network-id does not match bridge networkId(): "
            f"expected {_hex(expected_network_id)}, got {destination['network_id']}"
        )
    destination["expected_network_id"] = _hex(expected_network_id)
    destination["expected_network_id_matches"] = True
    expected_bridge_code_hash = getattr(args, "expected_bridge_code_hash", None)
    if expected_bridge_code_hash is not None:
        if _hex(expected_bridge_code_hash) != destination["bridge_code_hash"]:
            raise ValueError(
                "--expected-bridge-code-hash does not match live bridge "
                "runtime bytecode: "
                f"expected {_hex(expected_bridge_code_hash)}, "
                f"got {destination['bridge_code_hash']}"
            )
        destination["expected_bridge_code_hash"] = _hex(expected_bridge_code_hash)
        destination["expected_bridge_code_hash_matches"] = True
    expected_binding = getattr(args, "expected_destination_binding_hash", None)
    if expected_binding is not None:
        if _hex(expected_binding) != destination["destination_binding_hash"]:
            raise ValueError(
                "--expected-destination-binding-hash does not match live "
                "deployment inputs: "
                f"expected {_hex(expected_binding)}, "
                f"got {destination['destination_binding_hash']}"
            )
        destination["expected_destination_binding_hash"] = _hex(expected_binding)
        destination["expected_destination_binding_hash_matches"] = True
    summary["destination_bridge"] = destination
    route_allowlist_hash = getattr(args, "route_allowlist_hash", None)
    route_canary_evidence_hash = getattr(args, "route_canary_evidence_hash", None)
    route_canary_transaction_hash = getattr(args, "route_canary_transaction_hash", None)
    route_canary_log_index = getattr(args, "route_canary_log_index", None)
    if (
        route_canary_evidence_hash is not None
        and route_canary_transaction_hash is None
    ):
        raise ValueError(
            "--route-canary-evidence-hash requires "
            "--route-canary-transaction-hash"
        )
    if (
        route_canary_transaction_hash is not None
        and route_allowlist_hash is None
    ):
        raise ValueError("--route-canary-transaction-hash requires --route-allowlist-hash")
    if route_canary_transaction_hash is not None and route_canary_log_index is None:
        raise ValueError("--route-canary-transaction-hash requires --route-canary-log-index")
    route_canary_transaction = None
    if route_canary_transaction_hash is not None:
        route_canary_transaction = _collect_route_canary_transaction_evidence(
            args.rpc_url,
            destination=destination,
            route_allowlist_hash=route_allowlist_hash,
            transaction_hash=route_canary_transaction_hash,
            log_index=route_canary_log_index,
            block_tag=block_tag,
            opener=opener,
            timeout=args.timeout,
        )
        derived_canary_hash = _parse_hex32(
            str(route_canary_transaction["route_canary_evidence_hash"]),
            label="route canary evidence hash",
        )
        if (
            route_canary_evidence_hash is not None
            and route_canary_evidence_hash != derived_canary_hash
        ):
            raise ValueError(
                "--route-canary-evidence-hash does not match "
                "MessageProofAccepted transaction evidence hash: "
                f"expected {_hex(derived_canary_hash)}, "
                f"got {_hex(route_canary_evidence_hash)}"
            )
        expected_receipt_block_number = getattr(
            args,
            "route_canary_receipt_block_number",
            None,
        )
        if (
            expected_receipt_block_number is not None
            and expected_receipt_block_number != route_canary_transaction["block_number"]
        ):
            raise ValueError(
                "--route-canary-receipt-block-number does not match "
                "MessageProofAccepted transaction receipt block"
            )
        expected_receipt_block_hash = getattr(
            args,
            "route_canary_receipt_block_hash",
            None,
        )
        if expected_receipt_block_hash is not None and _hex(
            expected_receipt_block_hash
        ) != route_canary_transaction["block_hash"]:
            raise ValueError(
                "--route-canary-receipt-block-hash does not match "
                "MessageProofAccepted transaction receipt block"
            )
        expected_block_receipts_root = getattr(
            args,
            "route_canary_block_receipts_root",
            None,
        )
        if expected_block_receipts_root is not None and _hex(
            expected_block_receipts_root
        ) != route_canary_transaction["block_receipts_root"]:
            raise ValueError(
                "--route-canary-block-receipts-root does not match "
                "MessageProofAccepted transaction receipt block"
            )
        args.route_canary_evidence_hash = derived_canary_hash
        args.network_id = _parse_hex32(
            str(destination["network_id"]),
            label="destination bridge network id",
        )
        args.bridge_address = _parse_hex_bytes(
            str(destination["bridge_address"]),
            label="destination bridge address",
            byte_length=20,
        )
        args.route_canary_message_id = _parse_hex32(
            str(route_canary_transaction["message_id"]),
            label="route canary message id",
        )
        args.route_canary_call_data_sha256 = _parse_hex32(
            str(route_canary_transaction["call_data_sha256"]),
            label="route canary call data SHA-256",
        )
        args.route_canary_transaction_block_number = int(
            route_canary_transaction["transaction_block_number"]
        )
        args.route_canary_transaction_block_hash = _parse_hex32(
            str(route_canary_transaction["transaction_block_hash"]),
            label="route canary transaction block hash",
        )
        args.route_canary_receipt_block_number = int(
            route_canary_transaction["block_number"]
        )
        args.route_canary_receipt_block_hash = _parse_hex32(
            str(route_canary_transaction["block_hash"]),
            label="route canary receipt block hash",
        )
        args.route_canary_block_receipts_root = _parse_hex32(
            str(route_canary_transaction["block_receipts_root"]),
            label="route canary block receiptsRoot",
        )
        args.route_canary_payload_hash = _parse_hex32(
            str(route_canary_transaction["public_inputs_payload_hash"]),
            label="route canary payload hash",
        )
        args.route_canary_target_domain = int(
            route_canary_transaction["public_inputs_target_domain"]
        )
        args.route_canary_statement_hash = _parse_hex32(
            str(route_canary_transaction["statement_hash"]),
            label="route canary statement hash",
        )
        args.route_canary_commitment_root = _parse_hex32(
            str(route_canary_transaction["commitment_root"]),
            label="route canary commitment root",
        )
        args.route_canary_finality_height = _parse_hex32(
            str(route_canary_transaction["public_inputs_finality_height"]),
            label="route canary finality height",
        )
        args.route_canary_finality_block_hash = _parse_hex32(
            str(route_canary_transaction["public_inputs_finality_block_hash"]),
            label="route canary finality block hash",
        )
        args.route_canary_proof_version = int(route_canary_transaction["proof_version"])
        args.route_canary_proof_source_domain = int(
            route_canary_transaction["proof_source_domain"]
        )
        args.route_canary_used_message_proof = (
            route_canary_transaction.get("message_proof_used") is True
        )
        args.route_canary_receipt_block_finalized = (
            route_canary_transaction.get("receipt_block_finalized") is True
        )
    if route_allowlist_hash is not None:
        summary.update(
            _validate_route_allowlist_hash(
                args,
                destination,
                include_route_canary=(
                    route_canary_transaction is None
                    or route_canary_transaction.get("receipt_block_finalized") is True
                ),
            )
        )
        if route_canary_transaction is not None:
            summary["route_canary_transaction"] = route_canary_transaction
            route_canary = summary.get("route_canary")
            if isinstance(route_canary, dict):
                route_canary["transaction"] = route_canary_transaction
        if not _full_toml_prerequisites(summary):
            offline_toml = render_offline_toml(summary)
            summary["offline_toml_sha256"] = hashlib.sha256(
                offline_toml.encode("utf-8")
            ).hexdigest()
    summary["offline_evidence_args"] = _offline_args(summary)
    torii_destination_query_params = _torii_destination_query_params(summary)
    if torii_destination_query_params is not None:
        summary["torii_destination_query_params"] = torii_destination_query_params
        summary["torii_destination_query_proof_bytes_hex_required"] = True
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Read EVM SCCP destination bridge view functions and recompute "
            "deployment-bound destination evidence."
        ),
    )
    parser.add_argument("--rpc-url", required=True, help="Ethereum/BSC JSON-RPC URL.")
    parser.add_argument(
        "--domain",
        required=True,
        type=evidence.parse_destination_domain,
        help="Destination domain to verify: eth or bsc.",
    )
    parser.add_argument(
        "--bridge-address",
        required=True,
        help="Deployed SccpMessageBridge wrapper address.",
    )
    parser.add_argument(
        "--expected-network-id",
        type=lambda value: _parse_hex32(value, label="expected network id"),
        help=(
            "Expected bridge networkId() bytes32. Defaults to the canonical "
            "mainnet EIP-155 id for --domain: eth=1, bsc=56; an explicit "
            "value must match that canonical id."
        ),
    )
    parser.add_argument(
        "--expected-rpc-chain-id",
        type=_parse_rpc_chain_id,
        help=(
            "Expected eth_chainId for the RPC endpoint. Defaults to the "
            "canonical mainnet id for --domain: eth=1, bsc=56; an explicit "
            "value must match that canonical id."
        ),
    )
    parser.add_argument(
        "--expected-bridge-code-hash",
        type=lambda value: _parse_hex32(value, label="expected bridge code hash"),
        help="Expected non-zero deployed bridge wrapper runtime code hash.",
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected destination binding hash",
        ),
        help="Expected destination binding hash to compare against live views.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: _parse_hex32(value, label="route allowlist hash"),
        help=(
            "Governed route allowlist hash for TOML output. Must match the "
            "canonical source material, source adapter deployment, and "
            "destination binding tuple, and requires "
            "--expected-destination-binding-hash."
        ),
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: _parse_hex32(value, label="source verifier material hash"),
        help="Source verifier material record hash required with --route-allowlist-hash.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: _parse_hex32(
            value,
            label="source adapter engine deployment hash",
        ),
        help=(
            "Source adapter engine deployment record hash required with "
            "--route-allowlist-hash."
        ),
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: _parse_hex32(value, label="route canary evidence hash"),
        help=(
            "Expected post-deploy route canary evidence hash. When supplied, "
            "it must match --route-canary-transaction-hash."
        ),
    )
    parser.add_argument(
        "--route-canary-transaction-hash",
        type=lambda value: _parse_hex32(value, label="route canary transaction hash"),
        help=(
            "EVM transaction hash for a successful MessageProofAccepted canary. "
            "The helper verifies the receipt log, submitted calldata, and "
            "usedMessageProofs(messageId)."
        ),
    )
    parser.add_argument(
        "--route-canary-log-index",
        type=_parse_route_canary_log_index,
        help="Canonical decimal log index of the MessageProofAccepted canary event.",
    )
    parser.add_argument(
        "--route-canary-receipt-block-number",
        type=lambda value: evidence.parse_u64_decimal(
            value,
            label="route canary receipt block number",
        ),
        help=(
            "Optional expected receipt block number for the canary transaction. "
            "When supplied, it must match the fetched receipt block."
        ),
    )
    parser.add_argument(
        "--route-canary-receipt-block-hash",
        type=lambda value: _parse_hex32(
            value,
            label="route canary receipt block hash",
        ),
        help=(
            "Optional expected receipt block hash for the canary transaction. "
            "When supplied, it must match the fetched receipt block."
        ),
    )
    parser.add_argument(
        "--route-canary-block-receipts-root",
        type=lambda value: _parse_hex32(
            value,
            label="route canary block receiptsRoot",
        ),
        help=(
            "Optional expected receiptsRoot for the canary transaction block. "
            "When supplied, it must match eth_getBlockByNumber."
        ),
    )
    parser.add_argument(
        "--block-tag",
        default=None,
        type=parse_block_tag,
        help=(
            "JSON-RPC block tag for eth_call/eth_getCode. Must be latest, "
            "safe, finalized, or a positive canonical lowercase 0x block "
            "number. Defaults to finalized for Ethereum mainnet and latest "
            "for BSC."
        ),
    )
    parser.add_argument(
        "--full-toml",
        action="store_true",
        help="Print verified destination rollout TOML instead of JSON.",
    )
    parser.add_argument("--timeout", type=float, default=15.0, help="HTTP timeout in seconds.")
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def _public_summary(summary: dict[str, Any]) -> dict[str, Any]:
    return {key: summary[key] for key in PUBLIC_SUMMARY_FIELDS if key in summary}


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary = collect_live_evidence(args)
        if args.full_toml:
            sys.stdout.write(render_offline_toml(summary))
            return 0
    except (
        OSError,
        RuntimeError,
        TypeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP EVM live evidence collection failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    print(json.dumps(_public_summary(summary), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
