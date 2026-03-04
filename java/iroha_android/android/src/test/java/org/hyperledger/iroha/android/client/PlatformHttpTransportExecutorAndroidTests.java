package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertTrue;

import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.junit.Test;

public final class PlatformHttpTransportExecutorAndroidTests {

  @Test
  public void prefersOkHttpWhenAvailable() {
    final HttpTransportExecutor executor = PlatformHttpTransportExecutor.createDefault();
    assertTrue(executor instanceof OkHttpTransportExecutor);
  }
}
