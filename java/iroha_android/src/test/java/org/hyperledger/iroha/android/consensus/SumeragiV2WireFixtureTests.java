// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.Test;

/** Cross-SDK checks pinned to payloads emitted by the Rust production types. */
public final class SumeragiV2WireFixtureTests {
  private static final String FIXTURE_RELATIVE_PATH = "fixtures/sumeragi_v2/wire_v2.tsv";
  private static final Set<String> EXPECTED_MESSAGE_NAMES =
      new HashSet<>(
          Arrays.asList(
              "proposal",
              "vote",
              "quorum_certificate",
              "timeout_vote",
              "timeout_certificate",
              "payload_manifest",
              "payload_chunk",
              "certified_body_request",
              "certified_body_response",
              "commit_certificate_request",
              "commit_certificate_response"));

  @Test
  public void rustCanonicalMessageFixturesRoundtrip() throws Exception {
    Set<String> names = new HashSet<>();
    for (FixtureRow row : fixtureRows()) {
      if (!row.kind.equals("message")) continue;
      names.add(row.name);
      byte[] encoded = hexBytes(row.hex);
      SumeragiV2Wire.ConsensusMessageV2 decoded =
          SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(encoded);
      assertArrayEquals(row.name, encoded, decoded.encode());
    }
    assertEquals(EXPECTED_MESSAGE_NAMES, names);
  }

  @Test
  public void timeoutVoteCarriesTheCompletePrepareCertificate() throws Exception {
    SumeragiV2Wire.ConsensusPayload.TimeoutVoteMessage timeoutPayload =
        (SumeragiV2Wire.ConsensusPayload.TimeoutVoteMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                    hexBytes(fixtureRow("message", "timeout_vote").hex))
                .payload;
    SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage preparePayload =
        (SumeragiV2Wire.ConsensusPayload.QuorumCertificateMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                    hexBytes(fixtureRow("message", "quorum_certificate").hex))
                .payload;

    SumeragiV2Wire.QuorumCertificate embeddedPrepare = timeoutPayload.value.highestPrepareQc;
    if (embeddedPrepare == null) {
      throw new AssertionError("timeout vote omitted its highest PrepareQC");
    }
    assertArrayEquals(preparePayload.value.encode(), embeddedPrepare.encode());
    assertEquals(Arrays.asList(0L, 1L, 2L), embeddedPrepare.signers);
    assertEquals(48, embeddedPrepare.aggregateSignature().length);

    byte[] changedSignature = embeddedPrepare.aggregateSignature();
    changedSignature[0] ^= 1;
    SumeragiV2Wire.QuorumCertificate changedPrepare =
        new SumeragiV2Wire.QuorumCertificate(
            embeddedPrepare.round,
            embeddedPrepare.phase,
            embeddedPrepare.subject,
            embeddedPrepare.executionCommitment,
            embeddedPrepare.signers,
            changedSignature);
    SumeragiV2Wire.TimeoutVote changedVote =
        new SumeragiV2Wire.TimeoutVote(
            timeoutPayload.value.round,
            changedPrepare,
            timeoutPayload.value.signer,
            timeoutPayload.value.signature());
    if (Arrays.equals(timeoutPayload.value.encode(), changedVote.encode())) {
      throw new AssertionError(
          "timeout-vote wire bytes did not bind the embedded PrepareQC evidence");
    }
  }

  @Test
  public void commitCertificateSigningPreimagesMatchRustExactly() throws Exception {
    FixtureRow requestMessage = fixtureRow("message", "commit_certificate_request");
    FixtureRow responseMessage = fixtureRow("message", "commit_certificate_response");
    FixtureRow requestPreimage = fixtureRow("preimage", "commit_certificate_request");
    FixtureRow responsePreimage = fixtureRow("preimage", "commit_certificate_response");

    SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage requestPayload =
        (SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(hexBytes(requestMessage.hex)).payload;
    SumeragiV2Wire.CommitCertificateRequest request = requestPayload.value;
    assertEquals(SumeragiV2Wire.PROTOCOL_VERSION, request.protocolVersion);
    assertEquals("sumeragi-v2-test", request.chainId.value);
    assertEquals(1L, request.height);
    assertEquals(48, request.signature().length);
    assertArrayEquals(hexBytes(requestPreimage.hex), request.signaturePreimage());
    SumeragiV2Wire.CommitCertificateRequest reSignedRequest =
        new SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.chainId,
            request.contextId,
            request.height,
            request.requester,
            new byte[] {1});
    assertArrayEquals(request.signaturePreimage(), reSignedRequest.signaturePreimage());
    SumeragiV2Wire.CommitCertificateRequest crossChainRequest =
        new SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            new SumeragiV2Wire.ChainId("other-chain"),
            request.contextId,
            request.height,
            request.requester,
            new byte[] {1});
    if (Arrays.equals(request.signaturePreimage(), crossChainRequest.signaturePreimage())) {
      throw new AssertionError("commit request signature preimage did not bind chain ID");
    }

    SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage responsePayload =
        (SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(hexBytes(responseMessage.hex)).payload;
    SumeragiV2Wire.CommitCertificateResponse response = responsePayload.value;
    assertEquals(SumeragiV2Wire.GlobalPhase.COMMIT, response.certificate.phase);
    assertEquals(48, response.signature().length);
    assertEquals(response.requestHash, request.requestHash());
    response.validateAgainst(request);
    assertArrayEquals(hexBytes(responsePreimage.hex), response.signaturePreimage());
    SumeragiV2Wire.CommitCertificateResponse reSignedResponse =
        new SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash, response.certificate, response.responder, new byte[] {1});
    assertArrayEquals(response.signaturePreimage(), reSignedResponse.signaturePreimage());
    assertThrows(IllegalArgumentException.class, () -> response.validateAgainst(reSignedRequest));
    SumeragiV2Wire.CommitCertificateResponse changedResponder =
        new SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash, response.certificate, request.requester, new byte[] {1});
    if (Arrays.equals(response.signaturePreimage(), changedResponder.signaturePreimage())) {
      throw new AssertionError("commit response signature preimage did not bind responder");
    }

    byte[] changedContextBytes = request.contextId.hash.bytes();
    changedContextBytes[0] ^= 1;
    SumeragiV2Wire.CommitCertificateRequest changedContextRequest =
        new SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.chainId,
            new SumeragiV2Wire.HeightContextId(new SumeragiV2Wire.Hash32(changedContextBytes)),
            request.height,
            request.requester,
            request.signature());
    SumeragiV2Wire.CommitCertificateResponse mismatchedContextResponse =
        new SumeragiV2Wire.CommitCertificateResponse(
            changedContextRequest.requestHash(),
            response.certificate,
            response.responder,
            response.signature());
    assertThrows(
        IllegalArgumentException.class,
        () -> mismatchedContextResponse.validateAgainst(changedContextRequest));

    SumeragiV2Wire.CommitCertificateRequest changedHeightRequest =
        new SumeragiV2Wire.CommitCertificateRequest(
            request.protocolVersion,
            request.chainId,
            request.contextId,
            request.height + 1,
            request.requester,
            request.signature());
    SumeragiV2Wire.CommitCertificateResponse mismatchedHeightResponse =
        new SumeragiV2Wire.CommitCertificateResponse(
            changedHeightRequest.requestHash(),
            response.certificate,
            response.responder,
            response.signature());
    assertThrows(
        IllegalArgumentException.class,
        () -> mismatchedHeightResponse.validateAgainst(changedHeightRequest));

    SumeragiV2Wire.BlockSubject changedSubject =
        new SumeragiV2Wire.BlockSubject(
            response.certificate.subject.parentBlockHash,
            response.certificate.subject.payloadHash,
            response.certificate.subject.blockHash);
    SumeragiV2Wire.QuorumCertificate changedSubjectCertificate =
        new SumeragiV2Wire.QuorumCertificate(
            response.certificate.round,
            response.certificate.phase,
            changedSubject,
            response.certificate.executionCommitment,
            response.certificate.signers,
            response.certificate.aggregateSignature());
    SumeragiV2Wire.CommitCertificateResponse changedSubjectResponse =
        new SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            changedSubjectCertificate,
            response.responder,
            response.signature());
    if (Arrays.equals(response.signaturePreimage(), changedSubjectResponse.signaturePreimage())) {
      throw new AssertionError("commit response signature preimage did not bind QC subject");
    }

    byte[] changedParentState = response.certificate.executionCommitment.parentStateRoot.bytes();
    changedParentState[0] ^= 1;
    SumeragiV2Wire.ExecutionCommitment changedExecutionCommitment =
        SumeragiV2Wire.ExecutionCommitment.withoutTopups(
            new SumeragiV2Wire.Hash32(changedParentState),
            response.certificate.executionCommitment.postStateRoot,
            response.certificate.executionCommitment.ordinaryWritesRoot);
    SumeragiV2Wire.QuorumCertificate changedExecutionCertificate =
        new SumeragiV2Wire.QuorumCertificate(
            response.certificate.round,
            response.certificate.phase,
            response.certificate.subject,
            changedExecutionCommitment,
            response.certificate.signers,
            response.certificate.aggregateSignature());
    SumeragiV2Wire.CommitCertificateResponse changedExecutionResponse =
        new SumeragiV2Wire.CommitCertificateResponse(
            response.requestHash,
            changedExecutionCertificate,
            response.responder,
            response.signature());
    if (Arrays.equals(response.signaturePreimage(), changedExecutionResponse.signaturePreimage())) {
      throw new AssertionError(
          "commit response signature preimage did not bind the execution commitment");
    }
  }

  @Test
  public void executionCommitmentsRejectNonCanonicalTopupBindings() throws Exception {
    FixtureRow responseMessage = fixtureRow("message", "commit_certificate_response");
    SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage responsePayload =
        (SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(
                    hexBytes(responseMessage.hex))
                .payload;
    SumeragiV2Wire.ExecutionCommitment base =
        responsePayload.value.certificate.executionCommitment;
    SumeragiV2Wire.Hash32 topupRoot = base.parentStateRoot;

    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                0));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                null,
                1));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                SumeragiV2Wire.MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK + 1));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SumeragiV2Wire.ExecutionCommitment(
                base.parentStateRoot,
                base.postStateRoot,
                base.ordinaryWritesRoot,
                topupRoot,
                1));

    SumeragiV2Wire.Hash32 canonicalPostState =
        SumeragiV2Wire.ExecutionCommitment.topupPostStateRoot(
            1, base.ordinaryWritesRoot, topupRoot);
    SumeragiV2Wire.ExecutionCommitment valid =
        new SumeragiV2Wire.ExecutionCommitment(
            base.parentStateRoot,
            canonicalPostState,
            base.ordinaryWritesRoot,
            topupRoot,
            1);
    assertArrayEquals(
        valid.encode(), SumeragiV2Wire.ExecutionCommitment.decode(valid.encode()).encode());
  }

  @Test
  public void rustCanonicalCompactStatusFixtureRoundtrips() throws Exception {
    FixtureRow status = null;
    for (FixtureRow row : fixtureRows()) {
      if (row.kind.equals("status") && row.name.equals("compact")) status = row;
    }
    if (status == null) throw new AssertionError("missing compact status fixture");
    byte[] encoded = hexBytes(status.hex);
    SumeragiV2Wire.SumeragiV2Status decoded =
        SumeragiV2Wire.SumeragiV2Status.decodeCanonical(encoded);
    assertArrayEquals(encoded, decoded.encode());
    assertEquals(SumeragiV2Wire.PROTOCOL_VERSION, decoded.protocolVersion);
    assertEquals(1L, decoded.height);
    assertEquals(3L, decoded.view);
    assertEquals(SumeragiV2Wire.StatusPhase.PREPARE, decoded.phase);
    assertEquals(2L, decoded.leader);
    assertEquals(SumeragiV2Wire.BodyState.VALIDATED, decoded.bodyState);
    assertEquals(Long.valueOf(17L), decoded.pendingPersistenceId);
    assertEquals(0L, decoded.lastCommittedHeight);
    if (decoded.lockedPrepareQc == null
        || decoded.highestPrepareQc == null
        || decoded.lastTimeoutCertificate == null
        || decoded.lastCommittedSubject == null) {
      throw new AssertionError("compact status fixture omitted a required populated diagnostic");
    }
  }

  @Test
  public void malformedAndSemanticallyNoncanonicalFixturesFailClosed() throws Exception {
    for (FixtureRow row : fixtureRows()) {
      if (row.kind.equals("negative_message")) {
        assertThrows(
            row.name,
            IllegalArgumentException.class,
            () -> SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(hexBytes(row.hex)));
      } else if (row.kind.equals("negative_status")) {
        assertThrows(
            row.name,
            IllegalArgumentException.class,
            () -> SumeragiV2Wire.SumeragiV2Status.decodeCanonical(hexBytes(row.hex)));
      }
    }
  }

  @Test
  public void commitCertificateBindingCorruptionsFailAgainstExactRequest() throws Exception {
    FixtureRow requestRow = fixtureRow("message", "commit_certificate_request");
    SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage requestPayload =
        (SumeragiV2Wire.ConsensusPayload.CommitCertificateRequestMessage)
            SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(hexBytes(requestRow.hex)).payload;

    for (FixtureRow row : fixtureRows()) {
      if (!row.kind.equals("negative_binding")) continue;
      SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage responsePayload =
          (SumeragiV2Wire.ConsensusPayload.CommitCertificateResponseMessage)
              SumeragiV2Wire.ConsensusMessageV2.decodeCanonical(hexBytes(row.hex)).payload;
      assertThrows(
          row.name,
          IllegalArgumentException.class,
          () -> responsePayload.value.validateAgainst(requestPayload.value));
    }
  }

  private static List<FixtureRow> fixtureRows() throws IOException {
    List<FixtureRow> rows = new ArrayList<>();
    for (String line : Files.readAllLines(fixturePath(), StandardCharsets.UTF_8)) {
      if (line.trim().isEmpty() || line.startsWith("#")) continue;
      String[] columns = line.split("\\t", -1);
      if (columns.length != 4) throw new IllegalArgumentException("Malformed fixture row");
      if (!columns[3].equals("accept") && !columns[3].equals("reject")) {
        throw new IllegalArgumentException("Unknown fixture expectation");
      }
      rows.add(new FixtureRow(columns[0], columns[1], columns[2]));
    }
    return rows;
  }

  private static FixtureRow fixtureRow(String kind, String name) throws IOException {
    FixtureRow found = null;
    for (FixtureRow row : fixtureRows()) {
      if (!row.kind.equals(kind) || !row.name.equals(name)) continue;
      if (found != null) throw new AssertionError("duplicate fixture row " + kind + "/" + name);
      found = row;
    }
    if (found == null) throw new AssertionError("missing fixture row " + kind + "/" + name);
    return found;
  }

  private static Path fixturePath() {
    Path directory = Paths.get("").toAbsolutePath().normalize();
    while (directory != null) {
      Path candidate = directory.resolve(FIXTURE_RELATIVE_PATH);
      if (Files.isRegularFile(candidate)) return candidate;
      directory = directory.getParent();
    }
    throw new IllegalStateException("Unable to locate " + FIXTURE_RELATIVE_PATH);
  }

  private static byte[] hexBytes(String hex) {
    if ((hex.length() & 1) != 0) throw new IllegalArgumentException("hex has odd length");
    byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      int offset = i * 2;
      bytes[i] = (byte) Integer.parseInt(hex.substring(offset, offset + 2), 16);
    }
    return bytes;
  }

  private static final class FixtureRow {
    final String kind;
    final String name;
    final String hex;

    FixtureRow(String kind, String name, String hex) {
      this.kind = kind;
      this.name = name;
      this.hex = hex;
    }
  }
}
