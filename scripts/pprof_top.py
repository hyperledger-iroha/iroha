#!/usr/bin/env python3
"""
Minimal pprof (profile.proto) decoder to extract flat/cumulative hotspots.

We avoid external deps (no protobuf module) by implementing a small subset of
the Protobuf wire format sufficient for pprof CPU profiles emitted by pprof-rs.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple


class ProtoError(RuntimeError):
    pass


def read_varint(buf: bytes, off: int) -> Tuple[int, int]:
    """Return (value, new_off)."""
    shift = 0
    value = 0
    while True:
        if off >= len(buf):
            raise ProtoError("unexpected EOF while reading varint")
        b = buf[off]
        off += 1
        value |= (b & 0x7F) << shift
        if (b & 0x80) == 0:
            return value, off
        shift += 7
        if shift > 70:
            raise ProtoError("varint too long")


def read_key(buf: bytes, off: int) -> Tuple[int, int, int]:
    key, off = read_varint(buf, off)
    field = key >> 3
    wire = key & 0x07
    if field == 0:
        raise ProtoError("invalid field number 0")
    return field, wire, off


def read_len(buf: bytes, off: int) -> Tuple[bytes, int]:
    n, off = read_varint(buf, off)
    end = off + n
    if end > len(buf):
        raise ProtoError("unexpected EOF while reading length-delimited field")
    return buf[off:end], end


def skip_field(buf: bytes, off: int, wire: int) -> int:
    if wire == 0:  # varint
        _, off = read_varint(buf, off)
        return off
    if wire == 1:  # 64-bit
        off += 8
        if off > len(buf):
            raise ProtoError("unexpected EOF while skipping 64-bit field")
        return off
    if wire == 2:  # len-delimited
        payload, off = read_len(buf, off)
        _ = payload
        return off
    if wire == 5:  # 32-bit
        off += 4
        if off > len(buf):
            raise ProtoError("unexpected EOF while skipping 32-bit field")
        return off
    raise ProtoError(f"unsupported wire type: {wire}")


def read_packed_varints(payload: bytes) -> List[int]:
    vals: List[int] = []
    off = 0
    while off < len(payload):
        v, off = read_varint(payload, off)
        vals.append(v)
    return vals


def zigzag_decode_64(u: int) -> int:
    return (u >> 1) ^ -(u & 1)


@dataclass
class ValueType:
    type_str_idx: int
    unit_str_idx: int


@dataclass
class Function:
    id: int
    name_str_idx: int


@dataclass
class Line:
    function_id: int


@dataclass
class Location:
    id: int
    lines: List[Line]


@dataclass
class Sample:
    location_ids: List[int]
    values: List[int]


@dataclass
class Profile:
    sample_types: List[ValueType]
    samples: List[Sample]
    locations: Dict[int, Location]
    functions: Dict[int, Function]
    strings: List[str]


def parse_value_type(buf: bytes) -> ValueType:
    type_idx = 0
    unit_idx = 0
    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 0:
            type_idx, off = read_varint(buf, off)
        elif field == 2 and wire == 0:
            unit_idx, off = read_varint(buf, off)
        else:
            off = skip_field(buf, off, wire)
    return ValueType(type_str_idx=type_idx, unit_str_idx=unit_idx)


def parse_function(buf: bytes) -> Function:
    fid = 0
    name_idx = 0
    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 0:
            fid, off = read_varint(buf, off)
        elif field == 2 and wire == 0:
            name_idx, off = read_varint(buf, off)
        else:
            off = skip_field(buf, off, wire)
    return Function(id=fid, name_str_idx=name_idx)


def parse_line(buf: bytes) -> Line:
    fn_id = 0
    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 0:
            fn_id, off = read_varint(buf, off)
        else:
            off = skip_field(buf, off, wire)
    return Line(function_id=fn_id)


def parse_location(buf: bytes) -> Location:
    lid = 0
    lines: List[Line] = []
    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 0:
            lid, off = read_varint(buf, off)
        elif field == 4 and wire == 2:
            payload, off = read_len(buf, off)
            lines.append(parse_line(payload))
        else:
            off = skip_field(buf, off, wire)
    return Location(id=lid, lines=lines)


def parse_sample(buf: bytes) -> Sample:
    loc_ids: List[int] = []
    values: List[int] = []
    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 2:
            payload, off = read_len(buf, off)
            loc_ids.extend(read_packed_varints(payload))
        elif field == 2 and wire == 2:
            payload, off = read_len(buf, off)
            # values are *signed* int64, encoded as varint two's complement. Prost/pprof often
            # uses zigzag for sint64 but pprof uses int64, so we decode as signed via zigzag? No.
            # Here pprof-rs uses i64 and encodes via varint with two's complement, which is not
            # representable in Python varint decoding directly for negatives. In practice for CPU
            # profiles these values are non-negative, so plain varint works.
            values.extend(read_packed_varints(payload))
        else:
            off = skip_field(buf, off, wire)
    return Sample(location_ids=loc_ids, values=values)


def parse_profile(buf: bytes) -> Profile:
    sample_types: List[ValueType] = []
    samples: List[Sample] = []
    locations: Dict[int, Location] = {}
    functions: Dict[int, Function] = {}
    strings: List[str] = []

    off = 0
    while off < len(buf):
        field, wire, off = read_key(buf, off)
        if field == 1 and wire == 2:
            payload, off = read_len(buf, off)
            sample_types.append(parse_value_type(payload))
        elif field == 2 and wire == 2:
            payload, off = read_len(buf, off)
            samples.append(parse_sample(payload))
        elif field == 4 and wire == 2:
            payload, off = read_len(buf, off)
            loc = parse_location(payload)
            if loc.id:
                locations[loc.id] = loc
        elif field == 5 and wire == 2:
            payload, off = read_len(buf, off)
            fn = parse_function(payload)
            if fn.id:
                functions[fn.id] = fn
        elif field == 6 and wire == 2:
            payload, off = read_len(buf, off)
            try:
                strings.append(payload.decode("utf-8", errors="replace"))
            except Exception:
                strings.append("")
        else:
            off = skip_field(buf, off, wire)

    if not strings:
        strings = [""]
    return Profile(
        sample_types=sample_types,
        samples=samples,
        locations=locations,
        functions=functions,
        strings=strings,
    )


def choose_value_index(profile: Profile) -> int:
    # Prefer a time-based sample type if present.
    for idx, st in enumerate(profile.sample_types):
        typ = profile.strings[st.type_str_idx] if st.type_str_idx < len(profile.strings) else ""
        unit = profile.strings[st.unit_str_idx] if st.unit_str_idx < len(profile.strings) else ""
        typ_l = typ.lower()
        unit_l = unit.lower()
        if "cpu" in typ_l or "time" in typ_l:
            return idx
        if unit_l in ("nanoseconds", "ns", "microseconds", "us", "milliseconds", "ms", "seconds", "s"):
            return idx
    return 0


def function_name(profile: Profile, fn_id: int) -> str:
    fn = profile.functions.get(fn_id)
    if fn is None:
        return f"<fn:{fn_id}>"
    idx = fn.name_str_idx
    if 0 <= idx < len(profile.strings):
        return profile.strings[idx]
    return f"<fn:{fn_id}>"


def sample_leaf_function_ids(profile: Profile, sample: Sample) -> List[int]:
    if not sample.location_ids:
        return []
    loc = profile.locations.get(sample.location_ids[0])
    if not loc or not loc.lines:
        return []
    # Heuristic: first line tends to represent the leaf-most inlined function for this location.
    return [loc.lines[0].function_id]


def sample_stack_function_ids(profile: Profile, sample: Sample) -> List[int]:
    fns: List[int] = []
    for loc_id in sample.location_ids:
        loc = profile.locations.get(loc_id)
        if not loc:
            continue
        for line in loc.lines:
            if line.function_id:
                fns.append(line.function_id)
    return fns


def compute_hotspots(profile: Profile, top_n: int) -> Tuple[List[Tuple[str, int]], List[Tuple[str, int]]]:
    value_idx = choose_value_index(profile)
    flat: Dict[int, int] = {}
    cum: Dict[int, int] = {}
    total = 0

    for s in profile.samples:
        if value_idx >= len(s.values):
            continue
        v = int(s.values[value_idx])
        if v <= 0:
            continue
        total += v

        leaf_ids = sample_leaf_function_ids(profile, s)
        if leaf_ids:
            fid = leaf_ids[0]
            flat[fid] = flat.get(fid, 0) + v

        seen: set[int] = set()
        for fid in sample_stack_function_ids(profile, s):
            if fid in seen:
                continue
            seen.add(fid)
            cum[fid] = cum.get(fid, 0) + v

    def sort_map(m: Dict[int, int]) -> List[Tuple[str, int]]:
        items = sorted(m.items(), key=lambda kv: kv[1], reverse=True)[:top_n]
        return [(function_name(profile, fid), v) for fid, v in items]

    flat_items = sort_map(flat)
    cum_items = sort_map(cum)
    return flat_items, cum_items


def main(argv: List[str]) -> int:
    ap = argparse.ArgumentParser(description="Extract top pprof hotspots from a pprof profile.pb")
    ap.add_argument("profile", type=Path, help="Path to pprof profile (protobuf, not gz)")
    ap.add_argument("--top", type=int, default=20)
    args = ap.parse_args(argv)

    data = args.profile.read_bytes()
    profile = parse_profile(data)

    value_idx = choose_value_index(profile)
    # Compute total early so we can render percentages.
    total = 0
    for s in profile.samples:
        if value_idx >= len(s.values):
            continue
        v = int(s.values[value_idx])
        if v > 0:
            total += v

    if profile.sample_types and value_idx < len(profile.sample_types):
        st = profile.sample_types[value_idx]
        typ = profile.strings[st.type_str_idx] if st.type_str_idx < len(profile.strings) else ""
        unit = profile.strings[st.unit_str_idx] if st.unit_str_idx < len(profile.strings) else ""
        print(f"Value: idx={value_idx} type={typ!r} unit={unit!r}")
    else:
        print(f"Value: idx={value_idx}")
    print(f"Total: {total} ({total/1e9:.3f}s)")

    flat, cum = compute_hotspots(profile, args.top)

    print("")
    print(f"Top {args.top} flat")
    for name, v in flat:
        pct = (100.0 * v / total) if total else 0.0
        print(f"{v:>12}  {pct:6.2f}%  {name}")

    print("")
    print(f"Top {args.top} cumulative")
    for name, v in cum:
        pct = (100.0 * v / total) if total else 0.0
        print(f"{v:>12}  {pct:6.2f}%  {name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
