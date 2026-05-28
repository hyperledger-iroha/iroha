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
    if (kind == null || kind.isBlank()) {
      throw new IllegalArgumentException("kind must not be blank");
    }
    if (transactionStatus == null || transactionStatus.isBlank()) {
      throw new IllegalArgumentException("transactionStatus must not be blank");
    }
    this.kind = kind;
    this.transactionStatus = transactionStatus;
    this.transactionHashHex = transactionHashHex;
    this.encodedInstruction = Arrays.copyOf(Objects.requireNonNull(encodedInstruction, "encodedInstruction"), encodedInstruction.length);
    if (this.encodedInstruction.length == 0) {
      throw new IllegalArgumentException("encodedInstruction must not be empty");
    }
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
