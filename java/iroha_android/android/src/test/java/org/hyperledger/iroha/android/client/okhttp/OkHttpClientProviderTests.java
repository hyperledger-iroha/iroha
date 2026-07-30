package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNotSame;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertSame;

import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
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

  @Test
  public void defaultSharedClientDisablesRedirectsAndConnectionRetries() {
    final OkHttpClient client = OkHttpClientProvider.shared();

    assertFalse("default client must not follow HTTP redirects", client.followRedirects());
    assertFalse("default client must not follow HTTPS redirects", client.followSslRedirects());
    assertFalse(
        "default client must not retry signed requests after connection failure",
        client.retryOnConnectionFailure());
  }

  @Test
  public void defaultSharedClientDoesNotRedirectSignedRequest() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start();
      server.enqueue(
          new MockResponse()
              .setResponseCode(307)
              .setHeader("Location", server.url("/redirected")));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("must not be reached"));
      final Request request =
          new Request.Builder()
              .url(server.url("/signed"))
              .header("X-Iroha-Signature", "canonical-signature")
              .build();

      try (Response response = OkHttpClientProvider.shared().newCall(request).execute()) {
        assertEquals(307, response.code());
      }

      assertEquals(1, server.getRequestCount());
      final RecordedRequest signed = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("signed request was not received", signed);
      assertEquals("/signed", signed.getPath());
      assertEquals("canonical-signature", signed.getHeader("X-Iroha-Signature"));
      assertNull(
          "signed request must not be redirected",
          server.takeRequest(100, TimeUnit.MILLISECONDS));
    }
  }
}
