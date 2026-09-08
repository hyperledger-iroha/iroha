package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.TransactionPayload;

/** Receipt-bound public operation identity authenticated by every prepared result. */
public final class PreparedOperationBindingV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.prepared-operation.binding.v1";
  public static final String ONBOARDING = "onboarding";
  public static final String FAUCET = "faucet";

  private final String schema;
  private final String semanticHashHex;
  private final String kind;
  private final String requestId;
  private final long executionExpiresAtUnixMs;

  /** Constructs an exact public operation binding with a stable caller request identifier. */
  public PreparedOperationBindingV1(
      final String semanticHashHex,
      final String kind,
      final String requestId,
      final long executionExpiresAtUnixMs) {
    this(SCHEMA, semanticHashHex, kind, requestId, executionExpiresAtUnixMs);
  }

  /** Constructs an explicitly schema-bound public operation identity. */
  public PreparedOperationBindingV1(
      final String schema,
      final String semanticHashHex,
      final String kind,
      final String requestId,
      final long executionExpiresAtUnixMs) {
    if (!SCHEMA.equals(schema)) throw new IllegalArgumentException("unsupported binding schema");
    if (!ONBOARDING.equals(kind) && !FAUCET.equals(kind)) {
      throw new IllegalArgumentException("unsupported binding kind");
    }
    if (executionExpiresAtUnixMs <= 0L) {
      throw new IllegalArgumentException("executionExpiresAtUnixMs must be positive");
    }
    this.schema = schema;
    this.semanticHashHex = requireLowerHex32(semanticHashHex, "semanticHashHex");
    this.kind = kind;
    this.requestId = requireLowerHex32(requestId, "requestId");
    this.executionExpiresAtUnixMs = executionExpiresAtUnixMs;
  }

  public String schema() { return schema; }
  public String semanticHashHex() { return semanticHashHex; }
  public String kind() { return kind; }
  public String requestId() { return requestId; }
  public long executionExpiresAtUnixMs() { return executionExpiresAtUnixMs; }

  /**
   * Binds a self-consistent signed receipt to a stable request and bounded deadline.
   * Callers must independently verify the expected network, authority and original request.
   */
  public static PreparedOperationBindingV1 onboarding(
      final AccountOnboardingPlanReceiptV1 receipt,
      final String requestId,
      final long executionExpiresAtUnixMs) {
    Objects.requireNonNull(receipt, "receipt");
    AccountOnboardingReceiptVerifier.requireValid(
        receipt, receipt.body().networkId(), receipt.body().authority());
    final byte[] receiptHash = AliasNameSupport.decodeHash(receipt.planHash());
    if (receiptHash == null) throw new IllegalArgumentException("receipt plan hash is invalid");
    final PreparedOperationBindingV1 binding = new PreparedOperationBindingV1(
        PreparedTransactionSignatureV1.hexLower(receiptHash), ONBOARDING, requestId,
        executionExpiresAtUnixMs);
    binding.requireOnboardingReceipt(receipt);
    return binding;
  }

  /** Binds an exact solved faucet claim to a stable request and positive deadline. */
  public static PreparedOperationBindingV1 faucet(
      final AccountFaucetClaimV1 claim,
      final String requestId,
      final long executionExpiresAtUnixMs) {
    return new PreparedOperationBindingV1(
        Objects.requireNonNull(claim, "claim").semanticHashHex(), FAUCET, requestId,
        executionExpiresAtUnixMs);
  }

  void requireOnboardingReceipt(final AccountOnboardingPlanReceiptV1 receipt) {
    final byte[] receiptHash = AliasNameSupport.decodeHash(receipt.planHash());
    if (!ONBOARDING.equals(kind) || receiptHash == null
        || !semanticHashHex.equals(PreparedTransactionSignatureV1.hexLower(receiptHash))) {
      throw new IllegalArgumentException("onboarding binding semantic hash differs from the receipt");
    }
    if (executionExpiresAtUnixMs > receipt.body().validUntilMs()) {
      throw new IllegalArgumentException("onboarding binding deadline exceeds receipt validity");
    }
  }

  void requireFaucetClaim(final AccountFaucetClaimV1 claim) {
    if (!FAUCET.equals(kind) || !semanticHashHex.equals(claim.semanticHashHex())) {
      throw new IllegalArgumentException("faucet binding semantic hash differs from the claim");
    }
  }

  void requireTransactionLifetime(final TransactionPayload payload) {
    final Long ttl = payload.timeToLiveMs().orElse(null);
    if (ttl == null || ttl <= 0 || payload.creationTimeMs() < 0
        || payload.creationTimeMs() >= executionExpiresAtUnixMs
        || ttl > executionExpiresAtUnixMs - payload.creationTimeMs()) {
      throw new IllegalArgumentException("prepared transaction lifetime exceeds its operation deadline");
    }
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("semantic_hash_hex", semanticHashHex);
    map.put("kind", kind);
    map.put("request_id", requestId);
    map.put("execution_expires_at_unix_ms", executionExpiresAtUnixMs);
    return map;
  }

  static String requireLowerHex32(final String value, final String field) {
    if (value == null || !value.matches("[0-9a-f]{64}")) {
      throw new IllegalArgumentException(field + " must contain exactly 64 lowercase hex characters");
    }
    return value;
  }

  static String requireTransactionHash(final String value, final String field) {
    if (value == null || !value.matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalArgumentException(
          field + " must match the canonical Iroha HashOf marker pattern [0-9a-f]{63}[13579bdf]");
    }
    return value;
  }

  static String requireLowerHex(final String value, final String field) {
    if (value == null || value.isEmpty() || (value.length() & 1) != 0
        || !value.matches("[0-9a-f]+")) {
      throw new IllegalArgumentException(field + " must contain non-empty even-length lowercase hex");
    }
    return value;
  }

  static String requireHex(final String value, final String field) {
    if (value == null || value.isEmpty() || (value.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must contain non-empty even-length hex");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.digit(value.charAt(index), 16) < 0) {
        throw new IllegalArgumentException(field + " must contain non-empty even-length hex");
      }
    }
    return value;
  }

}
