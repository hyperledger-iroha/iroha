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
}
