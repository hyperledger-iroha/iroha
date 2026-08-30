package org.hyperledger.iroha.android.client;

import java.util.Set;

/** Closed V1 mutation catalog for atomic private settlement. */
public enum AtomicPrivateSettlementOperationV1 {
  /** Persist provisional encrypted material and obtain one availability share. */
  AVAILABILITY_SHARE(
      "/v1/nexus/private-settlements/legs/availability-shares",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("material"),
      32 * 1024 * 1024),
  /** Ask one exact four-member committee validator for a Prepare vote. */
  PREPARE_VOTE(
      "/v1/nexus/private-settlements/phases/prepare-votes",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("manifest", "payload_digest"),
      8 * 1024 * 1024),
  /** Ask one exact four-member committee validator for a Commit vote. */
  COMMIT_VOTE(
      "/v1/nexus/private-settlements/phases/commit-votes",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("payload_digest", "barrier"),
      8 * 1024 * 1024),
  /** Persist an aggregate Prepare or Commit certificate. */
  PHASE_CERTIFICATE(
      "/v1/nexus/private-settlements/phases/certificates",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("manifest", "payload_digest", "certificate"),
      8 * 1024 * 1024),
  /** Promote one availability-certified encrypted leg. */
  LEG_UPLOAD(
      "/v1/nexus/private-settlements/legs",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("manifest", "audit_policy", "committee_authority", "payload"),
      32 * 1024 * 1024),
  /** Submit one purpose-separated governed auditor approval. */
  AUDIT_APPROVAL(
      "/v1/nexus/private-settlements/legs/{payload_digest}/audit-approvals",
      AtomicPrivateSettlementAuthV1.ROLE_IDENTITY,
      Set.of("approval"),
      2 * 1024 * 1024),
  /** Submit one sponsor-signed exact global finalization or abort carrier. */
  BUNDLE_SUBMIT(
      "/v1/nexus/private-settlements/bundles",
      AtomicPrivateSettlementAuthV1.SPONSOR,
      Set.of("transaction"),
      8 * 1024 * 1024);

  private final String path;
  private final AtomicPrivateSettlementAuthV1 auth;
  private final Set<String> topLevelFields;
  private final int maximumRequestBytes;

  AtomicPrivateSettlementOperationV1(
      final String path,
      final AtomicPrivateSettlementAuthV1 auth,
      final Set<String> topLevelFields,
      final int maximumRequestBytes) {
    this.path = path;
    this.auth = auth;
    this.topLevelFields = topLevelFields;
    this.maximumRequestBytes = maximumRequestBytes;
  }

  /** Returns the exact route, with a payload placeholder only for auditor approval. */
  public String path() {
    return path;
  }

  /** Returns the exact required authentication class. */
  public AtomicPrivateSettlementAuthV1 auth() {
    return auth;
  }

  Set<String> topLevelFields() {
    return topLevelFields;
  }

  int maximumRequestBytes() {
    return maximumRequestBytes;
  }
}
