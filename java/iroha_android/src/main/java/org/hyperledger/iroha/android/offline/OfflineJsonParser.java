package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineJsonParser {

  private OfflineJsonParser() {}

  public static OfflineAllowanceList parseAllowances(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final long total = asLong(object.get("total"), "total");
    final List<Object> items = asArray(object.get("items"), "items");
    final List<OfflineAllowanceList.OfflineAllowanceItem> parsed = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final Map<String, Object> entry = expectObject(items.get(i), "items[" + i + "]");
      parsed.add(
          new OfflineAllowanceList.OfflineAllowanceItem(
              asString(entry.get("certificate_id_hex"), "items[" + i + "].certificate_id_hex"),
              asString(entry.get("controller_id"), "items[" + i + "].controller_id"),
              asString(entry.get("controller_display"), "items[" + i + "].controller_display"),
              asString(entry.get("asset_id"), "items[" + i + "].asset_id"),
              asString(
                  entry.get("asset_definition_id"), "items[" + i + "].asset_definition_id"),
              asString(
                  entry.get("asset_definition_name"), "items[" + i + "].asset_definition_name"),
              requireOptionalString(
                  entry, "asset_definition_alias", "items[" + i + "].asset_definition_alias"),
              asLong(entry.get("registered_at_ms"), "items[" + i + "].registered_at_ms"),
              asLongOrDefault(entry.get("expires_at_ms"), "items[" + i + "].expires_at_ms", 0L),
              asLongOrDefault(
                  entry.get("policy_expires_at_ms"), "items[" + i + "].policy_expires_at_ms", 0L),
              asOptionalLong(entry.get("refresh_at_ms"), "items[" + i + "].refresh_at_ms"),
              asOptionalString(entry.get("verdict_id_hex")),
              asOptionalString(entry.get("attestation_nonce_hex")),
              asString(entry.get("remaining_amount"), "items[" + i + "].remaining_amount"),
              JsonEncoder.encode(entry.get("record"))));
    }
    return new OfflineAllowanceList(parsed, total);
  }

  public static OfflineTransferList parseTransfers(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final long total = asLong(object.get("total"), "total");
    final List<Object> items = asArray(object.get("items"), "items");
    final List<OfflineTransferList.OfflineTransferItem> parsed = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final Map<String, Object> entry = expectObject(items.get(i), "items[" + i + "]");
      parsed.add(parseTransferItemObject(entry, "items[" + i + "]"));
    }
    return new OfflineTransferList(parsed, total);
  }

  public static OfflineTransferList.OfflineTransferItem parseTransferItem(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    return parseTransferItemObject(object, "root");
  }

  public static OfflineSummaryList parseSummaries(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final long total = asLong(object.get("total"), "total");
    final List<Object> items = asArray(object.get("items"), "items");
    final List<OfflineSummaryList.OfflineSummaryItem> parsed = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final Map<String, Object> entry = expectObject(items.get(i), "items[" + i + "]");
      parsed.add(
          new OfflineSummaryList.OfflineSummaryItem(
              asString(entry.get("certificate_id_hex"), "items[" + i + "].certificate_id_hex"),
              asString(entry.get("controller_id"), "items[" + i + "].controller_id"),
              asString(
              entry.get("controller_display"), "items[" + i + "].controller_display"),
          asString(entry.get("summary_hash_hex"), "items[" + i + "].summary_hash_hex"),
          asCounterMap(entry.get("apple_key_counters"), "items[" + i + "].apple_key_counters"),
          asCounterMap(
              entry.get("android_series_counters"), "items[" + i + "].android_series_counters")));
    }
    return new OfflineSummaryList(parsed, total);
  }

  public static OfflineRevocationList parseRevocations(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final long total = asLong(object.get("total"), "total");
    final List<Object> items = asArray(object.get("items"), "items");
    final List<OfflineRevocationList.OfflineRevocationItem> parsed =
        new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      final Map<String, Object> entry = expectObject(items.get(i), "items[" + i + "]");
      final String metadataJson =
          entry.containsKey("metadata") ? JsonEncoder.encode(entry.get("metadata")) : null;
      parsed.add(
          new OfflineRevocationList.OfflineRevocationItem(
              asString(entry.get("verdict_id_hex"), "items[" + i + "].verdict_id_hex"),
              asString(entry.get("issuer_id"), "items[" + i + "].issuer_id"),
              asString(entry.get("issuer_display"), "items[" + i + "].issuer_display"),
              asLong(entry.get("revoked_at_ms"), "items[" + i + "].revoked_at_ms"),
              asString(entry.get("reason"), "items[" + i + "].reason"),
              asOptionalString(entry.get("note")),
              metadataJson,
              JsonEncoder.encode(entry.get("record"))));
    }
    return new OfflineRevocationList(parsed, total);
  }

  public static OfflineCertificateIssueResponse parseCertificateIssueResponse(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final String certificateIdHex = asString(object.get("certificate_id_hex"), "certificate_id_hex");
    final Map<String, Object> certificate =
        expectObject(object.get("certificate"), "certificate");
    final OfflineWalletCertificate parsed = parseCertificate(certificate, "certificate");
    return new OfflineCertificateIssueResponse(certificateIdHex, parsed);
  }

  public static OfflineBuildClaimIssueResponse parseBuildClaimIssueResponse(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final String claimIdHex =
        OfflineHashLiteral.parseHex(asString(object.get("claim_id_hex"), "claim_id_hex"), "claim_id_hex");
    final Map<String, Object> buildClaimRaw = expectObject(object.get("build_claim"), "build_claim");
    final OfflineBuildClaim typedBuildClaim = parseBuildClaim(buildClaimRaw, "build_claim");
    return new OfflineBuildClaimIssueResponse(claimIdHex, buildClaimRaw, typedBuildClaim);
  }

  public static OfflineAllowanceRegisterResponse parseAllowanceRegisterResponse(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final String certificateIdHex = asString(object.get("certificate_id_hex"), "certificate_id_hex");
    return new OfflineAllowanceRegisterResponse(certificateIdHex);
  }

  public static OfflineSettlementSubmitResponse parseSettlementSubmitResponse(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final String bundleIdHex = asString(object.get("bundle_id_hex"), "bundle_id_hex");
    final String txHashHex = asString(object.get("transaction_hash_hex"), "transaction_hash_hex");
    return new OfflineSettlementSubmitResponse(bundleIdHex, txHashHex);
  }

  public static OfflineBundleProofStatus parseBundleProofStatus(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final Object summaryNode = object.get("proof_summary");
    final OfflineBundleProofStatus.ProofSummary summary;
    if (summaryNode == null) {
      summary = null;
    } else {
      final Map<String, Object> summaryObject = expectObject(summaryNode, "proof_summary");
      summary =
          new OfflineBundleProofStatus.ProofSummary(
              (int) asLong(summaryObject.get("version"), "proof_summary.version"),
              asOptionalLong(summaryObject.get("proof_sum_bytes"), "proof_summary.proof_sum_bytes"),
              asOptionalLong(
                  summaryObject.get("proof_counter_bytes"), "proof_summary.proof_counter_bytes"),
              asOptionalLong(
                  summaryObject.get("proof_replay_bytes"), "proof_summary.proof_replay_bytes"),
              asStringList(summaryObject.get("metadata_keys"), "proof_summary.metadata_keys"));
    }
    return new OfflineBundleProofStatus(
        asString(object.get("bundle_id_hex"), "bundle_id_hex"),
        asString(object.get("receipts_root_hex"), "receipts_root_hex"),
        asOptionalString(object.get("aggregate_proof_root_hex")),
        asOptionalBoolean(object.get("receipts_root_matches"), "receipts_root_matches"),
        asString(object.get("proof_status"), "proof_status"),
        summary);
  }

  /** Returns a canonical JSON string for the provided payload (keys sorted). */
  public static String canonicalJson(final byte[] payload) {
    final Object root = parse(payload);
    return JsonEncoder.encode(root);
  }

  private static Object parse(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Empty JSON payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> asArray(final Object value, final String path) {
    if (!(value instanceof List)) {
      throw new IllegalStateException(path + " is not a JSON array");
    }
    return (List<Object>) value;
  }

  private static String asString(final Object value, final String path) {
    if (value == null) {
      throw new IllegalStateException(path + " is missing");
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static String asOptionalString(final Object value) {
    if (value == null) {
      return null;
    }
    return value instanceof String string ? string : String.valueOf(value);
  }

  private static String requireOptionalString(
      final Map<String, Object> object, final String key, final String path) {
    if (!object.containsKey(key)) {
      throw new IllegalStateException(path + " is missing");
    }
    return asOptionalString(object.get(key));
  }

  private static String requireNonBlank(final String value, final String path) {
    final String normalized = normalizeOptionalNonBlank(value);
    if (normalized == null) {
      throw new IllegalStateException(path + " must be a non-empty string");
    }
    return normalized;
  }

  private static String normalizeOptionalNonBlank(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " is not a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static long asNonNegativeLong(final Object value, final String path) {
    final long parsed = asLong(value, path);
    if (parsed < 0) {
      throw new IllegalStateException(path + " must be non-negative");
    }
    return parsed;
  }

  private static long asLongOrDefault(
      final Object value, final String path, final long defaultValue) {
    if (value == null) {
      return defaultValue;
    }
    return asLong(value, path);
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
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " is not a boolean");
    }
    return bool;
  }

  private static List<String> asStringList(final Object value, final String path) {
    final List<Object> items = asArray(value, path);
    final List<String> result = new ArrayList<>(items.size());
    for (int i = 0; i < items.size(); i++) {
      result.add(asString(items.get(i), path + "[" + i + "]"));
    }
    return List.copyOf(result);
  }

  private static Map<String, Long> asCounterMap(final Object value, final String path) {
    final Map<String, Object> counters = expectObject(value, path);
    final Map<String, Long> normalized = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : counters.entrySet()) {
      final String key = Objects.requireNonNull(entry.getKey(), path + " key");
      normalized.put(key, asLong(entry.getValue(), path + "." + key));
    }
    return Map.copyOf(normalized);
  }

  private static OfflineTransferList.OfflineTransferItem parseTransferItemObject(
      final Map<String, Object> entry, final String path) {
    final String statusTransitionsJson =
        entry.containsKey("status_transitions") && entry.get("status_transitions") != null
            ? JsonEncoder.encode(entry.get("status_transitions"))
            : null;
    return new OfflineTransferList.OfflineTransferItem(
        asString(entry.get("bundle_id_hex"), path + ".bundle_id_hex"),
        asString(entry.get("receiver_id"), path + ".receiver_id"),
        asString(entry.get("receiver_display"), path + ".receiver_display"),
        asString(entry.get("deposit_account_id"), path + ".deposit_account_id"),
        asString(entry.get("deposit_account_display"), path + ".deposit_account_display"),
        asOptionalString(entry.get("asset_id")),
        asLong(entry.get("receipt_count"), path + ".receipt_count"),
        asString(entry.get("total_amount"), path + ".total_amount"),
        asString(entry.get("claimed_delta"), path + ".claimed_delta"),
        asOptionalString(entry.get("status")),
        asOptionalLong(entry.get("recorded_at_ms"), path + ".recorded_at_ms"),
        asOptionalLong(entry.get("recorded_at_height"), path + ".recorded_at_height"),
        statusTransitionsJson,
        asOptionalString(entry.get("platform_policy")),
        asPlatformTokenSnapshot(entry.get("platform_token_snapshot"), path + ".platform_token_snapshot"),
        JsonEncoder.encode(entry.get("transfer")));
  }

  private static OfflineWalletCertificate parseCertificate(
      final Map<String, Object> entry, final String path) {
    final String controller = asString(entry.get("controller"), path + ".controller");
    final String operator = asString(entry.get("operator"), path + ".operator");
    final Map<String, Object> allowanceEntry =
        expectObject(entry.get("allowance"), path + ".allowance");
    final OfflineAllowanceCommitment allowance =
        parseAllowanceCommitment(allowanceEntry, path + ".allowance");
    final String spendPublicKey =
        asString(entry.get("spend_public_key"), path + ".spend_public_key");
    final byte[] attestationReport =
        asBytes(entry.get("attestation_report"), path + ".attestation_report");
    final long issuedAtMs = asLong(entry.get("issued_at_ms"), path + ".issued_at_ms");
    final long expiresAtMs = asLong(entry.get("expires_at_ms"), path + ".expires_at_ms");
    final Map<String, Object> policyEntry =
        expectObject(entry.get("policy"), path + ".policy");
    final OfflineWalletPolicy policy = parsePolicy(policyEntry, path + ".policy");
    final String operatorSignature =
        asString(entry.get("operator_signature"), path + ".operator_signature");
    final Map<String, Object> metadata =
        entry.get("metadata") instanceof Map<?, ?>
            ? Map.copyOf(expectObject(entry.get("metadata"), path + ".metadata"))
            : Map.of();
    final String verdictIdLiteral = asOptionalString(entry.get("verdict_id"));
    final String verdictIdHex =
        verdictIdLiteral == null
            ? null
            : OfflineHashLiteral.parseHex(verdictIdLiteral, path + ".verdict_id");
    final String attestationNonceLiteral = asOptionalString(entry.get("attestation_nonce"));
    final String attestationNonceHex =
        attestationNonceLiteral == null
            ? null
            : OfflineHashLiteral.parseHex(attestationNonceLiteral, path + ".attestation_nonce");
    final Long refreshAtMs = asOptionalLong(entry.get("refresh_at_ms"), path + ".refresh_at_ms");
    return new OfflineWalletCertificate(
        controller,
        operator,
        allowance,
        spendPublicKey,
        attestationReport,
        issuedAtMs,
        expiresAtMs,
        policy,
        operatorSignature,
        metadata,
        verdictIdHex,
        attestationNonceHex,
        refreshAtMs);
  }

  private static OfflineBuildClaim parseBuildClaim(
      final Map<String, Object> entry, final String path) {
    final String claimId =
        OfflineHashLiteral.normalize(
            asString(entry.get("claim_id"), path + ".claim_id"),
            path + ".claim_id");
    final String nonce =
        OfflineHashLiteral.normalize(asString(entry.get("nonce"), path + ".nonce"), path + ".nonce");
    final String platform = parseBuildClaimPlatform(entry.get("platform"), path + ".platform");
    final String appId = requireNonBlank(asString(entry.get("app_id"), path + ".app_id"), path + ".app_id");
    final long buildNumber = asNonNegativeLong(entry.get("build_number"), path + ".build_number");
    final long issuedAtMs = asNonNegativeLong(entry.get("issued_at_ms"), path + ".issued_at_ms");
    final long expiresAtMs = asNonNegativeLong(entry.get("expires_at_ms"), path + ".expires_at_ms");
    final String lineageScope = normalizeOptionalNonBlank(asOptionalString(entry.get("lineage_scope")));
    final String operatorSignature =
        requireNonBlank(
            asString(entry.get("operator_signature"), path + ".operator_signature"),
            path + ".operator_signature");
    return new OfflineBuildClaim(
        claimId,
        nonce,
        platform,
        appId,
        buildNumber,
        issuedAtMs,
        expiresAtMs,
        lineageScope,
        operatorSignature);
  }

  private static OfflineAllowanceCommitment parseAllowanceCommitment(
      final Map<String, Object> entry, final String path) {
    final String asset = asString(entry.get("asset"), path + ".asset");
    final String amount = asString(entry.get("amount"), path + ".amount");
    final byte[] commitment = asBytes(entry.get("commitment"), path + ".commitment");
    return new OfflineAllowanceCommitment(asset, amount, commitment);
  }

  private static OfflineWalletPolicy parsePolicy(
      final Map<String, Object> entry, final String path) {
    final String maxBalance = asString(entry.get("max_balance"), path + ".max_balance");
    final String maxTxValue = asString(entry.get("max_tx_value"), path + ".max_tx_value");
    final long expiresAtMs = asLong(entry.get("expires_at_ms"), path + ".expires_at_ms");
    return new OfflineWalletPolicy(maxBalance, maxTxValue, expiresAtMs);
  }

  private static byte[] asBytes(final Object value, final String path) {
    final List<Object> items = asArray(value, path);
    final byte[] bytes = new byte[items.size()];
    for (int i = 0; i < items.size(); i++) {
      final Object entry = items.get(i);
      final long numeric = asLong(entry, path + "[" + i + "]");
      if (numeric < 0 || numeric > 255) {
        throw new IllegalStateException(path + "[" + i + "] is not a byte");
      }
      bytes[i] = (byte) (numeric & 0xff);
    }
    return bytes;
  }

  private static String parseBuildClaimPlatform(final Object value, final String path) {
    final String normalized = requireNonBlank(asString(value, path), path).toLowerCase(Locale.ROOT);
    if ("apple".equals(normalized) || "ios".equals(normalized)) {
      return "Apple";
    }
    if ("android".equals(normalized)) {
      return "Android";
    }
    throw new IllegalStateException(path + " must be either \"Apple\" or \"Android\"");
  }

  private static OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot
      asPlatformTokenSnapshot(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    final Map<String, Object> snapshot = expectObject(value, path);
    final String policy = asString(snapshot.get("policy"), path + ".policy");
    final String token =
        asString(snapshot.get("attestation_jws_b64"), path + ".attestation_jws_b64");
    return new OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot(policy, token);
  }
}
