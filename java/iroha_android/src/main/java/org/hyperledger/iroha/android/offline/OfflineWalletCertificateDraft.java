package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Draft offline wallet certificate that is missing an operator signature. */
public final class OfflineWalletCertificateDraft {
  private final String controller;
  private final OfflineAllowanceCommitment allowance;
  private final String spendPublicKey;
  private final byte[] attestationReport;
  private final long issuedAtMs;
  private final long expiresAtMs;
  private final OfflineWalletPolicy policy;
  private final Map<String, Object> metadata;
  private final String verdictIdHex;
  private final String attestationNonceHex;
  private final Long refreshAtMs;

  public OfflineWalletCertificateDraft(
      final String controller,
      final OfflineAllowanceCommitment allowance,
      final String spendPublicKey,
      final byte[] attestationReport,
      final long issuedAtMs,
      final long expiresAtMs,
      final OfflineWalletPolicy policy,
      final Map<String, Object> metadata,
      final String verdictIdHex,
      final String attestationNonceHex,
      final Long refreshAtMs) {
    this.controller = Objects.requireNonNull(controller, "controller");
    this.allowance = Objects.requireNonNull(allowance, "allowance");
    this.spendPublicKey = Objects.requireNonNull(spendPublicKey, "spendPublicKey");
    this.attestationReport = Objects.requireNonNull(attestationReport, "attestationReport").clone();
    this.issuedAtMs = issuedAtMs;
    this.expiresAtMs = expiresAtMs;
    this.policy = Objects.requireNonNull(policy, "policy");
    this.metadata = metadata == null ? Map.of() : Map.copyOf(metadata);
    this.verdictIdHex = verdictIdHex;
    this.attestationNonceHex = attestationNonceHex;
    this.refreshAtMs = refreshAtMs;
  }

  /**
   * @deprecated Operator is derived by Torii from its configured keypair and ignored in draft
   *     payloads.
   */
  @Deprecated(since = "2.0.0", forRemoval = false)
  public OfflineWalletCertificateDraft(
      final String controller,
      final String operator,
      final OfflineAllowanceCommitment allowance,
      final String spendPublicKey,
      final byte[] attestationReport,
      final long issuedAtMs,
      final long expiresAtMs,
      final OfflineWalletPolicy policy,
      final Map<String, Object> metadata,
      final String verdictIdHex,
      final String attestationNonceHex,
      final Long refreshAtMs) {
    this(
        controller,
        allowance,
        spendPublicKey,
        attestationReport,
        issuedAtMs,
        expiresAtMs,
        policy,
        metadata,
        verdictIdHex,
        attestationNonceHex,
        refreshAtMs);
  }

  public String controller() {
    return controller;
  }

  public OfflineAllowanceCommitment allowance() {
    return allowance;
  }

  public String spendPublicKey() {
    return spendPublicKey;
  }

  public byte[] attestationReport() {
    return attestationReport.clone();
  }

  public long issuedAtMs() {
    return issuedAtMs;
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  public OfflineWalletPolicy policy() {
    return policy;
  }

  public Map<String, Object> metadata() {
    return metadata;
  }

  public String verdictIdHex() {
    return verdictIdHex;
  }

  public String attestationNonceHex() {
    return attestationNonceHex;
  }

  public Long refreshAtMs() {
    return refreshAtMs;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("controller", controller);
    map.put("allowance", allowance.toJsonMap());
    map.put("spend_public_key", spendPublicKey);
    map.put("attestation_report", OfflineAllowanceCommitment.encodeBytes(attestationReport));
    map.put("issued_at_ms", issuedAtMs);
    map.put("expires_at_ms", expiresAtMs);
    map.put("policy", policy.toJsonMap());
    map.put("metadata", metadata);
    map.put(
        "verdict_id",
        verdictIdHex == null ? null : OfflineHashLiteral.normalize(verdictIdHex, "verdict_id"));
    map.put(
        "attestation_nonce",
        attestationNonceHex == null
            ? null
            : OfflineHashLiteral.normalize(attestationNonceHex, "attestation_nonce"));
    map.put("refresh_at_ms", refreshAtMs);
    return map;
  }
}
