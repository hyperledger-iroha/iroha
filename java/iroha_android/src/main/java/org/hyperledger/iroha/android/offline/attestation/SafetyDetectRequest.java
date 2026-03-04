package org.hyperledger.iroha.android.offline.attestation;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.offline.OfflineVerdictMetadata;

/** Immutable payload describing the context of a Safety Detect attestation attempt. */
public final class SafetyDetectRequest {

  private final String certificateIdHex;
  private final String appId;
  private final String nonceHex;
  private final String packageName;
  private final String signingDigestSha256;
  private final List<String> requiredEvaluations;
  private final Long maxTokenAgeMs;
  private final OfflineVerdictMetadata.SafetyDetectMetadata metadata;

  private SafetyDetectRequest(final Builder builder) {
    this.certificateIdHex = builder.certificateIdHex;
    this.appId = builder.appId;
    this.nonceHex = builder.nonceHex;
    this.packageName = builder.packageName;
    this.signingDigestSha256 = builder.signingDigestSha256;
    this.requiredEvaluations =
        builder.requiredEvaluations == null
            ? null
            : Collections.unmodifiableList(new ArrayList<>(builder.requiredEvaluations));
    this.maxTokenAgeMs = builder.maxTokenAgeMs;
    this.metadata = builder.metadata;
  }

  public static Builder builder() {
    return new Builder();
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public String appId() {
    return appId;
  }

  public String nonceHex() {
    return nonceHex;
  }

  public String packageName() {
    return packageName;
  }

  public String signingDigestSha256() {
    return signingDigestSha256;
  }

  public List<String> requiredEvaluations() {
    if (requiredEvaluations != null) {
      return requiredEvaluations;
    }
    if (metadata != null) {
      return metadata.requiredEvaluations();
    }
    return List.of();
  }

  public Long maxTokenAgeMs() {
    if (maxTokenAgeMs != null) {
      return maxTokenAgeMs;
    }
    if (metadata != null) {
      return metadata.maxTokenAgeMs();
    }
    return null;
  }

  public OfflineVerdictMetadata.SafetyDetectMetadata metadata() {
    return metadata;
  }

  public static final class Builder {
    private String certificateIdHex;
    private String appId;
    private String nonceHex;
    private String packageName;
    private String signingDigestSha256;
    private List<String> requiredEvaluations;
    private Long maxTokenAgeMs;
    private OfflineVerdictMetadata.SafetyDetectMetadata metadata;

    private Builder() {}

    public Builder setCertificateIdHex(final String certificateIdHex) {
      this.certificateIdHex = certificateIdHex;
      return this;
    }

    public Builder setAppId(final String appId) {
      this.appId = appId;
      return this;
    }

    public Builder setNonceHex(final String nonceHex) {
      this.nonceHex = nonceHex;
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

    public Builder setRequiredEvaluations(final List<String> requiredEvaluations) {
      this.requiredEvaluations = requiredEvaluations;
      return this;
    }

    public Builder setMaxTokenAgeMs(final Long maxTokenAgeMs) {
      this.maxTokenAgeMs = maxTokenAgeMs;
      return this;
    }

    public Builder setMetadata(
        final OfflineVerdictMetadata.SafetyDetectMetadata metadata) {
      this.metadata = metadata;
      return this;
    }

    public SafetyDetectRequest build() {
      Objects.requireNonNull(certificateIdHex, "certificateIdHex");
      Objects.requireNonNull(appId, "appId");
      Objects.requireNonNull(nonceHex, "nonceHex");
      Objects.requireNonNull(packageName, "packageName");
      Objects.requireNonNull(signingDigestSha256, "signingDigestSha256");
      return new SafetyDetectRequest(this);
    }
  }
}
