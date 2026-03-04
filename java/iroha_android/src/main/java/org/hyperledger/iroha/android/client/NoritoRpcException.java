package org.hyperledger.iroha.android.client;

/**
 * Exception thrown when Norito RPC requests fail or cannot be executed.
 */
public final class NoritoRpcException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public NoritoRpcException(final String message) {
    super(message);
  }

  public NoritoRpcException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
