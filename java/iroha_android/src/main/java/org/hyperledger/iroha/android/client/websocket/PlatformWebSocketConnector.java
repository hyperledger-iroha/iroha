package org.hyperledger.iroha.android.client.websocket;

/**
 * Factory that selects a WebSocket connector per runtime (OkHttp on Android, JDK connector on JVM).
 */
public final class PlatformWebSocketConnector {

  private PlatformWebSocketConnector() {}

  /** Returns a platform-appropriate connector (prefers OkHttp when available). */
  public static ToriiWebSocketClient.WebSocketConnector createDefault() {
    return createDefault(PlatformWebSocketConnector.class.getClassLoader());
  }

  static ToriiWebSocketClient.WebSocketConnector createDefault(final ClassLoader loader) {
    final ToriiWebSocketClient.WebSocketConnector okHttp = tryCreateOkHttpConnector(loader);
    if (okHttp != null) {
      return okHttp;
    }
    final ToriiWebSocketClient.WebSocketConnector jdk = tryCreateJdkConnector(loader);
    if (jdk != null) {
      return jdk;
    }
    throw new IllegalStateException("No WebSocket connector is available on the classpath.");
  }

  private static ToriiWebSocketClient.WebSocketConnector tryCreateOkHttpConnector(
      final ClassLoader loader) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformWebSocketConnector.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(
              "org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorFactory",
              true,
              effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof ToriiWebSocketClient.WebSocketConnector connector) {
        return connector;
      }
    } catch (final ClassNotFoundException ignored) {
      return null;
    } catch (final Exception ignored) {
      // OkHttp present but failed to initialise; fall back to the JDK connector.
      return null;
    }
    return null;
  }

  private static ToriiWebSocketClient.WebSocketConnector tryCreateJdkConnector(
      final ClassLoader loader) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformWebSocketConnector.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(
              "org.hyperledger.iroha.android.client.websocket.JdkWebSocketConnectorFactory",
              true,
              effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof ToriiWebSocketClient.WebSocketConnector connector) {
        return connector;
      }
    } catch (final ClassNotFoundException ignored) {
      return null;
    } catch (final Exception ignored) {
      // JDK connector not available.
      return null;
    }
    return null;
  }
}
