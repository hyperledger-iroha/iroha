package org.hyperledger.iroha.android.sorafs;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * Aggregates the manifest metadata, provider descriptors, and fetch options for a SoraFS gateway
 * request.
 *
 * <p>Call {@link #toJson()} to obtain the structure expected by the Rust orchestrator (the same
 * layout produced by the CLI `sorafs_cli fetch` command).
 */
public final class GatewayFetchRequest {
  private final String manifestIdHex;
  private final String chunkerHandle;
  private final GatewayFetchOptions options;
  private final List<GatewayProvider> providers;

  private GatewayFetchRequest(final Builder builder) {
    this.manifestIdHex = builder.manifestIdHex;
    this.chunkerHandle = builder.chunkerHandle;
    this.options = builder.options;
    this.providers = List.copyOf(builder.providers);
  }

  public String manifestIdHex() {
    return manifestIdHex;
  }

  public String chunkerHandle() {
    return chunkerHandle;
  }

  public GatewayFetchOptions options() {
    return options;
  }

  public List<GatewayProvider> providers() {
    return providers;
  }

  public Map<String, Object> toJson() {
    final Map<String, Object> root = new LinkedHashMap<>();
    root.put("manifest_id_hex", manifestIdHex);
    if (chunkerHandle != null) {
      root.put("chunker_handle", chunkerHandle);
    }
    root.put("options", options.toJson());
    final List<Map<String, Object>> providerMaps = new ArrayList<>(providers.size());
    for (GatewayProvider provider : providers) {
      providerMaps.add(provider.toJson());
    }
    root.put("providers", providerMaps);
    return Collections.unmodifiableMap(root);
  }

  /** Returns the JSON representation of this request as a UTF-8 encoded string. */
  public String toJsonString() {
    return JsonWriter.encode(toJson());
  }

  /** Returns the JSON representation of this request as UTF-8 bytes. */
  public byte[] toJsonBytes() {
    return SorafsGatewayClient.encodeRequestPayload(this);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String manifestIdHex;
    private String chunkerHandle;
    private GatewayFetchOptions options = GatewayFetchOptions.builder().build();
    private final List<GatewayProvider> providers = new ArrayList<>();

    public Builder setManifestIdHex(final String manifestIdHex) {
      this.manifestIdHex = SorafsInputValidator.normalizeHexBytes(manifestIdHex, "manifestIdHex", 32);
      return this;
    }

    public Builder setChunkerHandle(final String chunkerHandle) {
      this.chunkerHandle = chunkerHandle == null ? null : chunkerHandle.trim();
      return this;
    }

    public Builder setOptions(final GatewayFetchOptions options) {
      this.options = Objects.requireNonNull(options, "options");
      return this;
    }

    public Builder addProvider(final GatewayProvider provider) {
      providers.add(Objects.requireNonNull(provider, "provider"));
      return this;
    }

    public Builder clearProviders() {
      providers.clear();
      return this;
    }

    public GatewayFetchRequest build() {
      Objects.requireNonNull(manifestIdHex, "manifestIdHex");
      if (providers.isEmpty()) {
        throw new IllegalStateException("at least one provider must be configured");
      }
      return new GatewayFetchRequest(this);
    }
  }
}
