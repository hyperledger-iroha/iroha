package org.hyperledger.iroha.android.nexus;

/** Typed error raised by {@link NexusAppClient}. */
public final class NexusAppError extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String code;

  public NexusAppError(final String code, final String message) {
    super(message);
    this.code = code;
  }

  public NexusAppError(final String code, final String message, final Throwable cause) {
    super(message, cause);
    this.code = code;
  }

  public String code() {
    return code;
  }
}
