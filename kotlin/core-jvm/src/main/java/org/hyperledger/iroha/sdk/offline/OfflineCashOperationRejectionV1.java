package org.hyperledger.iroha.sdk.offline;

import java.util.Objects;
import org.jetbrains.annotations.NotNull;

/** Stable terminal rejection returned by an Offline Cash V1 operation. */
public final class OfflineCashOperationRejectionV1 {
  private static final String REJECTION_CODE = "offline_operation_rejected";
  private static final int MAX_MESSAGE_CODE_POINTS = 1024;

  private final String code;
  private final String message;

  private OfflineCashOperationRejectionV1(
      @NotNull final String code, @NotNull final String message) {
    Objects.requireNonNull(code, "rejectionCode");
    if (!REJECTION_CODE.equals(code)) {
      throw new IllegalArgumentException(
          "rejectionCode must equal " + REJECTION_CODE);
    }
    this.code = code;
    this.message = requireCanonicalRejectionMessage(message);
  }

  @NotNull
  static OfflineCashOperationRejectionV1 fromValidatedProjection(
      @NotNull final String code, @NotNull final String message) {
    return new OfflineCashOperationRejectionV1(code, message);
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
        || other instanceof OfflineCashOperationRejectionV1
            && code.equals(((OfflineCashOperationRejectionV1) other).code)
            && message.equals(((OfflineCashOperationRejectionV1) other).message);
  }

  @Override
  public int hashCode() {
    return 31 * code.hashCode() + message.hashCode();
  }

  @Override
  public String toString() {
    return "OfflineCashOperationRejectionV1(code=" + code + ", message=" + message + ")";
  }

  private static String requireCanonicalRejectionMessage(@NotNull final String value) {
    Objects.requireNonNull(value, "rejectionMessage");
    boolean containsControl = false;
    for (int offset = 0; offset < value.length(); ) {
      final int codePoint = value.codePointAt(offset);
      if (Character.isISOControl(codePoint)) {
        containsControl = true;
        break;
      }
      offset += Character.charCount(codePoint);
    }
    if (value.isEmpty()
        || hasBoundaryWhitespace(value)
        || containsControl
        || value.codePointCount(0, value.length()) > MAX_MESSAGE_CODE_POINTS) {
      throw new IllegalArgumentException(
          "rejectionMessage must contain 1..1024 canonical Unicode scalars");
    }
    return value;
  }

  private static boolean hasBoundaryWhitespace(@NotNull final String value) {
    final int first = value.codePointAt(0);
    final int last = value.codePointBefore(value.length());
    return isUnicodeWhitespace(first) || isUnicodeWhitespace(last);
  }

  private static boolean isUnicodeWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
  }
}
