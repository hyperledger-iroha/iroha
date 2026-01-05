package org.hyperledger.iroha.android.model.instructions;

import java.util.Locale;

/** Matches the discriminants exposed by `iroha_data_model::isi::InstructionType`. */
public enum InstructionKind {
  SET_PARAMETER("SetParameter", 0),
  SET_KEY_VALUE("SetKeyValue", 1),
  REMOVE_KEY_VALUE("RemoveKeyValue", 2),
  REGISTER("Register", 3),
  UNREGISTER("Unregister", 4),
  MINT("Mint", 5),
  BURN("Burn", 6),
  TRANSFER("Transfer", 7),
  GRANT("Grant", 8),
  REVOKE("Revoke", 9),
  UPGRADE("Upgrade", 10),
  EXECUTE_TRIGGER("ExecuteTrigger", 11),
  LOG("Log", 12),
  CUSTOM("Custom", 13);

  private final String displayName;
  private final int discriminant;

  InstructionKind(final String displayName, final int discriminant) {
    this.displayName = displayName;
    this.discriminant = discriminant;
  }

  public String displayName() {
    return displayName;
  }

  public int discriminant() {
    return discriminant;
  }

  public static InstructionKind fromDisplayName(final String name) {
    final String normalized = name.toLowerCase(Locale.ROOT);
    for (InstructionKind kind : values()) {
      if (kind.displayName.toLowerCase(Locale.ROOT).equals(normalized)) {
        return kind;
      }
    }
    throw new IllegalArgumentException("Unknown instruction kind: " + name);
  }

  public static InstructionKind fromDiscriminant(final long value) {
    for (InstructionKind kind : values()) {
      if (kind.discriminant == value) {
        return kind;
      }
    }
    throw new IllegalArgumentException("Unknown instruction discriminant: " + value);
  }

}
