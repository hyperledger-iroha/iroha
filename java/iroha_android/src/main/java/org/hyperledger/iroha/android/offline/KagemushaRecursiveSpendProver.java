package org.hyperledger.iroha.android.offline;

/** Native recursive Kagemusha spend ABI-6 bridge. */
public final class KagemushaRecursiveSpendProver {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 6;
  public static final int RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7;
  public static final String RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-aggregation-v1";
  public static final String RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
      "kagemusha-recursive-compact-v1";
  public static final String RECURSIVE_AGGREGATION_PROOF_BACKEND = "halo2/ipa";
  public static final String RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-spend-lineage-v1";
  public static final String RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-spend-lineage-onehop-v1";
  public static final String RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 =
      "kagemusha-recursive-spend-lineage-append-v1";
  public static final int COMPACT_TOKEN_MAX_HOPS = 64;
  public static final int RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64;
  public static final boolean RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = true;
  public static final int RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1;
  public static final int RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024;
  public static final int RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128;
  public static final int NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
  public static final String RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN =
      "iroha:kagemusha:v1:recursive-spend-transition-profile";
  public static final String RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN =
      "iroha:kagemusha:v1:recursive-spend-transition-profile-digest";
  public static final String RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN =
      "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest";
  public static final String RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1 =
      "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
  public static final String RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1 =
      "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
  public static final String
      RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1 =
          "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
  public static final String
      RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1 =
          "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final byte[] MALFORMED_NATIVE_PROBE_ARCHIVE = new byte[] {0x00};
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  public enum Mode {
    RECURSIVE_COMPACT_V1("recursive_compact_v1"),
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
    return preferredMode(
        KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable(), NATIVE_AVAILABLE);
  }

  public static Mode preferredMode(final boolean recursiveSpendAvailable) {
    return preferredMode(false, recursiveSpendAvailable);
  }

  public static Mode preferredMode(
      final boolean recursiveCompactAvailable, final boolean recursiveSpendAvailable) {
    // Kept for source compatibility; compact mode is not a production default yet.
    return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;
  }

  public static boolean canRedeemWitnessless(final String circuitId, final int hopCount) {
    return RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        && isLineageProofCircuitId(circuitId)
        && hopCount >= 1
        && hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1;
  }

  public static boolean isLineageProofCircuitId(final String circuitId) {
    return RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.equals(circuitId)
        || RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.equals(circuitId)
        || RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(circuitId);
  }

  public static boolean isLineageAppendOutputCircuitId(final String outputCircuitId) {
    return RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.equals(outputCircuitId)
        || RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(outputCircuitId);
  }

  public static boolean isSupportedLineageKeyArtifactOpeningLen(final int verifierOpeningLen) {
    switch (verifierOpeningLen) {
      case 2:
      case 4:
      case 8:
      case 16:
      case 32:
      case 64:
      case 128:
        return true;
      default:
        return false;
    }
  }

  public static LineageKeyArtifacts lineageKeyArtifactsForInit(
      final int verifierOpeningLen,
      final String lineageVerifierKeyBackend,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive) {
    return lineageKeyArtifacts(
        RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        verifierOpeningLen,
        lineageVerifierKeyBackend,
        lineageVerifierKey,
        lineageProvingKeyArchive);
  }

  public static LineageKeyArtifacts lineageKeyArtifactsForAppend(
      final int verifierOpeningLen,
      final String lineageVerifierKeyBackend,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive) {
    return lineageKeyArtifacts(
        RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        verifierOpeningLen,
        lineageVerifierKeyBackend,
        lineageVerifierKey,
        lineageProvingKeyArchive);
  }

  public static LineageKeyArtifacts validateLineageKeyArtifacts(
      final LineageKeyArtifacts artifacts) {
    if (artifacts == null) {
      throw new IllegalArgumentException("lineage_key_artifacts");
    }
    if (!RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.equals(artifacts.proofCircuitId)
        && !RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
            artifacts.proofCircuitId)) {
      throw new IllegalArgumentException("proof_circuit_id");
    }
    if (!isSupportedLineageKeyArtifactOpeningLen(artifacts.verifierOpeningLen)) {
      throw new IllegalArgumentException("verifier_opening_len");
    }
    final byte[] lineageVerifierKey = artifacts.lineageVerifierKey();
    final byte[] lineageProvingKeyArchive = artifacts.lineageProvingKeyArchive();
    if (!RECURSIVE_AGGREGATION_PROOF_BACKEND.equals(artifacts.lineageVerifierKeyBackend)
        || lineageVerifierKey == null
        || lineageVerifierKey.length == 0) {
      throw new IllegalArgumentException("lineage_verifier_key");
    }
    if (lineageProvingKeyArchive == null || lineageProvingKeyArchive.length == 0) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    return artifacts;
  }

  public static LineageKeyArtifacts lineageKeyArtifacts(
      final String proofCircuitId,
      final int verifierOpeningLen,
      final String lineageVerifierKeyBackend,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive) {
    return validateLineageKeyArtifacts(
        new LineageKeyArtifacts(
            proofCircuitId,
            verifierOpeningLen,
            lineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive));
  }

  public static boolean requiresLineageKeyArtifactsForInit() {
    return true;
  }

  public static boolean requiresLineageWitnessForRedeem(
      final String circuitId, final int hopCount) {
    return !canRedeemWitnessless(circuitId, hopCount);
  }

  public static boolean canAppendWitnesslessLineage(final int previousHopCount) {
    return RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        && previousHopCount >= 1
        && previousHopCount < RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1;
  }

  public static String normalizeAppendOutputCircuitId(final String outputCircuitId) {
    if (outputCircuitId == null || outputCircuitId.isEmpty()) {
      return RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1;
    }
    if (RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.equals(outputCircuitId)) {
      return RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1;
    }
    return outputCircuitId;
  }

  public static boolean isSupportedAppendOutputCircuitId(final String outputCircuitId) {
    final String normalized = normalizeAppendOutputCircuitId(outputCircuitId);
    return RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalized)
        || RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(normalized);
  }

  public static boolean requiresLineageKeyArtifactsForAppendOutput(final String outputCircuitId) {
    return isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId));
  }

  public static boolean isSupportedPreviousProofCircuitId(final String previousProofCircuitId) {
    return RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(previousProofCircuitId)
        || isLineageProofCircuitId(previousProofCircuitId);
  }

  public static boolean requiresPreviousLineageVerifierRecordForAppend(
      final String previousProofCircuitId) {
    return isLineageProofCircuitId(previousProofCircuitId);
  }

  public static boolean isSupportedAppendProofTransition(
      final String previousProofCircuitId, final String outputCircuitId) {
    final String normalizedOutput = normalizeAppendOutputCircuitId(outputCircuitId);
    return (RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(previousProofCircuitId)
            && RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalizedOutput))
        || (isLineageProofCircuitId(previousProofCircuitId)
            && (RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalizedOutput)
                || RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(normalizedOutput)));
  }

  public static String preferredAppendOutputCircuitId(final int previousHopCount) {
    if (canAppendWitnesslessLineage(previousHopCount)) {
      return RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1;
    }
    return RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1;
  }

  public static boolean canProveAppendOutputCircuitId(
      final String outputCircuitId, final int previousHopCount) {
    if (previousHopCount < 1) {
      return false;
    }
    final String normalized = normalizeAppendOutputCircuitId(outputCircuitId);
    if (RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalized)) {
      return previousHopCount < COMPACT_TOKEN_MAX_HOPS;
    }
    if (RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(normalized)) {
      return canAppendWitnesslessLineage(previousHopCount);
    }
    return false;
  }

  public static boolean canSelectAppendOutputCircuitId(
      final String previousProofCircuitId,
      final String outputCircuitId,
      final int previousHopCount) {
    if (!canProveAppendOutputCircuitId(outputCircuitId, previousHopCount)) {
      return false;
    }
    if (!isSupportedPreviousProofCircuitId(previousProofCircuitId)) {
      return false;
    }
    return isSupportedAppendProofTransition(previousProofCircuitId, outputCircuitId);
  }

  public static boolean requiresPreviousProofOpenEnvelopesForAppend(
      final String outputCircuitId, final int previousHopCount) {
    return isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId))
        && previousHopCount >= 1;
  }

  public static byte[] initSpend(final byte[] requestArchive) {
    return call("init", requestArchive, KagemushaRecursiveSpendProver::nativeInitSpend);
  }

  public static byte[] appendSpend(final byte[] requestArchive) {
    return call("append", requestArchive, KagemushaRecursiveSpendProver::nativeAppendSpend);
  }

  public static byte[] transitionProfileInit(final byte[] requestArchive) {
    return call(
        "transition profile init",
        requestArchive,
        KagemushaRecursiveSpendProver::nativeTransitionProfileInit);
  }

  public static byte[] transitionProfileAppend(final byte[] requestArchive) {
    return call(
        "transition profile append",
        requestArchive,
        KagemushaRecursiveSpendProver::nativeTransitionProfileAppend);
  }

  public static byte[] lineageAppendBoundary(final byte[] profileArchive) {
    return callArchive(
        "lineage append boundary",
        "profileArchive",
        profileArchive,
        KagemushaRecursiveSpendProver::nativeLineageAppendBoundary);
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
    return callArchive(label, "requestArchive", requestArchive, call);
  }

  private static byte[] callArchive(
      final String label, final String archiveName, final byte[] archive, final NativeCall call) {
    requireNativeInput(archive, archiveName);
    requireNative();
    final byte[] output = call.run(archive);
    return requireRecursiveSpendOutput(output, label);
  }

  private static byte[] call(
      final String label,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativePairCall call) {
    requireNativeInput(requestArchive, "requestArchive");
    requireNativeInput(bundleArchive, "bundleArchive");
    requireNative();
    final byte[] output = call.run(requestArchive, bundleArchive);
    return requireRecursiveSpendOutput(output, label);
  }

  private static byte[] call(
      final String label,
      final byte[] previousWitnessArchive,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativeTripleCall call) {
    requireNativeInput(previousWitnessArchive, "previousWitnessArchive");
    requireNativeInput(requestArchive, "requestArchive");
    requireNativeInput(bundleArchive, "bundleArchive");
    requireNative();
    final byte[] output = call.run(previousWitnessArchive, requestArchive, bundleArchive);
    return requireRecursiveSpendOutput(output, label);
  }

  static byte[] requireRecursiveSpendOutput(final byte[] output, final String label) {
    return requireNativeOutput(output, "native " + label);
  }

  private static void requireNativeInput(final byte[] archive, final String archiveName) {
    if (archive == null || archive.length == 0) {
      throw new IllegalArgumentException(archiveName + " must not be empty");
    }
    if (!KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
      throw new IllegalArgumentException(archiveName + " must be a valid Norito archive");
    }
    if (!KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
      throw new IllegalArgumentException(
          archiveName + " must contain a non-empty Norito payload");
    }
  }

  private static byte[] requireNativeOutput(final byte[] output, final String label) {
    if (output == null) {
      throw new IllegalStateException(label + " returned no output");
    }
    if (output.length == 0) {
      throw new IllegalStateException(label + " returned empty output");
    }
    if (output.length > NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalStateException(label + " returned oversized output");
    }
    if (!KagemushaCompactPaymentTokenProver.isValidNoritoArchive(output)) {
      throw new IllegalStateException(label + " returned invalid Norito archive");
    }
    if (!KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(output)) {
      throw new IllegalStateException(label + " returned empty Norito payload");
    }
    return output;
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

  private static boolean probeRequiredNativeSymbols() {
    final byte[] probe = MALFORMED_NATIVE_PROBE_ARCHIVE;
    boolean available = true;
    available &= expectIllegalArgumentProbe(() -> nativeInitSpend(probe));
    available &= expectIllegalArgumentProbe(() -> nativeAppendSpend(probe));
    available &= expectIllegalArgumentProbe(() -> nativeTransitionProfileInit(new byte[0]));
    available &= expectIllegalArgumentProbe(() -> nativeTransitionProfileAppend(new byte[0]));
    available &= expectIllegalArgumentProbe(() -> nativeLineageAppendBoundary(new byte[0]));
    available &= expectIllegalArgumentProbe(() -> nativeVerifySpend(probe));
    available &=
        expectIllegalArgumentProbe(
        () -> nativeLineageWitnessFromInitResult(probe, probe));
    available &=
        expectIllegalArgumentProbe(
        () -> nativeLineageWitnessAppendResult(probe, probe, probe));
    available &= expectIllegalArgumentProbe(() -> nativeRedeemSpend(probe));
    return available;
  }

  static boolean expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    }
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe bridgeAbiVersion,
      final NativeSymbolProbe probeSymbol) {
    return detectNativeAvailability(
        loadLibrary, bridgeAbiVersion, probeSymbol, REQUIRED_BRIDGE_ABI_VERSION);
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe bridgeAbiVersion,
      final NativeSymbolProbe probeSymbol,
      final int requiredBridgeAbiVersion) {
    try {
      loadLibrary.run();
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    } catch (final RuntimeException error) {
      return false;
    }
    final int abiVersion;
    try {
      abiVersion = bridgeAbiVersion.run();
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    } catch (final RuntimeException error) {
      return false;
    }
    if (abiVersion < requiredBridgeAbiVersion) {
      return false;
    }
    try {
      return probeSymbol.run();
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    } catch (final RuntimeException error) {
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

  interface NativeSymbolProbe {
    boolean run();
  }

  interface NativeAbiVersionProbe {
    int run();
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeInitSpend(byte[] requestArchive);

  private static native byte[] nativeAppendSpend(byte[] requestArchive);

  private static native byte[] nativeTransitionProfileInit(byte[] requestArchive);

  private static native byte[] nativeTransitionProfileAppend(byte[] requestArchive);

  private static native byte[] nativeLineageAppendBoundary(byte[] profileArchive);

  private static native byte[] nativeLineageWitnessFromInitResult(
      byte[] requestArchive, byte[] bundleArchive);

  private static native byte[] nativeLineageWitnessAppendResult(
      byte[] previousWitnessArchive, byte[] requestArchive, byte[] bundleArchive);

  private static native byte[] nativeVerifySpend(byte[] requestArchive);

  private static native byte[] nativeRedeemSpend(byte[] requestArchive);

  /** Portable Reserved-lineage verifier/proving key artifact package. */
  public static final class LineageKeyArtifacts {
    public final String proofCircuitId;
    public final int verifierOpeningLen;
    public final String lineageVerifierKeyBackend;
    private final byte[] lineageVerifierKey;
    private final byte[] lineageProvingKeyArchive;

    private LineageKeyArtifacts(
        final String proofCircuitId,
        final int verifierOpeningLen,
        final String lineageVerifierKeyBackend,
        final byte[] lineageVerifierKey,
        final byte[] lineageProvingKeyArchive) {
      this.proofCircuitId = proofCircuitId;
      this.verifierOpeningLen = verifierOpeningLen;
      this.lineageVerifierKeyBackend = lineageVerifierKeyBackend;
      this.lineageVerifierKey = lineageVerifierKey == null ? null : lineageVerifierKey.clone();
      this.lineageProvingKeyArchive =
          lineageProvingKeyArchive == null ? null : lineageProvingKeyArchive.clone();
    }

    public boolean isInitArtifact() {
      return RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.equals(proofCircuitId);
    }

    public boolean isAppendArtifact() {
      return RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(proofCircuitId);
    }

    public byte[] lineageVerifierKey() {
      return lineageVerifierKey == null ? null : lineageVerifierKey.clone();
    }

    public byte[] lineageProvingKeyArchive() {
      return lineageProvingKeyArchive == null ? null : lineageProvingKeyArchive.clone();
    }
  }
}
