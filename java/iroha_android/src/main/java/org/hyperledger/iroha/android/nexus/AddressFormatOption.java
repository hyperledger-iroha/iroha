package org.hyperledger.iroha.android.nexus;

/**
 * Address format preference accepted by Torii account/UAID endpoints.
 */
public enum AddressFormatOption {
  IH58("ih58"),
  CANONICAL("canonical"),
  COMPRESSED("compressed");

  private final String parameter;

  AddressFormatOption(final String parameter) {
    this.parameter = parameter;
  }

  public String parameterValue() {
    return parameter;
  }
}
