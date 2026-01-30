package org.hyperledger.iroha.android.tx;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.junit.Test;

public final class TransactionPayloadFixtureTests {

  @Test
  public void validatePayloadFixtures() throws Exception {
    runFixtures();
  }

  @Test
  public void fixtureLoaderAcceptsWireInstructionEntries() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-fixture", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final String payloadBase64 = Base64.getEncoder().encodeToString(wirePayload);

    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("wire_name", "iroha.custom");
    instruction.put("payload_base64", payloadBase64);
    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("chain", "00000001");
    payload.put("authority", "alice@wonderland");
    payload.put("creation_time_ms", 0L);
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap = new LinkedHashMap<>();
    fixtureMap.put("name", "wire_instruction_fixture");
    fixtureMap.put("chain", "00000001");
    fixtureMap.put("authority", "alice@wonderland");
    fixtureMap.put("creation_time_ms", 0L);
    fixtureMap.put("payload", payload);

    final TransactionPayloadFixtures.Fixture fixture =
        TransactionPayloadFixtures.Fixture.fromObject(fixtureMap);
    final TransactionPayload decoded = fixture.toPayload();
    assert decoded.executable().isInstructions() : "Expected instruction executable";
    final List<InstructionBox> boxes = decoded.executable().instructions();
    assert boxes.size() == 1 : "Expected one instruction";
    final InstructionBox box = boxes.get(0);
    assert box.payload() instanceof InstructionBox.WirePayload : "Expected wire payload";
    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
    assert "iroha.custom".equals(wire.wireName()) : "Wire name should round-trip";
    assert Arrays.equals(wirePayload, wire.payloadBytes()) : "Wire payload bytes should round-trip";
  }

  @Test
  public void fixtureLoaderRejectsWireInstructionArguments() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-arguments", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final String payloadBase64 = Base64.getEncoder().encodeToString(wirePayload);

    final Map<String, Object> wireArgs = new LinkedHashMap<>();
    wireArgs.put("wire_name", "iroha.custom");
    wireArgs.put("payload_base64", payloadBase64);

    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("arguments", wireArgs);
    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("chain", "00000001");
    payload.put("authority", "alice@wonderland");
    payload.put("creation_time_ms", 0L);
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap = new LinkedHashMap<>();
    fixtureMap.put("name", "wire_instruction_arguments_fixture");
    fixtureMap.put("chain", "00000001");
    fixtureMap.put("authority", "alice@wonderland");
    fixtureMap.put("creation_time_ms", 0L);
    fixtureMap.put("payload", payload);

    final TransactionPayloadFixtures.Fixture fixture =
        TransactionPayloadFixtures.Fixture.fromObject(fixtureMap);
    assertThrows(
        fixture::toPayload,
        "expected wire payload arguments to be rejected");
  }

  public static void main(final String[] args) throws Exception {
    runFixtures();
  }

  private static void runFixtures() throws Exception {
    final Path path = TransactionPayloadFixtures.resolveFixturePath();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    for (TransactionPayloadFixtures.Fixture fixture : TransactionPayloadFixtures.load(path)) {
      final String name = fixture.name();
      if (!fixture.isDecodable()) {
        throw new IllegalStateException(name + ": encoded payload not decodable");
      }
      final TransactionPayload payload = fixture.toPayload();
      assert Objects.equals(fixture.chain(), payload.chainId())
          : name + ": chain mismatch vs fixture metadata";
      assert Objects.equals(fixture.authority(), payload.authority())
          : name + ": authority mismatch vs fixture metadata";
      assert fixture.creationTimeMs() == payload.creationTimeMs()
          : name + ": creation_time_ms mismatch vs fixture metadata";
      assert Objects.equals(fixture.timeToLiveMs(), payload.timeToLiveMs())
          : name + ": TTL mismatch vs fixture metadata";
      assert Objects.equals(fixture.nonce(), payload.nonce())
          : name + ": nonce mismatch vs fixture metadata";
      final byte[] encoded = adapter.encodeTransaction(payload);
      fixture.encoded().ifPresent(expected -> {
        if (payload.executable().isInstructions()) {
          assert allInstructionsWire(payload)
              : name + ": instruction fixtures must use wire payloads";
          final String actual = Base64.getEncoder().encodeToString(encoded);
          assert expected.equals(actual) : name + ": encoded payload mismatch";
          return;
        }
        final String actual = Base64.getEncoder().encodeToString(encoded);
        assert expected.equals(actual) : name + ": encoded payload mismatch";
      });
      if (fixture.encoded().isEmpty()) {
        final String base64 = Base64.getEncoder().encodeToString(encoded);
        System.out.println("[fixture] " + name + "=" + base64);
      }
      final TransactionPayload decoded = adapter.decodeTransaction(encoded);
      assertPayloadEquals(name, payload, decoded);
    }
    System.out.println("[IrohaAndroid] Transaction payload fixture tests passed.");
  }

  private static void assertPayloadEquals(
      final String name, final TransactionPayload expected, final TransactionPayload actual) {
    assert Objects.equals(expected.chainId(), actual.chainId())
        : name + ": chain mismatch";
    assert Objects.equals(expected.authority(), actual.authority())
        : name + ": authority mismatch";
    assert expected.creationTimeMs() == actual.creationTimeMs()
        : name + ": creation time mismatch";
    if (expected.executable().isIvm()) {
      assert actual.executable().isIvm() : name + ": executable type mismatch";
      assert java.util.Arrays.equals(expected.executable().ivmBytes(), actual.executable().ivmBytes())
          : name + ": IVM bytes mismatch";
    } else {
      assert actual.executable().isInstructions() : name + ": executable type mismatch";
      final java.util.List<InstructionBox> expectedInstr = expected.executable().instructions();
      final java.util.List<InstructionBox> actualInstr = actual.executable().instructions();
      assert expectedInstr.size() == actualInstr.size() : name + ": instruction count mismatch";
      for (int i = 0; i < expectedInstr.size(); i++) {
        final InstructionBox expectedBox = expectedInstr.get(i);
        final InstructionBox actualBox = actualInstr.get(i);
        assert expectedBox.kind() == actualBox.kind()
            : name + ": instruction kind mismatch at index " + i;
        assert Objects.equals(expectedBox.arguments(), actualBox.arguments())
            : name + ": instruction arguments mismatch at index " + i;
        assert expectedBox.payload().getClass().equals(actualBox.payload().getClass())
            : name + ": instruction payload type mismatch at index " + i;
      }
    }
    assert Objects.equals(expected.timeToLiveMs(), actual.timeToLiveMs())
        : name + ": TTL mismatch";
    assert Objects.equals(expected.nonce(), actual.nonce())
        : name + ": nonce mismatch";
      assert Objects.equals(expected.metadata(), actual.metadata())
        : name + ": metadata mismatch";
  }

  private static boolean allInstructionsWire(final TransactionPayload payload) {
    if (!payload.executable().isInstructions()) {
      return false;
    }
    for (InstructionBox box : payload.executable().instructions()) {
      if (!(box.payload() instanceof InstructionBox.WirePayload)) {
        return false;
      }
    }
    return true;
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
