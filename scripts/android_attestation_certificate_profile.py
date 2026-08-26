"""Strict Android Key Attestation certificate parsing and time profiles."""

from __future__ import annotations

import base64
import datetime as dt
import hashlib
from pathlib import PurePosixPath
import re


ANDROID_KEY_ATTESTATION_EXTENSION_OID = "1.3.6.1.4.1.11129.2.1.17"
ANDROID_KEY_ATTESTATION_EXTENSION_OID_DER = bytes.fromhex(
    "060a2b06010401d679020111"
)
ANDROID_SECURITY_LEVEL_STRONGBOX = 2
ANDROID_VERIFIED_BOOT_STATE_VERIFIED = 0
ANDROID_TAG_ALL_APPLICATIONS = 600
ANDROID_TAG_ROOT_OF_TRUST = 704
ANDROID_TAG_ATTESTATION_APPLICATION_ID = 709
# SHA-256 of the exact legacy Google RSA Android Key Attestation root DER.
# Google factory chains anchored here are the one documented exception whose
# non-target certificates may be expired; not-yet-valid certificates remain
# inadmissible. The ECDSA/RKP root does not receive this exception.
ANDROID_LEGACY_GOOGLE_ATTESTATION_ROOT_SHA256 = (
    "cedb1cb6dc896ae5ec797348bce9286753c2b38ee71ce0fbe34a9a1248800dfc"
)
X509_COMMON_NAME_OID_DER_VALUE = bytes.fromhex("550403")
X509_SERIAL_NUMBER_OID_DER_VALUE = bytes.fromhex("550405")
X509_ORGANIZATION_OID_DER_VALUE = bytes.fromhex("55040a")


class _StrictDerReader:
    """Small strict DER reader for Android certificates and KeyDescription."""

    def __init__(self, payload: bytes):
        self.payload = payload
        self.offset = 0

    def remaining(self) -> bool:
        return self.offset < len(self.payload)

    def read(self) -> tuple[int, bool, int, bytes, bytes]:
        start = self.offset
        if self.offset >= len(self.payload):
            raise ValueError("DER value is truncated")
        first = self.payload[self.offset]
        self.offset += 1
        tag_class = first >> 6
        constructed = bool(first & 0x20)
        tag = first & 0x1F
        if tag == 0x1F:
            tag = 0
            first_tag_octet = True
            while True:
                if self.offset >= len(self.payload):
                    raise ValueError("DER high tag number is truncated")
                octet = self.payload[self.offset]
                self.offset += 1
                if first_tag_octet and octet == 0x80:
                    raise ValueError("DER high tag number is non-minimal")
                first_tag_octet = False
                if tag > (1 << 31):
                    raise ValueError("DER tag number is too large")
                tag = (tag << 7) | (octet & 0x7F)
                if not octet & 0x80:
                    break
            if tag < 31:
                raise ValueError("DER high tag number is non-minimal")
        if self.offset >= len(self.payload):
            raise ValueError("DER length is truncated")
        first_length = self.payload[self.offset]
        self.offset += 1
        if first_length < 0x80:
            length = first_length
        else:
            octets = first_length & 0x7F
            if octets == 0 or octets > 4 or self.offset + octets > len(self.payload):
                raise ValueError("DER length is invalid")
            encoded = self.payload[self.offset : self.offset + octets]
            self.offset += octets
            if encoded[0] == 0:
                raise ValueError("DER length is non-minimal")
            length = int.from_bytes(encoded, "big")
            if length < 0x80:
                raise ValueError("DER length is non-minimal")
        end = self.offset + length
        if end > len(self.payload):
            raise ValueError("DER value is truncated")
        value = self.payload[self.offset : end]
        self.offset = end
        return tag_class, constructed, tag, value, self.payload[start:end]

    def expect(
        self,
        tag_class: int,
        constructed: bool,
        tag: int,
        label: str,
    ) -> bytes:
        actual_class, actual_constructed, actual_tag, value, _ = self.read()
        if (actual_class, actual_constructed, actual_tag) != (
            tag_class,
            constructed,
            tag,
        ):
            raise ValueError(f"{label} has an unexpected DER tag")
        return value

    def finish(self, label: str) -> None:
        if self.remaining():
            raise ValueError(f"{label} contains trailing DER data")


def _der_unsigned_integer(value: bytes, label: str) -> int:
    if not value or value[0] & 0x80:
        raise ValueError(f"{label} must be a non-negative DER integer")
    if len(value) > 1 and value[0] == 0 and not value[1] & 0x80:
        raise ValueError(f"{label} DER integer is non-minimal")
    return int.from_bytes(value, "big")


def _der_boolean(value: bytes, label: str) -> bool:
    if value not in (b"\x00", b"\xff"):
        raise ValueError(f"{label} must be one canonical DER boolean")
    return value == b"\xff"


def _x509_algorithm_identifier(
    encoded: bytes,
) -> tuple[bytes, tuple[int, bool, int, bytes, bytes] | None]:
    wrapper = _StrictDerReader(encoded)
    sequence = wrapper.expect(0, True, 16, "X.509 signature algorithm")
    wrapper.finish("X.509 signature algorithm")
    reader = _StrictDerReader(sequence)
    oid = reader.expect(0, False, 6, "X.509 signature algorithm OID")
    if not oid:
        raise ValueError("X.509 signature algorithm OID must not be empty")
    parameters = reader.read() if reader.remaining() else None
    reader.finish("X.509 signature algorithm")
    return oid, parameters


def _x509_algorithm_parameters_are_absent_or_null(
    parameters: tuple[int, bool, int, bytes, bytes] | None,
) -> bool:
    return parameters is None or parameters == (0, False, 5, b"", b"\x05\x00")


def _x509_hash_algorithm(encoded: bytes) -> tuple[bytes, int]:
    oid, parameters = _x509_algorithm_identifier(encoded)
    if not _x509_algorithm_parameters_are_absent_or_null(parameters):
        raise ValueError("X.509 RSA-PSS hash parameters are invalid")
    digest_bytes = {
        bytes.fromhex("608648016503040201"): 32,
        bytes.fromhex("608648016503040202"): 48,
        bytes.fromhex("608648016503040203"): 64,
    }.get(oid)
    if digest_bytes is None:
        raise ValueError("X.509 RSA-PSS hash algorithm is not approved")
    return oid, digest_bytes


def _x509_single_sequence(encoded: bytes, label: str) -> bytes:
    reader = _StrictDerReader(encoded)
    tag_class, constructed, tag, _, raw = reader.read()
    reader.finish(label)
    if (tag_class, constructed, tag) != (0, True, 16):
        raise ValueError(f"{label} must be one DER sequence")
    return raw


def _validate_x509_rsa_pss_parameters(encoded: bytes) -> None:
    wrapper = _StrictDerReader(encoded)
    sequence = wrapper.expect(0, True, 16, "X.509 RSA-PSS parameters")
    wrapper.finish("X.509 RSA-PSS parameters")
    parameters = _StrictDerReader(sequence)
    hash_field = parameters.expect(2, True, 0, "X.509 RSA-PSS hash field")
    mask_field = parameters.expect(2, True, 1, "X.509 RSA-PSS mask field")
    salt_field = parameters.expect(2, True, 2, "X.509 RSA-PSS salt field")
    parameters.finish("X.509 RSA-PSS parameters")

    hash_algorithm = _x509_single_sequence(hash_field, "X.509 RSA-PSS hash field")
    hash_oid, digest_bytes = _x509_hash_algorithm(hash_algorithm)
    mask_algorithm = _x509_single_sequence(mask_field, "X.509 RSA-PSS mask field")
    mask_oid, mask_parameters = _x509_algorithm_identifier(mask_algorithm)
    if mask_oid != bytes.fromhex("2a864886f70d010108"):
        raise ValueError("X.509 RSA-PSS mask algorithm is not MGF1")
    if mask_parameters is None or mask_parameters[:3] != (0, True, 16):
        raise ValueError("X.509 RSA-PSS MGF1 parameters are malformed")
    mask_hash_oid, mask_digest_bytes = _x509_hash_algorithm(
        _x509_single_sequence(mask_parameters[4], "X.509 RSA-PSS MGF1 parameters")
    )
    if (mask_hash_oid, mask_digest_bytes) != (hash_oid, digest_bytes):
        raise ValueError("X.509 RSA-PSS MGF1 hash does not match the signature hash")

    salt = _StrictDerReader(salt_field)
    salt_length = _der_unsigned_integer(
        salt.expect(0, False, 2, "X.509 RSA-PSS salt length"),
        "X.509 RSA-PSS salt length",
    )
    salt.finish("X.509 RSA-PSS salt field")
    if salt_length != digest_bytes:
        raise ValueError("X.509 RSA-PSS salt length does not match the signature hash")


def _validate_x509_signature_algorithm(encoded: bytes) -> None:
    oid, parameters = _x509_algorithm_identifier(encoded)
    if oid == bytes.fromhex("2a864886f70d01010a"):
        if parameters is None or parameters[:3] != (0, True, 16):
            raise ValueError("X.509 RSA-PSS parameters are missing or malformed")
        _validate_x509_rsa_pss_parameters(parameters[4])
    elif oid in {
        bytes.fromhex("2a864886f70d01010b"),
        bytes.fromhex("2a864886f70d01010c"),
        bytes.fromhex("2a864886f70d01010d"),
    }:
        if not _x509_algorithm_parameters_are_absent_or_null(parameters):
            raise ValueError("X.509 RSA signature parameters are invalid")
    elif oid in {
        bytes.fromhex("2a8648ce3d040302"),
        bytes.fromhex("2a8648ce3d040303"),
        bytes.fromhex("2b6570"),
    }:
        if parameters is not None:
            raise ValueError("X.509 signature parameters must be absent")
    else:
        raise ValueError("X.509 signature algorithm is not approved")


def _x509_certificate_header(certificate: bytes) -> tuple[_StrictDerReader, int]:
    certificate_reader = _StrictDerReader(certificate)
    certificate_sequence = certificate_reader.expect(0, True, 16, "X.509 certificate")
    certificate_reader.finish("X.509 certificate")
    outer = _StrictDerReader(certificate_sequence)
    tbs_class, tbs_constructed, tbs_tag, tbs, _ = outer.read()
    algorithm_class, algorithm_constructed, algorithm_tag, _, outer_algorithm = (
        outer.read()
    )
    signature_class, signature_constructed, signature_tag, signature, _ = outer.read()
    outer.finish("X.509 certificate")
    if (tbs_class, tbs_constructed, tbs_tag) != (0, True, 16):
        raise ValueError("X.509 TBSCertificate is malformed")
    if (algorithm_class, algorithm_constructed, algorithm_tag) != (0, True, 16):
        raise ValueError("X.509 signatureAlgorithm is malformed")
    if (
        (signature_class, signature_constructed, signature_tag) != (0, False, 3)
        or len(signature) < 2
        or signature[0] != 0
    ):
        raise ValueError("X.509 signatureValue must be an octet-aligned BIT STRING")

    reader = _StrictDerReader(tbs)
    version_class, version_constructed, version_tag, version_value, _ = reader.read()
    if (version_class, version_constructed, version_tag) != (2, True, 0):
        raise ValueError("Android attestation certificates must explicitly use X.509 version 3")
    version = _StrictDerReader(version_value)
    version_number = _der_unsigned_integer(
        version.expect(0, False, 2, "X.509 version"), "X.509 version"
    )
    version.finish("X.509 version")
    if version_number != 2:
        raise ValueError("Android attestation certificates must use X.509 version 3")

    serial_value = reader.expect(0, False, 2, "X.509 serialNumber")
    if len(serial_value) > 20:
        raise ValueError("X.509 serialNumber must be a canonical positive 20-byte integer")
    serial = _der_unsigned_integer(serial_value, "X.509 serialNumber")
    if serial == 0:
        raise ValueError("X.509 serialNumber must be positive")

    inner_class, inner_constructed, inner_tag, _, inner_algorithm = reader.read()
    if (
        (inner_class, inner_constructed, inner_tag) != (0, True, 16)
        or inner_algorithm != outer_algorithm
    ):
        raise ValueError(
            "X.509 inner and outer signature algorithms must match exactly"
        )
    _validate_x509_signature_algorithm(inner_algorithm)
    return reader, serial


def _x509_tbs_extension_payload(reader: _StrictDerReader) -> bytes | None:
    extension_payload: bytes | None = None
    previous_tag = 0
    while reader.remaining():
        tag_class, constructed, tag, value, _ = reader.read()
        if tag_class != 2 or tag not in {1, 2, 3} or tag <= previous_tag:
            raise ValueError("X.509 TBSCertificate contains an unexpected trailing field")
        previous_tag = tag
        if tag in {1, 2}:
            if constructed or not value or value[0] > 7:
                raise ValueError("X.509 unique identifier is malformed")
        elif not constructed:
            raise ValueError("X.509 extensions field is malformed")
        else:
            extension_payload = value
    return extension_payload


def _x509_extensions(extension_payload: bytes) -> dict[bytes, bytes]:
    wrapper = _StrictDerReader(extension_payload)
    extension_sequence = wrapper.expect(0, True, 16, "X.509 extensions")
    wrapper.finish("X.509 extensions")
    extensions = _StrictDerReader(extension_sequence)
    values: dict[bytes, bytes] = {}
    while extensions.remaining():
        encoded_extension = extensions.expect(0, True, 16, "X.509 extension")
        extension = _StrictDerReader(encoded_extension)
        oid = extension.expect(0, False, 6, "X.509 extension OID")
        if not oid or oid in values:
            raise ValueError("X.509 certificate contains duplicate or empty extension OIDs")
        next_class, next_constructed, next_tag, next_value, _ = extension.read()
        if (next_class, next_constructed, next_tag) == (0, False, 1):
            _der_boolean(next_value, "X.509 extension critical")
            value = extension.expect(0, False, 4, "X.509 extension value")
        elif (next_class, next_constructed, next_tag) == (0, False, 4):
            value = next_value
        else:
            raise ValueError("X.509 extension value is malformed")
        extension.finish("X.509 extension")
        values[oid] = value
    return values


def _split_der_certificate_chain(payload: bytes) -> list[bytes]:
    reader = _StrictDerReader(payload)
    certificates: list[bytes] = []
    while reader.remaining():
        tag_class, constructed, tag, _, encoded = reader.read()
        if (tag_class, constructed, tag) != (0, True, 16):
            raise ValueError("DER attestation chain must contain only X.509 sequences")
        certificates.append(encoded)
    return certificates


def _decode_attestation_certificate_chain(relative: str, payload: bytes) -> list[bytes]:
    suffix = PurePosixPath(relative).suffix.lower()
    if suffix == ".der":
        certificates = _split_der_certificate_chain(payload)
    elif suffix == ".pem":
        pattern = re.compile(
            rb"-----BEGIN CERTIFICATE-----\r?\n([A-Za-z0-9+/=\r\n]+)"
            rb"-----END CERTIFICATE-----"
        )
        certificates = []
        position = 0
        for match in pattern.finditer(payload):
            if payload[position : match.start()].strip():
                raise ValueError("PEM attestation chain contains non-certificate data")
            encoded = re.sub(rb"\s+", b"", match.group(1))
            try:
                certificate = base64.b64decode(encoded, validate=True)
            except ValueError as error:
                raise ValueError("PEM attestation chain contains invalid base64") from error
            parsed = _split_der_certificate_chain(certificate)
            if len(parsed) != 1 or parsed[0] != certificate:
                raise ValueError("PEM attestation chain contains invalid certificate DER")
            certificates.append(certificate)
            position = match.end()
        if payload[position:].strip():
            raise ValueError("PEM attestation chain contains trailing non-certificate data")
    else:
        raise ValueError("attestation chain suffix is unsupported")
    if len(certificates) < 2:
        raise ValueError("attestation certificate chain must contain at least two certificates")
    if len(certificates) > 8:
        raise ValueError("attestation certificate chain contains too many certificates")
    digests = [hashlib.sha256(certificate).digest() for certificate in certificates]
    if len(set(digests)) != len(digests):
        raise ValueError("attestation certificate chain repeats a certificate")
    return certificates


def _x509_certificate_serial_and_attestation_extension(
    certificate: bytes,
) -> tuple[str, bytes]:
    reader, serial = _x509_certificate_header(certificate)
    for label in (
        "X.509 issuer",
        "X.509 validity",
        "X.509 subject",
        "X.509 subjectPublicKeyInfo",
    ):
        reader.expect(0, True, 16, label)

    extension_payload = _x509_tbs_extension_payload(reader)
    if extension_payload is None:
        raise ValueError("X.509 certificate has no extensions")
    oid_value = ANDROID_KEY_ATTESTATION_EXTENSION_OID_DER[2:]
    attestation_extension = _x509_extensions(extension_payload).get(oid_value)
    if attestation_extension is None:
        raise ValueError(
            f"leaf certificate is missing Android extension {ANDROID_KEY_ATTESTATION_EXTENSION_OID}"
        )
    return format(serial, "x"), attestation_extension


def _x509_certificate_serial(certificate: bytes) -> str:
    _, serial = _x509_certificate_header(certificate)
    return format(serial, "x")


def _x509_certificate_tbs_sha256(certificate: bytes) -> str:
    """Return lowercase SHA-256 of the exact DER TBSCertificate value."""

    # Validate the complete certificate before projecting the exact first
    # element of Certificate ::= SEQUENCE { tbsCertificate, ... }.
    _x509_certificate_header(certificate)
    certificate_reader = _StrictDerReader(certificate)
    certificate_sequence = certificate_reader.expect(0, True, 16, "X.509 certificate")
    certificate_reader.finish("X.509 certificate")
    outer = _StrictDerReader(certificate_sequence)
    tag_class, constructed, tag, _, raw_tbs = outer.read()
    if (tag_class, constructed, tag) != (0, True, 16):
        raise ValueError("X.509 TBSCertificate is malformed")
    return hashlib.sha256(raw_tbs).hexdigest()


def _x509_certificate_validity_and_subject(
    certificate: bytes,
) -> tuple[tuple[int, int], bytes]:
    """Return exact UTC validity milliseconds and raw subject Name contents."""

    reader, _ = _x509_certificate_header(certificate)
    reader.expect(0, True, 16, "X.509 issuer")
    validity = reader.expect(0, True, 16, "X.509 validity")
    subject = reader.expect(0, True, 16, "X.509 subject")
    reader.expect(0, True, 16, "X.509 subjectPublicKeyInfo")
    extension_payload = _x509_tbs_extension_payload(reader)
    if extension_payload is not None:
        _x509_extensions(extension_payload)

    validity_reader = _StrictDerReader(validity)
    not_before = _x509_time_seconds(validity_reader, "X.509 notBefore")
    not_after = _x509_time_seconds(validity_reader, "X.509 notAfter")
    validity_reader.finish("X.509 validity")
    if not_after < not_before:
        raise ValueError("X.509 validity ends before it starts")
    return (not_before * 1_000, not_after * 1_000), subject


def _x509_time_seconds(reader: _StrictDerReader, label: str) -> int:
    tag_class, constructed, tag, value, _ = reader.read()
    if tag_class != 0 or constructed or tag not in {23, 24}:
        raise ValueError(f"{label} must be canonical UTC or GeneralizedTime")
    try:
        encoded = value.decode("ascii")
        if tag == 23:
            if re.fullmatch(r"[0-9]{12}Z", encoded) is None:
                raise ValueError
            year = int(encoded[:2])
            year += 1900 if year >= 50 else 2000
            canonical = f"{year:04d}{encoded[2:]}"
        else:
            if re.fullmatch(r"[0-9]{14}Z", encoded) is None:
                raise ValueError
            year = int(encoded[:4])
            if not 2050 <= year <= 9999:
                raise ValueError
            canonical = encoded
        parsed = dt.datetime.strptime(canonical, "%Y%m%d%H%M%SZ").replace(
            tzinfo=dt.timezone.utc
        )
    except (UnicodeDecodeError, ValueError) as error:
        raise ValueError(f"{label} is not canonical RFC 5280 time") from error
    return int(parsed.timestamp())


def _x509_directory_string(tag: int, value: bytes, label: str) -> str:
    try:
        if tag == 12:  # UTF8String
            return value.decode("utf-8")
        if tag in {19, 22}:  # PrintableString or IA5String
            return value.decode("ascii")
        if tag == 30:  # BMPString
            if len(value) % 2:
                raise UnicodeDecodeError(
                    "utf-16-be", value, len(value) - 1, len(value), "odd"
                )
            return value.decode("utf-16-be")
        if tag == 28:  # UniversalString
            if len(value) % 4:
                raise UnicodeDecodeError(
                    "utf-32-be", value, len(value) - 1, len(value), "odd"
                )
            return value.decode("utf-32-be")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} is not a valid DirectoryString") from error
    raise ValueError(f"{label} uses an unsupported DirectoryString encoding")


def _x509_subject_attributes(subject: bytes) -> list[tuple[bytes, int, bytes]]:
    attributes: list[tuple[bytes, int, bytes]] = []
    name = _StrictDerReader(subject)
    while name.remaining():
        rdn = _StrictDerReader(name.expect(0, True, 17, "X.509 subject RDN"))
        previous_attribute: bytes | None = None
        if not rdn.remaining():
            raise ValueError("X.509 subject RDN must not be empty")
        while rdn.remaining():
            tag_class, constructed, tag, attribute_value, encoded = rdn.read()
            if (tag_class, constructed, tag) != (0, True, 16):
                raise ValueError("X.509 subject attribute is malformed")
            if previous_attribute is not None and encoded < previous_attribute:
                raise ValueError("X.509 subject RDN SET is not in canonical DER order")
            previous_attribute = encoded
            attribute = _StrictDerReader(attribute_value)
            oid = attribute.expect(0, False, 6, "X.509 subject attribute OID")
            value_class, value_constructed, value_tag, value, _ = attribute.read()
            if value_class != 0 or value_constructed:
                raise ValueError("X.509 subject attribute value is malformed")
            attribute.finish("X.509 subject attribute")
            attributes.append((oid, value_tag, value))
    return attributes


def _classify_android_attestation_certificate_chain(
    root_nearest_non_anchor: bytes,
) -> str:
    """Classify the Android chain using only Google's exact subject profiles."""

    _, subject = _x509_certificate_validity_and_subject(root_nearest_non_anchor)
    attributes = _x509_subject_attributes(subject)
    has_factory_serial_number = any(
        oid == X509_SERIAL_NUMBER_OID_DER_VALUE for oid, _, _ in attributes
    )
    decoded_rkp_attributes: list[tuple[bytes, str]] = []
    if len(attributes) == 2 and all(
        oid in {X509_COMMON_NAME_OID_DER_VALUE, X509_ORGANIZATION_OID_DER_VALUE}
        for oid, _, _ in attributes
    ):
        decoded_rkp_attributes = [
            (
                oid,
                _x509_directory_string(
                    value_tag, value, "Android RKP subject attribute value"
                ),
            )
            for oid, value_tag, value in attributes
        ]
    has_rkp_identity = sorted(decoded_rkp_attributes) == sorted(
        (
            (X509_COMMON_NAME_OID_DER_VALUE, "Droid CA2"),
            (X509_ORGANIZATION_OID_DER_VALUE, "Google LLC"),
        )
    )
    if has_factory_serial_number and not has_rkp_identity:
        return "factory"
    if has_rkp_identity and not has_factory_serial_number:
        return "rkp"
    if has_factory_serial_number and has_rkp_identity:
        raise ValueError(
            "Android Key Attestation certificate chain classification is ambiguous"
        )
    raise ValueError("Android Key Attestation certificate chain classification is unknown")


def _validate_android_attestation_certificate_time_profile(
    certificates: list[bytes],
    *,
    evaluation_time_ms: int,
) -> str:
    """Apply the Core Factory/RKP non-target certificate validity profile."""

    if len(certificates) < 2:
        raise ValueError(
            "Android Key Attestation chain has no root-nearest non-anchor certificate"
        )
    chain_kind = _classify_android_attestation_certificate_chain(certificates[-2])
    legacy_factory_root = (
        chain_kind == "factory"
        and hashlib.sha256(certificates[-1]).hexdigest()
        == ANDROID_LEGACY_GOOGLE_ATTESTATION_ROOT_SHA256
    )
    for certificate in certificates[1:]:
        (not_before, not_after), _ = _x509_certificate_validity_and_subject(certificate)
        if evaluation_time_ms < not_before:
            raise ValueError(
                "Android Key Attestation non-target certificate is not yet valid at validation time"
            )
        if chain_kind == "factory":
            if not legacy_factory_root and evaluation_time_ms > not_after:
                raise ValueError(
                    "Android Key Attestation factory certificate is expired at validation time"
                )
        elif evaluation_time_ms > not_after:
            raise ValueError(
                "Android Key Attestation RKP certificate is not valid at validation time"
            )

    # This physical-capture preflight has no on-chain registration lifetime to
    # bind. For RKP it proves non-target validity at this validation horizon;
    # Core independently requires the same chain to remain valid through each
    # actual offline-device registration's authenticated expiry timestamp.
    return chain_kind


def _parse_attestation_application_id(value: bytes) -> tuple[set[str], set[bytes]]:
    explicit = _StrictDerReader(value)
    encoded = explicit.expect(0, False, 4, "attestationApplicationId OCTET STRING")
    explicit.finish("attestationApplicationId")
    wrapper = _StrictDerReader(encoded)
    sequence = wrapper.expect(0, True, 16, "attestationApplicationId")
    wrapper.finish("attestationApplicationId")
    reader = _StrictDerReader(sequence)
    packages_bytes = reader.expect(0, True, 17, "attestation packageInfos")
    digests_bytes = reader.expect(0, True, 17, "attestation signatureDigests")
    reader.finish("attestationApplicationId")

    packages: set[str] = set()
    package_reader = _StrictDerReader(packages_bytes)
    while package_reader.remaining():
        package_sequence = package_reader.expect(0, True, 16, "attestation packageInfo")
        package = _StrictDerReader(package_sequence)
        name_bytes = package.expect(0, False, 4, "attestation packageName")
        _der_unsigned_integer(
            package.expect(0, False, 2, "attestation packageVersion"),
            "attestation packageVersion",
        )
        package.finish("attestation packageInfo")
        try:
            name = name_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError("attestation packageName must be UTF-8") from error
        if not name or name in packages:
            raise ValueError("attestationApplicationId repeats or empties a package name")
        packages.add(name)

    digests: set[bytes] = set()
    digest_reader = _StrictDerReader(digests_bytes)
    while digest_reader.remaining():
        digest = digest_reader.expect(0, False, 4, "attestation signatureDigest")
        if len(digest) != 32 or digest in digests:
            raise ValueError("attestationApplicationId has an invalid signing digest")
        digests.add(digest)
    if not packages or not digests:
        raise ValueError("attestationApplicationId must bind a package and signing digest")
    return packages, digests


def _parse_android_root_of_trust(value: bytes) -> None:
    explicit = _StrictDerReader(value)
    sequence = explicit.expect(0, True, 16, "rootOfTrust")
    explicit.finish("rootOfTrust")
    reader = _StrictDerReader(sequence)
    verified_boot_key = reader.expect(0, False, 4, "verifiedBootKey")
    locked = _der_boolean(reader.expect(0, False, 1, "deviceLocked"), "deviceLocked")
    state = _der_unsigned_integer(
        reader.expect(0, False, 10, "verifiedBootState"),
        "verifiedBootState",
    )
    verified_boot_hash = (
        reader.expect(0, False, 4, "verifiedBootHash") if reader.remaining() else None
    )
    reader.finish("rootOfTrust")
    if not verified_boot_key:
        raise ValueError("verifiedBootKey must be non-empty")
    if not locked:
        raise ValueError("Android attestation requires deviceLocked=true")
    if state != ANDROID_VERIFIED_BOOT_STATE_VERIFIED:
        raise ValueError("Android attestation requires verifiedBootState=Verified")
    if verified_boot_hash is None or len(verified_boot_hash) != 32:
        raise ValueError("Android StrongBox attestation requires a SHA-256 verifiedBootHash")


def _parse_android_authorization_list(
    value: bytes,
    *,
    hardware: bool,
) -> tuple[list[tuple[set[str], set[bytes]]], int]:
    reader = _StrictDerReader(value)
    applications: list[tuple[set[str], set[bytes]]] = []
    roots = 0
    seen_tags: set[int] = set()
    while reader.remaining():
        tag_class, _, tag, entry, _ = reader.read()
        if tag_class != 2:
            raise ValueError("Android authorization entry must be context-specific")
        if tag in seen_tags:
            raise ValueError(f"Android authorization list repeats tag {tag}")
        seen_tags.add(tag)
        if tag == ANDROID_TAG_ALL_APPLICATIONS:
            raise ValueError("Android attestation must not authorize all applications")
        if tag == ANDROID_TAG_ATTESTATION_APPLICATION_ID:
            applications.append(_parse_attestation_application_id(entry))
        elif tag == ANDROID_TAG_ROOT_OF_TRUST:
            if not hardware:
                raise ValueError("rootOfTrust must be hardware-enforced")
            _parse_android_root_of_trust(entry)
            roots += 1
    return applications, roots


def _parse_android_key_description(
    extension: bytes,
    *,
    expected_challenge: bytes,
    expected_package: str,
    expected_signing_digest: bytes,
) -> None:
    wrapper = _StrictDerReader(extension)
    sequence = wrapper.expect(0, True, 16, "Android KeyDescription")
    wrapper.finish("Android KeyDescription")
    reader = _StrictDerReader(sequence)
    attestation_version = _der_unsigned_integer(
        reader.expect(0, False, 2, "attestationVersion"), "attestationVersion"
    )
    attestation_level = _der_unsigned_integer(
        reader.expect(0, False, 10, "attestationSecurityLevel"),
        "attestationSecurityLevel",
    )
    keymint_version = _der_unsigned_integer(
        reader.expect(0, False, 2, "keyMintVersion"), "keyMintVersion"
    )
    keymint_level = _der_unsigned_integer(
        reader.expect(0, False, 10, "keyMintSecurityLevel"),
        "keyMintSecurityLevel",
    )
    challenge = reader.expect(0, False, 4, "attestationChallenge")
    reader.expect(0, False, 4, "uniqueId")
    software = reader.expect(0, True, 16, "softwareEnforced")
    hardware = reader.expect(0, True, 16, "hardwareEnforced")
    reader.finish("Android KeyDescription")
    if attestation_version <= 0 or keymint_version <= 0:
        raise ValueError("Android attestation and KeyMint versions must be positive")
    if (
        attestation_level != ANDROID_SECURITY_LEVEL_STRONGBOX
        or keymint_level != ANDROID_SECURITY_LEVEL_STRONGBOX
    ):
        raise ValueError(
            "Android attestationSecurityLevel and keyMintSecurityLevel must both be StrongBox(2)"
        )
    if len(challenge) != 32 or challenge != expected_challenge:
        raise ValueError("leaf Android attestation challenge is not the exact candidate challenge")

    app_ids: list[tuple[set[str], set[bytes]]] = []
    root_count = 0
    parsed_apps, parsed_roots = _parse_android_authorization_list(
        software, hardware=False
    )
    app_ids.extend(parsed_apps)
    root_count += parsed_roots
    parsed_apps, parsed_roots = _parse_android_authorization_list(
        hardware, hardware=True
    )
    app_ids.extend(parsed_apps)
    root_count += parsed_roots
    if len(app_ids) != 1:
        raise ValueError("Android attestation must contain exactly one attestationApplicationId")
    packages, digests = app_ids[0]
    if packages != {expected_package}:
        raise ValueError("attestationApplicationId does not bind exactly the wallet package")
    if digests != {expected_signing_digest}:
        raise ValueError(
            "attestationApplicationId does not bind exactly the production wallet signing digest"
        )
    if root_count != 1:
        raise ValueError("Android attestation must contain exactly one hardware rootOfTrust")


def _certificate_pem(certificate: bytes) -> bytes:
    encoded = base64.b64encode(certificate)
    lines = [encoded[index : index + 64] for index in range(0, len(encoded), 64)]
    return b"-----BEGIN CERTIFICATE-----\n" + b"\n".join(lines) + (
        b"\n-----END CERTIFICATE-----\n"
    )
