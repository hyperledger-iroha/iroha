package org.hyperledger.iroha.android.privacy;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.model.NetworkId;

/**
 * Canonical first-release local privacy build-metadata bridge.
 *
 * <p>The native bridge exposes this binary's typed Norito compiled-profile catalog, validates
 * Torii's canonical Exact12 capability manifest, and exposes the Rust-derived byte-complete
 * fixture bundle. The local catalog never establishes network activation or readiness.
 */
public final class PrivacyNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 22;
  public static final int CONFIDENTIAL_DERIVATION_CONTRACT_REVISION_V3 = 1;
  public static final int EXACT12_CAPABILITY_MANIFEST_MAX_BYTES = 256 * 1024;
  public static final int COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES = 256 * 1024;
  public static final int EXACT12_FIXTURE_BUNDLE_MAX_BYTES = 2 * 1024 * 1024;
  private static final int CONFIDENTIAL_TREE_DEPTH = 16;
  private static final int CONFIDENTIAL_TREE_CAPACITY = 1 << CONFIDENTIAL_TREE_DEPTH;
  private static final int CONFIDENTIAL_MERKLE_PATH_BYTES =
      32 + CONFIDENTIAL_TREE_DEPTH * 32 + CONFIDENTIAL_TREE_DEPTH;
  private static final String LIBRARY_NAME = "connect_norito_bridge";

  /** Stable ABI-22 result of validating one typed local compiled-profile catalog. */
  public enum CompiledProfileCatalogValidationStatusV1 {
    VALID(0),
    NULL_POINTER(1),
    EMPTY(2),
    ARCHIVE_TOO_LARGE(3),
    DECODE_RESOURCE_LIMIT(4),
    SCHEMA_MISMATCH(5),
    NON_CANONICAL(6),
    MALFORMED_ARCHIVE(7),
    INVALID_CATALOG(8);

    private final int code;

    CompiledProfileCatalogValidationStatusV1(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }
  }

  /** Stable ABI-22 result of validating the Rust-derived exact-12 fixture bundle. */
  public enum Exact12FixtureValidationStatusV1 {
    VALID(0),
    NULL_POINTER(1),
    EMPTY(2),
    ARCHIVE_TOO_LARGE(3),
    DECODE_RESOURCE_LIMIT(4),
    SCHEMA_MISMATCH(5),
    NON_CANONICAL(6),
    MALFORMED_ARCHIVE(7),
    INVALID_BUNDLE(8);

    private final int code;

    Exact12FixtureValidationStatusV1(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }

    private static Exact12FixtureValidationStatusV1 fromCode(final int code) {
      for (final Exact12FixtureValidationStatusV1 value : values()) {
        if (value.code == code) {
          return value;
        }
      }
      throw new IllegalStateException(
          "native exact-12 privacy fixture validation returned an unknown status");
    }
  }

  /** Stable native result of validating one canonical committed Exact12 manifest. */
  public enum Exact12CapabilityManifestValidationStatusV1 {
    VALID(0),
    NULL_POINTER(1),
    EMPTY(2),
    ARCHIVE_TOO_LARGE(3),
    DECODE_RESOURCE_LIMIT(4),
    SCHEMA_MISMATCH(5),
    NON_CANONICAL(6),
    MALFORMED_ARCHIVE(7),
    INVALID_MANIFEST(8);

    private final int code;

    Exact12CapabilityManifestValidationStatusV1(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }

    private static Exact12CapabilityManifestValidationStatusV1 fromCode(final int code) {
      for (final Exact12CapabilityManifestValidationStatusV1 value : values()) {
        if (value.code == code) {
          return value;
        }
      }
      throw new IllegalStateException(
          "native Exact12 capability validation returned an unknown status");
    }
  }

  private static final List<PrivacyProtocolIdV1> PROTOCOLS =
      Collections.unmodifiableList(Arrays.asList(PrivacyProtocolIdV1.values()));
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private PrivacyNativeBridge() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  /** Returns all twelve protocol identities in exact wire order. */
  public static List<PrivacyProtocolIdV1> protocolsV1() {
    return PROTOCOLS;
  }

  static byte[] defaultConfidentialDiversifierV3() {
    return confidentialDigest(
        "default diversifier", PrivacyNativeBridge::nativeDefaultConfidentialDiversifierV3);
  }

  static byte[] deriveConfidentialDiversifierV3(final byte[] seed) {
    if (seed == null || seed.length == 0 || seed.length > 4096) {
      throw new IllegalArgumentException(
          "confidential diversifier seed must contain 1..4096 bytes");
    }
    final byte[] snapshot = seed.clone();
    return confidentialDigest(
        "diversifier", () -> nativeDeriveConfidentialDiversifierV3(snapshot));
  }

  static byte[] deriveConfidentialOwnerTagV3(
      final byte[] spendKey, final byte[] diversifier) {
    final byte[] spend = confidentialInput32(spendKey, "spendKey");
    final byte[] diversified = confidentialInput32(diversifier, "diversifier");
    return confidentialDigest(
        "owner tag", () -> nativeDeriveConfidentialOwnerTagV3(spend, diversified));
  }

  static byte[] deriveConfidentialAssetTagV3(final String asset) {
    final byte[] encoded = confidentialText(asset, "asset");
    return confidentialDigest("asset tag", () -> nativeDeriveConfidentialAssetTagV3(encoded));
  }

  static byte[] deriveConfidentialNetworkTagV3(final NetworkId networkId) {
    if (networkId == null) {
      throw new IllegalArgumentException("networkId must be provided");
    }
    return confidentialDigest(
        "network tag", () -> nativeDeriveConfidentialNetworkTagV3(networkId.bytes()));
  }

  static byte[] deriveConfidentialNoteCommitmentV3(
      final String asset,
      final String amount,
      final byte[] rho,
      final byte[] ownerTag) {
    final byte[] encodedAsset = confidentialText(asset, "asset");
    final byte[] encodedAmount = confidentialPositiveU128(amount);
    final byte[] exactRho = confidentialInput32(rho, "rho");
    final byte[] exactOwner = confidentialInput32(ownerTag, "ownerTag");
    return confidentialDigest(
        "note commitment",
        () ->
            nativeDeriveConfidentialNoteCommitmentV3(
                encodedAsset, encodedAmount, exactRho, exactOwner));
  }

  static byte[] deriveConfidentialNullifierV3(
      final NetworkId networkId,
      final String asset,
      final byte[] spendKey,
      final byte[] rho) {
    if (networkId == null) {
      throw new IllegalArgumentException("networkId must be provided");
    }
    final byte[] encodedAsset = confidentialText(asset, "asset");
    final byte[] spend = confidentialInput32(spendKey, "spendKey");
    final byte[] exactRho = confidentialInput32(rho, "rho");
    return confidentialDigest(
        "nullifier",
        () ->
            nativeDeriveConfidentialNullifierV3(
                networkId.bytes(), encodedAsset, spend, exactRho));
  }

  static byte[] deriveConfidentialMerklePathV3(
      final List<byte[]> commitments, final long leafIndex) {
    if (commitments == null
        || commitments.isEmpty()
        || commitments.size() > CONFIDENTIAL_TREE_CAPACITY) {
      throw new IllegalArgumentException(
          "commitments must contain 1.." + CONFIDENTIAL_TREE_CAPACITY + " leaves");
    }
    if (leafIndex < 0 || leafIndex >= commitments.size()) {
      throw new IllegalArgumentException("leafIndex must identify one supplied commitment");
    }
    final byte[] packed = new byte[commitments.size() * 32];
    for (int index = 0; index < commitments.size(); index++) {
      final byte[] commitment =
          confidentialInput32(commitments.get(index), "commitments[" + index + "]");
      System.arraycopy(commitment, 0, packed, index * 32, 32);
    }
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native confidential V3 Merkle derivation is unavailable");
    }
    final byte[] path = nativeDeriveConfidentialMerklePathV3(packed, leafIndex);
    if (path == null || path.length != CONFIDENTIAL_MERKLE_PATH_BYTES) {
      throw new IllegalStateException(
          "native confidential V3 Merkle derivation returned an invalid path");
    }
    return path.clone();
  }

  static boolean verifyConfidentialMerklePathV3(
      final byte[] commitment,
      final long leafIndex,
      final List<byte[]> siblings,
      final byte[] directions,
      final byte[] root) {
    if (leafIndex < 0
        || siblings == null
        || siblings.size() != CONFIDENTIAL_TREE_DEPTH
        || directions == null
        || directions.length != CONFIDENTIAL_TREE_DEPTH
        || commitment == null
        || commitment.length != 32
        || root == null
        || root.length != 32) {
      return false;
    }
    final byte[] packedSiblings = new byte[CONFIDENTIAL_TREE_DEPTH * 32];
    for (int index = 0; index < siblings.size(); index++) {
      final byte[] sibling = siblings.get(index);
      if (sibling == null || sibling.length != 32) {
        return false;
      }
      System.arraycopy(sibling, 0, packedSiblings, index * 32, 32);
    }
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native confidential V3 Merkle verification is unavailable");
    }
    return nativeVerifyConfidentialMerklePathV3(
        commitment.clone(),
        leafIndex,
        packedSiblings,
        directions.clone(),
        root.clone());
  }

  private static byte[] confidentialDigest(
      final String label, final NativeDigestDerivation derivation) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native confidential V3 derivation is unavailable");
    }
    final byte[] digest;
    try {
      digest = derivation.derive();
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException("native confidential " + label + " derivation failed", error);
    }
    if (digest == null || digest.length != 32 || allZero(digest)) {
      throw new IllegalStateException(
          "native confidential " + label + " derivation returned an invalid digest");
    }
    return digest.clone();
  }

  private static byte[] confidentialInput32(final byte[] value, final String label) {
    if (value == null || value.length != 32 || allZero(value)) {
      throw new IllegalArgumentException(label + " must be exactly 32 non-zero bytes");
    }
    return value.clone();
  }

  private static byte[] confidentialText(final String value, final String label) {
    if (value == null
        || value.isEmpty()
        || !value.equals(value.trim())
        || value.indexOf('\0') >= 0) {
      throw new IllegalArgumentException(label + " must be canonical non-empty text");
    }
    final byte[] encoded = value.getBytes(StandardCharsets.UTF_8);
    if (encoded.length > 512) {
      throw new IllegalArgumentException(label + " exceeds the native byte bound");
    }
    return encoded;
  }

  private static byte[] confidentialPositiveU128(final String value) {
    if (value == null || value.isEmpty() || value.length() > 39) {
      throw new IllegalArgumentException("amount must be a canonical positive u128");
    }
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) < '0' || value.charAt(index) > '9') {
        throw new IllegalArgumentException("amount must be a canonical positive u128");
      }
    }
    if (value.equals("0")
        || (value.length() > 1 && value.charAt(0) == '0')
        || (value.length() == 39
            && value.compareTo("340282366920938463463374607431768211455") > 0)) {
      throw new IllegalArgumentException("amount must be a canonical positive u128");
    }
    return value.getBytes(StandardCharsets.US_ASCII);
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) {
      if (item != 0) {
        return false;
      }
    }
    return true;
  }

  @FunctionalInterface
  private interface NativeDigestDerivation {
    byte[] derive();
  }

  /**
   * Returns this binary's canonical {@code PrivacyCompiledProfileCatalogV1} Norito archive.
   *
   * <p>This is local build metadata only. Fetch a fresh committed Exact12 capability manifest from
   * live Torii for activation and proof-submission readiness.
   */
  public static byte[] compiledProfileCatalogV1() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native privacy compiled-profile catalog is unavailable");
    }
    final byte[] archive;
    try {
      archive = nativeCompiledProfileCatalog();
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException(
          "native privacy compiled-profile catalog query failed", error);
    }
    return requireCompiledProfileCatalog(archive);
  }

  /** Returns this binary's local catalog as the closed typed first-release model. */
  public static org.hyperledger.iroha.sdk.privacy.PrivacyCompiledProfileCatalogV1
      compiledProfileCatalogTypedV1() {
    return org.hyperledger.iroha.sdk.privacy.PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(
        compiledProfileCatalogV1());
  }

  /** Validate one canonical committed Exact12 manifest through the native Rust decoder. */
  public static Exact12CapabilityManifestValidationStatusV1
      validateExact12CapabilityManifestV1(final byte[] archive) {
    if (archive == null) {
      return Exact12CapabilityManifestValidationStatusV1.NULL_POINTER;
    }
    if (archive.length == 0) {
      return Exact12CapabilityManifestValidationStatusV1.EMPTY;
    }
    if (archive.length > EXACT12_CAPABILITY_MANIFEST_MAX_BYTES) {
      return Exact12CapabilityManifestValidationStatusV1.ARCHIVE_TOO_LARGE;
    }
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native Exact12 capability validation is unavailable");
    }
    final int code;
    try {
      code = nativeValidateExact12CapabilityManifest(archive);
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException("native Exact12 capability validation failed", error);
    }
    return Exact12CapabilityManifestValidationStatusV1.fromCode(code);
  }

  /**
   * Decode native-validated canonical Torii bytes into the shared JVM model.
   *
   * <p>The Kotlin bridge repeats native validation and obtains the Rust-owned complete local tuple
   * comparison. No Java or Kotlin fallback can authorize a missing native artifact.
   */
  public static org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityManifestV1
      decodeExact12CapabilityManifestV1(final byte[] archive) {
    if (validateExact12CapabilityManifestV1(archive)
        != Exact12CapabilityManifestValidationStatusV1.VALID) {
      throw new IllegalStateException("invalid canonical Exact12 capability manifest");
    }
    return org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge
        .decodeExact12CapabilityManifestV1(Arrays.copyOf(archive, archive.length));
  }

  /** Validates bytes as the exact compiled-profile catalog of the loaded binary. */
  public static CompiledProfileCatalogValidationStatusV1 validateCompiledProfileCatalogV1(
      final byte[] archive) {
    if (archive == null) {
      return CompiledProfileCatalogValidationStatusV1.NULL_POINTER;
    }
    if (archive.length == 0) {
      return CompiledProfileCatalogValidationStatusV1.EMPTY;
    }
    if (archive.length > COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES) {
      return CompiledProfileCatalogValidationStatusV1.ARCHIVE_TOO_LARGE;
    }
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native privacy compiled-profile catalog is unavailable");
    }
    final int code;
    try {
      code = nativeValidateCompiledProfileCatalog(archive);
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException(
          "native privacy compiled-profile catalog validation failed", error);
    }
    for (final CompiledProfileCatalogValidationStatusV1 value :
        CompiledProfileCatalogValidationStatusV1.values()) {
      if (value.code() == code) {
        return value;
      }
    }
    throw new IllegalStateException(
        "native privacy compiled-profile catalog validation returned an unknown status");
  }

  /** Returns canonical Rust-derived exact-12 bytes through signed-transaction and hash layers. */
  public static byte[] exact12FixtureBundleV1() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native exact-12 privacy fixture bridge is unavailable");
    }
    final byte[] archive;
    try {
      archive = nativeExact12FixtureBundle();
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException("native exact-12 privacy fixture query failed", error);
    }
    return requireExact12FixtureBundle(archive);
  }

  /**
   * Validates untrusted bytes against the canonical Rust-derived exact-12 fixture bundle.
   *
   * <p>Null, empty, and oversized inputs are rejected before JNI copies any bytes.
   */
  public static Exact12FixtureValidationStatusV1 validateExact12FixtureBundleV1(
      final byte[] archive) {
    if (archive == null) {
      return Exact12FixtureValidationStatusV1.NULL_POINTER;
    }
    if (archive.length == 0) {
      return Exact12FixtureValidationStatusV1.EMPTY;
    }
    if (archive.length > EXACT12_FIXTURE_BUNDLE_MAX_BYTES) {
      return Exact12FixtureValidationStatusV1.ARCHIVE_TOO_LARGE;
    }
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native exact-12 privacy fixture bridge is unavailable");
    }
    final int code;
    try {
      code = nativeValidateExact12FixtureBundle(archive);
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException(
          "native exact-12 privacy fixture validation failed", error);
    }
    return Exact12FixtureValidationStatusV1.fromCode(code);
  }

  static byte[] requireCompiledProfileCatalog(final byte[] archive) {
    if (archive == null
        || archive.length == 0
        || archive.length > COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES) {
      throw new IllegalStateException("invalid privacy compiled-profile catalog length");
    }
    final int status = nativeValidateCompiledProfileCatalog(archive);
    if (status != CompiledProfileCatalogValidationStatusV1.VALID.code()) {
      throw new IllegalStateException("invalid typed privacy compiled-profile catalog");
    }
    final byte[] snapshot = Arrays.copyOf(archive, archive.length);
    org.hyperledger.iroha.sdk.privacy.PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(snapshot);
    return snapshot;
  }

  static byte[] requireExact12FixtureBundle(final byte[] archive) {
    if (archive == null
        || archive.length == 0
        || archive.length > EXACT12_FIXTURE_BUNDLE_MAX_BYTES) {
      throw new IllegalStateException("invalid exact-12 privacy fixture bundle length");
    }
    final int status = nativeValidateExact12FixtureBundle(archive);
    if (status != Exact12FixtureValidationStatusV1.VALID.code()) {
      throw new IllegalStateException("invalid exact-12 privacy fixture bundle");
    }
    return Arrays.copyOf(archive, archive.length);
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION
          && nativeConfidentialDerivationContractRevisionV3()
              == CONFIDENTIAL_DERIVATION_CONTRACT_REVISION_V3
          && nativeValidateExact12CapabilityManifest(null)
              == Exact12CapabilityManifestValidationStatusV1.NULL_POINTER.code()
          && nativeInspectExact12CapabilityManifest(null) == null
          && !nativeRequireExact12CapabilityTuple(null, -1)
          && !nativeValidateExact12SubmitProofConstruction(null, -1, null)
          && requireCompiledProfileCatalog(nativeCompiledProfileCatalog()).length > 0
          && requireExact12FixtureBundle(nativeExact12FixtureBundle()).length > 0;
    } catch (final RuntimeException | LinkageError error) {
      return false;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native int nativeConfidentialDerivationContractRevisionV3();

  private static native byte[] nativeDefaultConfidentialDiversifierV3();

  private static native byte[] nativeDeriveConfidentialDiversifierV3(byte[] seed);

  private static native byte[] nativeDeriveConfidentialOwnerTagV3(
      byte[] spendKey, byte[] diversifier);

  private static native byte[] nativeDeriveConfidentialAssetTagV3(byte[] assetUtf8);

  private static native byte[] nativeDeriveConfidentialNetworkTagV3(byte[] networkId);

  private static native byte[] nativeDeriveConfidentialNoteCommitmentV3(
      byte[] assetUtf8, byte[] amountAscii, byte[] rho, byte[] ownerTag);

  private static native byte[] nativeDeriveConfidentialNullifierV3(
      byte[] networkId, byte[] assetUtf8, byte[] spendKey, byte[] rho);

  private static native byte[] nativeDeriveConfidentialMerklePathV3(
      byte[] commitments, long leafIndex);

  private static native boolean nativeVerifyConfidentialMerklePathV3(
      byte[] commitment,
      long leafIndex,
      byte[] siblings,
      byte[] directions,
      byte[] root);

  private static native byte[] nativeCompiledProfileCatalog();

  private static native int nativeValidateCompiledProfileCatalog(byte[] archive);

  private static native int nativeValidateExact12CapabilityManifest(byte[] archive);

  private static native byte[] nativeInspectExact12CapabilityManifest(byte[] archive);

  private static native boolean nativeRequireExact12CapabilityTuple(
      byte[] archive, int protocolIndex);

  private static native boolean nativeValidateExact12SubmitProofConstruction(
      byte[] manifestArchive, int protocolIndex, byte[] instructionArchive);

  private static native byte[] nativeExact12FixtureBundle();

  private static native int nativeValidateExact12FixtureBundle(byte[] archive);
}
