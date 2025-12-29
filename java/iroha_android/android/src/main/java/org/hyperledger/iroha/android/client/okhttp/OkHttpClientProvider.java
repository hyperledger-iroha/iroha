package org.hyperledger.iroha.android.client.okhttp;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.OkHttpClient;

/**
 * Provides a shared {@link OkHttpClient} instance so Android transports reuse the same connection
 * pool by default.
 *
 * <p>Factories fall back to a lazily initialised singleton; tests may swap/reset the shared client
 * to inject instrumented instances.
 */
public final class OkHttpClientProvider {

  private static final AtomicReference<OkHttpClient> SHARED = new AtomicReference<>();

  private OkHttpClientProvider() {}

  /** Returns the shared OkHttp client, creating it when missing. */
  public static OkHttpClient shared() {
    final OkHttpClient existing = SHARED.get();
    if (existing != null) {
      return existing;
    }
    final OkHttpClient created = new OkHttpClient();
    return SHARED.compareAndSet(null, created) ? created : SHARED.get();
  }

  /** Installs {@code client} as the shared instance, returning the previous value. */
  static OkHttpClient installForTests(final OkHttpClient client) {
    return SHARED.getAndSet(Objects.requireNonNull(client, "client"));
  }

  /** Clears the shared client so the next lookup creates a fresh instance. */
  static void resetForTests() {
    SHARED.set(null);
  }
}
