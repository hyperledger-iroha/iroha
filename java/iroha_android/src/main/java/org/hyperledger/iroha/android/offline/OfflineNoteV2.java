package org.hyperledger.iroha.android.offline;

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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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
          throw new UnsupportedOperationException("Offline Note V2 decoding is not supported yet");
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

  private static void writeAccountId(final NoritoEncoder encoder, final String accountId) {
    encoder.writeBytes(encodeAccountIdPayload(accountId));
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
            writeField(
                memberEncoder,
                child -> writePublicKey(child, member.curveId(), member.publicKey()));
            writeField(memberEncoder, child -> child.writeUInt(member.weight(), 16));
          });
    }
  }

  private static void writePublicKey(
      final NoritoEncoder encoder, final int curveId, final byte[] publicKey) {
    writeConstVec(encoder, PublicKeyCodec.compactPublicKeyPayload(curveId, publicKey));
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
