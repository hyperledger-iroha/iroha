// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public final class NoritoColumnar {
  public static final int DESC_U64_STR_BOOL = 0x13;
  public static final int DESC_U64_BYTES = 0x21;
  private static final int DESC_U64_DELTA_STR_BOOL = 0x53;
  private static final int DESC_U64_DICT_STR_BOOL = 0x93;
  private static final int DESC_U64_OPTIONAL_BYTES = 0x61;

  private static final int ADAPTIVE_TAG_AOS = 0x00;
  private static final int ADAPTIVE_TAG_NCB = 0x01;

  private static final int AOS_NCB_SMALL_N = 64;
  private static final int COMBO_NO_DELTA_SMALL_N_IF_EMPTY = 2;
  private static final int COMBO_ID_DELTA_MIN_ROWS = 2;
  private static final boolean COMBO_ENABLE_ID_DELTA = true;
  private static final boolean COMBO_ENABLE_NAME_DICT = true;
  private static final double COMBO_DICT_RATIO_MAX = 0.40;
  private static final double COMBO_DICT_AVG_LEN_MIN = 8.0;

  private NoritoColumnar() {}

  public static byte[] encodeNcbU64StrBool(List<StrBoolRow> rows) {
    DictResult dict = buildDict(rows);
    if (dict.useDict()) {
      return encodeNcbDict(rows, dict);
    }
    if (shouldUseIdDelta(rows)) {
      return encodeNcbDelta(rows);
    }
    return encodeNcbOffsets(rows);
  }

  public static byte[] encodeRowsU64StrBoolAdaptive(List<StrBoolRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64StrBool(rows);
      byte[] ncb = encodeNcbU64StrBool(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    // Columnar auto-selection currently disabled in Rust (always AoS)
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64StrBool(rows));
  }

  public static List<StrBoolRow> decodeRowsU64StrBoolAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64StrBool(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64StrBool(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
  }

  public static List<StrBoolRow> decodeNcbU64StrBool(byte[] data) {
    int offset = 0;
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB payload too short");
    }
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_STR_BOOL && desc != DESC_U64_DELTA_STR_BOOL && desc != DESC_U64_DICT_STR_BOOL) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    List<Long> ids = new ArrayList<>(n);
    offset = align(offset, 8);
    if (desc == DESC_U64_DELTA_STR_BOOL) {
      long base = readU64(data, offset);
      offset += 8;
      ids.add(base);
      while (ids.size() < n) {
        Varint.DecodeResult res = Varint.decode(data, offset);
        offset = res.nextOffset();
        long delta = zigzagDecode(res.value());
        ids.add(ids.get(ids.size() - 1) + delta);
      }
    } else {
      for (int i = 0; i < n; i++) {
        ids.add(readU64(data, offset));
        offset += 8;
      }
    }
    offset = align(offset, 4);
    List<String> names = new ArrayList<>(n);
    if (desc == DESC_U64_DICT_STR_BOOL) {
      int dictLen = readU32(data, offset);
      offset += 4;
      int[] offs = new int[dictLen + 1];
      for (int i = 0; i < dictLen + 1; i++) {
        offs[i] = readU32(data, offset);
        offset += 4;
      }
      int blobLen = offs[dictLen];
      byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
      offset += blobLen;
      String[] dictionary = new String[dictLen];
      for (int i = 0; i < dictLen; i++) {
        dictionary[i] = new String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8);
      }
      offset = align(offset, 4);
      int[] codes = new int[n];
      for (int i = 0; i < n; i++) {
        codes[i] = readU32(data, offset);
        offset += 4;
        names.add(dictionary[codes[i]]);
      }
    } else {
      int[] offs = new int[n + 1];
      for (int i = 0; i < n + 1; i++) {
        offs[i] = readU32(data, offset);
        offset += 4;
      }
      int blobLen = offs[n];
      byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
      offset += blobLen;
      for (int i = 0; i < n; i++) {
        names.add(new String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8));
      }
    }
    int bitBytes = (n + 7) / 8;
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    List<StrBoolRow> rows = new ArrayList<>(n);
    for (int i = 0; i < n; i++) {
      boolean flag = ((flags[i / 8] >> (i % 8)) & 1) != 0;
      rows.add(new StrBoolRow(ids.get(i), names.get(i), flag));
    }
    return rows;
  }

  public record StrBoolRow(long id, String name, boolean flag) {}

  public static final class BytesRow {
    private final long id;
    private final byte[] data;

    public BytesRow(long id, byte[] data) {
      if (data == null) {
        throw new IllegalArgumentException("data must not be null");
      }
      this.id = id;
      this.data = data.clone();
    }

    public long id() {
      return id;
    }

    public byte[] data() {
      return data.clone();
    }

    byte[] dataRaw() {
      return data;
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof BytesRow other)) {
        return false;
      }
      return id == other.id && Arrays.equals(data, other.data);
    }

    @Override
    public int hashCode() {
      return 31 * Long.hashCode(id) + Arrays.hashCode(data);
    }
  }

  public static final class BytesOptionalRow {
    private final long id;
    private final byte[] data;

    public BytesOptionalRow(long id, byte[] data) {
      this.id = id;
      this.data = data != null ? data.clone() : null;
    }

    public long id() {
      return id;
    }

    public boolean isPresent() {
      return data != null;
    }

    public byte[] data() {
      return data != null ? data.clone() : null;
    }

    byte[] dataRaw() {
      return data;
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof BytesOptionalRow other)) {
        return false;
      }
      if (id != other.id) {
        return false;
      }
      return Arrays.equals(data, other.data);
    }

    @Override
    public int hashCode() {
      return 31 * Long.hashCode(id) + Arrays.hashCode(data);
    }
  }

  public static byte[] encodeNcbU64Bytes(List<BytesRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    out.write(DESC_U64_BYTES);
    padTo(out, 8);
    for (BytesRow row : rows) {
      writeU64(out, row.id());
    }
    padTo(out, 4);
    int[] offs = new int[rows.size() + 1];
    int acc = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (int i = 0; i < rows.size(); i++) {
      byte[] value = rows.get(i).dataRaw();
      acc += value.length;
      offs[i + 1] = acc;
      blob.writeBytes(value);
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
    return out.toByteArray();
  }

  public static List<BytesRow> decodeNcbU64Bytes(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_BYTES) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    List<Long> ids = new ArrayList<>(n);
    offset = align(offset, 8);
    for (int i = 0; i < n; i++) {
      ids.add(readU64(data, offset));
      offset += 8;
    }
    offset = align(offset, 4);
    int[] offs = new int[n + 1];
    for (int i = 0; i < n + 1; i++) {
      offs[i] = readU32(data, offset);
      offset += 4;
    }
    int blobLen = offs[n];
    if (offset + blobLen > data.length) {
      throw new IllegalArgumentException("Invalid blob length in columnar payload");
    }
    byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
    offset += blobLen;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after columnar decode");
    }
    List<BytesRow> rows = new ArrayList<>(n);
    for (int i = 0; i < n; i++) {
      int start = offs[i];
      int end = offs[i + 1];
      if (start > end || end > blob.length) {
        throw new IllegalArgumentException("Invalid offset table in columnar payload");
      }
      byte[] value = Arrays.copyOfRange(blob, start, end);
      rows.add(new BytesRow(ids.get(i), value));
    }
    return rows;
  }

  public static byte[] encodeRowsU64BytesAdaptive(List<BytesRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64Bytes(rows);
      byte[] ncb = encodeNcbU64Bytes(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64Bytes(rows));
  }

  public static List<BytesRow> decodeRowsU64BytesAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64Bytes(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64Bytes(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
  }

  public static byte[] encodeNcbU64OptionalBytes(List<BytesOptionalRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    out.write(DESC_U64_OPTIONAL_BYTES);
    padTo(out, 8);
    for (BytesOptionalRow row : rows) {
      writeU64(out, row.id());
    }
    padTo(out, 4);
    int[] offs = new int[rows.size() + 1];
    int acc = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (int i = 0; i < rows.size(); i++) {
      byte[] value = rows.get(i).dataRaw();
      if (value != null) {
        acc += value.length;
        blob.writeBytes(value);
      }
      offs[i + 1] = acc;
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
    byte[] flags = buildPresenceFlags(rows);
    out.writeBytes(flags);
    return out.toByteArray();
  }

  public static List<BytesOptionalRow> decodeNcbU64OptionalBytes(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_OPTIONAL_BYTES) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    List<Long> ids = new ArrayList<>(n);
    offset = align(offset, 8);
    for (int i = 0; i < n; i++) {
      ids.add(readU64(data, offset));
      offset += 8;
    }
    offset = align(offset, 4);
    int[] offs = new int[n + 1];
    for (int i = 0; i < n + 1; i++) {
      offs[i] = readU32(data, offset);
      offset += 4;
    }
    int blobLen = offs[n];
    if (offset + blobLen > data.length) {
      throw new IllegalArgumentException("Invalid blob length in optional columnar payload");
    }
    byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
    offset += blobLen;
    int bitBytes = (n + 7) / 8;
    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("Optional columnar payload missing presence bitmap");
    }
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    offset += bitBytes;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after optional columnar decode");
    }
    List<BytesOptionalRow> rows = new ArrayList<>(n);
    for (int i = 0; i < n; i++) {
      boolean present = ((flags[i / 8] >> (i % 8)) & 1) != 0;
      int start = offs[i];
      int end = offs[i + 1];
      if (start > end || end > blob.length) {
        throw new IllegalArgumentException("Invalid offset table in optional columnar payload");
      }
      if (present) {
        byte[] value = Arrays.copyOfRange(blob, start, end);
        rows.add(new BytesOptionalRow(ids.get(i), value));
      } else {
        if (end != start) {
          throw new IllegalArgumentException("Absent entry must have zero-length slice");
        }
        rows.add(new BytesOptionalRow(ids.get(i), null));
      }
    }
    return rows;
  }

  private static byte[] concat(int tag, byte[] payload) {
    byte[] out = new byte[payload.length + 1];
    out[0] = (byte) tag;
    System.arraycopy(payload, 0, out, 1, payload.length);
    return out;
  }

  private static byte[] encodeNcbOffsets(List<StrBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    out.write(DESC_U64_STR_BOOL);
    padTo(out, 8);
    for (StrBoolRow row : rows) {
      writeU64(out, row.id());
    }
    padTo(out, 4);
    int[] offs = new int[rows.size() + 1];
    int acc = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (int i = 0; i < rows.size(); i++) {
      byte[] encoded = rows.get(i).name().getBytes(StandardCharsets.UTF_8);
      acc += encoded.length;
      offs[i + 1] = acc;
      blob.writeBytes(encoded);
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
    out.writeBytes(buildFlags(rows));
    return out.toByteArray();
  }

  private static byte[] encodeNcbDelta(List<StrBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    out.write(DESC_U64_DELTA_STR_BOOL);
    padTo(out, 8);
    long base = rows.get(0).id();
    writeU64(out, base);
    long prev = base;
    for (int i = 1; i < rows.size(); i++) {
      long delta = rows.get(i).id() - prev;
      out.writeBytes(Varint.encode(zigzagEncode(delta)));
      prev = rows.get(i).id();
    }
    padTo(out, 4);
    int[] offs = new int[rows.size() + 1];
    int acc = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (int i = 0; i < rows.size(); i++) {
      byte[] encoded = rows.get(i).name().getBytes(StandardCharsets.UTF_8);
      acc += encoded.length;
      offs[i + 1] = acc;
      blob.writeBytes(encoded);
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
    out.writeBytes(buildFlags(rows));
    return out.toByteArray();
  }

  private static byte[] encodeNcbDict(List<StrBoolRow> rows, DictResult dict) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    out.write(DESC_U64_DICT_STR_BOOL);
    padTo(out, 8);
    for (StrBoolRow row : rows) {
      writeU64(out, row.id());
    }
    padTo(out, 4);
    writeU32(out, dict.dictionary().size());
    int[] offs = new int[dict.dictionary().size() + 1];
    int acc = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (int i = 0; i < dict.dictionary().size(); i++) {
      byte[] encoded = dict.dictionary().get(i).getBytes(StandardCharsets.UTF_8);
      acc += encoded.length;
      offs[i + 1] = acc;
      blob.writeBytes(encoded);
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
    padTo(out, 4);
    for (StrBoolRow row : rows) {
      writeU32(out, dict.mapping().get(row.name()));
    }
    out.writeBytes(buildFlags(rows));
    return out.toByteArray();
  }

  private static byte[] buildFlags(List<StrBoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).flag()) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static byte[] buildPresenceFlags(List<BytesOptionalRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).isPresent()) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static DictResult buildDict(List<StrBoolRow> rows) {
    if (!COMBO_ENABLE_NAME_DICT || rows.isEmpty()) {
      return DictResult.disabled();
    }
    Map<String, Integer> mapping = new HashMap<>();
    int totalLen = 0;
    for (StrBoolRow row : rows) {
      totalLen += row.name().length();
      mapping.computeIfAbsent(row.name(), k -> mapping.size());
    }
    double ratio = (double) mapping.size() / rows.size();
    double avg = (double) totalLen / rows.size();
    if (ratio <= COMBO_DICT_RATIO_MAX && avg >= COMBO_DICT_AVG_LEN_MIN) {
      List<String> dictionary = new ArrayList<>(mapping.size());
      for (int i = 0; i < mapping.size(); i++) {
        dictionary.add("");
      }
      for (Map.Entry<String, Integer> entry : mapping.entrySet()) {
        dictionary.set(entry.getValue(), entry.getKey());
      }
      return DictResult.enabled(mapping, dictionary);
    }
    return DictResult.disabled();
  }

  private static boolean shouldUseIdDelta(List<StrBoolRow> rows) {
    if (!COMBO_ENABLE_ID_DELTA || rows.size() < COMBO_ID_DELTA_MIN_ROWS) {
      return false;
    }
    if (rows.size() <= COMBO_NO_DELTA_SMALL_N_IF_EMPTY) {
      for (StrBoolRow row : rows) {
        if (row.name().isEmpty()) {
          return false;
        }
      }
    }
    long prev = rows.get(0).id();
    int varintBytes = 0;
    for (int i = 1; i < rows.size(); i++) {
      long delta = rows.get(i).id() - prev;
      long zz = zigzagEncode(delta);
      varintBytes += varintLength(zz);
      if (varintBytes >= 8 * (rows.size() - 1)) {
        return false;
      }
      prev = rows.get(i).id();
    }
    return true;
  }

  private static int varintLength(long value) {
    int length = 1;
    long v = value;
    while (v >= 0x80) {
      v >>>= 7;
      length += 1;
    }
    return length;
  }

  private static long zigzagEncode(long value) {
    return (value << 1) ^ (value >> 63);
  }

  private static long zigzagDecode(long value) {
    return (value >>> 1) ^ -(value & 1L);
  }

  private static void writeU32(ByteArrayOutputStream out, int value) {
    out.write(value & 0xFF);
    out.write((value >>> 8) & 0xFF);
    out.write((value >>> 16) & 0xFF);
    out.write((value >>> 24) & 0xFF);
  }

  private static void writeU64(ByteArrayOutputStream out, long value) {
    out.write((int) (value & 0xFF));
    out.write((int) ((value >>> 8) & 0xFF));
    out.write((int) ((value >>> 16) & 0xFF));
    out.write((int) ((value >>> 24) & 0xFF));
    out.write((int) ((value >>> 32) & 0xFF));
    out.write((int) ((value >>> 40) & 0xFF));
    out.write((int) ((value >>> 48) & 0xFF));
    out.write((int) ((value >>> 56) & 0xFF));
  }

  private static void padTo(ByteArrayOutputStream out, int align) {
    int mis = out.size() % align;
    if (mis != 0) {
      int pad = align - mis;
      out.write(new byte[pad], 0, pad);
    }
  }

  private static int align(int offset, int align) {
    int mis = offset % align;
    return mis == 0 ? offset : offset + (align - mis);
  }

  private static int readU32(byte[] data, int offset) {
    return (data[offset] & 0xFF)
        | ((data[offset + 1] & 0xFF) << 8)
        | ((data[offset + 2] & 0xFF) << 16)
        | ((data[offset + 3] & 0xFF) << 24);
  }

  private static long readU64(byte[] data, int offset) {
    long value = 0;
    for (int i = 0; i < 8; i++) {
      value |= (long) (data[offset + i] & 0xFF) << (8 * i);
    }
    return value;
  }

  private record DictResult(boolean useDict, Map<String, Integer> mapping, List<String> dictionary) {
    static DictResult enabled(Map<String, Integer> mapping, List<String> dictionary) {
      return new DictResult(true, mapping, dictionary);
    }

    static DictResult disabled() {
      return new DictResult(false, Map.of(), List.of());
    }
  }
}
