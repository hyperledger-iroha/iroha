package org.hyperledger.iroha.android.model;

import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.junit.Test;

public final class InstructionBoxTests {

  @Test
  public void fromNoritoDetectsWirePayloadArguments() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-payload", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final Map<String, String> args = new LinkedHashMap<>();
    args.put("wire_name", "iroha.custom");
    args.put("payload_base64", Base64.getEncoder().encodeToString(wirePayload));

    final InstructionBox box = InstructionBox.fromNorito(InstructionKind.CUSTOM, args);

    assert box.payload() instanceof InstructionBox.WirePayload
        : "Expected wire payload to be detected from arguments";
    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
    assert "iroha.custom".equals(wire.wireName()) : "Wire name should round-trip";
    assert Arrays.equals(wirePayload, wire.payloadBytes())
        : "Wire payload bytes should round-trip";
  }
}
