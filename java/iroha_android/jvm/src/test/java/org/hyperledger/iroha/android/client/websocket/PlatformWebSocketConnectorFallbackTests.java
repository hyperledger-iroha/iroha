package org.hyperledger.iroha.android.client.websocket;

import static org.junit.Assert.assertTrue;

import org.junit.Test;

/** Ensures JVM builds fall back to the JDK WebSocket connector when OkHttp is absent. */
public final class PlatformWebSocketConnectorFallbackTests {

  private static final String OKHTTP_FACTORY_FQCN =
      "org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorFactory";

  @Test
  public void usesJdkConnectorWhenOkHttpFactoryMissing() {
    final ClassLoader blockingLoader =
        new ClassLoader(PlatformWebSocketConnectorFallbackTests.class.getClassLoader()) {
          @Override
          protected Class<?> loadClass(final String name, final boolean resolve)
              throws ClassNotFoundException {
            if (OKHTTP_FACTORY_FQCN.equals(name)) {
              throw new ClassNotFoundException(name);
            }
            return super.loadClass(name, resolve);
          }
        };

    final ToriiWebSocketClient.WebSocketConnector connector =
        PlatformWebSocketConnector.createDefault(blockingLoader);
    assertTrue(connector instanceof JdkWebSocketConnector);
  }
}
