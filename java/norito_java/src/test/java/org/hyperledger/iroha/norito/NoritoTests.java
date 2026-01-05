// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

public final class NoritoTests {
  private NoritoTests() {}

  public static void main(String[] args) {
    testCrc64();
    testHeaderRoundtrip();
    testHeaderPaddingAccepted();
    testSchemaHashCanonicalPath();
    testEncodeDecodeUInt();
    testEncodeDecodeString();
    testEncodeDecodeSequence();
    testSequenceAcceptsEmptyPackedTail();
    testSequenceAcceptsEmptyPackedTailWithFollowingData();
    testOption();
    testResult();
    testTryDecode();
    testTryDecodeSurfacesStructErrors();
    testArchiveViewFlagsHint();
    testChecksumMismatch();
    testStructAdapter();
    testMapAdapter();
    testMapAdapterSortsKeys();
    testPackedMapLayout();
    testStructuralSchemaHash();
    testCompression();
    testCompressionProfiles();
    testColumnarHelpers();
    testDecodeRequiresCompactLenFlag();
    testSequenceLenRespectsExplicitFlags();
    testEncodeWithHeaderFlagsAdaptiveBits();
    testDecodeAdaptiveRespectsGuard();
    testDecodeAdaptiveInstallsRootPayload();
    testEffectiveDecodeFlagsDefaultState();
    testEffectiveDecodeFlagsGuardHint();
    testEffectiveDecodeFlagsHeaderContext();
    testHeaderFlagsOverrideDecodeGuard();
    testEffectiveDecodeFlagsAdaptiveContext();
    testVarintRejectsOverlongEncodings();
    testPublicKeyCanonicalArchive();
    testEncryptionSuiteAdapterRoundtrip();
    testTelemetryEventAdapterRoundtrip();
    testControlFrameManifestRoundtrip();
    testControlFrameTransportCapsRoundtrip();
    testControlFrameErrorRoundtrip();
    testStreamingTicketRoundtrip();
    testTicketRevocationRoundtrip();
    System.out.println("All Norito Java tests passed.");
  }

  private static void testCompressionProfiles() {
    NoritoCodec.CompressionConfig.CompressionProfile fast =
        NoritoCodec.CompressionConfig.CompressionProfile.FAST;
    NoritoCodec.CompressionConfig.CompressionProfile balanced =
        NoritoCodec.CompressionConfig.CompressionProfile.BALANCED;
    NoritoCodec.CompressionConfig.CompressionProfile compact =
        NoritoCodec.CompressionConfig.CompressionProfile.COMPACT;

    assert NoritoCodec.CompressionConfig.zstdProfile(fast, 32 * 1024).level() == 1;
    assert NoritoCodec.CompressionConfig.zstdProfile(fast, 128 * 1024).level() == 2;
    assert NoritoCodec.CompressionConfig.zstdProfile(fast, 2 * 1024 * 1024).level() == 3;
    assert NoritoCodec.CompressionConfig.zstdProfile(fast, 8 * 1024 * 1024).level() == 4;

    assert NoritoCodec.CompressionConfig.zstdProfile(balanced, 32 * 1024).level() == 3;
    assert NoritoCodec.CompressionConfig.zstdProfile(balanced, 128 * 1024).level() == 5;
    assert NoritoCodec.CompressionConfig.zstdProfile(balanced, 2 * 1024 * 1024).level() == 7;
    assert NoritoCodec.CompressionConfig.zstdProfile(balanced, 8 * 1024 * 1024).level() == 9;

    assert NoritoCodec.CompressionConfig.zstdProfile(compact, 32 * 1024).level() == 7;
    assert NoritoCodec.CompressionConfig.zstdProfile(compact, 128 * 1024).level() == 11;
    assert NoritoCodec.CompressionConfig.zstdProfile(compact, 2 * 1024 * 1024).level() == 15;
    assert NoritoCodec.CompressionConfig.zstdProfile(compact, 8 * 1024 * 1024).level() == 19;

    assert NoritoCodec.CompressionConfig.zstdProfile("balanced", 256 * 1024).level() == 5;
    assert NoritoCodec.CompressionConfig.zstdProfile("compact", 6 * 1024 * 1024).level() == 19;

    boolean unknownProfileFailed = false;
    try {
      NoritoCodec.CompressionConfig.zstdProfile("unknown", 1024);
    } catch (IllegalArgumentException ex) {
      unknownProfileFailed = true;
    }
    assert unknownProfileFailed : "Expected unknown profile to throw";

    boolean negativeLenFailed = false;
    try {
      NoritoCodec.CompressionConfig.zstdProfile(fast, -1);
    } catch (IllegalArgumentException ex) {
      negativeLenFailed = true;
    }
    assert negativeLenFailed : "Expected negative payload length to throw";
  }

  private static byte[] fill(int seed, int length) {
    byte[] data = new byte[length];
    for (int i = 0; i < length; i++) {
      data[i] = (byte) ((seed + i) & 0xFF);
    }
    return data;
  }

  private static void testCrc64() {
    long crc = CRC64.compute("123456789".getBytes());
    assert crc == 0x6C40DF5F0B497347L : "CRC64 mismatch";
  }

  private static void testSchemaHashCanonicalPath() {
    byte[] hash = SchemaHash.hash16("iroha.test.Header");
    assert toHex(hash).equals("3d20b9b63277094a3d20b9b63277094a") : "Canonical schema hash mismatch";
  }

  private static void testHeaderRoundtrip() {
    byte[] schemaHash = SchemaHash.hash16("iroha.test.Header");
    NoritoHeader header =
        new NoritoHeader(schemaHash, 4, 0x12345678ABCDEFL, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.COMPRESSION_NONE);
    byte[] encoded = header.encode();
    byte[] payload = new byte[] {1, 2, 3, 4};
    byte[] combined = new byte[encoded.length + payload.length];
    System.arraycopy(encoded, 0, combined, 0, encoded.length);
    System.arraycopy(payload, 0, combined, encoded.length, payload.length);
    NoritoHeader.DecodeResult result = NoritoHeader.decode(combined, schemaHash);
    assert result.header().checksum() == header.checksum();
    assert result.payload().length == 4;
  }

  private static void testHeaderPaddingAccepted() {
    byte[] schemaHash = SchemaHash.hash16("iroha.test.Padding");
    byte[] payload = new byte[] {9, 8, 7};
    long checksum = CRC64.compute(payload);
    NoritoHeader header =
        new NoritoHeader(
            schemaHash,
            payload.length,
            checksum,
            NoritoCodec.DEFAULT_FLAGS,
            NoritoHeader.COMPRESSION_NONE);
    byte[] headerBytes = header.encode();
    byte[] padding = new byte[16];
    byte[] combined = new byte[headerBytes.length + padding.length + payload.length];
    System.arraycopy(headerBytes, 0, combined, 0, headerBytes.length);
    System.arraycopy(padding, 0, combined, headerBytes.length, padding.length);
    System.arraycopy(payload, 0, combined, headerBytes.length + padding.length, payload.length);
    NoritoHeader.DecodeResult result = NoritoHeader.decode(combined, schemaHash);
    assert Arrays.equals(result.payload(), payload) : "Payload mismatch with padding";
  }

  private static void testEncodeDecodeUInt() {
    TypeAdapter<Long> adapter = NoritoAdapters.uint(64);
    byte[] encoded = NoritoCodec.encode(42L, "iroha.test.Value", adapter);
    long decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.Value");
    assert decoded == 42L;
  }

  private static void testEncodeDecodeString() {
    TypeAdapter<String> adapter = NoritoAdapters.stringAdapter();
    byte[] encoded = NoritoCodec.encode("こんにちは", "iroha.test.Greeting", adapter);
    String decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.Greeting");
    assert decoded.equals("こんにちは");
  }

  private static void testEncodeDecodeSequence() {
    TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(32));
    List<Long> values = List.of(1L, 2L, 3L, 4L);
    byte[] encoded = NoritoCodec.encode(values, "iroha.test.Sequence", adapter);
    List<Long> decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.Sequence");
    assert decoded.equals(values);
  }

  private static void testSequenceAcceptsEmptyPackedTail() {
    byte[] len = Varint.encode(0);
    byte[] tail = new byte[Long.BYTES];
    byte[] payload = new byte[len.length + tail.length];
    System.arraycopy(len, 0, payload, 0, len.length);
    System.arraycopy(tail, 0, payload, len.length, tail.length);

    int flags =
        NoritoHeader.PACKED_SEQ
            | NoritoHeader.COMPACT_LEN
            | NoritoHeader.VARINT_OFFSETS
            | NoritoHeader.COMPACT_SEQ_LEN;
    NoritoDecoder decoder = new NoritoDecoder(payload, flags, NoritoHeader.MINOR_VERSION);
    TypeAdapter<List<byte[]>> adapter = NoritoAdapters.sequence(NoritoAdapters.bytesAdapter());
    List<byte[]> decoded = adapter.decode(decoder);
    assert decoded.isEmpty() : "Expected empty sequence";
    assert decoder.remaining() == 0 : "Decoder should consume compat tail";
  }

  private static void testSequenceAcceptsEmptyPackedTailWithFollowingData() {
    byte[] len = Varint.encode(0);
    byte[] tail = new byte[Long.BYTES];
    byte[] trailing = new byte[] {(byte) 0x12, (byte) 0x34, (byte) 0x56};
    byte[] payload = new byte[len.length + tail.length + trailing.length];
    System.arraycopy(len, 0, payload, 0, len.length);
    System.arraycopy(tail, 0, payload, len.length, tail.length);
    System.arraycopy(trailing, 0, payload, len.length + tail.length, trailing.length);

    int flags =
        NoritoHeader.PACKED_SEQ
            | NoritoHeader.COMPACT_LEN
            | NoritoHeader.VARINT_OFFSETS
            | NoritoHeader.COMPACT_SEQ_LEN;
    NoritoDecoder decoder = new NoritoDecoder(payload, flags, NoritoHeader.MINOR_VERSION);
    TypeAdapter<List<byte[]>> adapter = NoritoAdapters.sequence(NoritoAdapters.bytesAdapter());
    List<byte[]> decoded = adapter.decode(decoder);
    assert decoded.isEmpty() : "Expected empty sequence";
    assert decoder.remaining() == trailing.length : "Decoder should leave trailing bytes";
    int first = decoder.readByte();
    int second = decoder.readByte();
    int third = decoder.readByte();
    assert first == 0x12 && second == 0x34 && third == 0x56 : "Trailing bytes mismatch";
    assert decoder.remaining() == 0 : "All trailing bytes should be consumed";
  }

  private static void testOption() {
    TypeAdapter<Optional<String>> adapter = NoritoAdapters.option(NoritoAdapters.stringAdapter());
    byte[] encodedNone = NoritoCodec.encode(Optional.empty(), "iroha.test.Option", adapter);
    Optional<String> decodedNone = NoritoCodec.decode(encodedNone, adapter, "iroha.test.Option");
    assert decodedNone.isEmpty();
    byte[] encodedSome = NoritoCodec.encode(Optional.of("alice"), "iroha.test.Option", adapter);
    Optional<String> decodedSome = NoritoCodec.decode(encodedSome, adapter, "iroha.test.Option");
    assert decodedSome.isPresent() && decodedSome.get().equals("alice");
  }

  private static void testResult() {
    TypeAdapter<Result<Long, String>> adapter =
        NoritoAdapters.result(NoritoAdapters.uint(8), NoritoAdapters.stringAdapter());
    byte[] encodedOk = NoritoCodec.encode(new Result.Ok<Long, String>(7L), "iroha.test.Result", adapter);
    Result<Long, String> decodedOk = NoritoCodec.decode(encodedOk, adapter, "iroha.test.Result");
    assert decodedOk instanceof Result.Ok<Long, String> ok && ok.value() == 7L;
    byte[] encodedErr =
        NoritoCodec.encode(new Result.Err<Long, String>("bad"), "iroha.test.Result", adapter);
    Result<Long, String> decodedErr = NoritoCodec.decode(encodedErr, adapter, "iroha.test.Result");
    assert decodedErr instanceof Result.Err<Long, String> err && err.error().equals("bad");
  }

  private static void testTryDecode() {
    TypeAdapter<Long> adapter = NoritoAdapters.uint(32);
    byte[] encoded = NoritoCodec.encode(321L, "iroha.test.TryDecode", adapter);
    Result<Long, RuntimeException> ok =
        NoritoCodec.tryDecode(encoded, adapter, "iroha.test.TryDecode");
    if (!(ok instanceof Result.Ok<?, ?> okResult)) {
      throw new AssertionError("tryDecode should return Ok for valid payloads");
    }
    long decodedValue = ((Number) okResult.value()).longValue();
    assert decodedValue == 321L : "Expected decoded value 321";

    byte[] corrupted = Arrays.copyOf(encoded, encoded.length);
    corrupted[corrupted.length - 1] ^= 0x01;
    Result<Long, RuntimeException> err =
        NoritoCodec.tryDecode(corrupted, adapter, "iroha.test.TryDecode");
    if (!(err instanceof Result.Err<?, ?> errResult)) {
      throw new AssertionError("tryDecode should return Err for corrupted payloads");
    }
    assert errResult.error() instanceof RuntimeException
        : "Expected RuntimeException for corrupted payload decode";
  }

  private static void testTryDecodeSurfacesStructErrors() {
    TypeAdapter<Object> adapter =
        NoritoAdapters.struct(
            List.of(
                NoritoAdapters.field("flag", NoritoAdapters.uint(8)),
                NoritoAdapters.field("name", NoritoAdapters.stringAdapter())));
    Map<String, Object> value = Map.of("flag", 7L, "name", "bob");
    String schema = "iroha.test.StructErr";
    byte[] encoded = NoritoCodec.encode(value, schema, adapter);
    byte[] schemaHash = SchemaHash.hash16(schema);
    NoritoHeader.DecodeResult decoded = NoritoHeader.decode(encoded, schemaHash);
    byte[] payload = decoded.payload();
    byte[] tampered = Arrays.copyOf(payload, payload.length);
    if (tampered.length <= 2) {
      throw new AssertionError("Struct payload should contain string bytes");
    }
    tampered[1] = (byte) (((tampered[1] & 0xFF) + 4) & 0xFF);
    long checksum = CRC64.compute(tampered);
    NoritoHeader header = decoded.header();
    NoritoHeader mutatedHeader =
        new NoritoHeader(
            header.schemaHash(), header.payloadLength(), checksum, header.flags(), header.compression());
    byte[] mutatedHeaderBytes = mutatedHeader.encode();
    byte[] corrupted = new byte[mutatedHeaderBytes.length + tampered.length];
    System.arraycopy(mutatedHeaderBytes, 0, corrupted, 0, mutatedHeaderBytes.length);
    System.arraycopy(tampered, 0, corrupted, mutatedHeaderBytes.length, tampered.length);
    Result<Object, RuntimeException> outcome = NoritoCodec.tryDecode(corrupted, adapter, schema);
    if (!(outcome instanceof Result.Err<?, ?> err)) {
      throw new AssertionError("tryDecode should return Err for truncated struct payloads");
    }
    Object error = err.error();
    if (!(error instanceof IllegalArgumentException)) {
      throw new AssertionError("Unexpected error type from tryDecode: " + error.getClass().getName());
    }
  }

  private static void testArchiveViewFlagsHint() {
    TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(32));
    List<Long> value = List.of(10L, 20L, 30L);
    String schema = "iroha.test.View";
    byte[] encoded = NoritoCodec.encode(value, schema, adapter);
    NoritoCodec.ArchiveView view = NoritoCodec.fromBytesView(encoded, schema);
    assert view.flags() == NoritoCodec.DEFAULT_FLAGS : "Archive flags mismatch";
    assert view.flagsHint() == NoritoHeader.MINOR_VERSION : "Archive flags hint mismatch";
    byte[] expectedPayload =
        Arrays.copyOfRange(encoded, NoritoHeader.HEADER_LENGTH, encoded.length);
    assert Arrays.equals(expectedPayload, view.asBytes()) : "Archive payload slice mismatch";
    List<Long> decoded = view.decode(adapter);
    assert decoded.equals(value) : "Archive view decode mismatch";
  }

  private static void testChecksumMismatch() {
    TypeAdapter<Long> adapter = NoritoAdapters.uint(16);
    byte[] encoded = NoritoCodec.encode(5L, "iroha.test.Checksum", adapter);
    encoded[encoded.length - 1] ^= 0x01;
    boolean failed = false;
    try {
      NoritoCodec.decode(encoded, adapter, "iroha.test.Checksum");
    } catch (IllegalArgumentException ex) {
      failed = true;
    }
    assert failed : "Expected checksum mismatch";
  }

  private static void testStructAdapter() {
    List<NoritoAdapters.StructField<?>> fields = List.of(
        NoritoAdapters.field("id", NoritoAdapters.sint(32)),
        NoritoAdapters.field("name", NoritoAdapters.stringAdapter()),
        NoritoAdapters.field("values", NoritoAdapters.sequence(NoritoAdapters.uint(8))));

    NoritoAdapters.StructAdapter structAdapter =
        NoritoAdapters.struct(fields, map -> new Demo(
            ((Long) map.get("id")),
            (String) map.get("name"),
            coerceLongList(map.get("values"))));

    Demo demo = new Demo(-7L, "alice", List.of(1L, 2L, 3L));
    byte[] encoded = NoritoCodec.encode(demo, "iroha.test.Struct", structAdapter);
    Demo decoded = (Demo) NoritoCodec.decode(encoded, structAdapter, "iroha.test.Struct");
    assert decoded.equals(demo);
  }

  private static void testMapAdapter() {
    TypeAdapter<Map<String, String>> adapter =
        NoritoAdapters.map(NoritoAdapters.stringAdapter(), NoritoAdapters.stringAdapter());
    Map<String, String> value = new LinkedHashMap<>();
    value.put("alpha", "1");
    value.put("beta", "2");
    byte[] encoded = NoritoCodec.encode(value, "iroha.test.Map", adapter);
    Map<String, String> decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.Map");
    assert decoded.equals(value) : "Map adapter roundtrip mismatch";
  }

  private static void testMapAdapterSortsKeys() {
    TypeAdapter<Map<Long, Long>> adapter =
        NoritoAdapters.map(NoritoAdapters.uint(8), NoritoAdapters.uint(8));
    Map<Long, Long> value = new java.util.HashMap<>();
    value.put(3L, 4L);
    value.put(1L, 2L);
    byte[] encoded = NoritoCodec.encode(value, "iroha.test.MapOrder", adapter);
    Map<Long, Long> decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.MapOrder");
    List<Long> keys = new ArrayList<>(decoded.keySet());
    assert keys.equals(List.of(1L, 3L)) : "Map keys must encode in sorted order";
  }

  private static void testPackedMapLayout() {
    TypeAdapter<Map<Long, Long>> adapter =
        NoritoAdapters.map(NoritoAdapters.uint(8), NoritoAdapters.uint(8));
    Map<Long, Long> value = new java.util.HashMap<>();
    value.put(3L, 4L);
    value.put(1L, 2L);
    int flags =
        NoritoHeader.PACKED_SEQ
            | NoritoHeader.VARINT_OFFSETS
            | NoritoHeader.COMPACT_SEQ_LEN;
    String schema = "iroha.test.PackedMap";
    byte[] encoded = NoritoCodec.encode(value, schema, adapter, flags);
    NoritoHeader.DecodeResult result =
        NoritoHeader.decode(encoded, SchemaHash.hash16(schema));
    byte[] expected =
        new byte[] {
            (byte) 0x02,
            (byte) 0x01,
            (byte) 0x01,
            (byte) 0x01,
            (byte) 0x01,
            (byte) 0x01,
            (byte) 0x03,
            (byte) 0x02,
            (byte) 0x04
        };
    assert Arrays.equals(result.payload(), expected) : "Packed map payload mismatch";

    Map<Long, Long> decoded = NoritoCodec.decode(encoded, adapter, schema);
    assert decoded.equals(Map.of(1L, 2L, 3L, 4L)) : "Packed map roundtrip mismatch";
    List<Long> keys = new ArrayList<>(decoded.keySet());
    assert keys.equals(List.of(1L, 3L)) : "Packed map keys must be sorted";
  }

  private static void testVarintRejectsOverlongEncodings() {
    assertVarintRejects(new byte[] {(byte) 0x80, (byte) 0x00});
    assertVarintRejects(new byte[] {(byte) 0x81, (byte) 0x00});
    assertVarintRejects(new byte[] {(byte) 0x80, (byte) 0x80, (byte) 0x00});
    assertVarintRejects(
        new byte[] {
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x80,
            (byte) 0x02
        });
  }

  private static void assertVarintRejects(final byte[] input) {
    boolean failed = false;
    try {
      Varint.decode(input, 0);
    } catch (final IllegalArgumentException ex) {
      failed = true;
    }
    assert failed : "Expected varint decode to reject non-canonical encoding";
  }

  private static List<Long> coerceLongList(Object value) {
    if (!(value instanceof List<?> list)) {
      throw new AssertionError("Expected list for struct field values");
    }
    List<Long> result = new ArrayList<>(list.size());
    for (Object entry : list) {
      result.add(((Number) entry).longValue());
    }
    return result;
  }

  private static void testStructuralSchemaHash() {
    Map<String, Object> schema = Map.of(
        "Sample",
            Map.of(
                "Struct",
                    List.of(
                        Map.of("name", "id", "type", "u64"),
                        Map.of("name", "name", "type", "String"),
                        Map.of("name", "flag", "type", "bool"))),
        "String", "String",
        "bool", "bool",
        "u64", Map.of("Int", "FixedWidth"));
    byte[] expected = new byte[] {
        (byte) 0x01, 0x07, (byte) 0xFE, (byte) 0xC5, (byte) 0xFC, 0x24, (byte) 0xAC, 0x0D,
        (byte) 0x01, 0x07, (byte) 0xFE, (byte) 0xC5, (byte) 0xFC, 0x24, (byte) 0xAC, 0x0D
    };
    byte[] actual = SchemaHash.hash16FromStructural(schema);
    assert Arrays.equals(expected, actual) : "Structural schema hash mismatch";
  }

  private static void testCompression() {
    TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(32));
    List<Long> values = List.of(1L, 2L, 3L, 4L, 5L);
    if (NoritoCompression.hasZstd()) {
      NoritoCodec.AdaptiveEncoding adaptive = NoritoCodec.encodeAdaptive(values, adapter);
      int rawLen = adaptive.payload().length;
      byte[] encoded =
          NoritoCodec.encode(
              values,
              "iroha.test.Compression",
              adapter,
              NoritoCodec.DEFAULT_FLAGS,
              NoritoCodec.CompressionConfig.zstd(3));
      List<Long> decoded =
          NoritoCodec.decode(encoded, adapter, "iroha.test.Compression");
      assert decoded.equals(values);

      NoritoCodec.CompressionConfig profileConfig =
          NoritoCodec.CompressionConfig.zstdProfile(
              NoritoCodec.CompressionConfig.CompressionProfile.BALANCED, rawLen);
      byte[] encodedProfile =
          NoritoCodec.encode(
              values,
              "iroha.test.Compression",
              adapter,
              NoritoCodec.DEFAULT_FLAGS,
              profileConfig);
      List<Long> decodedProfile =
          NoritoCodec.decode(encodedProfile, adapter, "iroha.test.Compression");
      assert decodedProfile.equals(values);
    } else {
      boolean failed = false;
      try {
        NoritoCodec.encode(
            values,
            "iroha.test.Compression",
            adapter,
            NoritoCodec.DEFAULT_FLAGS,
            NoritoCodec.CompressionConfig.zstd(3));
      } catch (UnsupportedOperationException ex) {
        failed = true;
      }
      assert failed : "Expected compression request to fail without backend";
    }
  }

  private static void testColumnarHelpers() {
    List<NoritoColumnar.StrBoolRow> rows = List.of(
        new NoritoColumnar.StrBoolRow(1L, "alice", true),
        new NoritoColumnar.StrBoolRow(2L, "bob", false),
        new NoritoColumnar.StrBoolRow(3L, "charlie", true));
    byte[] columnar = NoritoColumnar.encodeNcbU64StrBool(rows);
    byte[] adaptive = NoritoColumnar.encodeRowsU64StrBoolAdaptive(rows);
    assert toHex(columnar).equals(
            "03000000530000000100000000000000020200000000000005000000080000000f000000"
                + "616c696365626f62636861726c696505")
        : "Columnar encoding mismatch";
    assert toHex(adaptive).equals(
            "000301010000000000000005616c69636501020000000000000003626f62000300000000000000"
                + "07636861726c696501")
        : "Adaptive encoding mismatch";
    List<NoritoColumnar.StrBoolRow> decoded = NoritoColumnar.decodeRowsU64StrBoolAdaptive(adaptive);
    assert decoded.equals(rows);

    List<NoritoColumnar.StrBoolRow> single = List.of(new NoritoColumnar.StrBoolRow(1L, "a", true));
    byte[] adaptiveSmall = NoritoColumnar.encodeRowsU64StrBoolAdaptive(single);
    assert toHex(adaptiveSmall).equals("0001010100000000000000016101");
    List<NoritoColumnar.StrBoolRow> decodedSmall = NoritoColumnar.decodeRowsU64StrBoolAdaptive(adaptiveSmall);
    assert decodedSmall.equals(single);

    List<NoritoColumnar.BytesRow> bytesRows = List.of(
        new NoritoColumnar.BytesRow(10L, fill(0x10, 3)),
        new NoritoColumnar.BytesRow(11L, fill(0x20, 0)),
        new NoritoColumnar.BytesRow(12L, fill(0x30, 5)));
    byte[] bytesColumnar = NoritoColumnar.encodeNcbU64Bytes(bytesRows);
    List<NoritoColumnar.BytesRow> bytesDecodedColumnar = NoritoColumnar.decodeNcbU64Bytes(bytesColumnar);
    assert bytesDecodedColumnar.equals(bytesRows);
    byte[] bytesAdaptive = NoritoColumnar.encodeRowsU64BytesAdaptive(bytesRows);
    List<NoritoColumnar.BytesRow> bytesDecodedAdaptive = NoritoColumnar.decodeRowsU64BytesAdaptive(bytesAdaptive);
    assert bytesDecodedAdaptive.equals(bytesRows);
    byte[] bytesAos = NoritoAoS.encodeU64Bytes(bytesRows);
    List<NoritoColumnar.BytesRow> bytesDecodedAos = NoritoAoS.decodeU64Bytes(bytesAos);
    assert bytesDecodedAos.equals(bytesRows);

    List<NoritoColumnar.BytesOptionalRow> optionalRows = List.of(
        new NoritoColumnar.BytesOptionalRow(20L, fill(0x40, 4)),
        new NoritoColumnar.BytesOptionalRow(21L, null),
        new NoritoColumnar.BytesOptionalRow(22L, new byte[0]),
        new NoritoColumnar.BytesOptionalRow(23L, fill(0x50, 2)));
    byte[] optionalColumnar = NoritoColumnar.encodeNcbU64OptionalBytes(optionalRows);
    List<NoritoColumnar.BytesOptionalRow> optionalDecodedColumnar =
        NoritoColumnar.decodeNcbU64OptionalBytes(optionalColumnar);
    assert optionalDecodedColumnar.equals(optionalRows);
    byte[] optionalAos = NoritoAoS.encodeU64OptionalBytes(optionalRows);
    List<NoritoColumnar.BytesOptionalRow> optionalDecodedAos =
        NoritoAoS.decodeU64OptionalBytes(optionalAos);
    assert optionalDecodedAos.equals(optionalRows);
  }

  private static void testDecodeRequiresCompactLenFlag() {
    TypeAdapter<String> adapter = NoritoAdapters.stringAdapter();
    byte[] encoded = NoritoCodec.encode("abc", "iroha.test.FixedStr", adapter, 0);
    String decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.FixedStr");
    assert decoded.equals("abc");

    byte[] varint = Varint.encode(3);
    byte[] payload = new byte[varint.length + 3];
    System.arraycopy(varint, 0, payload, 0, varint.length);
    payload[varint.length] = 'a';
    payload[varint.length + 1] = 'b';
    payload[varint.length + 2] = 'c';
    boolean failed = false;
    try {
      adapter.decode(new NoritoDecoder(payload, 0));
    } catch (IllegalArgumentException ex) {
      failed = true;
    }
    assert failed : "Expected compact-len decode to fail without flag";
  }

  private static void testSequenceLenRespectsExplicitFlags() {
    TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(8));
    List<Long> values = List.of(1L, 2L, 3L, 4L);
    byte[] encoded = NoritoCodec.encode(values, "iroha.test.SeqFixed", adapter, 0);
    List<Long> decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.SeqFixed");
    assert decoded.equals(values);

    byte[] varintLen = Varint.encode(3);
    byte[] payload = new byte[varintLen.length + 3];
    System.arraycopy(varintLen, 0, payload, 0, varintLen.length);
    payload[varintLen.length] = 7;
    payload[varintLen.length + 1] = 8;
    payload[varintLen.length + 2] = 9;
    boolean failed = false;
    try {
      adapter.decode(new NoritoDecoder(payload, 0));
    } catch (IllegalArgumentException ex) {
      failed = true;
    }
    assert failed : "Expected sequence varint decode to require COMPACT_SEQ_LEN flag";
  }

  private static void testEncodeWithHeaderFlagsAdaptiveBits() {
    TypeAdapter<List<String>> adapter = NoritoAdapters.sequence(NoritoAdapters.stringAdapter());

    NoritoCodec.AdaptiveEncoding empty = NoritoCodec.encodeWithHeaderFlags(List.of(), adapter);
    assert empty.flags() == NoritoCodec.DEFAULT_FLAGS : "Empty adaptive flags should match defaults";
    List<String> decodedEmpty = NoritoCodec.decodeAdaptive(empty.payload(), adapter);
    assert decodedEmpty.isEmpty();

    List<String> names = List.of("alice", "bob");
    NoritoCodec.AdaptiveEncoding encoding = NoritoCodec.encodeWithHeaderFlags(names, adapter);
    assert encoding.flags() == NoritoCodec.DEFAULT_FLAGS : "Adaptive flags must remain default";
    List<String> decoded = NoritoCodec.decodeAdaptive(encoding.payload(), adapter);
    assert decoded.equals(names);
  }

  private static void testDecodeAdaptiveRespectsGuard() {
    TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(16));
    List<Long> values = List.of(1L, 2L, 3L);
    NoritoCodec.AdaptiveEncoding encoding = NoritoCodec.encodeWithHeaderFlags(values, adapter);
    try (NoritoCodec.DecodeFlagsGuard guard = NoritoCodec.DecodeFlagsGuard.enter(encoding.flags())) {
      List<Long> decoded = NoritoCodec.decodeAdaptive(encoding.payload(), adapter);
      assert decoded.equals(values);
    }
    NoritoCodec.resetDecodeState();
  }

  private static void testDecodeAdaptiveInstallsRootPayload() {
    TypeAdapter<Long> adapter = new RootAwareAdapter();
    NoritoCodec.AdaptiveEncoding encoding =
        NoritoCodec.encodeWithHeaderFlags(99L, NoritoAdapters.uint(32));
    try (NoritoCodec.DecodeFlagsGuard guard =
        NoritoCodec.DecodeFlagsGuard.enterWithHint(encoding.flags(), encoding.flags())) {
      long decoded = NoritoCodec.decodeAdaptive(encoding.payload(), adapter);
      assert decoded == 99L : "Root-aware decode mismatch";
    }
    assert NoritoCodec.payloadRootBytes() == null : "Root payload should be cleared";
    NoritoCodec.resetDecodeState();
  }

  private static void testEffectiveDecodeFlagsDefaultState() {
    NoritoCodec.resetDecodeState();
    assert NoritoCodec.effectiveDecodeFlags() == null : "Expected no effective decode flags";
  }

  private static void testEffectiveDecodeFlagsGuardHint() {
    NoritoCodec.resetDecodeState();
    try (NoritoCodec.DecodeFlagsGuard guard =
        NoritoCodec.DecodeFlagsGuard.enterWithHint(0x12, 0x34)) {
      Integer effective = NoritoCodec.effectiveDecodeFlags();
      assert effective != null;
      assert effective == 0x12 : "Guard effective flags mismatch";
    }
    assert NoritoCodec.effectiveDecodeFlags() == null : "Effective flags should clear after guard";
  }

  private static void testEffectiveDecodeFlagsHeaderContext() {
    FlagAwareAdapter adapter = new FlagAwareAdapter();
    byte[] encoded =
        NoritoCodec.encode(21L, "iroha.test.FlagAware", adapter, NoritoHeader.VARINT_OFFSETS);
    long decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.FlagAware");
    assert decoded == 21L;
    Integer observed = adapter.observedFlags();
    int expected = NoritoHeader.VARINT_OFFSETS;
    assert observed != null && observed == expected : "Header effective flags mismatch";
    NoritoCodec.resetDecodeState();
  }

  private static void testHeaderFlagsOverrideDecodeGuard() {
    FlagAwareAdapter adapter = new FlagAwareAdapter();
    byte[] encoded =
        NoritoCodec.encode(17L, "iroha.test.FlagAware", adapter, NoritoHeader.VARINT_OFFSETS);
    try (NoritoCodec.DecodeFlagsGuard guard =
        NoritoCodec.DecodeFlagsGuard.enter(
            NoritoHeader.PACKED_SEQ | NoritoHeader.COMPACT_LEN | NoritoHeader.COMPACT_SEQ_LEN)) {
      long decoded = NoritoCodec.decode(encoded, adapter, "iroha.test.FlagAware");
      assert decoded == 17L;
    }
    Integer observed = adapter.observedFlags();
    int expected = NoritoHeader.VARINT_OFFSETS;
    assert observed != null && observed == expected : "Header flags should override guard";
    NoritoCodec.resetDecodeState();
  }

  private static void testEffectiveDecodeFlagsAdaptiveContext() {
    FlagAwareAdapter adapter = new FlagAwareAdapter();
    NoritoCodec.AdaptiveEncoding encoding =
        NoritoCodec.encodeAdaptive(42L, NoritoAdapters.uint(32));
    long decoded = NoritoCodec.decodeAdaptive(encoding.payload(), adapter);
    assert decoded == 42L;
    Integer observed = adapter.observedFlags();
    int expected = encoding.flags();
    assert observed != null && observed == expected : "Adaptive effective flags mismatch";
    NoritoCodec.resetDecodeState();
  }

  private static void testPublicKeyCanonicalArchive() {
    String hex =
        "4e5254300000308ea40f1c2e0d24308ea40f1c2e0d2400470000000000000096a36ea041b651ec"
            + "374665643031323045444636443742353243373033324430334145433639364632303638424435"
            + "333130313532384633433742363038314246463035413136363244374643323435";
    byte[] archive = fromHex(hex);
    byte[] schemaHash = SchemaHash.hash16("iroha_crypto::PublicKey");
    NoritoHeader.DecodeResult result = NoritoHeader.decode(archive, schemaHash);
    NoritoHeader header = result.header();
    byte[] payload = result.payload();
    assert header.flags() == 0x37 : "Unexpected layout flags";
    assert header.payloadLength() == payload.length : "Payload length mismatch";
    assert header.checksum() == 0xEC51B641A06EA396L : "Checksum mismatch";
    NoritoDecoder decoder = new NoritoDecoder(payload, header.flags(), NoritoHeader.MINOR_VERSION);
    TypeAdapter<String> adapter = NoritoAdapters.stringAdapter();
    String value = adapter.decode(decoder);
    assert decoder.remaining() == 0 : "Trailing bytes after decode";
    assert value.equals("ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245");
  }

  private static void testEncryptionSuiteAdapterRoundtrip() {
    TypeAdapter<NoritoStreaming.EncryptionSuite> adapter = NoritoStreaming.encryptionSuiteAdapter();
    NoritoStreaming.EncryptionSuite suite =
        new NoritoStreaming.X25519ChaCha20Poly1305Suite(fill(0xAB, NoritoStreaming.HASH_LEN));
    byte[] encoded = NoritoCodec.encode(suite, "iroha.test.EncryptionSuite", adapter);
    NoritoStreaming.EncryptionSuite decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.EncryptionSuite");
    assert decoded.equals(suite);

    NoritoStreaming.EncryptionSuite kyber =
        new NoritoStreaming.Kyber768XChaCha20Poly1305Suite(fill(0x01, NoritoStreaming.HASH_LEN));
    byte[] encodedKyber = NoritoCodec.encode(kyber, "iroha.test.EncryptionSuite", adapter);
    NoritoStreaming.EncryptionSuite decodedKyber =
        NoritoCodec.decode(encodedKyber, adapter, "iroha.test.EncryptionSuite");
    assert decodedKyber.equals(kyber);
  }

  private static void testTelemetryEventAdapterRoundtrip() {
    TypeAdapter<NoritoStreaming.TelemetryEvent> adapter = NoritoStreaming.telemetryEventAdapter();
    NoritoStreaming.TelemetryEncodeEvent event =
        new NoritoStreaming.TelemetryEncodeEvent(new NoritoStreaming.TelemetryEncodeStats(42L, 17L, 3L, 4L, 9L));
    byte[] encoded = NoritoCodec.encode(event, "iroha.test.Telemetry", adapter);
    NoritoStreaming.TelemetryEvent decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.Telemetry");
    assert decoded.equals(event);

    NoritoStreaming.TelemetrySecurityEvent securityEvent =
        new NoritoStreaming.TelemetrySecurityEvent(
            new NoritoStreaming.TelemetrySecurityStats(
                new NoritoStreaming.Kyber768XChaCha20Poly1305Suite(
                    fill(0xEE, NoritoStreaming.HASH_LEN)),
                5L,
                7L,
                Optional.of(11L),
                Optional.of(22L)));
    byte[] encodedSecurity = NoritoCodec.encode(securityEvent, "iroha.test.Telemetry", adapter);
    NoritoStreaming.TelemetryEvent decodedSecurity =
        NoritoCodec.decode(encodedSecurity, adapter, "iroha.test.Telemetry");
    assert decodedSecurity.equals(securityEvent);

    NoritoStreaming.TelemetryAuditOutcomeEvent auditEvent =
        new NoritoStreaming.TelemetryAuditOutcomeEvent(
            new NoritoStreaming.TelemetryAuditOutcome(
                "TRACE-AUDIT",
                128L,
                "qa-veracity",
                "pass",
                Optional.of("https://example.com/mitigation")));
    byte[] encodedAudit = NoritoCodec.encode(auditEvent, "iroha.test.Telemetry", adapter);
    NoritoStreaming.TelemetryEvent decodedAudit =
        NoritoCodec.decode(encodedAudit, adapter, "iroha.test.Telemetry");
    assert decodedAudit.equals(auditEvent);
  }

  private static void testControlFrameManifestRoundtrip() {
    NoritoStreaming.ChunkDescriptor descriptor =
        new NoritoStreaming.ChunkDescriptor(
            0L, 0L, 128L, fill(0x40, NoritoStreaming.HASH_LEN), false);
    NoritoStreaming.ManifestV1 manifest =
        new NoritoStreaming.ManifestV1(
            fill(0x10, NoritoStreaming.HASH_LEN),
            1L,
            7L,
            123456L,
            new NoritoStreaming.ProfileId(0),
            "nsc://example",
            fill(0x20, NoritoStreaming.HASH_LEN),
            9L,
            fill(0x30, NoritoStreaming.HASH_LEN),
            List.of(descriptor),
            fill(0x50, NoritoStreaming.HASH_LEN),
            new NoritoStreaming.X25519ChaCha20Poly1305Suite(
                fill(0x60, NoritoStreaming.HASH_LEN)),
            NoritoStreaming.FecScheme.RS12_10,
            List.of(),
            Optional.empty(),
            new NoritoStreaming.StreamMetadata("demo", Optional.empty(), Optional.empty(), List.of("test")),
            new NoritoStreaming.CapabilityFlags(0),
            fill(0x70, NoritoStreaming.SIGNATURE_LEN));
    NoritoStreaming.ControlFrame frame =
        new NoritoStreaming.ControlFrame(
            NoritoStreaming.ControlFrameVariant.MANIFEST_ANNOUNCE,
            new NoritoStreaming.ManifestAnnounceFrame(manifest));
    TypeAdapter<NoritoStreaming.ControlFrame> adapter = NoritoStreaming.controlFrameAdapter();
    byte[] encoded = NoritoCodec.encode(frame, "iroha.test.ControlFrame", adapter);
    NoritoStreaming.ControlFrame decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.ControlFrame");
    assert decoded.equals(frame);
  }

  private static void testControlFrameTransportCapsRoundtrip() {
    NoritoStreaming.TransportCapabilities capabilities =
        new NoritoStreaming.TransportCapabilities(
            new NoritoStreaming.HpkeSuiteMask(1 << NoritoStreaming.HpkeSuite.KYBER768_AUTH_PSK.bit()),
            true,
            1400L,
            200L,
            NoritoStreaming.PrivacyBucketGranularity.STANDARD_V1);
    NoritoStreaming.ControlFrame frame =
        new NoritoStreaming.ControlFrame(
            NoritoStreaming.ControlFrameVariant.TRANSPORT_CAPABILITIES,
            new NoritoStreaming.TransportCapabilitiesFrame(
                NoritoStreaming.CapabilityRole.PUBLISHER, capabilities));
    TypeAdapter<NoritoStreaming.ControlFrame> adapter = NoritoStreaming.controlFrameAdapter();
    byte[] encoded = NoritoCodec.encode(frame, "iroha.test.ControlFrame", adapter);
    NoritoStreaming.ControlFrame decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.ControlFrame");
    assert decoded.equals(frame);
  }

  private static void testControlFrameErrorRoundtrip() {
    NoritoStreaming.ControlFrame frame =
        new NoritoStreaming.ControlFrame(
            NoritoStreaming.ControlFrameVariant.ERROR,
            new NoritoStreaming.ControlErrorFrame(
                NoritoStreaming.ErrorCode.PROTOCOL_VIOLATION, "oops"));
    TypeAdapter<NoritoStreaming.ControlFrame> adapter = NoritoStreaming.controlFrameAdapter();
    byte[] encoded = NoritoCodec.encode(frame, "iroha.test.ControlFrame", adapter);
    NoritoStreaming.ControlFrame decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.ControlFrame");
    assert decoded.equals(frame);
  }

  private static void testStreamingTicketRoundtrip() {
    NoritoStreaming.TicketPolicy policy =
        new NoritoStreaming.TicketPolicy(
            2, List.of("us-east", "eu-central"), Optional.of(15_000L));
    NoritoStreaming.TicketCapabilities capabilities =
        new NoritoStreaming.TicketCapabilities(
            NoritoStreaming.TicketCapabilities.LIVE | NoritoStreaming.TicketCapabilities.HDR);
    NoritoStreaming.StreamingTicket ticket =
        new NoritoStreaming.StreamingTicket(
            fill(0x33, NoritoStreaming.HASH_LEN),
            "alice@wonderland",
            42,
            3,
            7,
            100,
            200,
            BigInteger.valueOf(1_000_000L),
            64,
            5,
            fill(0x34, NoritoStreaming.HASH_LEN),
            1234,
            fill(0x35, NoritoStreaming.SIGNATURE_LEN),
            fill(0x36, NoritoStreaming.HASH_LEN),
            fill(0x37, NoritoStreaming.HASH_LEN),
            fill(0x38, NoritoStreaming.HASH_LEN),
            1_700_000_000L,
            1_800_000_000L,
            Optional.of(policy),
            capabilities);
    TypeAdapter<NoritoStreaming.StreamingTicket> adapter = NoritoStreaming.streamingTicketAdapter();
    byte[] encoded = NoritoCodec.encode(ticket, "iroha.test.StreamingTicket", adapter);
    NoritoStreaming.StreamingTicket decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.StreamingTicket");
    assert decoded.equals(ticket);
  }

  private static void testTicketRevocationRoundtrip() {
    NoritoStreaming.TicketRevocation revocation =
        new NoritoStreaming.TicketRevocation(
            fill(0x39, NoritoStreaming.HASH_LEN),
            fill(0x3A, NoritoStreaming.HASH_LEN),
            42,
            fill(0x3B, NoritoStreaming.SIGNATURE_LEN));
    TypeAdapter<NoritoStreaming.TicketRevocation> adapter =
        NoritoStreaming.ticketRevocationAdapter();
    byte[] encoded = NoritoCodec.encode(revocation, "iroha.test.TicketRevocation", adapter);
    NoritoStreaming.TicketRevocation decoded =
        NoritoCodec.decode(encoded, adapter, "iroha.test.TicketRevocation");
    assert decoded.equals(revocation);
  }

  private static final class RootAwareAdapter implements TypeAdapter<Long> {
    private final TypeAdapter<Long> inner = NoritoAdapters.uint(32);

    @Override
    public void encode(NoritoEncoder encoder, Long value) {
      inner.encode(encoder, value);
    }

    @Override
    public Long decode(NoritoDecoder decoder) {
      byte[] root = NoritoCodec.payloadRootBytes();
      assert root != null : "Expected root payload during decode";
      NoritoDecoder rootDecoder =
          new NoritoDecoder(root, decoder.flags(), decoder.flagsHint());
      long rootValue = inner.decode(rootDecoder);
      long actual = inner.decode(decoder);
      assert rootValue == actual : "Root payload mismatch";
      return actual;
    }

    @Override
    public int fixedSize() {
      return inner.fixedSize();
    }

    @Override
    public boolean isSelfDelimiting() {
      return inner.isSelfDelimiting();
    }
  }

  private static final class FlagAwareAdapter implements TypeAdapter<Long> {
    private final TypeAdapter<Long> inner = NoritoAdapters.uint(32);
    private Integer observed;

    @Override
    public void encode(NoritoEncoder encoder, Long value) {
      inner.encode(encoder, value);
    }

    @Override
    public Long decode(NoritoDecoder decoder) {
      observed = NoritoCodec.effectiveDecodeFlags();
      return inner.decode(decoder);
    }

    @Override
    public int fixedSize() {
      return inner.fixedSize();
    }

    @Override
    public boolean isSelfDelimiting() {
      return inner.isSelfDelimiting();
    }

    Integer observedFlags() {
      return observed;
    }
  }

  private static String toHex(byte[] data) {
    StringBuilder sb = new StringBuilder(data.length * 2);
    for (byte b : data) {
      sb.append(String.format("%02x", b & 0xFF));
    }
    return sb.toString();
  }

  private static byte[] fromHex(String hex) {
    int len = hex.length();
    if ((len & 1) != 0) {
      throw new IllegalArgumentException("Hex string must have even length");
    }
    byte[] out = new byte[len / 2];
    for (int i = 0; i < len; i += 2) {
      out[i / 2] = (byte) Integer.parseInt(hex.substring(i, i + 2), 16);
    }
    return out;
  }

  private record Demo(long id, String name, List<Long> values) {}
}
