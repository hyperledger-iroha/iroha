package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.nexus.UaidLiteral;

/** Minimal JSON parser for identifier-policy and identifier-resolution payloads. */
public final class IdentifierJsonParser {

  private IdentifierJsonParser() {}

  public static IdentifierPolicyListResponse parsePolicyList(final byte[] payload) {
    final Map<String, Object> root = expectObject(parse(payload, "identifier policy list"), "identifier policy list");
    final List<Object> itemValues = asArrayOrEmpty(root.get("items"), "identifier policy list.items");
    final List<IdentifierPolicySummary> items = new ArrayList<>(itemValues.size());
    for (int i = 0; i < itemValues.size(); i++) {
      final Map<String, Object> item =
          expectObject(itemValues.get(i), "identifier policy list.items[" + i + "]");
      items.add(
          new IdentifierPolicySummary(
              requiredString(item.get("policy_id"), "identifier policy list.items[" + i + "].policy_id"),
              optionalString(item.get("program_id")) == null
                  ? requiredString(item.get("policy_id"), "identifier policy list.items[" + i + "].policy_id")
                      .replace('#', '_')
                  : requiredString(
                      item.get("program_id"),
                      "identifier policy list.items[" + i + "].program_id"),
              requiredString(item.get("owner"), "identifier policy list.items[" + i + "].owner"),
              Boolean.TRUE.equals(item.get("active")),
              IdentifierNormalization.fromWireValue(
                  requiredString(
                      item.get("normalization"),
                      "identifier policy list.items[" + i + "].normalization")),
              requiredString(
                  item.get("resolver_public_key"),
                  "identifier policy list.items[" + i + "].resolver_public_key"),
              optionalString(item.get("output_opening_public_key")) == null
                  ? requiredString(
                      item.get("resolver_public_key"),
                      "identifier policy list.items[" + i + "].resolver_public_key")
                  : requiredString(
                      item.get("output_opening_public_key"),
                      "identifier policy list.items[" + i + "].output_opening_public_key"),
              requiredString(item.get("backend"), "identifier policy list.items[" + i + "].backend"),
              optionalString(item.get("input_encryption")),
              optionalString(item.get("input_encryption_public_parameters")),
	              item.get("input_encryption_public_parameters_decoded") == null
	                  ? null
	                  : parseBfvPublicParameters(
	                      expectObject(
	                          item.get("input_encryption_public_parameters_decoded"),
	                          "identifier policy list.items[" + i + "].input_encryption_public_parameters_decoded"),
	                      "identifier policy list.items[" + i + "].input_encryption_public_parameters_decoded"),
	              optionalString(item.get("note")),
	              item.get("proof_verifier") == null
	                  ? null
	                  : parseProofVerifier(
	                      expectObject(
	                          item.get("proof_verifier"),
	                          "identifier policy list.items[" + i + "].proof_verifier"),
	                      "identifier policy list.items[" + i + "].proof_verifier")));
    }
    final long total =
        root.containsKey("total")
            ? asLong(root.get("total"), "identifier policy list.total")
            : items.size();
    return new IdentifierPolicyListResponse(total, items);
  }

  public static IdentifierResolutionReceipt parseResolutionReceipt(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "identifier resolution receipt"), "identifier resolution receipt");
    return new IdentifierResolutionReceipt(
        parseResolutionPayload(
            expectObject(
                root.get("payload"),
                "identifier resolution receipt.payload"),
            "identifier resolution receipt.payload"),
        parseReceiptAttestation(
            expectObject(
                root.get("attestation"),
                "identifier resolution receipt.attestation"),
            "identifier resolution receipt.attestation"));
  }

  public static IdentifierClaimRecord parseClaimRecord(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "identifier claim record"), "identifier claim record");
    return new IdentifierClaimRecord(
        requiredString(root.get("policy_id"), "identifier claim record.policy_id"),
        canonicalizeOpaque(
            requiredString(root.get("opaque_id"), "identifier claim record.opaque_id"),
            "identifier claim record.opaque_id"),
        canonicalizeHex32(
            requiredString(root.get("receipt_hash"), "identifier claim record.receipt_hash"),
            "identifier claim record.receipt_hash"),
        UaidLiteral.canonicalize(
            requiredString(root.get("uaid"), "identifier claim record.uaid"),
            "identifier claim record.uaid"),
        requiredString(root.get("account_id"), "identifier claim record.account_id"),
        asLong(root.get("verified_at_ms"), "identifier claim record.verified_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), "identifier claim record.expires_at_ms")
            : null);
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

  private static String canonicalizeOpaque(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    final String literal = Objects.requireNonNull(value, context + " must not be null").trim();
    if (literal.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    final String lower = literal.toLowerCase(Locale.ROOT);
    final String hexPortion =
        lower.startsWith("opaque:") ? literal.substring("opaque:".length()) : literal;
    final String trimmedHex = hexPortion.trim();
    if (trimmedHex.length() != 64 || !trimmedHex.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    return "opaque:" + trimmedHex.toLowerCase(Locale.ROOT);
  }

  private static String canonicalizeHex32(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    String trimmed = Objects.requireNonNull(value, context + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (trimmed.toLowerCase(Locale.ROOT).startsWith("hash:")) {
      trimmed = trimmed.substring("hash:".length());
    }
    final int suffixIndex = trimmed.indexOf('#');
    if (suffixIndex >= 0) {
      trimmed = trimmed.substring(0, suffixIndex);
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if (trimmed.length() != 64 || !trimmed.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }

  private static String canonicalizeHex(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    String trimmed = Objects.requireNonNull(value, context + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) == 1 || !trimmed.matches("(?i)[0-9a-f]+")) {
      throw new IllegalArgumentException(context + " must contain an even number of hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
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

  private static RamLfeProofVerifierMetadata parseProofVerifier(
      final Map<String, Object> root, final String context) {
    return new RamLfeProofVerifierMetadata(
        requiredString(root.get("proof_backend"), context + ".proof_backend"),
        requiredString(root.get("circuit_id"), context + ".circuit_id"),
        canonicalizeHex32(
            requiredString(root.get("public_inputs_schema_hash"), context + ".public_inputs_schema_hash"),
            context + ".public_inputs_schema_hash"),
        requiredString(root.get("verifying_key_bytes_b64"), context + ".verifying_key_bytes_b64"));
  }

  private static IdentifierResolutionPayload parseResolutionPayload(
      final Map<String, Object> root, final String context) {
    return new IdentifierResolutionPayload(
        requiredString(root.get("policy_id"), context + ".policy_id"),
        parseResolutionExecutionPayload(
            expectObject(root.get("execution"), context + ".execution"),
            context + ".execution"),
        parseOutputOpening(
            expectObject(root.get("opening"), context + ".opening"),
            context + ".opening"),
        canonicalizeOpaque(
            requiredString(root.get("opaque_id"), context + ".opaque_id"),
            context + ".opaque_id"),
        canonicalizeHex32(
            requiredString(root.get("receipt_hash"), context + ".receipt_hash"),
            context + ".receipt_hash"),
        UaidLiteral.canonicalize(
            requiredString(root.get("uaid"), context + ".uaid"), context + ".uaid"),
        requiredString(root.get("account_id"), context + ".account_id"));
  }

  private static IdentifierResolutionExecutionPayload parseResolutionExecutionPayload(
      final Map<String, Object> root, final String context) {
    return new IdentifierResolutionExecutionPayload(
        requiredString(root.get("program_id"), context + ".program_id"),
        canonicalizeHex32(
            requiredString(root.get("program_digest"), context + ".program_digest"),
            context + ".program_digest"),
        requiredString(root.get("backend"), context + ".backend").toLowerCase(Locale.ROOT),
        requiredString(root.get("verification_mode"), context + ".verification_mode")
            .toLowerCase(Locale.ROOT),
        canonicalizeHex32(
            requiredString(root.get("input_ciphertext_hash"), context + ".input_ciphertext_hash"),
            context + ".input_ciphertext_hash"),
        canonicalizeHex32(
            requiredString(root.get("output_ciphertext_hash"), context + ".output_ciphertext_hash"),
            context + ".output_ciphertext_hash"),
        canonicalizeHex32(
            requiredString(root.get("parameter_digest"), context + ".parameter_digest"),
            context + ".parameter_digest"),
        canonicalizeHex32(
            requiredString(root.get("evaluation_key_digest"), context + ".evaluation_key_digest"),
            context + ".evaluation_key_digest"),
        canonicalizeHex32(
            requiredString(root.get("output_hash"), context + ".output_hash"),
            context + ".output_hash"),
        canonicalizeHex32(
            requiredString(root.get("associated_data_hash"), context + ".associated_data_hash"),
            context + ".associated_data_hash"),
        asLong(root.get("executed_at_ms"), context + ".executed_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), context + ".expires_at_ms")
            : null);
  }

  static RamLfeOutputOpening parseOutputOpening(
      final Map<String, Object> root, final String context) {
    final Map<String, Object> payload = expectObject(root.get("payload"), context + ".payload");
    return new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload(
            requiredString(payload.get("program_id"), context + ".payload.program_id"),
            canonicalizeHex32(
                requiredString(
                    payload.get("input_ciphertext_hash"),
                    context + ".payload.input_ciphertext_hash"),
                context + ".payload.input_ciphertext_hash"),
            canonicalizeHex32(
                requiredString(
                    payload.get("output_ciphertext_hash"),
                    context + ".payload.output_ciphertext_hash"),
                context + ".payload.output_ciphertext_hash"),
            canonicalizeHex32(
                requiredString(payload.get("parameter_digest"), context + ".payload.parameter_digest"),
                context + ".payload.parameter_digest"),
            canonicalizeHex32(
                requiredString(
                    payload.get("evaluation_key_digest"),
                    context + ".payload.evaluation_key_digest"),
                context + ".payload.evaluation_key_digest"),
            canonicalizeHex32(
                requiredString(
                    payload.get("opened_output_hash"),
                    context + ".payload.opened_output_hash"),
                context + ".payload.opened_output_hash"),
            asLong(payload.get("opened_at_ms"), context + ".payload.opened_at_ms"),
            payload.containsKey("expires_at_ms")
                ? asOptionalLong(payload.get("expires_at_ms"), context + ".payload.expires_at_ms")
                : null),
        canonicalizeHex(
            requiredString(root.get("signature"), context + ".signature"),
            context + ".signature"));
  }

  private static IdentifierReceiptAttestation parseReceiptAttestation(
      final Map<String, Object> root, final String context) {
    final String kind = requiredString(root.get("kind"), context + ".kind").toLowerCase(Locale.ROOT);
    switch (kind) {
      case "signed":
        if (root.get("proof_backend") != null || root.get("proof_b64") != null) {
          throw new IllegalStateException(context + " signed attestation must not include proof fields");
        }
        return new IdentifierReceiptAttestation(
            kind,
            canonicalizeHex(
                requiredString(root.get("signature"), context + ".signature"),
                context + ".signature"),
            null,
            null);
      case "proof":
        if (root.get("signature") != null) {
          throw new IllegalStateException(context + " proof attestation must not include signature");
        }
        return new IdentifierReceiptAttestation(
            kind,
            null,
            requiredString(root.get("proof_backend"), context + ".proof_backend"),
            requiredString(root.get("proof_b64"), context + ".proof_b64"));
      default:
        throw new IllegalStateException(context + ".kind must be signed or proof");
    }
  }

  private static List<Long> asLongList(final Object value, final String path) {
    final List<Object> values = asArrayOrEmpty(value, path);
    final List<Long> normalized = new ArrayList<>(values.size());
    for (int index = 0; index < values.size(); index++) {
      normalized.add(asLong(values.get(index), path + "[" + index + "]"));
    }
    return normalized;
  }
}
