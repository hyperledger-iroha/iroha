package org.hyperledger.iroha.android.offline;

/** Stable request/payment/acknowledgement identifiers carried by IPM1. */
public enum IrohaPeerPayloadKind {
  RECEIVE_REQUEST(1),
  PAYMENT(2),
  ACKNOWLEDGEMENT(3);

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
