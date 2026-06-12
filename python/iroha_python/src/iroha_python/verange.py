"""VeRange SDK helpers built on the shared OpenVerify envelope format."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any

from norito.crc64 import crc64

VERANGE_BACKEND = "stark/fri/sha256-goldilocks"
VERANGE_CIRCUIT_ID = "stark/fri/sha256-goldilocks:verange_transparent_range_v1"
VERANGE_DOMAIN_SEPARATOR = "iroha:verange:transparent-range:v1"
VERANGE_DEV_PROOF_PREFIX = b"iroha:verange:dev-fixture:v1:"
VERANGE_MAX_AGGREGATION_COUNT = 1024
VERANGE_MAX_BIT_LENGTH = 256
VERANGE_MAX_PAYLOAD_BYTES = 1024 * 1024
VERANGE_COMMITMENT_SCHEMES = frozenset(
    {
        "pedersen-v1",
        "pedersen-bls12-381",
        "pedersen-decaf377",
        "verange-pedersen-v1",
    }
)

DEFAULT_PRIVACY_MAX_PROOF_BYTES = 64 * 1024 * 1024
DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES = 1024 * 1024
DEFAULT_PRIVACY_MAX_AUX_BYTES = 64 * 1024

_OPEN_VERIFY_SCHEMA_HASH = hashlib.sha256(
    b"norito:v1:type-name\0iroha_data_model::zk::OpenVerifyEnvelope"
).digest()[:16]
_BACKEND_TAGS = {
    "halo2ipapasta": (0, "Halo2IpaPasta"),
    "halo2pasta": (0, "Halo2IpaPasta"),
    "halo2ipa": (0, "Halo2IpaPasta"),
    "pasta": (0, "Halo2IpaPasta"),
    "halo2bn254": (1, "Halo2Bn254"),
    "groth16": (2, "Groth16"),
    "stark": (3, "Stark"),
    "starkfri": (3, "Stark"),
    "starkfrisha256goldilocks": (3, "Stark"),
    "starkfriposeidon2goldilocks": (3, "Stark"),
    "starkfrisha256goldilocksv1": (3, "Stark"),
    "unsupported": (4, "Unsupported"),
    "halo2ipaorchard": (5, "Halo2IpaOrchard"),
    "halo2pastaactionbundle": (5, "Halo2IpaOrchard"),
    "orchard": (5, "Halo2IpaOrchard"),
    "zcashorchard": (5, "Halo2IpaOrchard"),
    "groth16bls12377": (6, "Groth16Bls12377"),
    "groth16bls12377decaf377": (6, "Groth16Bls12377"),
    "bls12377": (6, "Groth16Bls12377"),
    "decaf377": (6, "Groth16Bls12377"),
    "masp": (6, "Groth16Bls12377"),
    "penumbra": (6, "Groth16Bls12377"),
    "penumbramasp": (6, "Groth16Bls12377"),
    "halo2ipapenumbra": (6, "Groth16Bls12377"),
    "halo2ipamasp": (6, "Groth16Bls12377"),
    "fcmppluspluscurvetree": (7, "FcmpPlusPlusCurveTree"),
    "fcmppluspluscurvetreesbulletproofs": (7, "FcmpPlusPlusCurveTree"),
    "fcmp": (7, "FcmpPlusPlusCurveTree"),
    "monero": (7, "FcmpPlusPlusCurveTree"),
    "monerofcmp": (7, "FcmpPlusPlusCurveTree"),
    "monerofcmpplusplus": (7, "FcmpPlusPlusCurveTree"),
    "curvetree": (7, "FcmpPlusPlusCurveTree"),
    "halo2ipamonero": (7, "FcmpPlusPlusCurveTree"),
    "halo2ipacurvetree": (7, "FcmpPlusPlusCurveTree"),
    "latticepcssis": (8, "LatticePcsSis"),
    "latticepcszk": (8, "LatticePcsSis"),
    "jindo": (8, "LatticePcsSis"),
    "jindolatticepcszk": (8, "LatticePcsSis"),
    "jindolatticepcszkv0": (8, "LatticePcsSis"),
    "jindolatticepcssis": (8, "LatticePcsSis"),
    "starkfrimiden": (9, "MidenStark"),
    "midenstark": (9, "MidenStark"),
    "starkvmnotetransaction": (9, "MidenStark"),
    "aztecplonkishprivatekernel": (10, "AztecPlonkishPrivateKernel"),
    "aztecprivatekernel": (10, "AztecPlonkishPrivateKernel"),
    "plonkishprivatekernelrollup": (10, "AztecPlonkishPrivateKernel"),
    "pqmaspstarkfri": (11, "PqMaspStarkFri"),
    "pqmaspstark": (11, "PqMaspStarkFri"),
    "starkfripqmaspstarkfri": (11, "PqMaspStarkFri"),
    "postquantummasp": (11, "PqMaspStarkFri"),
    "anonymouspgc": (12, "AnonymousPgc"),
    "anonymouspgckoutofn": (12, "AnonymousPgc"),
    "anonymouspgckoutofnv1": (12, "AnonymousPgc"),
    "verange": (13, "VeRange"),
    "verangetransparentrange": (13, "VeRange"),
    "verangetransparentrangev1": (13, "VeRange"),
    "zkat": (14, "ZkAt"),
    "zkatpolicyprivateauthenticator": (14, "ZkAt"),
    "zkatpolicyprivateauthv1": (14, "ZkAt"),
    "recursiveanonymousadmission": (15, "RecursiveAnonymousAdmission"),
    "recursiveanonymousadmissionv0": (15, "RecursiveAnonymousAdmission"),
    "zkamsrecursiveadmission": (15, "RecursiveAnonymousAdmission"),
    "zkamsrecursiveadmissionv0": (15, "RecursiveAnonymousAdmission"),
    "vegaexistingcredentialzk": (16, "VegaExistingCredentialZk"),
    "vegaexistingcredentialzkv0": (16, "VegaExistingCredentialZk"),
    "silentthresholdanoncred": (17, "SilentThresholdAnoncred"),
    "silentthresholdanoncredv0": (17, "SilentThresholdAnoncred"),
    "silentthresholdanonymouscredential": (17, "SilentThresholdAnoncred"),
    "thresholdanonymouscredentials": (17, "SilentThresholdAnoncred"),
    "zkx509": (18, "ZkX509"),
    "zkvmx509identity": (18, "ZkX509"),
    "zkx509onchainidentity": (18, "ZkX509"),
    "zkx509onchainidentityv0": (18, "ZkX509"),
    "siswithhints": (19, "SisWithHints"),
    "sishints": (19, "SisWithHints"),
    "sishintsanoncredpqv0": (19, "SisWithHints"),
    "latticeanonymouscredentials": (19, "SisWithHints"),
}
_BACKEND_NAMES_BY_TAG = {value[0]: value[1] for value in _BACKEND_TAGS.values()}
_MISSING = object()

__all__ = [
    "VERANGE_BACKEND",
    "VERANGE_CIRCUIT_ID",
    "VERANGE_DOMAIN_SEPARATOR",
    "build_range_commitment",
    "build_verange_proof_envelope",
    "build_verange_dev_proof_fixture",
    "build_verange_proof_v1",
    "verify_verange_proof_v1",
    "verify_verange_proof_locally",
    "build_privacy_proof_envelope",
    "decode_privacy_proof_envelope",
    "buildRangeCommitment",
    "buildVeRangeProofEnvelope",
    "buildVeRangeDevProofFixture",
    "buildVeRangeProofV1",
    "verifyVeRangeProofV1",
    "verifyVeRangeProofLocally",
    "buildPrivacyProofEnvelope",
    "decodePrivacyProofEnvelope",
]


def _require_mapping(value: Any, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be a mapping")
    return value


def _require_plain_mapping(value: Any, context: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise TypeError(f"{context} must be a plain dict")
    return value


def _reject_unknown_fields(
    source: Mapping[str, Any],
    allowed: set[str],
    context: str,
) -> None:
    unknown = sorted(
        str(key) for key in source if type(key) is not str or key not in allowed
    )
    if unknown:
        raise TypeError(f"{context} contains unsupported field `{unknown[0]}`")


def _read_single_alias(
    source: Mapping[str, Any],
    aliases: Sequence[str],
    context: str,
    description: str,
) -> tuple[str | None, Any]:
    present = [key for key in aliases if key in source]
    if len(present) > 1:
        raise TypeError(
            f"{context} must not include multiple {description} aliases: "
            + ", ".join(present)
        )
    if not present:
        return None, _MISSING
    return present[0], source[present[0]]


def _require_non_blank_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    text = value.strip()
    if not text:
        raise ValueError(f"{context} must be non-empty")
    return text


def _positive_u32(value: Any, context: str) -> int:
    if isinstance(value, bool):
        raise TypeError(f"{context} must be a positive integer")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str) and value.strip().isdigit():
        parsed = int(value.strip(), 10)
    else:
        raise TypeError(f"{context} must be a positive integer")
    if parsed <= 0 or parsed > 0xFFFF_FFFF:
        raise ValueError(f"{context} must be between 1 and 4294967295")
    return parsed


def _open_verify_positive_u32(value: Any, context: str) -> int:
    if isinstance(value, str):
        if not value or not value.isdigit() or value.startswith("0"):
            raise TypeError(f"{context} must be a positive integer")
        parsed = int(value, 10)
        if parsed > 0xFFFF_FFFF:
            raise ValueError(f"{context} must be between 1 and 4294967295")
        return parsed
    return _positive_u32(value, context)


def _decode_string_bytes(value: str, context: str) -> bytes:
    text = value.strip()
    if not text:
        return b""
    hex_text = text[2:] if text.lower().startswith("0x") else text
    if len(hex_text) % 2 == 0 and all(
        char in "0123456789abcdefABCDEF" for char in hex_text
    ):
        return bytes.fromhex(hex_text)
    compact = "".join(text.split())
    if compact and len(compact) % 4 == 0:
        try:
            return base64.b64decode(compact, validate=True)
        except binascii.Error:
            pass
    return text.encode("utf-8")


def _decode_open_verify_base64(value: str, context: str) -> bytes:
    if not value:
        raise ValueError(f"{context} must be a non-empty base64 string")
    if value.strip() != value or any(char.isspace() for char in value):
        raise ValueError(f"{context} must be a clean base64 string without whitespace")

    alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+/"
    padded = value
    padding_index = value.find("=")
    if padding_index != -1:
        head = value[:padding_index]
        padding = value[padding_index:]
        if not head or any(char not in alphabet for char in head) or padding not in ("=", "=="):
            raise ValueError(f"{context} must be a valid base64 string")
        if len(value) % 4 != 0:
            raise ValueError(f"{context} must be a valid base64 string")
    else:
        if any(char not in alphabet for char in value) or len(value) % 4 == 1:
            raise ValueError(f"{context} must be a valid base64 string")
        padded = value + ("=" * ((4 - (len(value) % 4)) % 4))

    try:
        decoded = base64.b64decode(padded, validate=True)
    except binascii.Error as exc:
        raise ValueError(f"{context} must be a valid base64 string") from exc
    if base64.b64encode(decoded).decode("ascii") != padded:
        raise ValueError(f"{context} must be a valid base64 string")
    return decoded


def _open_verify_memoryview_bytes(value: memoryview, context: str) -> bytes:
    if value.itemsize != 1 or value.format != "B":
        raise TypeError(f"{context} must be an unsigned byte memoryview")
    try:
        return bytes(value)
    except (TypeError, ValueError) as exc:
        raise TypeError(f"{context} must be a contiguous byte memoryview") from exc


def _bytes_value(value: Any, context: str, *, allow_empty: bool = False) -> bytes:
    if value is _MISSING or value is None:
        if allow_empty:
            return b""
        raise TypeError(f"{context} is required")
    if isinstance(value, str):
        data = _decode_string_bytes(value, context)
    elif isinstance(value, (bytes, bytearray)):
        data = bytes(value)
    elif isinstance(value, memoryview):
        data = _open_verify_memoryview_bytes(value, context)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        buffer = bytearray()
        for index, byte in enumerate(value):
            if isinstance(byte, bool) or not isinstance(byte, int) or byte < 0 or byte > 0xFF:
                raise TypeError(f"{context}[{index}] must be an integer between 0 and 255")
            buffer.append(byte)
        data = bytes(buffer)
    else:
        raise TypeError(f"{context} must be bytes-like")
    if not data and not allow_empty:
        raise ValueError(f"{context} must be non-empty")
    return data


def _open_verify_bytes(value: Any, context: str, *, allow_empty: bool = False) -> bytes:
    if isinstance(value, str):
        data = _decode_open_verify_base64(value, context)
    elif isinstance(value, (bytes, bytearray)):
        data = bytes(value)
    elif isinstance(value, memoryview):
        data = _open_verify_memoryview_bytes(value, context)
    elif isinstance(value, Sequence):
        buffer = bytearray()
        for index, byte in enumerate(value):
            if isinstance(byte, bool) or not isinstance(byte, int) or byte < 0 or byte > 0xFF:
                raise TypeError(f"{context}[{index}] must be an integer between 0 and 255")
            buffer.append(byte)
        data = bytes(buffer)
    else:
        data = _bytes_value(value, context, allow_empty=allow_empty)
    if not data and not allow_empty:
        raise ValueError(f"{context} must be non-empty")
    return data


def _open_verify_bounded_bytes(
    value: Any,
    context: str,
    *,
    max_bytes: int,
    allow_empty: bool = False,
) -> bytes:
    data = _open_verify_bytes(value, context, allow_empty=allow_empty)
    if len(data) > max_bytes:
        raise ValueError(f"{context} must be no larger than {max_bytes} bytes")
    return data


def _open_verify_fixed_bytes(
    value: Any,
    context: str,
    length: int,
    *,
    nonzero: bool = False,
) -> bytes:
    if isinstance(value, str):
        if value.strip() != value or any(char.isspace() for char in value):
            raise ValueError(f"{context} must be clean and without whitespace")
        if len(value) == length * 2 and all(
            char in "0123456789abcdefABCDEF" for char in value
        ):
            data = bytes.fromhex(value)
        else:
            data = _decode_open_verify_base64(value, context)
    else:
        data = _open_verify_bytes(value, context)
    if len(data) != length:
        raise ValueError(f"{context} must contain exactly {length} bytes")
    if nonzero and all(byte == 0 for byte in data):
        raise ValueError(f"{context} must be nonzero")
    return data


def _optional_aux_value(source: Mapping[str, Any], context: str) -> Any:
    if "aux" not in source:
        return b""
    value = source["aux"]
    if value is None:
        raise TypeError(f"{context}.aux must be bytes-like when present")
    return value


def _bounded_bytes(
    value: Any,
    context: str,
    *,
    max_bytes: int,
    allow_empty: bool = False,
) -> bytes:
    data = _bytes_value(value, context, allow_empty=allow_empty)
    if len(data) > max_bytes:
        raise ValueError(f"{context} must be no larger than {max_bytes} bytes")
    return data


def _fixed_bytes(
    value: Any,
    context: str,
    length: int,
    *,
    nonzero: bool = False,
) -> bytes:
    data = _bytes_value(value, context)
    if len(data) != length:
        raise ValueError(f"{context} must contain exactly {length} bytes")
    if nonzero and all(byte == 0 for byte in data):
        raise ValueError(f"{context} must be nonzero")
    return data


def _canonical_json_bytes(value: Any, context: str) -> bytes:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise TypeError(f"{context} must be JSON serializable") from exc


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_bit_length(value: Any, context: str) -> int:
    bit_length = _positive_u32(value, context)
    if bit_length > VERANGE_MAX_BIT_LENGTH:
        raise ValueError(f"{context} must be between 1 and {VERANGE_MAX_BIT_LENGTH}")
    return bit_length


def _normalize_aggregation_count(value: Any, context: str) -> int:
    count = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if count > VERANGE_MAX_AGGREGATION_COUNT:
        raise ValueError(
            f"{context} must be between 1 and {VERANGE_MAX_AGGREGATION_COUNT}"
        )
    return count


def _normalize_commitment_scheme(value: Any, context: str) -> str:
    scheme = _require_non_blank_string(
        "pedersen-v1" if value is _MISSING or value is None else value,
        context,
    ).lower()
    if scheme not in VERANGE_COMMITMENT_SCHEMES:
        allowed = ", ".join(sorted(VERANGE_COMMITMENT_SCHEMES))
        raise ValueError(f"{context} must be one of {allowed}")
    return scheme


def _normalize_payload_digest(source: Mapping[str, Any], context: str) -> bytes:
    digest_key, digest_value = _read_single_alias(
        source,
        ("payloadDigest", "payload_digest", "txDigest", "tx_digest"),
        f"{context}.payloadDigest",
        "payload digest",
    )
    payload_key, payload_value = _read_single_alias(
        source,
        ("payload", "payloadBytes", "payload_bytes", "payloadJson", "payload_json"),
        f"{context}.payload",
        "payload",
    )
    max_payload_key, max_payload_value = _read_single_alias(
        source,
        ("maxPayloadBytes", "max_payload_bytes"),
        f"{context}.maxPayloadBytes",
        "max payload byte limit",
    )
    if max_payload_key is not None and payload_key is None:
        raise TypeError(f"{context}.maxPayloadBytes requires {context}.payload")
    explicit_digest = (
        None
        if digest_key is None
        else _fixed_bytes(digest_value, f"{context}.payloadDigest", 32, nonzero=True)
    )
    if payload_key is None:
        payload_digest = None
    elif payload_key in {"payloadJson", "payload_json"}:
        payload_bytes = _canonical_json_bytes(payload_value, f"{context}.payloadJson")
        payload_digest = hashlib.sha256(payload_bytes).digest()
    else:
        max_payload_bytes = _positive_u32(
            (
                VERANGE_MAX_PAYLOAD_BYTES
                if max_payload_key is None
                else max_payload_value
            ),
            f"{context}.maxPayloadBytes",
        )
        payload_bytes = _bounded_bytes(
            payload_value,
            f"{context}.payload",
            max_bytes=max_payload_bytes,
        )
        payload_digest = hashlib.sha256(payload_bytes).digest()
    if explicit_digest is None and payload_digest is None:
        raise ValueError(f"{context}.payloadDigest or {context}.payload is required")
    if explicit_digest is not None and payload_digest is not None:
        if explicit_digest != payload_digest:
            raise ValueError(
                f"{context}.payloadDigest must match the SHA-256 digest of {context}.payload"
            )
    return explicit_digest if explicit_digest is not None else payload_digest  # type: ignore[return-value]


def _normalize_backend(
    value: Any,
    context: str,
    *,
    allow_unsupported: bool = False,
) -> tuple[int, str]:
    raw_value = VERANGE_BACKEND if value is _MISSING or value is None else value
    text = _require_non_blank_string(
        raw_value,
        context,
    )
    if text != raw_value:
        raise ValueError(f"{context} uses unsupported backend tag {raw_value}")
    if any(not char.isascii() for char in text):
        raise ValueError(f"{context} uses unsupported backend tag {text}")
    normalized = "".join(
        char for char in text.lower() if char.isascii() and char.isalnum()
    )
    if normalized not in _BACKEND_TAGS:
        raise ValueError(f"{context} uses unsupported backend tag {text}")
    tag, decoded = _BACKEND_TAGS[normalized]
    if decoded == "Unsupported" and not allow_unsupported:
        raise ValueError(f"{context} uses unsupported backend tag {text}")
    return tag, decoded


def _normalize_backend_allowing_unsupported(
    value: Any,
    context: str,
) -> tuple[int, str]:
    return _normalize_backend(value, context, allow_unsupported=True)


def _normalize_verange_backend(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(value, context)
    if decoded != "Stark":
        raise ValueError(f"{context} must be {VERANGE_BACKEND}")
    return VERANGE_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        VERANGE_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {VERANGE_CIRCUIT_ID, "verange_transparent_range_v1"}:
        raise ValueError(f"{context} must identify verange_transparent_range_v1")
    return circuit_id


def build_range_commitment(
    options: Mapping[str, Any],
    context: str = "rangeCommitment",
) -> dict[str, Any]:
    """Normalize a prepared VeRange commitment descriptor.

    This helper does not create a cryptographic commitment; callers must provide
    a nonzero 32-byte commitment produced by their wallet/prover.
    """

    source = _require_plain_mapping(options, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        context,
    )
    _commitment_key, commitment_value = _read_single_alias(
        source,
        (
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
        ),
        f"{context}.commitment",
        "commitment",
    )
    _bit_key, bit_value = _read_single_alias(
        source,
        ("bitLength", "bit_length"),
        f"{context}.bitLength",
        "bit length",
    )
    _aggregation_key, aggregation_value = _read_single_alias(
        source,
        ("aggregationCount", "aggregation_count"),
        f"{context}.aggregationCount",
        "aggregation count",
    )
    _scheme_key, scheme_value = _read_single_alias(
        source,
        ("commitmentScheme", "commitment_scheme"),
        f"{context}.commitmentScheme",
        "commitment scheme",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "commitment": _fixed_bytes(
            commitment_value,
            f"{context}.commitment",
            32,
            nonzero=True,
        ),
        "bit_length": _normalize_bit_length(bit_value, f"{context}.bitLength"),
        "aggregation_count": _normalize_aggregation_count(
            aggregation_value,
            f"{context}.aggregationCount",
        ),
        "commitment_scheme": _normalize_commitment_scheme(
            scheme_value,
            f"{context}.commitmentScheme",
        ),
        "domain_separator": _require_non_blank_string(
            VERANGE_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
            f"{context}.domainSeparator",
        ),
        "payload_digest": _normalize_payload_digest(source, context),
    }


def _normalize_commitments(source: Mapping[str, Any], context: str) -> list[dict[str, Any]]:
    list_key, list_value = _read_single_alias(
        source,
        ("commitments", "rangeCommitments", "range_commitments"),
        f"{context}.commitments",
        "range commitment list",
    )
    single_key, _single_value = _read_single_alias(
        source,
        (
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
        ),
        f"{context}.commitment",
        "range commitment",
    )
    if list_key is not None and single_key is not None:
        raise TypeError(f"{context} must include either commitments or commitment, not both")
    common = {
        key: source[key]
        for key in (
            "version",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "maxPayloadBytes",
            "max_payload_bytes",
        )
        if key in source
    }
    if list_key is None:
        entries = [
            {
                **common,
                **{
                    key: source[key]
                    for key in source
                    if key
                    in {
                        "commitment",
                        "rangeCommitment",
                        "range_commitment",
                        "valueCommitment",
                        "value_commitment",
                    }
                },
            }
        ]
    else:
        if not isinstance(list_value, Sequence) or isinstance(
            list_value,
            (str, bytes, bytearray, memoryview),
        ):
            raise TypeError(f"{context}.commitments must be a non-empty sequence")
        if not list_value:
            raise ValueError(f"{context}.commitments must be non-empty")
        entries = []
        for index, entry in enumerate(list_value):
            if isinstance(entry, Mapping):
                if type(entry) is not dict:
                    raise TypeError(f"{context}.commitments[{index}] must be a plain dict")
                entries.append({**common, **entry})
            else:
                entries.append({**common, "commitment": entry})
    return [
        build_range_commitment(entry, f"{context}.commitments[{index}]")
        for index, entry in enumerate(entries)
    ]


def _ensure_commitment_consistency(
    commitments: list[dict[str, Any]],
    context: str,
) -> None:
    first = commitments[0]
    seen: set[bytes] = set()
    for index, commitment in enumerate(commitments):
        prefix = f"{context}.commitments[{index}]"
        if commitment["bit_length"] != first["bit_length"]:
            raise ValueError(f"{prefix}.bitLength must match the first commitment")
        if commitment["commitment_scheme"] != first["commitment_scheme"]:
            raise ValueError(f"{prefix}.commitmentScheme must match the first commitment")
        if commitment["domain_separator"] != first["domain_separator"]:
            raise ValueError(f"{prefix}.domainSeparator must match the first commitment")
        if commitment["payload_digest"] != first["payload_digest"]:
            raise ValueError(f"{prefix}.payloadDigest must match the first commitment")
        if commitment["commitment"] in seen:
            raise ValueError(f"{context}.commitments must not contain duplicate commitments")
        seen.add(commitment["commitment"])


def _normalize_proof_parts(
    source: Mapping[str, Any],
    context: str,
    *,
    require_proof_bytes: bool,
) -> dict[str, Any]:
    _backend_key, backend_value = _read_single_alias(
        source,
        ("backendTag", "backend_tag", "backend"),
        f"{context}.backendTag",
        "backend tag",
    )
    _circuit_key, circuit_value = _read_single_alias(
        source,
        ("circuitId", "circuit_id"),
        f"{context}.circuitId",
        "circuit id",
    )
    _vk_key, vk_hash_value = _read_single_alias(
        source,
        ("vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"),
        f"{context}.vkHash",
        "verifying key hash",
    )
    _proof_key, proof_value = _read_single_alias(
        source,
        ("proofBytes", "proof_bytes", "proof"),
        f"{context}.proofBytes",
        "proof bytes",
    )
    if require_proof_bytes and proof_value is _MISSING:
        raise TypeError(f"{context}.proofBytes is required")
    _aggregation_key, aggregation_value = _read_single_alias(
        source,
        ("aggregationCount", "aggregation_count"),
        f"{context}.aggregationCount",
        "aggregation count",
    )
    commitments = _normalize_commitments(source, context)
    _ensure_commitment_consistency(commitments, context)
    aggregation_count = (
        len(commitments)
        if aggregation_value is _MISSING
        else _normalize_aggregation_count(aggregation_value, f"{context}.aggregationCount")
    )
    if aggregation_count != len(commitments):
        raise ValueError(f"{context}.aggregationCount must equal the number of commitments")
    for index, commitment in enumerate(commitments):
        if commitment["aggregation_count"] not in {1, aggregation_count}:
            raise ValueError(
                f"{context}.commitments[{index}].aggregationCount must be 1 or match {context}.aggregationCount"
            )
    first = commitments[0]
    public_inputs = {
        "version": 1,
        "commitments": [commitment["commitment"].hex() for commitment in commitments],
        "range_parameters": {
            "bit_length": first["bit_length"],
            "commitment_scheme": first["commitment_scheme"],
        },
        "aggregation_count": aggregation_count,
        "domain_separator": first["domain_separator"],
        "payload_digest": first["payload_digest"].hex(),
    }
    public_input_bytes = _canonical_json_bytes(public_inputs, f"{context}.publicInputs")
    _max_proof_key, max_proof_value = _read_single_alias(
        source,
        ("maxProofBytes", "max_proof_bytes"),
        f"{context}.maxProofBytes",
        "max proof byte limit",
    )
    _max_public_input_key, max_public_input_value = _read_single_alias(
        source,
        ("maxPublicInputBytes", "max_public_input_bytes"),
        f"{context}.maxPublicInputBytes",
        "max public input byte limit",
    )
    max_proof_bytes = _positive_u32(
        (
            DEFAULT_PRIVACY_MAX_PROOF_BYTES
            if max_proof_value is _MISSING
            else max_proof_value
        ),
        f"{context}.maxProofBytes",
    )
    max_public_input_bytes = _positive_u32(
        (
            DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES
            if max_public_input_value is _MISSING
            else max_public_input_value
        ),
        f"{context}.maxPublicInputBytes",
    )
    return {
        "backend": _normalize_verange_backend(backend_value, f"{context}.backendTag"),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "public_inputs": public_inputs,
        "public_input_bytes": public_input_bytes,
        "proof_bytes": (
            None
            if proof_value is _MISSING
            else _bounded_bytes(
                proof_value,
                f"{context}.proofBytes",
                max_bytes=max_proof_bytes,
            )
        ),
        "max_proof_bytes": max_proof_bytes,
        "max_public_input_bytes": max_public_input_bytes,
    }


def _encode_field(payload: bytes) -> bytes:
    return len(payload).to_bytes(8, "little") + payload


def _read_field(payload: bytes, offset: int, context: str) -> tuple[bytes, int]:
    if offset + 8 > len(payload):
        raise ValueError(f"{context} is truncated")
    length = int.from_bytes(payload[offset : offset + 8], "little")
    offset += 8
    end = offset + length
    if end > len(payload):
        raise ValueError(f"{context} length exceeds available bytes")
    return payload[offset:end], end


def _decode_open_verify_circuit_id(value: bytes, context: str) -> str:
    if not value:
        raise ValueError(f"{context} must be non-empty")
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ValueError(f"{context} must contain valid UTF-8") from exc
    if text.strip() != text or not text.strip():
        raise ValueError(f"{context} must be clean and non-empty")
    return text


def _check_decoded_open_verify_field_size(field: str, data: bytes) -> None:
    limits = {
        "public_inputs": DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
        "proof_bytes": DEFAULT_PRIVACY_MAX_PROOF_BYTES,
        "aux": DEFAULT_PRIVACY_MAX_AUX_BYTES,
    }
    max_bytes = limits[field]
    if len(data) > max_bytes:
        raise ValueError(
            f"privacyProofEnvelope.{field} must be no larger than {max_bytes} bytes"
        )


def _encode_open_verify_payload(
    envelope: Mapping[str, Any],
    *,
    allow_unsupported_backend: bool = False,
) -> bytes:
    tag, _decoded = _normalize_backend(
        envelope["backend"],
        "OpenVerifyEnvelope.backend",
        allow_unsupported=allow_unsupported_backend,
    )
    circuit_id = _require_non_blank_string(
        envelope["circuit_id"],
        "OpenVerifyEnvelope.circuit_id",
    ).encode("utf-8")
    return b"".join(
        [
            _encode_field(tag.to_bytes(4, "little")),
            _encode_field(_encode_field(circuit_id)),
            _encode_field(_fixed_bytes(envelope["vk_hash"], "OpenVerifyEnvelope.vk_hash", 32)),
            _encode_field(
                _encode_field(
                    _bytes_value(envelope["public_inputs"], "OpenVerifyEnvelope.public_inputs")
                )
            ),
            _encode_field(
                _encode_field(_bytes_value(envelope["proof_bytes"], "OpenVerifyEnvelope.proof_bytes"))
            ),
            _encode_field(
                _encode_field(
                    _bytes_value(
                        envelope.get("aux", b""),
                        "OpenVerifyEnvelope.aux",
                        allow_empty=True,
                    )
                )
            ),
        ]
    )


def _frame_open_verify_payload(payload: bytes) -> bytes:
    return b"".join(
        [
            b"NRT0",
            b"\x00\x00",
            _OPEN_VERIFY_SCHEMA_HASH,
            b"\x00",
            len(payload).to_bytes(8, "little"),
            crc64(payload).to_bytes(8, "little"),
            b"\x00",
            payload,
        ]
    )


def build_privacy_proof_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical Norito bytes for an OpenVerify proof envelope."""

    return _build_privacy_proof_envelope_internal(options)


def _build_privacy_proof_envelope_internal(
    options: Mapping[str, Any],
    *,
    allow_unsupported_backend: bool = False,
) -> bytes:
    source = _require_plain_mapping(options, "privacyProofEnvelope")
    _reject_unknown_fields(
        source,
        {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "publicInputs",
            "public_inputs",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
        },
        "privacyProofEnvelope",
    )
    _backend_key, backend_value = _read_single_alias(
        source,
        ("backendTag", "backend_tag", "backend"),
        "privacyProofEnvelope.backendTag",
        "backend tag",
    )
    _circuit_key, circuit_value = _read_single_alias(
        source,
        ("circuitId", "circuit_id"),
        "privacyProofEnvelope.circuitId",
        "circuit id",
    )
    _vk_key, vk_hash_value = _read_single_alias(
        source,
        ("vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"),
        "privacyProofEnvelope.vkHash",
        "verifying key hash",
    )
    _public_key, public_inputs_value = _read_single_alias(
        source,
        ("publicInputs", "public_inputs"),
        "privacyProofEnvelope.publicInputs",
        "public inputs",
    )
    _proof_key, proof_value = _read_single_alias(
        source,
        ("proofBytes", "proof_bytes", "proof"),
        "privacyProofEnvelope.proofBytes",
        "proof bytes",
    )
    _max_proof_key, max_proof_value = _read_single_alias(
        source,
        ("maxProofBytes", "max_proof_bytes"),
        "privacyProofEnvelope.maxProofBytes",
        "max proof byte limit",
    )
    _max_public_input_key, max_public_input_value = _read_single_alias(
        source,
        ("maxPublicInputBytes", "max_public_input_bytes"),
        "privacyProofEnvelope.maxPublicInputBytes",
        "max public input byte limit",
    )
    max_proof_bytes = _open_verify_positive_u32(
        DEFAULT_PRIVACY_MAX_PROOF_BYTES if max_proof_value is _MISSING else max_proof_value,
        "privacyProofEnvelope.maxProofBytes",
    )
    max_public_input_bytes = _open_verify_positive_u32(
        (
            DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES
            if max_public_input_value is _MISSING
            else max_public_input_value
        ),
        "privacyProofEnvelope.maxPublicInputBytes",
    )
    if backend_value is _MISSING or backend_value is None:
        raise TypeError("privacyProofEnvelope.backendTag is required")
    backend_label = backend_value
    circuit_id = _require_non_blank_string(
        circuit_value,
        "privacyProofEnvelope.circuitId",
    )
    if circuit_id != circuit_value:
        raise ValueError("privacyProofEnvelope.circuitId must be clean and already trimmed")
    envelope = {
        "backend": _normalize_backend(
            backend_label,
            "privacyProofEnvelope.backendTag",
            allow_unsupported=allow_unsupported_backend,
        )[1],
        "circuit_id": circuit_id,
        "vk_hash": _open_verify_fixed_bytes(
            vk_hash_value,
            "privacyProofEnvelope.vkHash",
            32,
            nonzero=True,
        ),
        "public_inputs": _open_verify_bounded_bytes(
            public_inputs_value,
            "privacyProofEnvelope.publicInputs",
            max_bytes=max_public_input_bytes,
        ),
        "proof_bytes": _open_verify_bounded_bytes(
            proof_value,
            "privacyProofEnvelope.proofBytes",
            max_bytes=max_proof_bytes,
        ),
        "aux": _open_verify_bounded_bytes(
            _optional_aux_value(source, "privacyProofEnvelope"),
            "privacyProofEnvelope.aux",
            max_bytes=DEFAULT_PRIVACY_MAX_AUX_BYTES,
            allow_empty=True,
        ),
    }
    return _frame_open_verify_payload(
        _encode_open_verify_payload(
            envelope,
            allow_unsupported_backend=allow_unsupported_backend,
        )
    )


def decode_privacy_proof_envelope(value: Any) -> dict[str, Any]:
    """Decode standalone Norito bytes for an OpenVerify proof envelope."""

    return _decode_privacy_proof_envelope_internal(value)


def _decode_privacy_proof_envelope_internal(
    value: Any,
    *,
    allow_unsupported_backend: bool = False,
) -> dict[str, Any]:
    data = _bytes_value(value, "privacyProofEnvelope")
    if len(data) < 40 or data[:4] != b"NRT0":
        raise ValueError("privacyProofEnvelope is not an NRT0 frame")
    if data[4] != 0 or data[5] != 0:
        raise ValueError("privacyProofEnvelope uses unsupported NRT0 version")
    if data[6:22] != _OPEN_VERIFY_SCHEMA_HASH:
        raise ValueError("privacyProofEnvelope schema hash did not match OpenVerifyEnvelope")
    if data[22] != 0:
        raise ValueError("privacyProofEnvelope uses unsupported compression")
    payload_length = int.from_bytes(data[23:31], "little")
    expected_crc = int.from_bytes(data[31:39], "little")
    flags = data[39]
    if flags != 0:
        raise ValueError("privacyProofEnvelope uses unsupported layout flags")
    payload = data[40:]
    if len(payload) != payload_length:
        raise ValueError("privacyProofEnvelope payload length mismatch")
    if crc64(payload) != expected_crc:
        raise ValueError("privacyProofEnvelope CRC64 mismatch")
    fields: dict[str, bytes] = {}
    offset = 0
    for name in ("backend", "circuit_id", "vk_hash", "public_inputs", "proof_bytes", "aux"):
        fields[name], offset = _read_field(payload, offset, f"privacyProofEnvelope.{name}")
    if offset != len(payload):
        raise ValueError("privacyProofEnvelope has trailing payload bytes")
    if len(fields["backend"]) != 4:
        raise ValueError("privacyProofEnvelope.backend must contain a u32 tag")
    backend_tag = int.from_bytes(fields["backend"], "little")
    if backend_tag not in _BACKEND_NAMES_BY_TAG:
        raise ValueError("privacyProofEnvelope.backend uses unsupported tag")
    backend_name = _BACKEND_NAMES_BY_TAG[backend_tag]
    if backend_name == "Unsupported" and not allow_unsupported_backend:
        raise ValueError("privacyProofEnvelope.backend uses unsupported tag")
    if len(fields["vk_hash"]) != 32:
        raise ValueError("privacyProofEnvelope.vk_hash must contain exactly 32 bytes")
    if all(byte == 0 for byte in fields["vk_hash"]):
        raise ValueError("privacyProofEnvelope.vk_hash must be nonzero")
    circuit_id, end = _read_field(fields["circuit_id"], 0, "privacyProofEnvelope.circuit_id")
    if end != len(fields["circuit_id"]):
        raise ValueError("privacyProofEnvelope.circuit_id has trailing bytes")
    for field in ("public_inputs", "proof_bytes", "aux"):
        inner, end = _read_field(fields[field], 0, f"privacyProofEnvelope.{field}")
        if end != len(fields[field]):
            raise ValueError(f"privacyProofEnvelope.{field} has trailing bytes")
        if field != "aux" and not inner:
            raise ValueError(f"privacyProofEnvelope.{field} must be non-empty")
        _check_decoded_open_verify_field_size(field, inner)
        fields[field] = inner
    return {
        "backend": backend_name,
        "circuit_id": _decode_open_verify_circuit_id(
            circuit_id,
            "privacyProofEnvelope.circuit_id",
        ),
        "vk_hash": fields["vk_hash"],
        "public_inputs": fields["public_inputs"],
        "proof_bytes": fields["proof_bytes"],
        "aux": fields["aux"],
    }


def build_verange_proof_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical Norito bytes for an externally generated VeRange proof."""

    source = _require_plain_mapping(options, "veRangeProofEnvelope")
    _reject_unknown_fields(
        source,
        {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "maxPayloadBytes",
            "max_payload_bytes",
            "version",
        },
        "veRangeProofEnvelope",
    )
    parts = _normalize_proof_parts(source, "veRangeProofEnvelope", require_proof_bytes=True)
    aux = _open_verify_bounded_bytes(
        _optional_aux_value(source, "veRangeProofEnvelope"),
        "veRangeProofEnvelope.aux",
        max_bytes=DEFAULT_PRIVACY_MAX_AUX_BYTES,
        allow_empty=True,
    )
    return build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": aux,
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )


def build_verange_proof_v1(options: Mapping[str, Any]) -> bytes:
    """Build canonical production VeRange proof envelope bytes."""

    source = _require_plain_mapping(options, "veRangeProofV1")
    _reject_unknown_fields(
        source,
        {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "maxPayloadBytes",
            "max_payload_bytes",
            "version",
        },
        "veRangeProofV1",
    )
    parts = _normalize_proof_parts(source, "veRangeProofV1", require_proof_bytes=True)
    if parts["proof_bytes"].startswith(VERANGE_DEV_PROOF_PREFIX):
        raise ValueError("veRangeProofV1.proofBytes must not contain a dev fixture proof")
    aux = _open_verify_bounded_bytes(
        _optional_aux_value(source, "veRangeProofV1"),
        "veRangeProofV1.aux",
        max_bytes=DEFAULT_PRIVACY_MAX_AUX_BYTES,
        allow_empty=True,
    )
    return build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": aux,
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )


def _build_verange_proof_v1(options: Mapping[str, Any]) -> bytes:
    return build_verange_proof_v1(options)


def _dev_proof_bytes(*, circuit_id: str, vk_hash: bytes, public_input_bytes: bytes) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:verange:dev-fixture:v1")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return VERANGE_DEV_PROOF_PREFIX + digest.digest()


def build_verange_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic VeRange dev fixture bound to an OpenVerify envelope."""

    source = _require_plain_mapping(options, "veRangeDevProofFixture")
    _reject_unknown_fields(
        source,
        {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "maxPayloadBytes",
            "max_payload_bytes",
            "version",
        },
        "veRangeDevProofFixture",
    )
    parts = _normalize_proof_parts(source, "veRangeDevProofFixture", require_proof_bytes=False)
    proof_bytes = _dev_proof_bytes(
        circuit_id=parts["circuit_id"],
        vk_hash=parts["vk_hash"],
        public_input_bytes=parts["public_input_bytes"],
    )
    aux = _open_verify_bounded_bytes(
        _optional_aux_value(source, "veRangeDevProofFixture"),
        "veRangeDevProofFixture.aux",
        max_bytes=DEFAULT_PRIVACY_MAX_AUX_BYTES,
        allow_empty=True,
    )
    envelope = build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": proof_bytes,
            "aux": aux,
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )
    return {
        "kind": "verange-dev-fixture-v1",
        "production": False,
        "proof_bytes": proof_bytes,
        "public_inputs": parts["public_inputs"],
        "public_input_bytes": parts["public_input_bytes"],
        "envelope": envelope,
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be a mapping")
    _reject_unknown_fields(
        value,
        {
            "version",
            "commitments",
            "range_parameters",
            "rangeParameters",
            "aggregation_count",
            "aggregationCount",
            "domain_separator",
            "domainSeparator",
            "payload_digest",
            "payloadDigest",
        },
        context,
    )
    _range_key, range_value = _read_single_alias(
        value,
        ("range_parameters", "rangeParameters"),
        f"{context}.rangeParameters",
        "range parameters",
    )
    range_parameters = _require_mapping(range_value, f"{context}.rangeParameters")
    _reject_unknown_fields(
        range_parameters,
        {"bit_length", "bitLength", "commitment_scheme", "commitmentScheme"},
        f"{context}.rangeParameters",
    )
    _bit_key, bit_value = _read_single_alias(
        range_parameters,
        ("bit_length", "bitLength"),
        f"{context}.rangeParameters.bitLength",
        "bit length",
    )
    _scheme_key, scheme_value = _read_single_alias(
        range_parameters,
        ("commitment_scheme", "commitmentScheme"),
        f"{context}.rangeParameters.commitmentScheme",
        "commitment scheme",
    )
    _aggregation_key, aggregation_value = _read_single_alias(
        value,
        ("aggregation_count", "aggregationCount"),
        f"{context}.aggregationCount",
        "aggregation count",
    )
    _domain_key, domain_value = _read_single_alias(
        value,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    _digest_key, digest_value = _read_single_alias(
        value,
        ("payload_digest", "payloadDigest"),
        f"{context}.payloadDigest",
        "payload digest",
    )
    commitments_value = value.get("commitments")
    if not isinstance(commitments_value, Sequence) or isinstance(
        commitments_value,
        (str, bytes, bytearray, memoryview),
    ):
        raise TypeError(f"{context}.commitments must be a non-empty sequence")
    if not commitments_value:
        raise ValueError(f"{context}.commitments must be non-empty")
    commitments = [
        _fixed_bytes(entry, f"{context}.commitments[{index}]", 32, nonzero=True).hex()
        for index, entry in enumerate(commitments_value)
    ]
    if len(set(commitments)) != len(commitments):
        raise ValueError(f"{context}.commitments must not contain duplicate commitments")
    aggregation_count = _normalize_aggregation_count(
        aggregation_value,
        f"{context}.aggregationCount",
    )
    if aggregation_count != len(commitments):
        raise ValueError(f"{context}.aggregationCount must equal the number of commitments")
    return {
        "version": _normalize_version(value.get("version", _MISSING), f"{context}.version"),
        "commitments": commitments,
        "range_parameters": {
            "bit_length": _normalize_bit_length(
                bit_value,
                f"{context}.rangeParameters.bitLength",
            ),
            "commitment_scheme": _normalize_commitment_scheme(
                scheme_value,
                f"{context}.rangeParameters.commitmentScheme",
            ),
        },
        "aggregation_count": aggregation_count,
        "domain_separator": _require_non_blank_string(
            domain_value,
            f"{context}.domainSeparator",
        ),
        "payload_digest": _fixed_bytes(
            digest_value,
            f"{context}.payloadDigest",
            32,
            nonzero=True,
        ).hex(),
    }


def _parse_public_inputs(value: bytes, context: str) -> dict[str, Any]:
    try:
        parsed = json.loads(value.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"{context} must contain valid JSON public inputs") from exc
    normalized = _normalize_public_inputs(parsed, context)
    if value != _canonical_json_bytes(normalized, context):
        raise ValueError(f"{context} must use canonical JSON encoding")
    return normalized


def _ensure_verification_expectations(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
    context: str,
) -> None:
    if any(
        key in source
        for key in (
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
        )
    ):
        expected_digest = _normalize_payload_digest(source, context).hex()
        if expected_digest != public_inputs["payload_digest"]:
            raise ValueError(f"{context}.payloadDigest must match the envelope public inputs")
    _bit_key, bit_value = _read_single_alias(
        source,
        ("bitLength", "bit_length"),
        f"{context}.bitLength",
        "bit length",
    )
    if bit_value is not _MISSING and _normalize_bit_length(bit_value, f"{context}.bitLength") != public_inputs["range_parameters"]["bit_length"]:
        raise ValueError(f"{context}.bitLength must match the envelope public inputs")
    _scheme_key, scheme_value = _read_single_alias(
        source,
        ("commitmentScheme", "commitment_scheme"),
        f"{context}.commitmentScheme",
        "commitment scheme",
    )
    if scheme_value is not _MISSING and _normalize_commitment_scheme(scheme_value, f"{context}.commitmentScheme") != public_inputs["range_parameters"]["commitment_scheme"]:
        raise ValueError(f"{context}.commitmentScheme must match the envelope public inputs")
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    if domain_value is not _MISSING and _require_non_blank_string(domain_value, f"{context}.domainSeparator") != public_inputs["domain_separator"]:
        raise ValueError(f"{context}.domainSeparator must match the envelope public inputs")
    _aggregation_key, aggregation_value = _read_single_alias(
        source,
        ("aggregationCount", "aggregation_count"),
        f"{context}.aggregationCount",
        "aggregation count",
    )
    if aggregation_value is not _MISSING and _normalize_aggregation_count(aggregation_value, f"{context}.aggregationCount") != public_inputs["aggregation_count"]:
        raise ValueError(f"{context}.aggregationCount must match the envelope public inputs")
    if any(
        key in source
        for key in (
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
        )
    ):
        expected_commitments = [
            commitment["commitment"].hex()
            for commitment in _normalize_commitments(source, context)
        ]
        if expected_commitments != list(public_inputs["commitments"]):
            raise ValueError(f"{context}.commitments must match the envelope public inputs")


def verify_verange_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic VeRange dev fixture through an OpenVerify envelope."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "veRangeProofLocalVerification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        "veRangeProofLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "veRangeProofLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("veRangeProofLocalVerification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "veRangeProofLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "veRangeProofLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "veRangeProofLocalVerification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "veRangeProofLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "veRangeProofLocalVerification proof bytes are not a valid VeRange dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "verange-dev-fixture-v1",
        "backend": VERANGE_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
    }


def verify_verange_proof_v1(options: Any) -> dict[str, Any]:
    """Validate a production VeRange proof envelope binding."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "veRangeProofV1Verification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
            "payloadDigest",
            "payload_digest",
            "txDigest",
            "tx_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "commitments",
            "rangeCommitments",
            "range_commitments",
            "commitment",
            "rangeCommitment",
            "range_commitment",
            "valueCommitment",
            "value_commitment",
            "bitLength",
            "bit_length",
            "aggregationCount",
            "aggregation_count",
            "commitmentScheme",
            "commitment_scheme",
            "domainSeparator",
            "domain_separator",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        "veRangeProofV1Verification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "veRangeProofV1Verification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("veRangeProofV1Verification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "veRangeProofV1Verification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "veRangeProofV1Verification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "veRangeProofV1Verification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "veRangeProofV1Verification",
    )
    if decoded["proof_bytes"].startswith(VERANGE_DEV_PROOF_PREFIX):
        raise ValueError(
            "veRangeProofV1Verification proof bytes must not contain a VeRange dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "verange-transparent-range-v1",
        "backend": "Stark",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "aggregation_count": public_inputs["aggregation_count"],
        "bit_length": public_inputs["range_parameters"]["bit_length"],
        "commitment_scheme": public_inputs["range_parameters"]["commitment_scheme"],
    }


def _verify_verange_proof_v1(options: Any) -> dict[str, Any]:
    return verify_verange_proof_v1(options)


buildRangeCommitment = build_range_commitment
buildVeRangeProofEnvelope = build_verange_proof_envelope
buildVeRangeDevProofFixture = build_verange_dev_proof_fixture
buildVeRangeProofV1 = build_verange_proof_v1
verifyVeRangeProofV1 = verify_verange_proof_v1
verifyVeRangeProofLocally = verify_verange_proof_locally
buildPrivacyProofEnvelope = build_privacy_proof_envelope
decodePrivacyProofEnvelope = decode_privacy_proof_envelope
