package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/** Java consumers share Kotlin's ciphertext-only execution response contract. */
final class RamLfeExecuteJavaConsumerTest {
  @Test
  void parsesCiphertextAndReceiptWithoutInventingAPlaintextOpening() {
    String json = HttpClientTransportTestSupportKt.ramLfeExecuteResponseJson();
    RamLfeExecuteResponse response = RamLfeJsonParser.parseExecuteResponse(
        json.getBytes(StandardCharsets.UTF_8));
    assertEquals("identifier_lookup_retail", response.programId);
    assertEquals("abcd", response.outputCiphertext);
    assertEquals(42L, response.executedAtMs);
    assertTrue(response.receipt.containsKey("payload"));
  }

  @Test
  void rejectsRetiredOpeningEvenWhenTheRemainingResponseIsValid() {
    String json = HttpClientTransportTestSupportKt.ramLfeExecuteResponseJson();
    String changed = json.replaceFirst("\\{", "{\"output_opening\":{},");
    IllegalStateException error = assertThrows(IllegalStateException.class,
        () -> RamLfeJsonParser.parseExecuteResponse(changed.getBytes(StandardCharsets.UTF_8)));
    assertTrue(error.getMessage().contains("ram-lfe execute response.output_opening"));
  }

  @Test
  void rejectsRetiredAndUnknownBackendAndModeTags() {
    String json = HttpClientTransportTestSupportKt.ramLfeExecuteResponseJson();
    for (String backend : new String[] {
        "bfv-affine-sha3-256-v1", "bfv-programmed-sha3-256-v1", "unknown"}) {
      String changed = json.replace("\"backend\": \"bfv-programmed-v1\"",
          "\"backend\": \"" + backend + "\"");
      assertTrue(!changed.equals(json));
      IllegalStateException error = assertThrows(IllegalStateException.class,
          () -> RamLfeJsonParser.parseExecuteResponse(changed.getBytes(StandardCharsets.UTF_8)));
      assertTrue(error.getMessage().contains("ram-lfe execute response.backend"));
    }
    for (String mode : new String[] {"unknown", "ivm-proved"}) {
      String changed = json.replace("\"verification_mode\": \"signed\"",
          "\"verification_mode\": \"" + mode + "\"");
      assertTrue(!changed.equals(json));
      IllegalStateException error = assertThrows(IllegalStateException.class,
          () -> RamLfeJsonParser.parseExecuteResponse(changed.getBytes(StandardCharsets.UTF_8)));
      assertTrue(error.getMessage().contains("ram-lfe execute response.verification_mode"));
    }
  }

  @Test
  void kotlinOwnedEncryptorsExposeAStableRefusalToJava() {
    String key = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29";
    IdentifierPolicySummary policy = new IdentifierPolicySummary(
        "email#retail", "lookup", "owner", true, IdentifierNormalization.EXACT,
        key, "bfv-affine-v1", "bfv-v1", null, null, null, key);
    String hash = String.join("", java.util.Collections.nCopies(32, "11"));
    RamLfeOutputOpening opening = new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload("lookup", hash, hash, hash, hash, hash, 1L, null), "00");
    RamLfeEncryptionUnavailableException error = assertThrows(
        RamLfeEncryptionUnavailableException.class, () -> policy.encryptInput("private@example.org"));
    assertEquals("ram_lfe_encryption_unavailable", error.code);
    assertTrue(!error.getMessage().contains("private@example.org"));
    assertThrows(RamLfeEncryptionUnavailableException.class,
        () -> policy.encryptedRequestFromInput("private@example.org", opening));
    assertThrows(RamLfeEncryptionUnavailableException.class,
        () -> IdentifierResolveRequest.encryptedFromInput(policy, "private@example.org", opening));
  }
}
