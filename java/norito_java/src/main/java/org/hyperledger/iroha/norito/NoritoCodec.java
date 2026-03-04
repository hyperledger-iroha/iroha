// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.Deque;
import java.util.Locale;
import java.util.Objects;

/** High-level helpers for Norito encoding/decoding. */
public final class NoritoCodec {
  private static final int DYNAMIC_FLAGS_MASK = 0;
  public static final int DEFAULT_FLAGS = NoritoHeader.MINOR_VERSION;

  private static final ThreadLocal<Deque<Integer>> DECODE_FLAGS_STACK =
      ThreadLocal.withInitial(ArrayDeque::new);
  private static final ThreadLocal<Deque<Integer>> DECODE_FLAGS_HINT_STACK =
      ThreadLocal.withInitial(ArrayDeque::new);
  private static final ThreadLocal<Deque<ContextFlags>> ACTIVE_DECODE_CONTEXTS =
      ThreadLocal.withInitial(ArrayDeque::new);

  private static final ThreadLocal<Integer> LAST_ENCODE_FLAGS = new ThreadLocal<>();
  private static final ThreadLocal<byte[]> DECODE_ROOT_PAYLOAD = new ThreadLocal<>();

  private NoritoCodec() {}

  private static int applyAdaptiveFlags(int flags, int payloadLen) {
    return flags;
  }

  public static <T> byte[] encode(T value, String schemaPath, TypeAdapter<T> adapter) {
    return encode(value, schemaPath, adapter, DEFAULT_FLAGS, CompressionConfig.none());
  }

  public static <T> byte[] encode(
      T value, String schemaPath, TypeAdapter<T> adapter, int flags) {
    return encode(value, schemaPath, adapter, flags, CompressionConfig.none());
  }

  public static <T> byte[] encode(
      T value,
      String schemaPath,
      TypeAdapter<T> adapter,
      int flags,
      CompressionConfig compression) {
    Objects.requireNonNull(adapter);
    Objects.requireNonNull(schemaPath);
    Objects.requireNonNull(compression);
    NoritoEncoder encoder = new NoritoEncoder(flags);
    adapter.encode(encoder, value);
    byte[] payload = encoder.toByteArray();
    byte[] payloadBytes = payload;
    int compressionTag = NoritoHeader.COMPRESSION_NONE;
    if (compression.mode == NoritoHeader.COMPRESSION_ZSTD) {
      if (!NoritoCompression.hasZstd()) {
        throw new UnsupportedOperationException("Zstd compression requested but backend unavailable");
      }
      payloadBytes = NoritoCompression.compressZstd(payload, compression.level);
      compressionTag = NoritoHeader.COMPRESSION_ZSTD;
    } else if (compression.mode != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException("Unsupported compression tag: " + compression.mode);
    }
    long checksum = CRC64.compute(payload);
    byte[] schemaHash = SchemaHash.hash16(schemaPath);
    NoritoHeader header =
        new NoritoHeader(schemaHash, payload.length, checksum, flags, compressionTag);
    byte[] headerBytes = header.encode();
    byte[] out = new byte[headerBytes.length + payloadBytes.length];
    System.arraycopy(headerBytes, 0, out, 0, headerBytes.length);
    System.arraycopy(payloadBytes, 0, out, headerBytes.length, payloadBytes.length);
    LAST_ENCODE_FLAGS.set(flags & 0xFF);
    return out;
  }

  public static <T> AdaptiveEncoding encodeAdaptive(T value, TypeAdapter<T> adapter) {
    return encodeAdaptive(value, adapter, DEFAULT_FLAGS);
  }

  public static <T> AdaptiveEncoding encodeAdaptive(
      T value, TypeAdapter<T> adapter, int flags) {
    Objects.requireNonNull(adapter);
    NoritoEncoder encoder = new NoritoEncoder(flags);
    adapter.encode(encoder, value);
    byte[] payload = encoder.toByteArray();
    int finalFlags = applyAdaptiveFlags(flags, payload.length);
    if (finalFlags != flags) {
      encoder = new NoritoEncoder(finalFlags);
      adapter.encode(encoder, value);
      payload = encoder.toByteArray();
    }
    LAST_ENCODE_FLAGS.set(finalFlags & 0xFF);
    return new AdaptiveEncoding(payload, finalFlags & 0xFF);
  }

  public static <T> AdaptiveEncoding encodeWithHeaderFlags(T value, TypeAdapter<T> adapter) {
    return encodeAdaptive(value, adapter, DEFAULT_FLAGS);
  }

  public static Integer takeLastEncodeFlags() {
    Integer current = LAST_ENCODE_FLAGS.get();
    LAST_ENCODE_FLAGS.remove();
    return current;
  }

  private static int combineFlags(int flags, int hint) {
    int normalizedFlags = flags & 0xFF;
    return normalizedFlags;
  }

  public static <T> T decodeAdaptive(byte[] payload, TypeAdapter<T> adapter) {
    Objects.requireNonNull(payload);
    Objects.requireNonNull(adapter);
    Deque<Integer> stack = DECODE_FLAGS_STACK.get();
    int flags = stack.isEmpty() ? applyAdaptiveFlags(DEFAULT_FLAGS, payload.length) : stack.peekLast();
    T value;
    try (RootGuard guard = new RootGuard(payload, flags, NoritoHeader.MINOR_VERSION)) {
      guard.keepAlive();
      NoritoDecoder decoder = new NoritoDecoder(payload, flags, NoritoHeader.MINOR_VERSION);
      value = adapter.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Norito decode");
      }
    }
    return value;
  }

  public static void resetDecodeState() {
    DECODE_FLAGS_STACK.get().clear();
    DECODE_FLAGS_HINT_STACK.get().clear();
    ACTIVE_DECODE_CONTEXTS.get().clear();
    LAST_ENCODE_FLAGS.remove();
    DECODE_ROOT_PAYLOAD.remove();
  }

  public static Integer effectiveDecodeFlags() {
    Deque<ContextFlags> contexts = ACTIVE_DECODE_CONTEXTS.get();
    if (!contexts.isEmpty()) {
      ContextFlags ctx = contexts.peekLast();
      return combineFlags(ctx.flags(), ctx.hint());
    }
    Deque<Integer> stack = DECODE_FLAGS_STACK.get();
    if (!stack.isEmpty()) {
      Deque<Integer> hints = DECODE_FLAGS_HINT_STACK.get();
      int hint = hints.isEmpty() ? NoritoHeader.MINOR_VERSION : hints.peekLast();
      return combineFlags(stack.peekLast(), hint);
    }
    return null;
  }

  public static final class DecodeFlagsGuard implements AutoCloseable {
    private boolean active;

    private DecodeFlagsGuard(int flags, int hint) {
      Deque<Integer> stack = DECODE_FLAGS_STACK.get();
      stack.addLast(flags & 0xFF);
      Deque<Integer> hints = DECODE_FLAGS_HINT_STACK.get();
      hints.addLast(hint & 0xFF);
      this.active = true;
    }

    public static DecodeFlagsGuard enter(int flags) {
      return new DecodeFlagsGuard(flags, NoritoHeader.MINOR_VERSION);
    }

    public static DecodeFlagsGuard enterWithHint(int flags, int hint) {
      return new DecodeFlagsGuard(flags, hint);
    }

    @Override
    public void close() {
      if (active) {
        Deque<Integer> stack = DECODE_FLAGS_STACK.get();
        if (!stack.isEmpty()) {
          stack.removeLast();
        }
        Deque<Integer> hints = DECODE_FLAGS_HINT_STACK.get();
        if (!hints.isEmpty()) {
          hints.removeLast();
        }
        active = false;
      }
    }
  }

  public static final class AdaptiveEncoding {
    private final byte[] payload;
    private final int flags;

    public AdaptiveEncoding(byte[] payload, int flags) {
      this.payload = Objects.requireNonNull(payload);
      this.flags = flags & 0xFF;
    }

    public byte[] payload() {
      return payload;
    }

    public int flags() {
      return flags;
    }
  }

  public static ArchiveView fromBytesView(
      byte[] data, String schemaPath) {
    Objects.requireNonNull(data);
    resetDecodeState();
    byte[] expectedHash = schemaPath == null ? null : SchemaHash.hash16(schemaPath);
    NoritoHeader.DecodeResult result = NoritoHeader.decode(data, expectedHash);
    NoritoHeader header = result.header();
    if (header.compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException(
          "Archive views do not support compressed Norito payloads (found tag " + header.compression() + ")");
    }
    byte[] payload = result.payload();
    header.validateChecksum(payload);
    return new ArchiveView(payload, header.flags(), header.minor());
  }

  public static final class ArchiveView {
    private final byte[] payload;
    private final int flags;
    private final int flagsHint;

    ArchiveView(byte[] payload, int flags, int flagsHint) {
      this.payload = Objects.requireNonNull(payload);
      this.flags = flags & 0xFF;
      this.flagsHint = flagsHint & 0xFF;
    }

    public byte[] asBytes() {
      return Arrays.copyOf(payload, payload.length);
    }

    public int flags() {
      return flags;
    }

    public int flagsHint() {
      return flagsHint;
    }

    public <T> T decode(TypeAdapter<T> adapter) {
      Objects.requireNonNull(adapter);
      try (RootGuard guard = new RootGuard(payload, flags, flagsHint)) {
        guard.keepAlive();
        NoritoDecoder decoder = new NoritoDecoder(payload, flags, flagsHint);
        T value = adapter.decode(decoder);
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after Norito decode");
        }
        return value;
      }
    }
  }

  public static <T> T decode(
      byte[] data, TypeAdapter<T> adapter, String schemaPath) {
    Objects.requireNonNull(data);
    Objects.requireNonNull(adapter);
    byte[] expectedHash = schemaPath == null ? null : SchemaHash.hash16(schemaPath);
    NoritoHeader.DecodeResult result = NoritoHeader.decode(data, expectedHash);
    NoritoHeader header = result.header();
    byte[] payload = result.payload();
    byte[] decodedPayload;
    if (header.compression() == NoritoHeader.COMPRESSION_ZSTD) {
      decodedPayload = NoritoCompression.decompressZstd(payload, header.payloadLength());
    } else if (header.compression() == NoritoHeader.COMPRESSION_NONE) {
      decodedPayload = payload;
    } else {
      throw new IllegalArgumentException("Unsupported compression tag: " + header.compression());
    }
    header.validateChecksum(decodedPayload);
    T value;
    try (RootGuard guard = new RootGuard(decodedPayload, header.flags(), header.minor())) {
      guard.keepAlive();
      NoritoDecoder decoder =
          new NoritoDecoder(decodedPayload, header.flags(), header.minor());
      value = adapter.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Norito decode");
      }
    }
    return value;
  }

  public static <T> T decode(
      ByteBuffer data, TypeAdapter<T> adapter, String schemaPath) {
    Objects.requireNonNull(data);
    Objects.requireNonNull(adapter);
    byte[] expectedHash = schemaPath == null ? null : SchemaHash.hash16(schemaPath);
    NoritoHeader.DecodeView result = NoritoHeader.decodeView(data, expectedHash);
    NoritoHeader header = result.header();
    ByteBuffer payload = result.payload();
    if (header.compression() == NoritoHeader.COMPRESSION_ZSTD) {
      byte[] compressed = new byte[payload.remaining()];
      payload.get(compressed);
      byte[] decodedPayload = NoritoCompression.decompressZstd(compressed, header.payloadLength());
      header.validateChecksum(decodedPayload);
      T value;
      try (RootGuard guard = new RootGuard(decodedPayload, header.flags(), header.minor())) {
        guard.keepAlive();
        NoritoDecoder decoder =
            new NoritoDecoder(decodedPayload, header.flags(), header.minor());
        value = adapter.decode(decoder);
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after Norito decode");
        }
      }
      return value;
    }
    if (header.compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException("Unsupported compression tag: " + header.compression());
    }
    header.validateChecksum(payload);
    T value;
    try (RootGuard guard = new RootGuard(payload, header.flags(), header.minor())) {
      guard.keepAlive();
      NoritoDecoder decoder = new NoritoDecoder(payload, header.flags(), header.minor());
      value = adapter.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Norito decode");
      }
    }
    return value;
  }


  /**
   * Fallible decode helper mirroring Rust's {@code NoritoDeserialize::try_deserialize}.
   *
   * <p>Returns {@link Result.Ok} when decoding succeeds or {@link Result.Err} carrying the runtime
   * exception describing the failure. This avoids having to rely on exception control-flow at the
   * call site.
   */
  public static <T> Result<T, RuntimeException> tryDecode(
      byte[] data, TypeAdapter<T> adapter, String schemaPath) {
    try {
      return new Result.Ok<>(decode(data, adapter, schemaPath));
    } catch (RuntimeException ex) {
      return new Result.Err<>(ex);
    }
  }

  public static <T> Result<T, RuntimeException> tryDecode(
      ByteBuffer data, TypeAdapter<T> adapter, String schemaPath) {
    try {
      return new Result.Ok<>(decode(data, adapter, schemaPath));
    } catch (RuntimeException ex) {
      return new Result.Err<>(ex);
    }
  }


  public static final class CompressionConfig {
    private static final int ZSTD_MIN_LEVEL = 1;
    private static final int ZSTD_MAX_LEVEL = 22;

    public enum CompressionProfile {
      FAST(
          new LevelBucket[] {
            new LevelBucket(0, 64 * 1024, 1),
            new LevelBucket(64 * 1024, 512 * 1024, 2),
            new LevelBucket(512 * 1024, 4 * 1024 * 1024, 3),
            new LevelBucket(4 * 1024 * 1024, Integer.MAX_VALUE, 4)
          }),
      BALANCED(
          new LevelBucket[] {
            new LevelBucket(0, 64 * 1024, 3),
            new LevelBucket(64 * 1024, 512 * 1024, 5),
            new LevelBucket(512 * 1024, 4 * 1024 * 1024, 7),
            new LevelBucket(4 * 1024 * 1024, Integer.MAX_VALUE, 9)
          }),
      COMPACT(
          new LevelBucket[] {
            new LevelBucket(0, 64 * 1024, 7),
            new LevelBucket(64 * 1024, 512 * 1024, 11),
            new LevelBucket(512 * 1024, 4 * 1024 * 1024, 15),
            new LevelBucket(4 * 1024 * 1024, Integer.MAX_VALUE, 19)
          });

      private final LevelBucket[] buckets;

      CompressionProfile(LevelBucket[] buckets) {
        this.buckets = buckets;
      }

      int resolveLevel(int payloadBytes) {
        if (payloadBytes < 0) {
          throw new IllegalArgumentException("payloadBytes must be non-negative");
        }
        for (LevelBucket bucket : buckets) {
          if (bucket.matches(payloadBytes)) {
            return clampLevel(bucket.level());
          }
        }
        return clampLevel(buckets[buckets.length - 1].level());
      }

      static CompressionProfile fromString(String value) {
        Objects.requireNonNull(value, "profile");
        try {
          return CompressionProfile.valueOf(value.trim().toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException ex) {
          throw new IllegalArgumentException("Unknown compression profile: " + value, ex);
        }
      }
    }

    private record LevelBucket(int lowerInclusive, int upperExclusive, int level) {
      boolean matches(int payloadBytes) {
        return payloadBytes >= lowerInclusive && payloadBytes < upperExclusive;
      }
    }

    private final int mode;
    private final int level;

    private CompressionConfig(int mode, int level) {
      this.mode = mode;
      this.level = level;
    }

    public static CompressionConfig none() {
      return new CompressionConfig(NoritoHeader.COMPRESSION_NONE, 0);
    }

    public static CompressionConfig zstd(int level) {
      return new CompressionConfig(NoritoHeader.COMPRESSION_ZSTD, clampLevel(level));
    }

    public static CompressionConfig zstdProfile(String profile, int payloadBytes) {
      return zstdProfile(CompressionProfile.fromString(profile), payloadBytes);
    }

    public static CompressionConfig zstdProfile(CompressionProfile profile, int payloadBytes) {
      Objects.requireNonNull(profile, "profile");
      int resolvedLevel = profile.resolveLevel(payloadBytes);
      return new CompressionConfig(NoritoHeader.COMPRESSION_ZSTD, resolvedLevel);
    }

    public int mode() {
      return mode;
    }

    public int level() {
      return level;
    }

    private static int clampLevel(int level) {
      if (level < ZSTD_MIN_LEVEL || level > ZSTD_MAX_LEVEL) {
        throw new IllegalArgumentException(
            "Zstandard level " + level + " must be between " + ZSTD_MIN_LEVEL + " and " + ZSTD_MAX_LEVEL);
      }
      return level;
    }
  }

  private static final class RootGuard implements AutoCloseable {
    private final boolean installed;
    private final boolean contextPushed;

    RootGuard(byte[] payload) {
      this(payload, null, null);
    }

    RootGuard(byte[] payload, Integer flags, Integer hint) {
      Objects.requireNonNull(payload, "payload");
      if (DECODE_ROOT_PAYLOAD.get() == null) {
        DECODE_ROOT_PAYLOAD.set(Arrays.copyOf(payload, payload.length));
        installed = true;
      } else {
        installed = false;
      }
      if (flags != null) {
        int normalizedHint = hint == null ? NoritoHeader.MINOR_VERSION : hint & 0xFF;
        ACTIVE_DECODE_CONTEXTS
            .get()
            .addLast(new ContextFlags(flags & 0xFF, normalizedHint));
        contextPushed = true;
      } else {
        contextPushed = false;
      }
    }

    RootGuard(ByteBuffer payload, Integer flags, Integer hint) {
      Objects.requireNonNull(payload, "payload");
      if (DECODE_ROOT_PAYLOAD.get() == null) {
        ByteBuffer view = payload.slice();
        byte[] bytes = new byte[view.remaining()];
        view.get(bytes);
        DECODE_ROOT_PAYLOAD.set(bytes);
        installed = true;
      } else {
        installed = false;
      }
      if (flags != null) {
        int normalizedHint = hint == null ? NoritoHeader.MINOR_VERSION : hint & 0xFF;
        ACTIVE_DECODE_CONTEXTS
            .get()
            .addLast(new ContextFlags(flags & 0xFF, normalizedHint));
        contextPushed = true;
      } else {
        contextPushed = false;
      }
    }

    void keepAlive() {
      // Intentionally empty; referenced by callers to silence -Xlint:try.
    }

    @Override
    public void close() {
      if (contextPushed) {
        Deque<ContextFlags> contexts = ACTIVE_DECODE_CONTEXTS.get();
        if (!contexts.isEmpty()) {
          contexts.removeLast();
        }
      }
      if (installed) {
        DECODE_ROOT_PAYLOAD.remove();
      }
    }
  }

  public static byte[] payloadRootBytes() {
    byte[] current = DECODE_ROOT_PAYLOAD.get();
    return current == null ? null : Arrays.copyOf(current, current.length);
  }

  private record ContextFlags(int flags, int hint) {}
}
