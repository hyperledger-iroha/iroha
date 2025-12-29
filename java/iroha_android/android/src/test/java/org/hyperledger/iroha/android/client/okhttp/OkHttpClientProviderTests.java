package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertNotSame;
import static org.junit.Assert.assertSame;

import okhttp3.OkHttpClient;
import org.junit.After;
import org.junit.Test;

/** Tests for {@link OkHttpClientProvider}. */
public final class OkHttpClientProviderTests {

  @After
  public void reset() {
    OkHttpClientProvider.resetForTests();
  }

  @Test
  public void sharedClientIsSingletonAndSwappable() {
    final OkHttpClient first = OkHttpClientProvider.shared();
    final OkHttpClient second = OkHttpClientProvider.shared();
    assertSame("shared() should reuse the cached client", first, second);

    final OkHttpClient injected = new OkHttpClient.Builder().build();
    final OkHttpClient previous = OkHttpClientProvider.installForTests(injected);
    assertSame("installForTests should return the prior client", first, previous);
    assertSame("shared() should return the injected instance", injected, OkHttpClientProvider.shared());
  }

  @Test
  public void resetRebuildsClient() {
    final OkHttpClient first = OkHttpClientProvider.shared();
    OkHttpClientProvider.resetForTests();
    final OkHttpClient rebuilt = OkHttpClientProvider.shared();
    assertNotSame("resetForTests should drop the cached client", first, rebuilt);
  }
}
