from __future__ import annotations

from contextlib import contextmanager, nullcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
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
    def assert_no_owned_live_process_group_member(self, pgid: int) -> None:
        """Accept missing/zombie/reused groups but reject a live owned descendant."""

        try:
            os.killpg(pgid, 0)
        except ProcessLookupError:
            return
        except PermissionError:
            # Darwin reports EPERM for a group containing only zombies.  It
            # also reports EPERM if the numeric PGID was reused solely by an
            # unrelated UID, so inspect without signalling it.
            pass
        completed = subprocess.run(
            ["/bin/ps", "-axo", "pid=,pgid=,stat=,uid="],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="ascii",
            errors="strict",
            timeout=5,
        )
        live_owned: list[str] = []
        for line in completed.stdout.splitlines():
            fields = line.split()
            if len(fields) != 4:
                continue
            _pid_text, pgid_text, state, uid_text = fields
            if (
                int(pgid_text) == pgid
                and int(uid_text) == os.geteuid()
                and not state.startswith("Z")
            ):
                live_owned.append(line.strip())
        self.assertEqual(
            live_owned,
            [],
            f"sealed Cargo process group {pgid} retained live owned members",
        )

    def test_bounded_cargo_runner_kills_output_flood(self) -> None:
        with mock.patch.object(builder, "SEALED_CARGO_STDOUT_MAX_BYTES", 32):
            with self.assertRaisesRegex(
                builder.CandidateBuildError, "stdout byte limit"
            ):
                builder._run_bounded_cargo_command(
                    ["/bin/sh", "-c", "printf '%0100d' 0"],
                    cwd=Path("/"),
                    env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                    drop_privileges=lambda: None,
                )

    def test_bounded_cargo_runner_kills_timeout(self) -> None:
        with mock.patch.object(builder, "SEALED_CARGO_TIMEOUT_SECONDS", 0.05):
            with self.assertRaisesRegex(
                builder.CandidateBuildError, "wall-clock timeout"
            ):
                builder._run_bounded_cargo_command(
                    ["/bin/sh", "-c", "/bin/sleep 1"],
                    cwd=Path("/"),
                    env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                    drop_privileges=lambda: None,
                )

    def test_bounded_cargo_runner_sweeps_success_descendant(self) -> None:
        completed = builder._run_bounded_cargo_command(
            [
                "/bin/sh",
                "-c",
                "echo $$; /bin/sleep 10 </dev/null >/dev/null 2>&1 & exit 0",
            ],
            cwd=Path("/"),
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            drop_privileges=lambda: None,
        )
        self.assertEqual(completed.returncode, 0)
        self.assert_no_owned_live_process_group_member(int(completed.stdout.strip()))

    def test_bounded_cargo_runner_cleans_up_after_read_error(self) -> None:
        original_read = builder.os.read
        failed = False
        pid_path = self.root / "read-error-pgid"

        def fail_first_read(descriptor: int, size: int) -> bytes:
            nonlocal failed
            if not failed:
                failed = True
                raise OSError("synthetic pipe read failure")
            return original_read(descriptor, size)

        with self.assertRaisesRegex(OSError, "synthetic pipe read failure"):
            builder._run_bounded_cargo_command(
                [
                    "/bin/sh",
                    "-c",
                    f"echo $$ > {shlex.quote(str(pid_path))}; printf x; /bin/sleep 10",
                ],
                cwd=Path("/"),
                env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                drop_privileges=lambda: None,
                pipe_reader=fail_first_read,
            )
        self.assertTrue(pid_path.is_file())
        self.assert_no_owned_live_process_group_member(
            int(pid_path.read_text(encoding="ascii").strip())
        )
    def setUp(self) -> None:
        # macOS attaches host-specific provenance xattrs to temporary-directory
        # ancestors.  These tests exercise admission logic with synthetic roots;
        # individual hostile tests restore/replace this inspector explicitly.
        self.extended_metadata_patcher = mock.patch.object(
            builder, "_require_no_extended_metadata_fd"
        )
        self.extended_metadata_inspector = self.extended_metadata_patcher.start()
        self.addCleanup(self.extended_metadata_patcher.stop)
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
        tree_identity = {
            "bytes": 4096,
            "files": 4,
            "records": 6,
            "sha256": "7" * 64,
        }
        self.build_input_closure_value = {
            "cargo_home": {
                "roots": ["git", "registry"],
                "tree": dict(tree_identity),
            },
            "cargo_toolchain": {
                "cargo_relative_path": "bin/cargo",
                "tree": {**tree_identity, "sha256": "d" * 64},
            },
            "developer_dir": {
                "path": "/private/var/db/kagemusha/Xcode/Developer",
                "tree": {**tree_identity, "sha256": "8" * 64},
            },
            "host_tools": [
                {
                    "binary_sha256": f"{index + 1:064x}",
                    "binary_size_bytes": 4096 + index,
                    "path": path,
                    "resolved_path": path,
                }
                for index, path in enumerate(builder.REQUIRED_HOST_TOOL_PATHS)
            ],
            "platform": "darwin",
            "python_runtime": {
                "interpreter_path": (
                    "/private/var/db/iroha-kagemusha-python-runtime-v1/"
                    "python-3.13.7/bin/python3"
                ),
                "interpreter_sha256": "6" * 64,
                "root": (
                    "/private/var/db/iroha-kagemusha-python-runtime-v1/"
                    "python-3.13.7"
                ),
                "tree_sha256": "a" * 64,
            },
            "rust_toolchain": {
                "rustc_relative_path": "bin/rustc",
                "tree": {**tree_identity, "sha256": "b" * 64},
            },
            "runtime_identity": {
                "account_name": builder.RUNTIME_ACCOUNT_NAME,
                "gid": 65_534,
                "group_name": builder.RUNTIME_GROUP_NAME,
                "policy": builder.RUNTIME_IDENTITY_POLICY,
                "uid": 65_534,
            },
            "sandbox": {
                "backend": "macos-seatbelt-v1",
                "os_build": "25F84",
                "profile_schema": (
                    "iroha.kagemusha.sealed_candidate_build_seatbelt.v1"
                ),
                "qualification": [
                    "deny-ambient-read-v1",
                    "deny-ambient-write-v1",
                    "deny-network-v1",
                    "deny-unlisted-exec-v1",
                    "fresh-cargo-rustc-link-v1",
                ],
                "xcode_build": "17C52",
            },
            "schema": builder.BUILD_INPUT_CLOSURE_SCHEMA,
            "sdkroot": {
                "path": (
                    "/private/var/db/kagemusha/Xcode/Developer/Platforms/"
                    "MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
                ),
                "tree": {**tree_identity, "sha256": "c" * 64},
            },
        }
        build_input_payload = builder._canonical_json_line(
            self.build_input_closure_value
        )
        package_root = "/private/controller/source/crates/iroha_core"
        raw_unit_graph_value = {
            "roots": [0],
            "units": [
                {
                    "dependencies": [],
                    "features": list(builder.SOURCE_SEAL_RESOLVED_FEATURES),
                    "mode": "build",
                    "pkg_id": f"iroha_core 3.0.0 (path+file://{package_root})",
                    "platform": builder.SOURCE_SEAL_TARGET,
                    "profile": {
                        "codegen_backend": None,
                        "codegen_units": None,
                        "debug_assertions": False,
                        "debuginfo": 0,
                        "incremental": False,
                        "lto": "false",
                        "name": "release",
                        "opt_level": "3",
                        "overflow_checks": False,
                        "panic": "unwind",
                        "rpath": False,
                        "split_debuginfo": None,
                        "strip": {"resolved": {"Named": "debuginfo"}},
                    },
                    "target": {
                        "crate_types": ["bin"],
                        "doc": True,
                        "doctest": False,
                        "edition": "2024",
                        "kind": ["bin"],
                        "name": builder.BINARY_NAME,
                        "required-features": [
                            "dev-tools",
                            "kagemusha-candidate-evidence-lab",
                            "zk-halo2-ipa",
                        ],
                        "src_path": (
                            f"{package_root}/src/bin/"
                            "kagemusha_recursive_spend_v4_bundle.rs"
                        ),
                        "test": True,
                    },
                }
            ],
            "version": 1,
        }
        self.raw_unit_graph = self.root / "raw-unit-graph.json"
        raw_unit_graph_payload = builder._canonical_json_line(raw_unit_graph_value)
        self.raw_unit_graph.write_bytes(raw_unit_graph_payload)
        self.normalized_unit_graph = self.root / "normalized-unit-graph.json"
        normalized_unit_graph_payload = builder.normalize_source_seal_unit_graph(
            raw_unit_graph_payload
        )
        self.normalized_unit_graph.write_bytes(normalized_unit_graph_payload)
        self.raw_unit_graph_sha256 = hashlib.sha256(raw_unit_graph_payload).hexdigest()
        self.normalized_unit_graph_sha256 = hashlib.sha256(
            normalized_unit_graph_payload
        ).hexdigest()
        unit_graph_summary = builder._unit_graph_summary(normalized_unit_graph_payload)
        self.unit_graph_summary = unit_graph_summary
        self.raw_unit_graph_size_bytes = len(raw_unit_graph_payload)
        self.normalized_unit_graph_size_bytes = len(normalized_unit_graph_payload)
        self.capture_receipt = {
            "build_inputs_sha256": hashlib.sha256(build_input_payload).hexdigest(),
            "cargo_binary_sha256": hashlib.sha256(self.cargo.read_bytes()).hexdigest(),
            "exit_status": 0,
            "raw_stdout_sha256": self.raw_unit_graph_sha256,
            "raw_stdout_size_bytes": len(raw_unit_graph_payload),
            "rustc_binary_sha256": hashlib.sha256(self.rustc.read_bytes()).hexdigest(),
            "schema": builder.SOURCE_SEAL_CAPTURE_RECEIPT_SCHEMA,
            "source_commit": self.identity.source_commit,
            "source_tree_sha256": self.identity.source_tree_sha256,
            "stderr_sha256": hashlib.sha256(b"").hexdigest(),
            "stderr_size_bytes": 0,
        }
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
                "build_inputs_hex": build_input_payload.hex(),
                "build_inputs_sha256": hashlib.sha256(
                    build_input_payload
                ).hexdigest(),
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
                        **unit_graph_summary,
                        "capture_receipt": dict(self.capture_receipt),
                        "raw_sha256": self.raw_unit_graph_sha256,
                        "raw_size_bytes": len(raw_unit_graph_payload),
                    },
                },
                "execution_policy_sha256": "e" * 64,
                "schema": builder.SOURCE_SEAL_OUTER_POLICY_SCHEMA,
                "toolchain": {
                    "cargo": {
                        "binary_sha256": hashlib.sha256(
                            self.cargo.read_bytes()
                        ).hexdigest(),
                        "binary_size_bytes": self.cargo.stat().st_size,
                    },
                    "rustc": {
                        "binary_sha256": hashlib.sha256(
                            self.rustc.read_bytes()
                        ).hexdigest(),
                        "binary_size_bytes": self.rustc.stat().st_size,
                    },
                },
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
            executable = (
                Path(command[command.index("--target-dir") + 1])
                / "release"
                / builder.BINARY_NAME
            )
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
        kwargs.setdefault("runtime_lock", lambda _uid: nullcontext())
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
        kwargs.setdefault("raw_unit_graph", self.raw_unit_graph)
        kwargs.setdefault("raw_unit_graph_sha256", self.raw_unit_graph_sha256)
        kwargs.setdefault("normalized_unit_graph", self.normalized_unit_graph)
        kwargs.setdefault(
            "normalized_unit_graph_sha256", self.normalized_unit_graph_sha256
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
        kwargs.setdefault("runtime_uid", 65_534)
        kwargs.setdefault("runtime_gid", 65_534)
        kwargs.setdefault("input_snapshotter", self.passthrough_snapshotter)
        kwargs.setdefault(
            "unit_graph_preflight",
            lambda _inputs, _environment, evidence: {
                "capture_exit_status": evidence.capture_receipt["exit_status"],
                "capture_stderr_sha256": evidence.capture_receipt["stderr_sha256"],
                "capture_stderr_size_bytes": evidence.capture_receipt[
                    "stderr_size_bytes"
                ],
                "controller_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
                "controller_raw_size_bytes": len(evidence.raw),
                "fresh_exit_status": evidence.capture_receipt["exit_status"],
                "fresh_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
                "fresh_raw_size_bytes": len(evidence.raw),
                "fresh_stderr_sha256": evidence.capture_receipt["stderr_sha256"],
                "fresh_stderr_size_bytes": evidence.capture_receipt[
                    "stderr_size_bytes"
                ],
                "normalized_sha256": hashlib.sha256(evidence.normalized).hexdigest(),
                "normalized_size_bytes": len(evidence.normalized),
                **evidence.summary,
            },
        )
        return builder.build_candidate_bundle(root, cargo, **kwargs)

    def passthrough_snapshotter(
        self,
        root: Path,
        target_dir: Path,
        _identity: builder.source_seal.SourceIdentity,
        _reviewed_source_closure: Path,
        _reviewed_source_closure_sha256: str,
        cargo_home: Path,
        cargo: builder.AdmittedExecutable,
        rustc: builder.AdmittedExecutable,
        _build_inputs: dict[str, object],
        _identity_reader: object,
        _runtime_uid: int,
        _runtime_gid: int,
    ) -> builder.MaterializedBuildInputs:
        """Avoid privileged multi-gigabyte snapshots in focused unit tests."""

        verification_target = target_dir / "verification-target"
        verification_target.mkdir(mode=0o700)
        verification_source = target_dir / "verification-source"
        verification_source.mkdir(mode=0o700)
        verification_temporary = target_dir / "verification-temporary"
        verification_temporary.mkdir(mode=0o700)
        return builder.MaterializedBuildInputs(
            source_root=root,
            verification_source_root=verification_source,
            cargo_home=cargo_home,
            cargo=cargo,
            rustc=rustc,
            host_tool_bin=Path("/usr/bin"),
            target_dir=target_dir,
            verification_target_dir=verification_target,
            unit_graph_target_dir=verification_target,
            temporary_dir=self.external_root,
            verification_temporary_dir=verification_temporary,
            unit_graph_temporary_dir=verification_temporary,
            output_uid=os.geteuid(),
            launch_prefix=(),
            verification_launch_prefix=(),
            unit_graph_launch_prefix=(),
            drop_privileges=lambda: None,
            reset_cargo_lock=lambda: None,
            revalidate=lambda: None,
        )

    def copied_tool_snapshotter(self, *args: object) -> builder.MaterializedBuildInputs:
        """Model the production inode-independent Cargo/rustc snapshots."""

        root = args[0]
        target_dir = args[1]
        cargo_home = args[5]
        cargo = args[6]
        rustc = args[7]
        assert isinstance(root, Path)
        assert isinstance(target_dir, Path)
        assert isinstance(cargo_home, Path)
        assert isinstance(cargo, builder.AdmittedExecutable)
        assert isinstance(rustc, builder.AdmittedExecutable)
        tool_dir = target_dir / "fixture-tool-snapshot"
        tool_dir.mkdir(mode=0o700)
        copied_cargo_path = tool_dir / "cargo"
        copied_rustc_path = tool_dir / "rustc"
        copied_cargo_path.write_bytes(cargo.resolved_path.read_bytes())
        copied_rustc_path.write_bytes(rustc.resolved_path.read_bytes())
        copied_cargo_path.chmod(0o700)
        copied_rustc_path.chmod(0o700)
        verification_target = target_dir / "verification-target"
        verification_target.mkdir(mode=0o700)
        verification_source = target_dir / "verification-source"
        verification_source.mkdir(mode=0o700)
        verification_temporary = target_dir / "verification-temporary"
        verification_temporary.mkdir(mode=0o700)
        return builder.MaterializedBuildInputs(
            source_root=root,
            verification_source_root=verification_source,
            cargo_home=cargo_home,
            cargo=builder._admit_direct_executable(str(copied_cargo_path), "Cargo"),
            rustc=builder._admit_direct_executable(str(copied_rustc_path), "rustc"),
            host_tool_bin=Path("/usr/bin"),
            target_dir=target_dir,
            verification_target_dir=verification_target,
            unit_graph_target_dir=verification_target,
            temporary_dir=self.external_root,
            verification_temporary_dir=verification_temporary,
            unit_graph_temporary_dir=verification_temporary,
            output_uid=os.geteuid(),
            launch_prefix=(),
            verification_launch_prefix=(),
            unit_graph_launch_prefix=(),
            drop_privileges=lambda: None,
            reset_cargo_lock=lambda: None,
            revalidate=lambda: None,
        )

    def test_build_command_and_report_bind_exact_source_and_release_binary(self) -> None:
        identities: list[Path] = []
        invocations: list[tuple[list[str], dict[str, str]]] = []
        invocation_kwargs: list[dict[str, object]] = []
        working_directories: list[Path] = []
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
            invocation_kwargs.append(dict(kwargs))
            working_directories.append(kwargs["cwd"])  # type: ignore[arg-type]
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
        self.assertEqual(len(invocations), 2)
        for kwargs in invocation_kwargs:
            self.assertIs(kwargs["stdin"], subprocess.DEVNULL)
            self.assertIs(kwargs["stdout"], subprocess.PIPE)
            self.assertIs(kwargs["stderr"], subprocess.PIPE)
            self.assertTrue(kwargs["close_fds"])
            self.assertEqual(kwargs["timeout"], builder.SEALED_CARGO_TIMEOUT_SECONDS)
        self.assertEqual(len(set(working_directories)), 2)
        command, environment = invocations[0]
        self.assertEqual(command[0], str(self.cargo.resolve()))
        self.assertIn("--release", command)
        self.assertIn("--locked", command)
        self.assertIn("--offline", command)
        self.assertEqual(
            command[command.index("--target") + 1],
            builder.SOURCE_SEAL_TARGET,
        )
        self.assertEqual(
            command[command.index("--target-dir") + 1],
            str(self.target_dir),
        )
        self.assertNotEqual(
            invocations[0][0][invocations[0][0].index("--target-dir") + 1],
            invocations[1][0][invocations[1][0].index("--target-dir") + 1],
        )
        self.assertNotEqual(invocations[0][1]["TMPDIR"], invocations[1][1]["TMPDIR"])
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
            "HOST_AR",
            "LIBRARY_PATH",
            "MACOSX_DEPLOYMENT_TARGET",
            "PKG_CONFIG_PATH",
            "aarch64_apple_darwin_AR",
        ):
            self.assertNotIn(native_control, environment)
        self.assertEqual(environment["RUSTC_WRAPPER"], "")
        self.assertEqual(environment["RUSTFLAGS"], "")
        self.assertEqual(environment["CARGO_HOME"], str(self.cargo_home))
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(environment["HOME"], "/var/empty")
        self.assertEqual(environment["PATH"], "/usr/bin")
        self.assertEqual(environment["TMPDIR"], str(self.external_root))
        self.assertEqual(
            environment["DEVELOPER_DIR"],
            self.build_input_closure_value["developer_dir"]["path"],
        )
        self.assertEqual(
            environment["SDKROOT"],
            self.build_input_closure_value["sdkroot"]["path"],
        )
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
            "KAGEMUSHA_BUILD_BUILD_INPUTS_HEX",
            "KAGEMUSHA_BUILD_BUILD_INPUTS_SHA256",
            "KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SIZE_BYTES",
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256",
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SIZE_BYTES",
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
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_EXIT_STATUS",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_STDERR_SHA256",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CAPTURE_STDERR_SIZE_BYTES",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SHA256",
            "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SIZE_BYTES",
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
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256": hashlib.sha256(
                self.cargo.read_bytes()
            ).hexdigest(),
            "KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SIZE_BYTES": str(
                self.cargo.stat().st_size
            ),
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256": hashlib.sha256(
                self.rustc.read_bytes()
            ).hexdigest(),
            "KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SIZE_BYTES": str(
                self.rustc.stat().st_size
            ),
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
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_PACKAGES": str(
                self.unit_graph_summary["custom_build_packages"]
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS": str(
                self.unit_graph_summary["custom_build_units"]
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS": str(
                self.unit_graph_summary["iroha_core_units"]
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES": str(
                self.unit_graph_summary["packages"]
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SHA256": self.raw_unit_graph_sha256,
            "KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SIZE_BYTES": str(
                self.raw_unit_graph_size_bytes
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256": self.normalized_unit_graph_sha256,
            "KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES": str(
                self.normalized_unit_graph_size_bytes
            ),
            "KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS": str(
                self.unit_graph_summary["units"]
            ),
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
        self.assertEqual(report["reproducible_build_count"], 2)
        self.assertEqual(report["schema"], builder.SEALED_DOUBLE_BUILD_REPORT_SCHEMA)
        self.assertEqual(len(report["builds"]), 2)
        self.assertEqual(
            [entry["identity"]["ordinal"] for entry in report["builds"]],
            [1, 2],
        )
        for entry in report["builds"]:
            self.assertEqual(
                entry["identity_sha256"],
                hashlib.sha256(
                    builder._canonical_json_line(entry["identity"])
                ).hexdigest(),
            )
            self.assertEqual(entry["output"]["sha256"], report["binary_sha256"])
            self.assertEqual(
                entry["output"]["size_bytes"], report["binary_size_bytes"]
            )
        self.assertEqual(
            report["candidate_generator"],
            {
                "selected_build_ordinal": 1,
                "sha256": report["binary_sha256"],
                "size_bytes": report["binary_size_bytes"],
            },
        )
        self.assertEqual(report["byte_equality"]["equal"], True)
        self.assertNotIn("cargo_executable_sha256", report)
        self.assertNotIn("rustc_executable_sha256", report)

    def test_build_environment_is_the_exact_signed_allowlist(self) -> None:
        hostile_controls = {
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
            # These names are consumed directly by crates/norito/build.rs and
            # used to survive the blacklist, changing compiled behavior with
            # no change to the signed source or Cargo unit graph.
            "ENABLE_CABAC": "1",
            "ENABLE_TRELLIS": "1",
            "CONNECT_NORITO_ACCEL": "hostile",
            "DOCS_RS": "1",
            "SOME_FUTURE_BUILD_SCRIPT_INPUT": "hostile",
        }
        rustc = builder._admit_direct_executable(str(self.rustc), "rustc")

        with mock.patch.dict(
            os.environ,
            {**hostile_controls, "LANG": "C.UTF-8", "TZ": "Pacific/Honolulu"},
            clear=True,
        ):
            environment = builder._sanitized_build_environment(self.cargo_home, rustc)

        expected = dict(builder.SOURCE_SEAL_CAPTURE_ENVIRONMENT)
        expected["CARGO_HOME"] = str(self.cargo_home)
        expected["RUSTC"] = rustc.command_path
        self.assertEqual(environment, expected)
        self.assertEqual(environment["LANG"], "C")
        self.assertEqual(environment["LC_ALL"], "C")
        self.assertEqual(environment["TZ"], "UTC")
        self.assertTrue(
            (hostile_controls.keys() - {"DEVELOPER_DIR", "SDKROOT"}).isdisjoint(
                environment
            ),
            "no ambient name may survive the signed environment allowlist",
        )
        self.assertNotEqual(environment["DEVELOPER_DIR"], hostile_controls["DEVELOPER_DIR"])
        self.assertNotEqual(environment["SDKROOT"], hostile_controls["SDKROOT"])

    def test_same_uid_snapshot_materialization_is_rejected(self) -> None:
        """Read-only mode bits are not an immutability boundary for their owner."""

        with mock.patch.object(builder.os, "geteuid", return_value=501):
            with self.assertRaisesRegex(
                builder.CandidateBuildError,
                "must run as root",
            ):
                builder._materialize_sealed_build_inputs(
                    self.root,
                    self.external_root,
                    self.identity,
                    self.reviewed_source_closure,
                    self.reviewed_source_closure_sha256,
                    self.cargo_home,
                    builder._admit_direct_executable(str(self.cargo), "Cargo"),
                    builder._admit_direct_executable(str(self.rustc), "rustc"),
                    self.build_input_closure_value,
                    lambda _root, _path, _sha256: self.identity,
                    65_534,
                    65_534,
                )

    def test_stock_stable_cargo_unit_graph_preflight_is_rejected(self) -> None:
        materialized = builder.MaterializedBuildInputs(
            source_root=self.root,
            verification_source_root=self.root,
            cargo_home=self.cargo_home,
            cargo=builder._admit_direct_executable(str(self.cargo), "Cargo"),
            rustc=builder._admit_direct_executable(str(self.rustc), "rustc"),
            host_tool_bin=Path("/usr/bin"),
            target_dir=self.external_root,
            verification_target_dir=self.external_root,
            unit_graph_target_dir=self.external_root,
            temporary_dir=self.external_root,
            verification_temporary_dir=self.external_root,
            unit_graph_temporary_dir=self.external_root,
            output_uid=os.geteuid(),
            launch_prefix=(),
            verification_launch_prefix=(),
            unit_graph_launch_prefix=(),
            drop_privileges=lambda: None,
            reset_cargo_lock=lambda: None,
            revalidate=lambda: None,
        )
        stable_failure = subprocess.CompletedProcess(
            [],
            101,
            stdout=b"",
            stderr=b"the -Z flag is only accepted on the nightly channel\n",
        )
        with mock.patch.object(
            builder, "_run_bounded_cargo_command", return_value=stable_failure
        ) as bounded_runner:
            with self.assertRaisesRegex(
                builder.CandidateBuildError,
                "pinned nightly Cargo cannot emit",
            ):
                builder._preflight_unit_graph_launch(
                    materialized,
                    {},
                    builder.UnitGraphEvidence(
                        raw=self.raw_unit_graph.read_bytes(),
                        normalized=self.normalized_unit_graph.read_bytes(),
                        summary=self.unit_graph_summary,
                        capture_receipt=dict(self.capture_receipt),
                    ),
                )
        self.assertEqual(bounded_runner.call_args.kwargs["timeout_seconds"], 300)
        self.assertEqual(
            bounded_runner.call_args.kwargs["stdout_max_bytes"], 16 * 1024 * 1024
        )
        self.assertEqual(
            bounded_runner.call_args.kwargs["stderr_max_bytes"], 16 * 1024 * 1024
        )

    def test_fresh_nightly_unit_graph_must_equal_signed_normalization(self) -> None:
        """A successful nightly launch cannot substitute a different unit graph."""

        materialized = builder.MaterializedBuildInputs(
            source_root=self.root,
            verification_source_root=self.root,
            cargo_home=self.cargo_home,
            cargo=builder._admit_direct_executable(str(self.cargo), "Cargo"),
            rustc=builder._admit_direct_executable(str(self.rustc), "rustc"),
            host_tool_bin=Path("/usr/bin"),
            target_dir=self.external_root,
            verification_target_dir=self.external_root,
            unit_graph_target_dir=self.external_root,
            temporary_dir=self.external_root,
            verification_temporary_dir=self.external_root,
            unit_graph_temporary_dir=self.external_root,
            output_uid=os.geteuid(),
            launch_prefix=(),
            verification_launch_prefix=(),
            unit_graph_launch_prefix=(),
            drop_privileges=lambda: None,
            reset_cargo_lock=lambda: None,
            revalidate=lambda: None,
        )
        evidence = builder.UnitGraphEvidence(
            raw=self.raw_unit_graph.read_bytes(),
            normalized=self.normalized_unit_graph.read_bytes(),
            summary=self.unit_graph_summary,
            capture_receipt=dict(self.capture_receipt),
        )
        substituted = json.loads(self.raw_unit_graph.read_bytes())
        substituted["units"][0]["target"]["src_path"] = (
            "/private/controller/source/crates/iroha_core/src/lib.rs"
        )
        result = subprocess.CompletedProcess(
            [],
            0,
            stdout=builder._canonical_json_line(substituted),
            stderr=b"",
        )
        with mock.patch.object(
            builder, "_run_bounded_cargo_command", return_value=result
        ):
            with self.assertRaisesRegex(
                builder.CandidateBuildError,
                "differs from the controller-signed graph",
            ):
                builder._preflight_unit_graph_launch(materialized, {}, evidence)

    def test_fresh_nightly_stderr_must_equal_signed_capture_receipt(self) -> None:
        """Successful stdout cannot hide unsigned diagnostics from the fresh launch."""

        materialized = builder.MaterializedBuildInputs(
            source_root=self.root,
            verification_source_root=self.root,
            cargo_home=self.cargo_home,
            cargo=builder._admit_direct_executable(str(self.cargo), "Cargo"),
            rustc=builder._admit_direct_executable(str(self.rustc), "rustc"),
            host_tool_bin=Path("/usr/bin"),
            target_dir=self.external_root,
            verification_target_dir=self.external_root,
            unit_graph_target_dir=self.external_root,
            temporary_dir=self.external_root,
            verification_temporary_dir=self.external_root,
            unit_graph_temporary_dir=self.external_root,
            output_uid=os.geteuid(),
            launch_prefix=(),
            verification_launch_prefix=(),
            unit_graph_launch_prefix=(),
            drop_privileges=lambda: None,
            reset_cargo_lock=lambda: None,
            revalidate=lambda: None,
        )
        evidence = builder.UnitGraphEvidence(
            raw=self.raw_unit_graph.read_bytes(),
            normalized=self.normalized_unit_graph.read_bytes(),
            summary=self.unit_graph_summary,
            capture_receipt=dict(self.capture_receipt),
        )
        result = subprocess.CompletedProcess(
            [],
            0,
            stdout=self.raw_unit_graph.read_bytes(),
            stderr=b"unsigned warning\n",
        )
        with mock.patch.object(
            builder, "_run_bounded_cargo_command", return_value=result
        ):
            with self.assertRaisesRegex(
                builder.CandidateBuildError,
                "exit/stderr differ from the signed capture receipt",
            ):
                builder._preflight_unit_graph_launch(materialized, {}, evidence)

    def test_unit_graph_set_like_duplicates_fail_closed(self) -> None:
        """Normalization cannot silently erase repeated roots or feature values."""

        for mutation in ("root", "feature"):
            with self.subTest(mutation=mutation):
                graph = json.loads(self.raw_unit_graph.read_bytes())
                if mutation == "root":
                    graph["roots"].append(graph["roots"][0])
                    diagnostic = "roots repeat"
                else:
                    graph["units"][0]["features"] = ["dev-tools", "dev-tools"]
                    diagnostic = "repeats a set-like value"
                with self.assertRaisesRegex(builder.CandidateBuildError, diagnostic):
                    builder.normalize_source_seal_unit_graph(
                        builder._canonical_json_line(graph)
                    )

    def test_path_package_normalization_preserves_repository_location(self) -> None:
        """Same-name/version path packages in different directories cannot collide."""

        left = json.loads(self.raw_unit_graph.read_bytes())
        shadow = json.loads(json.dumps(left["units"][0]))
        shadow["pkg_id"] = (
            "shadow 1.0.0 "
            "(path+file:///private/controller/source/crates/shadow-left)"
        )
        shadow["target"]["src_path"] = (
            "/private/controller/source/crates/shadow-left/src/lib.rs"
        )
        left["units"].append(shadow)
        right = json.loads(json.dumps(left))
        right["units"][-1]["pkg_id"] = (
            "shadow 1.0.0 "
            "(path+file:///private/controller/source/crates/shadow-right)"
        )
        right["units"][-1]["target"]["src_path"] = (
            "/private/controller/source/crates/shadow-right/src/lib.rs"
        )

        left_normalized = builder.normalize_source_seal_unit_graph(
            builder._canonical_json_line(left)
        )
        right_normalized = builder.normalize_source_seal_unit_graph(
            builder._canonical_json_line(right)
        )

        self.assertNotEqual(left_normalized, right_normalized)
        self.assertIn(b"<SOURCE_ROOT>/crates/shadow-left", left_normalized)
        self.assertIn(b"<SOURCE_ROOT>/crates/shadow-right", right_normalized)

    def test_seatbelt_profile_does_not_broaden_unbound_host_inputs(self) -> None:
        """Mach, sysctl, root reads, and unlisted execution stay fail-closed."""

        profile = builder._sealed_build_seatbelt_profile(
            source_roots=(self.root / "source-a", self.root / "source-b"),
            cargo_home=self.external_root / "cargo-home-snapshot",
            cargo_toolchain=self.external_root / "cargo-toolchain",
            rustc_toolchain=self.external_root / "rustc-toolchain",
            host_tools=self.external_root / "host-tools",
            developer_dir=Path("/private/var/db/kagemusha/Xcode/Developer"),
            sdkroot=Path(
                "/private/var/db/kagemusha/Xcode/Developer/Platforms/"
                "MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
            ),
            output_dirs=(self.external_root / "output-a", self.external_root / "output-b"),
            temporary_dirs=(
                self.external_root / "temporary-a",
                self.external_root / "temporary-b",
            ),
        ).decode("ascii")
        self.assertNotIn("(allow mach", profile)
        self.assertNotIn("sysctl-read", profile)
        self.assertNotIn('(literal "/")', profile)
        self.assertNotIn('(literal "/usr/bin/true")', profile)
        self.assertIn(
            f'(allow process-exec (subpath "{self.external_root / "output-a"}"))',
            profile,
        )

    def test_seatbelt_build_profiles_exclude_sibling_target_and_tmp(self) -> None:
        """A and B profiles cannot use either sibling tree as a data channel."""

        common = {
            "cargo_home": self.external_root / "cargo-home-snapshot",
            "cargo_toolchain": self.external_root / "cargo-toolchain",
            "rustc_toolchain": self.external_root / "rustc-toolchain",
            "host_tools": self.external_root / "host-tools",
            "developer_dir": Path("/private/var/db/kagemusha/Xcode/Developer"),
            "sdkroot": Path(
                "/private/var/db/kagemusha/Xcode/Developer/Platforms/"
                "MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
            ),
        }
        output_a = self.external_root / "output-a"
        output_b = self.external_root / "output-b"
        temporary_a = self.external_root / "temporary-a"
        temporary_b = self.external_root / "temporary-b"
        profile_a = builder._sealed_build_seatbelt_profile(
            source_roots=(self.root / "source-a",),
            output_dirs=(output_a,),
            temporary_dirs=(temporary_a,),
            **common,
        ).decode("ascii")
        profile_b = builder._sealed_build_seatbelt_profile(
            source_roots=(self.root / "source-b",),
            output_dirs=(output_b,),
            temporary_dirs=(temporary_b,),
            **common,
        ).decode("ascii")

        self.assertNotIn(str(output_b), profile_a)
        self.assertNotIn(str(temporary_b), profile_a)
        self.assertNotIn(str(output_a), profile_b)
        self.assertNotIn(str(temporary_a), profile_b)

    def test_seatbelt_isolation_runs_write_and_read_copy_probes(self) -> None:
        """Qualification exercises both directions of the independent-build boundary."""

        output_a = self.external_root / "probe-output-a"
        output_b = self.external_root / "probe-output-b"
        output_a.mkdir()
        output_b.mkdir()
        denied = subprocess.CompletedProcess([], 1, stdout=b"", stderr=b"")
        with mock.patch.object(builder.subprocess, "run", return_value=denied) as run:
            builder._prove_seatbelt_build_isolation(
                build_a_launch_prefix=("sandbox-a",),
                build_b_launch_prefix=("sandbox-b",),
                build_a_source_root=self.root,
                build_b_source_root=self.root,
                build_a_environment={},
                build_b_environment={},
                build_a_output_dir=output_a,
                build_b_output_dir=output_b,
                host_tool_bin=Path("/sealed-tools"),
                drop_privileges=lambda: None,
            )
        self.assertEqual(run.call_count, 2)
        first_command = run.call_args_list[0].args[0]
        second_command = run.call_args_list[1].args[0]
        self.assertEqual(first_command[0], "sandbox-a")
        self.assertIn(str(output_b / ".seatbelt-build-a-hostile-write"), first_command)
        self.assertEqual(second_command[0], "sandbox-b")
        self.assertIn(str(output_a / ".seatbelt-build-a-read-probe"), second_command)
        self.assertIn(str(output_b / ".seatbelt-build-b-hostile-copy"), second_command)

    def test_cargo_lock_mutation_is_rejected_before_cross_run_reset(self) -> None:
        """A run cannot pass bytes to its successor through Cargo's lock leaf."""

        lock = self.external_root / ".package-cache"
        lock.write_bytes(b"hostile cross-run state")
        lock.chmod(0o600)
        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "not an empty private single-link file",
        ):
            builder._require_empty_private_cargo_lock(
                lock, os.geteuid(), os.getegid()
            )

    @unittest.skipUnless(os.geteuid() == 0, "requires a real root-to-runtime UID drop")
    def test_runtime_child_cannot_chmod_or_write_root_snapshot(self) -> None:
        root = self.external_root / "root-sealed-input"
        root.mkdir(mode=0o555)
        root.chmod(0o555)
        builder._prove_runtime_cannot_mutate_snapshots(65_534, 65_534, (root,))

    def test_build_input_closure_rejects_broad_xcode_roots(self) -> None:
        """A valid tree digest cannot authorize a mutable workstation Xcode root."""

        for developer, sdk in (
            (
                "/Applications/Xcode.app/Contents/Developer",
                "/Applications/Xcode.app/Contents/Developer/Platforms/"
                "MacOSX.platform/Developer/SDKs/MacOSX.sdk",
            ),
            ("/Applications", "/Applications"),
        ):
            with self.subTest(developer=developer):
                closure = json.loads(json.dumps(self.build_input_closure_value))
                closure["developer_dir"]["path"] = developer
                closure["sdkroot"]["path"] = sdk
                with self.assertRaisesRegex(
                    builder.CandidateBuildError, "fixed private Xcode layout"
                ):
                    builder._validate_build_input_closure(closure)

    def test_cargo_home_admission_checks_every_ancestor_for_acl_and_xattrs(self) -> None:
        """Leaf-only metadata checks cannot hide authority on a parent directory."""

        self.extended_metadata_inspector.reset_mock()
        admitted = builder._admit_cargo_home(self.root, self.cargo_home)
        self.assertEqual(admitted, self.cargo_home)
        labels = [call.args[1] for call in self.extended_metadata_inspector.call_args_list]
        self.assertIn("Cargo home ancestry component /", labels)
        self.assertIn(
            f"Cargo home ancestry component {self.cargo_home.parent}", labels
        )
        self.assertIn(f"Cargo home ancestry component {self.cargo_home}", labels)

        with mock.patch.object(
            builder,
            "_require_no_extended_metadata_fd",
            side_effect=builder.CandidateBuildError("hostile ancestor xattr"),
        ):
            with self.assertRaisesRegex(
                builder.CandidateBuildError, "hostile ancestor xattr"
            ):
                builder._admit_cargo_home(self.root, self.cargo_home)

    def test_dedicated_runtime_process_inventory_rejects_concurrent_uid(self) -> None:
        """The service account cannot already own any process at admission."""

        inventory = subprocess.CompletedProcess(
            ["/bin/ps"], 0, stdout=b"0\n65534\n501\n", stderr=b""
        )
        with mock.patch.object(builder.subprocess, "run", return_value=inventory):
            with self.assertRaisesRegex(
                builder.CandidateBuildError, "already owns a concurrent process"
            ):
                builder._require_no_process_for_runtime_uid(Path("/bin/ps"), 65_534)

    def test_requested_runtime_identity_must_equal_signed_closure(self) -> None:
        """A caller-selected spare UID cannot replace the authenticated account."""

        command_runner = mock.Mock()
        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "requested runtime UID/GID differ from the authenticated",
        ):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                runtime_uid=65_533,
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=command_runner,
            )
        command_runner.assert_not_called()

    def test_runtime_identity_lease_spans_snapshot_preflight_and_both_builds(self) -> None:
        """The same-UID exclusion lease covers every attacker-relevant phase."""

        active = False
        events: list[str] = []

        @contextmanager
        def runtime_lock(uid: int):
            nonlocal active
            self.assertEqual(uid, 65_534)
            self.assertFalse(active)
            events.append("runtime-enter")
            active = True
            try:
                yield
            finally:
                active = False
                events.append("runtime-exit")

        def snapshotter(*args: object) -> builder.MaterializedBuildInputs:
            self.assertTrue(active)
            events.append("snapshot")
            return self.passthrough_snapshotter(*args)

        def preflight(
            _inputs: builder.MaterializedBuildInputs,
            _environment: dict[str, str],
            evidence: builder.UnitGraphEvidence,
        ) -> dict[str, int | str]:
            self.assertTrue(active)
            events.append("preflight")
            return {
                "capture_exit_status": evidence.capture_receipt["exit_status"],
                "capture_stderr_sha256": evidence.capture_receipt["stderr_sha256"],
                "capture_stderr_size_bytes": evidence.capture_receipt[
                    "stderr_size_bytes"
                ],
                "controller_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
                "controller_raw_size_bytes": len(evidence.raw),
                "fresh_exit_status": evidence.capture_receipt["exit_status"],
                "fresh_raw_sha256": hashlib.sha256(evidence.raw).hexdigest(),
                "fresh_raw_size_bytes": len(evidence.raw),
                "fresh_stderr_sha256": evidence.capture_receipt["stderr_sha256"],
                "fresh_stderr_size_bytes": evidence.capture_receipt[
                    "stderr_size_bytes"
                ],
                "normalized_sha256": hashlib.sha256(evidence.normalized).hexdigest(),
                "normalized_size_bytes": len(evidence.normalized),
                **evidence.summary,
            }

        def run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(active)
            events.append("build")
            return self.cargo_result(command)

        self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            runtime_lock=runtime_lock,
            input_snapshotter=snapshotter,
            unit_graph_preflight=preflight,
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=run,
        )
        self.assertEqual(
            events,
            [
                "runtime-enter",
                "snapshot",
                "preflight",
                "build",
                "build",
                "runtime-exit",
            ],
        )

    @unittest.skipUnless(sys.platform == "darwin", "Darwin helper closure")
    def test_real_darwin_host_tool_allowlist_has_fingerprintable_executables(self) -> None:
        observed = []
        for path in builder.REQUIRED_HOST_TOOL_PATHS:
            tool = builder._admit_direct_executable(path, f"host helper {path}")
            observed.append((path, tool.resolved_path, tool.sha256))
        self.assertEqual([entry[0] for entry in observed], list(builder.REQUIRED_HOST_TOOL_PATHS))
        self.assertTrue(all(len(entry[2]) == 64 and entry[2] != "0" * 64 for entry in observed))

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

    def test_original_rustc_transient_swap_cannot_reach_private_snapshot(self) -> None:
        original = self.rustc.read_bytes()

        def command_runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertNotEqual(kwargs["env"]["RUSTC"], str(self.rustc))
            result = self.cargo_result(command)
            self.rustc.write_bytes(b"substituted rustc fixture\n")
            self.rustc.chmod(0o700)
            self.rustc.write_bytes(original)
            self.rustc.chmod(0o700)
            return result

        self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=command_runner,
            input_snapshotter=self.copied_tool_snapshotter,
        )

    def test_original_cargo_transient_swap_cannot_reach_private_snapshot(self) -> None:
        original = self.cargo.read_bytes()

        def command_runner(
            command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertNotEqual(command[0], str(self.cargo))
            result = self.cargo_result(command)
            self.cargo.write_bytes(b"substituted cargo fixture\n")
            self.cargo.chmod(0o700)
            self.cargo.write_bytes(original)
            self.cargo.chmod(0o700)
            return result

        self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=command_runner,
            input_snapshotter=self.copied_tool_snapshotter,
        )

    def test_original_source_transient_swap_cannot_reach_commit_snapshot(self) -> None:
        original_marker = self.root / "transient-source-marker"
        original_marker.write_bytes(b"reviewed\n")
        private_source = self.external_root / "private-commit-source"
        private_source.mkdir(mode=0o700)

        def source_snapshotter(*args: object) -> builder.MaterializedBuildInputs:
            materialized = self.passthrough_snapshotter(*args)
            return builder.MaterializedBuildInputs(
                source_root=private_source,
                verification_source_root=private_source,
                cargo_home=materialized.cargo_home,
                cargo=materialized.cargo,
                rustc=materialized.rustc,
                host_tool_bin=materialized.host_tool_bin,
                target_dir=materialized.target_dir,
                verification_target_dir=materialized.verification_target_dir,
                unit_graph_target_dir=materialized.unit_graph_target_dir,
                temporary_dir=materialized.temporary_dir,
                verification_temporary_dir=materialized.verification_temporary_dir,
                unit_graph_temporary_dir=materialized.unit_graph_temporary_dir,
                output_uid=materialized.output_uid,
                launch_prefix=materialized.launch_prefix,
                verification_launch_prefix=materialized.verification_launch_prefix,
                unit_graph_launch_prefix=materialized.unit_graph_launch_prefix,
                drop_privileges=materialized.drop_privileges,
                reset_cargo_lock=materialized.reset_cargo_lock,
                revalidate=materialized.revalidate,
            )

        def command_runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(kwargs["cwd"], private_source)
            original_marker.write_bytes(b"hostile transient bytes\n")
            original_marker.write_bytes(b"reviewed\n")
            return self.cargo_result(command)

        self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=command_runner,
            input_snapshotter=source_snapshotter,
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

    def test_actual_tools_must_match_authenticated_execution_policy(self) -> None:
        """Independent build pins cannot replace the signed graph tool identities."""

        mutations = (
            ("cargo", "binary_sha256", "9" * 64, "Cargo"),
            (
                "rustc",
                "binary_size_bytes",
                self.rustc.stat().st_size + 1,
                "rustc",
            ),
        )
        for mutation_index, (tool, field, value, diagnostic) in enumerate(mutations):
            with self.subTest(tool=tool, field=field):
                self.target_dir = self.external_root / f"cargo-target-{mutation_index}"
                projection = json.loads(
                    json.dumps(self.authenticated_source_seal_projection_value)
                )
                projection["outer_policy"]["toolchain"][tool][field] = value
                if field == "binary_sha256":
                    projection["outer_policy"]["cargo"]["unit_graph"][
                        "capture_receipt"
                    ][f"{tool}_binary_sha256"] = value
                _, projection_sha256 = self.write_projection(projection)
                command_runner = mock.Mock()

                with self.assertRaisesRegex(
                    builder.CandidateBuildError,
                    rf"{diagnostic} executable differs from the authenticated execution policy",
                ):
                    self.build_candidate_bundle(
                        self.root,
                        str(self.cargo),
                        authenticated_source_seal_projection_sha256=(
                            projection_sha256
                        ),
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

        def configured_result(command: list[str]) -> subprocess.CompletedProcess[bytes]:
            command_target = Path(command[command.index("--target-dir") + 1])
            executable = (
                command_target
                / "configured-triple"
                / "release"
                / builder.BINARY_NAME
            )
            return self.cargo_result(command, executable=executable)

        report = self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=lambda command, **_kwargs: configured_result(command),
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
            'require_actual_tool_identity(\n        "CARGO",\n        &reviewed_cargo_binary_sha256',
            build_rs,
        )
        self.assertIn(
            'require_actual_tool_identity(\n        "RUSTC",\n        &reviewed_rustc_binary_sha256',
            build_rs,
        )
        self.assertIn("reviewed_cargo_binary_size", build_rs)
        self.assertIn("KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SHA256", build_rs)
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
