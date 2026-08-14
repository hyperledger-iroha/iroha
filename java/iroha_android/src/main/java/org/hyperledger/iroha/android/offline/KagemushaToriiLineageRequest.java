package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.TransportRequest;

/** Builds the exact-network, one-shot receiver-lineage request. */
final class KagemushaToriiLineageRequest {
  private KagemushaToriiLineageRequest() {}

  static TransportRequest build(
      final String baseUri,
      final KagemushaRecursiveSpendProver.RecipientLineageQueryV2 query,
      final LocalSigningContext localSigningContext,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final URI target =
        URI.create(baseUri + KagemushaRecursiveSpendProver.ToriiClient.RECEIVER_LINEAGE_PATH);
    final byte[] body = Objects.requireNonNull(query, "query").noritoEncoded();
    final ToriiCanonicalRequestAuth requiredAuth =
        Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final Long timestampMs = requiredAuth.timestampMs();
    final String nonce = requiredAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    final Map<String, String> authHeaders =
        timestampMs == null
            ? CanonicalRequestSigner.buildHeaders(
                localSigningContext.networkId(), "POST", target, body, requiredAuth)
            : CanonicalRequestSigner.buildHeaders(
                localSigningContext.networkId(),
                "POST",
                target,
                body,
                requiredAuth,
                timestampMs.longValue(),
                nonce);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setMethod("POST")
            .setUri(target)
            .addHeader("Accept", KagemushaRecursiveSpendProver.ToriiClient.NORITO_MEDIA_TYPE)
            .addHeader("Content-Type", KagemushaRecursiveSpendProver.ToriiClient.NORITO_MEDIA_TYPE)
            .setBody(body)
            .setMaximumResponseBytes(
                (long) KagemushaRecursiveSpendProver.MAX_TORII_RESPONSE_BYTES);
    authHeaders.forEach(builder::addHeader);
    return builder.build();
  }
}
