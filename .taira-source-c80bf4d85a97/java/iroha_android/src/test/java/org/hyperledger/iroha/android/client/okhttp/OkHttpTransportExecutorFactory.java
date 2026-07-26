package org.hyperledger.iroha.android.client.okhttp;

import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.testing.FakeHttpTransportExecutor;

/**
 * Test-only stub used to verify {@link org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor}
 * prefers the OkHttp factory when present on the classpath. This lives in the test sources so it
 * never ships in runtime artifacts.
 */
public final class OkHttpTransportExecutorFactory {

  private static final FakeHttpTransportExecutor EXECUTOR = new FakeHttpTransportExecutor();

  private OkHttpTransportExecutorFactory() {}

  public static HttpTransportExecutor createDefault() {
    return EXECUTOR;
  }

  /** Visible for assertions to ensure the factory singleton is reused. */
  public static FakeHttpTransportExecutor executor() {
    return EXECUTOR;
  }
}
