package org.hyperledger.iroha.android.nexus;

/**
 * Address format preference accepted by Torii account/UAID endpoints.
 */
public enum AddressFormatOption {
  I105("i105");

  private final String parameter;

  AddressFormatOption(final String parameter) {
    this.parameter = parameter;
  }

  public String parameterValue() {
    return parameter;
  }
}
