package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/** Static configuration for a SORA Nexus app facade instance. */
public final class NexusAppConfig {

  private final NetworkId networkId;
  private final String chainId;
  private final int chainDiscriminant;
  private final String appId;
  private final String relayUrl;
  private final String node;
  private final String authority;
  private final byte[] signingPublicKey;
  private final Map<String, String> appMetadata;

  public NexusAppConfig(
      final NetworkId networkId, final String chainId, final int chainDiscriminant) {
    this(
        networkId,
        chainId,
        chainDiscriminant,
        null,
        null,
        null,
        null,
        null,
        Collections.emptyMap());
  }

  public NexusAppConfig(
      final NetworkId networkId,
      final String chainId,
      final int chainDiscriminant,
      final String appId,
      final String relayUrl,
      final String node,
      final String authority,
      final byte[] signingPublicKey,
      final Map<String, String> appMetadata) {
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.chainId = NexusModelUtils.requireNonBlank(chainId, "chainId");
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    this.chainDiscriminant = chainDiscriminant;
    this.appId = appId;
    this.relayUrl = relayUrl;
    this.node = node;
    this.authority = authority;
    this.signingPublicKey = NexusModelUtils.copy(signingPublicKey);
    this.appMetadata = NexusModelUtils.copyMap(appMetadata);
  }

  /** Returns the exact canonical hash identity of the configured network. */
  public NetworkId networkId() {
    return networkId;
  }

  public String chainId() {
    return chainId;
  }

  public int chainDiscriminant() {
    return chainDiscriminant;
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
