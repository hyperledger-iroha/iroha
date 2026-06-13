package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
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
              requiredExactString(
                  item.get("program_id"),
                  "ram-lfe program policy list.items[" + i + "].program_id"),
              requiredExactString(
                  item.get("owner"), "ram-lfe program policy list.items[" + i + "].owner"),
              Boolean.TRUE.equals(item.get("active")),
              requiredExactString(
                  item.get("resolver_public_key"),
                  "ram-lfe program policy list.items[" + i + "].resolver_public_key"),
              optionalString(item.get("output_opening_public_key")) == null
                  ? requiredExactString(
                      item.get("resolver_public_key"),
                      "ram-lfe program policy list.items[" + i + "].resolver_public_key")
                  : requiredExactString(
                      item.get("output_opening_public_key"),
                      "ram-lfe program policy list.items[" + i + "].output_opening_public_key"),
              requiredExactLowercaseString(
                  item.get("backend"), "ram-lfe program policy list.items[" + i + "].backend"),
              requiredExactLowercaseString(
                  item.get("verification_mode"),
                  "ram-lfe program policy list.items[" + i + "].verification_mode"),
              optionalExactString(
                  item.get("input_encryption"),
                  "ram-lfe program policy list.items[" + i + "].input_encryption"),
              optionalExactHex(
                  item.get("input_encryption_public_parameters"),
                  "ram-lfe program policy list.items["
                      + i
                      + "].input_encryption_public_parameters"),
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
              optionalString(item.get("note")),
              item.get("proof_verifier") == null
                  ? null
                  : parseProofVerifier(
                      expectObject(
                          item.get("proof_verifier"),
                          "ram-lfe program policy list.items[" + i + "].proof_verifier"),
                      "ram-lfe program policy list.items[" + i + "].proof_verifier")));
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
        requiredExactString(root.get("program_id"), "ram-lfe execute response.program_id"),
        canonicalizeExactHash32(root.get("opaque_hash"), "ram-lfe execute response.opaque_hash"),
        canonicalizeExactHash32(root.get("receipt_hash"), "ram-lfe execute response.receipt_hash"),
        canonicalizeExactHex(
            root.get("output_ciphertext"),
            "ram-lfe execute response.output_ciphertext"),
        canonicalizeExactHash32(root.get("output_hash"), "ram-lfe execute response.output_hash"),
        canonicalizeExactHash32(
            root.get("associated_data_hash"), "ram-lfe execute response.associated_data_hash"),
        asLong(root.get("executed_at_ms"), "ram-lfe execute response.executed_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), "ram-lfe execute response.expires_at_ms")
            : null,
        requiredExactLowercaseString(root.get("backend"), "ram-lfe execute response.backend"),
        requiredExactLowercaseString(
            root.get("verification_mode"), "ram-lfe execute response.verification_mode"),
        expectObject(root.get("receipt"), "ram-lfe execute response.receipt"),
        IdentifierJsonParser.parseOutputOpening(
            expectObject(root.get("output_opening"), "ram-lfe execute response.output_opening"),
            "ram-lfe execute response.output_opening"));
  }

  public static RamLfeReceiptVerifyResponse parseReceiptVerifyResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(
            parse(payload, "ram-lfe receipt verify response"), "ram-lfe receipt verify response");
    return new RamLfeReceiptVerifyResponse(
        Boolean.TRUE.equals(root.get("valid")),
        requiredExactString(
            root.get("program_id"), "ram-lfe receipt verify response.program_id"),
        requiredExactLowercaseString(
            root.get("backend"), "ram-lfe receipt verify response.backend"),
        requiredExactLowercaseString(
            root.get("verification_mode"), "ram-lfe receipt verify response.verification_mode"),
        canonicalizeExactHash32(
            root.get("output_hash"), "ram-lfe receipt verify response.output_hash"),
        canonicalizeExactHash32(
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

  private static String requiredExactString(final Object value, final String path) {
    final String string = optionalString(value);
    if (string == null || string.isBlank()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    if (!string.trim().equals(string)) {
      throw new IllegalStateException(path + " must not contain surrounding whitespace");
    }
    return string;
  }

  private static String requiredExactLowercaseString(final Object value, final String path) {
    final String string = requiredExactString(value, path);
    if (!string.toLowerCase(Locale.ROOT).equals(string)) {
      throw new IllegalStateException(path + " must be an exact lowercase string");
    }
    return string;
  }

  private static String optionalExactString(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return requiredExactString(value, path);
  }

  private static String optionalExactHex(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return canonicalizeExactHex(value, path);
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String string ? string : String.valueOf(value);
  }

  private static long asLong(final Object value, final String path) {
    if (value instanceof String string) {
      return new BigInteger(string).longValue();
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

  private static String canonicalizeExactHex(final Object value, final String context) {
    String hex = requiredExactString(value, context);
    if (hex.startsWith("0x") || hex.startsWith("0X")) {
      hex = hex.substring(2);
    }
    if (hex.isEmpty() || (hex.length() & 1) == 1 || !hex.matches("(?i)[0-9a-f]+")) {
      throw new IllegalArgumentException(context + " must contain an even number of hex characters");
    }
    return hex.toLowerCase(Locale.ROOT);
  }

  private static String canonicalizeExactHash32(final Object value, final String context) {
    String body = requiredExactString(value, context);
    if (body.toLowerCase(Locale.ROOT).startsWith("hash:")) {
      body = body.substring("hash:".length());
    }
    final int suffixIndex = body.indexOf('#');
    if (suffixIndex >= 0) {
      body = body.substring(0, suffixIndex);
    }
    if (body.startsWith("0x") || body.startsWith("0X")) {
      body = body.substring(2);
    }
    if (body.length() != 64 || !body.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 32 bytes");
    }
    return body.toLowerCase(Locale.ROOT);
  }

  private static String canonicalizeHex32(final String value, final String context) {
    final String normalized = canonicalizeHex(value, context);
    if (normalized.length() != 64) {
      throw new IllegalArgumentException(context + " must contain 32 bytes");
    }
    return normalized;
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
        Math.toIntExact(asLong(root.get("max_input_bytes"), context + ".max_input_bytes")),
        root.get("norito_length_encoding") instanceof String
            ? (String) root.get("norito_length_encoding")
            : null);
  }

  private static RamLfeProofVerifierMetadata parseProofVerifier(
      final Map<String, Object> root, final String context) {
    return new RamLfeProofVerifierMetadata(
        requiredExactString(root.get("proof_backend"), context + ".proof_backend"),
        requiredExactString(root.get("circuit_id"), context + ".circuit_id"),
        canonicalizeExactHash32(
            root.get("public_inputs_schema_hash"), context + ".public_inputs_schema_hash"),
        requiredExactString(
            root.get("verifying_key_bytes_b64"), context + ".verifying_key_bytes_b64"));
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
