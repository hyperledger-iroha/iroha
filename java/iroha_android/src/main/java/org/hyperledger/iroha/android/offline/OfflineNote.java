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

/** Native Java implementation of Iroha Offline Note canonical Norito encodings. */
public final class OfflineNote {
  public static final String KEY_CERTIFICATE_PAYLOAD_DOMAIN =
      "iroha:offline-note:key-certificate-payload";
  public static final String ISSUED_CLAIM_DOMAIN = "iroha:offline-note:issued-claim";
  public static final String REDEEM_PUBLIC_INPUTS_DOMAIN =
      "iroha:offline-note:redeem-public-inputs";
  public static final String AUDIT_PUBLIC_INPUTS_DOMAIN =
      "iroha:offline-note:audit-public-inputs";
  public static final String NOTE_COMMITMENT_DOMAIN =
      "iroha:offline-note:note-commitment";
  public static final String INPUT_NULLIFIER_DOMAIN =
      "iroha:offline-note:input-nullifier";
  public static final String PAYMENT_TOKEN_ID_DOMAIN =
      "iroha:offline-note:payment-token-id";
  public static final String RECURSIVE_BACKEND = "halo2/ipa";
  public static final String RECURSIVE_VERIFIER_NAME = "offline-note-recursive";
  public static final String RECURSIVE_PUBLIC_INPUTS_SCHEMA =
      "{\"schema\":\"offline_note_recursive\",\"public_inputs\":[\"public_inputs_hash_limb0\",\"public_inputs_hash_limb1\",\"public_inputs_hash_limb2\",\"public_inputs_hash_limb3\",\"proof_mode\",\"input_count\",\"output_count\",\"input_amount_sum\",\"output_amount_sum\",\"input_nullifier_sum_limb0\",\"output_commitment_sum_limb0\",\"key_certificate_payload_hash_limb0\",\"source_or_token_limb0\",\"input_claim_hash_sum_limb0\",\"output_claim_hash_sum_limb0\",\"reserved_zero\"]}";
  public static final int KEY_CERTIFICATE_VERSION = 1;

  private static final int MULTISIG_POLICY_VERSION = 1;
  private static final int MAX_NUMERIC_SCALE = 28;
  private static final int MAX_BIGINT_BYTES = 64;
  private static final int PUBLIC_VALUE_COUNT = 16;
  private static final int MAX_INPUT_AMOUNTS = 4;
  private static final int MAX_OUTPUT_AMOUNTS = 2;
  private static final long MODE_REDEEM = 1L;
  private static final long MODE_AUDIT = 2L;
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private static final String KEY_CERTIFICATE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteKeyCertificate";
  private static final String KEY_CERTIFICATE_PAYLOAD_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayload";
  private static final String ISSUE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteIssue";
  private static final String ISSUED_CLAIM_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteIssuedClaim";
  private static final String RECURSIVE_PROOF_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteRecursiveProof";
  private static final String REDEEM_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteRedeem";
  private static final String REDEEM_PUBLIC_INPUTS_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputs";
  private static final String AUDIT_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteAuditBundle";
  private static final String AUDIT_PUBLIC_INPUTS_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteAuditPublicInputs";
  private static final String NOTE_COMMITMENT_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteCommitmentPreimage";
  private static final String INPUT_NULLIFIER_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNoteInputNullifierPreimage";
  private static final String PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA =
      "iroha_data_model::offline::model::OfflineNotePaymentTokenIdPreimage";
  public static final String ISSUE_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::IssueOfflineNote";
  public static final String REDEEM_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::RedeemOfflineNote";
  public static final String AUDIT_INSTRUCTION_SCHEMA =
      "iroha_data_model::isi::offline::AuditOfflineNote";

  private OfflineNote() {}

  public static byte[] encodeCertificatePayload(final KeyCertificatePayload value) {
    return encodeWithHeader(value, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KEY_CERTIFICATE_PAYLOAD_ADAPTER);
  }

  public static byte[] encodeCertificate(final KeyCertificate value) {
    return encodeWithHeader(value, KEY_CERTIFICATE_SCHEMA, KEY_CERTIFICATE_ADAPTER);
  }

  public static byte[] encodeIssue(final Issue value) {
    return encodeWithHeader(value, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static byte[] encodeIssuedClaim(final IssuedClaim value) {
    return encodeWithHeader(value, ISSUED_CLAIM_SCHEMA, ISSUED_CLAIM_ADAPTER);
  }

  public static byte[] encodeRedeem(final Redeem value) {
    return encodeWithHeader(value, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static byte[] encodeRedeemPublicInputs(final RedeemPublicInputs value) {
    return encodeWithHeader(value, REDEEM_PUBLIC_INPUTS_SCHEMA, REDEEM_PUBLIC_INPUTS_ADAPTER);
  }

  public static byte[] encodeAudit(final AuditBundle value) {
    return encodeWithHeader(value, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static byte[] encodeAuditPublicInputs(final AuditPublicInputs value) {
    return encodeWithHeader(value, AUDIT_PUBLIC_INPUTS_SCHEMA, AUDIT_PUBLIC_INPUTS_ADAPTER);
  }

  public static byte[] encodeNoteCommitmentPreimage(final NoteCommitmentPreimage value) {
    return encodeWithHeader(
        value, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NOTE_COMMITMENT_PREIMAGE_ADAPTER);
  }

  public static byte[] encodeInputNullifierPreimage(final InputNullifierPreimage value) {
    return encodeWithHeader(
        value, INPUT_NULLIFIER_PREIMAGE_SCHEMA, INPUT_NULLIFIER_PREIMAGE_ADAPTER);
  }

  public static byte[] encodePaymentTokenIdPreimage(final PaymentTokenIdPreimage value) {
    return encodeWithHeader(
        value, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER);
  }

  public static KeyCertificatePayload decodeCertificatePayload(final byte[] bytes) {
    return decodeWithHeader(bytes, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KEY_CERTIFICATE_PAYLOAD_ADAPTER);
  }

  public static KeyCertificate decodeCertificate(final byte[] bytes) {
    return decodeWithHeader(bytes, KEY_CERTIFICATE_SCHEMA, KEY_CERTIFICATE_ADAPTER);
  }

  public static Issue decodeIssue(final byte[] bytes) {
    return decodeWithHeader(bytes, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static IssuedClaim decodeIssuedClaim(final byte[] bytes) {
    return decodeWithHeader(bytes, ISSUED_CLAIM_SCHEMA, ISSUED_CLAIM_ADAPTER);
  }

  public static RecursiveProof decodeRecursiveProof(final byte[] bytes) {
    return decodeWithHeader(bytes, RECURSIVE_PROOF_SCHEMA, RECURSIVE_PROOF_ADAPTER);
  }

  public static Redeem decodeRedeem(final byte[] bytes) {
    return decodeWithHeader(bytes, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static RedeemPublicInputs decodeRedeemPublicInputs(final byte[] bytes) {
    return decodeWithHeader(bytes, REDEEM_PUBLIC_INPUTS_SCHEMA, REDEEM_PUBLIC_INPUTS_ADAPTER);
  }

  public static AuditBundle decodeAudit(final byte[] bytes) {
    return decodeWithHeader(bytes, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static AuditPublicInputs decodeAuditPublicInputs(final byte[] bytes) {
    return decodeWithHeader(bytes, AUDIT_PUBLIC_INPUTS_SCHEMA, AUDIT_PUBLIC_INPUTS_ADAPTER);
  }

  public static NoteCommitmentPreimage decodeNoteCommitmentPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NOTE_COMMITMENT_PREIMAGE_ADAPTER);
  }

  public static InputNullifierPreimage decodeInputNullifierPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, INPUT_NULLIFIER_PREIMAGE_SCHEMA, INPUT_NULLIFIER_PREIMAGE_ADAPTER);
  }

  public static PaymentTokenIdPreimage decodePaymentTokenIdPreimage(final byte[] bytes) {
    return decodeWithHeader(
        bytes, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER);
  }

  public static Issue decodeIssueInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, ISSUE_INSTRUCTION_SCHEMA, ISSUE_SCHEMA, ISSUE_ADAPTER);
  }

  public static Redeem decodeRedeemInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, REDEEM_INSTRUCTION_SCHEMA, REDEEM_SCHEMA, REDEEM_ADAPTER);
  }

  public static AuditBundle decodeAuditInstruction(final byte[] bytes) {
    return decodeInstructionModel(
        bytes, AUDIT_INSTRUCTION_SCHEMA, AUDIT_SCHEMA, AUDIT_ADAPTER);
  }

  public static InstructionBox issueInstruction(final Issue value) {
    return InstructionBox.fromWirePayload(
        ISSUE_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(ISSUE_INSTRUCTION_SCHEMA, value, ISSUE_ADAPTER));
  }

  public static InstructionBox redeemInstruction(final Redeem value) {
    value.validateProofBinding();
    return InstructionBox.fromWirePayload(
        REDEEM_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(REDEEM_INSTRUCTION_SCHEMA, value, REDEEM_ADAPTER));
  }

  public static InstructionBox auditInstruction(final AuditBundle value) {
    value.validateProofBinding();
    return InstructionBox.fromWirePayload(
        AUDIT_INSTRUCTION_SCHEMA,
        encodeInstructionWrapper(AUDIT_INSTRUCTION_SCHEMA, value, AUDIT_ADAPTER));
  }

  public static byte[] deriveNoteCommitment(final NoteCommitmentPreimage value) {
    return hash(encodeNoteCommitmentPreimage(value));
  }

  public static byte[] deriveInputNullifier(final InputNullifierPreimage value) {
    return hash(encodeInputNullifierPreimage(value));
  }

  public static byte[] derivePaymentTokenId(final PaymentTokenIdPreimage value) {
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

  private static <T> byte[] encodeInstructionWrapper(
      final String schema, final T value, final TypeAdapter<T> adapter) {
    final NoritoCodec.AdaptiveEncoding modelPayload =
        NoritoCodec.encodeAdaptive(value, adapter, NoritoHeader.COMPACT_LEN);
    return NoritoCodec.encode(
        new InstructionModelPayload(modelPayload.payload(), modelPayload.flags()),
        schema,
        INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER,
        modelPayload.flags());
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
    throw new IllegalArgumentException("Offline Note instruction envelope is invalid");
  }

  private static byte[] tryDecodeInstructionPair(
      final byte[] bytes, final String expectedWireName, final int flags) {
    try {
      final NoritoDecoder decoder = new NoritoDecoder(bytes, flags);
      final String wireName = readField(decoder, OfflineNote::readString);
      if (!expectedWireName.equals(wireName)) {
        throw new IllegalArgumentException(
            "Offline Note instruction wire name mismatch: " + wireName);
      }
      final byte[] wirePayload = readField(decoder, OfflineNote::readBytesVec);
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
          throw new IllegalArgumentException("Trailing bytes after Offline Note model decode");
        }
        return value;
      } catch (final RuntimeException ex) {
        lastError = ex;
      }
    }
    throw new IllegalArgumentException(
        "Offline Note instruction model payload is invalid", lastError);
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
      this.backend = requireNoColon(requireNonBlankUnpadded(backend, "verifying key backend"),
          "verifying key backend");
      this.name = requireNoColon(requireNonBlankUnpadded(name, "verifying key name"),
          "verifying key name");
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
      this.backend = requireNonBlankUnpadded(backend, "proof backend");
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

  public static final class RecursiveProof {
    private final VerifyingKeyIdReference verifierKeyId;
    private final byte[] publicInputsHash;
    private final ProofBox proof;

    public RecursiveProof(final byte[] publicInputsHash, final ProofBox proof) {
      this(new VerifyingKeyIdReference(), publicInputsHash, proof);
    }

    public RecursiveProof(
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

    public void validateCanonicalMetadata() {
      if (!RECURSIVE_BACKEND.equals(verifierKeyId.backend())
          || !RECURSIVE_VERIFIER_NAME.equals(verifierKeyId.name())) {
        throw new IllegalArgumentException(
            "recursive proof verifier key must be "
                + RECURSIVE_BACKEND
                + ":"
                + RECURSIVE_VERIFIER_NAME);
      }
      if (!RECURSIVE_BACKEND.equals(proof.backend())) {
        throw new IllegalArgumentException(
            "recursive proof backend must be " + RECURSIVE_BACKEND);
      }
    }
  }

  public static final class KeyCertificatePayload {
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

    public KeyCertificatePayload(
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

    public KeyCertificatePayload(
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
      this.domain = requireDomain(domain, KEY_CERTIFICATE_PAYLOAD_DOMAIN, "domain");
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
      requireCertificateCore(version, accountId, this.publicKey, assertionUsageCountLimit, oneUse);
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

  public static final class KeyCertificate {
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

    public KeyCertificate(
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
      requireCertificateCore(version, accountId, this.publicKey, assertionUsageCountLimit, oneUse);
      if (this.issuerSignature.length != 64) {
        throw new IllegalArgumentException("issuer signature must be 64 bytes");
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

    public KeyCertificatePayload signingPayload() {
      return new KeyCertificatePayload(
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

  public abstract static class CommitmentOrigin {
    private CommitmentOrigin() {}

    public static final class IssuerLoad extends CommitmentOrigin {
      private final String operationId;
      private final String lineageId;
      private final long localRevision;

      public IssuerLoad(
          final String operationId, final String lineageId, final long localRevision) {
        this.operationId = requireNonBlankUnpadded(operationId, "operation_id");
        this.lineageId = requireNonBlankUnpadded(lineageId, "lineage_id");
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

    public static final class P2pOutput extends CommitmentOrigin {
      private final String paymentRequestId;
      private final int outputIndex;

      public P2pOutput(final String paymentRequestId, final int outputIndex) {
        this.paymentRequestId = requireNonBlankUnpadded(paymentRequestId, "payment_request_id");
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

  public static final class NoteCommitmentPreimage {
    private final String domain;
    private final String chainId;
    private final byte[] ownerKeyCertificatePayloadHash;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;
    private final byte[] noteSecret;
    private final CommitmentOrigin origin;

    public NoteCommitmentPreimage(
        final String chainId,
        final byte[] ownerKeyCertificatePayloadHash,
        final String assetId,
        final String amount,
        final byte[] noteSecret,
        final CommitmentOrigin origin) {
      this(
          NOTE_COMMITMENT_DOMAIN,
          chainId,
          ownerKeyCertificatePayloadHash,
          assetId,
          amount,
          noteSecret,
          origin);
    }

    public NoteCommitmentPreimage(
        final String domain,
        final String chainId,
        final byte[] ownerKeyCertificatePayloadHash,
        final String assetId,
        final String amount,
        final byte[] noteSecret,
        final CommitmentOrigin origin) {
      if (!NOTE_COMMITMENT_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported note commitment domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlankUnpadded(chainId, "chain_id");
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

    public CommitmentOrigin origin() {
      return origin;
    }

    public byte[] noritoEncoded() {
      return encodeNoteCommitmentPreimage(this);
    }

    public byte[] deriveNoteCommitment() {
      return OfflineNote.deriveNoteCommitment(this);
    }
  }

  public static final class InputNullifierPreimage {
    private final String domain;
    private final String chainId;
    private final byte[] sourceNoteCommitment;
    private final byte[] ownerKeyCertificatePayloadHash;
    private final byte[] noteSecret;

    public InputNullifierPreimage(
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

    public InputNullifierPreimage(
        final String domain,
        final String chainId,
        final byte[] sourceNoteCommitment,
        final byte[] ownerKeyCertificatePayloadHash,
        final byte[] noteSecret) {
      if (!INPUT_NULLIFIER_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported input nullifier domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlankUnpadded(chainId, "chain_id");
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
      return OfflineNote.deriveInputNullifier(this);
    }
  }

  public static final class PaymentTokenIdPreimage {
    private final String domain;
    private final String chainId;
    private final String paymentRequestId;
    private final long createdAtMs;
    private final byte[] tokenNonce;
    private final byte[] senderKeyCertificatePayloadHash;
    private final List<byte[]> inputNullifiers;
    private final List<byte[]> outputCommitments;

    public PaymentTokenIdPreimage(
        final String chainId,
        final String paymentRequestId,
        final long createdAtMs,
        final byte[] tokenNonce,
        final byte[] senderKeyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<byte[]> outputCommitments) {
      this(
          PAYMENT_TOKEN_ID_DOMAIN,
          chainId,
          paymentRequestId,
          createdAtMs,
          tokenNonce,
          senderKeyCertificatePayloadHash,
          inputNullifiers,
          outputCommitments);
    }

    public PaymentTokenIdPreimage(
        final String domain,
        final String chainId,
        final String paymentRequestId,
        final long createdAtMs,
        final byte[] tokenNonce,
        final byte[] senderKeyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<byte[]> outputCommitments) {
      if (!PAYMENT_TOKEN_ID_DOMAIN.equals(domain)) {
        throw new IllegalArgumentException("unsupported payment token id domain");
      }
      this.domain = domain;
      this.chainId = requireNonBlankUnpadded(chainId, "chain_id");
      this.paymentRequestId = requireNonBlankUnpadded(paymentRequestId, "payment_request_id");
      this.createdAtMs = createdAtMs;
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

    public String paymentRequestId() {
      return paymentRequestId;
    }

    public long createdAtMs() {
      return createdAtMs;
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
      return OfflineNote.derivePaymentTokenId(this);
    }
  }

  public static final class Issue {
    private final byte[] noteCommitment;
    private final KeyCertificate keyCertificate;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public Issue(
        final byte[] noteCommitment,
        final KeyCertificate keyCertificate,
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

    public KeyCertificate keyCertificate() {
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

    public IssuedClaim issuedClaim() {
      return new IssuedClaim(
          noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
    }

    public byte[] noritoEncoded() {
      return encodeIssue(this);
    }
  }

  public static final class IssuedClaim {
    private final String domain;
    private final byte[] noteCommitment;
    private final byte[] keyCertificatePayloadHash;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public IssuedClaim(
        final byte[] noteCommitment,
        final byte[] keyCertificatePayloadHash,
        final String assetId,
        final String amount) {
      this(ISSUED_CLAIM_DOMAIN, noteCommitment, keyCertificatePayloadHash, assetId, amount);
    }

    public IssuedClaim(
        final String domain,
        final byte[] noteCommitment,
        final byte[] keyCertificatePayloadHash,
        final String assetId,
        final String amount) {
      this.domain = requireDomain(domain, ISSUED_CLAIM_DOMAIN, "domain");
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

  public static final class AuditOutputClaim {
    private final byte[] noteCommitment;
    private final KeyCertificate keyCertificate;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public AuditOutputClaim(
        final byte[] noteCommitment,
        final KeyCertificate keyCertificate,
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

    public KeyCertificate keyCertificate() {
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

    public IssuedClaim issuedClaim() {
      return new IssuedClaim(
          noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
    }
  }

  public static final class RedeemPublicInputs {
    private final String domain;
    private final byte[] sourceNoteCommitment;
    private final List<byte[]> inputNullifiers;
    private final byte[] keyCertificatePayloadHash;
    private final String recipient;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;

    public RedeemPublicInputs(
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

    public RedeemPublicInputs(
        final String domain,
        final byte[] sourceNoteCommitment,
        final List<byte[]> inputNullifiers,
        final byte[] keyCertificatePayloadHash,
        final String recipient,
        final String assetId,
        final String amount) {
      this.domain = requireDomain(domain, REDEEM_PUBLIC_INPUTS_DOMAIN, "domain");
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

  public static final class Redeem {
    private final byte[] sourceNoteCommitment;
    private final List<byte[]> inputNullifiers;
    private final KeyCertificate senderKeyCertificate;
    private final String recipient;
    private final String assetId;
    private final String amount;
    private final String canonicalAmount;
    private final RecursiveProof recursiveProof;

    public Redeem(
        final byte[] sourceNoteCommitment,
        final List<byte[]> inputNullifiers,
        final KeyCertificate senderKeyCertificate,
        final String recipient,
        final String assetId,
        final String amount,
        final RecursiveProof recursiveProof) {
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

    public KeyCertificate senderKeyCertificate() {
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

    public RecursiveProof recursiveProof() {
      return recursiveProof;
    }

    public RedeemPublicInputs publicInputs() {
      return new RedeemPublicInputs(
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
      recursiveProof.validateCanonicalMetadata();
      if (!Arrays.equals(recursiveProof.publicInputsHash(), publicInputsHash())) {
        throw new IllegalArgumentException("recursive proof public inputs hash mismatch");
      }
    }

    public Redeem replacingRecursiveProof(final RecursiveProof recursiveProof) {
      return new Redeem(
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

  public static final class AuditPublicInputs {
    private final String domain;
    private final byte[] tokenId;
    private final byte[] keyCertificatePayloadHash;
    private final List<byte[]> inputNullifiers;
    private final List<IssuedClaim> inputClaims;
    private final List<byte[]> outputCommitments;
    private final List<IssuedClaim> outputClaims;

    public AuditPublicInputs(
        final byte[] tokenId,
        final byte[] keyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaim> inputClaims,
        final List<byte[]> outputCommitments,
        final List<IssuedClaim> outputClaims) {
      this(
          AUDIT_PUBLIC_INPUTS_DOMAIN,
          tokenId,
          keyCertificatePayloadHash,
          inputNullifiers,
          inputClaims,
          outputCommitments,
          outputClaims);
    }

    public AuditPublicInputs(
        final String domain,
        final byte[] tokenId,
        final byte[] keyCertificatePayloadHash,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaim> inputClaims,
        final List<byte[]> outputCommitments,
        final List<IssuedClaim> outputClaims) {
      this.domain = requireDomain(domain, AUDIT_PUBLIC_INPUTS_DOMAIN, "domain");
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
      if (this.outputClaims.size() != this.outputCommitments.size()) {
        throw new IllegalArgumentException(
            "output claim count must match output commitment count");
      }
      for (int i = 0; i < this.outputCommitments.size(); i++) {
        if (!Arrays.equals(this.outputClaims.get(i).noteCommitment(), this.outputCommitments.get(i))) {
          throw new IllegalArgumentException(
              "audit output claims must be ordered one-to-one with output commitments");
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

    public List<IssuedClaim> inputClaims() {
      return inputClaims;
    }

    public List<byte[]> outputCommitments() {
      return copyByteList(outputCommitments, "outputCommitments");
    }

    public List<IssuedClaim> outputClaims() {
      return outputClaims;
    }

    public byte[] noritoEncoded() {
      return encodeAuditPublicInputs(this);
    }

    public byte[] publicInputsHash() {
      return hash(noritoEncoded());
    }
  }

  public static final class AuditBundle {
    private final byte[] tokenId;
    private final KeyCertificate senderKeyCertificate;
    private final List<byte[]> inputNullifiers;
    private final List<IssuedClaim> inputClaims;
    private final List<byte[]> outputCommitments;
    private final List<AuditOutputClaim> outputClaims;
    private final RecursiveProof recursiveProof;

    public AuditBundle(
        final byte[] tokenId,
        final KeyCertificate senderKeyCertificate,
        final List<byte[]> inputNullifiers,
        final List<IssuedClaim> inputClaims,
        final List<byte[]> outputCommitments,
        final List<AuditOutputClaim> outputClaims,
        final RecursiveProof recursiveProof) {
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
      if (this.outputClaims.size() != this.outputCommitments.size()) {
        throw new IllegalArgumentException(
            "output claim count must match output commitment count");
      }
      for (int i = 0; i < this.outputCommitments.size(); i++) {
        if (!Arrays.equals(this.outputClaims.get(i).noteCommitment(), this.outputCommitments.get(i))) {
          throw new IllegalArgumentException(
              "audit output claims must be ordered one-to-one with output commitments");
        }
      }
    }

    public byte[] tokenId() {
      return Arrays.copyOf(tokenId, tokenId.length);
    }

    public KeyCertificate senderKeyCertificate() {
      return senderKeyCertificate;
    }

    public List<byte[]> inputNullifiers() {
      return copyByteList(inputNullifiers, "inputNullifiers");
    }

    public List<IssuedClaim> inputClaims() {
      return inputClaims;
    }

    public List<byte[]> outputCommitments() {
      return copyByteList(outputCommitments, "outputCommitments");
    }

    public List<AuditOutputClaim> outputClaims() {
      return outputClaims;
    }

    public RecursiveProof recursiveProof() {
      return recursiveProof;
    }

    public AuditPublicInputs publicInputs() {
      final List<IssuedClaim> issuedOutputs = new ArrayList<>();
      for (final AuditOutputClaim claim : outputClaims) {
        issuedOutputs.add(claim.issuedClaim());
      }
      return new AuditPublicInputs(
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
      recursiveProof.validateCanonicalMetadata();
      if (!Arrays.equals(recursiveProof.publicInputsHash(), publicInputsHash())) {
        throw new IllegalArgumentException("recursive proof public inputs hash mismatch");
      }
    }

    public AuditBundle replacingRecursiveProof(final RecursiveProof recursiveProof) {
      return new AuditBundle(
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
            "Offline public instance count must be " + PUBLIC_VALUE_COUNT);
      }
      if (this.inputAmounts.length != MAX_INPUT_AMOUNTS) {
        throw new IllegalArgumentException(
            "Offline input amount witness count must be " + MAX_INPUT_AMOUNTS);
      }
      if (this.outputAmounts.length != MAX_OUTPUT_AMOUNTS) {
        throw new IllegalArgumentException(
            "Offline output amount witness count must be " + MAX_OUTPUT_AMOUNTS);
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

    public static InstanceValues redeemInstanceValues(final Redeem redemption) {
      final long inputCount =
          validateCount(redemption.inputNullifiers().size(), MAX_INPUT_AMOUNTS, "redemption input");
      final List<String> amounts = new ArrayList<>();
      amounts.add(redemption.canonicalAmount());
      amounts.add(redemption.canonicalAmount());
      final List<Long> normalizedAmounts = normalizedAmountUnits(amounts);
      final long inputSum = normalizedAmounts.get(0);
      final long outputSum = normalizedAmounts.get(1);
      final byte[] issuedClaimHash =
          new IssuedClaim(
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

    public static InstanceValues auditInstanceValues(final AuditBundle audit) {
      final long inputCount =
          validateCount(audit.inputClaims().size(), MAX_INPUT_AMOUNTS, "audit input");
      final long outputCount =
          validateCount(audit.outputClaims().size(), MAX_OUTPUT_AMOUNTS, "audit output");
      if (audit.inputNullifiers().size() != audit.inputClaims().size()) {
        throw new IllegalArgumentException(
            "audit input nullifier count must match input claim count");
      }
      if (audit.outputCommitments().size() != audit.outputClaims().size()) {
        throw new IllegalArgumentException(
            "audit output claim count must match output commitment count");
      }
      for (int i = 0; i < audit.outputCommitments().size(); i++) {
        if (!Arrays.equals(audit.outputClaims().get(i).noteCommitment(), audit.outputCommitments().get(i))) {
          throw new IllegalArgumentException(
              "audit output claims must be ordered one-to-one with output commitments");
        }
      }
      final byte[] senderCertificateHash = audit.senderKeyCertificate().payloadHash();
      for (final IssuedClaim claim : audit.inputClaims()) {
        if (!Arrays.equals(claim.keyCertificatePayloadHash(), senderCertificateHash)) {
          throw new IllegalArgumentException(
              "audit input claims must match sender key certificate");
        }
      }
      final byte[] inputDefinition =
          parseAssetId(audit.inputClaims().get(0).assetId()).definitionBytes;
      for (final IssuedClaim claim : audit.inputClaims()) {
        if (!Arrays.equals(parseAssetId(claim.assetId()).definitionBytes, inputDefinition)) {
          throw new IllegalArgumentException(
              "audit input and output asset definitions must match");
        }
      }
      for (final AuditOutputClaim claim : audit.outputClaims()) {
        if (!Arrays.equals(parseAssetId(claim.assetId()).definitionBytes, inputDefinition)) {
          throw new IllegalArgumentException(
              "audit input and output asset definitions must match");
        }
      }

      final List<byte[]> inputClaimHashes = new ArrayList<>();
      final List<String> amountStrings = new ArrayList<>();
      for (final IssuedClaim claim : audit.inputClaims()) {
        inputClaimHashes.add(claim.claimHash());
        amountStrings.add(claim.canonicalAmount());
      }
      final List<byte[]> outputClaimHashes = new ArrayList<>();
      for (final AuditOutputClaim claim : audit.outputClaims()) {
        final IssuedClaim issued = claim.issuedClaim();
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
        throw new IllegalArgumentException("Offline audit amounts are not conserved");
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

  private static final TypeAdapter<KeyCertificatePayload> KEY_CERTIFICATE_PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final KeyCertificatePayload value) {
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
        public KeyCertificatePayload decode(final NoritoDecoder decoder) {
          return new KeyCertificatePayload(
              readField(decoder, OfflineNote::readString),
              readField(decoder, child -> (int) child.readUInt(16)),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readAccountId),
              readField(decoder, OfflineNote::readBytesVec),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readBytesVec),
              readField(decoder, OfflineNote::readOptionU32),
              readField(decoder, OfflineNote::readBool));
        }
      };

  private static final TypeAdapter<KeyCertificate> KEY_CERTIFICATE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final KeyCertificate value) {
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
        public KeyCertificate decode(final NoritoDecoder decoder) {
          return new KeyCertificate(
              readField(decoder, child -> (int) child.readUInt(16)),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readAccountId),
              readField(decoder, OfflineNote::readBytesVec),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readBytesVec),
              readField(decoder, OfflineNote::readOptionU32),
              readField(decoder, OfflineNote::readBool),
              readField(decoder, OfflineNote::readConstVec));
        }
      };

  private static final TypeAdapter<RecursiveProof> RECURSIVE_PROOF_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RecursiveProof value) {
          writeField(encoder, child -> writeVerifyingKeyId(child, value.verifierKeyId()));
          writeField(encoder, child -> child.writeBytes(value.publicInputsHash()));
          writeField(encoder, child -> writeProofBox(child, value.proof()));
        }

        @Override
        public RecursiveProof decode(final NoritoDecoder decoder) {
          return new RecursiveProof(
              readField(decoder, OfflineNote::readVerifyingKeyId),
              readField(decoder, child -> readHash(child, "public_inputs_hash")),
              readField(decoder, OfflineNote::readProofBox));
        }
      };

  private static final TypeAdapter<Issue> ISSUE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final Issue value) {
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.keyCertificate()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public Issue decode(final NoritoDecoder decoder) {
          return new Issue(
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric));
        }
      };

  private static final TypeAdapter<IssuedClaim> ISSUED_CLAIM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final IssuedClaim value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> child.writeBytes(value.keyCertificatePayloadHash()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public IssuedClaim decode(final NoritoDecoder decoder) {
          return new IssuedClaim(
              readField(decoder, OfflineNote::readString),
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric));
        }
      };

  private static final TypeAdapter<AuditOutputClaim> AUDIT_OUTPUT_CLAIM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditOutputClaim value) {
          writeField(encoder, child -> child.writeBytes(value.noteCommitment()));
          writeField(encoder, child -> KEY_CERTIFICATE_ADAPTER.encode(child, value.keyCertificate()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public AuditOutputClaim decode(final NoritoDecoder decoder) {
          return new AuditOutputClaim(
              readField(decoder, child -> readHash(child, "note_commitment")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric));
        }
      };

  private static final TypeAdapter<RedeemPublicInputs> REDEEM_PUBLIC_INPUTS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final RedeemPublicInputs value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> child.writeBytes(value.sourceNoteCommitment()));
          writeField(encoder, child -> writeVec(child, value.inputNullifiers(), NoritoEncoder::writeBytes));
          writeField(encoder, child -> child.writeBytes(value.keyCertificatePayloadHash()));
          writeField(encoder, child -> writeAccountId(child, value.recipient()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
        }

        @Override
        public RedeemPublicInputs decode(final NoritoDecoder decoder) {
          return new RedeemPublicInputs(
              readField(decoder, OfflineNote::readString),
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, OfflineNote::readAccountId),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric));
        }
      };

  private static final TypeAdapter<Redeem> REDEEM_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final Redeem value) {
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
        public Redeem decode(final NoritoDecoder decoder) {
          return new Redeem(
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, OfflineNote::readAccountId),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric),
              readField(decoder, RECURSIVE_PROOF_ADAPTER::decode));
        }
      };

  private static final TypeAdapter<AuditPublicInputs> AUDIT_PUBLIC_INPUTS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditPublicInputs value) {
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
        public AuditPublicInputs decode(final NoritoDecoder decoder) {
          return new AuditPublicInputs(
              readField(decoder, OfflineNote::readString),
              readField(decoder, child -> readHash(child, "token_id")),
              readField(decoder, child -> readHash(child, "key_certificate_payload_hash")),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)),
              readField(decoder, child -> readVec(child, element -> readHash(element, "output_commitment"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)));
        }
      };

  private static final TypeAdapter<AuditBundle> AUDIT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final AuditBundle value) {
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
        public AuditBundle decode(final NoritoDecoder decoder) {
          return new AuditBundle(
              readField(decoder, child -> readHash(child, "token_id")),
              readField(decoder, KEY_CERTIFICATE_ADAPTER::decode),
              readField(decoder, child -> readVec(child, element -> readHash(element, "input_nullifier"))),
              readField(decoder, child -> readVec(child, ISSUED_CLAIM_ADAPTER::decode)),
              readField(decoder, child -> readVec(child, element -> readHash(element, "output_commitment"))),
              readField(decoder, child -> readVec(child, AUDIT_OUTPUT_CLAIM_ADAPTER::decode)),
              readField(decoder, RECURSIVE_PROOF_ADAPTER::decode));
        }
      };

  private static final TypeAdapter<NoteCommitmentPreimage> NOTE_COMMITMENT_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final NoteCommitmentPreimage value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> child.writeBytes(value.ownerKeyCertificatePayloadHash()));
          writeField(encoder, child -> writeAssetId(child, value.assetId()));
          writeField(encoder, child -> writeNumeric(child, value.canonicalAmount()));
          writeField(encoder, child -> writeBytesVec(child, value.noteSecret()));
          writeField(encoder, child -> writeCommitmentOrigin(child, value.origin()));
        }

        @Override
        public NoteCommitmentPreimage decode(final NoritoDecoder decoder) {
          return new NoteCommitmentPreimage(
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readChainId),
              readField(decoder, child -> readHash(child, "owner_key_certificate_payload_hash")),
              readField(decoder, OfflineNote::readAssetId),
              readField(decoder, OfflineNote::readNumeric),
              readField(decoder, OfflineNote::readBytesVec),
              readField(decoder, OfflineNote::readCommitmentOrigin));
        }
      };

  private static final TypeAdapter<InputNullifierPreimage> INPUT_NULLIFIER_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final InputNullifierPreimage value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> child.writeBytes(value.sourceNoteCommitment()));
          writeField(encoder, child -> child.writeBytes(value.ownerKeyCertificatePayloadHash()));
          writeField(encoder, child -> writeBytesVec(child, value.noteSecret()));
        }

        @Override
        public InputNullifierPreimage decode(final NoritoDecoder decoder) {
          return new InputNullifierPreimage(
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readChainId),
              readField(decoder, child -> readHash(child, "source_note_commitment")),
              readField(decoder, child -> readHash(child, "owner_key_certificate_payload_hash")),
              readField(decoder, OfflineNote::readBytesVec));
        }
      };

  private static final TypeAdapter<PaymentTokenIdPreimage> PAYMENT_TOKEN_ID_PREIMAGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final PaymentTokenIdPreimage value) {
          writeField(encoder, child -> writeString(child, value.domain()));
          writeField(encoder, child -> writeChainId(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> child.writeUInt(value.createdAtMs(), 64));
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
        public PaymentTokenIdPreimage decode(final NoritoDecoder decoder) {
          return new PaymentTokenIdPreimage(
              readField(decoder, OfflineNote::readString),
              readField(decoder, OfflineNote::readChainId),
              readField(decoder, OfflineNote::readString),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, OfflineNote::readBytesVec),
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
      throw new IllegalArgumentException("Trailing bytes after Offline Note field decode");
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
        readField(decoder, OfflineNote::readString),
        readField(decoder, OfflineNote::readString));
  }

  private static ProofBox readProofBox(final NoritoDecoder decoder) {
    return new ProofBox(
        readField(decoder, OfflineNote::readString),
        readField(decoder, OfflineNote::readBytesVec));
  }

  private static String readChainId(final NoritoDecoder decoder) {
    return readField(decoder, OfflineNote::readString);
  }

  private static CommitmentOrigin readCommitmentOrigin(final NoritoDecoder decoder) {
    final long tag = decoder.readUInt(32);
    if (tag == 0L) {
      return readField(
          decoder,
          payload ->
              new CommitmentOrigin.IssuerLoad(
                  readField(payload, OfflineNote::readString),
                  readField(payload, OfflineNote::readString),
                  readField(payload, child -> child.readUInt(64))));
    }
    if (tag == 1L) {
      return readField(
          decoder,
          payload ->
              new CommitmentOrigin.P2pOutput(
                  readField(payload, OfflineNote::readString),
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
                          readField(member, OfflineNote::readPublicKeyPayload);
                      final int weight = readField(member, child -> (int) child.readUInt(16));
                      return MultisigMemberPayload.of(publicKey.curveId(), weight, publicKey.keyBytes());
                    }));
    return MultisigPolicyPayload.of(version, threshold, members);
  }

  private static String readAssetId(final NoritoDecoder decoder) {
    final String accountId = readField(decoder, OfflineNote::readAccountId);
    final byte[] definitionBytes = readField(decoder, OfflineNote::readAssetDefinitionAddress);
    final String definitionId = AssetDefinitionIdEncoder.encodeFromBytes(definitionBytes);
    final Long dataspaceId = readField(decoder, OfflineNote::readAssetBalanceScope);
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
      final NoritoEncoder encoder, final CommitmentOrigin origin) {
    if (origin instanceof CommitmentOrigin.IssuerLoad) {
      final CommitmentOrigin.IssuerLoad issuerLoad = (CommitmentOrigin.IssuerLoad) origin;
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
    if (origin instanceof CommitmentOrigin.P2pOutput) {
      final CommitmentOrigin.P2pOutput output = (CommitmentOrigin.P2pOutput) origin;
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
    if (policy.version() != MULTISIG_POLICY_VERSION) {
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
    sorted.sort(Comparator.comparing(OfflineNote::canonicalSortKey, OfflineNote::compareUnsigned));
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
          "Offline " + label + " count " + count + " must be in 1.." + max);
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
            "Offline amount " + numeric.original + " must not be negative");
      }
      final int scaleDelta = targetScale - numeric.scale;
      final BigInteger aligned = numeric.mantissa.multiply(BigInteger.TEN.pow(scaleDelta));
      if (aligned.bitLength() > 64) {
        throw new IllegalArgumentException(
            "Offline amount "
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
            "Offline " + label + " amount sum overflows u64 witness units");
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
      final int version,
      final String accountId,
      final byte[] publicKey,
      final Integer assertionUsageCountLimit,
      final boolean oneUse) {
    if (version != KEY_CERTIFICATE_VERSION) {
      throw new IllegalArgumentException("Offline Note key certificate format is unsupported");
    }
    if (!oneUse || (assertionUsageCountLimit != null && assertionUsageCountLimit.intValue() != 1)) {
      throw new IllegalArgumentException(
          "Offline Note key certificate must be one-use with usage limit 1 when present");
    }
    if (publicKey.length != 32) {
      throw new IllegalArgumentException("Offline Note note public key must be 32 bytes");
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

  private static String requireNonBlankUnpadded(final String value, final String field) {
    final String checked = requireNonBlank(value, field);
    if (!checked.trim().equals(checked)) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    return checked;
  }

  private static String requireDomain(
      final String value, final String expected, final String field) {
    final String checked = Objects.requireNonNull(value, field);
    if (!expected.equals(checked)) {
      throw new IllegalArgumentException(field + " must be " + expected);
    }
    return checked;
  }

  private static String requireNoColon(final String value, final String field) {
    if (value.indexOf(':') >= 0) {
      throw new IllegalArgumentException(field + " must not contain ':'");
    }
    return value;
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
