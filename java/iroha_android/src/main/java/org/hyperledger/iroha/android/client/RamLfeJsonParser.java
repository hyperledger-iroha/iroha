package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** Minimal JSON parser for RAM-LFE program-policy, execute, and verify payloads. */
public final class RamLfeJsonParser {

  private RamLfeJsonParser() {}

  public static RamLfeProgramPolicyListResponse parsePolicyList(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "ram-lfe program policy list"), "ram-lfe program policy list");
    final List<Object> itemValues =
        asArrayOrEmpty(root.get("items"), "ram-lfe program policy list.items");
    final List<RamLfeProgramPolicySummary> items = new ArrayList<>(itemValues.size());
    for (int i = 0; i < itemValues.size(); i++) {
      final Map<String, Object> item =
          expectObject(itemValues.get(i), "ram-lfe program policy list.items[" + i + "]");
      items.add(
          new RamLfeProgramPolicySummary(
              requiredString(
                  item.get("program_id"),
                  "ram-lfe program policy list.items[" + i + "].program_id"),
              requiredString(
                  item.get("owner"), "ram-lfe program policy list.items[" + i + "].owner"),
              Boolean.TRUE.equals(item.get("active")),
              requiredString(
                  item.get("resolver_public_key"),
                  "ram-lfe program policy list.items[" + i + "].resolver_public_key"),
              requiredString(
                  item.get("backend"), "ram-lfe program policy list.items[" + i + "].backend"),
              normalizedMode(
                  requiredString(
                      item.get("verification_mode"),
                      "ram-lfe program policy list.items[" + i + "].verification_mode")),
              optionalString(item.get("input_encryption")),
              optionalString(item.get("input_encryption_public_parameters")),
              item.get("input_encryption_public_parameters_decoded") == null
                  ? null
                  : parseBfvPublicParameters(
                      expectObject(
                          item.get("input_encryption_public_parameters_decoded"),
                          "ram-lfe program policy list.items["
                              + i
                              + "].input_encryption_public_parameters_decoded"),
                      "ram-lfe program policy list.items["
                          + i
                          + "].input_encryption_public_parameters_decoded"),
              optionalString(item.get("note"))));
    }
    final long total =
        root.containsKey("total")
            ? asLong(root.get("total"), "ram-lfe program policy list.total")
            : items.size();
    return new RamLfeProgramPolicyListResponse(total, items);
  }

  public static RamLfeExecuteResponse parseExecuteResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "ram-lfe execute response"), "ram-lfe execute response");
    return new RamLfeExecuteResponse(
        requiredString(root.get("program_id"), "ram-lfe execute response.program_id"),
        requiredString(root.get("opaque_hash"), "ram-lfe execute response.opaque_hash"),
        requiredString(root.get("receipt_hash"), "ram-lfe execute response.receipt_hash"),
        canonicalizeHex(
            requiredString(root.get("output_hex"), "ram-lfe execute response.output_hex"),
            "ram-lfe execute response.output_hex"),
        requiredString(root.get("output_hash"), "ram-lfe execute response.output_hash"),
        requiredString(
            root.get("associated_data_hash"), "ram-lfe execute response.associated_data_hash"),
        asLong(root.get("executed_at_ms"), "ram-lfe execute response.executed_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), "ram-lfe execute response.expires_at_ms")
            : null,
        requiredString(root.get("backend"), "ram-lfe execute response.backend"),
        normalizedMode(
            requiredString(
                root.get("verification_mode"), "ram-lfe execute response.verification_mode")),
        expectObject(root.get("receipt"), "ram-lfe execute response.receipt"));
  }

  public static RamLfeReceiptVerifyResponse parseReceiptVerifyResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "ram-lfe receipt verify response"), "ram-lfe receipt verify response");
    return new RamLfeReceiptVerifyResponse(
        Boolean.TRUE.equals(root.get("valid")),
        requiredString(root.get("program_id"), "ram-lfe receipt verify response.program_id"),
        requiredString(root.get("backend"), "ram-lfe receipt verify response.backend"),
        normalizedMode(
            requiredString(
                root.get("verification_mode"),
                "ram-lfe receipt verify response.verification_mode")),
        requiredString(root.get("output_hash"), "ram-lfe receipt verify response.output_hash"),
        requiredString(
            root.get("associated_data_hash"),
            "ram-lfe receipt verify response.associated_data_hash"),
        root.containsKey("output_hash_matches")
            ? asOptionalBoolean(
                root.get("output_hash_matches"),
                "ram-lfe receipt verify response.output_hash_matches")
            : null,
        optionalString(root.get("error")));
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
      return List.of();
    }
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " must be a JSON array");
    }
    return (List<Object>) list;
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
    return value instanceof String string ? string : String.valueOf(value);
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " must be a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static Long asOptionalLong(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return asLong(value, path);
  }

  private static Boolean asOptionalBoolean(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (value instanceof Boolean bool) {
      return bool;
    }
    throw new IllegalStateException(path + " must be a boolean");
  }

  private static String canonicalizeHex(final String value, final String context) {
    String trimmed = value.trim();
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) == 1 || !trimmed.matches("(?i)[0-9a-f]+")) {
      throw new IllegalArgumentException(context + " must contain an even number of hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }

  private static String normalizedMode(final String value) {
    return value.trim().toLowerCase(Locale.ROOT);
  }

  private static IdentifierBfvPublicParameters parseBfvPublicParameters(
      final Map<String, Object> root, final String context) {
    final Map<String, Object> parameters =
        expectObject(root.get("parameters"), context + ".parameters");
    final Map<String, Object> publicKey =
        expectObject(root.get("public_key"), context + ".public_key");
    return new IdentifierBfvPublicParameters(
        new IdentifierBfvPublicParameters.Parameters(
            asLong(parameters.get("polynomial_degree"), context + ".parameters.polynomial_degree"),
            asLong(parameters.get("plaintext_modulus"), context + ".parameters.plaintext_modulus"),
            asLong(
                parameters.get("ciphertext_modulus"),
                context + ".parameters.ciphertext_modulus"),
            Math.toIntExact(
                asLong(
                    parameters.get("decomposition_base_log"),
                    context + ".parameters.decomposition_base_log"))),
        new IdentifierBfvPublicParameters.PublicKey(
            asLongList(publicKey.get("b"), context + ".public_key.b"),
            asLongList(publicKey.get("a"), context + ".public_key.a")),
        Math.toIntExact(asLong(root.get("max_input_bytes"), context + ".max_input_bytes")));
  }

  private static List<Long> asLongList(final Object value, final String path) {
    final List<Object> values = asArrayOrEmpty(value, path);
    final List<Long> normalized = new ArrayList<>(values.size());
    for (int i = 0; i < values.size(); i++) {
      normalized.add(asLong(values.get(i), path + "[" + i + "]"));
    }
    return normalized;
  }
}
