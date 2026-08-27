"""Internal structural parser for canonical AccountId I105 literals.

The lightweight client does not depend on the Rust data model, so account
identifiers received from or submitted to Torii are checked here against the
same first-release address envelope.  This module intentionally validates the
canonical public-key wire envelope and performs the pure arithmetic checks
that do not require an optional signature-library dependency.
"""

from __future__ import annotations

from typing import Dict, List, Optional, Sequence, Tuple

BASE58_ALPHABET = tuple(
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
)
IROHA_POEM_KANA_HALFWIDTH = (
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
I105_ALPHABET = BASE58_ALPHABET + IROHA_POEM_KANA_HALFWIDTH
I105_INDEX = {symbol: index for index, symbol in enumerate(I105_ALPHABET)}
I105_BASE = len(I105_ALPHABET)
I105_CHECKSUM_LEN = 6
I105_BECH32M_CONST = 0x2BC830A3
I105_SENTINELS = ("sora", "test", "dev")
I105_SENTINEL_DISCRIMINANTS = {"sora": 0x02F1, "test": 0x0171, "dev": 0}
I105_NUMERIC_SENTINEL_PREFIX = "n"
I105_DISCRIMINANT_MAX = 0xFFFF

_ADDRESS_VERSION_V1 = 0
_ADDRESS_NORM_VERSION_V1 = 1
_ADDRESS_CLASS_SINGLE = 0
_ADDRESS_CLASS_MULTISIG = 1
_CONTROLLER_SINGLE_KEY_TAG = 0
_CONTROLLER_MULTISIG_TAG = 1
_CONTROLLER_SINGLE_KEY_EXTENDED_TAG = 2
_MULTISIG_POLICY_VERSION_V1 = 1

# Curve ids and public-key payload widths are the first-release registry in
# ``iroha_data_model::account::curve``.  SM2 is a variable-width envelope and
# is checked separately below.
_CURVE_PUBLIC_KEY_LENGTHS: Dict[int, int] = {
    1: 32,  # Ed25519
    2: 1_952,  # ML-DSA-65
    3: 48,  # BLS12-381 normal
    4: 33,  # secp256k1
    5: 96,  # BLS12-381 small
    10: 64,  # GOST R 34.10-2012 256, parameter set A
    11: 64,  # GOST R 34.10-2012 256, parameter set B
    12: 64,  # GOST R 34.10-2012 256, parameter set C
    13: 128,  # GOST R 34.10-2012 512, parameter set A
    14: 128,  # GOST R 34.10-2012 512, parameter set B
}
_CURVE_ALGORITHM_NAMES = {
    1: "ed25519",
    2: "ml-dsa",
    3: "bls_normal",
    4: "secp256k1",
    5: "bls_small",
    10: "gost3410-2012-256-paramset-a",
    11: "gost3410-2012-256-paramset-b",
    12: "gost3410-2012-256-paramset-c",
    13: "gost3410-2012-512-paramset-a",
    14: "gost3410-2012-512-paramset-b",
    15: "sm2",
}
_SM2_CURVE_ID = 15
_SM2_SEC1_PUBLIC_KEY_LENGTH = 65
_SM2_MAX_DISTID_BYTES = I105_DISCRIMINANT_MAX // 8

_ED25519_FIELD_MODULUS = (1 << 255) - 19
_ED25519_CURVE_D = (
    -121665 * pow(121666, _ED25519_FIELD_MODULUS - 2, _ED25519_FIELD_MODULUS)
) % _ED25519_FIELD_MODULUS
_ED25519_SQRT_MINUS_ONE = pow(
    2, (_ED25519_FIELD_MODULUS - 1) // 4, _ED25519_FIELD_MODULUS
)
_ED25519_SUBGROUP_ORDER = (
    (1 << 252) + 27742317777372353535851937790883648493
)
_SECP256K1_FIELD_MODULUS = (1 << 256) - (1 << 32) - 977
_SM2_FIELD_MODULUS = int(
    "FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16
)
_SM2_CURVE_A = int(
    "FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16
)
_SM2_CURVE_B = int(
    "28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16
)


def _decode_base_n(digits: Sequence[int], base: int) -> bytes:
    value = 0
    for digit in digits:
        value = value * base + digit
    decoded = (
        b""
        if value == 0
        else value.to_bytes((value.bit_length() + 7) // 8, byteorder="big")
    )
    leading_zeroes = 0
    for digit in digits:
        if digit != 0:
            break
        leading_zeroes += 1
    return b"\x00" * leading_zeroes + decoded


def _convert_to_base32(data: bytes) -> List[int]:
    accumulator = 0
    bits = 0
    result: List[int] = []
    for byte in data:
        accumulator = (accumulator << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            result.append((accumulator >> bits) & 0x1F)
    if bits:
        result.append((accumulator << (5 - bits)) & 0x1F)
    return result


def _bech32_polymod(values: Sequence[int]) -> int:
    generators = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
    check = 1
    for value in values:
        top = check >> 25
        check = ((check & 0x1FFFFFF) << 5) ^ value
        for index, generator in enumerate(generators):
            if (top >> index) & 1:
                check ^= generator
    return check


def _i105_checksum_digits(canonical: bytes) -> List[int]:
    hrp = "snx"
    expanded_hrp = [ord(char) >> 5 for char in hrp]
    expanded_hrp.append(0)
    expanded_hrp.extend(ord(char) & 0x1F for char in hrp)
    values = expanded_hrp + _convert_to_base32(canonical) + [0] * I105_CHECKSUM_LEN
    polymod = _bech32_polymod(values) ^ I105_BECH32M_CONST
    return [
        (polymod >> (5 * (I105_CHECKSUM_LEN - 1 - index))) & 0x1F
        for index in range(I105_CHECKSUM_LEN)
    ]


def parse_i105_sentinel_and_payload(encoded: str) -> Tuple[str, int, str]:
    """Return the literal sentinel, chain discriminant, and encoded payload."""

    if not isinstance(encoded, str):
        raise ValueError("i105 address must be a string")
    for sentinel in I105_SENTINELS:
        if encoded.startswith(sentinel):
            return (
                sentinel,
                I105_SENTINEL_DISCRIMINANTS[sentinel],
                encoded[len(sentinel) :],
            )
    if encoded.startswith(I105_NUMERIC_SENTINEL_PREFIX):
        index = len(I105_NUMERIC_SENTINEL_PREFIX)
        while index < len(encoded) and "0" <= encoded[index] <= "9":
            index += 1
        if index > len(I105_NUMERIC_SENTINEL_PREFIX):
            discriminant = int(
                encoded[len(I105_NUMERIC_SENTINEL_PREFIX) : index]
            )
            if discriminant > I105_DISCRIMINANT_MAX:
                raise ValueError(
                    "i105 chain discriminant must fit in an unsigned 16-bit integer"
                )
            return encoded[:index], discriminant, encoded[index:]
    raise ValueError("i105 address is missing the expected chain-discriminant sentinel")


def encode_i105_account_id(canonical: bytes, discriminant: int) -> str:
    """Render canonical account-address bytes with their canonical I105 sentinel."""

    if not isinstance(canonical, bytes):
        raise TypeError("canonical account-address bytes must be bytes")
    if (
        isinstance(discriminant, bool)
        or not isinstance(discriminant, int)
        or not 0 <= discriminant <= I105_DISCRIMINANT_MAX
    ):
        raise ValueError("i105 chain discriminant must fit in an unsigned 16-bit integer")

    leading_zeroes = len(canonical) - len(canonical.lstrip(b"\x00"))
    value = int.from_bytes(canonical, byteorder="big")
    digits: List[int] = []
    while value:
        value, remainder = divmod(value, I105_BASE)
        digits.append(remainder)
    encoded_digits = [0] * leading_zeroes + list(reversed(digits))
    if not encoded_digits:
        encoded_digits = [0]

    sentinel = next(
        (
            name
            for name, known_discriminant in I105_SENTINEL_DISCRIMINANTS.items()
            if known_discriminant == discriminant
        ),
        f"{I105_NUMERIC_SENTINEL_PREFIX}{discriminant}",
    )
    return sentinel + "".join(
        I105_ALPHABET[digit]
        for digit in (*encoded_digits, *_i105_checksum_digits(canonical))
    )


def _take(canonical: bytes, cursor: int, length: int, context: str) -> Tuple[bytes, int]:
    end = cursor + length
    if end > len(canonical):
        raise ValueError(f"truncated {context} in account-address payload")
    return canonical[cursor:end], end


def _ed25519_add(
    left: Tuple[int, int, int, int], right: Tuple[int, int, int, int]
) -> Tuple[int, int, int, int]:
    """Add extended Edwards coordinates using the complete Ed25519 formula."""

    modulus = _ED25519_FIELD_MODULUS
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = ((y1 - x1) * (y2 - x2)) % modulus
    b = ((y1 + x1) * (y2 + x2)) % modulus
    c = (2 * _ED25519_CURVE_D * t1 * t2) % modulus
    d = (2 * z1 * z2) % modulus
    e = (b - a) % modulus
    f = (d - c) % modulus
    g = (d + c) % modulus
    h = (b + a) % modulus
    return (e * f % modulus, g * h % modulus, f * g % modulus, e * h % modulus)


def _ed25519_multiply(
    point: Tuple[int, int, int, int], scalar: int
) -> Tuple[int, int, int, int]:
    result = (0, 1, 1, 0)
    addend = point
    while scalar:
        if scalar & 1:
            result = _ed25519_add(result, addend)
        addend = _ed25519_add(addend, addend)
        scalar >>= 1
    return result


def _ed25519_is_identity(point: Tuple[int, int, int, int]) -> bool:
    x, y, z, _ = point
    modulus = _ED25519_FIELD_MODULUS
    return x % modulus == 0 and (y - z) % modulus == 0


def _validate_ed25519_public_key(payload: bytes) -> None:
    encoded_y = bytearray(payload)
    sign = encoded_y[31] >> 7
    encoded_y[31] &= 0x7F
    y = int.from_bytes(encoded_y, byteorder="little")
    modulus = _ED25519_FIELD_MODULUS
    if y >= modulus:
        raise ValueError("non-canonical Ed25519 public-key encoding")

    y_squared = y * y % modulus
    numerator = (y_squared - 1) % modulus
    denominator = (_ED25519_CURVE_D * y_squared + 1) % modulus
    if denominator == 0:
        raise ValueError("invalid compressed Ed25519 public key")
    x_squared = numerator * pow(denominator, modulus - 2, modulus) % modulus
    x = pow(x_squared, (modulus + 3) // 8, modulus)
    if x * x % modulus != x_squared:
        x = x * _ED25519_SQRT_MINUS_ONE % modulus
    if x * x % modulus != x_squared:
        raise ValueError("invalid compressed Ed25519 public key")
    if x & 1 != sign:
        x = (-x) % modulus

    rerendered = bytearray(y.to_bytes(32, byteorder="little"))
    rerendered[31] |= (x & 1) << 7
    if bytes(rerendered) != payload:
        raise ValueError("non-canonical Ed25519 public-key encoding")

    point = (x, y, 1, x * y % modulus)
    if _ed25519_is_identity(_ed25519_multiply(point, 8)):
        raise ValueError("small-order Ed25519 public key")
    if not _ed25519_is_identity(
        _ed25519_multiply(point, _ED25519_SUBGROUP_ORDER)
    ):
        raise ValueError("Ed25519 public key is outside the prime-order subgroup")


def _validate_secp256k1_public_key(payload: bytes) -> None:
    if payload[0] not in (0x02, 0x03):
        raise ValueError("invalid secp256k1 public-key envelope")
    x = int.from_bytes(payload[1:], byteorder="big")
    modulus = _SECP256K1_FIELD_MODULUS
    if x >= modulus:
        raise ValueError("invalid compressed secp256k1 public key")
    y_squared = (pow(x, 3, modulus) + 7) % modulus
    y = pow(y_squared, (modulus + 1) // 4, modulus)
    if y * y % modulus != y_squared:
        raise ValueError("invalid compressed secp256k1 public key")


def _validate_sm2_sec1_public_key(sec1: bytes) -> None:
    x = int.from_bytes(sec1[1:33], byteorder="big")
    y = int.from_bytes(sec1[33:65], byteorder="big")
    modulus = _SM2_FIELD_MODULUS
    if x >= modulus or y >= modulus:
        raise ValueError("invalid SM2 SEC1 public key")
    expected = (pow(x, 3, modulus) + _SM2_CURVE_A * x + _SM2_CURVE_B) % modulus
    if y * y % modulus != expected:
        raise ValueError("invalid SM2 SEC1 public key")


def _validate_public_key(curve: int, payload: bytes) -> None:
    if curve == _SM2_CURVE_ID:
        if len(payload) < 2:
            raise ValueError("invalid SM2 public-key envelope")
        distid_length = int.from_bytes(payload[:2], byteorder="big")
        if distid_length > _SM2_MAX_DISTID_BYTES:
            raise ValueError("invalid SM2 public-key envelope")
        sm2_expected_length = 2 + distid_length + _SM2_SEC1_PUBLIC_KEY_LENGTH
        if len(payload) != sm2_expected_length:
            raise ValueError("invalid SM2 public-key envelope")
        try:
            payload[2 : 2 + distid_length].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ValueError("invalid SM2 public-key distinguishing identifier") from exc
        sec1 = payload[2 + distid_length :]
        if sec1[0] != 0x04:
            raise ValueError("invalid SM2 SEC1 public-key envelope")
        _validate_sm2_sec1_public_key(sec1)
        return

    expected_length = _CURVE_PUBLIC_KEY_LENGTHS.get(curve)
    if expected_length is None:
        raise ValueError(f"unknown account-controller curve id {curve}")
    if len(payload) != expected_length or not any(payload):
        raise ValueError(f"invalid public-key material for curve id {curve}")
    if curve == 1:
        _validate_ed25519_public_key(payload)
    elif curve == 4:
        _validate_secp256k1_public_key(payload)


def _decode_single_key(canonical: bytes, cursor: int, *, extended: bool) -> int:
    fixed, cursor = _take(
        canonical,
        cursor,
        3 if extended else 2,
        "single-key controller header",
    )
    curve = fixed[0]
    if extended:
        key_length = int.from_bytes(fixed[1:3], byteorder="big")
        if key_length <= 0xFF:
            raise ValueError("short public keys must use the compact controller tag")
    else:
        key_length = fixed[1]
    public_key, cursor = _take(
        canonical, cursor, key_length, "single-key controller public key"
    )
    _validate_public_key(curve, public_key)
    return cursor


def _decode_multisig(canonical: bytes, cursor: int) -> int:
    fixed, cursor = _take(canonical, cursor, 5, "multisig policy header")
    version = fixed[0]
    threshold = int.from_bytes(fixed[1:3], byteorder="big")
    member_count = int.from_bytes(fixed[3:5], byteorder="big")
    if version != _MULTISIG_POLICY_VERSION_V1:
        raise ValueError(f"unsupported multisig policy version {version}")
    if threshold == 0:
        raise ValueError("multisig policy threshold must be non-zero")
    if member_count == 0:
        raise ValueError("multisig policy must contain at least one member")

    total_weight = 0
    member_keys = set()
    member_sort_keys = []
    for member_index in range(member_count):
        member_header, cursor = _take(
            canonical, cursor, 5, f"multisig member {member_index} header"
        )
        curve = member_header[0]
        weight = int.from_bytes(member_header[1:3], byteorder="big")
        key_length = int.from_bytes(member_header[3:5], byteorder="big")
        if weight == 0:
            raise ValueError("multisig member weight must be non-zero")
        public_key, cursor = _take(
            canonical,
            cursor,
            key_length,
            f"multisig member {member_index} public key",
        )
        _validate_public_key(curve, public_key)
        identity = (curve, public_key)
        if identity in member_keys:
            raise ValueError("multisig policy contains a duplicate public key")
        member_keys.add(identity)
        member_sort_keys.append((_CURVE_ALGORITHM_NAMES[curve], public_key))
        total_weight += weight

    if threshold > total_weight:
        raise ValueError("multisig policy threshold exceeds total member weight")
    if member_sort_keys != sorted(member_sort_keys):
        raise ValueError("multisig policy members are not in canonical order")
    return cursor


def validate_canonical_account_id_bytes(canonical: bytes) -> None:
    """Validate the exact first-release account-address byte envelope."""

    if not canonical:
        raise ValueError("account-address payload must be non-empty")
    header = canonical[0]
    if header & 1:
        raise ValueError("account-address header extension flag is unsupported")
    version = header >> 5
    address_class = (header >> 3) & 0b11
    norm_version = (header >> 1) & 0b11
    if address_class not in (_ADDRESS_CLASS_SINGLE, _ADDRESS_CLASS_MULTISIG):
        raise ValueError(f"unknown account-address class {address_class}")
    if version != _ADDRESS_VERSION_V1:
        raise ValueError(f"unsupported account-address version {version}")
    if norm_version != _ADDRESS_NORM_VERSION_V1:
        raise ValueError(
            f"unsupported account-address normalization version {norm_version}"
        )

    tag_bytes, cursor = _take(canonical, 1, 1, "account controller tag")
    controller_tag = tag_bytes[0]
    if controller_tag == _CONTROLLER_SINGLE_KEY_TAG:
        controller_class = _ADDRESS_CLASS_SINGLE
        cursor = _decode_single_key(canonical, cursor, extended=False)
    elif controller_tag == _CONTROLLER_SINGLE_KEY_EXTENDED_TAG:
        controller_class = _ADDRESS_CLASS_SINGLE
        cursor = _decode_single_key(canonical, cursor, extended=True)
    elif controller_tag == _CONTROLLER_MULTISIG_TAG:
        controller_class = _ADDRESS_CLASS_MULTISIG
        cursor = _decode_multisig(canonical, cursor)
    else:
        raise ValueError(f"unknown account controller tag {controller_tag}")

    if address_class != controller_class:
        raise ValueError("account-address header class does not match its controller")
    if cursor != len(canonical):
        raise ValueError("unexpected trailing account-address bytes")


def decode_i105_account_id(
    encoded: str, *, expected_discriminant: Optional[int] = None
) -> bytes:
    """Decode an I105 AccountId and validate its structural controller grammar."""

    _, discriminant, payload = parse_i105_sentinel_and_payload(encoded)
    if expected_discriminant is not None:
        if (
            isinstance(expected_discriminant, bool)
            or not isinstance(expected_discriminant, int)
            or not 0 <= expected_discriminant <= I105_DISCRIMINANT_MAX
        ):
            raise ValueError(
                "expected i105 chain discriminant must fit in an unsigned 16-bit integer"
            )
        if discriminant != expected_discriminant:
            raise ValueError(
                "i105 chain discriminant mismatch: "
                f"expected {expected_discriminant}, found {discriminant}"
            )

    try:
        digits = [I105_INDEX[symbol] for symbol in payload]
    except KeyError as exc:
        raise ValueError("invalid character in i105 address") from exc
    if len(digits) <= I105_CHECKSUM_LEN:
        raise ValueError("i105 address too short")
    data_digits = digits[:-I105_CHECKSUM_LEN]
    checksum_digits = digits[-I105_CHECKSUM_LEN:]
    canonical = _decode_base_n(data_digits, I105_BASE)
    if checksum_digits != _i105_checksum_digits(canonical):
        raise ValueError("i105 checksum mismatch")
    validate_canonical_account_id_bytes(canonical)
    return canonical


def decode_canonical_i105_account_id(
    encoded: str, *, expected_discriminant: Optional[int] = None
) -> bytes:
    """Decode an AccountId and reject every non-canonical I105 rendering."""

    _, discriminant, _ = parse_i105_sentinel_and_payload(encoded)
    canonical = decode_i105_account_id(
        encoded, expected_discriminant=expected_discriminant
    )
    if encode_i105_account_id(canonical, discriminant) != encoded:
        raise ValueError("i105 address must use its exact canonical rendering")
    return canonical
