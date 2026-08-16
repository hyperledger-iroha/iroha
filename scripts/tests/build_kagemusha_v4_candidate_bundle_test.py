from __future__ import annotations

from contextlib import contextmanager, nullcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "build_kagemusha_v4_candidate_bundle.py"
SPEC = importlib.util.spec_from_file_location(
    "build_kagemusha_v4_candidate_bundle", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
builder = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = builder
SPEC.loader.exec_module(builder)


class SealedCandidateBuildTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve(strict=True)
        self.external = tempfile.TemporaryDirectory()
        self.external_root = Path(self.external.name).resolve(strict=True)
        self.target_dir = self.external_root / "cargo-target"
        self.cargo = self.root / "admitted-cargo"
        self.cargo.write_bytes(b"cargo fixture\n")
        self.cargo.chmod(0o700)
        self.rustc = self.root / "admitted-rustc"
        self.rustc.write_bytes(b"rustc fixture\n")
        self.rustc.chmod(0o700)
        self.cargo_home = self.external_root / "cargo-home"
        self.cargo_home.mkdir(mode=0o700)
        self.reviewed_source_closure = self.root / "reviewed-source-closure.json"
        self.reviewed_source_closure_value = {
            "schema": builder.source_seal.REVIEWED_SOURCE_CLOSURE_SCHEMA,
            "source_repo_dirty": False,
            "fixture": True,
        }
        reviewed_source_closure_payload = builder._canonical_json_line(
            self.reviewed_source_closure_value
        )
        self.reviewed_source_closure.write_bytes(reviewed_source_closure_payload)
        self.reviewed_source_closure_sha256 = hashlib.sha256(
            reviewed_source_closure_payload
        ).hexdigest()
        self.source_authority = builder.source_seal.SourceAuthority(
            commit="a" * 40,
            commit_object_sha256="f" * 64,
            commit_object_size=2048,
            committer_epoch=builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1,
            git_tree="c" * 40,
            ordered_parents=(builder.AUTHORIZED_SOURCE_PARENT_COMMIT,),
            ordered_parent_trees=(builder.AUTHORIZED_SOURCE_PARENT_TREE,),
            signature=builder.source_seal.VerifiedSshSignature(
                principal="kagemusha-release@example.test",
                public_key_sha256="2" * 64,
                allowed_signers_sha256="1" * 64,
                revocation_sha256="3" * 64,
            ),
        )
        self.identity = builder.source_seal.SourceIdentity(
            source_commit="a" * 40,
            source_tree_sha256="b" * 64,
            source_repo_dirty=False,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
            source_authority=self.source_authority,
        )
        self.authenticated_source_seal_projection = (
            self.root / "authenticated-source-seal-projection.json"
        )
        self.authenticated_source_seal_projection_value = {
            "build_script_observed": {
                "debug_assertions": False,
                "features": list(builder.SOURCE_SEAL_RESOLVED_FEATURES),
                "host": builder.SOURCE_SEAL_TARGET,
                "num_jobs": 1,
                "opt_level": "3",
                "profile": "release",
                "schema": builder.SOURCE_SEAL_BUILD_SCRIPT_OBSERVED_SCHEMA,
                "target": builder.SOURCE_SEAL_TARGET,
            },
            "outer_policy": {
                "cargo": {
                    "binary": builder.BINARY_NAME,
                    "explicit_features": list(
                        builder.SOURCE_SEAL_EXPLICIT_FEATURES
                    ),
                    "package": "iroha_core",
                    "profile": "release",
                    "semantic_argv": list(builder.SOURCE_SEAL_SEMANTIC_ARGV),
                    "target": builder.SOURCE_SEAL_TARGET,
                    "unit_graph": {
                        "custom_build_packages": 2,
                        "custom_build_units": 3,
                        "iroha_core_units": 4,
                        "normalization": (
                            builder.SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION
                        ),
                        "packages": 100,
                        "sha256": "d" * 64,
                        "size_bytes": 4096,
                        "units": 200,
                    },
                },
                "execution_policy_sha256": "e" * 64,
                "schema": builder.SOURCE_SEAL_OUTER_POLICY_SCHEMA,
            },
            "reviewed_source_closure_hex": (
                reviewed_source_closure_payload.hex()
            ),
            "reviewed_source_closure_sha256": (
                self.reviewed_source_closure_sha256
            ),
            "schema": builder.AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA,
            "source_authority": {
                "commit": self.identity.source_commit,
                "commit_object_sha256": "f" * 64,
                "commit_object_size": 2048,
                "committer_epoch": builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1,
                "git_tree": "c" * 40,
                "ordered_parents": [builder.AUTHORIZED_SOURCE_PARENT_COMMIT],
                "parent_commit": builder.AUTHORIZED_SOURCE_PARENT_COMMIT,
                "parent_tree": builder.AUTHORIZED_SOURCE_PARENT_TREE,
                "signature": {
                    "allowed_signers_sha256": "1" * 64,
                    "mechanism": "git-commit-ssh-signature-v1",
                    "principal": "kagemusha-release@example.test",
                    "public_key_sha256": "2" * 64,
                    "revocation_sha256": "3" * 64,
                    "signature_namespace": "git",
                },
            },
            "source_commit": self.identity.source_commit,
            "source_date_epoch": builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1,
            "source_repo_dirty": False,
            "source_tree_sha256": self.identity.source_tree_sha256,
        }
        projection_payload = builder._canonical_json_line(
            self.authenticated_source_seal_projection_value
        )
        self.authenticated_source_seal_projection.write_bytes(projection_payload)
        self.authenticated_source_seal_projection_sha256 = hashlib.sha256(
            projection_payload
        ).hexdigest()

    def cargo_result(
        self, command: list[str], *, executable: Path | None = None
    ) -> subprocess.CompletedProcess[bytes]:
        if executable is None:
            executable = self.target_dir / "release" / builder.BINARY_NAME
        if not executable.exists() and not executable.is_symlink():
            executable.parent.mkdir(parents=True, exist_ok=True)
            executable.write_bytes(b"sealed candidate fixture\n")
            executable.chmod(0o700)
        message = {
            "reason": "compiler-artifact",
            "target": {"kind": ["bin"], "name": builder.BINARY_NAME},
            "profile": {"debug_assertions": False, "test": False},
            "executable": str(executable),
        }
        return subprocess.CompletedProcess(
            command,
            0,
            stdout=(json.dumps(message) + "\n").encode(),
        )

    def write_projection(
        self,
        value: dict[str, object],
        *,
        path: Path | None = None,
    ) -> tuple[Path, str]:
        """Write one canonical projection fixture and return its digest pin."""

        if path is None:
            path = self.authenticated_source_seal_projection
        payload = builder._canonical_json_line(value)
        path.write_bytes(payload)
        return path, hashlib.sha256(payload).hexdigest()

    def tearDown(self) -> None:
        self.external.cleanup()
        self.temporary.cleanup()

    def build_candidate_bundle(
        self, root: Path, cargo: str, **kwargs: object
    ) -> dict[str, object]:
        """Build with deterministic admission dependencies for unit tests."""

        kwargs.setdefault(
            "physical_memory_reader",
            lambda: builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES,
        )
        kwargs.setdefault("build_lock", lambda: nullcontext())
        kwargs.setdefault(
            "reviewed_source_closure",
            self.reviewed_source_closure,
        )
        kwargs.setdefault(
            "reviewed_source_closure_sha256",
            self.reviewed_source_closure_sha256,
        )
        kwargs.setdefault(
            "authenticated_source_seal_projection",
            self.authenticated_source_seal_projection,
        )
        kwargs.setdefault(
            "authenticated_source_seal_projection_sha256",
            self.authenticated_source_seal_projection_sha256,
        )
        kwargs.setdefault("rustc", str(self.rustc))
        kwargs.setdefault(
            "cargo_sha256", hashlib.sha256(self.cargo.read_bytes()).hexdigest()
        )
        kwargs.setdefault(
            "rustc_sha256", hashlib.sha256(self.rustc.read_bytes()).hexdigest()
        )
        kwargs.setdefault("cargo_home", self.cargo_home)
        kwargs.setdefault("target_dir", self.target_dir)
        return builder.build_candidate_bundle(root, cargo, **kwargs)

    def test_build_command_and_report_bind_exact_source_and_release_binary(self) -> None:
        identities: list[Path] = []
        invocations: list[tuple[list[str], dict[str, str]]] = []
        lock_events: list[str] = []
        lock_active = False

        def identity_reader(
            root: Path,
            descriptor_path: str,
            descriptor_sha256: str,
        ) -> builder.source_seal.SourceIdentity:
            identities.append(root)
            self.assertEqual(descriptor_path, str(self.reviewed_source_closure))
            self.assertEqual(
                descriptor_sha256,
                self.reviewed_source_closure_sha256,
            )
            return self.identity

        def command_runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(lock_active)
            invocations.append((command, kwargs["env"]))  # type: ignore[arg-type]
            return self.cargo_result(command)

        @contextmanager
        def build_lock():
            nonlocal lock_active
            lock_events.append("enter")
            lock_active = True
            try:
                yield
            finally:
                lock_active = False
                lock_events.append("exit")

        with mock.patch.dict(
            os.environ,
            {
                "CARGO_BUILD_RUSTC": "/hostile/rustc",
                "CARGO_BUILD_TARGET": "ambient-target",
                "CARGO_HOME": "/hostile/cargo-home",
                "CARGO_NET_OFFLINE": "false",
                "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS": "true",
                "CARGO_TARGET_DIR": "elsewhere",
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS": "--cfg hostile",
                "CC": "/hostile/cc",
                "CC_aarch64-apple-darwin": "/hostile/target-cc",
                "CFLAGS": "-include /hostile/injected.h",
                "CPATH": "/hostile/include",
                "CPPFLAGS": "-DHOSTILE=1",
                "CXX": "/hostile/cxx",
                "CXXFLAGS": "-stdlib=hostile",
                "DEVELOPER_DIR": "/hostile/Xcode.app",
                "DYLD_INSERT_LIBRARIES": "/hostile/injected.dylib",
                "HOST_AR": "/hostile/ar",
                "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256": "0" * 64,
                "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE": "/hostile/closure",
                "KAGEMUSHA_BUILD_SOURCE_COMMIT": "0" * 40,
                "KAGEMUSHA_SOURCE_SEAL_PYTHON": "/hostile/python",
                "LIBRARY_PATH": "/hostile/lib",
                "MACOSX_DEPLOYMENT_TARGET": "99.0",
                "PKG_CONFIG_PATH": "/hostile/pkgconfig",
                "HOME": "/hostile/home",
                "PATH": "/hostile/bin",
                "RUSTC": "/hostile/rustc",
                "RUSTC_WRAPPER": "/hostile/wrapper",
                "RUSTFLAGS": "--cfg hostile",
                "RUSTUP_HOME": "/hostile/rustup",
                "SDKROOT": "/hostile/SDK.sdk",
                "SOURCE_DATE_EPOCH": "0",
                "aarch64_apple_darwin_AR": "/hostile/target-ar",
            },
        ):
            report = self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=identity_reader,
                command_runner=command_runner,
                physical_memory_reader=lambda: 32 * 1024 * 1024 * 1024,
                build_lock=build_lock,
            )

        self.assertEqual(len(identities), 3)
        command, environment = invocations[0]
        self.assertEqual(command[0], str(self.cargo.resolve()))
        self.assertIn("--release", command)
        self.assertIn("--locked", command)
        self.assertEqual(
            command[command.index("--target-dir") + 1],
            str(self.target_dir),
        )
        features = set(command[command.index("--features") + 1].split(","))
        self.assertEqual(features, set(builder.CANDIDATE_BUILD_FEATURES))
        self.assertIn("iroha_core/dev-tools", features)
        self.assertIn(f"iroha_core/{builder.SEALED_FEATURE}", features)
        self.assertIn(f"iroha_core/{builder.CANDIDATE_EVIDENCE_FEATURE}", features)
        self.assertIn("--message-format=json-render-diagnostics", command)
        self.assertNotIn("CARGO_BUILD_TARGET", environment)
        self.assertNotIn("CARGO_BUILD_RUSTC", environment)
        self.assertNotIn("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", environment)
        self.assertNotIn("CARGO_TARGET_DIR", environment)
        self.assertNotIn("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", environment)
        self.assertNotIn("DYLD_INSERT_LIBRARIES", environment)
        for native_control in (
            "CC",
            "CC_aarch64-apple-darwin",
            "CFLAGS",
            "CPATH",
            "CPPFLAGS",
            "CXX",
            "CXXFLAGS",
            "DEVELOPER_DIR",
            "HOST_AR",
            "LIBRARY_PATH",
            "MACOSX_DEPLOYMENT_TARGET",
            "PKG_CONFIG_PATH",
            "SDKROOT",
            "aarch64_apple_darwin_AR",
        ):
            self.assertNotIn(native_control, environment)
        self.assertEqual(environment["RUSTC_WRAPPER"], "")
        self.assertEqual(environment["RUSTFLAGS"], "")
        self.assertEqual(environment["CARGO_HOME"], str(self.cargo_home))
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(environment["HOME"], "/var/empty")
        self.assertEqual(environment["PATH"], "/usr/bin:/bin")
        self.assertEqual(environment["CARGO"], str(self.cargo.resolve()))
        self.assertEqual(environment["RUSTC"], str(self.rustc.resolve()))
        self.assertNotIn("RUSTUP_HOME", environment)
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_COMMIT"],
            self.identity.source_commit,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_TREE_SHA256"],
            self.identity.source_tree_sha256,
        )
        self.assertNotIn("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE", environment)
        self.assertNotIn("KAGEMUSHA_SOURCE_SEAL_PYTHON", environment)
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_HEX"],
            self.authenticated_source_seal_projection_value[
                "reviewed_source_closure_hex"
            ],
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256"],
            self.reviewed_source_closure_sha256,
        )
        self.assertEqual(
            environment[
                "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256"
            ],
            self.authenticated_source_seal_projection_sha256,
        )
        self.assertEqual(
            bytes.fromhex(
                environment[
                    "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX"
                ]
            ),
            self.authenticated_source_seal_projection.read_bytes(),
        )
        expected_sealed_environment = {
            "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX",
            "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256",
            "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_HEX",
            "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256",
            "KAGEMUSHA_BUILD_SOURCE_COMMIT",
            "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SHA256",
            "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SIZE",
            "KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH",
            "KAGEMUSHA_BUILD_SOURCE_GIT_TREE",
            "KAGEMUSHA_BUILD_SOURCE_PARENT_COMMIT",
            "KAGEMUSHA_BUILD_SOURCE_PARENT_TREE",
            "KAGEMUSHA_BUILD_SOURCE_SSH_ALLOWED_SIGNERS_SHA256",
            "KAGEMUSHA_BUILD_SOURCE_SSH_PUBLIC_KEY_SHA256",
            "KAGEMUSHA_BUILD_SOURCE_SSH_REVOCATION_SHA256",
            "KAGEMUSHA_BUILD_SOURCE_SSH_SIGNER_PRINCIPAL",
            "KAGEMUSHA_BUILD_SOURCE_TREE_SHA256",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_PACKAGES",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS",
        }
        self.assertEqual(
            {key for key in environment if key.startswith("KAGEMUSHA_BUILD_")},
            expected_sealed_environment,
        )
        expected_projection_values = {
            "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256": "e" * 64,
            "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SHA256": "f" * 64,
            "KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SIZE": "2048",
            "KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH": str(
                builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1
            ),
            "KAGEMUSHA_BUILD_SOURCE_GIT_TREE": "c" * 40,
            "KAGEMUSHA_BUILD_SOURCE_PARENT_COMMIT": (
                builder.AUTHORIZED_SOURCE_PARENT_COMMIT
            ),
            "KAGEMUSHA_BUILD_SOURCE_PARENT_TREE": (
                builder.AUTHORIZED_SOURCE_PARENT_TREE
            ),
            "KAGEMUSHA_BUILD_SOURCE_SSH_ALLOWED_SIGNERS_SHA256": "1" * 64,
            "KAGEMUSHA_BUILD_SOURCE_SSH_PUBLIC_KEY_SHA256": "2" * 64,
            "KAGEMUSHA_BUILD_SOURCE_SSH_REVOCATION_SHA256": "3" * 64,
            "KAGEMUSHA_BUILD_SOURCE_SSH_SIGNER_PRINCIPAL": (
                "kagemusha-release@example.test"
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_PACKAGES": "2",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS": "3",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS": "4",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES": "100",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256": "d" * 64,
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES": "4096",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS": "200",
        }
        for key, expected in expected_projection_values.items():
            self.assertEqual(environment[key], expected)
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256"],
            hashlib.sha256(self.cargo.read_bytes()).hexdigest(),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256"],
            hashlib.sha256(self.rustc.read_bytes()).hexdigest(),
        )
        self.assertEqual(
            environment["SOURCE_DATE_EPOCH"],
            str(builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1),
        )
        self.assertEqual(report["build_profile"], "release")
        self.assertEqual(report["target_dir"], str(self.target_dir))
        self.assertEqual(report["source_commit"], self.identity.source_commit)
        self.assertIs(report["source_repo_dirty"], False)
        self.assertEqual(
            report["reviewed_source_closure"],
            self.reviewed_source_closure_value,
        )
        self.assertEqual(
            report["reviewed_source_closure_descriptor_sha256"],
            self.reviewed_source_closure_sha256,
        )
        self.assertEqual(
            report["authenticated_source_seal_projection_sha256"],
            self.authenticated_source_seal_projection_sha256,
        )
        self.assertEqual(
            report["source_date_epoch"], builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1
        )
        self.assertEqual(lock_events, ["enter", "exit"])
        self.assertEqual(
            report["minimum_build_physical_memory_bytes"],
            builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES,
        )
        self.assertEqual(
            report["physical_memory_bytes_at_admission"],
            32 * 1024 * 1024 * 1024,
        )
        self.assertRegex(str(report["binary_sha256"]), r"^[0-9a-f]{64}$")
        self.assertEqual(
            report["reviewed_cargo_binary_sha256"],
            hashlib.sha256(self.cargo.read_bytes()).hexdigest(),
        )
        self.assertEqual(
            report["reviewed_rustc_binary_sha256"],
            hashlib.sha256(self.rustc.read_bytes()).hexdigest(),
        )
        self.assertNotIn("cargo_executable_sha256", report)
        self.assertNotIn("rustc_executable_sha256", report)

    def test_all_native_toolchain_control_forms_are_removed(self) -> None:
        native_controls = {
            "AR": "/hostile/ar",
            "AR_aarch64_apple_darwin": "/hostile/target-ar",
            "BINDGEN_EXTRA_CLANG_ARGS_AARCH64_APPLE_DARWIN": "-include hostile.h",
            "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER": "/hostile/linker",
            "CC": "/hostile/cc",
            "CC_aarch64-apple-darwin": "/hostile/target-cc",
            "CFLAGS_aarch64_apple_darwin": "-DHOSTILE=1",
            "CPATH": "/hostile/include",
            "CPPFLAGS": "-include hostile.h",
            "CXX": "/hostile/cxx",
            "CXXFLAGS": "-stdlib=hostile",
            "DEVELOPER_DIR": "/hostile/Xcode.app",
            "HOST_CC": "/hostile/host-cc",
            "LIBRARY_PATH": "/hostile/lib",
            "MACOSX_DEPLOYMENT_TARGET": "99.0",
            "OPENSSL_DIR": "/hostile/openssl",
            "PKG_CONFIG_PATH": "/hostile/pkgconfig",
            "SDKROOT": "/hostile/SDK.sdk",
            "TARGET_CXX": "/hostile/target-cxx",
            "VCPKG_ROOT": "/hostile/vcpkg",
            "aarch64_apple_darwin_AR": "/hostile/suffix-ar",
            "aarch64_apple_darwin_CC": "/hostile/suffix-cc",
            "aarch64_apple_darwin_CXXFLAGS": "-DHOSTILE=1",
        }
        rustc = builder._admit_direct_executable(str(self.rustc), "rustc")

        with mock.patch.dict(
            os.environ,
            {**native_controls, "LANG": "C.UTF-8"},
            clear=True,
        ):
            environment = builder._sanitized_build_environment(self.cargo_home, rustc)

        for control in native_controls:
            self.assertNotIn(control, environment)
        self.assertEqual(environment["LANG"], "C.UTF-8")
        self.assertFalse(
            any(
                builder._is_ambient_native_toolchain_control(key)
                for key in environment
            )
        )

    def test_source_change_after_build_fails_closed(self) -> None:
        changed = builder.source_seal.SourceIdentity(
            source_commit="c" * 40,
            source_tree_sha256="d" * 64,
            source_repo_dirty=False,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
            source_authority=self.source_authority,
        )
        identities = iter((self.identity, changed))

        with self.assertRaisesRegex(builder.CandidateBuildError, "changed during"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: next(identities),
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_projection_digest_pin_is_checked_before_cargo(self) -> None:
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "digest differs"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256="9" * 64,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_unknown_member_is_rejected_before_cargo(self) -> None:
        projection = dict(self.authenticated_source_seal_projection_value)
        projection["ambient_override"] = True
        _, digest = self.write_projection(projection)
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "members are not exact"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256=digest,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_source_identity_mismatch_is_rejected_before_cargo(self) -> None:
        projection = dict(self.authenticated_source_seal_projection_value)
        projection["source_commit"] = "9" * 40
        _, digest = self.write_projection(projection)
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "source identity"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256=digest,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_cannot_claim_a_different_commit_object(self) -> None:
        projection = json.loads(
            json.dumps(self.authenticated_source_seal_projection_value)
        )
        projection["source_authority"]["commit_object_sha256"] = "9" * 64
        _, digest = self.write_projection(projection)
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "verified HEAD"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256=digest,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_cannot_claim_a_different_committer_epoch(self) -> None:
        projection = json.loads(
            json.dumps(self.authenticated_source_seal_projection_value)
        )
        changed_epoch = builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 2
        projection["source_date_epoch"] = changed_epoch
        projection["source_authority"]["committer_epoch"] = changed_epoch
        _, digest = self.write_projection(projection)
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "verified HEAD"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256=digest,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_cannot_claim_a_different_verified_ssh_key(self) -> None:
        projection = json.loads(
            json.dumps(self.authenticated_source_seal_projection_value)
        )
        projection["source_authority"]["signature"]["public_key_sha256"] = (
            "9" * 64
        )
        _, digest = self.write_projection(projection)
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "verified HEAD"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection_sha256=digest,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_projection_path_symlink_is_rejected(self) -> None:
        link = self.root / "projection-link.json"
        link.symlink_to(self.authenticated_source_seal_projection)

        with self.assertRaisesRegex(builder.CandidateBuildError, "regular file"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection=link,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_projection_path_must_be_absolute_and_normalized(self) -> None:
        with self.assertRaisesRegex(builder.CandidateBuildError, "absolute and normalized"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                authenticated_source_seal_projection=Path(
                    self.authenticated_source_seal_projection.name
                ),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_projection_mutation_during_cargo_fails_closed(self) -> None:
        def command_runner(
            command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            projection = dict(self.authenticated_source_seal_projection_value)
            projection["source_date_epoch"] = builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 2
            self.write_projection(projection)
            return self.cargo_result(command)

        with self.assertRaisesRegex(builder.CandidateBuildError, "digest differs"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

    def test_rustc_mutation_during_cargo_fails_closed(self) -> None:
        def command_runner(
            command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            result = self.cargo_result(command)
            self.rustc.write_bytes(b"substituted rustc fixture\n")
            self.rustc.chmod(0o700)
            return result

        with self.assertRaisesRegex(builder.CandidateBuildError, "rustc executable changed"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

    def test_cargo_mutation_during_build_fails_closed(self) -> None:
        def command_runner(
            command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            result = self.cargo_result(command)
            self.cargo.write_bytes(b"substituted cargo fixture\n")
            self.cargo.chmod(0o700)
            return result

        with self.assertRaisesRegex(builder.CandidateBuildError, "Cargo executable changed"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

    def test_cargo_digest_pin_is_required_before_build(self) -> None:
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "SHA-256 pin"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                cargo_sha256="9" * 64,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_rustc_digest_pin_is_required_before_cargo(self) -> None:
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "SHA-256 pin"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                rustc_sha256="9" * 64,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_ambient_cargo_home_configuration_is_rejected(self) -> None:
        (self.cargo_home / "config.toml").write_text(
            "[build]\nrustc = '/hostile/rustc'\n", encoding="utf-8"
        )
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "ambient Cargo"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_source_ancestor_cargo_configuration_is_rejected(self) -> None:
        nested_root = self.root / "workspace"
        nested_root.mkdir()
        ambient = self.root / ".cargo"
        ambient.mkdir()
        (ambient / "config").write_text(
            "[build]\nrustc = '/hostile/rustc'\n", encoding="utf-8"
        )
        command_runner = mock.Mock()

        with self.assertRaisesRegex(builder.CandidateBuildError, "ancestors"):
            self.build_candidate_bundle(
                nested_root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

        command_runner.assert_not_called()

    def test_dirty_source_identity_is_rejected_before_cargo(self) -> None:
        dirty_closure = dict(self.reviewed_source_closure_value)
        dirty_closure["source_repo_dirty"] = True
        dirty = builder.source_seal.SourceIdentity(
            source_commit=self.identity.source_commit,
            source_tree_sha256=self.identity.source_tree_sha256,
            source_repo_dirty=True,
            reviewed_source_closure=dirty_closure,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
            source_authority=self.source_authority,
        )
        with self.assertRaisesRegex(builder.CandidateBuildError, "clean signed"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: dirty,
                command_runner=lambda _command, **_kwargs: self.fail(
                    "Cargo must not run for a dirty source identity"
                ),
            )

    def test_exact_release_artifact_symlink_is_rejected_before_resolution(self) -> None:
        linked_binary = self.target_dir / "release" / builder.BINARY_NAME

        def symlink_result(command: list[str]) -> subprocess.CompletedProcess[bytes]:
            linked_binary.parent.mkdir(parents=True)
            real_binary = linked_binary.with_name("real-candidate")
            real_binary.write_bytes(b"replacement candidate\n")
            real_binary.chmod(0o700)
            linked_binary.symlink_to(real_binary)
            return self.cargo_result(command, executable=linked_binary)

        with self.assertRaisesRegex(builder.CandidateBuildError, "symbolic link"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: symlink_result(command),
            )

    def test_cargo_artifact_path_not_default_layout_is_authoritative(self) -> None:
        configured_binary = (
            self.target_dir / "configured-triple" / "release" / builder.BINARY_NAME
        )

        report = self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=lambda command, **_kwargs: self.cargo_result(
                command, executable=configured_binary
            ),
        )

        self.assertEqual(report["binary_path"], str(configured_binary.resolve()))

    def test_missing_compiler_artifact_fails_closed(self) -> None:
        with self.assertRaisesRegex(builder.CandidateBuildError, "exactly one"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: subprocess.CompletedProcess(
                    command, 0, stdout=b""
                ),
            )

    def test_debug_compiler_artifact_fails_closed(self) -> None:
        def debug_result(command: list[str]) -> subprocess.CompletedProcess[bytes]:
            message = {
                "reason": "compiler-artifact",
                "target": {"kind": ["bin"], "name": builder.BINARY_NAME},
                "profile": {"debug_assertions": True, "test": False},
                "executable": str(
                    self.target_dir / "release" / builder.BINARY_NAME
                ),
            }
            return subprocess.CompletedProcess(
                command, 0, stdout=(json.dumps(message) + "\n").encode()
            )

        with self.assertRaisesRegex(builder.CandidateBuildError, "debug assertions"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: debug_result(command),
            )

    def test_low_memory_refuses_before_source_inspection_lock_or_cargo(self) -> None:
        identity_reader = mock.Mock(return_value=self.identity)
        command_runner = mock.Mock()
        build_lock = mock.Mock(return_value=nullcontext())

        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "requires at least",
        ):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=identity_reader,
                command_runner=command_runner,
                physical_memory_reader=lambda: (
                    builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES - 1
                ),
                build_lock=build_lock,
            )

        identity_reader.assert_not_called()
        build_lock.assert_not_called()
        command_runner.assert_not_called()

    def test_target_directory_must_be_fresh_absolute_and_external(self) -> None:
        common = {
            "identity_reader": lambda _root, _path, _sha256: self.identity,
            "command_runner": lambda command, **_kwargs: self.cargo_result(command),
        }
        with self.assertRaisesRegex(builder.CandidateBuildError, "absolute and normalized"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                target_dir=Path("relative-target"),
                **common,
            )

        with self.assertRaisesRegex(builder.CandidateBuildError, "outside"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                target_dir=self.root / "target",
                **common,
            )

        self.target_dir.mkdir()
        with self.assertRaisesRegex(builder.CandidateBuildError, "fresh nonexistent"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                **common,
            )

    def test_detects_macos_physical_memory_with_sysctl(self) -> None:
        expected = 24 * 1024 * 1024 * 1024
        inspection_runner = mock.Mock(
            return_value=subprocess.CompletedProcess(
                ["sysctl"], 0, stdout=f"{expected}\n"
            )
        )

        actual = builder._physical_memory_bytes(
            platform_name="darwin",
            inspection_runner=inspection_runner,
            sysconf_reader=mock.Mock(side_effect=AssertionError("unexpected fallback")),
        )

        self.assertEqual(actual, expected)
        self.assertEqual(
            inspection_runner.call_args.args[0],
            ["/usr/sbin/sysctl", "-n", "hw.memsize"],
        )

    def test_detects_linux_physical_memory_with_sysconf(self) -> None:
        values = {"SC_PHYS_PAGES": 8_388_608, "SC_PAGE_SIZE": 4096}

        actual = builder._physical_memory_bytes(
            platform_name="linux",
            sysconf_reader=lambda name: values[name],
            linux_meminfo_reader=mock.Mock(
                side_effect=AssertionError("unexpected fallback")
            ),
        )

        self.assertEqual(actual, 32 * 1024 * 1024 * 1024)

    def test_linux_meminfo_is_used_when_sysconf_is_unavailable(self) -> None:
        actual = builder._physical_memory_bytes(
            platform_name="linux",
            sysconf_reader=mock.Mock(side_effect=ValueError("unavailable")),
            linux_meminfo_reader=lambda: "MemTotal:       25165824 kB\n",
        )

        self.assertEqual(actual, builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES)

    def test_shared_build_lock_uses_memory_heavy_host_lock(self) -> None:
        host_lock = mock.Mock(return_value=nullcontext())

        with mock.patch.object(builder.resource_guard, "_host_lock", host_lock):
            with builder._shared_memory_heavy_build_lock():
                pass

        host_lock.assert_called_once_with(
            builder.resource_guard.HEAVY_JOB_LOCK_PATH,
            description="memory-heavy job",
        )

    def test_bundle_validates_embedded_seal_before_generation(self) -> None:
        bundle = (
            Path(__file__).parents[2]
            / "crates"
            / "iroha_core"
            / "src"
            / "bin"
            / "kagemusha_recursive_spend_v4_bundle.rs"
        ).read_text(encoding="utf-8")
        build_rs = (
            Path(__file__).parents[2] / "crates" / "iroha_core" / "build.rs"
        ).read_text(encoding="utf-8")

        build_candidate = bundle[bundle.index("fn build_candidate(") :]
        self.assertLess(
            build_candidate.index("validate_embedded_candidate_source("),
            build_candidate.index("prepare_bundle_metadata("),
        )
        self.assertIn('option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT")', bundle)
        self.assertIn("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256", build_rs)
        self.assertIn(
            "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX", build_rs
        )
        self.assertIn(
            'require_actual_tool_digest("CARGO", &reviewed_cargo_binary_sha256',
            build_rs,
        )
        self.assertIn(
            'require_actual_tool_digest("RUSTC", &reviewed_rustc_binary_sha256',
            build_rs,
        )
        self.assertIn(
            'option_env!("KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256")', bundle
        )
        self.assertIn(
            'option_env!("KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256")', bundle
        )
        self.assertNotIn("kagemusha_source_tree_seal.py", build_rs)

    def test_builder_disables_python_bytecode_before_local_imports(self) -> None:
        source = (
            Path(__file__).parents[2]
            / "scripts"
            / "build_kagemusha_v4_candidate_bundle.py"
        ).read_text(encoding="utf-8")

        disable = source.index("sys.dont_write_bytecode = True")
        local_import = source.index(
            "from scripts import kagemusha_source_tree_seal as source_seal"
        )
        self.assertLess(disable, local_import)


if __name__ == "__main__":
    unittest.main()
