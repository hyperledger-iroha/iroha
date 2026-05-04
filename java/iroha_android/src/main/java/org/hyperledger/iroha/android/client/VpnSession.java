package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Objects;

/** Active VPN session response. */
public final class VpnSession {
  private final String sessionId;
  private final String accountId;
  private final String exitClass;
  private final String relayEndpoint;
  private final long leaseSecs;
  private final long expiresAtMs;
  private final long connectedAtMs;
  private final String meterFamily;
  private final String quoteId;
  private final String paymentReference;
  private final String paymentTxHash;
  private final String feeAssetId;
  private final String escrowAccountId;
  private final String operatorAccountId;
  private final long leaseFeeNanos;
  private final int flowLabelBits;
  private final int paddingBudgetMs;
  private final String relayTlsSpkiSha256Hex;
  private final List<String> routePushes;
  private final List<String> excludedRoutes;
  private final List<String> dnsServers;
  private final List<String> tunnelAddresses;
  private final long mtuBytes;
  private final String helperTicketHex;
  private final long bytesIn;
  private final long bytesOut;
  private final String status;

  public VpnSession(
      final String sessionId,
      final String accountId,
      final String exitClass,
      final String relayEndpoint,
      final long leaseSecs,
      final long expiresAtMs,
      final long connectedAtMs,
      final String meterFamily,
      final String quoteId,
      final String paymentReference,
      final String paymentTxHash,
      final String feeAssetId,
      final String escrowAccountId,
      final String operatorAccountId,
      final long leaseFeeNanos,
      final int flowLabelBits,
      final int paddingBudgetMs,
      final String relayTlsSpkiSha256Hex,
      final List<String> routePushes,
      final List<String> excludedRoutes,
      final List<String> dnsServers,
      final List<String> tunnelAddresses,
      final long mtuBytes,
      final String helperTicketHex,
      final long bytesIn,
      final long bytesOut,
      final String status) {
    this.sessionId = Objects.requireNonNull(sessionId, "sessionId");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.exitClass = Objects.requireNonNull(exitClass, "exitClass");
    this.relayEndpoint = Objects.requireNonNull(relayEndpoint, "relayEndpoint");
    this.leaseSecs = leaseSecs;
    this.expiresAtMs = expiresAtMs;
    this.connectedAtMs = connectedAtMs;
    this.meterFamily = Objects.requireNonNull(meterFamily, "meterFamily");
    this.quoteId = Objects.requireNonNull(quoteId, "quoteId");
    this.paymentReference = Objects.requireNonNull(paymentReference, "paymentReference");
    this.paymentTxHash = Objects.requireNonNull(paymentTxHash, "paymentTxHash");
    this.feeAssetId = Objects.requireNonNull(feeAssetId, "feeAssetId");
    this.escrowAccountId = Objects.requireNonNull(escrowAccountId, "escrowAccountId");
    this.operatorAccountId = Objects.requireNonNull(operatorAccountId, "operatorAccountId");
    this.leaseFeeNanos = leaseFeeNanos;
    this.flowLabelBits = flowLabelBits;
    this.paddingBudgetMs = paddingBudgetMs;
    this.relayTlsSpkiSha256Hex = relayTlsSpkiSha256Hex;
    this.routePushes = VpnProfile.immutableList(routePushes);
    this.excludedRoutes = VpnProfile.immutableList(excludedRoutes);
    this.dnsServers = VpnProfile.immutableList(dnsServers);
    this.tunnelAddresses = VpnProfile.immutableList(tunnelAddresses);
    this.mtuBytes = mtuBytes;
    this.helperTicketHex = Objects.requireNonNull(helperTicketHex, "helperTicketHex");
    this.bytesIn = bytesIn;
    this.bytesOut = bytesOut;
    this.status = Objects.requireNonNull(status, "status");
  }

  public String sessionId() { return sessionId; }
  public String accountId() { return accountId; }
  public String exitClass() { return exitClass; }
  public String relayEndpoint() { return relayEndpoint; }
  public long leaseSecs() { return leaseSecs; }
  public long expiresAtMs() { return expiresAtMs; }
  public long connectedAtMs() { return connectedAtMs; }
  public String meterFamily() { return meterFamily; }
  public String quoteId() { return quoteId; }
  public String paymentReference() { return paymentReference; }
  public String paymentTxHash() { return paymentTxHash; }
  public String feeAssetId() { return feeAssetId; }
  public String escrowAccountId() { return escrowAccountId; }
  public String operatorAccountId() { return operatorAccountId; }
  public long leaseFeeNanos() { return leaseFeeNanos; }
  public int flowLabelBits() { return flowLabelBits; }
  public int paddingBudgetMs() { return paddingBudgetMs; }
  public String relayTlsSpkiSha256Hex() { return relayTlsSpkiSha256Hex; }
  public List<String> routePushes() { return routePushes; }
  public List<String> excludedRoutes() { return excludedRoutes; }
  public List<String> dnsServers() { return dnsServers; }
  public List<String> tunnelAddresses() { return tunnelAddresses; }
  public long mtuBytes() { return mtuBytes; }
  public String helperTicketHex() { return helperTicketHex; }
  public long bytesIn() { return bytesIn; }
  public long bytesOut() { return bytesOut; }
  public String status() { return status; }
}
