"""Regression tests for the retired offline-gated Taira reset entry point."""

from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
import unittest

from scripts import prepare_taira_offline_reset_bundle as retired_reset


class RetiredTairaOfflineResetTests(unittest.TestCase):
    """Keep the obsolete command fail-closed and free of backend gate semantics."""

    def test_refuses_execution_and_points_to_generic_empty_reset(self) -> None:
        stderr = StringIO()
        with redirect_stderr(stderr):
            result = retired_reset.main(["--apply", "ignored"])

        self.assertEqual(result, 2)
        self.assertIn(retired_reset.REPLACEMENT, stderr.getvalue())
        self.assertIn("universally available", stderr.getvalue())

    def test_retired_source_contains_no_backend_offline_gate_fields(self) -> None:
        source = Path(retired_reset.__file__).read_text(encoding="utf-8")
        for retired_field in (
            "escrow" + "_required",
            "escrow" + "_accounts",
            "offline." + "enabled",
        ):
            with self.subTest(retired_field=retired_field):
                self.assertNotIn(retired_field, source)


if __name__ == "__main__":
    unittest.main()
