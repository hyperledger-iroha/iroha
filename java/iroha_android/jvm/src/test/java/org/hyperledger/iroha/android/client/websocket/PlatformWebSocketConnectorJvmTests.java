package org.hyperledger.iroha.android.client.websocket;

import org.junit.Test;

/** JVM-only coverage for {@link PlatformWebSocketConnector} fallback behaviour. */
public final class PlatformWebSocketConnectorJvmTests {

  @Test
  public void fallsBackToJdkConnectorWhenOkHttpFactoryMissing() {
    final ToriiWebSocketClient.WebSocketConnector connector =
        PlatformWebSocketConnector.createDefault();
    assert connector instanceof JdkWebSocketConnector : "JDK connector should be chosen on JVM";
  }
}
