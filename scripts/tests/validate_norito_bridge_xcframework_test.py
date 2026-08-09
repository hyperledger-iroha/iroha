#!/usr/bin/env python3
"""Tests for the shared strict NoritoBridge inventory validator."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import plistlib
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/validate_norito_bridge_xcframework.py"
SPEC = importlib.util.spec_from_file_location("validate_norito_bridge_xcframework", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = validator
SPEC.loader.exec_module(validator)


class StrictNoritoBridgeValidatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.artifact_root = Path(self.temporary.name).resolve() / "artifact"
        self.xcframework = self.artifact_root / "NoritoBridge.xcframework"
        self.xcframework.mkdir(parents=True)
        self.hashes: dict[str, str] = {}
        header = (
            ROOT / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
        ).read_bytes()
        libraries = []
        for identifier, expected in validator.EXPECTED_SLICES.items():
            headers = self.xcframework / identifier / "Headers"
            headers.mkdir(parents=True)
            binary = self.xcframework / identifier / validator.LIBRARY_NAME
            binary.write_bytes((identifier + "\n").encode("ascii"))
            self.hashes[identifier] = hashlib.sha256(binary.read_bytes()).hexdigest()
            (headers / "NoritoBridge.h").write_bytes(
                (
                    ROOT
                    / "crates/connect_norito_bridge/include/NoritoBridge.h"
                ).read_bytes()
            )
            (headers / "connect_norito_bridge.h").write_bytes(header)
            (headers / "module.modulemap").write_bytes(
                (
                    ROOT
                    / "crates/connect_norito_bridge/module.modulemap.template"
                ).read_bytes()
            )
            library = {
                "LibraryIdentifier": identifier,
                "LibraryPath": validator.LIBRARY_NAME,
                "HeadersPath": "Headers",
                "SupportedArchitectures": expected["architectures"],
                "SupportedPlatform": expected["platform"],
            }
            if expected["variant"] is not None:
                library["SupportedPlatformVariant"] = expected["variant"]
            libraries.append(library)
        with (self.xcframework / "Info.plist").open("wb") as output:
            plistlib.dump(
                {
                    "AvailableLibraries": libraries,
                    "CFBundlePackageType": "XFWK",
                    "XCFrameworkFormatVersion": "1.0",
                },
                output,
            )
        self.manifest = self.xcframework / validator.MANIFEST_NAME
        exact_hash = "a" * 64
        rust_commit = "b" * 40
        self.payload = {
            "version": "1.0.0",
            "native_bridge_abi_version": 22,
            "privacy_production_enabled": False,
            "cargo_features": [],
            "build_environment": {
                "schema": "iroha.mobile-native-build-environment.v1",
                "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
                "hermetic_runner_sha256": hashlib.sha256(
                    (ROOT / "scripts/run_mobile_hermetic_command.py").read_bytes()
                ).hexdigest(),
                "environment_profiles": validator.EXPECTED_ENVIRONMENT_PROFILES,
                "cargo_build_jobs": 1,
                "rust_toolchain_channel": "1.93.1",
                "cargo_release": "1.93.1",
                "cargo_commit_hash": "c" * 40,
                "cargo_binary_sha256": exact_hash,
                "rustc_release": "1.93.1",
                "rustc_commit_hash": rust_commit,
                "rustc_binary_sha256": exact_hash,
                "rustdoc_release": "1.93.1",
                "rustdoc_commit_hash": rust_commit,
                "rustdoc_binary_sha256": exact_hash,
                "python_version": "3.12.13",
                "python_binary_sha256": exact_hash,
                "git_version": "2.50.1",
                "git_binary_sha256": exact_hash,
                "rustup_version": "1.28.2",
                "rustup_binary_sha256": exact_hash,
                "xcode_version": "26.5",
                "xcode_build_version": "17F12",
                "iphoneos_sdk_version": "26.5",
                "iphonesimulator_sdk_version": "26.5",
                "macosx_sdk_version": "26.5",
                "iphoneos_deployment_target": "15.0",
                "iphonesimulator_deployment_target": "15.0",
                "macosx_deployment_target": "12.0",
            },
            "source_commit": "1" * 40,
            "source_tree_dirty": False,
            "source_fingerprint_sha256": "2" * 64,
            "cargo_lock_sha256": hashlib.sha256(
                (ROOT / "Cargo.lock").read_bytes()
            ).hexdigest(),
            "bridge_header_sha256": hashlib.sha256(header).hexdigest(),
            "required_symbols": list(validator.EXPECTED_REQUIRED_SYMBOLS),
            "forbidden_symbols": list(validator.EXPECTED_FORBIDDEN_SYMBOLS),
            "kagemusha_mobile_artifact_roles": validator.expected_kagemusha_roles(
                False
            ),
            "hashes": self.hashes,
        }
        self.write_manifest()
        self.manifest_link = self.artifact_root / validator.MANIFEST_NAME
        self.manifest_link.symlink_to(
            "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
        )
        self.loader = Path(self.temporary.name) / "NativeBridge.swift"
        self.write_loader(self.hashes)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_manifest(self) -> None:
        self.manifest.write_text(
            json.dumps(self.payload, indent=2) + "\n", encoding="utf-8"
        )

    def write_loader(self, hashes: dict[str, str]) -> None:
        self.loader.write_text(
            "    private static let expectedHashes: [String: String] = [\n"
            + "\n".join(
                f'        "{key}": "{hashes[key]}"{"," if index < 2 else ""}'
                for index, key in enumerate(
                    (
                        "macos-arm64_x86_64",
                        "ios-arm64",
                        "ios-arm64_x86_64-simulator",
                    )
                )
            )
            + "\n    ]\n",
            encoding="utf-8",
        )

    def validate(self) -> None:
        validator.validate(
            root=ROOT,
            xcframework=self.xcframework,
            manifest_path=self.manifest,
            manifest_link=self.manifest_link,
            expected_link_target="NoritoBridge.xcframework/NoritoBridge.artifacts.json",
            swift_loader=self.loader,
        )

    def test_accepts_only_the_canonical_inventory(self) -> None:
        self.validate()

    def test_rejects_unknown_manifest_fields_and_stale_pins(self) -> None:
        self.payload["legacy_hash"] = "4" * 64
        self.write_manifest()
        with self.assertRaisesRegex(validator.ValidationError, "field inventory"):
            self.validate()
        del self.payload["legacy_hash"]
        self.write_manifest()
        stale = dict(self.hashes)
        stale["ios-arm64"] = "5" * 64
        self.write_loader(stale)
        with self.assertRaisesRegex(validator.ValidationError, "pins are stale"):
            self.validate()

    def test_decoy_pin_dictionary_cannot_replace_expected_hashes(self) -> None:
        contents = self.loader.read_text(encoding="utf-8")
        decoy = contents.replace(
            "    private static let expectedHashes: [String: String] = [",
            "let decoyHashes = [",
        ).replace("        ", "    ").replace("    ]", "]")
        self.loader.write_text(
            contents.replace(
                "private static let expectedHashes",
                "private static let retiredHashes",
            )
            + decoy,
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            validator.ValidationError,
            "canonical expectedHashes block",
        ):
            self.validate()

    def test_rejects_extra_files_slices_and_symlinks(self) -> None:
        extra = self.xcframework / "historical.txt"
        extra.write_text("retired\n", encoding="utf-8")
        with self.assertRaisesRegex(validator.ValidationError, "inventory is not exact"):
            self.validate()
        extra.unlink()

        old_slice = self.xcframework / "ios-x86_64"
        old_slice.mkdir()
        with self.assertRaisesRegex(validator.ValidationError, "inventory is not exact"):
            self.validate()
        old_slice.rmdir()

        symbolic = self.xcframework / "alias"
        symbolic.symlink_to("Info.plist")
        with self.assertRaisesRegex(validator.ValidationError, "contains a symlink"):
            self.validate()

    def test_rejects_fabricated_environment_policy_and_source_identity(self) -> None:
        environment = self.payload["build_environment"]
        assert isinstance(environment, dict)
        environment["cargo_build_jobs"] = 2
        self.write_manifest()
        with self.assertRaisesRegex(validator.ValidationError, "environment identity"):
            self.validate()

        environment["cargo_build_jobs"] = 1
        self.payload["required_symbols"] = ["connect_norito_bridge_abi_version"]
        self.write_manifest()
        with self.assertRaisesRegex(validator.ValidationError, "required symbol"):
            self.validate()

        self.payload["required_symbols"] = list(validator.EXPECTED_REQUIRED_SYMBOLS)
        self.payload["kagemusha_mobile_artifact_roles"] = [{"role": "native_bridge"}]
        self.write_manifest()
        with self.assertRaisesRegex(validator.ValidationError, "role registry"):
            self.validate()

        self.payload["kagemusha_mobile_artifact_roles"] = (
            validator.expected_kagemusha_roles(False)
        )
        self.payload["cargo_lock_sha256"] = "3" * 64
        self.write_manifest()
        with self.assertRaisesRegex(validator.ValidationError, "Cargo.lock digest"):
            self.validate()

    def test_standalone_owner_recomputes_source_provenance(self) -> None:
        source_seal = types.SimpleNamespace(
            seal_inputs=lambda _root, _platform, _lock: ["Cargo.lock"],
            fingerprint=lambda _root, _inputs, _lock: "2" * 64,
            status=lambda _root, _inputs, _lock: "",
        )
        pin_commit = types.SimpleNamespace(
            validate_pin_relationship=lambda _root, _commit: "direct"
        )
        with (
            mock.patch.object(
                validator,
                "_load_repository_module",
                side_effect=(source_seal, pin_commit),
            ),
            mock.patch.object(validator, "_validate_tool_provenance"),
        ):
            validator._validate_repository_provenance(ROOT, self.payload)

        dirty_source_seal = types.SimpleNamespace(
            seal_inputs=lambda _root, _platform, _lock: ["Cargo.lock"],
            fingerprint=lambda _root, _inputs, _lock: "2" * 64,
            status=lambda _root, _inputs, _lock: " M Cargo.lock\n",
        )
        pin_parent = types.SimpleNamespace(
            validate_pin_relationship=lambda _root, _commit: "pin-parent"
        )
        self.payload["source_tree_dirty"] = True
        with (
            mock.patch.object(
                validator,
                "_load_repository_module",
                side_effect=(dirty_source_seal, pin_parent),
            ),
            mock.patch.object(validator, "_validate_tool_provenance"),
            self.assertRaisesRegex(
                validator.ValidationError, "clean authenticated source closure"
            ),
        ):
            validator._validate_repository_provenance(ROOT, self.payload)

        self.payload["source_tree_dirty"] = False

        self.payload["source_fingerprint_sha256"] = "4" * 64
        with (
            mock.patch.object(
                validator,
                "_load_repository_module",
                side_effect=(source_seal, pin_commit),
            ),
            mock.patch.object(validator, "_validate_tool_provenance"),
            self.assertRaisesRegex(validator.ValidationError, "fingerprint"),
        ):
            validator._validate_repository_provenance(ROOT, self.payload)

    def test_tool_provenance_accepts_exact_tools_and_rejects_identity_drift(self) -> None:
        tools_root = Path(self.temporary.name).resolve() / "tools"
        tools_root.mkdir()
        tool_paths = {}
        for name in ("cargo", "rustc", "rustdoc", "git", "rustup"):
            path = tools_root / name
            path.write_bytes((name + "\n").encode("ascii"))
            tool_paths[name] = path

        authenticated_tools = {
            name: types.SimpleNamespace(
                canonical=tool_paths[name],
                authenticate=mock.Mock(),
            )
            for name in ("cargo", "rustc", "rustdoc")
        }
        source_seal = types.SimpleNamespace(
            source_seal_tools=lambda: (
                authenticated_tools["cargo"],
                authenticated_tools["rustc"],
                authenticated_tools["rustdoc"],
                tool_paths["git"],
            )
        )
        environment = self.payload["build_environment"]
        assert isinstance(environment, dict)
        for name in ("cargo", "rustc", "rustdoc", "git", "rustup"):
            environment[f"{name}_binary_sha256"] = hashlib.sha256(
                tool_paths[name].read_bytes()
            ).hexdigest()
        python = Path(sys.executable).resolve(strict=True)
        environment["python_binary_sha256"] = hashlib.sha256(
            python.read_bytes()
        ).hexdigest()
        environment["python_version"] = validator.platform.python_version()
        actual_rust_identities = {
            "cargo": ("1.93.1", "c" * 40),
            "rustc": ("1.93.1", "b" * 40),
            "rustdoc": ("1.93.1", "b" * 40),
        }

        def tool_output(
            executable: Path, arguments: list[str], _environment: dict[str, str]
        ) -> str:
            if arguments == ["--version", "--verbose"]:
                release, commit = actual_rust_identities[executable.name]
                return f"release: {release}\ncommit-hash: {commit}\n"
            if executable == tool_paths["git"]:
                return f"git version {environment['git_version']}\n"
            if executable == tool_paths["rustup"]:
                return f"rustup {environment['rustup_version']}\n"
            if executable == Path("/usr/bin/xcodebuild"):
                return (
                    f"Xcode {environment['xcode_version']}\n"
                    f"Build version {environment['xcode_build_version']}\n"
                )
            sdk = arguments[1]
            return f"{environment[f'{sdk}_sdk_version']}\n"

        with (
            mock.patch.dict(
                validator.os.environ,
                {
                    "NORITO_BRIDGE_SEAL_RUSTUP": str(tool_paths["rustup"]),
                    "NORITO_BRIDGE_SEAL_DEVELOPER_DIR": str(tools_root),
                },
                clear=False,
            ),
            mock.patch.object(validator, "_tool_output", side_effect=tool_output),
        ):
            validator._validate_tool_provenance(self.payload, source_seal)

            environment["cargo_binary_sha256"] = "0" * 64
            with self.assertRaisesRegex(
                validator.ValidationError,
                "artifact build tool digest mismatch: cargo_binary_sha256",
            ):
                validator._validate_tool_provenance(self.payload, source_seal)
            environment["cargo_binary_sha256"] = hashlib.sha256(
                tool_paths["cargo"].read_bytes()
            ).hexdigest()

            environment["rustc_release"] = "1.93.0"
            with self.assertRaisesRegex(
                validator.ValidationError,
                "Rust tool identity",
            ):
                validator._validate_tool_provenance(self.payload, source_seal)

    def test_rejects_extra_plist_keys_and_copied_header_drift(self) -> None:
        info_path = self.xcframework / "Info.plist"
        with info_path.open("rb") as source:
            info = plistlib.load(source)
        info["LegacyCompatibility"] = True
        with info_path.open("wb") as output:
            plistlib.dump(info, output)
        with self.assertRaisesRegex(validator.ValidationError, "field inventory"):
            self.validate()

        del info["LegacyCompatibility"]
        info["AvailableLibraries"][0]["LegacyPath"] = "retired"
        with info_path.open("wb") as output:
            plistlib.dump(info, output)
        with self.assertRaisesRegex(validator.ValidationError, "field inventory"):
            self.validate()

        del info["AvailableLibraries"][0]["LegacyPath"]
        with info_path.open("wb") as output:
            plistlib.dump(info, output)
        wrapper = self.xcframework / "ios-arm64/Headers/NoritoBridge.h"
        wrapper.write_bytes(wrapper.read_bytes() + b"// drift\n")
        with self.assertRaisesRegex(validator.ValidationError, "authoritative source"):
            self.validate()

    def test_binary_path_shape_is_uniform_across_every_slice(self) -> None:
        info_path = self.xcframework / "Info.plist"
        with info_path.open("rb") as source:
            info = plistlib.load(source)
        for library in info["AvailableLibraries"]:
            library["BinaryPath"] = validator.LIBRARY_NAME
        with info_path.open("wb") as output:
            plistlib.dump(info, output)
        self.validate()

        del info["AvailableLibraries"][0]["BinaryPath"]
        with info_path.open("wb") as output:
            plistlib.dump(info, output)
        with self.assertRaisesRegex(validator.ValidationError, "must be uniform"):
            self.validate()

    def test_production_marker_is_an_empty_regular_file(self) -> None:
        self.payload["privacy_production_enabled"] = True
        self.payload["cargo_features"] = ["privacy-production-enabled"]
        self.payload["kagemusha_mobile_artifact_roles"] = (
            validator.expected_kagemusha_roles(True)
        )
        self.write_manifest()
        marker = self.xcframework / ".privacy-production-enabled"
        marker.mkdir()
        with self.assertRaisesRegex(validator.ValidationError, "regular file"):
            self.validate()
        marker.rmdir()

        marker.write_bytes(b"enabled\n")
        with self.assertRaisesRegex(validator.ValidationError, "must be empty"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
