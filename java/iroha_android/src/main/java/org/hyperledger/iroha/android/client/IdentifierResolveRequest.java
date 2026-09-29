package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Typed request wrapper for identifier resolve and claim-receipt flows. */
public final class IdentifierResolveRequest {
  private final String policyId;
  private final String encryptedInputHex;
  private final RamLfeOutputOpening outputOpening;

  private IdentifierResolveRequest(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.encryptedInputHex = encryptedInputHex;
    this.outputOpening = outputOpening;
  }

  public static IdentifierResolveRequest encrypted(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    final String normalizedPolicyId = HttpClientTransport.normalizeNonBlank(policyId, "policyId");
    final String normalizedEncryptedInput =
        HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    return new IdentifierResolveRequest(
        normalizedPolicyId,
        normalizedEncryptedInput,
        Objects.requireNonNull(outputOpening, "outputOpening"));
  }

  public static IdentifierResolveRequest encrypted(
      final IdentifierPolicySummary policy,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    Objects.requireNonNull(policy, "policy");
    if (!"bfv-v1".equalsIgnoreCase(policy.inputEncryption())) {
      throw new IllegalArgumentException(
          "Policy " + policy.policyId() + " does not publish BFV encrypted-input support");
    }
    return encrypted(policy.policyId(), encryptedInputHex, outputOpening);
  }

  /** Refuses encryption until a secure replacement profile is qualified. */
  public static IdentifierResolveRequest encryptedFromInput(
      final IdentifierPolicySummary policy, final String input, final RamLfeOutputOpening outputOpening) {
    throw new UnsupportedOperationException("ram_lfe_encryption_unavailable: RAM-LFE encryption is unavailable: the insecure exact-lift BFV profile must be replaced");
  }

  public String policyId() {
    return policyId;
  }

  public String encryptedInputHex() {
    return encryptedInputHex;
  }

  public RamLfeOutputOpening outputOpening() {
    return outputOpening;
  }
}
