package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.Map;
import java.util.Set;

/** App-role Connect registration options. */
public final class NexusConnectOptions {

  private final Set<String> scopes;
  private final String walletUriBase;
  private final String node;
  private final Map<String, String> metadata;
  private final String sessionId;

  public NexusConnectOptions() {
    this(Collections.emptySet(), null, null, Collections.emptyMap(), null);
  }

  public NexusConnectOptions(
      final Set<String> scopes,
      final String walletUriBase,
      final String node,
      final Map<String, String> metadata) {
    this(scopes, walletUriBase, node, metadata, null);
  }

  public NexusConnectOptions(
      final Set<String> scopes,
      final String walletUriBase,
      final String node,
      final Map<String, String> metadata,
      final String sessionId) {
    this.scopes = NexusModelUtils.copySet(scopes);
    this.walletUriBase = walletUriBase;
    this.node = node;
    this.metadata = NexusModelUtils.copyMap(metadata);
    this.sessionId = sessionId;
  }

  public Set<String> scopes() {
    return scopes;
  }

  public String walletUriBase() {
    return walletUriBase;
  }

  public String node() {
    return node;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public String sessionId() {
    return sessionId;
  }
}
