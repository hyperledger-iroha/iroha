package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.ExpireReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.IssueReplicationOrderInstruction;
import org.hyperledger.iroha.android.model.instructions.ProviderIngestCompletionAuthorityV1;
import org.hyperledger.iroha.android.model.instructions.ProviderIngestCompletionSignerPolicyV1;
import org.hyperledger.iroha.android.model.instructions.ProviderIngestFinalizedAnchorV1;

/** Ensures the replication order builders emit the expected argument schema. */
public final class SorafsReplicationInstructionBuilderTests {

  private SorafsReplicationInstructionBuilderTests() {}

  private static final String ORDER_ID =
      "44b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c0";
  private static final String MUSUBI_ARCHIVE_ID = "45".repeat(32);
  private static final String PROVIDER_ID = "11".repeat(32);
  private static final String PROVIDER_OWNER =
      "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
  private static final String POLICY_ID = "21".repeat(32);
  private static final String PREDECESSOR_DIGEST = "32".repeat(32);
  private static final String POLICY_DIGEST = "43".repeat(32);
  private static final String BLOCK_HASH = "54".repeat(32);

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
    assert payloadInstruction.musubiArchiveIdHex() == null
        : "ordinary order must omit the Musubi archive purpose";

    final IssueReplicationOrderInstruction bound =
        IssueReplicationOrderInstruction.builder()
            .setOrderIdHex(ORDER_ID)
            .setOrderPayloadBase64(payload)
            .setIssuedEpoch(20)
            .setDeadlineEpoch(28)
            .setMusubiArchiveIdHex(MUSUBI_ARCHIVE_ID)
            .build();
    assert MUSUBI_ARCHIVE_ID.equals(bound.toArguments().get("musubi_archive_id_hex"))
        : "Musubi archive argument mismatch";
    assert bound.equals(IssueReplicationOrderInstruction.fromArguments(bound.toArguments()))
        : "bound issue argument roundtrip mismatch";
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

    threw = false;
    try {
      IssueReplicationOrderInstruction.builder().setMusubiArchiveIdHex("00".repeat(32));
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected zero Musubi archive identifier to throw";

    final byte[] highBytes = new byte[32];
    java.util.Arrays.fill(highBytes, (byte) 0x80);
    final IssueReplicationOrderInstruction highId =
        IssueReplicationOrderInstruction.builder()
            .setOrderId(highBytes)
            .setOrderPayload(new byte[] {1})
            .setIssuedEpoch(1)
            .setDeadlineEpoch(2)
            .setMusubiArchiveId(highBytes)
            .build();
    assert "80".repeat(32).equals(highId.orderIdHex()) : "raw order id must be fixed-width hex";
    assert "80".repeat(32).equals(highId.musubiArchiveIdHex())
        : "raw Musubi archive id must be fixed-width hex";
  }

  private static void testCompleteReplicationOrder() {
    final CompleteReplicationOrderInstruction instruction =
        CompleteReplicationOrderInstruction.builder()
            .setOrderId(ORDER_ID)
            .setProviderId(PROVIDER_ID)
            .setCompletionEpoch(31)
            .setExpectedAuthority(authority())
            .setExpectedAssignmentRevision(3)
            .setFinalizedAnchor(anchor())
            .build();
    final Map<String, String> args = instruction.toArguments();
    assert "CompleteReplicationOrder".equals(args.get("action"))
        : "action mismatch";
    assert "31".equals(args.get("completion_epoch")) : "completion epoch mismatch";
    assert PROVIDER_ID.equals(args.get("provider_id")) : "provider id mismatch";
    assert PROVIDER_ID.equals(instruction.providerId()) : "provider id mismatch";
    assert args.keySet()
        .equals(
            new java.util.LinkedHashSet<>(
                java.util.Arrays.asList(
                    "action",
                    "order_id",
                    "provider_id",
                    "completion_epoch",
                    "expected_authority",
                    "expected_assignment_revision",
                    "finalized_anchor"))) : "completion hard-cut fields mismatch";
    assert instruction.completionEpoch() == 31 : "completion epoch mismatch";
    assert instruction.equals(
        CompleteReplicationOrderInstruction.fromArguments(instruction.toArguments()));
  }

  private static void testCompleteReplicationOrderRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      CompleteReplicationOrderInstruction.builder()
          .setOrderId(ORDER_ID)
          .setProviderId(PROVIDER_ID)
          .setCompletionEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative completion epoch to throw";

    threw = false;
    try {
      CompleteReplicationOrderInstruction.builder()
          .setOrderId(ORDER_ID)
          .setProviderId(PROVIDER_ID)
          .setCompletionEpoch(31)
          .setExpectedAuthority(authority())
          .setExpectedAssignmentRevision(0);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected zero assignment revision to throw";

    final Map<String, String> retiredThreeField = new LinkedHashMap<>();
    retiredThreeField.put("action", "CompleteReplicationOrder");
    retiredThreeField.put("order_id", ORDER_ID);
    retiredThreeField.put("provider_id", PROVIDER_ID);
    retiredThreeField.put("completion_epoch", "31");
    threw = false;
    try {
      CompleteReplicationOrderInstruction.fromArguments(retiredThreeField);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected retired three-field completion to throw";
  }

  private static ProviderIngestCompletionAuthorityV1 authority() {
    return new ProviderIngestCompletionAuthorityV1(
        PROVIDER_OWNER,
        new ProviderIngestCompletionSignerPolicyV1(
            POLICY_ID, 2, PREDECESSOR_DIGEST, POLICY_DIGEST));
  }

  private static ProviderIngestFinalizedAnchorV1 anchor() {
    return new ProviderIngestFinalizedAnchorV1(41, BLOCK_HASH);
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
