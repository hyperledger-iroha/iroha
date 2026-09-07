// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.AbstractList;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.function.Consumer;
import org.junit.jupiter.api.Test;

/** Canonical consensus vectors own their validated inputs and bound allocation by encoded bytes. */
final class SumeragiV2WireOwnershipTest {
  @Test
  void quorumCertificateOwnsItsSignerOrderAfterConstruction() throws IOException {
    SumeragiV2Wire.QuorumCertificate base = certificate();
    List<Long> signers = new ArrayList<>(Arrays.asList(0L, 1L, 2L));
    SumeragiV2Wire.QuorumCertificate value = certificateWithSigners(base, signers);
    byte[] wire = value.encode();
    int hash = value.hashCode();
    signers.set(1, 0L);
    assertArrayEquals(wire, value.encode());
    assertEquals(hash, value.hashCode());
    assertEquals(Arrays.asList(0L, 1L, 2L), value.signers);
    assertThrows(UnsupportedOperationException.class, () -> value.signers.set(1, 0L));
    assertThrows(UnsupportedOperationException.class, () -> value.signers.clear());
    assertThrows(IllegalArgumentException.class, () -> certificateWithSigners(base, signers));
  }

  @Test
  void timeoutGroupOwnsItsNonemptySignerOrder() {
    List<Long> signers = new ArrayList<>(Arrays.asList(0L, 1L));
    SumeragiV2Wire.TimeoutVoteGroup value =
        new SumeragiV2Wire.TimeoutVoteGroup(null, signers, new byte[] {1});
    byte[] wire = value.encode();
    int hash = value.hashCode();
    signers.clear();
    assertArrayEquals(wire, value.encode());
    assertEquals(hash, value.hashCode());
    assertEquals(Arrays.asList(0L, 1L), value.signers);
    assertThrows(UnsupportedOperationException.class, () -> value.signers.set(1, 0L));
    assertThrows(UnsupportedOperationException.class, () -> value.signers.clear());
    assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutVoteGroup(null, Arrays.asList(1L, 0L), new byte[] {1}));
  }

  @Test
  void timeoutCertificateOwnsItsGroupsAndTheirSignerMembership() throws IOException {
    SumeragiV2Wire.QuorumCertificate base = certificate();
    SumeragiV2Wire.TimeoutVoteGroup first =
        new SumeragiV2Wire.TimeoutVoteGroup(null, Arrays.asList(0L, 1L), new byte[] {1});
    SumeragiV2Wire.TimeoutVoteGroup second =
        new SumeragiV2Wire.TimeoutVoteGroup(null, Arrays.asList(2L, 3L), new byte[] {2});
    List<SumeragiV2Wire.TimeoutVoteGroup> groups = new ArrayList<>(Arrays.asList(first, second));
    SumeragiV2Wire.TimeoutCertificate value = new SumeragiV2Wire.TimeoutCertificate(base.round, groups);
    byte[] wire = value.encode();
    int hash = value.hashCode();
    groups.set(1, first);
    assertArrayEquals(wire, value.encode());
    assertEquals(hash, value.hashCode());
    assertEquals(Arrays.asList(first, second), value.groups);
    assertThrows(UnsupportedOperationException.class, () -> value.groups.set(1, first));
    assertThrows(UnsupportedOperationException.class, () -> value.groups.get(1).signers.set(0, 0L));
    assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutCertificate(base.round, groups));
  }

  @Test
  void payloadManifestOwnsItsNonemptyChunkCommitments() throws IOException {
    SumeragiV2Wire.PayloadManifest base = manifest();
    List<SumeragiV2Wire.Hash32> hashes =
        new ArrayList<>(Arrays.asList(base.chunkRoot, base.subject.blockHash));
    SumeragiV2Wire.PayloadManifest value = manifestWithHashes(base, hashes);
    byte[] wire = value.encode();
    int hash = value.hashCode();
    hashes.clear();
    assertArrayEquals(wire, value.encode());
    assertEquals(hash, value.hashCode());
    assertEquals(2, value.chunkHashes.size());
    assertThrows(UnsupportedOperationException.class, () -> value.chunkHashes.clear());
    assertThrows(UnsupportedOperationException.class, () -> value.chunkHashes.set(0, base.subject.blockHash));
  }

  @Test
  void livenessStatusOwnsAllSixInputVectors() throws IOException {
    SumeragiV2Wire.LivenessStatus base = status().liveness;
    List<SumeragiV2Wire.VoteQuorumStatus> prepare = new ArrayList<>(base.prepareQuorums);
    List<SumeragiV2Wire.VoteQuorumStatus> commit = new ArrayList<>(base.commitQuorums);
    List<SumeragiV2Wire.TimeoutQuorumStatus> timeout = new ArrayList<>(base.timeoutQuorums);
    List<SumeragiV2Wire.OutboundIntentStatus> outbound = new ArrayList<>(base.outboundIntents);
    List<SumeragiV2Wire.QueueStatus> queues = new ArrayList<>(base.queues);
    List<SumeragiV2Wire.IgnoreCount> ignores = new ArrayList<>(Collections.singletonList(
        new SumeragiV2Wire.IgnoreCount(SumeragiV2Wire.IgnoreReason.UNSAFE_PROPOSAL, 1)));
    SumeragiV2Wire.LivenessStatus value = new SumeragiV2Wire.LivenessStatus(
        base.generation, prepare, commit, timeout, outbound, base.work, queues,
        base.lastProgress, base.noProgressAgeMs, base.blocker, ignores);
    byte[] wire = value.encode();
    int hash = value.hashCode();
    for (List<?> input : Arrays.asList(prepare, commit, timeout, outbound, queues, ignores)) {
      assertEquals(1, input.size(), "fixture exercises every populated input vector");
      input.clear();
    }
    assertArrayEquals(wire, value.encode());
    assertEquals(hash, value.hashCode());
    for (List<?> retained : Arrays.asList(value.prepareQuorums, value.commitQuorums,
        value.timeoutQuorums, value.outboundIntents, value.queues, value.ignoreCounts)) {
      assertEquals(1, retained.size());
      assertThrows(UnsupportedOperationException.class, retained::clear);
    }
  }

  @Test
  void everyWireVectorRejectsJavaNullElementsAtConstruction() throws IOException {
    SumeragiV2Wire.QuorumCertificate certificate = certificate();
    SumeragiV2Wire.PayloadManifest manifest = manifest();
    assertThrows(IllegalArgumentException.class,
        () -> certificateWithSigners(certificate, Collections.singletonList(null)));
    assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutVoteGroup(null, Collections.singletonList(null), new byte[] {1}));
    assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutCertificate(certificate.round, Collections.singletonList(null)));
    assertThrows(IllegalArgumentException.class,
        () -> manifestWithHashes(manifest, Collections.singletonList(null)));
    SumeragiV2Wire.LivenessStatus base = status().liveness;
    for (int index = 0; index < 6; index++) {
      final int field = index;
      assertThrows(IllegalArgumentException.class, () -> new SumeragiV2Wire.LivenessStatus(
          base.generation,
          field == 0 ? Collections.singletonList(null) : base.prepareQuorums,
          field == 1 ? Collections.singletonList(null) : base.commitQuorums,
          field == 2 ? Collections.singletonList(null) : base.timeoutQuorums,
          field == 3 ? Collections.singletonList(null) : base.outboundIntents,
          base.work,
          field == 4 ? Collections.singletonList(null) : base.queues,
          base.lastProgress, base.noProgressAgeMs, base.blocker,
          field == 5 ? Collections.singletonList(null) : base.ignoreCounts));
    }
  }

  @Test
  void nonemptyVectorRequirementsRejectBeforeTraversingInputs() throws IOException {
    SumeragiV2Wire.QuorumCertificate certificate = certificate();
    SumeragiV2Wire.PayloadManifest manifest = manifest();
    assertEquals("timeout group must contain a signer", assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutVoteGroup(null, emptyUntraversableList(), new byte[] {1})).getMessage());
    assertEquals("timeout certificate must contain a group", assertThrows(IllegalArgumentException.class,
        () -> new SumeragiV2Wire.TimeoutCertificate(certificate.round, emptyUntraversableList())).getMessage());
    assertEquals("payload manifest must contain a chunk hash", assertThrows(IllegalArgumentException.class,
        () -> manifestWithHashes(manifest, emptyUntraversableList())).getMessage());
  }

  @Test
  void everyConsensusVectorRejectsCountsWithoutEnoughEncodedFields() throws IOException {
    byte[] certificate = fixture("message", "quorum_certificate");
    assertHostileVectorCounts("qc.signers", vector ->
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
            replaceMessageBodyField(certificate, vector, 5)));

    byte[] timeout = fixture("message", "timeout_certificate");
    assertHostileVectorCounts("tc.groups", vector ->
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
            replaceMessageBodyField(timeout, vector, 1)));
    byte[] group = new SumeragiV2Wire.TimeoutVoteGroup(
        null, Collections.singletonList(0L), new byte[] {1}).encode();
    assertHostileVectorCounts("timeout_group.signers", vector ->
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(replaceMessageBodyField(timeout,
            concatenate(u64(1), compact(replaceCompactField(group, vector, 1))), 1)));

    byte[] proposal = fixture("message", "proposal");
    assertHostileVectorCounts("manifest.chunk_hashes", vector ->
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
            replaceMessageBodyField(proposal, vector, 3, 4)));
  }

  @Test
  void everyLivenessVectorRejectsCountsWithoutEnoughEncodedFields() throws IOException {
    byte[] status = fixture("status", "compact");
    int[] fields = {1, 2, 3, 4, 6, 10};
    String[] labels = {"prepare", "commit", "timeout", "outbound", "queues", "ignore_counts"};
    for (int index = 0; index < fields.length; index++) {
      final int field = fields[index];
      assertHostileVectorCounts("status.liveness." + labels[index], vector ->
          SumeragiV2Wire.SumeragiV2Status.decodeCanonical(
              replaceCompactField(status, vector, 19, field)));
    }
  }

  @Test
  void zeroCountVectorsRemainCanonicalWhenTheirModelAllowsEmptyCollections() throws IOException {
    byte[] certificate = replaceMessageBodyField(
        fixture("message", "quorum_certificate"), u64(0), 5);
    SumeragiV2Wire.ConsensusMessageV2 decodedCertificate =
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(certificate);
    assertEquals(Collections.emptyList(),
        ((SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage)
            decodedCertificate.payload).value.signers);
    assertArrayEquals(certificate, decodedCertificate.encode());

    byte[] status = fixture("status", "compact");
    for (int field : new int[] {1, 2, 3, 4, 6, 10}) {
      status = replaceCompactField(status, u64(0), 19, field);
    }
    SumeragiV2Wire.SumeragiV2Status decodedStatus =
        SumeragiV2Wire.SumeragiV2Status.decodeCanonical(status);
    for (List<?> values : Arrays.asList(decodedStatus.liveness.prepareQuorums,
        decodedStatus.liveness.commitQuorums, decodedStatus.liveness.timeoutQuorums,
        decodedStatus.liveness.outboundIntents, decodedStatus.liveness.queues,
        decodedStatus.liveness.ignoreCounts)) {
      assertEquals(Collections.emptyList(), values);
    }
    assertArrayEquals(status, decodedStatus.encode());
  }

  @Test
  void elementLengthPrefixCountsEvenWhenItsPayloadIsEmpty() throws IOException {
    byte[] certificate = fixture("message", "quorum_certificate");
    byte[] emptyElement = concatenate(u64(1), compact(new byte[0]));
    // The generic vector bound accepts the one encoded field. The u32 element
    // decoder then rejects its empty payload according to that element's schema.
    assertEquals("qc.signer is truncated", assertThrows(IllegalArgumentException.class,
        () -> SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
            replaceMessageBodyField(certificate, emptyElement, 5))).getMessage());
  }

  private static void assertHostileVectorCounts(String label, Consumer<byte[]> decode) {
    for (long count : new long[] {1, 2, 1_048_576, Integer.MAX_VALUE}) {
      assertEquals(label + " count exceeds remaining encoded fields",
          assertThrows(IllegalArgumentException.class, () -> decode.accept(u64(count))).getMessage(),
          label + " rejects count " + count + " from an eight-byte vector before allocation");
    }
    assertEquals(label + " count exceeds remaining encoded fields",
        assertThrows(IllegalArgumentException.class,
            () -> decode.accept(concatenate(u64(2), compact(new byte[0])))).getMessage(),
        label + " requires one length prefix for each declared element");
    for (long count : new long[] {1L + Integer.MAX_VALUE, Long.MAX_VALUE, -1L}) {
      assertEquals(label + " count exceeds JVM range",
          assertThrows(IllegalArgumentException.class, () -> decode.accept(u64(count))).getMessage(),
          label + " retains unsigned count and JVM range validation");
    }
  }

  private static <T> List<T> emptyUntraversableList() {
    return new AbstractList<T>() {
      @Override public int size() { return 0; }
      @Override public T get(int index) { throw new AssertionError("empty input must not be read"); }
      @Override public Iterator<T> iterator() {
        throw new AssertionError("empty input must be rejected before copying");
      }
    };
  }

  private static SumeragiV2Wire.QuorumCertificate certificateWithSigners(
      SumeragiV2Wire.QuorumCertificate base, List<Long> signers) {
    return new SumeragiV2Wire.QuorumCertificate(base.round, base.proposalRound, base.phase,
        base.subject, base.executionCommitment, signers, base.aggregateSignature());
  }

  private static SumeragiV2Wire.PayloadManifest manifestWithHashes(
      SumeragiV2Wire.PayloadManifest base, List<SumeragiV2Wire.Hash32> hashes) {
    return new SumeragiV2Wire.PayloadManifest(base.round, base.subject, base.payloadSizeBytes,
        base.layout, hashes, base.chunkRoot);
  }

  private static SumeragiV2Wire.QuorumCertificate certificate() throws IOException {
    return ((SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage)
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(fixture("message", "quorum_certificate")).payload).value;
  }

  private static SumeragiV2Wire.PayloadManifest manifest() throws IOException {
    return ((SumeragiV2Wire.ConsensusPayload.ProposalMessage)
        SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(fixture("message", "proposal")).payload).value.manifest;
  }

  private static SumeragiV2Wire.SumeragiV2Status status() throws IOException {
    return SumeragiV2Wire.SumeragiV2Status.decodeCanonical(fixture("status", "compact"));
  }

  private static byte[] replaceMessageBodyField(byte[] message, byte[] value, int... fields) {
    byte[] payload = compactFieldBytes(message, 1);
    byte[] body = compactFieldBytes(Arrays.copyOfRange(payload, 4, payload.length), 0);
    return replaceCompactField(message, concatenate(Arrays.copyOf(payload, 4),
        compact(replaceCompactField(body, value, fields))), 1);
  }

  private static byte[] replaceCompactField(byte[] bytes, byte[] value, int... fields) {
    if (fields.length == 0) throw new AssertionError("empty compact field path");
    int[] range = compactFieldRange(bytes, fields[0]);
    byte[] replacement = fields.length == 1 ? value : replaceCompactField(
        Arrays.copyOfRange(bytes, range[1], range[2]), value,
        Arrays.copyOfRange(fields, 1, fields.length));
    return concatenate(Arrays.copyOf(bytes, range[0]), compact(replacement),
        Arrays.copyOfRange(bytes, range[2], bytes.length));
  }

  private static byte[] compactFieldBytes(byte[] bytes, int field) {
    int[] range = compactFieldRange(bytes, field);
    return Arrays.copyOfRange(bytes, range[1], range[2]);
  }

  private static int[] compactFieldRange(byte[] bytes, int field) {
    int offset = 0;
    for (int index = 0; index <= field; index++) {
      int start = offset;
      long length = 0;
      int shift = 0;
      int next;
      do {
        if (offset == bytes.length || shift > 28) {
          throw new AssertionError("invalid fixture compact field length");
        }
        next = bytes[offset++] & 0xff;
        length |= ((long) (next & 0x7f)) << shift;
        shift += 7;
      } while ((next & 0x80) != 0);
      if (length > bytes.length - offset) throw new AssertionError("truncated fixture compact field");
      int end = offset + (int) length;
      if (index == field) return new int[] {start, offset, end};
      offset = end;
    }
    throw new AssertionError("invalid fixture field index");
  }

  private static byte[] compact(byte[] bytes) {
    ByteArrayOutputStream output = new ByteArrayOutputStream();
    int remaining = bytes.length;
    while (remaining >= 0x80) {
      output.write((remaining & 0x7f) | 0x80);
      remaining >>>= 7;
    }
    output.write(remaining);
    output.write(bytes, 0, bytes.length);
    return output.toByteArray();
  }

  private static byte[] concatenate(byte[]... parts) {
    ByteArrayOutputStream output = new ByteArrayOutputStream();
    for (byte[] part : parts) output.write(part, 0, part.length);
    return output.toByteArray();
  }

  private static byte[] u64(long value) {
    return ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array();
  }

  private static byte[] fixture(String kind, String name) throws IOException {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      Path fixture = current.resolve("fixtures/sumeragi_v2/wire_v2.tsv");
      if (Files.isRegularFile(fixture)) {
        for (String line : Files.readAllLines(fixture, StandardCharsets.UTF_8)) {
          String[] fields = line.split("\\t", -1);
          if (fields.length != 4 || !fields[0].equals(kind) || !fields[1].equals(name)) continue;
          assertEquals("accept", fields[3], "ownership tests require a canonical positive fixture");
          byte[] bytes = new byte[fields[2].length() / 2];
          for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) Integer.parseInt(fields[2].substring(index * 2, index * 2 + 2), 16);
          }
          return bytes;
        }
        throw new AssertionError("missing Rust-owned wire fixture " + kind + "/" + name);
      }
      current = current.getParent();
    }
    throw new AssertionError("Rust-owned Sumeragi wire fixture was not found");
  }
}
