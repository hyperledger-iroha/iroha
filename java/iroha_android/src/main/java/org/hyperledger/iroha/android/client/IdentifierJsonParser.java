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
              requiredString(item.get("owner"), "identifier policy list.items[" + i + "].owner"),
              Boolean.TRUE.equals(item.get("active")),
              IdentifierNormalization.fromWireValue(
                  requiredString(
                      item.get("normalization"),
                      "identifier policy list.items[" + i + "].normalization")),
              requiredString(
                  item.get("resolver_public_key"),
                  "identifier policy list.items[" + i + "].resolver_public_key"),
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
              optionalString(item.get("note"))));
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
    final String policyId =
        requiredString(root.get("policy_id"), "identifier resolution receipt.policy_id");
    final String opaqueId =
        canonicalizeOpaque(
            requiredString(root.get("opaque_id"), "identifier resolution receipt.opaque_id"),
            "identifier resolution receipt.opaque_id");
    final String receiptHash =
        canonicalizeHex32(
            requiredString(root.get("receipt_hash"), "identifier resolution receipt.receipt_hash"),
            "identifier resolution receipt.receipt_hash");
    final String uaid =
        UaidLiteral.canonicalize(
            requiredString(root.get("uaid"), "identifier resolution receipt.uaid"),
            "identifier resolution receipt.uaid");
    final String accountId =
        requiredString(root.get("account_id"), "identifier resolution receipt.account_id");
    final long resolvedAtMs =
        asLong(root.get("resolved_at_ms"), "identifier resolution receipt.resolved_at_ms");
    final Long expiresAtMs =
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), "identifier resolution receipt.expires_at_ms")
            : null;
    final String backend =
        requiredString(root.get("backend"), "identifier resolution receipt.backend");
    final String signature =
        requiredString(root.get("signature"), "identifier resolution receipt.signature");
    final String signaturePayloadHex =
        canonicalizeHex(
            requiredString(
                root.get("signature_payload_hex"),
                "identifier resolution receipt.signature_payload_hex"),
            "identifier resolution receipt.signature_payload_hex");
    return new IdentifierResolutionReceipt(
        policyId,
        opaqueId,
        receiptHash,
        uaid,
        accountId,
        resolvedAtMs,
        expiresAtMs,
        backend,
        signature,
        signaturePayloadHex,
        parseResolutionPayload(
            expectObject(
                root.get("signature_payload"),
                "identifier resolution receipt.signature_payload"),
            "identifier resolution receipt.signature_payload",
            policyId,
            opaqueId,
            receiptHash,
            uaid,
            accountId,
            resolvedAtMs,
            expiresAtMs));
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

  private static IdentifierResolutionPayload parseResolutionPayload(
      final Map<String, Object> root,
      final String context,
      final String fallbackPolicyId,
      final String fallbackOpaqueId,
      final String fallbackReceiptHash,
      final String fallbackUaid,
      final String fallbackAccountId,
      final long fallbackResolvedAtMs,
      final Long fallbackExpiresAtMs) {
    if (root.get("policy_id") instanceof Map<?, ?> || root.containsKey("execution")) {
      return parseStructuredResolutionPayload(
          root,
          context,
          fallbackPolicyId,
          fallbackOpaqueId,
          fallbackReceiptHash,
          fallbackUaid,
          fallbackAccountId,
          fallbackResolvedAtMs,
          fallbackExpiresAtMs);
    }
    return new IdentifierResolutionPayload(
        requiredString(root.get("policy_id"), context + ".policy_id"),
        canonicalizeOpaque(
            requiredString(root.get("opaque_id"), context + ".opaque_id"),
            context + ".opaque_id"),
        canonicalizeHex32(
            requiredString(root.get("receipt_hash"), context + ".receipt_hash"),
            context + ".receipt_hash"),
        UaidLiteral.canonicalize(
            requiredString(root.get("uaid"), context + ".uaid"), context + ".uaid"),
        requiredString(root.get("account_id"), context + ".account_id"),
        asLong(root.get("resolved_at_ms"), context + ".resolved_at_ms"),
        root.containsKey("expires_at_ms")
            ? asOptionalLong(root.get("expires_at_ms"), context + ".expires_at_ms")
            : null);
  }

  private static IdentifierResolutionPayload parseStructuredResolutionPayload(
      final Map<String, Object> root,
      final String context,
      final String fallbackPolicyId,
      final String fallbackOpaqueId,
      final String fallbackReceiptHash,
      final String fallbackUaid,
      final String fallbackAccountId,
      final long fallbackResolvedAtMs,
      final Long fallbackExpiresAtMs) {
    return new IdentifierResolutionPayload(
        parseStructuredPolicyId(root.get("policy_id"), context + ".policy_id", fallbackPolicyId),
        parseStructuredOpaqueId(root.get("opaque_id"), context + ".opaque_id", fallbackOpaqueId),
        parseStructuredReceiptHash(
            root.get("receipt_hash"), context + ".receipt_hash", fallbackReceiptHash),
        parseStructuredUaid(root.get("uaid"), context + ".uaid", fallbackUaid),
        root.containsKey("account_id")
            ? requiredString(root.get("account_id"), context + ".account_id")
            : fallbackAccountId,
        parseStructuredResolvedAtMs(root, context, fallbackResolvedAtMs),
        parseStructuredExpiresAtMs(root, context, fallbackExpiresAtMs));
  }

  private static String parseStructuredPolicyId(
      final Object value, final String context, final String fallback) {
    if (value == null) {
      return fallback;
    }
    if (value instanceof String) {
      return requiredString(value, context);
    }
    final Map<String, Object> policy = expectObject(value, context);
    final String kind = requiredString(policy.get("kind"), context + ".kind");
    final String businessRule =
        requiredString(policy.get("business_rule"), context + ".business_rule");
    return kind + "#" + businessRule;
  }

  private static String parseStructuredOpaqueId(
      final Object value, final String context, final String fallback) {
    if (value == null) {
      return fallback;
    }
    if (value instanceof String) {
      return canonicalizeOpaque(requiredString(value, context), context);
    }
    return "opaque:" + canonicalizeHashLiteral(firstArrayString(value, context), context + "[0]");
  }

  private static String parseStructuredReceiptHash(
      final Object value, final String context, final String fallback) {
    if (value == null) {
      return fallback;
    }
    if (value instanceof String) {
      return canonicalizeHex32OrHashLiteral(requiredString(value, context), context);
    }
    return canonicalizeHashLiteral(firstArrayString(value, context), context + "[0]");
  }

  private static String parseStructuredUaid(
      final Object value, final String context, final String fallback) {
    if (value == null) {
      return fallback;
    }
    if (value instanceof String) {
      final String literal = requiredString(value, context);
      if (looksLikeHashLiteral(literal)) {
        return "uaid:" + canonicalizeHashLiteral(literal, context);
      }
      return UaidLiteral.canonicalize(literal, context);
    }
    return "uaid:" + canonicalizeHashLiteral(firstArrayString(value, context), context + "[0]");
  }

  private static long parseStructuredResolvedAtMs(
      final Map<String, Object> root, final String context, final long fallback) {
    if (root.containsKey("resolved_at_ms")) {
      return asLong(root.get("resolved_at_ms"), context + ".resolved_at_ms");
    }
    if (root.containsKey("execution")) {
      final Map<String, Object> execution =
          expectObject(root.get("execution"), context + ".execution");
      if (execution.containsKey("executed_at_ms")) {
        return asLong(execution.get("executed_at_ms"), context + ".execution.executed_at_ms");
      }
    }
    return fallback;
  }

  private static Long parseStructuredExpiresAtMs(
      final Map<String, Object> root, final String context, final Long fallback) {
    if (root.containsKey("expires_at_ms")) {
      return asOptionalLong(root.get("expires_at_ms"), context + ".expires_at_ms");
    }
    if (root.containsKey("execution")) {
      final Map<String, Object> execution =
          expectObject(root.get("execution"), context + ".execution");
      if (execution.containsKey("expires_at_ms")) {
        return asOptionalLong(execution.get("expires_at_ms"), context + ".execution.expires_at_ms");
      }
    }
    return fallback;
  }

  private static String firstArrayString(final Object value, final String context) {
    final List<Object> values = asArrayOrEmpty(value, context);
    if (values.isEmpty()) {
      throw new IllegalStateException(context + " must contain at least one value");
    }
    return requiredString(values.get(0), context + "[0]");
  }

  private static boolean looksLikeHashLiteral(final String value) {
    final String trimmed = value == null ? "" : value.trim();
    final String lower = trimmed.toLowerCase(Locale.ROOT);
    return lower.startsWith("hash:") || trimmed.indexOf('#') >= 0;
  }

  private static String canonicalizeHex32OrHashLiteral(
      final String value, final String context) {
    return looksLikeHashLiteral(value)
        ? canonicalizeHashLiteral(value, context)
        : canonicalizeHex32(value, context);
  }

  private static String canonicalizeHashLiteral(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    String trimmed = Objects.requireNonNull(value, context + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (trimmed.regionMatches(true, 0, "hash:", 0, "hash:".length())) {
      trimmed = trimmed.substring("hash:".length());
    }
    final int suffixIndex = trimmed.indexOf('#');
    if (suffixIndex >= 0) {
      trimmed = trimmed.substring(0, suffixIndex);
    }
    trimmed = trimmed.trim();
    if (trimmed.length() != 64 || !trimmed.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    return trimmed.toLowerCase(Locale.ROOT);
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
