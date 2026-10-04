package org.hyperledger.iroha.sdk.sorafs;

import static org.junit.jupiter.api.Assertions.*;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.sdk.core.model.instructions.UpsertProviderCreditInstruction;
import org.hyperledger.iroha.sdk.core.util.HashLiteral;
import org.junit.jupiter.api.Test;

/** Java consumers use the Kotlin-owned guarded credit argument template. */
public final class SorafsProviderCreditJavaConsumerTest {
  @Test void migratedCreditAssertionsAndExplicitAbsenceUseCanonicalKotlin() {
    UpsertProviderCreditInstruction instruction = credit(null);
    Map<String, String> args = instruction.getArguments();
    // These four assertions are retained from the retired Java builder suite.
    assertEquals("UpsertProviderCredit", args.get("action"), "action mismatch");
    assertEquals("123456789000", args.get("record.available_credit_nano"), "available credit mismatch");
    assertEquals(Integer.valueOf(2), instruction.underDeliveryStrikes, "strikes mismatch");
    assertEquals("jp", instruction.getMetadata().get("region"), "metadata mismatch");
    assertTrue(args.containsKey("expected_current"));
    assertEquals("null", args.get("expected_current"));
    assertNull(instruction.expectedCurrent);
    UpsertProviderCreditInstruction decoded = UpsertProviderCreditInstruction.fromArguments(args);
    assertEquals(instruction, decoded);
    assertEquals(instruction.hashCode(), decoded.hashCode());
  }

  @Test void exactCurrentGuardSurvivesJavaRoundtripAndCallerMutation() {
    byte[] bytes = new byte[32];
    java.util.Arrays.fill(bytes, (byte) 0xAB);
    String guard = HashLiteral.canonicalize(bytes);
    UpsertProviderCreditInstruction instruction = credit(guard);
    Map<String, String> args = new LinkedHashMap<>(instruction.getArguments());
    UpsertProviderCreditInstruction decoded = UpsertProviderCreditInstruction.fromArguments(args);
    args.put("expected_current", "null");
    instruction.getArguments().put("expected_current", "null");
    instruction.getMetadata().put("region", "altered");
    assertEquals(guard, instruction.expectedCurrent);
    assertEquals(guard, decoded.getArguments().get("expected_current"));
    assertEquals("jp", instruction.getMetadata().get("region"));
    assertNotEquals(credit(null), instruction);
    args.remove("expected_current");
    assertThrows(IllegalArgumentException.class, () -> UpsertProviderCreditInstruction.fromArguments(args));
  }

  private static UpsertProviderCreditInstruction credit(String expectedCurrent) {
    StringBuilder provider = new StringBuilder();
    for (int i = 0; i < 32; i++) provider.append("11");
    Map<String, String> metadata = new LinkedHashMap<>();
    metadata.put("region", "jp");
    metadata.put("tier", "cold");
    return new UpsertProviderCreditInstruction(expectedCurrent, provider.toString(),
        new BigInteger("123456789000"), new BigInteger("555000000000"),
        new BigInteger("777000000000"), new BigInteger("333000000000"),
        1700000L, 1700800L, 1700500L, new BigInteger("1000"), 2, 1700600L, metadata);
  }
}
