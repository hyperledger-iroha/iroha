"""Account address encoding helpers matching the Rust data-model implementation."""

from __future__ import annotations

import binascii
import hashlib
from dataclasses import dataclass
from enum import Enum, IntEnum
from typing import Iterable, List, Mapping, Optional, Sequence, Tuple

DEFAULT_DOMAIN_NAME = "default"

LOCAL_DOMAIN_KEY = b"SORA-LOCAL-K:v1"
IH58_CHECKSUM_PREFIX = b"IH58PRE"
HEADER_VERSION_V1 = 0
HEADER_NORM_VERSION_V1 = 1
COMPRESSED_SENTINEL = "snx1"
COMPRESSED_CHECKSUM_LEN = 6
BECH32M_CONST = 0x2BC830A3
IH58_CHECKSUM_BYTES = 2
COMPRESSED_WARNING = (
    "Compressed Sora addresses rely on half-width kana and are only interoperable inside Sora-aware apps. "
    "Prefer IH58 when sharing with explorers, wallets, or QR codes. See docs/source/sns/address_display_guidelines.md."
)
COMPRESSED_WARNING = (
    "Compressed Sora addresses rely on half-width kana and are only interoperable inside Sora-aware apps. "
    "Prefer IH58 when sharing with explorers, wallets, or QR codes."
)

IH58_ALPHABET: Tuple[str, ...] = (
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "J",
    "K",
    "L",
    "M",
    "N",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
)

SORA_KANA: Tuple[str, ...] = (
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

COMPRESSED_ALPHABET: Tuple[str, ...] = IH58_ALPHABET + SORA_KANA
COMPRESSED_BASE = len(COMPRESSED_ALPHABET)
COMPRESSED_INDEX = {symbol: idx for idx, symbol in enumerate(COMPRESSED_ALPHABET)}
IH58_INDEX = {symbol: idx for idx, symbol in enumerate(IH58_ALPHABET)}


class AccountAddressError(ValueError):
    """Raised when an address cannot be parsed or encoded."""


class AccountAddressFormat(Enum):
    """Textual format used to render or parse an address."""

    IH58 = "ih58"
    COMPRESSED = "compressed"
    CANONICAL_HEX = "canonical_hex"


class AddressClass(IntEnum):
    SINGLE_KEY = 0
    MULTI_SIG = 1


@dataclass(frozen=True)
class AddressHeader:
    version: int
    class_: AddressClass
    norm_version: int
    ext_flag: bool = False

    @classmethod
    def new(cls, version: int, class_: AddressClass, norm_version: int) -> "AddressHeader":
        if not 0 <= version <= 0b111:
            raise AccountAddressError(f"invalid address header version: {version}")
        if not 0 <= norm_version <= 0b11:
            raise AccountAddressError(f"invalid normalization version: {norm_version}")
        return cls(version=version, class_=class_, norm_version=norm_version, ext_flag=False)

    def encode(self) -> int:
        byte = (self.version & 0b111) << 5
        byte |= (int(self.class_) & 0b11) << 3
        byte |= (self.norm_version & 0b11) << 1
        byte |= 1 if self.ext_flag else 0
        return byte

    @classmethod
    def decode(cls, byte: int) -> "AddressHeader":
        version = (byte >> 5) & 0b111
        class_bits = (byte >> 3) & 0b11
        norm_version = (byte >> 1) & 0b11
        ext_flag = bool(byte & 0b1)
        if ext_flag:
            raise AccountAddressError("address header reserves extension flag but it was set")
        try:
            class_ = AddressClass(class_bits)
        except ValueError as exc:
            raise AccountAddressError(f"unknown address class: {class_bits}") from exc
        return cls.new(version, class_, norm_version)


@dataclass(frozen=True)
class DomainSelector:
    tag: int
    payload: Optional[bytes] = None

    @classmethod
    def default(cls) -> "DomainSelector":
        return cls(tag=0)

    @classmethod
    def local12(cls, digest: bytes) -> "DomainSelector":
        if len(digest) != 12:
            raise AccountAddressError("local domain digest must be 12 bytes")
        return cls(tag=1, payload=digest)

    @classmethod
    def global_id(cls, registry_id: int) -> "DomainSelector":
        if not 0 <= registry_id <= 0xFFFF_FFFF:
            raise AccountAddressError("registry id out of range")
        return cls(tag=2, payload=registry_id.to_bytes(4, "big"))

    @classmethod
    def from_domain(cls, domain: str) -> "DomainSelector":
        if domain.lower() == DEFAULT_DOMAIN_NAME:
            return cls.default()
        digest = compute_local_digest(domain)
        return cls.local12(digest)

    def encode_into(self, out: bytearray) -> None:
        out.append(self.tag)
        if self.payload:
            out.extend(self.payload)

    @classmethod
    def decode(cls, data: bytes, cursor: int) -> Tuple["DomainSelector", int]:
        if cursor >= len(data):
            raise AccountAddressError("invalid length for address payload")
        tag = data[cursor]
        cursor += 1
        if tag == 0x00:
            return cls.default(), cursor
        if tag == 0x01:
            end = cursor + 12
            if end > len(data):
                raise AccountAddressError("invalid length for address payload")
            return cls.local12(data[cursor:end]), end
        if tag == 0x02:
            end = cursor + 4
            if end > len(data):
                raise AccountAddressError("invalid length for address payload")
            return cls(tag=tag, payload=data[cursor:end]), end
        raise AccountAddressError(f"unknown domain selector tag: {tag}")


class CurveId(IntEnum):
    ED25519 = 1
    MLDSA = 2
    GOST_256_A = 10
    GOST_256_B = 11
    GOST_256_C = 12
    GOST_512_A = 13
    GOST_512_B = 14

    @classmethod
    def from_algorithm(cls, algorithm: str) -> "CurveId":
        normalized = algorithm.strip().lower()
        mapping = {
            "ed25519": cls.ED25519,
            "ed": cls.ED25519,
            "mldsa": cls.MLDSA,
            "ml-dsa": cls.MLDSA,
            "ml_dsa": cls.MLDSA,
            "gost256a": cls.GOST_256_A,
            "gost-256-a": cls.GOST_256_A,
            "gost256b": cls.GOST_256_B,
            "gost-256-b": cls.GOST_256_B,
            "gost256c": cls.GOST_256_C,
            "gost-256-c": cls.GOST_256_C,
            "gost512a": cls.GOST_512_A,
            "gost-512-a": cls.GOST_512_A,
            "gost512b": cls.GOST_512_B,
            "gost-512-b": cls.GOST_512_B,
        }
        try:
            return mapping[normalized]
        except KeyError as exc:
            raise AccountAddressError(f"unsupported signing algorithm: {algorithm}") from exc

    @classmethod
    def from_byte(cls, value: int) -> "CurveId":
        try:
            return cls(value)
        except ValueError as exc:
            raise AccountAddressError(f"unknown curve id: {value}") from exc


@dataclass(frozen=True)
class ControllerPayload:
    tag: int
    curve: CurveId
    public_key: bytes

    CONTROLLER_SINGLE_KEY_TAG = 0x00

    @classmethod
    def single_key(cls, public_key: bytes, algorithm: str) -> "ControllerPayload":
        curve = CurveId.from_algorithm(algorithm)
        if len(public_key) > 0xFF:
            raise AccountAddressError(f"key payload too long: {len(public_key)} bytes")
        return cls(tag=cls.CONTROLLER_SINGLE_KEY_TAG, curve=curve, public_key=bytes(public_key))

    def encode_into(self, out: bytearray) -> None:
        if self.tag != self.CONTROLLER_SINGLE_KEY_TAG:
            raise AccountAddressError("unsupported controller payload variant")
        out.append(self.tag)
        out.append(int(self.curve))
        out.append(len(self.public_key))
        out.extend(self.public_key)

    @classmethod
    def decode(cls, data: bytes, cursor: int) -> Tuple["ControllerPayload", int]:
        if cursor >= len(data):
            raise AccountAddressError("invalid length for address payload")
        tag = data[cursor]
        cursor += 1
        if tag != cls.CONTROLLER_SINGLE_KEY_TAG:
            raise AccountAddressError(f"unknown controller payload tag: {tag}")
        if cursor >= len(data):
            raise AccountAddressError("invalid length for address payload")
        curve = CurveId.from_byte(data[cursor])
        cursor += 1
        if cursor >= len(data):
            raise AccountAddressError("invalid length for address payload")
        length = data[cursor]
        cursor += 1
        end = cursor + length
        if end > len(data):
            raise AccountAddressError("invalid length for address payload")
        public_key = data[cursor:end]
        return cls(tag=tag, curve=curve, public_key=public_key), end


@dataclass(frozen=True)
class AccountAddress:
    header: AddressHeader
    domain: DomainSelector
    controller: ControllerPayload

    @classmethod
    def from_account(
        cls, *, domain: str, public_key: bytes, algorithm: str = "ed25519"
    ) -> "AccountAddress":
        header = AddressHeader.new(
            version=HEADER_VERSION_V1, class_=AddressClass.SINGLE_KEY, norm_version=HEADER_NORM_VERSION_V1
        )
        selector = DomainSelector.from_domain(domain)
        controller = ControllerPayload.single_key(public_key, algorithm)
        return cls(header=header, domain=selector, controller=controller)

    @classmethod
    def from_canonical_bytes(cls, payload: bytes) -> "AccountAddress":
        if not payload:
            raise AccountAddressError("invalid length for address payload")
        header = AddressHeader.decode(payload[0])
        cursor = 1
        domain, cursor = DomainSelector.decode(payload, cursor)
        controller, cursor = ControllerPayload.decode(payload, cursor)
        if cursor != len(payload):
            raise AccountAddressError("unexpected trailing bytes in canonical payload")
        return cls(header=header, domain=domain, controller=controller)

    @classmethod
    def from_canonical_hex(cls, encoded: str) -> "AccountAddress":
        body = encoded[2:] if encoded.lower().startswith("0x") else encoded
        try:
            payload = binascii.unhexlify(body)
        except binascii.Error as exc:
            raise AccountAddressError("invalid canonical hex account address") from exc
        return cls.from_canonical_bytes(payload)

    @classmethod
    def from_ih58(
        cls, encoded: str, expected_prefix: Optional[int] = None
    ) -> "AccountAddress":
        prefix, payload = decode_ih58_string(encoded)
        if expected_prefix is not None and prefix != expected_prefix:
            raise AccountAddressError(
                f"unexpected IH58 network prefix: expected {expected_prefix}, found {prefix}"
            )
        return cls.from_canonical_bytes(payload)

    @classmethod
    def from_compressed_sora(cls, encoded: str) -> "AccountAddress":
        canonical = decode_compressed_string(encoded)
        return cls.from_canonical_bytes(canonical)

    @classmethod
    def parse_any(
        cls, address: str, expected_prefix: Optional[int] = None
    ) -> Tuple["AccountAddress", AccountAddressFormat]:
        token = address.strip()
        if not token:
            raise AccountAddressError("invalid length for address payload")
        if token.startswith(COMPRESSED_SENTINEL):
            parsed = cls.from_compressed_sora(token)
            return parsed, AccountAddressFormat.COMPRESSED
        if token.startswith(("0x", "0X")):
            parsed = cls.from_canonical_hex(token)
            return parsed, AccountAddressFormat.CANONICAL_HEX
        try:
            parsed = cls.from_ih58(token, expected_prefix=expected_prefix)
            return parsed, AccountAddressFormat.IH58
        except AccountAddressError as err:
            raise AccountAddressError("unsupported address format") from err

    def canonical_bytes(self) -> bytes:
        out = bytearray()
        out.append(self.header.encode())
        self.domain.encode_into(out)
        self.controller.encode_into(out)
        return bytes(out)

    def canonical_hex(self) -> str:
        return "0x" + binascii.hexlify(self.canonical_bytes()).decode("ascii")

    def to_ih58(self, network_prefix: int) -> str:
        return encode_ih58_string(network_prefix, self.canonical_bytes())

    def to_compressed_sora(self) -> str:
        return encode_compressed_string(self.canonical_bytes())

    def display_formats(self, network_prefix: int = 753) -> Mapping[str, object]:
        ih58 = self.to_ih58(network_prefix)
        compressed = self.to_compressed_sora()
        return {
            "ih58": ih58,
            "compressed": compressed,
            "network_prefix": network_prefix,
            "compressed_warning": COMPRESSED_WARNING,
        }

    def __str__(self) -> str:
        return self.canonical_hex()


def compute_local_digest(label: str) -> bytes:
    mac = hashlib.blake2s(label.encode("utf-8"), key=LOCAL_DOMAIN_KEY, digest_size=32)
    return mac.digest()[:12]


def encode_ih58_prefix(prefix: int) -> bytes:
    if prefix < 0 or prefix > 0x3FFF:
        raise AccountAddressError(f"invalid IH58 prefix: {prefix}")
    if prefix <= 63:
        return bytes([prefix])
    lower = (prefix & 0b0011_1111) | 0b0100_0000
    upper = prefix >> 6
    return bytes([lower, upper])


def decode_ih58_prefix(buffer: bytes) -> Tuple[int, int]:
    if not buffer:
        raise AccountAddressError("invalid length for IH58 payload")
    first = buffer[0]
    if first <= 63:
        return first, 1
    if first & 0b0100_0000:
        if len(buffer) < 2:
            raise AccountAddressError("invalid length for IH58 payload")
        second = buffer[1]
        value = ((second & 0xFF) << 6) | (first & 0b0011_1111)
        return value, 2
    raise AccountAddressError(f"invalid IH58 prefix encoding: {first}")


def encode_ih58_string(prefix: int, canonical: bytes) -> str:
    body = bytearray(encode_ih58_prefix(prefix))
    body.extend(canonical)
    checksum_input = IH58_CHECKSUM_PREFIX + body
    checksum = hashlib.blake2b(checksum_input, digest_size=64).digest()
    body.extend(checksum[:IH58_CHECKSUM_BYTES])
    digits = encode_base_n(bytes(body), 58)
    return "".join(IH58_ALPHABET[d] for d in digits)


def decode_ih58_string(encoded: str) -> Tuple[int, bytes]:
    digits: List[int] = []
    for ch in encoded:
        try:
            digits.append(IH58_INDEX[ch])
        except KeyError as exc:
            raise AccountAddressError("invalid IH58 base58 encoding") from exc
    body = decode_base_n(digits, 58)
    if len(body) < 1 + IH58_CHECKSUM_BYTES:
        raise AccountAddressError("invalid length for IH58 payload")
    payload = body[:-IH58_CHECKSUM_BYTES]
    checksum_bytes = body[-IH58_CHECKSUM_BYTES:]
    prefix, prefix_len = decode_ih58_prefix(payload)
    checksum_input = IH58_CHECKSUM_PREFIX + payload
    expected = hashlib.blake2b(checksum_input, digest_size=64).digest()[:IH58_CHECKSUM_BYTES]
    if checksum_bytes != expected:
        raise AccountAddressError("IH58 checksum mismatch")
    canonical = payload[prefix_len:]
    return prefix, canonical


def encode_compressed_string(canonical: bytes) -> str:
    digits = encode_base_n(canonical, COMPRESSED_BASE)
    checksum = compressed_checksum_digits(canonical)
    pieces = [COMPRESSED_SENTINEL]
    pieces.extend(COMPRESSED_ALPHABET[digit] for digit in digits)
    pieces.extend(COMPRESSED_ALPHABET[digit] for digit in checksum)
    return "".join(pieces)


def decode_compressed_string(encoded: str) -> bytes:
    if not encoded.startswith(COMPRESSED_SENTINEL):
        raise AccountAddressError("compressed address must start with snx1 sentinel")
    payload = encoded[len(COMPRESSED_SENTINEL) :]
    if len(payload) <= COMPRESSED_CHECKSUM_LEN:
        raise AccountAddressError("compressed address is too short")
    digits = [compressed_digit(symbol) for symbol in payload]
    data_digits = digits[:-COMPRESSED_CHECKSUM_LEN]
    checksum_digits = digits[-COMPRESSED_CHECKSUM_LEN:]
    canonical = decode_base_n(data_digits, COMPRESSED_BASE)
    expected = compressed_checksum_digits(canonical)
    if list(expected) != checksum_digits:
        raise AccountAddressError("compressed checksum mismatch")
    return canonical


def compressed_digit(symbol: str) -> int:
    try:
        return COMPRESSED_INDEX[symbol]
    except KeyError as exc:
        raise AccountAddressError(f"invalid compressed alphabet symbol: {symbol}") from exc


def encode_base_n(data: bytes, base: int) -> List[int]:
    if base < 2:
        raise AccountAddressError("invalid base for encoding")
    if not data:
        return [0]
    value = list(data)
    leading = 0
    while leading < len(value) and value[leading] == 0:
        leading += 1
    digits: List[int] = []
    start = leading
    while start < len(value):
        remainder = 0
        for idx in range(start, len(value)):
            acc = (remainder << 8) | value[idx]
            value[idx] = acc // base
            remainder = acc % base
        digits.append(remainder)
        while start < len(value) and value[start] == 0:
            start += 1
    digits.extend([0] * leading)
    if not digits:
        digits.append(0)
    return list(reversed(digits))


def decode_base_n(digits: Sequence[int], base: int) -> bytes:
    if base < 2:
        raise AccountAddressError("invalid base for decoding")
    if not digits:
        raise AccountAddressError("invalid length for address payload")
    value = list(digits)
    for digit in value:
        if digit < 0 or digit >= base:
            raise AccountAddressError(f"invalid digit {digit} for base {base}")
    leading = 0
    while leading < len(value) and value[leading] == 0:
        leading += 1
    bytes_out: List[int] = []
    start = leading
    while start < len(value):
        remainder = 0
        for idx in range(start, len(value)):
            acc = remainder * base + value[idx]
            value[idx] = acc // 256
            remainder = acc % 256
        bytes_out.append(remainder)
        while start < len(value) and value[start] == 0:
            start += 1
    bytes_out.extend([0] * leading)
    bytes_out.reverse()
    return bytes(bytes_out)


def convert_to_base32(data: bytes) -> List[int]:
    acc = 0
    bits = 0
    out: List[int] = []
    for byte in data:
        acc = (acc << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            out.append((acc >> bits) & 0x1F)
    if bits:
        out.append((acc << (5 - bits)) & 0x1F)
    return out


def bech32_polymod(values: Iterable[int]) -> int:
    generators = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
    chk = 1
    for value in values:
        top = chk >> 25
        chk = ((chk & 0x1FF_FFFF) << 5) ^ value
        for idx, generator in enumerate(generators):
            if (top >> idx) & 1:
                chk ^= generator
    return chk


def expand_hrp(hrp: str) -> List[int]:
    out: List[int] = []
    for ch in hrp:
        value = ord(ch)
        out.append(value >> 5)
    out.append(0)
    out.extend(ord(ch) & 0x1F for ch in hrp)
    return out


def bech32m_checksum(data: Sequence[int]) -> List[int]:
    values = expand_hrp("snx")
    values.extend(data)
    values.extend([0] * COMPRESSED_CHECKSUM_LEN)
    polymod = bech32_polymod(values) ^ BECH32M_CONST
    return [
        (polymod >> (5 * (COMPRESSED_CHECKSUM_LEN - 1 - i))) & 0x1F
        for i in range(COMPRESSED_CHECKSUM_LEN)
    ]


def compressed_checksum_digits(canonical: bytes) -> List[int]:
    base32_digits = convert_to_base32(canonical)
    return bech32m_checksum(base32_digits)


__all__ = [
    "AccountAddress",
    "AccountAddressError",
    "AccountAddressFormat",
    "DEFAULT_DOMAIN_NAME",
    "COMPRESSED_WARNING",
]
