package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.Map;

/** Static configuration for a SORA Nexus app facade instance. */
public final class NexusAppConfig {

  private final String chainId;
  private final String appId;
  private final String relayUrl;
  private final String node;
  private final String authority;
  private final byte[] signingPublicKey;
  private final Map<String, String> appMetadata;

  public NexusAppConfig(final String chainId) {
    this(chainId, null, null, null, null, null, Collections.emptyMap());
  }

  public NexusAppConfig(
      final String chainId,
      final String appId,
      final String relayUrl,
      final String node,
      final String authority,
      final byte[] signingPublicKey,
      final Map<String, String> appMetadata) {
    this.chainId = NexusModelUtils.requireNonBlank(chainId, "chainId");
    this.appId = appId;
    this.relayUrl = relayUrl;
    this.node = node;
    this.authority = authority;
    this.signingPublicKey = NexusModelUtils.copy(signingPublicKey);
    this.appMetadata = NexusModelUtils.copyMap(appMetadata);
  }

  public String chainId() {
    return chainId;
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

  public String authority() {
    return authority;
  }

  public byte[] signingPublicKey() {
    return NexusModelUtils.copy(signingPublicKey);
  }

  public Map<String, String> appMetadata() {
    return appMetadata;
  }
}
