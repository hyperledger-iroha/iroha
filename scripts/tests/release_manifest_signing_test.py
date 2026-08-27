from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

from scripts import release_manifest_signing as signing


TEST_PUBLIC_KEY = bytes.fromhex(
    "2152f8d19b791d24453242e15f2eab6c"
    "b7cffa7b6a5ed30097960e069881db12"
)
TEST_FINGERPRINT = (
    "3097e2dee2cb4a34b53840cdb705aed7"
    "1067c36f68db0e0f559c3f3fa043315f"
)
TEST_SIGNATURE = bytes.fromhex(
    "5a9e89b16ce487ecf4667ac0cf84ea79"
    "4b4730d440f3c2ca64143267204e0ccb"
    "e818d9f87a9e0be8bab2d7ba31f19afa"
    "4553ba8427bb493e24c2c5edd90a020e"
)
TEST_MANIFEST = (
    json.dumps(
        {
            "arch": "x86_64",
            "artifacts": [],
            "commit": "abcdef0",
            "version": "1.0.0",
        },
        indent=2,
        sort_keys=True,
    )
    + "\n"
).encode()


def _noncanonical_signature(kind: str) -> bytes:
    signature = bytearray(TEST_SIGNATURE)
    if kind == "r":
        signature[: signing.ED25519_PUBLIC_KEY_SIZE] = b"\xff" * 32
    elif kind == "s":
        signature[signing.ED25519_PUBLIC_KEY_SIZE :] = (
            signing.ED25519_SCALAR_ORDER.to_bytes(32, "little")
        )
    else:  # pragma: no cover - test helper contract
        raise AssertionError(f"unsupported noncanonical component: {kind}")
    return bytes(signature)


def _write_executable(path: Path, body: str) -> None:
    path.write_text("#!/usr/bin/env python3\n" + body, encoding="utf-8")
    path.chmod(0o700)


def _manifest(tmp_path: Path) -> Path:
    manifest = tmp_path / "release_manifest.json"
    manifest.write_bytes(TEST_MANIFEST)
    manifest.chmod(0o644)
    return manifest


def _external_signer(tmp_path: Path) -> Path:
    signer = tmp_path / "external-ed25519-signer"
    _write_executable(
        signer,
        "import sys\n"
        "from pathlib import Path\n"
        f"expected = bytes.fromhex({TEST_MANIFEST.hex()!r})\n"
        f"signature = bytes.fromhex({TEST_SIGNATURE.hex()!r})\n"
        "if Path(sys.argv[1]).read_bytes() != expected:\n"
        "    raise SystemExit(2)\n"
        "Path(sys.argv[2]).write_bytes(signature)\n",
    )
    return signer


def _raw_public_key(tmp_path: Path) -> Path:
    public_key = tmp_path / "release-public.raw"
    public_key.write_bytes(TEST_PUBLIC_KEY)
    public_key.chmod(0o600)
    return public_key


def _native_verifier(
    tmp_path: Path,
    *,
    mutate_source_during_execution: bool = False,
    mutate_input_during_execution: str | None = None,
    forbidden_input_paths: dict[str, Path] | None = None,
    require_sanitized_environment: bool = False,
) -> tuple[Path, str, Path]:
    verifier = tmp_path / "sorafs-validate"
    invocation_log = tmp_path / "native-verifier-invocations.log"
    mutation = ""
    if mutate_source_during_execution:
        mutation = (
            f"Path({str(verifier)!r}).write_text("
            "'#!/usr/bin/env python3\\nraise SystemExit(91)\\n', encoding='utf-8')\n"
        )
    if mutate_input_during_execution is not None:
        option = {
            "manifest": "--manifest",
            "public_key": "--public-key",
            "signature": "--signature",
        }[mutate_input_during_execution]
        mutation += (
            f"target = Path(options[{option!r}])\n"
            "mutated = bytearray(target.read_bytes())\n"
            "mutated[0] ^= 1\n"
            "target.write_bytes(mutated)\n"
        )
    input_guards = ""
    for input_name, forbidden_path in (forbidden_input_paths or {}).items():
        option = {
            "manifest": "--manifest",
            "public_key": "--public-key",
            "signature": "--signature",
        }[input_name]
        input_guards += (
            f"if Path(options[{option!r}]).resolve() == "
            f"Path({str(forbidden_path)!r}).resolve():\n"
            "    raise SystemExit(93)\n"
        )
    if require_sanitized_environment:
        input_guards += (
            "if 'SORAFS_RELEASE_VERIFIER_BYPASS' in os.environ:\n"
            "    raise SystemExit(94)\n"
        )
    _write_executable(
        verifier,
        "import hashlib\n"
        "import os\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"expected_manifest = bytes.fromhex({TEST_MANIFEST.hex()!r})\n"
        f"expected_key = bytes.fromhex({TEST_PUBLIC_KEY.hex()!r})\n"
        f"expected_signature = bytes.fromhex({TEST_SIGNATURE.hex()!r})\n"
        f"invocation_log = Path({str(invocation_log)!r})\n"
        "args = sys.argv[1:]\n"
        "if len(args) != 9 or args[0] != 'release-manifest':\n"
        "    raise SystemExit(4)\n"
        "options = dict(zip(args[1::2], args[2::2]))\n"
        "if set(options) != {\n"
        "    '--manifest', '--public-key', '--public-key-fingerprint', '--signature'\n"
        "}:\n"
        "    raise SystemExit(4)\n"
        + input_guards
        + "manifest = Path(options['--manifest']).read_bytes()\n"
        "public_key = Path(options['--public-key']).read_bytes()\n"
        "signature = Path(options['--signature']).read_bytes()\n"
        "fingerprint = options['--public-key-fingerprint']\n"
        "if manifest != expected_manifest:\n"
        "    raise SystemExit(2)\n"
        "if public_key != expected_key or len(public_key) != 32:\n"
        "    raise SystemExit(2)\n"
        "if hashlib.sha256(public_key).hexdigest() != fingerprint:\n"
        "    raise SystemExit(2)\n"
        "if signature != expected_signature or len(signature) != 64:\n"
        "    raise SystemExit(2)\n"
        "with invocation_log.open('a', encoding='utf-8') as handle:\n"
        "    handle.write('release-manifest\\n')\n"
        + mutation,
    )
    return verifier, hashlib.sha256(verifier.read_bytes()).hexdigest(), invocation_log


def _native_verifier_with_timed_ovn_audit(
    tmp_path: Path,
) -> tuple[Path, str, Path]:
    verifier = tmp_path / "sorafs-validate"
    invocation_log = tmp_path / "native-verifier-invocations.log"
    _write_executable(
        verifier,
        "import hashlib\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"expected_manifest = bytes.fromhex({TEST_MANIFEST.hex()!r})\n"
        f"expected_key = bytes.fromhex({TEST_PUBLIC_KEY.hex()!r})\n"
        f"expected_signature = bytes.fromhex({TEST_SIGNATURE.hex()!r})\n"
        f"invocation_log = Path({str(invocation_log)!r})\n"
        "args = sys.argv[1:]\n"
        "if args and args[0] == 'timed-ovn-release-audit':\n"
        "    if len(args) != 15:\n"
        "        raise SystemExit(4)\n"
        "    options = dict(zip(args[1::2], args[2::2]))\n"
        "    if set(options) != {\n"
        "        '--audit-manifest', '--implementation-source-archive',\n"
        "        '--release-artifact-manifest', '--supported-target-inventory',\n"
        "        '--audit-report', '--audit-evidence-archive',\n"
        "        '--trusted-reviewer-public-key'\n"
        "    }:\n"
        "        raise SystemExit(4)\n"
        "    if len(Path(options['--audit-manifest']).read_bytes()) != 301:\n"
        "        raise SystemExit(2)\n"
        "    if Path(options['--release-artifact-manifest']).read_bytes() != expected_manifest:\n"
        "        raise SystemExit(2)\n"
        "    if Path(options['--trusted-reviewer-public-key']).read_bytes() != expected_key:\n"
        "        raise SystemExit(2)\n"
        "    for option in (\n"
        "        '--implementation-source-archive', '--supported-target-inventory',\n"
        "        '--audit-report', '--audit-evidence-archive'\n"
        "    ):\n"
        "        if not Path(options[option]).read_bytes():\n"
        "            raise SystemExit(2)\n"
        "    with invocation_log.open('a', encoding='utf-8') as handle:\n"
        "        handle.write('timed-ovn-release-audit\\n')\n"
        "elif args and args[0] == 'release-manifest':\n"
        "    if len(args) != 9:\n"
        "        raise SystemExit(4)\n"
        "    options = dict(zip(args[1::2], args[2::2]))\n"
        "    if Path(options['--manifest']).read_bytes() != expected_manifest:\n"
        "        raise SystemExit(2)\n"
        "    public_key = Path(options['--public-key']).read_bytes()\n"
        "    if public_key != expected_key:\n"
        "        raise SystemExit(2)\n"
        "    if hashlib.sha256(public_key).hexdigest() != options['--public-key-fingerprint']:\n"
        "        raise SystemExit(2)\n"
        "    if Path(options['--signature']).read_bytes() != expected_signature:\n"
        "        raise SystemExit(2)\n"
        "    with invocation_log.open('a', encoding='utf-8') as handle:\n"
        "        handle.write('release-manifest\\n')\n"
        "else:\n"
        "    raise SystemExit(4)\n",
    )
    return verifier, hashlib.sha256(verifier.read_bytes()).hexdigest(), invocation_log


def _sign(
    tmp_path: Path,
    manifest: Path,
    signature: Path,
    public_key_output: Path,
    *,
    signer: Path | None = None,
    raw_public_key: Path | None = None,
    fingerprint: str = TEST_FINGERPRINT,
    verifier: Path | None = None,
    verifier_digest: str | None = None,
) -> dict[str, object]:
    signer = signer or _external_signer(tmp_path)
    raw_public_key = raw_public_key or _raw_public_key(tmp_path)
    if verifier is None:
        verifier, default_digest, _ = _native_verifier(tmp_path)
        verifier_digest = verifier_digest or default_digest
    assert verifier_digest is not None
    return signing.sign_release_manifest(
        manifest,
        signer,
        raw_public_key,
        fingerprint,
        signature,
        public_key_output,
        verifier,
        verifier_digest,
    )


def test_sign_verify_and_deterministic_signature_bytes(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, invocation_log = _native_verifier(tmp_path)
    first_signature = tmp_path / "first.sig"
    first_public_key = tmp_path / "first.pub"
    second_signature = tmp_path / "second.sig"
    second_public_key = tmp_path / "second.pub"

    first = _sign(
        tmp_path,
        manifest,
        first_signature,
        first_public_key,
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )
    second = _sign(
        tmp_path,
        manifest,
        second_signature,
        second_public_key,
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )

    assert first_signature.read_bytes() == second_signature.read_bytes()
    assert first_public_key.read_bytes() == second_public_key.read_bytes()
    assert first_public_key.read_bytes() == TEST_PUBLIC_KEY
    assert first["manifest_sha256"] == second["manifest_sha256"]
    assert first["signature_algorithm"] == "ed25519"
    assert first["public_key_format"] == "raw-ed25519-32"
    assert first["signer_fingerprint_sha256"] == TEST_FINGERPRINT
    assert first["native_verifier_protocol"] == signing.NATIVE_VERIFIER_PROTOCOL
    assert first["native_verifier_sha256"] == verifier_digest
    assert first["signature_verified"] is True
    assert signing.verify_release_manifest(
        manifest,
        first_signature,
        first_public_key,
        TEST_FINGERPRINT,
        verifier,
        verifier_digest,
    )["signature_verified"] is True
    assert invocation_log.read_text(encoding="utf-8").splitlines() == [
        "release-manifest"
    ] * 5


def test_timed_ovn_release_audit_runs_before_external_signing(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, invocation_log = _native_verifier_with_timed_ovn_audit(
        tmp_path
    )
    signer = tmp_path / "external-ed25519-signer"
    _write_executable(
        signer,
        "import sys\n"
        "from pathlib import Path\n"
        f"log = Path({str(invocation_log)!r})\n"
        f"signature = bytes.fromhex({TEST_SIGNATURE.hex()!r})\n"
        "with log.open('a', encoding='utf-8') as handle:\n"
        "    handle.write('external-signer\\n')\n"
        "Path(sys.argv[2]).write_bytes(signature)\n",
    )
    audit_manifest = tmp_path / "timed-ovn-audit.manifest"
    implementation_source_archive = tmp_path / "implementation-source.tar.zst"
    supported_target_inventory = tmp_path / "target-inventory.json"
    audit_report = tmp_path / "audit-report.pdf"
    audit_evidence_archive = tmp_path / "audit-evidence.tar.zst"
    reviewer_public_key = tmp_path / "audit-reviewer.ed25519.pub"
    audit_manifest.write_bytes(b"A" * signing.TIMED_OVN_AUDIT_MANIFEST_SIZE)
    implementation_source_archive.write_bytes(b"reviewed source")
    supported_target_inventory.write_bytes(b"reviewed targets")
    audit_report.write_bytes(b"independent report")
    audit_evidence_archive.write_bytes(b"independent evidence")
    reviewer_public_key.write_bytes(TEST_PUBLIC_KEY)

    result = signing.sign_release_manifest(
        manifest,
        signer,
        raw_key,
        TEST_FINGERPRINT,
        tmp_path / "release.sig",
        tmp_path / "release.pub",
        verifier,
        verifier_digest,
        timed_ovn_audit_manifest_path=audit_manifest,
        timed_ovn_implementation_source_archive_path=implementation_source_archive,
        timed_ovn_supported_target_inventory_path=supported_target_inventory,
        timed_ovn_audit_report_path=audit_report,
        timed_ovn_audit_evidence_archive_path=audit_evidence_archive,
        timed_ovn_trusted_reviewer_public_key_path=reviewer_public_key,
    )
    assert result["timed_ovn_release_audit_verified"] is True
    assert result["timed_ovn_release_audit_protocol"] == (
        signing.NATIVE_TIMED_OVN_AUDIT_PROTOCOL
    )
    assert result["timed_ovn_release_audit_manifest_sha256"] == hashlib.sha256(
        audit_manifest.read_bytes()
    ).hexdigest()
    assert result["timed_ovn_release_audit_reviewer_key_sha256"] == hashlib.sha256(
        reviewer_public_key.read_bytes()
    ).hexdigest()
    assert invocation_log.read_text(encoding="utf-8").splitlines() == [
        "timed-ovn-release-audit",
        "external-signer",
        "release-manifest",
        "release-manifest",
    ]


def test_timed_ovn_release_audit_rejects_partial_inputs_before_signing(
    tmp_path: Path,
) -> None:
    signer_log = tmp_path / "signer.log"
    signer = tmp_path / "external-ed25519-signer"
    _write_executable(
        signer,
        "from pathlib import Path\n"
        f"Path({str(signer_log)!r}).write_text('invoked', encoding='utf-8')\n",
    )
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="audit inputs must be supplied together",
    ):
        signing.sign_release_manifest(
            _manifest(tmp_path),
            signer,
            _raw_public_key(tmp_path),
            TEST_FINGERPRINT,
            tmp_path / "release.sig",
            tmp_path / "release.pub",
            verifier,
            verifier_digest,
            timed_ovn_audit_manifest_path=tmp_path / "audit.manifest",
        )
    assert not signer_log.exists()


def test_rejects_wrong_fingerprint_and_malformed_raw_key(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="does not match the reviewed fingerprint",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "wrong.sig",
            tmp_path / "wrong.pub",
            signer=signer,
            raw_public_key=raw_key,
            fingerprint="0" * 64,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    malformed_key = tmp_path / "malformed.raw"
    malformed_key.write_bytes(b"k" * 31)
    malformed_key.chmod(0o600)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly 32 raw bytes",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "malformed.sig",
            tmp_path / "malformed.pub",
            signer=signer,
            raw_public_key=malformed_key,
            fingerprint=hashlib.sha256(malformed_key.read_bytes()).hexdigest(),
            verifier=verifier,
            verifier_digest=verifier_digest,
        )


def test_rejects_rsa_pem_and_noncanonical_or_weak_public_keys(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    signature = tmp_path / "release.sig"
    signature.write_bytes(TEST_SIGNATURE)
    signature.chmod(0o600)

    rsa_pem = tmp_path / "rsa-public.pem"
    rsa_pem.write_bytes(
        b"-----BEGIN PUBLIC KEY-----\n"
        b"MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A\n"
        b"-----END PUBLIC KEY-----\n"
    )
    rsa_pem.chmod(0o600)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly 32 raw bytes",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            rsa_pem,
            hashlib.sha256(rsa_pem.read_bytes()).hexdigest(),
            verifier,
            verifier_digest,
        )

    noncanonical_key = tmp_path / "noncanonical.raw"
    noncanonical_key.write_bytes(b"\xff" * signing.ED25519_PUBLIC_KEY_SIZE)
    noncanonical_key.chmod(0o600)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="non-canonical point encoding",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            noncanonical_key,
            hashlib.sha256(noncanonical_key.read_bytes()).hexdigest(),
            verifier,
            verifier_digest,
        )

    weak_key = tmp_path / "weak.raw"
    weak_key.write_bytes(b"\x01" + b"\x00" * 31)
    weak_key.chmod(0o600)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="native release-manifest Ed25519 verification failed",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            weak_key,
            hashlib.sha256(weak_key.read_bytes()).hexdigest(),
            verifier,
            verifier_digest,
        )


@pytest.mark.parametrize(
    ("component", "diagnostic"),
    (
        ("r", "non-canonical Ed25519 R encoding"),
        ("s", "non-canonical Ed25519 scalar"),
    ),
)
def test_signing_rejects_noncanonical_signature_before_native_verification(
    tmp_path: Path,
    component: str,
    diagnostic: str,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, invocation_log = _native_verifier(tmp_path)
    signer = tmp_path / f"noncanonical-{component}-signer"
    bad_signature = _noncanonical_signature(component)
    _write_executable(
        signer,
        "import sys\n"
        "from pathlib import Path\n"
        f"signature = bytes.fromhex({bad_signature.hex()!r})\n"
        "Path(sys.argv[2]).write_bytes(signature)\n",
    )

    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match=diagnostic,
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / f"noncanonical-{component}.sig",
            tmp_path / f"noncanonical-{component}.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    assert not invocation_log.exists()
    assert not (tmp_path / f"noncanonical-{component}.sig").exists()
    assert not (tmp_path / f"noncanonical-{component}.pub").exists()


@pytest.mark.parametrize(
    ("component", "diagnostic"),
    (
        ("r", "non-canonical Ed25519 R encoding"),
        ("s", "non-canonical Ed25519 scalar"),
    ),
)
def test_verification_rejects_noncanonical_signature_before_native_verification(
    tmp_path: Path,
    component: str,
    diagnostic: str,
) -> None:
    manifest = _manifest(tmp_path)
    public_key = _raw_public_key(tmp_path)
    signature = tmp_path / f"noncanonical-{component}.sig"
    signature.write_bytes(_noncanonical_signature(component))
    signature.chmod(0o600)
    verifier, verifier_digest, invocation_log = _native_verifier(tmp_path)

    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match=diagnostic,
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )

    assert not invocation_log.exists()


def test_rejects_noncanonical_manifest_bytes(tmp_path: Path) -> None:
    manifest = tmp_path / "release_manifest.json"
    manifest.write_bytes(b'{"artifacts":[],"version":"1.0.0"}\n')
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="not canonical deterministic JSON",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "noncanonical.sig",
            tmp_path / "noncanonical.pub",
            verifier=verifier,
            verifier_digest=verifier_digest,
        )


def test_native_verifier_rejects_tampered_signature_and_encoded_key(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    signature = tmp_path / "release_manifest.json.sig"
    public_key = tmp_path / "release_manifest.json.pub"
    _sign(
        tmp_path,
        manifest,
        signature,
        public_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )
    tampered = bytearray(signature.read_bytes())
    tampered[0] ^= 1
    signature.write_bytes(tampered)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="native release-manifest Ed25519 verification failed",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )

    encoded_key = tmp_path / "encoded-public.pem"
    encoded_key.write_bytes(b"-----BEGIN PUBLIC KEY-----\n")
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly 32 raw bytes",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            encoded_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )


@pytest.mark.skipif(not hasattr(os, "symlink"), reason="symlinks are unavailable")
def test_rejects_symlinked_key_signature_and_verifier_inputs(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    key_link = tmp_path / "public-link.raw"
    key_link.symlink_to(raw_key)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="symlink path component",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "linked.sig",
            tmp_path / "linked.pub",
            signer=signer,
            raw_public_key=key_link,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    signature = tmp_path / "real.sig"
    public_key = tmp_path / "real.pub"
    _sign(
        tmp_path,
        manifest,
        signature,
        public_key,
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )
    signature_link = tmp_path / "signature-link.sig"
    signature_link.symlink_to(signature)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="symlink path component",
    ):
        signing.verify_release_manifest(
            manifest,
            signature_link,
            public_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )
    verifier_link = tmp_path / "verifier-link"
    verifier_link.symlink_to(verifier)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="symlink path component",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            verifier_link,
            verifier_digest,
        )


def test_rejects_hardlinked_and_unsafe_key_or_verifier_inputs(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    hardlink = tmp_path / "public-hardlink.raw"
    os.link(raw_key, hardlink)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly one hard link",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "hardlink.sig",
            tmp_path / "hardlink.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    hardlink.unlink()
    raw_key.chmod(0o666)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="group- or world-writable",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "unsafe.sig",
            tmp_path / "unsafe.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    raw_key.chmod(0o600)
    verifier_hardlink = tmp_path / "verifier-hardlink"
    os.link(verifier, verifier_hardlink)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly one hard link",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "verifier-hardlink.sig",
            tmp_path / "verifier-hardlink.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    verifier_hardlink.unlink()
    verifier.chmod(0o722)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="group- or world-writable",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "verifier-unsafe.sig",
            tmp_path / "verifier-unsafe.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    verifier.chmod(0o700)
    signer_hardlink = tmp_path / "signer-hardlink"
    os.link(signer, signer_hardlink)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly one hard link",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "signer-hardlink.sig",
            tmp_path / "signer-hardlink.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    signer_hardlink.unlink()
    signer.chmod(0o722)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="group- or world-writable",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "signer-unsafe.sig",
            tmp_path / "signer-unsafe.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )


def test_rejects_hardlinked_and_unsafe_signature_inputs(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    signer = _external_signer(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    signature = tmp_path / "release.sig"
    public_key = tmp_path / "release.pub"
    _sign(
        tmp_path,
        manifest,
        signature,
        public_key,
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )

    signature_hardlink = tmp_path / "release-hardlink.sig"
    os.link(signature, signature_hardlink)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly one hard link",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )
    signature_hardlink.unlink()
    signature.chmod(0o666)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="group- or world-writable",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            verifier,
            verifier_digest,
        )


@pytest.mark.parametrize("link_kind", ["symlink", "hardlink"])
def test_rejects_external_signer_link_outputs(
    tmp_path: Path,
    link_kind: str,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    seed = tmp_path / "signature-seed"
    seed.write_bytes(TEST_SIGNATURE)
    seed.chmod(0o600)
    malicious_signer = tmp_path / f"{link_kind}-signer"
    link_statement = (
        f"Path(sys.argv[2]).symlink_to(Path({str(seed)!r}))\n"
        if link_kind == "symlink"
        else f"os.link(Path({str(seed)!r}), Path(sys.argv[2]))\n"
    )
    _write_executable(
        malicious_signer,
        "import os\n"
        "import sys\n"
        "from pathlib import Path\n"
        + link_statement,
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="symlink path component|exactly one hard link",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / f"{link_kind}.sig",
            tmp_path / f"{link_kind}.pub",
            signer=malicious_signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )


def test_detects_manifest_and_native_verifier_identity_drift(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    mutating_signer = tmp_path / "mutating-signer"
    _write_executable(
        mutating_signer,
        "import sys\n"
        "from pathlib import Path\n"
        f"Path(sys.argv[2]).write_bytes(bytes.fromhex({TEST_SIGNATURE.hex()!r}))\n"
        "with Path(sys.argv[1]).open('ab') as handle:\n"
        "    handle.write(b' ')\n",
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="external signer exited|signer manifest snapshot changed",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "mutated.sig",
            tmp_path / "mutated.pub",
            signer=mutating_signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    assert manifest.read_bytes() == TEST_MANIFEST

    manifest.write_bytes(TEST_MANIFEST)
    drifting_verifier, drifting_digest, _ = _native_verifier(
        tmp_path,
        mutate_source_during_execution=True,
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="changed during execution",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "drift.sig",
            tmp_path / "drift.pub",
            raw_public_key=raw_key,
            verifier=drifting_verifier,
            verifier_digest=drifting_digest,
        )


def test_external_signer_executes_pinned_snapshot_during_path_substitution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    malicious_marker = tmp_path / "substituted-signer-ran"
    replacement = tmp_path / "replacement-signer"
    _write_executable(
        replacement,
        "from pathlib import Path\n"
        f"Path({str(malicious_marker)!r}).write_text('ran', encoding='utf-8')\n"
        "raise SystemExit(91)\n",
    )
    real_run = subprocess.run
    signer_substituted = False

    def substitute_before_execution(*args: object, **kwargs: object) -> object:
        nonlocal signer_substituted
        command = args[0]
        if (
            not signer_substituted
            and isinstance(command, list)
            and len(command) == 3
            and Path(command[0]).name.startswith(
                "external-ed25519-signer-pinned"
            )
        ):
            os.replace(replacement, signer)
            signer_substituted = True
        return real_run(*args, **kwargs)

    monkeypatch.setattr(signing.subprocess, "run", substitute_before_execution)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="external signer changed during execution",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "substitution.sig",
            tmp_path / "substitution.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    assert signer_substituted
    assert not malicious_marker.exists()


def test_external_signer_receives_private_manifest_snapshot(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    observation = tmp_path / "signer-observation.json"
    signer = tmp_path / "snapshot-observing-signer"
    _write_executable(
        signer,
        "import json\n"
        "import stat\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"original = Path({str(manifest)!r})\n"
        "snapshot = Path(sys.argv[1])\n"
        "if snapshot.resolve() == original.resolve():\n"
        "    raise SystemExit(92)\n"
        f"Path({str(observation)!r}).write_text(\n"
        "    json.dumps({\n"
        "        'mode': stat.S_IMODE(snapshot.stat().st_mode),\n"
        "        'payload': snapshot.read_bytes().hex(),\n"
        "    }),\n"
        "    encoding='utf-8',\n"
        ")\n"
        f"Path(sys.argv[2]).write_bytes(bytes.fromhex({TEST_SIGNATURE.hex()!r}))\n",
    )

    result = _sign(
        tmp_path,
        manifest,
        tmp_path / "snapshot.sig",
        tmp_path / "snapshot.pub",
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )

    assert result["signature_verified"] is True
    observed = json.loads(observation.read_text(encoding="utf-8"))
    assert observed == {
        "mode": 0o600,
        "payload": TEST_MANIFEST.hex(),
    }
    assert manifest.read_bytes() == TEST_MANIFEST


def test_external_signer_receives_only_fixed_secret_free_environment(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    observation = tmp_path / "signer-environment.json"
    signer = tmp_path / "environment-observing-signer"
    _write_executable(
        signer,
        "import json\n"
        "import os\n"
        "import sys\n"
        "from pathlib import Path\n"
        "if 'TAIRA_RELEASE_SIGNER_BYPASS' in os.environ:\n"
        "    raise SystemExit(95)\n"
        f"Path({str(observation)!r}).write_text(\n"
        "    json.dumps(dict(os.environ), sort_keys=True), encoding='utf-8'\n"
        ")\n"
        f"Path(sys.argv[2]).write_bytes(bytes.fromhex({TEST_SIGNATURE.hex()!r}))\n",
    )
    monkeypatch.setenv("TAIRA_RELEASE_SIGNER_BYPASS", "1")
    monkeypatch.setenv("GITHUB_TOKEN", "must-not-reach-signer")

    result = _sign(
        tmp_path,
        manifest,
        tmp_path / "environment.sig",
        tmp_path / "environment.pub",
        signer=signer,
        raw_public_key=raw_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )

    assert result["signature_verified"] is True
    observed = json.loads(observation.read_text(encoding="utf-8"))
    assert {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"} <= set(observed)
    assert "TAIRA_RELEASE_SIGNER_BYPASS" not in observed
    assert "GITHUB_TOKEN" not in observed
    parent_environment = signing._external_signer_environment(Path(observed["HOME"]))
    fixed_names = {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"}
    assert fixed_names <= set(parent_environment)
    assert set(parent_environment) <= fixed_names | {"SYSTEMROOT", "WINDIR"}
    assert observed["LANG"] == "C"
    assert observed["LC_ALL"] == "C"
    assert observed["PATH"] == os.defpath
    assert observed["HOME"] == observed["TMPDIR"]
    assert Path(observed["HOME"]).name.startswith("iroha-release-manifest-sign-")


@pytest.mark.parametrize("mutated_input", ["manifest", "public_key", "signature"])
def test_native_verifier_cannot_mutate_original_verification_inputs(
    tmp_path: Path,
    mutated_input: str,
) -> None:
    manifest = _manifest(tmp_path)
    signature = tmp_path / "release_manifest.json.sig"
    public_key = tmp_path / "release_manifest.json.pub"
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    _sign(
        tmp_path,
        manifest,
        signature,
        public_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )
    original_inputs = {
        "manifest": manifest.read_bytes(),
        "public_key": public_key.read_bytes(),
        "signature": signature.read_bytes(),
    }

    mutating_verifier, mutating_digest, _ = _native_verifier(
        tmp_path,
        mutate_input_during_execution=mutated_input,
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="verification failed|changed during verification",
    ):
        signing.verify_release_manifest(
            manifest,
            signature,
            public_key,
            TEST_FINGERPRINT,
            mutating_verifier,
            mutating_digest,
        )
    assert manifest.read_bytes() == original_inputs["manifest"]
    assert public_key.read_bytes() == original_inputs["public_key"]
    assert signature.read_bytes() == original_inputs["signature"]


def test_native_verifier_receives_private_snapshots_and_sanitized_environment(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manifest = _manifest(tmp_path)
    signature = tmp_path / "release_manifest.json.sig"
    public_key = tmp_path / "release_manifest.json.pub"
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    _sign(
        tmp_path,
        manifest,
        signature,
        public_key,
        verifier=verifier,
        verifier_digest=verifier_digest,
    )

    strict_verifier, strict_digest, invocation_log = _native_verifier(
        tmp_path,
        forbidden_input_paths={
            "manifest": manifest,
            "public_key": public_key,
            "signature": signature,
        },
        require_sanitized_environment=True,
    )
    invocation_log.unlink(missing_ok=True)
    monkeypatch.setenv("SORAFS_RELEASE_VERIFIER_BYPASS", "1")
    result = signing.verify_release_manifest(
        manifest,
        signature,
        public_key,
        TEST_FINGERPRINT,
        strict_verifier,
        strict_digest,
    )

    assert result["signature_verified"] is True
    assert invocation_log.read_text(encoding="utf-8") == "release-manifest\n"


def test_rejects_wrong_verifier_digest_malformed_signature_and_clobber(
    tmp_path: Path,
) -> None:
    manifest = _manifest(tmp_path)
    raw_key = _raw_public_key(tmp_path)
    signer = _external_signer(tmp_path)
    verifier, verifier_digest, _ = _native_verifier(tmp_path)
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="does not match the reviewed SHA256",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "wrong-verifier.sig",
            tmp_path / "wrong-verifier.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest="0" * 64,
        )

    malformed_signer = tmp_path / "malformed-signer"
    _write_executable(
        malformed_signer,
        "import sys\nfrom pathlib import Path\nPath(sys.argv[2]).write_bytes(b'bad')\n",
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="exactly 64 raw bytes",
    ):
        _sign(
            tmp_path,
            manifest,
            tmp_path / "bad.sig",
            tmp_path / "bad.pub",
            signer=malformed_signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )

    existing_signature = tmp_path / "existing.sig"
    existing_signature.write_bytes(b"do-not-overwrite")
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="already exists",
    ):
        _sign(
            tmp_path,
            manifest,
            existing_signature,
            tmp_path / "new.pub",
            signer=signer,
            raw_public_key=raw_key,
            verifier=verifier,
            verifier_digest=verifier_digest,
        )
    assert existing_signature.read_bytes() == b"do-not-overwrite"
    assert not (tmp_path / "new.pub").exists()


@pytest.mark.parametrize("entrypoint", ["sign", "verify"])
def test_root_without_sealed_identity_fails_before_snapshot_or_invocation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    entrypoint: str,
) -> None:
    monkeypatch.setattr(signing.os, "geteuid", lambda: 0)
    monkeypatch.setattr(signing.os, "getegid", lambda: 0)
    monkeypatch.delenv(signing.EXTERNAL_TOOL_UID_ENV, raising=False)
    monkeypatch.delenv(signing.EXTERNAL_TOOL_GID_ENV, raising=False)

    def forbidden(*_args, **_kwargs):
        pytest.fail("root inspected or invoked an external-tool input before refusal")

    monkeypatch.setattr(signing, "_stable_read", forbidden)
    monkeypatch.setattr(signing, "_stable_digest", forbidden)
    monkeypatch.setattr(signing.subprocess, "run", forbidden)

    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="root may not snapshot or invoke",
    ):
        if entrypoint == "verify":
            signing.verify_release_manifest(
                tmp_path / "manifest",
                tmp_path / "signature",
                tmp_path / "public-key",
                "0" * 64,
                tmp_path / "verifier",
                "1" * 64,
            )
        else:
            signing.sign_release_manifest(
                tmp_path / "manifest",
                tmp_path / "signer",
                tmp_path / "raw-public-key",
                "0" * 64,
                tmp_path / "signature-output",
                tmp_path / "public-key-output",
                tmp_path / "verifier",
                "1" * 64,
            )


@pytest.mark.parametrize(
    ("raw_uid", "raw_gid", "message"),
    [
        (None, "41", "incomplete"),
        ("41", None, "incomplete"),
        ("0", "41", "positive canonical"),
        ("41", "0", "positive canonical"),
        ("041", "42", "positive canonical"),
        ("+41", "42", "noncanonical"),
        ("41 ", "42", "noncanonical"),
        ("４１", "42", "noncanonical"),
    ],
)
def test_external_tool_identity_rejects_incomplete_zero_and_noncanonical_ids(
    monkeypatch: pytest.MonkeyPatch,
    raw_uid: str | None,
    raw_gid: str | None,
    message: str,
) -> None:
    monkeypatch.setattr(signing.os, "geteuid", lambda: 0)
    monkeypatch.setattr(signing.os, "getegid", lambda: 0)
    for name, value in (
        (signing.EXTERNAL_TOOL_UID_ENV, raw_uid),
        (signing.EXTERNAL_TOOL_GID_ENV, raw_gid),
    ):
        if value is None:
            monkeypatch.delenv(name, raising=False)
        else:
            monkeypatch.setenv(name, value)

    with pytest.raises(signing.ReleaseManifestSignatureError, match=message):
        signing._external_tool_execution_identity()


def test_external_tool_identity_is_exact_for_root_and_matching_non_root(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv(signing.EXTERNAL_TOOL_UID_ENV, "41")
    monkeypatch.setenv(signing.EXTERNAL_TOOL_GID_ENV, "42")
    monkeypatch.setattr(signing.os, "geteuid", lambda: 0)
    monkeypatch.setattr(signing.os, "getegid", lambda: 0)
    assert signing._external_tool_execution_identity() == (41, 42)

    monkeypatch.setattr(signing.os, "geteuid", lambda: 41)
    monkeypatch.setattr(signing.os, "getegid", lambda: 42)
    assert signing._external_tool_execution_identity() is None

    monkeypatch.setenv(signing.EXTERNAL_TOOL_UID_ENV, "43")
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="differs from the current non-root identity",
    ):
        signing._external_tool_execution_identity()


def test_external_tool_privilege_drop_clears_groups_and_reaches_exact_ids(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    state: dict[str, object] = {"uid": 0, "gid": 0, "groups": [7, 8]}
    calls: list[tuple[str, object]] = []
    monkeypatch.setattr(signing.os, "geteuid", lambda: state["uid"])
    monkeypatch.setattr(signing.os, "getegid", lambda: state["gid"])
    monkeypatch.setattr(signing.os, "getgroups", lambda: list(state["groups"]))

    def setgroups(groups: list[int]) -> None:
        calls.append(("setgroups", groups))
        state["groups"] = list(groups)

    def setgid(gid: int) -> None:
        calls.append(("setgid", gid))
        state["gid"] = gid

    def setuid(uid: int) -> None:
        calls.append(("setuid", uid))
        state["uid"] = uid

    monkeypatch.setattr(signing.os, "setgroups", setgroups)
    monkeypatch.setattr(signing.os, "setgid", setgid)
    monkeypatch.setattr(signing.os, "setuid", setuid)
    monkeypatch.setattr(
        signing.os,
        "umask",
        lambda mask: calls.append(("umask", mask)) or 0o022,
    )

    signing._drop_external_tool_identity(41, 42)

    assert calls == [
        ("setgroups", []),
        ("setgid", 42),
        ("setuid", 41),
        ("umask", 0o077),
    ]
    assert state == {"uid": 41, "gid": 42, "groups": []}


def test_external_tool_invocations_have_exact_argv_environment_and_fd_contract(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[list[str], dict[str, object]]] = []
    preexec = lambda: None
    monkeypatch.setenv("TAIRA_EXTERNAL_TOOL_INJECTION", "must-not-propagate")
    monkeypatch.setattr(
        signing,
        "_external_tool_preexec",
        lambda identity: preexec if identity == (41, 42) else pytest.fail(
            "wrong child identity"
        ),
    )

    def run(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        calls.append((argv, kwargs))
        return subprocess.CompletedProcess(argv, 0)

    monkeypatch.setattr(signing.subprocess, "run", run)
    verifier = tmp_path / "verifier"
    manifest = tmp_path / "manifest"
    public_key = tmp_path / "public-key"
    signature = tmp_path / "signature"
    signing._invoke_native_verifier(
        verifier,
        manifest,
        public_key,
        "a" * 64,
        signature,
        (41, 42),
    )
    signer = tmp_path / "signer"
    private_home = tmp_path / "private-home"
    signing._invoke_external_signer(
        signer,
        manifest,
        signature,
        private_home,
        (41, 42),
    )

    verifier_argv, verifier_kwargs = calls[0]
    assert verifier_argv == [
        str(verifier),
        "release-manifest",
        "--manifest",
        str(manifest),
        "--public-key",
        str(public_key),
        "--public-key-fingerprint",
        "a" * 64,
        "--signature",
        str(signature),
    ]
    assert verifier_kwargs["env"] == signing._native_verifier_environment()
    assert verifier_kwargs["cwd"] == str(verifier.parent)

    signer_argv, signer_kwargs = calls[1]
    assert signer_argv == [str(signer), str(manifest), str(signature)]
    assert signer_kwargs["env"] == signing._external_signer_environment(private_home)
    assert signer_kwargs["cwd"] == str(private_home)

    for kwargs in (verifier_kwargs, signer_kwargs):
        assert kwargs["stdin"] is subprocess.DEVNULL
        assert kwargs["stdout"] is subprocess.DEVNULL
        assert kwargs["stderr"] is subprocess.DEVNULL
        assert kwargs["check"] is False
        assert kwargs["timeout"] == 120
        assert kwargs["close_fds"] is True
        assert kwargs["pass_fds"] == ()
        assert kwargs["restore_signals"] is True
        assert kwargs["preexec_fn"] is preexec
        assert "TAIRA_EXTERNAL_TOOL_INJECTION" not in kwargs["env"]


@pytest.mark.skipif(os.name == "nt", reason="preexec_fn is POSIX-only")
def test_external_tool_preexec_failure_is_fail_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_before_exec() -> None:
        raise RuntimeError("injected privilege-drop failure")

    monkeypatch.setattr(
        signing,
        "_external_tool_preexec",
        lambda _identity: fail_before_exec,
    )
    with pytest.raises(
        signing.ReleaseManifestSignatureError,
        match="cannot execute native release-manifest verifier",
    ):
        signing._invoke_native_verifier(
            Path(sys.executable),
            tmp_path / "manifest",
            tmp_path / "public-key",
            "a" * 64,
            tmp_path / "signature",
            (41, 42),
        )


@pytest.mark.skipif(os.name == "nt", reason="POSIX identity observation")
def test_signer_and_verifier_children_observe_the_requested_identity() -> None:
    execution_identity: tuple[int, int] | None = None
    expected_uid = os.geteuid()
    expected_gid = os.getegid()
    if expected_uid == 0:
        expected_uid = 65_534
        expected_gid = 65_534
        execution_identity = (expected_uid, expected_gid)

    with tempfile.TemporaryDirectory(
        prefix="iroha-external-tool-identity-test-"
    ) as temp_raw:
        temp_dir = Path(temp_raw)
        if execution_identity is not None:
            os.chown(temp_dir, expected_uid, expected_gid)
            temp_dir.chmod(0o700)

        observations: list[Path] = []
        probes: list[Path] = []
        for name in ("verifier", "signer"):
            observation = temp_dir / f"{name}-identity.json"
            probe = temp_dir / name
            _write_executable(
                probe,
                "import json\n"
                "import os\n"
                "from pathlib import Path\n"
                f"Path({str(observation)!r}).write_text(json.dumps({{"
                "'uid': os.geteuid(), 'gid': os.getegid(), "
                "'groups': os.getgroups()}), encoding='utf-8')\n",
            )
            if execution_identity is not None:
                os.chown(probe, expected_uid, expected_gid)
                probe.chmod(0o500)
            observations.append(observation)
            probes.append(probe)

        signing._invoke_native_verifier(
            probes[0],
            temp_dir / "manifest",
            temp_dir / "public-key",
            "a" * 64,
            temp_dir / "signature",
            execution_identity,
        )
        signing._invoke_external_signer(
            probes[1],
            temp_dir / "manifest",
            temp_dir / "signature",
            temp_dir,
            execution_identity,
        )

        for observation in observations:
            observed = json.loads(observation.read_text(encoding="utf-8"))
            assert (observed["uid"], observed["gid"]) == (
                expected_uid,
                expected_gid,
            )
            if execution_identity is not None:
                assert observed["groups"] == []
