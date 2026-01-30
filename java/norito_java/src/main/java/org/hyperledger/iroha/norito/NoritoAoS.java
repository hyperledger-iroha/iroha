// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

final class NoritoAoS {
  private static final int VERSION = 0x1;

  private NoritoAoS() {}

  static byte[] encodeU64StrBool(List<NoritoColumnar.StrBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeVarint(out, rows.size());
    out.write(VERSION);
    for (NoritoColumnar.StrBoolRow row : rows) {
      writeLong(out, row.id());
      byte[] data = row.name().getBytes(StandardCharsets.UTF_8);
      writeVarint(out, data.length);
      out.writeBytes(data);
      out.write(row.flag() ? 1 : 0);
    }
    return out.toByteArray();
  }

  static List<NoritoColumnar.StrBoolRow> decodeU64StrBool(byte[] body) {
    Varint.DecodeResult lenRes = Varint.decode(body, 0);
    int length = (int) lenRes.value();
    int offset = lenRes.nextOffset();
    int version = body[offset++] & 0xFF;
    if (version != VERSION) {
      throw new IllegalArgumentException("Unsupported AoS version byte: " + version);
    }
    List<NoritoColumnar.StrBoolRow> rows = new ArrayList<>(length);
    for (int i = 0; i < length; i++) {
      long id = readLong(body, offset);
      offset += 8;
      Varint.DecodeResult nameRes = Varint.decode(body, offset);
      int nameLen = (int) nameRes.value();
      offset = nameRes.nextOffset();
      String name = new String(body, offset, nameLen, StandardCharsets.UTF_8);
      offset += nameLen;
      boolean flag = (body[offset++] & 0xFF) != 0;
      rows.add(new NoritoColumnar.StrBoolRow(id, name, flag));
    }
    if (offset != body.length) {
      throw new IllegalArgumentException("Trailing bytes after AoS decode");
    }
    return rows;
  }

  static byte[] encodeU64Bytes(List<NoritoColumnar.BytesRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeVarint(out, rows.size());
    out.write(VERSION);
    for (NoritoColumnar.BytesRow row : rows) {
      writeLong(out, row.id());
      byte[] data = row.data();
      writeVarint(out, data.length);
      out.writeBytes(data);
    }
    return out.toByteArray();
  }

  static List<NoritoColumnar.BytesRow> decodeU64Bytes(byte[] body) {
    Varint.DecodeResult lenRes = Varint.decode(body, 0);
    int length = (int) lenRes.value();
    int offset = lenRes.nextOffset();
    int version = body[offset++] & 0xFF;
    if (version != VERSION) {
      throw new IllegalArgumentException("Unsupported AoS version byte: " + version);
    }
    List<NoritoColumnar.BytesRow> rows = new ArrayList<>(length);
    for (int i = 0; i < length; i++) {
      long id = readLong(body, offset);
      offset += 8;
      Varint.DecodeResult lenVal = Varint.decode(body, offset);
      int dataLen = (int) lenVal.value();
      offset = lenVal.nextOffset();
      if (offset + dataLen > body.length) {
        throw new IllegalArgumentException("AoS bytes row exceeds payload bounds");
      }
      byte[] data = new byte[dataLen];
      System.arraycopy(body, offset, data, 0, dataLen);
      offset += dataLen;
      rows.add(new NoritoColumnar.BytesRow(id, data));
    }
    if (offset != body.length) {
      throw new IllegalArgumentException("Trailing bytes after AoS decode");
    }
    return rows;
  }

  static byte[] encodeU64OptionalBytes(List<NoritoColumnar.BytesOptionalRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeVarint(out, rows.size());
    out.write(VERSION);
    for (NoritoColumnar.BytesOptionalRow row : rows) {
      writeLong(out, row.id());
      byte[] data = row.data();
      if (data == null) {
        out.write(0);
      } else {
        out.write(1);
        writeVarint(out, data.length);
        out.writeBytes(data);
      }
    }
    return out.toByteArray();
  }

  static List<NoritoColumnar.BytesOptionalRow> decodeU64OptionalBytes(byte[] body) {
    Varint.DecodeResult lenRes = Varint.decode(body, 0);
    int length = (int) lenRes.value();
    int offset = lenRes.nextOffset();
    int version = body[offset++] & 0xFF;
    if (version != VERSION) {
      throw new IllegalArgumentException("Unsupported AoS version byte: " + version);
    }
    List<NoritoColumnar.BytesOptionalRow> rows = new ArrayList<>(length);
    for (int i = 0; i < length; i++) {
      long id = readLong(body, offset);
      offset += 8;
      int present = body[offset++] & 0xFF;
      if (present == 0) {
        rows.add(new NoritoColumnar.BytesOptionalRow(id, null));
      } else if (present == 1) {
        Varint.DecodeResult lenVal = Varint.decode(body, offset);
        int dataLen = (int) lenVal.value();
        offset = lenVal.nextOffset();
        if (offset + dataLen > body.length) {
          throw new IllegalArgumentException("AoS optional bytes row exceeds payload bounds");
        }
        byte[] data = new byte[dataLen];
        System.arraycopy(body, offset, data, 0, dataLen);
        offset += dataLen;
        rows.add(new NoritoColumnar.BytesOptionalRow(id, data));
      } else {
        throw new IllegalArgumentException("Invalid presence flag in AoS optional bytes payload");
      }
    }
    if (offset != body.length) {
      throw new IllegalArgumentException("Trailing bytes after AoS decode");
    }
    return rows;
  }

  static byte[] encodeU64EnumBool(List<NoritoColumnar.EnumBoolRow> rows) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    // Historical enum AoS layout omits the version byte to preserve golden payloads.
    writeVarint(out, rows.size());
    for (NoritoColumnar.EnumBoolRow row : rows) {
      writeLong(out, row.id());
      NoritoColumnar.EnumValue value = row.value();
      if (value instanceof NoritoColumnar.EnumName name) {
        out.write(0);
        byte[] data = name.name().getBytes(StandardCharsets.UTF_8);
        writeVarint(out, data.length);
        out.writeBytes(data);
      } else if (value instanceof NoritoColumnar.EnumCode code) {
        out.write(1);
        writeU32(out, code.code());
      } else {
        throw new IllegalArgumentException("Unsupported enum value type: " + value.getClass());
      }
      out.write(row.flag() ? 1 : 0);
    }
    return out.toByteArray();
  }

  static List<NoritoColumnar.EnumBoolRow> decodeU64EnumBool(byte[] body) {
    Varint.DecodeResult lenRes = Varint.decode(body, 0);
    int length = (int) lenRes.value();
    int offset = lenRes.nextOffset();
    List<NoritoColumnar.EnumBoolRow> rows = new ArrayList<>(length);
    for (int i = 0; i < length; i++) {
      if (offset + 8 > body.length) {
        throw new IllegalArgumentException("AoS enum payload truncated (id)");
      }
      long id = readLong(body, offset);
      offset += 8;
      if (offset >= body.length) {
        throw new IllegalArgumentException("AoS enum payload truncated (tag)");
      }
      int tag = body[offset++] & 0xFF;
      NoritoColumnar.EnumValue value;
      if (tag == 0) {
        Varint.DecodeResult nameRes = Varint.decode(body, offset);
        int nameLen = (int) nameRes.value();
        offset = nameRes.nextOffset();
        if (offset + nameLen > body.length) {
          throw new IllegalArgumentException("AoS enum payload truncated (name)");
        }
        String name = new String(body, offset, nameLen, StandardCharsets.UTF_8);
        offset += nameLen;
        value = new NoritoColumnar.EnumName(name);
      } else if (tag == 1) {
        if (offset + 4 > body.length) {
          throw new IllegalArgumentException("AoS enum payload truncated (code)");
        }
        long code = Integer.toUnsignedLong(readU32(body, offset));
        offset += 4;
        value = new NoritoColumnar.EnumCode(code);
      } else {
        throw new IllegalArgumentException("Invalid enum tag: " + tag);
      }
      if (offset >= body.length) {
        throw new IllegalArgumentException("AoS enum payload truncated (flag)");
      }
      boolean flag = (body[offset++] & 0xFF) != 0;
      rows.add(new NoritoColumnar.EnumBoolRow(id, value, flag));
    }
    if (offset != body.length) {
      throw new IllegalArgumentException("Trailing bytes after AoS enum decode");
    }
    return rows;
  }

  private static void writeVarint(ByteArrayOutputStream out, int value) {
    out.writeBytes(Varint.encode(value));
  }

  private static void writeLong(ByteArrayOutputStream out, long value) {
    out.write((int) (value & 0xFF));
    out.write((int) ((value >>> 8) & 0xFF));
    out.write((int) ((value >>> 16) & 0xFF));
    out.write((int) ((value >>> 24) & 0xFF));
    out.write((int) ((value >>> 32) & 0xFF));
    out.write((int) ((value >>> 40) & 0xFF));
    out.write((int) ((value >>> 48) & 0xFF));
    out.write((int) ((value >>> 56) & 0xFF));
  }

  private static void writeU32(ByteArrayOutputStream out, long value) {
    out.write((int) (value & 0xFF));
    out.write((int) ((value >>> 8) & 0xFF));
    out.write((int) ((value >>> 16) & 0xFF));
    out.write((int) ((value >>> 24) & 0xFF));
  }

  private static long readLong(byte[] data, int offset) {
    long value = 0;
    for (int i = 0; i < 8; i++) {
      value |= (long) (data[offset + i] & 0xFF) << (8 * i);
    }
    return value;
  }

  private static int readU32(byte[] data, int offset) {
    return (data[offset] & 0xFF)
        | ((data[offset + 1] & 0xFF) << 8)
        | ((data[offset + 2] & 0xFF) << 16)
        | ((data[offset + 3] & 0xFF) << 24);
  }
}
