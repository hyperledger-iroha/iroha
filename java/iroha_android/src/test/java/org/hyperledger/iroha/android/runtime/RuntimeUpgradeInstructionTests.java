package org.hyperledger.iroha.android.runtime;

import java.util.Arrays;
import java.util.Base64;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.ActivateRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.CancelRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.ProposeRuntimeUpgradeInstruction;

/** Regression tests for runtime-upgrade instruction helpers. */
public final class RuntimeUpgradeInstructionTests {

  private RuntimeUpgradeInstructionTests() {}

  public static void main(final String[] args) {
    proposeRoundTrip();
    activateRoundTrip();
    cancelRoundTrip();
    System.out.println("[IrohaAndroid] RuntimeUpgradeInstructionTests passed.");
  }

  private static void proposeRoundTrip() {
    final byte[] manifest = new byte[] {0x01, 0x23, (byte) 0xFF};
    final InstructionBox box = InstructionBuilders.proposeRuntimeUpgrade(manifest);
    assert box.kind() == InstructionKind.CUSTOM : "propose kind mismatch";
    final String encoded = Base64.getEncoder().encodeToString(manifest);
    assert encoded.equals(box.arguments().get("manifest_bytes_base64"))
        : "manifest base64 argument mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof ProposeRuntimeUpgradeInstruction
        : "decoded payload type mismatch";
    final ProposeRuntimeUpgradeInstruction payload =
        (ProposeRuntimeUpgradeInstruction) decoded.payload();
    assert Arrays.equals(payload.manifestBytes(), manifest) : "manifest bytes mismatch";
    assert ProposeRuntimeUpgradeInstruction.ACTION.equals(box.arguments().get("action"))
        : "propose action missing";
  }

  private static void activateRoundTrip() {
    final String idUpperHex = "AB".repeat(32);
    final InstructionBox box = InstructionBuilders.activateRuntimeUpgrade(idUpperHex);
    assert box.kind() == InstructionKind.CUSTOM : "activate kind mismatch";
    assert idUpperHex.toLowerCase().equals(box.arguments().get("id_hex"))
        : "id hex argument mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof ActivateRuntimeUpgradeInstruction
        : "decoded payload type mismatch";
    final ActivateRuntimeUpgradeInstruction payload =
        (ActivateRuntimeUpgradeInstruction) decoded.payload();
    assert payload.idHex().equals(idUpperHex.toLowerCase()) : "payload id mismatch";
    assert ActivateRuntimeUpgradeInstruction.ACTION.equals(box.arguments().get("action"))
        : "activate action missing";
  }

  private static void cancelRoundTrip() {
    final String idUpperHex = "CD".repeat(32);
    final InstructionBox box = InstructionBuilders.cancelRuntimeUpgrade(idUpperHex);
    assert box.kind() == InstructionKind.CUSTOM : "cancel kind mismatch";
    assert idUpperHex.toLowerCase().equals(box.arguments().get("id_hex"))
        : "cancel id hex mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof CancelRuntimeUpgradeInstruction
        : "decoded payload type mismatch";
    final CancelRuntimeUpgradeInstruction payload =
        (CancelRuntimeUpgradeInstruction) decoded.payload();
    assert payload.idHex().equals(idUpperHex.toLowerCase()) : "payload id mismatch";
    assert CancelRuntimeUpgradeInstruction.ACTION.equals(box.arguments().get("action"))
        : "cancel action missing";
  }
}
