package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Exact public-reset mutation identity authenticated by every prepared result. */
public final class TairaPublicResetMutationBindingV1 extends AliasJsonValue {
  public static final String SCHEMA = "iroha.taira.public-reset.mutation-binding.v1";
  public static final String ONBOARDING = "onboarding";
  public static final String FAUCET = "faucet";

  private final String schema;
  private final String authorizationSha256;
  private final String authorizationNonce;
  private final String kind;
  private final String phase;
  private final String idempotencyKey;
  private final long executionExpiresAtUnixMs;

  /** Constructs a canonical V1 mutation binding. */
  public TairaPublicResetMutationBindingV1(
      final String authorizationSha256,
      final String authorizationNonce,
      final String kind,
      final String phase,
      final String idempotencyKey,
      final long executionExpiresAtUnixMs) {
    this(SCHEMA, authorizationSha256, authorizationNonce, kind, phase, idempotencyKey,
        executionExpiresAtUnixMs);
  }

  /** Constructs an explicitly schema-bound mutation binding. */
  public TairaPublicResetMutationBindingV1(
      final String schema,
      final String authorizationSha256,
      final String authorizationNonce,
      final String kind,
      final String phase,
      final String idempotencyKey,
      final long executionExpiresAtUnixMs) {
    if (!SCHEMA.equals(schema)) throw new IllegalArgumentException("unsupported binding schema");
    if (authorizationNonce == null || authorizationNonce.length() != 32
        || !isBindingToken(authorizationNonce)) {
      throw new IllegalArgumentException(
          "authorizationNonce must contain exactly 32 lowercase token characters");
    }
    if (!ONBOARDING.equals(kind) && !FAUCET.equals(kind)) {
      throw new IllegalArgumentException("unsupported binding kind");
    }
    if (phase == null || phase.isEmpty() || phase.length() > 128 || !isBindingToken(phase)) {
      throw new IllegalArgumentException("phase must contain 1..128 lowercase token characters");
    }
    if (executionExpiresAtUnixMs < 0L) {
      throw new IllegalArgumentException("executionExpiresAtUnixMs must not be negative");
    }
    this.schema = schema;
    this.authorizationSha256 = requireLowerHex32(authorizationSha256, "authorizationSha256");
    this.authorizationNonce = authorizationNonce;
    this.kind = kind;
    this.phase = phase;
    this.idempotencyKey = requireLowerHex32(idempotencyKey, "idempotencyKey");
    this.executionExpiresAtUnixMs = executionExpiresAtUnixMs;
  }

  public String schema() { return schema; }
  public String authorizationSha256() { return authorizationSha256; }
  public String authorizationNonce() { return authorizationNonce; }
  public String kind() { return kind; }
  public String phase() { return phase; }
  public String idempotencyKey() { return idempotencyKey; }
  public long executionExpiresAtUnixMs() { return executionExpiresAtUnixMs; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema", schema);
    map.put("authorization_sha256", authorizationSha256);
    map.put("authorization_nonce", authorizationNonce);
    map.put("kind", kind);
    map.put("phase", phase);
    map.put("idempotency_key", idempotencyKey);
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

  private static boolean isBindingToken(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= 'a' && character <= 'z')
          || (character >= '0' && character <= '9')
          || character == '-' || character == '_')) return false;
    }
    return true;
  }
}
