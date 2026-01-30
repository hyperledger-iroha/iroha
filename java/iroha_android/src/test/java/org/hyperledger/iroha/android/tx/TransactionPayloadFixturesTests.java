package org.hyperledger.iroha.android.tx;

import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.Test;

public final class TransactionPayloadFixturesTests {

  @Test
  public void toPayloadRejectsLegacyInstructionArguments() {
    final Map<String, Object> arguments = new LinkedHashMap<>();
    arguments.put("action", "RegisterDomain");
    arguments.put("domain", "wonderland");

    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("wire_name", "iroha.register");
    instruction.put(
        "payload_base64",
        Base64.getEncoder().encodeToString(new byte[] {0x01}));
    instruction.put("arguments", arguments);

    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("chain", "00000002");
    payload.put("authority", "alice@wonderland");
    payload.put("creation_time_ms", 1_735_000_000_000L);
    payload.put("executable", executable);
    payload.put("metadata", new LinkedHashMap<>());

    final Map<String, Object> fixture = new LinkedHashMap<>();
    fixture.put("name", "legacy-instructions");
    fixture.put("chain", "00000002");
    fixture.put("authority", "alice@wonderland");
    fixture.put("creation_time_ms", 1_735_000_000_000L);
    fixture.put("payload", payload);

    final TransactionPayloadFixtures.Fixture parsed =
        TransactionPayloadFixtures.Fixture.fromObject(fixture);
    assertThrows(
        parsed::toPayload,
        "expected legacy instruction arguments to be rejected");
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
