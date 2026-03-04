package org.hyperledger.iroha.android.offline.attestation;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.offline.OfflineVerdictMetadata;

/** Immutable payload describing the context of a Play Integrity attestation attempt. */
public final class PlayIntegrityRequest {

  private final String certificateIdHex;
  private final String nonceHex;
  private final long cloudProjectNumber;
  private final String environment;
  private final String packageName;
  private final String signingDigestSha256;
  private final List<String> allowedAppVerdicts;
  private final List<String> allowedDeviceVerdicts;
  private final Long maxTokenAgeMs;
  private final OfflineVerdictMetadata.PlayIntegrityMetadata metadata;

  private PlayIntegrityRequest(final Builder builder) {
    this.certificateIdHex = builder.certificateIdHex;
    this.nonceHex = builder.nonceHex;
    this.cloudProjectNumber = builder.cloudProjectNumber;
    this.environment = builder.environment;
    this.packageName = builder.packageName;
    this.signingDigestSha256 = builder.signingDigestSha256;
    this.allowedAppVerdicts =
        builder.allowedAppVerdicts == null
            ? List.of()
            : Collections.unmodifiableList(new ArrayList<>(builder.allowedAppVerdicts));
    this.allowedDeviceVerdicts =
        builder.allowedDeviceVerdicts == null
            ? List.of()
            : Collections.unmodifiableList(new ArrayList<>(builder.allowedDeviceVerdicts));
    this.maxTokenAgeMs = builder.maxTokenAgeMs;
    this.metadata = builder.metadata;
  }

  public static Builder builder() {
    return new Builder();
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public String nonceHex() {
    return nonceHex;
  }

  public long cloudProjectNumber() {
    return cloudProjectNumber;
  }

  public String environment() {
    return environment;
  }

  public String packageName() {
    return packageName;
  }

  public String signingDigestSha256() {
    return signingDigestSha256;
  }

  public List<String> allowedAppVerdicts() {
    return allowedAppVerdicts;
  }

  public List<String> allowedDeviceVerdicts() {
    return allowedDeviceVerdicts;
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

  public OfflineVerdictMetadata.PlayIntegrityMetadata metadata() {
    return metadata;
  }

  public static final class Builder {
    private String certificateIdHex;
    private String nonceHex;
    private long cloudProjectNumber;
    private String environment;
    private String packageName;
    private String signingDigestSha256;
    private List<String> allowedAppVerdicts;
    private List<String> allowedDeviceVerdicts;
    private Long maxTokenAgeMs;
    private OfflineVerdictMetadata.PlayIntegrityMetadata metadata;

    private Builder() {}

    public Builder setCertificateIdHex(final String certificateIdHex) {
      this.certificateIdHex = certificateIdHex;
      return this;
    }

    public Builder setNonceHex(final String nonceHex) {
      this.nonceHex = nonceHex;
      return this;
    }

    public Builder setCloudProjectNumber(final long cloudProjectNumber) {
      this.cloudProjectNumber = cloudProjectNumber;
      return this;
    }

    public Builder setEnvironment(final String environment) {
      this.environment = environment;
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

    public Builder setAllowedAppVerdicts(final List<String> allowedAppVerdicts) {
      this.allowedAppVerdicts = allowedAppVerdicts;
      return this;
    }

    public Builder setAllowedDeviceVerdicts(final List<String> allowedDeviceVerdicts) {
      this.allowedDeviceVerdicts = allowedDeviceVerdicts;
      return this;
    }

    public Builder setMaxTokenAgeMs(final Long maxTokenAgeMs) {
      this.maxTokenAgeMs = maxTokenAgeMs;
      return this;
    }

    public Builder setMetadata(final OfflineVerdictMetadata.PlayIntegrityMetadata metadata) {
      this.metadata = metadata;
      return this;
    }

    public PlayIntegrityRequest build() {
      Objects.requireNonNull(certificateIdHex, "certificateIdHex");
      Objects.requireNonNull(nonceHex, "nonceHex");
      Objects.requireNonNull(environment, "environment");
      Objects.requireNonNull(packageName, "packageName");
      Objects.requireNonNull(signingDigestSha256, "signingDigestSha256");
      if (cloudProjectNumber <= 0) {
        throw new IllegalArgumentException("cloudProjectNumber must be positive");
      }
      return new PlayIntegrityRequest(this);
    }
  }
}
