// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;

/** Reader for Norito payloads. */
public final class NoritoDecoder {
  private final byte[] data;
  private final int flags;
  private final int flagsHint;
  private int offset;

  public NoritoDecoder(byte[] data, int flags) {
    this(data, flags, NoritoHeader.MINOR_VERSION);
  }

  public NoritoDecoder(byte[] data, int flags, int flagsHint) {
    this.data = Arrays.copyOf(data, data.length);
    this.flags = flags;
    this.flagsHint = flagsHint;
    this.offset = 0;
  }

  public int flags() {
    return flags;
  }

  public int flagsHint() {
    return flagsHint;
  }

  public boolean compactLenActive() {
    return explicitlyEnabled(NoritoHeader.COMPACT_LEN);
  }

  private boolean explicitlyEnabled(int flag) {
    return (flags & flag) != 0;
  }

  public int remaining() {
    return data.length - offset;
  }

  public byte[] readBytes(int count) {
    ensure(count);
    byte[] out = Arrays.copyOfRange(data, offset, offset + count);
    offset += count;
    return out;
  }

  public int readByte() {
    ensure(1);
    return data[offset++] & 0xFF;
  }

  public long readUInt(int bits) {
    int bytes = bits / 8;
    ensure(bytes);
    ByteBuffer bb = ByteBuffer.wrap(data, offset, bytes).order(ByteOrder.LITTLE_ENDIAN);
    offset += bytes;
    return switch (bits) {
      case 8 -> bb.get() & 0xFFL;
      case 16 -> bb.getShort() & 0xFFFFL;
      case 32 -> bb.getInt() & 0xFFFFFFFFL;
      case 64 -> bb.getLong();
      default -> throw new IllegalArgumentException("Unsupported integer size: " + bits);
    };
  }

  public long readInt(int bits) {
    int bytes = bits / 8;
    ensure(bytes);
    ByteBuffer bb = ByteBuffer.wrap(data, offset, bytes).order(ByteOrder.LITTLE_ENDIAN);
    offset += bytes;
    return switch (bits) {
      case 8 -> bb.get();
      case 16 -> bb.getShort();
      case 32 -> bb.getInt();
      case 64 -> bb.getLong();
      default -> throw new IllegalArgumentException("Unsupported integer size: " + bits);
    };
  }

  public long readLength(boolean compact) {
    if (compact) {
      return readVarint();
    }
    return readUInt(64);
  }

  public long readVarint() {
    Varint.DecodeResult result = Varint.decode(data, offset);
    offset = result.nextOffset();
    return result.value();
  }

  private void ensure(int count) {
    if (offset + count > data.length) {
      throw new IllegalArgumentException("Unexpected end of data");
    }
  }
}
