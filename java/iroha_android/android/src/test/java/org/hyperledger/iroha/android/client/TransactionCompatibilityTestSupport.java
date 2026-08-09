package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.RecordedRequest;

/** Shared fixtures for the mandatory transaction-submission capabilities probe. */
public final class TransactionCompatibilityTestSupport {
  private TransactionCompatibilityTestSupport() {}

  /** Returns the canonical capabilities response accepted by this SDK build. */
  public static MockResponse compatibleCapabilitiesResponse() {
    return new MockResponse()
        .setResponseCode(200)
        .addHeader("Content-Type", "application/json")
        .setBody(
            "{\"data_model_version\":"
                + ToriiTransactionCompatibility.EXPECTED_DATA_MODEL_VERSION
                + ",\"signed_transaction_schema_hash_hex\":\""
                + ToriiTransactionCompatibility.EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX
                + "\"}");
  }

  /** Asserts that a request is the mandatory capabilities probe. */
  public static void assertCompatibleCapabilitiesRequest(final RecordedRequest request) {
    assertNotNull("mock server must observe the capabilities probe", request);
    assertEquals("GET", request.getMethod());
    assertEquals("/v1/node/capabilities", request.getPath());
  }
}
