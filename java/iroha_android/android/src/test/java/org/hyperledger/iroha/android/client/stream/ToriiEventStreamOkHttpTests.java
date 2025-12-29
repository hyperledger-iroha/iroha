package org.hyperledger.iroha.android.client.stream;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.junit.Test;

public final class ToriiEventStreamOkHttpTests {

  @Test
  public void okHttpExecutorConsumesSseEvents() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final String body = "event: update\nid: 7\ndata: hello\n\ndata: keepalive\n\n";
      server.enqueue(new MockResponse().setResponseCode(200).setBody(body));
      server.start();

      final TransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(new URI(server.url("/").toString()))
              .setTransportExecutor(executor)
              .build();

      final RecordingListener listener = new RecordingListener();
      final ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);

      stream.completion().get(2, TimeUnit.SECONDS);
      assertTrue(listener.events.size() >= 2);
      assertEquals("update", listener.events.get(0).event());
      assertEquals("hello", listener.events.get(0).data());
      assertEquals("7", listener.events.get(0).id());
      assertEquals("message", listener.events.get(1).event());
      assertEquals("keepalive", listener.events.get(1).data());
    }
  }

  private static final class RecordingListener implements ToriiEventStreamListener {
    private final List<ServerSentEvent> events = new ArrayList<>();

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
    }
  }
}
