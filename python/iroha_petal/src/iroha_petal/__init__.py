# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Petal Stream: the Sakura-storm optical transport, in pure Python.

A Petal frame (format 2) is a square image whose data lives in five
independent lanes: the dots on three concentric rings (lane ``D``), and a storm
of 3,376 petals on concentric bands inside them, cut into four sectors (lanes
``F0``-``F3``), each petal lit or dark. The petals inside the SORA ``天`` are
drawn white and the others pink, over a faint ``天``. Every lane is one
Reed-Solomon codeword, so any lane that reads cleanly yields fountain-coded
payload atoms.

Sender::

    encoder = StreamEncoder(payload, kind=1)
    for frame in range(encoder.systematic_frames() * 2):
        rgb = render_frame(encoder.cells(frame), RenderOptions(size=768))

Receiver::

    session = ScanSession()  # tracks the code from frame to frame
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
    FieldEvidence,
    LaneResult,
    decode_frame,
    decode_frame_at,
    field_evidence,
    merge_fields,
    observed_cells,
    track_frame,
)
from .fountain import FountainDecoder, encode_atom, mask_len, mask_words, mix32, split_payload
from .image import Homography, Luma, Pose, Rgb
from .lanes import (
    ATOM_LEN,
    D_DATA,
    D_WORD,
    DECODE_ORDER,
    FIELD_DATA,
    FIELD_LANES,
    FIELD_WORD,
    FrameCells,
    Lane,
    decode_lane,
    decode_lane_counted,
    encode_lane,
)
from .layout import (
    FIELD_CELLS,
    FINDER_CENTERS,
    MASK,
    SECTORS,
    SlotKind,
    SlotRole,
    cell_angle,
    cell_center,
    cell_light,
    data_slots,
    finder_lit,
    petal_lit,
    sector_cells,
    slot_center,
    slot_roles,
    split_slot,
)
from .locate import Finder, FinderSet, locate, locate_candidates
from .pngio import decode_png, encode_pgm, encode_png, image_to_luma, read_luma, write_image
from .prng import Xorshift32
from .render import (
    BackdropCell,
    Circle,
    DrawList,
    FinderShape,
    Palette,
    PetalShape,
    RenderOptions,
    draw_list,
    render_frame,
)
from .rs import ReedSolomon, RsError, RsErrorKind
from .session import TRACK_WINDOW_MS, ScanLimits, ScanOutcome, ScanSession, ScanStats
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
    LaneData,
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
    lane_first_id,
    parse_atom_lane,
    parse_d_lane,
)

__version__ = "0.1.0"

__all__ = [
    "ATOM_LEN",
    "BEACON_INTERVAL",
    "D_DATA",
    "D_WORD",
    "DECODE_ORDER",
    "DEFAULT_MAX_PAYLOAD_LEN",
    "FIELD_CELLS",
    "FIELD_DATA",
    "FIELD_LANES",
    "FIELD_WORD",
    "FINDER_CENTERS",
    "FORMAT_VERSION",
    "MASK",
    "MAX_PAYLOAD_LEN",
    "SECTORS",
    "TRACK_WINDOW_MS",
    "AssemblerLimits",
    "AtomPacket",
    "BackdropCell",
    "Beacon",
    "Circle",
    "Completed",
    "DLane",
    "DecodeError",
    "DecodeErrorKind",
    "DecodeOptions",
    "DecodedFrame",
    "DrawList",
    "FieldEvidence",
    "Finder",
    "FinderSet",
    "FinderShape",
    "FountainDecoder",
    "FrameCells",
    "Homography",
    "Lane",
    "LaneData",
    "LaneHeader",
    "LaneResult",
    "Luma",
    "Palette",
    "PetalShape",
    "Pose",
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
    "Xorshift32",
    "atoms_in_frame",
    "cell_angle",
    "cell_center",
    "cell_light",
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
    "field_evidence",
    "finder_lit",
    "first_atom_id",
    "image_to_luma",
    "is_beacon_frame",
    "lane_first_id",
    "locate",
    "locate_candidates",
    "mask_len",
    "mask_words",
    "merge_fields",
    "mix32",
    "observed_cells",
    "parse_atom_lane",
    "parse_d_lane",
    "petal_lit",
    "read_luma",
    "render_frame",
    "sector_cells",
    "slot_center",
    "slot_roles",
    "split_payload",
    "split_slot",
    "track_frame",
    "write_image",
]
