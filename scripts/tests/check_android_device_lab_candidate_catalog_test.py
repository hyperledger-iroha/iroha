"""Candidate artifact-catalog tests split from the Android device-lab suite."""

from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest
import zipfile


TESTS_DIR = Path(__file__).resolve().parent
if str(TESTS_DIR) not in sys.path:
    sys.path.insert(0, str(TESTS_DIR))

from check_android_device_lab_slot_test import device_lab  # noqa: E402


class AndroidDeviceLabCandidateCatalogTest(unittest.TestCase):
    def test_candidate_lab_apk_forbidden_catalog_is_exactly_the_eight_krv4_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            clean = root / "clean.apk"
            with zipfile.ZipFile(clean, "w") as archive:
                archive.writestr("assets/candidate/candidate-v4.norito", b"candidate")
            self.assertEqual(
                device_lab._candidate_lab_apk_forbidden_krv4_entries(clean),
                [],
            )
            forbidden = root / "forbidden.apk"
            with zipfile.ZipFile(forbidden, "w") as archive:
                for name in device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4:
                    archive.writestr(f"assets/artifacts/{name}", b"must stay external")
            self.assertEqual(
                device_lab._candidate_lab_apk_forbidden_krv4_entries(forbidden),
                sorted(
                    (
                        f"assets/artifacts/{name}"
                        for name in device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
                    ),
                    key=lambda value: value.encode("utf-8"),
                ),
            )

    def test_krv4_artifact_limit_matches_core_and_rejects_next_byte(self) -> None:
        maximum = 5 * 1024 * 1024 * 1024
        self.assertEqual(device_lab.MAX_KAGEMUSHA_KRV4_ARTIFACT_BYTES, maximum)
        self.assertFalse(device_lab._kagemusha_krv4_size_exceeds_bound(maximum))
        self.assertTrue(device_lab._kagemusha_krv4_size_exceeds_bound(maximum + 1))
