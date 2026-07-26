package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** SoraFS-backed encrypted artifact reference used by private uploaded-model execution. */
public final class SoracloudPrivateModelArtifactRef {
  private final long schemaVersion;
  private final String sorafsManifestDigest;
  private final String artifactHash;
  private final long ciphertextBytes;
  private final String artifactRole;

  public SoracloudPrivateModelArtifactRef(
      final long schemaVersion,
      final String sorafsManifestDigest,
      final String artifactHash,
      final long ciphertextBytes,
      final String artifactRole) {
    this.schemaVersion = schemaVersion;
    this.sorafsManifestDigest =
        Objects.requireNonNull(sorafsManifestDigest, "sorafsManifestDigest");
    this.artifactHash = Objects.requireNonNull(artifactHash, "artifactHash");
    this.ciphertextBytes = ciphertextBytes;
    this.artifactRole = Objects.requireNonNull(artifactRole, "artifactRole");
  }

  public long schemaVersion() {
    return schemaVersion;
  }

  public String sorafsManifestDigest() {
    return sorafsManifestDigest;
  }

  public String artifactHash() {
    return artifactHash;
  }

  public long ciphertextBytes() {
    return ciphertextBytes;
  }

  public String artifactRole() {
    return artifactRole;
  }
}

