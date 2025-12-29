package org.hyperledger.iroha.android.client;

import org.junit.Test;

/** JVM-only coverage for {@link PlatformHttpTransportExecutor} fallback behaviour. */
public final class PlatformHttpTransportExecutorJvmTests {

  @Test
  public void fallsBackToJavaExecutorWhenOkHttpFactoryMissing() {
    final HttpTransportExecutor executor = PlatformHttpTransportExecutor.createDefault();
    assert executor instanceof JavaHttpExecutor : "fallback should use JavaHttpExecutor on JVM";
    assert executor.supportsClientUnwrap() : "JavaHttpExecutor unwrap should be available";
  }
}
