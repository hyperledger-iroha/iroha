package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Map;

/** Minimal JSON parser for Torii contract deploy/call responses. */
public final class ContractJsonParser {

  private ContractJsonParser() {}

  public static ContractDeployResponse parseDeployResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "contract deploy response"), "contract deploy response");
    return new ContractDeployResponse(
        Boolean.TRUE.equals(root.get("ok")),
        optionalString(root.get("contract_alias")),
        optionalString(root.get("contract_address")),
        optionalString(root.get("previous_contract_address")),
        Boolean.TRUE.equals(root.get("upgraded")),
        optionalString(root.get("dataspace")),
        root.containsKey("deploy_nonce")
            ? asOptionalLong(root.get("deploy_nonce"), "contract deploy response.deploy_nonce")
            : null,
        root.containsKey("tx_hash_hex") && root.get("tx_hash_hex") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("tx_hash_hex"), "contract deploy response.tx_hash_hex"),
                "txHashHex")
            : null,
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("code_hash_hex"), "contract deploy response.code_hash_hex"),
            "codeHashHex"),
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("abi_hash_hex"), "contract deploy response.abi_hash_hex"),
            "abiHashHex"));
  }

  public static ContractCallResponse parseCallResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "contract call response"), "contract call response");
    return new ContractCallResponse(
        Boolean.TRUE.equals(root.get("ok")),
        Boolean.TRUE.equals(root.get("submitted")),
        requiredString(root.get("dataspace"), "contract call response.dataspace"),
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("code_hash_hex"), "contract call response.code_hash_hex"),
            "codeHashHex"),
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("abi_hash_hex"), "contract call response.abi_hash_hex"),
            "abiHashHex"),
        asLong(root.get("creation_time_ms"), "contract call response.creation_time_ms"),
        optionalString(root.get("contract_address")),
        root.containsKey("tx_hash_hex") && root.get("tx_hash_hex") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("tx_hash_hex"), "contract call response.tx_hash_hex"),
                "txHashHex")
            : null,
        optionalString(root.get("entrypoint")),
        optionalBase64(root.get("transaction_scaffold_b64"), "contract call response.transaction_scaffold_b64"),
        optionalBase64(root.get("signed_transaction_b64"), "contract call response.signed_transaction_b64"),
        optionalBase64(root.get("signing_message_b64"), "contract call response.signing_message_b64"));
  }

  public static GovernanceContractResponse parseGovernanceContractResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "governance contract response"), "governance contract response");
    return new GovernanceContractResponse(
        Boolean.TRUE.equals(root.get("found")),
        requiredString(root.get("contract_address"), "governance contract response.contract_address"),
        optionalString(root.get("dataspace")),
        root.containsKey("code_hash_hex") && root.get("code_hash_hex") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("code_hash_hex"), "governance contract response.code_hash_hex"),
                "codeHashHex")
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
    final String string = value instanceof String ? (String) value : String.valueOf(value);
    final String trimmed = string.trim();
    return trimmed.isEmpty() ? null : trimmed;
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

  private static String optionalBase64(final Object value, final String path) {
    final String literal = optionalString(value);
    if (literal == null) {
      return null;
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(literal);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(path + " must be valid base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalStateException(path + " must not decode to empty bytes");
    }
    return literal;
  }
}
