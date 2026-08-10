package org.hyperledger.iroha.android.client;

final class HttpClientTransportTestFixtures {
  private HttpClientTransportTestFixtures() {}

  static String privacyCapabilitySnapshotJson() {
    final StringBuilder rows = new StringBuilder();
    for (final org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1 protocol :
        org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1.values()) {
      if (rows.length() != 0) {
        rows.append(',');
      }
      rows.append("{\"protocol_id\":{\"protocol\":\"")
          .append(protocol.getCanonicalLabel())
          .append(
              "\",\"value\":null},\"compiled_profile\":{\"status\":\"unavailable\","
                  + "\"value\":{\"reason\":\"engine-unavailable\",\"detail\":null}},"
                  + "\"activation\":null}");
    }
    return "{\"version\":1,\"committed_height\":42,\"consensus_policy\":{"
        + "\"current_limits\":{"
        + "\"max_actions_per_transaction\":1,\"max_actions_per_block\":2,"
        + "\"max_proof_bytes_per_action\":9437184,\"max_action_bytes\":9437184,"
        + "\"max_privacy_bytes_per_transaction\":9437184,"
        + "\"max_privacy_bytes_per_block\":18874368,"
        + "\"max_statement_and_encrypted_output_bytes_per_transaction\":262144,"
        + "\"max_nullifiers_per_action\":8,\"max_commitments_per_action\":8,"
        + "\"retained_root_count\":2048},\"pending_tightening\":null},"
        + "\"protocols\":["
        + rows
        + "]}";
  }
}
