package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Set;

/** Exact Kagemusha issuer result independently authenticated by validator finality evidence. */
public final class AuthenticatedFinalizedKagemushaOutcomeV1 {
  private static final Set<String> REJECTION_CODES_V1 =
      Set.of(
          "account_does_not_exist",
          "limit_check",
          "validation",
          "instruction_execution",
          "ivm_execution",
          "trigger_execution");

  public enum TerminalState { APPLIED, REJECTED }

  private final TerminalState terminalState;
  private final byte[] operationId;
  private final String operationKind;
  private final String transactionHashHex;
  private final String queryAuthorityAccountId;
  private final String transactionAuthorityAccountId;
  private final String blockHashHex;
  private final String resultHashHex;
  private final long committedBlockHeight;
  private final AuthenticatedFinalityCheckpointV1 finalizedCheckpoint;
  private final String executedBlockWireHashHex;
  private final String rejectionCode;
  private final String rejectionMessage;
  private final String evidenceIdHex;
  private final String transactionDetailsHashHex;
  private final String finalityPageHashHex;

  AuthenticatedFinalizedKagemushaOutcomeV1(
      final TerminalState terminalState,
      final byte[] operationId,
      final String operationKind,
      final String transactionHashHex,
      final String queryAuthorityAccountId,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final long committedBlockHeight,
      final AuthenticatedFinalityCheckpointV1 finalizedCheckpoint,
      final String executedBlockWireHashHex,
      final String rejectionCode,
      final String rejectionMessage,
      final String evidenceIdHex,
      final String transactionDetailsHashHex,
      final String finalityPageHashHex) {
    if (terminalState == null) throw new NullPointerException("terminalState");
    if (operationId == null || operationId.length != 32 || allZero(operationId)) {
      throw new IllegalArgumentException("operationId must contain exactly 32 nonzero bytes");
    }
    if (!"top_up".equals(operationKind) && !"redeem".equals(operationKind)) {
      throw new IllegalArgumentException("operationKind must be top_up or redeem");
    }
    AuthenticatedFinalityProofPageV1.requireHash(transactionHashHex, "transactionHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(blockHashHex, "blockHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(resultHashHex, "resultHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(
        executedBlockWireHashHex, "executedBlockWireHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(evidenceIdHex, "evidenceIdHex");
    AuthenticatedFinalityProofPageV1.requireHash(
        transactionDetailsHashHex, "transactionDetailsHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(finalityPageHashHex, "finalityPageHashHex");
    requireText(queryAuthorityAccountId, "queryAuthorityAccountId", 16 * 1024);
    requireText(transactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024);
    if (committedBlockHeight <= 0
        || finalizedCheckpoint == null
        || finalizedCheckpoint.height() != committedBlockHeight) {
      throw new IllegalArgumentException("finalized checkpoint must equal committedBlockHeight");
    }
    if (terminalState == TerminalState.APPLIED) {
      if (rejectionCode != null || rejectionMessage != null) {
        throw new IllegalArgumentException("APPLIED outcome must not carry rejection fields");
      }
    } else {
      if (!REJECTION_CODES_V1.contains(rejectionCode)) {
        throw new IllegalArgumentException(
            "rejectionCode is not one of the six ABI-22 terminal rejection kinds");
      }
      requireText(rejectionCode, "rejectionCode", 128);
      requireText(rejectionMessage, "rejectionMessage", 1_024);
    }
    this.terminalState = terminalState;
    this.operationId = operationId.clone();
    this.operationKind = operationKind;
    this.transactionHashHex = transactionHashHex;
    this.queryAuthorityAccountId = queryAuthorityAccountId;
    this.transactionAuthorityAccountId = transactionAuthorityAccountId;
    this.blockHashHex = blockHashHex;
    this.resultHashHex = resultHashHex;
    this.committedBlockHeight = committedBlockHeight;
    this.finalizedCheckpoint = finalizedCheckpoint;
    this.executedBlockWireHashHex = executedBlockWireHashHex;
    this.rejectionCode = rejectionCode;
    this.rejectionMessage = rejectionMessage;
    this.evidenceIdHex = evidenceIdHex;
    this.transactionDetailsHashHex = transactionDetailsHashHex;
    this.finalityPageHashHex = finalityPageHashHex;
  }

  public TerminalState terminalState() { return terminalState; }
  public byte[] operationId() { return operationId.clone(); }
  public String operationKind() { return operationKind; }
  public String transactionHashHex() { return transactionHashHex; }
  public String queryAuthorityAccountId() { return queryAuthorityAccountId; }
  public String transactionAuthorityAccountId() { return transactionAuthorityAccountId; }
  public String blockHashHex() { return blockHashHex; }
  public String resultHashHex() { return resultHashHex; }
  public long committedBlockHeight() { return committedBlockHeight; }
  public AuthenticatedFinalityCheckpointV1 finalizedCheckpoint() { return finalizedCheckpoint; }
  public String executedBlockWireHashHex() { return executedBlockWireHashHex; }
  public String rejectionCode() { return rejectionCode; }
  public String rejectionMessage() { return rejectionMessage; }
  public String evidenceIdHex() { return evidenceIdHex; }
  public String transactionDetailsHashHex() { return transactionDetailsHashHex; }
  public String finalityPageHashHex() { return finalityPageHashHex; }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte current : value) aggregate |= current;
    return aggregate == 0;
  }

  private static void requireText(final String value, final String field, final int maxBytes) {
    if (value == null
        || value.isEmpty()
        || value.getBytes(StandardCharsets.UTF_8).length > maxBytes
        || !value.equals(value.trim())
        || value.codePoints().anyMatch(Character::isISOControl)) {
      throw new IllegalArgumentException(field + " violates its closed text bound");
    }
  }
}
