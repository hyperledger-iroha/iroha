package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Tests focused on HttpClientTransport export options handling. */
public final class HttpClientTransportExportOptionsTests {

  private HttpClientTransportExportOptionsTests() {}

  public static void main(final String[] args) throws Exception {
    submissionDoesNotInvokeExportProvider();
    System.out.println("[IrohaAndroid] Http client export options tests passed.");
  }

  private static void submissionDoesNotInvokeExportProvider() throws Exception {
    final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareProvider();
    keyManager.generateOrLoad("alias", KeySecurityPreference.SOFTWARE_ONLY);
    final char[] shared = "export-passphrase".toCharArray();
    final AtomicInteger providerCalls = new AtomicInteger();

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("http://localhost:8080"))
            .setExportOptions(
                ClientConfig.ExportOptions.builder()
                    .setKeyManager(keyManager)
                    .setPassphraseProvider(
                        alias -> {
                          providerCalls.incrementAndGet();
                          return shared;
                        })
                    .build())
            .build();

    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(new NoopExecutor(), config);

    final NoritoJavaCodecAdapter codec =
        new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(
                org.hyperledger.iroha.android.testing.TestNetworkIds.fromSeed(0L))
            .setAuthority(TestAccountIds.ed25519Authority(0x37))
            .setCreationTimeMs(1_700_000_000_000L)
            .setInstructionBytes(new byte[] {0x01})
            .setTimeToLiveMs(5_000L)
            .setNonce(1L)
            .setMetadata(Collections.emptyMap())
            .build();
    final SignedTransaction transaction =
        new SignedTransaction(
            codec.encodeTransaction(payload),
            new byte[64],
            new byte[32],
            codec.schemaName(),
            "alias");

    transport.submitTransaction(transaction).join();

    assert providerCalls.get() == 0 : "submission must not export or queue signing keys";
    for (final char c : shared) {
      assert c != '\0' : "unused passphrase must remain caller-owned";
    }
  }

  private static final class NoopExecutor implements HttpTransportExecutor {
    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (HttpClientTransportSubmissionContractTests.isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(
            HttpClientTransportSubmissionContractTests.compatibleCapabilitiesResponse());
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", java.util.Map.of()));
    }
  }
}
