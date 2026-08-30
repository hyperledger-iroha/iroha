package org.hyperledger.iroha.android.client.transport;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.SocketTimeoutException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.Test;

public final class UrlConnectionTransportExecutorTests {

  @Test
  public void credentialFreeRequestFailsClosedBeforeOpeningAConnection() throws Exception {
    try (ServerSocket server = loopbackServer()) {
      server.setSoTimeout(500);
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(URI.create("http://127.0.0.1:" + server.getLocalPort() + "/v1/zk/vk"))
              .setAllowAmbientCredentials(false)
              .build();
      try {
        new UrlConnectionTransportExecutor().execute(request).get();
        fail("credential-free URLConnection request must fail closed");
      } catch (final ExecutionException expected) {
        assertTrue(hasCauseMessage(expected, "unsupported by URLConnection"));
      }
      try {
        server.accept().close();
        fail("credential-free request must fail before opening the connection");
      } catch (final SocketTimeoutException expected) {
        // Expected: URLConnection cannot isolate its authentication cache, so no socket opens.
      }
    }
  }

  @Test
  public void credentialFreeStreamingFailsClosed() throws Exception {
    final TransportRequest request =
        TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("http://127.0.0.1:1/v1/zk/vk"))
            .setAllowAmbientCredentials(false)
            .build();
    try {
      new UrlConnectionTransportExecutor().openStream(request).get();
      fail("credential-free URLConnection stream must fail closed");
    } catch (final ExecutionException expected) {
      assertTrue(hasCauseMessage(expected, "unsupported by URLConnection"));
    }
  }

  @Test
  public void executeReturns404WithEmptyBodyWhenServerSendsNoContent() throws Exception {
    try (ServerSocket server = loopbackServer()) {
      final int port = server.getLocalPort();
      final Thread serverThread =
          new Thread(
              () -> {
                try (Socket socket = server.accept()) {
                  final InputStream input = socket.getInputStream();
                  final StringBuilder sb = new StringBuilder();
                  while (true) {
                    final int b = input.read();
                    if (b == -1) break;
                    sb.append((char) b);
                    if (sb.toString().endsWith("\r\n\r\n")) break;
                  }
                  final OutputStream out = socket.getOutputStream();
                  out.write(
                      "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                          .getBytes(StandardCharsets.UTF_8));
                  out.flush();
                } catch (final Exception ignored) {
                }
              });
      serverThread.setDaemon(true);
      serverThread.start();

      final UrlConnectionTransportExecutor executor = new UrlConnectionTransportExecutor();
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("POST")
              .setUri(URI.create("http://127.0.0.1:" + port + "/v1/aliases/resolve"))
              .addHeader("Content-Type", "application/json")
              .setBody("{\"alias\":\"missing@test\"}".getBytes(StandardCharsets.UTF_8))
              .build();

      final TransportResponse response = executor.execute(request).get();

      assertEquals("Status code should be 404", 404, response.statusCode());
      assertEquals("Body should be empty for 404 with Content-Length: 0", 0, response.body().length);

      serverThread.join(2000);
    }
  }

  @Test
  public void permanentRedirectsAreReturnedWithoutForwardingSensitiveHeaders() throws Exception {
    assertRedirectIsNotFollowed(307);
    assertRedirectIsNotFollowed(308);
  }

  private static void assertRedirectIsNotFollowed(final int status) throws Exception {
    try (ServerSocket server = loopbackServer()) {
      server.setSoTimeout(1_000);
      final int port = server.getLocalPort();
      final AtomicReference<String> redirectedRequest = new AtomicReference<>();
      final Thread serverThread =
          new Thread(
              () -> {
                try (Socket socket = server.accept()) {
                  readHeaders(socket.getInputStream());
                  final OutputStream output = socket.getOutputStream();
                  output.write(
                      ("HTTP/1.1 " + status + " Redirect\r\n"
                              + "Location: http://127.0.0.1:"
                              + port
                              + "/redirected\r\n"
                              + "Content-Length: 0\r\nConnection: close\r\n\r\n")
                          .getBytes(StandardCharsets.UTF_8));
                  output.flush();
                } catch (final IOException ignored) {
                  return;
                }
                try (Socket socket = server.accept()) {
                  redirectedRequest.set(readHeaders(socket.getInputStream()));
                  final OutputStream output = socket.getOutputStream();
                  output.write(
                      "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                          .getBytes(StandardCharsets.UTF_8));
                  output.flush();
                } catch (final SocketTimeoutException expected) {
                  // Expected: the executor must expose the redirect without following it.
                } catch (final IOException ignored) {
                  // The assertion below still detects an unexpected redirected request.
                }
              });
      serverThread.setDaemon(true);
      serverThread.start();
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(URI.create("http://127.0.0.1:" + port + "/original"))
              .addHeader("X-Iroha-Onboarding-Token", "sensitive-runtime-token")
              .build();

      final TransportResponse response =
          new UrlConnectionTransportExecutor().execute(request).get();

      assertEquals(status, response.statusCode());
      serverThread.join(3_000);
      assertEquals(
          "redirect target must not receive a second request", null, redirectedRequest.get());
    }
  }

  @Test
  public void bufferedResponseAcceptsExactConfiguredLimit() throws Exception {
    final TransportResponse response =
        executeRawResponse(
            "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678",
            8L);

    assertArrayEquals("12345678".getBytes(StandardCharsets.UTF_8), response.body());
  }

  private static String readHeaders(final InputStream input) throws IOException {
    final StringBuilder result = new StringBuilder();
    while (!result.toString().endsWith("\r\n\r\n")) {
      final int next = input.read();
      if (next == -1) break;
      result.append((char) next);
    }
    return result.toString();
  }

  @Test
  public void headResponseDoesNotTreatRepresentationLengthAsBufferedBodyLength()
      throws Exception {
    final TransportResponse response =
        executeRawResponse(
            "HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\n",
            8L,
            "HEAD");

    assertEquals(0, response.body().length);
  }

  @Test
  public void bufferedResponseRejectsDeclaredLengthAboveLimit() throws Exception {
    final ExecutionException failure =
        expectExecutionFailure(
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
            8L);

    assertTrue(hasCauseMessage(failure, "Content-Length 9 exceeds the 8-byte limit"));
  }

  @Test
  public void declaredOverflowClosesStreamWithoutReadingBody() throws Exception {
    final TrackingInputStream input = new TrackingInputStream();
    try {
      BoundedResponseBodyReader.read(
          input, Map.of("Content-Length", List.of("9")), 8L);
      fail("Declared overflow must be rejected");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("Content-Length 9 exceeds"));
    }

    assertTrue("Rejected response stream must be closed", input.closed);
    assertEquals("Declared overflow must be rejected before reading", 0, input.readCalls);
  }

  @Test
  public void bodylessHttpSemanticsCloseStreamWithoutApplyingRepresentationLength() throws Exception {
    final TrackingInputStream input = new TrackingInputStream();
    final byte[] body =
        BoundedResponseBodyReader.read(
            input, Map.of("Content-Length", List.of("99")), 8L, false);

    assertEquals(0, body.length);
    assertTrue("Bodyless response stream must be closed", input.closed);
    assertEquals("Bodyless response must not be read", 0, input.readCalls);
  }

  @Test
  public void bufferedResponseRejectsActualOverflowWithoutContentLength() throws Exception {
    final ExecutionException failure =
        expectExecutionFailure(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n123456789", 8L);

    assertTrue(hasCauseMessage(failure, "body exceeds the 8-byte limit"));
  }

  @Test
  public void perRequestLimitAppliesToDeclaredAndActualNonSuccessBodies() throws Exception {
    final ExecutionException declared =
        expectExecutionFailure(
            "HTTP/1.1 500 Error\r\n"
                + "Content-Length: 9\r\nConnection: close\r\n\r\n123456789",
            64L,
            8L);
    assertTrue(hasCauseMessage(declared, "Content-Length 9 exceeds the 8-byte limit"));

    final ExecutionException actual =
        expectExecutionFailure(
            "HTTP/1.1 500 Error\r\nConnection: close\r\n\r\n123456789", 64L, 8L);
    assertTrue(hasCauseMessage(actual, "body exceeds the 8-byte limit"));
  }

  @Test
  public void perRequestLimitCannotRaiseExecutorMaximum() throws Exception {
    final ExecutionException failure =
        expectExecutionFailure(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n123456789", 8L, 16L);
    assertTrue(hasCauseMessage(failure, "body exceeds the 8-byte limit"));
  }

  @Test
  public void boundedReaderRejectsActualOverflowWithUnderstatedContentLength() throws Exception {
    try {
      BoundedResponseBodyReader.readBounded(
          new ByteArrayInputStream("123456789".getBytes(StandardCharsets.UTF_8)), 8L, 1L);
      fail("Understated Content-Length must not bypass the actual byte limit");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("body exceeds the 8-byte limit"));
    }
  }

  @Test
  public void boundedReaderRejectsTruncatedDeclaredBody() throws Exception {
    try {
      BoundedResponseBodyReader.readBounded(
          new ByteArrayInputStream("1234".getBytes(StandardCharsets.UTF_8)), 8L, 5L);
      fail("Truncated declared body must be rejected");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("does not match Content-Length 5"));
    }
  }

  @Test
  public void boundedReaderRejectsHostileInputStreamReadCounts() throws Exception {
    final InputStream noProgress =
        new InputStream() {
          @Override
          public int read() {
            return 0;
          }

          @Override
          public int read(final byte[] bytes, final int offset, final int length) {
            return 0;
          }
        };
    try {
      BoundedResponseBodyReader.readBounded(noProgress, 8L, null);
      fail("A no-progress stream must be rejected");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("made no read progress"));
    }

    final InputStream overReported =
        new InputStream() {
          @Override
          public int read() {
            return 0;
          }

          @Override
          public int read(final byte[] bytes, final int offset, final int length) {
            return length + 1;
          }
        };
    try {
      BoundedResponseBodyReader.readBounded(overReported, 8L, null);
      fail("An over-reported stream read count must be rejected");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("invalid read count"));
    }
  }

  @Test
  public void encodedContentLengthIsNotComparedToDecodedBufferedBytes() throws Exception {
    final Map<String, List<String>> headers =
        Map.of("Content-Length", List.of("1"), "Content-Encoding", List.of("gzip"));
    final byte[] exact =
        BoundedResponseBodyReader.read(
            new ByteArrayInputStream("12345678".getBytes(StandardCharsets.UTF_8)),
            headers,
            8L);
    assertArrayEquals("12345678".getBytes(StandardCharsets.UTF_8), exact);

    try {
      BoundedResponseBodyReader.read(
          new ByteArrayInputStream("123456789".getBytes(StandardCharsets.UTF_8)),
          headers,
          8L);
      fail("Decoded/body-buffer expansion must still obey the actual byte limit");
    } catch (final IOException expected) {
      assertTrue(expected.getMessage().contains("body exceeds the 8-byte limit"));
    }
  }

  @Test
  public void contentLengthMustBeCanonicalUnsignedDecimal() throws Exception {
    assertEquals(0L, BoundedResponseBodyReader.parseCanonicalContentLength("0"));
    assertEquals(8L, BoundedResponseBodyReader.parseCanonicalContentLength("8"));
    assertEquals(
        Long.MAX_VALUE,
        BoundedResponseBodyReader.parseCanonicalContentLength(Long.toString(Long.MAX_VALUE)));

    final String[] invalid = {
      "", "-1", "+1", "01", " 1", "1 ", "1,1", "1x", "١", "9223372036854775808"
    };
    for (final String value : invalid) {
      try {
        BoundedResponseBodyReader.parseCanonicalContentLength(value);
        fail("Non-canonical Content-Length must be rejected: " + value);
      } catch (final IOException expected) {
        // Expected.
      }
    }
  }

  @Test
  public void bufferedResponseRejectsAmbiguousLengthAndTransferEncoding() throws Exception {
    final ExecutionException failure =
        expectExecutionFailure(
            "HTTP/1.1 200 OK\r\n"
                + "Content-Length: 1\r\n"
                + "Transfer-Encoding: chunked\r\n"
                + "Connection: close\r\n\r\n"
                + "1\r\na\r\n0\r\n\r\n",
            8L);

    assertTrue(
        hasCauseMessage(
            failure, "must not combine Content-Length with Transfer-Encoding"));
  }

  @Test
  public void bufferedResponseLimitMustFitInByteArray() {
    expectInvalidLimit(0L);
    expectInvalidLimit((long) Integer.MAX_VALUE + 1L);
    assertEquals(
        64L * 1024L * 1024L,
        BoundedResponseBodyReader.DEFAULT_MAXIMUM_RESPONSE_BYTES);
    try {
      TransportRequest.builder().setMaximumResponseBytes(0L);
      fail("Invalid per-request response limit must be rejected");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static ExecutionException expectExecutionFailure(
      final String rawResponse, final long maximumResponseBytes) throws Exception {
    try {
      executeRawResponse(rawResponse, maximumResponseBytes);
      fail("Response should have been rejected");
      throw new AssertionError("unreachable");
    } catch (final ExecutionException expected) {
      return expected;
    }
  }

  private static ExecutionException expectExecutionFailure(
      final String rawResponse,
      final long maximumResponseBytes,
      final long requestMaximumResponseBytes)
      throws Exception {
    try {
      executeRawResponse(
          rawResponse, maximumResponseBytes, "GET", requestMaximumResponseBytes);
      fail("Response should have been rejected");
      throw new AssertionError("unreachable");
    } catch (final ExecutionException expected) {
      return expected;
    }
  }

  private static TransportResponse executeRawResponse(
      final String rawResponse, final long maximumResponseBytes) throws Exception {
    return executeRawResponse(rawResponse, maximumResponseBytes, "GET", null);
  }

  private static TransportResponse executeRawResponse(
      final String rawResponse,
      final long maximumResponseBytes,
      final String requestMethod)
      throws Exception {
    return executeRawResponse(rawResponse, maximumResponseBytes, requestMethod, null);
  }

  private static TransportResponse executeRawResponse(
      final String rawResponse,
      final long maximumResponseBytes,
      final String requestMethod,
      final Long requestMaximumResponseBytes)
      throws Exception {
    try (ServerSocket server = loopbackServer()) {
      final Thread serverThread =
          new Thread(
              () -> {
                try (Socket socket = server.accept()) {
                  final InputStream input = socket.getInputStream();
                  final StringBuilder requestHeaders = new StringBuilder();
                  while (!requestHeaders.toString().endsWith("\r\n\r\n")) {
                    final int next = input.read();
                    if (next == -1) {
                      break;
                    }
                    requestHeaders.append((char) next);
                  }
                  final OutputStream output = socket.getOutputStream();
                  output.write(rawResponse.getBytes(StandardCharsets.UTF_8));
                  output.flush();
                } catch (final IOException ignored) {
                  // The client may close early after rejecting response framing.
                }
              });
      serverThread.setDaemon(true);
      serverThread.start();

      try {
        final TransportRequest.Builder requestBuilder =
            TransportRequest.builder()
                .setMethod(requestMethod)
                .setUri(URI.create("http://127.0.0.1:" + server.getLocalPort() + "/bounded"))
                .setHeaders(Collections.emptyMap());
        if (requestMaximumResponseBytes != null) {
          requestBuilder.setMaximumResponseBytes(requestMaximumResponseBytes);
        }
        final TransportRequest request = requestBuilder.build();
        return new UrlConnectionTransportExecutor(maximumResponseBytes).execute(request).get();
      } finally {
        serverThread.join(2000L);
      }
    }
  }

  private static ServerSocket loopbackServer() throws IOException {
    return new ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"));
  }

  private static boolean hasCauseMessage(final Throwable failure, final String fragment) {
    Throwable current = failure;
    while (current != null) {
      if (current.getMessage() != null && current.getMessage().contains(fragment)) {
        return true;
      }
      current = current.getCause();
    }
    return false;
  }

  private static void expectInvalidLimit(final long maximumResponseBytes) {
    try {
      new UrlConnectionTransportExecutor(maximumResponseBytes);
      fail("Invalid maximumResponseBytes must be rejected: " + maximumResponseBytes);
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static final class TrackingInputStream extends ByteArrayInputStream {
    private int readCalls;
    private boolean closed;

    private TrackingInputStream() {
      super("123456789".getBytes(StandardCharsets.UTF_8));
    }

    @Override
    public synchronized int read(final byte[] bytes, final int offset, final int length) {
      readCalls++;
      return super.read(bytes, offset, length);
    }

    @Override
    public void close() throws IOException {
      closed = true;
      super.close();
    }
  }
}
