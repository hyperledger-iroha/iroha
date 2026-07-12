package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * ABI-18 Kagemusha V3 artifact streaming and capability bridge.
 *
 * <p>The first-release Android SDK deliberately exposes no spend protocol. The supported wallet
 * lifecycle is implemented by the Swift SDK; this JVM surface only prepares and atomically installs
 * the opaque six-file proof artifact set used by that lifecycle.
 */
public final class KagemushaRecursiveSpendProver {
  public static final int REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 18;
  public static final String PRODUCT_MODE = "recursive_spend_v1";
  public static final String ARTIFACT_MANIFEST_MODE = "recursive_spend_v2";
  public static final String ARTIFACT_MANIFEST_SCHEMA =
      "kagemusha.offline.recursive_spend.artifact_manifest.v3";
  public static final List<String> ARTIFACT_FILES =
      List.of(
          "transition-eq.parameters.krv3",
          "transition-eq.proving-key.krv3",
          "transition-eq.verifying-key.krv3",
          "state-ep.parameters.krv3",
          "state-ep.proving-key.krv3",
          "state-ep.verifying-key.krv3");
  public static final int ARTIFACT_COUNT = 6;
  public static final int MAX_MANIFEST_BYTES = 1024 * 1024;

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean ARTIFACT_BRIDGE_AVAILABLE = loadArtifactBridge();
  private static final boolean PROOF_BACKEND_AVAILABLE = loadProofBackendCapability();

  private KagemushaRecursiveSpendProver() {}

  public static boolean isArtifactStreamingAvailable() {
    return ARTIFACT_BRIDGE_AVAILABLE;
  }

  public static boolean isProofBackendAvailable() {
    return PROOF_BACKEND_AVAILABLE;
  }

  public static ArtifactIngest beginArtifactIngest(
      final byte[] manifestNorito,
      final byte[] manifestSha256,
      final byte[] expectedArtifactSha256) {
    requireArtifactBridge();
    final byte[] manifest = requireManifest(manifestNorito);
    final byte[] manifestDigest = requireDigest(manifestSha256, "manifestSha256");
    final byte[] artifactDigest =
        requireDigest(expectedArtifactSha256, "expectedArtifactSha256");
    final long handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest);
    if (handle <= 0) {
      throw new IllegalStateException("native Kagemusha artifact ingest returned no handle");
    }
    return new ArtifactIngest(handle);
  }

  public static ArtifactInstallSession beginArtifactInstallSession(
      final byte[] manifestNorito, final byte[] manifestSha256) {
    requireArtifactBridge();
    return new ArtifactInstallSession(
        requireManifest(manifestNorito), requireDigest(manifestSha256, "manifestSha256"));
  }

  static boolean isExactBridgeAbi(final int abiVersion) {
    return abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
  }

  static boolean detectExactNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe abiVersion,
      final NativeSymbolProbe symbolProbe) {
    Objects.requireNonNull(loadLibrary, "loadLibrary");
    Objects.requireNonNull(abiVersion, "abiVersion");
    Objects.requireNonNull(symbolProbe, "symbolProbe");
    try {
      loadLibrary.run();
      return isExactBridgeAbi(abiVersion.run()) && symbolProbe.run();
    } catch (final UnsatisfiedLinkError | RuntimeException error) {
      return false;
    }
  }

  private static boolean loadArtifactBridge() {
    return detectExactNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveSpendProver::nativeBridgeAbiVersion,
        () ->
            expectIllegalArgumentProbe(
                () -> nativeArtifactBeginV3(new byte[] {0}, new byte[32], new byte[32])));
  }

  private static boolean loadProofBackendCapability() {
    return detectExactNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveSpendProver::nativeBridgeAbiVersion,
        KagemushaRecursiveSpendProver::nativePastaCycleV3BackendAvailable);
  }

  private static boolean expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    }
  }

  private static void requireArtifactBridge() {
    if (!ARTIFACT_BRIDGE_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME + " ABI " + REQUIRED_NATIVE_BRIDGE_ABI_VERSION
              + " artifact streaming is unavailable");
    }
  }

  private static byte[] requireManifest(final byte[] value) {
    if (value == null || value.length == 0 || value.length > MAX_MANIFEST_BYTES) {
      throw new IllegalArgumentException(
          "manifestNorito must contain 1.." + MAX_MANIFEST_BYTES + " bytes");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireDigest(final byte[] value, final String name) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(name + " must contain exactly 32 bytes");
    }
    int accumulator = 0;
    for (final byte octet : value) {
      accumulator |= octet;
    }
    if (accumulator == 0) {
      throw new IllegalArgumentException(name + " must be non-zero");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireChunk(final byte[] value) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException("chunk must not be empty");
    }
    return Arrays.copyOf(value, value.length);
  }

  /** Owns one native artifact spool until installation or cancellation. */
  public static final class ArtifactIngest implements AutoCloseable {
    private long handle;
    private boolean finalized;
    private boolean installClaimed;

    private ArtifactIngest(final long handle) {
      this.handle = handle;
    }

    public synchronized void write(final byte[] chunk) {
      requireOpen(false);
      nativeArtifactWriteV3(handle, requireChunk(chunk));
    }

    public synchronized void finish() {
      requireOpen(false);
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
        throw new IllegalStateException("artifact ingest is being installed");
      }
      nativeArtifactCancelV3(handle);
      handle = 0;
      finalized = false;
    }

    private synchronized long claimFinalizedHandle() {
      if (handle == 0 || !finalized || installClaimed) {
        throw new IllegalStateException("artifact ingest is not installable");
      }
      installClaimed = true;
      return handle;
    }

    private synchronized void releaseInstallClaim(final long expectedHandle) {
      if (handle == expectedHandle) {
        installClaimed = false;
      }
    }

    private synchronized void relinquishInstalledHandle(final long expectedHandle) {
      if (handle != expectedHandle || !finalized || !installClaimed) {
        throw new IllegalStateException("artifact install ownership mismatch");
      }
      handle = 0;
      finalized = false;
      installClaimed = false;
    }

    private void requireOpen(final boolean allowFinalized) {
      if (handle == 0) {
        throw new IllegalStateException("artifact ingest is closed");
      }
      if (finalized && !allowFinalized) {
        throw new IllegalStateException("artifact ingest is already finalized");
      }
      if (installClaimed) {
        throw new IllegalStateException("artifact ingest is being installed");
      }
    }
  }

  /** Coordinates one atomic six-artifact generation install. */
  public static final class ArtifactInstallSession implements AutoCloseable {
    private final byte[] manifestNorito;
    private final byte[] manifestSha256;
    private final Map<String, ArtifactIngest> artifacts = new LinkedHashMap<>();
    private boolean installed;
    private boolean closed;

    private ArtifactInstallSession(final byte[] manifestNorito, final byte[] manifestSha256) {
      this.manifestNorito = Arrays.copyOf(manifestNorito, manifestNorito.length);
      this.manifestSha256 = Arrays.copyOf(manifestSha256, manifestSha256.length);
    }

    public synchronized ArtifactIngest beginArtifact(final byte[] expectedArtifactSha256) {
      requirePending();
      if (artifacts.size() == ARTIFACT_COUNT) {
        throw new IllegalStateException("artifact set already has six streams");
      }
      final byte[] digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256");
      final String key = hex(digest);
      if (artifacts.containsKey(key)) {
        throw new IllegalArgumentException("expectedArtifactSha256 is duplicated");
      }
      final ArtifactIngest ingest =
          beginArtifactIngest(manifestNorito, manifestSha256, digest);
      artifacts.put(key, ingest);
      return ingest;
    }

    public synchronized void install() {
      requirePending();
      if (artifacts.size() != ARTIFACT_COUNT) {
        throw new IllegalStateException("artifact set must contain exactly six streams");
      }
      final ArtifactIngest[] ordered = artifacts.values().toArray(new ArtifactIngest[0]);
      final long[] handles = new long[ARTIFACT_COUNT];
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
      return !closed && nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256);
    }

    public synchronized void uninstall() {
      if (!installed || closed) {
        return;
      }
      nativeArtifactSetUninstallV3(manifestSha256);
      installed = false;
      closed = true;
    }

    @Override
    public synchronized void close() {
      if (closed || installed) {
        return;
      }
      RuntimeException firstFailure = null;
      for (final ArtifactIngest ingest : artifacts.values()) {
        try {
          ingest.close();
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

  private static String hex(final byte[] digest) {
    final StringBuilder value = new StringBuilder(64);
    for (final byte octet : digest) {
      value.append(Character.forDigit((octet >>> 4) & 0x0f, 16));
      value.append(Character.forDigit(octet & 0x0f, 16));
    }
    return value.toString();
  }

  interface NativeProbe {
    void run();
  }

  interface NativeAbiVersionProbe {
    int run();
  }

  interface NativeSymbolProbe {
    boolean run();
  }

  private static native int nativeBridgeAbiVersion();

  private static native boolean nativePastaCycleV3BackendAvailable();

  private static native long nativeArtifactBeginV3(
      byte[] manifestNorito, byte[] manifestSha256, byte[] expectedArtifactSha256);

  private static native void nativeArtifactWriteV3(long handle, byte[] chunk);

  private static native void nativeArtifactFinalizeV3(long handle);

  private static native void nativeArtifactCancelV3(long handle);

  private static native void nativeArtifactSetInstallV3(
      byte[] manifestNorito, byte[] manifestSha256, long[] artifactHandles);

  private static native boolean nativeArtifactSetIsInstalledV3(
      byte[] manifestNorito, byte[] manifestSha256);

  private static native void nativeArtifactSetUninstallV3(byte[] manifestSha256);
}
