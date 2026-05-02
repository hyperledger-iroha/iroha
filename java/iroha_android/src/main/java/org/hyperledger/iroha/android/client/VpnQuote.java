package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Objects;

/** Quote response binding XOR lease escrow terms before a VPN session is opened. */
public final class VpnQuote {
  private final String quoteId;
  private final String leaseIdHex;
  private final String sessionIdHex;
  private final String paymentReference;
  private final String accountId;
  private final String exitClass;
  private final String relayEndpoint;
  private final long leaseSecs;
  private final long quoteExpiresAtMs;
  private final String feeAssetId;
  private final String escrowAccountId;
  private final String operatorAccountId;
  private final long leaseFeeNanos;
  private final List<String> routePushes;
  private final List<String> excludedRoutes;
  private final List<String> dnsServers;
  private final List<String> tunnelAddresses;
  private final long mtuBytes;
  private final String meterFamily;
  private final int flowLabelBits;
  private final int paddingBudgetMs;
  private final String relayTlsSpkiSha256Hex;
  private final String meteringPublicKeyHex;
  private final VpnTxInstruction openLeaseInstruction;
  private final List<VpnTxInstruction> txInstructions;

  public VpnQuote(
      final String quoteId,
      final String leaseIdHex,
      final String sessionIdHex,
      final String paymentReference,
      final String accountId,
      final String exitClass,
      final String relayEndpoint,
      final long leaseSecs,
      final long quoteExpiresAtMs,
      final String feeAssetId,
      final String escrowAccountId,
      final String operatorAccountId,
      final long leaseFeeNanos,
      final List<String> routePushes,
      final List<String> excludedRoutes,
      final List<String> dnsServers,
      final List<String> tunnelAddresses,
      final long mtuBytes,
      final String meterFamily,
      final int flowLabelBits,
      final int paddingBudgetMs,
      final String relayTlsSpkiSha256Hex,
      final String meteringPublicKeyHex,
      final VpnTxInstruction openLeaseInstruction,
      final List<VpnTxInstruction> txInstructions) {
    this.quoteId = Objects.requireNonNull(quoteId, "quoteId");
    this.leaseIdHex = Objects.requireNonNull(leaseIdHex, "leaseIdHex");
    this.sessionIdHex = Objects.requireNonNull(sessionIdHex, "sessionIdHex");
    this.paymentReference = Objects.requireNonNull(paymentReference, "paymentReference");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.exitClass = Objects.requireNonNull(exitClass, "exitClass");
    this.relayEndpoint = Objects.requireNonNull(relayEndpoint, "relayEndpoint");
    this.leaseSecs = leaseSecs;
    this.quoteExpiresAtMs = quoteExpiresAtMs;
    this.feeAssetId = Objects.requireNonNull(feeAssetId, "feeAssetId");
    this.escrowAccountId = Objects.requireNonNull(escrowAccountId, "escrowAccountId");
    this.operatorAccountId = Objects.requireNonNull(operatorAccountId, "operatorAccountId");
    this.leaseFeeNanos = leaseFeeNanos;
    this.routePushes = VpnProfile.immutableList(routePushes);
    this.excludedRoutes = VpnProfile.immutableList(excludedRoutes);
    this.dnsServers = VpnProfile.immutableList(dnsServers);
    this.tunnelAddresses = VpnProfile.immutableList(tunnelAddresses);
    this.mtuBytes = mtuBytes;
    this.meterFamily = Objects.requireNonNull(meterFamily, "meterFamily");
    this.flowLabelBits = flowLabelBits;
    this.paddingBudgetMs = paddingBudgetMs;
    this.relayTlsSpkiSha256Hex = relayTlsSpkiSha256Hex;
    this.meteringPublicKeyHex = Objects.requireNonNull(meteringPublicKeyHex, "meteringPublicKeyHex");
    this.openLeaseInstruction = openLeaseInstruction;
    this.txInstructions = java.util.Collections.unmodifiableList(new java.util.ArrayList<>(Objects.requireNonNull(txInstructions, "txInstructions")));
  }

  public String quoteId() { return quoteId; }
  public String leaseIdHex() { return leaseIdHex; }
  public String sessionIdHex() { return sessionIdHex; }
  public String paymentReference() { return paymentReference; }
  public String accountId() { return accountId; }
  public String exitClass() { return exitClass; }
  public String relayEndpoint() { return relayEndpoint; }
  public long leaseSecs() { return leaseSecs; }
  public long quoteExpiresAtMs() { return quoteExpiresAtMs; }
  public String feeAssetId() { return feeAssetId; }
  public String escrowAccountId() { return escrowAccountId; }
  public String operatorAccountId() { return operatorAccountId; }
  public long leaseFeeNanos() { return leaseFeeNanos; }
  public List<String> routePushes() { return routePushes; }
  public List<String> excludedRoutes() { return excludedRoutes; }
  public List<String> dnsServers() { return dnsServers; }
  public List<String> tunnelAddresses() { return tunnelAddresses; }
  public long mtuBytes() { return mtuBytes; }
  public String meterFamily() { return meterFamily; }
  public int flowLabelBits() { return flowLabelBits; }
  public int paddingBudgetMs() { return paddingBudgetMs; }
  public String relayTlsSpkiSha256Hex() { return relayTlsSpkiSha256Hex; }
  public String meteringPublicKeyHex() { return meteringPublicKeyHex; }
  public VpnTxInstruction openLeaseInstruction() { return openLeaseInstruction; }
  public List<VpnTxInstruction> txInstructions() { return txInstructions; }
}
