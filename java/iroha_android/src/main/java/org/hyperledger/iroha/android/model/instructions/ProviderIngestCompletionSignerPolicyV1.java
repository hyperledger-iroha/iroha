package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Exact governed signer-policy identity expected at provider-completion commit. */
public final class ProviderIngestCompletionSignerPolicyV1 {

  private final String policyId;
  private final long revision;
  private final String predecessorDigest;
  private final String policyDigest;

  /**
   * Creates one exact governed signer-policy identity.
   *
   * @param policyId stable non-zero policy identifier
   * @param revision monotonic revision beginning at one
   * @param predecessorDigest preceding leaf digest, absent only at revision one
   * @param policyDigest exact governed signer leaf digest
   */
  public ProviderIngestCompletionSignerPolicyV1(
      final String policyId,
      final long revision,
      final String predecessorDigest,
      final String policyDigest) {
    this.policyId =
        ReplicationOrderInstructionValidation.requireDigest(policyId, "policyId");
    this.revision =
        ReplicationOrderInstructionValidation.requirePositiveRevision(revision, "revision");
    this.policyDigest =
        ReplicationOrderInstructionValidation.requireDigest(policyDigest, "policyDigest");
    if (revision == 1) {
      if (predecessorDigest != null) {
        throw new IllegalArgumentException(
            "predecessorDigest must be absent at revision one");
      }
      this.predecessorDigest = null;
    } else {
      if (predecessorDigest == null) {
        throw new IllegalArgumentException(
            "predecessorDigest is required after revision one");
      }
      this.predecessorDigest =
          ReplicationOrderInstructionValidation.requireDigest(
              predecessorDigest, "predecessorDigest");
    }
  }

  public String policyId() {
    return policyId;
  }

  public long revision() {
    return revision;
  }

  public String predecessorDigest() {
    return predecessorDigest;
  }

  public String policyDigest() {
    return policyDigest;
  }

  String canonicalJson() {
    final String predecessor =
        predecessorDigest == null ? "null" : "\"" + predecessorDigest + "\"";
    return "{\"policy_id\":\""
        + policyId
        + "\",\"revision\":"
        + revision
        + ",\"predecessor_digest\":"
        + predecessor
        + ",\"policy_digest\":\""
        + policyDigest
        + "\"}";
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProviderIngestCompletionSignerPolicyV1)) {
      return false;
    }
    final ProviderIngestCompletionSignerPolicyV1 other =
        (ProviderIngestCompletionSignerPolicyV1) obj;
    return revision == other.revision
        && Objects.equals(policyId, other.policyId)
        && Objects.equals(predecessorDigest, other.predecessorDigest)
        && Objects.equals(policyDigest, other.policyDigest);
  }

  @Override
  public int hashCode() {
    return Objects.hash(policyId, revision, predecessorDigest, policyDigest);
  }
}
