"""Regression tests for Taira empty-reset host storage admission."""

from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest import mock

from scripts import prepare_taira_empty_reset_bundle as reset_bundle


class TairaResetFreeSpaceTests(unittest.TestCase):
    """Exercise the fail-closed free-space guard before reset materialization."""

    def test_accepts_filesystem_at_or_above_required_free_space(self) -> None:
        with mock.patch.object(
            reset_bundle.shutil,
            "disk_usage",
            return_value=SimpleNamespace(free=16_384),
        ) as disk_usage:
            self.assertEqual(
                reset_bundle.require_minimum_free_space(Path("/sealed"), 16_384),
                16_384,
            )
        disk_usage.assert_called_once_with(Path("/sealed"))

    def test_rejects_filesystem_below_required_free_space(self) -> None:
        with mock.patch.object(
            reset_bundle.shutil,
            "disk_usage",
            return_value=SimpleNamespace(free=16_383),
        ):
            with self.assertRaisesRegex(
                RuntimeError,
                "16383 bytes available, 16384 required",
            ):
                reset_bundle.require_minimum_free_space(
                    Path("/sealed"), 16_384
                )

    def test_rejects_negative_required_free_space(self) -> None:
        with self.assertRaisesRegex(
            RuntimeError, "minimum free bytes must be non-negative"
        ):
            reset_bundle.require_minimum_free_space(Path("/sealed"), -1)


class TairaResetIdentityTests(unittest.TestCase):
    """Exercise self-contained config retargeting and artifact identity checks."""

    def test_retargets_every_source_bundle_path(self) -> None:
        source = Path("/private/reset-v19")
        output = Path("/private/reset-v20")
        encoded = (
            b'file = "/private/reset-v19/genesis.signed.nrt"\n'
            b'private_key_file = "/private/reset-v19/runtime/key"\n'
        )
        retargeted = reset_bundle.retarget_bundle_paths(
            encoded, source, output
        )
        self.assertNotIn(str(source).encode(), retargeted)
        self.assertEqual(retargeted.count(str(output).encode()), 2)

    def test_rejects_config_without_source_bundle_path(self) -> None:
        with self.assertRaisesRegex(
            RuntimeError,
            "does not reference its source bundle",
        ):
            reset_bundle.retarget_bundle_paths(
                b'file = "/foreign/genesis.signed.nrt"\n',
                Path("/private/reset-v19"),
                Path("/private/reset-v20"),
            )

    def test_accepts_only_lowercase_sha256(self) -> None:
        digest = "ab" * 32
        self.assertEqual(
            reset_bundle.require_sha256(digest, "artifact"),
            digest,
        )
        with self.assertRaisesRegex(
            RuntimeError, "must be a lowercase SHA-256 digest"
        ):
            reset_bundle.require_sha256(digest.upper(), "artifact")

    def test_accepts_only_nonzero_lowercase_source_commit(self) -> None:
        commit = "ab" * 20
        self.assertEqual(reset_bundle.require_source_commit(commit), commit)
        for rejected in (commit.upper(), "0" * 40, commit[:-1], f"{commit}0"):
            with self.subTest(rejected=rejected):
                with self.assertRaisesRegex(
                    RuntimeError,
                    "source commit must be a nonzero lowercase Git object id",
                ):
                    reset_bundle.require_source_commit(rejected)


if __name__ == "__main__":
    unittest.main()
