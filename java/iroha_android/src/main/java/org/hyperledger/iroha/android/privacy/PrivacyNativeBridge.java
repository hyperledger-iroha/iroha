package org.hyperledger.iroha.android.privacy;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/**
 * Canonical first-release local privacy build-metadata bridge.
 *
 * <p>The native bridge exposes this binary's typed Norito compiled-profile catalog and the
 * Rust-derived byte-complete exact-12 fixture bundle. The catalog never establishes network
 * activation or readiness; callers must fetch a fresh authoritative PrivacyCapabilitySnapshotV1
 * from live Torii before submitting a privacy proof.
 */
public final class PrivacyNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 21;
  public static final int COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES = 256 * 1024;
  public static final int EXACT12_FIXTURE_BUNDLE_MAX_BYTES = 2 * 1024 * 1024;
  private static final String LIBRARY_NAME = "connect_norito_bridge";

  /** Stable ABI-21 result of validating one typed local compiled-profile catalog. */
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

  /** Stable ABI-21 result of validating the Rust-derived exact-12 fixture bundle. */
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

  /**
   * Returns this binary's canonical {@code PrivacyCompiledProfileCatalogV1} Norito archive.
   *
   * <p>This is local build metadata only. Fetch a fresh committed capability snapshot from live
   * Torii for activation and proof-submission readiness.
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
          && requireCompiledProfileCatalog(nativeCompiledProfileCatalog()).length > 0
          && requireExact12FixtureBundle(nativeExact12FixtureBundle()).length > 0;
    } catch (final RuntimeException | LinkageError error) {
      return false;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeCompiledProfileCatalog();

  private static native int nativeValidateCompiledProfileCatalog(byte[] archive);

  private static native byte[] nativeExact12FixtureBundle();

  private static native int nativeValidateExact12FixtureBundle(byte[] archive);
}
