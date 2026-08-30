package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;

/** Minimal JSON parser for Torii contract deploy/call responses. */
public final class ContractJsonParser {

  private ContractJsonParser() {}

  /** Parses the complete `/v1/contracts/code/{code_hash}` manifest response. */
  public static ContractManifestRecord parseManifestRecord(final byte[] payload) {
    return ContractManifestJsonParser.parseRecord(payload);
  }

  public static ContractCallResponse parseCallResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "contract call response"), "contract call response");
    rejectRetiredDraftFields(root, "contract call response");
    final ContractCallResponse response = new ContractCallResponse(
        requiredBoolean(root.get("ok"), "contract call response.ok"),
        requiredBoolean(root.get("submitted"), "contract call response.submitted"),
        requiredString(root.get("dataspace"), "contract call response.dataspace"),
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("code_hash_hex"), "contract call response.code_hash_hex"),
            "codeHashHex"),
        HttpClientTransport.normalizeHex32(
            requiredString(root.get("abi_hash_hex"), "contract call response.abi_hash_hex"),
            "abiHashHex"),
        asNonNegativeLong(
            root.get("creation_time_ms"), "contract call response.creation_time_ms"),
        optionalString(root.get("contract_address"), "contract call response.contract_address"),
        root.containsKey("tx_hash_hex") && root.get("tx_hash_hex") != null
            ? transactionHash(root.get("tx_hash_hex"), "contract call response.tx_hash_hex")
            : null,
        optionalObject(root.get("pipeline_status"), "contract call response.pipeline_status"),
        optionalString(root.get("entrypoint"), "contract call response.entrypoint"),
        asOptionalNonNegativeLong(
            root.get("transaction_ttl_ms"), "contract call response.transaction_ttl_ms"),
        optionalTransactionHash(root.get("entrypoint_hash_hex"), "contract call response.entrypoint_hash_hex"),
        optionalBase64(root.get("transaction_payload_b64"), "contract call response.transaction_payload_b64"),
        optionalBase64(root.get("signing_message_b64"), "contract call response.signing_message_b64"),
        parseOperationReceipt(
            expectObject(root.get("operation_receipt"), "contract call response.operation_receipt"),
            "contract call response.operation_receipt"));
    validateUnsignedTransactionState(
        response.submitted(),
        response.txHashHex(),
        null,
        response.creationTimeMs(),
        response.operationReceipt().feePayment(),
        response.transactionPayloadB64(),
        response.signingMessageB64(),
        "contract call response");
    if (!response.submitted()
        && (response.entrypointHashHex() != null
            || response.operationReceipt().txHashHex() != null
            || response.operationReceipt().entrypointHashHex() != null)) {
      throw new IllegalStateException(
          "contract call response unsigned draft must not contain transaction hashes");
    }
    return response;
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
        optionalString(receipt.get("contract_alias"), path + ".contract_alias"),
        optionalString(receipt.get("contract_address"), path + ".contract_address"),
        optionalHash(receipt.get("code_hash_hex"), path + ".code_hash_hex"),
        optionalHash(receipt.get("abi_hash_hex"), path + ".abi_hash_hex"),
        optionalTransactionHash(receipt.get("tx_hash_hex"), path + ".tx_hash_hex"),
        optionalString(receipt.get("entrypoint"), path + ".entrypoint"),
        optionalTransactionHash(receipt.get("entrypoint_hash_hex"), path + ".entrypoint_hash_hex"),
        gasLimit,
        asOptionalNonNegativeLong(receipt.get("gas_used"), path + ".gas_used"),
        receipt.get("fee_payment") == null
            ? null : FeePaymentJson.parse(receipt.get("fee_payment"), path + ".fee_payment"),
        exactLowerHex32(receipt.get("payload_digest_hex"), path + ".payload_digest_hex"));
  }

  private static String exactLowerHex32(final Object value, final String path) {
    final String literal = requiredString(value, path);
    if (literal.length() != 64) {
      throw new IllegalStateException(path + " must be canonical lowercase 32-byte hex");
    }
    for (int index = 0; index < literal.length(); index++) {
      final char character = literal.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalStateException(path + " must be canonical lowercase 32-byte hex");
      }
    }
    return literal;
  }

  public static MultisigResponse parseMultisigResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "multisig response"), "multisig response");
    if (!Boolean.TRUE.equals(root.get("ok"))) {
      throw new IllegalStateException("multisig response.ok must be true");
    }
    rejectRetiredDraftFields(root, "multisig response");
    final MultisigResponse response = new MultisigResponse(
        true,
        requiredExactAccountId(root.get("resolved_multisig_account_id"), "multisig response.resolved_multisig_account_id"),
        requiredBoolean(root.get("submitted"), "multisig response.submitted"),
        optionalString(root.get("proposal_id"), "multisig response.proposal_id"),
        root.containsKey("instructions_hash") && root.get("instructions_hash") != null
            ? HttpClientTransport.normalizeHex32(
                requiredString(root.get("instructions_hash"), "multisig response.instructions_hash"),
                "instructionsHash")
            : null,
        root.containsKey("tx_hash_hex") && root.get("tx_hash_hex") != null
            ? transactionHash(root.get("tx_hash_hex"), "multisig response.tx_hash_hex")
            : null,
        root.containsKey("executed_tx_hash_hex") && root.get("executed_tx_hash_hex") != null
            ? transactionHash(
                root.get("executed_tx_hash_hex"), "multisig response.executed_tx_hash_hex")
            : null,
        asOptionalNonNegativeLong(root.get("creation_time_ms"), "multisig response.creation_time_ms"),
        FeePaymentJson.parse(root.get("fee_payment"), "multisig response.fee_payment"),
        optionalBase64(root.get("transaction_payload_b64"), "multisig response.transaction_payload_b64"),
        optionalBase64(root.get("signing_message_b64"), "multisig response.signing_message_b64"));
    validateUnsignedTransactionState(
        response.submitted(),
        response.txHashHex(),
        response.executedTxHashHex(),
        response.creationTimeMs(),
        response.feePayment(),
        response.transactionPayloadB64(),
        response.signingMessageB64(),
        "multisig response");
    return response;
  }

  public static GovernanceContractResponse parseGovernanceContractResponse(final byte[] payload) {
    final Map<String, Object> root =
        expectObject(parse(payload, "governance contract response"), "governance contract response");
    final String context = "governance contract response";
    final boolean found = requiredBoolean(root.get("found"), context + ".found");
    final Boolean active = found
        ? Boolean.valueOf(requiredBoolean(root.get("active"), context + ".active"))
        : null;
    final Set<String> expectedFields;
    if (!found) {
      expectedFields = Set.of("found", "contract_address", "dataspace");
    } else if (active.booleanValue()) {
      expectedFields = Set.of(
          "found",
          "contract_address",
          "contract_subject_account",
          "dataspace",
          "active",
          "lifecycle",
          "emergency_hold_active",
          "code_hash_hex",
          "abi_hash_hex",
          "public_entrypoints");
    } else {
      expectedFields = Set.of(
          "found",
          "contract_address",
          "contract_subject_account",
          "dataspace",
          "active",
          "lifecycle",
          "emergency_hold_active");
    }
    requireExactKeys(root, expectedFields, context);
    final String contractSubjectAccount = found
        ? requiredExactAccountId(
            root.get("contract_subject_account"), context + ".contract_subject_account")
        : null;
    final String dataspace = requiredExactString(root.get("dataspace"), context + ".dataspace");
    final GovernanceContractLifecycle lifecycle = found
        ? parseGovernanceContractLifecycle(root.get("lifecycle"), context + ".lifecycle")
        : null;
    final Boolean emergencyHoldActive = found
        ? Boolean.valueOf(requiredBoolean(
            root.get("emergency_hold_active"), context + ".emergency_hold_active"))
        : null;
    final String codeHashHex = Boolean.TRUE.equals(active)
        ? exactLowerHex32(root.get("code_hash_hex"), context + ".code_hash_hex")
        : null;
    final String abiHashHex = Boolean.TRUE.equals(active)
        ? exactLowerHex32(root.get("abi_hash_hex"), context + ".abi_hash_hex")
        : null;
    final List<String> publicEntrypoints = Boolean.TRUE.equals(active)
        ? governancePublicEntrypoints(
            root.get("public_entrypoints"), context + ".public_entrypoints")
        : null;
    if (found) {
      if (active.booleanValue()) {
        if (!codeHashHex.equals(lifecycle.activeCodeHashHex())) {
          throw new IllegalStateException(
              context + " lifecycle active code hash must match code_hash_hex");
        }
      } else if (lifecycle.activeCodeHashHex() != null) {
        throw new IllegalStateException(
            context + " lifecycle active code hash must be null for an inactive contract");
      }
      if (emergencyHoldActive.booleanValue() && lifecycle.emergencyHold() == null) {
        throw new IllegalStateException(
            context + " active emergency-hold state requires a retained hold record");
      }
    }
    return new GovernanceContractResponse(
        found,
        requiredExactString(root.get("contract_address"), context + ".contract_address"),
        contractSubjectAccount,
        dataspace,
        active,
        lifecycle,
        emergencyHoldActive,
        codeHashHex,
        abiHashHex,
        publicEntrypoints);
  }

  private static GovernanceContractLifecycle parseGovernanceContractLifecycle(
      final Object value, final String context) {
    final Map<String, Object> record = expectObject(value, context);
    requireExactKeys(
        record,
        Set.of(
            "version",
            "origin",
            "origin_account",
            "origin_proposal_content_id_hex",
            "origin_governance_attempt_id_hex",
            "owner",
            "pending_owner",
            "parliament_delegated",
            "active_code_hash_hex",
            "revision",
            "emergency_hold"),
        context);
    final long version = asNonNegativeLong(record.get("version"), context + ".version");
    if (version != 1L) {
      throw new IllegalStateException(
          context + ".version must equal the first-release lifecycle schema version 1");
    }
    final String origin = requiredExactString(record.get("origin"), context + ".origin");
    if (!"direct".equals(origin) && !"parliament".equals(origin)) {
      throw new IllegalStateException(context + ".origin must be direct or parliament");
    }
    final long revision = asNonNegativeLong(record.get("revision"), context + ".revision");
    if (revision == 0L) {
      throw new IllegalStateException(context + ".revision must be positive");
    }
    final String proposalContentIdHex = optionalExactHash(
        record.get("origin_proposal_content_id_hex"),
        context + ".origin_proposal_content_id_hex");
    final String governanceAttemptIdHex = optionalExactHash(
        record.get("origin_governance_attempt_id_hex"),
        context + ".origin_governance_attempt_id_hex");
    if ("direct".equals(origin)
        && (proposalContentIdHex != null || governanceAttemptIdHex != null)) {
      throw new IllegalStateException(
          context + " direct origin must not carry Parliament identifiers");
    }
    if ("parliament".equals(origin)
        && (proposalContentIdHex == null || governanceAttemptIdHex == null)) {
      throw new IllegalStateException(
          context + " Parliament origin requires both governance identifiers");
    }
    return new GovernanceContractLifecycle(
        (int) version,
        origin,
        requiredExactAccountId(record.get("origin_account"), context + ".origin_account"),
        proposalContentIdHex,
        governanceAttemptIdHex,
        governanceContractOwner(record.get("owner"), context + ".owner"),
        record.get("pending_owner") == null
            ? null
            : governanceContractOwner(record.get("pending_owner"), context + ".pending_owner"),
        requiredBoolean(record.get("parliament_delegated"), context + ".parliament_delegated"),
        optionalExactHash(
            record.get("active_code_hash_hex"), context + ".active_code_hash_hex"),
        revision,
        record.get("emergency_hold") == null
            ? null
            : parseGovernanceContractEmergencyHold(
                record.get("emergency_hold"), context + ".emergency_hold"));
  }

  private static GovernanceContractEmergencyHold parseGovernanceContractEmergencyHold(
      final Object value, final String context) {
    final Map<String, Object> record = expectObject(value, context);
    requireExactKeys(
        record,
        Set.of(
            "incident_digest_hex",
            "proposal_content_id_hex",
            "governance_attempt_id_hex",
            "reason",
            "imposed_at_height",
            "expires_at_height"),
        context);
    final long imposedAtHeight =
        asNonNegativeLong(record.get("imposed_at_height"), context + ".imposed_at_height");
    final long expiresAtHeight =
        asNonNegativeLong(record.get("expires_at_height"), context + ".expires_at_height");
    if (imposedAtHeight == 0L) {
      throw new IllegalStateException(context + ".imposed_at_height must be positive");
    }
    if (expiresAtHeight <= imposedAtHeight) {
      throw new IllegalStateException(context + ".expires_at_height must follow imposed_at_height");
    }
    final String incidentDigestHex =
        optionalExactHash(record.get("incident_digest_hex"), context + ".incident_digest_hex");
    final String proposalContentIdHex =
        optionalExactHash(
            record.get("proposal_content_id_hex"), context + ".proposal_content_id_hex");
    final String governanceAttemptIdHex =
        optionalExactHash(
            record.get("governance_attempt_id_hex"), context + ".governance_attempt_id_hex");
    if (incidentDigestHex == null || proposalContentIdHex == null || governanceAttemptIdHex == null) {
      throw new IllegalStateException(context + " must contain all three hash fields");
    }
    return new GovernanceContractEmergencyHold(
        incidentDigestHex,
        proposalContentIdHex,
        governanceAttemptIdHex,
        requiredExactString(record.get("reason"), context + ".reason"),
        imposedAtHeight,
        expiresAtHeight);
  }

  private static void requireExactKeys(
      final Map<String, Object> value, final Set<String> expected, final String path) {
    if (!value.keySet().equals(expected)) {
      throw new IllegalStateException(path + " fields must match the exact first-release schema");
    }
  }

  private static String optionalExactHash(final Object value, final String path) {
    return value == null ? null : exactLowerHex32(value, path);
  }

  private static String governanceContractOwner(final Object value, final String path) {
    final String literal = requiredExactString(value, path);
    return "parliament".equals(literal) ? literal : requiredExactAccountId(literal, path);
  }

  private static List<String> governancePublicEntrypoints(
      final Object value, final String path) {
    final List<Object> raw = requiredList(value, path);
    if (raw.isEmpty()) {
      throw new IllegalStateException(path + " must not be empty");
    }
    final java.util.ArrayList<String> entries = new java.util.ArrayList<>(raw.size());
    String previous = null;
    for (int index = 0; index < raw.size(); index++) {
      final String entry = requiredExactString(raw.get(index), path + "[" + index + "]");
      if (!entry.matches("[a-z][a-z0-9_]{0,127}")) {
        throw new IllegalStateException(
            path + "[" + index + "] must be a canonical public entrypoint name");
      }
      if (previous != null && previous.compareTo(entry) >= 0) {
        throw new IllegalStateException(path + " must be sorted and contain no duplicates");
      }
      entries.add(entry);
      previous = entry;
    }
    return entries;
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
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string");
    }
    if (string.trim().isEmpty()) {
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
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string");
    }
    if (string.trim().isEmpty()) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    if (!string.trim().equals(string)) {
      throw new IllegalStateException(path + " must not contain surrounding whitespace");
    }
    return string;
  }

  private static String optionalString(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a string when present");
    }
    final String trimmed = string.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static String optionalHash(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return HttpClientTransport.normalizeHex32(requiredString(value, path), path);
  }

  private static String transactionHash(final Object value, final String path) {
    if (!(value instanceof String literal)) {
      throw new IllegalStateException(path + " must be a string");
    }
    if (!literal.matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalStateException(
          path + " must match [0-9a-f]{63}[13579bdf] with the Iroha HashOf marker");
    }
    return literal;
  }

  private static String optionalTransactionHash(final Object value, final String path) {
    return value == null ? null : transactionHash(value, path);
  }

  private static Map<String, Object> optionalObject(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    return Collections.unmodifiableMap(new LinkedHashMap<>(expectObject(value, path)));
  }

  private static boolean requiredBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return bool.booleanValue();
  }

  private static long asLong(final Object value, final String path) {
    return JsonNumbers.asLong(value, path);
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

  private static long asNonNegativeLong(final Object value, final String path) {
    final long parsed = asLong(value, path);
    if (parsed < 0L) {
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
    if (!(value instanceof String string)) {
      throw new IllegalStateException(path + " must be a base64 string when present");
    }
    final String literal = string;
    if (literal.isEmpty() || !literal.equals(literal.trim())) {
      throw new IllegalStateException(path + " must be exact standard-base64");
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
    if (!Base64.getEncoder().encodeToString(decoded).equals(literal)) {
      throw new IllegalStateException(path + " must be exact standard-base64");
    }
    return literal;
  }

  private static void rejectRetiredDraftFields(
      final Map<String, Object> value, final String context) {
    for (final String field : RETIRED_DRAFT_FIELDS) {
      if (value.containsKey(field)) {
        throw new IllegalStateException(context + " contains retired field `" + field + "`");
      }
    }
  }

  private static void validateUnsignedTransactionState(
      final boolean submitted,
      final String txHashHex,
      final String executedTxHashHex,
      final Long creationTimeMs,
      final FeePaymentIntent feePayment,
      final String transactionPayloadB64,
      final String signingMessageB64,
      final String context) {
    if (submitted) {
      if (txHashHex == null || transactionPayloadB64 != null || signingMessageB64 != null) {
        throw new IllegalStateException(
            context + " submitted response must contain only the final transaction hash");
      }
      return;
    }
    if (txHashHex != null || transactionPayloadB64 == null || signingMessageB64 == null) {
      throw new IllegalStateException(
          context + " unsigned response must contain exactly one payload and signing-message pair");
    }
    final byte[] transactionPayload = Base64.getDecoder().decode(transactionPayloadB64);
    final byte[] signingMessage = Base64.getDecoder().decode(signingMessageB64);
    final TransactionPayload decodedPayload;
    try {
      decodedPayload = NoritoJavaCodecAdapter.decodeCanonicalTransactionPayload(
          transactionPayload, TransactionAdmissionIntent.ORDINARY);
    } catch (final Exception ex) {
      throw new IllegalStateException(
          context + ".transaction_payload_b64 must contain one canonical TransactionPayload", ex);
    }
    if (signingMessage.length != 32
        || !Arrays.equals(signingMessage, IrohaHash.prehash(transactionPayload))) {
      throw new IllegalStateException(
          context + ".signing_message_b64 must be the exact TransactionPayload hash");
    }
    if (feePayment == null || !decodedPayload.feePayment().equals(feePayment)) {
      throw new IllegalStateException(
          context + ".fee_payment must match the exact TransactionPayload");
    }
    if (creationTimeMs == null || decodedPayload.creationTimeMs() != creationTimeMs.longValue()) {
      throw new IllegalStateException(
          context + ".creation_time_ms must match the exact TransactionPayload");
    }
    if (executedTxHashHex != null) {
      throw new IllegalStateException(
          context + " unsigned response must not contain transaction hashes");
    }
  }

  private static final List<String> RETIRED_DRAFT_FIELDS =
      List.of(
          "transaction_scaffold_b64",
          "transaction_scaffold_base64",
          "signed_transaction_b64",
          "placeholder_transaction_hash_hex",
          "placeholder_entrypoint_hash_hex");
}
