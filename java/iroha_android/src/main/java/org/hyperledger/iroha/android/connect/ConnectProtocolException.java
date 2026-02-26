package org.hyperledger.iroha.android.connect;

/** Raised when a Connect frame/envelope cannot be parsed or encoded. */
public final class ConnectProtocolException extends Exception {
  public ConnectProtocolException(final String message) {
    super(message);
  }

  public ConnectProtocolException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
