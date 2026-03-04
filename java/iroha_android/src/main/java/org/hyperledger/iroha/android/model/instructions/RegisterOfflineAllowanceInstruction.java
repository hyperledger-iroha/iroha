package org.hyperledger.iroha.android.model.instructions;

import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RegisterOfflineAllowance} instruction.
 *
 * <p>The instruction carries an {@link OfflineWalletCertificate} describing the allowance
 * commitment, policy, attestation bundle, and metadata required for OA1 ledger admission. This
 * builder keeps the canonical Norito argument names (`certificate.*`) aligned with the Rust data
 * model so automation can mint certificates from Android fixtures without resorting to manual map
 * mutations.
 */
public final class RegisterOfflineAllowanceInstruction implements InstructionTemplate {

  public static final String ACTION = "RegisterOfflineAllowance";

  private final OfflineWalletCertificate certificate;
  private final Map<String, String> arguments;

  private RegisterOfflineAllowanceInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterOfflineAllowanceInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.certificate = builder.certificate;
    this.arguments =
        Map.copyOf(Objects.requireNonNull(argumentOrder, "argument order must not be null"));
  }

  public OfflineWalletCertificate certificate() {
    return certificate;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static RegisterOfflineAllowanceInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder().setCertificate(OfflineWalletCertificate.fromArguments(arguments));
    return new RegisterOfflineAllowanceInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String requireBase64(final String value, final String fieldName) {
    final String trimmed = Objects.requireNonNull(value, fieldName).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    final byte[] decoded;
    try {
      decoded = java.util.Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be numeric: " + value, ex);
    }
  }

  public static final class Builder {
    private OfflineWalletCertificate certificate;

    private Builder() {}

    public Builder setCertificate(final OfflineWalletCertificate certificate) {
      this.certificate = Objects.requireNonNull(certificate, "certificate");
      return this;
    }

    public RegisterOfflineAllowanceInstruction build() {
      if (certificate == null) {
        throw new IllegalStateException("certificate must be provided");
      }
      return new RegisterOfflineAllowanceInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      certificate.appendArguments(args);
      return args;
    }
  }

  /** Canonical allowance bundle attached to {@code RegisterOfflineAllowance}. */
  public static final class OfflineWalletCertificate {
    private final String controller;
    private final String operator;
    private final OfflineAllowance allowance;
    private final String spendPublicKey;
    private final String attestationReportBase64;
    private final long issuedAtMs;
    private final long expiresAtMs;
    private final OfflineWalletPolicy policy;
    private final String operatorSignatureHex;
    private final Map<String, String> metadata;
    private final String verdictIdHex;
    private final String attestationNonceHex;
    private final Long refreshAtMs;

    private OfflineWalletCertificate(final Builder builder) {
      this.controller = builder.controller;
      this.operator = builder.operator;
      this.allowance = builder.allowance;
      this.spendPublicKey = builder.spendPublicKey;
      this.attestationReportBase64 = builder.attestationReportBase64;
      this.issuedAtMs = builder.issuedAtMs;
      this.expiresAtMs = builder.expiresAtMs;
      this.policy = builder.policy;
      this.operatorSignatureHex = builder.operatorSignatureHex;
      this.metadata =
          Map.copyOf(Objects.requireNonNull(builder.metadata, "metadata map must not be null"));
      this.verdictIdHex = builder.verdictIdHex;
      this.attestationNonceHex = builder.attestationNonceHex;
      this.refreshAtMs = builder.refreshAtMs;
    }

    public String controller() {
      return controller;
    }

    public String operator() {
      return operator;
    }

    public OfflineAllowance allowance() {
      return allowance;
    }

    public String spendPublicKey() {
      return spendPublicKey;
    }

    public String attestationReportBase64() {
      return attestationReportBase64;
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

    public String operatorSignatureHex() {
      return operatorSignatureHex;
    }

    public Map<String, String> metadata() {
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

    private void appendArguments(final Map<String, String> target) {
      target.put("certificate.controller", controller);
      target.put("certificate.operator", operator);
      allowance.appendArguments(target);
      target.put("certificate.spend_public_key", spendPublicKey);
      target.put("certificate.attestation_report_b64", attestationReportBase64);
      target.put("certificate.issued_at_ms", Long.toString(issuedAtMs));
      target.put("certificate.expires_at_ms", Long.toString(expiresAtMs));
      policy.appendArguments(target);
      target.put("certificate.operator_signature_hex", operatorSignatureHex);
      metadata.forEach(
          (key, value) -> target.put("certificate.metadata." + key, Objects.requireNonNull(value)));
      if (verdictIdHex != null && !verdictIdHex.isBlank()) {
        target.put("certificate.verdict_id_hex", verdictIdHex);
      }
      if (attestationNonceHex != null && !attestationNonceHex.isBlank()) {
        target.put("certificate.attestation_nonce_hex", attestationNonceHex);
      }
      if (refreshAtMs != null) {
        target.put("certificate.refresh_at_ms", Long.toString(refreshAtMs));
      }
    }

    public static OfflineWalletCertificate fromArguments(final Map<String, String> arguments) {
      final Builder builder =
          builder()
              .setController(require(arguments, "certificate.controller"))
              .setOperator(require(arguments, "certificate.operator"))
              .setAllowance(OfflineAllowance.fromArguments(arguments))
              .setSpendPublicKey(require(arguments, "certificate.spend_public_key"))
              .setAttestationReportBase64(require(arguments, "certificate.attestation_report_b64"))
              .setIssuedAtMs(requireLong(arguments, "certificate.issued_at_ms"))
              .setExpiresAtMs(requireLong(arguments, "certificate.expires_at_ms"))
              .setPolicy(OfflineWalletPolicy.fromArguments(arguments))
              .setOperatorSignatureHex(require(arguments, "certificate.operator_signature_hex"))
              .setMetadata(extractPrefixed(arguments, "certificate.metadata."));
      if (arguments.containsKey("certificate.verdict_id_hex")) {
        builder.setVerdictIdHex(arguments.get("certificate.verdict_id_hex"));
      }
      if (arguments.containsKey("certificate.attestation_nonce_hex")) {
        builder.setAttestationNonceHex(arguments.get("certificate.attestation_nonce_hex"));
      }
      if (arguments.containsKey("certificate.refresh_at_ms")) {
        builder.setRefreshAtMs(requireLong(arguments, "certificate.refresh_at_ms"));
      }
      return builder.build();
    }

    private static Map<String, String> extractPrefixed(
        final Map<String, String> source, final String prefix) {
      final Map<String, String> extracted = new LinkedHashMap<>();
      source.forEach(
          (key, value) -> {
            if (key.startsWith(prefix)) {
              final String metadataKey = key.substring(prefix.length());
              if (!metadataKey.isEmpty()) {
                extracted.put(metadataKey, value);
              }
            }
          });
      return extracted;
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof OfflineWalletCertificate other)) {
        return false;
      }
      return Objects.equals(controller, other.controller)
          && Objects.equals(operator, other.operator)
          && Objects.equals(allowance, other.allowance)
          && Objects.equals(spendPublicKey, other.spendPublicKey)
          && Objects.equals(attestationReportBase64, other.attestationReportBase64)
          && issuedAtMs == other.issuedAtMs
          && expiresAtMs == other.expiresAtMs
          && Objects.equals(policy, other.policy)
          && Objects.equals(operatorSignatureHex, other.operatorSignatureHex)
          && Objects.equals(metadata, other.metadata)
          && Objects.equals(verdictIdHex, other.verdictIdHex)
          && Objects.equals(attestationNonceHex, other.attestationNonceHex)
          && Objects.equals(refreshAtMs, other.refreshAtMs);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          controller,
          operator,
          allowance,
          spendPublicKey,
          attestationReportBase64,
          issuedAtMs,
          expiresAtMs,
          policy,
          operatorSignatureHex,
          metadata,
          verdictIdHex,
          attestationNonceHex,
          refreshAtMs);
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private String controller;
      private String operator;
      private OfflineAllowance allowance;
      private String spendPublicKey;
      private String attestationReportBase64;
      private Long issuedAtMs;
      private Long expiresAtMs;
      private OfflineWalletPolicy policy;
      private String operatorSignatureHex;
      private final Map<String, String> metadata = new LinkedHashMap<>();
      private String verdictIdHex;
      private String attestationNonceHex;
      private Long refreshAtMs;

      private Builder() {}

      public Builder setController(final String controller) {
        if (controller == null || controller.isBlank()) {
          throw new IllegalArgumentException("controller must not be blank");
        }
        this.controller = controller;
        return this;
      }

      public Builder setOperator(final String operator) {
        if (operator == null || operator.isBlank()) {
          throw new IllegalArgumentException("operator must not be blank");
        }
        this.operator = operator;
        return this;
      }

      public Builder setAllowance(final OfflineAllowance allowance) {
        this.allowance = Objects.requireNonNull(allowance, "allowance");
        return this;
      }

      public Builder setSpendPublicKey(final String spendPublicKey) {
        if (spendPublicKey == null || spendPublicKey.isBlank()) {
          throw new IllegalArgumentException("spendPublicKey must not be blank");
        }
        this.spendPublicKey = spendPublicKey;
        return this;
      }

      public Builder setAttestationReportBase64(final String attestationReportBase64) {
        this.attestationReportBase64 =
            requireBase64(attestationReportBase64, "attestationReportBase64");
        return this;
      }

      public Builder setIssuedAtMs(final long issuedAtMs) {
        if (issuedAtMs <= 0L) {
          throw new IllegalArgumentException("issuedAtMs must be positive");
        }
        this.issuedAtMs = issuedAtMs;
        return this;
      }

      public Builder setExpiresAtMs(final long expiresAtMs) {
        if (expiresAtMs <= 0L) {
          throw new IllegalArgumentException("expiresAtMs must be positive");
        }
        this.expiresAtMs = expiresAtMs;
        return this;
      }

      public Builder setPolicy(final OfflineWalletPolicy policy) {
        this.policy = Objects.requireNonNull(policy, "policy");
        return this;
      }

      public Builder setOperatorSignatureHex(final String operatorSignatureHex) {
        if (operatorSignatureHex == null || operatorSignatureHex.isBlank()) {
          throw new IllegalArgumentException("operatorSignatureHex must not be blank");
        }
        this.operatorSignatureHex = operatorSignatureHex;
        return this;
      }

      public Builder putMetadata(final String key, final String value) {
        if (key == null || key.isBlank()) {
          throw new IllegalArgumentException("metadata key must not be blank");
        }
        metadata.put(key, Objects.requireNonNull(value, "metadata value must not be null"));
        return this;
      }

      /**
       * Replaces the metadata map with {@code entries}, preserving insertion order.
       *
       * @param entries metadata entries keyed by suffix (without the {@code certificate.metadata.}
       *     prefix)
       */
      public Builder setMetadata(final Map<String, String> entries) {
        Objects.requireNonNull(entries, "metadata entries must not be null");
        metadata.clear();
        entries.forEach(
            (key, value) -> {
              if (key == null || key.isBlank()) {
                throw new IllegalArgumentException("metadata key must not be blank");
              }
              metadata.put(key, Objects.requireNonNull(value, "metadata value must not be null"));
            });
        return this;
      }

      public Builder putAllMetadata(final Map<String, String> entries) {
        if (entries != null) {
          entries.forEach(this::putMetadata);
        }
        return this;
      }

      public Builder setVerdictIdHex(final String verdictIdHex) {
        this.verdictIdHex = verdictIdHex;
        return this;
      }

      public Builder setAttestationNonceHex(final String attestationNonceHex) {
        this.attestationNonceHex = attestationNonceHex;
        return this;
      }

      public Builder setRefreshAtMs(final Long refreshAtMs) {
        this.refreshAtMs = refreshAtMs;
        return this;
      }

      public OfflineWalletCertificate build() {
        if (controller == null) {
          throw new IllegalStateException("controller must be provided");
        }
        if (operator == null) {
          throw new IllegalStateException("operator must be provided");
        }
        if (allowance == null) {
          throw new IllegalStateException("allowance must be provided");
        }
        if (spendPublicKey == null) {
          throw new IllegalStateException("spendPublicKey must be provided");
        }
        if (attestationReportBase64 == null) {
          throw new IllegalStateException("attestationReportBase64 must be provided");
        }
        if (issuedAtMs == null) {
          throw new IllegalStateException("issuedAtMs must be provided");
        }
        if (expiresAtMs == null) {
          throw new IllegalStateException("expiresAtMs must be provided");
        }
        if (policy == null) {
          throw new IllegalStateException("policy must be provided");
        }
        if (operatorSignatureHex == null) {
          throw new IllegalStateException("operatorSignatureHex must be provided");
        }
        return new OfflineWalletCertificate(this);
      }
    }
  }

  /** Allowance commitment embedded in the certificate. */
  public static final class OfflineAllowance {
    private final String assetId;
    private final String amount;
    private final String commitmentHex;

    private OfflineAllowance(final Builder builder) {
      this.assetId = builder.assetId;
      this.amount = builder.amount;
      this.commitmentHex = builder.commitmentHex;
    }

    public String assetId() {
      return assetId;
    }

    public String amount() {
      return amount;
    }

    public String commitmentHex() {
      return commitmentHex;
    }

    private void appendArguments(final Map<String, String> target) {
      target.put("certificate.allowance.asset", assetId);
      target.put("certificate.allowance.amount", amount);
      target.put("certificate.allowance.commitment_hex", commitmentHex);
    }

    public static OfflineAllowance fromArguments(final Map<String, String> arguments) {
      return builder()
          .setAssetId(require(arguments, "certificate.allowance.asset"))
          .setAmount(require(arguments, "certificate.allowance.amount"))
          .setCommitmentHex(require(arguments, "certificate.allowance.commitment_hex"))
          .build();
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof OfflineAllowance other)) {
        return false;
      }
      return Objects.equals(assetId, other.assetId)
          && Objects.equals(amount, other.amount)
          && Objects.equals(commitmentHex, other.commitmentHex);
    }

    @Override
    public int hashCode() {
      return Objects.hash(assetId, amount, commitmentHex);
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private String assetId;
      private String amount;
      private String commitmentHex;

      private Builder() {}

      public Builder setAssetId(final String assetId) {
        if (assetId == null || assetId.isBlank()) {
          throw new IllegalArgumentException("assetId must not be blank");
        }
        this.assetId = assetId;
        return this;
      }

      public Builder setAmount(final String amount) {
        if (amount == null || amount.isBlank()) {
          throw new IllegalArgumentException("amount must not be blank");
        }
        this.amount = amount;
        return this;
      }

      public Builder setCommitmentHex(final String commitmentHex) {
        if (commitmentHex == null || commitmentHex.isBlank()) {
          throw new IllegalArgumentException("commitmentHex must not be blank");
        }
        this.commitmentHex = commitmentHex;
        return this;
      }

      public OfflineAllowance build() {
        if (assetId == null) {
          throw new IllegalStateException("assetId must be provided");
        }
        if (amount == null) {
          throw new IllegalStateException("amount must be provided");
        }
        if (commitmentHex == null) {
          throw new IllegalStateException("commitmentHex must be provided");
        }
        return new OfflineAllowance(this);
      }
    }
  }

  /** Certificate policy bounds. */
  public static final class OfflineWalletPolicy {
    private final String maxBalance;
    private final String maxTxValue;
    private final long expiresAtMs;

    private OfflineWalletPolicy(final Builder builder) {
      this.maxBalance = builder.maxBalance;
      this.maxTxValue = builder.maxTxValue;
      this.expiresAtMs = builder.expiresAtMs;
    }

    public String maxBalance() {
      return maxBalance;
    }

    public String maxTxValue() {
      return maxTxValue;
    }

    public long expiresAtMs() {
      return expiresAtMs;
    }

    private void appendArguments(final Map<String, String> target) {
      target.put("certificate.policy.max_balance", maxBalance);
      target.put("certificate.policy.max_tx_value", maxTxValue);
      target.put("certificate.policy.expires_at_ms", Long.toString(expiresAtMs));
    }

    public static OfflineWalletPolicy fromArguments(final Map<String, String> arguments) {
      return builder()
          .setMaxBalance(require(arguments, "certificate.policy.max_balance"))
          .setMaxTxValue(require(arguments, "certificate.policy.max_tx_value"))
          .setExpiresAtMs(requireLong(arguments, "certificate.policy.expires_at_ms"))
          .build();
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof OfflineWalletPolicy other)) {
        return false;
      }
      return Objects.equals(maxBalance, other.maxBalance)
          && Objects.equals(maxTxValue, other.maxTxValue)
          && expiresAtMs == other.expiresAtMs;
    }

    @Override
    public int hashCode() {
      return Objects.hash(maxBalance, maxTxValue, expiresAtMs);
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private String maxBalance;
      private String maxTxValue;
      private Long expiresAtMs;

      private Builder() {}

      public Builder setMaxBalance(final String maxBalance) {
        if (maxBalance == null || maxBalance.isBlank()) {
          throw new IllegalArgumentException("maxBalance must not be blank");
        }
        this.maxBalance = maxBalance;
        return this;
      }

      public Builder setMaxTxValue(final String maxTxValue) {
        if (maxTxValue == null || maxTxValue.isBlank()) {
          throw new IllegalArgumentException("maxTxValue must not be blank");
        }
        this.maxTxValue = maxTxValue;
        return this;
      }

      public Builder setExpiresAtMs(final long expiresAtMs) {
        if (expiresAtMs <= 0L) {
          throw new IllegalArgumentException("expiresAtMs must be positive");
        }
        this.expiresAtMs = expiresAtMs;
        return this;
      }

      public OfflineWalletPolicy build() {
        if (maxBalance == null) {
          throw new IllegalStateException("maxBalance must be provided");
        }
        if (maxTxValue == null) {
          throw new IllegalStateException("maxTxValue must be provided");
        }
        if (expiresAtMs == null) {
          throw new IllegalStateException("expiresAtMs must be provided");
        }
        return new OfflineWalletPolicy(this);
      }
    }
  }
}
