"""Fail-closed validation for canonical native VPN trust metadata."""

from __future__ import annotations

import ipaddress
import re
from typing import Any, Mapping, Union

_ED25519_FIELD_MODULUS = (1 << 255) - 19
_ED25519_SUBGROUP_ORDER = (1 << 252) + 27742317777372353535851937790883648493
_ED25519_D = (
    -121665 * pow(121666, _ED25519_FIELD_MODULUS - 2, _ED25519_FIELD_MODULUS)
) % _ED25519_FIELD_MODULUS
_ED25519_SQRT_M1 = pow(2, (_ED25519_FIELD_MODULUS - 1) // 4, _ED25519_FIELD_MODULUS)
_ML_DSA_65_PUBLIC_KEY_HEX_LENGTH = 1_952 * 2


def _ed25519_extended_add(
    left: tuple[int, int, int, int],
    right: tuple[int, int, int, int],
) -> tuple[int, int, int, int]:
    """Add two Ed25519 points in extended coordinates."""

    modulus = _ED25519_FIELD_MODULUS
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = ((y1 - x1) * (y2 - x2)) % modulus
    b = ((y1 + x1) * (y2 + x2)) % modulus
    c = (2 * _ED25519_D * t1 * t2) % modulus
    d = (2 * z1 * z2) % modulus
    e = (b - a) % modulus
    f = (d - c) % modulus
    g = (d + c) % modulus
    h = (b + a) % modulus
    return (e * f % modulus, g * h % modulus, f * g % modulus, e * h % modulus)


def _ed25519_scalar_multiply(
    point: tuple[int, int, int, int], scalar: int
) -> tuple[int, int, int, int]:
    """Multiply one extended-coordinate Ed25519 point by a public scalar."""

    result = (0, 1, 1, 0)
    addend = point
    while scalar:
        if scalar & 1:
            result = _ed25519_extended_add(result, addend)
        addend = _ed25519_extended_add(addend, addend)
        scalar >>= 1
    return result


def _is_canonical_prime_order_ed25519_public_key(public_key: bytes) -> bool:
    """Return whether bytes encode a non-identity prime-order Ed25519 point."""

    if len(public_key) != 32:
        return False
    encoded = int.from_bytes(public_key, "little")
    sign = encoded >> 255
    y = encoded & ((1 << 255) - 1)
    modulus = _ED25519_FIELD_MODULUS
    if y >= modulus:
        return False
    y_squared = y * y % modulus
    denominator = (_ED25519_D * y_squared + 1) % modulus
    if denominator == 0:
        return False
    x_squared = (y_squared - 1) * pow(denominator, modulus - 2, modulus) % modulus
    x = pow(x_squared, (modulus + 3) // 8, modulus)
    if x * x % modulus != x_squared:
        x = x * _ED25519_SQRT_M1 % modulus
    if x * x % modulus != x_squared:
        return False
    if x == 0 and sign == 1:
        return False
    if (x & 1) != sign:
        x = modulus - x
    if x == 0 and y == 1:
        return False
    subgroup_check = _ed25519_scalar_multiply(
        (x, y, 1, x * y % modulus), _ED25519_SUBGROUP_ORDER
    )
    check_x, check_y, check_z, _ = subgroup_check
    return check_x == 0 and check_y == check_z


def _require_exact_lower_hex_string(
    value: Any, *, context: str, expected_length: int
) -> str:
    if (
        not isinstance(value, str)
        or len(value) != expected_length
        or re.fullmatch(r"[0-9a-f]+", value) is None
    ):
        raise RuntimeError(
            f"{context} must be an exact lowercase {expected_length // 2}-byte hex string"
        )
    return value


def require_vpn_relay_id(
    value: Any, *, context: str, allow_empty: bool = False
) -> str:
    """Require a canonical prime-order Ed25519 relay identity."""

    if allow_empty and value == "":
        return ""
    literal = _require_exact_lower_hex_string(
        value, context=context, expected_length=64
    )
    if not _is_canonical_prime_order_ed25519_public_key(bytes.fromhex(literal)):
        raise RuntimeError(
            f"{context} must encode a canonical prime-order Ed25519 public key"
        )
    return literal


def require_vpn_mldsa65_public_key(
    value: Any, *, context: str, allow_empty: bool = False
) -> str:
    """Require one nonzero canonical ML-DSA-65 relay public key."""

    if allow_empty and value == "":
        return ""
    if (
        not isinstance(value, str)
        or len(value) != _ML_DSA_65_PUBLIC_KEY_HEX_LENGTH
        or re.fullmatch(r"[0-9a-f]+", value) is None
    ):
        raise RuntimeError(
            f"{context} must be exactly {_ML_DSA_65_PUBLIC_KEY_HEX_LENGTH} "
            "lowercase hexadecimal characters"
        )
    if not any(character != "0" for character in value):
        raise RuntimeError(f"{context} must not be the all-zero ML-DSA-65 key")
    return value


def require_vpn_trust_digest(
    value: Any, *, context: str, allow_empty: bool = False
) -> str:
    """Require one nonzero canonical SHA-256-sized trust digest."""

    if allow_empty and value == "":
        return ""
    literal = _require_exact_lower_hex_string(
        value, context=context, expected_length=64
    )
    if not any(bytes.fromhex(literal)):
        raise RuntimeError(f"{context} must not be the all-zero digest")
    return literal


def require_vpn_tls_server_name(
    value: Any, *, context: str, allow_empty: bool = False
) -> str:
    """Require a canonical lowercase DNS server name."""

    if allow_empty and value == "":
        return ""
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"{context} must be a canonical lowercase DNS name")
    labels = value.split(".")
    if (
        len(value) > 253
        or value != value.lower()
        or any(
            not label
            or len(label) > 63
            or re.fullmatch(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?", label) is None
            for label in labels
        )
    ):
        raise RuntimeError(f"{context} must be a canonical lowercase DNS name")
    return value


def require_vpn_relay_endpoint(
    value: Any, *, context: str, allow_empty: bool = False
) -> str:
    """Require the hard-cut UDP/QUIC VPN relay multiaddress shape."""

    if allow_empty and value == "":
        return ""
    if not isinstance(value, str) or not value:
        raise RuntimeError(
            f"{context} must use /{{ip4|ip6|dns|dns4|dns6}}/host/udp/port/quic"
        )
    parts = value.split("/")
    if (
        len(parts) != 6
        or parts[0] != ""
        or parts[1] not in {"ip4", "ip6", "dns", "dns4", "dns6"}
        or parts[3] != "udp"
        or parts[5] != "quic"
    ):
        raise RuntimeError(
            f"{context} must use /{{ip4|ip6|dns|dns4|dns6}}/host/udp/port/quic"
        )
    protocol, host, port_literal = parts[1], parts[2], parts[4]
    if protocol == "ip4":
        try:
            address = ipaddress.IPv4Address(host)
        except ipaddress.AddressValueError as exc:
            raise RuntimeError(
                f"{context} must contain a canonical IPv4 address"
            ) from exc
        if str(address) != host:
            raise RuntimeError(f"{context} must contain a canonical IPv4 address")
    elif protocol == "ip6":
        try:
            ipv6_address = ipaddress.IPv6Address(host)
        except ipaddress.AddressValueError as exc:
            raise RuntimeError(
                f"{context} must contain a canonical lowercase IPv6 address"
            ) from exc
        if ipv6_address.compressed != host:
            raise RuntimeError(
                f"{context} must contain a canonical lowercase IPv6 address"
            )
    else:
        require_vpn_tls_server_name(host, context=f"{context} host")
    try:
        port = int(port_literal, 10)
    except ValueError as exc:
        raise RuntimeError(
            f"{context} must contain a canonical non-zero UDP port"
        ) from exc
    if not 1 <= port <= 65535 or str(port) != port_literal:
        raise RuntimeError(f"{context} must contain a canonical non-zero UDP port")
    return value


def normalize_vpn_canonical_hex_input(
    value: Union[str, bytes, bytearray, memoryview],
    *,
    context: str,
    expected_length: int,
) -> str:
    """Normalize bytes or require an already-canonical lowercase hex literal."""

    if isinstance(value, (bytes, bytearray, memoryview)):
        literal = bytes(value).hex()
        if not literal:
            raise RuntimeError(f"{context} must be a non-empty hex string")
        if len(literal) != expected_length:
            raise RuntimeError(f"{context} must contain {expected_length} hex characters")
        return literal
    return _require_exact_lower_hex_string(
        value, context=context, expected_length=expected_length
    )


def parse_vpn_trust_fields(
    record: Mapping[str, Any], *, context: str, allow_empty: bool = False
) -> dict[str, str]:
    """Parse the exact trust tuple shared by VPN response objects."""

    return {
        "relay_id_hex": require_vpn_relay_id(
            record.get("relay_id_hex"),
            context=f"{context}.relay_id_hex",
            allow_empty=allow_empty,
        ),
        "relay_mldsa65_public_key_hex": require_vpn_mldsa65_public_key(
            record.get("relay_mldsa65_public_key_hex"),
            context=f"{context}.relay_mldsa65_public_key_hex",
            allow_empty=allow_empty,
        ),
        "descriptor_commit_hex": require_vpn_trust_digest(
            record.get("descriptor_commit_hex"),
            context=f"{context}.descriptor_commit_hex",
            allow_empty=allow_empty,
        ),
        "tls_server_name": require_vpn_tls_server_name(
            record.get("tls_server_name"),
            context=f"{context}.tls_server_name",
            allow_empty=allow_empty,
        ),
        "relay_tls_spki_sha256_hex": require_vpn_trust_digest(
            record.get("relay_tls_spki_sha256_hex"),
            context=f"{context}.relay_tls_spki_sha256_hex",
            allow_empty=allow_empty,
        ),
        "relay_certificate_sha256_hex": require_vpn_trust_digest(
            record.get("relay_certificate_sha256_hex"),
            context=f"{context}.relay_certificate_sha256_hex",
            allow_empty=allow_empty,
        ),
        "directory_snapshot_digest_hex": require_vpn_trust_digest(
            record.get("directory_snapshot_digest_hex"),
            context=f"{context}.directory_snapshot_digest_hex",
            allow_empty=allow_empty,
        ),
    }
