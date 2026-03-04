package org.hyperledger.iroha.android.sorafs;

/** Exception raised when SoraFS storage interactions fail. */
public final class SorafsStorageException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  SorafsStorageException(final String message) {
    super(message);
  }

  SorafsStorageException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
