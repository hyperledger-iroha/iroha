package org.hyperledger.iroha.android.client;

/** Factory for constructing the canonical Java/Android transport executor. */
public final class PlatformHttpTransportExecutor {

  private PlatformHttpTransportExecutor() {}

  /** Returns the canonical executor used by default clients. */
  public static HttpTransportExecutor createDefault() {
    return createDefault(PlatformHttpTransportExecutor.class.getClassLoader());
  }

  static HttpTransportExecutor createDefault(final ClassLoader loader) {
    final String factory;
    if (isAndroidRuntime()) {
      factory = "org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactory";
    } else if (isClassPresent(
        loader, "org.hyperledger.iroha.android.client.JavaHttpExecutorFactory")) {
      factory = "org.hyperledger.iroha.android.client.JavaHttpExecutorFactory";
    } else {
      return new org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor();
    }
    return createFromFactory(loader, factory);
  }

  private static HttpTransportExecutor createFromFactory(
      final ClassLoader loader, final String factoryName) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformHttpTransportExecutor.class.getClassLoader() : loader;
    try {
      final Class<?> factoryClass =
          Class.forName(factoryName, true, effectiveLoader);
      final var method = factoryClass.getMethod("createDefault");
      final Object result = method.invoke(null);
      if (result instanceof HttpTransportExecutor executor) {
        return executor;
      }
      throw new IllegalStateException(factoryName + " did not return an HTTP transport executor");
    } catch (final ReflectiveOperationException ex) {
      throw new IllegalStateException("Required HTTP transport factory is unavailable", ex);
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

  private static boolean isClassPresent(final ClassLoader loader, final String className) {
    final ClassLoader effectiveLoader =
        loader == null ? PlatformHttpTransportExecutor.class.getClassLoader() : loader;
    try {
      Class.forName(className, false, effectiveLoader);
      return true;
    } catch (final ClassNotFoundException ignored) {
      return false;
    }
  }
}
