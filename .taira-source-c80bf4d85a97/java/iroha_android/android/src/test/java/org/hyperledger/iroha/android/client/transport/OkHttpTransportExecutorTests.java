package org.hyperledger.iroha.android.client.transport;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.transport.TransportRequest.Builder;

public final class OkHttpTransportExecutorTests {

  private OkHttpTransportExecutorTests() {}

  public static void main(final String[] args) throws Exception {
    shouldExecuteGet();
    shouldStreamGet();
    System.out.println("[IrohaAndroid] OkHttpTransportExecutor transport tests passed.");
  }

  private static void shouldExecuteGet() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("ok"));
      server.start();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new okhttp3.OkHttpClient());
      final TransportRequest request =
          new Builder()
              .setUri(new URI(server.url("/").toString()))
              .setMethod("GET")
              .build();
      final TransportResponse response = executor.execute(request).join();
      assert response.statusCode() == 200 : "expected 200";
      assert "ok".equals(new String(response.body(), StandardCharsets.UTF_8)) : "expected body";
    }
  }

  private static void shouldStreamGet() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("data: ok\n\n"));
      server.start();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new okhttp3.OkHttpClient());
      final TransportRequest request =
          new Builder()
              .setUri(new URI(server.url("/").toString()))
              .setMethod("GET")
              .build();
      final TransportStreamResponse response = executor.openStream(request).join();
      final String body = readBody(response.body());
      response.close();
      assert response.statusCode() == 200 : "expected 200";
      assert body.contains("data: ok") : "expected streaming body";
    }
  }

  private static String readBody(final InputStream input) throws Exception {
    try (InputStream stream = input; ByteArrayOutputStream output = new ByteArrayOutputStream()) {
      final byte[] buffer = new byte[4096];
      int read;
      while ((read = stream.read(buffer)) != -1) {
        output.write(buffer, 0, read);
      }
      return output.toString(StandardCharsets.UTF_8);
    }
  }
}
