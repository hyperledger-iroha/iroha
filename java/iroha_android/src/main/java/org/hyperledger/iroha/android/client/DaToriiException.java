package org.hyperledger.iroha.android.client;

/** Failure returned by {@link DaToriiClient}. */
public final class DaToriiException extends RuntimeException {
  public DaToriiException(final String message) {
    super(message);
  }

  public DaToriiException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
