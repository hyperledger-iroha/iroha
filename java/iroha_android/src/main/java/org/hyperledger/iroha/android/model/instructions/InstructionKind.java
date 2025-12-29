package org.hyperledger.iroha.android.model.instructions;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

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

  private static final Map<String, InstructionKind> LEGACY_ALIASES = buildLegacyAliasMap();

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
    final InstructionKind aliased = LEGACY_ALIASES.get(normalized);
    if (aliased != null) {
      return aliased;
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

  private static Map<String, InstructionKind> buildLegacyAliasMap() {
    final Map<String, InstructionKind> aliases = new HashMap<>();
    aliases.put("iroha.set_key_value", InstructionKind.SET_KEY_VALUE);
    aliases.put("iroha.remove_key_value", InstructionKind.REMOVE_KEY_VALUE);
    aliases.put("iroha.grant", InstructionKind.GRANT);
    aliases.put("iroha.revoke", InstructionKind.REVOKE);
    return aliases;
  }
}
