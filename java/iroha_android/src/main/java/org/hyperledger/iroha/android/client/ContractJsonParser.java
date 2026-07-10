package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Minimal JSON parser for Torii contract deploy/call responses. */
public final class ContractJsonParser {

  private ContractJsonParser() {}

  public static ContractDeployResponse parseDeployResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "contract deploy response"), "contract deploy response");
    final List<Object> contracts =
        requiredList(root.get("contracts"), "contract deploy response.contracts");
    final List<Object> hajimariCalls =
        requiredList(root.get("hajimari_calls"), "contract deploy response.hajimari_calls");
    final List<Object> assertions =
        requiredList(root.get("assertions"), "contract deploy response.assertions");
    return new ContractDeployResponse(
        Boolean.TRUE.equals(root.get("ok")),
        requiredString(root.get("bundle_name"), "contract deploy response.bundle_name"),
        requiredString(root.get("bundle_digest"), "contract deploy response.bundle_digest"),
        requiredString(root.get("chain_fingerprint"), "contract deploy response.chain_fingerprint"),
        Boolean.TRUE.equals(root.get("dry_run")),
        requiredStringList(root.get("completed_stages"), "contract deploy response.completed_stages"),
        optionalString(root.get("failure_point")),
        contracts.stream()
            .map(
                item -> {
                  final Map<String, Object> contract =
                      expectObject(item, "contract deploy response.contracts[]");
                  return new ContractDeployResponse.ContractReceipt(
                      requiredString(contract.get("name"), "contract deploy response.contracts[].name"),
                      optionalString(contract.get("contract_alias")),
                      optionalString(contract.get("contract_address")),
                      optionalString(contract.get("previous_contract_address")),
                      Boolean.TRUE.equals(contract.get("upgraded")),
                      optionalString(contract.get("dataspace")),
                      contract.containsKey("deploy_nonce")
                          ? asOptionalLong(
                              contract.get("deploy_nonce"),
                              "contract deploy response.contracts[].deploy_nonce")
                          : null,
                      contract.containsKey("tx_hash_hex") && contract.get("tx_hash_hex") != null
                          ? HttpClientTransport.normalizeHex32(
                              requiredString(
                                  contract.get("tx_hash_hex"),
                                  "contract deploy response.contracts[].tx_hash_hex"),
                              "txHashHex")
                          : null,
                      optionalObject(
                          contract.get("pipeline_status"),
                          "contract deploy response.contracts[].pipeline_status"),
                      HttpClientTransport.normalizeHex32(
                          requiredString(
                              contract.get("code_hash_hex"),
                              "contract deploy response.contracts[].code_hash_hex"),
                          "codeHashHex"),
                      HttpClientTransport.normalizeHex32(
                          requiredString(
                              contract.get("abi_hash_hex"),
                              "contract deploy response.contracts[].abi_hash_hex"),
                          "abiHashHex"),
                      requiredString(
                          contract.get("status"),
                          "contract deploy response.contracts[].status"));
                })
            .collect(Collectors.toList()),
        hajimariCalls.stream()
            .map(
                item -> {
                  final Map<String, Object> call =
                      expectObject(item, "contract deploy response.hajimari_calls[]");
                  return new ContractDeployResponse.HajimariCallReceipt(
                      requiredString(call.get("id"), "contract deploy response.hajimari_calls[].id"),
                      optionalString(call.get("contract_alias")),
                      optionalString(call.get("entrypoint")),
                      call.containsKey("tx_hash_hex") && call.get("tx_hash_hex") != null
                          ? HttpClientTransport.normalizeHex32(
                              requiredString(
                                  call.get("tx_hash_hex"),
                                  "contract deploy response.hajimari_calls[].tx_hash_hex"),
                              "txHashHex")
                          : null,
                      optionalObject(
                          call.get("pipeline_status"),
                          "contract deploy response.hajimari_calls[].pipeline_status"),
                      requiredString(
                          call.get("status"),
                          "contract deploy response.hajimari_calls[].status"));
                })
            .collect(Collectors.toList()),
        assertions.stream()
            .map(
                item -> {
                  final Map<String, Object> assertion =
                      expectObject(item, "contract deploy response.assertions[]");
                  return new ContractDeployResponse.AssertionReceipt(
                      requiredString(assertion.get("id"), "contract deploy response.assertions[].id"),
                      optionalString(assertion.get("contract_alias")),
                      optionalString(assertion.get("entrypoint")),
                      requiredString(
                          assertion.get("status"),
                          "contract deploy response.assertions[].status"),
                      assertion.get("actual_result"),
                      assertion.get("expected_result"),
                      optionalString(assertion.get("error")));
                })
            .collect(Collectors.toList()));
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
        optionalObject(root.get("pipeline_status"), "contract call response.pipeline_status"),
        optionalString(root.get("entrypoint")),
        asOptionalNonNegativeLong(
            root.get("transaction_ttl_ms"), "contract call response.transaction_ttl_ms"),
        optionalHash(root.get("entrypoint_hash_hex"), "contract call response.entrypoint_hash_hex"),
        optionalBase64(root.get("transaction_scaffold_b64"), "contract call response.transaction_scaffold_b64"),
        optionalBase64(root.get("signed_transaction_b64"), "contract call response.signed_transaction_b64"),
        optionalBase64(root.get("signing_message_b64"), "contract call response.signing_message_b64"),
        parseOperationReceipt(
            expectObject(root.get("operation_receipt"), "contract call response.operation_receipt"),
            "contract call response.operation_receipt"));
  }

  private static ContractOperationReceipt parseOperationReceipt(
      final Map<String, Object> receipt, final String path) {
    final Long gasLimit = asOptionalNonNegativeLong(receipt.get("gas_limit"), path + ".gas_limit");
    if (gasLimit != null && gasLimit.longValue() == 0L) {
      throw new IllegalStateException(path + ".gas_limit must be positive");
    }
    return new ContractOperationReceipt(
        requiredString(receipt.get("operation_kind"), path + ".operation_kind"),
        requiredString(receipt.get("status"), path + ".status"),
        requiredString(receipt.get("transport"), path + ".transport"),
        requiredString(receipt.get("dataspace"), path + ".dataspace"),
        optionalString(receipt.get("contract_alias")),
        optionalString(receipt.get("contract_address")),
        optionalHash(receipt.get("code_hash_hex"), path + ".code_hash_hex"),
        optionalHash(receipt.get("abi_hash_hex"), path + ".abi_hash_hex"),
        optionalHash(receipt.get("tx_hash_hex"), path + ".tx_hash_hex"),
        optionalString(receipt.get("entrypoint")),
        optionalHash(receipt.get("entrypoint_hash_hex"), path + ".entrypoint_hash_hex"),
        gasLimit,
        asOptionalNonNegativeLong(receipt.get("gas_used"), path + ".gas_used"),
        optionalString(receipt.get("gas_asset_id")),
        optionalString(receipt.get("fee_sponsor")),
        HttpClientTransport.normalizeHex32(
            requiredString(receipt.get("payload_digest_hex"), path + ".payload_digest_hex"),
            "payloadDigestHex"));
  }

  public static MultisigResponse parseMultisigResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "multisig response"), "multisig response");
    if (!Boolean.TRUE.equals(root.get("ok"))) {
      throw new IllegalStateException("multisig response.ok must be true");
    }
    return new MultisigResponse(
        true,
        requiredExactAccountId(root.get("resolved_multisig_account_id"), "multisig response.resolved_multisig_account_id"),
        optionalBoolean(root.get("submitted"), "multisig response.submitted"),
        optionalString(root.get("proposal_id")),
        root.containsKey("instructions_hash") && root.get("instructions_hash") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("instructions_hash"), "multisig response.instructions_hash"),
                "instructionsHash")
            : null,
        root.containsKey("tx_hash_hex") && root.get("tx_hash_hex") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("tx_hash_hex"), "multisig response.tx_hash_hex"),
                "txHashHex")
            : null,
        root.containsKey("executed_tx_hash_hex") && root.get("executed_tx_hash_hex") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("executed_tx_hash_hex"), "multisig response.executed_tx_hash_hex"),
                "executedTxHashHex")
            : null,
        asOptionalNonNegativeLong(root.get("creation_time_ms"), "multisig response.creation_time_ms"),
        optionalBase64(root.get("signing_message_b64"), "multisig response.signing_message_b64"));
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
    if (string == null || string.trim().isEmpty()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return string.trim();
  }

  private static String requiredExactAccountId(final Object value, final String path) {
    final String string = requiredExactString(value, path);
    try {
      return AccountIdLiteral.requireCanonicalI105Address(string, path);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(path + " must be a canonical I105 account id", ex);
    }
  }

  private static String requiredExactString(final Object value, final String path) {
    if (value == null) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    final String string = value instanceof String ? (String) value : String.valueOf(value);
    if (string == null || string.trim().isEmpty()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    if (!string.trim().equals(string)) {
      throw new IllegalStateException(path + " must not contain surrounding whitespace");
    }
    return string;
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    final String string = value instanceof String ? (String) value : String.valueOf(value);
    final String trimmed = string.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static String optionalHash(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return HttpClientTransport.normalizeHex32(requiredString(value, path), path);
  }

  private static Map<String, Object> optionalObject(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return Collections.unmodifiableMap(new LinkedHashMap<>(expectObject(value, path)));
  }

  private static Boolean optionalBoolean(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Boolean)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return (Boolean) value;
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

  private static Long asOptionalNonNegativeLong(final Object value, final String path) {
    final Long parsed = asOptionalLong(value, path);
    if (parsed != null && parsed.longValue() < 0L) {
      throw new IllegalStateException(path + " must be non-negative");
    }
    return parsed;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> requiredList(final Object value, final String path) {
    if (!(value instanceof List<?>)) {
      throw new IllegalStateException(path + " must be an array");
    }
    return (List<Object>) value;
  }

  private static List<String> requiredStringList(final Object value, final String path) {
    return requiredList(value, path).stream()
        .map(item -> requiredString(item, path + "[]"))
        .collect(Collectors.toList());
  }

  private static String optionalBase64(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    final String literal = (value instanceof String ? (String) value : String.valueOf(value)).trim();
    if (literal.isEmpty()) {
      throw new IllegalStateException(path + " must be a non-empty base64 string");
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
