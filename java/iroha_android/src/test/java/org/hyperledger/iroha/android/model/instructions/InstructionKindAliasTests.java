package org.hyperledger.iroha.android.model.instructions;

/** Regression coverage for legacy instruction kind aliases. */
public final class InstructionKindAliasTests {

  private InstructionKindAliasTests() {}

  public static void main(final String[] args) {
    assert InstructionKind.fromDisplayName("iroha.set_key_value") == InstructionKind.SET_KEY_VALUE
        : "set_key_value alias mismatch";
    assert InstructionKind.fromDisplayName("iroha.remove_key_value")
            == InstructionKind.REMOVE_KEY_VALUE
        : "remove_key_value alias mismatch";
    assert InstructionKind.fromDisplayName("iroha.grant") == InstructionKind.GRANT
        : "grant alias mismatch";
    assert InstructionKind.fromDisplayName("IROHA.REVOKE") == InstructionKind.REVOKE
        : "revoke alias mismatch";
    System.out.println("[IrohaAndroid] InstructionKindAliasTests passed.");
  }
}
