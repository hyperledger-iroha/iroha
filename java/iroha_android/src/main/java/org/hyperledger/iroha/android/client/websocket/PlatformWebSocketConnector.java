package org.hyperledger.iroha.android.client.websocket;

/** Factory that constructs the canonical WebSocket connector for the current runtime. */
public final class PlatformWebSocketConnector {

  private PlatformWebSocketConnector() {}

  /** Returns the canonical connector for Android or JVM. */
  public static ToriiWebSocketClient.WebSocketConnector createDefault() {
    return createDefault(PlatformWebSocketConnector.class.getClassLoader());
  }

  static ToriiWebSocketClient.WebSocketConnector createDefault(final ClassLoader loader) {
    final String factory =
        isAndroidRuntime()
            ? "org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorFactory"
            : "org.hyperledger.iroha.android.client.websocket.JdkWebSocketConnectorFactory";
    return createFromFactory(loader, factory);
  }

  private static ToriiWebSocketClient.WebSocketConnector createFromFactory(
      final ClassLoader loader, final String factoryName) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformWebSocketConnector.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(factoryName, true, effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof ToriiWebSocketClient.WebSocketConnector connector) {
        return connector;
      }
      throw new IllegalStateException(factoryName + " did not return a WebSocket connector");
    } catch (final ReflectiveOperationException ex) {
      throw new IllegalStateException("Required WebSocket connector factory is unavailable", ex);
    }
  }

  private static boolean isAndroidRuntime() {
    try {
      Class.forName("android.os.Build");
      return true;
    } catch (final ClassNotFoundException ignored) {
      return false;
    }
  }
}
