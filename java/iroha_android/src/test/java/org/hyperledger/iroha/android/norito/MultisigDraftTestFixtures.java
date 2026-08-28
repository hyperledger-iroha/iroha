package org.hyperledger.iroha.android.norito;

import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Canonical multisig outer-executable fixtures shared by Java transport tests. */
public final class MultisigDraftTestFixtures {
  private MultisigDraftTestFixtures() {}

  /** Builds Torii's exact propose and optional immediate-approve instruction executable. */
  public static Executable proposalExecutable(
      final String account,
      final List<byte[]> proposalInstructions,
      final boolean includeApprove)
      throws NoritoException {
    final List<String> encodedInstructions = new ArrayList<>(proposalInstructions.size());
    for (final byte[] instruction : proposalInstructions) {
      encodedInstructions.add(Base64.getEncoder().encodeToString(instruction));
    }
    final Map<String, Object> proposeBody = new LinkedHashMap<>();
    proposeBody.put("account", account);
    proposeBody.put("instructions", encodedInstructions);
    proposeBody.put("transaction_ttl_ms", null);
    final List<InstructionBox> outer = new ArrayList<>();
    outer.add(customInstruction("Propose", proposeBody));
    if (includeApprove) {
      final Map<String, Object> approveBody = new LinkedHashMap<>();
      approveBody.put("account", account);
      approveBody.put(
          "instructions_hash",
          HashLiteral.canonicalize(
              NoritoJavaCodecAdapter.hashCanonicalInstructionBoxes(proposalInstructions)));
      outer.add(customInstruction("Approve", approveBody));
    }
    return Executable.instructions(outer);
  }

  private static InstructionBox customInstruction(
      final String variant, final Map<String, Object> body) {
    final Map<String, Object> root = new LinkedHashMap<>();
    root.put(variant, body);
    final String canonical = JsonValue.parse(JsonEncoder.encode(root)).canonicalJson();
    return InstructionBox.fromWirePayload(
        "iroha.custom",
        TransactionPayloadAdapter.encodeCanonicalCustomInstructionJson(canonical));
  }
}
