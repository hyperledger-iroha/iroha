package org.hyperledger.iroha.android.client;

import org.junit.Test;

/** JVM-only coverage for {@link PlatformHttpTransportExecutor}. */
public final class PlatformHttpTransportExecutorJvmTests {

  @Test
  public void usesJavaExecutorOnJvm() {
    final HttpTransportExecutor executor = PlatformHttpTransportExecutor.createDefault();
    assert executor instanceof JavaHttpExecutor : "JVM transport must be JavaHttpExecutor";
    assert executor.supportsClientUnwrap() : "JavaHttpExecutor unwrap should be available";
  }
}
