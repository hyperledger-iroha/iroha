package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;

/** First-release compatibility values required before submitting a transaction to Torii. */
public final class ToriiTransactionCompatibility {
  /** Current data-model version encoded by this SDK. */
  public static final int EXPECTED_DATA_MODEL_VERSION = 4;

  /** Current {@code SignedTransaction} Norito schema hash encoded by this SDK. */
  public static final String EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX =
      "7ab5ff9c572efb316deac478f19209c5";

  private ToriiTransactionCompatibility() {}

  static void requireCompatible(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8);
    if (!Arrays.equals(payload, json.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalArgumentException(
          "node capabilities response must be valid UTF-8 JSON");
    }
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof Map)) {
      throw new IllegalArgumentException(
          "node capabilities response must be a JSON object");
    }
    final Map<?, ?> fields = (Map<?, ?>) parsed;

    final int actualVersion =
        JsonNumbers.asInt(
            fields.get("data_model_version"),
            "node capabilities response.data_model_version");
    if (actualVersion != EXPECTED_DATA_MODEL_VERSION) {
      throw new ToriiDataModelMismatchException(
          EXPECTED_DATA_MODEL_VERSION, actualVersion);
    }

    final Object schemaValue = fields.get("signed_transaction_schema_hash_hex");
    final String actualSchemaHash =
        schemaValue instanceof String ? (String) schemaValue : null;
    if (!EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX.equals(actualSchemaHash)) {
      throw new ToriiTransactionSchemaMismatchException(
          EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX, actualSchemaHash);
    }
  }
}
