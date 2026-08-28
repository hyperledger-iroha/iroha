"""Owner-controlled Android authority tools and signed APK test fixtures."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import ModuleType
import unittest
import zipfile


_SIGNED_CANDIDATE_APK_FIXTURE: tuple[bytes, bytes, str] | None = None
_SIGNED_WALLET_APK_FIXTURE: tuple[bytes, str] | None = None


def _java_tool_environment(authority: Path) -> dict[str, str]:
    """Return a fixed environment without ambient JVM or loader injection."""

    tool_home = authority / "tool-home"
    tool_tmp = authority / "tool-tmp"
    for directory in (tool_home, tool_tmp):
        directory.mkdir(mode=0o700, exist_ok=True)
        if directory.is_symlink() or not directory.is_dir():
            raise AssertionError(
                f"private Android authority working path is invalid: {directory}"
            )
        directory.chmod(0o700)
    return {
        "HOME": str(tool_home),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": str(tool_tmp),
    }


def stage_private_android_authority_tools(
    authority: Path,
    source_java: Path,
    source_apksigner_jar: Path,
) -> tuple[Path, Path]:
    """Stage owner-controlled Java and apksigner inputs for authority tests."""

    jlink = source_java.parent / "jlink"
    if not jlink.is_file() or not os.access(jlink, os.X_OK):
        raise unittest.SkipTest("jlink is required for Android authority tests")

    runtime = authority / "java-runtime"
    completed = subprocess.run(
        [
            str(jlink),
            "--add-modules",
            "java.base,java.logging",
            "--output",
            str(runtime),
            "--no-header-files",
            "--no-man-pages",
            "--strip-debug",
        ],
        env=_java_tool_environment(authority),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "failed to stage the private Android authority Java runtime: "
            f"{completed.stderr.strip()}"
        )

    runtime.chmod(0o700)
    for entry in sorted(runtime.rglob("*")):
        if entry.is_symlink():
            target = entry.resolve(strict=True)
            if not target.is_file() or runtime not in target.parents:
                raise AssertionError(
                    "private Android authority Java runtime contains an invalid "
                    f"symlink: {entry}"
                )
            payload = target.read_bytes()
            entry.unlink()
            entry.write_bytes(payload)
        if entry.is_dir():
            entry.chmod(0o700)
        elif entry.is_file():
            executable = entry.stat().st_mode & 0o111
            entry.chmod(0o700 if executable else 0o600)
        else:
            raise AssertionError(
                f"private Android authority Java runtime has a special entry: {entry}"
            )

    java = runtime / "bin" / "java"
    keytool = runtime / "bin" / "keytool"
    if not java.is_file() or not os.access(java, os.X_OK):
        raise AssertionError("private Android authority Java launcher is unavailable")
    if not keytool.is_file() or not os.access(keytool, os.X_OK):
        raise AssertionError("private Android authority keytool is unavailable")

    apksigner_jar = authority / "apksigner.jar"
    source_digest_before = hashlib.sha256(source_apksigner_jar.read_bytes()).hexdigest()
    shutil.copyfile(source_apksigner_jar, apksigner_jar)
    apksigner_jar.chmod(0o600)
    source_digest_after = hashlib.sha256(source_apksigner_jar.read_bytes()).hexdigest()
    staged_digest = hashlib.sha256(apksigner_jar.read_bytes()).hexdigest()
    if source_digest_before != source_digest_after or staged_digest != source_digest_before:
        raise AssertionError("private Android authority apksigner copy is not exact")

    apksigner_probe = subprocess.run(
        [str(java), "-jar", str(apksigner_jar), "--version"],
        env=_java_tool_environment(authority),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if apksigner_probe.returncode != 0:
        raise AssertionError(
            "private Android authority Java runtime cannot execute apksigner: "
            f"{apksigner_probe.stderr.strip()}"
        )
    return java.resolve(strict=True), apksigner_jar.resolve(strict=True)


def signed_candidate_apk_fixture(device_lab: ModuleType) -> tuple[bytes, bytes, str]:
    """Create and cache the pair of signed candidate APK fixtures."""

    global _SIGNED_CANDIDATE_APK_FIXTURE
    if _SIGNED_CANDIDATE_APK_FIXTURE is not None:
        return _SIGNED_CANDIDATE_APK_FIXTURE
    sdk_roots = tuple(
        dict.fromkeys(
            Path(value).expanduser()
            for value in (
                os.environ.get("ANDROID_SDK_ROOT"),
                os.environ.get("ANDROID_HOME"),
                str(Path.home() / "Library" / "Android" / "sdk"),
            )
            if value
        )
    )
    authority = device_lab._ANDROID_EVIDENCE_AUTHORITY  # type: ignore[attr-defined]
    if authority is None:
        raise AssertionError("Android authority is not configured")
    java = Path(authority["java"]["path"]).resolve(strict=True)
    apksigner_jar = Path(authority["apksigner_jar"]["path"]).resolve(strict=True)
    authority_root = apksigner_jar.parent
    if java.parent.parent.parent != authority_root:
        raise AssertionError("Android authority Java and apksigner roots differ")
    apksigner_command = [
        os.fspath(java),
        "-jar",
        os.fspath(apksigner_jar),
    ]
    java_environment = _java_tool_environment(authority_root)
    aapt2 = shutil.which("aapt2")
    if aapt2 is None:
        candidates = sorted(
            candidate
            for sdk_root in sdk_roots
            for candidate in (sdk_root / "build-tools").glob("*/aapt2")
            if candidate.is_file() and os.access(candidate, os.X_OK)
        )
        if candidates:
            aapt2 = str(candidates[-1])
    android_jars = sorted(
        candidate
        for sdk_root in sdk_roots
        for candidate in (sdk_root / "platforms").glob("android-*/android.jar")
        if candidate.is_file()
    )
    keytool = java.with_name("keytool")
    if aapt2 is None or not android_jars or not keytool.is_file():
        raise AssertionError(
            "candidate APK validator tests require aapt2, android.jar, and keytool"
        )
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        manifest = root / "AndroidManifest.xml"
        manifest.write_text(
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            'package="org.hyperledger.iroha.candidate.fixture">\n'
            '  <uses-sdk android:minSdkVersion="28" android:targetSdkVersion="35"/>\n'
            '  <application android:hasCode="false"/>\n'
            '</manifest>\n',
            encoding="utf-8",
        )
        keystore = root / "candidate-lab-test.p12"
        subprocess.run(
            [
                str(keytool),
                "-genkeypair",
                "-alias",
                "candidate-lab",
                "-keystore",
                str(keystore),
                "-storetype",
                "PKCS12",
                "-storepass",
                "candidate-lab-test",
                "-keypass",
                "candidate-lab-test",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "3650",
                "-dname",
                "CN=Iroha Candidate Lab Test",
            ],
            env=java_environment,
            check=True,
            capture_output=True,
        )
        signed_payloads: list[bytes] = []
        signed_paths: list[Path] = []
        for label in ("main", "androidTest"):
            unsigned = root / f"{label}-unsigned.apk"
            signed = root / f"{label}.apk"
            subprocess.run(
                [
                    aapt2,
                    "link",
                    "--manifest",
                    str(manifest),
                    "-I",
                    str(android_jars[-1]),
                    "-o",
                    str(unsigned),
                ],
                check=True,
                capture_output=True,
            )
            with zipfile.ZipFile(
                unsigned, "a", compression=zipfile.ZIP_STORED
            ) as archive:
                archive.writestr("fixture.txt", f"candidate-lab-{label}\n")
            subprocess.run(
                [
                    *apksigner_command,
                    "sign",
                    "--min-sdk-version",
                    "28",
                    "--ks",
                    str(keystore),
                    "--ks-pass",
                    "pass:candidate-lab-test",
                    "--key-pass",
                    "pass:candidate-lab-test",
                    "--out",
                    str(signed),
                    str(unsigned),
                ],
                env=java_environment,
                check=True,
                capture_output=True,
            )
            signed_payloads.append(signed.read_bytes())
            signed_paths.append(signed)
        certificate_sha256 = device_lab.extract_apk_signing_certificate_sha256(
            signed_paths[0]
        )
        if (
            device_lab.extract_apk_signing_certificate_sha256(signed_paths[1])
            != certificate_sha256
        ):
            raise AssertionError("candidate APK fixture signers differ")
        _SIGNED_CANDIDATE_APK_FIXTURE = (
            signed_payloads[0],
            signed_payloads[1],
            certificate_sha256,
        )
    return _SIGNED_CANDIDATE_APK_FIXTURE


def signed_wallet_apk_fixture(device_lab: ModuleType) -> tuple[bytes, str]:
    """Create and cache the signed production-wallet APK fixture."""

    global _SIGNED_WALLET_APK_FIXTURE
    if _SIGNED_WALLET_APK_FIXTURE is not None:
        return _SIGNED_WALLET_APK_FIXTURE
    authority = device_lab._ANDROID_EVIDENCE_AUTHORITY  # type: ignore[attr-defined]
    if authority is None:
        raise AssertionError("Android authority is not configured")
    java = Path(authority["java"]["path"]).resolve(strict=True)
    apksigner_jar = Path(authority["apksigner_jar"]["path"]).resolve(strict=True)
    authority_root = apksigner_jar.parent
    if java.parent.parent.parent != authority_root:
        raise AssertionError("Android authority Java and apksigner roots differ")
    keytool = java.with_name("keytool")
    java_environment = _java_tool_environment(authority_root)
    sdk_roots = tuple(
        Path(value).expanduser()
        for value in (
            os.environ.get("ANDROID_SDK_ROOT"),
            os.environ.get("ANDROID_HOME"),
            str(Path.home() / "Library" / "Android" / "sdk"),
        )
        if value
    )
    aapt_candidates = sorted(
        candidate
        for sdk_root in sdk_roots
        for candidate in (sdk_root / "build-tools").glob("*/aapt2")
        if candidate.is_file() and os.access(candidate, os.X_OK)
    )
    android_jars = sorted(
        candidate
        for sdk_root in sdk_roots
        for candidate in (sdk_root / "platforms").glob("android-*/android.jar")
        if candidate.is_file()
    )
    if not keytool.is_file() or not aapt_candidates or not android_jars:
        raise AssertionError("wallet APK fixture requires keytool, aapt2, and android.jar")
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        unsigned = root / "wallet-unsigned.apk"
        signed = root / "wallet-signed.apk"
        manifest = root / "AndroidManifest.xml"
        manifest.write_text(
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            'package="org.hyperledger.iroha.kagemushawallet">\n'
            '  <uses-sdk android:minSdkVersion="28" android:targetSdkVersion="35"/>\n'
            '  <application android:hasCode="false"/>\n'
            '</manifest>\n',
            encoding="utf-8",
        )
        subprocess.run(
            [
                str(aapt_candidates[-1]),
                "link",
                "--manifest",
                str(manifest),
                "-I",
                str(android_jars[-1]),
                "-o",
                str(unsigned),
            ],
            check=True,
            capture_output=True,
        )
        with zipfile.ZipFile(unsigned, "a", compression=zipfile.ZIP_STORED) as archive:
            archive.writestr("fixture.txt", "production-wallet-fixture\n")
        keystore = root / "wallet-test.p12"
        subprocess.run(
            [
                str(keytool),
                "-genkeypair",
                "-alias",
                "wallet",
                "-keystore",
                str(keystore),
                "-storetype",
                "PKCS12",
                "-storepass",
                "wallet-test",
                "-keypass",
                "wallet-test",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "3650",
                "-dname",
                "CN=Iroha Wallet Test",
            ],
            env=java_environment,
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                str(java),
                "-jar",
                str(apksigner_jar),
                "sign",
                "--min-sdk-version",
                "28",
                "--ks",
                str(keystore),
                "--ks-pass",
                "pass:wallet-test",
                "--key-pass",
                "pass:wallet-test",
                "--out",
                str(signed),
                str(unsigned),
            ],
            env=java_environment,
            check=True,
            capture_output=True,
        )
        certificate = device_lab.extract_apk_signing_certificate_sha256(signed)
        _SIGNED_WALLET_APK_FIXTURE = (signed.read_bytes(), certificate)
    return _SIGNED_WALLET_APK_FIXTURE
