package org.hyperledger.iroha.android.client.transport;

import static org.junit.Assert.assertEquals;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import org.junit.Test;

public final class UrlConnectionTransportExecutorTests {

  @Test
  public void executeReturns404WithEmptyBodyWhenServerSendsNoContent() throws Exception {
    try (ServerSocket server = new ServerSocket(0)) {
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
              .setUri(URI.create("http://localhost:" + port + "/v1/aliases/resolve"))
              .addHeader("Content-Type", "application/json")
              .setBody("{\"alias\":\"missing@test\"}".getBytes(StandardCharsets.UTF_8))
              .build();

      final TransportResponse response = executor.execute(request).get();

      assertEquals("Status code should be 404", 404, response.statusCode());
      assertEquals("Body should be empty for 404 with Content-Length: 0", 0, response.body().length);

      serverThread.join(2000);
    }
  }
}
