package org.hyperledger.iroha.android.sorafs;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Helper for composing SoraFS gateway fetch options.
 *
 * <p>The builder exposes the subset of orchestrator configuration that Android clients commonly
 * tweak (telemetry labels, provider limits, retry budgets, transport/anonymity policy). The helper
 * serialises to a map matching the Norito JSON structure consumed by the Rust orchestrator.
 */
public final class GatewayFetchOptions {
  private final String manifestEnvelopeBase64;
  private final String manifestCidHex;
  private final String clientId;
  private final String telemetryRegion;
  private final String rolloutPhase;
  private final Integer maxPeers;
  private final Integer retryBudget;
  private final TransportPolicy transportPolicy;
  private final AnonymityPolicy anonymityPolicy;
  private final WriteModeHint writeModeHint;

  private GatewayFetchOptions(final Builder builder) {
    this.manifestEnvelopeBase64 = builder.manifestEnvelopeBase64;
    this.manifestCidHex = builder.manifestCidHex;
    this.clientId = builder.clientId;
    this.telemetryRegion = builder.telemetryRegion;
    this.rolloutPhase = builder.rolloutPhase;
    this.maxPeers = builder.maxPeers;
    this.retryBudget = builder.retryBudget;
    this.transportPolicy = builder.transportPolicy;
    this.anonymityPolicy = builder.anonymityPolicy;
    this.writeModeHint = builder.writeModeHint;
  }

  public String manifestEnvelopeBase64() {
    return manifestEnvelopeBase64;
  }

  public String manifestCidHex() {
    return manifestCidHex;
  }

  public String clientId() {
    return clientId;
  }

  public String telemetryRegion() {
    return telemetryRegion;
  }

  public String rolloutPhase() {
    return rolloutPhase;
  }

  public Integer maxPeers() {
    return maxPeers;
  }

  public Integer retryBudget() {
    return retryBudget;
  }

  public TransportPolicy transportPolicy() {
    return transportPolicy;
  }

  public AnonymityPolicy anonymityPolicy() {
    return anonymityPolicy;
  }

  public WriteModeHint writeModeHint() {
    return writeModeHint;
  }

  /**
   * Serialise the options to a JSON-compatible map. Only explicitly configured entries are
   * included.
   */
  public Map<String, Object> toJson() {
    final Map<String, Object> root = new LinkedHashMap<>();
    if (manifestEnvelopeBase64 != null) {
      root.put("manifest_envelope_b64", manifestEnvelopeBase64);
    }
    if (manifestCidHex != null) {
      root.put("manifest_cid_hex", manifestCidHex);
    }
    if (clientId != null) {
      root.put("client_id", clientId);
    }
    if (telemetryRegion != null) {
      root.put("telemetry_region", telemetryRegion);
    }
    if (rolloutPhase != null) {
      root.put("rollout_phase", rolloutPhase);
    }
    if (maxPeers != null) {
      root.put("max_peers", maxPeers);
    }
    if (retryBudget != null) {
      root.put("retry_budget", retryBudget);
    }
    if (transportPolicy != null) {
      root.put("transport_policy", transportPolicy.label());
    }
    if (anonymityPolicy != null) {
      root.put("anonymity_policy", anonymityPolicy.label());
    }
    if (writeModeHint != null && writeModeHint != WriteModeHint.READ_ONLY) {
      root.put("write_mode_hint", writeModeHint.label());
    }
    return root;
  }

  public Builder toBuilder() {
    return new Builder()
        .setManifestEnvelopeBase64(manifestEnvelopeBase64)
        .setManifestCidHex(manifestCidHex)
        .setClientId(clientId)
        .setTelemetryRegion(telemetryRegion)
        .setRolloutPhase(rolloutPhase)
        .setMaxPeers(maxPeers)
        .setRetryBudget(retryBudget)
        .setTransportPolicy(transportPolicy)
        .setAnonymityPolicy(anonymityPolicy)
        .setWriteModeHint(writeModeHint);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String manifestEnvelopeBase64;
    private String manifestCidHex;
    private String clientId;
    private String telemetryRegion;
    private String rolloutPhase;
    private Integer maxPeers;
    private Integer retryBudget;
    // Default to SoraNet-first; callers must explicitly request DirectOnly when staging a downgrade.
    private TransportPolicy transportPolicy = TransportPolicy.SORANET_FIRST;
    private AnonymityPolicy anonymityPolicy = AnonymityPolicy.ANON_GUARD_PQ;
    private WriteModeHint writeModeHint = WriteModeHint.READ_ONLY;

    public Builder setManifestEnvelopeBase64(final String manifestEnvelopeBase64) {
      if (manifestEnvelopeBase64 == null || manifestEnvelopeBase64.trim().isEmpty()) {
        this.manifestEnvelopeBase64 = null;
      } else {
        this.manifestEnvelopeBase64 =
            SorafsInputValidator.normalizeBase64MaybeUrl(
                manifestEnvelopeBase64, "manifestEnvelopeBase64");
      }
      return this;
    }

    public Builder setManifestCidHex(final String manifestCidHex) {
      if (manifestCidHex == null || manifestCidHex.trim().isEmpty()) {
        this.manifestCidHex = null;
      } else {
        this.manifestCidHex = SorafsInputValidator.normalizeHex(manifestCidHex, "manifestCidHex");
      }
      return this;
    }

    public Builder setClientId(final String clientId) {
      this.clientId = emptyToNull(clientId);
      return this;
    }

    public Builder setTelemetryRegion(final String telemetryRegion) {
      this.telemetryRegion = emptyToNull(telemetryRegion);
      return this;
    }

    public Builder setRolloutPhase(final String rolloutPhase) {
      this.rolloutPhase = emptyToNull(rolloutPhase);
      return this;
    }

    public Builder setMaxPeers(final Integer maxPeers) {
      if (maxPeers != null && maxPeers < 1) {
        throw new IllegalArgumentException("maxPeers must be greater than zero");
      }
      this.maxPeers = maxPeers;
      return this;
    }

    public Builder setRetryBudget(final Integer retryBudget) {
      if (retryBudget != null && retryBudget < 0) {
        throw new IllegalArgumentException("retryBudget must be non-negative");
      }
      this.retryBudget = retryBudget;
      return this;
    }

    public Builder setTransportPolicy(final TransportPolicy transportPolicy) {
      this.transportPolicy = Objects.requireNonNull(transportPolicy, "transportPolicy");
      return this;
    }

    public Builder setAnonymityPolicy(final AnonymityPolicy anonymityPolicy) {
      this.anonymityPolicy = Objects.requireNonNull(anonymityPolicy, "anonymityPolicy");
      return this;
    }

    public Builder setWriteModeHint(final WriteModeHint writeModeHint) {
      this.writeModeHint = Objects.requireNonNull(writeModeHint, "writeModeHint");
      return this;
    }

    public GatewayFetchOptions build() {
      return new GatewayFetchOptions(this);
    }

    private static String emptyToNull(final String value) {
      if (value == null) {
        return null;
      }
      final String trimmed = value.trim();
      return trimmed.isEmpty() ? null : trimmed;
    }
  }
}
