package org.hyperledger.iroha.android.client;

/**
 * Factory for selecting a transport executor per runtime (OkHttp on Android, JDK client elsewhere).
 */
public final class PlatformHttpTransportExecutor {

  private PlatformHttpTransportExecutor() {}

  /** Returns a platform-appropriate executor (prefers OkHttp when available). */
  public static HttpTransportExecutor createDefault() {
    return createDefault(PlatformHttpTransportExecutor.class.getClassLoader());
  }

  static HttpTransportExecutor createDefault(final ClassLoader loader) {
    final HttpTransportExecutor okHttp = tryCreateOkHttpExecutor(loader);
    if (okHttp != null) {
      return okHttp;
    }
    final HttpTransportExecutor javaExecutor = tryCreateJavaHttpExecutor(loader);
    if (javaExecutor != null) {
      return javaExecutor;
    }
    return new org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor();
  }

  private static HttpTransportExecutor tryCreateOkHttpExecutor(final ClassLoader loader) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformHttpTransportExecutor.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(
              "org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactory",
              true,
              effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof HttpTransportExecutor executor) {
        return executor;
      }
    } catch (final ClassNotFoundException ignored) {
      return null;
    } catch (final Exception ignored) {
      // OkHttp present but failed to initialize; fall back to JavaHttpExecutor.
      return null;
    }
    return null;
  }

  private static HttpTransportExecutor tryCreateJavaHttpExecutor(final ClassLoader loader) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformHttpTransportExecutor.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(
              "org.hyperledger.iroha.android.client.JavaHttpExecutorFactory",
              true,
              effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof HttpTransportExecutor executor) {
        return executor;
      }
    } catch (final ClassNotFoundException ignored) {
      return null;
    } catch (final Exception ignored) {
      // Java HTTP client not available; caller will surface the failure.
      return null;
    }
    return null;
  }
}
