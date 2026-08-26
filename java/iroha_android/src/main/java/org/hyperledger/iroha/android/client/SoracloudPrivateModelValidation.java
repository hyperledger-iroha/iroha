package org.hyperledger.iroha.android.client;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.bouncycastle.crypto.agreement.X25519Agreement;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.Blake3;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Shared invariants for the Soracloud private uploaded-model client surface. */
final class SoracloudPrivateModelValidation {
  static final long U32_MAX = 4_294_967_295L;
  static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  static final long ENCRYPTED_ARTIFACT_MAX_BYTES = 72L * 1024L * 1024L;
  static final int NAME_MAX_UTF8_BYTES = 255;
  static final int IDENTIFIER_MAX_BYTES = 128;
  static final int SERVICE_VERSION_MAX_UTF8_BYTES = 256;
  static final int PRIVATE_RECEIPT_CURSOR_LENGTH_V1 = 114;
  static final String RUNTIME_VERSION_V1 = "soracloud.quantized-cpu.v1";
  static final String X25519_HKDF_SHA256 = "X25519HkdfSha256";
  static final String AES_256_GCM = "Aes256Gcm";
  private static final int MAX_JSON_DEPTH = 128;
  private static final int AUTO_REPLICATION_ORDER_NAMESPACE_TAG = 0x80;
  private static final byte[] AUTO_REPLICATION_ORDER_DOMAIN_V1 =
      "sorafs:auto-replication-order:v1".getBytes(StandardCharsets.US_ASCII);

  private static final String ZERO_PREHASH_SENTINEL =
      "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
  private static final byte[] LOW_ORDER_X25519_PROBE_PRIVATE_KEY = filledBytes((byte) 1, 32);

  private SoracloudPrivateModelValidation() {}

  static long requireSchemaVersion(final long value, final String field) {
    if (value != 1L) {
      throw new IllegalArgumentException(field + " must equal 1");
    }
    return value;
  }

  static long requireU32(final long value, final String field) {
    if (value < 0L || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be within 0..=" + U32_MAX);
    }
    return value;
  }

  static long requirePositiveU32(final long value, final String field) {
    if (value < 1L || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be within 1..=" + U32_MAX);
    }
    return value;
  }

  static String requireCanonicalString(final String value, final String field) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    if (isWhitespace(value.charAt(0)) || isWhitespace(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException(
          field + " must not contain leading or trailing whitespace");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (Character.isHighSurrogate(character)) {
        if (index + 1 >= value.length() || !Character.isLowSurrogate(value.charAt(index + 1))) {
          throw new IllegalArgumentException(field + " must contain valid Unicode scalar values");
        }
        index++;
      } else if (Character.isLowSurrogate(character)) {
        throw new IllegalArgumentException(field + " must contain valid Unicode scalar values");
      } else if (Character.isISOControl(character)) {
        throw new IllegalArgumentException(field + " must be free of control characters");
      }
    }
    return value;
  }

  static String requireCanonicalName(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.getBytes(StandardCharsets.UTF_8).length > NAME_MAX_UTF8_BYTES) {
      throw new IllegalArgumentException(
          field + " must contain at most " + NAME_MAX_UTF8_BYTES + " UTF-8 bytes");
    }
    if (!Normalizer.isNormalized(canonical, Normalizer.Form.NFC)) {
      throw new IllegalArgumentException(field + " must use its exact NFC-normalized spelling");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (isWhitespace(character)
          || isBidiControl(character)
          || character == '@'
          || character == '#'
          || character == '$') {
        throw new IllegalArgumentException(field + " must be a canonical Iroha Name");
      }
    }
    return canonical;
  }

  static String requireIdentifier(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.length() > IDENTIFIER_MAX_BYTES) {
      throw new IllegalArgumentException(
          field + " must contain at most " + IDENTIFIER_MAX_BYTES + " ASCII bytes");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (!(character >= 'A' && character <= 'Z')
          && !(character >= 'a' && character <= 'z')
          && !(character >= '0' && character <= '9')
          && character != '-'
          && character != '_'
          && character != '.'
          && character != ':'
          && character != '#') {
        throw new IllegalArgumentException(
            field + " must use only ASCII letters, digits, or [-_.:#]");
      }
    }
    return canonical;
  }

  static String requireServiceVersion(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.getBytes(StandardCharsets.UTF_8).length > SERVICE_VERSION_MAX_UTF8_BYTES) {
      throw new IllegalArgumentException(
          field
              + " must contain at most "
              + SERVICE_VERSION_MAX_UTF8_BYTES
              + " UTF-8 bytes");
    }
    return canonical;
  }

  static String requireArtifactRole(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (!"input".equals(canonical) && !"output".equals(canonical)) {
      throw new IllegalArgumentException(field + " must equal input or output");
    }
    return canonical;
  }

  static String requireNetworkId(final String value, final String field) {
    final String canonical = NetworkId.parse(requireCanonicalString(value, field)).literal();
    if (!canonical.equals(value)) {
      throw new IllegalArgumentException(
          field + " must be an exact canonical checksummed 32-byte NetworkId literal");
    }
    return canonical;
  }

  static String requireHash(final String value, final String field) {
    DaJson.requireHash(value, field);
    return value;
  }

  static String requireSoracloudHash(final String value, final String field) {
    requireHash(value, field);
    if (ZERO_PREHASH_SENTINEL.equals(value)) {
      throw new IllegalArgumentException(field + " must not be the zero prehash sentinel");
    }
    return value;
  }

  static byte[] decodeCanonicalX25519PublicKey(final String encoded, final String field) {
    requireCanonicalString(encoded, field);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(field + " must be canonical base64", error);
    }
    if (decoded.length != 32 || !Base64.getEncoder().encodeToString(decoded).equals(encoded)) {
      throw new IllegalArgumentException(
          field + " must be canonical base64 encoding exactly 32 bytes");
    }
    if (isLowOrderX25519PublicKey(decoded)) {
      throw new IllegalArgumentException(field + " must not be a low-order X25519 public key");
    }
    return decoded;
  }

  static String requireRecipientFingerprint(
      final String fingerprint, final byte[] publicKeyBytes, final String field) {
    final String canonical = requireSoracloudHash(fingerprint, field);
    final String expected = HashLiteral.canonicalize(IrohaHash.prehash(publicKeyBytes));
    if (!expected.equals(canonical)) {
      throw new IllegalArgumentException(
          field + " must equal IrohaHash.prehash(publicKeyBytes)");
    }
    return canonical;
  }

  static byte[] requireSorafsAutoReplicationOrderIdV1(
      final byte[] value, final byte[] outputManifestDigest, final String field) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(field + " must contain exactly 32 bytes");
    }
    final byte[] expected = deriveSorafsAutoReplicationOrderIdV1(outputManifestDigest);
    if (!Arrays.equals(value, expected)) {
      throw new IllegalArgumentException(
          field
              + " must equal the tagged automatic replication-order ID derived from "
              + "outputArtifact.sorafsManifestDigest");
    }
    return value.clone();
  }

  static byte[] deriveSorafsAutoReplicationOrderIdV1(final byte[] outputManifestDigest) {
    if (outputManifestDigest == null || outputManifestDigest.length != 32) {
      throw new IllegalArgumentException(
          "outputManifestDigest must contain exactly 32 bytes");
    }
    final byte[] preimage =
        Arrays.copyOf(
            AUTO_REPLICATION_ORDER_DOMAIN_V1,
            AUTO_REPLICATION_ORDER_DOMAIN_V1.length + outputManifestDigest.length);
    System.arraycopy(
        outputManifestDigest,
        0,
        preimage,
        AUTO_REPLICATION_ORDER_DOMAIN_V1.length,
        outputManifestDigest.length);
    final byte[] orderId = Blake3.hash(preimage);
    orderId[0] = (byte) (orderId[0] | AUTO_REPLICATION_ORDER_NAMESPACE_TAG);
    return orderId;
  }

  static void requireValidatorIdentity(
      final String validatorAccountId, final String peerId) {
    final String canonicalAccount =
        AccountIdLiteral.requireCanonicalI105Address(
            requireCanonicalString(validatorAccountId, "validatorAccountId"),
            "validatorAccountId");
    final Optional<AccountAddress.SingleKeyPayload> signatory;
    try {
      signatory = AccountAddress.fromI105(canonicalAccount, null).singleKeyPayload();
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException(
          "validatorAccountId must use a canonical universal domainless AccountId", error);
    }
    if (!signatory.isPresent()) {
      throw new IllegalArgumentException(
          "validatorAccountId must have exactly one signatory");
    }

    final String canonicalPeerId = requireCanonicalString(peerId, "peerId");
    final PublicKeyCodec.PublicKeyPayload peer;
    try {
      peer = PublicKeyCodec.decodePublicKeyLiteral(canonicalPeerId);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException("peerId must be a canonical PeerId", error);
    }
    if (peer == null
        || !PublicKeyCodec.encodePublicKeyMultihash(peer.curveId(), peer.keyBytes())
            .equals(canonicalPeerId)) {
      throw new IllegalArgumentException(
          "peerId must use the exact canonical peer public-key spelling");
    }
    final AccountAddress.SingleKeyPayload accountSignatory = signatory.get();
    if (peer.curveId() != accountSignatory.curveId()
        || !Arrays.equals(peer.keyBytes(), accountSignatory.publicKey())) {
      throw new IllegalArgumentException(
          "peerId must equal validatorAccountId's exact single signatory");
    }
  }

  static void requireLedgerCoordinates(
      final BigInteger authorizationClaimBlockHeight,
      final BigInteger authorizationClaimEpoch,
      final BigInteger sequence,
      final BigInteger blockHeight,
      final BigInteger epoch) {
    Objects.requireNonNull(authorizationClaimBlockHeight, "authorizationClaimBlockHeight");
    Objects.requireNonNull(authorizationClaimEpoch, "authorizationClaimEpoch");
    Objects.requireNonNull(sequence, "emittedSequence");
    Objects.requireNonNull(blockHeight, "emittedBlockHeight");
    Objects.requireNonNull(epoch, "emittedEpoch");
    if (authorizationClaimBlockHeight.signum() < 0
        || authorizationClaimEpoch.signum() < 0
        || sequence.signum() < 0
        || blockHeight.signum() < 0
        || epoch.signum() < 0
        || authorizationClaimBlockHeight.compareTo(U64_MAX) > 0
        || authorizationClaimEpoch.compareTo(U64_MAX) > 0
        || sequence.compareTo(U64_MAX) > 0
        || blockHeight.compareTo(U64_MAX) > 0
        || epoch.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(
          "authorization and emission coordinates must fit unsigned 64-bit integers");
    }
    if (!((authorizationClaimBlockHeight.signum() == 0
            && authorizationClaimEpoch.signum() == 0
            && sequence.signum() == 0
            && blockHeight.signum() == 0
            && epoch.signum() == 0)
        || (authorizationClaimBlockHeight.signum() > 0
            && authorizationClaimEpoch.signum() > 0
            && sequence.signum() > 0
            && blockHeight.signum() > 0
            && epoch.signum() > 0))) {
      throw new IllegalArgumentException(
          "authorization and emission coordinates must all be zero or all be positive");
    }
    if (authorizationClaimBlockHeight.signum() > 0
        && (blockHeight.compareTo(authorizationClaimBlockHeight) < 0
            || epoch.compareTo(authorizationClaimEpoch) < 0)) {
      throw new IllegalArgumentException(
          "emission coordinates must not precede authorization claim coordinates");
    }
  }

  static Map<String, Object> snapshotUploadedModelStatus(
      final Map<String, Object> status) {
    Objects.requireNonNull(status, "status");
    validateUploadedModelStatus(status, "status");
    return immutableJsonObject(status, "status", new IdentityHashMap<>(), 0);
  }

  static void validateUploadedModelStatus(
      final Map<String, Object> status, final String path) {
    requireExactFields(status, path, "schema_version", "bundle", "artifact");
    requireSchemaVersionOne(status.get("schema_version"), path + ".schema_version");
    final Map<String, Object> bundle =
        exactObject(
            status.get("bundle"),
            path + ".bundle",
            "schema_version",
            "service_name",
            "model_id",
            "weight_version",
            "family",
            "modalities",
            "plaintext_root",
            "runtime_format",
            "bundle_root",
            "sorafs_manifest_digest",
            "chunk_count",
            "plaintext_bytes",
            "ciphertext_bytes",
            "chunk_manifest_root",
            "upload_recipient",
            "wrapped_bundle_key",
            "pricing_policy",
            "decryption_policy_ref");
    validateUploadedModelBundle(bundle, path + ".bundle");
    if (status.get("artifact") != null) {
      validateUploadedModelArtifactStatus(
          exactObject(
              status.get("artifact"),
              path + ".artifact",
              "service_name",
              "model_name",
              "artifact_id",
              "training_job_id",
              "weight_version",
              "weight_artifact_hash",
              "dataset_ref",
              "training_config_hash",
              "reproducibility_hash",
              "provenance_attestation_hash",
              "registered_sequence",
              "consumed_by_version",
              "chunk_manifest_root"),
          path + ".artifact");
    }
  }

  static void requireUploadedModelStatusMatchesReceipt(
      final Map<String, Object> status,
      final SoracloudPrivateUploadedModelExecutionReceipt receipt,
      final String path) {
    @SuppressWarnings("unchecked")
    final Map<String, Object> bundle = (Map<String, Object>) status.get("bundle");
    requireEqual(bundle.get("service_name"), receipt.serviceName(), path + ".bundle.service_name");
    requireEqual(bundle.get("model_id"), receipt.modelId(), path + ".bundle.model_id");
    requireEqual(
        bundle.get("weight_version"), receipt.weightVersion(), path + ".bundle.weight_version");
    requireEqual(
        bundle.get("bundle_root"), receipt.modelBundleRoot(), path + ".bundle.bundle_root");
    requireEqual(
        bundle.get("decryption_policy_ref"),
        receipt.policyId(),
        path + ".bundle.decryption_policy_ref");
    if (!Arrays.equals(
        exactManifestDigest(
            bundle.get("sorafs_manifest_digest"), path + ".bundle.sorafs_manifest_digest"),
        receipt.modelManifestDigest())) {
      throw new IllegalArgumentException(
          path + ".bundle.sorafs_manifest_digest must match receipt.modelManifestDigest");
    }
    if (status.get("artifact") != null) {
      @SuppressWarnings("unchecked")
      final Map<String, Object> artifact = (Map<String, Object>) status.get("artifact");
      requireEqual(
          artifact.get("service_name"), receipt.serviceName(), path + ".artifact.service_name");
      requireEqual(
          artifact.get("weight_version"),
          receipt.weightVersion(),
          path + ".artifact.weight_version");
      requireEqual(
          artifact.get("chunk_manifest_root"),
          bundle.get("chunk_manifest_root"),
          path + ".artifact.chunk_manifest_root");
    }
  }

  private static void validateUploadedModelBundle(
      final Map<String, Object> bundle, final String path) {
    requireSchemaVersionOne(bundle.get("schema_version"), path + ".schema_version");
    requireCanonicalName(
        exactString(bundle.get("service_name"), path + ".service_name"),
        path + ".service_name");
    requireIdentifier(
        exactString(bundle.get("model_id"), path + ".model_id"), path + ".model_id");
    requireIdentifier(
        exactString(bundle.get("weight_version"), path + ".weight_version"),
        path + ".weight_version");
    requireCanonicalString(exactString(bundle.get("family"), path + ".family"), path + ".family");
    final List<Object> modalities = exactArray(bundle.get("modalities"), path + ".modalities");
    if (modalities.isEmpty()) {
      throw new IllegalArgumentException(path + ".modalities must not be empty");
    }
    final List<String> canonicalModalities = new ArrayList<>();
    for (int index = 0; index < modalities.size(); index++) {
      final String modality =
          requireCanonicalString(
              exactString(modalities.get(index), path + ".modalities[" + index + "]"),
              path + ".modalities[" + index + "]");
      if (canonicalModalities.contains(modality)) {
        throw new IllegalArgumentException(path + ".modalities entries must be unique");
      }
      canonicalModalities.add(modality);
    }
    exactHash(bundle.get("plaintext_root"), path + ".plaintext_root");
    exactUnitVariant(
        bundle.get("runtime_format"),
        "runtime_format",
        "DeterministicQuantizedCpuV1",
        path + ".runtime_format");
    exactHash(bundle.get("bundle_root"), path + ".bundle_root");
    exactManifestDigest(bundle.get("sorafs_manifest_digest"), path + ".sorafs_manifest_digest");
    exactUnsigned(
        bundle.get("chunk_count"), path + ".chunk_count", BigInteger.valueOf(U32_MAX), true);
    exactUnsigned(bundle.get("plaintext_bytes"), path + ".plaintext_bytes", U64_MAX, true);
    exactUnsigned(bundle.get("ciphertext_bytes"), path + ".ciphertext_bytes", U64_MAX, true);
    exactHash(bundle.get("chunk_manifest_root"), path + ".chunk_manifest_root");
    final Map<String, Object> recipient =
        exactObject(
            bundle.get("upload_recipient"),
            path + ".upload_recipient",
            "schema_version",
            "key_id",
            "key_version",
            "kem",
            "aead",
            "public_key_bytes",
            "public_key_fingerprint");
    validateUploadedModelRecipient(recipient, path + ".upload_recipient");
    validateUploadedModelWrappedKey(
        exactObject(
            bundle.get("wrapped_bundle_key"),
            path + ".wrapped_bundle_key",
            "schema_version",
            "recipient_key_id",
            "recipient_key_version",
            "kem",
            "aead",
            "ephemeral_public_key",
            "nonce",
            "wrapped_key_ciphertext",
            "ciphertext_hash",
            "aad_digest"),
        recipient,
        path + ".wrapped_bundle_key");
    final Map<String, Object> pricing =
        exactObject(bundle.get("pricing_policy"), path + ".pricing_policy", "storage_price");
    final String storagePrice =
        exactString(pricing.get("storage_price"), path + ".pricing_policy.storage_price");
    try {
      NumericV1.QuantityValue.parseCanonical(storagePrice);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          path + ".pricing_policy.storage_price must be a canonical quantity", error);
    }
    requireCanonicalString(
        exactString(bundle.get("decryption_policy_ref"), path + ".decryption_policy_ref"),
        path + ".decryption_policy_ref");
  }

  private static void validateUploadedModelArtifactStatus(
      final Map<String, Object> artifact, final String path) {
    requireCanonicalName(
        exactString(artifact.get("service_name"), path + ".service_name"),
        path + ".service_name");
    for (final String field :
        Arrays.asList("model_name", "artifact_id", "training_job_id", "dataset_ref")) {
      requireCanonicalString(
          exactString(artifact.get(field), path + "." + field), path + "." + field);
    }
    final String weightVersion =
        optionalString(artifact.get("weight_version"), path + ".weight_version");
    if (weightVersion != null) {
      requireIdentifier(weightVersion, path + ".weight_version");
    }
    for (final String field :
        Arrays.asList(
            "weight_artifact_hash",
            "training_config_hash",
            "reproducibility_hash",
            "provenance_attestation_hash")) {
      exactHash(artifact.get(field), path + "." + field);
    }
    exactUnsigned(
        artifact.get("registered_sequence"), path + ".registered_sequence", U64_MAX, true);
    final String consumed =
        optionalString(artifact.get("consumed_by_version"), path + ".consumed_by_version");
    if (consumed != null) {
      requireCanonicalString(consumed, path + ".consumed_by_version");
    }
    if (artifact.get("chunk_manifest_root") != null) {
      exactHash(artifact.get("chunk_manifest_root"), path + ".chunk_manifest_root");
    }
  }

  private static void validateUploadedModelRecipient(
      final Map<String, Object> recipient, final String path) {
    final long schemaVersion =
        exactUnsigned(
                recipient.get("schema_version"),
                path + ".schema_version",
                BigInteger.ONE,
                true)
            .longValue();
    final String keyId = exactString(recipient.get("key_id"), path + ".key_id");
    final long keyVersion =
        exactUnsigned(
                recipient.get("key_version"),
                path + ".key_version",
                BigInteger.valueOf(U32_MAX),
                true)
            .longValue();
    final String kem =
        exactUnitVariant(recipient.get("kem"), "kem", X25519_HKDF_SHA256, path + ".kem");
    final String aead =
        exactUnitVariant(recipient.get("aead"), "aead", AES_256_GCM, path + ".aead");
    new SoracloudUploadedModelEncryptionRecipient(
        schemaVersion,
        keyId,
        keyVersion,
        kem,
        aead,
        exactString(recipient.get("public_key_bytes"), path + ".public_key_bytes"),
        exactHash(recipient.get("public_key_fingerprint"), path + ".public_key_fingerprint"));
  }

  private static void validateUploadedModelWrappedKey(
      final Map<String, Object> wrappedKey,
      final Map<String, Object> recipient,
      final String path) {
    requireSchemaVersionOne(wrappedKey.get("schema_version"), path + ".schema_version");
    final String keyId =
        requireCanonicalString(
            exactString(wrappedKey.get("recipient_key_id"), path + ".recipient_key_id"),
            path + ".recipient_key_id");
    final BigInteger keyVersion =
        exactUnsigned(
            wrappedKey.get("recipient_key_version"),
            path + ".recipient_key_version",
            BigInteger.valueOf(U32_MAX),
            true);
    exactUnitVariant(wrappedKey.get("kem"), "kem", X25519_HKDF_SHA256, path + ".kem");
    exactUnitVariant(wrappedKey.get("aead"), "aead", AES_256_GCM, path + ".aead");
    decodeCanonicalX25519PublicKey(
        exactString(wrappedKey.get("ephemeral_public_key"), path + ".ephemeral_public_key"),
        path + ".ephemeral_public_key");
    canonicalBase64(wrappedKey.get("nonce"), path + ".nonce", 1, 256);
    final byte[] ciphertext =
        canonicalBase64(
            wrappedKey.get("wrapped_key_ciphertext"),
            path + ".wrapped_key_ciphertext",
            1,
            4096);
    final String ciphertextHash =
        exactHash(wrappedKey.get("ciphertext_hash"), path + ".ciphertext_hash");
    if (!HashLiteral.canonicalize(IrohaHash.prehash(ciphertext)).equals(ciphertextHash)) {
      throw new IllegalArgumentException(
          path + ".ciphertext_hash must match wrapped_key_ciphertext");
    }
    exactHash(wrappedKey.get("aad_digest"), path + ".aad_digest");
    if (!keyId.equals(recipient.get("key_id"))
        || !keyVersion.equals(
            exactUnsigned(
                recipient.get("key_version"),
                path + ".recipient_key_version",
                BigInteger.valueOf(U32_MAX),
                true))) {
      throw new IllegalArgumentException(path + " recipient key must match upload_recipient");
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> exactObject(
      final Object value, final String path, final String... fields) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(path + " must be a JSON object");
    }
    final Map<?, ?> raw = (Map<?, ?>) value;
    for (final Object key : raw.keySet()) {
      if (!(key instanceof String)) {
        throw new IllegalArgumentException(path + " keys must be strings");
      }
    }
    final Map<String, Object> object = (Map<String, Object>) value;
    requireExactFields(object, path, fields);
    return object;
  }

  private static void requireExactFields(
      final Map<String, Object> value, final String path, final String... fields) {
    if (value.size() != fields.length) {
      throw new IllegalArgumentException(path + " must contain the exact V1 field set");
    }
    for (final String field : fields) {
      if (!value.containsKey(field)) {
        throw new IllegalArgumentException(path + "." + field + " is missing required field");
      }
    }
  }

  @SuppressWarnings("unchecked")
  private static List<Object> exactArray(final Object value, final String path) {
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(path + " must be a JSON array");
    }
    return (List<Object>) value;
  }

  private static String exactString(final Object value, final String path) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(path + " must be a string");
    }
    return (String) value;
  }

  private static String optionalString(final Object value, final String path) {
    return value == null ? null : exactString(value, path);
  }

  private static void requireSchemaVersionOne(final Object value, final String path) {
    if (!BigInteger.ONE.equals(exactUnsigned(value, path, BigInteger.ONE, true))) {
      throw new IllegalArgumentException(path + " must equal 1");
    }
  }

  private static BigInteger exactUnsigned(
      final Object value, final String path, final BigInteger maximum, final boolean positive) {
    final BigInteger parsed;
    if (value instanceof BigInteger) {
      parsed = (BigInteger) value;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      parsed = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalArgumentException(path + " must be an integer");
    }
    if (parsed.signum() < (positive ? 1 : 0) || parsed.compareTo(maximum) > 0) {
      throw new IllegalArgumentException(path + " is outside its unsigned range");
    }
    return parsed;
  }

  private static byte[] exactManifestDigest(final Object value, final String path) {
    final List<Object> values = exactArray(value, path);
    if (values.size() != 32) {
      throw new IllegalArgumentException(path + " must contain exactly 32 bytes");
    }
    final byte[] digest = new byte[32];
    for (int index = 0; index < digest.length; index++) {
      digest[index] =
          exactUnsigned(
                  values.get(index),
                  path + "[" + index + "]",
                  BigInteger.valueOf(255L),
                  false)
              .byteValue();
    }
    return digest;
  }

  private static String exactHash(final Object value, final String path) {
    return requireSoracloudHash(exactString(value, path), path);
  }

  private static String exactUnitVariant(
      final Object value,
      final String tag,
      final String expected,
      final String path) {
    final Map<String, Object> variant = exactObject(value, path, tag, "value");
    if (!expected.equals(exactString(variant.get(tag), path + "." + tag))) {
      throw new IllegalArgumentException(path + "." + tag + " must equal " + expected);
    }
    if (variant.get("value") != null) {
      throw new IllegalArgumentException(path + ".value must be null");
    }
    return expected;
  }

  private static byte[] canonicalBase64(
      final Object value,
      final String path,
      final int minimumBytes,
      final int maximumBytes) {
    final String encoded = exactString(value, path);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(path + " must be canonical base64", error);
    }
    if (decoded.length < minimumBytes
        || decoded.length > maximumBytes
        || !Base64.getEncoder().encodeToString(decoded).equals(encoded)) {
      throw new IllegalArgumentException(path + " must be canonical base64 within its V1 bounds");
    }
    return decoded;
  }

  private static void requireEqual(
      final Object actual, final Object expected, final String path) {
    if (!Objects.equals(actual, expected)) {
      throw new IllegalArgumentException(path + " must match receipt");
    }
  }

  private static Map<String, Object> immutableJsonObject(
      final Map<?, ?> source,
      final String path,
      final IdentityHashMap<Object, Boolean> active,
      final int depth) {
    if (depth > MAX_JSON_DEPTH) {
      throw new IllegalArgumentException(path + " exceeds maximum JSON nesting depth");
    }
    if (active.put(source, Boolean.TRUE) != null) {
      throw new IllegalArgumentException(path + " must not contain a reference cycle");
    }
    try {
      final Map<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : source.entrySet()) {
        if (!(entry.getKey() instanceof String)) {
          throw new IllegalArgumentException(path + " must use string object keys");
        }
        final String key = (String) entry.getKey();
        copy.put(
            key,
            immutableJsonValue(entry.getValue(), path + "." + key, active, depth + 1));
      }
      return Collections.unmodifiableMap(copy);
    } finally {
      active.remove(source);
    }
  }

  private static Object immutableJsonValue(
      final Object value,
      final String path,
      final IdentityHashMap<Object, Boolean> active,
      final int depth) {
    if (value == null
        || value instanceof String
        || value instanceof Boolean
        || value instanceof BigInteger
        || value instanceof BigDecimal
        || value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return value;
    }
    if (value instanceof Float || value instanceof Double) {
      if (!Double.isFinite(((Number) value).doubleValue())) {
        throw new IllegalArgumentException(path + " must be a finite JSON number");
      }
      return value;
    }
    if (value instanceof Map<?, ?>) {
      return immutableJsonObject((Map<?, ?>) value, path, active, depth);
    }
    if (value instanceof List<?>) {
      if (depth > MAX_JSON_DEPTH) {
        throw new IllegalArgumentException(path + " exceeds maximum JSON nesting depth");
      }
      if (active.put(value, Boolean.TRUE) != null) {
        throw new IllegalArgumentException(path + " must not contain a reference cycle");
      }
      try {
        final List<Object> copy = new ArrayList<>(((List<?>) value).size());
        int index = 0;
        for (final Object element : (List<?>) value) {
          copy.add(immutableJsonValue(element, path + "[" + index + "]", active, depth + 1));
          index++;
        }
        return Collections.unmodifiableList(copy);
      } finally {
        active.remove(value);
      }
    }
    throw new IllegalArgumentException(path + " must contain only JSON values");
  }

  static void requireExecuteResponseState(
      final SoracloudPrivateUploadedModelSubmissionPhase submissionPhase,
      final String transactionHash,
      final SoracloudPrivateUploadedModelExecutionReceipt receipt,
      final SoracloudPrivateModelArtifactRef outputArtifact) {
    final SoracloudPrivateUploadedModelSubmissionPhase canonicalPhase =
        Objects.requireNonNull(submissionPhase, "submissionPhase");
    Objects.requireNonNull(receipt, "receipt");
    Objects.requireNonNull(outputArtifact, "outputArtifact");
    if (canonicalPhase.requiresTransactionHash() != (transactionHash != null)) {
      throw new IllegalArgumentException(
          canonicalPhase.requiresTransactionHash()
              ? "transactionHash is required for " + canonicalPhase.wireValue()
              : "transactionHash must be null for " + canonicalPhase.wireValue());
    }
    final boolean receiptIsAssigned =
        receipt.authorizationClaimBlockHeight().signum() > 0
            && receipt.authorizationClaimEpoch().signum() > 0
            && receipt.emittedSequence().signum() > 0
            && receipt.emittedBlockHeight().signum() > 0
            && receipt.emittedEpoch().signum() > 0;
    if (canonicalPhase.requiresAssignedReceipt() != receiptIsAssigned) {
      throw new IllegalArgumentException(
          canonicalPhase.requiresAssignedReceipt()
              ? "committed receipt must use positive ledger coordinates"
              : canonicalPhase.wireValue() + " receipt must use zero ledger coordinates");
    }
    if (!sameArtifact(outputArtifact, receipt.outputArtifact())) {
      throw new IllegalArgumentException("outputArtifact must match receipt.outputArtifact");
    }
  }

  static void requireReceiptListMetadata(
      final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts,
      final Long total,
      final long returnedItems,
      final Long remainingItems,
      final boolean hasMore,
      final String countMode,
      final String continueCursor) {
    Objects.requireNonNull(receipts, "receipts");
    if (!"bounded".equals(countMode) && !"exact".equals(countMode)) {
      throw new IllegalArgumentException("countMode must equal bounded or exact");
    }
    if (total != null) {
      requireU32(total.longValue(), "total");
    }
    requireU32(returnedItems, "returnedItems");
    if (remainingItems != null) {
      requireU32(remainingItems.longValue(), "remainingItems");
    }
    if (("bounded".equals(countMode)) != (total == null)) {
      throw new IllegalArgumentException(
          "total must be null for bounded countMode and non-null for exact countMode");
    }
    if (returnedItems != receipts.size()) {
      throw new IllegalArgumentException("returnedItems must equal receipts.size()");
    }
    SoracloudPrivateUploadedModelExecutionReceipt previous = null;
    for (final SoracloudPrivateUploadedModelExecutionReceipt receipt : receipts) {
      if (receipt == null
          || receipt.authorizationClaimBlockHeight().signum() <= 0
          || receipt.authorizationClaimEpoch().signum() <= 0
          || receipt.emittedSequence().signum() <= 0
          || receipt.emittedBlockHeight().signum() <= 0
          || receipt.emittedEpoch().signum() <= 0) {
        throw new IllegalArgumentException(
            "receipt-list entries must have positive ledger coordinates");
      }
      if (previous != null
          && (previous.emittedSequence().compareTo(receipt.emittedSequence()) > 0
              || (previous.emittedSequence().equals(receipt.emittedSequence())
                  && previous.receiptId().compareTo(receipt.receiptId()) >= 0))) {
        throw new IllegalArgumentException(
            "receipt-list entries must be strictly ordered by emittedSequence and receiptId");
      }
      previous = receipt;
    }
    if (("bounded".equals(countMode)) != (remainingItems == null)) {
      throw new IllegalArgumentException(
          "remainingItems must be null for bounded countMode and non-null for exact countMode");
    }
    if (hasMore != (continueCursor != null)) {
      throw new IllegalArgumentException("hasMore must equal continueCursor presence");
    }
    if (remainingItems != null) {
      if (hasMore != (remainingItems.longValue() > 0L)) {
        throw new IllegalArgumentException("hasMore must equal (remainingItems > 0) in exact mode");
      }
      final long saturatedKnownItems =
          Math.min(U32_MAX, returnedItems + remainingItems.longValue());
      if (total.longValue() < saturatedKnownItems) {
        throw new IllegalArgumentException(
            "total must cover returnedItems and remainingItems in exact mode");
      }
    }
  }

  static String requirePrivateReceiptCursor(final String value, final String field) {
    final String canonical = requireCanonicalString(value, field);
    if (canonical.length() != PRIVATE_RECEIPT_CURSOR_LENGTH_V1) {
      throw new IllegalArgumentException(field + " must be an exact canonical V1 receipt cursor");
    }
    for (int index = 0; index < canonical.length(); index++) {
      final char character = canonical.charAt(index);
      if (!(character >= 'A' && character <= 'Z')
          && !(character >= 'a' && character <= 'z')
          && !(character >= '0' && character <= '9')
          && character != '-'
          && character != '_') {
        throw new IllegalArgumentException(field + " must be an exact canonical V1 receipt cursor");
      }
    }
    return canonical;
  }

  static boolean sameArtifact(
      final SoracloudPrivateModelArtifactRef left,
      final SoracloudPrivateModelArtifactRef right) {
    return left.schemaVersion() == right.schemaVersion()
        && left.ciphertextBytes() == right.ciphertextBytes()
        && Arrays.equals(left.sorafsManifestDigest(), right.sorafsManifestDigest())
        && left.sorafsRootCid().equals(right.sorafsRootCid())
        && left.artifactHash().equals(right.artifactHash())
        && left.artifactRole().equals(right.artifactRole());
  }

  private static boolean isLowOrderX25519PublicKey(final byte[] publicKey) {
    final X25519PublicKeyParameters peer = new X25519PublicKeyParameters(publicKey, 0);
    final X25519PrivateKeyParameters probe =
        new X25519PrivateKeyParameters(LOW_ORDER_X25519_PROBE_PRIVATE_KEY, 0);
    final X25519Agreement agreement = new X25519Agreement();
    final byte[] shared = new byte[32];
    try {
      agreement.init(probe);
      agreement.calculateAgreement(peer, shared, 0);
      for (final byte value : shared) {
        if (value != 0) {
          return false;
        }
      }
      return true;
    } catch (final IllegalStateException ignored) {
      return true;
    } finally {
      Arrays.fill(shared, (byte) 0);
    }
  }

  private static boolean isWhitespace(final char value) {
    return Character.isWhitespace(value) || Character.isSpaceChar(value);
  }

  private static boolean isBidiControl(final char value) {
    return value == '\u061c'
        || value == '\u200e'
        || value == '\u200f'
        || (value >= '\u202a' && value <= '\u202e')
        || (value >= '\u2066' && value <= '\u2069');
  }

  private static byte[] filledBytes(final byte value, final int size) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, value);
    return bytes;
  }
}
