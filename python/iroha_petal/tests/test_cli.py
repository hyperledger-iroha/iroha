# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""``python3 -m iroha_petal`` encode/decode/inspect."""

from __future__ import annotations

import contextlib
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from petal_test_support import SRC_DIR, captures_fixture, luma_of, payload

from iroha_petal.cli import main
from iroha_petal.pngio import write_image


def run(*argv: str):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = main(list(argv))
    return status, out.getvalue(), err.getvalue()


class CliTest(unittest.TestCase):
    def test_encode_then_decode_roundtrips_a_payload(self) -> None:
        data = payload(100, 77)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "payload.bin").write_bytes(data)
            status, out, _ = run(
                "encode",
                "--input", str(root / "payload.bin"),
                "--output", str(root / "frames"),
                "--kind", "5",
                "--size", "512",
                "--supersample", "1",
            )  # fmt: skip
            self.assertEqual(status, 0, out)
            frames = sorted((root / "frames").iterdir())
            # 7 source atoms fit in 2 systematic frames; the default doubles that
            self.assertEqual([f.name for f in frames], [f"frame_{i:04d}.png" for i in range(4)])
            # frames may arrive in any order
            status, out, err = run(
                "decode",
                "--input", str(frames[3]),
                "--input", str(frames[1]),
                "--input", str(frames[0]),
                "--output", str(root / "out.bin"),
            )  # fmt: skip
            self.assertEqual(status, 0, err)
            self.assertEqual((root / "out.bin").read_bytes(), data)
            self.assertIn("kind 5", out)
            # the frames share one pose, so every frame after the first follows it
            self.assertIn(f"{frames[1].name}: PKD (tracked)", err)
            self.assertIn("frames: 3 read, 3 located, 3 readable, 2 tracked", err)

    def test_decode_reports_incomplete_streams(self) -> None:
        data = payload(2_000, 78)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "payload.bin").write_bytes(data)
            status, _, _ = run(
                "encode",
                "--input", str(root / "payload.bin"),
                "--output", str(root / "frames"),
                "--frames", "1",
                "--size", "512",
                "--supersample", "1",
                "--format", "pgm",
            )  # fmt: skip
            self.assertEqual(status, 0)
            status, _, err = run(
                "decode", "--input-dir", str(root / "frames"), "--output", str(root / "out.bin")
            )
            self.assertEqual(status, 1)
            self.assertIn("incomplete", err)
            self.assertFalse((root / "out.bin").exists())

    def test_svg_frames_and_argument_errors(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "payload.bin").write_bytes(b"petal")
            status, _, _ = run(
                "encode",
                "--input", str(root / "payload.bin"),
                "--output", str(root / "svg"),
                "--frames", "2",
                "--format", "svg",
            )  # fmt: skip
            self.assertEqual(status, 0)
            text = (root / "svg" / "frame_0000.svg").read_text(encoding="utf-8")
            self.assertTrue(text.startswith("<svg"))
            (root / "empty.bin").write_bytes(b"")
            status, _, err = run(
                "encode", "--input", str(root / "empty.bin"), "--output", str(root / "x")
            )
            self.assertEqual(status, 2)
            self.assertIn("empty", err)
            status, _, err = run("decode", "--output", str(root / "out.bin"))
            self.assertEqual(status, 2)

    def test_inspect_reports_lanes_and_pose(self) -> None:
        capture = next(c for c in captures_fixture()["captures"] if c["name"] == "small-480p")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture.png"
            write_image(path, luma_of(capture))
            status, out, _ = run("inspect", str(path))
            self.assertEqual(status, 0)
            self.assertIn("lane P: ok", out)
            self.assertIn("lane D: ok", out)
            self.assertIn("rotation 3", out)
            self.assertIn(capture["p_data"], out)
            self.assertNotIn("inferred corner", out)
            hidden = next(
                c for c in captures_fixture()["captures"] if c["name"] == "hidden-corner-540p"
            )
            covered = Path(directory) / "covered.png"
            write_image(covered, luma_of(hidden))
            status, out, _ = run("inspect", str(covered))
            self.assertEqual(status, 0)
            self.assertIn("inferred corner: 3 (bottom-left blossom hidden)", out)
            blank = Path(directory) / "blank.pgm"
            write_image(blank, luma_of(captures_fixture()["negatives"][0]))
            status, out, _ = run("inspect", str(blank))
            self.assertEqual(status, 1)
            self.assertIn("no_finders", out)

    def test_module_entry_point_runs(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "iroha_petal", "--help"],
            capture_output=True,
            text=True,
            env={"PYTHONPATH": str(SRC_DIR)},
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("encode", result.stdout)


if __name__ == "__main__":
    unittest.main()
