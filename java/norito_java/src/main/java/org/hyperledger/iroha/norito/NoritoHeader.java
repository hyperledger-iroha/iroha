// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;

/** Represents the Norito header (NRT0 major 0 with fixed v1 minor). */
public final class NoritoHeader {
  public static final byte[] MAGIC = new byte[] {'N', 'R', 'T', '0'};
  public static final int MAJOR_VERSION = 0;

  public static final int PACKED_SEQ = 0x01;
  public static final int COMPACT_LEN = 0x02;
  public static final int PACKED_STRUCT = 0x04;
  public static final int VARINT_OFFSETS = 0x08;
  public static final int COMPACT_SEQ_LEN = 0x10;
  public static final int FIELD_BITSET = 0x20;

  private static final int SUPPORTED_FLAGS_MASK =
      PACKED_SEQ | COMPACT_LEN | PACKED_STRUCT | VARINT_OFFSETS | COMPACT_SEQ_LEN | FIELD_BITSET;

  /** Minor version is fixed for v1; layout flags are declared in the header flag byte. */
  public static final int MINOR_VERSION = 0;

  public static final int COMPRESSION_NONE = 0;
  public static final int COMPRESSION_ZSTD = 1;

  public static final int HEADER_LENGTH = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1;
  public static final int MAX_HEADER_PADDING = 64;

  private final byte[] schemaHash;
  private final int payloadLength;
  private final long checksum;
  private final int flags;
  private final int compression;
  private final int minor;

  public NoritoHeader(byte[] schemaHash, int payloadLength, long checksum, int flags, int compression) {
    this(schemaHash, payloadLength, checksum, flags, compression, MINOR_VERSION);
  }

  public NoritoHeader(
      byte[] schemaHash, int payloadLength, long checksum, int flags, int compression, int minor) {
    if (schemaHash.length != 16) {
      throw new IllegalArgumentException("schemaHash must be 16 bytes");
    }
    int normalizedMinor = minor & 0xFF;
    if (normalizedMinor != MINOR_VERSION) {
      throw new IllegalArgumentException(
          String.format("Unsupported Norito minor version: 0x%02x", normalizedMinor));
    }
    this.schemaHash = Arrays.copyOf(schemaHash, schemaHash.length);
    this.payloadLength = payloadLength;
    this.checksum = checksum;
    this.flags = flags;
    this.compression = compression;
    this.minor = normalizedMinor;
  }

  public byte[] schemaHash() {
    return Arrays.copyOf(schemaHash, schemaHash.length);
  }

  public int payloadLength() {
    return payloadLength;
  }

  public long checksum() {
    return checksum;
  }

  public int flags() {
    return flags;
  }

  public int compression() {
    return compression;
  }

  public int minor() {
    return minor;
  }

  public byte[] encode() {
    ByteBuffer buffer = ByteBuffer.allocate(HEADER_LENGTH).order(ByteOrder.LITTLE_ENDIAN);
    buffer.put(MAGIC);
    buffer.put((byte) MAJOR_VERSION);
    buffer.put((byte) (minor & 0xFF));
    buffer.put(schemaHash);
    buffer.put((byte) compression);
    buffer.putLong(payloadLength & 0xFFFFFFFFL);
    buffer.putLong(checksum);
    buffer.put((byte) (flags & 0xFF));
    return buffer.array();
  }

  public void validateChecksum(byte[] payload) {
    long actual = CRC64.compute(payload);
    if (actual != checksum) {
      throw new IllegalArgumentException(
          String.format("Checksum mismatch: expected 0x%016x got 0x%016x", checksum, actual));
    }
  }

  public static DecodeResult decode(byte[] data, byte[] expectedHash) {
    if (data.length < HEADER_LENGTH) {
      throw new IllegalArgumentException("Insufficient data for Norito header");
    }
    ByteBuffer buffer = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN);
    byte[] magic = new byte[4];
    buffer.get(magic);
    if (!Arrays.equals(magic, MAGIC)) {
      throw new IllegalArgumentException("Invalid Norito magic");
    }
    int major = buffer.get() & 0xFF;
    int minor = buffer.get() & 0xFF;
    if (major != MAJOR_VERSION) {
      throw new IllegalArgumentException("Unsupported Norito version: " + major + "." + minor);
    }
    if (minor != MINOR_VERSION) {
      throw new IllegalArgumentException("Unsupported Norito version: " + major + "." + minor);
    }
    byte[] schemaHash = new byte[16];
    buffer.get(schemaHash);
    if (expectedHash != null && !Arrays.equals(expectedHash, schemaHash)) {
      throw new IllegalArgumentException("Schema mismatch");
    }
    int compression = buffer.get() & 0xFF;
    if (compression != COMPRESSION_NONE && compression != COMPRESSION_ZSTD) {
      throw new IllegalStateException("Unsupported compression byte: " + compression);
    }
    long payloadLength = buffer.getLong();
    if (payloadLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Payload too large for Java implementation");
    }
    long checksum = buffer.getLong();
    int flags = buffer.get() & 0xFF;
    int unsupportedFlags = flags & ~SUPPORTED_FLAGS_MASK;
    if (unsupportedFlags != 0) {
      throw new IllegalArgumentException(
          String.format("Unsupported Norito layout flags: 0x%02x", unsupportedFlags));
    }
    int intPayloadLength = (int) payloadLength;
    int payloadOffset = HEADER_LENGTH;
    byte[] payload;
    if (compression == COMPRESSION_NONE) {
      int minEnd = payloadOffset + intPayloadLength;
      if (minEnd > data.length) {
        throw new IllegalArgumentException("Length mismatch between header and payload");
      }
      int paddingLength = data.length - minEnd;
      if (paddingLength > MAX_HEADER_PADDING) {
        throw new IllegalArgumentException("Trailing data after Norito payload");
      }
      if (paddingLength > 0) {
        for (int i = 0; i < paddingLength; i++) {
          if (data[payloadOffset + i] != 0) {
            throw new IllegalArgumentException("Non-zero Norito header padding");
          }
        }
      }
      payloadOffset += paddingLength;
      int payloadEnd = payloadOffset + intPayloadLength;
      if (payloadEnd != data.length) {
        throw new IllegalArgumentException("Trailing data after Norito payload");
      }
      payload = Arrays.copyOfRange(data, payloadOffset, payloadEnd);
    } else {
      if (payloadOffset >= data.length) {
        throw new IllegalArgumentException("Missing compressed payload bytes");
      }
      payload = Arrays.copyOfRange(data, payloadOffset, data.length);
    }
    NoritoHeader header =
        new NoritoHeader(schemaHash, intPayloadLength, checksum, flags, compression, minor);
    return new DecodeResult(header, payload);
  }

  public record DecodeResult(NoritoHeader header, byte[] payload) {}
}
