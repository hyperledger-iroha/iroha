package org.hyperledger.iroha.android.offline;

public enum IrohaPeerContentEncodingV1 {
  NONE(0),
  ZLIB(1);

  private final int code;

  IrohaPeerContentEncodingV1(final int code) {
    this.code = code;
  }

  public int code() {
    return code;
  }

  public static IrohaPeerContentEncodingV1 fromCode(final int code) {
    for (final IrohaPeerContentEncodingV1 value : values()) if (value.code == code) return value;
    return null;
  }
}
