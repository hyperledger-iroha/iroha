package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/** Typed offline build-claim payload returned by Torii. */
public final class OfflineBuildClaim {
  private final String claimId;
  private final String nonce;
  private final String platform;
  private final String appId;
  private final long buildNumber;
  private final long issuedAtMs;
  private final long expiresAtMs;
  private final String lineageScope;
  private final String operatorSignatureHex;

  public OfflineBuildClaim(
      final String claimId,
      final String nonce,
      final String platform,
      final String appId,
      final long buildNumber,
      final long issuedAtMs,
      final long expiresAtMs,
      final String lineageScope,
      final String operatorSignatureHex) {
    this.claimId = Objects.requireNonNull(claimId, "claimId");
    this.nonce = Objects.requireNonNull(nonce, "nonce");
    this.platform = normalizePlatform(Objects.requireNonNull(platform, "platform"));
    this.appId = Objects.requireNonNull(appId, "appId");
    this.buildNumber = buildNumber;
    this.issuedAtMs = issuedAtMs;
    this.expiresAtMs = expiresAtMs;
    this.lineageScope = lineageScope == null || lineageScope.isBlank() ? null : lineageScope.trim();
    this.operatorSignatureHex =
        Objects.requireNonNull(operatorSignatureHex, "operatorSignatureHex");
  }

  public String claimId() {
    return claimId;
  }

  public String nonce() {
    return nonce;
  }

  /** Returns canonical wire token (`Apple` or `Android`). */
  public String platform() {
    return platform;
  }

  public String appId() {
    return appId;
  }

  public long buildNumber() {
    return buildNumber;
  }

  public long issuedAtMs() {
    return issuedAtMs;
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  public String lineageScope() {
    return lineageScope;
  }

  public String operatorSignatureHex() {
    return operatorSignatureHex;
  }

  /** Converts the claim to a JSON-ready map that can be embedded into receipts. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("claim_id", claimId);
    map.put("nonce", nonce);
    map.put("platform", platform);
    map.put("app_id", appId);
    map.put("build_number", buildNumber);
    map.put("issued_at_ms", issuedAtMs);
    map.put("expires_at_ms", expiresAtMs);
    if (lineageScope != null) {
      map.put("lineage_scope", lineageScope);
    }
    map.put("operator_signature", operatorSignatureHex);
    return map;
  }

  private static String normalizePlatform(final String platform) {
    final String normalized = platform.trim().toLowerCase(Locale.ROOT);
    if ("apple".equals(normalized) || "ios".equals(normalized)) {
      return "Apple";
    }
    if ("android".equals(normalized)) {
      return "Android";
    }
    throw new IllegalArgumentException("platform must be either \"Apple\" or \"Android\"");
  }
}
