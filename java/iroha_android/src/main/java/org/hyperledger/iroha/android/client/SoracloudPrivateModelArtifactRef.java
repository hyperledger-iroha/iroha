package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** SoraFS-backed encrypted artifact reference used by private uploaded-model execution. */
public final class SoracloudPrivateModelArtifactRef {
  private final long schemaVersion;
  private final byte[] sorafsManifestDigest;
  private final List<Integer> sorafsRootCid;
  private final String artifactHash;
  private final long ciphertextBytes;
  private final String artifactRole;

  public SoracloudPrivateModelArtifactRef(
      final long schemaVersion,
      final byte[] sorafsManifestDigest,
      final List<Integer> sorafsRootCid,
      final String artifactHash,
      final long ciphertextBytes,
      final String artifactRole) {
    SoracloudPrivateModelValidation.requireSchemaVersion(schemaVersion, "schemaVersion");
    if (ciphertextBytes < 1L
        || ciphertextBytes
            > SoracloudPrivateModelValidation.ENCRYPTED_ARTIFACT_MAX_BYTES) {
      throw new IllegalArgumentException(
          "ciphertextBytes must be within 1..="
              + SoracloudPrivateModelValidation.ENCRYPTED_ARTIFACT_MAX_BYTES);
    }
    this.schemaVersion = schemaVersion;
    this.sorafsManifestDigest = canonicalManifestDigest(sorafsManifestDigest);
    this.sorafsRootCid = canonicalSorafsRootCid(sorafsRootCid);
    this.artifactHash =
        SoracloudPrivateModelValidation.requireSoracloudHash(artifactHash, "artifactHash");
    this.ciphertextBytes = ciphertextBytes;
    this.artifactRole =
        SoracloudPrivateModelValidation.requireArtifactRole(artifactRole, "artifactRole");
  }

  public long schemaVersion() {
    return schemaVersion;
  }

  public byte[] sorafsManifestDigest() {
    return sorafsManifestDigest.clone();
  }

  public List<Integer> sorafsRootCid() {
    return sorafsRootCid;
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

  private static byte[] canonicalManifestDigest(final byte[] value) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(
          "sorafsManifestDigest must contain exactly 32 bytes");
    }
    return value.clone();
  }

  private static List<Integer> canonicalSorafsRootCid(final List<Integer> value) {
    Objects.requireNonNull(value, "sorafsRootCid");
    if (value.size() != 36) {
      throw new IllegalArgumentException("sorafsRootCid must contain exactly 36 bytes");
    }
    final ArrayList<Integer> copy = new ArrayList<>(value.size());
    boolean nonzeroDigest = false;
    for (int index = 0; index < value.size(); index++) {
      final Integer element = value.get(index);
      if (element == null || element.intValue() < 0 || element.intValue() > 255) {
        throw new IllegalArgumentException("sorafsRootCid elements must be unsigned bytes");
      }
      copy.add(element);
      if (index >= 4 && element.intValue() != 0) {
        nonzeroDigest = true;
      }
    }
    if (copy.get(0) != 1 || copy.get(1) != 0x71 || copy.get(2) != 0x1f || copy.get(3) != 32) {
      throw new IllegalArgumentException(
          "sorafsRootCid must use canonical CIDv1/dag-cbor/BLAKE3-256 framing");
    }
    if (!nonzeroDigest) {
      throw new IllegalArgumentException("sorafsRootCid digest must be nonzero");
    }
    return Collections.unmodifiableList(copy);
  }
}
