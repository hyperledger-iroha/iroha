package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** Minimal JSON parser for Soracloud private uploaded-model execute and receipt surfaces. */
public final class SoracloudPrivateUploadedModelJsonParser {
  public static final String PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID =
      "iroha_data_model::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt";

  private SoracloudPrivateUploadedModelJsonParser() {}

  public static SoracloudPrivateUploadedModelExecuteResponse parseExecuteResponse(
      final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "soracloud private execute response"), "soracloud private execute response");
    return new SoracloudPrivateUploadedModelExecuteResponse(
        asLong(root.get("schema_version"), "soracloud private execute response.schema_version"),
        expectObject(root.get("status"), "soracloud private execute response.status"),
        parseReceipt(
            expectObject(root.get("receipt"), "soracloud private execute response.receipt"),
            "soracloud private execute response.receipt"),
        parseTxInstructions(
            root.get("tx_instructions"),
            "soracloud private execute response.tx_instructions"));
  }

  public static SoracloudPrivateUploadedModelReceiptListResponse parseReceiptList(
      final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "soracloud private receipt list"), "soracloud private receipt list");
    final List<Object> receiptValues =
        asArrayOrEmpty(root.get("receipts"), "soracloud private receipt list.receipts");
    final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts =
        new ArrayList<>(receiptValues.size());
    for (int i = 0; i < receiptValues.size(); i++) {
      receipts.add(
          parseReceipt(
              expectObject(receiptValues.get(i), "soracloud private receipt list.receipts[" + i + "]"),
              "soracloud private receipt list.receipts[" + i + "]"));
    }
    return new SoracloudPrivateUploadedModelReceiptListResponse(
        asLong(root.get("schema_version"), "soracloud private receipt list.schema_version"),
        receipts,
        root.containsKey("total")
            ? asOptionalLong(root.get("total"), "soracloud private receipt list.total")
            : null,
        asLong(root.get("returned_items"), "soracloud private receipt list.returned_items"),
        asLong(root.get("remaining_items"), "soracloud private receipt list.remaining_items"),
        asBoolean(root.get("has_more"), "soracloud private receipt list.has_more"),
        requiredString(root.get("count_mode"), "soracloud private receipt list.count_mode")
            .toLowerCase(Locale.ROOT),
        optionalString(root.get("continue_cursor")));
  }

  public static SoracloudTxInstruction privateUploadedModelReceiptInstruction(
      final List<SoracloudTxInstruction> txInstructions) {
    for (final SoracloudTxInstruction instruction : txInstructions) {
      if (PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID.equals(instruction.wireId())) {
        canonicalizeHex(
            instruction.payloadHex(), "soracloud private receipt instruction.payload_hex");
        return instruction;
      }
    }
    throw new IllegalStateException(
        "missing " + PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID + " instruction skeleton");
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt parseReceipt(
      final Map<String, Object> root, final String context) {
    return new SoracloudPrivateUploadedModelExecutionReceipt(
        asLong(root.get("schema_version"), context + ".schema_version"),
        requiredString(root.get("receipt_id"), context + ".receipt_id"),
        requiredString(root.get("service_name"), context + ".service_name"),
        requiredString(root.get("model_id"), context + ".model_id"),
        requiredString(root.get("weight_version"), context + ".weight_version"),
        requiredString(root.get("runtime_version"), context + ".runtime_version"),
        requiredString(root.get("model_manifest_digest"), context + ".model_manifest_digest"),
        requiredString(root.get("model_bundle_root"), context + ".model_bundle_root"),
        requiredString(root.get("policy_id"), context + ".policy_id"),
        parseArtifact(expectObject(root.get("input_artifact"), context + ".input_artifact"), context + ".input_artifact"),
        parseArtifact(expectObject(root.get("output_artifact"), context + ".output_artifact"), context + ".output_artifact"),
        requiredString(root.get("input_commitment"), context + ".input_commitment"),
        requiredString(root.get("output_commitment"), context + ".output_commitment"),
        requiredString(root.get("request_commitment"), context + ".request_commitment"),
        requiredString(root.get("result_commitment"), context + ".result_commitment"),
        asLong(root.get("emitted_sequence"), context + ".emitted_sequence"));
  }

  private static SoracloudPrivateModelArtifactRef parseArtifact(
      final Map<String, Object> root, final String context) {
    return new SoracloudPrivateModelArtifactRef(
        asLong(root.get("schema_version"), context + ".schema_version"),
        requiredString(root.get("sorafs_manifest_digest"), context + ".sorafs_manifest_digest"),
        requiredString(root.get("artifact_hash"), context + ".artifact_hash"),
        asLong(root.get("ciphertext_bytes"), context + ".ciphertext_bytes"),
        requiredString(root.get("artifact_role"), context + ".artifact_role"));
  }

  private static List<SoracloudTxInstruction> parseTxInstructions(
      final Object value, final String path) {
    final List<Object> values = asArrayOrEmpty(value, path);
    final List<SoracloudTxInstruction> instructions = new ArrayList<>(values.size());
    for (int i = 0; i < values.size(); i++) {
      final Map<String, Object> root = expectObject(values.get(i), path + "[" + i + "]");
      final String payloadHex = requiredString(root.get("payload_hex"), path + "[" + i + "].payload_hex");
      canonicalizeHex(payloadHex, path + "[" + i + "].payload_hex");
      instructions.add(
          new SoracloudTxInstruction(
              requiredString(root.get("wire_id"), path + "[" + i + "].wire_id"), payloadHex));
    }
    return instructions;
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
  private static List<Object> asArrayOrEmpty(final Object value, final String path) {
    if (value == null) {
      return new ArrayList<>();
    }
    if (!(value instanceof List<?>)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) value;
  }

  private static String requiredString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.isBlank()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return string.trim();
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String ? (String) value : String.valueOf(value);
  }

  private static long asLong(final Object value, final String path) {
    if (value instanceof BigInteger bigInteger) {
      try {
        return bigInteger.longValueExact();
      } catch (final ArithmeticException err) {
        throw new IllegalStateException(path + " is outside signed 64-bit range", err);
      }
    }
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static Long asOptionalLong(final Object value, final String path) {
    return value == null ? null : asLong(value, path);
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (value instanceof Boolean bool) {
      return bool.booleanValue();
    }
    throw new IllegalStateException(path + " must be a boolean");
  }

  private static String canonicalizeHex(final String value, final String context) {
    String trimmed = value.trim();
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if (trimmed.isEmpty() || (trimmed.length() & 1) == 1 || !trimmed.matches("(?i)[0-9a-f]+")) {
      throw new IllegalArgumentException(
          context + " must contain a non-empty even number of hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }
}

