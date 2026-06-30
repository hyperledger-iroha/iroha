package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** One explorer instruction outcome used by Offline Note wallet reconciliation. */
public final class OfflineNoteExplorerInstructionOutcome {
  private final String kind;
  private final String transactionStatus;
  private final String transactionHashHex;
  private final byte[] encodedInstruction;

  public OfflineNoteExplorerInstructionOutcome(
      final String kind, final String transactionStatus, final byte[] encodedInstruction) {
    this(kind, transactionStatus, null, encodedInstruction);
  }

  public OfflineNoteExplorerInstructionOutcome(
      final String kind,
      final String transactionStatus,
      final String transactionHashHex,
      final byte[] encodedInstruction) {
    if (!isExactNonEmpty(kind)) {
      throw new IllegalArgumentException("kind must be an exact non-empty string");
    }
    if (!isExactNonEmpty(transactionStatus)) {
      throw new IllegalArgumentException("transactionStatus must be an exact non-empty string");
    }
    this.kind = kind;
    this.transactionStatus = transactionStatus;
    this.transactionHashHex = transactionHashHex;
    this.encodedInstruction = Arrays.copyOf(Objects.requireNonNull(encodedInstruction, "encodedInstruction"), encodedInstruction.length);
    if (this.encodedInstruction.length == 0) {
      throw new IllegalArgumentException("encodedInstruction must not be empty");
    }
  }

  private static boolean isExactNonEmpty(final String value) {
    return value != null && !value.isEmpty() && value.equals(value.trim());
  }

  public String kind() {
    return kind;
  }

  public String transactionStatus() {
    return transactionStatus;
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }

  public byte[] encodedInstruction() {
    return Arrays.copyOf(encodedInstruction, encodedInstruction.length);
  }
}
