package org.hyperledger.iroha.android.connect.error;

/** Optional overrides when mapping errors to the taxonomy. */
public record ConnectErrorOptions(Boolean fatal, Integer httpStatus) {
  public static ConnectErrorOptions empty() {
    return new ConnectErrorOptions(null, null);
  }
}
