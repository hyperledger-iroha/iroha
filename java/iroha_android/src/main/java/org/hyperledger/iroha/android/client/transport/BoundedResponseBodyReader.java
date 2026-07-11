package org.hyperledger.iroha.android.client.transport;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Strict, size-bounded reader for buffered HTTP response bodies. */
public final class BoundedResponseBodyReader {

  /** Default maximum body size buffered by transport executors, in bytes (64 MiB). */
  public static final long DEFAULT_MAXIMUM_RESPONSE_BYTES = 64L * 1024L * 1024L;

  private BoundedResponseBodyReader() {}

  /**
   * Reads a complete response body while enforcing canonical framing and an actual byte limit.
   *
   * @param input response stream, or {@code null} when the response has no stream
   * @param headers response headers
   * @param maximumResponseBytes inclusive maximum number of bytes to buffer
   * @return the complete body
   * @throws IOException if framing is malformed, the body is truncated, or the limit is exceeded
   */
  public static byte[] read(
      final InputStream input,
      final Map<String, List<String>> headers,
      final long maximumResponseBytes)
      throws IOException {
    return read(input, headers, maximumResponseBytes, true);
  }

  /**
   * Reads a complete response body with explicit HTTP body-presence semantics.
   *
   * @param input response stream, or {@code null} when the response has no stream
   * @param headers response headers
   * @param maximumResponseBytes inclusive maximum number of bytes to buffer
   * @param bodyExpected whether the request method and response status permit a body
   * @return the complete body, or an empty body when HTTP semantics prohibit one
   * @throws IOException if framing is malformed, the body is truncated, or the limit is exceeded
   */
  public static byte[] read(
      final InputStream input,
      final Map<String, List<String>> headers,
      final long maximumResponseBytes,
      final boolean bodyExpected)
      throws IOException {
    final Long declaredLength;
    final boolean contentLengthMatchesBufferedBody;
    try {
      validateMaximum(maximumResponseBytes);
      declaredLength = canonicalContentLength(headers);
      contentLengthMatchesBufferedBody = contentLengthMatchesBufferedBody(headers);
      if (bodyExpected
          && contentLengthMatchesBufferedBody
          && declaredLength != null
          && declaredLength > maximumResponseBytes) {
        throw new IOException(
            "HTTP response Content-Length "
                + declaredLength
                + " exceeds the "
                + maximumResponseBytes
                + "-byte limit");
      }
      rejectAmbiguousFraming(headers, declaredLength);
    } catch (final IOException | RuntimeException failure) {
      closeAfterRejection(input, failure);
      throw failure;
    }
    if (!bodyExpected) {
      if (input != null) {
        input.close();
      }
      return new byte[0];
    }
    if (input == null) {
      if (declaredLength != null && declaredLength != 0L) {
        throw new IOException(
            "HTTP response ended before its " + declaredLength + "-byte Content-Length");
      }
      return new byte[0];
    }
    return readBounded(
        input,
        maximumResponseBytes,
        contentLengthMatchesBufferedBody ? declaredLength : null);
  }

  static byte[] readBounded(
      final InputStream input,
      final long maximumResponseBytes,
      final Long declaredLength)
      throws IOException {
    Objects.requireNonNull(input, "input");
    validateMaximum(maximumResponseBytes);
    final long capacityHint = declaredLength == null ? 32L : declaredLength;
    final int initialCapacity = (int) Math.min(Math.min(capacityHint, maximumResponseBytes), 8192L);
    try (InputStream responseBody = input;
        ByteArrayOutputStream buffer = new ByteArrayOutputStream(initialCapacity)) {
      final byte[] chunk = new byte[8192];
      long total = 0L;
      while (true) {
        final long remaining = maximumResponseBytes - total;
        final int requested = (int) Math.min(chunk.length, remaining + 1L);
        final int count = responseBody.read(chunk, 0, requested);
        if (count == -1) {
          break;
        }
        if (count < -1 || count > requested) {
          throw new IOException("HTTP response body stream returned an invalid read count");
        }
        if (count == 0) {
          throw new IOException("HTTP response body stream made no read progress");
        }
        if (count > remaining) {
          throw new IOException(
              "HTTP response body exceeds the " + maximumResponseBytes + "-byte limit");
        }
        buffer.write(chunk, 0, count);
        total += count;
      }
      if (declaredLength != null && total != declaredLength) {
        throw new IOException(
            "HTTP response body length "
                + total
                + " does not match Content-Length "
                + declaredLength);
      }
      return buffer.toByteArray();
    }
  }

  static long parseCanonicalContentLength(final String value) throws IOException {
    if (value == null || value.isEmpty() || (value.length() > 1 && value.charAt(0) == '0')) {
      throw invalidContentLength();
    }
    long parsed = 0L;
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') {
        throw invalidContentLength();
      }
      final int digit = character - '0';
      if (parsed > (Long.MAX_VALUE - digit) / 10L) {
        throw new IOException("HTTP response Content-Length exceeds the supported range");
      }
      parsed = parsed * 10L + digit;
    }
    return parsed;
  }

  /** Validates a configured buffered-response limit. */
  public static void validateMaximum(final long maximumResponseBytes) {
    if (maximumResponseBytes < 1L || maximumResponseBytes > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(
          "maximumResponseBytes must be between 1 and " + Integer.MAX_VALUE);
    }
  }

  private static Long canonicalContentLength(final Map<String, List<String>> headers)
      throws IOException {
    if (headers == null) {
      return null;
    }
    String value = null;
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      final String name = entry.getKey();
      if (name == null || !name.equalsIgnoreCase("Content-Length")) {
        continue;
      }
      final List<String> values = entry.getValue();
      if (value != null || values == null || values.size() != 1) {
        throw new IOException("HTTP response must contain at most one Content-Length value");
      }
      value = values.get(0);
    }
    return value == null ? null : parseCanonicalContentLength(value);
  }

  private static void rejectAmbiguousFraming(
      final Map<String, List<String>> headers, final Long declaredLength) throws IOException {
    if (headers == null || declaredLength == null) {
      return;
    }
    for (final String name : headers.keySet()) {
      if (name != null && name.equalsIgnoreCase("Transfer-Encoding")) {
        throw new IOException(
            "HTTP response must not combine Content-Length with Transfer-Encoding");
      }
    }
  }

  private static boolean contentLengthMatchesBufferedBody(
      final Map<String, List<String>> headers) {
    if (headers == null) {
      return true;
    }
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      final String name = entry.getKey();
      if (name == null || !name.equalsIgnoreCase("Content-Encoding")) {
        continue;
      }
      final List<String> values = entry.getValue();
      if (values == null || values.isEmpty()) {
        return false;
      }
      for (final String value : values) {
        if (value == null) {
          return false;
        }
        final String[] encodings = value.split(",", -1);
        for (final String encoding : encodings) {
          if (!encoding.trim().equalsIgnoreCase("identity")) {
            return false;
          }
        }
      }
    }
    return true;
  }

  private static IOException invalidContentLength() {
    return new IOException(
        "HTTP response Content-Length must be a canonical unsigned decimal");
  }

  private static void closeAfterRejection(final InputStream input, final Throwable failure) {
    if (input == null) {
      return;
    }
    try {
      input.close();
    } catch (final IOException closeFailure) {
      failure.addSuppressed(closeFailure);
    }
  }
}
