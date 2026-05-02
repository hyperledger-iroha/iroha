package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Objects;

/** VPN receipt response including earned/refunded XOR and native settlement instructions. */
public final class VpnReceipt {
  private final String sessionId;
  private final String accountId;
  private final String exitClass;
  private final String relayEndpoint;
  private final String meterFamily;
  private final long connectedAtMs;
  private final long disconnectedAtMs;
  private final long durationMs;
  private final long bytesIn;
  private final long bytesOut;
  private final String status;
  private final String receiptSource;
  private final String quoteId;
  private final String paymentTxHash;
  private final String feeAssetId;
  private final String escrowAccountId;
  private final String operatorAccountId;
  private final long leaseFeeNanos;
  private final long earnedFeeNanos;
  private final long refundedFeeNanos;
  private final String leaseIdHex;
  private final VpnTxInstruction settleLeaseInstruction;
  private final List<VpnTxInstruction> txInstructions;

  public VpnReceipt(
      final String sessionId,
      final String accountId,
      final String exitClass,
      final String relayEndpoint,
      final String meterFamily,
      final long connectedAtMs,
      final long disconnectedAtMs,
      final long durationMs,
      final long bytesIn,
      final long bytesOut,
      final String status,
      final String receiptSource,
      final String quoteId,
      final String paymentTxHash,
      final String feeAssetId,
      final String escrowAccountId,
      final String operatorAccountId,
      final long leaseFeeNanos,
      final long earnedFeeNanos,
      final long refundedFeeNanos,
      final String leaseIdHex,
      final VpnTxInstruction settleLeaseInstruction,
      final List<VpnTxInstruction> txInstructions) {
    this.sessionId = Objects.requireNonNull(sessionId, "sessionId");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.exitClass = Objects.requireNonNull(exitClass, "exitClass");
    this.relayEndpoint = Objects.requireNonNull(relayEndpoint, "relayEndpoint");
    this.meterFamily = Objects.requireNonNull(meterFamily, "meterFamily");
    this.connectedAtMs = connectedAtMs;
    this.disconnectedAtMs = disconnectedAtMs;
    this.durationMs = durationMs;
    this.bytesIn = bytesIn;
    this.bytesOut = bytesOut;
    this.status = Objects.requireNonNull(status, "status");
    this.receiptSource = Objects.requireNonNull(receiptSource, "receiptSource");
    this.quoteId = Objects.requireNonNull(quoteId, "quoteId");
    this.paymentTxHash = Objects.requireNonNull(paymentTxHash, "paymentTxHash");
    this.feeAssetId = Objects.requireNonNull(feeAssetId, "feeAssetId");
    this.escrowAccountId = Objects.requireNonNull(escrowAccountId, "escrowAccountId");
    this.operatorAccountId = Objects.requireNonNull(operatorAccountId, "operatorAccountId");
    this.leaseFeeNanos = leaseFeeNanos;
    this.earnedFeeNanos = earnedFeeNanos;
    this.refundedFeeNanos = refundedFeeNanos;
    this.leaseIdHex = Objects.requireNonNull(leaseIdHex, "leaseIdHex");
    this.settleLeaseInstruction = settleLeaseInstruction;
    this.txInstructions = java.util.Collections.unmodifiableList(new java.util.ArrayList<>(Objects.requireNonNull(txInstructions, "txInstructions")));
  }

  public String sessionId() { return sessionId; }
  public String accountId() { return accountId; }
  public String exitClass() { return exitClass; }
  public String relayEndpoint() { return relayEndpoint; }
  public String meterFamily() { return meterFamily; }
  public long connectedAtMs() { return connectedAtMs; }
  public long disconnectedAtMs() { return disconnectedAtMs; }
  public long durationMs() { return durationMs; }
  public long bytesIn() { return bytesIn; }
  public long bytesOut() { return bytesOut; }
  public String status() { return status; }
  public String receiptSource() { return receiptSource; }
  public String quoteId() { return quoteId; }
  public String paymentTxHash() { return paymentTxHash; }
  public String feeAssetId() { return feeAssetId; }
  public String escrowAccountId() { return escrowAccountId; }
  public String operatorAccountId() { return operatorAccountId; }
  public long leaseFeeNanos() { return leaseFeeNanos; }
  public long earnedFeeNanos() { return earnedFeeNanos; }
  public long refundedFeeNanos() { return refundedFeeNanos; }
  public String leaseIdHex() { return leaseIdHex; }
  public VpnTxInstruction settleLeaseInstruction() { return settleLeaseInstruction; }
  public List<VpnTxInstruction> txInstructions() { return txInstructions; }
}
