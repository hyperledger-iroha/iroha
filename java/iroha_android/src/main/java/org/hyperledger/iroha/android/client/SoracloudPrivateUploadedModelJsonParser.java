package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Minimal JSON parser for Soracloud private uploaded-model execute and receipt surfaces. */
public final class SoracloudPrivateUploadedModelJsonParser {
  private static final long U32_MAX = SoracloudPrivateModelValidation.U32_MAX;
  private static final Set<String> EXECUTE_RESPONSE_FIELDS =
      fields(
          "schema_version",
          "status",
          "submission_phase",
          "transaction_hash",
          "receipt",
          "output_artifact");
  private static final Set<String> RECEIPT_FIELDS =
      fields(
          "schema_version",
          "network_id",
          "receipt_id",
          "service_name",
          "service_version",
          "model_id",
          "weight_version",
          "runtime_version",
          "model_manifest_digest",
          "model_bundle_root",
          "policy_id",
          "decryption_request_id",
          "attesting_validator",
          "input_artifact",
          "output_artifact",
          "output_replication_order_id",
          "input_commitment",
          "output_commitment",
          "output_recipient",
          "request_commitment",
          "result_commitment",
          "authorization_claim_block_height",
          "authorization_claim_epoch",
          "emitted_sequence",
          "emitted_block_height",
          "emitted_epoch");
  private static final Set<String> ARTIFACT_FIELDS =
      fields(
          "schema_version",
          "sorafs_manifest_digest",
          "sorafs_root_cid",
          "artifact_hash",
          "ciphertext_bytes",
          "artifact_role");
  private static final Set<String> ATTESTING_VALIDATOR_FIELDS =
      fields("lane_id", "validator_account_id", "peer_id");
  private static final Set<String> OUTPUT_RECIPIENT_FIELDS =
      fields(
          "schema_version",
          "key_id",
          "key_version",
          "kem",
          "aead",
          "public_key_bytes",
          "public_key_fingerprint");
  private static final Set<String> KEM_FIELDS = fields("kem", "value");
  private static final Set<String> AEAD_FIELDS = fields("aead", "value");
  private static final Set<String> RECEIPT_LIST_REQUIRED_FIELDS =
      fields(
          "schema_version",
          "receipts",
          "total",
          "returned_items",
          "remaining_items",
          "has_more",
          "count_mode",
          "continue_cursor");
  private static final Set<String> RECEIPT_LIST_ALLOWED_FIELDS =
      fields(
          "schema_version",
          "receipts",
          "total",
          "returned_items",
          "remaining_items",
          "has_more",
          "count_mode",
          "continue_cursor");

  private SoracloudPrivateUploadedModelJsonParser() {}

  public static SoracloudPrivateUploadedModelExecuteResponse parseExecuteResponse(
      final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "soracloud private execute response"), "soracloud private execute response");
    requireFields(
        root,
        EXECUTE_RESPONSE_FIELDS,
        EXECUTE_RESPONSE_FIELDS,
        "soracloud private execute response");
    final SoracloudPrivateUploadedModelSubmissionPhase submissionPhase =
        submissionPhase(
            root.get("submission_phase"),
            "soracloud private execute response.submission_phase");
    final String transactionHash =
        optionalHash(
            root.get("transaction_hash"),
            "soracloud private execute response.transaction_hash");
    final SoracloudPrivateUploadedModelExecutionReceipt receipt =
        parseReceipt(
            expectObject(root.get("receipt"), "soracloud private execute response.receipt"),
            "soracloud private execute response.receipt");
    final SoracloudPrivateModelArtifactRef outputArtifact =
        parseArtifact(
            expectObject(
                root.get("output_artifact"),
                "soracloud private execute response.output_artifact"),
            "soracloud private execute response.output_artifact",
            "output");
    final Map<String, Object> status =
        expectObject(root.get("status"), "soracloud private execute response.status");
    try {
      SoracloudPrivateModelValidation.validateUploadedModelStatus(
          status, "soracloud private execute response.status");
      SoracloudPrivateModelValidation.requireUploadedModelStatusMatchesReceipt(
          status, receipt, "soracloud private execute response.status");
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(error.getMessage(), error);
    }
    return new SoracloudPrivateUploadedModelExecuteResponse(
        schemaVersion(
            root.get("schema_version"), "soracloud private execute response.schema_version"),
        status,
        submissionPhase,
        transactionHash,
        receipt,
        outputArtifact);
  }

  public static SoracloudPrivateUploadedModelReceiptListResponse parseReceiptList(
      final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "soracloud private receipt list"), "soracloud private receipt list");
    requireFields(
        root,
        RECEIPT_LIST_ALLOWED_FIELDS,
        RECEIPT_LIST_REQUIRED_FIELDS,
        "soracloud private receipt list");
    final List<Object> receiptValues =
        asArray(root.get("receipts"), "soracloud private receipt list.receipts");
    final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts =
        new ArrayList<>(receiptValues.size());
    for (int i = 0; i < receiptValues.size(); i++) {
      final String path = "soracloud private receipt list.receipts[" + i + "]";
      receipts.add(parseReceipt(expectObject(receiptValues.get(i), path), path));
    }
    final Long total =
        asOptionalBoundedLong(
            root.get("total"), "soracloud private receipt list.total", 0L, U32_MAX);
    final String countMode =
        requiredString(root.get("count_mode"), "soracloud private receipt list.count_mode");
    final long returnedItems =
        boundedLong(
            root.get("returned_items"),
            "soracloud private receipt list.returned_items",
            0L,
            U32_MAX);
    final Long remainingItems =
        asOptionalBoundedLong(
            root.get("remaining_items"),
            "soracloud private receipt list.remaining_items",
            0L,
            U32_MAX);
    final boolean hasMore =
        asBoolean(root.get("has_more"), "soracloud private receipt list.has_more");
    return new SoracloudPrivateUploadedModelReceiptListResponse(
        schemaVersion(root.get("schema_version"), "soracloud private receipt list.schema_version"),
        receipts,
        total,
        returnedItems,
        remainingItems,
        hasMore,
        countMode,
        optionalNonBlankString(
            root.get("continue_cursor"), "soracloud private receipt list.continue_cursor"));
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt parseReceipt(
      final Map<String, Object> root, final String context) {
    requireFields(root, RECEIPT_FIELDS, RECEIPT_FIELDS, context);
    final BigInteger emittedSequence =
        unsigned64Integer(root.get("emitted_sequence"), context + ".emitted_sequence");
    final BigInteger emittedBlockHeight =
        unsigned64Integer(root.get("emitted_block_height"), context + ".emitted_block_height");
    final BigInteger emittedEpoch =
        unsigned64Integer(root.get("emitted_epoch"), context + ".emitted_epoch");
    final BigInteger authorizationClaimBlockHeight =
        unsigned64Integer(
            root.get("authorization_claim_block_height"),
            context + ".authorization_claim_block_height");
    final BigInteger authorizationClaimEpoch =
        unsigned64Integer(
            root.get("authorization_claim_epoch"), context + ".authorization_claim_epoch");
    final SoracloudPrivateModelArtifactRef outputArtifact =
        parseArtifact(
            expectObject(root.get("output_artifact"), context + ".output_artifact"),
            context + ".output_artifact",
            "output");
    final byte[] outputReplicationOrderId =
        fixedBytes32(
            root.get("output_replication_order_id"),
            context + ".output_replication_order_id");
    try {
      SoracloudPrivateModelValidation.requireSorafsAutoReplicationOrderIdV1(
          outputReplicationOrderId,
          outputArtifact.sorafsManifestDigest(),
          context + ".output_replication_order_id");
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(error.getMessage(), error);
    }
    return new SoracloudPrivateUploadedModelExecutionReceipt(
        schemaVersion(root.get("schema_version"), context + ".schema_version"),
        networkId(root.get("network_id"), context + ".network_id"),
        requiredHash(root.get("receipt_id"), context + ".receipt_id"),
        requiredString(root.get("service_name"), context + ".service_name"),
        requiredString(root.get("service_version"), context + ".service_version"),
        requiredString(root.get("model_id"), context + ".model_id"),
        requiredString(root.get("weight_version"), context + ".weight_version"),
        requiredString(root.get("runtime_version"), context + ".runtime_version"),
        fixedBytes32(root.get("model_manifest_digest"), context + ".model_manifest_digest"),
        requiredHash(root.get("model_bundle_root"), context + ".model_bundle_root"),
        requiredString(root.get("policy_id"), context + ".policy_id"),
        requiredString(
            root.get("decryption_request_id"), context + ".decryption_request_id"),
        parseAttestingValidator(
            expectObject(root.get("attesting_validator"), context + ".attesting_validator"),
            context + ".attesting_validator"),
        parseArtifact(
            expectObject(root.get("input_artifact"), context + ".input_artifact"),
            context + ".input_artifact",
            "input"),
        outputArtifact,
        outputReplicationOrderId,
        requiredHash(root.get("input_commitment"), context + ".input_commitment"),
        requiredHash(root.get("output_commitment"), context + ".output_commitment"),
        parseOutputRecipient(
            expectObject(root.get("output_recipient"), context + ".output_recipient"),
            context + ".output_recipient"),
        requiredHash(root.get("request_commitment"), context + ".request_commitment"),
        requiredHash(root.get("result_commitment"), context + ".result_commitment"),
        authorizationClaimBlockHeight,
        authorizationClaimEpoch,
        emittedSequence,
        emittedBlockHeight,
        emittedEpoch);
  }

  private static SoracloudPrivateModelArtifactRef parseArtifact(
      final Map<String, Object> root, final String context, final String requiredRole) {
    requireFields(root, ARTIFACT_FIELDS, ARTIFACT_FIELDS, context);
    final String artifactRole = requiredString(root.get("artifact_role"), context + ".artifact_role");
    if (!requiredRole.equals(artifactRole)) {
      throw new IllegalStateException(
          context + ".artifact_role must equal `" + requiredRole + "`");
    }
    return new SoracloudPrivateModelArtifactRef(
        schemaVersion(root.get("schema_version"), context + ".schema_version"),
        fixedBytes32(root.get("sorafs_manifest_digest"), context + ".sorafs_manifest_digest"),
        sorafsRootCid(root.get("sorafs_root_cid"), context + ".sorafs_root_cid"),
        requiredHash(root.get("artifact_hash"), context + ".artifact_hash"),
        boundedLong(
            root.get("ciphertext_bytes"),
            context + ".ciphertext_bytes",
            1L,
            SoracloudPrivateModelValidation.ENCRYPTED_ARTIFACT_MAX_BYTES),
        artifactRole);
  }

  private static SoracloudRuntimeDeterministicValidatorHost parseAttestingValidator(
      final Map<String, Object> root, final String context) {
    requireFields(
        root, ATTESTING_VALIDATOR_FIELDS, ATTESTING_VALIDATOR_FIELDS, context);
    final String validatorAccountId =
        requiredString(root.get("validator_account_id"), context + ".validator_account_id");
    final String peerId = requiredString(root.get("peer_id"), context + ".peer_id");
    return new SoracloudRuntimeDeterministicValidatorHost(
        boundedLong(root.get("lane_id"), context + ".lane_id", 0L, U32_MAX),
        validatorAccountId,
        peerId);
  }

  private static SoracloudUploadedModelEncryptionRecipient parseOutputRecipient(
      final Map<String, Object> root, final String context) {
    requireFields(root, OUTPUT_RECIPIENT_FIELDS, OUTPUT_RECIPIENT_FIELDS, context);
    final String kem =
        parseUnitSuite(
            expectObject(root.get("kem"), context + ".kem"),
            KEM_FIELDS,
            "kem",
            SoracloudPrivateModelValidation.X25519_HKDF_SHA256,
            context + ".kem");
    final String aead =
        parseUnitSuite(
            expectObject(root.get("aead"), context + ".aead"),
            AEAD_FIELDS,
            "aead",
            SoracloudPrivateModelValidation.AES_256_GCM,
            context + ".aead");
    final String publicKeyBytesBase64 =
        requiredString(root.get("public_key_bytes"), context + ".public_key_bytes");
    final String publicKeyFingerprint =
        requiredHash(
            root.get("public_key_fingerprint"), context + ".public_key_fingerprint");
    return new SoracloudUploadedModelEncryptionRecipient(
        schemaVersion(root.get("schema_version"), context + ".schema_version"),
        requiredString(root.get("key_id"), context + ".key_id"),
        boundedLong(root.get("key_version"), context + ".key_version", 1L, U32_MAX),
        kem,
        aead,
        publicKeyBytesBase64,
        publicKeyFingerprint);
  }

  private static String parseUnitSuite(
      final Map<String, Object> root,
      final Set<String> fields,
      final String tag,
      final String expected,
      final String context) {
    requireFields(root, fields, fields, context);
    final String actual = requiredString(root.get(tag), context + "." + tag);
    if (!expected.equals(actual)) {
      throw new IllegalStateException(context + "." + tag + " must equal `" + expected + "`");
    }
    if (root.get("value") != null) {
      throw new IllegalStateException(context + ".value must be null");
    }
    return actual;
  }

  private static Object parse(final byte[] payload, final String context) {
    if (payload == null || payload.length == 0) {
      throw new IllegalStateException(context + " returned an empty payload");
    }
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException(context + " returned a blank payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asArray(final Object value, final String path) {
    if (!(value instanceof List<?>)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) value;
  }

  private static String requiredString(final Object value, final String path) {
    if (!(value instanceof String)) {
      throw new IllegalStateException(path + " must be a string");
    }
    final String string = (String) value;
    try {
      return SoracloudPrivateModelValidation.requireCanonicalString(string, path);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(error.getMessage(), error);
    }
  }

  private static String optionalNonBlankString(final Object value, final String path) {
    return value == null ? null : requiredString(value, path);
  }

  private static String requiredHash(final Object value, final String path) {
    if (!(value instanceof String)) {
      throw new IllegalStateException(path + " must be a canonical checksummed Iroha hash");
    }
    final String hash = (String) value;
    try {
      return SoracloudPrivateModelValidation.requireSoracloudHash(hash, path);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(error.getMessage(), error);
    }
  }

  private static String optionalHash(final Object value, final String path) {
    return value == null ? null : requiredHash(value, path);
  }

  private static long asLong(final Object value, final String path) {
    return JsonNumbers.asLong(value, path);
  }

  private static BigInteger unsigned64Integer(final Object value, final String path) {
    final BigInteger parsed = JsonNumbers.asBigInteger(value, path);
    if (parsed.signum() < 0
        || parsed.compareTo(SoracloudPrivateModelValidation.U64_MAX) > 0) {
      throw new IllegalStateException(path + " must fit in unsigned 64-bit range");
    }
    return parsed;
  }

  private static long boundedLong(
      final Object value, final String path, final long minimum, final long maximum) {
    final long parsed = asLong(value, path);
    if (parsed < minimum || parsed > maximum) {
      throw new IllegalStateException(
          path + " must be within " + minimum + "..=" + maximum);
    }
    return parsed;
  }

  private static Long asOptionalBoundedLong(
      final Object value,
      final String path,
      final long minimum,
      final long maximum) {
    return value == null ? null : Long.valueOf(boundedLong(value, path, minimum, maximum));
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (value instanceof Boolean) {
      return ((Boolean) value).booleanValue();
    }
    throw new IllegalStateException(path + " must be a boolean");
  }

  private static long schemaVersion(final Object value, final String path) {
    final long parsed = asLong(value, path);
    try {
      return SoracloudPrivateModelValidation.requireSchemaVersion(parsed, path);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(error.getMessage(), error);
    }
  }

  private static String networkId(final Object value, final String path) {
    final String literal = requiredString(value, path);
    try {
      return SoracloudPrivateModelValidation.requireNetworkId(literal, path);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(
          path + " must be an exact canonical checksummed 32-byte NetworkId literal", error);
    }
  }

  private static List<Integer> sorafsRootCid(final Object value, final String path) {
    final List<Object> values = asArray(value, path);
    if (values.size() != 36) {
      throw new IllegalStateException(path + " must contain exactly 36 unsigned integer bytes");
    }
    final ArrayList<Integer> bytes = new ArrayList<>(values.size());
    boolean nonzeroDigest = false;
    for (int index = 0; index < values.size(); index++) {
      final int element = (int) boundedLong(values.get(index), path + "[" + index + "]", 0L, 255L);
      bytes.add(Integer.valueOf(element));
      if (index >= 4 && element != 0) {
        nonzeroDigest = true;
      }
    }
    if (bytes.get(0).intValue() != 1
        || bytes.get(1).intValue() != 0x71
        || bytes.get(2).intValue() != 0x1f
        || bytes.get(3).intValue() != 32) {
      throw new IllegalStateException(
          path + " must use canonical CIDv1/dag-cbor/BLAKE3-256 framing");
    }
    if (!nonzeroDigest) {
      throw new IllegalStateException(path + " digest must be nonzero");
    }
    return Collections.unmodifiableList(bytes);
  }

  private static byte[] fixedBytes32(final Object value, final String path) {
    final List<Object> values = asArray(value, path);
    if (values.size() != 32) {
      throw new IllegalStateException(path + " must contain exactly 32 unsigned integer bytes");
    }
    final byte[] bytes = new byte[32];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] =
          (byte) boundedLong(values.get(index), path + "[" + index + "]", 0L, 255L);
    }
    return bytes;
  }

  private static SoracloudPrivateUploadedModelSubmissionPhase submissionPhase(
      final Object value, final String path) {
    final String parsed = requiredString(value, path);
    try {
      return SoracloudPrivateUploadedModelSubmissionPhase.fromWireValue(parsed);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(path + " has an unknown first-release phase", error);
    }
  }

  private static Set<String> fields(final String... values) {
    return Collections.unmodifiableSet(new HashSet<>(Arrays.asList(values)));
  }

  private static void requireFields(
      final Map<String, Object> root,
      final Set<String> allowed,
      final Set<String> required,
      final String path) {
    for (final String field : root.keySet()) {
      if (!allowed.contains(field)) {
        throw new IllegalStateException(path + " contains unknown field `" + field + "`");
      }
    }
    for (final String field : required) {
      if (!root.containsKey(field)) {
        throw new IllegalStateException(path + "." + field + " is missing required field");
      }
    }
  }
}
