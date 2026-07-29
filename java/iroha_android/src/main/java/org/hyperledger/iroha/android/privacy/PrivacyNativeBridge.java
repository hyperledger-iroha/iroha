package org.hyperledger.iroha.android.privacy;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/**
 * Canonical first-release privacy capability bridge.
 *
 * <p>The native bridge exposes the authoritative typed Norito capability snapshot only. Proof
 * construction and verification are protocol-specific APIs; this class deliberately has no
 * free-form algorithm, entrypoint, request, build, or verify route.
 */
public final class PrivacyNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 21;
  public static final int PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 256 * 1024;
  private static final String LIBRARY_NAME = "connect_norito_bridge";

  /** Stable ABI-21 result of validating one typed privacy capability archive. */
  public enum ValidationStatusV1 {
    VALID(0),
    NULL_POINTER(1),
    EMPTY(2),
    ARCHIVE_TOO_LARGE(3),
    DECODE_RESOURCE_LIMIT(4),
    SCHEMA_MISMATCH(5),
    NON_CANONICAL(6),
    MALFORMED_ARCHIVE(7),
    INVALID_SNAPSHOT(8);

    private final int code;

    ValidationStatusV1(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }
  }

  /** Closed first-release protocol identity in canonical Norito discriminant order. */
  public enum ProtocolIdV1 {
    ZK_ACE_PQ_AUTHORIZATION_V0("zk-ace-pq-authorization-v0"),
    ANONYMOUS_PGC_K_OUT_OF_N_V1("anonymous-pgc-k-out-of-n-v1"),
    VERANGE_TRANSPARENT_RANGE_V1("verange-transparent-range-v1"),
    IROHA_ZK_AMS_V1("iroha-zk-ams-v1"),
    VEGA_EXISTING_CREDENTIAL_ZK_V0("vega-existing-credential-zk-v0"),
    IROHA_ZK_X509_STARK_P256_V0("iroha-zk-x509-stark-p256-v0"),
    IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0("iroha-jindo-polynomial-commitment-v0"),
    IROHA_BOOTLE_LANTERN_ANONCRED_V1("iroha-bootle-lantern-anoncred-v1"),
    ORCHARD_HALO2_ACTIONS_V1("orchard-halo2-actions-v1"),
    MONERO_FCMP_PLUS_PLUS_V1("monero-fcmp-plus-plus-v1"),
    IROHA_IVM_PRIVATE_NOTE_STARK_V1("iroha-ivm-private-note-stark-v1"),
    PQ_MASP_STARK_V0("pq-masp-stark-v0");

    private final String canonicalLabel;

    ProtocolIdV1(final String canonicalLabel) {
      this.canonicalLabel = canonicalLabel;
    }

    public String canonicalLabel() {
      return canonicalLabel;
    }

    /**
     * Parses one exact canonical label.
     *
     * @throws IllegalArgumentException for aliases, retired identifiers, case changes, whitespace,
     *     or unknown labels
     */
    public static ProtocolIdV1 fromCanonicalLabel(final String label) {
      if (label != null) {
        for (final ProtocolIdV1 value : values()) {
          if (value.canonicalLabel.equals(label)) {
            return value;
          }
        }
      }
      throw new IllegalArgumentException("unknown canonical privacy protocol id");
    }
  }

  private static final List<ProtocolIdV1> PROTOCOLS =
      Collections.unmodifiableList(Arrays.asList(ProtocolIdV1.values()));
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private PrivacyNativeBridge() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  /** Returns all twelve protocol identities in exact wire order. */
  public static List<ProtocolIdV1> protocolsV1() {
    return PROTOCOLS;
  }

  /**
   * Returns the authoritative {@code PrivacyCapabilitySnapshotV1} Norito archive.
   *
   * <p>The archive is rejected unless the shared Rust validator decodes the exact canonical typed
   * snapshot and validates all twelve rows. Consumers may use Torii JSON as its dynamic typed
   * projection; there is no legacy codec.
   */
  public static byte[] capabilitiesArchiveV1() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("native privacy capability bridge is unavailable");
    }
    final byte[] archive;
    try {
      archive = nativeCapabilities();
    } catch (final RuntimeException | LinkageError error) {
      throw new IllegalStateException("native privacy capability query failed", error);
    }
    return requireCapabilityArchive(archive);
  }

  static byte[] requireCapabilityArchive(final byte[] archive) {
    if (archive == null
        || archive.length == 0
        || archive.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalStateException("invalid privacy capability archive length");
    }
    final int status = nativeValidateCapabilities(archive);
    if (status != ValidationStatusV1.VALID.code()) {
      throw new IllegalStateException("invalid typed privacy capability archive");
    }
    return Arrays.copyOf(archive, archive.length);
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION
          && requireCapabilityArchive(nativeCapabilities()).length > 0;
    } catch (final RuntimeException | LinkageError error) {
      return false;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeCapabilities();

  private static native int nativeValidateCapabilities(byte[] archive);
}
