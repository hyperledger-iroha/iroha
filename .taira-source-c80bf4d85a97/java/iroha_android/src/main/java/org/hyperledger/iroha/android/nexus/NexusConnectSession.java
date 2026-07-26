package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.Map;

/** Registered Connect session plus wallet launch metadata. */
public final class NexusConnectSession {

  private final String sessionId;
  private final String walletLaunchUri;
  private final String appId;
  private final String relayUrl;
  private final String node;
  private final String approvedAccount;
  private final byte[] signingPublicKey;
  private final Map<String, String> metadata;

  public NexusConnectSession(final String sessionId, final String walletLaunchUri) {
    this(sessionId, walletLaunchUri, null, null, null, null, null, Collections.emptyMap());
  }

  public NexusConnectSession(
      final String sessionId,
      final String walletLaunchUri,
      final String appId,
      final String relayUrl,
      final String node,
      final String approvedAccount,
      final byte[] signingPublicKey,
      final Map<String, String> metadata) {
    this.sessionId = NexusModelUtils.requireNonBlank(sessionId, "sessionId");
    this.walletLaunchUri = NexusModelUtils.requireNonBlank(walletLaunchUri, "walletLaunchUri");
    this.appId = appId;
    this.relayUrl = relayUrl;
    this.node = node;
    this.approvedAccount = approvedAccount;
    this.signingPublicKey = NexusModelUtils.copy(signingPublicKey);
    this.metadata = NexusModelUtils.copyMap(metadata);
  }

  public NexusConnectSession withApproval(
      final String approvedAccount, final byte[] signingPublicKey) {
    return new NexusConnectSession(
        sessionId, walletLaunchUri, appId, relayUrl, node, approvedAccount, signingPublicKey, metadata);
  }

  public String sessionId() {
    return sessionId;
  }

  public String walletLaunchUri() {
    return walletLaunchUri;
  }

  public String appId() {
    return appId;
  }

  public String relayUrl() {
    return relayUrl;
  }

  public String node() {
    return node;
  }

  public String approvedAccount() {
    return approvedAccount;
  }

  public byte[] signingPublicKey() {
    return NexusModelUtils.copy(signingPublicKey);
  }

  public Map<String, String> metadata() {
    return metadata;
  }
}
