package org.hyperledger.iroha.android.client.websocket;

import static org.junit.Assert.assertTrue;

import org.junit.Test;

/** Ensures JVM builds use the canonical JDK WebSocket connector. */
public final class PlatformWebSocketConnectorTests {

  @Test
  public void usesJdkConnectorOnJvm() {
    final ToriiWebSocketClient.WebSocketConnector connector =
        PlatformWebSocketConnector.createDefault(
            PlatformWebSocketConnectorTests.class.getClassLoader());
    assertTrue(connector instanceof JdkWebSocketConnector);
  }
}
