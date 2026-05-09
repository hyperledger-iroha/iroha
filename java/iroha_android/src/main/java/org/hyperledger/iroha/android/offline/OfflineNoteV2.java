package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountAddress.AccountAddressException;
import org.hyperledger.iroha.android.address.AccountAddress.MultisigMemberPayload;
import org.hyperledger.iroha.android.address.AccountAddress.MultisigPolicyPayload;
import org.hyperledger.iroha.android.address.AccountAddress.SingleKeyPayload;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Native Java implementation of Iroha Offline Note V2 canonical Norito encodings. */
public final class OfflineNoteV2 {
  public static final String KEY_CERTIFICATE_PAYLOAD_DOMAIN =
      "iroha:offline-note-v2:key-certificate-payload:v1";
  public static final String ISSUED_CLAIM_DOMAIN = "iroha:offline-note-v2:issued-claim:v1";
  public static final String REDEEM_PUBLIC_INPUTS_DOMAIN =
      "iroha:offline-note-v2:redeem-public-inputs:v1";
  public static final String AUDIT_PUBLIC_INPUTS_DOMAIN =
      "iroha:offline-note-v2:audit-public-inputs:v1";
  public static final String NOTE_COMMITMENT_DOMAIN =
      "iroha:offline-note-v2:note-commitment:v1";
  public static final String INPUT_NULLIFIER_DOMAIN =
      "iroha:offline-note-v2:input-nullifier:v1";
  public static final String PAYMENT_TOKEN_ID_DOMAIN =
      "iroha:offline-note-v2:payment-token-id:v1";
  public static final String RECURSIVE_BACKEND = "halo2/ipa";
  public static final String RECURSIVE_VERIFIER_NAME = "offline-note-v2-recursive-v1";
  public static final String RECURSIVE_PUBLIC_INPUTS_SCHEMA_V1 =
      "{\"schema\":\"offline_note_v2_recursive_v1\",\"public_inputs\":[\"public_inputs_hash_limb0\",\"public_inputs_hash_limb1\",\"public_inputs_hash_limb2\",\"public_inputs_hash_limb3\",\"proof_mode\",\"input_count\",\"output_count\",\"input_amount_sum\",\"output_amount_sum\",\"input_nullifier_sum_limb0\",\"output_commitment_sum_limb0\",\"key_certificate_payload_hash_limb0\",\"source_or_token_limb0\",\"input_claim_hash_sum_limb0\",\"output_claim_hash_sum_limb0\",\"reserved_zero\"]}";

  private static final int MULTISIG_POLICY_VERSION_V1 = 1;
  private static final int MAX_NUMERIC_SCALE = 28;
  private static final int MAX_BIGINT_BYTES = 64;
  private static final int PUBLIC_VALUE_COUNT = 16;
  private static final int MAX_INPUT_AMOUNTS = 4;
  private static final int MAX_OUTPUT_AMOUNTS = 2;
  private static final long MODE_REDEEM = 1L;
  private static final long MODE_AUDIT = 2L;
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private static final String KEY_CERTIFICATE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteKeyCertificateV2";
  private static final String KEY_CERTIFICATE_PAYLOAD_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayloadV2";
  private static final String ISSUE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteIssueV2";
  private static final String ISSUED_CLAIM_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteIssuedClaimV2";
  private static final String REDEEM_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteRedeemV2";
  private static final String REDEEM_PUBLIC_INPUTS_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputsV2";
  private static final String AUDIT_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteAuditBundleV2";
  private static final String AUDIT_PUBLIC_INPUTS_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteAuditPublicInputsV2";
  private static final String NOTE_COMMITMENT_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteCommitmentPreimageV2";
  private static final String INPUT_NULLIFIER_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteInputNullifierPreimageV2";
  private static final String PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNotePaymentTokenIdPreimageV2";
  public static final String ISSUE_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::IssueOfflineNoteV2";
  public static final String REDEEM_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::RedeemOfflineNoteV2";
  public static final String AUDIT_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::AuditOfflineNoteV2";

  private OfflineNoteV2() {}

  public static byte[] encodeCertificatePayload(final KeyCertificatePayloadV2 value) {
    return encodeWithHeader(value, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KEY_CERTIFICATE_PAYLOAD_ADAPTER);
  }

  public static byte[] encodeCertificate(final KeyCertificateV2 value) {
    return encodeWithHeader(value, KEY_CERTIFICATE_SCHEMA, KEY_CERTIFICATE_ADAPTER);
  }

  public static byte[] encodeIssue(final IssueV2 value) {
    return encodeWithHeader(value, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static byte[] encodeIssuedClaim(final IssuedClaimV2 value) {
    return encodeWithHeader(value, ISSUED_CLAIM_SCHEMA, ISSUED_CLAIM_ADAPTER);
  }

  public static byte[] encodeRedeem(final RedeemV2 value) {
    return encodeWithHeader(value, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static byte[] encodeRedeemPublicInputs(final RedeemPublicInputsV2 value) {
    return encodeWithHeader(value, REDEEM_PUBLIC_INPUTS_SCHEMA, REDEEM_PUBLIC_INPUTS_ADAPTER);
  }

  public static byte[] encodeAudit(final AuditBundleV2 value) {
    return encodeWithHeader(value, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static byte[] encodeAuditPublicInputs(final AuditPublicInputsV2 value) {
    return encodeWithHeader(value, AUDIT_PUBLIC_INPUTS_SCHEMA, AUDIT_PUBLIC_INPUTS_ADAPTER);
  }

  public static byte[] encodeNoteCommitmentPreimage(final NoteCommitmentPreimageV2 value) {
    return encodeWithHeader(
        value, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NOTE_COMMITMENT_PREIMAGE_ADAPTER);
  }

  public static byte[] encodeInputNullifierPreimage(final InputNullifierPreimageV2 value) {
    return encodeWithHeader(
        value, INPUT_NULLIFIER_PREIMAGE_SCHEMA, INPUT_NULLIFIER_PREIMAGE_ADAPTER);
  }

  public static byte[] encodePaymentTokenIdPreimage(final PaymentTokenIdPreimageV2 value) {
    return encodeWithHeader(
        value, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER);
  }

  public static KeyCertificatePayloadV2 decodeCertificatePayload(final byte[] bytes) {
    return decodeWithHeader(bytes, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KEY_CERTIFICATE_PAYLOAD_ADAPTER);
  }

  public static KeyCertificateV2 decodeCertificate(final byte[] bytes) {
    return decodeWithHeader(bytes, KEY_CERTIFICATE_SCHEMA, KEY_CERTIFICATE_ADAPTER);
  }

  public static IssueV2 decodeIssue(final byte[] bytes) {
    return decodeWithHeader(bytes, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static IssuedClaimV2 decodeIssuedClaim(final byte[] bytes) {
    return decodeWithHeader(bytes, ISSUED_CLAIM_SCHEMA, ISSUED_CLAIM_ADAPTER);
  }

  public static RedeemV2 decodeRedeem(final byte[] bytes) {
    return decodeWithHeader(bytes, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static RedeemPublicInputsV2 decodeRedeemPublicInputs(final byte[] bytes) {
    return decodeWithHeader(bytes, REDEEM_PUBLIC_INPUTS_SCHEMA, REDEEM_PUBLIC_INPUTS_ADAPTER);
  }

  public static AuditBundleV2 decodeAudit(final byte[] bytes) {
    return decodeWithHeader(bytes, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static AuditPublicInputsV2 decodeAuditPublicInputs(final byte[] bytes) {
    return decodeWithHeader(bytes, AUDIT_PUBLIC_INPUTS_SCHEMA, AUDIT_PUBLIC_INPUTS_ADAPTER);
  }

  public static NoteCommitmentPreimageV2 decodeNoteCommitmentPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NOTE_COMMITMENT_PREIMAGE_ADAPTER);
  }

  public static InputNullifierPreimageV2 decodeInputNullifierPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, INPUT_NULLIFIER_PREIMAGE_SCHEMA, INPUT_NULLIFIER_PREIMAGE_ADAPTER);
  }

  public static PaymentTokenIdPreimageV2 decodePaymentTokenIdPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER);
  }

  public static IssueV2 decodeIssueInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, ISSUE_INSTRUCTION_SCHEMA, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static RedeemV2 decodeRedeemInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, REDEEM_INSTRUCTION_SCHEMA, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static AuditBundleV2 decodeAuditInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, AUDIT_INSTRUCTION_SCHEMA, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static InstructionBox issueInstruction(final IssueV2 value) {
    return InstructionBox.fromWirePayload(
        ISSUE_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(ISSUE_INSTRUCTION_SCHEMA, encodeIssue(value)));
  }

  public static InstructionBox redeemInstruction(final RedeemV2 value) {
    value.validateProofBinding();
    return InstructionBox.fromWirePayload(
        REDEEM_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(REDEEM_INSTRUCTION_SCHEMA, encodeRedeem(value)));
  }

  public static InstructionBox auditInstruction(final AuditBundleV2 value) {
    value.validateProofBinding();
    return InstructionBox.fromWirePayload(
        AUDIT_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(AUDIT_INSTRUCTION_SCHEMA, encodeAudit(value)));
  }

  public static byte[] deriveNoteCommitment(final NoteCommitmentPreimageV2 value) {
    return hash(encodeNoteCommitmentPreimage(value));
  }

  public static byte[] deriveInputNullifier(final InputNullifierPreimageV2 value) {
    return hash(encodeInputNullifierPreimage(value));
  }

  public static byte[] derivePaymentTokenId(final PaymentTokenIdPreimageV2 value) {
    return hash(encodePaymentTokenIdPreimage(value));
  }

  public static byte[] hash(final byte[] bytes) {
    return IrohaHash.prehash(bytes);
  }

  public static byte[] instanceScalarBytes(final long value) {
    final byte[] out = new byte[32];
    long word = value;
    for (int idx = 0; idx < 8; idx++) {
      out[idx] = (byte) (word & 0xFFL);
      word >>>= 8;
    }
    return out;
  }

  private static <T> byte[] encodeWithHeader(
      final T value, final String schema, final TypeAdapter<T> adapter) {
    return NoritoCodec.encode(value, schema, adapter, NoritoHeader.COMPACT_LEN);
  }

  private static <T> T decodeWithHeader(
      final byte[] bytes, final String schema, final TypeAdapter<T> adapter) {
    return NoritoCodec.decode(bytes, adapter, schema);
  }

  private static byte[] encodeInstructionWrapper(final String schema, final byte[] modelPayload) {
    return NoritoCodec.encode(modelPayload, schema, INSTRUCTION_WRAPPER_ADAPTER, 0);
  }

  private static <T> T decodeInstructionModel(
      final byte[] bytes,
      final String instructionSchema,
      final String modelSchema,
      final TypeAdapter<T> modelAdapter) {
    final byte[] wirePayload = extractInstructionWirePayload(bytes, instructionSchema);
    final InstructionModelPayload modelPayload =
        NoritoCodec.decode(wirePayload, INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER, instructionSchema);
    return decodeModelPayload(modelPayload.bytes(), modelSchema, modelAdapter, modelPayload.flags());
  }

  private static byte[] extractInstructionWirePayload(
      final byte[] bytes, final String expectedWireName) {
    if (isNoritoFrame(bytes)) {
      return Arrays.copyOf(bytes, bytes.length);
    }
    byte[] wirePayload = tryDecodeInstructionPair(bytes, expectedWireName, NoritoHeader.COMPACT_LEN);
    if (wirePayload != null) {
      return wirePayload;
    }
    wirePayload = tryDecodeInstructionPair(bytes, expectedWireName, 0);
    if (wirePayload != null) {
      return wirePayload;
    }
    throw new IllegalArgumentException("Offline Note V2 instruction envelope is invalid");
  }

  private static byte[] tryDecodeInstructionPair(
      final byte[] bytes, final String expectedWireName, final int flags) {
    try {
      final NoritoDecoder decoder = new NoritoDecoder(bytes, flags);
      final String wireName = readField(decoder, OfflineNoteV2::readString);
      if (!expectedWireName.equals(wireName)) {
        throw new IllegalArgumentException(
            "Offline Note V2 instruction wire name mismatch: " + wireName);
      }
      final byte[] wirePayload = readField(decoder, OfflineNoteV2::readBytesVec);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after instruction envelope");
      }
      return wirePayload;
    } catch (final RuntimeException ex) {
      return null;
    }
  }

  private static <T> T decodeModelPayload(
      final byte[] bytes,
      final String modelSchema,
      final TypeAdapter<T> modelAdapter,
      final int flags) {
    if (isNoritoFrame(bytes)) {
      return decodeWithHeader(bytes, modelSchema, modelAdapter);
    }
    final int[] attempts =
        flags == NoritoHeader.COMPACT_LEN ? new int[] {flags, 0} : new int[] {flags, NoritoHeader.COMPACT_LEN};
    RuntimeException lastError = null;
    for (final int attemptFlags : attempts) {
      try {
        final NoritoDecoder decoder = new NoritoDecoder(bytes, attemptFlags);
        final T value = modelAdapter.decode(decoder);
        if (decoder.remaining() != 0) {
          throw new IllegalArgumentException("Trailing bytes after Offline Note V2 model decode");
        }
        return value;
      } catch (final RuntimeException ex) {
        lastError = ex;
      }
    }
    throw new IllegalArgumentException(
        "Offline Note V2 instruction model payload is invalid", lastError);
  }

  private static boolean isNoritoFrame(final byte[] bytes) {
    return bytes != null
        && bytes.length >= NoritoHeader.HEADER_LENGTH
        && bytes[0] == (byte) 'N'
        && bytes[1] == (byte) 'R'
        && bytes[2] == (byte) 'T'
        && bytes[3] == (byte) '0';
  }

  public static final class VerifyingKeyIdReference {
    private final String backend;
    private final String name;

    public VerifyingKeyIdReference() {
      this(RECURSIVE_BACKEND, RECURSIVE_VERIFIER_NAME);
    }

    public VerifyingKeyIdReference(final String backend, final String name) {
      this.backend = requireNonBlank(backend, "verifying key backend");
      this.name = requireNonBlank(name, "verifying key name");
    }

    public String backend() {
      return backend;
    }

    public String name() {
      return name;
    }
  }

  public static final class ProofBox {
    private final String backend;
    private final byte[] bytes;

    public ProofBox(final String backend, final byte[] bytes) {
      this.backend = requireNonBlank(backend, "proof backend");
      this.bytes = copy(bytes, "proof bytes");
      if (this.bytes.length == 0) {
        throw new IllegalArgumentException("proof bytes must not be empty");
      }
    }

    public String backend() {
      return backend;
    }

    public byte[] bytes() {
      return Arrays.copyOf(bytes, bytes.length);
    }
  }

  public static final class RecursiveProofV2 {
    private final VerifyingKeyIdReference verifierKeyId;
    private final byte[] publicInputsHash;
    private final ProofBox proof;

    public RecursiveProofV2(final byte[] publicInputsHash, final ProofBox proof) {
      this(new VerifyingKeyIdReference(), publicInputsHash, proof);
    }

    public RecursiveProofV2(
        final VerifyingKeyIdReference verifierKeyId,
        final byte[] publicInputsHash,
        final ProofBox proof) {
      this.verifierKeyId = Objects.requireNonNull(verifierKeyId, "verifierKeyId");
      this.publicInputsHash = copy(publicInputsHash, "publicInputsHash");
      requireHash(this.publicInputsHash, "public_inputs_hash");
      this.proof = Objects.requireNonNull(proof, "proof");
    }

    public VerifyingKeyIdReference verifierKeyId() {
      return verifierKeyId;
    }

    public byte[] publicInputsHash() {
      return Arrays.copyOf(publicInputsHash, publicInputsHash.length);
    }

    public ProofBox proof() {
      return proof;
    }
  }

  public static final class KeyCertificatePayloadV2 {
    private final String domain;
    private final int version;
    private final String platform;
    private final String keyId;
    private final String deviceId;
    private final String accountId;
    private final byte[] publicKey;
    private final String assertionScheme;
    private final String assertionKeyAlgorithm;
    private final byte[] assertionPublicKey;
    private final Integer assertionUsageCountLimit;
    private final boolean oneUse;

    public KeyCertificatePayloadV2(
        final int version,
        final String platform,
        final String keyId,
        final String deviceId,
        final String accountId,
        final byte[] publicKey,
        final String assertionScheme,
        final String assertionKeyAlgorithm,
        final byte[] assertionPublicKey,
        final Integer assertionUsageCountLimit,
        final boolean oneUse) {
      this(
          KEY_CERTIFICATE_PAYLOAD_DOMAIN,
          version,
          platform,
          keyId,
          deviceId,
          accountId,
          publicKey,
          assertionScheme,
          assertionKeyAlgorithm,
          assertionPublicKey,
          assertionUsageCountLimit,
          oneUse);
    }

    public KeyCertificatePayloadV2(
        final String domain,
        final int version,
        final String platform,
        final String keyId,
        final String deviceId,
        final String accountId,
        final byte[] publicKey,
        final String assertionScheme,
        final String assertionKeyAlgorithm,
        final byte[] assertionPublicKey,
        final Integer assertionUsageCountLimit,
        final boolean oneUse) {
      this.domain = requireNonBlank(domain, "domain");
      this.version = version;
      this.platform = Objects.requireNonNull(platform, "platform");
      this.keyId = Objects.requireNonNull(keyId, "keyId");
      this.deviceId = Objects.requireNonNull(deviceId, "deviceId");
      this.accountId = Objects.requireNonNull(accountId, "accountId");
      this.publicKey = copy(publicKey, "publicKey");
      this.assertionScheme = Objects.requireNonNull(assertionScheme, "assertionScheme");
      this.assertionKeyAlgorithm =
          Objects.requireNonNull(assertionKeyAlgorithm, "assertionKeyAlgorithm");
      this.assertionPublicKey = copy(assertionPublicKey, "assertionPublicKey");
      this.assertionUsageCountLimit = assertionUsageCountLimit;
      this.oneUse = oneUse;
      requireCertificateCore(version, accountId, this.publicKey, oneUse);
      if (assertionUsageCountLimit != null && assertionUsageCountLimit < 0) {
        throw new IllegalArgumentException("assertion usage count limit must be non-negative");
      }
    }

    public String domain() {
      return domain;
    }

    public int version() {
      return version;
    }

    public String platform() {
      return platform;
    }

    public String keyId() {
      return keyId;
    }

    public String deviceId() {
      return deviceId;
    }

    public String accountId() {
      return accountId;
    }

    public byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }

    public String assertionScheme() {
      return assertionScheme;
    }

    public String assertionKeyAlgorithm() {
      return assertionKeyAlgorithm;
    }

    public byte[] assertionPublicKey() {
      return Arrays.copyOf(assertionPublicKey, assertionPublicKey.length);
    }

    public Integer assertionUsageCountLimit() {
      return assertionUsageCountLimit;
    }

    public boolean oneUse() {
      return oneUse;
    }

    public byte[] noritoEncoded() {
      return encodeCertificatePayload(this);
    }

    public byte[] payloadHash() {
      return hash(noritoEncoded());
    }
  }

  public static final class KeyCertificateV2 {
    private final int version;
    private final String platform;
    private final String keyId;
    private final String deviceId;
    private final String accountId;
    private final byte[] publicKey;
    private final String assertionScheme;
    private final String assertionKeyAlgorithm;
    private final byte[] assertionPublicKey;
    private final Integer assertionUsageCountLimit;
    private final boolean oneUse;
    private final byte[] issuerSignature;

    public KeyCertificateV2(
        final int version,
        final String platform,
        final String keyId,
        final String deviceId,
        final String accountId,
        final byte[] publicKey,
        final String assertionScheme,
        final String assertionKeyAlgorithm,
        final byte[] assertionPublicKey,
        final Integer assertionUsageCountLimit,
        final boolean oneUse,
        final byte[] issuerSignature) {
      this.version = version;
      this.platform = Objects.requireNonNull(platform, "platform");
      this.keyId = Objects.requireNonNull(keyId, "keyId");
      this.deviceId = Objects.requireNonNull(deviceId, "deviceId");
      this.accountId = Objects.requireNonNull(accountId, "accountId");
      this.publicKey = copy(publicKey, "publicKey");
      this.assertionScheme = Objects.requireNonNull(assertionScheme, "assertionScheme");
      this.assertionKeyAlgorithm =
          Objects.requireNonNull(assertionKeyAlgorithm, "assertionKeyAlgorithm");
      this.assertionPublicKey = copy(assertionPublicKey, "assertionPublicKey");
      this.assertionUsageCountLimit = assertionUsageCountLimit;
      this.oneUse = oneUse;
      this.issuerSignature = copy(issuerSignature, "issuerSignature");
      requireCertificateCore(version, accountId, this.publicKey, oneUse);
      if (this.issuerSignature.length != 64) {
        throw new IllegalArgumentException("issuer signature must be 64 bytes");
      }
      if (assertionUsageCountLimit != null && assertionUsageCountLimit < 0) {
        throw new IllegalArgumentException("assertion usage count limit must be non-negative");
      }
    }

    public int version() {
      return version;
    }

    public String platform() {
      return platform;
    }

    public String keyId() {
      return keyId;
    }

    public String deviceId() {
      return deviceId;
    }

    public String accountId() {
      return accountId;
    }

    public byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }

    public String assertionScheme() {
      return assertionScheme;
    }

    public String assertionKeyAlgorithm() {
      return assertionKeyAlgorithm;
    }

    public byte[] assertionPublicKey() {
      return Arrays.copyOf(assertionPublicKey, assertionPublicKey.length);
    }

    public Integer assertionUsageCountLimit() {
      return assertionUsageCountLimit;
    }

    public boolean oneUse() {
      return oneUse;
    }

    public byte[] issuerSignature() {
      return Arrays.copyOf(issuerSignature, issuerSignature.length);
    }

    public KeyCertificatePayloadV2 signingPayload() {
      return new KeyCertificatePayloadV2(
          version,
          platform,
          keyId,
          deviceId,
          accountId,
          publicKey(),
          assertionScheme,
          assertionKeyAlgorithm,
          assertionPublicKey(),
          assertionUsageCountLimit,
          oneUse);
    }

    public byte[] signingBytes() {
      return signingPayload().noritoEncoded();
    }

    public byte[] payloadHash() {
      return hash(signingBytes());
    }

    public byte[] noritoEncoded() {
      return encodeCertificate(this);
    }
  }

  public abstract static class CommitmentOriginV2 {
    private CommitmentOriginV2() {}

    public static final class IssuerLoad extends CommitmentOriginV2 {
      private final String operationId;
      private final String lineageId;
      private final long localRevision;

      public IssuerLoad(
          final String operationId, final String lineageId, final long localRevision) {
        this.operationId = requireNonBlank(operationId, "operation_id");
        this.lineageId = requireNonBlank(lineageId, "lineage_id");
        if (localRevision < 0) {
          throw new IllegalArgumentException("local_revision must be non-negative");
        }
        this.localRevision = localRevision;
      }

      public String operationId() {
        return operationId;
      }

      public String lineageId() {
        return lineageId;
      }

      public long localRevision() {
        return localRevision;
      }
    }

    public static final class P2pOutput extends CommitmentOriginV2 {
      private final String paymentRequestId;
      private final int outputIndex;

      public P2pOutput(final String paymentRequestId, final int outputIndex) {
        this.paymentRequestId = requireNonBlank(paymentRequestId, "payment_request_id");
        if (outputIndex < 0) {
          throw new IllegalArgumentException("output_index must be non-negative");
        }
        this.outputIndex = outputIndex;
      }

      public String paymentRequestId() {
        return paymentRequestId;
      }

      public int outputIndex() {
        return outputIndex;
      }
    }
  }

  public static final class NoteCommitmentPreimageV2 {
    private final String domain;
    private final String chainId;
    private final byte[] ownerKeyCertificatePayloadHash;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;
    private final byte[] noteSecret;
    private final CommitmentOriginV2 origin;

    public NoteCommitmentPreimageV2(
        final String chainId,
        final byte[] ownerKeyCertificatePayloadHash,
        final String assetId,
        final String amount,
        final byte[] noteSecret,
        final CommitmentOriginV2 origin) {
      this(
          NOTE_COMMITMENT_DOMAIN,
          chainId,
          ownerKeyCertificatePayloadHash,
          assetId,
          amount,
          noteSecret,
          origin);
    }

    public NoteCommitmentPreimageV2(
        final String domain,
        final String chainId,
        final byte[] ownerKeyCertificatePayloadHash,
        final String assetId,
        final String amount,
        final byte[] noteSecret,
        final CommitmentOriginV2 origin) {
      if (!NOTE_COMMITMENT_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported note commitment domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlank(chainId, "chain_id");
      this.ownerKeyCertificatePayloadHash =
          copy(ownerKeyCertificatePayloadHash, "ownerKeyCertificatePayloadHash");
      requireHash(this.ownerKeyCertificatePayloadHash, "owner_key_certificate_payload_hash");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      parseAssetId(assetId);
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
      this.noteSecret = copy(noteSecret, "noteSecret");
      requireRandomBytes(this.noteSecret, "note_secret");
      this.origin = Objects.requireNonNull(origin, "origin");
    }

    public String domain() {
      return domain;
    }

    public String chainId() {
      return chainId;
    }

    public byte[] ownerKeyCertificatePayloadHash() {
      return Arrays.copyOf(
          ownerKeyCertificatePayloadHash, ownerKeyCertificatePayloadHash.length);
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public byte[] noteSecret() {
      return Arrays.copyOf(noteSecret, noteSecret.length);
    }

    public CommitmentOriginV2 origin() {
      return origin;
    }

    public byte[] noritoEncoded() {
      return encodeNoteCommitmentPreimage(this);
    }

    public byte[] deriveNoteCommitment() {
      return OfflineNoteV2.deriveNoteCommitment(this);
    }
  }

  public static final class InputNullifierPreimageV2 {
    private final String domain;
    private final String chainId;
    private final byte[] sourceNoteCommitment;
    private final byte[] ownerKeyCertificatePayloadHash;
    private final byte[] noteSecret;

    public InputNullifierPreimageV2(
        final String chainId,
        final byte[] sourceNoteCommitment,
        final byte[] ownerKeyCertificatePayloadHash,
        final byte[] noteSecret) {
      this(
          INPUT_NULLIFIER_DOMAIN,
          chainId,
          sourceNoteCommitment,
          ownerKeyCertificatePayloadHash,
          noteSecret);
    }

    public InputNullifierPreimageV2(
        final String domain,
        final String chainId,
        final byte[] sourceNoteCommitment,
        final byte[] ownerKeyCertificatePayloadHash,
        final byte[] noteSecret) {
      if (!INPUT_NULLIFIER_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported input nullifier domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlank(chainId, "chain_id");
      this.sourceNoteCommitment = copy(sourceNoteCommitment, "sourceNoteCommitment");
      this.ownerKeyCertificatePayloadHash =
          copy(ownerKeyCertificatePayloadHash, "ownerKeyCertificatePayloadHash");
      this.noteSecret = copy(noteSecret, "noteSecret");
      requireHash(this.sourceNoteCommitment, "source_note_commitment");
      requireHash(this.ownerKeyCertificatePayloadHash, "owner_key_certificate_payload_hash");
      requireRandomBytes(this.noteSecret, "note_secret");
    }

    public String domain() {
      return domain;
    }

    public String chainId() {
      return chainId;
    }

    public byte[] sourceNoteCommitment() {
      return Arrays.copyOf(sourceNoteCommitment, sourceNoteCommitment.length);
    }

    public byte[] ownerKeyCertificatePayloadHash() {
      return Arrays.copyOf(
          ownerKeyCertificatePayloadHash, ownerKeyCertificatePayloadHash.length);
    }

    public byte[] noteSecret() {
      return Arrays.copyOf(noteSecret, noteSecret.length);
    }

    public byte[] noritoEncoded() {
      return encodeInputNullifierPreimage(this);
    }

    public byte[] deriveInputNullifier() {
      return OfflineNoteV2.deriveInputNullifier(this);
    }
  }

  public static final class PaymentTokenIdPreimageV2 {
    private final String domain;
    private final String chainId;
    private final byte[] tokenNonce;
    private final byte[] senderKeyCertificatePayloadHash;
    private final List<byte[]> inputNullifiers;
    private final List<byte[]> outputCommitments;

    public PaymentTokenIdPreimageV2(
        final String chainId,
        final byte[] tokenNonce,
        final byte[] senderKeyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<byte[]> outputCommitments) {
      this(
          PAYMENT_TOKEN_ID_DOMAIN,
          chainId,
          tokenNonce,
          senderKeyCertificatePayloadHash,
          inputNullifiers,
          outputCommitments);
    }

    public PaymentTokenIdPreimageV2(
        final String domain,
        final String chainId,
        final byte[] tokenNonce,
        final byte[] senderKeyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<byte[]> outputCommitments) {
      if (!PAYMENT_TOKEN_ID_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported payment token id domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlank(chainId, "chain_id");
      this.tokenNonce = copy(tokenNonce, "tokenNonce");
      this.senderKeyCertificatePayloadHash =
          copy(senderKeyCertificatePayloadHash, "senderKeyCertificatePayloadHash");
      this.inputNullifiers = copyByteList(inputNullifiers, "inputNullifiers");
      this.outputCommitments = copyByteList(outputCommitments, "outputCommitments");
      requireRandomBytes(this.tokenNonce, "token_nonce");
      requireHash(this.senderKeyCertificatePayloadHash, "sender_key_certificate_payload_hash");
      requireHashes(this.inputNullifiers, "input_nullifiers");
      requireHashes(this.outputCommitments, "output_commitments");
    }

    public String domain() {
      return domain;
    }

    public String chainId() {
      return chainId;
    }

    public byte[] tokenNonce() {
      return Arrays.copyOf(tokenNonce, tokenNonce.length);
    }

    public byte[] senderKeyCertificatePayloadHash() {
      return Arrays.copyOf(
          senderKeyCertificatePayloadHash, senderKeyCertificatePayloadHash.length);
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public List<byte[]> outputCommitments() {
      return copyByteList(outputCommitments, "outputCommitments");
    }

    public byte[] noritoEncoded() {
      return encodePaymentTokenIdPreimage(this);
    }

    public byte[] derivePaymentTokenId() {
      return OfflineNoteV2.derivePaymentTokenId(this);
    }
  }

  public static final class IssueV2 {
    private final byte[] noteCommitment;
    private final KeyCertificateV2 keyCertificate;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public IssueV2(
        final byte[] noteCommitment,
        final KeyCertificateV2 keyCertificate,
        final String assetId,
        final String amount) {
      this.noteCommitment = copy(noteCommitment, "noteCommitment");
      requireHash(this.noteCommitment, "note_commitment");
      this.keyCertificate = Objects.requireNonNull(keyCertificate, "keyCertificate");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      parseAssetId(assetId);
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
    }

    public byte[] noteCommitment() {
      return Arrays.copyOf(noteCommitment, noteCommitment.length);
    }

    public KeyCertificateV2 keyCertificate() {
      return keyCertificate;
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public IssuedClaimV2 issuedClaim() {
      return new IssuedClaimV2(
          noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
    }

    public byte[] noritoEncoded() {
      return encodeIssue(this);
    }
  }

  public static final class IssuedClaimV2 {
    private final String domain;
    private final byte[] noteCommitment;
    private final byte[] keyCertificatePayloadHash;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public IssuedClaimV2(
        final byte[] noteCommitment,
        final byte[] keyCertificatePayloadHash,
        final String assetId,
        final String amount) {
      this(ISSUED_CLAIM_DOMAIN, noteCommitment, keyCertificatePayloadHash, assetId, amount);
    }

    public IssuedClaimV2(
        final String domain,
        final byte[] noteCommitment,
        final byte[] keyCertificatePayloadHash,
        final String assetId,
        final String amount) {
      this.domain = requireNonBlank(domain, "domain");
      this.noteCommitment = copy(noteCommitment, "noteCommitment");
      this.keyCertificatePayloadHash =
          copy(keyCertificatePayloadHash, "keyCertificatePayloadHash");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
      requireHash(this.noteCommitment, "note_commitment");
      requireHash(this.keyCertificatePayloadHash, "key_certificate_payload_hash");
      parseAssetId(assetId);
    }

    public String domain() {
      return domain;
    }

    public byte[] noteCommitment() {
      return Arrays.copyOf(noteCommitment, noteCommitment.length);
    }

    public byte[] keyCertificatePayloadHash() {
      return Arrays.copyOf(keyCertificatePayloadHash, keyCertificatePayloadHash.length);
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public byte[] noritoEncoded() {
      return encodeIssuedClaim(this);
    }

    public byte[] claimHash() {
      return hash(noritoEncoded());
    }
  }

  public static final class AuditOutputClaimV2 {
    private final byte[] noteCommitment;
    private final KeyCertificateV2 keyCertificate;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public AuditOutputClaimV2(
        final byte[] noteCommitment,
        final KeyCertificateV2 keyCertificate,
        final String assetId,
        final String amount) {
      this.noteCommitment = copy(noteCommitment, "noteCommitment");
      requireHash(this.noteCommitment, "note_commitment");
      this.keyCertificate = Objects.requireNonNull(keyCertificate, "keyCertificate");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      parseAssetId(assetId);
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
    }

    public byte[] noteCommitment() {
      return Arrays.copyOf(noteCommitment, noteCommitment.length);
    }

    public KeyCertificateV2 keyCertificate() {
      return keyCertificate;
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public IssuedClaimV2 issuedClaim() {
      return new IssuedClaimV2(
          noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
    }
  }

  public static final class RedeemPublicInputsV2 {
    private final String domain;
    private final byte[] sourceNoteCommitment;
    private final List<byte[]> inputNullifiers;
    private final byte[] keyCertificatePayloadHash;
    private final String recipient;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public RedeemPublicInputsV2(
        final byte[] sourceNoteCommitment,
        final List<byte[]> inputNullifiers,
        final byte[] keyCertificatePayloadHash,
        final String recipient,
        final String assetId,
        final String amount) {
      this(
          REDEEM_PUBLIC_INPUTS_DOMAIN,
          sourceNoteCommitment,
          inputNullifiers,
          keyCertificatePayloadHash,
          recipient,
          assetId,
          amount);
    }

    public RedeemPublicInputsV2(
        final String domain,
        final byte[] sourceNoteCommitment,
        final List<byte[]> inputNullifiers,
        final byte[] keyCertificatePayloadHash,
        final String recipient,
        final String assetId,
        final String amount) {
      this.domain = requireNonBlank(domain, "domain");
      this.sourceNoteCommitment = copy(sourceNoteCommitment, "sourceNoteCommitment");
      this.inputNullifiers = copyByteList(inputNullifiers, "inputNullifiers");
      this.keyCertificatePayloadHash =
          copy(keyCertificatePayloadHash, "keyCertificatePayloadHash");
      this.recipient = Objects.requireNonNull(recipient, "recipient");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
      requireHash(this.sourceNoteCommitment, "source_note_commitment");
      requireHashes(this.inputNullifiers, "input_nullifiers");
      requireHash(this.keyCertificatePayloadHash, "key_certificate_payload_hash");
      encodeAccountIdPayload(recipient);
      parseAssetId(assetId);
    }

    public String domain() {
      return domain;
    }

    public byte[] sourceNoteCommitment() {
      return Arrays.copyOf(sourceNoteCommitment, sourceNoteCommitment.length);
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public byte[] keyCertificatePayloadHash() {
      return Arrays.copyOf(keyCertificatePayloadHash, keyCertificatePayloadHash.length);
    }

    public String recipient() {
      return recipient;
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public byte[] noritoEncoded() {
      return encodeRedeemPublicInputs(this);
    }

    public byte[] publicInputsHash() {
      return hash(noritoEncoded());
    }
  }

  public static final class RedeemV2 {
    private final byte[] sourceNoteCommitment;
    private final List<byte[]> inputNullifiers;
    private final KeyCertificateV2 senderKeyCertificate;
    private final String recipient;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;
    private final RecursiveProofV2 recursiveProof;

    public RedeemV2(
        final byte[] sourceNoteCommitment,
        final List<byte[]> inputNullifiers,
        final KeyCertificateV2 senderKeyCertificate,
        final String recipient,
        final String assetId,
        final String amount,
        final RecursiveProofV2 recursiveProof) {
      this.sourceNoteCommitment = copy(sourceNoteCommitment, "sourceNoteCommitment");
      this.inputNullifiers = copyByteList(inputNullifiers, "inputNullifiers");
      this.senderKeyCertificate =
          Objects.requireNonNull(senderKeyCertificate, "senderKeyCertificate");
      this.recipient = Objects.requireNonNull(recipient, "recipient");
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      this.amount = Objects.requireNonNull(amount, "amount");
      this.canonicalAmount = parseNumeric(amount).canonicalString;
      this.recursiveProof = Objects.requireNonNull(recursiveProof, "recursiveProof");
      requireHash(this.sourceNoteCommitment, "source_note_commitment");
      requireHashes(this.inputNullifiers, "input_nullifiers");
      encodeAccountIdPayload(recipient);
      parseAssetId(assetId);
    }

    public byte[] sourceNoteCommitment() {
      return Arrays.copyOf(sourceNoteCommitment, sourceNoteCommitment.length);
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public KeyCertificateV2 senderKeyCertificate() {
      return senderKeyCertificate;
    }

    public String recipient() {
      return recipient;
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String canonicalAmount() {
      return canonicalAmount;
    }

    public RecursiveProofV2 recursiveProof() {
      return recursiveProof;
    }

    public RedeemPublicInputsV2 publicInputs() {
      return new RedeemPublicInputsV2(
          sourceNoteCommitment(),
          inputNullifiers(),
          senderKeyCertificate.payloadHash(),
          recipient,
          assetId,
          canonicalAmount);
    }

    public byte[] publicInputsHash() {
      return publicInputs().publicInputsHash();
    }

    public void validateProofBinding() {
      if (!Arrays.equals(recursiveProof.publicInputsHash(), publicInputsHash())) {
        throw new IllegalArgumentException("recursive proof public inputs hash mismatch");
      }
    }

    public RedeemV2 replacingRecursiveProof(final RecursiveProofV2 recursiveProof) {
      return new RedeemV2(
          sourceNoteCommitment(),
          inputNullifiers(),
          senderKeyCertificate,
          recipient,
          assetId,
          amount,
          recursiveProof);
    }

    public byte[] noritoEncoded() {
      return encodeRedeem(this);
    }
  }

  public static final class AuditPublicInputsV2 {
    private final String domain;
    private final byte[] tokenId;
    private final byte[] keyCertificatePayloadHash;
    private final List<byte[]> inputNullifiers;
    private final List<IssuedClaimV2> inputClaims;
    private final List<byte[]> outputCommitments;
    private final List<IssuedClaimV2> outputClaims;

    public AuditPublicInputsV2(
        final byte[] tokenId,
        final byte[] keyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaimV2> inputClaims,
        final List<byte[]> outputCommitments,
        final List<IssuedClaimV2> outputClaims) {
      this(
          AUDIT_PUBLIC_INPUTS_DOMAIN,
          tokenId,
          keyCertificatePayloadHash,
          inputNullifiers,
          inputClaims,
          outputCommitments,
          outputClaims);
    }

    public AuditPublicInputsV2(
        final String domain,
        final byte[] tokenId,
        final byte[] keyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaimV2> inputClaims,
        final List<byte[]> outputCommitments,
        final List<IssuedClaimV2> outputClaims) {
      this.domain = requireNonBlank(domain, "domain");
      this.tokenId = copy(tokenId, "tokenId");
      this.keyCertificatePayloadHash =
          copy(keyCertificatePayloadHash, "keyCertificatePayloadHash");
      this.inputNullifiers = copyByteList(inputNullifiers, "inputNullifiers");
      this.inputClaims =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(inputClaims)));
      this.outputCommitments = copyByteList(outputCommitments, "outputCommitments");
      this.outputClaims =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(outputClaims)));
      requireHash(this.tokenId, "token_id");
      requireHash(this.keyCertificatePayloadHash, "key_certificate_payload_hash");
      requireHashes(this.inputNullifiers, "input_nullifiers");
      if (this.inputClaims.isEmpty()) {
        throw new IllegalArgumentException("input claims must not be empty");
      }
      if (this.inputClaims.size() != this.inputNullifiers.size()) {
        throw new IllegalArgumentException(
            "input nullifier count must match input claim count");
      }
      requireHashes(this.outputCommitments, "output_commitments");
      if (this.outputClaims.isEmpty()) {
        throw new IllegalArgumentException("output claims must not be empty");
      }
      final List<String> committed = new ArrayList<>();
      for (final byte[] commitment : this.outputCommitments) {
        committed.add(hexLower(commitment));
      }
      for (final IssuedClaimV2 claim : this.outputClaims) {
        if (!committed.contains(hexLower(claim.noteCommitment()))) {
          throw new IllegalArgumentException("audit output claim is not listed in output commitments");
        }
      }
    }

    public String domain() {
      return domain;
    }

    public byte[] tokenId() {
      return Arrays.copyOf(tokenId, tokenId.length);
    }

    public byte[] keyCertificatePayloadHash() {
      return Arrays.copyOf(keyCertificatePayloadHash, keyCertificatePayloadHash.length);
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public List<IssuedClaimV2> inputClaims() {
      return inputClaims;
    }

    public List<byte[]> outputCommitments() {
      return copyByteList(outputCommitments, "outputCommitments");
    }

    public List<IssuedClaimV2> outputClaims() {
      return outputClaims;
    }

    public byte[] noritoEncoded() {
      return encodeAuditPublicInputs(this);
    }

    public byte[] publicInputsHash() {
      return hash(noritoEncoded());
    }
  }

  public static final class AuditBundleV2 {
    private final byte[] tokenId;
    private final KeyCertificateV2 senderKeyCertificate;
    private final List<byte[]> inputNullifiers;
    private final List<IssuedClaimV2> inputClaims;
    private final List<byte[]> outputCommitments;
    private final List<AuditOutputClaimV2> outputClaims;
    private final RecursiveProofV2 recursiveProof;

    public AuditBundleV2(
        final byte[] tokenId,
        final KeyCertificateV2 senderKeyCertificate,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaimV2> inputClaims,
        final List<byte[]> outputCommitments,
        final List<AuditOutputClaimV2> outputClaims,
        final RecursiveProofV2 recursiveProof) {
      this.tokenId = copy(tokenId, "tokenId");
      this.senderKeyCertificate =
          Objects.requireNonNull(senderKeyCertificate, "senderKeyCertificate");
      this.inputNullifiers = copyByteList(inputNullifiers, "inputNullifiers");
      this.inputClaims =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(inputClaims)));
      this.outputCommitments = copyByteList(outputCommitments, "outputCommitments");
      this.outputClaims =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(outputClaims)));
      this.recursiveProof = Objects.requireNonNull(recursiveProof, "recursiveProof");
      requireHash(this.tokenId, "token_id");
      requireHashes(this.inputNullifiers, "input_nullifiers");
      if (this.inputClaims.isEmpty()) {
        throw new IllegalArgumentException("input claims must not be empty");
      }
      if (this.inputClaims.size() != this.inputNullifiers.size()) {
        throw new IllegalArgumentException(
            "input nullifier count must match input claim count");
      }
      requireHashes(this.outputCommitments, "output_commitments");
      if (this.outputClaims.isEmpty()) {
        throw new IllegalArgumentException("output claims must not be empty");
      }
    }

    public byte[] tokenId() {
      return Arrays.copyOf(tokenId, tokenId.length);
    }

    public KeyCertificateV2 senderKeyCertificate() {
      return senderKeyCertificate;
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public List<IssuedClaimV2> inputClaims() {
      return inputClaims;
    }

    public List<byte[]> outputCommitments() {
      return copyByteList(outputCommitments, "outputCommitments");
    }

    public List<AuditOutputClaimV2> outputClaims() {
      return outputClaims;
    }

    public RecursiveProofV2 recursiveProof() {
      return recursiveProof;
    }

    public AuditPublicInputsV2 publicInputs() {
      final List<IssuedClaimV2> issuedOutputs = new ArrayList<>();
      for (final AuditOutputClaimV2 claim : outputClaims) {
        issuedOutputs.add(claim.issuedClaim());
      }
      return new AuditPublicInputsV2(
          tokenId(),
          senderKeyCertificate.payloadHash(),
          inputNullifiers(),
          inputClaims,
          outputCommitments(),
          issuedOutputs);
    }

    public byte[] publicInputsHash() {
      return publicInputs().publicInputsHash();
    }

    public void validateProofBinding() {
      if (!Arrays.equals(recursiveProof.publicInputsHash(), publicInputsHash())) {
        throw new IllegalArgumentException("recursive proof public inputs hash mismatch");
      }
    }

    public AuditBundleV2 replacingRecursiveProof(final RecursiveProofV2 recursiveProof) {
      return new AuditBundleV2(
          tokenId(),
          senderKeyCertificate,
          inputNullifiers(),
          inputClaims,
          outputCommitments(),
          outputClaims,
          recursiveProof);
    }

    public byte[] noritoEncoded() {
      return encodeAudit(this);
    }
  }

  public static final class InstanceValues {
    private final long[] publicValues;
    private final long[] inputAmounts;
    private final long[] outputAmounts;

    public InstanceValues(
        final long[] publicValues, final long[] inputAmounts, final long[] outputAmounts) {
      this.publicValues = Arrays.copyOf(publicValues, publicValues.length);
      this.inputAmounts = Arrays.copyOf(inputAmounts, inputAmounts.length);
      this.outputAmounts = Arrays.copyOf(outputAmounts, outputAmounts.length);
      if (this.publicValues.length != PUBLIC_VALUE_COUNT) {
        throw new IllegalArgumentException(
            "Offline V2 public instance count must be " + PUBLIC_VALUE_COUNT);
      }
      if (this.inputAmounts.length != MAX_INPUT_AMOUNTS) {
        throw new IllegalArgumentException(
            "Offline V2 input amount witness count must be " + MAX_INPUT_AMOUNTS);
      }
      if (this.outputAmounts.length != MAX_OUTPUT_AMOUNTS) {
        throw new IllegalArgumentException(
            "Offline V2 output amount witness count must be " + MAX_OUTPUT_AMOUNTS);
      }
    }

    public long[] publicValues() {
      return Arrays.copyOf(publicValues, publicValues.length);
    }

    public long[] inputAmounts() {
      return Arrays.copyOf(inputAmounts, inputAmounts.length);
    }

    public long[] outputAmounts() {
      return Arrays.copyOf(outputAmounts, outputAmounts.length);
    }

    public List<byte[]> publicInstanceColumns() {
      final List<byte[]> columns = new ArrayList<>();
      for (final long value : publicValues) {
        columns.add(instanceScalarBytes(value));
      }
      return Collections.unmodifiableList(columns);
    }
  }

  public static final class InstanceBuilder {
    private InstanceBuilder() {}

    public static InstanceValues redeemInstanceValues(final RedeemV2 redemption) {
      final long inputCount =
          validateCount(redemption.inputNullifiers().size(), MAX_INPUT_AMOUNTS, "redemption input");
      final List<String> amounts = new ArrayList<>();
      amounts.add(redemption.canonicalAmount());
      amounts.add(redemption.canonicalAmount());
      final List<Long> normalizedAmounts = normalizedAmountUnits(amounts);
      final long inputSum = normalizedAmounts.get(0);
      final long outputSum = normalizedAmounts.get(1);
      final byte[] issuedClaimHash =
          new IssuedClaimV2(
                  redemption.sourceNoteCommitment(),
                  redemption.senderKeyCertificate().payloadHash(),
                  redemption.assetId(),
                  redemption.canonicalAmount())
              .claimHash();
      final long[] publicValues =
          publicValues(
              redemption.publicInputsHash(),
              MODE_REDEEM,
              inputCount,
              1L,
              inputSum,
              outputSum,
              hashLimb0Sum(redemption.inputNullifiers()),
              0L,
              redemption.senderKeyCertificate().payloadHash(),
              redemption.sourceNoteCommitment(),
              hashLimb0(issuedClaimHash),
              0L);
      final long[] inputAmounts = new long[MAX_INPUT_AMOUNTS];
      inputAmounts[0] = inputSum;
      final long[] outputAmounts = new long[MAX_OUTPUT_AMOUNTS];
      outputAmounts[0] = outputSum;
      return new InstanceValues(publicValues, inputAmounts, outputAmounts);
    }

    public static InstanceValues auditInstanceValues(final AuditBundleV2 audit) {
      final long inputCount =
          validateCount(audit.inputClaims().size(), MAX_INPUT_AMOUNTS, "audit input");
      final long outputCount =
          validateCount(audit.outputClaims().size(), MAX_OUTPUT_AMOUNTS, "audit output");
      if (audit.inputNullifiers().size() != audit.inputClaims().size()) {
        throw new IllegalArgumentException(
            "audit input nullifier count must match input claim count");
      }

      final List<byte[]> inputClaimHashes = new ArrayList<>();
      final List<String> amountStrings = new ArrayList<>();
      for (final IssuedClaimV2 claim : audit.inputClaims()) {
        inputClaimHashes.add(claim.claimHash());
        amountStrings.add(claim.canonicalAmount());
      }
      final List<byte[]> outputClaimHashes = new ArrayList<>();
      for (final AuditOutputClaimV2 claim : audit.outputClaims()) {
        final IssuedClaimV2 issued = claim.issuedClaim();
        outputClaimHashes.add(issued.claimHash());
        amountStrings.add(issued.canonicalAmount());
      }

      final List<Long> normalizedAmounts = normalizedAmountUnits(amountStrings);
      final List<Long> inputUnits =
          normalizedAmounts.subList(0, audit.inputClaims().size());
      final List<Long> outputUnits =
          normalizedAmounts.subList(audit.inputClaims().size(), normalizedAmounts.size());
      final long inputSum = checkedSum(inputUnits, "input");
      final long outputSum = checkedSum(outputUnits, "output");
      if (inputSum != outputSum) {
        throw new IllegalArgumentException("Offline V2 audit amounts are not conserved");
      }

      final long[] inputAmounts = new long[MAX_INPUT_AMOUNTS];
      for (int idx = 0; idx < inputUnits.size(); idx++) {
        inputAmounts[idx] = inputUnits.get(idx);
      }
      final long[] outputAmounts = new long[MAX_OUTPUT_AMOUNTS];
      for (int idx = 0; idx < outputUnits.size(); idx++) {
        outputAmounts[idx] = outputUnits.get(idx);
      }

      return new InstanceValues(
          publicValues(
              audit.publicInputsHash(),
              MODE_AUDIT,
              inputCount,
              outputCount,
              inputSum,
              outputSum,
              hashLimb0Sum(audit.inputNullifiers()),
              hashLimb0Sum(audit.outputCommitments()),
              audit.senderKeyCertificate().payloadHash(),
              audit.tokenId(),
              hashLimb0Sum(inputClaimHashes),
              hashLimb0Sum(outputClaimHashes)),
          inputAmounts,
          outputAmounts);
    }
  }

  private static final TypeAdapter<byte[]> INSTRUCTION_WRAPPER_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final byte[] value) {
          writeField(encoder, child -> child.writeBytes(value));
        }

        @Override
        public byte[] decode(final NoritoDecoder decoder) {
          return readField(decoder, child -> child.readBytes(child.remaining()));
        }
      };

  private record InstructionModelPayload(byte[] bytes, int flags) {}

  private static final TypeAdapter<InstructionModelPayload> INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final InstructionModelPayload value) {
          writeField(encoder, child -> child.writeBytes(value.bytes()));
        }

        @Override
        public InstructionModelPayload decode(final NoritoDecoder decoder) {
          return new InstructionModelPayload(
              readField(decoder, child -> child.readBytes(child.remaining())),
              decoder.flags());
        }
      };

  private static final TypeAdapter<KeyCertificatePayloadV2> KEY_CERTIFICATE_PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final KeyCertificatePayloadV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.platform()));
          writeField(encoder, child -> writeString(child, value.keyId()));
          writeField(encoder, child -> writeString(child, value.deviceId()));
          writeField(encoder, child -> writeAccountId(child, value.accountId()));
          writeField(encoder, child -> writeBytesVec(child, value.publicKey()));
          writeField(encoder, child -> writeString(child, value.assertionScheme()));
          writeField(encoder, child -> writeString(child, value.assertionKeyAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.assertionPublicKey()));
          writeField(encoder, child -> writeOptionU32(child, value.assertionUsageCountLimit()));
          writeField(encoder, child -> child.writeByte(value.oneUse() ? 1 : 0));
        }

        @Override
        public KeyCertificatePayloadV2 decode(final NoritoDecoder decoder) {
          return new KeyCertificatePayloadV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, child -> (int) child.readUInt(16)),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readAccountId),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, OfflineNoteV2::readOptionU32),
              readField(decoder, OfflineNoteV2::readBool));
        }
      };

  private static final TypeAdapter<KeyCertificateV2> KEY_CERTIFICATE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final KeyCertificateV2 value) {
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.platform()));
          writeField(encoder, child -> writeString(child, value.keyId()));
          writeField(encoder, child -> writeString(child, value.deviceId()));
          writeField(encoder, child -> writeAccountId(child, value.accountId()));
          writeField(encoder, child -> writeBytesVec(child, value.publicKey()));
          writeField(encoder, child -> writeString(child, value.assertionScheme()));
          writeField(encoder, child -> writeString(child, value.assertionKeyAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.assertionPublicKey()));
          writeField(encoder, child -> writeOptionU32(child, value.assertionUsageCountLimit()));
          writeField(encoder, child -> child.writeByte(value.oneUse() ? 1 : 0));
          writeField(encoder, child -> writeConstVec(child, value.issuerSignature()));
        }

        @Override
        public KeyCertificateV2 decode(final NoritoDecoder decoder) {
          return new KeyCertificateV2(
              readField(decoder, child -> (int) child.readUInt(16)),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readAccountId),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, OfflineNoteV2::readOptionU32),
              readField(decoder, OfflineNoteV2::readBool),
              readField(decoder, OfflineNoteV2::readConstVec));
        }
      };

  private static final TypeAdapter<RecursiveProofV2> RECURSIVE_PROOF_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RecursiveProofV2 value) {
          writeField(encoder, child -> writeVerifyingKeyId(child, value.verifierKeyId()));
          writeField(encoder, child -> child.writeBytes(value.publicInputsHash()));
          writeField(encoder, child -> writeProofBox(child, value.proof()));
        }

        @Override
        public RecursiveProofV2 decode(final NoritoDecoder decoder) {
          return new RecursiveProofV2(
              readField(decoder, OfflineNoteV2::readVerifyingKeyId),
              readField(decoder, child -> readHash(child, "public_inputs_hash")),
              readField(decoder, OfflineNoteV2::readProofBox));
        }
      };

  private static final TypeAdapter<IssueV2> ISSUE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final IssueV2 value) {
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.keyCertificate()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public IssueV2 decode(final NoritoDecoder decoder) {
          return new IssueV2(
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric));
        }
      };

  private static final TypeAdapter<IssuedClaimV2> ISSUED_CLAIM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final IssuedClaimV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> child.writeBytes(value.keyCertificatePayloadHash()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public IssuedClaimV2 decode(final NoritoDecoder decoder) {
          return new IssuedClaimV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric));
        }
      };

  private static final TypeAdapter<AuditOutputClaimV2> AUDIT_OUTPUT_CLAIM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditOutputClaimV2 value) {
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.keyCertificate()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public AuditOutputClaimV2 decode(final NoritoDecoder decoder) {
          return new AuditOutputClaimV2(
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric));
        }
      };

  private static final TypeAdapter<RedeemPublicInputsV2> REDEEM_PUBLIC_INPUTS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RedeemPublicInputsV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeBytes(value.sourceNoteCommitment()));
          writeField(encoder, child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(encoder, child -> child.writeBytes(value.keyCertificatePayloadHash()));
          writeField(encoder, child -> writeAccountId(child, value.recipient()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public RedeemPublicInputsV2 decode(final NoritoDecoder decoder) {
          return new RedeemPublicInputsV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, OfflineNoteV2::readAccountId),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric));
        }
      };

  private static final TypeAdapter<RedeemV2> REDEEM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RedeemV2 value) {
          writeField(encoder, child -> child.writeBytes(value.sourceNoteCommitment()));
          writeField(encoder, child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(
              encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.senderKeyCertificate()));
          writeField(encoder, child -> writeAccountId(child, value.recipient()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
          writeField(encoder, child -> RECURSIVE_PROOF_ADAPTER.encode(child, value.recursiveProof()));
        }

        @Override
        public RedeemV2 decode(final NoritoDecoder decoder) {
          return new RedeemV2(
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNoteV2::readAccountId),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric),
              readField(decoder, RECURSIVE_PROOF_ADAPTER::decode));
        }
      };

  private static final TypeAdapter<AuditPublicInputsV2> AUDIT_PUBLIC_INPUTS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditPublicInputsV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(encoder, child -> child.writeBytes(value.keyCertificatePayloadHash()));
          writeField(encoder, child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(
              encoder, child -> writeVec(child, value.inputClaims(), ISSUED_CLAIM_ADAPTER::encode));
          writeField(encoder, child -> writeVec(child, value.outputCommitments(), NoritoEncoder::writeBytes));
          writeField(
              encoder, child -> writeVec(child, value.outputClaims(), ISSUED_CLAIM_ADAPTER::encode));
        }

        @Override
        public AuditPublicInputsV2 decode(final NoritoDecoder decoder) {
          return new AuditPublicInputsV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, child -> readHash(child, "token_id")),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)),
              readField(decoder, child -> readVec(child, element -> readHash(element, "output_commitment"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)));
        }
      };

  private static final TypeAdapter<AuditBundleV2> AUDIT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditBundleV2 value) {
          writeField(encoder, child -> child.writeBytes(value.tokenId()));
          writeField(
              encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.senderKeyCertificate()));
          writeField(encoder, child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(
              encoder, child -> writeVec(child, value.inputClaims(), ISSUED_CLAIM_ADAPTER::encode));
          writeField(encoder, child -> writeVec(child, value.outputCommitments(), NoritoEncoder::writeBytes));
          writeField(
              encoder,
              child -> writeVec(child, value.outputClaims(), AUDIT_OUTPUT_CLAIM_ADAPTER::encode));
          writeField(encoder, child -> RECURSIVE_PROOF_ADAPTER.encode(child, value.recursiveProof()));
        }

        @Override
        public AuditBundleV2 decode(final NoritoDecoder decoder) {
          return new AuditBundleV2(
              readField(decoder, child -> readHash(child, "token_id")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)),
              readField(decoder, child -> readVec(child, element -> readHash(element, "output_commitment"))),
              readField(decoder, child -> readVec(child, AUDIT_OUTPUT_CLAIM_ADAPTER::decode)),
              readField(decoder, RECURSIVE_PROOF_ADAPTER::decode));
        }
      };

  private static final TypeAdapter<NoteCommitmentPreimageV2> NOTE_COMMITMENT_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final NoteCommitmentPreimageV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> child.writeBytes(value.ownerKeyCertificatePayloadHash()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
          writeField(encoder, child -> writeBytesVec(child, value.noteSecret()));
          writeField(encoder, child -> writeCommitmentOrigin(child, value.origin()));
        }

        @Override
        public NoteCommitmentPreimageV2 decode(final NoritoDecoder decoder) {
          return new NoteCommitmentPreimageV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readChainId),
              readField(decoder, child -> readHash(child, "owner_key_certificate_payload_hash")),
              readField(decoder, OfflineNoteV2::readAssetId),
              readField(decoder, OfflineNoteV2::readNumeric),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, OfflineNoteV2::readCommitmentOrigin));
        }
      };

  private static final TypeAdapter<InputNullifierPreimageV2> INPUT_NULLIFIER_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final InputNullifierPreimageV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> child.writeBytes(value.sourceNoteCommitment()));
          writeField(encoder, child -> child.writeBytes(value.ownerKeyCertificatePayloadHash()));
          writeField(encoder, child -> writeBytesVec(child, value.noteSecret()));
        }

        @Override
        public InputNullifierPreimageV2 decode(final NoritoDecoder decoder) {
          return new InputNullifierPreimageV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readChainId),
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readHash(child, "owner_key_certificate_payload_hash")),
              readField(decoder, OfflineNoteV2::readBytesVec));
        }
      };

  private static final TypeAdapter<PaymentTokenIdPreimageV2> PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final PaymentTokenIdPreimageV2 value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> writeBytesVec(child, value.tokenNonce()));
          writeField(encoder, child -> child.writeBytes(value.senderKeyCertificatePayloadHash()));
          writeField(
              encoder,
              child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(
              encoder,
              child -> writeVec(child, value.outputCommitments(), NoritoEncoder::writeBytes));
        }

        @Override
        public PaymentTokenIdPreimageV2 decode(final NoritoDecoder decoder) {
          return new PaymentTokenIdPreimageV2(
              readField(decoder, OfflineNoteV2::readString),
              readField(decoder, OfflineNoteV2::readChainId),
              readField(decoder, OfflineNoteV2::readBytesVec),
              readField(decoder, child -> readHash(child, "sender_key_certificate_payload_hash")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readVec(child, element -> readHash(element, "output_commitment"))));
        }
      };

  @FunctionalInterface
  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  @FunctionalInterface
  private interface ElementWriter<T> {
    void write(NoritoEncoder encoder, T value);
  }

  @FunctionalInterface
  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  private static <T> T readField(
      final NoritoDecoder parent, final FieldReader<T> readPayload) {
    final int length = checkedLength(parent.readLength(compact(parent)), "field length");
    final NoritoDecoder child = new NoritoDecoder(parent.readBytes(length), parent.flags(), parent.flagsHint());
    final T value = readPayload.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after Offline Note V2 field decode");
    }
    return value;
  }

  private static String readString(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readLength(compact(decoder)), "string length");
    return new String(decoder.readBytes(length), StandardCharsets.UTF_8);
  }

  private static boolean readBool(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return false;
    }
    if (tag == 1) {
      return true;
    }
    throw new IllegalArgumentException("invalid boolean tag: " + tag);
  }

  private static byte[] readBytesVec(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readUInt(64), "byte vector length");
    return decoder.readBytes(length);
  }

  private static byte[] readConstVec(final NoritoDecoder decoder) {
    final int length = checkedLength(decoder.readUInt(64), "const vector length");
    final byte[] out = new byte[length];
    for (int idx = 0; idx < out.length; idx++) {
      final long elementLength = decoder.readLength(compact(decoder));
      if (elementLength != 1L) {
        throw new IllegalArgumentException("const u8 vector element length must be 1");
      }
      out[idx] = (byte) decoder.readByte();
    }
    return out;
  }

  private static Integer readOptionU32(final NoritoDecoder decoder) {
    final int tag = decoder.readByte();
    if (tag == 0) {
      return null;
    }
    if (tag == 1) {
      return readField(decoder, child -> (int) child.readUInt(32));
    }
    throw new IllegalArgumentException("invalid option tag: " + tag);
  }

  private static <T> List<T> readVec(
      final NoritoDecoder decoder, final FieldReader<T> readElement) {
    final int count = checkedLength(decoder.readUInt(64), "vector length");
    final List<T> values = new ArrayList<>(count);
    for (int idx = 0; idx < count; idx++) {
      values.add(readField(decoder, readElement));
    }
    return Collections.unmodifiableList(values);
  }

  private static byte[] readHash(final NoritoDecoder decoder, final String field) {
    final byte[] bytes = decoder.readBytes(32);
    requireHash(bytes, field);
    return bytes;
  }

  private static VerifyingKeyIdReference readVerifyingKeyId(final NoritoDecoder decoder) {
    return new VerifyingKeyIdReference(
        readField(decoder, OfflineNoteV2::readString),
        readField(decoder, OfflineNoteV2::readString));
  }

  private static ProofBox readProofBox(final NoritoDecoder decoder) {
    return new ProofBox(
        readField(decoder, OfflineNoteV2::readString),
        readField(decoder, OfflineNoteV2::readBytesVec));
  }

  private static String readChainId(final NoritoDecoder decoder) {
    return readField(decoder, OfflineNoteV2::readString);
  }

  private static CommitmentOriginV2 readCommitmentOrigin(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    if (tag == 0L) {
      return readField(
          decoder,
          payload ->
              new CommitmentOriginV2.IssuerLoad(
                  readField(payload, OfflineNoteV2::readString),
                  readField(payload, OfflineNoteV2::readString),
                  readField(payload, child -> child.readUInt(64))));
    }
    if (tag == 1L) {
      return readField(
          decoder,
          payload ->
              new CommitmentOriginV2.P2pOutput(
                  readField(payload, OfflineNoteV2::readString),
                  readField(payload, child -> (int) child.readUInt(32))));
    }
    throw new IllegalArgumentException("unsupported commitment origin tag: " + tag);
  }

  private static String readAccountId(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    if (tag == 0L) {
      return readField(
          decoder,
          payload -> {
            final PublicKeyCodec.PublicKeyPayload publicKey = readPublicKeyPayload(payload);
            final String algorithm = PublicKeyCodec.algorithmForCurveId(publicKey.curveId());
            if (algorithm == null) {
              throw new IllegalArgumentException(
                  "unsupported public key curve id: " + publicKey.curveId());
            }
            try {
              return AccountAddress.fromAccount(publicKey.keyBytes(), algorithm)
                  .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
            } catch (final AccountAddressException ex) {
              throw new IllegalArgumentException("invalid decoded account id", ex);
            }
          });
    }
    if (tag == 1L) {
      return readField(
          decoder,
          payload -> {
            try {
              return AccountAddress.fromMultisigPolicy(readMultisigPolicy(payload))
                  .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
            } catch (final AccountAddressException ex) {
              throw new IllegalArgumentException("invalid decoded multisig account id", ex);
            }
          });
    }
    throw new IllegalArgumentException("unsupported account controller tag: " + tag);
  }

  private static PublicKeyCodec.PublicKeyPayload readPublicKeyPayload(
      final NoritoDecoder decoder) {
    final PublicKeyCodec.PublicKeyPayload payload =
        PublicKeyCodec.decodeCompactPublicKeyPayload(readConstVec(decoder));
    if (payload == null) {
      throw new IllegalArgumentException("invalid public key payload");
    }
    return payload;
  }

  private static MultisigPolicyPayload readMultisigPolicy(final NoritoDecoder decoder) {
    final int version = readField(decoder, child -> (int) child.readUInt(8));
    final int threshold = readField(decoder, child -> (int) child.readUInt(16));
    final List<MultisigMemberPayload> members =
        readField(
            decoder,
            payload ->
                readVec(
                    payload,
                    member -> {
                      final PublicKeyCodec.PublicKeyPayload publicKey =
                          readField(member, OfflineNoteV2::readPublicKeyPayload);
                      final int weight = readField(member, child -> (int) child.readUInt(16));
                      return MultisigMemberPayload.of(publicKey.curveId(), weight, publicKey.keyBytes());
                    }));
    return MultisigPolicyPayload.of(version, threshold, members);
  }

  private static String readAssetId(final NoritoDecoder decoder) {
    final String accountId = readField(decoder, OfflineNoteV2::readAccountId);
    final byte[] definitionBytes = readField(decoder, OfflineNoteV2::readAssetDefinitionAddress);
    final String definitionId = AssetDefinitionIdEncoder.encodeFromBytes(definitionBytes);
    final Long dataspaceId = readField(decoder, OfflineNoteV2::readAssetBalanceScope);
    final String base = definitionId + "#" + accountId;
    return dataspaceId == null ? base : base + "#dataspace:" + dataspaceId;
  }

  private static byte[] readAssetDefinitionAddress(final NoritoDecoder decoder) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    while (decoder.remaining() > 0) {
      final long length = decoder.readLength(compact(decoder));
      if (length != 1L) {
        throw new IllegalArgumentException("asset definition byte field length must be 1");
      }
      out.write(decoder.readByte());
    }
    return out.toByteArray();
  }

  private static Long readAssetBalanceScope(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    if (tag == 0L) {
      return null;
    }
    if (tag == 1L) {
      return readField(decoder, child -> child.readUInt(64));
    }
    throw new IllegalArgumentException("unsupported asset balance scope tag: " + tag);
  }

  private static String readNumeric(final NoritoDecoder decoder) {
    final byte[] mantissaBytes =
        readField(
            decoder,
            payload -> {
              final int length = checkedLength(payload.readUInt(32), "numeric mantissa length");
              return payload.readBytes(length);
            });
    final int scale = readField(decoder, child -> (int) child.readUInt(32));
    return canonicalNumericString(bigIntegerFromLittleEndianTwosComplement(mantissaBytes), scale);
  }

  private static BigInteger bigIntegerFromLittleEndianTwosComplement(final byte[] bytes) {
    if (bytes.length == 0) {
      return BigInteger.ZERO;
    }
    final byte[] bigEndian = new byte[bytes.length];
    for (int idx = 0; idx < bytes.length; idx++) {
      bigEndian[idx] = bytes[bytes.length - 1 - idx];
    }
    return new BigInteger(bigEndian);
  }

  private static int checkedLength(final long value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must be non-negative");
    }
    if (value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " exceeds JVM array limit");
    }
    return (int) value;
  }

  private static void writeField(final NoritoEncoder parent, final FieldWriter writePayload) {
    final NoritoEncoder child = parent.childEncoder();
    writePayload.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, compact(parent));
    parent.writeBytes(payload);
  }

  private static <T> void writeVec(
      final NoritoEncoder encoder, final List<T> values, final ElementWriter<T> writeElement) {
    encoder.writeUInt(values.size(), 64);
    for (final T value : values) {
      writeField(encoder, child -> writeElement.write(child, value));
    }
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
    encoder.writeUInt(value.length, 64);
    for (final byte b : value) {
      encoder.writeLength(1, compact(encoder));
      encoder.writeByte(b);
    }
  }

  private static void writeOptionU32(final NoritoEncoder encoder, final Integer value) {
    if (value == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    writeField(encoder, child -> child.writeUInt(value, 32));
  }

  private static void writeVerifyingKeyId(
      final NoritoEncoder encoder, final VerifyingKeyIdReference value) {
    writeField(encoder, child -> writeString(child, value.backend()));
    writeField(encoder, child -> writeString(child, value.name()));
  }

  private static void writeProofBox(final NoritoEncoder encoder, final ProofBox value) {
    writeField(encoder, child -> writeString(child, value.backend()));
    writeField(encoder, child -> writeBytesVec(child, value.bytes()));
  }

  private static void writeCommitmentOrigin(
      final NoritoEncoder encoder, final CommitmentOriginV2 origin) {
    if (origin instanceof CommitmentOriginV2.IssuerLoad) {
      final CommitmentOriginV2.IssuerLoad issuerLoad = (CommitmentOriginV2.IssuerLoad) origin;
      encoder.writeUInt(0, 32);
      writeField(
          encoder,
          payload -> {
            writeField(payload, child -> writeString(child, issuerLoad.operationId()));
            writeField(payload, child -> writeString(child, issuerLoad.lineageId()));
            writeField(payload, child -> child.writeUInt(issuerLoad.localRevision(), 64));
          });
      return;
    }
    if (origin instanceof CommitmentOriginV2.P2pOutput) {
      final CommitmentOriginV2.P2pOutput output = (CommitmentOriginV2.P2pOutput) origin;
      encoder.writeUInt(1, 32);
      writeField(
          encoder,
          payload -> {
            writeField(payload, child -> writeString(child, output.paymentRequestId()));
            writeField(payload, child -> child.writeUInt(output.outputIndex(), 32));
          });
      return;
    }
    throw new IllegalArgumentException("unsupported commitment origin");
  }

  private static void writeAccountId(final NoritoEncoder encoder, final String accountId) {
    encoder.writeBytes(encodeAccountIdPayload(accountId));
  }

  private static void writeChainId(final NoritoEncoder encoder, final String chainId) {
    writeField(encoder, child -> writeString(child, chainId));
  }

  private static byte[] encodeAccountIdPayload(final String accountId) {
    final AccountAddress address;
    try {
      address = AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).address;
    } catch (final AccountAddressException ex) {
      throw new IllegalArgumentException("account id must use canonical I105 form", ex);
    }
    try {
      final Optional<SingleKeyPayload> single = address.singleKeyPayloadIgnoringCurveSupport();
      if (single.isPresent()) {
        final SingleKeyPayload payload = single.get();
        final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
        encoder.writeUInt(0, 32);
        writeField(encoder, child -> writePublicKey(child, payload.curveId(), payload.publicKey()));
        return encoder.toByteArray();
      }
      final Optional<MultisigPolicyPayload> multisig =
          address.multisigPolicyPayloadIgnoringCurveSupport();
      if (!multisig.isPresent()) {
        throw new IllegalArgumentException("account id has no supported controller");
      }
      final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
      encoder.writeUInt(1, 32);
      writeField(encoder, child -> writeMultisigPolicy(child, multisig.get()));
      return encoder.toByteArray();
    } catch (final AccountAddressException ex) {
      throw new IllegalArgumentException("account id must use a supported controller", ex);
    }
  }

  private static void writeMultisigPolicy(
      final NoritoEncoder encoder, final MultisigPolicyPayload policy) {
    if (policy.version() != MULTISIG_POLICY_VERSION_V1) {
      throw new IllegalArgumentException("unsupported multisig policy version");
    }
    if (policy.threshold() <= 0) {
      throw new IllegalArgumentException("multisig threshold must be positive");
    }
    if (policy.members().isEmpty()) {
      throw new IllegalArgumentException("multisig policy must have members");
    }
    writeField(encoder, child -> child.writeUInt(policy.version(), 8));
    writeField(encoder, child -> child.writeUInt(policy.threshold(), 16));
    writeField(encoder, child -> writeMultisigMembers(child, policy.members()));
  }

  private static void writeMultisigMembers(
      final NoritoEncoder encoder, final List<MultisigMemberPayload> members) {
    final List<MultisigMemberPayload> sorted = new ArrayList<>(members);
    sorted.sort(Comparator.comparing(OfflineNoteV2::canonicalSortKey, OfflineNoteV2::compareUnsigned));
    for (int i = 1; i < sorted.size(); i++) {
      if (Arrays.equals(canonicalSortKey(sorted.get(i - 1)), canonicalSortKey(sorted.get(i)))) {
        throw new IllegalArgumentException("duplicate multisig member");
      }
    }
    encoder.writeUInt(sorted.size(), 64);
    for (final MultisigMemberPayload member : sorted) {
      writeField(
          encoder,
          memberEncoder -> {
            writeField(memberEncoder, child -> writePublicKey(child, member.curveId(), member.publicKey()));
            writeField(memberEncoder, child -> child.writeUInt(member.weight(), 16));
          });
    }
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
        throw new IllegalArgumentException("Unsupported curve id: " + curveId);
    }
    final byte[] bytes = new byte[1 + publicKey.length];
    bytes[0] = (byte) tag;
    System.arraycopy(publicKey, 0, bytes, 1, publicKey.length);
    return bytes;
  }

  private static void writeAssetId(final NoritoEncoder encoder, final String assetId) {
    final ParsedAssetId parsed = parseAssetId(assetId);
    writeField(encoder, child -> writeAccountId(child, parsed.accountId));
    writeField(encoder, child -> writeAssetDefinitionAddress(child, parsed.definitionBytes));
    writeField(encoder, child -> writeAssetBalanceScope(child, parsed.dataspaceId));
  }

  private static void writeAssetDefinitionAddress(
      final NoritoEncoder encoder, final byte[] bytes) {
    for (final byte b : bytes) {
      encoder.writeLength(1, compact(encoder));
      encoder.writeByte(b);
    }
  }

  private static void writeAssetBalanceScope(final NoritoEncoder encoder, final Long dataspaceId) {
    if (dataspaceId == null) {
      encoder.writeUInt(0, 32);
      return;
    }
    encoder.writeUInt(1, 32);
    writeField(encoder, child -> child.writeUInt(dataspaceId, 64));
  }

  private static void writeNumeric(final NoritoEncoder encoder, final String value) {
    final NumericValue numeric = parseNumeric(value);
    writeField(
        encoder,
        child -> {
          child.writeUInt(numeric.mantissaBytes.length, 32);
          child.writeBytes(numeric.mantissaBytes);
        });
    writeField(encoder, child -> child.writeUInt(numeric.scale, 32));
  }

  private static ParsedAssetId parseAssetId(final String value) {
    final String[] parts = Objects.requireNonNull(value, "assetId").split("#", -1);
    if (parts.length != 2 && parts.length != 3) {
      throw new IllegalArgumentException(
          "asset id must be '<asset-definition>#<account>' with optional '#dataspace:<id>'");
    }
    final byte[] definitionBytes = AssetDefinitionIdEncoder.parseAddressBytes(parts[0]);
    encodeAccountIdPayload(parts[1]);
    final Long dataspaceId;
    if (parts.length == 3) {
      if (!parts[2].startsWith("dataspace:")) {
        throw new IllegalArgumentException("asset scope must use dataspace:<id>");
      }
      dataspaceId = Long.parseLong(parts[2].substring("dataspace:".length()));
    } else {
      dataspaceId = null;
    }
    return new ParsedAssetId(parts[1], definitionBytes, dataspaceId);
  }

  private static NumericValue parseNumeric(final String value) {
    final BigDecimal decimal = new BigDecimal(Objects.requireNonNull(value, "amount"));
    final int scale = Math.max(decimal.scale(), 0);
    if (scale > MAX_NUMERIC_SCALE) {
      throw new IllegalArgumentException("numeric scale exceeds " + MAX_NUMERIC_SCALE);
    }
    final BigInteger mantissa = decimal.movePointRight(scale).toBigIntegerExact();
    final byte[] mantissaBytes = toTwosComplementLittleEndian(mantissa);
    if (mantissaBytes.length > MAX_BIGINT_BYTES) {
      throw new IllegalArgumentException(
          "numeric mantissa exceeds " + MAX_BIGINT_BYTES + " bytes");
    }
    return new NumericValue(mantissaBytes, scale, canonicalNumericString(mantissa, scale));
  }

  private static String canonicalNumericString(final BigInteger mantissa, final int scale) {
    final boolean negative = mantissa.signum() < 0;
    String digits = mantissa.abs().toString();
    while (digits.length() > 1 && digits.charAt(0) == '0') {
      digits = digits.substring(1);
    }
    if (scale == 0) {
      return negative && !"0".equals(digits) ? "-" + digits : digits;
    }
    while (digits.length() <= scale) {
      digits = "0" + digits;
    }
    final int splitAt = digits.length() - scale;
    final String body = digits.substring(0, splitAt) + "." + digits.substring(splitAt);
    return negative && mantissa.signum() != 0 ? "-" + body : body;
  }

  private static byte[] toTwosComplementLittleEndian(final BigInteger value) {
    if (value.signum() == 0) {
      return new byte[0];
    }
    final byte[] be = value.toByteArray();
    final byte[] le = new byte[be.length];
    for (int i = 0; i < be.length; i++) {
      le[i] = be[be.length - 1 - i];
    }
    int len = le.length;
    if (value.signum() > 0) {
      while (len > 1 && le[len - 1] == 0 && (le[len - 2] & 0x80) == 0) {
        len--;
      }
    } else {
      while (len > 1 && le[len - 1] == (byte) 0xFF && (le[len - 2] & 0x80) != 0) {
        len--;
      }
    }
    return len == le.length ? le : Arrays.copyOf(le, len);
  }

  private static long validateCount(final int count, final int max, final String label) {
    if (count < 1 || count > max) {
      throw new IllegalArgumentException(
          "Offline V2 " + label + " count " + count + " must be in 1.." + max);
    }
    return count;
  }

  private static long[] publicValues(
      final byte[] publicInputsHash,
      final long mode,
      final long inputCount,
      final long outputCount,
      final long inputSum,
      final long outputSum,
      final long inputNullifierSum,
      final long outputCommitmentSum,
      final byte[] keyCertificatePayloadHash,
      final byte[] sourceOrToken,
      final long inputClaimHashSum,
      final long outputClaimHashSum) {
    final long[] limbs = hashLimbsLE(publicInputsHash);
    return new long[] {
      limbs[0],
      limbs[1],
      limbs[2],
      limbs[3],
      mode,
      inputCount,
      outputCount,
      inputSum,
      outputSum,
      inputNullifierSum,
      outputCommitmentSum,
      hashLimb0(keyCertificatePayloadHash),
      hashLimb0(sourceOrToken),
      inputClaimHashSum,
      outputClaimHashSum,
      0L,
    };
  }

  private static List<Long> normalizedAmountUnits(final List<String> amounts) {
    final List<TrimmedNumeric> trimmed = new ArrayList<>();
    int targetScale = 0;
    for (final String amount : amounts) {
      final TrimmedNumeric numeric = trimmedNumeric(amount);
      trimmed.add(numeric);
      targetScale = Math.max(targetScale, numeric.scale);
    }

    final List<Long> result = new ArrayList<>();
    for (final TrimmedNumeric numeric : trimmed) {
      if (numeric.mantissa.signum() < 0) {
        throw new IllegalArgumentException(
            "Offline V2 amount " + numeric.original + " must not be negative");
      }
      final int scaleDelta = targetScale - numeric.scale;
      final BigInteger aligned = numeric.mantissa.multiply(BigInteger.TEN.pow(scaleDelta));
      if (aligned.bitLength() > 64) {
        throw new IllegalArgumentException(
            "Offline V2 amount "
                + numeric.original
                + " does not fit the u64 witness corridor");
      }
      result.add(aligned.longValue());
    }
    return Collections.unmodifiableList(result);
  }

  private static TrimmedNumeric trimmedNumeric(final String amount) {
    final NumericValue numeric = parseNumeric(amount);
    final BigDecimal decimal = new BigDecimal(numeric.canonicalString).stripTrailingZeros();
    final int scale = Math.max(decimal.scale(), 0);
    final BigInteger mantissa = decimal.movePointRight(scale).toBigIntegerExact();
    return new TrimmedNumeric(amount, mantissa, scale);
  }

  private static long checkedSum(final List<Long> values, final String label) {
    BigInteger sum = BigInteger.ZERO;
    for (final long value : values) {
      sum = sum.add(unsignedLongToBigInteger(value));
      if (sum.compareTo(MAX_U64) > 0) {
        throw new IllegalArgumentException(
            "Offline V2 " + label + " amount sum overflows u64 witness units");
      }
    }
    return sum.longValue();
  }

  private static BigInteger unsignedLongToBigInteger(final long value) {
    final byte[] bytes = new byte[9];
    for (int idx = 0; idx < 8; idx++) {
      bytes[8 - idx] = (byte) (value >>> (idx * 8));
    }
    return new BigInteger(bytes);
  }

  private static long hashLimb0Sum(final List<byte[]> hashes) {
    long sum = 0L;
    for (final byte[] hash : hashes) {
      sum += hashLimb0(hash);
    }
    return sum;
  }

  private static long hashLimb0(final byte[] hash) {
    return hashLimbsLE(hash)[0];
  }

  private static long[] hashLimbsLE(final byte[] hash) {
    if (hash.length != 32) {
      throw new IllegalArgumentException("hash must be 32 bytes");
    }
    final long[] limbs = new long[4];
    for (int idx = 0; idx < 4; idx++) {
      final int start = idx * 8;
      long value = 0L;
      for (int offset = 0; offset < 8; offset++) {
        value |= (hash[start + offset] & 0xFFL) << (offset * 8);
      }
      limbs[idx] = value;
    }
    return limbs;
  }

  private static void requireCertificateCore(
      final int version, final String accountId, final byte[] publicKey, final boolean oneUse) {
    if (version != 2) {
      throw new IllegalArgumentException("Offline Note V2 key certificate version must be 2");
    }
    if (!oneUse) {
      throw new IllegalArgumentException("Offline Note V2 key certificate must be one-use");
    }
    if (publicKey.length != 32) {
      throw new IllegalArgumentException("Offline Note V2 note public key must be 32 bytes");
    }
    encodeAccountIdPayload(accountId);
  }

  private static void requireHash(final byte[] value, final String field) {
    if (value.length != 32) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    if ((value[value.length - 1] & 1) != 1) {
      throw new IllegalArgumentException(field + " must carry the Iroha prehash marker");
    }
  }

  private static void requireHashes(final List<byte[]> values, final String field) {
    if (values.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    for (int i = 0; i < values.size(); i++) {
      requireHash(values.get(i), field + "[" + i + "]");
    }
  }

  private static void requireRandomBytes(final byte[] value, final String field) {
    if (value.length != 32) {
      throw new IllegalArgumentException(field + " must be exactly 32 bytes");
    }
  }

  private static byte[] canonicalSortKey(final MultisigMemberPayload member) {
    final String algorithm = PublicKeyCodec.algorithmForCurveId(member.curveId());
    if (algorithm == null) {
      throw new IllegalArgumentException("unknown multisig curve id");
    }
    final byte[] algorithmBytes = algorithm.getBytes(StandardCharsets.UTF_8);
    final byte[] keyBytes = member.publicKey();
    final byte[] sortKey = new byte[algorithmBytes.length + 1 + keyBytes.length];
    System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.length);
    sortKey[algorithmBytes.length] = 0;
    System.arraycopy(keyBytes, 0, sortKey, algorithmBytes.length + 1, keyBytes.length);
    return sortKey;
  }

  private static int compareUnsigned(final byte[] a, final byte[] b) {
    final int len = Math.min(a.length, b.length);
    for (int i = 0; i < len; i++) {
      final int cmp = (a[i] & 0xFF) - (b[i] & 0xFF);
      if (cmp != 0) {
        return cmp;
      }
    }
    return Integer.compare(a.length, b.length);
  }

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean compact(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xFF));
    }
    return builder.toString();
  }

  private static String requireNonBlank(final String value, final String field) {
    final String checked = Objects.requireNonNull(value, field);
    if (checked.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return checked;
  }

  private static byte[] copy(final byte[] value, final String field) {
    return Arrays.copyOf(Objects.requireNonNull(value, field), value.length);
  }

  private static List<byte[]> copyByteList(final List<byte[]> values, final String field) {
    final List<byte[]> result = new ArrayList<>();
    for (final byte[] value : Objects.requireNonNull(values, field)) {
      result.add(copy(value, field));
    }
    return Collections.unmodifiableList(result);
  }

  private static final class ParsedAssetId {
    private final String accountId;
    private final byte[] definitionBytes;
    private final Long dataspaceId;

    private ParsedAssetId(
        final String accountId, final byte[] definitionBytes, final Long dataspaceId) {
      this.accountId = accountId;
      this.definitionBytes = Arrays.copyOf(definitionBytes, definitionBytes.length);
      this.dataspaceId = dataspaceId;
    }
  }

  private static final class NumericValue {
    private final byte[] mantissaBytes;
    private final int scale;
    private final String canonicalString;

    private NumericValue(
        final byte[] mantissaBytes, final int scale, final String canonicalString) {
      this.mantissaBytes = Arrays.copyOf(mantissaBytes, mantissaBytes.length);
      this.scale = scale;
      this.canonicalString = canonicalString;
    }
  }

  private static final class TrimmedNumeric {
    private final String original;
    private final BigInteger mantissa;
    private final int scale;

    private TrimmedNumeric(final String original, final BigInteger mantissa, final int scale) {
      this.original = original;
      this.mantissa = mantissa;
      this.scale = scale;
    }
  }

  static String base64(final byte[] bytes) {
    return Base64.getEncoder().encodeToString(bytes);
  }
}
