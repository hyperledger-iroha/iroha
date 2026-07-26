// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

public final class NoritoColumnar {
  public static final int DESC_U64_STR_BOOL = 0x13;
  public static final int DESC_U64_BYTES = 0x21;
  public static final int DESC_U64_BYTES_BOOL = 0x14;
  public static final int DESC_U64_OPTSTR_BOOL = 0x1B;
  public static final int DESC_U64_OPTU32_BOOL = 0x1C;
  private static final int DESC_U64_DELTA_STR_BOOL = 0x53;
  private static final int DESC_U64_DELTA_BYTES_BOOL = 0x54;
  private static final int DESC_U64_DELTA_OPTSTR_BOOL = 0x5B;
  private static final int DESC_U64_DELTA_OPTU32_BOOL = 0x5C;
  private static final int DESC_U64_DICT_STR_BOOL = 0x93;
  private static final int DESC_U64_OPTIONAL_BYTES = 0x71;
  private static final int DESC_U64_ENUM_BOOL = 0x61;
  private static final int DESC_U64_DELTA_ENUM_BOOL = 0x63;
  private static final int DESC_U64_ENUM_BOOL_CODEDELTA = 0x65;
  private static final int DESC_U64_DELTA_ENUM_BOOL_CODEDELTA = 0x67;
  private static final int DESC_U64_ENUM_BOOL_DICT = 0xE1;
  private static final int DESC_U64_DELTA_ENUM_BOOL_DICT = 0xE3;
  private static final int DESC_U64_ENUM_BOOL_DICT_CODEDELTA = 0xE5;
  private static final int DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA = 0xE7;

  private static final int TAG_ENUM_NAME = 0;
  private static final int TAG_ENUM_CODE = 1;

  private static final int ADAPTIVE_TAG_AOS = 0x00;
  private static final int ADAPTIVE_TAG_NCB = 0x01;

  private static final int AOS_NCB_SMALL_N = 64;
  private static final int COMBO_NO_DELTA_SMALL_N_IF_EMPTY = 2;
  private static final int COMBO_ID_DELTA_MIN_ROWS = 2;
  private static final boolean COMBO_ENABLE_ID_DELTA = true;
  private static final boolean COMBO_ENABLE_NAME_DICT = true;
  private static final double COMBO_DICT_RATIO_MAX = 0.40;
  private static final double COMBO_DICT_AVG_LEN_MIN = 8.0;
  private static final long U32_MAX = 0xFFFF_FFFFL;

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

  public sealed interface EnumValue permits EnumName, EnumCode {}

  public record EnumName(String name) implements EnumValue {
    public EnumName {
      Objects.requireNonNull(name, "name must not be null");
    }
  }

  public record EnumCode(long code) implements EnumValue {
    public EnumCode {
      if (code < 0 || code > U32_MAX) {
        throw new IllegalArgumentException("code must fit into u32");
      }
    }
  }

  public record EnumBoolRow(long id, EnumValue value, boolean flag) {
    public EnumBoolRow {
      Objects.requireNonNull(value, "value must not be null");
    }
  }

  public static byte[] encodeNcbU64EnumBool(List<EnumBoolRow> rows) {
    boolean useDeltaIds = shouldUseIdDeltaEnum(rows);
    boolean useNameDict = shouldUseNameDictEnum(rows);
    boolean useCodeDelta = shouldUseCodeDeltaEnum(rows);
    return encodeNcbU64EnumBool(rows, useDeltaIds, useNameDict, useCodeDelta);
  }

  public static byte[] encodeRowsU64EnumBoolAdaptive(List<EnumBoolRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64EnumBool(rows);
      byte[] ncb = encodeNcbU64EnumBool(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    // Columnar auto-selection currently disabled in Rust (always AoS)
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64EnumBool(rows));
  }

  public static List<EnumBoolRow> decodeRowsU64EnumBoolAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64EnumBool(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64EnumBool(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
  }

  public static List<EnumBoolRow> decodeNcbU64EnumBool(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB enum payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    EnumDescriptor descriptor = parseEnumDescriptor(desc);
    List<Long> ids = new ArrayList<>(n);
    offset = align(offset, 8);
    if (descriptor.deltaIds()) {
      if (n > 0) {
        long base = readU64(data, offset);
        offset += 8;
        ids.add(base);
        while (ids.size() < n) {
          Varint.DecodeResult res = Varint.decode(data, offset);
          offset = res.nextOffset();
          long delta = zigzagDecode(res.value());
          ids.add(ids.get(ids.size() - 1) + delta);
        }
      }
    } else {
      for (int i = 0; i < n; i++) {
        ids.add(readU64(data, offset));
        offset += 8;
      }
    }
    if (offset + n > data.length) {
      throw new IllegalArgumentException("NCB enum payload truncated (tags)");
    }
    byte[] tags = Arrays.copyOfRange(data, offset, offset + n);
    offset += n;
    int nameCount = 0;
    for (byte tag : tags) {
      int value = tag & 0xFF;
      if (value == TAG_ENUM_NAME) {
        nameCount += 1;
      } else if (value != TAG_ENUM_CODE) {
        throw new IllegalArgumentException("Invalid enum tag: " + value);
      }
    }
    int codeCount = n - nameCount;
    List<String> names = new ArrayList<>(nameCount);
    if (descriptor.nameDict()) {
      offset = align(offset, 4);
      if (offset + 4 > data.length) {
        throw new IllegalArgumentException("NCB enum payload truncated (dict len)");
      }
      int dictLen = readU32(data, offset);
      offset += 4;
      int[] offs = new int[dictLen + 1];
      for (int i = 0; i < dictLen + 1; i++) {
        if (offset + 4 > data.length) {
          throw new IllegalArgumentException("NCB enum payload truncated (dict offsets)");
        }
        offs[i] = readU32(data, offset);
        offset += 4;
      }
      int blobLen = offs[dictLen];
      if (offset + blobLen > data.length) {
        throw new IllegalArgumentException("NCB enum payload truncated (dict blob)");
      }
      byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
      offset += blobLen;
      String[] dictionary = new String[dictLen];
      for (int i = 0; i < dictLen; i++) {
        dictionary[i] = new String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8);
      }
      offset = align(offset, 4);
      for (int i = 0; i < nameCount; i++) {
        if (offset + 4 > data.length) {
          throw new IllegalArgumentException("NCB enum payload truncated (dict codes)");
        }
        int code = readU32(data, offset);
        offset += 4;
        if (code < 0 || code >= dictLen) {
          throw new IllegalArgumentException("NCB enum payload invalid dict index");
        }
        names.add(dictionary[code]);
      }
    } else {
      offset = align(offset, 4);
      int[] offs = new int[nameCount + 1];
      for (int i = 0; i < nameCount + 1; i++) {
        if (offset + 4 > data.length) {
          throw new IllegalArgumentException("NCB enum payload truncated (name offsets)");
        }
        offs[i] = readU32(data, offset);
        offset += 4;
      }
      int blobLen = offs[nameCount];
      if (offset + blobLen > data.length) {
        throw new IllegalArgumentException("NCB enum payload truncated (name blob)");
      }
      byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
      offset += blobLen;
      for (int i = 0; i < nameCount; i++) {
        names.add(new String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8));
      }
    }
    offset = align(offset, 4);
    List<Long> codes = new ArrayList<>(codeCount);
    if (codeCount > 0) {
      if (descriptor.codeDelta()) {
        if (offset + 4 > data.length) {
          throw new IllegalArgumentException("NCB enum payload truncated (code base)");
        }
        long base = Integer.toUnsignedLong(readU32(data, offset));
        offset += 4;
        codes.add(base);
        long prev = base;
        while (codes.size() < codeCount) {
          Varint.DecodeResult res = Varint.decode(data, offset);
          offset = res.nextOffset();
          long delta = zigzagDecode(res.value());
          long next = (prev + delta) & U32_MAX;
          codes.add(next);
          prev = next;
        }
      } else {
        for (int i = 0; i < codeCount; i++) {
          if (offset + 4 > data.length) {
            throw new IllegalArgumentException("NCB enum payload truncated (codes)");
          }
          codes.add(Integer.toUnsignedLong(readU32(data, offset)));
          offset += 4;
        }
      }
    }
    int bitBytes = (n + 7) / 8;
    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB enum payload truncated (flags)");
    }
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    offset += bitBytes;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after enum decode");
    }
    List<EnumBoolRow> rows = new ArrayList<>(n);
    int nameIndex = 0;
    int codeIndex = 0;
    for (int i = 0; i < n; i++) {
      int tag = tags[i] & 0xFF;
      EnumValue value;
      if (tag == TAG_ENUM_NAME) {
        if (nameIndex >= names.size()) {
          throw new IllegalArgumentException("Enum name column underflow");
        }
        value = new EnumName(names.get(nameIndex++));
      } else if (tag == TAG_ENUM_CODE) {
        if (codeIndex >= codes.size()) {
          throw new IllegalArgumentException("Enum code column underflow");
        }
        value = new EnumCode(codes.get(codeIndex++));
      } else {
        throw new IllegalArgumentException("Invalid enum tag: " + tag);
      }
      boolean flag = ((flags[i / 8] >> (i % 8)) & 1) != 0;
      rows.add(new EnumBoolRow(ids.get(i), value, flag));
    }
    return rows;
  }

  private static byte[] encodeNcbU64EnumBool(
      List<EnumBoolRow> rows, boolean useDeltaIds, boolean useNameDict, boolean useCodeDelta) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, rows.size());
    int desc =
        (useNameDict ? DESC_U64_ENUM_BOOL_DICT : DESC_U64_ENUM_BOOL)
            | (useDeltaIds ? 0x02 : 0)
            | (useCodeDelta ? 0x04 : 0);
    out.write(desc);
    padTo(out, 8);
    if (useDeltaIds && !rows.isEmpty()) {
      long base = rows.get(0).id();
      writeU64(out, base);
      long prev = base;
      for (int i = 1; i < rows.size(); i++) {
        long delta = rows.get(i).id() - prev;
        out.writeBytes(Varint.encode(zigzagEncode(delta)));
        prev = rows.get(i).id();
      }
    } else {
      for (EnumBoolRow row : rows) {
        writeU64(out, row.id());
      }
    }
    byte[] tags = new byte[rows.size()];
    List<String> names = new ArrayList<>();
    List<Long> codes = new ArrayList<>();
    for (int i = 0; i < rows.size(); i++) {
      EnumValue value = rows.get(i).value();
      if (value instanceof EnumName name) {
        tags[i] = (byte) TAG_ENUM_NAME;
        names.add(name.name());
      } else if (value instanceof EnumCode code) {
        tags[i] = (byte) TAG_ENUM_CODE;
        codes.add(code.code());
      } else {
        throw new IllegalArgumentException("Unsupported enum value type: " + value.getClass());
      }
    }
    out.writeBytes(tags);
    if (useNameDict) {
      Map<String, Integer> mapping = new HashMap<>();
      List<String> dictionary = new ArrayList<>();
      for (String name : names) {
        if (!mapping.containsKey(name)) {
          mapping.put(name, dictionary.size());
          dictionary.add(name);
        }
      }
      padTo(out, 4);
      writeU32(out, dictionary.size());
      int[] offs = new int[dictionary.size() + 1];
      int acc = 0;
      ByteArrayOutputStream blob = new ByteArrayOutputStream();
      offs[0] = 0;
      for (int i = 0; i < dictionary.size(); i++) {
        byte[] encoded = dictionary.get(i).getBytes(StandardCharsets.UTF_8);
        acc += encoded.length;
        offs[i + 1] = acc;
        blob.writeBytes(encoded);
      }
      for (int value : offs) {
        writeU32(out, value);
      }
      out.writeBytes(blob.toByteArray());
      padTo(out, 4);
      for (String name : names) {
        writeU32(out, mapping.get(name));
      }
    } else {
      padTo(out, 4);
      int[] offs = new int[names.size() + 1];
      int acc = 0;
      ByteArrayOutputStream blob = new ByteArrayOutputStream();
      offs[0] = 0;
      for (int i = 0; i < names.size(); i++) {
        byte[] encoded = names.get(i).getBytes(StandardCharsets.UTF_8);
        acc += encoded.length;
        offs[i + 1] = acc;
        blob.writeBytes(encoded);
      }
      for (int value : offs) {
        writeU32(out, value);
      }
      out.writeBytes(blob.toByteArray());
    }
    padTo(out, 4);
    if (useCodeDelta && !codes.isEmpty()) {
      long base = codes.get(0);
      writeU32(out, (int) base);
      long prev = base;
      for (int i = 1; i < codes.size(); i++) {
        long delta = codes.get(i) - prev;
        out.writeBytes(Varint.encode(zigzagEncode(delta)));
        prev = codes.get(i);
      }
    } else {
      for (long code : codes) {
        writeU32(out, (int) code);
      }
    }
    out.writeBytes(buildEnumFlags(rows));
    return out.toByteArray();
  }

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

  public record OptionalStringBoolRow(long id, String value, boolean flag) {}

  public record OptionalU32BoolRow(long id, Long value, boolean flag) {
    public OptionalU32BoolRow {
      if (value != null && (value < 0 || value > U32_MAX)) {
        throw new IllegalArgumentException("value must fit into u32");
      }
    }
  }

  public static final class BytesBoolRow {
    private final long id;
    private final byte[] data;
    private final boolean flag;

    public BytesBoolRow(long id, byte[] data, boolean flag) {
      if (data == null) {
        throw new IllegalArgumentException("data must not be null");
      }
      this.id = id;
      this.data = data.clone();
      this.flag = flag;
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

    public boolean flag() {
      return flag;
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof BytesBoolRow other)) {
        return false;
      }
      return id == other.id && flag == other.flag && Arrays.equals(data, other.data);
    }

    @Override
    public int hashCode() {
      int result = 31 * Long.hashCode(id) + Arrays.hashCode(data);
      return 31 * result + Boolean.hashCode(flag);
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

  public static byte[] encodeNcbU64OptionalStringBool(List<OptionalStringBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    boolean useDelta = shouldUseIdDeltaOptionalString(rows);
    writeU32(out, rows.size());
    out.write(useDelta ? DESC_U64_DELTA_OPTSTR_BOOL : DESC_U64_OPTSTR_BOOL);
    writeIds(out, rows, useDelta, OptionalStringBoolRow::id);
    writeOptionalStringColumn(out, rows);
    out.writeBytes(buildOptionalStringFlags(rows));
    return out.toByteArray();
  }

  public static List<OptionalStringBoolRow> decodeNcbU64OptionalStringBool(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB optional string payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_OPTSTR_BOOL && desc != DESC_U64_DELTA_OPTSTR_BOOL) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    DecodeIdsResult decodedIds = decodeIds(data, offset, n, desc == DESC_U64_DELTA_OPTSTR_BOOL, "optional string");
    List<Long> ids = decodedIds.ids();
    offset = decodedIds.offset();

    int bitBytes = (n + 7) / 8;
    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB optional string payload missing presence bitmap");
    }
    byte[] presence = Arrays.copyOfRange(data, offset, offset + bitBytes);
    validateBitsetPadding(presence, n, "optional string presence");
    int present = countSetBits(presence);
    offset += bitBytes + localPadding(bitBytes, 4);

    int[] offs = readOffsetTable(data, offset, present + 1, "optional string offsets");
    offset += (present + 1) * 4;
    int blobLen = offs[present];
    if (offset + blobLen > data.length) {
      throw new IllegalArgumentException("NCB optional string payload truncated (blob)");
    }
    validateOffsets(offs, blobLen, "optional string");
    byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
    offset += blobLen;

    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB optional string payload truncated (flags)");
    }
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    validateBitsetPadding(flags, n, "optional string flags");
    offset += bitBytes;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after optional string columnar decode");
    }

    List<OptionalStringBoolRow> rows = new ArrayList<>(n);
    int presentIndex = 0;
    for (int i = 0; i < n; i++) {
      String value = null;
      if (bitIsSet(presence, i)) {
        int start = offs[presentIndex];
        int end = offs[presentIndex + 1];
        value = decodeUtf8Strict(blob, start, end);
        presentIndex += 1;
      }
      boolean flag = bitIsSet(flags, i);
      rows.add(new OptionalStringBoolRow(ids.get(i), value, flag));
    }
    return rows;
  }

  public static byte[] encodeRowsU64OptionalStringBoolAdaptive(List<OptionalStringBoolRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64OptionalStringBool(rows);
      byte[] ncb = encodeNcbU64OptionalStringBool(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64OptionalStringBool(rows));
  }

  public static List<OptionalStringBoolRow> decodeRowsU64OptionalStringBoolAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64OptionalStringBool(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64OptionalStringBool(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
  }

  public static byte[] encodeNcbU64OptionalU32Bool(List<OptionalU32BoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    boolean useDelta = shouldUseIdDeltaOptionalU32(rows);
    writeU32(out, rows.size());
    out.write(useDelta ? DESC_U64_DELTA_OPTU32_BOOL : DESC_U64_OPTU32_BOOL);
    writeIds(out, rows, useDelta, OptionalU32BoolRow::id);
    writeOptionalU32Column(out, rows);
    out.writeBytes(buildOptionalU32Flags(rows));
    return out.toByteArray();
  }

  public static List<OptionalU32BoolRow> decodeNcbU64OptionalU32Bool(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB optional u32 payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_OPTU32_BOOL && desc != DESC_U64_DELTA_OPTU32_BOOL) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    DecodeIdsResult decodedIds = decodeIds(data, offset, n, desc == DESC_U64_DELTA_OPTU32_BOOL, "optional u32");
    List<Long> ids = decodedIds.ids();
    offset = decodedIds.offset();

    int bitBytes = (n + 7) / 8;
    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB optional u32 payload missing presence bitmap");
    }
    byte[] presence = Arrays.copyOfRange(data, offset, offset + bitBytes);
    validateBitsetPadding(presence, n, "optional u32 presence");
    int present = countSetBits(presence);
    offset += bitBytes + localPadding(bitBytes, 4);

    int valuesLen = present * 4;
    if (offset + valuesLen > data.length) {
      throw new IllegalArgumentException("NCB optional u32 payload truncated (values)");
    }
    long[] values = new long[present];
    for (int i = 0; i < present; i++) {
      values[i] = Integer.toUnsignedLong(readU32(data, offset));
      offset += 4;
    }

    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB optional u32 payload truncated (flags)");
    }
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    validateBitsetPadding(flags, n, "optional u32 flags");
    offset += bitBytes;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after optional u32 columnar decode");
    }

    List<OptionalU32BoolRow> rows = new ArrayList<>(n);
    int presentIndex = 0;
    for (int i = 0; i < n; i++) {
      Long value = null;
      if (bitIsSet(presence, i)) {
        value = values[presentIndex++];
      }
      rows.add(new OptionalU32BoolRow(ids.get(i), value, bitIsSet(flags, i)));
    }
    return rows;
  }

  public static byte[] encodeRowsU64OptionalU32BoolAdaptive(List<OptionalU32BoolRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64OptionalU32Bool(rows);
      byte[] ncb = encodeNcbU64OptionalU32Bool(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64OptionalU32Bool(rows));
  }

  public static List<OptionalU32BoolRow> decodeRowsU64OptionalU32BoolAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64OptionalU32Bool(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64OptionalU32Bool(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
  }

  public static byte[] encodeNcbU64BytesBool(List<BytesBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    boolean useDelta = shouldUseIdDeltaBytesBool(rows);
    writeU32(out, rows.size());
    out.write(useDelta ? DESC_U64_DELTA_BYTES_BOOL : DESC_U64_BYTES_BOOL);
    writeIds(out, rows, useDelta, BytesBoolRow::id);
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
    out.writeBytes(buildBytesBoolFlags(rows));
    return out.toByteArray();
  }

  public static List<BytesBoolRow> decodeNcbU64BytesBool(byte[] data) {
    if (data.length < 5) {
      throw new IllegalArgumentException("NCB bytes bool payload too short");
    }
    int offset = 0;
    int n = readU32(data, offset);
    offset += 4;
    int desc = data[offset++] & 0xFF;
    if (desc != DESC_U64_BYTES_BOOL && desc != DESC_U64_DELTA_BYTES_BOOL) {
      throw new IllegalArgumentException(String.format("Unsupported descriptor 0x%02x", desc));
    }
    DecodeIdsResult decodedIds = decodeIds(data, offset, n, desc == DESC_U64_DELTA_BYTES_BOOL, "bytes bool");
    List<Long> ids = decodedIds.ids();
    offset = align(decodedIds.offset(), 4);

    int[] offs = readOffsetTable(data, offset, n + 1, "bytes bool offsets");
    offset += (n + 1) * 4;
    int blobLen = offs[n];
    if (offset + blobLen > data.length) {
      throw new IllegalArgumentException("NCB bytes bool payload truncated (blob)");
    }
    validateOffsets(offs, blobLen, "bytes bool");
    byte[] blob = Arrays.copyOfRange(data, offset, offset + blobLen);
    offset += blobLen;

    int bitBytes = (n + 7) / 8;
    if (offset + bitBytes > data.length) {
      throw new IllegalArgumentException("NCB bytes bool payload truncated (flags)");
    }
    byte[] flags = Arrays.copyOfRange(data, offset, offset + bitBytes);
    validateBitsetPadding(flags, n, "bytes bool flags");
    offset += bitBytes;
    if (offset != data.length) {
      throw new IllegalArgumentException("Trailing bytes after bytes bool columnar decode");
    }

    List<BytesBoolRow> rows = new ArrayList<>(n);
    for (int i = 0; i < n; i++) {
      int start = offs[i];
      int end = offs[i + 1];
      rows.add(new BytesBoolRow(ids.get(i), Arrays.copyOfRange(blob, start, end), bitIsSet(flags, i)));
    }
    return rows;
  }

  public static byte[] encodeRowsU64BytesBoolAdaptive(List<BytesBoolRow> rows) {
    if (rows.size() <= AOS_NCB_SMALL_N) {
      byte[] aos = NoritoAoS.encodeU64BytesBool(rows);
      byte[] ncb = encodeNcbU64BytesBool(rows);
      if (ncb.length < aos.length) {
        return concat(ADAPTIVE_TAG_NCB, ncb);
      }
      return concat(ADAPTIVE_TAG_AOS, aos);
    }
    return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64BytesBool(rows));
  }

  public static List<BytesBoolRow> decodeRowsU64BytesBoolAdaptive(byte[] payload) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Adaptive payload is empty");
    }
    int tag = payload[0] & 0xFF;
    byte[] body = Arrays.copyOfRange(payload, 1, payload.length);
    return switch (tag) {
      case ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64BytesBool(body);
      case ADAPTIVE_TAG_NCB -> decodeNcbU64BytesBool(body);
      default -> throw new IllegalArgumentException("Unknown adaptive tag: " + tag);
    };
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

  private static byte[] buildEnumFlags(List<EnumBoolRow> rows) {
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

  private static byte[] buildOptionalStringPresenceFlags(List<OptionalStringBoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).value() != null) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static byte[] buildOptionalStringFlags(List<OptionalStringBoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).flag()) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static byte[] buildOptionalU32PresenceFlags(List<OptionalU32BoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).value() != null) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static byte[] buildOptionalU32Flags(List<OptionalU32BoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).flag()) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static byte[] buildBytesBoolFlags(List<BytesBoolRow> rows) {
    int bytes = (rows.size() + 7) / 8;
    byte[] bits = new byte[bytes];
    for (int i = 0; i < rows.size(); i++) {
      if (rows.get(i).flag()) {
        bits[i / 8] |= (byte) (1 << (i % 8));
      }
    }
    return bits;
  }

  private static void writeOptionalStringColumn(ByteArrayOutputStream out, List<OptionalStringBoolRow> rows) {
    byte[] presence = buildOptionalStringPresenceFlags(rows);
    out.writeBytes(presence);
    out.write(new byte[localPadding(presence.length, 4)], 0, localPadding(presence.length, 4));

    int present = countSetBits(presence);
    int[] offs = new int[present + 1];
    int acc = 0;
    int presentIndex = 0;
    ByteArrayOutputStream blob = new ByteArrayOutputStream();
    offs[0] = 0;
    for (OptionalStringBoolRow row : rows) {
      String value = row.value();
      if (value != null) {
        byte[] encoded = value.getBytes(StandardCharsets.UTF_8);
        acc += encoded.length;
        presentIndex += 1;
        offs[presentIndex] = acc;
        blob.writeBytes(encoded);
      }
    }
    for (int value : offs) {
      writeU32(out, value);
    }
    out.writeBytes(blob.toByteArray());
  }

  private static void writeOptionalU32Column(ByteArrayOutputStream out, List<OptionalU32BoolRow> rows) {
    byte[] presence = buildOptionalU32PresenceFlags(rows);
    out.writeBytes(presence);
    out.write(new byte[localPadding(presence.length, 4)], 0, localPadding(presence.length, 4));
    for (OptionalU32BoolRow row : rows) {
      Long value = row.value();
      if (value != null) {
        writeU32(out, value.intValue());
      }
    }
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

  private static boolean shouldUseIdDeltaOptionalString(List<OptionalStringBoolRow> rows) {
    return shouldUseIdDeltaGeneric(rows, OptionalStringBoolRow::id);
  }

  private static boolean shouldUseIdDeltaOptionalU32(List<OptionalU32BoolRow> rows) {
    return shouldUseIdDeltaGeneric(rows, OptionalU32BoolRow::id);
  }

  private static boolean shouldUseIdDeltaBytesBool(List<BytesBoolRow> rows) {
    if (!COMBO_ENABLE_ID_DELTA || rows.size() < COMBO_ID_DELTA_MIN_ROWS) {
      return false;
    }
    if (rows.size() <= COMBO_NO_DELTA_SMALL_N_IF_EMPTY) {
      for (BytesBoolRow row : rows) {
        if (row.dataRaw().length == 0) {
          return false;
        }
      }
    }
    return shouldUseIdDeltaGeneric(rows, BytesBoolRow::id);
  }

  private static <T> boolean shouldUseIdDeltaGeneric(List<T> rows, LongGetter<T> getter) {
    if (!COMBO_ENABLE_ID_DELTA || rows.size() < COMBO_ID_DELTA_MIN_ROWS) {
      return false;
    }
    long prev = getter.get(rows.get(0));
    int varintBytes = 0;
    for (int i = 1; i < rows.size(); i++) {
      long current = getter.get(rows.get(i));
      long delta = current - prev;
      long zz = zigzagEncode(delta);
      varintBytes += varintLength(zz);
      if (varintBytes >= 8 * (rows.size() - 1)) {
        return false;
      }
      prev = current;
    }
    return true;
  }

  private static boolean shouldUseIdDeltaEnum(List<EnumBoolRow> rows) {
    if (rows.size() < 2) {
      return false;
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

  private static boolean shouldUseNameDictEnum(List<EnumBoolRow> rows) {
    int totalLen = 0;
    int nameCount = 0;
    Map<String, Integer> distinct = new HashMap<>();
    for (EnumBoolRow row : rows) {
      EnumValue value = row.value();
      if (value instanceof EnumName name) {
        totalLen += name.name().length();
        nameCount += 1;
        distinct.computeIfAbsent(name.name(), k -> distinct.size());
      }
    }
    if (nameCount == 0) {
      return false;
    }
    double ratio = (double) distinct.size() / nameCount;
    double avg = (double) totalLen / nameCount;
    return ratio <= COMBO_DICT_RATIO_MAX && avg >= COMBO_DICT_AVG_LEN_MIN;
  }

  private static boolean shouldUseCodeDeltaEnum(List<EnumBoolRow> rows) {
    List<Long> codes = new ArrayList<>();
    for (EnumBoolRow row : rows) {
      EnumValue value = row.value();
      if (value instanceof EnumCode code) {
        codes.add(code.code());
      }
    }
    if (codes.size() < 2) {
      return false;
    }
    long prev = codes.get(0);
    int varintBytes = 0;
    for (int i = 1; i < codes.size(); i++) {
      long delta = codes.get(i) - prev;
      long zz = zigzagEncode(delta);
      varintBytes += varintLength(zz);
      if (varintBytes >= 4 * (codes.size() - 1)) {
        return false;
      }
      prev = codes.get(i);
    }
    return true;
  }

  private static <T> void writeIds(ByteArrayOutputStream out, List<T> rows, boolean useDelta, LongGetter<T> getter) {
    padTo(out, 8);
    if (useDelta && !rows.isEmpty()) {
      long base = getter.get(rows.get(0));
      writeU64(out, base);
      long prev = base;
      for (int i = 1; i < rows.size(); i++) {
        long current = getter.get(rows.get(i));
        long delta = current - prev;
        out.writeBytes(Varint.encode(zigzagEncode(delta)));
        prev = current;
      }
    } else {
      for (T row : rows) {
        writeU64(out, getter.get(row));
      }
    }
  }

  private static DecodeIdsResult decodeIds(byte[] data, int offset, int n, boolean useDelta, String context) {
    offset = align(offset, 8);
    List<Long> ids = new ArrayList<>(n);
    if (useDelta) {
      if (n > 0) {
        if (offset + 8 > data.length) {
          throw new IllegalArgumentException("NCB " + context + " payload truncated (id base)");
        }
        long base = readU64(data, offset);
        offset += 8;
        ids.add(base);
        while (ids.size() < n) {
          Varint.DecodeResult res = Varint.decode(data, offset);
          offset = res.nextOffset();
          long delta = zigzagDecode(res.value());
          ids.add(ids.get(ids.size() - 1) + delta);
        }
      }
    } else {
      for (int i = 0; i < n; i++) {
        if (offset + 8 > data.length) {
          throw new IllegalArgumentException("NCB " + context + " payload truncated (ids)");
        }
        ids.add(readU64(data, offset));
        offset += 8;
      }
    }
    return new DecodeIdsResult(ids, offset);
  }

  private static int[] readOffsetTable(byte[] data, int offset, int count, String context) {
    if (count < 1) {
      throw new IllegalArgumentException("Offset table must include a sentinel");
    }
    if (offset + count * 4 > data.length) {
      throw new IllegalArgumentException("NCB payload truncated (" + context + ")");
    }
    int[] offs = new int[count];
    for (int i = 0; i < count; i++) {
      offs[i] = readU32(data, offset);
      offset += 4;
    }
    if (offs[0] != 0) {
      throw new IllegalArgumentException("Invalid offset table in " + context);
    }
    return offs;
  }

  private static void validateOffsets(int[] offs, int blobLen, String context) {
    int prev = 0;
    for (int off : offs) {
      if (off < prev || off > blobLen) {
        throw new IllegalArgumentException("Invalid offset table in " + context + " payload");
      }
      prev = off;
    }
  }

  private static void validateBitsetPadding(byte[] bits, int rows, String context) {
    int usedBits = rows % 8;
    if (usedBits == 0 || bits.length == 0) {
      return;
    }
    int mask = 0xFF << usedBits;
    if ((bits[bits.length - 1] & mask) != 0) {
      throw new IllegalArgumentException("Non-zero padding bits in " + context + " bitmap");
    }
  }

  private static int countSetBits(byte[] bits) {
    int count = 0;
    for (byte bit : bits) {
      count += Integer.bitCount(bit & 0xFF);
    }
    return count;
  }

  private static boolean bitIsSet(byte[] bits, int index) {
    return ((bits[index / 8] >> (index % 8)) & 1) != 0;
  }

  private static int localPadding(int length, int align) {
    int mis = length % align;
    return mis == 0 ? 0 : align - mis;
  }

  private static String decodeUtf8Strict(byte[] bytes, int start, int end) {
    try {
      return StandardCharsets.UTF_8
          .newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(bytes, start, end - start))
          .toString();
    } catch (CharacterCodingException ex) {
      throw new IllegalArgumentException("Invalid UTF-8 in columnar string payload", ex);
    }
  }

  private interface LongGetter<T> {
    long get(T value);
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

  private static EnumDescriptor parseEnumDescriptor(int desc) {
    return switch (desc) {
      case DESC_U64_ENUM_BOOL -> new EnumDescriptor(false, false, false);
      case DESC_U64_DELTA_ENUM_BOOL -> new EnumDescriptor(true, false, false);
      case DESC_U64_ENUM_BOOL_CODEDELTA -> new EnumDescriptor(false, false, true);
      case DESC_U64_DELTA_ENUM_BOOL_CODEDELTA -> new EnumDescriptor(true, false, true);
      case DESC_U64_ENUM_BOOL_DICT -> new EnumDescriptor(false, true, false);
      case DESC_U64_DELTA_ENUM_BOOL_DICT -> new EnumDescriptor(true, true, false);
      case DESC_U64_ENUM_BOOL_DICT_CODEDELTA -> new EnumDescriptor(false, true, true);
      case DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA -> new EnumDescriptor(true, true, true);
      default ->
          throw new IllegalArgumentException(String.format("Unsupported enum descriptor 0x%02x", desc));
    };
  }

  private record EnumDescriptor(boolean deltaIds, boolean nameDict, boolean codeDelta) {}

  private record DecodeIdsResult(List<Long> ids, int offset) {}

  private record DictResult(boolean useDict, Map<String, Integer> mapping, List<String> dictionary) {
    static DictResult enabled(Map<String, Integer> mapping, List<String> dictionary) {
      return new DictResult(true, mapping, dictionary);
    }

    static DictResult disabled() {
      return new DictResult(false, Map.of(), List.of());
    }
  }
}
