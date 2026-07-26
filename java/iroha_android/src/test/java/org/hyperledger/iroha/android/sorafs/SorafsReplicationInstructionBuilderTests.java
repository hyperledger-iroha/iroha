package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.ExpireReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.IssueReplicationOrderInstruction;

/** Ensures the replication order builders emit the expected argument schema. */
public final class SorafsReplicationInstructionBuilderTests {

  private SorafsReplicationInstructionBuilderTests() {}

  private static final String ORDER_ID =
      "44b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c0";
  private static final String PROVIDER_ID = "11".repeat(32);

  public static void main(final String[] args) {
    testIssueReplicationOrder();
    testIssueReplicationOrderRejectsInvalidBase64();
    testIssueReplicationOrderRejectsNegativeEpoch();
    testIssueReplicationOrderRejectsMalformedInputs();
    testCompleteReplicationOrder();
    testCompleteReplicationOrderRejectsNegativeEpoch();
    testExpireReplicationOrder();
    testArgumentDecodersRejectUnknownAndWrongAction();
    System.out.println(
        "[IrohaAndroid] SorafsReplicationInstructionBuilderTests passed (issue/complete/expire).");
  }

  private static void testIssueReplicationOrder() {
    final String payload =
        Base64.getEncoder().encodeToString("replication-order".getBytes(StandardCharsets.UTF_8));
    final IssueReplicationOrderInstruction payloadInstruction =
        IssueReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setOrderPayloadBase64(payload)
            .setIssuedEpoch(20)
            .setDeadlineEpoch(28)
            .build();

    final Map<String, String> args = payloadInstruction.toArguments();
    assert "IssueReplicationOrder".equals(args.get("action"))
        : "action mismatch";
    assert ORDER_ID.equals(args.get("order_id_hex")) : "order_id_hex mismatch";
    assert payload.equals(args.get("order_payload_base64")) : "payload mismatch";
    assert ORDER_ID.equals(payloadInstruction.orderIdHex()) : "rehydrated orderId mismatch";
    assert payload.equals(payloadInstruction.orderPayloadBase64()) : "rehydrated payload mismatch";
    assert payloadInstruction.issuedEpoch() == 20 : "issued epoch mismatch";
    assert payloadInstruction.deadlineEpoch() == 28 : "deadline epoch mismatch";
  }

  private static void testIssueReplicationOrderRejectsInvalidBase64() {
    boolean threw = false;
    try {
      IssueReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setOrderPayloadBase64("not!base64");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid order payload base64 to throw";
  }

  private static void testIssueReplicationOrderRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      IssueReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setOrderPayloadBase64("AAECAw==")
          .setIssuedEpoch(-1)
          .setDeadlineEpoch(10);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative issued epoch to throw";

    threw = false;
    try {
      IssueReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setOrderPayloadBase64("AAECAw==")
          .setIssuedEpoch(1)
          .setDeadlineEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative deadline epoch to throw";
  }

  private static void testIssueReplicationOrderRejectsMalformedInputs() {
    boolean threw = false;
    try {
      IssueReplicationOrderInstruction.builder().setOrderIdHex("AA".repeat(32));
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected uppercase order identifier to throw";

    threw = false;
    try {
      IssueReplicationOrderInstruction.builder().setOrderId(new byte[32]);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected zero order identifier to throw";

    threw = false;
    try {
      IssueReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setOrderPayload(new byte[] {1})
          .setIssuedEpoch(10)
          .setDeadlineEpoch(10)
          .build();
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected non-increasing order window to throw";

    threw = false;
    try {
      IssueReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setOrderPayload(new byte[1024 * 1024 + 1]);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected oversized order payload to throw";

    final byte[] highBytes = new byte[32];
    java.util.Arrays.fill(highBytes, (byte) 0x80);
    final IssueReplicationOrderInstruction highId =
        IssueReplicationOrderInstruction.builder()
            .setOrderId(highBytes)
            .setOrderPayload(new byte[] {1})
            .setIssuedEpoch(1)
            .setDeadlineEpoch(2)
            .build();
    assert "80".repeat(32).equals(highId.orderIdHex()) : "raw order id must be fixed-width hex";
  }

  private static void testCompleteReplicationOrder() {
    final CompleteReplicationOrderInstruction instruction =
        CompleteReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setProviderIdHex(PROVIDER_ID)
            .setCompletionEpoch(31)
            .build();
    final Map<String, String> args = instruction.toArguments();
    assert "CompleteReplicationOrder".equals(args.get("action"))
        : "action mismatch";
    assert "31".equals(args.get("completion_epoch")) : "completion epoch mismatch";
    assert PROVIDER_ID.equals(args.get("provider_id_hex")) : "provider id mismatch";
    assert PROVIDER_ID.equals(instruction.providerIdHex()) : "provider id mismatch";
    assert instruction.completionEpoch() == 31 : "completion epoch mismatch";
  }

  private static void testCompleteReplicationOrderRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      CompleteReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setProviderIdHex(PROVIDER_ID)
          .setCompletionEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative completion epoch to throw";
  }

  private static void testExpireReplicationOrder() {
    final ExpireReplicationOrderInstruction instruction =
        ExpireReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setExpirationEpoch(32)
            .build();
    assert "ExpireReplicationOrder".equals(instruction.toArguments().get("action"));
    assert instruction.expirationEpoch() == 32 : "expiration epoch mismatch";
    assert instruction.equals(
        ExpireReplicationOrderInstruction.fromArguments(instruction.toArguments()));

    boolean threw = false;
    try {
      ExpireReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setExpirationEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative expiration epoch to throw";
  }

  private static void testArgumentDecodersRejectUnknownAndWrongAction() {
    final IssueReplicationOrderInstruction issue =
        IssueReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setOrderPayload(new byte[] {1})
            .setIssuedEpoch(1)
            .setDeadlineEpoch(2)
            .build();
    final Map<String, String> withUnknown = new LinkedHashMap<>(issue.toArguments());
    withUnknown.put("unexpected", "field");
    boolean threw = false;
    try {
      IssueReplicationOrderInstruction.fromArguments(withUnknown);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected unknown argument to throw";

    final Map<String, String> wrongAction = new LinkedHashMap<>(issue.toArguments());
    wrongAction.put("action", "CompleteReplicationOrder");
    threw = false;
    try {
      IssueReplicationOrderInstruction.fromArguments(wrongAction);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected wrong action to throw";
  }

}
