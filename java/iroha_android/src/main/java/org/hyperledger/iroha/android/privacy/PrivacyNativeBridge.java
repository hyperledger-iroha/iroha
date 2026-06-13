package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.TypeAdapter;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Raw Norito V1 privacy proof bridge backed by {@code connect_norito_bridge}. */
public final class PrivacyNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 7;
  public static final int PRIVACY_FFI_VERSION_V1 = 1;
  public static final String PRODUCTION_GATE_VERSION = "privacy-production-gate-v1";
  public static final int STATUS_OK = 0;
  public static final int STATUS_ERROR = 1;
  public static final int ERROR_NULL_POINTER = 1;
  public static final int ERROR_MALFORMED_NORITO = 2;
  public static final int ERROR_UNSUPPORTED_ALGORITHM = 3;
  public static final int ERROR_PRODUCTION_DISABLED = 4;
  public static final int ERROR_INVALID_REQUEST = 5;
  public static final int ERROR_PROVING_FAILED = 6;
  public static final int PRIVACY_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;

  private static final int PRIVACY_NORITO_HEADER_BYTES = 40;
  private static final int PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES = 64;
  private static final int PRIVACY_NORITO_SUPPORTED_FLAGS_MASK = 0x27;
  private static final int PRIVACY_NORITO_FIELD_BITSET_FLAG = 0x20;
  private static final int PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06;
  private static final int PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES = 1024;
  private static final int PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES = 1024 * 1024;
  private static final int PRIVACY_REQUEST_WITNESS_MAX_BYTES = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2;
  private static final int PRIVACY_REQUEST_PROOF_MAX_BYTES = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2;
  private static final int PRIVACY_SCHEMA_REQUEST = 0x52;
  private static final int PRIVACY_SCHEMA_CAPABILITIES_RESULT = 0x50;
  private static final int PRIVACY_SCHEMA_BUILD_PROOF_RESULT = 0x42;
  private static final int PRIVACY_SCHEMA_VERIFY_PROOF_RESULT = 0x56;
  private static final long PRIVACY_CRC64_REFLECTED_POLY = 0xC96C5795D7870F42L;
  private static final String CONFIDENTIAL_TRANSFER_ALGORITHM_ID = "confidential-transfer-v2";
  private static final String UNSHIELD_ALGORITHM_ID = "unshield";
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final byte[] PRIVACY_NORITO_MAGIC = new byte[] {'N', 'R', 'T', '0'};
  private static final long[] PRIVACY_CRC64_TABLE = buildPrivacyCrc64Table();
  private static final byte[] PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE =
      buildPrivacyNativeAvailabilityProbeArchive();
  private static final List<String> PRODUCTION_GATE_MISSING =
      Collections.unmodifiableList(
          Arrays.asList(
              "real proving engine is not registered",
              "real verifier is not registered",
              "chain admission path is not enabled",
              "cross-SDK parity is incomplete",
              "wallet/state support is incomplete",
              "witness privacy checks are incomplete",
              "deterministic tests are incomplete",
              "negative/adversarial tests are incomplete",
              "replay/nullifier rejection tests are incomplete",
              "fuzzing gate is incomplete",
              "parser fuzzing gate is incomplete",
              "verifier fuzzing gate is incomplete",
              "performance gate is incomplete",
              "internal cryptographic review signoff is missing",
              "implementation stage is not production-hardened",
              "planned SDK entrypoints remain",
              "dev fixture entrypoints are not production entrypoints",
              "Iroha production allowlist is not enabled for this audited row"));
  private static final List<String> PRODUCTION_GATE_REQUIRED =
      Collections.unmodifiableList(
          Arrays.asList(
              "real_proving",
              "real_verification",
              "chain_admission",
              "sdk_parity",
              "wallet_state",
              "witness_privacy_checks",
              "deterministic_tests",
              "negative_adversarial_tests",
              "replay_nullifier_tests",
              "fuzzing",
              "parser_fuzzing",
              "verifier_fuzzing",
              "performance_gates",
              "external_audit"));
  private static final List<String> READY_AUDIT_REFERENCE_PREFIXES =
      Collections.unmodifiableList(
          Arrays.asList(
              "chain_id:",
              "reviewer:",
              "review_artifact_hash:",
              "review_artifact_signature:",
              "fuzz_artifact_hash:",
              "performance_artifact_hash:",
              "localnet_run_id:",
              "localnet_smoke_tx_hash:",
              "localnet_replay_rejection_hash:",
              "localnet_restart_replay_rejection_hash:",
              "localnet_state_recovery_hash:",
              "localnet_lifecycle_shield_tx_hash:",
              "localnet_lifecycle_hop_proof_hash:",
              "localnet_lifecycle_recursive_init_hash:",
              "localnet_lifecycle_recursive_init_verify_hash:",
              "localnet_lifecycle_recursive_append_hash:",
              "localnet_lifecycle_recursive_append_verify_hash:",
              "localnet_lifecycle_unshield_proof_hash:",
              "localnet_lifecycle_redeem_tx_hash:"));
  private static final Set<String> READY_HASH_REFERENCE_PREFIXES =
      readyHashReferencePrefixes();
  private static final List<String> PRODUCTION_GATE_AUDIT_REFERENCES = Collections.emptyList();
  private static final TypeAdapter<Long> NATIVE_U32 = NoritoAdapters.uint(32);
  private static final TypeAdapter<String> NATIVE_STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Boolean> NATIVE_BOOL = NoritoAdapters.boolAdapter();
  private static final TypeAdapter<NativeGateStatus> NATIVE_GATE_STATUS_ADAPTER =
      typedStruct(
          Arrays.asList(
              NoritoAdapters.field("key", NATIVE_STRING),
              NoritoAdapters.field("passed", NATIVE_BOOL)),
          fields -> new NativeGateStatus(
              stringField(fields, "key"),
              booleanField(fields, "passed")));
  private static final TypeAdapter<NativeProductionGate> NATIVE_PRODUCTION_GATE_ADAPTER =
      typedStruct(
          Arrays.asList(
              NoritoAdapters.field("version", NATIVE_STRING),
              NoritoAdapters.field("ready", NATIVE_BOOL),
              NoritoAdapters.field("gates", NoritoAdapters.sequence(NATIVE_GATE_STATUS_ADAPTER)),
              NoritoAdapters.field("required_gates", NoritoAdapters.sequence(NATIVE_STRING)),
              NoritoAdapters.field("missing", NoritoAdapters.sequence(NATIVE_STRING)),
              NoritoAdapters.field("audit_references", NoritoAdapters.sequence(NATIVE_STRING))),
          fields ->
              new NativeProductionGate(
                  stringField(fields, "version"),
                  booleanField(fields, "ready"),
                  listField(fields, "gates"),
                  listField(fields, "required_gates"),
                  listField(fields, "missing"),
                  listField(fields, "audit_references")));
  private static final TypeAdapter<NativeCapability> NATIVE_CAPABILITY_ADAPTER =
      typedStruct(
          Arrays.asList(
              NoritoAdapters.field("algorithm_id", NATIVE_STRING),
              NoritoAdapters.field("proof_family", NATIVE_STRING),
              NoritoAdapters.field("backend_family", NATIVE_STRING),
              NoritoAdapters.field("sdk_entrypoints", NoritoAdapters.sequence(NATIVE_STRING)),
              NoritoAdapters.field("planned_entrypoints", NoritoAdapters.sequence(NATIVE_STRING)),
              NoritoAdapters.field("production_ready", NATIVE_BOOL),
              NoritoAdapters.field("production_gate", NATIVE_PRODUCTION_GATE_ADAPTER)),
          fields ->
              new NativeCapability(
                  stringField(fields, "algorithm_id"),
                  stringField(fields, "proof_family"),
                  stringField(fields, "backend_family"),
                  listField(fields, "sdk_entrypoints"),
                  listField(fields, "planned_entrypoints"),
                  booleanField(fields, "production_ready"),
                  objectField(fields, "production_gate")));
  private static final TypeAdapter<NativeCapabilities> NATIVE_CAPABILITIES_ADAPTER =
      typedStruct(
          Arrays.asList(
              NoritoAdapters.field("version", NATIVE_U32),
              NoritoAdapters.field("gate_version", NATIVE_STRING),
              NoritoAdapters.field("algorithms", NoritoAdapters.sequence(NATIVE_CAPABILITY_ADAPTER))),
          fields ->
              new NativeCapabilities(
                  uintField(fields, "version"),
                  stringField(fields, "gate_version"),
                  listField(fields, "algorithms")));
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private PrivacyNativeBridge() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static PrivacyCapabilities privacyCapabilities() {
    return privacyCapabilities(NATIVE_AVAILABLE);
  }

  public static byte[] capabilitiesArchive() {
    requireNative();
    return requireNativeOutput(
        invokeNativeOutput("privacy capabilities", PrivacyNativeBridge::nativeCapabilities),
        "privacy capabilities",
        PRIVACY_SCHEMA_CAPABILITIES_RESULT);
  }

  public static byte[] privacyProofRequestV1(
      final String algorithmId,
      final String entrypoint,
      final String vkRef,
      final byte[] publicInputs) {
    return privacyProofRequestV1(
        algorithmId,
        entrypoint,
        vkRef,
        publicInputs,
        new byte[0],
        new byte[0]);
  }

  public static byte[] privacyProofRequestV1(
      final String algorithmId,
      final String entrypoint,
      final String vkRef,
      final byte[] publicInputs,
      final byte[] witness,
      final byte[] proof) {
    final byte[] algorithmIdBytes = privacyRequestTextBytes(algorithmId, "algorithmId");
    final byte[] entrypointBytes = privacyRequestTextBytes(entrypoint, "entrypoint");
    final byte[] vkRefBytes = privacyRequestTextBytes(vkRef, "vkRef");
    final byte[] publicInputBytes =
        privacyRequestComponentBytes(
            publicInputs, "publicInputs", PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES, false);
    final byte[] witnessBytes =
        privacyRequestComponentBytes(witness, "witness", PRIVACY_REQUEST_WITNESS_MAX_BYTES, true);
    final byte[] proofBytes =
        privacyRequestComponentBytes(proof, "proof", PRIVACY_REQUEST_PROOF_MAX_BYTES, true);
    try {
      requireNative();
      return requireNativeOutput(
          invokeNativeOutput(
              "privacy proof request",
              () ->
                  nativeProofRequest(
                      algorithmIdBytes,
                      entrypointBytes,
                      vkRefBytes,
                      publicInputBytes,
                      witnessBytes,
                      proofBytes)),
          "privacy proof request",
          PRIVACY_SCHEMA_REQUEST);
    } finally {
      Arrays.fill(algorithmIdBytes, (byte) 0);
      Arrays.fill(entrypointBytes, (byte) 0);
      Arrays.fill(vkRefBytes, (byte) 0);
      Arrays.fill(publicInputBytes, (byte) 0);
      Arrays.fill(witnessBytes, (byte) 0);
      Arrays.fill(proofBytes, (byte) 0);
    }
  }

  public static byte[] buildProof(final byte[] requestArchive) {
    return call("build proof", requestArchive, PrivacyNativeBridge::nativeBuildProof);
  }

  public static byte[] buildConfidentialTransferProofV2(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildConfidentialUnshieldProofV3(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildZkAceAuthorizationProofV1(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildJindoLatticeProofV0(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildSisHintsAnonymousCredentialProofV0(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildSilentThresholdCredentialShowingProofV0(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildVegaCredentialPredicateProofV0(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildZkAmsAdmissionBatchProofV0(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] buildZkAtPolicyProofV1(final byte[] requestArchive) {
    return buildProof(requestArchive);
  }

  public static byte[] verifyProof(final byte[] requestArchive) {
    return call("verify proof", requestArchive, PrivacyNativeBridge::nativeVerifyProof);
  }

  public static byte[] verifyJindoPolynomialCommitmentV0(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  public static byte[] verifySisHintsAnonymousCredentialProofV0(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  public static byte[] verifySilentThresholdCredentialShowingProofV0(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  public static byte[] verifyVegaCredentialPredicateProofV0(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  public static byte[] verifyZkAmsAdmissionBatchProofV0(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  public static byte[] verifyZkAtPolicyProofV1(final byte[] requestArchive) {
    return verifyProof(requestArchive);
  }

  static byte[] call(
      final String label, final byte[] requestArchive, final NativeCall call) {
    return call(label, requestArchive, call, NATIVE_AVAILABLE);
  }

  static byte[] call(
      final String label,
      final byte[] requestArchive,
      final NativeCall call,
      final boolean nativeAvailable) {
    if (requestArchive == null || requestArchive.length == 0) {
      throw new IllegalArgumentException("requestArchive must not be empty");
    }
    if (requestArchive.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalArgumentException(
          "requestArchive must not exceed " + PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + " bytes");
    }
    if (!isValidPrivacyNoritoArchive(requestArchive)) {
      throw new IllegalArgumentException("requestArchive must be a valid Norito V1 archive");
    }
    if (!hasPrivacyNoritoSchema(requestArchive, PRIVACY_SCHEMA_REQUEST)) {
      throw new IllegalArgumentException("requestArchive must use the privacy request schema");
    }
    if (!hasNonEmptyPrivacyNoritoPayload(requestArchive)) {
      throw new IllegalArgumentException(
          "requestArchive must contain a non-empty privacy request payload");
    }
    requireNative(nativeAvailable);
    final String outputLabel = "privacy " + label;
    final int expectedSchemaByte = expectedPrivacyResultSchema(outputLabel);
    if (expectedSchemaByte < 0) {
      throw new IllegalStateException(outputLabel + " is not a supported privacy native operation");
    }
    final byte[] request = Arrays.copyOf(requestArchive, requestArchive.length);
    try {
      return requireNativeOutput(
          invokeNativeOutput(outputLabel, () -> call.run(request)),
          outputLabel,
          expectedSchemaByte);
    } finally {
      Arrays.fill(request, (byte) 0);
    }
  }

  static byte[] invokeNativeOutput(final String label, final NativeByteArrayProbe probe) {
    try {
      return probe.run();
    } catch (final RuntimeException error) {
      throw new IllegalStateException(label + " failed");
    } catch (final LinkageError error) {
      throw new IllegalStateException(label + " failed");
    }
  }

  static byte[] requireNativeOutput(final byte[] output, final String label) {
    final int expectedSchemaByte = expectedPrivacyResultSchema(label);
    if (expectedSchemaByte < 0) {
      throw new IllegalStateException(label + " is not a supported privacy native operation");
    }
    return requireNativeOutput(output, label, expectedSchemaByte);
  }

  static byte[] requireNativeOutput(
      final byte[] output, final String label, final int expectedSchemaByte) {
    if (expectedSchemaByte < 0) {
      throw new IllegalStateException(label + " is not a supported privacy native operation");
    }
    if (output == null) {
      throw new IllegalStateException(label + " returned no output");
    }
    try {
      if (output.length == 0) {
        throw new IllegalStateException(label + " returned empty output");
      }
      if (output.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
        throw new IllegalStateException(label + " returned oversized output");
      }
      if (!isValidPrivacyNoritoArchive(output)) {
        throw new IllegalStateException(label + " returned invalid Norito V1 archive");
      }
      if (!hasNonEmptyPrivacyNoritoPayload(output)) {
        throw new IllegalStateException(label + " returned empty privacy result payload");
      }
      if (!hasPrivacyNoritoSchema(output, expectedSchemaByte)) {
        throw new IllegalStateException(label + " returned unexpected privacy result schema");
      }
      return Arrays.copyOf(output, output.length);
    } finally {
      Arrays.fill(output, (byte) 0);
    }
  }

  private static void requireNative() {
    requireNative(NATIVE_AVAILABLE);
  }

  private static void requireNative(final boolean nativeAvailable) {
    if (!nativeAvailable) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        PrivacyNativeBridge::nativeBridgeAbiVersion,
        PrivacyNativeBridge::probeRequiredNativeSymbols);
  }

  private static boolean probeRequiredNativeSymbols() {
    boolean available = true;
    available &= returnsOutputProbe(
        PRIVACY_SCHEMA_CAPABILITIES_RESULT,
        PrivacyNativeBridge::nativeCapabilities);
    available &= proofRequestOutputProbe();
    available &= returnsOutputProbe(
        PRIVACY_SCHEMA_BUILD_PROOF_RESULT,
        () -> nativeBuildProof(privacyNativeAvailabilityProbeArchive()));
    available &= returnsOutputProbe(
        PRIVACY_SCHEMA_VERIFY_PROOF_RESULT,
        () -> nativeVerifyProof(privacyNativeAvailabilityProbeArchive()));
    return available;
  }

  private static boolean proofRequestOutputProbe() {
    final byte[] algorithmId = "zk-ace-pq-authorization-v0".getBytes(StandardCharsets.UTF_8);
    final byte[] entrypoint = "buildZkAceAuthorizationProofV1".getBytes(StandardCharsets.UTF_8);
    final byte[] vkRef =
        "stark-fri:zk_ace_pq_authorization_v0".getBytes(StandardCharsets.UTF_8);
    final byte[] publicInputs = "public-inputs".getBytes(StandardCharsets.UTF_8);
    try {
      return returnsOutputProbe(
          PRIVACY_SCHEMA_REQUEST,
          () -> nativeProofRequest(
              algorithmId,
              entrypoint,
              vkRef,
              publicInputs,
              new byte[0],
              new byte[0]));
    } finally {
      Arrays.fill(algorithmId, (byte) 0);
      Arrays.fill(entrypoint, (byte) 0);
      Arrays.fill(vkRef, (byte) 0);
      Arrays.fill(publicInputs, (byte) 0);
    }
  }

  static byte[] privacyNativeAvailabilityProbeArchive() {
    return Arrays.copyOf(
        PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE,
        PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE.length);
  }

  static boolean returnsOutputProbe(
      final int expectedSchemaByte, final NativeByteArrayProbe probe) {
    try {
      final byte[] output = probe.run();
      if (output == null) {
        return false;
      }
      try {
        return output.length > 0
            && output.length <= PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
            && isValidPrivacyNoritoArchive(output)
            && hasNonEmptyPrivacyNoritoPayload(output)
            && hasPrivacyNoritoSchema(output, expectedSchemaByte);
      } finally {
        Arrays.fill(output, (byte) 0);
      }
    } catch (final RuntimeException error) {
      return false;
    } catch (final LinkageError error) {
      return false;
    }
  }

  static boolean isValidPrivacyNoritoArchive(final byte[] output) {
    if (output == null
        || output.length < PRIVACY_NORITO_HEADER_BYTES
        || output.length > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
      return false;
    }
    for (int index = 0; index < PRIVACY_NORITO_MAGIC.length; index++) {
      if (output[index] != PRIVACY_NORITO_MAGIC[index]) {
        return false;
      }
    }
    if (output[4] != 0 || output[5] != 0 || output[22] != 0) {
      return false;
    }
    final int flags = output[39] & 0xFF;
    if ((flags & ~PRIVACY_NORITO_SUPPORTED_FLAGS_MASK) != 0) {
      return false;
    }
    if ((flags & PRIVACY_NORITO_FIELD_BITSET_FLAG) != 0
        && (flags & PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS)
            != PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS) {
      return false;
    }
    final long payloadLengthLong = readLongLittleEndian(output, 23);
    if (payloadLengthLong < 0
        || payloadLengthLong > Integer.MAX_VALUE - PRIVACY_NORITO_HEADER_BYTES) {
      return false;
    }
    final int payloadLength = (int) payloadLengthLong;
    final int minimumLength = PRIVACY_NORITO_HEADER_BYTES + payloadLength;
    if (output.length < minimumLength) {
      return false;
    }
    final int paddingLength = output.length - minimumLength;
    if (paddingLength > PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES) {
      return false;
    }
    for (int index = PRIVACY_NORITO_HEADER_BYTES;
        index < PRIVACY_NORITO_HEADER_BYTES + paddingLength;
        index++) {
      if (output[index] != 0) {
        return false;
      }
    }
    final int payloadOffset = PRIVACY_NORITO_HEADER_BYTES + paddingLength;
    final long expectedCrc = readLongLittleEndian(output, 31);
    return privacyCrc64(output, payloadOffset, output.length - payloadOffset) == expectedCrc;
  }

  static boolean hasNonEmptyPrivacyNoritoPayload(final byte[] output) {
    return isValidPrivacyNoritoArchive(output) && readLongLittleEndian(output, 23) > 0;
  }

  private static int expectedPrivacyResultSchema(final String label) {
    if ("privacy capabilities".equals(label)) {
      return PRIVACY_SCHEMA_CAPABILITIES_RESULT;
    }
    if ("privacy proof request".equals(label)) {
      return PRIVACY_SCHEMA_REQUEST;
    }
    if ("privacy build proof".equals(label)) {
      return PRIVACY_SCHEMA_BUILD_PROOF_RESULT;
    }
    if ("privacy verify proof".equals(label)) {
      return PRIVACY_SCHEMA_VERIFY_PROOF_RESULT;
    }
    return -1;
  }

  private static byte[] privacyRequestTextBytes(final String value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must not be null");
    }
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    if (bytes.length > PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES) {
      throw new IllegalArgumentException(
          name + " must not exceed " + PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES + " bytes");
    }
    return bytes;
  }

  private static byte[] privacyRequestComponentBytes(
      final byte[] value, final String name, final int maxBytes, final boolean allowEmpty) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must not be null");
    }
    if (!allowEmpty && value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
    if (value.length > maxBytes) {
      throw new IllegalArgumentException(name + " must not exceed " + maxBytes + " bytes");
    }
    return Arrays.copyOf(value, value.length);
  }

  static boolean hasPrivacyNoritoSchema(
      final byte[] output, final int expectedSchemaByte) {
    if (expectedSchemaByte < 0) {
      return false;
    }
    final byte expected = (byte) expectedSchemaByte;
    for (int index = 6; index < 22; index++) {
      if (output[index] != expected) {
        return false;
      }
    }
    return true;
  }

  private static long[] buildPrivacyCrc64Table() {
    final long[] table = new long[256];
    for (int index = 0; index < table.length; index++) {
      long crc = index;
      for (int bit = 0; bit < 8; bit++) {
        crc =
            (crc & 1L) != 0L
                ? (crc >>> 1) ^ PRIVACY_CRC64_REFLECTED_POLY
                : crc >>> 1;
      }
      table[index] = crc;
    }
    return table;
  }

  private static byte[] buildPrivacyNativeAvailabilityProbeArchive() {
    final byte[] archive = new byte[PRIVACY_NORITO_HEADER_BYTES];
    System.arraycopy(PRIVACY_NORITO_MAGIC, 0, archive, 0, PRIVACY_NORITO_MAGIC.length);
    Arrays.fill(archive, 6, 22, (byte) PRIVACY_SCHEMA_REQUEST);
    return archive;
  }

  private static long privacyCrc64(final byte[] output, final int offset, final int length) {
    long crc = -1L;
    for (int index = offset; index < offset + length; index++) {
      crc = PRIVACY_CRC64_TABLE[((int) crc ^ output[index]) & 0xFF] ^ (crc >>> 8);
    }
    return crc ^ -1L;
  }

  private static long readLongLittleEndian(final byte[] output, final int offset) {
    long value = 0L;
    for (int index = 0; index < 8; index++) {
      value |= ((long) output[offset + index] & 0xFFL) << (8 * index);
    }
    return value;
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe bridgeAbiVersion,
      final NativeSymbolProbe probeSymbol) {
    try {
      loadLibrary.run();
    } catch (final RuntimeException error) {
      return false;
    } catch (final LinkageError error) {
      return false;
    }
    final int abiVersion;
    try {
      abiVersion = bridgeAbiVersion.run();
    } catch (final RuntimeException error) {
      return false;
    } catch (final LinkageError error) {
      return false;
    }
    if (abiVersion < REQUIRED_BRIDGE_ABI_VERSION) {
      return false;
    }
    try {
      return probeSymbol.run();
    } catch (final RuntimeException error) {
      return false;
    } catch (final LinkageError error) {
      return false;
    }
  }

  interface NativeCall {
    byte[] run(byte[] requestArchive);
  }

  interface NativeProbe {
    void run();
  }

  interface NativeByteArrayProbe {
    byte[] run();
  }

  interface NativeSymbolProbe {
    boolean run();
  }

  interface NativeAbiVersionProbe {
    int run();
  }

  static PrivacyCapabilities privacyCapabilities(final boolean bridgeAvailable) {
    if (!bridgeAvailable || !NATIVE_AVAILABLE) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    }
    try {
      final byte[] archive =
          requireNativeOutput(
              invokeNativeOutput("privacy capabilities", PrivacyNativeBridge::nativeCapabilities),
              "privacy capabilities",
              PRIVACY_SCHEMA_CAPABILITIES_RESULT);
      return privacyCapabilitiesFromArchive(archive, bridgeAvailable);
    } catch (final RuntimeException error) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    } catch (final LinkageError error) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    }
  }

  static PrivacyCapabilities privacyCapabilitiesFromArchive(
      final byte[] archive, final boolean bridgeAvailable) {
    try {
      return privacyCapabilitiesFromNative(
          NoritoCodec.decode(archive, NATIVE_CAPABILITIES_ADAPTER, null), bridgeAvailable);
    } catch (final RuntimeException error) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    }
  }

  private static PrivacyCapabilities privacyCapabilitiesFromNative(
      final NativeCapabilities nativeCapabilities, final boolean bridgeAvailable) {
    if (nativeCapabilities.version != PRIVACY_FFI_VERSION_V1
        || !PRODUCTION_GATE_VERSION.equals(nativeCapabilities.gateVersion)) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    }

    final Map<String, NativeCapability> algorithmsById = new LinkedHashMap<>();
    for (final NativeCapability algorithm : nativeCapabilities.algorithms) {
      if (algorithmsById.put(algorithm.algorithmId, algorithm) != null) {
        return PrivacyCapabilities.failClosed(bridgeAvailable);
      }
    }
    final NativeCapability confidentialTransfer =
        algorithmsById.get(CONFIDENTIAL_TRANSFER_ALGORITHM_ID);
    final NativeCapability unshield = algorithmsById.get(UNSHIELD_ALGORITHM_ID);
    if (confidentialTransfer == null || unshield == null) {
      return PrivacyCapabilities.failClosed(bridgeAvailable);
    }

    final List<NativeCapability> rows = Arrays.asList(confidentialTransfer, unshield);
    for (final NativeCapability row : rows) {
      if (!nativeCapabilityRowIsExact(row)) {
        return PrivacyCapabilities.failClosed(bridgeAvailable);
      }
    }

    boolean rowReady = true;
    for (final NativeCapability row : rows) {
      rowReady &=
          row.productionReady
              && row.plannedEntrypoints.isEmpty()
              && row.productionGate.ready;
    }
    final List<String> auditReferences = nativeAuditReferences(rows);
    boolean aggregateReady = rowReady && !auditReferences.isEmpty();
    for (final String key : PRODUCTION_GATE_REQUIRED) {
      aggregateReady &= nativeGatePassed(rows, key);
    }

    List<String> missing = nativeMissing(rows);
    if (!aggregateReady && missing.isEmpty()) {
      missing = PRODUCTION_GATE_MISSING;
    }

    return new PrivacyCapabilities(
        true,
        bridgeAvailable,
        aggregateReady,
        nativeGatePassed(rows, "real_proving"),
        nativeGatePassed(rows, "real_verification"),
        nativeGatePassed(rows, "chain_admission"),
        nativeGatePassed(rows, "sdk_parity"),
        nativeGatePassed(rows, "wallet_state"),
        nativeGatePassed(rows, "witness_privacy_checks"),
        nativeGatePassed(rows, "deterministic_tests"),
        nativeGatePassed(rows, "negative_adversarial_tests"),
        nativeGatePassed(rows, "replay_nullifier_tests"),
        nativeGatePassed(rows, "fuzzing"),
        nativeGatePassed(rows, "parser_fuzzing"),
        nativeGatePassed(rows, "verifier_fuzzing"),
        nativeGatePassed(rows, "performance_gates"),
        nativeGatePassed(rows, "external_audit"),
        aggregateReady ? Collections.emptyList() : missing,
        auditReferences);
  }

  private static boolean nativeGatePassed(
      final List<NativeCapability> rows, final String key) {
    for (final NativeCapability row : rows) {
      if (!row.productionGate.requiredGates.contains(key)) {
        continue;
      }
      boolean passed = false;
      for (final NativeGateStatus status : row.productionGate.gates) {
        if (key.equals(status.key) && status.passed) {
          passed = true;
          break;
        }
      }
      if (!passed) {
        return false;
      }
    }
    return true;
  }

  private static List<String> nativeMissing(final List<NativeCapability> rows) {
    final List<String> missing = new ArrayList<>();
    for (final NativeCapability row : rows) {
      missing.addAll(row.productionGate.missing);
    }
    return stableDistinct(missing);
  }

  private static List<String> nativeAuditReferences(final List<NativeCapability> rows) {
    final List<String> references = new ArrayList<>();
    for (final NativeCapability row : rows) {
      references.addAll(row.productionGate.auditReferences);
    }
    return stableDistinct(references);
  }

  private static List<String> stableDistinct(final List<String> values) {
    return Collections.unmodifiableList(new ArrayList<>(new LinkedHashSet<>(values)));
  }

  private static boolean nativeCapabilityRowIsExact(final NativeCapability row) {
    final NativeProductionGate gate = row.productionGate;
    if (!PRODUCTION_GATE_VERSION.equals(gate.version)
        || row.productionReady != gate.ready
        || !gate.requiredGates.equals(PRODUCTION_GATE_REQUIRED)
        || gate.gates.size() != PRODUCTION_GATE_REQUIRED.size()) {
      return false;
    }
    for (int index = 0; index < PRODUCTION_GATE_REQUIRED.size(); index++) {
      final NativeGateStatus status = gate.gates.get(index);
      if (!PRODUCTION_GATE_REQUIRED.get(index).equals(status.key)
          || status.passed != gate.ready) {
        return false;
      }
    }
    if (gate.ready) {
      return row.plannedEntrypoints.isEmpty()
          && gate.missing.isEmpty()
          && readyAuditReferencesAreExact(gate.auditReferences);
    }
    return gate.auditReferences.isEmpty() && !gate.missing.isEmpty();
  }

  private static boolean readyAuditReferencesAreExact(final List<String> references) {
    if (references.size() != READY_AUDIT_REFERENCE_PREFIXES.size()
        || new LinkedHashSet<>(references).size() != references.size()) {
      return false;
    }
    for (int index = 0; index < READY_AUDIT_REFERENCE_PREFIXES.size(); index++) {
      final String reference = references.get(index);
      final String prefix = READY_AUDIT_REFERENCE_PREFIXES.get(index);
      if (!reference.startsWith(prefix) || !productionEvidenceTextIsClean(reference)) {
        return false;
      }
      final String value = reference.substring(prefix.length());
      if ("review_artifact_signature:".equals(prefix)) {
        if (!productionSignatureIsValid(value)) {
          return false;
        }
      } else if (READY_HASH_REFERENCE_PREFIXES.contains(prefix)
          && !productionHashIsValid(value)) {
        return false;
      }
    }
    return true;
  }

  private static boolean productionHashIsValid(final String value) {
    if (!value.startsWith("sha256:") || value.length() != "sha256:".length() + 64) {
      return false;
    }
    for (int index = "sha256:".length(); index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'))) {
        return false;
      }
    }
    return true;
  }

  private static boolean productionSignatureIsValid(final String value) {
    if (!value.startsWith("ed25519:") || value.length() != "ed25519:".length() + 128) {
      return false;
    }
    for (int index = "ed25519:".length(); index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'))) {
        return false;
      }
    }
    return true;
  }

  private static boolean productionEvidenceTextIsClean(final String value) {
    if (value.isEmpty()
        || value.length() > 768
        || !value.trim().equals(value)
        || value.indexOf('\\') >= 0) {
      return false;
    }
    final StringBuilder compact = new StringBuilder(value.length());
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (ch < 0x20 || ch > 0x7e) {
        return false;
      }
      if (Character.isLetterOrDigit(ch)) {
        compact.append(Character.toLowerCase(ch));
      }
    }
    final String normalized = compact.toString();
    return !normalized.contains("devfixture")
        && !normalized.contains("devprooffixture")
        && !normalized.contains("localonly")
        && !normalized.contains("mock");
  }

  private static Set<String> readyHashReferencePrefixes() {
    final Set<String> prefixes = new HashSet<>();
    for (final String prefix : READY_AUDIT_REFERENCE_PREFIXES) {
      if (prefix.endsWith("_hash:")
          || prefix.endsWith("_tx_hash:")
          || prefix.endsWith("_proof_hash:")) {
        prefixes.add(prefix);
      }
    }
    return Collections.unmodifiableSet(prefixes);
  }

  @SuppressWarnings("unchecked")
  private static <T> TypeAdapter<T> typedStruct(
      final List<? extends NoritoAdapters.StructField<?>> fields,
      final NoritoAdapters.StructAdapter.StructFactory factory) {
    return (TypeAdapter<T>) (TypeAdapter<?>) NoritoAdapters.struct(fields, factory);
  }

  private static String stringField(final Map<String, Object> fields, final String name) {
    final Object value = fields.get(name);
    if (!(value instanceof String)) {
      throw new IllegalArgumentException("missing native capability string field " + name);
    }
    return (String) value;
  }

  private static boolean booleanField(final Map<String, Object> fields, final String name) {
    final Object value = fields.get(name);
    if (!(value instanceof Boolean)) {
      throw new IllegalArgumentException("missing native capability bool field " + name);
    }
    return (Boolean) value;
  }

  private static int uintField(final Map<String, Object> fields, final String name) {
    final Object value = fields.get(name);
    if (!(value instanceof Long)) {
      throw new IllegalArgumentException("missing native capability uint field " + name);
    }
    final long number = (Long) value;
    if (number < 0 || number > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("native capability uint field " + name + " is out of range");
    }
    return (int) number;
  }

  @SuppressWarnings("unchecked")
  private static <T> List<T> listField(final Map<String, Object> fields, final String name) {
    final Object value = fields.get(name);
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException("missing native capability list field " + name);
    }
    return Collections.unmodifiableList(new ArrayList<>((List<T>) value));
  }

  @SuppressWarnings("unchecked")
  private static <T> T objectField(final Map<String, Object> fields, final String name) {
    final Object value = fields.get(name);
    if (value == null) {
      throw new IllegalArgumentException("missing native capability object field " + name);
    }
    return (T) value;
  }

  private static final class NativeGateStatus {
    private final String key;
    private final boolean passed;

    private NativeGateStatus(final String key, final boolean passed) {
      this.key = key;
      this.passed = passed;
    }
  }

  private static final class NativeProductionGate {
    private final String version;
    private final boolean ready;
    private final List<NativeGateStatus> gates;
    private final List<String> requiredGates;
    private final List<String> missing;
    private final List<String> auditReferences;

    private NativeProductionGate(
        final String version,
        final boolean ready,
        final List<NativeGateStatus> gates,
        final List<String> requiredGates,
        final List<String> missing,
        final List<String> auditReferences) {
      this.version = version;
      this.ready = ready;
      this.gates = gates;
      this.requiredGates = requiredGates;
      this.missing = missing;
      this.auditReferences = auditReferences;
    }
  }

  private static final class NativeCapability {
    private final String algorithmId;
    @SuppressWarnings("unused")
    private final String proofFamily;
    @SuppressWarnings("unused")
    private final String backendFamily;
    @SuppressWarnings("unused")
    private final List<String> sdkEntrypoints;
    private final List<String> plannedEntrypoints;
    private final boolean productionReady;
    private final NativeProductionGate productionGate;

    private NativeCapability(
        final String algorithmId,
        final String proofFamily,
        final String backendFamily,
        final List<String> sdkEntrypoints,
        final List<String> plannedEntrypoints,
        final boolean productionReady,
        final NativeProductionGate productionGate) {
      this.algorithmId = algorithmId;
      this.proofFamily = proofFamily;
      this.backendFamily = backendFamily;
      this.sdkEntrypoints = sdkEntrypoints;
      this.plannedEntrypoints = plannedEntrypoints;
      this.productionReady = productionReady;
      this.productionGate = productionGate;
    }
  }

  private static final class NativeCapabilities {
    private final int version;
    private final String gateVersion;
    private final List<NativeCapability> algorithms;

    private NativeCapabilities(
        final int version,
        final String gateVersion,
        final List<NativeCapability> algorithms) {
      this.version = version;
      this.gateVersion = gateVersion;
      this.algorithms = algorithms;
    }
  }

  public static final class PrivacyCapabilities {
    private final boolean androidSdkAvailable;
    private final boolean bridgeAvailable;
    private final String productionGateVersion;
    private final boolean productionReady;
    private final boolean realProving;
    private final boolean realVerification;
    private final boolean chainAdmission;
    private final boolean sdkParity;
    private final boolean walletState;
    private final boolean witnessPrivacyChecks;
    private final boolean deterministicTests;
    private final boolean negativeAdversarialTests;
    private final boolean replayNullifierTests;
    private final boolean fuzzing;
    private final boolean parserFuzzing;
    private final boolean verifierFuzzing;
    private final boolean performanceGates;
    private final boolean externalAudit;
    private final List<String> missingProductionGates;
    private final List<String> auditReferences;

    private static PrivacyCapabilities failClosed(final boolean bridgeAvailable) {
      return new PrivacyCapabilities(
          true,
          bridgeAvailable,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          false,
          PRODUCTION_GATE_MISSING,
          PRODUCTION_GATE_AUDIT_REFERENCES);
    }

    private PrivacyCapabilities(
        final boolean androidSdkAvailable,
        final boolean bridgeAvailable,
        final boolean productionReady,
        final boolean realProving,
        final boolean realVerification,
        final boolean chainAdmission,
        final boolean sdkParity,
        final boolean walletState,
        final boolean witnessPrivacyChecks,
        final boolean deterministicTests,
        final boolean negativeAdversarialTests,
        final boolean replayNullifierTests,
        final boolean fuzzing,
        final boolean parserFuzzing,
        final boolean verifierFuzzing,
        final boolean performanceGates,
        final boolean externalAudit,
        final List<String> missingProductionGates,
        final List<String> auditReferences) {
      this.androidSdkAvailable = androidSdkAvailable;
      this.bridgeAvailable = bridgeAvailable;
      this.productionGateVersion = PRODUCTION_GATE_VERSION;
      this.productionReady = productionReady;
      this.realProving = realProving;
      this.realVerification = realVerification;
      this.chainAdmission = chainAdmission;
      this.sdkParity = sdkParity;
      this.walletState = walletState;
      this.witnessPrivacyChecks = witnessPrivacyChecks;
      this.deterministicTests = deterministicTests;
      this.negativeAdversarialTests = negativeAdversarialTests;
      this.replayNullifierTests = replayNullifierTests;
      this.fuzzing = fuzzing;
      this.parserFuzzing = parserFuzzing;
      this.verifierFuzzing = verifierFuzzing;
      this.performanceGates = performanceGates;
      this.externalAudit = externalAudit;
      this.missingProductionGates = stableDistinct(missingProductionGates);
      this.auditReferences = stableDistinct(auditReferences);
    }

    public boolean isAndroidSdkAvailable() {
      return androidSdkAvailable;
    }

    public boolean isBridgeAvailable() {
      return bridgeAvailable;
    }

    public String productionGateVersion() {
      return productionGateVersion;
    }

    public boolean isProductionReady() {
      return productionReady;
    }

    public boolean hasRealProving() {
      return realProving;
    }

    public boolean hasRealVerification() {
      return realVerification;
    }

    public boolean hasChainAdmission() {
      return chainAdmission;
    }

    public boolean hasSdkParity() {
      return sdkParity;
    }

    public boolean hasWalletState() {
      return walletState;
    }

    public boolean hasWitnessPrivacyChecks() {
      return witnessPrivacyChecks;
    }

    public boolean hasDeterministicTests() {
      return deterministicTests;
    }

    public boolean hasNegativeAdversarialTests() {
      return negativeAdversarialTests;
    }

    public boolean hasReplayNullifierTests() {
      return replayNullifierTests;
    }

    public boolean hasFuzzing() {
      return fuzzing;
    }

    public boolean hasParserFuzzing() {
      return parserFuzzing;
    }

    public boolean hasVerifierFuzzing() {
      return verifierFuzzing;
    }

    public boolean hasPerformanceGates() {
      return performanceGates;
    }

    public boolean hasExternalAudit() {
      return externalAudit;
    }

    public List<String> missingProductionGates() {
      return missingProductionGates;
    }

    public List<String> requiredProductionGates() {
      return PRODUCTION_GATE_REQUIRED;
    }

    public List<String> auditReferences() {
      return auditReferences;
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[] nativeCapabilities();

  private static native byte[] nativeProofRequest(
      byte[] algorithmId,
      byte[] entrypoint,
      byte[] vkRef,
      byte[] publicInputs,
      byte[] witness,
      byte[] proof);

  private static native byte[] nativeBuildProof(byte[] requestArchive);

  private static native byte[] nativeVerifyProof(byte[] requestArchive);
}
