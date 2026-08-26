package org.hyperledger.iroha.sdk.offline;

import java.util.Objects;
import org.jetbrains.annotations.NotNull;

/** One canonical blocker from the universal asset-neutral Offline Cash V1 readiness response. */
public final class OfflineCashReadinessBlockerV1 {
  private final String code;
  private final String message;

  private OfflineCashReadinessBlockerV1(
      @NotNull final String code, @NotNull final String message) {
    this.code = Objects.requireNonNull(code, "code");
    this.message = Objects.requireNonNull(message, "message");
  }

  @NotNull
  static OfflineCashReadinessBlockerV1 fromValidatedProjection(
      @NotNull final String code, @NotNull final String message) {
    return new OfflineCashReadinessBlockerV1(code, message);
  }

  @NotNull
  public String getCode() {
    return code;
  }

  @NotNull
  public String getMessage() {
    return message;
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof OfflineCashReadinessBlockerV1
            && code.equals(((OfflineCashReadinessBlockerV1) other).code)
            && message.equals(((OfflineCashReadinessBlockerV1) other).message);
  }

  @Override
  public int hashCode() {
    return 31 * code.hashCode() + message.hashCode();
  }

  @Override
  public String toString() {
    return "OfflineCashReadinessBlockerV1(code=" + code + ", message=" + message + ")";
  }
}
