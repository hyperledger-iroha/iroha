# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Cross-language dense fixtures and strict layout-boundary checks."""

import json
from pathlib import Path
import unittest

from norito import (
    Err, Ok, SchemaDescriptor, StructAdapter, StructField, WireLayout,
    bool_, bytes_, decode, decode_adaptive, encode, encode_adaptive,
    encode_with_header_flags, fixed_bytes, from_bytes_view, has_zstd,
    map_adapter, option, result, seq, string, try_decode, tuple_adapter,
    u8, u16, u32, u64, u128,
)
from norito.codec import (
    DecodeFlagsGuard, NoritoDecoder, NoritoEncoder, effective_decode_flags,
    payload_root_bytes,
)
from norito.compression import compress_zstd, decompress_zstd
from norito.crc64 import crc64
from norito.errors import DecodeError, NoritoError, UnsupportedVersionError
from norito.header import COMPACT_LEN, NoritoHeader


ROOT = Path(__file__).resolve().parents[3]
VECTORS = json.loads((ROOT / "fixtures/norito_dense/vectors.json").read_text())
DENSE = WireLayout.DENSE
CURRENT = WireLayout.CURRENT


class ChoiceAdapter:
    """The fixture's explicitly tagged enum, using the shared codec helpers."""

    variants = {1: string(), 300: u64()}

    def encode(self, encoder, value):
        tag, child = value
        encoder.write_variant_index(tag)
        if tag:
            encoder.write_field(self.variants[tag], child)

    def decode(self, decoder):
        tag = decoder.read_variant_index()
        if tag == 0:
            return (tag, None)
        if tag not in self.variants:
            raise DecodeError("unknown Choice tag")
        return (tag, decoder.read_field(self.variants[tag]))

    def fixed_size(self):
        return None

    def is_self_delimiting(self):
        return True


CHOICE = ChoiceAdapter()
RECORD = StructAdapter([
    StructField("id", u32()),
    StructField("key", fixed_bytes(4)),
    StructField("digests", seq(fixed_bytes(4, framed=True), framed=True)),
    StructField("label", option(string(), framed=True)),
    StructField("choice", CHOICE),
], framed=True)
WIDE = StructAdapter([
    StructField("value", u128()), StructField("tag", fixed_bytes(2)),
], framed=True)
CASES = {
    "u8_vec": (seq(u8()), [1, 2, 3]),
    "u8_vec_128": (seq(u8()), [0x11] * 128),
    "u8_array": (fixed_bytes(4, framed=True), bytes([1, 2, 3, 4])),
    "option_u8_array": (option(fixed_bytes(4, framed=True), framed=True), b"\x09" * 4),
    "option_none": (option(fixed_bytes(4, framed=True), framed=True), None),
    "vec_u8_array": (seq(fixed_bytes(2, framed=True), framed=True), [b"\x07" * 2, b"\x08" * 2]),
    "vec_string": (seq(string(), framed=True), ["ab"]),
    "vec_string_empty": (seq(string(), framed=True), []),
    "map_string_u16": (map_adapter(string(), u16(), framed=True), {"k": 5}),
    "enum_unit": (CHOICE, (0, None)),
    "enum_tuple": (CHOICE, (1, "hi")),
    "enum_far": (CHOICE, (300, 1)),
    "result_err": (result(u8(), u8(), framed=True), Err(7)),
    "record": (RECORD, {
        "id": 7, "key": bytes([1, 2, 3, 4]),
        "digests": [b"\xaa" * 4, b"\xbb" * 4],
        "label": "hi", "choice": (300, 1),
    }),
}


class DenseLayoutTests(unittest.TestCase):
    def test_streaming_adapters_propagate_dense_tags_and_fields(self):
        from norito import streaming as s

        cases = [
            (s.profile_id_adapter(), s.ProfileId(1), "020100"),
            (s.simple_enum_adapter(s.HpkeSuite), s.HpkeSuite.KYBER1024_AUTH_PSK, "01"),
            (s.encryption_suite_adapter(), s.X25519ChaCha20Poly1305Suite(bytes(32)), "0020" + "00" * 32),
            (s.encryption_suite_adapter(), s.Kyber768XChaCha20Poly1305Suite(bytes(32)), "0120" + "00" * 32),
            (s.resolution_adapter(), s.Resolution(s.ResolutionKind.CUSTOM, s.ResolutionCustom(1, 2)), "0406020100020200"),
            (s.resolution_adapter(), s.Resolution(s.ResolutionKind.R720P), "00"),
            (s.telemetry_event_adapter(), s.TelemetryEnergyEvent(s.TelemetryEnergyStats(1, 2, 3)), "041308010000000000000004020000000403000000"),
            (s.control_frame_adapter(), s.ControlFrame(s.ControlFrameVariant.CHUNK_REQUEST, s.ChunkRequestFrame(1, 2)), "010c080100000000000000020200"),
        ]
        for adapter, value, expected in cases:
            with self.subTest(value=value):
                dense = bytes.fromhex(expected)
                self.assertEqual(encode_adaptive(value, adapter, layout=DENSE), dense)
                self.assertEqual(decode_adaptive(dense, adapter, layout=DENSE), value)
                current = encode_adaptive(value, adapter)
                self.assertEqual(decode_adaptive(current, adapter), value)

    def test_every_bare_vector_and_current_twin(self):
        self.assertEqual(set(CASES), {row["name"] for row in VECTORS["bare"]})
        for row in VECTORS["bare"]:
            adapter, expected = CASES[row["name"]]
            for layout, key, flags in ((DENSE, "dense", 0), (CURRENT, "compact", COMPACT_LEN)):
                with self.subTest(vector=row["name"], layout=layout):
                    payload = bytes.fromhex(row[key])
                    value = decode_adaptive(payload, adapter, layout=layout, flags=flags)
                    self.assertEqual(value, expected)
                    self.assertEqual(encode_adaptive(value, adapter, layout=layout, flags=flags), payload)

    def test_nested_record_refusal_and_success_restore_the_callers_decode_scope(self):
        row = next(row for row in VECTORS["bare"] if row["name"] == "record")
        adapter, expected = CASES["record"]
        for layout, key, flags in ((CURRENT, "compact", COMPACT_LEN), (DENSE, "dense", 0)):
            with self.subTest(layout=layout):
                payload = bytes.fromhex(row[key])
                outer_flags = 0 if flags == COMPACT_LEN else COMPACT_LEN
                with DecodeFlagsGuard.enter(outer_flags):
                    with self.assertRaises(DecodeError):
                        decode_adaptive(payload[:-1], adapter, layout=layout, flags=flags)
                    self.assertIsNone(payload_root_bytes())
                    self.assertEqual(effective_decode_flags(), outer_flags)
                    self.assertEqual(decode_adaptive(payload, adapter, layout=layout, flags=flags), expected)
                    self.assertIsNone(payload_root_bytes())
                    self.assertEqual(effective_decode_flags(), outer_flags)

    def test_every_framed_vector_and_current_twin(self):
        self.assertEqual([row["name"] for row in VECTORS["frames"]], ["wide"])
        expected = {"value": 0x0102030405060708090A0B0C0D0E0F10, "tag": b"\xca\xfe"}
        for row in VECTORS["frames"]:
            schema = SchemaDescriptor.type_name(row["schema_name"])
            for layout, key in ((DENSE, "dense"), (CURRENT, "compact")):
                with self.subTest(layout=layout):
                    frame = bytes.fromhex(row[key])
                    view = from_bytes_view(frame, schema=schema, layout=layout)
                    self.assertEqual(view.layout, layout)
                    self.assertEqual(view.flags, 0 if layout is DENSE else COMPACT_LEN)
                    self.assertEqual(view.decode(WIDE), expected)
                    self.assertEqual(decode(frame, WIDE, schema=schema), expected)
                    self.assertEqual(try_decode(frame, WIDE, schema=schema, layout=layout), Ok(expected))
                    if layout is DENSE:
                        self.assertEqual(encode(expected, schema, WIDE, layout=DENSE), frame)
                    else:
                        # The current fixture's eight alignment bytes remain header padding.
                        self.assertEqual(encode_adaptive(expected, WIDE, flags=COMPACT_LEN), view.as_bytes())

    def test_truncated_and_suffixed_bare_values_are_rejected(self):
        for row in VECTORS["bare"]:
            payload = bytes.fromhex(row["dense"])
            adapter, _ = CASES[row["name"]]
            for cut in range(len(payload)):
                with self.subTest(vector=row["name"], cut=cut), self.assertRaises(NoritoError):
                    decode_adaptive(payload[:cut], adapter, layout=DENSE)
            for suffix in (b"\x00", b"\xff"):
                with self.subTest(vector=row["name"], suffix=suffix), self.assertRaises(NoritoError):
                    decode_adaptive(payload + suffix, adapter, layout=DENSE)

    def test_header_versions_flags_padding_and_exact_frame_boundary(self):
        frame = bytes.fromhex(VECTORS["frames"][0]["dense"])
        for cut in range(len(frame)):
            with self.subTest(cut=cut), self.assertRaises(NoritoError):
                decode(frame[:cut], WIDE)
        for minor in range(2, 256):
            with self.subTest(minor=minor), self.assertRaises(UnsupportedVersionError):
                decode(frame[:5] + bytes([minor]) + frame[6:], WIDE)
        for flag in range(1, 256):
            with self.subTest(flag=flag), self.assertRaises(NoritoError):
                decode(frame[:39] + bytes([flag]) + frame[40:], WIDE)
        for bad in (frame + b"\x00", frame + b"\xff", frame[:40] + b"\x00" + frame[40:]):
            with self.subTest(data=bad), self.assertRaises(NoritoError):
                decode(bad, WIDE)
        for layout, key, wrong in ((DENSE, "dense", CURRENT), (CURRENT, "compact", DENSE)):
            frame = bytes.fromhex(VECTORS["frames"][0][key])
            with self.subTest(layout=layout):
                with self.assertRaises(UnsupportedVersionError):
                    decode(frame, WIDE, layout=wrong)
                with self.assertRaises(UnsupportedVersionError):
                    from_bytes_view(frame, layout=wrong)
                self.assertIsInstance(try_decode(frame, WIDE, layout=wrong), Err)

    def test_layout_is_never_inferred_from_payload_bytes(self):
        adapter = seq(string(), framed=True)
        schema = SchemaDescriptor.type_name("dense.test.Sequence")
        for layout in (CURRENT, DENSE):
            frame = encode(["ab"], schema, adapter, layout=layout)
            wrong = DENSE if layout is CURRENT else CURRENT
            retagged = frame[:5] + bytes([wrong.value]) + frame[6:]
            with self.subTest(layout=layout), self.assertRaises(NoritoError):
                decode(retagged, adapter)

    def test_dense_varints_are_shortest_and_bounded(self):
        for payload in (b"\x80\x00", b"\x81\x00\x01", b"\x80", b"\x80" * 9 + b"\x02", b"\xff" * 10):
            with self.subTest(payload=payload), self.assertRaises(NoritoError):
                decode_adaptive(payload, seq(u8()), layout=DENSE)
        for value in (0, 127, 128, (1 << 64) - 1):
            writer = NoritoEncoder(0, layout=DENSE)
            writer.write_count(value)
            self.assertEqual(NoritoDecoder(writer.finish(), 0, layout=DENSE).read_count(), value)
        for value in (-1, 1 << 64):
            with self.assertRaises(ValueError):
                NoritoEncoder(0, layout=DENSE).write_count(value)
        for value in (-1, 1 << 32):
            with self.assertRaises(ValueError):
                NoritoEncoder(0, layout=DENSE).write_variant_index(value)
        with self.assertRaises(DecodeError):
            NoritoDecoder(b"\x80\x80\x80\x80\x10", 0, layout=DENSE).read_variant_index()

    def test_dense_child_boundaries_and_invalid_tags(self):
        for adapter, payload in (
            (option(u8()), b"\x01\x02\x07\x00"),
            (result(u8(), u8()), b"\x01\x02\x07\x00"),
            (tuple_adapter(u8()), b"\x02\x07\x00"),
            (seq(u8()), b"\x02\x07"),
            (option(u8()), b"\x02"),
            (result(u8(), u8()), b"\x02"),
            (bool_(), b"\x02"),
        ):
            with self.subTest(payload=payload, adapter=adapter), self.assertRaises(NoritoError):
                decode_adaptive(payload, adapter, layout=DENSE)
        with self.assertRaises(DecodeError):
            NoritoDecoder(b"", 0, offset=-1)
        with self.assertRaises(DecodeError):
            NoritoDecoder(b"", 0).read_exact(-1)

    def test_dense_composites_use_the_same_child_layout(self):
        adapter = tuple_adapter(bytes_(), result(u16(), string()), option(u32()), bool_())
        value = (b"abc", Ok(513), 7, True)
        expected = bytes.fromhex("04036162630400020102060104070000000101")
        self.assertEqual(encode_adaptive(value, adapter, layout=DENSE), expected)
        self.assertEqual(decode_adaptive(expected, adapter, layout=DENSE), value)
        self.assertEqual(encode_with_header_flags(value, adapter, layout=DENSE), (expected, 0))
        self.assertEqual(option(u32(), framed=True).fixed_size(), None)
        self.assertEqual(fixed_bytes(2, framed=True).fixed_size(), None)
        self.assertEqual(tuple_adapter(u8(), framed=True).fixed_size(), None)
        self.assertEqual(tuple_adapter(u8()).fixed_size(), 1)

    def test_dense_maps_are_sorted_and_reject_duplicate_or_descending_keys(self):
        adapter = map_adapter(u8(), u8())
        payload = bytes.fromhex("020101010a01020114")
        self.assertEqual(encode_adaptive({2: 20, 1: 10}, adapter, layout=DENSE), payload)
        self.assertEqual(decode_adaptive(payload, adapter, layout=DENSE), {1: 10, 2: 20})
        for bad in (bytes.fromhex("020101010a01010114"), bytes.fromhex("020102010a01010114"), b"\xff\x01"):
            with self.assertRaises(DecodeError):
                decode_adaptive(bad, adapter, layout=DENSE)

    def test_current_explicit_field_shapes_keep_fixed_and_compact_lengths(self):
        adapter = tuple_adapter(option(u16(), framed=True), result(u8(), u8(), framed=True), framed=True)
        value = (513, Ok(7))
        for flags in (0, COMPACT_LEN):
            payload = encode_adaptive(value, adapter, flags=flags)
            self.assertEqual(decode_adaptive(payload, adapter, flags=flags), value)
        # The existing unframed map shape remains composable, too.
        adapter = map_adapter(u8(), u16())
        payload = encode_adaptive({1: 2}, adapter)
        self.assertEqual(payload, b"\x01" + b"\x00" * 7 + b"\x01\x02\x00")
        self.assertEqual(decode_adaptive(payload, adapter), {1: 2})

    def test_encoder_rejects_invalid_layout_metadata(self):
        for major, minor, flags in ((1, 0, 0), (0, 2, 0), (0, 1, COMPACT_LEN), (0, 1, 256)):
            with self.subTest(major=major, minor=minor, flags=flags), self.assertRaises(NoritoError):
                NoritoHeader(bytes(16), 0, crc64(b""), flags, major=major, minor=minor).encode()
        with self.assertRaises(NoritoError):
            encode([], SchemaDescriptor.type_name("dense.test"), seq(u8()), flags=COMPACT_LEN, layout=DENSE)

    @unittest.skipUnless(has_zstd(), "zstandard extra is not installed")
    def test_compressed_dense_frames_are_exact(self):
        schema = SchemaDescriptor.type_name("dense.test.Compressed")
        adapter = seq(string())
        frame = encode(["dense"], schema, adapter, layout=DENSE, compression=True)
        self.assertEqual(decode(frame, adapter), ["dense"])
        self.assertEqual(decompress_zstd(compress_zstd(b"hello")), b"hello")
        with self.assertRaises(NoritoError):
            decompress_zstd(compress_zstd(b"hello"), target_size=6)
        for bad in (frame[:-1], frame + b"\x00", frame[:40] + b"\x00" + frame[40:]):
            with self.assertRaises(NoritoError):
                decode(bad, adapter)


if __name__ == "__main__":
    unittest.main()
