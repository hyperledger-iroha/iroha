package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;

/** Exact first-release ABI-18 Kagemusha recursive-spend bridge. */
public final class KagemushaRecursiveSpendProver {
  public static final int REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 18;
  public static final String ARTIFACT_MANIFEST_SCHEMA =
      "kagemusha.offline.recursive_spend.artifact_manifest.v3";
  public static final String MODE = "recursive_spend_v2";
  public static final String PROOF_BACKEND = "halo2/ipa-pasta-cycle-v1";
  public static final String TRANSCRIPT_PROFILE = "kagemusha-pasta-cycle-poseidon-v1";
  public static final String TRANSITION_CIRCUIT_ID =
      "kagemusha-recursive-spend-transition-eq-v1";
  public static final String STATE_CIRCUIT_ID = "kagemusha-recursive-spend-state-ep-v1";
  public static final int MAX_PROOF_BYTES = 4_096;
  public static final int MAX_MANIFEST_BYTES = 1024 * 1024;

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadExactNativeBridge();
  private static final boolean ARTIFACT_INGEST_AVAILABLE = probeArtifactIngest();
  private static final boolean PROOF_BACKEND_AVAILABLE = probeProofBackend();

  public enum Mode {
    RECURSIVE_SPEND("recursive_spend_v2");

    private final String wireName;

    Mode(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }
  }

  private KagemushaRecursiveSpendProver() {}

  /** True only when the loaded bridge ABI is exactly 18. */
  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  /** Exact ABI-18 streaming artifact-ingest surface. */
  public static boolean isArtifactIngestAvailable() {
    return ARTIFACT_INGEST_AVAILABLE;
  }

  /** Audited proof capability; false in the current bridge build. */
  public static boolean isProofBackendAvailable() {
    return PROOF_BACKEND_AVAILABLE;
  }

  public static Mode preferredMode() {
    return preferredMode(PROOF_BACKEND_AVAILABLE);
  }

  public static Mode preferredMode(final boolean proofBackendAvailable) {
    return proofBackendAvailable ? Mode.RECURSIVE_SPEND : null;
  }

  /** Begin one manifest-bound, bounded stream for a complete KRV3 package. */
  public static ArtifactIngest beginArtifactIngest(
      final byte[] manifestNorito,
      final byte[] manifestSha256,
      final byte[] artifactSha256) {
    final byte[] manifest = requireManifest(manifestNorito);
    final byte[] manifestDigest = requireDigest(manifestSha256, "manifestSha256");
    final byte[] artifactDigest = requireDigest(artifactSha256, "artifactSha256");
    requireArtifactBridge();
    final long handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest);
    if (handle <= 0) {
      throw new IllegalStateException("native Kagemusha artifact ingest returned no handle");
    }
    return new ArtifactIngest(handle);
  }

  /** Begin one all-or-nothing six-artifact installation. */
  public static ArtifactInstallSession beginArtifactInstallSession(
      final byte[] manifestNorito, final byte[] manifestSha256) {
    final byte[] manifest = requireManifest(manifestNorito);
    final byte[] manifestDigest = requireDigest(manifestSha256, "manifestSha256");
    requireArtifactBridge();
    return new ArtifactInstallSession(manifest, manifestDigest);
  }

  static boolean isExactBridgeAbi(final int abiVersion) {
    return abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
  }

  private static byte[] requireManifest(final byte[] value) {
    if (value == null || value.length == 0 || value.length > MAX_MANIFEST_BYTES) {
      throw new IllegalArgumentException(
          "manifestNorito must contain between 1 and " + MAX_MANIFEST_BYTES + " bytes");
    }
    return value;
  }

  private static byte[] requireDigest(final byte[] value, final String name) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(name + " must be exactly 32 bytes");
    }
    int nonzero = 0;
    for (final byte octet : value) {
      nonzero |= octet;
    }
    if (nonzero == 0) {
      throw new IllegalArgumentException(name + " must not be all zero");
    }
    return value;
  }

  private static void requireArtifactBridge() {
    if (!ARTIFACT_INGEST_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME + " exact ABI-18 artifact ingest is not available in this runtime");
    }
  }

  private static boolean loadExactNativeBridge() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return isExactBridgeAbi(nativeBridgeAbiVersion());
    } catch (final UnsatisfiedLinkError | RuntimeException unavailable) {
      return false;
    }
  }

  private static boolean probeArtifactIngest() {
    if (!NATIVE_AVAILABLE) {
      return false;
    }
    try {
      nativeArtifactBeginV3(new byte[] {0}, new byte[] {1}, new byte[] {1});
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    } catch (final UnsatisfiedLinkError | RuntimeException unavailable) {
      return false;
    }
  }

  private static boolean probeProofBackend() {
    if (!NATIVE_AVAILABLE) {
      return false;
    }
    try {
      return isExactBridgeAbi(nativeBridgeAbiVersion())
          && nativePastaCycleV3BackendAvailable();
    } catch (final UnsatisfiedLinkError | RuntimeException unavailable) {
      return false;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native boolean nativePastaCycleV3BackendAvailable();

  private static native long nativeArtifactBeginV3(
      byte[] manifestNorito, byte[] manifestSha256, byte[] artifactSha256);

  private static native void nativeArtifactWriteV3(long handle, byte[] chunk);

  private static native void nativeArtifactFinalizeV3(long handle);

  private static native void nativeArtifactCancelV3(long handle);

  private static native void nativeArtifactSetInstallV3(
      byte[] manifestNorito, byte[] manifestSha256, long[] handles);

  private static native boolean nativeArtifactSetIsInstalledV3(
      byte[] manifestNorito, byte[] manifestSha256);

  private static native void nativeArtifactSetUninstallV3(byte[] manifestSha256);

  /** Owns one native KRV3 spool until installation or close. */
  public static final class ArtifactIngest implements AutoCloseable {
    private long handle;
    private boolean finalized;
    private boolean installClaimed;

    private ArtifactIngest(final long handle) {
      this.handle = handle;
    }

    public synchronized void write(final byte[] chunk) {
      requireMutable();
      if (chunk == null || chunk.length == 0) {
        throw new IllegalArgumentException("chunk must not be empty");
      }
      nativeArtifactWriteV3(handle, chunk);
    }

    public synchronized void finish() {
      requireMutable();
      nativeArtifactFinalizeV3(handle);
      finalized = true;
    }

    public synchronized boolean isFinalized() {
      return finalized;
    }

    @Override
    public synchronized void close() {
      if (handle == 0) {
        return;
      }
      if (installClaimed) {
        throw new IllegalStateException("Kagemusha artifact ingest is being installed");
      }
      final long current = handle;
      nativeArtifactCancelV3(current);
      handle = 0;
      finalized = false;
    }

    private synchronized long claimFinalizedHandle() {
      if (handle == 0 || !finalized || installClaimed) {
        throw new IllegalStateException("Kagemusha artifact ingest is not installable");
      }
      installClaimed = true;
      return handle;
    }

    private synchronized void releaseInstallClaim(final long expectedHandle) {
      if (handle == expectedHandle && installClaimed) {
        installClaimed = false;
      }
    }

    private synchronized void relinquishInstalledHandle(final long expectedHandle) {
      if (handle != expectedHandle || !finalized || !installClaimed) {
        throw new IllegalStateException("Kagemusha artifact install ownership mismatch");
      }
      handle = 0;
      finalized = false;
      installClaimed = false;
    }

    private void requireMutable() {
      if (handle == 0) {
        throw new IllegalStateException("Kagemusha artifact ingest is closed");
      }
      if (finalized) {
        throw new IllegalStateException("Kagemusha artifact ingest is already finalized");
      }
      if (installClaimed) {
        throw new IllegalStateException("Kagemusha artifact ingest is being installed");
      }
    }
  }

  /** Coordinates one atomic six-artifact V3 generation install. */
  public static final class ArtifactInstallSession implements AutoCloseable {
    private final byte[] manifestNorito;
    private final byte[] manifestSha256;
    private final Map<String, ArtifactIngest> artifacts = new LinkedHashMap<>();
    private boolean installed;
    private boolean closed;

    private ArtifactInstallSession(final byte[] manifestNorito, final byte[] manifestSha256) {
      this.manifestNorito = manifestNorito.clone();
      this.manifestSha256 = manifestSha256.clone();
    }

    public synchronized ArtifactIngest beginArtifact(final byte[] expectedArtifactSha256) {
      requirePending();
      if (artifacts.size() >= 6) {
        throw new IllegalStateException("artifact set already has six streams");
      }
      final byte[] digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256");
      final String key = hexDigest(digest);
      if (artifacts.containsKey(key)) {
        throw new IllegalArgumentException("expectedArtifactSha256 is duplicated");
      }
      final ArtifactIngest artifact = beginArtifactIngest(manifestNorito, manifestSha256, digest);
      artifacts.put(key, artifact);
      return artifact;
    }

    /** Native failure consumes no handles and preserves the previous generation. */
    public synchronized void install() {
      requirePending();
      if (artifacts.size() != 6) {
        throw new IllegalStateException("artifact set must contain exactly six streams");
      }
      final ArtifactIngest[] ordered = artifacts.values().toArray(new ArtifactIngest[0]);
      final long[] handles = new long[6];
      int claimed = 0;
      try {
        for (; claimed < ordered.length; claimed++) {
          handles[claimed] = ordered[claimed].claimFinalizedHandle();
        }
        nativeArtifactSetInstallV3(manifestNorito, manifestSha256, handles);
      } catch (final RuntimeException | UnsatisfiedLinkError failure) {
        for (int index = 0; index < claimed; index++) {
          ordered[index].releaseInstallClaim(handles[index]);
        }
        throw failure;
      }
      for (int index = 0; index < ordered.length; index++) {
        ordered[index].relinquishInstalledHandle(handles[index]);
      }
      artifacts.clear();
      installed = true;
    }

    public synchronized boolean isInstalled() {
      if (closed && !installed) {
        return false;
      }
      return nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256);
    }

    /** A digest guard prevents a stale session from removing a newer generation. */
    public synchronized void uninstall() {
      if (!installed || closed) {
        return;
      }
      nativeArtifactSetUninstallV3(manifestSha256);
      installed = false;
      closed = true;
    }

    /** Cancels pending streams; an installed generation requires explicit uninstall. */
    @Override
    public synchronized void close() {
      if (closed || installed) {
        return;
      }
      RuntimeException firstFailure = null;
      for (final ArtifactIngest artifact : artifacts.values()) {
        try {
          artifact.close();
        } catch (final RuntimeException failure) {
          if (firstFailure == null) {
            firstFailure = failure;
          }
        }
      }
      artifacts.clear();
      closed = true;
      if (firstFailure != null) {
        throw firstFailure;
      }
    }

    private void requirePending() {
      if (closed || installed) {
        throw new IllegalStateException("artifact install session is not pending");
      }
    }
  }

  private static String hexDigest(final byte[] digest) {
    final StringBuilder value = new StringBuilder(64);
    for (final byte octet : digest) {
      value.append(Character.forDigit((octet >>> 4) & 0x0f, 16));
      value.append(Character.forDigit(octet & 0x0f, 16));
    }
    return value.toString();
  }
}
