package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/** Pilot policy defaults for Offline Bearer Cash v1 handoffs. */
public final class OfflineBearerCashPolicyV1 {
  public static final OfflineBearerCashPolicyV1 DEFAULT = new OfflineBearerCashPolicyV1();

  private final int maxCustodyHops;
  private final int maxLineageSteps;
  private final int maxSingleQrPayloadBytes;
  private final int maxStreamPayloadBytes;
  private final int androidKeyPoolTarget;
  private final int androidKeyPoolReplenishBelow;
  private final int androidKeyPoolCap;

  public OfflineBearerCashPolicyV1() {
    this(5, 32, 2048, 12288, 20, 8, 40);
  }

  public OfflineBearerCashPolicyV1(
      final int maxCustodyHops,
      final int maxLineageSteps,
      final int maxSingleQrPayloadBytes,
      final int maxStreamPayloadBytes,
      final int androidKeyPoolTarget,
      final int androidKeyPoolReplenishBelow,
      final int androidKeyPoolCap) {
    requirePositive(maxCustodyHops, "maxCustodyHops");
    requirePositive(maxLineageSteps, "maxLineageSteps");
    requirePositive(maxSingleQrPayloadBytes, "maxSingleQrPayloadBytes");
    if (maxStreamPayloadBytes < maxSingleQrPayloadBytes) {
      throw new IllegalArgumentException(
          "maxStreamPayloadBytes must cover maxSingleQrPayloadBytes");
    }
    requirePositive(androidKeyPoolReplenishBelow, "androidKeyPoolReplenishBelow");
    if (androidKeyPoolTarget < androidKeyPoolReplenishBelow) {
      throw new IllegalArgumentException(
          "androidKeyPoolTarget must cover androidKeyPoolReplenishBelow");
    }
    if (androidKeyPoolCap < androidKeyPoolTarget) {
      throw new IllegalArgumentException("androidKeyPoolCap must cover androidKeyPoolTarget");
    }
    this.maxCustodyHops = maxCustodyHops;
    this.maxLineageSteps = maxLineageSteps;
    this.maxSingleQrPayloadBytes = maxSingleQrPayloadBytes;
    this.maxStreamPayloadBytes = maxStreamPayloadBytes;
    this.androidKeyPoolTarget = androidKeyPoolTarget;
    this.androidKeyPoolReplenishBelow = androidKeyPoolReplenishBelow;
    this.androidKeyPoolCap = androidKeyPoolCap;
  }

  public int maxCustodyHops() {
    return maxCustodyHops;
  }

  public int maxLineageSteps() {
    return maxLineageSteps;
  }

  public int maxSingleQrPayloadBytes() {
    return maxSingleQrPayloadBytes;
  }

  public int maxStreamPayloadBytes() {
    return maxStreamPayloadBytes;
  }

  public int androidKeyPoolTarget() {
    return androidKeyPoolTarget;
  }

  public int androidKeyPoolReplenishBelow() {
    return androidKeyPoolReplenishBelow;
  }

  public int androidKeyPoolCap() {
    return androidKeyPoolCap;
  }

  public OfflineBearerCashTransport recommendedTransportForPayloadByteCount(
      final int payloadByteCount) {
    requirePositive(payloadByteCount, "payloadByteCount");
    if (payloadByteCount <= maxSingleQrPayloadBytes) {
      return OfflineBearerCashTransport.STATIC_QR;
    }
    if (payloadByteCount <= maxStreamPayloadBytes) {
      return OfflineBearerCashTransport.STREAMING_QR;
    }
    return OfflineBearerCashTransport.FRAMED_BYTE_TRANSPORT;
  }

  public AuditTrailMetrics auditTrailMetrics(final List<OfflineNote.AuditBundle> audits) {
    return auditTrailMetrics(audits, null);
  }

  public AuditTrailMetrics auditTrailMetrics(
      final List<OfflineNote.AuditBundle> audits,
      final OfflineNote.AuditBundle terminalAudit) {
    Objects.requireNonNull(audits, "audits");
    if (terminalAudit != null
        && (audits.isEmpty()
            || !Arrays.equals(
                audits.get(audits.size() - 1).noritoEncoded(), terminalAudit.noritoEncoded()))) {
      throw new IllegalArgumentException("bearer audit trail must end with terminal audit");
    }
    if (audits.isEmpty()) {
      return new AuditTrailMetrics(0, 0);
    }

    final Set<String> tokenIds = new LinkedHashSet<>();
    final Set<String> nullifiers = new LinkedHashSet<>();
    final Map<String, Integer> outputProducerIndex = new LinkedHashMap<>();
    for (int index = 0; index < audits.size(); index++) {
      final OfflineNote.AuditBundle audit = audits.get(index);
      final String tokenId = OfflineNoteWallet.hexLower(audit.tokenId());
      if (!tokenIds.add(tokenId)) {
        throw new IllegalArgumentException("bearer audit trail has duplicate token id: " + tokenId);
      }
      for (final byte[] nullifier : audit.inputNullifiers()) {
        final String key = OfflineNoteWallet.hexLower(nullifier);
        if (!nullifiers.add(key)) {
          throw new IllegalArgumentException(
              "bearer audit trail has duplicate input nullifier: " + key);
        }
      }
      final Set<String> committed = new LinkedHashSet<>();
      for (final byte[] output : audit.outputCommitments()) {
        committed.add(OfflineNoteWallet.hexLower(output));
      }
      for (final OfflineNote.AuditOutputClaim claim : audit.outputClaims()) {
        final String key = OfflineNoteWallet.hexLower(claim.noteCommitment());
        if (!committed.contains(key)) {
          throw new IllegalArgumentException(
              "bearer audit trail output claim is not committed: " + key);
        }
      }
      for (final byte[] output : audit.outputCommitments()) {
        final String key = OfflineNoteWallet.hexLower(output);
        if (outputProducerIndex.containsKey(key)) {
          throw new IllegalArgumentException(
              "bearer audit trail has duplicate output commitment: " + key);
        }
        outputProducerIndex.put(key, index);
      }
    }

    final List<Integer> depths = new ArrayList<>(audits.size());
    int maxDepth = 0;
    for (int index = 0; index < audits.size(); index++) {
      final OfflineNote.AuditBundle audit = audits.get(index);
      int parentDepth = 0;
      for (final OfflineNote.IssuedClaim claim : audit.inputClaims()) {
        final String key = OfflineNoteWallet.hexLower(claim.noteCommitment());
        final Integer producerIndex = outputProducerIndex.get(key);
        if (producerIndex == null) {
          continue;
        }
        if (producerIndex >= index) {
          throw new IllegalArgumentException(
              "bearer audit trail input claim is out of order: " + key);
        }
        parentDepth = Math.max(parentDepth, depths.get(producerIndex));
      }
      final int depth = parentDepth + 1;
      depths.add(depth);
      maxDepth = Math.max(maxDepth, depth);
    }

    return new AuditTrailMetrics(maxDepth, audits.size());
  }

  public AuditTrailMetrics validateAuditTrail(final List<OfflineNote.AuditBundle> audits) {
    return validateAuditTrail(audits, null);
  }

  public AuditTrailMetrics validateAuditTrail(
      final List<OfflineNote.AuditBundle> audits,
      final OfflineNote.AuditBundle terminalAudit) {
    final AuditTrailMetrics metrics = auditTrailMetrics(audits, terminalAudit);
    if (metrics.custodyHops() > maxCustodyHops) {
      throw new IllegalArgumentException(
          "bearer audit trail custody hops "
              + metrics.custodyHops()
              + " exceed maxCustodyHops "
              + maxCustodyHops);
    }
    if (metrics.lineageSteps() > maxLineageSteps) {
      throw new IllegalArgumentException(
          "bearer audit trail lineage steps "
              + metrics.lineageSteps()
              + " exceed maxLineageSteps "
              + maxLineageSteps);
    }
    return metrics;
  }

  private static void requirePositive(final int value, final String field) {
    if (value <= 0) {
      throw new IllegalArgumentException(field + " must be positive");
    }
  }

  /** Derived policy metrics for an ordered Offline Bearer Cash audit trail. */
  public static final class AuditTrailMetrics {
    private final int custodyHops;
    private final int lineageSteps;

    public AuditTrailMetrics(final int custodyHops, final int lineageSteps) {
      this.custodyHops = custodyHops;
      this.lineageSteps = lineageSteps;
    }

    public int custodyHops() {
      return custodyHops;
    }

    public int lineageSteps() {
      return lineageSteps;
    }
  }
}
