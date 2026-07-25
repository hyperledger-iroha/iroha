from __future__ import annotations

import hashlib
import json
import os
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
        match="changed during verification",
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
