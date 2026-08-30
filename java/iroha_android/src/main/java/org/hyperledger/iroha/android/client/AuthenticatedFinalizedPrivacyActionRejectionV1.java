// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12ActionContractV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyLedgerEffectKindV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOperationSchemaV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1;

/** Exact rejected Exact12 action independently authenticated by block and Sumeragi-v2 finality. */
public final class AuthenticatedFinalizedPrivacyActionRejectionV1 {
  private final String networkIdHex;
  private final PrivacyProtocolIdV1 protocolId;
  private final PrivacyOperationSchemaV1 operationSchema;
  private final PrivacyLedgerEffectKindV1 ledgerEffectKind;
  private final String transactionHashHex;
  private final int actionIndex;
  private final byte[] transactionIntentDigest;
  private final byte[] statementDigest;
  private final byte[] proofEnvelopeHash;
  private final String queryAuthorityAccountId;
  private final String transactionAuthorityAccountId;
  private final String blockHashHex;
  private final String resultHashHex;
  private final AuthenticatedPrivacyActionRejectionCodeV1 rejectionCode;
  private final String rejectionMessage;
  private final long committedBlockHeight;
  private final AuthenticatedFinalityCheckpointV1 finalizedCheckpoint;
  private final String executedBlockWireHashHex;
  private final String evidenceIdHex;
  private final String transactionDetailsHashHex;
  private final String finalityPageHashHex;

  AuthenticatedFinalizedPrivacyActionRejectionV1(
      final String networkIdHex,
      final PrivacyProtocolIdV1 protocolId,
      final PrivacyOperationSchemaV1 operationSchema,
      final PrivacyLedgerEffectKindV1 ledgerEffectKind,
      final String transactionHashHex,
      final int actionIndex,
      final byte[] transactionIntentDigest,
      final byte[] statementDigest,
      final byte[] proofEnvelopeHash,
      final String queryAuthorityAccountId,
      final String transactionAuthorityAccountId,
      final String blockHashHex,
      final String resultHashHex,
      final AuthenticatedPrivacyActionRejectionCodeV1 rejectionCode,
      final String rejectionMessage,
      final long committedBlockHeight,
      final AuthenticatedFinalityCheckpointV1 finalizedCheckpoint,
      final String executedBlockWireHashHex,
      final String evidenceIdHex,
      final String transactionDetailsHashHex,
      final String finalityPageHashHex) {
    AuthenticatedFinalityProofPageV1.requireHash(networkIdHex, "networkIdHex");
    if (protocolId == null
        || operationSchema == null
        || ledgerEffectKind == null
        || protocolId != PrivacyExact12ActionContractV1.protocolId(operationSchema)
        || ledgerEffectKind != PrivacyExact12ActionContractV1.ledgerEffectKind(operationSchema)) {
      throw new IllegalArgumentException(
          "finalized rejection protocol or ledger effect does not match its operation");
    }
    AuthenticatedFinalityProofPageV1.requireHash(transactionHashHex, "transactionHashHex");
    if (actionIndex != 0) {
      throw new IllegalArgumentException("Exact12 V1 finalized rejection actionIndex must be zero");
    }
    requireNonzero32(transactionIntentDigest, "transactionIntentDigest");
    requireNonzero32(statementDigest, "statementDigest");
    requireNonzero32(proofEnvelopeHash, "proofEnvelopeHash");
    requireText(queryAuthorityAccountId, "queryAuthorityAccountId", 16 * 1024);
    requireText(transactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024);
    AuthenticatedFinalityProofPageV1.requireHash(blockHashHex, "blockHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(resultHashHex, "resultHashHex");
    if (rejectionCode == null) throw new NullPointerException("rejectionCode");
    requireText(rejectionMessage, "rejectionMessage", 1_024);
    if (committedBlockHeight <= 0
        || finalizedCheckpoint == null
        || finalizedCheckpoint.height() != committedBlockHeight) {
      throw new IllegalArgumentException("finalized checkpoint must equal committedBlockHeight");
    }
    AuthenticatedFinalityProofPageV1.requireHash(
        executedBlockWireHashHex, "executedBlockWireHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(evidenceIdHex, "evidenceIdHex");
    AuthenticatedFinalityProofPageV1.requireHash(
        transactionDetailsHashHex, "transactionDetailsHashHex");
    AuthenticatedFinalityProofPageV1.requireHash(finalityPageHashHex, "finalityPageHashHex");
    this.networkIdHex = networkIdHex;
    this.protocolId = protocolId;
    this.operationSchema = operationSchema;
    this.ledgerEffectKind = ledgerEffectKind;
    this.transactionHashHex = transactionHashHex;
    this.actionIndex = actionIndex;
    this.transactionIntentDigest = transactionIntentDigest.clone();
    this.statementDigest = statementDigest.clone();
    this.proofEnvelopeHash = proofEnvelopeHash.clone();
    this.queryAuthorityAccountId = queryAuthorityAccountId;
    this.transactionAuthorityAccountId = transactionAuthorityAccountId;
    this.blockHashHex = blockHashHex;
    this.resultHashHex = resultHashHex;
    this.rejectionCode = rejectionCode;
    this.rejectionMessage = rejectionMessage;
    this.committedBlockHeight = committedBlockHeight;
    this.finalizedCheckpoint = finalizedCheckpoint;
    this.executedBlockWireHashHex = executedBlockWireHashHex;
    this.evidenceIdHex = evidenceIdHex;
    this.transactionDetailsHashHex = transactionDetailsHashHex;
    this.finalityPageHashHex = finalityPageHashHex;
  }

  public String networkIdHex() { return networkIdHex; }
  public PrivacyProtocolIdV1 protocolId() { return protocolId; }
  public PrivacyOperationSchemaV1 operationSchema() { return operationSchema; }
  public PrivacyLedgerEffectKindV1 ledgerEffectKind() { return ledgerEffectKind; }
  public String transactionHashHex() { return transactionHashHex; }
  public int actionIndex() { return actionIndex; }
  public byte[] transactionIntentDigest() { return transactionIntentDigest.clone(); }
  public byte[] statementDigest() { return statementDigest.clone(); }
  public byte[] proofEnvelopeHash() { return proofEnvelopeHash.clone(); }
  public String queryAuthorityAccountId() { return queryAuthorityAccountId; }
  public String transactionAuthorityAccountId() { return transactionAuthorityAccountId; }
  public String blockHashHex() { return blockHashHex; }
  public String resultHashHex() { return resultHashHex; }
  public AuthenticatedPrivacyActionRejectionCodeV1 rejectionCode() { return rejectionCode; }
  public String rejectionMessage() { return rejectionMessage; }
  public long committedBlockHeight() { return committedBlockHeight; }
  public AuthenticatedFinalityCheckpointV1 finalizedCheckpoint() { return finalizedCheckpoint; }
  public String executedBlockWireHashHex() { return executedBlockWireHashHex; }
  public String evidenceIdHex() { return evidenceIdHex; }
  public String transactionDetailsHashHex() { return transactionDetailsHashHex; }
  public String finalityPageHashHex() { return finalityPageHashHex; }

  private static void requireNonzero32(final byte[] value, final String field) {
    if (value == null || value.length != 32 || allZero(value)) {
      throw new IllegalArgumentException(field + " must contain exactly 32 nonzero bytes");
    }
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte current : value) aggregate |= current;
    return aggregate == 0;
  }

  private static void requireText(final String value, final String field, final int maximumBytes) {
    if (value == null
        || value.isEmpty()
        || value.getBytes(StandardCharsets.UTF_8).length > maximumBytes
        || !value.equals(value.trim())
        || value.codePoints().anyMatch(Character::isISOControl)) {
      throw new IllegalArgumentException(field + " violates its closed text bound");
    }
  }
}
