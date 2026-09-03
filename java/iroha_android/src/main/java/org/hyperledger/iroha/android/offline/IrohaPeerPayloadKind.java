package org.hyperledger.iroha.android.offline;

/** Frozen five-message KAGEMUSHA V1 exchange identifiers carried by IPM1. */
public enum IrohaPeerPayloadKind {
  REQUEST(1),
  INTENT(2),
  TICKET(3),
  PAYMENT(4),
  ACKNOWLEDGEMENT(5);

  private final int code;

  IrohaPeerPayloadKind(final int code) {
    this.code = code;
  }

  public int code() {
    return code;
  }

  public static IrohaPeerPayloadKind fromCode(final int code) {
    for (final IrohaPeerPayloadKind value : values()) if (value.code == code) return value;
    return null;
  }
}
