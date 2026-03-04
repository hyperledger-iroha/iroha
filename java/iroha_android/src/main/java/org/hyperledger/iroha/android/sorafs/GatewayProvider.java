package org.hyperledger.iroha.android.sorafs;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Descriptor for a SoraFS gateway provider.
 *
 * <p>Matches the key/value structure used by the CLI (`--provider name=…`) so Android callers can
 * construct orchestrator requests deterministically.
 */
public final class GatewayProvider {
  private final String name;
  private final String providerIdHex;
  private final String baseUrl;
  private final String streamTokenBase64;

  private GatewayProvider(final Builder builder) {
    this.name = builder.name;
    this.providerIdHex = builder.providerIdHex;
    this.baseUrl = builder.baseUrl;
    this.streamTokenBase64 = builder.streamTokenBase64;
  }

  public String name() {
    return name;
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public String baseUrl() {
    return baseUrl;
  }

  public String streamTokenBase64() {
    return streamTokenBase64;
  }

  /** Serialise the provider descriptor to a JSON-compatible map. */
  public Map<String, Object> toJson() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("name", name);
    map.put("provider_id_hex", providerIdHex);
    map.put("base_url", baseUrl);
    map.put("stream_token_b64", streamTokenBase64);
    return map;
  }

  public Builder toBuilder() {
    return new Builder()
        .setName(name)
        .setProviderIdHex(providerIdHex)
        .setBaseUrl(baseUrl)
        .setStreamTokenBase64(streamTokenBase64);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String name;
    private String providerIdHex;
    private String baseUrl;
    private String streamTokenBase64;

    public Builder setName(final String name) {
      this.name = SorafsInputValidator.requireNonEmpty(name, "name");
      return this;
    }

    public Builder setProviderIdHex(final String providerIdHex) {
      this.providerIdHex = SorafsInputValidator.normalizeHexBytes(providerIdHex, "providerIdHex", 32);
      return this;
    }

    public Builder setBaseUrl(final String baseUrl) {
      this.baseUrl = SorafsInputValidator.requireNonEmpty(baseUrl, "baseUrl");
      return this;
    }

    public Builder setStreamTokenBase64(final String streamTokenBase64) {
      this.streamTokenBase64 =
          SorafsInputValidator.normalizeBase64MaybeUrl(streamTokenBase64, "streamTokenBase64");
      return this;
    }

    public GatewayProvider build() {
      Objects.requireNonNull(name, "name");
      Objects.requireNonNull(providerIdHex, "providerIdHex");
      Objects.requireNonNull(baseUrl, "baseUrl");
      Objects.requireNonNull(streamTokenBase64, "streamTokenBase64");
      return new GatewayProvider(this);
    }
  }
}
