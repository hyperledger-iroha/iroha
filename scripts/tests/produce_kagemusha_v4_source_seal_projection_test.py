"""Tests for the authenticated Kagemusha source-projection producer."""

from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

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
        self.closure = self.write("reviewed-source-closure.json", canonical(self.closure_value))
        self.execution_policy = self.write(
            "execution-policy.json",
            canonical(
                {
                    "profile": "release",
                    "schema": "iroha.kagemusha.fixture_execution_policy.v1",
                    "target": builder.SOURCE_SEAL_TARGET,
                }
            ),
        )
        self.graph_value = self.unit_graph()
        self.graph = self.write("unit-graph.json", canonical(self.graph_value))
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
            f"projection-controller@example.test {key_fields[0]} {key_fields[1]}\n".encode(),
        )
        self.revocation = self.write("controller-revocation", b"")
        derived_graph = producer._validate_normalized_unit_graph(self.graph.read_bytes())
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
            "codegen_units": None,
            "debug_assertions": False,
            "debuginfo": 0,
            "incremental": False,
            "lto": "false",
            "name": "release",
            "opt_level": "3",
            "overflow_checks": True,
            "panic": "unwind",
            "rpath": False,
        }

    @staticmethod
    def target(kind: str, name: str, source: str) -> dict[str, object]:
        """Return the exact normalized target shape."""

        return {
            "crate_types": ["bin" if kind != "lib" else "lib"],
            "doctest": False,
            "edition": "2024",
            "kind": [kind],
            "name": name,
            "src_path": source,
            "test": False,
        }

    def unit_graph(self) -> dict[str, object]:
        """Return a small genuine-shape normalized unit graph."""

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
                    "target": self.target("lib", "dependency", "src/lib.rs"),
                },
                {
                    "dependencies": [],
                    "features": [],
                    "mode": "run-custom-build",
                    "pkg_id": "dependency 1.0.0 (registry+https://example.test/index)",
                    "platform": None,
                    "profile": self.profile(),
                    "target": self.target(
                        "custom-build", "build-script-build", "build.rs"
                    ),
                },
                {
                    "dependencies": [
                        {"extern_crate_name": "dependency", "index": 0},
                        {"extern_crate_name": "dependency_build_script", "index": 1},
                    ],
                    "features": list(builder.SOURCE_SEAL_RESOLVED_FEATURES),
                    "mode": "build",
                    "pkg_id": "iroha_core 3.0.0 (path+file://<PACKAGE_ROOT>)",
                    "platform": builder.SOURCE_SEAL_TARGET,
                    "profile": self.profile(),
                    "target": self.target(
                        "bin", builder.BINARY_NAME, "src/bin/kagemusha_recursive_spend_v4_bundle.rs"
                    ),
                },
            ],
            "version": 1,
        }

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

        def identity_reader(root: Path, closure: str, closure_sha256: str) -> source_seal.SourceIdentity:
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
        output = self.root / "projection.json"
        producer._write_new_private_file(output, production.projection_bytes)
        self.assertEqual(output.read_bytes(), production.projection_bytes)
        self.assertEqual(output.stat().st_mode & 0o777, 0o600)

    def test_request_reconstructs_exact_signed_authorization(self) -> None:
        request, request_bytes = producer.construct_authorization_request(
            self.root,
            reviewed_source_closure=self.closure,
            reviewed_source_closure_sha256=digest(self.closure),
            execution_policy_path=self.execution_policy,
            execution_policy_sha256=digest(self.execution_policy),
            unit_graph_path=self.graph,
            unit_graph_sha256=digest(self.graph),
            identity_reader=lambda _root, _closure, _digest: self.identity,
        )

        self.assertEqual(request, self.authorization_value)
        self.assertEqual(request_bytes, self.authorization.read_bytes())
        self.assertEqual(request["unit_graph"]["iroha_core_units"], 1)

    def test_modified_graph_is_rejected_even_with_a_fresh_file_pin(self) -> None:
        graph = copy.deepcopy(self.graph_value)
        graph["units"][0]["pkg_id"] = "substituted 1.0.0 (registry+https://example.test/index)"
        self.graph.write_bytes(canonical(graph))

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "controller-authorized Cargo unit graph differs",
        ):
            self.construct()

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
        self.execution_policy.write_bytes(
            canonical(
                {
                    "profile": "substituted",
                    "schema": "iroha.kagemusha.fixture_execution_policy.v1",
                    "target": builder.SOURCE_SEAL_TARGET,
                }
            )
        )

        with self.assertRaisesRegex(
            producer.ProjectionProductionError,
            "controller authorization differs from the verified source and policy inputs",
        ):
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

    def test_projection_byte_bound_is_exact(self) -> None:
        maximum = builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES

        producer._require_projection_byte_bound(b"x" * maximum)
        with self.assertRaisesRegex(
            producer.ProjectionProductionError, "exceeds its byte bound"
        ):
            producer._require_projection_byte_bound(b"x" * (maximum + 1))

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
