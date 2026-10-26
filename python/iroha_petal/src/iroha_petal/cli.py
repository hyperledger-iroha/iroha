# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Command-line interface: ``python3 -m iroha_petal {encode,decode,inspect}``.

* ``encode`` renders numbered frames of a payload stream (PNG, PGM or SVG): cards,
  the square with a band of katakana tiles above and below it.
* ``decode`` feeds image files (any order) through a :class:`ScanSession` and
  writes the reassembled payload; it reports the lanes each frame read (``D``
  and the field sectors ``0``-``3``), the turn lanes it read (by the digit of
  their field lane), the kana lanes it read (``P0``, ``K0``, ``P1``, ``K1``), how
  many frames were read by tracking the previous pose, how many had a hidden
  corner blossom and which field lanes merging recovered.
* ``inspect`` reports what one image decodes to: the orientation, a hidden
  (inferred) corner blossom, the lens distortion, every lane, turn lane and kana
  lane, and the blur the turn reader fits.

No network and no camera access: frames are files.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import List, Optional, Sequence

from .decode import DecodeError, decode_frame
from .kana import KANA_LANES, kana_packet, read_kana
from .lanes import DECODE_ORDER
from .pngio import read_luma, write_image
from .render import RenderOptions, draw_list, render_frame
from .session import ScanSession
from .stream import AtomPacket, Beacon, LaneHeader, StreamEncoder, StreamError
from .turns import observed_turns, read_turns, turn_packet

__all__ = ["main"]

_IMAGE_SUFFIXES = (".png", ".pgm", ".ppm")
#: The default display rate of a Petal stream, in frames per second.
_DEFAULT_FPS = 12


def _encode(args: argparse.Namespace) -> int:
    payload = Path(args.input).read_bytes()
    try:
        encoder = StreamEncoder(payload, args.kind)
    except (StreamError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    systematic = encoder.systematic_frames()
    frames = args.frames if args.frames is not None else max(4, 2 * systematic)
    if not 1 <= frames <= 65_536:
        print("error: --frames must be within 1..=65536", file=sys.stderr)
        return 2
    output = Path(args.output)
    output.mkdir(parents=True, exist_ok=True)
    options = RenderOptions(size=args.size, supersample=args.supersample)
    width = max(4, len(str(frames - 1)))
    for frame in range(frames):
        cells = encoder.cells(frame)
        name = output / f"frame_{frame:0{width}d}.{args.format}"
        if args.format == "svg":
            name.write_text(draw_list(cells).to_svg(args.size), encoding="utf-8")
        else:
            image = render_frame(cells, options)
            write_image(name, image.to_luma() if args.format == "pgm" else image)
    meta = encoder.meta
    print(
        f"wrote {frames} frames to {output} (payload {meta.length} bytes, kind {meta.kind}, "
        f"crc32c 0x{meta.crc:08x}, {meta.source_atoms} source atoms, "
        f"{systematic} systematic frames)"
    )
    return 0


def _collect(args: argparse.Namespace) -> List[Path]:
    paths: List[Path] = [Path(p) for p in (args.input or [])]
    if args.input_dir:
        directory = Path(args.input_dir)
        paths.extend(sorted(p for p in directory.iterdir() if p.suffix.lower() in _IMAGE_SUFFIXES))
    return paths


def _decode(args: argparse.Namespace) -> int:
    paths = _collect(args)
    if not paths:
        print("error: no input frames (use --input or --input-dir)", file=sys.stderr)
        return 2
    # frames read from files keep no camera waiting: every capture has time for the turn lanes
    session = ScanSession(work_cost=lambda _work, _measured_ms: 0.0)
    completed = None
    stats = session.stats()
    for index, path in enumerate(paths):
        image = read_luma(path)
        factor_note = ""
        if args.max_side and max(image.width, image.height) > args.max_side:
            factor = -(-max(image.width, image.height) // args.max_side)
            image = image.downscaled(factor)
            factor_note = f" (downscaled 1/{factor})"
        outcome = session.push(image, index * args.frame_interval_ms)
        status = outcome.error.value if outcome.error is not None else (outcome.lanes or "-")
        before, stats = stats, session.stats()
        if stats.tracked > before.tracked:
            status += " (tracked)"
        if stats.inferred > before.inferred:
            status += " (one corner blossom is hidden)"
        if outcome.merged:
            status += f" (merged {outcome.merged})"
        if outcome.turned:
            status += f" (turn lanes {outcome.turned})"
        if outcome.kana:
            status += f" (kana lanes {outcome.kana})"
        progress = outcome.progress
        if not args.quiet:
            print(
                f"{path.name}: {status}{factor_note}; rank {progress.rank}/{progress.source_atoms}",
                file=sys.stderr,
            )
        if outcome.completed is not None:
            completed = outcome.completed
            break
    if not args.quiet:
        print(
            f"frames: {stats.frames} read, {stats.located} located, {stats.readable} readable, "
            f"{stats.tracked} tracked, {stats.inferred} with a hidden corner blossom; "
            f"lane D read in {stats.lane_d} frames, {stats.field_lanes} field lanes, "
            f"{stats.turn_lanes} turn lanes, {stats.kana_lanes} kana lanes; "
            f"{stats.merges} merges, {stats.merged_lanes} merged field lanes",
            file=sys.stderr,
        )
    if completed is None:
        progress = session.progress()
        print(
            f"incomplete: rank {progress.rank}/{progress.source_atoms} after {len(paths)} frames",
            file=sys.stderr,
        )
        return 1
    Path(args.output).write_bytes(completed.payload)
    print(
        f"wrote {len(completed.payload)} bytes to {args.output} "
        f"(kind {completed.meta.kind}, crc32c 0x{completed.meta.crc:08x})"
    )
    return 0


def _inspect(args: argparse.Namespace) -> int:
    image = read_luma(args.file)
    print(f"image: {image.width}x{image.height}")
    try:
        frame = decode_frame(image, max_side=args.max_side)
    except DecodeError as error:
        print(f"no decode: {error.kind.name.lower()} ({error})")
        return 1
    pose = frame.pose
    print(f"orientation: rotation {frame.rotation} quarter turns, mirrored {frame.mirrored}")
    if frame.inferred_corner is not None:
        # canonical index: 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left
        corner = ("top-left", "top-right", "bottom-right", "bottom-left")[frame.inferred_corner]
        print(f"inferred corner: {frame.inferred_corner} ({corner} blossom hidden)")
    print(f"lens distortion: {pose.distortion:+.3f} (0 is none, negative is barrel)")
    m = pose.homography.m
    print("homography (canvas -> undistorted pixels):")
    for row in range(3):
        print("  " + "  ".join(f"{m[row * 3 + col]: .6g}" for col in range(3)))
    for name, (x, y) in (
        ("canvas centre", (512.0, 512.0)),
        ("top-left finder", (72.0, 72.0)),
        ("bottom-right finder", (952.0, 952.0)),
    ):
        px, py = pose.apply(x, y)
        print(f"  {name}: ({px:.1f}, {py:.1f})")
    turns = read_turns(image, frame)
    turned = "".join(str(sector) for sector, result in enumerate(turns) if result is not None)
    # the kana lanes are bound to the frame the square's lanes name
    lane_d = frame.d_lane()
    packets = list(frame.atom_packets())
    if isinstance(lane_d, Beacon):
        header: Optional[LaneHeader] = lane_d.header
    elif isinstance(lane_d, AtomPacket):
        header = lane_d.header
    else:
        header = packets[0].header if packets else None
    kana = (None,) * len(KANA_LANES) if header is None else read_kana(image, frame, header)
    read = "".join(lane.value for lane, result in zip(KANA_LANES, kana) if result is not None)
    print(
        f"lanes decoded: {frame.lanes or '-'}; "
        + (f"turn lanes: {turned}" if turned else "no turn lane")
        + "; "
        + (f"kana lanes: {read}" if read else "no kana lane")
    )
    observed = observed_turns(image, frame)
    if observed is not None:
        # the turn lanes are read up to a fitted blur of 2.5 canvas units
        print(f"turn blur: {observed.blur:g} canvas units")
    for lane in DECODE_ORDER:
        result = frame.lane(lane)
        if result is None:
            print(f"lane {lane.letter}: not decoded")
        else:
            print(
                f"lane {lane.letter}: ok ({result.corrected} corrected, {result.erasures} erased) "
                f"{result.data.hex()}"
            )
    for sector, result in enumerate(turns):
        if result is not None:
            print(
                f"turn lane {sector}: ok ({result.corrected} corrected, {result.erasures} erased) "
                f"{result.data.hex()}"
            )
    for lane, found in zip(KANA_LANES, kana):
        if found is not None:
            result = found.result
            print(
                f"kana lane {lane.value}: ok ({result.corrected} corrected, {result.erasures} "
                f"erased, frame {found.header.frame}) {result.data.hex()}"
            )
    if isinstance(lane_d, Beacon):
        meta = lane_d.meta
        print(
            f"beacon: frame {lane_d.header.frame}, kind {meta.kind}, length {meta.length}, "
            f"crc32c 0x{meta.crc:08x}"
        )
    for sector, result in enumerate(turns):
        field = frame.fields[sector]
        if result is not None and field is not None:
            field_header = LaneHeader(field.data[0], (field.data[1] << 8) | field.data[2])
            packet = turn_packet(field_header, sector, result.data)
            if packet is not None:
                packets.append(packet)
    for lane, found in zip(KANA_LANES, kana):
        if found is not None:
            packet = kana_packet(found.header, lane, found.result.data)
            if packet is not None:
                packets.append(packet)
    for packet in packets:
        assert isinstance(packet, AtomPacket)
        last = packet.first_id + len(packet.atoms) - 1
        print(
            f"atoms: tag 0x{packet.header.tag:02x} frame {packet.header.frame} "
            f"ids {packet.first_id}..{last}"
        )
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python3 -m iroha_petal",
        description="Petal Stream optical transport: render frames, decode captures.",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    encode = commands.add_parser("encode", help="render the frames (cards) of a payload stream")
    encode.add_argument("--input", required=True, help="payload file")
    encode.add_argument("--output", required=True, help="output directory")
    encode.add_argument("--kind", type=int, default=0, help="payload kind byte (default 0)")
    encode.add_argument(
        "--frames",
        type=int,
        help="number of frames (default: twice the systematic frames, at least 4)",
    )
    encode.add_argument(
        "--size",
        type=int,
        default=768,
        help="card width in pixels; the card is 1668/1024 of it tall (default 768)",
    )
    encode.add_argument(
        "--supersample", type=int, default=2, help="anti-aliasing samples per side, 1-4 (default 2)"
    )
    encode.add_argument("--format", choices=("png", "pgm", "svg"), default="png")
    encode.set_defaults(handler=_encode)

    decode = commands.add_parser("decode", help="reassemble a payload from captured frames")
    decode.add_argument("--input", action="append", help="frame image (repeatable)")
    decode.add_argument("--input-dir", help="directory of PNG/PGM/PPM frames, read in name order")
    decode.add_argument("--output", required=True, help="where to write the payload")
    decode.add_argument(
        "--max-side",
        type=int,
        default=1280,
        help="downscale larger frames to fit this side before decoding (0 disables; default 1280)",
    )
    decode.add_argument(
        "--frame-interval-ms",
        type=int,
        default=1000 // _DEFAULT_FPS,
        help="timestamp spacing between frames for the session timeouts "
        f"(default {1000 // _DEFAULT_FPS}: the {_DEFAULT_FPS} fps display rate)",
    )
    decode.add_argument("--quiet", action="store_true", help="do not report every frame")
    decode.set_defaults(handler=_decode)

    inspect = commands.add_parser("inspect", help="report what one image decodes to")
    inspect.add_argument("file", help="PNG, PGM or PPM image")
    inspect.add_argument(
        "--max-side", type=int, default=None, help="downscale larger images to fit this side"
    )
    inspect.set_defaults(handler=_inspect)
    return parser


def main(argv: Optional[Sequence[str]] = None) -> int:
    """Run the command line; returns the process exit status."""
    args = _parser().parse_args(argv)
    return int(args.handler(args))


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
