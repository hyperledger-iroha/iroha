package org.hyperledger.iroha.android.client.websocket;

import org.junit.Test;

/** JVM-only coverage for {@link PlatformWebSocketConnector}. */
public final class PlatformWebSocketConnectorJvmTests {

  @Test
  public void usesJdkConnectorOnJvm() {
    final ToriiWebSocketClient.WebSocketConnector connector =
        PlatformWebSocketConnector.createDefault();
    assert connector instanceof JdkWebSocketConnector : "JDK connector should be chosen on JVM";
  }
}
