package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;

public final class TriggerInstructionUtilsTests {

  private TriggerInstructionUtilsTests() {}

  public static void main(final String[] args) {
    rejectsNonWireInstructions();
    rejectsMissingWireArguments();
    rejectsExtraWireArguments();
    parsesWireInstructionArguments();
    System.out.println("[IrohaAndroid] TriggerInstructionUtilsTests passed.");
  }

  private static void rejectsNonWireInstructions() {
    final InstructionBox nonWire =
        RegisterDomainInstruction.builder().setDomainId("wonderland").build().toInstructionBox();
    final Map<String, String> target = new LinkedHashMap<>();
    assertThrows(
        () -> TriggerInstructionUtils.appendInstructions(List.of(nonWire), target),
        "Expected non-wire trigger instruction to be rejected");
  }

  private static void rejectsMissingWireArguments() {
    final Map<String, String> args = new LinkedHashMap<>();
    args.put("instruction.0.kind", InstructionKind.CUSTOM.displayName());
    args.put("instruction.0.arg.action", "RegisterDomain");
    assertThrows(
        () -> TriggerInstructionUtils.parseInstructions(args),
        "Expected missing wire_name/payload_base64 to be rejected");
  }

  private static void rejectsExtraWireArguments() {
    final Map<String, String> args = new LinkedHashMap<>();
    args.put("instruction.0.kind", InstructionKind.CUSTOM.displayName());
    args.put("instruction.0.arg.wire_name", "iroha.custom");
    args.put("instruction.0.arg.payload_base64", "AAECAw==");
    args.put("instruction.0.arg.action", "LegacyAction");
    assertThrows(
        () -> TriggerInstructionUtils.parseInstructions(args),
        "Expected extra wire arguments to be rejected");
  }

  private static void parsesWireInstructionArguments() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-trigger", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final Map<String, String> args = new LinkedHashMap<>();
    args.put("instruction.0.kind", InstructionKind.CUSTOM.displayName());
    args.put("instruction.0.arg.wire_name", "iroha.custom");
    args.put(
        "instruction.0.arg.payload_base64",
        Base64.getEncoder().encodeToString(wirePayload));

    final List<InstructionBox> instructions = TriggerInstructionUtils.parseInstructions(args);
    assert instructions.size() == 1 : "Expected one instruction";
    final InstructionBox box = instructions.get(0);
    assert box.payload() instanceof InstructionBox.WirePayload : "Expected wire payload";
    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
    assert "iroha.custom".equals(wire.wireName()) : "Wire name mismatch";
    assert java.util.Arrays.equals(wirePayload, wire.payloadBytes())
        : "Wire payload mismatch";
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    boolean threw = false;
    try {
      runnable.run();
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : message;
  }
}
