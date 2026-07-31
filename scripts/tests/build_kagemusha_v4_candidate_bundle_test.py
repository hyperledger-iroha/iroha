from __future__ import annotations

from contextlib import contextmanager, nullcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import py_compile
import shutil
import subprocess
import sys
import tempfile
from typing import ContextManager
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
        self.linker = self.root / "admitted-linker"
        self.linker.write_bytes(b"linker fixture\n")
        self.linker.chmod(0o700)
        self.git = self.root / "admitted-git"
        self.git.write_bytes(b"git fixture\n")
        self.git.chmod(0o700)
        self.git_exec_path = self.root / "git-exec"
        self.git_exec_path.mkdir(mode=0o700)
        self.gpg = self.root / "admitted-gpg"
        self.gpg.write_bytes(b"gpg fixture\n")
        self.gpg.chmod(0o700)
        self.gnupghome = self.root / "gnupg-home"
        self.gnupghome.mkdir(mode=0o700)
        self.rustc_sysroot = self.root / "rustc-sysroot"
        self.rustc_sysroot.mkdir(mode=0o700)
        self.apple_developer_dir = self.root / "apple-developer"
        self.apple_developer_dir.mkdir(mode=0o700)
        self.apple_sdk = self.apple_developer_dir / "MacOSX.sdk"
        self.apple_sdk.mkdir(mode=0o700)
        self.clang_resource_dir = self.apple_developer_dir / "clang-resource"
        self.clang_resource_dir.mkdir(mode=0o700)
        self.cargo_home = self.root / "cargo-home"
        self.cargo_home.mkdir(mode=0o700)
        self.cargo_vendor = self.root / "cargo-vendor"
        self.cargo_vendor.mkdir(mode=0o700)
        self.python = self.root / "admitted-python"
        self.python.write_bytes(b"python fixture\n")
        self.python.chmod(0o700)
        self.toolchain_provenance = self.root / "toolchain-provenance.json"
        self.toolchain_provenance.write_bytes(b"{\"fixture\":true}\n")
        self.reviewed_source_closure = self.root / "reviewed-source-closure.json"
        self.reviewed_source_closure.write_bytes(b"{\"fixture\":true}\n")
        self.toolchain = builder.AdmittedRustToolchain(
            provenance=self.toolchain_provenance,
            closure_root=self.root,
            closure_tree_sha256="f" * 64,
            source_root=self.root,
            reviewed_source_closure=self.reviewed_source_closure,
            apple_developer_dir=self.apple_developer_dir,
            apple_sdk=self.apple_sdk,
            clang_resource_dir=self.clang_resource_dir,
            cargo_home=self.cargo_home,
            cargo_vendor=self.cargo_vendor,
            cargo=self.cargo,
            rustc=self.rustc,
            rustc_sysroot=self.rustc_sysroot,
            linker=self.linker,
            git=self.git,
            git_exec_path=self.git_exec_path,
            gpg=self.gpg,
            gnupghome=self.gnupghome,
            python=self.python,
            cargo_sha256=hashlib.sha256(self.cargo.read_bytes()).hexdigest(),
            rustc_sha256=hashlib.sha256(self.rustc.read_bytes()).hexdigest(),
            linker_sha256=hashlib.sha256(self.linker.read_bytes()).hexdigest(),
            git_sha256=hashlib.sha256(self.git.read_bytes()).hexdigest(),
            gpg_sha256=hashlib.sha256(self.gpg.read_bytes()).hexdigest(),
            python_sha256=hashlib.sha256(self.python.read_bytes()).hexdigest(),
            source_signing_key_fingerprint="E" * 40,
            provenance_sha256=hashlib.sha256(
                self.toolchain_provenance.read_bytes()
            ).hexdigest(),
        )
        self.reviewed_source_closure_sha256 = "c" * 64
        self.reviewed_source_closure_value = {
            "schema": builder.source_seal.REVIEWED_SOURCE_CLOSURE_SCHEMA,
            "fixture": True,
        }
        self.identity = builder.source_seal.SourceIdentity(
            source_commit="a" * 40,
            source_tree_sha256="b" * 64,
            source_repo_dirty=False,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
        )

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
        kwargs.setdefault(
            "build_identity_reader",
            lambda: (builder.PRODUCTION_BUILD_USER_NAME, 501),
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
        kwargs.setdefault("target_dir", self.target_dir)
        kwargs.setdefault("source_materializer", self.materialize_source)
        kwargs.setdefault("source_command_guard", self.guard_source_command)
        kwargs.setdefault("toolchain_admitter", self.admit_toolchain)
        kwargs.setdefault(
            "production_closure_admitter",
            lambda _root, _descriptor, _tools: None,
        )
        kwargs.setdefault(
            "source_git_configurer",
            lambda _git, _git_exec_path: None,
        )
        kwargs.setdefault("toolchain_rechecker", self.recheck_toolchain)
        kwargs.setdefault("signature_verifier", lambda _root, _commit, _tools: None)
        return builder.build_candidate_bundle(root, cargo, **kwargs)

    def admit_toolchain(
        self,
        cargo: str,
        provenance_path: Path | None,
        provenance_sha256: str | None,
    ) -> builder.AdmittedRustToolchain:
        self.assertEqual(cargo, str(self.cargo))
        self.assertIsNone(provenance_path)
        self.assertIsNone(provenance_sha256)
        return self.toolchain

    def recheck_toolchain(
        self,
        toolchain: builder.AdmittedRustToolchain,
    ) -> None:
        for path, expected, label in (
            (toolchain.cargo, toolchain.cargo_sha256, "Cargo"),
            (toolchain.rustc, toolchain.rustc_sha256, "rustc"),
            (toolchain.linker, toolchain.linker_sha256, "linker"),
            (toolchain.git, toolchain.git_sha256, "Git"),
            (toolchain.gpg, toolchain.gpg_sha256, "GPG"),
            (toolchain.python, toolchain.python_sha256, "Python"),
        ):
            observed, _ = builder._binary_sha256_for_tool(path)
            if observed != expected:
                raise builder.CandidateBuildError(
                    f"{label} changed after admission"
                )

    def materialize_source(
        self,
        root: Path,
        workspace: Path,
        descriptor_path: str,
        descriptor_sha256: str,
        *,
        expected_identity: builder.source_seal.SourceIdentity,
    ) -> ContextManager[builder.source_seal.SourceMaterialization]:
        self.assertEqual(root, self.root)
        self.assertEqual(descriptor_path, str(self.reviewed_source_closure))
        self.assertEqual(descriptor_sha256, self.reviewed_source_closure_sha256)
        self.assertEqual(expected_identity, self.identity)
        self.assertEqual(
            workspace,
            self.target_dir / builder.SEALED_SOURCE_WORKSPACE_NAME,
        )
        workspace.mkdir(mode=0o700)
        destination = workspace / builder.MATERIALIZED_SOURCE_DIR_NAME
        descriptor_destination = workspace / builder.MATERIALIZED_DESCRIPTOR_NAME
        destination.mkdir(mode=0o700)
        source_input = root / "source-input.txt"
        if source_input.exists():
            (destination / "source-input.txt").write_bytes(source_input.read_bytes())
        descriptor_destination.write_bytes(self.reviewed_source_closure.read_bytes())
        return nullcontext(
            builder.source_seal.SourceMaterialization(
                root=destination,
                reviewed_source_closure=descriptor_destination,
                identity=self.identity,
            )
        )

    def guard_source_command(
        self,
        root: Path,
        descriptor: Path,
        command: list[str],
    ) -> list[str]:
        workspace = self.target_dir / builder.SEALED_SOURCE_WORKSPACE_NAME
        self.assertEqual(root, workspace / builder.MATERIALIZED_SOURCE_DIR_NAME)
        self.assertEqual(
            descriptor,
            workspace / builder.MATERIALIZED_DESCRIPTOR_NAME,
        )
        return ["/source-write-denial-boundary", "--", *command]

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
            expected_descriptor = (
                self.reviewed_source_closure
                if root == self.root
                else self.target_dir
                / builder.SEALED_SOURCE_WORKSPACE_NAME
                / builder.MATERIALIZED_DESCRIPTOR_NAME
            )
            self.assertEqual(descriptor_path, str(expected_descriptor))
            self.assertEqual(
                descriptor_sha256,
                self.reviewed_source_closure_sha256,
            )
            return self.identity

        def command_runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(lock_active)
            self.assertEqual(
                kwargs["cwd"],
                self.target_dir
                / builder.SEALED_SOURCE_WORKSPACE_NAME
                / builder.MATERIALIZED_SOURCE_DIR_NAME,
            )
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

        hostile_home = self.root / "hostile-home"
        (hostile_home / ".cargo").mkdir(parents=True)
        (hostile_home / ".cargo" / "config.toml").write_text(
            '[build]\nrustc = "/hostile/config-rustc"\n',
            encoding="utf-8",
        )
        with mock.patch.dict(
            os.environ,
            {
                "CARGO_BUILD_RUSTC": "/hostile/rustc",
                "CARGO_BUILD_TARGET": "ambient-target",
                "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS": "true",
                "CARGO_TARGET_DIR": "elsewhere",
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS": "--cfg hostile",
                "HOME": str(hostile_home),
                "PATH": "/hostile/path",
                "RUSTC_WRAPPER": "/hostile/wrapper",
                "RUSTFLAGS": "--cfg hostile",
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

        self.assertEqual(len(identities), 4)
        command, environment = invocations[0]
        self.assertEqual(command[:2], ["/source-write-denial-boundary", "--"])
        cargo_command = command[2:]
        self.assertEqual(cargo_command[0], str(self.cargo.resolve()))
        self.assertIn("--release", cargo_command)
        self.assertIn("--locked", cargo_command)
        self.assertIn("--offline", cargo_command)
        self.assertIn("--frozen", cargo_command)
        self.assertEqual(
            cargo_command[cargo_command.index("--target-dir") + 1],
            str(self.target_dir),
        )
        self.assertIn(builder.SEALED_FEATURE, cargo_command)
        self.assertIn("--message-format=json-render-diagnostics", cargo_command)
        self.assertNotIn("CARGO_BUILD_TARGET", environment)
        self.assertNotIn("CARGO_BUILD_RUSTC", environment)
        self.assertNotIn("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", environment)
        self.assertNotIn("CARGO_TARGET_DIR", environment)
        self.assertNotIn("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", environment)
        self.assertEqual(environment["RUSTC_WRAPPER"], "")
        self.assertEqual(environment["RUSTFLAGS"], "")
        self.assertEqual(environment["PATH"], f"{self.root}:/usr/bin:/bin")
        self.assertEqual(environment["RUSTC"], str(self.rustc))
        self.assertEqual(
            environment["CARGO_ENCODED_RUSTFLAGS"],
            (
                f"-Clinker={self.linker}\x1f"
                f"--sysroot={self.rustc_sysroot}\x1f"
                f"-Clink-arg=-isysroot\x1f"
                f"-Clink-arg={self.apple_sdk}\x1f"
                f"-Clink-arg=-resource-dir\x1f"
                f"-Clink-arg={self.clang_resource_dir}\x1f"
                f"-Clink-arg=-B{self.root}"
            ),
        )
        self.assertEqual(environment["CC"], str(self.linker))
        self.assertEqual(environment["DEVELOPER_DIR"], str(self.apple_developer_dir))
        self.assertEqual(environment["SDKROOT"], str(self.apple_sdk))
        self.assertEqual(
            environment["HOME"],
            str(self.target_dir / ".build-home"),
        )
        self.assertEqual(
            environment["CARGO_HOME"],
            str(self.cargo_home),
        )
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_COMMIT"],
            self.identity.source_commit,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_TREE_SHA256"],
            self.identity.source_tree_sha256,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE"],
            str(
                self.target_dir
                / builder.SEALED_SOURCE_WORKSPACE_NAME
                / builder.MATERIALIZED_DESCRIPTOR_NAME
            ),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256"],
            self.reviewed_source_closure_sha256,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_GPG_EXECUTABLE"],
            str(self.gpg),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_GNUPGHOME"],
            str(self.gnupghome),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_SIGNING_KEY_FINGERPRINT"],
            "E" * 40,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_GIT_EXECUTABLE"],
            str(self.git),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_GIT_EXEC_PATH"],
            str(self.git_exec_path),
        )
        self.assertEqual(report["build_profile"], "release")
        self.assertEqual(
            report["build_user_name"],
            builder.PRODUCTION_BUILD_USER_NAME,
        )
        self.assertEqual(report["build_uid"], 501)
        self.assertEqual(
            report["publication_status"],
            "provisional_boi_build_worker_output",
        )
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

    def test_build_worker_identity_is_supervisor_bound(self) -> None:
        with (
            mock.patch.dict(
                os.environ,
                {
                    "KAGEMUSHA_BUILD_USER_NAME": (
                        builder.PRODUCTION_BUILD_USER_NAME
                    ),
                    "KAGEMUSHA_BUILD_UID": "777",
                },
                clear=True,
            ),
            mock.patch.object(builder.os, "geteuid", return_value=777),
        ):
            self.assertEqual(
                builder._admitted_build_worker_identity(),
                (builder.PRODUCTION_BUILD_USER_NAME, 777),
            )
        for environment in (
            {},
            {
                "KAGEMUSHA_BUILD_USER_NAME": "operator",
                "KAGEMUSHA_BUILD_UID": "777",
            },
            {
                "KAGEMUSHA_BUILD_USER_NAME": (
                    builder.PRODUCTION_BUILD_USER_NAME
                ),
                "KAGEMUSHA_BUILD_UID": "0",
            },
        ):
            with (
                mock.patch.dict(os.environ, environment, clear=True),
                mock.patch.object(builder.os, "geteuid", return_value=777),
                self.assertRaisesRegex(
                    builder.CandidateBuildError,
                    "boi-build UID",
                ),
            ):
                builder._admitted_build_worker_identity()

    def test_source_change_after_build_fails_closed(self) -> None:
        changed = builder.source_seal.SourceIdentity(
            source_commit="c" * 40,
            source_tree_sha256="d" * 64,
            source_repo_dirty=False,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
        )
        identities = iter((self.identity, self.identity, changed))

        with self.assertRaisesRegex(builder.CandidateBuildError, "changed during"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: next(identities),
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_original_mutation_during_cargo_cannot_change_private_source(self) -> None:
        source_input = self.root / "source-input.txt"
        source_input.write_bytes(b"reviewed source bytes\n")

        def command_runner(
            command: list[str],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            private_root = kwargs["cwd"]
            self.assertIsInstance(private_root, Path)
            self.assertNotEqual(private_root, self.root)
            source_input.write_bytes(b"attacker mutation during cargo\n")
            self.assertEqual(
                (private_root / "source-input.txt").read_bytes(),  # type: ignore[operator]
                b"reviewed source bytes\n",
            )
            return self.cargo_result(command)

        report = self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=command_runner,
        )

        self.assertEqual(report["source_tree_sha256"], self.identity.source_tree_sha256)

    def test_ambient_ancestor_cargo_config_is_rejected(self) -> None:
        cargo_config = self.external_root / ".cargo"
        cargo_config.mkdir()
        (cargo_config / "config.toml").write_text(
            '[build]\nrustc = "/hostile/ancestor-rustc"\n',
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "ambient Cargo configuration is forbidden",
        ):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_toolchain_mutation_during_cargo_is_rejected(self) -> None:
        def command_runner(
            command: list[str],
            **_kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            self.rustc.write_bytes(b"replaced rustc fixture\n")
            self.rustc.chmod(0o700)
            return self.cargo_result(command)

        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "rustc changed after admission",
        ):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )

    def test_builder_never_loads_checkout_bytecode(self) -> None:
        copied_root = self.external_root / "copied-repository"
        copied_scripts = copied_root / "scripts"
        copied_formal = copied_scripts / "formal"
        copied_formal.mkdir(parents=True)
        copied_builder = copied_scripts / SCRIPT.name
        copied_source_seal = copied_scripts / "kagemusha_source_tree_seal.py"
        copied_resource_guard = copied_formal / "run_sumeragi_v2_tlapm_guard.py"
        shutil.copyfile(SCRIPT, copied_builder)
        shutil.copyfile(
            SCRIPT.parent / "kagemusha_source_tree_seal.py",
            copied_source_seal,
        )
        shutil.copyfile(
            SCRIPT.parent / "formal/run_sumeragi_v2_tlapm_guard.py",
            copied_resource_guard,
        )

        marker = self.external_root / "malicious-pyc-executed"
        legitimate = copied_source_seal.read_bytes()
        malicious_prefix = (
            b"from pathlib import Path\n"
            + f"Path({str(marker)!r}).write_bytes(b'executed')\n".encode("ascii")
        )
        self.assertLess(len(malicious_prefix), len(legitimate))
        malicious = malicious_prefix + b"#" * (
            len(legitimate) - len(malicious_prefix)
        )
        fixed_seconds = 1_700_000_000
        copied_source_seal.write_bytes(malicious)
        os.utime(copied_source_seal, (fixed_seconds, fixed_seconds))
        cache_path = Path(importlib.util.cache_from_source(str(copied_source_seal)))
        cache_path.parent.mkdir()
        py_compile.compile(
            str(copied_source_seal),
            cfile=str(cache_path),
            doraise=True,
        )
        copied_source_seal.write_bytes(legitimate)
        os.utime(copied_source_seal, (fixed_seconds, fixed_seconds))

        ordinary_import = subprocess.run(
            [
                sys.executable,
                "-B",
                "-c",
                (
                    "import sys;"
                    f"sys.path.insert(0,{str(copied_root)!r});"
                    "from scripts import kagemusha_source_tree_seal"
                ),
            ],
            check=False,
            capture_output=True,
        )
        self.assertEqual(ordinary_import.returncode, 0, ordinary_import.stderr)
        self.assertEqual(marker.read_bytes(), b"executed")
        marker.unlink()

        isolated_builder = subprocess.run(
            [sys.executable, "-B", str(copied_builder), "--help"],
            check=False,
            capture_output=True,
            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        )
        self.assertEqual(isolated_builder.returncode, 0, isolated_builder.stderr)
        self.assertFalse(marker.exists())

    def test_production_tree_digest_requires_nonwritable_exact_bytes(self) -> None:
        closure = self.external_root / "root-published-fixture"
        source = closure / "source"
        source.mkdir(parents=True)
        payload = source / "payload"
        payload.write_bytes(b"root-published bytes\n")
        provenance = closure / builder.PRODUCTION_PROVENANCE_FILE_NAME
        provenance.write_bytes(b"excluded provenance\n")
        payload.chmod(0o444)
        provenance.chmod(0o444)
        source.chmod(0o555)
        closure.chmod(0o555)
        try:
            first = builder._production_closure_tree_sha256(
                closure,
                provenance,
                trusted_owner_uid=os.geteuid(),
                validate_parent_chain=False,
            )
            second = builder._production_closure_tree_sha256(
                closure,
                provenance,
                trusted_owner_uid=os.geteuid(),
                validate_parent_chain=False,
            )
            self.assertEqual(first, second)
            payload.chmod(0o644)
            with self.assertRaisesRegex(
                builder.CandidateBuildError,
                "remains writable",
            ):
                builder._production_closure_tree_sha256(
                    closure,
                    provenance,
                    trusted_owner_uid=os.geteuid(),
                    validate_parent_chain=False,
                )
        finally:
            closure.chmod(0o700)
            source.chmod(0o700)
            payload.chmod(0o600)
            provenance.chmod(0o600)

    def test_default_provenance_admission_rejects_user_owned_file(self) -> None:
        digest = hashlib.sha256(
            self.toolchain_provenance.read_bytes()
        ).hexdigest()
        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "unsafe metadata",
        ):
            builder._admitted_production_rust_toolchain(
                "cargo",
                self.toolchain_provenance,
                digest,
            )

    def test_offline_cargo_config_has_no_content_address_fixed_point(self) -> None:
        first_root = self.external_root / ("a" * 64)
        second_root = self.external_root / ("b" * 64)
        for closure in (first_root, second_root):
            (closure / "cargo-home").mkdir(parents=True)
            (closure / "cargo-vendor").mkdir()

        first = builder._canonical_offline_cargo_config(
            first_root / "cargo-home",
            first_root / "cargo-vendor",
        )
        second = builder._canonical_offline_cargo_config(
            second_root / "cargo-home",
            second_root / "cargo-vendor",
        )

        self.assertEqual(first, second)
        self.assertIn(b'directory = "../cargo-vendor"', first)
        self.assertNotIn(str(first_root).encode(), first)
        self.assertNotIn(str(second_root).encode(), second)

    def test_signed_commit_verifier_pins_gpg_keyring_and_fingerprint(self) -> None:
        calls: list[tuple[list[str], dict[str, str]]] = []

        def runner(
            command: list[str],
            **kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            environment = kwargs["env"]
            self.assertIsInstance(environment, dict)
            calls.append((command, environment))  # type: ignore[arg-type]
            if "verify-commit" in command:
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout=b"",
                    stderr=(
                        b"[GNUPG:] NEWSIG\n[GNUPG:] VALIDSIG "
                        + b"E" * 40
                        + b" 2026-07-30 0 0 4 0 22 10 00 "
                        + b"E" * 40
                        + b"\n"
                    ),
                )
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=(self.identity.source_commit + "\n").encode("ascii"),
                stderr=b"",
            )

        builder._verify_exact_signed_commit(
            self.root,
            self.identity.source_commit,
            self.toolchain,
            command_runner=runner,
        )

        self.assertEqual(len(calls), 3)
        signature_command, environment = calls[1]
        self.assertEqual(signature_command[0], str(self.git))
        self.assertIn(f"gpg.program={self.gpg}", signature_command)
        self.assertIn("gpg.format=openpgp", signature_command)
        self.assertEqual(environment["GNUPGHOME"], str(self.gnupghome))
        self.assertEqual(environment["GIT_EXEC_PATH"], str(self.git_exec_path))
        self.assertEqual(environment["PATH"], "/usr/bin:/bin")
        self.assertNotIn("HOME", environment)

    def test_system_dependency_allowlist_rejects_path_traversal(self) -> None:
        for load_name in (
            "/System/Library/../../tmp/evil.dylib",
            "/usr/lib/../local/libevil.dylib",
            "/usr/lib//libSystem.B.dylib",
            "/System/Library/./Frameworks/Evil.framework/Evil",
        ):
            with self.subTest(load_name=load_name):
                self.assertFalse(
                    builder._is_canonical_system_dependency(load_name)
                )
        for load_name in (
            "/usr/lib/libSystem.B.dylib",
            "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation",
        ):
            with self.subTest(load_name=load_name):
                self.assertTrue(
                    builder._is_canonical_system_dependency(load_name)
                )

    def test_macho_checker_rejects_executable_path_decoy(self) -> None:
        closure = self.external_root / "macho-closure"
        executable = closure / "bin/tool"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"Mach-O fixture")

        def otool(
            path: Path,
            *arguments: str,
            **_kwargs: object,
        ) -> str:
            self.assertEqual(path, executable)
            if arguments == ("-L",):
                return (
                    f"{path}:\n"
                    "\t@executable_path/../mutable/decoy.dylib "
                    "(compatibility version 0.0.0, current version 0.0.0)\n"
                )
            if arguments == ("-D",):
                return f"{path}:\n"
            if arguments == ("-l",):
                return f"{path}:\n"
            raise AssertionError(arguments)

        with (
            mock.patch.object(builder, "_otool_output", side_effect=otool),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "unsupported @executable_path",
            ),
        ):
            builder._admit_macos_dynamic_tool_closure(
                closure,
                [executable],
            )

    def test_otool_inspects_exactly_one_arm64_slice(self) -> None:
        executable = Path("/root/closure/toolchain/bin/git")
        otool = Path("/root/closure/apple/usr/bin/otool")
        calls: list[list[str]] = []

        def runner(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            calls.append(command)
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=(
                    f"{executable}:\n"
                    "\t/usr/lib/libSystem.B.dylib "
                    "(compatibility version 1.0.0, current version 1.0.0)\n"
                ),
                stderr="",
            )

        with (
            mock.patch.object(builder.sys, "platform", "darwin"),
            mock.patch.object(builder.subprocess, "run", side_effect=runner),
        ):
            output = builder._otool_output(
                executable,
                "-L",
                executable=otool,
            )

        self.assertTrue(output.startswith(f"{executable}:\n"))
        self.assertEqual(
            calls,
            [[str(otool), "-arch", "arm64", "-L", str(executable)]],
        )

    def test_otool_rejects_missing_or_duplicate_arm64_slice(self) -> None:
        executable = Path("/root/closure/toolchain/bin/git")
        otool = Path("/root/closure/apple/usr/bin/otool")
        missing = subprocess.CompletedProcess(
            [],
            1,
            stdout="",
            stderr="file does not contain architecture: arm64\n",
        )
        duplicate = subprocess.CompletedProcess(
            [],
            0,
            stdout=(
                f"{executable} (architecture arm64):\n"
                "\t/usr/lib/libSystem.B.dylib "
                "(compatibility version 1.0.0, current version 1.0.0)\n"
                f"{executable} (architecture arm64):\n"
                "\t/usr/lib/libz.1.dylib "
                "(compatibility version 1.0.0, current version 1.0.0)\n"
            ),
            stderr="",
        )
        with (
            mock.patch.object(builder.sys, "platform", "darwin"),
            mock.patch.object(builder.subprocess, "run", return_value=missing),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "no inspectable arm64",
            ),
        ):
            builder._otool_output(executable, "-L", executable=otool)
        with (
            mock.patch.object(builder.sys, "platform", "darwin"),
            mock.patch.object(builder.subprocess, "run", return_value=duplicate),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "exactly one selected arm64",
            ),
        ):
            builder._otool_output(executable, "-L", executable=otool)

    def test_macho_checker_rejects_inherited_rpath_in_dependency(self) -> None:
        closure = self.external_root / "macho-inherited-rpath"
        executable = closure / "bin/tool"
        dependency = closure / "lib/dependency.dylib"
        executable.parent.mkdir(parents=True)
        dependency.parent.mkdir(parents=True)
        executable.write_bytes(b"Mach-O executable fixture")
        dependency.write_bytes(b"Mach-O dependency fixture")

        def otool(
            path: Path,
            *arguments: str,
            **_kwargs: object,
        ) -> str:
            if path == executable and arguments == ("-L",):
                return (
                    f"{path}:\n"
                    "\t@loader_path/../lib/dependency.dylib "
                    "(compatibility version 0.0.0, current version 0.0.0)\n"
                )
            if path == dependency and arguments == ("-L",):
                return (
                    f"{path}:\n"
                    "\t@rpath/dependency.dylib "
                    "(compatibility version 0.0.0, current version 0.0.0)\n"
                    "\t@rpath/decoy.dylib "
                    "(compatibility version 0.0.0, current version 0.0.0)\n"
                )
            if arguments == ("-D",):
                install_name = (
                    "@rpath/dependency.dylib"
                    if path == dependency
                    else ""
                )
                return f"{path}:\n{install_name}\n"
            if arguments == ("-l",):
                return f"{path}:\n"
            raise AssertionError((path, arguments))

        with (
            mock.patch.object(builder, "_otool_output", side_effect=otool),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "inherited @rpath",
            ),
        ):
            builder._admit_macos_dynamic_tool_closure(
                closure,
                [executable],
            )

    def test_macho_checker_scans_python_extension_for_homebrew_escape(self) -> None:
        closure = self.external_root / "macho-python-runtime"
        python = closure / "python/bin/python3"
        extension = (
            closure
            / "python/lib/python3.14/lib-dynload/_hashlib.cpython-314-darwin.so"
        )
        python.parent.mkdir(parents=True)
        extension.parent.mkdir(parents=True)
        python.write_bytes(b"explicit Python launcher fixture")
        extension.write_bytes(b"\xcf\xfa\xed\xfe" + b"extension fixture")
        inspected: list[Path] = []

        def otool(
            path: Path,
            *arguments: str,
            **_kwargs: object,
        ) -> str:
            inspected.append(path)
            if arguments == ("-L",):
                dependency = (
                    "/opt/homebrew/opt/openssl/lib/libcrypto.3.dylib"
                    if path == extension
                    else "/usr/lib/libSystem.B.dylib"
                )
                return (
                    f"{path}:\n"
                    f"\t{dependency} "
                    "(compatibility version 1.0.0, current version 1.0.0)\n"
                )
            if arguments in {("-D",), ("-l",)}:
                return f"{path}:\n"
            raise AssertionError((path, arguments))

        with (
            mock.patch.object(builder, "_otool_output", side_effect=otool),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "escapes the production closure",
            ),
        ):
            builder._admit_macos_dynamic_tool_closure(
                closure,
                [python],
            )

        self.assertIn(extension, inspected)

    def test_scanned_macho_image_is_not_an_explicit_root_executable(self) -> None:
        closure = self.external_root / "macho-python-rpath"
        python = closure / "python/bin/python3"
        extension = closure / "python/lib/python3.14/site-packages/native.so"
        dependency = closure / "python/lib/libnative.dylib"
        python.parent.mkdir(parents=True)
        extension.parent.mkdir(parents=True)
        dependency.parent.mkdir(parents=True, exist_ok=True)
        python.write_bytes(b"explicit Python launcher fixture")
        extension.write_bytes(b"\xcf\xfa\xed\xfe" + b"extension fixture")
        dependency.write_bytes(b"\xcf\xfa\xed\xfe" + b"dependency fixture")

        def otool(
            path: Path,
            *arguments: str,
            **_kwargs: object,
        ) -> str:
            if arguments == ("-L",):
                dependency_name = (
                    "@rpath/libnative.dylib"
                    if path == extension
                    else "/usr/lib/libSystem.B.dylib"
                )
                return (
                    f"{path}:\n"
                    f"\t{dependency_name} "
                    "(compatibility version 1.0.0, current version 1.0.0)\n"
                )
            if arguments == ("-D",):
                return f"{path}:\n"
            if arguments == ("-l",):
                if path == extension:
                    return (
                        f"{path}:\n"
                        "          cmd LC_RPATH\n"
                        "      cmdsize 48\n"
                        "         path @loader_path/../../.. "
                        "(offset 12)\n"
                    )
                return f"{path}:\n"
            raise AssertionError((path, arguments))

        with (
            mock.patch.object(builder, "_otool_output", side_effect=otool),
            self.assertRaisesRegex(
                builder.CandidateBuildError,
                "inherited @rpath",
            ),
        ):
            builder._admit_macos_dynamic_tool_closure(
                closure,
                [python],
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
        self.assertIn("kagemusha_source_tree_seal.py", build_rs)


if __name__ == "__main__":
    unittest.main()
