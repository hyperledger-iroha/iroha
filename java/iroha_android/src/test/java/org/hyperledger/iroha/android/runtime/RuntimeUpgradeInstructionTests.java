package org.hyperledger.iroha.android.runtime;

import java.util.Arrays;
import java.util.Base64;
import org.hyperledger.iroha.android.model.instructions.ActivateRuntimeUpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.CancelRuntimeUpgradeInstruction;
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
    final ProposeRuntimeUpgradeInstruction instruction =
        ProposeRuntimeUpgradeInstruction.builder().setManifestBytes(manifest).build();
    final String encoded = Base64.getEncoder().encodeToString(manifest);
    assert encoded.equals(instruction.toArguments().get("manifest_bytes_base64"))
        : "manifest base64 argument mismatch";
    assert Arrays.equals(instruction.manifestBytes(), manifest) : "manifest bytes mismatch";
    assert ProposeRuntimeUpgradeInstruction.ACTION.equals(instruction.toArguments().get("action"))
        : "propose action missing";
  }

  private static void activateRoundTrip() {
    final String idUpperHex = "AB".repeat(32);
    final ActivateRuntimeUpgradeInstruction instruction =
        ActivateRuntimeUpgradeInstruction.builder().setIdHex(idUpperHex).build();
    assert idUpperHex.toLowerCase().equals(instruction.toArguments().get("id_hex"))
        : "id hex argument mismatch";
    assert instruction.idHex().equals(idUpperHex.toLowerCase()) : "payload id mismatch";
    assert ActivateRuntimeUpgradeInstruction.ACTION.equals(instruction.toArguments().get("action"))
        : "activate action missing";
  }

  private static void cancelRoundTrip() {
    final String idUpperHex = "CD".repeat(32);
    final CancelRuntimeUpgradeInstruction instruction =
        CancelRuntimeUpgradeInstruction.builder().setIdHex(idUpperHex).build();
    assert idUpperHex.toLowerCase().equals(instruction.toArguments().get("id_hex"))
        : "cancel id hex mismatch";
    assert instruction.idHex().equals(idUpperHex.toLowerCase()) : "payload id mismatch";
    assert CancelRuntimeUpgradeInstruction.ACTION.equals(instruction.toArguments().get("action"))
        : "cancel action missing";
  }
}
