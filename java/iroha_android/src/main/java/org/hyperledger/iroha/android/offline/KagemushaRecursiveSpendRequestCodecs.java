package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountAddress.AccountAddressException;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Norito request builders and result decoders for recursive Kagemusha spend ABI v1. */
public final class KagemushaRecursiveSpendRequestCodecs {
  public static final String SCHEMA_INIT_REQUEST =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1";
  public static final String SCHEMA_APPEND_REQUEST =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1";
  public static final String SCHEMA_VERIFY_REQUEST =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyRequestV1";
  public static final String SCHEMA_VERIFY_RESULT =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1";
  public static final String SCHEMA_REDEEM_REQUEST =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1";
  public static final String SCHEMA_BUNDLE =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1";
  public static final String SCHEMA_RECORD_BUNDLE =
      "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle";
  public static final String SCHEMA_LINEAGE_WITNESS =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1";
  public static final String SCHEMA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS =
      "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs";
  public static final String SCHEMA_PROOF_ATTACHMENT =
      "iroha_data_model::proof::ProofAttachment";
  public static final String SCHEMA_VERIFYING_KEY_RECORD =
      "iroha_data_model::proof::VerifyingKeyRecord";
  public static final String SCHEMA_OPEN_VERIFY_ENVELOPE =
      "iroha_data_model::zk::OpenVerifyEnvelope";
  public static final String CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID =
      "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified";
  public static final String CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID =
      "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified";

  private static final int REQUEST_FLAGS = NoritoHeader.COMPACT_LEN;
  private static final int KAGEMUSHA_FOLD_STEP_MAX_INPUTS = 2;
  private static final BigInteger MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final int MAX_U128_DECIMAL_DIGITS = MAX_U128.toString().length();
  private static final BigInteger BYTE_MASK = BigInteger.valueOf(0xffL);
  private static final int MULTISIG_POLICY_VERSION = 1;
  private static final char[] HEX = "0123456789abcdef".toCharArray();
  private static final int PRIVACY_FFI_VERSION_V1 = 1;
  private static final int PRIVACY_FFI_STATUS_OK = 0;
  private static final int PRIVACY_SCHEMA_BUILD_PROOF_RESULT = 0x42;
  private static final long BACKEND_TAG_HALO2_IPA_PASTA = 0L;
  private static final int CONFIDENTIAL_STATUS_ACTIVE = 1;
  private static final int CONFIDENTIAL_V2_MAX_PROOF_BYTES = 192 * 1024;
  private static final String KAGEMUSHA_VERIFIER_NAMESPACE = "offline_kagemusha";
  private static final String ZK_BACKEND_HALO2_IPA = "halo2/ipa";
  private static final String CONFIDENTIAL_TRANSFER_ALGORITHM_ID = "confidential-transfer-v2";
  private static final String CONFIDENTIAL_TRANSFER_ENTRYPOINT = "buildConfidentialTransferProofV2";
  private static final String CONFIDENTIAL_UNSHIELD_ALGORITHM_ID = "unshield";
  private static final String CONFIDENTIAL_UNSHIELD_ENTRYPOINT = "buildConfidentialUnshieldProofV3";
  private static final String CONFIDENTIAL_RECORD_CURVE = "pallas";
  private static final int ZK1_MAX_TLV_BYTES = 8 * 1024 * 1024;
  private static final int ZK1_MAX_INSTANCE_COLUMNS = 64;
  private static final int ZK1_MAX_INSTANCE_ROWS = 8192;
  private static final byte[] CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\","
              + "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);
  private static final byte[] CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\","
              + "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);
  private static final byte[] ZK1_MAGIC = new byte[] {0x5a, 0x4b, 0x31, 0x00};

  private KagemushaRecursiveSpendRequestCodecs() {}

  public static byte[] encodeInitRequest(final InitSpendRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"), SCHEMA_INIT_REQUEST, INIT_REQUEST_ADAPTER, REQUEST_FLAGS);
  }

  public static byte[] encodeAppendRequest(final AppendSpendRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"), SCHEMA_APPEND_REQUEST, APPEND_REQUEST_ADAPTER, REQUEST_FLAGS);
  }

  public static byte[] encodeVerifyRequest(final VerifySpendRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"), SCHEMA_VERIFY_REQUEST, VERIFY_REQUEST_ADAPTER, REQUEST_FLAGS);
  }

  public static byte[] encodeRedeemRequest(final RedeemSpendRequest request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"), SCHEMA_REDEEM_REQUEST, REDEEM_REQUEST_ADAPTER, REQUEST_FLAGS);
  }

  public static byte[] buildPallasOpenEnvelopesArchive(final List<VerifiedFoldHopEvidence> hops) {
    require(hops != null && !hops.isEmpty(), "hops must not be empty");
    return buildPallasOpenEnvelopesArchiveForRecordBundle(buildVerifiedFoldRecordBundle(hops));
  }

  public static byte[] buildPallasOpenEnvelopesArchiveForRecordBundle(
      final byte[] recordBundle) {
    require(recordBundle != null, "recordBundle is required");
    compactPayloadForRequest(recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle");
    return KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive(recordBundle);
  }

  public static byte[] buildPreviousProofOpenEnvelopesArchive(final byte[] previousBundle) {
    require(previousBundle != null, "previousBundle is required");
    decodeBundle(previousBundle);
    return KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive(previousBundle);
  }

  public static byte[] buildVerifiedFoldRecordBundle(
      final List<byte[]> hopProofOutputArchives, final List<VerifierRecordRef> hopVerifierRecords) {
    require(hopProofOutputArchives != null && !hopProofOutputArchives.isEmpty(),
        "hopProofOutputArchives must not be empty");
    require(hopVerifierRecords != null && hopVerifierRecords.size() == hopProofOutputArchives.size(),
        "hopVerifierRecords must match hopProofOutputArchives");
    throw new IllegalArgumentException(
        "chainId, asset, and rootAfter are required to build KagemushaVerifiedFoldRecordBundle; "
            + "use VerifiedFoldHopEvidence inputs instead");
  }

  public static byte[] buildVerifiedFoldRecordBundle(final List<VerifiedFoldHopEvidence> hops) {
    require(hops != null && !hops.isEmpty(), "hops must not be empty");
    require(hops.size() <= KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS,
        "hops must not exceed " + KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS);
    final List<PreparedVerifiedFoldHop> prepared = new ArrayList<>();
    for (int i = 0; i < hops.size(); i++) {
      prepared.add(prepareTransferHop(i, hops.get(i)));
    }
    final String chainId = prepared.get(0).chainId;
    final String asset = prepared.get(0).asset;
    byte[] expectedRootBefore = null;
    for (int i = 0; i < prepared.size(); i++) {
      final PreparedVerifiedFoldHop hop = prepared.get(i);
      require(chainId.equals(hop.chainId), "hop " + i + " chainId does not match first hop");
      require(asset.equals(hop.asset), "hop " + i + " asset does not match first hop");
      if (expectedRootBefore != null) {
        require(Arrays.equals(hop.publicInputs.rootBefore, expectedRootBefore),
            "hop " + i + " rootBefore must equal previous hop rootAfter");
      }
      require(!Arrays.equals(hop.publicInputs.rootBefore, hop.rootAfter),
          "hop " + i + " rootAfter must differ from rootBefore");
      expectedRootBefore = hop.rootAfter;
    }
    return NoritoCodec.encode(prepared, SCHEMA_RECORD_BUNDLE, VERIFIED_FOLD_RECORD_BUNDLE_ADAPTER, REQUEST_FLAGS);
  }

  public static byte[] buildRedeemProofAttachment(
      final byte[] unshieldProofOutputArchive, final VerifierRecordRef unshieldVerifierRecord) {
    require(unshieldProofOutputArchive != null, "unshieldProofOutputArchive is required");
    require(unshieldVerifierRecord != null, "unshieldVerifierRecord is required");
    final PrivacyBuildResult proof =
        parsePrivacyBuildResult(
            unshieldProofOutputArchive,
            CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            CONFIDENTIAL_UNSHIELD_ENTRYPOINT,
            "unshieldProofOutputArchive");
    final OpenVerifyEnvelopeValue envelope = decodeOpenVerifyEnvelope(proof.proof, "unshield proof");
    final VerifierRecordValue verifierRecord =
        decodeAndValidateVerifierRecord(
            unshieldVerifierRecord,
            envelope,
            CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            proof.proof.length,
            "unshieldVerifierRecord");
    return NoritoCodec.encode(
        proofAttachmentPayload(envelope, verifierRecord),
        SCHEMA_PROOF_ATTACHMENT,
        RAW_PAYLOAD_ADAPTER,
        REQUEST_FLAGS);
  }

  public static byte[] buildRecursiveSpendInitRequest(
      final VerifiedFoldHopEvidence hop,
      final byte[] pallasOpenEnvelopes,
      final SpendableNoteDescriptor spendableNote,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(hop != null, "hop is required");
    require(pallasOpenEnvelopes != null, "pallasOpenEnvelopes is required");
    require(spendableNote != null, "spendableNote is required");
    return encodeInitRequest(
        new InitSpendRequest(
            buildVerifiedFoldRecordBundle(Arrays.asList(hop)),
            pallasOpenEnvelopes,
            spendableNote,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendInitRequest(
      final VerifiedFoldHopEvidence hop,
      final SpendableNoteDescriptor spendableNote,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(hop != null, "hop is required");
    require(spendableNote != null, "spendableNote is required");
    final byte[] recordBundle = buildVerifiedFoldRecordBundle(Arrays.asList(hop));
    preflightInitLineageKeyMaterialForAutoGeneration(
        lineageVerifierKey, lineageProvingKeyArchive);
    final byte[] pallasOpenEnvelopes =
        buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle);
    return encodeInitRequest(
        new InitSpendRequest(
            recordBundle,
            pallasOpenEnvelopes,
            spendableNote,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendInitRequest(
      final VerifiedFoldHopEvidence hop,
      final byte[] pallasOpenEnvelopes,
      final SpendableNoteDescriptor spendableNote,
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
      final Long blockHeight) {
    require(hop != null, "hop is required");
    require(pallasOpenEnvelopes != null, "pallasOpenEnvelopes is required");
    require(spendableNote != null, "spendableNote is required");
    return encodeInitRequest(
        new InitSpendRequest(
            buildVerifiedFoldRecordBundle(Arrays.asList(hop)),
            pallasOpenEnvelopes,
            spendableNote,
            lineageKeyArtifacts,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendInitRequest(
      final VerifiedFoldHopEvidence hop,
      final SpendableNoteDescriptor spendableNote,
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
      final Long blockHeight) {
    require(hop != null, "hop is required");
    require(spendableNote != null, "spendableNote is required");
    final byte[] recordBundle = buildVerifiedFoldRecordBundle(Arrays.asList(hop));
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checkedLineageKeyArtifacts =
        requireInitLineageKeyArtifacts(lineageKeyArtifacts);
    final byte[] pallasOpenEnvelopes =
        buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle);
    return encodeInitRequest(
        new InitSpendRequest(
            recordBundle,
            pallasOpenEnvelopes,
            spendableNote,
            checkedLineageKeyArtifacts,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendInitRequest(
      final byte[] proofOutputArchive,
      final VerifierRecordRef verifierRecord,
      final SpendableNoteDescriptor spendableNote,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(proofOutputArchive != null && proofOutputArchive.length > 0,
        "proofOutputArchive must not be empty");
    require(verifierRecord != null, "verifierRecord is required");
    require(spendableNote != null, "spendableNote is required");
    return failClosedProofOnlyRecursiveSpendRequest();
  }

  public static byte[] buildRecursiveSpendAppendRequest(
      final byte[] previousBundle,
      final VerifiedFoldHopEvidence hop,
      final byte[] pallasOpenEnvelopes,
      final SpendableNoteDescriptor spendableNote,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord,
      final byte[] previousProofOpenEnvelopes,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(previousBundle != null, "previousBundle is required");
    require(hop != null, "hop is required");
    require(pallasOpenEnvelopes != null, "pallasOpenEnvelopes is required");
    require(spendableNote != null, "spendableNote is required");
    return encodeAppendRequest(
        new AppendSpendRequest(
            previousBundle,
            buildVerifiedFoldRecordBundle(Arrays.asList(hop)),
            pallasOpenEnvelopes,
            spendableNote,
            outputCircuitId,
            previousLineageVerifierRecord,
            previousProofOpenEnvelopes,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendAppendRequest(
      final byte[] previousBundle,
      final VerifiedFoldHopEvidence hop,
      final SpendableNoteDescriptor spendableNote,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord,
      final byte[] previousProofOpenEnvelopes,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(previousBundle != null, "previousBundle is required");
    require(hop != null, "hop is required");
    require(spendableNote != null, "spendableNote is required");
    final byte[] recordBundle = buildVerifiedFoldRecordBundle(Arrays.asList(hop));
    final SpendBundleSummary previousSummary =
        preflightAppendPreviousLineageForAutoGeneration(
            previousBundle, outputCircuitId, previousLineageVerifierRecord);
    preflightAppendLineageKeyMaterialForAutoGeneration(
        outputCircuitId, lineageVerifierKey, lineageProvingKeyArchive);
    final byte[] pallasOpenEnvelopes =
        buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle);
    final byte[] previousOpenEnvelopes =
        previousProofOpenEnvelopesOrGenerated(
            previousBundle, outputCircuitId, previousSummary, previousProofOpenEnvelopes);
    return encodeAppendRequest(
        new AppendSpendRequest(
            previousBundle,
            recordBundle,
            pallasOpenEnvelopes,
            spendableNote,
            outputCircuitId,
            previousLineageVerifierRecord,
            previousOpenEnvelopes,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendAppendRequest(
      final byte[] previousBundle,
      final VerifiedFoldHopEvidence hop,
      final byte[] pallasOpenEnvelopes,
      final SpendableNoteDescriptor spendableNote,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord,
      final byte[] previousProofOpenEnvelopes,
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
      final Long blockHeight) {
    require(previousBundle != null, "previousBundle is required");
    require(hop != null, "hop is required");
    require(pallasOpenEnvelopes != null, "pallasOpenEnvelopes is required");
    require(spendableNote != null, "spendableNote is required");
    return encodeAppendRequest(
        new AppendSpendRequest(
            previousBundle,
            buildVerifiedFoldRecordBundle(Arrays.asList(hop)),
            pallasOpenEnvelopes,
            spendableNote,
            outputCircuitId,
            previousLineageVerifierRecord,
            previousProofOpenEnvelopes,
            lineageKeyArtifacts,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendAppendRequest(
      final byte[] previousBundle,
      final VerifiedFoldHopEvidence hop,
      final SpendableNoteDescriptor spendableNote,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord,
      final byte[] previousProofOpenEnvelopes,
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
      final Long blockHeight) {
    require(previousBundle != null, "previousBundle is required");
    require(hop != null, "hop is required");
    require(spendableNote != null, "spendableNote is required");
    final byte[] recordBundle = buildVerifiedFoldRecordBundle(Arrays.asList(hop));
    final SpendBundleSummary previousSummary =
        preflightAppendPreviousLineageForAutoGeneration(
            previousBundle, outputCircuitId, previousLineageVerifierRecord);
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checkedLineageKeyArtifacts =
        preflightAppendLineageKeyArtifactsForAutoGeneration(outputCircuitId, lineageKeyArtifacts);
    final byte[] pallasOpenEnvelopes =
        buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle);
    final byte[] previousOpenEnvelopes =
        previousProofOpenEnvelopesOrGenerated(
            previousBundle, outputCircuitId, previousSummary, previousProofOpenEnvelopes);
    return encodeAppendRequest(
        new AppendSpendRequest(
            previousBundle,
            recordBundle,
            pallasOpenEnvelopes,
            spendableNote,
            outputCircuitId,
            previousLineageVerifierRecord,
            previousOpenEnvelopes,
            checkedLineageKeyArtifacts,
            blockHeight));
  }

  public static byte[] buildRecursiveSpendAppendRequest(
      final byte[] previousBundle,
      final byte[] proofOutputArchive,
      final VerifierRecordRef verifierRecord,
      final SpendableNoteDescriptor spendableNote,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord,
      final byte[] previousProofOpenEnvelopes,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive,
      final Long blockHeight) {
    require(previousBundle != null, "previousBundle is required");
    require(proofOutputArchive != null && proofOutputArchive.length > 0,
        "proofOutputArchive must not be empty");
    require(verifierRecord != null, "verifierRecord is required");
    require(spendableNote != null, "spendableNote is required");
    return failClosedProofOnlyRecursiveSpendRequest();
  }

  public static VerifySpendResult decodeVerifyResult(final byte[] archive) {
    final ArchivePayload payload = requirePayloadArchive(archive, SCHEMA_VERIFY_RESULT, "verifyResult");
    require(payload.flags == REQUEST_FLAGS, "verifyResult must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(payload.payload, payload.flags);
    final boolean valid = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBool);
    final int hopCount =
        readField(decoder, child -> checkedInt(child.readUInt(32), "hop_count"));
    final int encodedBytes =
        readField(decoder, child -> checkedInt(child.readUInt(32), "encoded_bytes"));
    final String reason = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final boolean chainAdmissible =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBool);
    final String chainAdmissionReason =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final boolean witnesslessRedeemSupported =
        decoder.remaining() == 0
            ? false
            : readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBool);
    final boolean lineageWitnessRequired =
        decoder.remaining() == 0
            ? false
            : readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBool);
    require(decoder.remaining() == 0, "Trailing bytes after verify result");
    return new VerifySpendResult(
        valid,
        hopCount,
        encodedBytes,
        reason,
        chainAdmissible,
        chainAdmissionReason,
        witnesslessRedeemSupported,
        lineageWitnessRequired);
  }

  public static SpendBundleSummary decodeBundle(final byte[] archive) {
    final ArchivePayload payload = requirePayloadArchive(archive, SCHEMA_BUNDLE, "bundle");
    require(payload.flags == REQUEST_FLAGS, "bundle must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(payload.payload, payload.flags);
    final byte[] accumulatorPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    final byte[] proofPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    require(decoder.remaining() == 0, "Trailing bytes after bundle");

    final AccumulatorSummary accumulator = readAccumulatorSummary(accumulatorPayload, payload.flags);
    final String proofCircuitId = readRecursiveProofCircuitId(proofPayload, payload.flags);
    require(
        KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(proofCircuitId),
        "bundle.proof_circuit_id unsupported recursive proof circuit id");
    return new SpendBundleSummary(
        accumulator.hopCount,
        proofCircuitId,
        accumulator.asset,
        accumulator.chainId,
        accumulator.initialRoot,
        accumulator.finalRoot,
        accumulator.topupAnchorNullifiers,
        accumulator.currentNote);
  }

  static boolean lineageWitnessHasReservedPreviousProof(final byte[] archive) {
    final ArchivePayload payload = requirePayloadArchive(archive, SCHEMA_LINEAGE_WITNESS, "lineageWitness");
    require(payload.flags == REQUEST_FLAGS, "lineageWitness must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(payload.payload, payload.flags);
    skipFields(decoder, 3);
    final byte[] previousProofsPayload =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    require(decoder.remaining() == 0, "Trailing bytes after lineageWitness");
    final NoritoDecoder previousProofs = new NoritoDecoder(previousProofsPayload, payload.flags);
    final int count = checkedInt(previousProofs.readUInt(64), "lineageWitness.previousRecursiveProofs count");
    boolean hasReserved = false;
    for (int index = 0; index < count; index++) {
      final int itemLength =
          checkedInt(
              previousProofs.readLength(compact(previousProofs)),
              "lineageWitness.previousRecursiveProofs[" + index + "] length");
      final byte[] proofPayload = previousProofs.readBytes(itemLength);
      final String circuitId = readPreviousRecursiveProofCircuitId(proofPayload, payload.flags);
      hasReserved = hasReserved || KagemushaRecursiveSpendProver.isLineageProofCircuitId(circuitId);
    }
    require(previousProofs.remaining() == 0, "Trailing bytes after lineageWitness.previousRecursiveProofs");
    return hasReserved;
  }

  private static String readPreviousRecursiveProofCircuitId(final byte[] payload, final int flags) {
    final String circuitId =
        readRecursiveProofCircuitId(payload, flags, RecursiveProofDecodeContext.LINEAGE_PREVIOUS_PROOF);
    require(
        KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(circuitId),
        "lineageWitness.previousRecursiveProofs verifierKeyId unsupported recursive proof circuit id");
    return circuitId;
  }

  static ArchivePayload requirePayloadArchive(
      final byte[] archive, final String schema, final String field) {
    require(archive != null && archive.length > 0, field + " must not be empty");
    require(
        archive.length <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        field + " must not exceed " + KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + " bytes");
    final NoritoHeader.DecodeResult decoded;
    try {
      decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema));
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be a valid " + schema + " Norito archive", ex);
    }
    require(decoded.header().compression() == NoritoHeader.COMPRESSION_NONE, field + " must not be compressed");
    require(decoded.header().payloadLength() > 0, field + " must contain a non-empty Norito payload");
    decoded.header().validateChecksum(decoded.payload());
    return new ArchivePayload(decoded.payload(), decoded.header().flags());
  }

  static byte[] compactPayloadForRequest(
      final byte[] archive, final String schema, final String field) {
    final ArchivePayload payload = requirePayloadArchive(archive, schema, field);
    require(payload.flags == REQUEST_FLAGS, field + " must use compact Norito layout");
    return payload.payload;
  }

  private static byte[] failClosedProofOnlyRecursiveSpendRequest() {
    throw new IllegalArgumentException(
        "recursive spend requests require explicit VerifiedFoldHopEvidence and a bridge-generated or explicit "
            + "Pallas open-envelopes archive; privacy proof outputs alone do not carry "
            + "Pallas IPA opening envelopes, chainId, asset, or rootAfter");
  }

  private static void preflightInitLineageKeyMaterialForAutoGeneration(
      final byte[] lineageVerifierKey, final byte[] lineageProvingKeyArchive) {
    require(lineageVerifierKey != null, "lineageVerifierKey is required for recursive spend init");
    require(lineageProvingKeyArchive != null,
        "lineageProvingKeyArchive is required for recursive spend init");
    require(lineageVerifierKey.length > 0, "lineageVerifierKey must not be empty");
    require(lineageProvingKeyArchive.length > 0, "lineageProvingKeyArchive must not be empty");
    validateLineageKeyArtifactsForInit(lineageVerifierKey, lineageProvingKeyArchive);
  }

  private static SpendBundleSummary preflightAppendPreviousLineageForAutoGeneration(
      final byte[] previousBundle,
      final String outputCircuitId,
      final VerifierRecordRef previousLineageVerifierRecord) {
    final SpendBundleSummary previousSummary = decodeBundle(previousBundle);
    final String normalizedOutput =
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputCircuitId);
    require(
        KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
            previousSummary.proofCircuitId, normalizedOutput, previousSummary.hopCount),
        "outputProofCircuitId is not valid for the previous bundle");
    final boolean appendNeedsPreviousLineageVerifierRecord =
        KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
            previousSummary.proofCircuitId);
    if (appendNeedsPreviousLineageVerifierRecord) {
      require(
          previousLineageVerifierRecord != null,
          "previousLineageVerifierRecord is required for lineage previous bundles");
    } else {
      require(
          previousLineageVerifierRecord == null,
          "previousLineageVerifierRecord is only valid for lineage previous bundles");
    }
    return previousSummary;
  }

  private static void preflightAppendLineageKeyMaterialForAutoGeneration(
      final String outputCircuitId,
      final byte[] lineageVerifierKey,
      final byte[] lineageProvingKeyArchive) {
    final String normalizedOutput =
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputCircuitId);
    final boolean appendNeedsLineageKeyArtifacts =
        KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput);
    final boolean suppliedLineageKeyMaterial =
        lineageVerifierKey != null || lineageProvingKeyArchive != null;
    require(!suppliedLineageKeyMaterial || appendNeedsLineageKeyArtifacts,
        "lineageKeyArtifacts are only valid for lineage append output");
    if (appendNeedsLineageKeyArtifacts) {
      require(lineageVerifierKey != null && lineageVerifierKey.length > 0,
          "lineageVerifierKey is required for lineage append output");
      require(lineageProvingKeyArchive != null && lineageProvingKeyArchive.length > 0,
          "lineageProvingKeyArchive is required for lineage append output");
      requireValidNestedArchive(lineageProvingKeyArchive, "lineageProvingKeyArchive");
      validateLineageKeyArtifactsForAppend(lineageVerifierKey, lineageProvingKeyArchive);
    }
  }

  private static KagemushaRecursiveSpendProver.LineageKeyArtifacts
      preflightAppendLineageKeyArtifactsForAutoGeneration(
          final String outputCircuitId,
          final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts) {
    final String normalizedOutput =
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputCircuitId);
    final boolean appendNeedsLineageKeyArtifacts =
        KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput);
    require(lineageKeyArtifacts == null || appendNeedsLineageKeyArtifacts,
        "lineageKeyArtifacts are only valid for lineage append output");
    if (!appendNeedsLineageKeyArtifacts) {
      return null;
    }
    require(lineageKeyArtifacts != null,
        "lineageKeyArtifacts are required for lineage append output");
    return requireAppendLineageKeyArtifacts(lineageKeyArtifacts);
  }

  private static byte[] previousProofOpenEnvelopesOrGenerated(
      final byte[] previousBundle,
      final String outputCircuitId,
      final SpendBundleSummary previousSummary,
      final byte[] provided) {
    if (provided != null) {
      return provided;
    }
    if (KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        outputCircuitId, previousSummary.hopCount)) {
      return buildPreviousProofOpenEnvelopesArchive(previousBundle);
    }
    return null;
  }

  static final class ArchivePayload {
    final byte[] payload;
    final int flags;

    ArchivePayload(final byte[] payload, final int flags) {
      this.payload = Arrays.copyOf(payload, payload.length);
      this.flags = flags;
    }
  }

  /** Spendable note descriptor carried by recursive Kagemusha spend requests and bundles. */
  public static final class SpendableNoteDescriptor {
    private final byte[] noteCommitment;
    private final byte[] spendNullifier;
    public final String amount;

    public SpendableNoteDescriptor(
        final byte[] noteCommitment, final byte[] spendNullifier, final String amount) {
      this.noteCommitment = fixedBytes(noteCommitment, 32, "noteCommitment");
      this.spendNullifier = fixedBytes(spendNullifier, 32, "spendNullifier");
      this.amount = canonicalU128Decimal(amount, "amount");
      require(!isZero(this.noteCommitment), "noteCommitment must be non-zero");
      require(!isZero(this.spendNullifier), "spendNullifier must be non-zero");
      require(!Arrays.equals(this.noteCommitment, this.spendNullifier),
          "spendNullifier must differ from noteCommitment");
    }

    public byte[] noteCommitment() {
      return Arrays.copyOf(noteCommitment, noteCommitment.length);
    }

    public byte[] spendNullifier() {
      return Arrays.copyOf(spendNullifier, spendNullifier.length);
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof SpendableNoteDescriptor)) {
        return false;
      }
      final SpendableNoteDescriptor rhs = (SpendableNoteDescriptor) other;
      return Arrays.equals(noteCommitment, rhs.noteCommitment)
          && Arrays.equals(spendNullifier, rhs.spendNullifier)
          && amount.equals(rhs.amount);
    }

    @Override
    public int hashCode() {
      int result = Arrays.hashCode(noteCommitment);
      result = 31 * result + Arrays.hashCode(spendNullifier);
      result = 31 * result + amount.hashCode();
      return result;
    }
  }

  /** Active verifier-record archive paired with the verifier-key registry id used to fetch it. */
  public static final class VerifierRecordRef {
    public final String verifierKeyId;
    private final byte[] recordBytes;

    public VerifierRecordRef(final String verifierKeyId, final byte[] recordBytes) {
      requirePortableId(verifierKeyId, "verifierKeyId");
      this.verifierKeyId = verifierKeyId;
      this.recordBytes = Arrays.copyOf(Objects.requireNonNull(recordBytes, "recordBytes"), recordBytes.length);
      require(this.recordBytes.length > 0, "recordBytes must not be empty");
      compactPayloadForRequest(this.recordBytes, SCHEMA_VERIFYING_KEY_RECORD, "recordBytes");
    }

    public byte[] recordBytes() {
      return Arrays.copyOf(recordBytes, recordBytes.length);
    }
  }

  /** One checked private hop plus the chain context that proof-output archives do not carry. */
  public static final class VerifiedFoldHopEvidence {
    private final byte[] proofOutputArchive;
    public final VerifierRecordRef verifierRecord;
    public final String chainId;
    public final String asset;
    private final byte[] rootAfter;

    public VerifiedFoldHopEvidence(
        final byte[] proofOutputArchive,
        final VerifierRecordRef verifierRecord,
        final String chainId,
        final String asset,
        final byte[] rootAfter) {
      this.proofOutputArchive = copyOf(proofOutputArchive, "proofOutputArchive");
      this.verifierRecord = Objects.requireNonNull(verifierRecord, "verifierRecord");
      this.chainId = Objects.requireNonNull(chainId, "chainId");
      this.asset = Objects.requireNonNull(asset, "asset");
      this.rootAfter = fixedBytes(rootAfter, 32, "rootAfter");
      require(this.proofOutputArchive.length > 0, "proofOutputArchive must not be empty");
      requirePortableId(this.chainId, "chainId");
      try {
        AssetDefinitionIdEncoder.parseAddressBytes(this.asset);
      } catch (final IllegalArgumentException ex) {
        throw new IllegalArgumentException("asset must be a canonical asset definition id", ex);
      }
      require(!isZero(this.rootAfter), "rootAfter must be non-zero");
    }

    public byte[] proofOutputArchive() {
      return Arrays.copyOf(proofOutputArchive, proofOutputArchive.length);
    }

    public byte[] rootAfter() {
      return Arrays.copyOf(rootAfter, rootAfter.length);
    }
  }

  /** Typed request for {@code KagemushaRecursiveSpendInitRequestV1}. */
  public static final class InitSpendRequest {
    private final byte[] recordBundle;
    private final byte[] pallasOpenEnvelopes;
    public final SpendableNoteDescriptor currentNote;
    private final byte[] lineageVerifierKey;
    private final byte[] lineageProvingKeyArchive;
    public final Long blockHeight;

    public InitSpendRequest(
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
        final Long blockHeight) {
      this(
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          lineageKeyMaterialForInit(lineageKeyArtifacts),
          blockHeight);
    }

    private InitSpendRequest(
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final LineageKeyMaterial lineageKeyMaterial,
        final Long blockHeight) {
      this(
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          lineageKeyMaterial.verifierKey,
          lineageKeyMaterial.provingKeyArchive,
          blockHeight);
    }

    public InitSpendRequest(
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final byte[] lineageVerifierKey,
        final byte[] lineageProvingKeyArchive,
        final Long blockHeight) {
      this.recordBundle = copyOf(recordBundle, "recordBundle");
      this.pallasOpenEnvelopes = copyOf(pallasOpenEnvelopes, "pallasOpenEnvelopes");
      this.currentNote = Objects.requireNonNull(currentNote, "currentNote");
      this.lineageVerifierKey = copyNullable(lineageVerifierKey);
      this.lineageProvingKeyArchive = copyNullable(lineageProvingKeyArchive);
      this.blockHeight = blockHeight;
      requireNonNegativeHeight(blockHeight);
      require(this.lineageVerifierKey != null, "lineageVerifierKey is required for recursive spend init");
      require(this.lineageVerifierKey.length > 0, "lineageVerifierKey must not be empty");
      require(this.lineageProvingKeyArchive != null,
          "lineageProvingKeyArchive is required for recursive spend init");
      require(this.lineageProvingKeyArchive.length > 0, "lineageProvingKeyArchive must not be empty");
      final byte[] recordBundlePayload =
          compactPayloadForRequest(this.recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle");
      final int recordBundleHopCount =
          readVerifiedFoldRecordBundleHopCount(recordBundlePayload, REQUEST_FLAGS, "recordBundle");
      requirePallasOpenEnvelopesArchive(
          this.pallasOpenEnvelopes,
          recordBundleHopCount,
          "pallasOpenEnvelopes",
          KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES);
      validateLineageKeyArtifactsForInit(this.lineageVerifierKey, this.lineageProvingKeyArchive);
    }

    public byte[] recordBundle() {
      return Arrays.copyOf(recordBundle, recordBundle.length);
    }

    public byte[] pallasOpenEnvelopes() {
      return Arrays.copyOf(pallasOpenEnvelopes, pallasOpenEnvelopes.length);
    }

    public byte[] lineageVerifierKey() {
      return copyNullable(lineageVerifierKey);
    }

    public byte[] lineageProvingKeyArchive() {
      return copyNullable(lineageProvingKeyArchive);
    }
  }

  /** Typed request for {@code KagemushaRecursiveSpendAppendRequestV1}. */
  public static final class AppendSpendRequest {
    private final byte[] previousBundle;
    private final byte[] recordBundle;
    private final byte[] pallasOpenEnvelopes;
    public final SpendableNoteDescriptor currentNote;
    public final String outputProofCircuitId;
    public final VerifierRecordRef previousLineageVerifierRecord;
    private final byte[] previousProofOpenEnvelopes;
    private final byte[] lineageVerifierKey;
    private final byte[] lineageProvingKeyArchive;
    public final Long blockHeight;

    public AppendSpendRequest(
        final byte[] previousBundle,
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final String outputProofCircuitId,
        final VerifierRecordRef previousLineageVerifierRecord,
        final byte[] previousProofOpenEnvelopes,
        final KagemushaRecursiveSpendProver.LineageKeyArtifacts lineageKeyArtifacts,
        final Long blockHeight) {
      this(
          previousBundle,
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          outputProofCircuitId,
          previousLineageVerifierRecord,
          previousProofOpenEnvelopes,
          lineageKeyMaterialForAppend(lineageKeyArtifacts),
          blockHeight);
    }

    private AppendSpendRequest(
        final byte[] previousBundle,
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final String outputProofCircuitId,
        final VerifierRecordRef previousLineageVerifierRecord,
        final byte[] previousProofOpenEnvelopes,
        final LineageKeyMaterial lineageKeyMaterial,
        final Long blockHeight) {
      this(
          previousBundle,
          recordBundle,
          pallasOpenEnvelopes,
          currentNote,
          outputProofCircuitId,
          previousLineageVerifierRecord,
          previousProofOpenEnvelopes,
          lineageKeyMaterial.verifierKey,
          lineageKeyMaterial.provingKeyArchive,
          blockHeight);
    }

    public AppendSpendRequest(
        final byte[] previousBundle,
        final byte[] recordBundle,
        final byte[] pallasOpenEnvelopes,
        final SpendableNoteDescriptor currentNote,
        final String outputProofCircuitId,
        final VerifierRecordRef previousLineageVerifierRecord,
        final byte[] previousProofOpenEnvelopes,
        final byte[] lineageVerifierKey,
        final byte[] lineageProvingKeyArchive,
        final Long blockHeight) {
      this.previousBundle = copyOf(previousBundle, "previousBundle");
      this.recordBundle = copyOf(recordBundle, "recordBundle");
      this.pallasOpenEnvelopes = copyOf(pallasOpenEnvelopes, "pallasOpenEnvelopes");
      this.currentNote = Objects.requireNonNull(currentNote, "currentNote");
      this.outputProofCircuitId = outputProofCircuitId;
      this.previousLineageVerifierRecord = previousLineageVerifierRecord;
      this.previousProofOpenEnvelopes = copyNullable(previousProofOpenEnvelopes);
      this.lineageVerifierKey = copyNullable(lineageVerifierKey);
      this.lineageProvingKeyArchive = copyNullable(lineageProvingKeyArchive);
      this.blockHeight = blockHeight;
      requireNonNegativeHeight(blockHeight);
      final byte[] recordBundlePayload =
          compactPayloadForRequest(this.recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle");
      final int recordBundleHopCount =
          readVerifiedFoldRecordBundleHopCount(recordBundlePayload, REQUEST_FLAGS, "recordBundle");
      requirePallasOpenEnvelopesArchive(
          this.pallasOpenEnvelopes,
          recordBundleHopCount,
          "pallasOpenEnvelopes",
          KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES);
      final SpendBundleSummary previousSummary = decodeBundle(this.previousBundle);
      final String normalizedOutput =
          KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputProofCircuitId);
      final boolean appendNeedsPreviousProofOpenEnvelopes =
          KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
              normalizedOutput, previousSummary.hopCount);
      final boolean appendNeedsPreviousLineageVerifierRecord =
          KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
              previousSummary.proofCircuitId);
      final boolean appendNeedsLineageKeyArtifacts =
          KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput);
      require(
          KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
              previousSummary.proofCircuitId, normalizedOutput, previousSummary.hopCount),
          "outputProofCircuitId is not valid for the previous bundle");
      final boolean suppliedLineageKeyMaterial =
          lineageVerifierKey != null || lineageProvingKeyArchive != null;
      require(!suppliedLineageKeyMaterial || appendNeedsLineageKeyArtifacts,
          "lineageKeyArtifacts are only valid for lineage append output");
      if (appendNeedsPreviousLineageVerifierRecord) {
        require(previousLineageVerifierRecord != null,
            "previousLineageVerifierRecord is required for lineage previous bundles");
      } else {
        require(previousLineageVerifierRecord == null,
            "previousLineageVerifierRecord is only valid for lineage previous bundles");
      }
      require(this.previousProofOpenEnvelopes == null || appendNeedsPreviousProofOpenEnvelopes,
          "previousProofOpenEnvelopes are only valid for lineage append output");
      if (this.previousProofOpenEnvelopes != null) {
        requirePreviousProofOpenEnvelopesArchive(this.previousProofOpenEnvelopes);
      }
      if (appendNeedsPreviousProofOpenEnvelopes) {
        require(previousProofOpenEnvelopes != null,
            "previousProofOpenEnvelopes is required for lineage append output");
      }
      if (appendNeedsLineageKeyArtifacts) {
        require(lineageVerifierKey != null && lineageVerifierKey.length > 0,
            "lineageVerifierKey is required for lineage append output");
        require(lineageProvingKeyArchive != null && lineageProvingKeyArchive.length > 0,
            "lineageProvingKeyArchive is required for lineage append output");
        requireValidNestedArchive(lineageProvingKeyArchive, "lineageProvingKeyArchive");
        validateLineageKeyArtifactsForAppend(lineageVerifierKey, lineageProvingKeyArchive);
      }
    }

    public byte[] previousBundle() {
      return Arrays.copyOf(previousBundle, previousBundle.length);
    }

    public byte[] recordBundle() {
      return Arrays.copyOf(recordBundle, recordBundle.length);
    }

    public byte[] pallasOpenEnvelopes() {
      return Arrays.copyOf(pallasOpenEnvelopes, pallasOpenEnvelopes.length);
    }

    public byte[] previousProofOpenEnvelopes() {
      return copyNullable(previousProofOpenEnvelopes);
    }

    public byte[] lineageVerifierKey() {
      return copyNullable(lineageVerifierKey);
    }

    public byte[] lineageProvingKeyArchive() {
      return copyNullable(lineageProvingKeyArchive);
    }
  }

  /** Typed request for {@code KagemushaRecursiveSpendVerifyRequestV1}. */
  public static final class VerifySpendRequest {
    private final byte[] bundle;
    public final VerifierRecordRef lineageVerifierRecord;
    public final Long blockHeight;

    public VerifySpendRequest(
        final byte[] bundle, final VerifierRecordRef lineageVerifierRecord, final Long blockHeight) {
      this.bundle = copyOf(bundle, "bundle");
      this.lineageVerifierRecord = lineageVerifierRecord;
      this.blockHeight = blockHeight;
      requireNonNegativeHeight(blockHeight);
      final SpendBundleSummary bundleSummary = decodeBundle(this.bundle);
      require(
          !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
              || this.lineageVerifierRecord != null,
          "lineageVerifierRecord is required for reserved-lineage bundles");
      require(
          KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
              || this.lineageVerifierRecord == null,
          "lineageVerifierRecord is only valid for reserved-lineage bundles");
    }

    public byte[] bundle() {
      return Arrays.copyOf(bundle, bundle.length);
    }
  }

  /** Decoded {@code KagemushaRecursiveSpendVerifyResultV1}. */
  public static final class VerifySpendResult {
    public final boolean valid;
    public final int hopCount;
    public final int encodedBytes;
    public final String reason;
    public final boolean chainAdmissible;
    public final String chainAdmissionReason;
    public final boolean witnesslessRedeemSupported;
    public final boolean lineageWitnessRequired;

    public VerifySpendResult(
        final boolean valid,
        final int hopCount,
        final int encodedBytes,
        final String reason,
        final boolean chainAdmissible,
        final String chainAdmissionReason,
        final boolean witnesslessRedeemSupported,
        final boolean lineageWitnessRequired) {
      this.valid = valid;
      this.hopCount = hopCount;
      this.encodedBytes = encodedBytes;
      this.reason = Objects.requireNonNull(reason, "reason");
      this.chainAdmissible = chainAdmissible;
      this.chainAdmissionReason = Objects.requireNonNull(chainAdmissionReason, "chainAdmissionReason");
      this.witnesslessRedeemSupported = witnesslessRedeemSupported;
      this.lineageWitnessRequired = lineageWitnessRequired;
    }
  }

  /** Typed request for {@code KagemushaRecursiveSpendRedeemRequestV1}. */
  public static final class RedeemSpendRequest {
    private final byte[] bundle;
    public final String recipient;
    public final String publicAmount;
    private final byte[] redeemProof;
    private final byte[] lineageWitness;
    private final byte[] changeOutput;
    public final VerifierRecordRef lineageVerifierRecord;
    public final Long blockHeight;

    public RedeemSpendRequest(
        final byte[] bundle,
        final String recipient,
        final String publicAmount,
        final byte[] redeemProof,
        final byte[] lineageWitness,
        final VerifierRecordRef lineageVerifierRecord,
        final Long blockHeight) {
      this(
          bundle,
          recipient,
          publicAmount,
          redeemProof,
          lineageWitness,
          null,
          lineageVerifierRecord,
          blockHeight);
    }

    public RedeemSpendRequest(
        final byte[] bundle,
        final String recipient,
        final String publicAmount,
        final byte[] redeemProof,
        final byte[] lineageWitness,
        final byte[] changeOutput,
        final VerifierRecordRef lineageVerifierRecord,
        final Long blockHeight) {
      this.bundle = copyOf(bundle, "bundle");
      this.recipient = Objects.requireNonNull(recipient, "recipient");
      this.publicAmount = canonicalU128Decimal(publicAmount, "publicAmount");
      this.redeemProof = copyOf(redeemProof, "redeemProof");
      this.lineageWitness = copyNullable(lineageWitness);
      this.changeOutput = changeOutput == null ? null : fixedBytes(changeOutput, 32, "changeOutput");
      if (this.changeOutput != null) {
        require(!isZero(this.changeOutput), "changeOutput must be non-zero");
      }
      this.lineageVerifierRecord = lineageVerifierRecord;
      this.blockHeight = blockHeight;
      requireNonNegativeHeight(blockHeight);
      requireNonBlankUnpadded(this.recipient, "recipient");
      final SpendBundleSummary bundleSummary = decodeBundle(this.bundle);
      requireRedeemChangeBinding(
          this.publicAmount,
          bundleSummary.currentNote.amount,
          this.changeOutput != null);
      if (this.changeOutput != null) {
        requireRedeemChangeOutputNotReserved(this.changeOutput, bundleSummary);
      }
      final boolean finalIsLineage =
          KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId);
      final boolean witnessHasReservedPrevious =
          this.lineageWitness != null
              && lineageWitnessHasReservedPreviousProof(this.lineageWitness);
      if (!finalIsLineage) {
        require(
            !witnessHasReservedPrevious || this.lineageVerifierRecord != null,
            "lineageVerifierRecord is required for lineage witnesses with reserved-lineage previous proofs");
        require(
            witnessHasReservedPrevious || this.lineageVerifierRecord == null,
            "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses");
      }
      require(
          !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
              bundleSummary.proofCircuitId, bundleSummary.hopCount)
              || this.lineageWitness != null,
          "lineageWitness is required for this bundle");
      require(
          !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
              || this.lineageVerifierRecord != null,
          "lineageVerifierRecord is required for reserved-lineage bundles");
      compactPayloadForRequest(this.redeemProof, SCHEMA_PROOF_ATTACHMENT, "redeemProof");
      if (this.lineageWitness != null) {
        compactPayloadForRequest(this.lineageWitness, SCHEMA_LINEAGE_WITNESS, "lineageWitness");
      }
    }

    public byte[] bundle() {
      return Arrays.copyOf(bundle, bundle.length);
    }

    public byte[] redeemProof() {
      return Arrays.copyOf(redeemProof, redeemProof.length);
    }

    public byte[] lineageWitness() {
      return copyNullable(lineageWitness);
    }

    public byte[] changeOutput() {
      return copyNullable(changeOutput);
    }
  }

  /** Read-only summary of a recursive spend bundle. */
  public static final class SpendBundleSummary {
    public final int hopCount;
    public final String proofCircuitId;
    public final String asset;
    public final String chainId;
    private final byte[] initialRoot;
    private final byte[] finalRoot;
    private final List<byte[]> topupAnchorNullifiers;
    public final SpendableNoteDescriptor currentNote;

    public SpendBundleSummary(
        final int hopCount,
        final String proofCircuitId,
        final String asset,
        final String chainId,
        final byte[] initialRoot,
        final byte[] finalRoot,
        final List<byte[]> topupAnchorNullifiers,
        final SpendableNoteDescriptor currentNote) {
      this.hopCount = hopCount;
      this.proofCircuitId = Objects.requireNonNull(proofCircuitId, "proofCircuitId");
      this.asset = Objects.requireNonNull(asset, "asset");
      this.chainId = Objects.requireNonNull(chainId, "chainId");
      requirePortableId(this.chainId, "chainId");
      this.initialRoot = fixedBytes(initialRoot, 32, "initialRoot");
      this.finalRoot = fixedBytes(finalRoot, 32, "finalRoot");
      final ArrayList<byte[]> copiedTopupAnchorNullifiers = new ArrayList<>();
      for (final byte[] value : Objects.requireNonNull(topupAnchorNullifiers, "topupAnchorNullifiers")) {
        copiedTopupAnchorNullifiers.add(fixedBytes(value, 32, "topupAnchorNullifier"));
      }
      this.topupAnchorNullifiers = Collections.unmodifiableList(copiedTopupAnchorNullifiers);
      this.currentNote = Objects.requireNonNull(currentNote, "currentNote");
    }

    public byte[] initialRoot() {
      return Arrays.copyOf(initialRoot, initialRoot.length);
    }

    public byte[] finalRoot() {
      return Arrays.copyOf(finalRoot, finalRoot.length);
    }

    public List<byte[]> topupAnchorNullifiers() {
      final ArrayList<byte[]> copied = new ArrayList<>();
      for (final byte[] value : topupAnchorNullifiers) {
        copied.add(Arrays.copyOf(value, value.length));
      }
      return Collections.unmodifiableList(copied);
    }
  }

  private static final TypeAdapter<InitSpendRequest> INIT_REQUEST_ADAPTER =
      new TypeAdapter<InitSpendRequest>() {
        @Override
        public void encode(final NoritoEncoder encoder, final InitSpendRequest value) {
          writeRawField(
              encoder,
              compactPayloadForRequest(value.recordBundle(), SCHEMA_RECORD_BUNDLE, "recordBundle"));
          writeField(encoder, child -> writeBytesVec(child, value.pallasOpenEnvelopes()));
          writeField(encoder, child -> writeSpendableNote(child, value.currentNote));
          writeField(encoder, child -> writeOptionRaw(child, verifyingKeyBoxPayload(value.lineageVerifierKey())));
          writeField(encoder, child -> writeOptionBytesVec(child, value.lineageProvingKeyArchive()));
          writeField(encoder, child -> writeOptionU64(child, value.blockHeight));
        }

        @Override
        public InitSpendRequest decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("recursive spend requests are encode-only");
        }
      };

  private static final TypeAdapter<AppendSpendRequest> APPEND_REQUEST_ADAPTER =
      new TypeAdapter<AppendSpendRequest>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AppendSpendRequest value) {
          writeRawField(encoder, compactPayloadForRequest(value.previousBundle(), SCHEMA_BUNDLE, "previousBundle"));
          writeRawField(encoder, compactPayloadForRequest(value.recordBundle(), SCHEMA_RECORD_BUNDLE, "recordBundle"));
          writeField(encoder, child -> writeBytesVec(child, value.pallasOpenEnvelopes()));
          writeField(encoder, child -> writeSpendableNote(child, value.currentNote));
          writeField(
              encoder,
              child -> {
                final String normalized =
                    KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(value.outputProofCircuitId);
                writeString(
                    child,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(normalized)
                        ? ""
                        : normalized);
              });
          writeField(
              encoder,
              child ->
                  writeOptionRaw(
                      child,
                      value.previousLineageVerifierRecord == null
                          ? null
                          : compactPayloadForRequest(
                              value.previousLineageVerifierRecord.recordBytes(),
                              SCHEMA_VERIFYING_KEY_RECORD,
                              "previousLineageVerifierRecord")));
          writeField(
              encoder,
              child ->
                  writeBytesVec(
                      child,
                      value.previousProofOpenEnvelopes() == null
                          ? new byte[0]
                          : value.previousProofOpenEnvelopes()));
          writeField(
              encoder,
              child -> writeOptionRaw(child, value.lineageVerifierKey() == null ? null : verifyingKeyBoxPayload(value.lineageVerifierKey())));
          writeField(encoder, child -> writeOptionBytesVec(child, value.lineageProvingKeyArchive()));
          writeField(encoder, child -> writeOptionU64(child, value.blockHeight));
        }

        @Override
        public AppendSpendRequest decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("recursive spend requests are encode-only");
        }
      };

  private static final TypeAdapter<VerifySpendRequest> VERIFY_REQUEST_ADAPTER =
      new TypeAdapter<VerifySpendRequest>() {
        @Override
        public void encode(final NoritoEncoder encoder, final VerifySpendRequest value) {
          writeRawField(encoder, compactPayloadForRequest(value.bundle(), SCHEMA_BUNDLE, "bundle"));
          writeField(
              encoder,
              child ->
                  writeOptionRaw(
                      child,
                      value.lineageVerifierRecord == null
                          ? null
                          : compactPayloadForRequest(
                              value.lineageVerifierRecord.recordBytes(),
                              SCHEMA_VERIFYING_KEY_RECORD,
                              "lineageVerifierRecord")));
          writeField(encoder, child -> writeOptionU64(child, value.blockHeight));
        }

        @Override
        public VerifySpendRequest decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("recursive spend requests are encode-only");
        }
      };

  private static final TypeAdapter<RedeemSpendRequest> REDEEM_REQUEST_ADAPTER =
      new TypeAdapter<RedeemSpendRequest>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RedeemSpendRequest value) {
          writeRawField(encoder, compactPayloadForRequest(value.bundle(), SCHEMA_BUNDLE, "bundle"));
          writeField(encoder, child -> writeAccountId(child, value.recipient));
          writeField(encoder, child -> writeU128(child, value.publicAmount));
          writeRawField(
              encoder,
              compactPayloadForRequest(value.redeemProof(), SCHEMA_PROOF_ATTACHMENT, "redeemProof"));
          writeField(
              encoder,
              child ->
                  writeOptionRaw(
                      child,
                      value.lineageWitness() == null
                          ? null
                          : compactPayloadForRequest(
                              value.lineageWitness(), SCHEMA_LINEAGE_WITNESS, "lineageWitness")));
          writeField(encoder, child -> writeOptionFixed32(child, value.changeOutput()));
          writeField(
              encoder,
              child ->
                  writeOptionRaw(
                      child,
                      value.lineageVerifierRecord == null
                          ? null
                          : compactPayloadForRequest(
                              value.lineageVerifierRecord.recordBytes(),
                              SCHEMA_VERIFYING_KEY_RECORD,
                              "lineageVerifierRecord")));
          writeField(encoder, child -> writeOptionU64(child, value.blockHeight));
        }

        @Override
        public RedeemSpendRequest decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("recursive spend requests are encode-only");
        }
      };

  private static final class AccumulatorSummary {
    final String chainId;
    final String asset;
    final byte[] initialRoot;
    final byte[] finalRoot;
    final List<byte[]> topupAnchorNullifiers;
    final int hopCount;
    final SpendableNoteDescriptor currentNote;

    AccumulatorSummary(
        final String chainId,
        final String asset,
        final byte[] initialRoot,
        final byte[] finalRoot,
        final List<byte[]> topupAnchorNullifiers,
        final int hopCount,
        final SpendableNoteDescriptor currentNote) {
      this.chainId = chainId;
      this.asset = asset;
      this.initialRoot = initialRoot;
      this.finalRoot = finalRoot;
      this.topupAnchorNullifiers = topupAnchorNullifiers;
      this.hopCount = hopCount;
      this.currentNote = currentNote;
    }
  }

  private static AccumulatorSummary readAccumulatorSummary(final byte[] payload, final int flags) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final String domain = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    require(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN.equals(domain),
        "bundle.accumulator.domain must be "
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN);
    final String chainId =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readAccumulatorChainId);
    final String asset = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readAssetDefinitionId);
    final byte[] initialRoot = readField(decoder, child -> readFixedBytes(child, 32, "initial_root"));
    final byte[] finalRoot = readField(decoder, child -> readFixedBytes(child, 32, "final_root"));
    requireAccumulatorRoots(initialRoot, finalRoot);
    final List<byte[]> topupAnchorNullifiers =
        readField(decoder, child -> readFixed32Sequence(child, "bundle.accumulator.topup_anchor_nullifiers"));
    final int hopCount =
        readField(
            decoder,
            "bundle.accumulator.hop_count",
            child -> checkedInt(child.readUInt(32), "hop_count"));
    require(
        hopCount >= 1
            && hopCount
                <= KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
        "bundle.accumulator.hop_count must be in 1.."
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1);
    requireAccumulatorCorridor(decoder, hopCount);
    final SpendableNoteDescriptor currentNote =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readSpendableNote);
    requireTopupAnchorNullifiers(topupAnchorNullifiers, currentNote);
    require(decoder.remaining() == 0, "Trailing bytes after accumulator");
    return new AccumulatorSummary(
        chainId,
        asset,
        initialRoot,
        finalRoot,
        topupAnchorNullifiers,
        hopCount,
        currentNote);
  }

  private static String readRecursiveProofCircuitId(final byte[] payload, final int flags) {
    return readRecursiveProofCircuitId(payload, flags, RecursiveProofDecodeContext.BUNDLE);
  }

  private static String readRecursiveProofCircuitId(
      final byte[] payload, final int flags, final RecursiveProofDecodeContext context) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final byte[] verifierKeyIdPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    final byte[] publicInputsPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    require(publicInputsPayload.length > 0, context.proofPublicInputsField + " empty recursive proof inputs");
    final byte[] publicInputsHash =
        readField(decoder, child -> readFixedBytes(child, 32, context.proofPublicInputsHashField));
    require(!isZero(publicInputsHash), context.proofPublicInputsHashField + " must be non-zero");
    final byte[] publicInputsArchive =
        NoritoCodec.encode(
            publicInputsPayload,
            SCHEMA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS,
            RAW_PAYLOAD_ADAPTER,
            NoritoHeader.COMPACT_LEN);
    require(Arrays.equals(publicInputsHash, irohaHash(publicInputsArchive)),
        context.proofPublicInputsHashField + " mismatch");
    final byte[] proofPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    require(decoder.remaining() == 0, "Trailing bytes after " + context.trailingField);
    final VerifyingKeyIdParts verifierKeyId = readVerifyingKeyId(verifierKeyIdPayload, flags, context);
    final String proofBackend = readProofBoxBackend(proofPayload, flags, context);
    require(
        verifierKeyId.backend.equals(proofBackend),
        context.proofBackendField + " recursive proof backend mismatch");
    return verifierKeyId.name;
  }

  private static final TypeAdapter<byte[]> RAW_PAYLOAD_ADAPTER =
      new TypeAdapter<byte[]>() {
        @Override
        public void encode(final NoritoEncoder encoder, final byte[] value) {
          encoder.writeBytes(value);
        }

        @Override
        public byte[] decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("raw payload archives are encode-only");
        }
      };

  private static final TypeAdapter<List<PreparedVerifiedFoldHop>> VERIFIED_FOLD_RECORD_BUNDLE_ADAPTER =
      new TypeAdapter<List<PreparedVerifiedFoldHop>>() {
        @Override
        public void encode(final NoritoEncoder encoder, final List<PreparedVerifiedFoldHop> value) {
          writeField(
              encoder,
              bundle -> {
                writeField(bundle, child -> writeChainId(child, value.get(0).chainId));
                writeField(bundle, child -> writeAssetDefinitionId(child, value.get(0).asset));
                writeField(bundle, child -> writeVerifiedFoldSteps(child, value));
              });
          writeField(encoder, child -> writeVerifiedFoldVerifierRecords(child, value));
        }

        @Override
        public List<PreparedVerifiedFoldHop> decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("verified fold bundles are encode-only");
        }
      };

  private static PreparedVerifiedFoldHop prepareTransferHop(final int index, final VerifiedFoldHopEvidence hop) {
    final PrivacyBuildResult proof =
        parsePrivacyBuildResult(
            hop.proofOutputArchive(),
            CONFIDENTIAL_TRANSFER_ALGORITHM_ID,
            CONFIDENTIAL_TRANSFER_ENTRYPOINT,
            "hop " + index + " proofOutputArchive");
    final OpenVerifyEnvelopeValue envelope = decodeOpenVerifyEnvelope(proof.proof, "hop " + index + " proof");
    final TransferPublicInputs publicInputs = parseTransferPublicInputs(envelope.proofBytes, "hop " + index);
    require(!isZero(publicInputs.rootBefore), "hop " + index + " rootBefore must be non-zero");
    final VerifierRecordValue verifierRecord =
        decodeAndValidateVerifierRecord(
            hop.verifierRecord,
            envelope,
            CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA,
            proof.proof.length,
            "hop " + index + " verifierRecord");
    return new PreparedVerifiedFoldHop(
        hop.chainId,
        hop.asset,
        hop.rootAfter(),
        publicInputs,
        envelope,
        verifierRecord);
  }

  private static PrivacyBuildResult parsePrivacyBuildResult(
      final byte[] archive,
      final String expectedAlgorithmId,
      final String expectedEntrypoint,
      final String label) {
    final ArchivePayload payload = requirePrivacyBuildResultPayload(archive, label);
    require(payload.flags == REQUEST_FLAGS, label + " must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(payload.payload, payload.flags);
    final int version = readField(decoder, child -> checkedInt(child.readUInt(32), label + ".version"));
    final int status = readField(decoder, child -> checkedInt(child.readUInt(32), label + ".status"));
    final int errorCode = readField(decoder, child -> checkedInt(child.readUInt(32), label + ".error_code"));
    final String message = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String algorithmId = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String entrypoint = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String vkRef = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final byte[] publicInputs = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    final byte[] proof = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    final boolean verified = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBool);
    require(decoder.remaining() == 0, "Trailing bytes after " + label);
    require(version == PRIVACY_FFI_VERSION_V1, label + " version must be " + PRIVACY_FFI_VERSION_V1);
    require(status == PRIVACY_FFI_STATUS_OK && errorCode == 0,
        label + " must be a successful privacy proof result: status=" + status + " error_code=" + errorCode);
    require(message.isEmpty(), label + " success message must be empty");
    require(expectedAlgorithmId.equals(algorithmId), label + " algorithm_id must be " + expectedAlgorithmId);
    require(expectedEntrypoint.equals(entrypoint), label + " entrypoint must be " + expectedEntrypoint);
    requirePortableId(vkRef, label + ".vk_ref");
    require(publicInputs.length == 0, label + " public_inputs must be empty; envelope carries authoritative inputs");
    require(proof.length > 0, label + " proof must not be empty");
    require(!verified, label + " build results must not claim online verification");
    return new PrivacyBuildResult(proof);
  }

  private static ArchivePayload requirePrivacyBuildResultPayload(final byte[] archive, final String label) {
    require(archive != null && archive.length > 0, label + " must not be empty");
    require(archive.length <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        label + " must not exceed " + KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + " bytes");
    final NoritoHeader.DecodeResult decoded;
    try {
      decoded = NoritoHeader.decode(archive, null);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(label + " must be a valid privacy build-result Norito archive", ex);
    }
    final byte[] schemaHash = decoded.header().schemaHash();
    for (final byte b : schemaHash) {
      require((b & 0xff) == PRIVACY_SCHEMA_BUILD_PROOF_RESULT,
          label + " must use privacy build-result schema marker");
    }
    require(decoded.header().compression() == NoritoHeader.COMPRESSION_NONE, label + " must not be compressed");
    require(decoded.header().payloadLength() > 0, label + " must contain a non-empty Norito payload");
    decoded.header().validateChecksum(decoded.payload());
    return new ArchivePayload(decoded.payload(), decoded.header().flags());
  }

  private static OpenVerifyEnvelopeValue decodeOpenVerifyEnvelope(final byte[] archive, final String label) {
    final ArchivePayload payload = requirePayloadArchive(archive, SCHEMA_OPEN_VERIFY_ENVELOPE, label);
    require(payload.flags == REQUEST_FLAGS, label + " must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(payload.payload, payload.flags);
    final long backendTag = readField(decoder, child -> child.readUInt(32));
    final String circuitId = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final byte[] vkHash = readField(decoder, child -> readFixedBytes(child, 32, "vk_hash"));
    final byte[] publicInputs = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    final byte[] proofBytes = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    final byte[] aux = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    require(decoder.remaining() == 0, "Trailing bytes after " + label + " OpenVerifyEnvelope");
    require(backendTag == BACKEND_TAG_HALO2_IPA_PASTA, label + " backend must be Halo2IpaPasta");
    requirePortableId(circuitId, label + ".circuit_id");
    require(!isZero(vkHash), label + " vk_hash must be non-zero");
    require(proofBytes.length > 0, label + " proof_bytes must not be empty");
    require(aux.length == 0, label + " aux must be empty");
    return new OpenVerifyEnvelopeValue(archive, circuitId, vkHash, publicInputs, proofBytes);
  }

  private static VerifierRecordValue decodeAndValidateVerifierRecord(
      final VerifierRecordRef ref,
      final OpenVerifyEnvelopeValue envelope,
      final String expectedCircuitId,
      final byte[] expectedSchema,
      final int proofArchiveSize,
      final String label) {
    final byte[] recordPayload = compactPayloadForRequest(ref.recordBytes(), SCHEMA_VERIFYING_KEY_RECORD, label);
    final DecodedVerifierRecord record = decodeVerifierRecordPayload(recordPayload, label);
    final VerifierKeyIdValue id = parseVerifierKeyId(ref.verifierKeyId, label + ".verifierKeyId");
    require(ZK_BACKEND_HALO2_IPA.equals(id.backend),
        label + " verifierKeyId backend must be " + ZK_BACKEND_HALO2_IPA);
    require(record.status == CONFIDENTIAL_STATUS_ACTIVE, label + " status must be Active");
    require(KAGEMUSHA_VERIFIER_NAMESPACE.equals(record.namespace),
        label + " namespace must be " + KAGEMUSHA_VERIFIER_NAMESPACE);
    require(record.backendTag == BACKEND_TAG_HALO2_IPA_PASTA, label + " backend must be Halo2IpaPasta");
    require(CONFIDENTIAL_RECORD_CURVE.equals(record.curve), label + " curve must be " + CONFIDENTIAL_RECORD_CURVE);
    require(expectedCircuitId.equals(record.circuitId), label + " circuit_id must be " + expectedCircuitId);
    require(expectedCircuitId.equals(envelope.circuitId), label + " envelope circuit_id must be " + expectedCircuitId);
    require(Arrays.equals(envelope.publicInputs, expectedSchema), label + " public-input schema mismatch");
    require(Arrays.equals(record.publicInputsSchemaHash, Blake2b.digest256(expectedSchema)),
        label + " public_inputs_schema_hash mismatch");
    require(Arrays.equals(record.commitment, envelope.vkHash), label + " commitment must match envelope vk_hash");
    require(record.maxProofBytes > 0, label + " max_proof_bytes must be non-zero");
    require(record.maxProofBytes <= CONFIDENTIAL_V2_MAX_PROOF_BYTES,
        label + " max_proof_bytes exceeds confidential-v2 cap");
    require(proofArchiveSize <= record.maxProofBytes, label + " proof exceeds max_proof_bytes");
    require(record.key != null, label + " must include inline verifier key");
    require(ZK_BACKEND_HALO2_IPA.equals(record.key.backend),
        label + " verifier key backend must be " + ZK_BACKEND_HALO2_IPA);
    require(record.key.bytes.length > 0, label + " verifier key bytes must not be empty");
    require(record.vkLen == record.key.bytes.length, label + " vk_len must equal inline verifier key bytes length");
    require(Arrays.equals(record.commitment, verifyingKeyCommitment(record.key.backend, record.key.bytes)),
        label + " inline verifier-key commitment mismatch");
    return new VerifierRecordValue(id, recordPayload, record.commitment, record.key);
  }

  private static DecodedVerifierRecord decodeVerifierRecordPayload(final byte[] payload, final String label) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, REQUEST_FLAGS);
    readField(decoder, child -> checkedInt(child.readUInt(32), label + ".version"));
    final String circuitId = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionString);
    final String namespace = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final long backendTag = readField(decoder, child -> child.readUInt(32));
    final String curve = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final byte[] schemaHash = readField(decoder, child -> readFixedBytes(child, 32, "public_inputs_schema_hash"));
    final byte[] commitment = readField(decoder, child -> readFixedBytes(child, 32, "commitment"));
    final int vkLen = readField(decoder, child -> checkedInt(child.readUInt(32), label + ".vk_len"));
    final int maxProofBytes =
        readField(decoder, child -> checkedInt(child.readUInt(32), label + ".max_proof_bytes"));
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionString);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionString);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionString);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionU64Value);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readOptionU64Value);
    final VerifyingKeyBoxValue key =
        readField(
            decoder,
            child -> {
              final byte[] keyPayload = readOptionRawPayload(child);
              return keyPayload == null ? null : readVerifyingKeyBoxPayload(keyPayload, label + ".key");
            });
    final int status = readField(decoder, child -> checkedInt(child.readUInt(8), label + ".status"));
    require(decoder.remaining() == 0, "Trailing bytes after " + label);
    return new DecodedVerifierRecord(
        circuitId,
        namespace,
        backendTag,
        curve,
        schemaHash,
        commitment,
        vkLen,
        maxProofBytes,
        key,
        status);
  }

  private static VerifyingKeyBoxValue readVerifyingKeyBoxPayload(final byte[] payload, final String label) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, REQUEST_FLAGS);
    final String backend = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final byte[] bytes = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    require(decoder.remaining() == 0, "Trailing bytes after " + label);
    return new VerifyingKeyBoxValue(backend, bytes);
  }

  private static TransferPublicInputs parseTransferPublicInputs(final byte[] proofBytes, final String label) {
    final List<List<byte[]>> columns = parseZk1InstanceColumns(proofBytes, label);
    boolean exactSingleRow = columns.size() == 9;
    for (int i = 0; exactSingleRow && i < columns.size(); i++) {
      exactSingleRow = columns.get(i).size() == 1;
    }
    require(exactSingleRow, label + " transfer proof must expose exactly 9 single-row instance columns");
    final List<byte[]> inputNullifiers = nonZeroSorted(
        Arrays.asList(columns.get(2).get(0), columns.get(3).get(0)), label + " input nullifiers");
    final List<byte[]> outputCommitments = nonZeroSorted(
        Arrays.asList(columns.get(4).get(0), columns.get(5).get(0)), label + " output commitments");
    return new TransferPublicInputs(columns.get(6).get(0), inputNullifiers, outputCommitments);
  }

  private static List<List<byte[]>> parseZk1InstanceColumns(final byte[] proofBytes, final String label) {
    require(proofBytes.length >= ZK1_MAGIC.length
        && Arrays.equals(Arrays.copyOfRange(proofBytes, 0, ZK1_MAGIC.length), ZK1_MAGIC),
        label + " proof must use strict ZK1 envelope");
    int offset = ZK1_MAGIC.length;
    boolean sawProof = false;
    List<List<byte[]>> columns = null;
    while (offset < proofBytes.length) {
      require(offset + 8 <= proofBytes.length, label + " malformed ZK1 TLV");
      final String tag = new String(Arrays.copyOfRange(proofBytes, offset, offset + 4), StandardCharsets.US_ASCII);
      final int length = readUInt32LittleEndian(proofBytes, offset + 4);
      require(length >= 0 && length <= ZK1_MAX_TLV_BYTES, label + " oversized ZK1 TLV");
      final int payloadStart = offset + 8;
      final long payloadEndLong = (long) payloadStart + length;
      require(payloadEndLong <= proofBytes.length, label + " truncated ZK1 TLV");
      final int payloadEnd = (int) payloadEndLong;
      final byte[] payload = Arrays.copyOfRange(proofBytes, payloadStart, payloadEnd);
      if ("PROF".equals(tag)) {
        require(!sawProof, label + " duplicate PROF TLV");
        require(payload.length > 0, label + " empty PROF TLV");
        sawProof = true;
      } else if ("I10P".equals(tag)) {
        require(columns == null, label + " duplicate I10P TLV");
        columns = readZk1InstanceColumnsPayload(payload, label);
      } else {
        throw new IllegalArgumentException(label + " unexpected ZK1 TLV");
      }
      offset = payloadEnd;
    }
    require(sawProof, label + " missing PROF TLV");
    if (columns == null) {
      throw new IllegalArgumentException(label + " missing I10P TLV");
    }
    return columns;
  }

  private static List<List<byte[]>> readZk1InstanceColumnsPayload(final byte[] payload, final String label) {
    require(payload.length >= 8, label + " malformed I10P TLV");
    final int columnCount = readUInt32LittleEndian(payload, 0);
    final int rowCount = readUInt32LittleEndian(payload, 4);
    require(columnCount > 0 && rowCount > 0, label + " empty I10P TLV");
    require(columnCount <= ZK1_MAX_INSTANCE_COLUMNS && rowCount <= ZK1_MAX_INSTANCE_ROWS,
        label + " oversized I10P TLV");
    final long expected = 8L + (long) columnCount * rowCount * 32L;
    require(expected <= Integer.MAX_VALUE && payload.length == (int) expected, label + " malformed I10P length");
    final List<List<byte[]>> columns = new ArrayList<>();
    for (int i = 0; i < columnCount; i++) {
      columns.add(new ArrayList<byte[]>(rowCount));
    }
    int offset = 8;
    for (int row = 0; row < rowCount; row++) {
      for (int column = 0; column < columnCount; column++) {
        columns.get(column).add(Arrays.copyOfRange(payload, offset, offset + 32));
        offset += 32;
      }
    }
    return columns;
  }

  private static byte[] proofAttachmentPayload(
      final OpenVerifyEnvelopeValue envelope, final VerifierRecordValue verifierRecord) {
    final NoritoEncoder encoder = new NoritoEncoder(REQUEST_FLAGS);
    writeField(encoder, child -> writeString(child, ZK_BACKEND_HALO2_IPA));
    writeField(encoder, child -> writeProofBox(child, envelope.archive));
    writeField(encoder, child -> writeVerifierKeyId(child, verifierRecord.id));
    writeField(encoder, child -> writeOptionFixed32(child, verifierRecord.commitment));
    writeField(encoder, child -> writeOptionFixed32(child, Blake2b.digest256(envelope.archive)));
    writeField(encoder, child -> writeOptionRaw(child, null));
    return encoder.toByteArray();
  }

  private static void writeVerifiedFoldSteps(
      final NoritoEncoder encoder, final List<PreparedVerifiedFoldHop> hops) {
    encoder.writeUInt(hops.size(), 64);
    for (final PreparedVerifiedFoldHop hop : hops) {
      writeField(
          encoder,
          step -> {
            writeField(step, child -> writeConstVec(child, hop.publicInputs.rootBefore));
            writeField(step, child -> writeFixed32Vec(child, hop.publicInputs.inputNullifiers));
            writeField(step, child -> writeFixed32Vec(child, hop.publicInputs.outputCommitments));
            writeField(step, child -> writeConstVec(child, hop.rootAfter));
            writeRawField(step, proofAttachmentPayload(hop.envelope, hop.verifierRecord));
            writeRawField(step, verifyingKeyBoxPayload(hop.verifierRecord.key.backend, hop.verifierRecord.key.bytes));
          });
    }
  }

  private static void writeVerifiedFoldVerifierRecords(
      final NoritoEncoder encoder, final List<PreparedVerifiedFoldHop> hops) {
    final List<VerifierRecordValue> unique = new ArrayList<>();
    for (final PreparedVerifiedFoldHop hop : hops) {
      boolean exists = false;
      for (final VerifierRecordValue record : unique) {
        if (record.id.equals(hop.verifierRecord.id)) {
          exists = true;
          break;
        }
      }
      if (!exists) {
        unique.add(hop.verifierRecord);
      }
    }
    encoder.writeUInt(unique.size(), 64);
    for (final VerifierRecordValue record : unique) {
      writeField(
          encoder,
          entry -> {
            writeField(entry, child -> writeVerifierKeyId(child, record.id));
            writeRawField(entry, record.recordPayload);
          });
    }
  }

  private static byte[] verifyingKeyBoxPayload(final byte[] bytes) {
    return verifyingKeyBoxPayload(KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND, bytes);
  }

  private static byte[] verifyingKeyBoxPayload(final String backend, final byte[] bytes) {
    requireNonBlankUnpadded(backend, "verifierKeyBackend");
    require(bytes != null && bytes.length > 0, "verifierKey must not be empty");
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    writeField(encoder, child -> writeString(child, backend));
    writeField(encoder, child -> writeBytesVec(child, bytes));
    return encoder.toByteArray();
  }

  private static void requireValidNestedArchive(final byte[] archive, final String field) {
    require(archive != null && archive.length > 0, field + " must not be empty");
    require(
        archive.length <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        field + " must not exceed " + KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + " bytes");
    require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive),
        field + " must be a valid Norito archive");
    require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive),
        field + " must contain a non-empty Norito payload");
  }

  private static void validateLineageKeyArtifactsForInit(
      final byte[] lineageVerifierKey, final byte[] lineageProvingKeyArchive) {
    try {
      KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
          LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN,
          KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
          lineageVerifierKey,
          lineageProvingKeyArchive);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("lineage key artifacts are invalid for recursive spend init", ex);
    }
  }

  private static LineageKeyMaterial lineageKeyMaterialForInit(
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts artifacts) {
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checked =
        requireInitLineageKeyArtifacts(artifacts);
    return new LineageKeyMaterial(
        checked.lineageVerifierKey(),
        checked.lineageProvingKeyArchive());
  }

  private static KagemushaRecursiveSpendProver.LineageKeyArtifacts requireInitLineageKeyArtifacts(
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts artifacts) {
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checked =
        KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(artifacts);
    require(checked.isInitArtifact(), "lineageKeyArtifacts must be init artifacts");
    return checked;
  }

  private static void validateLineageKeyArtifactsForAppend(
      final byte[] lineageVerifierKey, final byte[] lineageProvingKeyArchive) {
    try {
      KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
          LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN,
          KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
          lineageVerifierKey,
          lineageProvingKeyArchive);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("lineage key artifacts are invalid for lineage append output", ex);
    }
  }

  private static LineageKeyMaterial lineageKeyMaterialForAppend(
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts artifacts) {
    if (artifacts == null) {
      return new LineageKeyMaterial(null, null);
    }
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checked =
        requireAppendLineageKeyArtifacts(artifacts);
    return new LineageKeyMaterial(
        checked.lineageVerifierKey(),
        checked.lineageProvingKeyArchive());
  }

  private static KagemushaRecursiveSpendProver.LineageKeyArtifacts requireAppendLineageKeyArtifacts(
      final KagemushaRecursiveSpendProver.LineageKeyArtifacts artifacts) {
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts checked =
        KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(artifacts);
    require(checked.isAppendArtifact(), "lineageKeyArtifacts must be append artifacts");
    return checked;
  }

  private static final class LineageKeyMaterial {
    private final byte[] verifierKey;
    private final byte[] provingKeyArchive;

    private LineageKeyMaterial(final byte[] verifierKey, final byte[] provingKeyArchive) {
      this.verifierKey = verifierKey;
      this.provingKeyArchive = provingKeyArchive;
    }
  }

  private static int readVerifiedFoldRecordBundleHopCount(
      final byte[] payload, final int flags, final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final byte[] bundlePayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    require(decoder.remaining() == 0, "Trailing bytes after " + field);

    final NoritoDecoder bundle = new NoritoDecoder(bundlePayload, flags);
    skipFields(bundle, 2);
    final int hopCount =
        readField(bundle, child -> readVerifiedFoldStepCount(child, field + ".steps"));
    require(bundle.remaining() == 0, "Trailing bytes after " + field + " bundle");
    require(hopCount > 0, field + " must contain at least one fold step");
    require(
        hopCount <= KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS,
        field + " fold step count must not exceed " + KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS);
    return hopCount;
  }

  private static int readVerifiedFoldStepCount(final NoritoDecoder decoder, final String field) {
    final int count = checkedInt(decoder.readUInt(64), field + " count");
    for (int index = 0; index < count; index++) {
      final int itemLength = checkedInt(decoder.readLength(compact(decoder)), field + " item length");
      final NoritoDecoder item =
          new NoritoDecoder(decoder.readBytes(itemLength), decoder.flags(), decoder.flagsHint());
      skipFields(item, 6);
      require(item.remaining() == 0, "Trailing bytes after " + field + "[" + index + "]");
    }
    require(decoder.remaining() == 0, "Trailing bytes after " + field);
    return count;
  }

  private static void requirePallasOpenEnvelopesArchive(
      final byte[] archive, final int expectedEnvelopeCount, final String field, final int maxBytes) {
    require(archive != null && archive.length > 0, field + " must not be empty");
    require(
        archive.length <= maxBytes,
        field + " must not exceed " + maxBytes + " bytes");
    final NoritoHeader.DecodeResult decoded;
    try {
      decoded = NoritoHeader.decode(archive, OPEN_VERIFY_ENVELOPES_SCHEMA_HASH);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(
          field + " must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive", ex);
    }
    require(decoded.header().compression() == NoritoHeader.COMPRESSION_NONE, field + " must not be compressed");
    require(decoded.header().payloadLength() > 0, field + " must contain a non-empty Norito payload");
    decoded.header().validateChecksum(decoded.payload());
    require(decoded.header().flags() == NoritoHeader.COMPACT_LEN, field + " must use compact Norito layout");
    final NoritoDecoder decoder = new NoritoDecoder(decoded.payload(), decoded.header().flags());
    final int count = checkedInt(decoder.readUInt(64), field + " envelope count");
    require(
        count == expectedEnvelopeCount,
        field + " requires exactly " + expectedEnvelopeCount + " envelope(s)");
    for (int index = 0; index < count; index++) {
      final int itemLength = checkedInt(decoder.readLength(compact(decoder)), field + " envelope length");
      final byte[] itemPayload = decoder.readBytes(itemLength);
      validatePallasOpenEnvelopePayload(
          itemPayload, decoded.header().flags(), field + "[" + index + "]");
    }
    require(decoder.remaining() == 0, "Trailing bytes after " + field + " archive");
  }

  private static void requirePreviousProofOpenEnvelopesArchive(final byte[] archive) {
    requirePallasOpenEnvelopesArchive(
        archive,
        KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
        "previousProofOpenEnvelopes",
        KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES);
  }

  private static void validatePallasOpenEnvelopePayload(
      final byte[] payload, final int flags, final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final int paramsN = readField(decoder, child -> readPallasIpaParams(child, field + ".params"));
    final int publicN = readField(decoder, child -> readPallasPolyOpenPublic(child, field + ".public"));
    require(publicN == paramsN, field + " public opening length must match params.n");
    readField(
        decoder,
        child -> {
          readPallasIpaProof(child, paramsN, field + ".proof");
          return null;
        });
    final String transcriptLabel = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    require(!transcriptLabel.isEmpty(), field + " transcript_label must be non-empty");
    require(
        transcriptLabel.getBytes(StandardCharsets.UTF_8).length
            <= KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
        field
            + " transcript_label exceeds "
            + KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
            + " bytes");
    readField(
        decoder,
        field + ".vk_commitment",
        child -> readRequiredMetadataOption(child, field + ".vk_commitment"));
    readField(
        decoder,
        field + ".public_inputs_schema_hash",
        child -> readRequiredMetadataOption(child, field + ".public_inputs_schema_hash"));
    readField(
        decoder,
        field + ".domain_tag",
        child -> readRequiredMetadataOption(child, field + ".domain_tag"));
    require(decoder.remaining() == 0, "Trailing bytes after " + field);
  }

  private static int readPallasIpaParams(final NoritoDecoder decoder, final String field) {
    final int version = readField(decoder, child -> checkedInt(child.readUInt(16), field + ".version"));
    require(version == 1, field + ".version must be 1");
    final int curveId = readField(decoder, child -> checkedInt(child.readUInt(16), field + ".curve_id"));
    require(curveId == PALLAS_CURVE_ID, field + ".curve_id must be Pallas");
    final int n = readField(decoder, child -> checkedInt(child.readUInt(32), field + ".n"));
    require(n >= 2 && Integer.bitCount(n) == 1, field + ".n must be a power of two >= 2");
    require(
        n <= KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_N,
        field + ".n exceeds max 2^" + KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_K);
    final int gCount = readField(decoder, child -> readFixed32SequenceCount(child, field + ".g"));
    require(gCount == n, field + ".g length must equal params.n");
    final int hCount = readField(decoder, child -> readFixed32SequenceCount(child, field + ".h"));
    require(hCount == n, field + ".h length must equal params.n");
    readField(decoder, child -> readFixedBytes(child, 32, field + ".u"));
    require(decoder.remaining() == 0, "Trailing bytes after " + field);
    return n;
  }

  private static int readPallasPolyOpenPublic(final NoritoDecoder decoder, final String field) {
    final int version = readField(decoder, child -> checkedInt(child.readUInt(16), field + ".version"));
    require(version == 1, field + ".version must be 1");
    final int curveId = readField(decoder, child -> checkedInt(child.readUInt(16), field + ".curve_id"));
    require(curveId == PALLAS_CURVE_ID, field + ".curve_id must be Pallas");
    final int n = readField(decoder, child -> checkedInt(child.readUInt(32), field + ".n"));
    readField(decoder, child -> readFixedBytes(child, 32, field + ".z"));
    readField(decoder, child -> readFixedBytes(child, 32, field + ".t"));
    readField(decoder, child -> readFixedBytes(child, 32, field + ".p_g"));
    require(decoder.remaining() == 0, "Trailing bytes after " + field);
    return n;
  }

  private static void readPallasIpaProof(final NoritoDecoder decoder, final int n, final String field) {
    final int version = readField(decoder, child -> checkedInt(child.readUInt(16), field + ".version"));
    require(version == 1, field + ".version must be 1");
    final int lCount = readField(decoder, child -> readFixed32SequenceCount(child, field + ".l"));
    final int rCount = readField(decoder, child -> readFixed32SequenceCount(child, field + ".r"));
    require(lCount == rCount, field + " L/R round count mismatch");
    final int expectedRounds = Integer.numberOfTrailingZeros(n);
    require(
        lCount == expectedRounds,
        field + " round count mismatch: expected " + expectedRounds + ", found " + lCount);
    readField(decoder, child -> readFixedBytes(child, 32, field + ".a_final"));
    readField(decoder, child -> readFixedBytes(child, 32, field + ".b_final"));
    require(decoder.remaining() == 0, "Trailing bytes after " + field);
  }

  private static int readFixed32SequenceCount(final NoritoDecoder decoder, final String field) {
    return readFixed32Sequence(decoder, field).size();
  }

  private static List<byte[]> readFixed32Sequence(final NoritoDecoder decoder, final String field) {
    final int count = checkedInt(decoder.readUInt(64), field + " count");
    final ArrayList<byte[]> values = new ArrayList<>();
    for (int index = 0; index < count; index++) {
      final int itemLength = checkedInt(decoder.readLength(compact(decoder)), field + " item length");
      final NoritoDecoder item =
          new NoritoDecoder(decoder.readBytes(itemLength), decoder.flags(), decoder.flagsHint());
      values.add(readFixedBytes(item, 32, field + "[" + index + "]"));
      require(item.remaining() == 0, "Trailing bytes after " + field + "[" + index + "]");
    }
    return Collections.unmodifiableList(values);
  }

  private static byte[] readRequiredMetadataOption(final NoritoDecoder decoder, final String field) {
    final byte[] payload = readOptionRawPayload(decoder, field);
    if (payload == null) {
      throw new IllegalArgumentException(field + " is required");
    }
    require(payload.length == 32, field + " must be exactly 32 bytes");
    final byte[] value = Arrays.copyOf(payload, payload.length);
    require(!isZero(value), field + " must be non-zero");
    return value;
  }

  private static final byte[] OPEN_VERIFY_ENVELOPES_SCHEMA_HASH =
      new byte[] {
        (byte) 0xfe, 0x38, 0x26, 0x32,
        (byte) 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, (byte) 0xfe,
        0x11, 0x02, 0x60, (byte) 0xca
      };

  private static final int PALLAS_CURVE_ID = 1;
  private static final int LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN = 2;
  private static final int KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_K = 24;
  private static final int KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_N =
      1 << KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_K;
  private static final int KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES =
      128;

  private static byte[] fixedBytes(final byte[] value, final int expectedSize, final String field) {
    require(value != null && value.length == expectedSize, field + " must be exactly " + expectedSize + " bytes");
    return Arrays.copyOf(value, value.length);
  }

  private static boolean isZero(final byte[] value) {
    for (final byte b : value) {
      if (b != 0) {
        return false;
      }
    }
    return true;
  }

  private static byte[] irohaHash(final byte[] value) {
    final byte[] digest = Blake2b.digest256(value);
    digest[digest.length - 1] = (byte) (digest[digest.length - 1] | 0x01);
    return digest;
  }

  private static void requireNonNegativeHeight(final Long blockHeight) {
    require(blockHeight == null || blockHeight >= 0L, "blockHeight must be non-negative");
  }

  private static void requirePortableId(final String value, final String field) {
    requireNonBlankUnpadded(value, field);
    require(value.length() <= 256, field + " must not exceed 256 characters");
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      if ((ch >= 'A' && ch <= 'Z')
          || (ch >= 'a' && ch <= 'z')
          || (ch >= '0' && ch <= '9')
          || "._-/:@+=".indexOf(ch) >= 0) {
        continue;
      }
      throw new IllegalArgumentException(field + " must use portable registry syntax");
    }
  }

  private static void requireNonBlankUnpadded(final String value, final String field) {
    require(value != null && !value.trim().isEmpty(), field + " must not be blank");
    require(value.trim().equals(value), field + " must not contain surrounding whitespace");
  }

  private static String canonicalU128Decimal(final String value, final String field) {
    require(value != null && !value.isEmpty(), field + " must be a decimal integer");
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      require(ch >= '0' && ch <= '9', field + " must be a decimal integer");
    }
    require(value.length() == 1 || value.charAt(0) != '0', field + " must be canonical");
    require(value.length() <= MAX_U128_DECIMAL_DIGITS, field + " must fit in u128");
    final BigInteger integer = new BigInteger(value);
    require(integer.compareTo(BigInteger.ZERO) > 0, field + " must be greater than zero");
    require(integer.compareTo(MAX_U128) <= 0, field + " must fit in u128");
    return integer.toString();
  }

  private static void requireRedeemChangeBinding(
      final String publicAmount,
      final String currentAmount,
      final boolean hasChangeOutput) {
    final int comparison = compareCanonicalDecimal(publicAmount, currentAmount);
    if (hasChangeOutput) {
      require(
          comparison < 0,
          "publicAmount must be less than current note amount when changeOutput is present");
    } else {
      require(
          comparison >= 0,
          "changeOutput is required when publicAmount is less than current note amount");
      require(
          comparison == 0,
          "publicAmount must not exceed current note amount");
    }
  }

  private static void requireRedeemChangeOutputNotReserved(
      final byte[] changeOutput,
      final SpendBundleSummary bundleSummary) {
    require(
        !Arrays.equals(changeOutput, bundleSummary.currentNote.noteCommitment())
            && !Arrays.equals(changeOutput, bundleSummary.currentNote.spendNullifier()),
        "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier");
    for (final byte[] nullifier : bundleSummary.topupAnchorNullifiers()) {
      require(
          !Arrays.equals(changeOutput, nullifier),
          "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier");
    }
  }

  private static void requireTopupAnchorNullifiers(
      final List<byte[]> nullifiers,
      final SpendableNoteDescriptor currentNote) {
    require(
        !nullifiers.isEmpty() && nullifiers.size() <= KAGEMUSHA_FOLD_STEP_MAX_INPUTS,
        "bundle.accumulator.topup_anchor_nullifiers");
    for (int index = 0; index < nullifiers.size(); index++) {
      final byte[] nullifier = nullifiers.get(index);
      require(!isZero(nullifier), "bundle.accumulator.topup_anchor_nullifiers");
      if (index > 0) {
        require(
            compareUnsigned(nullifiers.get(index - 1), nullifier) < 0,
            "bundle.accumulator.topup_anchor_nullifiers");
      }
    }
    for (final byte[] nullifier : nullifiers) {
      require(
          !Arrays.equals(nullifier, currentNote.noteCommitment())
              && !Arrays.equals(nullifier, currentNote.spendNullifier()),
          "bundle.accumulator.topup_anchor_nullifiers");
    }
  }

  private static void requireAccumulatorRoots(final byte[] initialRoot, final byte[] finalRoot) {
    require(!isZero(initialRoot), "bundle.accumulator.initial_root");
    require(
        !isZero(finalRoot) && !Arrays.equals(finalRoot, initialRoot),
        "bundle.accumulator.final_root");
  }

  private static void requireAccumulatorCorridor(final NoritoDecoder decoder, final int hopCount) {
    final byte[] lineageDigest = requireNonzeroAccumulatorField(decoder, "lineage_digest");
    final byte[] aggregationTranscriptDigest =
        readAccumulatorFixed32(decoder, "aggregation_transcript_digest");
    require(
        !isZero(aggregationTranscriptDigest)
            && Arrays.equals(aggregationTranscriptDigest, lineageDigest),
        "bundle.accumulator.aggregation_transcript_digest");
    for (final String field :
        new String[] {
          "nullifier_digest",
          "output_commitment_digest",
          "fold_digest",
          "recursive_proof_chain_digest",
          "transition_profile_binding_digest"
        }) {
      requireNonzeroAccumulatorField(decoder, field);
    }
    final byte[] appendOpeningPreflightDigest =
        readAccumulatorFixed32(decoder, "append_opening_preflight_digest");
    require(
        isZero(appendOpeningPreflightDigest) || hopCount > 1,
        "bundle.accumulator.append_opening_preflight_digest");
    final byte[] appendBoundaryDigest = readAccumulatorFixed32(decoder, "append_boundary_digest");
    require(
        isZero(appendBoundaryDigest)
            || (!isZero(appendOpeningPreflightDigest) && hopCount > 1),
        "bundle.accumulator.append_boundary_digest");
    for (final String field :
        new String[] {
          "verifier_params_fingerprint",
          "fixed_window_table_schedule_digest",
          "fixed_window_shared_table_manifest_digest",
          "fixed_window_table_base_digest",
          "verifier_witness_batch_digest"
        }) {
      requireNonzeroAccumulatorField(decoder, field);
    }
    final int verifierOpeningLen =
        readField(
            decoder,
            "bundle.accumulator.verifier_opening_len",
            child -> checkedInt(child.readUInt(32), "verifier_opening_len"));
    require(
        verifierOpeningLen == 2
            || verifierOpeningLen == 4
            || verifierOpeningLen == 8
            || verifierOpeningLen == 16
            || verifierOpeningLen == 32
            || verifierOpeningLen == 64
            || verifierOpeningLen == 128,
        "bundle.accumulator.verifier_opening_len");
  }

  private static byte[] readAccumulatorFixed32(final NoritoDecoder decoder, final String field) {
    return readField(decoder, child -> readFixedBytes(child, 32, field));
  }

  private static byte[] requireNonzeroAccumulatorField(
      final NoritoDecoder decoder, final String field) {
    final byte[] value = readAccumulatorFixed32(decoder, field);
    require(!isZero(value), "bundle.accumulator." + field);
    return value;
  }

  private static int compareCanonicalDecimal(final String left, final String right) {
    if (left.length() != right.length()) {
      return Integer.compare(left.length(), right.length());
    }
    return left.compareTo(right);
  }

  private static void writeField(final NoritoEncoder parent, final FieldWriter writePayload) {
    final NoritoEncoder child = parent.childEncoder();
    writePayload.write(child);
    writeRawField(parent, child.toByteArray());
  }

  private static void writeRawField(final NoritoEncoder parent, final byte[] payload) {
    parent.writeLength(payload.length, compact(parent));
    parent.writeBytes(payload);
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, compact(encoder));
    encoder.writeBytes(bytes);
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static void writeConstVec(final NoritoEncoder encoder, final byte[] value) {
    for (final byte b : value) {
      encoder.writeLength(1, compact(encoder));
      encoder.writeByte(b);
    }
  }

  private static byte[] fixed32Payload(final byte[] value) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    writeConstVec(encoder, fixedBytes(value, 32, "fixed32"));
    return encoder.toByteArray();
  }

  private static void writeOptionRaw(final NoritoEncoder encoder, final byte[] payload) {
    if (payload == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    encoder.writeLength(payload.length, compact(encoder));
    encoder.writeBytes(payload);
  }

  private static void writeOptionBytesVec(final NoritoEncoder encoder, final byte[] value) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    writeField(encoder, child -> writeBytesVec(child, value));
  }

  private static void writeOptionU64(final NoritoEncoder encoder, final Long value) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    writeField(encoder, child -> child.writeUInt(value, 64));
  }

  private static void writeOptionFixed32(final NoritoEncoder encoder, final byte[] value) {
    writeOptionRaw(encoder, value == null ? null : fixed32Payload(value));
  }

  private static void writeSpendableNote(final NoritoEncoder encoder, final SpendableNoteDescriptor value) {
    writeField(encoder, child -> writeConstVec(child, value.noteCommitment()));
    writeField(encoder, child -> writeConstVec(child, value.spendNullifier()));
    writeField(encoder, child -> writeNumeric(child, value.amount));
  }

  private static void writeChainId(final NoritoEncoder encoder, final String value) {
    requirePortableId(value, "chainId");
    writeField(encoder, child -> writeString(child, value));
  }

  private static void writeAssetDefinitionId(final NoritoEncoder encoder, final String value) {
    final byte[] bytes;
    try {
      bytes = AssetDefinitionIdEncoder.parseAddressBytes(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("asset must be a canonical asset definition id", ex);
    }
    encoder.writeBytes(bytes);
  }

  private static void writeFixed32Vec(final NoritoEncoder encoder, final List<byte[]> values) {
    require(!values.isEmpty(), "fixed32 vector must not be empty");
    encoder.writeUInt(values.size(), 64);
    for (final byte[] value : values) {
      writeField(encoder, child -> writeConstVec(child, fixedBytes(value, 32, "fixed32 vector element")));
    }
  }

  private static void writeProofBox(final NoritoEncoder encoder, final byte[] proofBytes) {
    require(proofBytes.length > 0, "proof bytes must not be empty");
    writeField(encoder, child -> writeString(child, ZK_BACKEND_HALO2_IPA));
    writeField(encoder, child -> writeBytesVec(child, proofBytes));
  }

  private static void writeVerifierKeyId(final NoritoEncoder encoder, final VerifierKeyIdValue id) {
    requirePortableId(id.backend, "verifierKeyId.backend");
    requirePortableId(id.name, "verifierKeyId.name");
    writeField(encoder, child -> writeString(child, id.backend));
    writeField(encoder, child -> writeString(child, id.name));
  }

  private static void writeNumeric(final NoritoEncoder encoder, final String value) {
    final byte[] mantissaBytes = toTwosComplementLittleEndian(new BigInteger(value));
    writeField(
        encoder,
        child -> {
          child.writeUInt(mantissaBytes.length, 32);
          child.writeBytes(mantissaBytes);
        });
    writeField(encoder, child -> child.writeUInt(0, 32));
  }

  private static void writeU128(final NoritoEncoder encoder, final String value) {
    BigInteger integer = new BigInteger(value);
    for (int i = 0; i < 16; i++) {
      encoder.writeByte(integer.and(BYTE_MASK).intValue());
      integer = integer.shiftRight(8);
    }
  }

  private static byte[] toTwosComplementLittleEndian(final BigInteger value) {
    if (value.signum() == 0) {
      return new byte[0];
    }
    final byte[] bigEndian = value.toByteArray();
    final byte[] little = new byte[bigEndian.length];
    for (int i = 0; i < bigEndian.length; i++) {
      little[i] = bigEndian[bigEndian.length - 1 - i];
    }
    return little;
  }

  private static void writeAccountId(final NoritoEncoder encoder, final String accountId) {
    final AccountAddress address;
    try {
      address = AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).address;
    } catch (final AccountAddressException ex) {
      throw new IllegalArgumentException("recipient must use canonical I105 account form", ex);
    }
    final Optional<AccountAddress.SingleKeyPayload> single;
    try {
      single = address.singleKeyPayloadIgnoringCurveSupport();
    } catch (final AccountAddressException ex) {
      throw new IllegalArgumentException("recipient must use canonical I105 account form", ex);
    }
    if (single.isPresent()) {
      encoder.writeUInt(0, 32);
      writeField(encoder, child -> writePublicKey(child, single.get().curveId(), single.get().publicKey()));
      return;
    }
    final Optional<AccountAddress.MultisigPolicyPayload> multisig;
    try {
      multisig = address.multisigPolicyPayloadIgnoringCurveSupport();
    } catch (final AccountAddressException ex) {
      throw new IllegalArgumentException("recipient must use canonical I105 account form", ex);
    }
    if (!multisig.isPresent()) {
      throw new IllegalArgumentException("recipient has no supported controller");
    }
    encoder.writeUInt(1, 32);
    writeField(encoder, child -> writeMultisigPolicy(child, multisig.get()));
  }

  private static void writeMultisigPolicy(
      final NoritoEncoder encoder, final AccountAddress.MultisigPolicyPayload policy) {
    require(policy.version() == MULTISIG_POLICY_VERSION, "unsupported multisig policy version");
    require(policy.threshold() > 0, "multisig threshold must be positive");
    require(!policy.members().isEmpty(), "multisig policy must have members");
    writeField(encoder, child -> child.writeUInt(policy.version(), 8));
    writeField(encoder, child -> child.writeUInt(policy.threshold(), 16));
    writeField(encoder, child -> writeMultisigMembers(child, policy.members()));
  }

  private static void writeMultisigMembers(
      final NoritoEncoder encoder, final List<AccountAddress.MultisigMemberPayload> members) {
    final List<AccountAddress.MultisigMemberPayload> sorted = new ArrayList<>(members);
    Collections.sort(sorted, (left, right) -> compareUnsigned(memberSortKey(left), memberSortKey(right)));
    for (int i = 1; i < sorted.size(); i++) {
      require(!Arrays.equals(memberSortKey(sorted.get(i - 1)), memberSortKey(sorted.get(i))),
          "duplicate multisig member");
    }
    encoder.writeUInt(sorted.size(), 64);
    for (final AccountAddress.MultisigMemberPayload member : sorted) {
      writeField(
          encoder,
          memberEncoder -> {
            writeField(memberEncoder, child -> writePublicKey(child, member.curveId(), member.publicKey()));
            writeField(memberEncoder, child -> child.writeUInt(member.weight(), 16));
          });
    }
  }

  private static byte[] memberSortKey(final AccountAddress.MultisigMemberPayload member) {
    final byte[] publicKey = member.publicKey();
    final byte[] key = new byte[1 + publicKey.length];
    key[0] = (byte) member.curveId();
    System.arraycopy(publicKey, 0, key, 1, publicKey.length);
    return key;
  }

  private static int compareUnsigned(final byte[] left, final byte[] right) {
    final int min = Math.min(left.length, right.length);
    for (int i = 0; i < min; i++) {
      final int cmp = (left[i] & 0xff) - (right[i] & 0xff);
      if (cmp != 0) {
        return cmp;
      }
    }
    return left.length - right.length;
  }

  private static void writePublicKey(
      final NoritoEncoder encoder, final int curveId, final byte[] publicKey) {
    writeConstVec(encoder, publicKeyCompactPayload(curveId, publicKey));
  }

  private static byte[] publicKeyCompactPayload(final int curveId, final byte[] publicKey) {
    final int tag;
    switch (curveId) {
      case 0x01:
        tag = 0;
        break;
      case 0x04:
        tag = 1;
        break;
      case 0x03:
        tag = 2;
        break;
      case 0x05:
        tag = 3;
        break;
      case 0x02:
        tag = 4;
        break;
      case 0x0A:
        tag = 5;
        break;
      case 0x0B:
        tag = 6;
        break;
      case 0x0C:
        tag = 7;
        break;
      case 0x0D:
        tag = 8;
        break;
      case 0x0E:
        tag = 9;
        break;
      case 0x0F:
        tag = 10;
        break;
      default:
        throw new IllegalArgumentException("Unsupported recipient curve id: " + curveId);
    }
    final byte[] bytes = new byte[1 + publicKey.length];
    bytes[0] = (byte) tag;
    System.arraycopy(publicKey, 0, bytes, 1, publicKey.length);
    return bytes;
  }

  private static SpendableNoteDescriptor readSpendableNote(final NoritoDecoder decoder) {
    return new SpendableNoteDescriptor(
        readField(decoder, child -> readFixedBytes(child, 32, "note_commitment")),
        readField(decoder, child -> readFixedBytes(child, 32, "spend_nullifier")),
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readNumeric));
  }

  private static String readNumeric(final NoritoDecoder decoder) {
    final byte[] mantissaBytes =
        readField(
            decoder,
            payload -> {
              final int length = checkedInt(payload.readUInt(32), "numeric mantissa length");
              return payload.readBytes(length);
            });
    final int scale = readField(decoder, child -> checkedInt(child.readUInt(32), "numeric scale"));
    require(scale == 0, "numeric scale must be zero");
    final BigInteger value = bigIntegerFromLittleEndianTwosComplement(mantissaBytes);
    require(value.compareTo(BigInteger.ZERO) > 0, "numeric amount must be greater than zero");
    require(value.compareTo(MAX_U128) <= 0, "numeric amount must fit in u128");
    return value.toString();
  }

  private static BigInteger bigIntegerFromLittleEndianTwosComplement(final byte[] bytes) {
    if (bytes.length == 0) {
      return BigInteger.ZERO;
    }
    final byte[] bigEndian = new byte[bytes.length];
    for (int i = 0; i < bytes.length; i++) {
      bigEndian[i] = bytes[bytes.length - 1 - i];
    }
    return new BigInteger(bigEndian);
  }

  private static String readChainId(final NoritoDecoder decoder) {
    return readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
  }

  private static String readAccumulatorChainId(final NoritoDecoder decoder) {
    try {
      final String chainId = readChainId(decoder);
      require(decoder.remaining() == 0, "bundle.accumulator.chain_id has trailing bytes");
      requirePortableId(chainId, "bundle.accumulator.chain_id");
      return chainId;
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException("bundle.accumulator.chain_id", error);
    }
  }

  private static String readAssetDefinitionId(final NoritoDecoder decoder) {
    final byte[] bytes = readFixedBytes(decoder, 16, "asset");
    try {
      return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
    } catch (final IllegalArgumentException ignored) {
      return "hex:" + toHex(bytes);
    }
  }

  private static VerifyingKeyIdParts readVerifyingKeyId(final byte[] payload, final int flags) {
    return readVerifyingKeyId(payload, flags, RecursiveProofDecodeContext.BUNDLE);
  }

  private static VerifyingKeyIdParts readVerifyingKeyId(
      final byte[] payload, final int flags, final RecursiveProofDecodeContext context) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final String backend = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String name = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    require(decoder.remaining() == 0, "Trailing bytes after " + context.verifierTrailingField);
    requirePortableId(backend, context.verifierBackendField);
    require(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND.equals(backend),
        context.proofBackendField + " unsupported recursive proof backend");
    requirePortableId(name, context.verifierNameField);
    return new VerifyingKeyIdParts(backend, name);
  }

  private static String readVerifyingKeyIdName(final byte[] payload, final int flags) {
    return readVerifyingKeyId(payload, flags).name;
  }

  private static String readProofBoxBackend(final byte[] payload, final int flags) {
    return readProofBoxBackend(payload, flags, RecursiveProofDecodeContext.BUNDLE);
  }

  private static String readProofBoxBackend(
      final byte[] payload, final int flags, final RecursiveProofDecodeContext context) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final String backend = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final byte[] proofBytes = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readBytesVec);
    require(decoder.remaining() == 0, "Trailing bytes after " + context.proofBoxTrailingField);
    requirePortableId(backend, "proof.backend");
    require(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND.equals(backend),
        context.proofBackendField + " unsupported recursive proof backend");
    require(proofBytes.length > 0, context.proofBytesField + " empty recursive proof");
    return backend;
  }

  private static final class VerifyingKeyIdParts {
    final String backend;
    final String name;

    VerifyingKeyIdParts(final String backend, final String name) {
      this.backend = backend;
      this.name = name;
    }
  }

  private static final class RecursiveProofDecodeContext {
    static final RecursiveProofDecodeContext BUNDLE =
        new RecursiveProofDecodeContext(
            "recursive proof",
            "proof",
            "verifier key id",
            "verifierKeyId.backend",
            "verifierKeyId",
            "bundle.proof_public_inputs",
            "bundle.proof_public_inputs_hash",
            "bundle.proof_backend",
            "bundle.proof_bytes");

    static final RecursiveProofDecodeContext LINEAGE_PREVIOUS_PROOF =
        new RecursiveProofDecodeContext(
            "lineageWitness.previousRecursiveProofs",
            "lineageWitness.previousRecursiveProofs.proof",
            "lineageWitness.previousRecursiveProofs.verifierKeyId",
            "lineageWitness.previousRecursiveProofs.verifierKeyId.backend",
            "lineageWitness.previousRecursiveProofs.verifierKeyId.name",
            "lineageWitness.previousRecursiveProofs.proof_public_inputs",
            "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
            "lineageWitness.previousRecursiveProofs.proof_backend",
            "lineageWitness.previousRecursiveProofs.proof_bytes");

    final String trailingField;
    final String proofBoxTrailingField;
    final String verifierTrailingField;
    final String verifierBackendField;
    final String verifierNameField;
    final String proofPublicInputsField;
    final String proofPublicInputsHashField;
    final String proofBackendField;
    final String proofBytesField;

    RecursiveProofDecodeContext(
        final String trailingField,
        final String proofBoxTrailingField,
        final String verifierTrailingField,
        final String verifierBackendField,
        final String verifierNameField,
        final String proofPublicInputsField,
        final String proofPublicInputsHashField,
        final String proofBackendField,
        final String proofBytesField) {
      this.trailingField = trailingField;
      this.proofBoxTrailingField = proofBoxTrailingField;
      this.verifierTrailingField = verifierTrailingField;
      this.verifierBackendField = verifierBackendField;
      this.verifierNameField = verifierNameField;
      this.proofPublicInputsField = proofPublicInputsField;
      this.proofPublicInputsHashField = proofPublicInputsHashField;
      this.proofBackendField = proofBackendField;
      this.proofBytesField = proofBytesField;
    }
  }

  private static <T> T readField(final NoritoDecoder parent, final FieldReader<T> readPayload) {
    return readField(parent, "Trailing bytes after field decode", readPayload);
  }

  private static <T> T readField(
      final NoritoDecoder parent, final String trailingMessage, final FieldReader<T> readPayload) {
    final int length = checkedInt(parent.readLength(compact(parent)), "field length");
    final NoritoDecoder child = new NoritoDecoder(parent.readBytes(length), parent.flags(), parent.flagsHint());
    final T value = readPayload.read(child);
    require(child.remaining() == 0, trailingMessage);
    return value;
  }

  private static String readString(final NoritoDecoder decoder) {
    final int length = checkedInt(decoder.readLength(compact(decoder)), "string length");
    return new String(decoder.readBytes(length), StandardCharsets.UTF_8);
  }

  private static byte[] readBytesVec(final NoritoDecoder decoder) {
    final int length = checkedInt(decoder.readUInt(64), "byte vector length");
    return decoder.readBytes(length);
  }

  private static byte[] readOptionRawPayload(final NoritoDecoder decoder) {
    return readOptionRawPayload(decoder, "option");
  }

  private static byte[] readOptionRawPayload(final NoritoDecoder decoder, final String field) {
    final int tag = decoder.readByte();
    require(tag == 0 || tag == 1, field + " option tag must be 0 or 1");
    if (tag == 0) {
      return null;
    }
    final int length = checkedInt(decoder.readLength(compact(decoder)), field + " option payload length");
    require(decoder.remaining() >= length, field + " payload length mismatch");
    return decoder.readBytes(length);
  }

  private static String readOptionString(final NoritoDecoder decoder) {
    final byte[] payload = readOptionRawPayload(decoder);
    if (payload == null) {
      return null;
    }
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final String value = readString(child);
    require(child.remaining() == 0, "Trailing bytes after option string");
    return value;
  }

  private static Long readOptionU64Value(final NoritoDecoder decoder) {
    final byte[] payload = readOptionRawPayload(decoder);
    if (payload == null) {
      return null;
    }
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final long value = child.readUInt(64);
    require(child.remaining() == 0, "Trailing bytes after option u64");
    return value;
  }

  private static void skipFields(final NoritoDecoder decoder, final int count) {
    for (int i = 0; i < count; i++) {
      final int length = checkedInt(decoder.readLength(compact(decoder)), "field length");
      decoder.readBytes(length);
    }
  }

  private static int checkedInt(final long value, final String field) {
    require(value >= 0, field + " must be non-negative");
    require(value <= Integer.MAX_VALUE, field + " exceeds JVM Int range");
    return (int) value;
  }

  private static boolean readBool(final NoritoDecoder decoder) {
    final int raw = decoder.readByte();
    require(raw == 0 || raw == 1, "boolean field must be 0 or 1");
    return raw == 1;
  }

  private static byte[] readFixedBytes(
      final NoritoDecoder decoder, final int expectedSize, final String field) {
    if (decoder.remaining() == expectedSize) {
      return decoder.readBytes(expectedSize);
    }
    final byte[] output = new byte[expectedSize];
    int offset = 0;
    while (decoder.remaining() > 0) {
      final long length = decoder.readLength(compact(decoder));
      require(length == 1L, field + " byte field length must be 1");
      require(offset < expectedSize, field + " must be exactly " + expectedSize + " bytes");
      output[offset++] = (byte) decoder.readByte();
    }
    require(offset == expectedSize, field + " must be exactly " + expectedSize + " bytes");
    return output;
  }

  private static byte[] readRemainingBytes(final NoritoDecoder decoder) {
    return decoder.readBytes(decoder.remaining());
  }

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean compact(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }

  private static byte[] copyOf(final byte[] value, final String field) {
    require(value != null, field + " must not be null");
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] copyNullable(final byte[] value) {
    return value == null ? null : Arrays.copyOf(value, value.length);
  }

  private static String toHex(final byte[] bytes) {
    final char[] out = new char[bytes.length * 2];
    for (int i = 0; i < bytes.length; i++) {
      final int value = bytes[i] & 0xff;
      out[i * 2] = HEX[value >>> 4];
      out[i * 2 + 1] = HEX[value & 0x0f];
    }
    return new String(out);
  }

  private static VerifierKeyIdValue parseVerifierKeyId(final String value, final String field) {
    requirePortableId(value, field);
    final int separator = value.indexOf(':');
    require(separator > 0 && separator < value.length() - 1, field + " must use backend:name syntax");
    final String backend = value.substring(0, separator);
    final String name = value.substring(separator + 1);
    requirePortableId(backend, field + ".backend");
    requirePortableId(name, field + ".name");
    return new VerifierKeyIdValue(backend, name);
  }

  private static byte[] verifyingKeyCommitment(final String backend, final byte[] verifierKey) {
    requireNonBlankUnpadded(backend, "verifierKeyBackend");
    require(verifierKey != null && verifierKey.length > 0, "verifierKey must not be empty");
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] backendBytes = backend.getBytes(StandardCharsets.UTF_8);
      digest.update("iroha:zk:v1:vk".getBytes(StandardCharsets.US_ASCII));
      digest.update(longBigEndian(backendBytes.length));
      digest.update(backendBytes);
      digest.update(longBigEndian(verifierKey.length));
      digest.update(verifierKey);
      return digest.digest();
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest is unavailable", ex);
    }
  }

  private static byte[] longBigEndian(final long value) {
    final byte[] out = new byte[8];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) ((value >>> ((7 - i) * 8)) & 0xff);
    }
    return out;
  }

  private static int readUInt32LittleEndian(final byte[] bytes, final int offset) {
    require(offset >= 0 && offset + 4 <= bytes.length, "u32 field is truncated");
    return (bytes[offset] & 0xff)
        | ((bytes[offset + 1] & 0xff) << 8)
        | ((bytes[offset + 2] & 0xff) << 16)
        | ((bytes[offset + 3] & 0xff) << 24);
  }

  private static List<byte[]> nonZeroSorted(final List<byte[]> values, final String field) {
    final List<byte[]> filtered = new ArrayList<>();
    for (final byte[] value : values) {
      final byte[] fixed = fixedBytes(value, 32, field);
      if (!isZero(fixed)) {
        filtered.add(fixed);
      }
    }
    require(!filtered.isEmpty(), field + " must contain at least one non-zero value");
    Collections.sort(filtered, KagemushaRecursiveSpendRequestCodecs::compareUnsigned);
    for (int i = 1; i < filtered.size(); i++) {
      require(!Arrays.equals(filtered.get(i - 1), filtered.get(i)), field + " must not contain duplicates");
    }
    return filtered;
  }

  private static final class PrivacyBuildResult {
    final byte[] proof;

    PrivacyBuildResult(final byte[] proof) {
      this.proof = Arrays.copyOf(proof, proof.length);
    }
  }

  private static final class OpenVerifyEnvelopeValue {
    final byte[] archive;
    final String circuitId;
    final byte[] vkHash;
    final byte[] publicInputs;
    final byte[] proofBytes;

    OpenVerifyEnvelopeValue(
        final byte[] archive,
        final String circuitId,
        final byte[] vkHash,
        final byte[] publicInputs,
        final byte[] proofBytes) {
      this.archive = Arrays.copyOf(archive, archive.length);
      this.circuitId = circuitId;
      this.vkHash = Arrays.copyOf(vkHash, vkHash.length);
      this.publicInputs = Arrays.copyOf(publicInputs, publicInputs.length);
      this.proofBytes = Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  private static final class TransferPublicInputs {
    final byte[] rootBefore;
    final List<byte[]> inputNullifiers;
    final List<byte[]> outputCommitments;

    TransferPublicInputs(
        final byte[] rootBefore, final List<byte[]> inputNullifiers, final List<byte[]> outputCommitments) {
      this.rootBefore = Arrays.copyOf(rootBefore, rootBefore.length);
      this.inputNullifiers = copyByteList(inputNullifiers);
      this.outputCommitments = copyByteList(outputCommitments);
    }
  }

  private static final class VerifierKeyIdValue {
    final String backend;
    final String name;

    VerifierKeyIdValue(final String backend, final String name) {
      this.backend = backend;
      this.name = name;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof VerifierKeyIdValue)) {
        return false;
      }
      final VerifierKeyIdValue rhs = (VerifierKeyIdValue) other;
      return backend.equals(rhs.backend) && name.equals(rhs.name);
    }

    @Override
    public int hashCode() {
      return 31 * backend.hashCode() + name.hashCode();
    }
  }

  private static final class VerifyingKeyBoxValue {
    final String backend;
    final byte[] bytes;

    VerifyingKeyBoxValue(final String backend, final byte[] bytes) {
      this.backend = backend;
      this.bytes = Arrays.copyOf(bytes, bytes.length);
    }
  }

  private static final class DecodedVerifierRecord {
    final String circuitId;
    final String namespace;
    final long backendTag;
    final String curve;
    final byte[] publicInputsSchemaHash;
    final byte[] commitment;
    final int vkLen;
    final int maxProofBytes;
    final VerifyingKeyBoxValue key;
    final int status;

    DecodedVerifierRecord(
        final String circuitId,
        final String namespace,
        final long backendTag,
        final String curve,
        final byte[] publicInputsSchemaHash,
        final byte[] commitment,
        final int vkLen,
        final int maxProofBytes,
        final VerifyingKeyBoxValue key,
        final int status) {
      this.circuitId = circuitId;
      this.namespace = namespace;
      this.backendTag = backendTag;
      this.curve = curve;
      this.publicInputsSchemaHash = Arrays.copyOf(publicInputsSchemaHash, publicInputsSchemaHash.length);
      this.commitment = Arrays.copyOf(commitment, commitment.length);
      this.vkLen = vkLen;
      this.maxProofBytes = maxProofBytes;
      this.key = key;
      this.status = status;
    }
  }

  private static final class VerifierRecordValue {
    final VerifierKeyIdValue id;
    final byte[] recordPayload;
    final byte[] commitment;
    final VerifyingKeyBoxValue key;

    VerifierRecordValue(
        final VerifierKeyIdValue id,
        final byte[] recordPayload,
        final byte[] commitment,
        final VerifyingKeyBoxValue key) {
      this.id = id;
      this.recordPayload = Arrays.copyOf(recordPayload, recordPayload.length);
      this.commitment = Arrays.copyOf(commitment, commitment.length);
      this.key = key;
    }
  }

  private static final class PreparedVerifiedFoldHop {
    final String chainId;
    final String asset;
    final byte[] rootAfter;
    final TransferPublicInputs publicInputs;
    final OpenVerifyEnvelopeValue envelope;
    final VerifierRecordValue verifierRecord;

    PreparedVerifiedFoldHop(
        final String chainId,
        final String asset,
        final byte[] rootAfter,
        final TransferPublicInputs publicInputs,
        final OpenVerifyEnvelopeValue envelope,
        final VerifierRecordValue verifierRecord) {
      this.chainId = chainId;
      this.asset = asset;
      this.rootAfter = Arrays.copyOf(rootAfter, rootAfter.length);
      this.publicInputs = publicInputs;
      this.envelope = envelope;
      this.verifierRecord = verifierRecord;
    }
  }

  private static List<byte[]> copyByteList(final List<byte[]> values) {
    final List<byte[]> out = new ArrayList<>();
    for (final byte[] value : values) {
      out.add(Arrays.copyOf(value, value.length));
    }
    return out;
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }
}
