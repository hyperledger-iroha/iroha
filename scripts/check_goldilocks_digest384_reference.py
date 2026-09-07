#!/usr/bin/env python3
"""Independently reproduce the final six-lane Goldilocks digest and frame KAT.

This dependency-free reference uses hashlib SHAKE256 and Python integer modular
arithmetic. Literal domains and MDS values deliberately do not import Rust
implementation output. A successful check is local conformance evidence only.
"""

import hashlib
import json
import struct

P = 2**64 - 2**32 + 1
DOMAIN = b"iroha:goldilocks-digest384:message-frame:v1"
GENERATOR = b"shake256-rejection-sampling-u64le-below-goldilocks-v1"
SEED = b"iroha:first-release:native-stark:goldilocks-digest384:2026-08-28:v1"
PARAM_DOMAIN = b"iroha:goldilocks-digest384:poseidon-x7:parameter-generator:v1"
MDS = [
    [0x982513A23D22B592, 0xA3115DB8CF1D9C90, 0x46BA684B9EEE84B7],
    [0xBE3DCE25491DB768, 0xFB0A6F731943519F, 0xFCE5BD953CDE1896],
    [0xE624719C41EB1A09, 0xD2221B0F1AA2EBC4, 0x1AB5E60D03AD44BC],
]


def field(tag, data):
    """Frame one tagged byte string into canonical seven-byte field words."""
    words = [tag, len(data)]
    for start in range(0, len(data) - len(data) % 7, 7):
        words.append(int.from_bytes(data[start : start + 7], "little"))
    tail = data[len(data) - len(data) % 7 :]
    words.append(int.from_bytes(tail + b"\x01", "little"))
    return words


def frame(lane):
    """Return the complete padded KAT message frame for one canonical lane."""
    if not 0 <= lane < 6:
        raise ValueError("lane must be in 0..6")
    vals = [
        DOMAIN,
        b"iroha-privacy-exact12-v1",
        b"test-protocol-v1",
        b"stark-fri-poseidon-x7-goldilocks-6x64-v1",
        b"trace-merkle",
        b"leaf",
        struct.pack("<Q", 0),
        struct.pack("<Q", 7),
        struct.pack("<Q", 0),
        struct.pack("<Q", lane),
    ]
    words = []
    for tag, value in enumerate(vals, 1):
        words += field(tag, value)
    words += [11, 1] + field(12, b"payload") + [1]
    if len(words) % 2:
        words += [0]
    return words


def params(lane):
    """Generate the lane IV and 65 rounds by SHAKE256 rejection sampling."""
    if not 0 <= lane < 6:
        raise ValueError("lane must be in 0..6")
    preimage = (
        PARAM_DOMAIN
        + struct.pack("<Q", len(GENERATOR))
        + GENERATOR
        + struct.pack("<Q", len(SEED))
        + SEED
        + struct.pack("<Q", lane)
    )
    # All six frozen lanes yield their complete 198-word parameter set within
    # this bounded prefix. A changed generator must fail instead of padding it.
    stream = hashlib.shake_256(preimage).digest(4096)
    words = [v[0] for v in struct.iter_unpack("<Q", stream) if v[0] < P][:198]
    if len(words) != 198:
        raise AssertionError("the frozen lane parameter prefix is incomplete")
    return words[:3], [words[i : i + 3] for i in range(3, 198, 3)]


def permute(state, rounds):
    """Evaluate the width-three x7 permutation with exact integer arithmetic."""
    for r, constants in enumerate(rounds):
        state = [(a + b) % P for a, b in zip(state, constants)]
        full = r < 4 or r >= 61
        state = [pow(v, 7, P) if full or i == 0 else v for i, v in enumerate(state)]
        state = [sum(c * v for c, v in zip(row, state)) % P for row in MDS]
    return state


def check_reference():
    """Check all six digest words and return the exact canonical frame identity."""
    roots = []
    frames = []
    asset = bytearray(b"iroha:goldilocks-digest384:parameter-asset:v1" + GENERATOR + SEED)
    for lane in range(6):
        state, rounds = params(lane)
        asset += struct.pack("<Q", lane)
        asset += struct.pack("<3Q", *state)
        for row in rounds:
            asset += struct.pack("<3Q", *row)
        words = frame(lane)
        for offset in range(0, len(words), 2):
            state[0] = (state[0] + words[offset]) % P
            state[1] = (state[1] + words[offset + 1]) % P
            state = permute(state, rounds)
        roots.append(state[0])
        frames.append(words)
    for row in MDS:
        asset += struct.pack("<3Q", *row)
    expected = [
        0x0A084D2765A9990B,
        0xD59F602C37B69E1B,
        0xDE9BB3357209FA18,
        0x3FAF16BA65A67BA3,
        0xE68CCC7D9933B79D,
        0xCAD66B9479314D52,
    ]
    if roots != expected:
        raise AssertionError((roots, expected))
    offsets = [i for i, (a, b) in enumerate(zip(frames[0], frames[5])) if a != b]
    if offsets != [49] or len(frames[0]) != 58:
        raise AssertionError("canonical frame geometry differs")
    encoded = struct.pack("<" + "Q" * len(frames[0]), *frames[0])
    asset_hash = hashlib.sha3_256(asset).hexdigest()
    frame_hash = hashlib.sha3_256(encoded).hexdigest()
    if asset_hash != "84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6":
        raise AssertionError("canonical parameter asset differs")
    if frame_hash != "acaf7b7d927aab8e24da3a9f8ea8bd502abf2ba858e17fe92eddc42dbe7a9532":
        raise AssertionError("canonical lane-zero frame differs")
    return {
        "frame_words": len(frames[0]),
        "lane_word_index": offsets[0],
        "lane0_frame_sha3_256": frame_hash,
        "parameter_asset_sha3_256": asset_hash,
        "digest_words": [f"{v:016x}" for v in roots],
        "reference": "Python hashlib SHAKE256 and integer modular arithmetic; literal V1 domains and MDS",
    }


if __name__ == "__main__":
    print(json.dumps(check_reference(), indent=2))
