#!/usr/bin/env python3
"""Tests for exact ABI-21 byte authentication in Android app archives."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile


SCRIPT = Path(__file__).resolve().parents[1] / "check_android_app_native_archive.py"
ABIS = ("arm64-v8a", "x86_64")
LIBRARY_NAME = "libconnect_norito_bridge.so"
PROVENANCE_ENTRY = "assets/iroha/native-build-provenance-v1.json"


class AndroidAppNativeArchiveTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name).resolve()
        self.iroha_root = self.root / "iroha-source"
        self.artifact_root = self.root / "android-artifacts"
        self.client_root = (
            self.artifact_root
            / "gradle-build"
            / "iroha_kotlin_sdk"
            / "client-android"
        )
        self.iroha_root.mkdir()
        self.client_root.mkdir(parents=True)
        self.libraries = {
            abi: f"\x7fELF-authenticated-{abi}\n".encode("ascii")
            for abi in ABIS
        }
        for abi, payload in self.libraries.items():
            path = (
                self.client_root
                / "generated"
                / "jniLibs"
                / "production"
                / abi
                / LIBRARY_NAME
            )
            path.parent.mkdir(parents=True)
            path.write_bytes(payload)

        provenance = {
            "schema": "iroha.android-native-build-provenance.v1",
            "native_bridge_abi_version": 21,
            "build_profile": "release",
            "cargo_locked": True,
            "privacy_production_enabled": True,
            "cargo_features": ["privacy-production-enabled"],
            "build_environment": {
                "schema": "iroha.mobile-native-build-environment.v1",
            },
            "source_commit": "1" * 40,
            "source_tree_dirty": False,
            "source_fingerprint_sha256": "2" * 64,
            "cargo_lock_sha256": "3" * 64,
            "android_ndk_revision": "28.0.12674087",
            "strip_tool_sha256": "4" * 64,
            "libraries": {
                abi: {
                    "aar_path": f"jni/{abi}/{LIBRARY_NAME}",
                    "bytes": len(payload),
                    "raw_bytes": len(payload) + 1,
                    "raw_sha256": "5" * 64,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                }
                for abi, payload in self.libraries.items()
            },
        }
        self.provenance_bytes = (
            json.dumps(provenance, sort_keys=True, separators=(",", ":")) + "\n"
        ).encode("utf-8")
        provenance_path = (
            self.client_root
            / "generated"
            / "nativeProvenance"
            / "production"
            / "iroha"
            / "native-build-provenance-v1.json"
        )
        provenance_path.parent.mkdir(parents=True)
        provenance_path.write_bytes(self.provenance_bytes)

        self.aar_path = (
            self.client_root / "outputs" / "aar" / "client-android-release.aar"
        )
        self.aar_path.parent.mkdir(parents=True)
        self.write_aar()

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    @staticmethod
    def write_zip(
        path: Path,
        entries: list[tuple[str, bytes]],
    ) -> None:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(path, "w") as archive:
                for name, payload in entries:
                    archive.writestr(name, payload)

    def write_aar(
        self,
        *,
        extra_entries: dict[str, bytes] | None = None,
        duplicate_entries: list[tuple[str, bytes]] | None = None,
    ) -> None:
        entries = [
            ("AndroidManifest.xml", b"<manifest />\n"),
            (PROVENANCE_ENTRY, self.provenance_bytes),
            *[
                (f"jni/{abi}/{LIBRARY_NAME}", payload)
                for abi, payload in self.libraries.items()
            ],
            *(extra_entries or {}).items(),
            *(duplicate_entries or []),
        ]
        self.write_zip(self.aar_path, entries)

    def write_app_archive(
        self,
        kind: str,
        *,
        overrides: dict[str, bytes] | None = None,
        extra_entries: dict[str, bytes] | None = None,
        duplicate_entries: list[tuple[str, bytes]] | None = None,
    ) -> Path:
        prefix = "base/" if kind == "aab" else ""
        entries = {
            f"{prefix}assets/iroha/native-build-provenance-v1.json":
                self.provenance_bytes,
            **{
                f"{prefix}lib/{abi}/{LIBRARY_NAME}": payload
                for abi, payload in self.libraries.items()
            },
        }
        entries.update(overrides or {})
        entries.update(extra_entries or {})
        path = self.root / f"wallet-release.{kind}"
        self.write_zip(path, [*entries.items(), *(duplicate_entries or [])])
        return path

    def verify(self, archive: Path, kind: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-I",
                str(SCRIPT),
                "--archive",
                str(archive.resolve(strict=True)),
                "--kind",
                kind,
                "--artifact-dir",
                str(self.artifact_root.resolve(strict=True)),
                "--iroha-root",
                str(self.iroha_root.resolve(strict=True)),
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_exact_aab_and_apk_bytes(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(kind=kind):
                result = self.verify(self.write_app_archive(kind), kind)
                self.assertEqual(0, result.returncode, result.stderr)

    def test_accepts_canonical_native_directory_entries(self) -> None:
        self.write_aar(
            extra_entries={
                "jni/": b"",
                **{f"jni/{abi}/": b"" for abi in ABIS},
            },
        )
        for kind in ("aab", "apk"):
            with self.subTest(kind=kind):
                prefix = "base/" if kind == "aab" else ""
                result = self.verify(
                    self.write_app_archive(
                        kind,
                        extra_entries={
                            f"{prefix}lib/": b"",
                            **{f"{prefix}lib/{abi}/": b"" for abi in ABIS},
                        },
                    ),
                    kind,
                )
                self.assertEqual(0, result.returncode, result.stderr)

    def test_rejects_tampered_app_bridge(self) -> None:
        archive = self.write_app_archive(
            "aab",
            overrides={
                f"base/lib/{ABIS[0]}/{LIBRARY_NAME}": b"\x7fELF-tampered\n",
            },
        )
        result = self.verify(archive, "aab")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("differs from the authenticated generated/AAR bytes", result.stderr)

    def test_rejects_extra_bridge_abi(self) -> None:
        archive = self.write_app_archive(
            "apk",
            extra_entries={
                f"lib/armeabi-v7a/{LIBRARY_NAME}": b"\x7fELF-extra\n",
            },
        )
        result = self.verify(archive, "apk")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("native ABI directory inventory is not exact", result.stderr)

    def test_rejects_unrelated_library_under_unsupported_abi(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                archive = self.write_app_archive(
                    kind,
                    extra_entries={
                        f"{prefix}lib/armeabi-v7a/libunrelated.so": b"\x7fELF\n",
                    },
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("native ABI directory inventory is not exact", result.stderr)

        with self.subTest(archive="aar"):
            self.write_aar(
                extra_entries={
                    "jni/armeabi-v7a/libunrelated.so": b"\x7fELF\n",
                },
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("native ABI directory inventory is not exact", result.stderr)

    def test_rejects_unrelated_library_under_supported_abi(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                archive = self.write_app_archive(
                    kind,
                    extra_entries={
                        f"{prefix}lib/{ABIS[0]}/libunrelated.so": b"\x7fELF\n",
                    },
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("native library inventory is not exact", result.stderr)

        with self.subTest(archive="aar"):
            self.write_aar(
                extra_entries={
                    f"jni/{ABIS[0]}/libunrelated.so": b"\x7fELF\n",
                },
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("native library inventory is not exact", result.stderr)

    def test_rejects_nested_native_library_paths(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                archive = self.write_app_archive(
                    kind,
                    extra_entries={
                        f"{prefix}lib/{ABIS[0]}/nested/libunrelated.so":
                            b"\x7fELF\n",
                    },
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("malformed or nested native-library entry", result.stderr)

        with self.subTest(archive="aar"):
            self.write_aar(
                extra_entries={
                    f"jni/{ABIS[0]}/nested/libunrelated.so": b"\x7fELF\n",
                },
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("malformed or nested native-library entry", result.stderr)

    def test_rejects_noncanonical_native_library_paths(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                archive = self.write_app_archive(
                    kind,
                    extra_entries={
                        f"{prefix}lib//{ABIS[0]}/libunrelated.so": b"\x7fELF\n",
                    },
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("non-canonical ZIP path", result.stderr)

        with self.subTest(archive="aar"):
            self.write_aar(
                extra_entries={
                    f"jni//{ABIS[0]}/libunrelated.so": b"\x7fELF\n",
                },
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("non-canonical ZIP path", result.stderr)

    def test_rejects_native_library_path_traversal(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                archive = self.write_app_archive(
                    kind,
                    extra_entries={
                        f"{prefix}lib/{ABIS[0]}/../armeabi-v7a/libunrelated.so":
                            b"\x7fELF\n",
                    },
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("non-canonical ZIP path", result.stderr)

        with self.subTest(archive="aar"):
            self.write_aar(
                extra_entries={
                    f"jni/{ABIS[0]}/../armeabi-v7a/libunrelated.so": b"\x7fELF\n",
                },
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("non-canonical ZIP path", result.stderr)

    def test_rejects_duplicate_native_library_entries(self) -> None:
        for kind in ("aab", "apk"):
            with self.subTest(archive=kind):
                prefix = "base/" if kind == "aab" else ""
                duplicate_name = f"{prefix}lib/{ABIS[0]}/{LIBRARY_NAME}"
                archive = self.write_app_archive(
                    kind,
                    duplicate_entries=[(duplicate_name, self.libraries[ABIS[0]])],
                )
                result = self.verify(archive, kind)
                self.assertNotEqual(0, result.returncode)
                self.assertIn("duplicate ZIP entries", result.stderr)

        with self.subTest(archive="aar"):
            duplicate_name = f"jni/{ABIS[0]}/{LIBRARY_NAME}"
            self.write_aar(
                duplicate_entries=[(duplicate_name, self.libraries[ABIS[0]])],
            )
            result = self.verify(self.write_app_archive("aab"), "aab")
            self.assertNotEqual(0, result.returncode)
            self.assertIn("duplicate ZIP entries", result.stderr)

    def test_rejects_aar_provenance_that_differs_from_generated_bytes(self) -> None:
        with zipfile.ZipFile(self.aar_path) as source:
            entries = {
                info.filename: source.read(info)
                for info in source.infolist()
            }
        entries[PROVENANCE_ENTRY] += b" "
        with zipfile.ZipFile(self.aar_path, "w") as output:
            for name, payload in entries.items():
                output.writestr(name, payload)

        result = self.verify(self.write_app_archive("aab"), "aab")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("AAR provenance differs from generated provenance", result.stderr)


if __name__ == "__main__":
    unittest.main()
