package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountAddress.AccountAddressException;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
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
  public static final String SCHEMA_PROOF_ATTACHMENT =
      "iroha_data_model::proof::ProofAttachment";
  public static final String SCHEMA_VERIFYING_KEY_RECORD =
      "iroha_data_model::proof::VerifyingKeyRecord";

  private static final int REQUEST_FLAGS = NoritoHeader.COMPACT_LEN;
  private static final BigInteger MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final BigInteger BYTE_MASK = BigInteger.valueOf(0xffL);
  private static final int MULTISIG_POLICY_VERSION = 1;
  private static final char[] HEX = "0123456789abcdef".toCharArray();

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
    return new SpendBundleSummary(
        accumulator.hopCount,
        proofCircuitId,
        accumulator.asset,
        accumulator.chainId,
        accumulator.initialRoot,
        accumulator.finalRoot,
        accumulator.currentNote);
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
      requirePayloadArchive(this.recordBytes, SCHEMA_VERIFYING_KEY_RECORD, "recordBytes");
    }

    public byte[] recordBytes() {
      return Arrays.copyOf(recordBytes, recordBytes.length);
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
      requireValidNestedArchive(this.pallasOpenEnvelopes, "pallasOpenEnvelopes");
      requireValidNestedArchive(this.lineageProvingKeyArchive, "lineageProvingKeyArchive");
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
      requireValidNestedArchive(this.pallasOpenEnvelopes, "pallasOpenEnvelopes");
      if (this.previousProofOpenEnvelopes != null) {
        require(
            this.previousProofOpenEnvelopes.length
                <= KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
            "previousProofOpenEnvelopes must not exceed "
                + KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
                + " bytes");
        requireValidNestedArchive(this.previousProofOpenEnvelopes, "previousProofOpenEnvelopes");
      }
      if (this.lineageProvingKeyArchive != null) {
        requireValidNestedArchive(this.lineageProvingKeyArchive, "lineageProvingKeyArchive");
      }

      final SpendBundleSummary previousSummary = decodeBundle(this.previousBundle);
      final String normalizedOutput =
          KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputProofCircuitId);
      require(
          KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
              previousSummary.proofCircuitId, normalizedOutput, previousSummary.hopCount),
          "outputProofCircuitId is not valid for the previous bundle");
      if (KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
          previousSummary.proofCircuitId)) {
        require(previousLineageVerifierRecord != null,
            "previousLineageVerifierRecord is required for lineage previous bundles");
      }
      if (KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
          normalizedOutput, previousSummary.hopCount)) {
        require(previousProofOpenEnvelopes != null,
            "previousProofOpenEnvelopes is required for lineage append output");
      }
      if (KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput)) {
        require(lineageVerifierKey != null && lineageVerifierKey.length > 0,
            "lineageVerifierKey is required for lineage append output");
        require(lineageProvingKeyArchive != null && lineageProvingKeyArchive.length > 0,
            "lineageProvingKeyArchive is required for lineage append output");
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
      this.bundle = copyOf(bundle, "bundle");
      this.recipient = Objects.requireNonNull(recipient, "recipient");
      this.publicAmount = canonicalU128Decimal(publicAmount, "publicAmount");
      this.redeemProof = copyOf(redeemProof, "redeemProof");
      this.lineageWitness = copyNullable(lineageWitness);
      this.lineageVerifierRecord = lineageVerifierRecord;
      this.blockHeight = blockHeight;
      requireNonNegativeHeight(blockHeight);
      requireNonBlankUnpadded(this.recipient, "recipient");
      if (this.lineageWitness != null) {
        requireValidNestedArchive(this.lineageWitness, "lineageWitness");
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
  }

  /** Read-only summary of a recursive spend bundle. */
  public static final class SpendBundleSummary {
    public final int hopCount;
    public final String proofCircuitId;
    public final String asset;
    public final String chainId;
    private final byte[] initialRoot;
    private final byte[] finalRoot;
    public final SpendableNoteDescriptor currentNote;

    public SpendBundleSummary(
        final int hopCount,
        final String proofCircuitId,
        final String asset,
        final String chainId,
        final byte[] initialRoot,
        final byte[] finalRoot,
        final SpendableNoteDescriptor currentNote) {
      this.hopCount = hopCount;
      this.proofCircuitId = Objects.requireNonNull(proofCircuitId, "proofCircuitId");
      this.asset = Objects.requireNonNull(asset, "asset");
      this.chainId = Objects.requireNonNull(chainId, "chainId");
      this.initialRoot = fixedBytes(initialRoot, 32, "initialRoot");
      this.finalRoot = fixedBytes(finalRoot, 32, "finalRoot");
      this.currentNote = Objects.requireNonNull(currentNote, "currentNote");
    }

    public byte[] initialRoot() {
      return Arrays.copyOf(initialRoot, initialRoot.length);
    }

    public byte[] finalRoot() {
      return Arrays.copyOf(finalRoot, finalRoot.length);
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
    final int hopCount;
    final SpendableNoteDescriptor currentNote;

    AccumulatorSummary(
        final String chainId,
        final String asset,
        final byte[] initialRoot,
        final byte[] finalRoot,
        final int hopCount,
        final SpendableNoteDescriptor currentNote) {
      this.chainId = chainId;
      this.asset = asset;
      this.initialRoot = initialRoot;
      this.finalRoot = finalRoot;
      this.hopCount = hopCount;
      this.currentNote = currentNote;
    }
  }

  private static AccumulatorSummary readAccumulatorSummary(final byte[] payload, final int flags) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String chainId = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readChainId);
    final String asset = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readAssetDefinitionId);
    final byte[] initialRoot = readField(decoder, child -> readFixedBytes(child, 32, "initial_root"));
    final byte[] finalRoot = readField(decoder, child -> readFixedBytes(child, 32, "final_root"));
    skipFields(decoder, 1);
    final int hopCount = readField(decoder, child -> checkedInt(child.readUInt(32), "hop_count"));
    skipFields(decoder, 15);
    final SpendableNoteDescriptor currentNote =
        readField(decoder, KagemushaRecursiveSpendRequestCodecs::readSpendableNote);
    require(decoder.remaining() == 0, "Trailing bytes after accumulator");
    return new AccumulatorSummary(chainId, asset, initialRoot, finalRoot, hopCount, currentNote);
  }

  private static String readRecursiveProofCircuitId(final byte[] payload, final int flags) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    final byte[] verifierKeyIdPayload = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readRemainingBytes);
    return readVerifyingKeyIdName(verifierKeyIdPayload, flags);
  }

  private static byte[] verifyingKeyBoxPayload(final byte[] bytes) {
    require(bytes != null && bytes.length > 0, "lineageVerifierKey must not be empty");
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    writeField(encoder, child -> writeString(child, KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND));
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
    final BigInteger integer = new BigInteger(value);
    require(integer.compareTo(BigInteger.ZERO) > 0, field + " must be greater than zero");
    require(integer.compareTo(MAX_U128) <= 0, field + " must fit in u128");
    return integer.toString();
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

  private static void writeSpendableNote(final NoritoEncoder encoder, final SpendableNoteDescriptor value) {
    writeField(encoder, child -> writeConstVec(child, value.noteCommitment()));
    writeField(encoder, child -> writeConstVec(child, value.spendNullifier()));
    writeField(encoder, child -> writeNumeric(child, value.amount));
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

  private static String readAssetDefinitionId(final NoritoDecoder decoder) {
    final byte[] bytes = readFixedBytes(decoder, 16, "asset");
    try {
      return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
    } catch (final IllegalArgumentException ignored) {
      return "hex:" + toHex(bytes);
    }
  }

  private static String readVerifyingKeyIdName(final byte[] payload, final int flags) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, flags);
    readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    final String name = readField(decoder, KagemushaRecursiveSpendRequestCodecs::readString);
    require(decoder.remaining() == 0, "Trailing bytes after verifier key id");
    requirePortableId(name, "verifierKeyId");
    return name;
  }

  private static <T> T readField(final NoritoDecoder parent, final FieldReader<T> readPayload) {
    final int length = checkedInt(parent.readLength(compact(parent)), "field length");
    final NoritoDecoder child = new NoritoDecoder(parent.readBytes(length), parent.flags(), parent.flagsHint());
    final T value = readPayload.read(child);
    require(child.remaining() == 0, "Trailing bytes after field decode");
    return value;
  }

  private static String readString(final NoritoDecoder decoder) {
    final int length = checkedInt(decoder.readLength(compact(decoder)), "string length");
    return new String(decoder.readBytes(length), StandardCharsets.UTF_8);
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

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }
}
