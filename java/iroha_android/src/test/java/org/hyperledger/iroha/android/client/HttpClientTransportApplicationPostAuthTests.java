package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.NetworkId;

final class HttpClientTransportApplicationPostAuthTests {
  private HttpClientTransportApplicationPostAuthTests() {}

  public static void main(final String[] args) {
    runAll();
    System.exit(0);
  }

  static void runAll() {
    ramLfeSignatureSeparatesSameAccountAcrossForeignGenesis();
  }

  private static void ramLfeSignatureSeparatesSameAccountAcrossForeignGenesis() {
    final List<TransportRequest> requests = new ArrayList<>();
    final HttpTransportExecutor executor =
        request -> {
          requests.add(request);
          return CompletableFuture.completedFuture(
              new TransportResponse(404, new byte[0], "not found", Map.of()));
        };
    final ToriiCanonicalRequestAuth auth = applicationAuth();
    final List<NetworkId> networks =
        List.of(
            NetworkId.parse(
                "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"),
            NetworkId.parse(
                "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a9"));

    for (final NetworkId networkId : networks) {
      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(URI.create("https://torii.example"))
              .setLocalSigningContext(new LocalSigningContext(networkId))
              .build();
      HttpClientTransport.withExecutor(executor, config)
          .executeRamLfeProgram("lookup", RamLfeExecuteRequest.encrypted("ABCD"), auth);
    }

    final String localSignature =
        requests.get(0).headers().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0);
    final String foreignSignature =
        requests.get(1).headers().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0);
    assert !localSignature.equals(foreignSignature)
        : "same account/path/body must separate foreign genesis signatures";
  }

  private static ToriiCanonicalRequestAuth applicationAuth() {
    try {
      final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      return new ToriiCanonicalRequestAuth(
          "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
          message -> {
            try {
              final Signature signer = Signature.getInstance("Ed25519");
              signer.initSign(keyPair.getPrivate());
              signer.update(message);
              return signer.sign();
            } catch (final Exception ex) {
              throw new IllegalStateException("failed to sign application request", ex);
            }
          },
          1_700_000_000_123L,
          "application-foreign-genesis");
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create application request signer", ex);
    }
  }
}
