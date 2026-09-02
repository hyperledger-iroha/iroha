package org.hyperledger.iroha.android.offline;

/** Stable application profile identifiers carried by IPM1. */
public enum IrohaPeerPayloadProfile {
  OFFLINE_CASH_V1(1, 1);

  private final int code;
  private final int requiredSchemaVersion;

  IrohaPeerPayloadProfile(final int code, final int requiredSchemaVersion) {
    this.code = code;
    this.requiredSchemaVersion = requiredSchemaVersion;
  }

  public int code() {
    return code;
  }

  /** Returns the sole canonical payload schema admitted by this first-release profile. */
  public int requiredSchemaVersion() {
    return requiredSchemaVersion;
  }

  public static IrohaPeerPayloadProfile fromCode(final int code) {
    for (final IrohaPeerPayloadProfile value : values()) if (value.code == code) return value;
    return null;
  }
}
