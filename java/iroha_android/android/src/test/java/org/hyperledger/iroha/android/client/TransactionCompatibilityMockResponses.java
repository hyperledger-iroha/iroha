package org.hyperledger.iroha.android.client;

import okhttp3.mockwebserver.MockResponse;

/** Exact mock response for the mandatory fresh transaction-compatibility probe. */
public final class TransactionCompatibilityMockResponses {
  private TransactionCompatibilityMockResponses() {}

  /** Return a fresh response advertising the SDK's exact V1 transaction contract. */
  public static MockResponse compatibleCapabilities() {
    return new MockResponse()
        .setResponseCode(200)
        .addHeader("Content-Type", "application/json")
        .setBody(
            "{\"data_model_version\":"
                + ToriiTransactionCompatibility.EXPECTED_DATA_MODEL_VERSION
                + ","
                + "\"signed_transaction_schema_hash_hex\":"
                + "\""
                + ToriiTransactionCompatibility.EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX
                + "\"}");
  }
}
