package org.hyperledger.iroha.android.tx;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.Test;

public final class TransactionPayloadFixturesTests {

  @Test
  public void toPayloadRejectsMissingWireInstructionFields() {
    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("wire_name", "iroha.register");

    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("chain", "00000002");
    payload.put("authority", "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp");
    payload.put("creation_time_ms", 1_735_000_000_000L);
    payload.put("executable", executable);
    payload.put("metadata", new LinkedHashMap<>());

    final Map<String, Object> fixture = new LinkedHashMap<>();
    fixture.put("name", "missing-wire-fields");
    fixture.put("chain", "00000002");
    fixture.put("authority", "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp");
    fixture.put("creation_time_ms", 1_735_000_000_000L);
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
