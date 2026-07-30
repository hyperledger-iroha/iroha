package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Exact provider owner and signer policy expected at provider-completion commit. */
public final class ProviderIngestCompletionAuthorityV1 {

  private final String providerOwner;
  private final ProviderIngestCompletionSignerPolicyV1 signerPolicy;

  /**
   * Creates one exact provider-completion authority.
   *
   * @param providerOwner canonical I105 owner account
   * @param signerPolicy exact governed signer-policy identity
   */
  public ProviderIngestCompletionAuthorityV1(
      final String providerOwner,
      final ProviderIngestCompletionSignerPolicyV1 signerPolicy) {
    this.providerOwner =
        ReplicationOrderInstructionValidation.requireProviderOwner(providerOwner);
    this.signerPolicy = Objects.requireNonNull(signerPolicy, "signerPolicy");
  }

  public String providerOwner() {
    return providerOwner;
  }

  public ProviderIngestCompletionSignerPolicyV1 signerPolicy() {
    return signerPolicy;
  }

  String canonicalJson() {
    return "{\"provider_owner\":\""
        + providerOwner
        + "\",\"signer_policy\":"
        + signerPolicy.canonicalJson()
        + "}";
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProviderIngestCompletionAuthorityV1)) {
      return false;
    }
    final ProviderIngestCompletionAuthorityV1 other =
        (ProviderIngestCompletionAuthorityV1) obj;
    return Objects.equals(providerOwner, other.providerOwner)
        && Objects.equals(signerPolicy, other.signerPolicy);
  }

  @Override
  public int hashCode() {
    return Objects.hash(providerOwner, signerPolicy);
  }
}
