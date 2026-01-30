package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
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
      parsed.add(
          new OfflineTransferList.OfflineTransferItem(
              asString(entry.get("bundle_id_hex"), "items[" + i + "].bundle_id_hex"),
              asString(entry.get("receiver_id"), "items[" + i + "].receiver_id"),
              asString(entry.get("receiver_display"), "items[" + i + "].receiver_display"),
              asString(entry.get("deposit_account_id"), "items[" + i + "].deposit_account_id"),
              asString(
                  entry.get("deposit_account_display"),
                  "items[" + i + "].deposit_account_display"),
              asOptionalString(entry.get("asset_id")),
              asLong(entry.get("receipt_count"), "items[" + i + "].receipt_count"),
              asString(entry.get("total_amount"), "items[" + i + "].total_amount"),
              asString(entry.get("claimed_delta"), "items[" + i + "].claimed_delta"),
              asOptionalString(entry.get("platform_policy")),
              asPlatformTokenSnapshot(
                  entry.get("platform_token_snapshot"),
                  "items[" + i + "].platform_token_snapshot"),
              JsonEncoder.encode(entry.get("transfer"))));
    }
    return new OfflineTransferList(parsed, total);
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

  public static OfflineAllowanceRegisterResponse parseAllowanceRegisterResponse(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    final String certificateIdHex = asString(object.get("certificate_id_hex"), "certificate_id_hex");
    return new OfflineAllowanceRegisterResponse(certificateIdHex);
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

  private static long asLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " is not a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
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

  private static Map<String, Long> asCounterMap(final Object value, final String path) {
    final Map<String, Object> counters = expectObject(value, path);
    final Map<String, Long> normalized = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : counters.entrySet()) {
      final String key = Objects.requireNonNull(entry.getKey(), path + " key");
      normalized.put(key, asLong(entry.getValue(), path + "." + key));
    }
    return Map.copyOf(normalized);
  }

  private static OfflineWalletCertificate parseCertificate(
      final Map<String, Object> entry, final String path) {
    final String controller = asString(entry.get("controller"), path + ".controller");
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
