"""Reusable X.509 fixtures for Android Key Attestation checker tests."""

from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
import tempfile
from typing import Any


device_lab: Any = None


def bind_device_lab(module: Any) -> None:
    """Bind the exact checker module instance under test."""

    global device_lab
    device_lab = module


def configure_test_authority(openssl: Path, root_key: Path, root_cert: Path) -> None:
    """Install the ephemeral certificate issuer used by fixture builders."""

    global _TEST_ATTESTATION_OPENSSL
    global _TEST_ATTESTATION_ROOT_KEY
    global _TEST_ATTESTATION_ROOT_CERT
    _TEST_ATTESTATION_OPENSSL = openssl
    _TEST_ATTESTATION_ROOT_KEY = root_key
    _TEST_ATTESTATION_ROOT_CERT = root_cert


def clear_test_authority() -> None:
    """Forget the ephemeral issuer and all generated certificate chains."""

    global _TEST_ATTESTATION_OPENSSL
    global _TEST_ATTESTATION_ROOT_KEY
    global _TEST_ATTESTATION_ROOT_CERT
    _TEST_ATTESTATION_OPENSSL = None
    _TEST_ATTESTATION_ROOT_KEY = None
    _TEST_ATTESTATION_ROOT_CERT = None
    _TEST_ATTESTATION_CHAIN_CACHE.clear()


def authority_is_configured() -> bool:
    """Return whether certificate-chain fixtures can currently be built."""

    return _TEST_ATTESTATION_ROOT_CERT is not None


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    path.chmod(0o600)


_TEST_ATTESTATION_OPENSSL: Path | None = None
_TEST_ATTESTATION_ROOT_KEY: Path | None = None
_TEST_ATTESTATION_ROOT_CERT: Path | None = None
_TEST_ATTESTATION_CHAIN_CACHE: dict[
    tuple[bytes, str, bytes, int, int, int, bool, bool, str, int], bytes
] = {}


def _der_length(length: int) -> bytes:
    if length < 0x80:
        return bytes((length,))
    encoded = length.to_bytes((length.bit_length() + 7) // 8, "big")
    return bytes((0x80 | len(encoded),)) + encoded


def _der_tlv(tag: bytes, value: bytes) -> bytes:
    return tag + _der_length(len(value)) + value


def _der_integer(value: int, *, enumerated: bool = False) -> bytes:
    if value < 0:
        raise AssertionError("test DER integers must be non-negative")
    encoded = value.to_bytes(max(1, (value.bit_length() + 7) // 8), "big")
    if encoded[0] & 0x80:
        encoded = b"\0" + encoded
    return _der_tlv(b"\x0a" if enumerated else b"\x02", encoded)


def _der_sequence(*values: bytes) -> bytes:
    return _der_tlv(b"\x30", b"".join(values))


def _der_set(*values: bytes) -> bytes:
    return _der_tlv(b"\x31", b"".join(sorted(values)))


def _der_octets(value: bytes) -> bytes:
    return _der_tlv(b"\x04", value)


def _der_context_explicit(tag: int, value: bytes) -> bytes:
    if tag < 31:
        encoded_tag = bytes((0xA0 | tag,))
    else:
        chunks = [tag & 0x7F]
        tag >>= 7
        while tag:
            chunks.append(0x80 | (tag & 0x7F))
            tag >>= 7
        encoded_tag = b"\xbf" + bytes(reversed(chunks))
    return _der_tlv(encoded_tag, value)


def android_key_description_der(
    challenge: bytes,
    package_name: str,
    signing_digest: bytes,
    *,
    attestation_level: int = 2,
    keymint_level: int = 2,
    verified_boot_state: int = 0,
    device_locked: bool = True,
    append_ninth_sequence: bool = False,
) -> bytes:
    package_info = _der_sequence(
        _der_octets(package_name.encode("utf-8")),
        _der_integer(1),
    )
    app_id = _der_sequence(
        _der_set(package_info),
        _der_set(_der_octets(signing_digest)),
    )
    root_of_trust = _der_sequence(
        _der_octets(b"\x11" * 32),
        _der_tlv(b"\x01", b"\xff" if device_locked else b"\x00"),
        _der_integer(verified_boot_state, enumerated=True),
        _der_octets(b"\x22" * 32),
    )
    software = _der_sequence(
        _der_context_explicit(
            device_lab.ANDROID_TAG_ATTESTATION_APPLICATION_ID,
            _der_octets(app_id),
        )
    )
    hardware = _der_sequence(
        _der_context_explicit(
            device_lab.ANDROID_TAG_ROOT_OF_TRUST,
            root_of_trust,
        )
    )
    fields = [
        _der_integer(400),
        _der_integer(attestation_level, enumerated=True),
        _der_integer(400),
        _der_integer(keymint_level, enumerated=True),
        _der_octets(challenge),
        _der_octets(b""),
        software,
        hardware,
    ]
    if append_ninth_sequence:
        fields.append(_der_sequence())
    return _der_sequence(*fields)


def test_android_attestation_chain(
    challenge: bytes,
    package_name: str,
    signing_digest: bytes,
    *,
    attestation_level: int = 2,
    keymint_level: int = 2,
    verified_boot_state: int = 0,
    device_locked: bool = True,
    append_ninth_sequence: bool = False,
    chain_kind: str = "factory",
    leaf_days: int = 3650,
) -> bytes:
    cache_key = (
        challenge,
        package_name,
        signing_digest,
        attestation_level,
        keymint_level,
        verified_boot_state,
        device_locked,
        append_ninth_sequence,
        chain_kind,
        leaf_days,
    )
    cached = _TEST_ATTESTATION_CHAIN_CACHE.get(cache_key)
    if cached is not None:
        return cached
    if (
        _TEST_ATTESTATION_OPENSSL is None
        or _TEST_ATTESTATION_ROOT_KEY is None
        or _TEST_ATTESTATION_ROOT_CERT is None
    ):
        raise AssertionError("test Android attestation authority is not initialized")
    description = android_key_description_der(
        challenge,
        package_name,
        signing_digest,
        attestation_level=attestation_level,
        keymint_level=keymint_level,
        verified_boot_state=verified_boot_state,
        device_locked=device_locked,
        append_ninth_sequence=append_ninth_sequence,
    )
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        issuer_key = _TEST_ATTESTATION_ROOT_KEY
        issuer_cert = _TEST_ATTESTATION_ROOT_CERT
        intermediate_pem: Path | None = None
        if chain_kind != "factory":
            intermediate_key = root / "intermediate.key"
            intermediate_csr = root / "intermediate.csr"
            intermediate_pem = root / "intermediate.pem"
            intermediate_extensions = root / "intermediate-extensions.cnf"
            write_text(
                intermediate_extensions,
                "basicConstraints=critical,CA:TRUE,pathlen:0\n"
                "keyUsage=critical,keyCertSign,cRLSign\n"
                "subjectKeyIdentifier=hash\n"
                "authorityKeyIdentifier=keyid,issuer\n",
            )
            if chain_kind == "rkp":
                intermediate_subject = "/CN=Droid CA2/O=Google LLC"
            elif chain_kind == "unknown":
                intermediate_subject = "/CN=Unknown Android Attestation CA"
            else:
                raise AssertionError(f"unsupported test Android chain kind: {chain_kind}")
            subprocess.run(
                [
                    str(_TEST_ATTESTATION_OPENSSL),
                    "genpkey",
                    "-algorithm",
                    "EC",
                    "-pkeyopt",
                    "ec_paramgen_curve:P-256",
                    "-out",
                    str(intermediate_key),
                ],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [
                    str(_TEST_ATTESTATION_OPENSSL),
                    "req",
                    "-new",
                    "-key",
                    str(intermediate_key),
                    "-subj",
                    intermediate_subject,
                    "-out",
                    str(intermediate_csr),
                ],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [
                    str(_TEST_ATTESTATION_OPENSSL),
                    "x509",
                    "-req",
                    "-in",
                    str(intermediate_csr),
                    "-CA",
                    str(_TEST_ATTESTATION_ROOT_CERT),
                    "-CAkey",
                    str(_TEST_ATTESTATION_ROOT_KEY),
                    "-set_serial",
                    hex(
                        int.from_bytes(
                            hashlib.sha256(b"intermediate\0" + challenge).digest()[:16],
                            "big",
                        )
                        or 1
                    ),
                    "-days",
                    "3650",
                    "-sha256",
                    "-extfile",
                    str(intermediate_extensions),
                    "-out",
                    str(intermediate_pem),
                ],
                check=True,
                capture_output=True,
            )
            issuer_key = intermediate_key
            issuer_cert = intermediate_pem
        leaf_key = root / "leaf.key"
        leaf_csr = root / "leaf.csr"
        leaf_pem = root / "leaf.pem"
        extensions = root / "extensions.cnf"
        write_text(
            extensions,
            "basicConstraints=critical,CA:FALSE\n"
            "keyUsage=critical,digitalSignature\n"
            "subjectKeyIdentifier=hash\n"
            "authorityKeyIdentifier=keyid,issuer\n"
            f"{device_lab.ANDROID_KEY_ATTESTATION_EXTENSION_OID}=DER:{description.hex()}\n",
        )
        leaf_subject = (
            "/serialNumber=factory-fixture/CN=Iroha Android StrongBox Test Leaf"
            if chain_kind == "factory"
            else "/CN=Iroha Android StrongBox Test Leaf"
        )
        subprocess.run(
            [
                str(_TEST_ATTESTATION_OPENSSL),
                "genpkey",
                "-algorithm",
                "EC",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-out",
                str(leaf_key),
            ],
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                str(_TEST_ATTESTATION_OPENSSL),
                "req",
                "-new",
                "-key",
                str(leaf_key),
                "-subj",
                leaf_subject,
                "-out",
                str(leaf_csr),
            ],
            check=True,
            capture_output=True,
        )
        serial = int.from_bytes(hashlib.sha256(b"serial\0" + challenge).digest()[:16], "big") or 1
        subprocess.run(
            [
                str(_TEST_ATTESTATION_OPENSSL),
                "x509",
                "-req",
                "-in",
                str(leaf_csr),
                "-CA",
                str(issuer_cert),
                "-CAkey",
                str(issuer_key),
                "-set_serial",
                hex(serial),
                "-days",
                str(leaf_days),
                "-sha256",
                "-extfile",
                str(extensions),
                "-out",
                str(leaf_pem),
            ],
            check=True,
            capture_output=True,
        )
        chain = leaf_pem.read_bytes()
        if intermediate_pem is not None:
            chain += intermediate_pem.read_bytes()
        chain += _TEST_ATTESTATION_ROOT_CERT.read_bytes()
    _TEST_ATTESTATION_CHAIN_CACHE[cache_key] = chain
    return chain


test_android_attestation_chain.__test__ = False


def android_attestation_metadata(slot_id: str = "pixel8-crypto") -> dict[str, str]:
    signing_digest = hashlib.sha256(f"{slot_id}:wallet-signer".encode()).hexdigest()
    metadata = {
        "slot_id": slot_id,
        "candidate_record_sha256": hashlib.sha256(b"candidate-record").hexdigest(),
        "candidate_manifest_sha256": hashlib.sha256(b"candidate-manifest").hexdigest(),
        "candidate_stage_manifest_sha256": hashlib.sha256(b"stage-manifest").hexdigest(),
        "candidate_lab_native_library_sha256": hashlib.sha256(b"native-library").hexdigest(),
        "candidate_lab_apk_sha256": hashlib.sha256(b"candidate-apk").hexdigest(),
        "candidate_lab_test_apk_sha256": hashlib.sha256(b"candidate-test-apk").hexdigest(),
        "candidate_source_commit": "1" * 40,
        "candidate_source_tree_sha256": hashlib.sha256(b"source-tree").hexdigest(),
        "app_package_name": device_lab.KAGEMUSHA_WALLET_PACKAGE_NAME,
        "app_signing_certificate_sha256": signing_digest,
    }
    challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
    metadata["attestation_challenge_sha256"] = hashlib.sha256(challenge).hexdigest()
    return metadata
