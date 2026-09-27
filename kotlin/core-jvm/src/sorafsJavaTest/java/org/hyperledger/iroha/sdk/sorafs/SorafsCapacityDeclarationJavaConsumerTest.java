package org.hyperledger.iroha.sdk.sorafs;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind;
import org.hyperledger.iroha.sdk.core.model.instructions.RegisterCapacityDeclarationInstruction;
import org.junit.jupiter.api.Test;

/** Java callers exercise Kotlin's sole capacity-declaration input and its rejection boundaries. */
public final class SorafsCapacityDeclarationJavaConsumerTest {
  @Test void declarationBuilderPreservesPayloadAndCanonicalArguments() {
    byte[] declaration = new byte[] {1, 2, 3, 4};
    byte[] expected = declaration.clone();
    RegisterCapacityDeclarationInstruction.Builder builder =
        RegisterCapacityDeclarationInstruction.builder().setDeclarationBytes(declaration);
    declaration[0] = 9;
    RegisterCapacityDeclarationInstruction instruction = builder.build();
    assertEquals(InstructionKind.REGISTER, instruction.getKind());
    assertArrayEquals(expected, instruction.declarationBytes());
    Map<String, String> arguments = instruction.getArguments();
    assertEquals("RegisterCapacityDeclaration", arguments.get("action"));
    assertEquals(Base64.getEncoder().encodeToString(expected), arguments.get("declaration_b64"));
    assertEquals(2, arguments.size());
    RegisterCapacityDeclarationInstruction decoded = RegisterCapacityDeclarationInstruction.fromArguments(arguments);
    assertEquals(instruction, decoded);
    assertEquals(instruction.hashCode(), decoded.hashCode());
    byte[] returned = instruction.declarationBytes();
    returned[0] = 7;
    assertArrayEquals(expected, instruction.declarationBytes());
    assertThrows(UnsupportedOperationException.class, () -> arguments.put("registered_epoch", "1"));
  }

  @Test void declarationRejectsInvalidOrNoncanonicalBase64() {
    for (String invalid : new String[] {"", " ", "not!base64", "AQ", "AQ==\n", " AQ==", "AR=="}) {
      assertThrows(IllegalArgumentException.class, () ->
          RegisterCapacityDeclarationInstruction.builder().setDeclarationBase64(invalid));
    }
    assertThrows(IllegalStateException.class, () -> RegisterCapacityDeclarationInstruction.builder().build());
    assertThrows(IllegalArgumentException.class, () -> new RegisterCapacityDeclarationInstruction(new byte[0]));
    assertThrows(IllegalArgumentException.class, () -> new RegisterCapacityDeclarationInstruction(new byte[256 * 1024 + 1]));
  }

  @Test void callerCannotOverrideConsensusDerivedFieldsOrAction() {
    Map<String, String> original = new RegisterCapacityDeclarationInstruction(new byte[] {1, 2, 3}).getArguments();
    for (String retired : new String[] {"provider_id_hex", "committed_capacity_gib", "registered_epoch",
        "valid_from_epoch", "valid_until_epoch", "metadata.region", "metadata.notes", "record"}) {
      Map<String, String> arguments = new LinkedHashMap<>(original);
      arguments.put(retired, "1");
      assertThrows(IllegalArgumentException.class, () -> RegisterCapacityDeclarationInstruction.fromArguments(arguments));
    }
    Map<String, String> wrongAction = new LinkedHashMap<>(original);
    wrongAction.put("action", "Other");
    assertThrows(IllegalArgumentException.class, () -> RegisterCapacityDeclarationInstruction.fromArguments(wrongAction));
    Map<String, String> missing = new LinkedHashMap<>(original);
    missing.remove("declaration_b64");
    assertThrows(IllegalArgumentException.class, () -> RegisterCapacityDeclarationInstruction.fromArguments(missing));
  }
}
