package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.IssueReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordReplicationReceiptInstruction;

/** Ensures the replication order builders round-trip through InstructionBox. */
public final class SorafsReplicationInstructionBuilderTests {

  private SorafsReplicationInstructionBuilderTests() {}

  private static final String ORDER_ID =
      "44b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c0";
  private static final String PROVIDER_ID =
      "51fdb0bf4c6a79ce51fdb0bf4c6a79ce51fdb0bf4c6a79ce51fdb0bf4c6a79ce";

  public static void main(final String[] args) {
    testIssueReplicationOrder();
    testIssueReplicationOrderRejectsInvalidBase64();
    testIssueReplicationOrderRejectsNegativeEpoch();
    testCompleteReplicationOrder();
    testCompleteReplicationOrderRejectsNegativeEpoch();
    testRecordReplicationReceipt();
    testRecordReplicationReceiptRejectsNegativeTimestamp();
    System.out.println(
        "[IrohaAndroid] SorafsReplicationInstructionBuilderTests passed (issue/complete/receipt).");
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

    final InstructionBox box =
        InstructionBuilders.issueReplicationOrder(payloadInstruction);
    assert "IssueReplicationOrder".equals(box.arguments().get("action"))
        : "action mismatch";
    assert ORDER_ID.equals(box.arguments().get("order_id_hex")) : "order_id_hex mismatch";
    assert payload.equals(box.arguments().get("order_payload_base64")) : "payload mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof IssueReplicationOrderInstruction
        : "Expected IssueReplicationOrderInstruction";
    final IssueReplicationOrderInstruction rehydrated =
        (IssueReplicationOrderInstruction) decoded.payload();
    assert ORDER_ID.equals(rehydrated.orderIdHex()) : "rehydrated orderId mismatch";
    assert payload.equals(rehydrated.orderPayloadBase64()) : "rehydrated payload mismatch";
    assert rehydrated.issuedEpoch() == 20 : "issued epoch mismatch";
    assert rehydrated.deadlineEpoch() == 28 : "deadline epoch mismatch";
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

  private static void testCompleteReplicationOrder() {
    final CompleteReplicationOrderInstruction instruction =
        CompleteReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setCompletionEpoch(31)
            .build();
    final InstructionBox box =
        InstructionBuilders.completeReplicationOrder(instruction);
    assert "CompleteReplicationOrder".equals(box.arguments().get("action"))
        : "action mismatch";
    assert "31".equals(box.arguments().get("completion_epoch")) : "completion epoch mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof CompleteReplicationOrderInstruction
        : "Expected CompleteReplicationOrderInstruction";
    final CompleteReplicationOrderInstruction payload =
        (CompleteReplicationOrderInstruction) decoded.payload();
    assert payload.completionEpoch() == 31 : "completion epoch mismatch";
  }

  private static void testCompleteReplicationOrderRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      CompleteReplicationOrderInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setCompletionEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative completion epoch to throw";
  }

  private static void testRecordReplicationReceipt() {
    final RecordReplicationReceiptInstruction instruction =
        RecordReplicationReceiptInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setProviderIdHex(PROVIDER_ID)
            .setStatus(RecordReplicationReceiptInstruction.Status.ACCEPTED)
            .setTimestamp(1_717_171_111L)
            .setPorSampleDigestHex("aabbccdd")
            .build();
    final InstructionBox box =
        InstructionBuilders.recordReplicationReceipt(instruction);
    final Map<String, String> args = box.arguments();
    assert "RecordReplicationReceipt".equals(args.get("action")) : "action mismatch";
    assert PROVIDER_ID.equals(args.get("provider_id_hex")) : "provider id mismatch";
    assert "Accepted".equals(args.get("status")) : "status mismatch";
    assert "aabbccdd".equals(args.get("por_sample_digest_hex")) : "por digest mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, args);
    assert decoded.payload() instanceof RecordReplicationReceiptInstruction
        : "Expected RecordReplicationReceiptInstruction";
    final RecordReplicationReceiptInstruction payload =
        (RecordReplicationReceiptInstruction) decoded.payload();
    assert payload.timestamp() == 1_717_171_111L : "timestamp mismatch";
    assert payload.status() == RecordReplicationReceiptInstruction.Status.ACCEPTED
        : "status mismatch";
  }

  private static void testRecordReplicationReceiptRejectsNegativeTimestamp() {
    boolean threw = false;
    try {
      RecordReplicationReceiptInstruction.builder()
          .setOrderIdHex(ORDER_ID)
          .setProviderIdHex(PROVIDER_ID)
          .setStatus(RecordReplicationReceiptInstruction.Status.ACCEPTED)
          .setTimestamp(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative timestamp to throw";
  }
}
