package org.hyperledger.iroha.android.offline;

/** Native recursive Kagemusha spend init/append/verify/redeem bridge. */
public final class KagemushaRecursiveSpendProver {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 6;
  public static final String RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-aggregation-v1";
  public static final String RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-spend-lineage-v1";

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  public enum Mode {
    RECURSIVE_SPEND_V1("recursive_spend_v1"),
    CHECKED_PREFOLD_V1("checked_prefold_v1");

    private final String wireName;

    Mode(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }
  }

  private KagemushaRecursiveSpendProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static Mode preferredMode() {
    return preferredMode(NATIVE_AVAILABLE);
  }

  public static Mode preferredMode(final boolean recursiveSpendAvailable) {
    return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;
  }

  public static byte[] initSpend(final byte[] requestArchive) {
    return call("init", requestArchive, KagemushaRecursiveSpendProver::nativeInitSpend);
  }

  public static byte[] appendSpend(final byte[] requestArchive) {
    return call("append", requestArchive, KagemushaRecursiveSpendProver::nativeAppendSpend);
  }

  public static byte[] lineageWitnessFromInitResult(
      final byte[] requestArchive, final byte[] bundleArchive) {
    return call(
        "lineage witness from init result",
        requestArchive,
        bundleArchive,
        KagemushaRecursiveSpendProver::nativeLineageWitnessFromInitResult);
  }

  public static byte[] lineageWitnessAppendResult(
      final byte[] previousWitnessArchive,
      final byte[] requestArchive,
      final byte[] bundleArchive) {
    return call(
        "lineage witness append result",
        previousWitnessArchive,
        requestArchive,
        bundleArchive,
        KagemushaRecursiveSpendProver::nativeLineageWitnessAppendResult);
  }

  public static byte[] verifySpend(final byte[] requestArchive) {
    return call("verify", requestArchive, KagemushaRecursiveSpendProver::nativeVerifySpend);
  }

  public static byte[] redeemSpend(final byte[] requestArchive) {
    return call("redeem", requestArchive, KagemushaRecursiveSpendProver::nativeRedeemSpend);
  }

  private static byte[] call(final String label, final byte[] requestArchive, final NativeCall call) {
    if (requestArchive == null || requestArchive.length == 0) {
      throw new IllegalArgumentException("requestArchive must not be empty");
    }
    requireNative();
    final byte[] output = call.run(requestArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native " + label);
  }

  private static byte[] call(
      final String label,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativePairCall call) {
    if (requestArchive == null || requestArchive.length == 0) {
      throw new IllegalArgumentException("requestArchive must not be empty");
    }
    if (bundleArchive == null || bundleArchive.length == 0) {
      throw new IllegalArgumentException("bundleArchive must not be empty");
    }
    requireNative();
    final byte[] output = call.run(requestArchive, bundleArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native " + label);
  }

  private static byte[] call(
      final String label,
      final byte[] previousWitnessArchive,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativeTripleCall call) {
    if (previousWitnessArchive == null || previousWitnessArchive.length == 0) {
      throw new IllegalArgumentException("previousWitnessArchive must not be empty");
    }
    if (requestArchive == null || requestArchive.length == 0) {
      throw new IllegalArgumentException("requestArchive must not be empty");
    }
    if (bundleArchive == null || bundleArchive.length == 0) {
      throw new IllegalArgumentException("bundleArchive must not be empty");
    }
    requireNative();
    final byte[] output = call.run(previousWitnessArchive, requestArchive, bundleArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native " + label);
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveSpendProver::nativeBridgeAbiVersion,
        KagemushaRecursiveSpendProver::probeRequiredNativeSymbols);
  }

  private static void probeRequiredNativeSymbols() {
    expectIllegalArgumentProbe(() -> nativeVerifySpend(new byte[0]));
    expectIllegalArgumentProbe(
        () -> nativeLineageWitnessFromInitResult(new byte[0], new byte[] {0x01}));
    expectIllegalArgumentProbe(
        () -> nativeLineageWitnessAppendResult(new byte[0], new byte[] {0x01}, new byte[] {0x02}));
  }

  private static void expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe bridgeAbiVersion,
      final NativeProbe probeSymbol) {
    try {
      loadLibrary.run();
      if (bridgeAbiVersion.run() < REQUIRED_BRIDGE_ABI_VERSION) {
        return false;
      }
      probeSymbol.run();
      return true;
    } catch (final IllegalArgumentException error) {
      return true;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  private interface NativeCall {
    byte[] run(byte[] requestArchive);
  }

  private interface NativePairCall {
    byte[] run(byte[] requestArchive, byte[] bundleArchive);
  }

  private interface NativeTripleCall {
    byte[] run(byte[] previousWitnessArchive, byte[] requestArchive, byte[] bundleArchive);
  }

  interface NativeProbe {
    void run();
  }

  interface NativeAbiVersionProbe {
    int run();
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeInitSpend(byte[] requestArchive);

  private static native byte[] nativeAppendSpend(byte[] requestArchive);

  private static native byte[] nativeLineageWitnessFromInitResult(
      byte[] requestArchive, byte[] bundleArchive);

  private static native byte[] nativeLineageWitnessAppendResult(
      byte[] previousWitnessArchive, byte[] requestArchive, byte[] bundleArchive);

  private static native byte[] nativeVerifySpend(byte[] requestArchive);

  private static native byte[] nativeRedeemSpend(byte[] requestArchive);
}
