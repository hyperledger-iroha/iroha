"""Contract tests for the authenticated C# Linux ARM cross-build handoff."""

from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import csharp_linux_arm_cross_handoff as handoff  # noqa: E402


CONTRACT_ERRORS = (handoff.HandoffError, handoff.native_checker.ArtifactContractError)


def fake_aarch64_cdylib(payload: bytes = b"bridge") -> bytes:
    """Return a minimal ELF header plus deterministic test payload."""

    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"
    header[16:18] = b"\x03\x00"
    header[18:20] = b"\xb7\x00"
    return bytes(header) + payload


class CSharpLinuxArmCrossHandoffTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.artifact = self.root / "build" / handoff.ARTIFACT_NAME
        self.artifact.parent.mkdir()
        self.artifact.write_bytes(fake_aarch64_cdylib())
        self.candidate = self.root / "candidate"
        self.source_identity = ("a" * 40, "b" * 64)
        self.build_environment = {
            "cargo_version": "cargo 1.93.1 (000000000 2026-01-01)",
            "linker_dumpmachine": handoff.LINKER_MACHINE,
            "linker_version": "aarch64-linux-gnu-gcc (Ubuntu 14.2.0) 14.2.0",
            "rustc_commit_hash": "c" * 40,
            "rustc_host": handoff.BUILD_HOST,
            "rustc_release": "1.93.1",
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def authenticate(self, _source: Path) -> tuple[str, str]:
        return self.source_identity

    def seal(self) -> dict[str, object]:
        return handoff.seal_candidate(
            artifact=self.artifact,
            candidate_root=self.candidate,
            source_root=self.source,
            build_environment=self.build_environment,
            source_authenticator=self.authenticate,
        )

    def verify(self) -> dict[str, object]:
        return handoff.verify_candidate(
            candidate_root=self.candidate,
            source_root=self.source,
            source_authenticator=self.authenticate,
        )

    def test_roundtrip_seals_exact_canonical_inventory(self) -> None:
        sealed = self.seal()
        verified = self.verify()

        self.assertEqual(sealed, verified)
        self.assertEqual(
            {entry.name for entry in self.candidate.iterdir()},
            {handoff.ARTIFACT_NAME, handoff.MANIFEST_NAME},
        )
        manifest_path = self.candidate / handoff.MANIFEST_NAME
        self.assertEqual(
            manifest_path.read_bytes(), handoff.canonical_manifest_bytes(verified)
        )
        self.assertEqual(
            (self.candidate / handoff.ARTIFACT_NAME).read_bytes(),
            self.artifact.read_bytes(),
        )

    def test_verify_and_stage_copies_exact_private_candidate(self) -> None:
        sealed = self.seal()
        stage_root = self.root / "stage"
        stage_root.mkdir()
        staged = handoff.verify_and_stage_candidate(
            candidate_root=self.candidate,
            stage_artifact=stage_root / handoff.ARTIFACT_NAME,
            source_root=self.source,
            source_authenticator=self.authenticate,
        )

        self.assertEqual(staged.read_bytes(), self.artifact.read_bytes())
        self.assertEqual(staged.stat().st_mode & 0o077, 0)
        self.assertEqual(
            handoff.native_checker.stable_artifact_identity(staged),
            (sealed["artifact_sha256"], sealed["artifact_size"]),
        )

    def test_verify_and_stage_rejects_post_verification_replacement(self) -> None:
        self.seal()
        stage_root = self.root / "stage"
        stage_root.mkdir()
        original_verify = handoff.verify_candidate

        def verify_then_replace(**arguments):
            manifest = original_verify(**arguments)
            (self.candidate / handoff.ARTIFACT_NAME).write_bytes(
                fake_aarch64_cdylib(b"replacement")
            )
            return manifest

        with (
            mock.patch.object(handoff, "verify_candidate", verify_then_replace),
            self.assertRaisesRegex(CONTRACT_ERRORS, "does not match"),
        ):
            handoff.verify_and_stage_candidate(
                candidate_root=self.candidate,
                stage_artifact=stage_root / handoff.ARTIFACT_NAME,
                source_root=self.source,
                source_authenticator=self.authenticate,
            )

    def test_verify_rejects_artifact_tampering(self) -> None:
        self.seal()
        with (self.candidate / handoff.ARTIFACT_NAME).open("ab") as output:
            output.write(b"tampered")

        with self.assertRaisesRegex(CONTRACT_ERRORS, "digest or size"):
            self.verify()

    def test_verify_rejects_noncanonical_manifest_bytes(self) -> None:
        self.seal()
        manifest_path = self.candidate / handoff.MANIFEST_NAME
        manifest = json.loads(manifest_path.read_text(encoding="ascii"))
        manifest_path.write_text(json.dumps(manifest, indent=2), encoding="ascii")

        with self.assertRaisesRegex(CONTRACT_ERRORS, "bytes are not canonical"):
            self.verify()

    def test_verify_rejects_duplicate_manifest_key(self) -> None:
        self.seal()
        manifest_path = self.candidate / handoff.MANIFEST_NAME
        payload = manifest_path.read_bytes()
        manifest_path.write_bytes(payload.replace(b"{", b'{"schema":"duplicate",', 1))

        with self.assertRaisesRegex(CONTRACT_ERRORS, "duplicate key"):
            self.verify()

    def test_verify_rejects_wrong_target(self) -> None:
        self.seal()
        manifest_path = self.candidate / handoff.MANIFEST_NAME
        manifest = json.loads(manifest_path.read_text(encoding="ascii"))
        manifest["target"] = "x86_64-unknown-linux-gnu"
        manifest_path.write_text(
            json.dumps(
                manifest,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n",
            encoding="ascii",
        )

        with self.assertRaisesRegex(CONTRACT_ERRORS, "wrong Rust target"):
            self.verify()

    def test_verify_rejects_stale_checked_out_source(self) -> None:
        self.seal()
        self.source_identity = ("d" * 40, "e" * 64)

        with self.assertRaisesRegex(CONTRACT_ERRORS, "checked-out source"):
            self.verify()

    def test_verify_rejects_source_change_during_authentication(self) -> None:
        self.seal()
        identities = iter((self.source_identity, ("d" * 40, "e" * 64)))

        with self.assertRaisesRegex(CONTRACT_ERRORS, "source checkout changed"):
            handoff.verify_candidate(
                candidate_root=self.candidate,
                source_root=self.source,
                source_authenticator=lambda _source: next(identities),
            )

    def test_verify_rejects_extra_candidate_file(self) -> None:
        self.seal()
        (self.candidate / "unexpected").write_bytes(b"unexpected")

        with self.assertRaisesRegex(CONTRACT_ERRORS, "file inventory"):
            self.verify()

    def test_seal_rejects_non_aarch64_shared_object(self) -> None:
        artifact = bytearray(fake_aarch64_cdylib())
        artifact[18:20] = b"\x3e\x00"
        self.artifact.write_bytes(artifact)

        with self.assertRaisesRegex(CONTRACT_ERRORS, "AArch64 ELF shared object"):
            self.seal()

    @unittest.skipUnless(hasattr(os, "symlink"), "symbolic links unavailable")
    def test_seal_rejects_symbolic_candidate_parent(self) -> None:
        real_parent = self.root / "real-parent"
        real_parent.mkdir()
        symbolic_parent = self.root / "symbolic-parent"
        symbolic_parent.symlink_to(real_parent, target_is_directory=True)

        with self.assertRaisesRegex(CONTRACT_ERRORS, "parent must not contain symlinks"):
            handoff.seal_candidate(
                artifact=self.artifact,
                candidate_root=symbolic_parent / "candidate",
                source_root=self.source,
                build_environment=self.build_environment,
                source_authenticator=self.authenticate,
            )

    @unittest.skipUnless(hasattr(os, "symlink"), "symbolic links unavailable")
    def test_verify_rejects_symbolic_artifact_replacement(self) -> None:
        self.seal()
        candidate_artifact = self.candidate / handoff.ARTIFACT_NAME
        candidate_artifact.unlink()
        candidate_artifact.symlink_to(self.artifact)

        with self.assertRaisesRegex(CONTRACT_ERRORS, "owner-controlled regular file"):
            self.verify()

    @unittest.skipUnless(hasattr(os, "link"), "hard links unavailable")
    def test_verify_rejects_hard_linked_artifact_replacement(self) -> None:
        self.seal()
        candidate_artifact = self.candidate / handoff.ARTIFACT_NAME
        candidate_artifact.unlink()
        os.link(self.artifact, candidate_artifact)

        with self.assertRaisesRegex(CONTRACT_ERRORS, "one link"):
            self.verify()

    @unittest.skipUnless(hasattr(os, "link"), "hard links unavailable")
    def test_verify_rejects_hard_linked_manifest(self) -> None:
        self.seal()
        os.link(self.candidate / handoff.MANIFEST_NAME, self.root / "manifest-link")

        with self.assertRaisesRegex(CONTRACT_ERRORS, "one link"):
            self.verify()


if __name__ == "__main__":
    unittest.main()
