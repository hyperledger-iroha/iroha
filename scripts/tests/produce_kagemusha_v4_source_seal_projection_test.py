"""Tests for the authenticated Kagemusha source-projection producer."""

from __future__ import annotations

import copy
import hashlib
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from scripts import build_kagemusha_v4_candidate_bundle as builder
from scripts import kagemusha_source_tree_seal as source_seal
from scripts import produce_kagemusha_v4_source_seal_projection as producer


def canonical(value: object) -> bytes:
    """Encode one canonical protocol JSON line."""

    return builder._canonical_json_line(value)


def digest(path: Path) -> str:
    """Return the SHA-256 of one fixture file."""

    return hashlib.sha256(path.read_bytes()).hexdigest()


class AuthenticatedProjectionProducerTests(unittest.TestCase):
    """Exercise authenticated production, reconstruction, and substitution failure."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve(strict=True)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir(mode=0o700)
        self.closure_value = {
            "fixture": True,
            "schema": source_seal.REVIEWED_SOURCE_CLOSURE_SCHEMA,
            "source_repo_dirty": False,
        }
        self.closure = self.write(
            "reviewed-source-closure.json", canonical(self.closure_value)
        )
        self.graph_value = self.unit_graph()
        self.graph = self.write("unit-graph.json", canonical(self.graph_value))
        self.raw_graph = self.write(
            "raw-unit-graph.json",
            canonical(self.raw_unit_graph()),
        )
        self.execution_policy_value = self.make_execution_policy()
        self.execution_policy = self.write(
            "execution-policy.json", canonical(self.execution_policy_value)
        )
        self.identity = source_seal.SourceIdentity(
            source_commit="a" * 40,
            source_tree_sha256="b" * 64,
            source_repo_dirty=False,
            reviewed_source_closure=self.closure_value,
            reviewed_source_closure_descriptor_sha256=digest(self.closure),
            source_authority=source_seal.SourceAuthority(
                commit="a" * 40,
                commit_object_sha256="f" * 64,
                commit_object_size=2048,
                committer_epoch=builder.AUTHORIZED_SOURCE_PARENT_EPOCH + 1,
                git_tree="c" * 40,
                ordered_parents=(builder.AUTHORIZED_SOURCE_PARENT_COMMIT,),
                ordered_parent_trees=(builder.AUTHORIZED_SOURCE_PARENT_TREE,),
                signature=source_seal.VerifiedSshSignature(
                    principal="release-reviewer@example.test",
                    public_key_sha256="2" * 64,
                    allowed_signers_sha256="1" * 64,
                    revocation_sha256="3" * 64,
                ),
            ),
        )
        self.private_key = self.inputs / "controller-key"
        subprocess.run(
            [
                str(source_seal.SSH_KEYGEN),
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-f",
                str(self.private_key),
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        key_fields = self.private_key.with_suffix(".pub").read_text(
            encoding="utf-8"
        ).split()
        self.allowed_signers = self.write(
            "controller-allowed-signers",
            (
                f"projection-controller@example.test {key_fields[0]} "
                f"{key_fields[1]}\n"
            ).encode(),
        )
        self.revocation = self.write("controller-revocation", b"")
        derived_graph = producer._validate_normalized_unit_graph(
            self.graph.read_bytes()
        )
        self.authorization_value = {
            "execution_policy_sha256": digest(self.execution_policy),
            "projection_schema": builder.AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA,
            "reviewed_source_closure_sha256": hashlib.sha256(
                canonical(self.closure_value)
            ).hexdigest(),
            "schema": producer.AUTHORIZATION_SCHEMA,
            "source_commit": self.identity.source_commit,
            "source_tree_sha256": self.identity.source_tree_sha256,
            "unit_graph": derived_graph,
        }
        self.authorization = self.inputs / "controller-authorization.json"
        self.signature = self.inputs / "controller-authorization.json.sig"
        self.resign()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_isolated_snapshot_launcher_imports_only_explicit_package_root(self) -> None:
        """`python -I` can execute the privately snapshotted package closure."""

        repository = Path(__file__).parents[2]
        package_root = self.root / "authenticated-package"
        closure = (
            "scripts/run_kagemusha_source_projection_snapshot.py",
            "scripts/produce_kagemusha_v4_source_seal_projection.py",
            "scripts/build_kagemusha_v4_candidate_bundle.py",
            "scripts/profile_cargo_build.py",
            "scripts/kagemusha_source_tree_seal.py",
            "scripts/formal/run_sumeragi_v2_tlapm_guard.py",
        )
        for relative in closure:
            target = package_root / relative
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            target.write_bytes((repository / relative).read_bytes())
            target.chmod(0o400)
        launcher = package_root / closure[0]
        hostile_site = self.root / "hostile-site"
        hostile_site.mkdir()
        hostile_marker = self.root / "hostile-site-imported"
        (hostile_site / "sitecustomize.py").write_text(
            f"from pathlib import Path\nPath({str(hostile_marker)!r}).touch()\n",
            encoding="utf-8",
        )
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                str(launcher),
                str(package_root),
                "--help",
            ],
            cwd=Path("/"),
            env={
                "HOME": "/var/empty",
                "PATH": "/usr/bin:/bin",
                "PYTHONPATH": str(hostile_site),
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            close_fds=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertIn(b"authenticated Kagemusha V4 source projection", completed.stdout)
        self.assertFalse(hostile_marker.exists())

    def write(self, name: str, payload: bytes) -> Path:
        """Write one owner-controlled fixture input."""

        path = self.inputs / name
        path.write_bytes(payload)
        path.chmod(0o600)
        return path

    @staticmethod
    def profile() -> dict[str, object]:
        """Return the exact normalized release profile shape."""

        return {
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
        }

    @staticmethod
    def target(kind: str, name: str, source: str) -> dict[str, object]:
        """Return the exact normalized target shape."""

        return {
            "crate_types": ["bin" if kind != "lib" else "lib"],
            "doc": True,
            "doctest": False,
            "edition": "2024",
            "kind": [kind],
            "name": name,
            "src_path": source,
            "test": True,
        }

    def make_execution_policy(self) -> dict[str, object]:
        """Return the exact V1 controller policy for the synthetic fixtures."""

        tree_identity = {
            "bytes": 4096,
            "files": 4,
            "records": 6,
            "sha256": "7" * 64,
        }
        build_inputs = {
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
        return {
            "build_inputs": build_inputs,
            "cargo": {
                "binary_sha256": "4" * 64,
                "binary_size_bytes": 12_345_678,
                "capabilities": ["cargo-nightly-unit-graph-v1"],
                "version_argv": ["<DIRECT_CARGO>", "-Vv"],
                "version_stdout_lines": [
                    producer.PINNED_CARGO_VERSION_LINE,
                    "release: 1.93.0-nightly",
                    "host: aarch64-apple-darwin",
                ],
            },
            "rustc": {
                "binary_sha256": "5" * 64,
                "binary_size_bytes": 23_456_789,
                "capabilities": [],
                "version_argv": ["<DIRECT_RUSTC>", "-Vv"],
                "version_stdout_lines": [
                    producer.PINNED_RUSTC_VERSION_LINE,
                    "release: 1.93.1",
                    "host: aarch64-apple-darwin",
                ],
            },
            "schema": producer.EXECUTION_POLICY_SCHEMA,
            "unit_graph": {
                "capture_argv": list(producer.UNIT_GRAPH_CAPTURE_ARGV),
                "capture_environment": dict(
                    producer.UNIT_GRAPH_CAPTURE_ENVIRONMENT
                ),
                "capture_receipt": {
                    "build_inputs_sha256": hashlib.sha256(
                        canonical(build_inputs)
                    ).hexdigest(),
                    "cargo_binary_sha256": "4" * 64,
                    "exit_status": 0,
                    "raw_stdout_sha256": digest(self.raw_graph),
                    "raw_stdout_size_bytes": self.raw_graph.stat().st_size,
                    "rustc_binary_sha256": "5" * 64,
                    "schema": producer.CAPTURE_RECEIPT_SCHEMA,
                    "source_commit": "a" * 40,
                    "source_tree_sha256": "b" * 64,
                    "stderr_sha256": hashlib.sha256(b"").hexdigest(),
                    "stderr_size_bytes": 0,
                },
                "normalization": builder.SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION,
                "normalized_sha256": digest(self.graph),
                "normalized_size_bytes": self.graph.stat().st_size,
                "raw_sha256": digest(self.raw_graph),
                "raw_size_bytes": self.raw_graph.stat().st_size,
            },
        }

    def unit_graph(self) -> dict[str, object]:
        """Return a small synthetic graph with the Cargo 1.93 protocol shape.

        This is deliberately a protocol fixture, not Cargo evidence. It keeps
        only three structurally representative units for adversarial tests.
        """

        return {
            "roots": [2],
            "units": [
                {
                    "dependencies": [],
                    "features": [],
                    "mode": "build",
                    "pkg_id": "dependency 1.0.0 (registry+https://example.test/index)",
                    "platform": builder.SOURCE_SEAL_TARGET,
                    "profile": self.profile(),
                    "target": self.target(
                        "lib",
                        "dependency",
                        "<SOURCE_CACHE>/registry/src/example.test/dependency-1.0.0/src/lib.rs",
                    ),
                },
                {
                    "dependencies": [],
                    "features": [],
                    "mode": "run-custom-build",
                    "pkg_id": "dependency 1.0.0 (registry+https://example.test/index)",
                    "platform": None,
                    "profile": self.profile(),
                    "target": self.target(
                        "custom-build",
                        "build-script-build",
                        "<SOURCE_CACHE>/registry/src/example.test/dependency-1.0.0/build.rs",
                    ),
                },
                {
                    "dependencies": [
                        {
                            "extern_crate_name": "dependency",
                            "index": 0,
                            "noprelude": False,
                            "public": False,
                        },
                        {
                            "extern_crate_name": "dependency_build_script",
                            "index": 1,
                            "noprelude": False,
                            "public": False,
                        },
                    ],
                    "features": list(builder.SOURCE_SEAL_RESOLVED_FEATURES),
                    "mode": "build",
                    "pkg_id": (
                        "iroha_core 3.0.0 "
                        "(path+file://<SOURCE_ROOT>/crates/iroha_core)"
                    ),
                    "platform": builder.SOURCE_SEAL_TARGET,
                    "profile": self.profile(),
                    "target": {
                        **self.target(
                            "bin",
                            builder.BINARY_NAME,
                            "src/bin/kagemusha_recursive_spend_v4_bundle.rs",
                        ),
                        "required-features": list(producer.ROOT_REQUIRED_FEATURES),
                    },
                },
            ],
            "version": 1,
        }

    def raw_unit_graph(self) -> dict[str, object]:
        """Expand normalized placeholders to synthetic controller capture paths."""

        raw = copy.deepcopy(self.graph_value)
        dependency_prefix = (
            "/private/controller/cargo-home/registry/src/example.test/"
            "dependency-1.0.0/"
        )
        raw["units"][0]["target"]["src_path"] = f"{dependency_prefix}src/lib.rs"
        raw["units"][1]["target"]["src_path"] = f"{dependency_prefix}build.rs"
        package_root = "/private/controller/source/crates/iroha_core"
        raw["units"][2]["pkg_id"] = (
            f"iroha_core 3.0.0 (path+file://{package_root})"
        )
        raw["units"][2]["target"]["src_path"] = (
            f"{package_root}/src/bin/kagemusha_recursive_spend_v4_bundle.rs"
        )
        return raw

    def resign(self, namespace: str = producer.SIGNATURE_NAMESPACE) -> None:
        """Canonically encode and sign the current controller authorization."""

        self.authorization.write_bytes(canonical(self.authorization_value))
        self.authorization.chmod(0o600)
        self.signature.unlink(missing_ok=True)
        subprocess.run(
            [
                str(source_seal.SSH_KEYGEN),
                "-Y",
                "sign",
                "-f",
                str(self.private_key),
                "-n",
                namespace,
                str(self.authorization),
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.signature.chmod(0o600)

    def construct(self) -> producer.ProjectionProduction:
        """Run production with all input pins and a locally derived identity."""

        def identity_reader(
            root: Path, closure: str, closure_sha256: str
        ) -> source_seal.SourceIdentity:
            self.assertEqual(root, self.root)
            self.assertEqual(Path(closure), self.closure)
            self.assertEqual(closure_sha256, digest(self.closure))
            return self.identity

        return producer.construct_projection(
            self.root,
            reviewed_source_closure=self.closure,
            reviewed_source_closure_sha256=digest(self.closure),
            authorization_path=self.authorization,
            authorization_sha256=digest(self.authorization),
            controller_signature_path=self.signature,
            controller_signature_sha256=digest(self.signature),
            controller_allowed_signers_path=self.allowed_signers,
            controller_allowed_signers_sha256=digest(self.allowed_signers),
            controller_revocation_path=self.revocation,
            controller_revocation_sha256=digest(self.revocation),
            execution_policy_path=self.execution_policy,
            execution_policy_sha256=digest(self.execution_policy),
            raw_unit_graph_path=self.raw_graph,
            raw_unit_graph_sha256=digest(self.raw_graph),
            unit_graph_path=self.graph,
            unit_graph_sha256=digest(self.graph),
            identity_reader=identity_reader,
        )

    def test_signed_inputs_produce_exact_consumer_projection_and_receipt(self) -> None:
        production = self.construct()

        self.assertEqual(
            production.projection_bytes,
            canonical(production.projection),
        )
        self.assertLessEqual(
            len(production.projection_bytes),
            builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES,
        )
        self.assertEqual(
            production.receipt["projection_hex"], production.projection_bytes.hex()
        )
        self.assertEqual(production.receipt["cargo_binary_sha256"], "4" * 64)
        self.assertEqual(production.receipt["rustc_binary_sha256"], "5" * 64)
        self.assertEqual(
            production.receipt["raw_unit_graph_sha256"], digest(self.raw_graph)
        )
        self.assertEqual(
            production.receipt["raw_unit_graph_size_bytes"],
            self.raw_graph.stat().st_size,
        )
        environment = builder._projection_build_environment(
            production.projection,
            production.projection_bytes,
            production.projection_sha256,
            self.identity,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256"],
            digest(self.graph),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_UNIT_GRAPH_RAW_SHA256"],
            digest(self.raw_graph),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256"],
            "4" * 64,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256"],
            "5" * 64,
        )
        self.assertEqual(
            production.projection["outer_policy"]["toolchain"],
            {
                "cargo": {
                    "binary_sha256": "4" * 64,
                    "binary_size_bytes": 12_345_678,
                },
                "rustc": {
                    "binary_sha256": "5" * 64,
                    "binary_size_bytes": 23_456_789,
                },
            },
        )
        output = self.root / "projection.json"
        producer._write_new_private_file(output, production.projection_bytes)
        self.assertEqual(output.read_bytes(), production.projection_bytes)
        self.assertEqual(output.stat().st_mode & 0o777, 0o600)
        self.assertEqual(output.stat().st_nlink, 1)
        self.assertFalse(any(".tmp-" in path.name for path in self.root.iterdir()))

    def test_request_reconstructs_exact_signed_authorization(self) -> None:
        request, request_bytes = producer.construct_authorization_request(
            self.root,
            reviewed_source_closure=self.closure,
            reviewed_source_closure_sha256=digest(self.closure),
            execution_policy_path=self.execution_policy,
            execution_policy_sha256=digest(self.execution_policy),
            raw_unit_graph_path=self.raw_graph,
            raw_unit_graph_sha256=digest(self.raw_graph),
            unit_graph_path=self.graph,
            unit_graph_sha256=digest(self.graph),
            identity_reader=lambda _root, _closure, _digest: self.identity,
        )

        self.assertEqual(request, self.authorization_value)
        self.assertEqual(request_bytes, self.authorization.read_bytes())
        self.assertEqual(request["unit_graph"]["iroha_core_units"], 1)

    def test_modified_graph_is_rejected_even_with_a_fresh_file_pin(self) -> None:
        graph = copy.deepcopy(self.graph_value)
        graph["units"][0]["pkg_id"] = (
            "substituted 1.0.0 (registry+https://example.test/index)"
        )
        self.graph.write_bytes(canonical(graph))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "execution-policy normalized unit graph differs",
        ):
            self.construct()

    def test_modified_raw_graph_is_rejected_even_with_a_fresh_file_pin(self) -> None:
        self.raw_graph.write_bytes(b'{"substituted":"raw graph"}\n')

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "execution-policy raw unit graph differs from the supplied graph",
        ):
            self.construct()

    def test_controller_cannot_sign_a_normalized_graph_not_derived_from_raw(self) -> None:
        """Fresh artifact pins cannot bypass deterministic raw normalization."""

        graph = copy.deepcopy(self.graph_value)
        graph["units"][0]["target"]["src_path"] = (
            "<SOURCE_CACHE>/registry/src/example.test/dependency-1.0.0/src/other.rs"
        )
        self.graph.write_bytes(canonical(graph))
        policy = copy.deepcopy(self.execution_policy_value)
        policy["unit_graph"]["normalized_sha256"] = digest(self.graph)
        policy["unit_graph"]["normalized_size_bytes"] = self.graph.stat().st_size
        self.execution_policy.write_bytes(canonical(policy))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "differs from the deterministic raw capture normalization",
        ):
            self.construct()

    def test_fresh_projection_pin_cannot_authorize_substituted_bytes(self) -> None:
        production = self.construct()
        supplied = self.write("supplied-projection.json", production.projection_bytes)
        producer.verify_reconstructed_projection(
            production, supplied, digest(supplied)
        )
        supplied.write_bytes(production.projection_bytes[:-1] + b" ")

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "differs from deterministic reconstruction",
        ):
            producer.verify_reconstructed_projection(
                production, supplied, digest(supplied)
            )

    def test_stale_projection_cannot_survive_a_new_signed_policy(self) -> None:
        """A once-valid projection is not reusable after controller reauthorization."""

        stale_production = self.construct()
        stale = self.write("stale-projection.json", stale_production.projection_bytes)
        updated_policy = copy.deepcopy(self.execution_policy_value)
        updated_policy["cargo"]["binary_sha256"] = "6" * 64
        updated_policy["unit_graph"]["capture_receipt"][
            "cargo_binary_sha256"
        ] = "6" * 64
        self.execution_policy_value = updated_policy
        self.execution_policy.write_bytes(canonical(updated_policy))
        self.authorization_value["execution_policy_sha256"] = digest(
            self.execution_policy
        )
        self.resign()
        current_production = self.construct()

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "differs from deterministic reconstruction",
        ):
            producer.verify_reconstructed_projection(
                current_production, stale, digest(stale)
            )

    def test_verify_rejects_an_incomplete_controller_input_set(self) -> None:
        """Verification cannot fall back to a claimed projection summary."""

        projection = self.write("projection.json", self.construct().projection_bytes)
        arguments = [
            "verify",
            "--root",
            str(self.root),
            "--reviewed-source-closure",
            str(self.closure),
            "--reviewed-source-closure-sha256",
            digest(self.closure),
            "--authorization",
            str(self.authorization),
            "--authorization-sha256",
            digest(self.authorization),
            "--controller-signature",
            str(self.signature),
            # Deliberately omit --controller-signature-sha256.
            "--controller-allowed-signers",
            str(self.allowed_signers),
            "--controller-allowed-signers-sha256",
            digest(self.allowed_signers),
            "--controller-revocation",
            str(self.revocation),
            "--controller-revocation-sha256",
            digest(self.revocation),
            "--execution-policy",
            str(self.execution_policy),
            "--execution-policy-sha256",
            digest(self.execution_policy),
            "--raw-unit-graph",
            str(self.raw_graph),
            "--raw-unit-graph-sha256",
            digest(self.raw_graph),
            "--unit-graph",
            str(self.graph),
            "--unit-graph-sha256",
            digest(self.graph),
            "--projection",
            str(projection),
            "--projection-sha256",
            digest(projection),
        ]

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "verify require every controller authorization input and pin",
        ):
            producer.main(arguments)

    def test_modified_authorization_is_rejected_without_controller_resign(self) -> None:
        self.authorization_value["source_commit"] = "d" * 40
        self.authorization.write_bytes(canonical(self.authorization_value))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "does not have one trusted SSH signature",
        ):
            self.construct()

    def test_wrong_signature_namespace_is_rejected(self) -> None:
        self.resign("iroha-kagemusha-wrong-namespace")

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "does not have one trusted SSH signature",
        ):
            self.construct()

    def test_modified_execution_policy_is_rejected_with_a_fresh_file_pin(self) -> None:
        policy = copy.deepcopy(self.execution_policy_value)
        policy["cargo"]["binary_sha256"] = "6" * 64
        policy["unit_graph"]["capture_receipt"]["cargo_binary_sha256"] = "6" * 64
        self.execution_policy.write_bytes(canonical(policy))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "controller authorization differs from the verified source and "
            "policy inputs",
        ):
            self.construct()

    def test_empty_execution_policy_is_rejected(self) -> None:
        self.execution_policy.write_bytes(canonical({}))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "projection execution policy JSON members are not exact",
        ):
            self.construct()

    def test_execution_policy_field_and_capture_drift_are_rejected(self) -> None:
        mutations = []
        extra_field = copy.deepcopy(self.execution_policy_value)
        extra_field["ignored"] = True
        mutations.append((extra_field, "JSON members are not exact"))
        capture_argv = copy.deepcopy(self.execution_policy_value)
        capture_argv["unit_graph"]["capture_argv"][1] = "metadata"
        mutations.append((capture_argv, "capture argv is not exact"))
        capture_environment = copy.deepcopy(self.execution_policy_value)
        capture_environment["unit_graph"]["capture_environment"]["HOME"] = (
            "/tmp"
        )
        mutations.append((capture_environment, "capture environment is not exact"))
        normalization = copy.deepcopy(self.execution_policy_value)
        normalization["unit_graph"]["normalization"] = "substituted"
        mutations.append((normalization, "unit-graph normalization differs"))
        cargo_version = copy.deepcopy(self.execution_policy_value)
        cargo_version["cargo"]["version_stdout_lines"][0] = "cargo 1.92.0"
        mutations.append((cargo_version, "version is not the exact pinned build"))
        cargo_patch_version = copy.deepcopy(self.execution_policy_value)
        cargo_patch_version["cargo"]["version_stdout_lines"][0] = "cargo 1.93.2"
        mutations.append(
            (cargo_patch_version, "version is not the exact pinned build")
        )
        rustc_patch_version = copy.deepcopy(self.execution_policy_value)
        rustc_patch_version["rustc"]["version_stdout_lines"][0] = "rustc 1.93.0"
        mutations.append(
            (rustc_patch_version, "version is not the exact pinned build")
        )
        stock_stable_cargo = copy.deepcopy(self.execution_policy_value)
        stock_stable_cargo["cargo"]["version_stdout_lines"] = [
            "cargo 1.93.1 (083ac5135 2025-12-15)",
            "release: 1.93.1",
            "host: aarch64-apple-darwin",
        ]
        mutations.append(
            (stock_stable_cargo, "version is not the exact pinned build")
        )

        for policy, diagnostic in mutations:
            with self.subTest(diagnostic=diagnostic):
                self.execution_policy.write_bytes(canonical(policy))
                with self.assertRaisesRegex(
                    producer.ProjectionProductionError, diagnostic
                ):
                    self.construct()

    def test_execution_policy_graph_digest_and_size_mismatches_are_rejected(
        self,
    ) -> None:
        mutations = []
        normalized_digest = copy.deepcopy(self.execution_policy_value)
        normalized_digest["unit_graph"]["normalized_sha256"] = "7" * 64
        mutations.append(normalized_digest)
        normalized_size = copy.deepcopy(self.execution_policy_value)
        normalized_size["unit_graph"]["normalized_size_bytes"] += 1
        mutations.append(normalized_size)
        invalid_raw_digest = copy.deepcopy(self.execution_policy_value)
        invalid_raw_digest["unit_graph"]["raw_sha256"] = "0" * 64
        mutations.append(invalid_raw_digest)
        raw_digest = copy.deepcopy(self.execution_policy_value)
        raw_digest["unit_graph"]["raw_sha256"] = "7" * 64
        mutations.append(raw_digest)
        raw_size = copy.deepcopy(self.execution_policy_value)
        raw_size["unit_graph"]["raw_size_bytes"] += 1
        mutations.append(raw_size)

        for policy in mutations:
            with self.subTest(policy=policy["unit_graph"]):
                self.execution_policy.write_bytes(canonical(policy))
                with self.assertRaises(producer.ProjectionProductionError):
                    self.construct()

    def test_signed_false_graph_summary_is_rejected(self) -> None:
        self.authorization_value["unit_graph"]["units"] += 1
        self.resign()

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "controller-authorized Cargo unit graph differs",
        ):
            self.construct()

    def test_nonexact_target_and_profile_shapes_are_rejected(self) -> None:
        for field, extra in (
            ("target", {"ignored": True}),
            ("profile", {"ignored": True}),
        ):
            with self.subTest(field=field):
                graph = copy.deepcopy(self.graph_value)
                graph["units"][0][field].update(extra)
                with self.assertRaisesRegex(
                    producer.ProjectionProductionError,
                    rf"{field} fields are not exact",
                ):
                    producer._validate_normalized_unit_graph(canonical(graph))

    def test_boolean_unit_graph_version_is_rejected(self) -> None:
        graph = copy.deepcopy(self.graph_value)
        graph["version"] = True

        with self.assertRaisesRegex(
            producer.ProjectionProductionError, "version is not 1"
        ):
            producer._validate_normalized_unit_graph(canonical(graph))

    def test_absolute_package_and_source_paths_are_rejected(self) -> None:
        mutations = (
            ("pkg_id", "iroha_core 3.0.0 (path+file:///tmp/iroha)"),
            ("src_path", "/tmp/iroha/src/lib.rs"),
        )
        for field, value in mutations:
            with self.subTest(field=field):
                graph = copy.deepcopy(self.graph_value)
                if field == "pkg_id":
                    graph["units"][2][field] = value
                else:
                    graph["units"][2]["target"][field] = value
                with self.assertRaises(producer.ProjectionProductionError):
                    producer._validate_normalized_unit_graph(canonical(graph))

    def test_cargo_1_93_protocol_shape_is_fully_bound(self) -> None:
        root = self.graph_value["units"][2]

        self.assertEqual(
            producer.UNIT_GRAPH_CAPTURE_ARGV[:5],
            ("<DIRECT_CARGO>", "-Z", "unstable-options", "build", "--unit-graph"),
        )
        self.assertNotIn("--message-format", producer.UNIT_GRAPH_CAPTURE_ARGV)
        self.assertEqual(set(root), producer.UNIT_KEYS)
        self.assertEqual(
            set(root["target"]),
            producer.TARGET_BASE_KEYS | producer.TARGET_OPTIONAL_KEYS,
        )
        self.assertEqual(set(root["profile"]), producer.PROFILE_KEYS)
        self.assertEqual(
            root["target"]["required-features"],
            [
                "dev-tools",
                "kagemusha-candidate-evidence-lab",
                "zk-halo2-ipa",
            ],
        )
        self.assertIs(root["target"]["doc"], True)
        self.assertIs(root["target"]["test"], True)
        self.assertIs(root["profile"]["overflow_checks"], False)
        self.assertEqual(
            root["profile"]["strip"],
            {"resolved": {"Named": "debuginfo"}},
        )
        producer._validate_normalized_unit_graph(canonical(self.graph_value))

    def test_projection_byte_bound_is_exact(self) -> None:
        maximum = builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES

        producer._require_projection_byte_bound(b"x" * maximum)
        with self.assertRaisesRegex(
            producer.ProjectionProductionError, "exceeds its byte bound"
        ):
            producer._require_projection_byte_bound(b"x" * (maximum + 1))

    def test_publication_failure_cleans_temporary_and_retry_succeeds(self) -> None:
        output = self.root / "retry-projection.json"
        payload = b"complete projection bytes\n"

        with mock.patch.object(
            producer.os, "link", side_effect=OSError("injected link failure")
        ):
            with self.assertRaisesRegex(
                producer.ProjectionProductionError,
                "unavailable or already exists",
            ):
                producer._write_new_private_file(output, payload)
        self.assertFalse(output.exists())
        self.assertFalse(any(".tmp-" in path.name for path in self.root.iterdir()))

        producer._write_new_private_file(output, payload)
        self.assertEqual(output.read_bytes(), payload)

    def test_publication_is_no_replace_and_parent_fsynced(self) -> None:
        output = self.root / "no-replace-projection.json"
        payload = b"first complete projection\n"
        fsync_directory_calls = 0
        fsync_file_calls = 0
        real_fsync = producer.os.fsync

        def recording_fsync(descriptor: int) -> None:
            nonlocal fsync_directory_calls, fsync_file_calls
            if stat.S_ISDIR(os.fstat(descriptor).st_mode):
                fsync_directory_calls += 1
            else:
                fsync_file_calls += 1
            real_fsync(descriptor)

        with mock.patch.object(producer.os, "fsync", side_effect=recording_fsync):
            producer._write_new_private_file(output, payload)
        self.assertGreaterEqual(fsync_file_calls, 1)
        self.assertGreaterEqual(fsync_directory_calls, 2)

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "unavailable or already exists",
        ):
            producer._write_new_private_file(output, b"replacement bytes\n")
        self.assertEqual(output.read_bytes(), payload)
        self.assertFalse(any(".tmp-" in path.name for path in self.root.iterdir()))

    def test_post_link_failure_is_commit_uncertain_with_complete_final_bytes(
        self,
    ) -> None:
        output = self.root / "commit-uncertain-projection.json"
        payload = b"complete before hard-link publication\n"
        real_fsync = producer.os.fsync
        directory_calls = 0

        def fail_first_directory_fsync(descriptor: int) -> None:
            nonlocal directory_calls
            if stat.S_ISDIR(os.fstat(descriptor).st_mode):
                directory_calls += 1
                if directory_calls == 1:
                    raise OSError("injected directory fsync failure")
            real_fsync(descriptor)

        with mock.patch.object(
            producer.os, "fsync", side_effect=fail_first_directory_fsync
        ):
            with self.assertRaisesRegex(
                producer.ProjectionProductionError, "commit-uncertain state"
            ):
                producer._write_new_private_file(output, payload)
        self.assertEqual(output.read_bytes(), payload)
        self.assertFalse(any(".tmp-" in path.name for path in self.root.iterdir()))
        with self.assertRaises(producer.ProjectionProductionError):
            producer._write_new_private_file(output, payload)

    def test_promotion_and_producer_projection_caps_are_aligned(self) -> None:
        gate = (
            Path(__file__).parents[2] / "ci/check_kagemusha_production_readiness.sh"
        ).read_text(encoding="utf-8")

        self.assertIn("MAX_SOURCE_SEAL_PROJECTION_BYTES = 16 * 1024\n", gate)
        self.assertEqual(
            builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES,
            16 * 1024,
        )


if __name__ == "__main__":
    unittest.main()
