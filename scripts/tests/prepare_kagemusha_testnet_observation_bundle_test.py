"""Source-only packaging tests; no signed release or monetary proof is fabricated."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SOURCE = Path(__file__).resolve().parents[1] / "prepare_kagemusha_testnet_observation_bundle.py"
SPEC = importlib.util.spec_from_file_location("prepare_kagemusha_testnet_observation_bundle", SOURCE)
assert SPEC is not None and SPEC.loader is not None
BUNDLE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUNDLE
SPEC.loader.exec_module(BUNDLE)


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


class TestnetBundleTests(unittest.TestCase):
    def test_digest_and_immutable_control_file_require_exact_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "policy.norito"
            path.write_bytes(b"operator-controlled bytes")
            expected = digest(path.read_bytes())
            self.assertEqual(BUNDLE.digest_arg(expected, "policy"), expected)
            self.assertEqual(BUNDLE.read_control(path, "policy", expected), path.read_bytes())
            before = path.stat()
            self.assertTrue(BUNDLE.stable_identity(before, path.stat()))
            for bad in (None, "0" * 64, expected.upper(), expected[:-1]):
                with self.assertRaises(BUNDLE.BundleError):
                    BUNDLE.digest_arg(bad, "policy")
            with self.assertRaises(BUNDLE.BundleError):
                BUNDLE.read_control(path, "policy", "1" * 64)
            alias = path.with_name("alias.norito")
            alias.symlink_to(path)
            with self.assertRaises(BUNDLE.BundleError):
                BUNDLE.read_control(alias, "policy")

    def test_report_rejects_wrong_scope_threshold_and_artifact_inventory(self) -> None:
        args = argparse.Namespace(
            release_id="1" * 64,
            attestation_digest="2" * 64,
            authority_review_projection_sha256="3" * 64,
            native_artifact_manifest_sha256="4" * 64,
        )
        report = self.report(args)
        self.assertEqual(len(BUNDLE.verify_report(report, args)), 50)
        bad = dict(report, release_id="3" * 64)
        with self.assertRaises(BUNDLE.BundleError):
            BUNDLE.verify_report(bad, args)
        bad = dict(report, authority_threshold=1)
        with self.assertRaises(BUNDLE.BundleError):
            BUNDLE.verify_report(bad, args)
        bad = dict(report, approved_signers=["one"])
        with self.assertRaises(BUNDLE.BundleError):
            BUNDLE.verify_report(bad, args)
        for field, value in (
            ("native_artifact_manifest_authenticated", False),
            ("native_artifact_hash_verified", False),
            ("authority_review_projection_sha256", "5" * 64),
            ("native_artifact_manifest_sha256", "6" * 64),
        ):
            with self.subTest(field=field), self.assertRaises(BUNDLE.BundleError):
                BUNDLE.verify_report(dict(report, **{field: value}), args)
        bad = dict(report, artifacts=report["artifacts"][:-1])
        with self.assertRaises(BUNDLE.BundleError):
            BUNDLE.verify_report(bad, args)
        rows = list(report["artifacts"])
        rows[1] = dict(rows[1], sha256=rows[0]["sha256"])
        with self.assertRaises(BUNDLE.BundleError):
            BUNDLE.verify_report(dict(report, artifacts=rows), args)

    def test_artifact_copy_hashes_and_exclusive_json_writer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            target = root / "target"
            source.write_bytes(b"real artifact bytes")
            BUNDLE.copy_artifact(source, target, digest(source.read_bytes()), source.stat().st_size)
            self.assertEqual(target.read_bytes(), source.read_bytes())
            with self.assertRaises(FileExistsError):
                BUNDLE.copy_artifact(source, target, digest(source.read_bytes()), source.stat().st_size)
            with self.assertRaises(BUNDLE.BundleError):
                BUNDLE.copy_artifact(source, root / "wrong", "f" * 64, source.stat().st_size)
            document = root / "pins.json"
            BUNDLE.write_json(document, {"z": 2, "a": 1})
            self.assertEqual(document.read_bytes(), b'{"a":1,"z":2}\n')
            with self.assertRaises(FileExistsError):
                BUNDLE.write_json(document, {})

    def test_kagami_invocation_rejects_unpinned_executable_before_running(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary).resolve() / "kagami"
            binary.write_bytes(b"not the pinned binary")
            binary.chmod(0o700)
            args = argparse.Namespace(kagami=binary, kagami_sha256="1" * 64)
            with mock.patch.object(BUNDLE.subprocess, "run") as run:
                with self.assertRaises(BUNDLE.BundleError):
                    BUNDLE.run_kagami(args)
                run.assert_not_called()

    def test_kagami_runs_private_pinned_copy_when_source_path_is_replaced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            artifact_root = root / "source-artifacts"
            artifact_root.mkdir()
            args = self.args(root, artifact_root)
            source = args.kagami
            original = source.read_bytes()
            report = self.report(args)

            def replace_source(argv: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                private = Path(argv[0])
                self.assertNotEqual(private, source)
                self.assertEqual(private.read_bytes(), original)
                self.assertEqual(private.stat().st_mode & 0o777, 0o500)
                source.write_bytes(b"substituted executable")
                source.write_bytes(original)
                return subprocess.CompletedProcess(argv, 0, json.dumps(report), "")

            with mock.patch.object(BUNDLE.subprocess, "run", side_effect=replace_source):
                self.assertEqual(BUNDLE.run_kagami(args), report)

    def test_prepare_copies_exact_authenticated_bundle_and_candidate_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            artifact_root = root / "source-artifacts"
            artifact_root.mkdir()
            args = self.args(root, artifact_root)
            report = self.report(args)
            for index, row in enumerate(report["artifacts"]):
                (artifact_root / row["sha256"]).write_bytes(bytes([index + 1]))
            with mock.patch.object(BUNDLE, "run_kagami", return_value=report) as verifier:
                BUNDLE.prepare(args)
            self.assertEqual(verifier.call_count, 2)
            self.assertEqual(verifier.call_args.args[1], args.output)
            pins = json.loads((args.output / "operator/pins.candidate.json").read_text())
            self.assertEqual(pins["network_id"], args.network_id)
            self.assertEqual(pins["release_id"], args.release_id)
            self.assertEqual(pins["attestation_digest"], args.attestation_digest)
            self.assertEqual(pins["authority_policy_sha256"], args.authority_policy_sha256)
            self.assertEqual(pins["authority_policy_digest"], report["authority_policy_digest"])
            self.assertEqual(pins["artifact_count"], 50)
            self.assertEqual(pins["review_status"], "candidate_only")
            self.assertFalse(pins["monetary_admission"])
            self.assertEqual(len(list((args.output / "proof/artifacts").iterdir())), 50)
            self.assertEqual(
                (args.output / "operator/authority-policy.norito").read_bytes(),
                args.authority_policy.read_bytes(),
            )

    def test_prepare_removes_partial_bundle_on_artifact_substitution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            artifact_root = root / "source-artifacts"
            artifact_root.mkdir()
            args = self.args(root, artifact_root)
            report = self.report(args)
            for index, row in enumerate(report["artifacts"]):
                payload = b"wrong" if index == 10 else bytes([index + 1])
                (artifact_root / row["sha256"]).write_bytes(payload)
            with mock.patch.object(BUNDLE, "run_kagami", return_value=report):
                with self.assertRaises(BUNDLE.BundleError):
                    BUNDLE.prepare(args)
            self.assertFalse(args.output.exists())

    def test_prepare_rejects_a_changed_copied_release_report(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            artifact_root = root / "source-artifacts"
            artifact_root.mkdir()
            args = self.args(root, artifact_root)
            report = self.report(args)
            for index, row in enumerate(report["artifacts"]):
                (artifact_root / row["sha256"]).write_bytes(bytes([index + 1]))
            substituted = dict(report, native_source_commit="different")
            with mock.patch.object(BUNDLE, "run_kagami", side_effect=[report, substituted]):
                with self.assertRaises(BUNDLE.BundleError):
                    BUNDLE.prepare(args)
            self.assertFalse(args.output.exists())

    def test_main_rejects_invalid_pins_before_file_access(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            args = self.args(root, root)
            names = (
                "kagami", "manifest", "receipt", "authority_policy", "attestation",
                "recursive_profile", "artifact_root", "authority_review_projection",
                "native_artifact_manifest", "native_artifact", "output",
                "kagami_sha256", "network_id", "release_id", "attestation_digest",
                "authority_policy_sha256", "authority_review_projection_sha256",
                "native_artifact_manifest_sha256",
            )
            argv = [item for name in names for item in ("--" + name.replace("_", "-"), str(getattr(args, name)))]
            position = argv.index("--network-id") + 1
            argv[position] = "bad"
            self.assertEqual(BUNDLE.main(argv), 1)
            self.assertFalse(args.output.exists())

    @staticmethod
    def report(args: argparse.Namespace) -> dict[str, object]:
        return {
            "schema": BUNDLE.REPORT_SCHEMA,
            "schema_version": 1,
            "status": "authenticated",
            "runtime_loaded": True,
            "native_artifact_manifest_authenticated": True,
            "native_artifact_hash_verified": True,
            "authority_review_projection_sha256": args.authority_review_projection_sha256,
            "native_artifact_manifest_sha256": args.native_artifact_manifest_sha256,
            "release_id": args.release_id,
            "attestation_digest": args.attestation_digest,
            "authority_policy_digest": "d" * 64,
            "authority_threshold": 2,
            "approved_signers": ["authority-a", "authority-b"],
            "artifact_set_digest": "a" * 64,
            "artifacts": [
                {"role": f"role-{index}", "sha256": digest(bytes([index + 1])), "byte_len": 1}
                for index in range(50)
            ],
        }

    @staticmethod
    def args(root: Path, artifact_root: Path) -> argparse.Namespace:
        controls = {}
        for name in (
            "manifest", "receipt", "authority_policy", "attestation",
            "recursive_profile", "authority_review_projection", "native_artifact_manifest",
            "native_artifact", "kagami",
        ):
            path = root / name
            path.write_bytes(name.encode())
            controls[name] = path
        return argparse.Namespace(
            **controls,
            artifact_root=artifact_root,
            output=root / "bundle",
            kagami_sha256=digest(controls["kagami"].read_bytes()),
            network_id="1" * 64,
            release_id="2" * 64,
            attestation_digest="3" * 64,
            authority_policy_sha256=digest(controls["authority_policy"].read_bytes()),
            authority_review_projection_sha256=digest(controls["authority_review_projection"].read_bytes()),
            native_artifact_manifest_sha256=digest(controls["native_artifact_manifest"].read_bytes()),
        )


if __name__ == "__main__":
    unittest.main()
