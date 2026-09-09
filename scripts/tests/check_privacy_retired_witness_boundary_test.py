#!/usr/bin/env python3
"""Check retired witness absence and reject restored or relocated production producers."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "retired_witness_boundary", ROOT / "scripts/check_privacy_retired_witness_boundary.py"
)
assert SPEC is not None and SPEC.loader is not None
BOUNDARY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BOUNDARY)


class RetiredWitnessBoundaryTests(unittest.TestCase):
    """No production witness archive is restored by retaining negative test fixtures."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for relative in BOUNDARY.PRODUCTION_ROOTS:
            (self.root / relative).mkdir(parents=True, exist_ok=True)

    def test_current_production_has_no_retired_witness_surface(self) -> None:
        self.assertEqual((), BOUNDARY.check(ROOT))

    def test_every_retired_producer_is_rejected_even_if_empty(self) -> None:
        for relative in BOUNDARY.RETIRED_PRODUCERS:
            with self.subTest(relative=relative):
                path = self.root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("")
                self.assertIn(
                    "retired confidential witness producer must remain absent: " + relative,
                    BOUNDARY.check(self.root),
                )
                path.unlink()

    def test_every_retired_marker_is_rejected_in_every_production_owner(self) -> None:
        for relative in BOUNDARY.PRODUCTION_ROOTS:
            for marker in BOUNDARY.RETIRED_MARKERS:
                with self.subTest(relative=relative, marker=marker):
                    path = self.root / relative / "RelocatedFixture.kt"
                    path.write_text(marker)
                    self.assertEqual(
                        (f"retired confidential witness surface {marker}: {path.relative_to(self.root)}",),
                        BOUNDARY.check(self.root),
                    )
                    path.unlink()

    @unittest.skipIf(hasattr(os, "geteuid") and os.geteuid() == 0,
                     "permission-denial control requires an unprivileged user")
    def test_unreadable_directory_cannot_hide_retired_producer(self) -> None:
        hidden = self.root / BOUNDARY.PRODUCTION_ROOTS[0] / "hidden"
        hidden.mkdir()
        (hidden / "Hidden.java").write_text(BOUNDARY.RETIRED_MARKERS[0])
        hidden.chmod(0)
        try:
            self.assertIn("production source traversal failed: " + str(hidden),
                          BOUNDARY.check(self.root))
        finally:
            hidden.chmod(0o700)

    def test_every_shipping_swift_target_rejects_retired_producer(self) -> None:
        for target in ("IrohaSwift", "IrohaSwiftMobileTransports", "IrohaSwiftTransferUI"):
            with self.subTest(target=target):
                path = self.root / "IrohaSwift/Sources" / target / "RelocatedFixture.swift"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(BOUNDARY.RETIRED_MARKERS[0])
                self.assertIn(
                    f"retired confidential witness surface {BOUNDARY.RETIRED_MARKERS[0]}: {path.relative_to(self.root)}",
                    BOUNDARY.check(self.root),
                )
                path.unlink()

    def test_test_only_historical_fixtures_do_not_create_production_owners(self) -> None:
        fixture = self.root / "kotlin/core-jvm/src/test/kotlin/RetiredFixture.kt"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("\n".join(BOUNDARY.RETIRED_MARKERS))
        self.assertEqual((), BOUNDARY.check(self.root))

    def test_missing_roots_fail_closed(self) -> None:
        path = self.root / BOUNDARY.PRODUCTION_ROOTS[0]
        path.rmdir()
        self.assertTrue(BOUNDARY.check(self.root))

    def test_symlinked_roots_directories_and_files_fail_closed(self) -> None:
        root = self.root / BOUNDARY.PRODUCTION_ROOTS[0]
        target = self.root / "fixture"
        target.mkdir()
        for mode in ("root", "directory", "file"):
            with self.subTest(mode=mode):
                path = root if mode == "root" else root / ("owner" if mode == "directory" else "Owner.java")
                if mode == "root":
                    path.rmdir()
                path.symlink_to(target if mode != "file" else target / "absent.java")
                self.assertTrue(BOUNDARY.check(self.root))
                path.unlink()
                if mode == "root":
                    path.mkdir()

    def test_unreadable_source_fails_closed(self) -> None:
        path = self.root / BOUNDARY.PRODUCTION_ROOTS[0] / "Owner.java"
        path.write_bytes(b"\xff")
        self.assertIn("unreadable production source: " + str(path.relative_to(self.root)), BOUNDARY.check(self.root))


if __name__ == "__main__":
    unittest.main()
