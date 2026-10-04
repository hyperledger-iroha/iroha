package org.hyperledger.iroha.sdk.core.model;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.core.model.instructions.CompleteReplicationOrderInstruction;
import org.hyperledger.iroha.sdk.core.model.instructions.ExpireReplicationOrderInstruction;
import org.hyperledger.iroha.sdk.core.model.instructions.IssueReplicationOrderInstruction;
import org.hyperledger.iroha.sdk.core.model.instructions.ProviderIngestCompletionAuthorityV1;
import org.hyperledger.iroha.sdk.core.model.instructions.ProviderIngestCompletionSignerPolicyV1;
import org.hyperledger.iroha.sdk.core.model.instructions.ProviderIngestFinalizedAnchorV1;
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys;
import org.junit.jupiter.api.Test;

/** Every retired Java replication-builder assertion now consumes the canonical Kotlin API. */
public final class ReplicationOrderJavaConsumerTest {
  private static final String ORDER_ID =
      "44b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c0";
  private static final String ARCHIVE_ID = repeated("45", 32);
  private static final String PROVIDER_ID = repeated("11", 32);
  private static final String OWNER =
      "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
  private static final String SIGNER = signer();

  private static String signer() {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x71), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final org.hyperledger.iroha.sdk.address.AccountAddressException error) {
      throw new AssertionError("valid deterministic signer", error);
    }
  }

  private static String repeated(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) result.append(value);
    return result.toString();
  }

  private static ProviderIngestCompletionAuthorityV1 authority() {
    return new ProviderIngestCompletionAuthorityV1(OWNER, SIGNER,
        new ProviderIngestCompletionSignerPolicyV1(
            repeated("21", 32), 2L, repeated("32", 32), repeated("43", 32)));
  }

  private static ProviderIngestFinalizedAnchorV1 anchor() {
    return new ProviderIngestFinalizedAnchorV1(41L, repeated("54", 32));
  }

  private static CompleteReplicationOrderInstruction complete(final long epoch, final long revision) {
    return new CompleteReplicationOrderInstruction(
        ORDER_ID, PROVIDER_ID, epoch, authority(), revision, anchor());
  }

  @Test
  public void issueFieldsAndOptionalArchiveRoundTrip() {
    final String payload = Base64.getEncoder().encodeToString(
        "replication-order".getBytes(StandardCharsets.UTF_8));
    final IssueReplicationOrderInstruction instruction =
        new IssueReplicationOrderInstruction(ORDER_ID, payload, 20L, 28L, null);
    final Map<String, String> args = instruction.getArguments();
    assertEquals("IssueReplicationOrder", args.get("action"));
    assertEquals(ORDER_ID, args.get("order_id_hex"));
    assertEquals(payload, args.get("order_payload_base64"));
    assertEquals(ORDER_ID, instruction.getOrderIdHex());
    assertEquals(payload, instruction.getOrderPayloadBase64());
    assertEquals(20L, instruction.getIssuedEpoch());
    assertEquals(28L, instruction.getDeadlineEpoch());
    assertNull(instruction.getMusubiArchiveIdHex());
    final IssueReplicationOrderInstruction bound =
        new IssueReplicationOrderInstruction(ORDER_ID, payload, 20L, 28L, ARCHIVE_ID);
    assertEquals(ARCHIVE_ID, bound.getArguments().get("musubi_archive_id_hex"));
    assertEquals(bound, IssueReplicationOrderInstruction.fromArguments(bound.getArguments()));
  }

  @Test
  public void issueRejectsInvalidBase64AndNegativeEpochs() {
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID, "not!base64", 1L, 2L, null));
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID, "AAECAw==", -1L, 10L, null));
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID, "AAECAw==", 1L, -1L, null));
  }

  @Test
  public void issueRetainsEveryMalformedInputAndUnsignedByteControl() {
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(repeated("AA", 32), "AQ==", 1L, 2L, null));
    assertThrows(IllegalArgumentException.class,
        () -> IssueReplicationOrderInstruction.fromOrderBytes(new byte[32], new byte[] {1}, 1L, 2L, null));
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID, "AQ==", 10L, 10L, null));
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID,
            Base64.getEncoder().encodeToString(new byte[1024 * 1024 + 1]), 1L, 2L, null));
    assertThrows(IllegalArgumentException.class,
        () -> new IssueReplicationOrderInstruction(ORDER_ID, "AQ==", 1L, 2L, repeated("00", 32)));
    final byte[] highBytes = new byte[32];
    Arrays.fill(highBytes, (byte) 0x80);
    final IssueReplicationOrderInstruction highId = IssueReplicationOrderInstruction.fromOrderBytes(
        highBytes, new byte[] {1}, 1L, 2L, highBytes);
    assertEquals(repeated("80", 32), highId.getOrderIdHex());
    assertEquals(repeated("80", 32), highId.getMusubiArchiveIdHex());
  }

  @Test
  public void completionCarriesDistinctMandatorySignerAndFullOriginalArguments() {
    final CompleteReplicationOrderInstruction instruction = complete(31L, 3L);
    final Map<String, String> args = instruction.getArguments();
    assertEquals("CompleteReplicationOrder", args.get("action"));
    assertEquals("31", args.get("completion_epoch"));
    assertEquals(PROVIDER_ID, args.get("provider_id"));
    assertEquals(PROVIDER_ID, instruction.getProviderId());
    assertEquals(new LinkedHashSet<>(Arrays.asList("action", "order_id", "provider_id",
        "completion_epoch", "expected_authority", "expected_assignment_revision", "finalized_anchor")),
        args.keySet());
    assertEquals(31L, instruction.getCompletionEpoch());
    assertEquals(instruction, CompleteReplicationOrderInstruction.fromArguments(args));
    assertEquals(OWNER, instruction.getExpectedAuthority().getProviderOwner());
    assertEquals(SIGNER, instruction.getExpectedAuthority().getCompletionSigner());
    assertNotEquals(OWNER, SIGNER);
    assertEquals(authority(), instruction.getExpectedAuthority());
    assertEquals(authority().hashCode(), instruction.getExpectedAuthority().hashCode());
    args.clear();
    assertEquals(7, instruction.getArguments().size());
  }

  @Test
  public void completionRejectsNegativeEpochZeroRevisionAndRetiredLayout() {
    assertThrows(IllegalArgumentException.class, () -> complete(-1L, 3L));
    assertThrows(IllegalArgumentException.class, () -> complete(31L, 0L));
    final Map<String, String> retired = new LinkedHashMap<>();
    retired.put("action", "CompleteReplicationOrder");
    retired.put("order_id", ORDER_ID);
    retired.put("provider_id", PROVIDER_ID);
    retired.put("completion_epoch", "31");
    assertThrows(IllegalArgumentException.class,
        () -> CompleteReplicationOrderInstruction.fromArguments(retired));
    final Map<String, String> args = complete(31L, 3L).getArguments();
    final String original = args.get("expected_authority");
    for (final String changed : Arrays.asList(
        original.replace("\"completion_signer\":\"" + SIGNER + "\",", ""),
        original.replace("\"completion_signer\":\"" + SIGNER + "\"", "\"completion_signer\":null"),
        original.replace("\"completion_signer\":\"" + SIGNER + "\"", "\"completion_signer\":\"invalid\""),
        original.replace("\"signer_policy\":", "\"extra\":0,\"signer_policy\":"))) {
      args.put("expected_authority", changed);
      assertThrows(IllegalArgumentException.class,
          () -> CompleteReplicationOrderInstruction.fromArguments(args));
    }
  }

  @Test
  public void expirationFieldsAndRefusalsRemainCanonical() {
    final ExpireReplicationOrderInstruction instruction = new ExpireReplicationOrderInstruction(ORDER_ID, 32L);
    assertEquals("ExpireReplicationOrder", instruction.getArguments().get("action"));
    assertEquals(32L, instruction.getExpirationEpoch());
    assertEquals(instruction, ExpireReplicationOrderInstruction.fromArguments(instruction.getArguments()));
    assertThrows(IllegalArgumentException.class, () -> new ExpireReplicationOrderInstruction(ORDER_ID, -1L));
  }

  @Test
  public void argumentDecodersKeepUnknownFieldAndWrongActionRefusals() {
    final IssueReplicationOrderInstruction instruction =
        new IssueReplicationOrderInstruction(ORDER_ID, "AQ==", 1L, 2L, null);
    final Map<String, String> unknown = new LinkedHashMap<>(instruction.getArguments());
    unknown.put("unexpected", "field");
    assertThrows(IllegalArgumentException.class, () -> IssueReplicationOrderInstruction.fromArguments(unknown));
    final Map<String, String> wrongAction = new LinkedHashMap<>(instruction.getArguments());
    wrongAction.put("action", "CompleteReplicationOrder");
    assertThrows(IllegalArgumentException.class, () -> IssueReplicationOrderInstruction.fromArguments(wrongAction));
  }
}
