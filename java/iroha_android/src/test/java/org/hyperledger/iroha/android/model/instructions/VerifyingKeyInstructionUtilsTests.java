package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.HashMap;
import java.util.Map;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;

public final class VerifyingKeyInstructionUtilsTests {

  private VerifyingKeyInstructionUtilsTests() {}

  public static void main(final String[] args) {
    deprecationHeightMapsToWithdrawHeight();
    deprecationHeightMismatchThrows();
    System.out.println("[IrohaAndroid] VerifyingKeyInstructionUtilsTests passed.");
  }

  private static void deprecationHeightMapsToWithdrawHeight() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.deprecation_height", "42");
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa");
    final Map<String, String> encoded = record.toArguments("halo2/ipa");
    assert "42".equals(encoded.get("record.withdraw_height"))
        : "deprecation height should map to withdraw_height";
    assert !encoded.containsKey("record.deprecation_height")
        : "deprecation height should not be emitted";
  }

  private static void deprecationHeightMismatchThrows() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.withdraw_height", "7");
    arguments.put("record.deprecation_height", "8");
    assertThrows(
        () -> VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa"),
        "expected mismatched deprecation/withdraw heights to fail");
  }

  private static Map<String, String> baseArguments() {
    final Map<String, String> arguments = new HashMap<>();
    arguments.put("record.version", "1");
    arguments.put("record.circuit_id", "vk-test");
    arguments.put("record.backend_tag", "halo2-ipa-pasta");
    arguments.put("record.public_inputs_schema_hash_hex", repeatChar('a', 64));
    arguments.put("record.gas_schedule_id", "default");
    arguments.put(
        "record.vk_bytes_b64", Base64.getEncoder().encodeToString(new byte[] {1, 2, 3}));
    return arguments;
  }

  private static String repeatChar(final char value, final int count) {
    final StringBuilder builder = new StringBuilder(count);
    for (int i = 0; i < count; i++) {
      builder.append(value);
    }
    return builder.toString();
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
