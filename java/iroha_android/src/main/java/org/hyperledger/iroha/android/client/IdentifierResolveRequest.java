package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Typed request wrapper for identifier resolve and claim-receipt flows. */
public final class IdentifierResolveRequest {
  private final String policyId;
  private final String input;
  private final String encryptedInputHex;

  private IdentifierResolveRequest(
      final String policyId, final String input, final String encryptedInputHex) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.input = input;
    this.encryptedInputHex = encryptedInputHex;
  }

  public static IdentifierResolveRequest plaintext(final String policyId, final String input) {
    final String normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId");
    final String normalizedInput = HttpClientTransport.normalizeNonBlank(input, "input");
    return new IdentifierResolveRequest(normalizedPolicyId, normalizedInput, null);
  }

  public static IdentifierResolveRequest encrypted(
      final String policyId, final String encryptedInputHex) {
    final String normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId");
    final String normalizedEncryptedInput =
        HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    return new IdentifierResolveRequest(normalizedPolicyId, null, normalizedEncryptedInput);
  }

  public static IdentifierResolveRequest plaintext(
      final IdentifierPolicySummary policy, final String input) {
    Objects.requireNonNull(policy, "policy");
    final String normalized = policy.normalization().normalize(input, "input");
    return plaintext(policy.policyId(), normalized);
  }

  public static IdentifierResolveRequest encrypted(
      final IdentifierPolicySummary policy, final String encryptedInputHex) {
    Objects.requireNonNull(policy, "policy");
    if (!"bfv-v1".equalsIgnoreCase(policy.inputEncryption())) {
      throw new IllegalArgumentException(
          "Policy " + policy.policyId() + " does not publish BFV encrypted-input support");
    }
    return encrypted(policy.policyId(), encryptedInputHex);
  }

  public static IdentifierResolveRequest encryptedFromInput(
      final IdentifierPolicySummary policy, final String input) {
    return encryptedFromInput(policy, input, null);
  }

  public static IdentifierResolveRequest encryptedFromInput(
      final IdentifierPolicySummary policy, final String input, final byte[] seed) {
    Objects.requireNonNull(policy, "policy");
    return encrypted(policy.policyId(), IdentifierBfvEnvelopeBuilder.encrypt(policy, input, seed));
  }

  public String policyId() {
    return policyId;
  }

  public String input() {
    return input;
  }

  public String encryptedInputHex() {
    return encryptedInputHex;
  }
}
