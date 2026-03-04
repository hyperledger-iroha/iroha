package org.hyperledger.iroha.android.client;

import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;

/** Unit tests for {@link PlatformHttpTransportExecutor} fallback behaviour. */
public final class PlatformHttpTransportExecutorFallbackTests {

  private static final String OKHTTP_FACTORY_FQCN =
      "org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactory";
  private static final String JDK_FACTORY_FQCN =
      "org.hyperledger.iroha.android.client.JavaHttpExecutorFactory";

  private PlatformHttpTransportExecutorFallbackTests() {}

  public static void main(final String[] args) {
    fallsBackToUrlConnectionWhenFactoriesMissing();
    System.out.println("[IrohaAndroid] PlatformHttpTransportExecutorFallbackTests passed.");
  }

  private static void fallsBackToUrlConnectionWhenFactoriesMissing() {
    final ClassLoader blockingLoader =
        new ClassLoader(PlatformHttpTransportExecutorFallbackTests.class.getClassLoader()) {
          @Override
          protected Class<?> loadClass(final String name, final boolean resolve)
              throws ClassNotFoundException {
            if (OKHTTP_FACTORY_FQCN.equals(name) || JDK_FACTORY_FQCN.equals(name)) {
              throw new ClassNotFoundException(name);
            }
            return super.loadClass(name, resolve);
          }
        };

    final HttpTransportExecutor executor =
        PlatformHttpTransportExecutor.createDefault(blockingLoader);
    assert executor instanceof UrlConnectionTransportExecutor
        : "fallback should use UrlConnectionTransportExecutor when no factories are available";
  }
}
