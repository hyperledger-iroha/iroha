package org.hyperledger.iroha.android.norito;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.function.Supplier;
import org.hyperledger.iroha.android.client.MultisigProposeRequest;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.ContractInvocation;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.ExecutableBatchItem;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyMerkleWitness;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyProof;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyWitness;
import org.hyperledger.iroha.android.model.instructions.ProofAttachment;
import org.hyperledger.iroha.android.model.instructions.ProofVerifierKeyRef;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito adapter that mirrors the {@link TransactionPayload} structure used by the Android library.
 * IVM bytecode payloads are encoded directly. Instruction payloads must be provided as wire-framed
 * Norito blobs (wire id + Norito header). Metadata values use the one canonical JSON spelling
 * required by the Rust {@code Json} wrapper.
 */
final class TransactionPayloadAdapter implements TypeAdapter<TransactionPayload> {

  // Validation never exposes rendered account text. Controller bytes are independent of the I105
  // display prefix, so this private synthetic context permits canonical decode/re-encode checks
  // without acquiring a public or network default.
  private static final int CANONICAL_VALIDATION_DISCRIMINANT = 0;
  private static final long PROOF_BOX_MAX_ENCODED_BYTES = 64L * 1024L * 1024L;
  private static final long PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES = 8L + 256L;
  private static final long VERIFYING_KEY_REF_MAX_ENCODED_BYTES =
      2L * (8L + PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES);
  private static final long OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES = 1L + 8L + 32L * 9L;
  private static final long LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES = 1L + 8L + 32L;
  private static final long LANE_PRIVACY_MAX_AUDIT_PATH_ENCODED_BYTES =
      8L
          + (LanePrivacyMerkleWitness.MAX_DEPTH + 1L) * 8L
          + LanePrivacyMerkleWitness.MAX_DEPTH
              * LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES;
  private static final long LANE_PRIVACY_MAX_OPTION_ENCODED_BYTES = 16L * 1024L;
  private static final long LANE_PRIVACY_MERKLE_TAG = 0L;
  private static final ThreadLocal<Integer> CHAIN_DISCRIMINANT = new ThreadLocal<>();
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<String> ACCOUNT_ID_ADAPTER = new AccountIdAdapter();
  private static final TypeAdapter<NetworkId> TRANSACTION_DOMAIN_ADAPTER =
      new TransactionDomainAdapter();
  private static final TypeAdapter<String> JSON_VALUE_ADAPTER = new JsonAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT32_AS_LONG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTE_VEC_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<byte[]> FIXED_HASH_ADAPTER = NoritoAdapters.fixedBytes(32);
  private static final TypeAdapter<byte[]> HASH_ARRAY_ADAPTER = new FixedHashArrayAdapter();
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_HASH_ADAPTER =
      NoritoAdapters.option(HASH_ARRAY_ADAPTER);
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_LANE_PRIVACY_HASH_ADAPTER =
      NoritoAdapters.option(FIXED_HASH_ADAPTER);
  private static final TypeAdapter<List<byte[]>> LANE_PRIVACY_AUDIT_PATH_ADAPTER =
      new LanePrivacyAuditPathAdapter();
  private static final TypeAdapter<LanePrivacyMerkleProofValue>
      LANE_PRIVACY_MERKLE_PROOF_ADAPTER = new LanePrivacyMerkleProofAdapter();
  private static final TypeAdapter<LanePrivacyMerkleWitness>
      LANE_PRIVACY_MERKLE_WITNESS_ADAPTER = new LanePrivacyMerkleWitnessAdapter();
  private static final TypeAdapter<LanePrivacyWitness> LANE_PRIVACY_WITNESS_ADAPTER =
      new LanePrivacyWitnessAdapter();
  private static final TypeAdapter<LanePrivacyProof> LANE_PRIVACY_PROOF_ADAPTER =
      new LanePrivacyProofAdapter();
  private static final TypeAdapter<Optional<LanePrivacyProof>> OPTIONAL_LANE_PRIVACY_ADAPTER =
      NoritoAdapters.option(LANE_PRIVACY_PROOF_ADAPTER);
  private static final TypeAdapter<byte[]> CONTRACT_ARGUMENT_RECORD_ADAPTER =
      new ContractArgumentRecordAdapter();
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_CONTRACT_ARGUMENT_RECORD_ADAPTER =
      NoritoAdapters.option(CONTRACT_ARGUMENT_RECORD_ADAPTER);
  private static final TypeAdapter<byte[]> IVM_BYTECODE_ADAPTER = new IvmBytecodeAdapter();
  private static final TypeAdapter<Optional<String>> OPTIONAL_STRING_ADAPTER =
      NoritoAdapters.option(STRING_ADAPTER);
  private static final TypeAdapter<Optional<String>> OPTIONAL_ACCOUNT_ID_ADAPTER =
      NoritoAdapters.option(ACCOUNT_ID_ADAPTER);
  private static final TypeAdapter<InstructionBox> INSTRUCTION_ADAPTER = new InstructionAdapter();
  private static final TypeAdapter<List<InstructionBox>> INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(INSTRUCTION_ADAPTER);
  private static final TypeAdapter<ContractInvocation> CONTRACT_INVOCATION_ADAPTER =
      new ContractInvocationAdapter();
  private static final TypeAdapter<List<ExecutableBatchItem>> EXECUTABLE_BATCH_ADAPTER =
      NoritoAdapters.sequence(new ExecutableBatchItemAdapter());
  private static final TypeAdapter<List<byte[]>> ENCODED_INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new EncodedInstructionAdapter());
  private static final TypeAdapter<Long> ENUM_TAG_ADAPTER = NoritoAdapters.uint(32);
  private static final long EXECUTABLE_INSTRUCTIONS_TAG = 0L;
  private static final long TRANSACTION_DOMAIN_NETWORK_TAG = 0L;
  private static final long TRANSACTION_DOMAIN_GENESIS_TAG = 1L;
  private static final long EXECUTABLE_CONTRACT_CALL_TAG = 1L;
  private static final long EXECUTABLE_IVM_TAG = 2L;
  private static final long EXECUTABLE_IVM_PROVED_TAG = 3L;
  private static final long EXECUTABLE_BATCH_TAG = 4L;
  private static final long BATCH_ITEM_INSTRUCTION_TAG = 0L;
  private static final long BATCH_ITEM_CONTRACT_CALL_TAG = 1L;
  private static final long FEE_PAYER_AUTHORITY_TAG = 0L;
  private static final long FEE_PAYER_SPONSOR_TAG = 1L;
  private static final long FEE_CHARGE_NEXUS_TAG = 0L;
  private static final long FEE_CHARGE_PIPELINE_GAS_TAG = 1L;
  private static final TypeAdapter<Optional<Long>> TTL_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<Optional<Long>> NONCE_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(32));
  private static final TypeAdapter<Optional<Long>> GAS_LIMIT_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<FeeChargeKind> FEE_CHARGE_KIND_ADAPTER =
      new FeeChargeKindAdapter();
  private static final TypeAdapter<String> ASSET_DEFINITION_ID_ADAPTER =
      new AssetDefinitionIdAdapter();
  private static final TypeAdapter<NumericV1.QuantityValue> QUANTITY_ADAPTER =
      new QuantityAdapter();
  private static final TypeAdapter<List<FeeChargeLimit>> FEE_CHARGE_LIMIT_LIST_ADAPTER =
      NoritoAdapters.sequence(new FeeChargeLimitAdapter());
  private static final TypeAdapter<FeeSponsorProgramId> FEE_SPONSOR_PROGRAM_ID_ADAPTER =
      new FeeSponsorProgramIdAdapter();
  private static final TypeAdapter<FeePaymentIntent.Authority> AUTHORITY_FEE_PAYMENT_ADAPTER =
      new AuthorityFeePaymentAdapter();
  private static final TypeAdapter<FeePaymentIntent.Sponsor> SPONSOR_FEE_PAYMENT_ADAPTER =
      new SponsorFeePaymentAdapter();
  private static final TypeAdapter<FeePaymentIntent> FEE_PAYMENT_ADAPTER =
      new FeePaymentIntentAdapter();
  private static final TypeAdapter<Executable> EXECUTABLE_ADAPTER = new ExecutableAdapter();
  private static final TypeAdapter<Map<String, JsonValue>> METADATA_ADAPTER = new MetadataAdapter();
  private static final TypeAdapter<ProofBoxValue> PROOF_BOX_ADAPTER = new ProofBoxAdapter();
  private static final TypeAdapter<ProofVerifierKeyRef> PROOF_VERIFIER_KEY_REF_ADAPTER =
      new ProofVerifierKeyRefAdapter();
  private static final TypeAdapter<ProofAttachment> PROOF_ATTACHMENT_ADAPTER =
      new ProofAttachmentAdapter();
  private static final TypeAdapter<List<ProofAttachment>> PROOF_ATTACHMENT_SEQUENCE_ADAPTER =
      NoritoAdapters.sequence(PROOF_ATTACHMENT_ADAPTER);
  private static final TypeAdapter<List<ProofAttachment>> PROOF_ATTACHMENT_LIST_ADAPTER =
      new ProofAttachmentListAdapter();
  private static final TypeAdapter<Optional<List<ProofAttachment>>> ATTACHMENTS_OPTION_ADAPTER =
      NoritoAdapters.option(PROOF_ATTACHMENT_LIST_ADAPTER);
  private static final String INSTRUCTION_BOX_SCHEMA =
      "(alloc::string::String, alloc::vec::Vec<u8>)";
  private static final String MULTISIG_PROPOSE_DTO_SCHEMA =
      "iroha_torii::routing::MultisigProposeDto";
  private final int chainDiscriminant;

  private TransactionPayloadAdapter(final int chainDiscriminant) {
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    this.chainDiscriminant = chainDiscriminant;
  }

  static TransactionPayloadAdapter forChain(final int chainDiscriminant) {
    return new TransactionPayloadAdapter(chainDiscriminant);
  }

  static void validateCanonicalPayloadBytes(final byte[] encoded) {
    if (encoded == null) {
      throw new IllegalArgumentException("encoded transaction payload must not be null");
    }
    final TransactionPayloadAdapter validator =
        forChain(CANONICAL_VALIDATION_DISCRIMINANT);
    final TransactionPayload decoded = NoritoCodec.decodeAdaptive(encoded, validator);
    final byte[] reencoded = NoritoCodec.encodeAdaptive(decoded, validator).payload();
    if (!Arrays.equals(encoded, reencoded)) {
      throw new IllegalArgumentException(
          "transaction payload bytes are not the exact canonical encoding");
    }
  }

  @Override
  public void encode(final NoritoEncoder encoder, final TransactionPayload value) {
    withChainContext(
        chainDiscriminant,
        () -> {
          if (value.executable().requiresTransactionGasLimit()
              && value.feePayment().gasLimit() == null) {
            throw new IllegalArgumentException(
                "feePayment.gasLimit is required for IVM and contract-call executables");
          }
          encodeSizedField(encoder, TRANSACTION_DOMAIN_ADAPTER, value.networkId());
          encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.authority());
          encodeSizedField(encoder, UINT64_ADAPTER, value.creationTimeMs());
          encodeSizedField(encoder, EXECUTABLE_ADAPTER, value.executable());
          encodeSizedField(encoder, TTL_ADAPTER, value.timeToLiveMs());
          encodeSizedField(encoder, NONCE_ADAPTER, value.nonce());
          encodeSizedField(encoder, FEE_PAYMENT_ADAPTER, value.feePayment());
          encodeSizedField(encoder, METADATA_ADAPTER, value.metadata());
          encodeSizedField(encoder, ATTACHMENTS_OPTION_ADAPTER, value.attachments());
          return null;
        });
  }

  @Override
  public TransactionPayload decode(final NoritoDecoder decoder) {
    return withChainContext(
        chainDiscriminant,
        () -> {
          final NetworkId networkId = decodeSizedField(decoder, TRANSACTION_DOMAIN_ADAPTER);
          final String authority = decodeAuthorityField(decoder);
          final long creationTimeMs = decodeSizedField(decoder, UINT64_ADAPTER);
          final Executable executable = decodeSizedField(decoder, EXECUTABLE_ADAPTER);
          final Optional<Long> ttl = decodeSizedField(decoder, TTL_ADAPTER);
          if (!ttl.isPresent()) {
            throw new IllegalArgumentException(
                "TransactionPayload.time_to_live_ms must be signature-bound");
          }
          final Optional<Long> nonceRaw = decodeSizedField(decoder, NONCE_ADAPTER);
          final FeePaymentIntent feePayment = decodeSizedField(decoder, FEE_PAYMENT_ADAPTER);
          final Map<String, JsonValue> metadata =
              new LinkedHashMap<>(decodeSizedField(decoder, METADATA_ADAPTER));
          final Optional<List<ProofAttachment>> attachments =
              decodeSizedField(decoder, ATTACHMENTS_OPTION_ADAPTER);

          final TransactionPayload.Builder builder =
              TransactionPayload.builder()
                  .setNetworkId(networkId)
                  .setAuthority(authority)
                  .setCreationTimeMs(creationTimeMs)
                  .setExecutable(executable)
                  .setFeePayment(feePayment)
                  .setMetadata(metadata)
                  .setAttachments(attachments.orElse(null));
          builder.setTimeToLiveMs(ttl.get());
          nonceRaw.ifPresent(builder::setNonce);
          return builder.buildDecodedForCodec();
        });
  }

  private static final class ProofBoxValue {
    private final String backend;
    private final byte[] bytes;

    private ProofBoxValue(final String backend, final byte[] bytes) {
      this.backend = backend;
      this.bytes = bytes.clone();
    }

    private String backend() {
      return backend;
    }

    private byte[] bytes() {
      return bytes.clone();
    }
  }

  private static final class ProofBoxAdapter implements TypeAdapter<ProofBoxValue> {
    @Override
    public void encode(final NoritoEncoder encoder, final ProofBoxValue value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.backend());
      encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value.bytes());
    }

    @Override
    public ProofBoxValue decode(final NoritoDecoder decoder) {
      return new ProofBoxValue(
          decodeBoundedSizedField(
              decoder,
              STRING_ADAPTER,
              PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
              "ProofBox backend"),
          decodeSizedField(decoder, RAW_BYTE_VEC_ADAPTER));
    }
  }

  private static final class ProofVerifierKeyRefAdapter
      implements TypeAdapter<ProofVerifierKeyRef> {
    @Override
    public void encode(final NoritoEncoder encoder, final ProofVerifierKeyRef value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.backend());
      encodeSizedField(encoder, STRING_ADAPTER, value.name());
    }

    @Override
    public ProofVerifierKeyRef decode(final NoritoDecoder decoder) {
      return new ProofVerifierKeyRef(
          decodeBoundedSizedField(
              decoder,
              STRING_ADAPTER,
              PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
              "verifier-key backend"),
          decodeBoundedSizedField(
              decoder,
              STRING_ADAPTER,
              PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
              "verifier-key name"));
    }
  }

  private static final class LanePrivacyAuditPathAdapter
      implements TypeAdapter<List<byte[]>> {
    private final TypeAdapter<List<Optional<byte[]>>> delegate =
        NoritoAdapters.sequence(OPTIONAL_LANE_PRIVACY_HASH_ADAPTER);

    @Override
    public void encode(final NoritoEncoder encoder, final List<byte[]> value) {
      if (value.size() < 1 || value.size() > LanePrivacyMerkleWitness.MAX_DEPTH) {
        throw new IllegalArgumentException(
            "lane privacy audit path depth must be between 1 and "
                + LanePrivacyMerkleWitness.MAX_DEPTH);
      }
      final List<Optional<byte[]>> optional = new ArrayList<>(value.size());
      for (final byte[] sibling : value) {
        optional.add(Optional.of(sibling.clone()));
      }
      delegate.encode(encoder, optional);
    }

    @Override
    public List<byte[]> decode(final NoritoDecoder decoder) {
      final long countValue = decoder.readLength(false);
      if (countValue < 1L || countValue > LanePrivacyMerkleWitness.MAX_DEPTH) {
        throw new IllegalArgumentException(
            "lane privacy audit path depth must be between 1 and "
                + LanePrivacyMerkleWitness.MAX_DEPTH);
      }
      final int count = (int) countValue;
      if ((decoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        return decodePacked(decoder, count);
      }
      return decodeDelimited(decoder, count);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }

    private static List<byte[]> decodeDelimited(
        final NoritoDecoder decoder, final int count) {
      final List<byte[]> siblings = new ArrayList<>(count);
      for (int index = 0; index < count; index++) {
        final long length = decoder.readLength(decoder.compactLenActive());
        if (length < 1L || length > LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES) {
          throw new IllegalArgumentException(
              "lane privacy sibling " + index + " payload is oversized");
        }
        siblings.add(decodeSibling(decoder.readBytes((int) length), decoder, index));
      }
      return siblings;
    }

    private static List<byte[]> decodePacked(final NoritoDecoder decoder, final int count) {
      long previous = decoder.readUInt(64);
      if (previous != 0L) {
        throw new IllegalArgumentException("packed lane privacy offsets must start at zero");
      }
      final List<Integer> sizes = new ArrayList<>(count);
      for (int index = 0; index < count; index++) {
        final long current = decoder.readUInt(64);
        if (current < previous) {
          throw new IllegalArgumentException("packed lane privacy offsets must be monotonic");
        }
        final long size = current - previous;
        if (size < 1L || size > LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES) {
          throw new IllegalArgumentException(
              "lane privacy sibling " + index + " payload is oversized");
        }
        sizes.add((int) size);
        previous = current;
      }
      if (previous != decoder.remaining()) {
        throw new IllegalArgumentException(
            "packed lane privacy offsets must cover the complete path payload");
      }
      final List<byte[]> siblings = new ArrayList<>(count);
      for (int index = 0; index < count; index++) {
        siblings.add(decodeSibling(decoder.readBytes(sizes.get(index)), decoder, index));
      }
      return siblings;
    }

    private static byte[] decodeSibling(
        final byte[] payload, final NoritoDecoder parent, final int index) {
      final NoritoDecoder child =
          new NoritoDecoder(payload, parent.flags(), parent.flagsHint());
      final Optional<byte[]> sibling = OPTIONAL_LANE_PRIVACY_HASH_ADAPTER.decode(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException(
            "lane privacy sibling " + index + " has trailing bytes");
      }
      if (!sibling.isPresent()) {
        throw new IllegalArgumentException(
            "lane privacy sibling " + index + " must be present");
      }
      final byte[] bytes = sibling.get();
      if ((bytes[bytes.length - 1] & 1) != 1) {
        throw new IllegalArgumentException(
            "lane privacy sibling "
                + index
                + " is missing the Iroha prehashed marker");
      }
      return bytes;
    }
  }

  private static final class LanePrivacyMerkleProofValue {
    private final long leafIndex;
    private final List<byte[]> auditPath;

    private LanePrivacyMerkleProofValue(final long leafIndex, final List<byte[]> auditPath) {
      this.leafIndex = leafIndex;
      this.auditPath = auditPath;
    }
  }

  private static final class LanePrivacyMerkleProofAdapter
      implements TypeAdapter<LanePrivacyMerkleProofValue> {
    @Override
    public void encode(final NoritoEncoder encoder, final LanePrivacyMerkleProofValue value) {
      encodeSizedField(encoder, UINT32_ADAPTER, value.leafIndex);
      encodeSizedField(encoder, LANE_PRIVACY_AUDIT_PATH_ADAPTER, value.auditPath);
    }

    @Override
    public LanePrivacyMerkleProofValue decode(final NoritoDecoder decoder) {
      final long leafIndex = decodeSizedField(decoder, UINT32_ADAPTER);
      final List<byte[]> auditPath =
          decodeBoundedSizedField(
              decoder,
              LANE_PRIVACY_AUDIT_PATH_ADAPTER,
              LANE_PRIVACY_MAX_AUDIT_PATH_ENCODED_BYTES,
              "lane privacy audit path");
      return new LanePrivacyMerkleProofValue(leafIndex, auditPath);
    }
  }

  private static final class LanePrivacyMerkleWitnessAdapter
      implements TypeAdapter<LanePrivacyMerkleWitness> {
    @Override
    public void encode(final NoritoEncoder encoder, final LanePrivacyMerkleWitness value) {
      encodeSizedField(encoder, HASH_ARRAY_ADAPTER, value.leaf());
      encodeSizedField(
          encoder,
          LANE_PRIVACY_MERKLE_PROOF_ADAPTER,
          new LanePrivacyMerkleProofValue(value.leafIndex(), value.auditPath()));
    }

    @Override
    public LanePrivacyMerkleWitness decode(final NoritoDecoder decoder) {
      final byte[] leaf = decodeSizedField(decoder, HASH_ARRAY_ADAPTER);
      final LanePrivacyMerkleProofValue proof =
          decodeSizedField(decoder, LANE_PRIVACY_MERKLE_PROOF_ADAPTER);
      return new LanePrivacyMerkleWitness(leaf, proof.leafIndex, proof.auditPath);
    }
  }

  private static final class LanePrivacyWitnessAdapter
      implements TypeAdapter<LanePrivacyWitness> {
    @Override
    public void encode(final NoritoEncoder encoder, final LanePrivacyWitness value) {
      if (!(value instanceof LanePrivacyWitness.Merkle merkle)) {
        throw new IllegalArgumentException("unknown lane privacy witness subtype");
      }
      ENUM_TAG_ADAPTER.encode(encoder, LANE_PRIVACY_MERKLE_TAG);
      encodeSizedField(encoder, LANE_PRIVACY_MERKLE_WITNESS_ADAPTER, merkle.value());
    }

    @Override
    public LanePrivacyWitness decode(final NoritoDecoder decoder) {
      final long tag = ENUM_TAG_ADAPTER.decode(decoder);
      if (tag != LANE_PRIVACY_MERKLE_TAG) {
        throw new IllegalArgumentException("unknown lane privacy witness tag: " + tag);
      }
      return new LanePrivacyWitness.Merkle(
          decodeSizedField(decoder, LANE_PRIVACY_MERKLE_WITNESS_ADAPTER));
    }
  }

  private static final class LanePrivacyProofAdapter implements TypeAdapter<LanePrivacyProof> {
    @Override
    public void encode(final NoritoEncoder encoder, final LanePrivacyProof value) {
      encodeSizedField(encoder, UINT16_ADAPTER, (long) value.commitmentId());
      encodeSizedField(encoder, LANE_PRIVACY_WITNESS_ADAPTER, value.witness());
    }

    @Override
    public LanePrivacyProof decode(final NoritoDecoder decoder) {
      return new LanePrivacyProof(
          decodeSizedField(decoder, UINT16_ADAPTER).intValue(),
          decodeSizedField(decoder, LANE_PRIVACY_WITNESS_ADAPTER));
    }
  }

  private static final class ProofAttachmentAdapter implements TypeAdapter<ProofAttachment> {
    @Override
    public void encode(final NoritoEncoder encoder, final ProofAttachment value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.backend());
      encodeSizedField(
          encoder,
          PROOF_BOX_ADAPTER,
          new ProofBoxValue(value.backend(), value.proofBytes()));
      encodeSizedField(encoder, PROOF_VERIFIER_KEY_REF_ADAPTER, value.verifyingKeyRef());

      final byte[] commitment = value.verifyingKeyCommitment();
      final byte[] envelopeHash = value.envelopeHash();
      final LanePrivacyProof lanePrivacy = value.lanePrivacy();
      if (commitment != null || envelopeHash != null || lanePrivacy != null) {
        encodeSizedField(
            encoder, OPTIONAL_HASH_ADAPTER, Optional.ofNullable(commitment));
      }
      if (envelopeHash != null || lanePrivacy != null) {
        encodeSizedField(encoder, OPTIONAL_HASH_ADAPTER, Optional.ofNullable(envelopeHash));
      }
      if (lanePrivacy != null) {
        encodeSizedField(
            encoder, OPTIONAL_LANE_PRIVACY_ADAPTER, Optional.of(lanePrivacy));
      }
    }

    @Override
    public ProofAttachment decode(final NoritoDecoder decoder) {
      final String backend =
          decodeBoundedSizedField(
              decoder,
              STRING_ADAPTER,
              PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
              "ProofAttachment backend");
      final ProofBoxValue proof =
          decodeBoundedSizedField(
              decoder, PROOF_BOX_ADAPTER, PROOF_BOX_MAX_ENCODED_BYTES, "ProofBox");
      if (!proof.backend().equals(backend)) {
        throw new IllegalArgumentException(
            "proof.backend must match attachment backend");
      }
      final ProofVerifierKeyRef verifyingKeyRef =
          decodeBoundedSizedField(
              decoder,
              PROOF_VERIFIER_KEY_REF_ADAPTER,
              VERIFYING_KEY_REF_MAX_ENCODED_BYTES,
              "verifier-key reference");
      if (!verifyingKeyRef.backend().equals(backend)) {
        throw new IllegalArgumentException(
            "vk_ref.backend must match attachment backend");
      }

      final byte[] commitment =
          decoder.remaining() == 0
              ? null
              : decodeBoundedSizedField(
                      decoder,
                      OPTIONAL_HASH_ADAPTER,
                      OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES,
                      "verifier-key commitment")
                  .orElse(null);
      final byte[] envelopeHash =
          decoder.remaining() == 0
              ? null
              : decodeBoundedSizedField(
                      decoder,
                      OPTIONAL_HASH_ADAPTER,
                      OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES,
                      "envelope hash")
                  .orElse(null);
      final LanePrivacyProof lanePrivacy =
          decoder.remaining() == 0
              ? null
              : decodeBoundedSizedField(
                      decoder,
                      OPTIONAL_LANE_PRIVACY_ADAPTER,
                      LANE_PRIVACY_MAX_OPTION_ENCODED_BYTES,
                      "lane privacy proof")
                  .orElse(null);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("trailing ProofAttachment fields");
      }

      return new ProofAttachment(
          backend, proof.bytes(), verifyingKeyRef, commitment, envelopeHash, lanePrivacy);
    }
  }

  private static final class ProofAttachmentListAdapter
      implements TypeAdapter<List<ProofAttachment>> {
    @Override
    public void encode(final NoritoEncoder encoder, final List<ProofAttachment> value) {
      encodeSizedField(encoder, PROOF_ATTACHMENT_SEQUENCE_ADAPTER, value);
    }

    @Override
    public List<ProofAttachment> decode(final NoritoDecoder decoder) {
      return decodeSizedField(decoder, PROOF_ATTACHMENT_SEQUENCE_ADAPTER);
    }
  }

  private static final class FixedHashArrayAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value.length != 32) {
        throw new IllegalArgumentException("expected 32-byte hash");
      }
      encodeFixedByteArray(encoder, value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      return decodeFixedByteArray(decoder, 32, "proof attachment hash");
    }
  }

  static byte[] encodeInstructionBox(final InstructionBox instruction) {
    return NoritoCodec.encode(instruction, INSTRUCTION_BOX_SCHEMA, new InstructionAdapter());
  }

  static byte[] encodeProofAttachmentPayload(final ProofAttachment value) {
    return encodeProofAttachmentPayload(value, 0);
  }

  static byte[] encodeProofAttachmentPayload(final ProofAttachment value, final int flags) {
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    PROOF_ATTACHMENT_ADAPTER.encode(encoder, value);
    return encoder.toByteArray();
  }

  static ProofAttachment decodeProofAttachmentPayload(final byte[] encoded) {
    return decodeProofAttachmentPayload(encoded, 0);
  }

  static ProofAttachment decodeProofAttachmentPayload(final byte[] encoded, final int flags) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, flags, NoritoHeader.MINOR_VERSION);
    final ProofAttachment value = PROOF_ATTACHMENT_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("trailing ProofAttachment payload bytes");
    }
    return value;
  }

  static byte[] encodeMultisigProposeRequest(
      final MultisigProposeRequest request, final int chainDiscriminant) {
    return withChainContext(
        chainDiscriminant,
        () ->
            NoritoCodec.encode(
                request,
                MULTISIG_PROPOSE_DTO_SCHEMA,
                new MultisigProposeRequestAdapter(),
                NoritoHeader.COMPACT_LEN));
  }

  static InstructionBox decodeInstructionBox(final byte[] encoded) {
    return NoritoCodec.decode(encoded, new InstructionAdapter(), INSTRUCTION_BOX_SCHEMA);
  }

  private static final class FeePaymentIntentAdapter implements TypeAdapter<FeePaymentIntent> {
    @Override
    public void encode(final NoritoEncoder encoder, final FeePaymentIntent value) {
      if (value instanceof FeePaymentIntent.Authority) {
        ENUM_TAG_ADAPTER.encode(encoder, FEE_PAYER_AUTHORITY_TAG);
        encodeSizedField(
            encoder, AUTHORITY_FEE_PAYMENT_ADAPTER, (FeePaymentIntent.Authority) value);
        return;
      }
      if (value instanceof FeePaymentIntent.Sponsor) {
        ENUM_TAG_ADAPTER.encode(encoder, FEE_PAYER_SPONSOR_TAG);
        encodeSizedField(encoder, SPONSOR_FEE_PAYMENT_ADAPTER, (FeePaymentIntent.Sponsor) value);
        return;
      }
      throw new IllegalArgumentException("Unknown FeePaymentIntent subtype");
    }

    @Override
    public FeePaymentIntent decode(final NoritoDecoder decoder) {
      final long tag = ENUM_TAG_ADAPTER.decode(decoder);
      if (tag == FEE_PAYER_AUTHORITY_TAG) {
        return decodeSizedField(decoder, AUTHORITY_FEE_PAYMENT_ADAPTER);
      }
      if (tag == FEE_PAYER_SPONSOR_TAG) {
        return decodeSizedField(decoder, SPONSOR_FEE_PAYMENT_ADAPTER);
      }
      throw new IllegalArgumentException("Unknown FeePaymentIntent discriminant: " + tag);
    }
  }

  private static final class AuthorityFeePaymentAdapter
      implements TypeAdapter<FeePaymentIntent.Authority> {
    @Override
    public void encode(
        final NoritoEncoder encoder, final FeePaymentIntent.Authority value) {
      encodeSizedField(encoder, FEE_CHARGE_LIMIT_LIST_ADAPTER, value.chargeLimits());
      encodeSizedField(encoder, GAS_LIMIT_ADAPTER, Optional.ofNullable(value.gasLimit()));
    }

    @Override
    public FeePaymentIntent.Authority decode(final NoritoDecoder decoder) {
      final List<FeeChargeLimit> limits =
          decodeSizedField(decoder, FEE_CHARGE_LIMIT_LIST_ADAPTER);
      final Optional<Long> gasLimit = decodeSizedField(decoder, GAS_LIMIT_ADAPTER);
      return (FeePaymentIntent.Authority)
          FeePaymentIntent.authority(limits, gasLimit.orElse(null));
    }
  }

  private static final class SponsorFeePaymentAdapter
      implements TypeAdapter<FeePaymentIntent.Sponsor> {
    @Override
    public void encode(final NoritoEncoder encoder, final FeePaymentIntent.Sponsor value) {
      encodeSizedField(encoder, FEE_SPONSOR_PROGRAM_ID_ADAPTER, value.programId());
      encodeSizedField(encoder, UINT64_ADAPTER, value.programRevision());
      encodeSizedField(encoder, FEE_CHARGE_LIMIT_LIST_ADAPTER, value.chargeLimits());
      encodeSizedField(encoder, GAS_LIMIT_ADAPTER, Optional.ofNullable(value.gasLimit()));
    }

    @Override
    public FeePaymentIntent.Sponsor decode(final NoritoDecoder decoder) {
      final FeeSponsorProgramId programId =
          decodeSizedField(decoder, FEE_SPONSOR_PROGRAM_ID_ADAPTER);
      final long programRevision = decodeSizedField(decoder, UINT64_ADAPTER);
      final List<FeeChargeLimit> limits =
          decodeSizedField(decoder, FEE_CHARGE_LIMIT_LIST_ADAPTER);
      final Optional<Long> gasLimit = decodeSizedField(decoder, GAS_LIMIT_ADAPTER);
      return (FeePaymentIntent.Sponsor)
          FeePaymentIntent.sponsor(programId, programRevision, limits, gasLimit.orElse(null));
    }
  }

  private static final class FeeSponsorProgramIdAdapter
      implements TypeAdapter<FeeSponsorProgramId> {
    @Override
    public void encode(final NoritoEncoder encoder, final FeeSponsorProgramId value) {
      encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.sponsor());
      encodeSizedField(encoder, STRING_ADAPTER, value.name());
    }

    @Override
    public FeeSponsorProgramId decode(final NoritoDecoder decoder) {
      return new FeeSponsorProgramId(
          decodeSizedField(decoder, ACCOUNT_ID_ADAPTER),
          decodeSizedField(decoder, STRING_ADAPTER));
    }
  }

  private static final class FeeChargeLimitAdapter implements TypeAdapter<FeeChargeLimit> {
    @Override
    public void encode(final NoritoEncoder encoder, final FeeChargeLimit value) {
      encodeSizedField(encoder, FEE_CHARGE_KIND_ADAPTER, value.kind());
      encodeSizedField(encoder, ASSET_DEFINITION_ID_ADAPTER, value.assetDefinitionId());
      encodeSizedField(
          encoder, QUANTITY_ADAPTER, NumericV1.QuantityValue.parseCanonical(value.maxAmount()));
    }

    @Override
    public FeeChargeLimit decode(final NoritoDecoder decoder) {
      return new FeeChargeLimit(
          decodeSizedField(decoder, FEE_CHARGE_KIND_ADAPTER),
          decodeSizedField(decoder, ASSET_DEFINITION_ID_ADAPTER),
          decodeSizedField(decoder, QUANTITY_ADAPTER).toString());
    }
  }

  private static final class FeeChargeKindAdapter implements TypeAdapter<FeeChargeKind> {
    @Override
    public void encode(final NoritoEncoder encoder, final FeeChargeKind value) {
      ENUM_TAG_ADAPTER.encode(
          encoder,
          value == FeeChargeKind.NEXUS ? FEE_CHARGE_NEXUS_TAG : FEE_CHARGE_PIPELINE_GAS_TAG);
    }

    @Override
    public FeeChargeKind decode(final NoritoDecoder decoder) {
      final long tag = ENUM_TAG_ADAPTER.decode(decoder);
      if (tag == FEE_CHARGE_NEXUS_TAG) return FeeChargeKind.NEXUS;
      if (tag == FEE_CHARGE_PIPELINE_GAS_TAG) return FeeChargeKind.PIPELINE_GAS;
      throw new IllegalArgumentException("Unknown FeeChargeKind discriminant: " + tag);
    }
  }

  private static final class AssetDefinitionIdAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      encodeFixedByteArray(encoder, AssetDefinitionIdEncoder.parseAddressBytes(value));
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      return AssetDefinitionIdEncoder.encodeFromBytes(
          decodeFixedByteArray(decoder, 16, "AssetDefinitionId"));
    }
  }

  private static final class QuantityAdapter implements TypeAdapter<NumericV1.QuantityValue> {
    @Override
    public void encode(final NoritoEncoder encoder, final NumericV1.QuantityValue value) {
      encodeSizedBigInt(encoder, value.mantissa());
      encodeSizedField(encoder, UINT32_ADAPTER, (long) value.scale());
    }

    @Override
    public NumericV1.QuantityValue decode(final NoritoDecoder decoder) {
      return NumericV1.QuantityValue.of(
          decodeSizedBigInt(decoder),
          Math.toIntExact(decodeSizedField(decoder, UINT32_ADAPTER)));
    }
  }

  private static void encodeExecutable(final NoritoEncoder encoder, final Executable executable) {
    if (executable.isIvm()) {
      ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_IVM_TAG);
      encodeSizedField(encoder, IVM_BYTECODE_ADAPTER, executable.ivmBytes());
      return;
    }
    if (executable.isContractCall()) {
      ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_CONTRACT_CALL_TAG);
      encodeSizedField(encoder, CONTRACT_INVOCATION_ADAPTER, executable.contractInvocation());
      return;
    }
    if (executable.isBatch()) {
      ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_BATCH_TAG);
      encodeSizedField(encoder, EXECUTABLE_BATCH_ADAPTER, executable.batchItems());
      return;
    }
    if (executable.isInstructions()) {
      ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_INSTRUCTIONS_TAG);
      encodeSizedField(encoder, INSTRUCTION_LIST_ADAPTER, executable.instructions());
      return;
    }
    throw new IllegalArgumentException("Unknown Executable variant");
  }

  private static Executable decodeExecutable(final NoritoDecoder decoder) {
    final long tag = ENUM_TAG_ADAPTER.decode(decoder);
    if (tag == EXECUTABLE_IVM_TAG) {
      final byte[] bytes = decodeSizedField(decoder, IVM_BYTECODE_ADAPTER);
      return Executable.ivm(bytes);
    }
    if (tag == EXECUTABLE_INSTRUCTIONS_TAG) {
      final List<InstructionBox> instructions = decodeSizedField(decoder, INSTRUCTION_LIST_ADAPTER);
      return Executable.instructions(instructions);
    }
    if (tag == EXECUTABLE_CONTRACT_CALL_TAG) {
      return Executable.contractCall(decodeSizedField(decoder, CONTRACT_INVOCATION_ADAPTER));
    }
    if (tag == EXECUTABLE_BATCH_TAG) {
      return Executable.batch(decodeSizedField(decoder, EXECUTABLE_BATCH_ADAPTER));
    }
    if (tag == EXECUTABLE_IVM_PROVED_TAG) {
      throw new IllegalArgumentException("Unsupported Executable discriminant: " + tag);
    }
    throw new IllegalArgumentException("Unknown Executable discriminant: " + tag);
  }

  private static final class ContractInvocationAdapter
      implements TypeAdapter<ContractInvocation> {
    @Override
    public void encode(final NoritoEncoder encoder, final ContractInvocation value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.contractAddress());
      encodeSizedField(encoder, FIXED_HASH_ADAPTER, value.expectedCodeHash());
      encodeSizedField(encoder, STRING_ADAPTER, value.entrypoint());
      encodeSizedField(
          encoder,
          OPTIONAL_CONTRACT_ARGUMENT_RECORD_ADAPTER,
          Optional.ofNullable(value.arguments()));
    }

    @Override
    public ContractInvocation decode(final NoritoDecoder decoder) {
      final String contractAddress = decodeSizedField(decoder, STRING_ADAPTER);
      final byte[] expectedCodeHash = decodeSizedField(decoder, FIXED_HASH_ADAPTER);
      final String entrypoint = decodeSizedField(decoder, STRING_ADAPTER);
      final Optional<byte[]> arguments =
          decodeBoundedSizedField(
              decoder,
              OPTIONAL_CONTRACT_ARGUMENT_RECORD_ADAPTER,
              ContractInvocation.MAX_ARGUMENT_BYTES + 32L,
              "ContractInvocation.arguments");
      return new ContractInvocation(
          contractAddress, expectedCodeHash, entrypoint, arguments.orElse(null));
    }
  }

  private static final class ContractArgumentRecordAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value.length > ContractInvocation.MAX_ARGUMENT_BYTES) {
        throw new IllegalArgumentException(
            "ContractInvocation.arguments exceeds the signed wire limit");
      }
      RAW_BYTE_VEC_ADAPTER.encode(encoder, value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final long length = decoder.readLength(false);
      if (length < 0L || length > ContractInvocation.MAX_ARGUMENT_BYTES) {
        throw new IllegalArgumentException(
            "ContractInvocation.arguments exceeds the signed wire limit");
      }
      return decoder.readBytes((int) length);
    }
  }

  private static final class ExecutableBatchItemAdapter
      implements TypeAdapter<ExecutableBatchItem> {
    @Override
    public void encode(final NoritoEncoder encoder, final ExecutableBatchItem value) {
      if (value.isInstruction()) {
        ENUM_TAG_ADAPTER.encode(encoder, BATCH_ITEM_INSTRUCTION_TAG);
        encodeSizedField(encoder, INSTRUCTION_ADAPTER, value.instruction());
        return;
      }
      if (value.isContractCall()) {
        ENUM_TAG_ADAPTER.encode(encoder, BATCH_ITEM_CONTRACT_CALL_TAG);
        encodeSizedField(encoder, CONTRACT_INVOCATION_ADAPTER, value.contractInvocation());
        return;
      }
      throw new IllegalArgumentException("Unknown ExecutableBatchItem variant");
    }

    @Override
    public ExecutableBatchItem decode(final NoritoDecoder decoder) {
      final long tag = ENUM_TAG_ADAPTER.decode(decoder);
      if (tag == BATCH_ITEM_INSTRUCTION_TAG) {
        return ExecutableBatchItem.instruction(
            decodeSizedField(decoder, INSTRUCTION_ADAPTER));
      }
      if (tag == BATCH_ITEM_CONTRACT_CALL_TAG) {
        return ExecutableBatchItem.contractCall(
            decodeSizedField(decoder, CONTRACT_INVOCATION_ADAPTER));
      }
      throw new IllegalArgumentException("Unknown ExecutableBatchItem discriminant: " + tag);
    }
  }

  private static final class InstructionAdapter implements TypeAdapter<InstructionBox> {
    @Override
    public void encode(final NoritoEncoder encoder, final InstructionBox value) {
      value.requirePrivacyExact12ConstructionAdmission();
      final InstructionBox.InstructionPayload payload = value.payload();
      if (payload instanceof InstructionBox.WirePayload wire) {
        if (!isWirePayloadCandidate(wire.wireName(), wire.payloadBytes())) {
          throw new IllegalArgumentException("Wire payload must include a valid Norito header");
        }
        encodeSizedField(encoder, STRING_ADAPTER, wire.wireName());
        encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, wire.payloadBytes());
        return;
      }
      throw new IllegalArgumentException("Instruction payload must be wire-framed");
    }

    @Override
    public InstructionBox decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      if (payload.length == 0) {
        throw new IllegalArgumentException("Instruction payload must not be empty");
      }
      final InstructionBox wire = tryDecodeWireInstruction(payload, decoder.flags(), decoder.flagsHint());
      if (wire != null) {
        return wire;
      }
      throw new IllegalArgumentException("Instruction payload must be wire-framed");
    }
  }

  private static final class EncodedInstructionAdapter implements TypeAdapter<byte[]> {
    private static final InstructionAdapter INSTRUCTION_ADAPTER = new InstructionAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value == null || value.length == 0) {
        throw new IllegalArgumentException("instruction bytes must not be empty");
      }
      INSTRUCTION_ADAPTER.encode(encoder, decodeInstructionBox(value));
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Multisig instruction byte decoding is not supported");
    }
  }

  private static final class MultisigProposeRequestAdapter
      implements TypeAdapter<MultisigProposeRequest> {
    @Override
    public void encode(final NoritoEncoder encoder, final MultisigProposeRequest value) {
      validateMultisigProposeRequest(value);
      encodeSizedField(
          encoder,
          OPTIONAL_ACCOUNT_ID_ADAPTER,
          optionalString(value.multisigAccountId()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalString(value.multisigAccountAlias()));
      encodeSizedField(
          encoder,
          ACCOUNT_ID_ADAPTER,
          requireNonBlank(value.signerAccountId(), "signerAccountId"));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, Optional.empty());
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.publicKeyHex()));
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.signatureB64()));
      encodeSizedField(
          encoder,
          NoritoAdapters.option(UINT64_ADAPTER),
          Optional.ofNullable(value.creationTimeMs()));
      encodeSizedField(encoder, FEE_PAYMENT_ADAPTER, value.feePayment());
      encodeSizedField(encoder, OPTIONAL_STRING_ADAPTER, optionalString(value.memo()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeePolicyVersion(value.validationFeePolicyVersion()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeePolicyHash(value.validationFeePolicyHash()));
      encodeSizedField(encoder, ENCODED_INSTRUCTION_LIST_ADAPTER, value.instructions());
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeeInstructionIndex(value.validationFeeInstructionIndex()));
      encodeSizedField(
          encoder,
          OPTIONAL_STRING_ADAPTER,
          optionalValidationFeeTransferEntryIndex(value.validationFeeTransferEntryIndex()));
    }

    @Override
    public MultisigProposeRequest decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("MultisigProposeRequest decoding is not supported");
    }
  }

  private static final class ExecutableAdapter implements TypeAdapter<Executable> {
    @Override
    public void encode(final NoritoEncoder encoder, final Executable value) {
      encodeExecutable(encoder, value);
    }

    @Override
    public Executable decode(final NoritoDecoder decoder) {
      return decodeExecutable(decoder);
    }
  }

  private static final class AccountIdAdapter implements TypeAdapter<String> {
    private static final long SINGLE_CONTROLLER_TAG = 0L;
    private static final long MULTISIG_CONTROLLER_TAG = 1L;
    private static final TypeAdapter<ControllerPayload> CONTROLLER_ADAPTER = new AccountControllerAdapter();
    private static final TypeAdapter<AccountAddress.MultisigPolicyPayload> MULTISIG_POLICY_ADAPTER =
        new MultisigPolicyAdapter();
    private static final TypeAdapter<AccountAddress.MultisigMemberPayload> MULTISIG_MEMBER_ADAPTER =
        new MultisigMemberAdapter();
    private static final TypeAdapter<List<AccountAddress.MultisigMemberPayload>> MULTISIG_MEMBER_LIST_ADAPTER =
        NoritoAdapters.sequence(MULTISIG_MEMBER_ADAPTER);

    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      CONTROLLER_ADAPTER.encode(encoder, parseAuthority(value));
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static String decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder controllerDecoder = new NoritoDecoder(payload, flags, flagsHint);
      final ControllerPayload controller = decodeControllerPayload(controllerDecoder);
      if (controllerDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after authority payload");
      }
      return renderAuthority(controller);
    }

    private static ControllerPayload decodeControllerPayload(final NoritoDecoder decoder) {
      final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
      if (controllerTag == SINGLE_CONTROLLER_TAG) {
        return ControllerPayload.single(decodeSizedField(decoder, BYTE_VECTOR_ADAPTER));
      }
      if (controllerTag == MULTISIG_CONTROLLER_TAG) {
        return ControllerPayload.multisig(decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER));
      }
      throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
    }

    private static ControllerPayload parseAuthority(final String authority) {
      final String canonicalAuthority =
          AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
      final AccountAddress.ParseResult parsed;
      try {
        parsed =
            AccountAddress.parseEncodedIgnoringCurveSupport(
                canonicalAuthority, requiredChainDiscriminant());
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("authority must use canonical I105 encoding", ex);
      }
      return parseAddressToController(parsed.address);
    }

    private static ControllerPayload parseAddressToController(final AccountAddress address) {
      try {
        final java.util.Optional<AccountAddress.SingleKeyPayload> singlePayload =
            address.singleKeyPayloadIgnoringCurveSupport();
        if (singlePayload.isPresent()) {
          final AccountAddress.SingleKeyPayload payload = singlePayload.get();
          final byte[] publicKeyPayload =
              PublicKeyCodec.compactPublicKeyPayload(payload.curveId(), payload.publicKey());
          return ControllerPayload.single(publicKeyPayload);
        }
        final java.util.Optional<AccountAddress.MultisigPolicyPayload> multisigPayload =
            address.multisigPolicyPayloadIgnoringCurveSupport();
        if (multisigPayload.isPresent()) {
          return ControllerPayload.multisig(multisigPayload.get());
        }
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException(
            "Failed to extract controller from canonical I105 account id", ex);
      }
      throw new IllegalArgumentException(
          "Address contains neither single-key nor multisig controller");
    }

    private static String renderAuthority(final ControllerPayload controller) {
      if (controller.isSingle()) {
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodeCompactPublicKeyPayload(controller.publicKeyPayload());
        if (payload == null) {
          throw new IllegalArgumentException("Invalid single-key AccountController payload");
        }
        return renderSingleAuthority(payload);
      }
      return renderMultisigAuthority(controller.multisigPolicy());
    }

    private static final class ControllerPayload {
      private final byte[] publicKeyPayload;
      private final AccountAddress.MultisigPolicyPayload multisigPolicy;

      private ControllerPayload(
          final byte[] publicKeyPayload,
          final AccountAddress.MultisigPolicyPayload multisigPolicy) {
        this.publicKeyPayload =
            publicKeyPayload == null ? null : Arrays.copyOf(publicKeyPayload, publicKeyPayload.length);
        this.multisigPolicy = multisigPolicy;
      }

      private static ControllerPayload single(final byte[] publicKeyPayload) {
        if (publicKeyPayload == null || publicKeyPayload.length == 0) {
          throw new IllegalArgumentException("public key payload must not be empty");
        }
        return new ControllerPayload(publicKeyPayload, null);
      }

      private static ControllerPayload multisig(
          final AccountAddress.MultisigPolicyPayload multisigPolicy) {
        if (multisigPolicy == null) {
          throw new IllegalArgumentException("multisig policy must not be null");
        }
        return new ControllerPayload(null, multisigPolicy);
      }

      private boolean isSingle() {
        return multisigPolicy == null;
      }

      private byte[] publicKeyPayload() {
        return Arrays.copyOf(publicKeyPayload, publicKeyPayload.length);
      }

      private AccountAddress.MultisigPolicyPayload multisigPolicy() {
        return multisigPolicy;
      }
    }

    private static final class AccountControllerAdapter implements TypeAdapter<ControllerPayload> {
      @Override
      public void encode(final NoritoEncoder encoder, final ControllerPayload value) {
        if (value == null) {
          throw new IllegalArgumentException("AccountController payload must not be null");
        }
        if (value.isSingle()) {
          ENUM_TAG_ADAPTER.encode(encoder, SINGLE_CONTROLLER_TAG);
          encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value.publicKeyPayload());
          return;
        }
        ENUM_TAG_ADAPTER.encode(encoder, MULTISIG_CONTROLLER_TAG);
        encodeSizedField(encoder, MULTISIG_POLICY_ADAPTER, value.multisigPolicy());
      }

      @Override
      public ControllerPayload decode(final NoritoDecoder decoder) {
        final long controllerTag = ENUM_TAG_ADAPTER.decode(decoder);
        final ControllerPayload controller;
        if (controllerTag == SINGLE_CONTROLLER_TAG) {
          final byte[] publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER);
          controller = ControllerPayload.single(publicKeyPayload);
        } else if (controllerTag == MULTISIG_CONTROLLER_TAG) {
          final AccountAddress.MultisigPolicyPayload policy =
              decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER);
          controller = ControllerPayload.multisig(policy);
        } else {
          throw new IllegalArgumentException("Unsupported AccountController tag: " + controllerTag);
        }
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after AccountController payload");
        }
        return controller;
      }
    }

    private static final class MultisigPolicyAdapter
        implements TypeAdapter<AccountAddress.MultisigPolicyPayload> {
      @Override
      public void encode(
          final NoritoEncoder encoder, final AccountAddress.MultisigPolicyPayload value) {
        encodeSizedField(encoder, UINT8_ADAPTER, (long) value.version());
        encodeSizedField(encoder, UINT16_ADAPTER, (long) value.threshold());
        encodeSizedField(encoder, MULTISIG_MEMBER_LIST_ADAPTER, value.members());
      }

      @Override
      public AccountAddress.MultisigPolicyPayload decode(final NoritoDecoder decoder) {
        final int version = Math.toIntExact(decodeSizedField(decoder, UINT8_ADAPTER));
        final int threshold = Math.toIntExact(decodeSizedField(decoder, UINT16_ADAPTER));
        final List<AccountAddress.MultisigMemberPayload> members =
            decodeSizedField(decoder, MULTISIG_MEMBER_LIST_ADAPTER);
        return AccountAddress.MultisigPolicyPayload.of(version, threshold, members);
      }
    }

    private static final class MultisigMemberAdapter
        implements TypeAdapter<AccountAddress.MultisigMemberPayload> {
      @Override
      public void encode(
          final NoritoEncoder encoder, final AccountAddress.MultisigMemberPayload value) {
        final byte[] publicKeyPayload =
            PublicKeyCodec.compactPublicKeyPayload(value.curveId(), value.publicKey());
        encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, publicKeyPayload);
        encodeSizedField(encoder, UINT16_ADAPTER, (long) value.weight());
      }

      @Override
      public AccountAddress.MultisigMemberPayload decode(final NoritoDecoder decoder) {
        final byte[] publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER);
        final int weight = Math.toIntExact(decodeSizedField(decoder, UINT16_ADAPTER));
        final PublicKeyCodec.PublicKeyPayload payload =
            PublicKeyCodec.decodeCompactPublicKeyPayload(publicKeyPayload);
        if (payload == null) {
          throw new IllegalArgumentException("Invalid multisig member public key");
        }
        return AccountAddress.MultisigMemberPayload.of(
            payload.curveId(), weight, payload.keyBytes());
      }
    }

    private static String renderSingleAuthority(
        final PublicKeyCodec.PublicKeyPayload payload) {
      final String algorithm = PublicKeyCodec.algorithmForCurveId(payload.curveId());
      if (algorithm == null) {
        throw new IllegalArgumentException(
            "Unsupported curve id in AccountController payload: " + payload.curveId());
      }
      try {
        final AccountAddress address = AccountAddress.fromAccount(payload.keyBytes(), algorithm);
        return address.toI105(requiredChainDiscriminant());
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("Invalid single-key AccountController payload", ex);
      }
    }

    private static String renderMultisigAuthority(
        final AccountAddress.MultisigPolicyPayload policy) {
      try {
        final AccountAddress address = AccountAddress.fromMultisigPolicy(policy);
        return address.toI105(requiredChainDiscriminant());
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalArgumentException("Invalid multisig policy for AccountId", ex);
      }
    }
  }

  private static int requiredChainDiscriminant() {
    final Integer value = CHAIN_DISCRIMINANT.get();
    if (value == null) {
      throw new IllegalStateException(
          "Account controller encoding/rendering requires an explicit chainDiscriminant");
    }
    return value;
  }

  private static <T> T withChainContext(
      final int chainDiscriminant, final Supplier<T> operation) {
    if (chainDiscriminant < 0 || chainDiscriminant > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    final Integer previous = CHAIN_DISCRIMINANT.get();
    if (previous != null && previous.intValue() != chainDiscriminant) {
      throw new IllegalStateException("Conflicting nested chainDiscriminant context");
    }
    CHAIN_DISCRIMINANT.set(chainDiscriminant);
    try {
      return operation.get();
    } finally {
      if (previous == null) {
        CHAIN_DISCRIMINANT.remove();
      } else {
        CHAIN_DISCRIMINANT.set(previous);
      }
    }
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeSizedField(final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Field payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after field payload");
    }
    return value;
  }

  private static <T> T decodeBoundedSizedField(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final long maximumLength,
      final String fieldName) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length < 0L || length > maximumLength) {
      throw new IllegalArgumentException(fieldName + " payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static void encodeFixedByteArray(
      final NoritoEncoder encoder, final byte[] bytes) {
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    for (final byte value : bytes) {
      encoder.writeLength(1L, compact);
      encoder.writeByte(value);
    }
  }

  private static byte[] decodeFixedByteArray(
      final NoritoDecoder decoder, final int length, final String fieldName) {
    final byte[] out = new byte[length];
    final boolean compact = (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    for (int index = 0; index < length; index++) {
      if (decoder.readLength(compact) != 1L) {
        throw new IllegalArgumentException(
            fieldName + " element " + index + " must contain exactly one byte");
      }
      out[index] = (byte) decoder.readByte();
    }
    return out;
  }

  private static void encodeSizedBigInt(
      final NoritoEncoder encoder, final BigInteger value) {
    final NoritoEncoder child = encoder.childEncoder();
    final byte[] bytes = toTwosComplementLittleEndian(value);
    child.writeUInt(bytes.length, 32);
    child.writeBytes(bytes);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(payload);
  }

  private static BigInteger decodeSizedBigInt(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("numeric mantissa payload too large");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final long byteLength = child.readUInt(32);
    if (byteLength > 64L) {
      throw new IllegalArgumentException("numeric mantissa exceeds 512 bits");
    }
    final byte[] bytes = child.readBytes((int) byteLength);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after numeric mantissa payload");
    }
    final BigInteger value = fromTwosComplementLittleEndian(bytes);
    if (!Arrays.equals(toTwosComplementLittleEndian(value), bytes)) {
      throw new IllegalArgumentException("Numeric mantissa is not canonical");
    }
    return value;
  }

  private static BigInteger fromTwosComplementLittleEndian(final byte[] bytes) {
    if (bytes.length == 0) return BigInteger.ZERO;
    final byte[] reversed = bytes.clone();
    reverse(reversed);
    return new BigInteger(reversed);
  }

  private static byte[] toTwosComplementLittleEndian(final BigInteger value) {
    if (value.signum() == 0) return new byte[0];
    final byte[] result = value.toByteArray();
    reverse(result);
    int length = result.length;
    if (value.signum() > 0) {
      while (length > 1 && result[length - 1] == 0 && (result[length - 2] & 0x80) == 0) {
        length--;
      }
    } else {
      while (length > 1 && result[length - 1] == (byte) 0xff
          && (result[length - 2] & 0x80) != 0) {
        length--;
      }
    }
    return length == result.length ? result : Arrays.copyOf(result, length);
  }

  private static void reverse(final byte[] bytes) {
    for (int left = 0, right = bytes.length - 1; left < right; left++, right--) {
      final byte value = bytes[left];
      bytes[left] = bytes[right];
      bytes[right] = value;
    }
  }

  private static String decodeAuthorityField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Field payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    return AccountIdAdapter.decodePayload(payload, decoder.flags(), decoder.flagsHint());
  }

  private static Optional<String> optionalString(final String value) {
    if (value == null) {
      return Optional.empty();
    }
    final String normalized = value.trim();
    if (normalized.isEmpty()) {
      return Optional.empty();
    }
    return Optional.of(normalized);
  }

  private static Optional<String> optionalValidationFeePolicyVersion(final Long value) {
    if (value == null) {
      return Optional.empty();
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeePolicyVersion must be non-negative");
    }
    return Optional.of(value.toString());
  }

  private static Optional<String> optionalValidationFeePolicyHash(final String value) {
    if (value == null) {
      return Optional.empty();
    }
    return Optional.of(normalizeValidationFeePolicyHash(value));
  }

  private static Optional<String> optionalValidationFeeInstructionIndex(final Long value) {
    if (value == null) {
      return Optional.empty();
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeeInstructionIndex must be non-negative");
    }
    return Optional.of(value.toString());
  }

  private static Optional<String> optionalValidationFeeTransferEntryIndex(final Long value) {
    if (value == null) {
      return Optional.empty();
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeeTransferEntryIndex must be non-negative");
    }
    return Optional.of(value.toString());
  }

  private static String normalizeValidationFeePolicyHash(final String value) {
    final String normalized =
        requireNonBlank(value, "validationFeePolicyHash").toLowerCase(Locale.ROOT);
    if (normalized.length() != 64) {
      throw new IllegalArgumentException("validationFeePolicyHash must contain 64 hex characters");
    }
    for (int index = 0; index < normalized.length(); index++) {
      final char character = normalized.charAt(index);
      final boolean isHex =
          (character >= '0' && character <= '9')
              || (character >= 'a' && character <= 'f');
      if (!isHex) {
        throw new IllegalArgumentException("validationFeePolicyHash must contain 64 hex characters");
      }
    }
    return normalized;
  }

  private static String requireNonBlank(final String value, final String fieldName) {
    if (value == null) {
      throw new IllegalArgumentException(fieldName + " must not be null");
    }
    final String normalized = value.trim();
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    return normalized;
  }

  private static void validateMultisigProposeRequest(final MultisigProposeRequest request) {
    if (request == null) {
      throw new IllegalArgumentException("request must not be null");
    }
    final boolean hasAccountId = optionalString(request.multisigAccountId()).isPresent();
    final boolean hasAlias = optionalString(request.multisigAccountAlias()).isPresent();
    if (hasAccountId == hasAlias) {
      throw new IllegalArgumentException(
          "Exactly one of multisigAccountId or multisigAccountAlias must be provided");
    }
    requireNonBlank(request.signerAccountId(), "signerAccountId");
    if (request.instructions().isEmpty()) {
      throw new IllegalArgumentException("instructions must not be empty");
    }
    if (request.creationTimeMs() != null && request.creationTimeMs().longValue() < 0L) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
    final boolean hasPolicyVersion = request.validationFeePolicyVersion() != null;
    final boolean hasPolicyHash = request.validationFeePolicyHash() != null;
    final boolean hasInstructionIndex = request.validationFeeInstructionIndex() != null;
    final boolean hasTransferEntryIndex = request.validationFeeTransferEntryIndex() != null;
    if (hasPolicyVersion != hasPolicyHash) {
      throw new IllegalArgumentException(
          "validationFeePolicyVersion and validationFeePolicyHash must be provided together");
    }
    if (!hasPolicyVersion && hasInstructionIndex) {
      throw new IllegalArgumentException(
          "validationFeeInstructionIndex requires validation fee policy metadata");
    }
    if (!hasPolicyVersion && hasTransferEntryIndex) {
      throw new IllegalArgumentException(
          "validationFeeTransferEntryIndex requires validation fee policy metadata");
    }
    if (hasTransferEntryIndex && !hasInstructionIndex) {
      throw new IllegalArgumentException(
          "validationFeeTransferEntryIndex requires validationFeeInstructionIndex");
    }
    optionalValidationFeePolicyVersion(request.validationFeePolicyVersion());
    optionalValidationFeePolicyHash(request.validationFeePolicyHash());
    optionalValidationFeeInstructionIndex(request.validationFeeInstructionIndex());
    optionalValidationFeeTransferEntryIndex(request.validationFeeTransferEntryIndex());
  }

  private static InstructionBox tryDecodeWireInstruction(
      final byte[] payload, final int flags, final int flagsHint) {
    try {
      final NoritoDecoder wireDecoder = new NoritoDecoder(payload, flags, flagsHint);
      final String wireName = decodeSizedField(wireDecoder, STRING_ADAPTER);
      final byte[] wirePayload = decodeSizedField(wireDecoder, RAW_BYTE_VEC_ADAPTER);
      if (wireDecoder.remaining() != 0) {
        return null;
      }
      if (!isWirePayloadCandidate(wireName, wirePayload)) {
        return null;
      }
      return InstructionBox.fromWirePayload(wireName, wirePayload);
    } catch (final IllegalArgumentException ex) {
      return null;
    }
  }

  private static boolean isWirePayloadCandidate(final String wireName, final byte[] payload) {
    if (wireName == null || wireName.isBlank()) {
      return false;
    }
    if (payload == null || payload.length < NoritoHeader.HEADER_LENGTH) {
      return false;
    }
    if (payload[0] != 'N' || payload[1] != 'R' || payload[2] != 'T' || payload[3] != '0') {
      return false;
    }
    try {
      final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(payload, null);
      decoded.header().validateChecksum(decoded.payload());
      return true;
    } catch (final IllegalArgumentException ex) {
      return false;
    }
  }

  private static final class TransactionDomainAdapter implements TypeAdapter<NetworkId> {
    @Override
    public void encode(final NoritoEncoder encoder, final NetworkId value) {
      ENUM_TAG_ADAPTER.encode(encoder, TRANSACTION_DOMAIN_NETWORK_TAG);
      encodeSizedField(encoder, FIXED_HASH_ADAPTER, value.bytes());
    }

    @Override
    public NetworkId decode(final NoritoDecoder decoder) {
      final long tag = ENUM_TAG_ADAPTER.decode(decoder);
      if (tag == TRANSACTION_DOMAIN_GENESIS_TAG) {
        throw new IllegalArgumentException(
            "Genesis-only transaction domains are not accepted by the SDK");
      }
      if (tag != TRANSACTION_DOMAIN_NETWORK_TAG) {
        throw new IllegalArgumentException("Unknown TransactionDomain discriminant: " + tag);
      }
      return NetworkId.fromBytes(decodeSizedField(decoder, FIXED_HASH_ADAPTER));
    }
  }

  private static final class IvmBytecodeAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      return decodePayload(payload, decoder.flags(), decoder.flagsHint());
    }

    private static byte[] decodePayload(
        final byte[] payload, final int flags, final int flagsHint) {
      final NoritoDecoder sized = new NoritoDecoder(payload, flags, flagsHint);
      final byte[] value = decodeSizedField(sized, RAW_BYTE_VEC_ADAPTER);
      if (sized.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after IVM payload");
      }
      return value;
    }
  }

  private static final class JsonAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      if (value == null) {
        throw new IllegalArgumentException("Metadata values must not be null");
      }
      encodeSizedField(encoder, STRING_ADAPTER, value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      return decodeSizedField(decoder, STRING_ADAPTER);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class MetadataAdapter implements TypeAdapter<Map<String, JsonValue>> {
    private static final TypeAdapter<List<MetadataEntry>> ENTRY_LIST_ADAPTER =
        NoritoAdapters.sequence(new MetadataEntryAdapter());

    @Override
    public void encode(final NoritoEncoder encoder, final Map<String, JsonValue> value) {
      final List<MetadataEntry> entries = new ArrayList<>(value.size());
      final List<String> keys = new ArrayList<>(value.keySet());
      Collections.sort(keys);
      for (final String key : keys) {
        final JsonValue entryValue = value.get(key);
        if (entryValue == null) {
          throw new IllegalArgumentException("Metadata values must not be null");
        }
        entries.add(new MetadataEntry(key, entryValue));
      }
      ENTRY_LIST_ADAPTER.encode(encoder, entries);
    }

    @Override
    public Map<String, JsonValue> decode(final NoritoDecoder decoder) {
      final List<MetadataEntry> entries = ENTRY_LIST_ADAPTER.decode(decoder);
      final Map<String, JsonValue> decoded = new LinkedHashMap<>(entries.size());
      for (final MetadataEntry entry : entries) {
        if (decoded.put(entry.key(), entry.value()) != null) {
          throw new IllegalArgumentException("Duplicate metadata key");
        }
      }
      return decoded;
    }
  }

  private static final class MetadataEntry {
    private final String key;
    private final JsonValue value;

    private MetadataEntry(final String key, final JsonValue value) {
      this.key = key;
      this.value = value;
    }

    private String key() {
      return key;
    }

    private JsonValue value() {
      return value;
    }
  }

  private static final class MetadataEntryAdapter implements TypeAdapter<MetadataEntry> {
    @Override
    public void encode(final NoritoEncoder encoder, final MetadataEntry entry) {
      encodeSizedField(encoder, STRING_ADAPTER, entry.key());
      encodeSizedField(encoder, JSON_VALUE_ADAPTER, entry.value().canonicalJson());
    }

    @Override
    public MetadataEntry decode(final NoritoDecoder decoder) {
      final String key = decodeSizedField(decoder, STRING_ADAPTER);
      final String value = decodeSizedField(decoder, JSON_VALUE_ADAPTER);
      return new MetadataEntry(key, JsonValue.fromCanonicalWire(value));
    }
  }

}
