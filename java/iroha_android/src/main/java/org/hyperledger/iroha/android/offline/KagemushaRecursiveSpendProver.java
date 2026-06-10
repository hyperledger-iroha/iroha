package org.hyperledger.iroha.android.offline;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

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
  private static final byte[] KAGEMUSHA_ZK1_MAGIC = new byte[] {0x5A, 0x4B, 0x31, 0x00};
  private static final byte[] KAGEMUSHA_ZK1_TLV_CID1 =
      "CID1".getBytes(StandardCharsets.US_ASCII);
  private static final byte[] KAGEMUSHA_ZK1_TLV_IPAK =
      "IPAK".getBytes(StandardCharsets.US_ASCII);
  private static final byte[] KAGEMUSHA_ZK1_TLV_H2VK =
      "H2VK".getBytes(StandardCharsets.US_ASCII);
  private static final int KAGEMUSHA_NORITO_COMPACT_LEN_FLAG = 0x02;
  private static final int KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG = 0x04;
  private static final int PRIVACY_NORITO_FIELD_BITSET_FLAG = 0x20;
  private static final int KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 = 1;
  private static final byte[] KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH =
      new byte[] {
        (byte) 0xC8, (byte) 0x84, (byte) 0x89, 0x61,
        (byte) 0x8A, 0x01, 0x2C, 0x28,
        0x3F, (byte) 0xF3, (byte) 0xBB, 0x2E,
        (byte) 0xBA, (byte) 0xBC, 0x77, 0x75
      };
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  public enum Mode {
    CHECKED_PREFOLD_V1("checked_prefold_v1"),
    RECURSIVE_COMPACT_V1("recursive_compact_v1"),
    RECURSIVE_SPEND_V1("recursive_spend_v1");

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
    // ABI-7 compact mode is not a production default yet.
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
    validateLineageKeyArtifactPackageBinding(
        artifacts.proofCircuitId,
        artifacts.lineageVerifierKeyBackend,
        lineageVerifierKey,
        lineageProvingKeyArchive);
    return artifacts;
  }

  private static void validateLineageKeyArtifactPackageBinding(
      final String proofCircuitId,
      final String lineageVerifierKeyBackend,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive) {
    final String verifierCircuitId = lineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey);
    if (!proofCircuitId.equals(verifierCircuitId)) {
      throw new IllegalArgumentException("lineage_verifier_key");
    }
    final byte[] archivePayload = lineageProvingKeyArchivePayload(lineageProvingKeyArchive);
    final byte[] circuitIdBytes = proofCircuitId.getBytes(StandardCharsets.UTF_8);
    final byte[] verifierKeyCommitment =
        verifyingKeyCommitment(lineageVerifierKeyBackend, lineageVerifierKey);
    if (indexOfSlice(archivePayload, circuitIdBytes) < 0
        || indexOfSlice(archivePayload, verifierKeyCommitment) < 0) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    final LineageProvingKeyArchive archive =
        decodeLineageProvingKeyArchivePayload(
            archivePayload, lineageProvingKeyArchive[39] & 0xFF);
    if (archive.version != KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1
        || !proofCircuitId.equals(archive.circuitFamily)
        || !Arrays.equals(archive.verifierKeyCommitment, verifierKeyCommitment)
        || archive.provingKey.length == 0) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
  }

  private static String lineageVerifierKeyEnvelopeCircuitId(final byte[] lineageVerifierKey) {
    if (!startsWith(lineageVerifierKey, KAGEMUSHA_ZK1_MAGIC)) {
      throw new IllegalArgumentException("lineage_verifier_key");
    }
    int offset = KAGEMUSHA_ZK1_MAGIC.length;
    String circuitId = null;
    boolean sawIpaK = false;
    boolean sawH2Vk = false;
    while (offset < lineageVerifierKey.length) {
      if (offset + 8 > lineageVerifierKey.length) {
        throw new IllegalArgumentException("lineage_verifier_key");
      }
      final byte[] tag = Arrays.copyOfRange(lineageVerifierKey, offset, offset + 4);
      final int payloadLength = readIntLittleEndian(lineageVerifierKey, offset + 4);
      final int payloadStart = offset + 8;
      final long payloadEndLong = (long) payloadStart + payloadLength;
      if (payloadLength < 0 || payloadEndLong > lineageVerifierKey.length) {
        throw new IllegalArgumentException("lineage_verifier_key");
      }
      final int payloadEnd = (int) payloadEndLong;
      final byte[] payload = Arrays.copyOfRange(lineageVerifierKey, payloadStart, payloadEnd);
      if (Arrays.equals(tag, KAGEMUSHA_ZK1_TLV_CID1)) {
        if (circuitId != null || payload.length == 0 || !isPrintableAscii(payload)) {
          throw new IllegalArgumentException("lineage_verifier_key");
        }
        final String decoded = new String(payload, StandardCharsets.UTF_8).trim();
        if (decoded.isEmpty()) {
          throw new IllegalArgumentException("lineage_verifier_key");
        }
        circuitId = decoded;
      } else if (Arrays.equals(tag, KAGEMUSHA_ZK1_TLV_IPAK)) {
        if (sawIpaK || payload.length != 4) {
          throw new IllegalArgumentException("lineage_verifier_key");
        }
        sawIpaK = true;
      } else if (Arrays.equals(tag, KAGEMUSHA_ZK1_TLV_H2VK)) {
        if (sawH2Vk || payload.length == 0) {
          throw new IllegalArgumentException("lineage_verifier_key");
        }
        sawH2Vk = true;
      } else {
        throw new IllegalArgumentException("lineage_verifier_key");
      }
      offset = payloadEnd;
    }
    if (circuitId == null || !sawIpaK || !sawH2Vk) {
      throw new IllegalArgumentException("lineage_verifier_key");
    }
    return circuitId;
  }

  private static byte[] lineageProvingKeyArchivePayload(final byte[] lineageProvingKeyArchive) {
    if (!KagemushaCompactPaymentTokenProver.isValidNoritoArchive(lineageProvingKeyArchive)
        || !KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(
            lineageProvingKeyArchive)) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    if (!Arrays.equals(
            Arrays.copyOfRange(lineageProvingKeyArchive, 6, 22),
            KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH)
        || (lineageProvingKeyArchive[39] & KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG) != 0
        || (lineageProvingKeyArchive[39] & PRIVACY_NORITO_FIELD_BITSET_FLAG) != 0) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    final long payloadLength = readLongLittleEndian(lineageProvingKeyArchive, 23);
    if (payloadLength <= 0 || payloadLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    final int payloadOffset = lineageProvingKeyArchive.length - (int) payloadLength;
    return Arrays.copyOfRange(lineageProvingKeyArchive, payloadOffset, lineageProvingKeyArchive.length);
  }

  private static LineageProvingKeyArchive decodeLineageProvingKeyArchivePayload(
      final byte[] payload, final int flags) {
    int offset = 0;
    NoritoField field = readNoritoField(payload, offset, flags);
    if (field.payload.length != 2) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    final int version = readUnsignedShortLittleEndian(field.payload, 0);
    offset = field.offset;

    field = readNoritoField(payload, offset, flags);
    final String circuitFamily = decodeNoritoString(field.payload, flags);
    offset = field.offset;

    field = readNoritoField(payload, offset, flags);
    final byte[] verifierKeyCommitment = field.payload;
    if (verifierKeyCommitment.length != 32) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    offset = field.offset;

    field = readNoritoField(payload, offset, flags);
    final byte[] provingKey = decodeNoritoByteVec(field.payload);
    offset = field.offset;
    if (offset != payload.length) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    return new LineageProvingKeyArchive(
        version, circuitFamily, verifierKeyCommitment, provingKey);
  }

  private static NoritoField readNoritoField(
      final byte[] buffer, final int offset, final int flags) {
    final NoritoLength length = readNoritoLength(buffer, offset, flags);
    final long payloadEnd = (long) length.offset + length.value;
    if (payloadEnd > buffer.length) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    return new NoritoField(
        Arrays.copyOfRange(buffer, length.offset, (int) payloadEnd),
        (int) payloadEnd);
  }

  private static NoritoLength readNoritoLength(
      final byte[] buffer, final int offset, final int flags) {
    if (offset < 0) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    if ((flags & KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) == 0) {
      if (offset + 8 > buffer.length) {
        throw new IllegalArgumentException("lineage_proving_key_archive");
      }
      final long value = readLongLittleEndian(buffer, offset);
      if (value < 0 || value > Integer.MAX_VALUE || value > buffer.length) {
        throw new IllegalArgumentException("lineage_proving_key_archive");
      }
      return new NoritoLength((int) value, offset + 8);
    }

    long value = 0L;
    int shift = 0;
    int cursor = offset;
    for (int index = 0; index < 10; index++) {
      if (cursor >= buffer.length) {
        throw new IllegalArgumentException("lineage_proving_key_archive");
      }
      final int valueByte = buffer[cursor] & 0xFF;
      cursor += 1;
      final long chunk = valueByte & 0x7FL;
      if (shift >= 63 && chunk != 0L) {
        throw new IllegalArgumentException("lineage_proving_key_archive");
      }
      value |= chunk << shift;
      if ((valueByte & 0x80) == 0) {
        final int encodedLength = cursor - offset;
        if (encodedLength > 1 && value < (1L << (7 * (encodedLength - 1)))) {
          throw new IllegalArgumentException("lineage_proving_key_archive");
        }
        if (value > Integer.MAX_VALUE || value > buffer.length) {
          throw new IllegalArgumentException("lineage_proving_key_archive");
        }
        return new NoritoLength((int) value, cursor);
      }
      shift += 7;
    }
    throw new IllegalArgumentException("lineage_proving_key_archive");
  }

  private static String decodeNoritoString(final byte[] payload, final int flags) {
    final NoritoLength length = readNoritoLength(payload, 0, flags);
    final int end = length.offset + length.value;
    if (end != payload.length) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    try {
      return StandardCharsets.UTF_8
          .newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(payload, length.offset, length.value))
          .toString();
    } catch (final CharacterCodingException ex) {
      throw new IllegalArgumentException("lineage_proving_key_archive", ex);
    }
  }

  private static byte[] decodeNoritoByteVec(final byte[] payload) {
    if (payload.length < 8) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    final long length = readLongLittleEndian(payload, 0);
    final long end = 8L + length;
    if (length < 0 || end != payload.length) {
      throw new IllegalArgumentException("lineage_proving_key_archive");
    }
    return Arrays.copyOfRange(payload, 8, (int) end);
  }

  private static byte[] verifyingKeyCommitment(
      final String lineageVerifierKeyBackend, final byte[] lineageVerifierKey) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] backend = lineageVerifierKeyBackend.getBytes(StandardCharsets.UTF_8);
      digest.update("iroha:zk:v1:vk".getBytes(StandardCharsets.US_ASCII));
      digest.update(longBigEndian(backend.length));
      digest.update(backend);
      digest.update(longBigEndian(lineageVerifierKey.length));
      digest.update(lineageVerifierKey);
      return digest.digest();
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  private static boolean isPrintableAscii(final byte[] bytes) {
    for (final byte value : bytes) {
      final int unsigned = value & 0xFF;
      if (unsigned < 0x20 || unsigned > 0x7E) {
        return false;
      }
    }
    return true;
  }

  private static boolean startsWith(final byte[] bytes, final byte[] prefix) {
    return bytes != null
        && bytes.length >= prefix.length
        && Arrays.equals(Arrays.copyOfRange(bytes, 0, prefix.length), prefix);
  }

  private static int indexOfSlice(final byte[] bytes, final byte[] needle) {
    if (needle.length == 0 || needle.length > bytes.length) {
      return -1;
    }
    for (int offset = 0; offset <= bytes.length - needle.length; offset++) {
      boolean matched = true;
      for (int index = 0; index < needle.length; index++) {
        if (bytes[offset + index] != needle[index]) {
          matched = false;
          break;
        }
      }
      if (matched) {
        return offset;
      }
    }
    return -1;
  }

  private static int readIntLittleEndian(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xFF)
        | ((bytes[offset + 1] & 0xFF) << 8)
        | ((bytes[offset + 2] & 0xFF) << 16)
        | ((bytes[offset + 3] & 0xFF) << 24);
  }

  private static int readUnsignedShortLittleEndian(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xFF) | ((bytes[offset + 1] & 0xFF) << 8);
  }

  private static long readLongLittleEndian(final byte[] bytes, final int offset) {
    long value = 0L;
    for (int index = 0; index < 8; index++) {
      value |= (bytes[offset + index] & 0xFFL) << (index * 8);
    }
    return value;
  }

  private static byte[] longBigEndian(final long value) {
    final byte[] output = new byte[8];
    for (int index = 0; index < output.length; index++) {
      output[index] = (byte) ((value >>> ((7 - index) * 8)) & 0xFF);
    }
    return output;
  }

  private static final class LineageProvingKeyArchive {
    private final int version;
    private final String circuitFamily;
    private final byte[] verifierKeyCommitment;
    private final byte[] provingKey;

    private LineageProvingKeyArchive(
        final int version,
        final String circuitFamily,
        final byte[] verifierKeyCommitment,
        final byte[] provingKey) {
      this.version = version;
      this.circuitFamily = circuitFamily;
      this.verifierKeyCommitment = verifierKeyCommitment;
      this.provingKey = provingKey;
    }
  }

  private static final class NoritoField {
    private final byte[] payload;
    private final int offset;

    private NoritoField(final byte[] payload, final int offset) {
      this.payload = payload;
      this.offset = offset;
    }
  }

  private static final class NoritoLength {
    private final int value;
    private final int offset;

    private NoritoLength(final int value, final int offset) {
      this.value = value;
      this.offset = offset;
    }
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
    final byte[] ownedArchive = ownedNativeInput(archive, archiveName);
    requireNative();
    final byte[] output = call.run(ownedArchive);
    return requireRecursiveSpendOutput(output, label);
  }

  private static byte[] call(
      final String label,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativePairCall call) {
    final byte[] request = ownedNativeInput(requestArchive, "requestArchive");
    final byte[] bundle = ownedNativeInput(bundleArchive, "bundleArchive");
    requireNative();
    final byte[] output = call.run(request, bundle);
    return requireRecursiveSpendOutput(output, label);
  }

  private static byte[] call(
      final String label,
      final byte[] previousWitnessArchive,
      final byte[] requestArchive,
      final byte[] bundleArchive,
      final NativeTripleCall call) {
    final byte[] previousWitness = ownedNativeInput(previousWitnessArchive, "previousWitnessArchive");
    final byte[] request = ownedNativeInput(requestArchive, "requestArchive");
    final byte[] bundle = ownedNativeInput(bundleArchive, "bundleArchive");
    requireNative();
    final byte[] output = call.run(previousWitness, request, bundle);
    return requireRecursiveSpendOutput(output, label);
  }

  static byte[] requireRecursiveSpendOutput(final byte[] output, final String label) {
    return requireNativeOutput(output, "native " + label);
  }

  static byte[] ownedNativeInput(final byte[] archive, final String archiveName) {
    requireNativeInput(archive, archiveName);
    return Arrays.copyOf(archive, archive.length);
  }

  private static void requireNativeInput(final byte[] archive, final String archiveName) {
    if (archive == null || archive.length == 0) {
      throw new IllegalArgumentException(archiveName + " must not be empty");
    }
    if (archive.length > NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalArgumentException(
          archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes");
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
