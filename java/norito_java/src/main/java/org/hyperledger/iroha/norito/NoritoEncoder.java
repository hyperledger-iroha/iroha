// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Objects;

/** Low-level helper for writing Norito payloads. */
public final class NoritoEncoder {
  private final int flags;
  private final ByteArrayOutputStream buffer;

  public NoritoEncoder(int flags) {
    this.flags = flags;
    this.buffer = new ByteArrayOutputStream();
  }

  public int flags() {
    return flags;
  }

  public void writeByte(int value) {
    buffer.write(value & 0xFF);
  }

  public void writeBytes(byte[] data) {
    buffer.writeBytes(data);
  }

  public void writeUInt(long value, int bits) {
    int bytes = bits / 8;
    ByteBuffer bb = ByteBuffer.allocate(bytes).order(ByteOrder.LITTLE_ENDIAN);
    if (bits == 64) {
      bb.putLong(value);
    } else if (bits == 32) {
      bb.putInt((int) value);
    } else if (bits == 16) {
      bb.putShort((short) value);
    } else if (bits == 8) {
      bb.put((byte) value);
    } else {
      throw new IllegalArgumentException("Unsupported integer size: " + bits);
    }
    writeBytes(bb.array());
  }

  public void writeInt(long value, int bits) {
    writeUInt(value, bits); // little-endian same for signed writes
  }

  public void writeLength(long value, boolean compact) {
    if (compact) {
      writeBytes(Varint.encode(value));
    } else {
      writeUInt(value, 64);
    }
  }

  public NoritoEncoder childEncoder() {
    return new NoritoEncoder(flags);
  }

  public byte[] toByteArray() {
    return buffer.toByteArray();
  }

  public void append(byte[] data) {
    writeBytes(Objects.requireNonNull(data));
  }
}
