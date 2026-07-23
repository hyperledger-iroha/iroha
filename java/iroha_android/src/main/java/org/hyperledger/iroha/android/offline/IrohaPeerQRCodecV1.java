package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Canonical RFC 9285 Base45 IQR1 codec and fixed 256-byte shard encoder. */
public final class IrohaPeerQRCodecV1 {
  public static final String TEXT_PREFIX = "IQR1:";
  public static final String TEXT_SUFFIX = ":";
  public static final int MAXIMUM_FRAME_TEXT_BYTES = 700;
  public static final int SHARD_BYTES = 256;
  public static final int PARITY_GROUP = 2;
  public static final int HEADER_REPEAT_INTERVAL = 12;

  private IrohaPeerQRCodecV1() {}

  public static List<String> encode(final IrohaPeerCanonicalPayload payload) {
    return encode(new IrohaPeerWireMessageV1(
        payload, IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED));
  }

  public static List<String> encode(final IrohaPeerWireMessageV1 message) {
    Objects.requireNonNull(message, "message");
    final String complete = staticCompleteTextCandidate(message);
    return complete == null ? animatedFrameTexts(message) : Collections.singletonList(complete);
  }

  public static String staticCompleteTextCandidate(final IrohaPeerWireMessageV1 message) {
    Objects.requireNonNull(message, "message");
    final byte[] encodedMessage = message.encode();
    try {
      final IrohaPeerQRFrameV1 complete = new IrohaPeerQRFrameV1(
          IrohaPeerQRFrameV1.FrameKind.COMPLETE,
          message.canonicalPayload().profile(),
          message.canonicalPayload().kind(),
          message.streamId(), 0, 1, encodedMessage);
      final String text = encodeFrame(complete);
      return text.getBytes(StandardCharsets.UTF_8).length <= MAXIMUM_FRAME_TEXT_BYTES
          ? text : null;
    } finally {
      Arrays.fill(encodedMessage, (byte) 0);
    }
  }

  public static List<String> animatedFrameTexts(final IrohaPeerWireMessageV1 message) {
    Objects.requireNonNull(message, "message");
    final byte[] body = message.encodedBody();
    final byte[] encodedMessage = message.encode();
    try {
      final int dataCount = (body.length + SHARD_BYTES - 1) / SHARD_BYTES;
      require(dataCount >= 1 && dataCount <= 0xffff,
          "Peer message cannot be represented as QR");
      final List<byte[]> shards = new ArrayList<>(dataCount);
      for (int index = 0; index < dataCount; index++) {
        final byte[] shard = new byte[SHARD_BYTES];
        final int start = index * SHARD_BYTES;
        System.arraycopy(body, start, shard, 0, Math.min(SHARD_BYTES, body.length - start));
        shards.add(shard);
      }
      final IrohaPeerQRFrameV1 header = new IrohaPeerQRFrameV1(
          IrohaPeerQRFrameV1.FrameKind.HEADER,
          message.canonicalPayload().profile(), message.canonicalPayload().kind(),
          message.streamId(), 0, dataCount,
          Arrays.copyOf(encodedMessage, IrohaPeerWireMessageV1.HEADER_LENGTH));
      final List<IrohaPeerQRFrameV1> frames = new ArrayList<>();
      frames.add(header);
      int nonHeaderCount = 0;
      for (int pairIndex = 0; pairIndex < (dataCount + 1) / 2; pairIndex++) {
        final int firstIndex = pairIndex * 2;
        frames.add(new IrohaPeerQRFrameV1(
            IrohaPeerQRFrameV1.FrameKind.DATA,
            message.canonicalPayload().profile(), message.canonicalPayload().kind(),
            message.streamId(), firstIndex, dataCount, shards.get(firstIndex)));
        if (++nonHeaderCount % HEADER_REPEAT_INTERVAL == 0) frames.add(header);
        if (firstIndex + 1 < dataCount) {
          frames.add(new IrohaPeerQRFrameV1(
              IrohaPeerQRFrameV1.FrameKind.DATA,
              message.canonicalPayload().profile(), message.canonicalPayload().kind(),
              message.streamId(), firstIndex + 1, dataCount, shards.get(firstIndex + 1)));
          if (++nonHeaderCount % HEADER_REPEAT_INTERVAL == 0) frames.add(header);
        }
        final byte[] parity = shards.get(firstIndex).clone();
        if (firstIndex + 1 < dataCount) {
          final byte[] second = shards.get(firstIndex + 1);
          for (int byteIndex = 0; byteIndex < SHARD_BYTES; byteIndex++) {
            parity[byteIndex] ^= second[byteIndex];
          }
        }
        frames.add(new IrohaPeerQRFrameV1(
            IrohaPeerQRFrameV1.FrameKind.PARITY,
            message.canonicalPayload().profile(), message.canonicalPayload().kind(),
            message.streamId(), pairIndex, dataCount, parity));
        if (++nonHeaderCount % HEADER_REPEAT_INTERVAL == 0) frames.add(header);
        Arrays.fill(parity, (byte) 0);
      }
      final List<String> encoded = new ArrayList<>(frames.size());
      for (final IrohaPeerQRFrameV1 frame : frames) encoded.add(encodeFrame(frame));
      for (final byte[] shard : shards) Arrays.fill(shard, (byte) 0);
      return Collections.unmodifiableList(encoded);
    } finally {
      Arrays.fill(body, (byte) 0);
      Arrays.fill(encodedMessage, (byte) 0);
    }
  }

  public static String encodeFrame(final IrohaPeerQRFrameV1 frame) {
    final byte[] bytes = Objects.requireNonNull(frame, "frame").encode();
    try {
      final String value = TEXT_PREFIX + Base45.encode(bytes) + TEXT_SUFFIX;
      return value;
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  public static IrohaPeerQRFrameV1 decodeFrame(final String value) {
    Objects.requireNonNull(value, "value");
    require(value.getBytes(StandardCharsets.UTF_8).length <= MAXIMUM_FRAME_TEXT_BYTES
            && value.startsWith(TEXT_PREFIX)
            && value.endsWith(TEXT_SUFFIX)
            && value.length() > TEXT_PREFIX.length() + TEXT_SUFFIX.length(),
        "Malformed IQR1 text");
    final String body = value.substring(TEXT_PREFIX.length(), value.length() - TEXT_SUFFIX.length());
    final byte[] bytes = Base45.decode(body);
    require(bytes != null, "IQR1 body is not canonical Base45");
    try {
      require((TEXT_PREFIX + Base45.encode(bytes) + TEXT_SUFFIX).equals(value),
          "IQR1 body is not canonical Base45");
      return IrohaPeerQRFrameV1.decode(bytes);
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }

  private static final class Base45 {
    private static final byte[] ALPHABET =
        "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:".getBytes(StandardCharsets.US_ASCII);
    private static final int[] REVERSE = reverseTable();

    private Base45() {}

    static String encode(final byte[] data) {
      final byte[] output = new byte[(data.length / 2) * 3 + (data.length % 2) * 2];
      int source = 0;
      int target = 0;
      while (source + 1 < data.length) {
        int number = (data[source] & 0xff) * 256 + (data[source + 1] & 0xff);
        output[target++] = ALPHABET[number % 45];
        number /= 45;
        output[target++] = ALPHABET[number % 45];
        output[target++] = ALPHABET[number / 45];
        source += 2;
      }
      if (source < data.length) {
        final int number = data[source] & 0xff;
        output[target++] = ALPHABET[number % 45];
        output[target] = ALPHABET[number / 45];
      }
      return new String(output, StandardCharsets.US_ASCII);
    }

    static byte[] decode(final String value) {
      final byte[] input = value.getBytes(StandardCharsets.US_ASCII);
      if (input.length == 0 || input.length % 3 == 1 || input.length != value.length()) return null;
      final byte[] output =
          new byte[(input.length / 3) * 2 + (input.length % 3 == 2 ? 1 : 0)];
      int source = 0;
      int target = 0;
      while (source + 2 < input.length) {
        final int a = digit(input[source]);
        final int b = digit(input[source + 1]);
        final int c = digit(input[source + 2]);
        if (a < 0 || b < 0 || c < 0) return null;
        final int decoded = a + b * 45 + c * 2025;
        if (decoded > 0xffff) return null;
        output[target++] = (byte) (decoded / 256);
        output[target++] = (byte) decoded;
        source += 3;
      }
      if (source < input.length) {
        final int a = digit(input[source]);
        final int b = digit(input[source + 1]);
        if (a < 0 || b < 0) return null;
        final int decoded = a + b * 45;
        if (decoded > 0xff) return null;
        output[target] = (byte) decoded;
      }
      return output;
    }

    private static int digit(final byte value) {
      final int code = value & 0xff;
      return code < REVERSE.length ? REVERSE[code] : -1;
    }

    private static int[] reverseTable() {
      final int[] table = new int[128];
      Arrays.fill(table, -1);
      for (int index = 0; index < ALPHABET.length; index++) table[ALPHABET[index]] = index;
      return table;
    }
  }
}
