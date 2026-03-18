package org.hyperledger.iroha.android.client;

import java.lang.reflect.Method;
import java.net.URI;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Tests focused on HttpClientTransport export options handling. */
public final class HttpClientTransportExportOptionsTests {

  private HttpClientTransportExportOptionsTests() {}

  public static void main(final String[] args) throws Exception {
    passphraseProviderArraysAreWipedAfterUse();
    System.out.println("[IrohaAndroid] Http client export options tests passed.");
  }

  private static void passphraseProviderArraysAreWipedAfterUse() throws Exception {
    final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareFallback();
    keyManager.generateOrLoad("alias", KeySecurityPreference.SOFTWARE_ONLY);
    final char[] shared = "export-passphrase".toCharArray();

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("http://localhost:8080"))
            .setExportOptions(
                ClientConfig.ExportOptions.builder()
                    .setKeyManager(keyManager)
                    .setPassphraseProvider(alias -> shared)
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(new NoopExecutor(), config);

    final SignedTransaction transaction =
        new SignedTransaction(
            new byte[] {0x01},
            new byte[64],
            new byte[32],
            "schema.v1",
            "alias");

    final Method method =
        HttpClientTransport.class.getDeclaredMethod("maybeAttachExportBundle", SignedTransaction.class);
    method.setAccessible(true);
    method.invoke(transport, transaction);

    for (final char c : shared) {
      assert c == '\0' : "Passphrase characters must be zeroed after use";
    }
  }

  private static final class NoopExecutor implements HttpTransportExecutor {
    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(new UnsupportedOperationException("noop"));
      return future;
    }
  }
}
