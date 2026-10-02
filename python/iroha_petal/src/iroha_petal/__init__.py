# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Petal Stream: the Sakura-storm optical transport, in pure Python.

A Petal frame is a square image whose data lives in three independent lanes:
the light/dark polarity of 256 tiles shaped like the SORA ``天`` (lane ``P``),
the katakana glyph drawn in each tile (lane ``K``) and the dots on three
concentric rings (lane ``D``). Every lane is one Reed-Solomon codeword, so any
lane that reads cleanly yields fountain-coded payload atoms.

Sender::

    encoder = StreamEncoder(payload, kind=1)
    for frame in range(encoder.systematic_frames() * 2):
        rgb = render_frame(encoder.cells(frame), RenderOptions(size=768))

Receiver::

    session = ScanSession()
    outcome = session.push(Luma(width, height, y_plane), now_ms)
    if outcome.completed is not None:
        payload = outcome.completed.payload

This package is a bit-exact port of the Rust reference crate
``crates/iroha_petal`` and passes the shared golden fixtures in
``fixtures/petal``. It depends only on the Python standard library.
"""

from .crc import crc32c
from .decode import (
    DecodedFrame,
    DecodeError,
    DecodeErrorKind,
    DecodeOptions,
    LaneResult,
    decode_frame,
    decode_frame_at,
    observed_cells,
    tile_match_error,
)
from .fountain import FountainDecoder, encode_atom, mask_len, mask_words, mix32, split_payload
from .glyphs import GLYPH_CHARS, GLYPH_COUNT, STROKES, TEMPLATES, generate_templates, is_inked
from .image import Homography, Luma, Rgb
from .lanes import (
    ATOM_LEN,
    D_DATA,
    D_WORD,
    K_DATA,
    K_WORD,
    P_DATA,
    P_WORD,
    FrameCells,
    Lane,
    decode_lane,
    decode_lane_counted,
    encode_lane,
)
from .layout import (
    FINDER_CENTERS,
    MASK,
    TILES,
    SlotKind,
    SlotRole,
    data_slots,
    finder_lit,
    slot_center,
    slot_roles,
    split_slot,
    tile_center,
)
from .locate import Finder, locate
from .pngio import decode_png, encode_pgm, encode_png, image_to_luma, read_luma, write_image
from .prng import Xorshift32
from .render import (
    Circle,
    DrawList,
    FinderShape,
    Palette,
    RenderOptions,
    TileShape,
    draw_list,
    render_frame,
)
from .rs import ReedSolomon, RsError, RsErrorKind
from .session import ScanLimits, ScanOutcome, ScanSession, ScanStats
from .stream import (
    BEACON_INTERVAL,
    DEFAULT_MAX_PAYLOAD_LEN,
    FORMAT_VERSION,
    MAX_PAYLOAD_LEN,
    AssemblerLimits,
    AtomPacket,
    Beacon,
    Completed,
    DLane,
    LaneHeader,
    Progress,
    StreamAssembler,
    StreamEncoder,
    StreamError,
    StreamErrorKind,
    StreamMeta,
    atoms_in_frame,
    first_atom_id,
    is_beacon_frame,
    parse_atom_lane,
    parse_d_lane,
)

__version__ = "0.1.0"

__all__ = [
    "ATOM_LEN",
    "BEACON_INTERVAL",
    "D_DATA",
    "D_WORD",
    "DEFAULT_MAX_PAYLOAD_LEN",
    "FINDER_CENTERS",
    "FORMAT_VERSION",
    "GLYPH_CHARS",
    "GLYPH_COUNT",
    "K_DATA",
    "K_WORD",
    "MASK",
    "MAX_PAYLOAD_LEN",
    "P_DATA",
    "P_WORD",
    "STROKES",
    "TEMPLATES",
    "TILES",
    "AssemblerLimits",
    "AtomPacket",
    "Beacon",
    "Circle",
    "Completed",
    "DLane",
    "DecodeError",
    "DecodeErrorKind",
    "DecodeOptions",
    "DecodedFrame",
    "DrawList",
    "Finder",
    "FinderShape",
    "FountainDecoder",
    "FrameCells",
    "Homography",
    "Lane",
    "LaneHeader",
    "LaneResult",
    "Luma",
    "Palette",
    "Progress",
    "ReedSolomon",
    "RenderOptions",
    "Rgb",
    "RsError",
    "RsErrorKind",
    "ScanLimits",
    "ScanOutcome",
    "ScanSession",
    "ScanStats",
    "SlotKind",
    "SlotRole",
    "StreamAssembler",
    "StreamEncoder",
    "StreamError",
    "StreamErrorKind",
    "StreamMeta",
    "TileShape",
    "Xorshift32",
    "atoms_in_frame",
    "crc32c",
    "data_slots",
    "decode_frame",
    "decode_frame_at",
    "decode_lane",
    "decode_lane_counted",
    "decode_png",
    "draw_list",
    "encode_atom",
    "encode_lane",
    "encode_pgm",
    "encode_png",
    "finder_lit",
    "first_atom_id",
    "generate_templates",
    "image_to_luma",
    "is_beacon_frame",
    "is_inked",
    "locate",
    "mask_len",
    "mask_words",
    "mix32",
    "observed_cells",
    "parse_atom_lane",
    "parse_d_lane",
    "read_luma",
    "render_frame",
    "slot_center",
    "slot_roles",
    "split_payload",
    "split_slot",
    "tile_center",
    "tile_match_error",
    "write_image",
]
