package org.hyperledger.iroha.android.client;

/** Cryptographically verifies restricted atomic-private-settlement Torii responses. */
public interface AtomicPrivateSettlementResponseVerifierV1 {
  /** Fail unless this verifier can perform production-strength verification now. */
  void requireAvailable();

  /** Verify a committee proof against the configured network and requested payload digest. */
  void verifyCommitteeProofResponse(
      byte[] responseJson, byte[] expectedNetworkId, byte[] requestedPayloadDigest);

  /** Verify an auditor capsule against the exact governed auditor signing key. */
  void verifyAuditorCapsuleResponse(
      byte[] responseJson,
      byte[] requestJson,
      byte[] expectedNetworkId,
      byte[] requestedPayloadDigest,
      String auditorPublicKey);

  /** Verify an approval acknowledgement against its exact request and auditor signing key. */
  void verifyAuditApprovalResponse(
      byte[] responseJson,
      byte[] requestJson,
      byte[] expectedNetworkId,
      byte[] requestedPayloadDigest,
      String auditorPublicKey);
}
