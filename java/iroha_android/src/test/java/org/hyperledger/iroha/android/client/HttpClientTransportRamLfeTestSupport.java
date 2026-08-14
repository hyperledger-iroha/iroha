package org.hyperledger.iroha.android.client;

import java.security.KeyPair;
import java.security.KeyPairGenerator;

final class HttpClientTransportRamLfeTestSupport {
  private HttpClientTransportRamLfeTestSupport() {}

  static String ramLfeExecuteResponseJson() {
    return "{"
        + "\"program_id\":\"identifier_lookup_retail\","
        + "\"opaque_hash\":\""
        + "11".repeat(32)
        + "\","
        + "\"receipt_hash\":\""
        + "22".repeat(32)
        + "\","
        + "\"output_ciphertext\":\"abcd\","
        + "\"output_hash\":\""
        + "44".repeat(32)
        + "\","
        + "\"associated_data_hash\":\""
        + "55".repeat(32)
        + "\","
        + "\"executed_at_ms\":42,"
        + "\"expires_at_ms\":142,"
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"receipt\":{"
        + "\"payload\":{"
        + "\"program_id\":{\"name\":\"identifier_lookup_retail\"},"
        + "\"program_digest\":\"hash:"
        + "11".repeat(32).toUpperCase()
        + "#ABCD\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":{\"mode\":\"Signed\",\"value\":null},"
        + "\"output_hash\":\"hash:"
        + "22".repeat(32).toUpperCase()
        + "#BCDE\","
        + "\"associated_data_hash\":\"hash:"
        + "33".repeat(32).toUpperCase()
        + "#CDEF\","
        + "\"executed_at_ms\":42,"
        + "\"expires_at_ms\":142"
        + "},"
        + "\"signature\":\""
        + "aa".repeat(64)
        + "\""
        + "},"
        + "\"output_opening\":"
        + HttpClientTransportTests.identifierOpeningJson(
            HttpClientTransportTests.sampleOpening("identifier_lookup_retail"))
        + "}";
  }

  static String ramLfeReceiptVerifyResponseJson() {
    return "{"
        + "\"valid\":true,"
        + "\"program_id\":\"identifier_lookup_retail\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"output_hash\":\""
        + "44".repeat(32)
        + "\","
        + "\"associated_data_hash\":\""
        + "55".repeat(32)
        + "\","
        + "\"output_hash_matches\":true"
        + "}";
  }

  static ToriiCanonicalRequestAuth applicationAuth(
      final String accountId, final String nonce) {
    try {
      final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      return HttpClientTransportTests.canonicalAuth(
          accountId, keyPair, 1_700_000_000_123L, nonce);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create application request signer", ex);
    }
  }
}
