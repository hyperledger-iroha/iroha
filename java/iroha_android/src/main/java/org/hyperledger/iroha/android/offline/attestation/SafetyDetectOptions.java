package org.hyperledger.iroha.android.offline.attestation;

import java.net.URI;
import java.time.Duration;
import java.util.Objects;

/** Configuration controlling the optional Huawei Safety Detect attestation flow. */
public final class SafetyDetectOptions {

  private static final Duration DEFAULT_TIMEOUT = Duration.ofSeconds(10);
  private static final Duration DEFAULT_TOKEN_SKEW = Duration.ofSeconds(30);
  private static final URI DEFAULT_OAUTH_ENDPOINT =
      URI.create("https://oauth-login.cloud.huawei.com/oauth2/v3/token");
  private static final URI DEFAULT_APP_CHECK_ENDPOINT =
      URI.create("https://safetydetectapi.hwclouds.com/v5/appcheck");

  private final boolean enabled;
  private final URI oauthEndpoint;
  private final URI attestationEndpoint;
  private final String clientId;
  private final String clientSecret;
  private final String packageName;
  private final String signingDigestSha256;
  private final Duration requestTimeout;
  private final Duration tokenSkew;

  private SafetyDetectOptions(final Builder builder) {
    this.enabled = builder.enabled;
    this.oauthEndpoint = builder.oauthEndpoint;
    this.attestationEndpoint = builder.attestationEndpoint;
    this.clientId = builder.clientId;
    this.clientSecret = builder.clientSecret;
    this.packageName = builder.packageName;
    this.signingDigestSha256 = builder.signingDigestSha256;
    this.requestTimeout = builder.requestTimeout;
    this.tokenSkew = builder.tokenSkew;
  }

  public static Builder builder() {
    return new Builder();
  }

  public boolean enabled() {
    return enabled;
  }

  public URI oauthEndpoint() {
    return oauthEndpoint;
  }

  public URI attestationEndpoint() {
    return attestationEndpoint;
  }

  public String clientId() {
    return clientId;
  }

  public String clientSecret() {
    return clientSecret;
  }

  public String packageName() {
    return packageName;
  }

  public String signingDigestSha256() {
    return signingDigestSha256;
  }

  public Duration requestTimeout() {
    return requestTimeout;
  }

  public Duration tokenSkew() {
    return tokenSkew;
  }

  public static final class Builder {
    private boolean enabled = true;
    private URI oauthEndpoint = DEFAULT_OAUTH_ENDPOINT;
    private URI attestationEndpoint = DEFAULT_APP_CHECK_ENDPOINT;
    private String clientId;
    private String clientSecret;
    private String packageName;
    private String signingDigestSha256;
    private Duration requestTimeout = DEFAULT_TIMEOUT;
    private Duration tokenSkew = DEFAULT_TOKEN_SKEW;

    public Builder setEnabled(final boolean enabled) {
      this.enabled = enabled;
      return this;
    }

    public Builder setOauthEndpoint(final URI oauthEndpoint) {
      this.oauthEndpoint = Objects.requireNonNull(oauthEndpoint, "oauthEndpoint");
      return this;
    }

    public Builder setAttestationEndpoint(final URI attestationEndpoint) {
      this.attestationEndpoint =
          Objects.requireNonNull(attestationEndpoint, "attestationEndpoint");
      return this;
    }

    public Builder setClientId(final String clientId) {
      this.clientId = clientId;
      return this;
    }

    public Builder setClientSecret(final String clientSecret) {
      this.clientSecret = clientSecret;
      return this;
    }

    public Builder setPackageName(final String packageName) {
      this.packageName = packageName;
      return this;
    }

    public Builder setSigningDigestSha256(final String signingDigestSha256) {
      this.signingDigestSha256 = signingDigestSha256;
      return this;
    }

    public Builder setRequestTimeout(final Duration requestTimeout) {
      this.requestTimeout = Objects.requireNonNull(requestTimeout, "requestTimeout");
      return this;
    }

    public Builder setTokenSkew(final Duration tokenSkew) {
      this.tokenSkew = Objects.requireNonNull(tokenSkew, "tokenSkew");
      return this;
    }

    public SafetyDetectOptions build() {
      if (!enabled) {
        return new SafetyDetectOptions(this);
      }
      if (clientId == null || clientId.isBlank()) {
        throw new IllegalArgumentException("clientId must be set when Safety Detect is enabled");
      }
      if (clientSecret == null || clientSecret.isBlank()) {
        throw new IllegalArgumentException(
            "clientSecret must be set when Safety Detect is enabled");
      }
      if (packageName == null || packageName.isBlank()) {
        throw new IllegalArgumentException(
            "packageName must be set when Safety Detect is enabled");
      }
      if (signingDigestSha256 == null || signingDigestSha256.isBlank()) {
        throw new IllegalArgumentException(
            "signingDigestSha256 must be set when Safety Detect is enabled");
      }
      return new SafetyDetectOptions(this);
    }
  }
}
