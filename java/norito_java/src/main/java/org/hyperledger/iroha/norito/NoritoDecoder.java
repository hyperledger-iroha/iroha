// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;

/** Reader for Norito payloads. */
public final class NoritoDecoder {
  private final ByteBuffer buffer;
  private final int flags;
  private final int flagsHint;

  public NoritoDecoder(byte[] data, int flags) {
    this(data, flags, NoritoHeader.MINOR_VERSION);
  }

  public NoritoDecoder(byte[] data, int flags, int flagsHint) {
    this(ByteBuffer.wrap(Arrays.copyOf(data, data.length)), flags, flagsHint);
  }

  public NoritoDecoder(ByteBuffer data, int flags, int flagsHint) {
    this.buffer = data.slice().order(ByteOrder.LITTLE_ENDIAN);
    this.flags = flags;
    this.flagsHint = flagsHint;
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
    return buffer.remaining();
  }

  public byte[] readBytes(int count) {
    ensure(count);
    byte[] out = new byte[count];
    buffer.get(out);
    return out;
  }

  public int readByte() {
    ensure(1);
    return buffer.get() & 0xFF;
  }

  public long readUInt(int bits) {
    int bytes = bits / 8;
    ensure(bytes);
    return switch (bits) {
      case 8 -> buffer.get() & 0xFFL;
      case 16 -> buffer.getShort() & 0xFFFFL;
      case 32 -> buffer.getInt() & 0xFFFFFFFFL;
      case 64 -> buffer.getLong();
      default -> throw new IllegalArgumentException("Unsupported integer size: " + bits);
    };
  }

  public long readInt(int bits) {
    int bytes = bits / 8;
    ensure(bytes);
    return switch (bits) {
      case 8 -> buffer.get();
      case 16 -> buffer.getShort();
      case 32 -> buffer.getInt();
      case 64 -> buffer.getLong();
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
    Varint.DecodeResult result = Varint.decode(buffer);
    return result.value();
  }

  private void ensure(int count) {
    if (buffer.remaining() < count) {
      throw new IllegalArgumentException("Unexpected end of data");
    }
  }
}
