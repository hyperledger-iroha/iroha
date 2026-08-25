package org.hyperledger.iroha.android.tx;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.junit.Test;

public final class TransactionPayloadFixturesTests {

  private static final String SAMPLE_AUTHORITY = TestAccountIds.ed25519Authority(0x2E);
  private static final String TEST_NETWORK_ID =
      "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";

  @Test
  public void toPayloadRejectsMissingWireInstructionFields() {
    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("wire_name", "iroha.register");

    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("network_id", TEST_NETWORK_ID);
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 1_735_000_000_000L);
    payload.put("time_to_live_ms", 100_000L);
    payload.put("nonce", null);
    final Map<String, Object> feePaymentValue = new LinkedHashMap<>();
    feePaymentValue.put("charge_limits", Collections.emptyList());
    final Map<String, Object> feePayment = new LinkedHashMap<>();
    feePayment.put("payer", "authority");
    feePayment.put("value", feePaymentValue);
    payload.put("fee_payment", feePayment);
    final Map<String, Object> admissionIntent = new LinkedHashMap<>();
    admissionIntent.put("intent", "ordinary");
    admissionIntent.put("value", null);
    payload.put("admission_intent", admissionIntent);
    payload.put("executable", executable);
    payload.put("metadata", new LinkedHashMap<>());

    final Map<String, Object> fixture = new LinkedHashMap<>();
    fixture.put("name", "missing-wire-fields");
    fixture.put("network_id", TEST_NETWORK_ID);
    fixture.put("authority", SAMPLE_AUTHORITY);
    fixture.put("creation_time_ms", 1_735_000_000_000L);
    fixture.put("time_to_live_ms", 100_000L);
    fixture.put("nonce", null);
    fixture.put("payload_base64", "AA==");
    fixture.put("signed_base64", "AQ==");
    fixture.put(
        "payload_hash", "0000000000000000000000000000000000000000000000000000000000000000");
    fixture.put(
        "signed_hash", "1111111111111111111111111111111111111111111111111111111111111111");
    fixture.put("payload", payload);

    final TransactionPayloadFixtures.Fixture parsed =
        TransactionPayloadFixtures.Fixture.fromObject(fixture);
    assertThrows(
        parsed::toPayload,
        "expected missing instruction payload fields to be rejected");
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException ex) {
      return;
    }
    throw new AssertionError(message);
  }
}
