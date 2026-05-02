package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Response emitted by `GET /v1/vpn/profile`. */
public final class VpnProfile {
  private final boolean available;
  private final String relayEndpoint;
  private final List<String> supportedExitClasses;
  private final String defaultExitClass;
  private final long leaseSecs;
  private final long dnsPushIntervalSecs;
  private final String meterFamily;
  private final List<String> routePushes;
  private final List<String> excludedRoutes;
  private final List<String> dnsServers;
  private final List<String> tunnelAddresses;
  private final long mtuBytes;
  private final String displayBillingLabel;
  private final String feeAssetId;
  private final String escrowAccountId;
  private final String operatorAccountId;
  private final long leaseFeeNanos;
  private final long settlementGraceSecs;
  private final int flowLabelBits;
  private final int paddingBudgetMs;
  private final String relayTlsSpkiSha256Hex;

  public VpnProfile(
      final boolean available,
      final String relayEndpoint,
      final List<String> supportedExitClasses,
      final String defaultExitClass,
      final long leaseSecs,
      final long dnsPushIntervalSecs,
      final String meterFamily,
      final List<String> routePushes,
      final List<String> excludedRoutes,
      final List<String> dnsServers,
      final List<String> tunnelAddresses,
      final long mtuBytes,
      final String displayBillingLabel,
      final String feeAssetId,
      final String escrowAccountId,
      final String operatorAccountId,
      final long leaseFeeNanos,
      final long settlementGraceSecs,
      final int flowLabelBits,
      final int paddingBudgetMs,
      final String relayTlsSpkiSha256Hex) {
    this.available = available;
    this.relayEndpoint = Objects.requireNonNull(relayEndpoint, "relayEndpoint");
    this.supportedExitClasses = immutableList(supportedExitClasses);
    this.defaultExitClass = Objects.requireNonNull(defaultExitClass, "defaultExitClass");
    this.leaseSecs = leaseSecs;
    this.dnsPushIntervalSecs = dnsPushIntervalSecs;
    this.meterFamily = Objects.requireNonNull(meterFamily, "meterFamily");
    this.routePushes = immutableList(routePushes);
    this.excludedRoutes = immutableList(excludedRoutes);
    this.dnsServers = immutableList(dnsServers);
    this.tunnelAddresses = immutableList(tunnelAddresses);
    this.mtuBytes = mtuBytes;
    this.displayBillingLabel = Objects.requireNonNull(displayBillingLabel, "displayBillingLabel");
    this.feeAssetId = Objects.requireNonNull(feeAssetId, "feeAssetId");
    this.escrowAccountId = Objects.requireNonNull(escrowAccountId, "escrowAccountId");
    this.operatorAccountId = Objects.requireNonNull(operatorAccountId, "operatorAccountId");
    this.leaseFeeNanos = leaseFeeNanos;
    this.settlementGraceSecs = settlementGraceSecs;
    this.flowLabelBits = flowLabelBits;
    this.paddingBudgetMs = paddingBudgetMs;
    this.relayTlsSpkiSha256Hex = relayTlsSpkiSha256Hex;
  }

  public boolean available() { return available; }
  public String relayEndpoint() { return relayEndpoint; }
  public List<String> supportedExitClasses() { return supportedExitClasses; }
  public String defaultExitClass() { return defaultExitClass; }
  public long leaseSecs() { return leaseSecs; }
  public long dnsPushIntervalSecs() { return dnsPushIntervalSecs; }
  public String meterFamily() { return meterFamily; }
  public List<String> routePushes() { return routePushes; }
  public List<String> excludedRoutes() { return excludedRoutes; }
  public List<String> dnsServers() { return dnsServers; }
  public List<String> tunnelAddresses() { return tunnelAddresses; }
  public long mtuBytes() { return mtuBytes; }
  public String displayBillingLabel() { return displayBillingLabel; }
  public String feeAssetId() { return feeAssetId; }
  public String escrowAccountId() { return escrowAccountId; }
  public String operatorAccountId() { return operatorAccountId; }
  public long leaseFeeNanos() { return leaseFeeNanos; }
  public long settlementGraceSecs() { return settlementGraceSecs; }
  public int flowLabelBits() { return flowLabelBits; }
  public int paddingBudgetMs() { return paddingBudgetMs; }
  public String relayTlsSpkiSha256Hex() { return relayTlsSpkiSha256Hex; }

  static List<String> immutableList(final List<String> values) {
    return Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(values, "values")));
  }
}
