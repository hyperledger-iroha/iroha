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

from iroha_petal.cli import _parser, main
from iroha_petal.pngio import read_luma, write_image


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
            # 7 source atoms fit in one systematic frame; the default doubles that, at least 4
            self.assertEqual([f.name for f in frames], [f"frame_{i:04d}.png" for i in range(4)])
            self.assertIn("1 systematic frames", out)
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
            # the frames are cards (512 x 834) sharing one pose, so every frame after the first
            # follows it; frames read from files keep no camera waiting, so each one's bands and
            # turn lanes are both read
            self.assertEqual(read_luma(frames[0]).height, 834)
            self.assertIn(
                f"{frames[3].name}: D0123 (turn lanes 0123) (kana lanes P0K0P1K1); rank 0/0", err
            )
            self.assertIn(
                f"{frames[1].name}: D0123 (tracked) (turn lanes 0123) (kana lanes P0K0P1K1)", err
            )
            self.assertIn("frames: 3 read, 3 located, 3 readable, 2 tracked", err)
            self.assertIn(
                "lane D read in 3 frames, 12 field lanes, 12 turn lanes, 12 kana lanes; 0 merges",
                err,
            )

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
            # the card at the default width: 768 x 1251, square coordinates
            self.assertIn('width="768" height="1251" viewBox="0 -322 1024 1668"', text)
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
            # a 480p camera too soft for the turns: the turn reader fits a blur of 3.2 and stops;
            # the polarity lanes of both bands are read under the square's frame
            self.assertIn("lanes decoded: D0123; no turn lane; kana lanes: P0P1", out)
            self.assertIn(
                f"kana lane P0: ok (2 corrected, 0 erased, frame 12) {capture['kana_data'][0]}",
                out,
            )
            self.assertNotIn("kana lane K0", out)
            self.assertIn("turn blur: 3.2 canvas units", out)
            self.assertIn("lane D: ok", out)
            self.assertIn("lane 0: ok", out)
            self.assertIn("lane 3: ok", out)
            self.assertNotIn("turn lane 0", out)
            self.assertIn("rotation 3", out)
            self.assertIn("lens distortion: +0.000", out)
            self.assertIn(capture["d_data"], out)
            self.assertIn(capture["field_data"][2], out)
            self.assertNotIn("inferred corner", out)
            sharp = next(c for c in captures_fixture()["captures"] if c["name"] == "clean-512")
            clean = Path(directory) / "clean.png"
            write_image(clean, luma_of(sharp))
            status, out, _ = run("inspect", str(clean))
            self.assertEqual(status, 0)
            self.assertIn("lanes decoded: D0123; turn lanes: 0123; kana lanes: P0K0P1K1", out)
            self.assertIn("turn blur: 1 canvas units", out)
            for k, lane in enumerate(("P0", "K0", "P1", "K1")):
                self.assertIn(
                    f"kana lane {lane}: ok (0 corrected, 0 erased, frame 5) {sharp['kana_data'][k]}",
                    out,
                )
            for sector in range(4):
                self.assertIn(
                    f"turn lane {sector}: ok (0 corrected, 0 erased) {sharp['turn_data'][sector]}",
                    out,
                )
            # the turn lanes' repair atoms: 2^31 + 16 frame + 4 sector; the kana lanes': 2^31 +
            # 2^20 + 16 frame + 8 band (the glyph lane's seven, then the polarity lane's one)
            first = (1 << 31) + 16 * sharp["frame"]
            self.assertIn(f"ids {first}..{first + 3}", out)
            self.assertIn(f"ids {first + 12}..{first + 15}", out)
            kana = (1 << 31) + (1 << 20) + 16 * sharp["frame"]
            self.assertIn(f"ids {kana}..{kana + 6}", out)
            self.assertIn(f"ids {kana + 7}..{kana + 7}", out)
            self.assertIn(f"ids {kana + 15}..{kana + 15}", out)
            barrel = next(c for c in captures_fixture()["captures"] if c["name"] == "barrel-480p")
            lens = Path(directory) / "lens.png"
            write_image(lens, luma_of(barrel))
            status, out, _ = run("inspect", str(lens))
            self.assertEqual(status, 0)
            self.assertIn("lens distortion: -0.120", out)
            shadow = next(
                c for c in captures_fixture()["captures"] if c["name"] == "shadow-band-540p"
            )
            shaded = Path(directory) / "shadow.png"
            write_image(shaded, luma_of(shadow))
            status, out, _ = run("inspect", str(shaded))
            self.assertEqual(status, 0)
            self.assertIn("lanes decoded: 0123; turn lanes: 0123; no kana lane", out)
            self.assertIn("lane D: not decoded", out)
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

    def test_decode_spaces_frames_at_the_default_display_rate(self) -> None:
        # frames are timestamped as shown at the default 12 fps unless told otherwise
        args = _parser().parse_args(["decode", "--output", "out.bin"])
        self.assertEqual(args.frame_interval_ms, 83)

    def test_module_entry_point_runs(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "iroha_petal", "--help"],
            capture_output=True,
            text=True,
            # no bytecode written next to the sources
            env={"PYTHONPATH": str(SRC_DIR), "PYTHONDONTWRITEBYTECODE": "1"},
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("encode", result.stdout)


if __name__ == "__main__":
    unittest.main()
