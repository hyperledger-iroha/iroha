package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPairGenerator;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.InstructionBox;

public final class OfflineNoteV2Test {

  private OfflineNoteV2Test() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    offlineNoteV2ModelsMatchRustNoritoVectors();
    publicNoritoDecodersRoundTripFixturePayloads();
    publicNoritoInstructionDecodersReadExplorerEnvelopeBytes();
    walletDerivationsMatchRustVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    instanceValuesMatchRustVectors();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
    paymentTokenCodecRoundTripsNoritoTextAndQrFrames();
    walletNoteJsonCodecRoundTripsFixtureNote();
    walletLoadDerivesCommitmentBeforeIssuerSubmission();
    toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment();
    walletLifecycleBuildsAuditAcceptAndRedeemTransactions();
    walletSyncReconcilesPendingSpendChangeAndRedeemStates();
    walletRejectsDuplicateTokenAndAlreadyPendingInputs();
    walletSyncReconcilesFailedAuditAndRedeemOutcomes();
    outcomeIndexResolvesCommittedAndRejectedExplorerInstructions();
    System.out.println("[IrohaAndroid] OfflineNoteV2Test passed.");
  }

  private static void certificateSigningBytesMatchRustVector() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final Map<String, Object> certificates = obj(obj(fixture, "chain_vectors"), "certificates");

    assertEquals(
        string(certificates, "sender_payload_base64"),
        base64(sender.signingBytes()),
        "sender certificate payload");
    assertEquals(
        string(certificates, "sender_payload_hash"),
        hex(sender.payloadHash()),
        "sender certificate payload hash");
  }

  private static void offlineNoteV2ModelsMatchRustNoritoVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");

    assertEquals(
        string(obj(chain, "issue"), "norito_base64"),
        base64(issue(fixture).noritoEncoded()),
        "issue norito");
    assertEquals(
        string(obj(chain, "audit"), "norito_base64"),
        base64(audit(fixture).noritoEncoded()),
        "audit norito");
    assertEquals(
        string(obj(chain, "redeem"), "norito_base64"),
        base64(redeem(fixture).noritoEncoded()),
        "redeem norito");
  }

  private static void publicNoritoDecodersRoundTripFixturePayloads() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> certificates = obj(chain, "certificates");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issueVector = obj(chain, "issue");
    final Map<String, Object> redeemVector = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final byte[] senderPayloadBytes = base64Bytes(string(certificates, "sender_payload_base64"));
    final byte[] issueBytes = base64Bytes(string(issueVector, "norito_base64"));
    final byte[] auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"));
    final byte[] redeemBytes = base64Bytes(string(redeemVector, "norito_base64"));

    assertEquals(
        base64(senderPayloadBytes),
        base64(OfflineNoteV2.decodeCertificatePayload(senderPayloadBytes).noritoEncoded()),
        "decoded certificate payload");
    assertEquals(
        base64(senderCertificate.noritoEncoded()),
        base64(OfflineNoteV2.decodeCertificate(senderCertificate.noritoEncoded()).noritoEncoded()),
        "decoded certificate");
    assertEquals(
        base64(issueBytes),
        base64(OfflineNoteV2.decodeIssue(issueBytes).noritoEncoded()),
        "decoded issue");

    final OfflineNoteV2.AuditBundleV2 decodedAudit = OfflineNoteV2.decodeAudit(auditBytes);
    assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()), "decoded audit");
    assertEquals(
        base64(decodedAudit.inputClaims().get(0).noritoEncoded()),
        base64(
            OfflineNoteV2.decodeIssuedClaim(decodedAudit.inputClaims().get(0).noritoEncoded())
                .noritoEncoded()),
        "decoded issued claim");
    assertEquals(
        base64(decodedAudit.publicInputs().noritoEncoded()),
        base64(
            OfflineNoteV2.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded())
                .noritoEncoded()),
        "decoded audit public inputs");

    final OfflineNoteV2.RedeemV2 decodedRedeem = OfflineNoteV2.decodeRedeem(redeemBytes);
    assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()), "decoded redeem");
    assertEquals(
        base64(decodedRedeem.publicInputs().noritoEncoded()),
        base64(
            OfflineNoteV2.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded())
                .noritoEncoded()),
        "decoded redeem public inputs");

    final OfflineNoteV2.NoteCommitmentPreimageV2 commitmentPreimage =
        new OfflineNoteV2.NoteCommitmentPreimageV2(
            string(derivation, "chain_id"),
            hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            string(issueVector, "asset_id"),
            string(redeemVector, "amount"),
            hexBytes(string(derivation, "source_note_secret_hex")),
            new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                string(derivation, "issuer_load_operation_id"),
                string(derivation, "issuer_load_lineage_id"),
                longValue(derivation, "issuer_load_local_revision")));
    assertEquals(
        base64(commitmentPreimage.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded note commitment preimage");

    final OfflineNoteV2.InputNullifierPreimageV2 nullifierPreimage =
        new OfflineNoteV2.InputNullifierPreimageV2(
            string(derivation, "chain_id"),
            hexBytes(string(derivation, "source_note_commitment")),
            hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            hexBytes(string(derivation, "source_note_secret_hex")));
    assertEquals(
        base64(nullifierPreimage.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded input nullifier preimage");

    final OfflineNoteV2.PaymentTokenIdPreimageV2 tokenPreimage =
        new OfflineNoteV2.PaymentTokenIdPreimageV2(
            string(derivation, "chain_id"),
            string(derivation, "payment_request_id"),
            longValue(payment, "created_at_ms"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            Collections.singletonList(hexBytes(string(derivation, "input_nullifier"))),
            Arrays.asList(
                hexBytes(string(derivation, "recipient_output_commitment")),
                hexBytes(string(derivation, "change_output_commitment"))));
    assertEquals(
        base64(tokenPreimage.noritoEncoded()),
        base64(
            OfflineNoteV2.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded payment token id preimage");
  }

  private static void publicNoritoInstructionDecodersReadExplorerEnvelopeBytes() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);

    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeIssueInstruction(
                    rawInstructionPair(
                        OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.issueInstruction(issue))))
                .noritoEncoded()),
        "decoded issue instruction");
    assertEquals(
        base64(audit.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeAuditInstruction(
                    rawInstructionPair(
                        OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.auditInstruction(audit))))
                .noritoEncoded()),
        "decoded audit instruction");
    assertEquals(
        base64(redeem.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeRedeemInstruction(
                    rawInstructionPair(
                        OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem))))
                .noritoEncoded()),
        "decoded redeem instruction");
  }

  private static void walletDerivationsMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issueVector = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final Map<String, Object> recipientOutput = asMap(list(payment, "output_claims").get(0), "recipient output");
    final Map<String, Object> changeOutput = asMap(list(payment, "output_claims").get(1), "change output");
    final String chainId = string(derivation, "chain_id");

    final byte[] sourceCommitment =
        OfflineNoteV2.deriveNoteCommitment(
            new OfflineNoteV2.NoteCommitmentPreimageV2(
                chainId,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                string(issueVector, "asset_id"),
                string(issueVector, "amount"),
                hexBytes(string(derivation, "source_note_secret_hex")),
                new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                    string(derivation, "issuer_load_operation_id"),
                    string(derivation, "issuer_load_lineage_id"),
                    longValue(derivation, "issuer_load_local_revision"))));
    assertEquals(
        string(derivation, "source_note_commitment"),
        hex(sourceCommitment),
        "source note commitment");

    final byte[] inputNullifier =
        OfflineNoteV2.deriveInputNullifier(
            new OfflineNoteV2.InputNullifierPreimageV2(
                chainId,
                sourceCommitment,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                hexBytes(string(derivation, "source_note_secret_hex"))));
    assertEquals(string(derivation, "input_nullifier"), hex(inputNullifier), "input nullifier");

    final byte[] recipientCommitment =
        OfflineNoteV2.deriveNoteCommitment(
            new OfflineNoteV2.NoteCommitmentPreimageV2(
                chainId,
                hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                string(recipientOutput, "asset_definition_id")
                    + "#"
                    + string(recipientOutput, "account_id"),
                string(recipientOutput, "amount"),
                hexBytes(string(derivation, "recipient_note_secret_hex")),
                new OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                    string(derivation, "payment_request_id"), 0)));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        hex(recipientCommitment),
        "recipient output commitment");

    final byte[] changeCommitment =
        OfflineNoteV2.deriveNoteCommitment(
            new OfflineNoteV2.NoteCommitmentPreimageV2(
                chainId,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                string(changeOutput, "asset_definition_id")
                    + "#"
                    + string(changeOutput, "account_id"),
                string(changeOutput, "amount"),
                hexBytes(string(derivation, "change_note_secret_hex")),
                new OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                    string(derivation, "payment_request_id"), 1)));
    assertEquals(
        string(derivation, "change_output_commitment"),
        hex(changeCommitment),
        "change output commitment");

    final byte[] tokenId =
        OfflineNoteV2.derivePaymentTokenId(
            new OfflineNoteV2.PaymentTokenIdPreimageV2(
                chainId,
                string(derivation, "payment_request_id"),
                longValue(payment, "created_at_ms"),
                hexBytes(string(derivation, "token_nonce_hex")),
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                Collections.singletonList(inputNullifier),
                Arrays.asList(recipientCommitment, changeCommitment)));
    assertEquals(string(derivation, "payment_token_id"), hex(tokenId), "payment token id");

    final byte[] redeemNullifier =
        OfflineNoteV2.deriveInputNullifier(
            new OfflineNoteV2.InputNullifierPreimageV2(
                chainId,
                recipientCommitment,
                hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                hexBytes(string(derivation, "recipient_note_secret_hex"))));
    assertEquals(
        string(derivation, "redeem_nullifier"), hex(redeemNullifier), "redeem nullifier");
  }

  private static void publicInputHashesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);

    assertEquals(
        string(obj(chain, "audit"), "public_inputs_hash"),
        hex(audit.publicInputsHash()),
        "audit public inputs hash");
    assertEquals(
        string(obj(chain, "redeem"), "public_inputs_hash"),
        hex(redeem.publicInputsHash()),
        "redeem public inputs hash");
    audit.validateProofBinding();
    redeem.validateProofBinding();
    audit.replacingRecursiveProof(audit.recursiveProof()).validateProofBinding();
    redeem.replacingRecursiveProof(redeem.recursiveProof()).validateProofBinding();
  }

  private static void proofBindingRejectsMismatch() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.RecursiveProofV2 badProof =
        new OfflineNoteV2.RecursiveProofV2(
            OfflineNoteV2.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    final OfflineNoteV2.RedeemV2 forged =
        new OfflineNoteV2.RedeemV2(
            redeem.sourceNoteCommitment(),
            redeem.inputNullifiers(),
            redeem.senderKeyCertificate(),
            redeem.recipient(),
            redeem.assetId(),
            redeem.amount(),
            badProof);

    assertThrows(forged::validateProofBinding, "proof binding mismatch should throw");
  }

  private static void instanceValuesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final OfflineNoteV2.InstanceValues auditValues =
        OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit(fixture));
    final OfflineNoteV2.InstanceValues redeemValues =
        OfflineNoteV2.InstanceBuilder.redeemInstanceValues(redeem(fixture));
    final long[] auditPublic = auditValues.publicValues();
    final long[] redeemPublic = redeemValues.publicValues();

    assertEquals(
        string(obj(chain, "audit"), "public_inputs_hash"),
        hex(hashFromPublicValues(auditPublic)),
        "audit instance public inputs hash limbs");
    assertEquals(
        string(obj(chain, "redeem"), "public_inputs_hash"),
        hex(hashFromPublicValues(redeemPublic)),
        "redeem instance public inputs hash limbs");
    assertEquals(2L, auditPublic[4], "audit mode");
    assertEquals(1L, auditPublic[5], "audit input count");
    assertEquals(2L, auditPublic[6], "audit output count");
    assertEquals(52L, auditPublic[7], "audit input sum");
    assertEquals(52L, auditPublic[8], "audit output sum");
    assertEquals(1L, redeemPublic[4], "redeem mode");
    assertEquals(1L, redeemPublic[5], "redeem input count");
    assertEquals(1L, redeemPublic[6], "redeem output count");
    assertEquals(5L, redeemPublic[7], "redeem input sum");
    assertEquals(5L, redeemPublic[8], "redeem output sum");
    assertEquals(52L, auditValues.inputAmounts()[0], "audit input amount");
    assertEquals(5L, auditValues.outputAmounts()[0], "audit first output amount");
    assertEquals(47L, auditValues.outputAmounts()[1], "audit second output amount");
    assertEquals(5L, redeemValues.inputAmounts()[0], "redeem input amount");
    assertEquals(5L, redeemValues.outputAmounts()[0], "redeem output amount");
    assertEquals(
        hex(OfflineNoteV2.instanceScalarBytes(auditPublic[0])),
        hex(auditValues.publicInstanceColumns().get(0)),
        "audit first instance scalar");
  }

  private static void nativeHalo2ProverProducesVerifyingPayloadWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_OFFLINE_V2_PROVER_TEST"))) {
      return;
    }
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.InstanceValues values =
        OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit);
    OfflineNoteV2Halo2Prover.prewarm();
    final byte[] payload = OfflineNoteV2Halo2Prover.proveZk1Payload(values);

    assertTrue(
        OfflineNoteV2Halo2Prover.verifyZk1Payload(payload, values.publicValues()),
        "Java Offline V2 Halo2 payload verifies");
    final OfflineNoteV2.RecursiveProofV2 proof = OfflineNoteV2Halo2Prover.proveAudit(audit);
    audit.replacingRecursiveProof(proof).validateProofBinding();
    assertTrue(
        proof.proof().bytes().length <= OfflineNoteV2Halo2Prover.MAX_ENVELOPE_BYTES,
        "Java Offline V2 Halo2 envelope fits QR budget");
  }

  private static void nativeHalo2ProverPerformanceWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_OFFLINE_V2_BENCH"))) {
      return;
    }
    final String configuredIterations = System.getenv("IROHA_JAVA_OFFLINE_V2_BENCH_ITERATIONS");
    final int iterations =
        configuredIterations == null ? 20 : Integer.parseInt(configuredIterations);
    assertTrue(iterations > 0, "Java Offline V2 benchmark iterations must be positive");

    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    OfflineNoteV2Halo2Prover.prewarm();
    OfflineNoteV2Halo2Prover.proveAudit(audit);
    OfflineNoteV2Halo2Prover.proveRedeem(redeem);

    final double[] auditSeconds =
        benchmarkSeconds(iterations, () -> OfflineNoteV2Halo2Prover.proveAudit(audit));
    final double[] redeemSeconds =
        benchmarkSeconds(iterations, () -> OfflineNoteV2Halo2Prover.proveRedeem(redeem));
    System.out.println(
        "offline_note_v2_java_bench audit="
            + summary(auditSeconds)
            + " redeem="
            + summary(redeemSeconds));
  }

  private static void qrFixtureUsesSdkTextPrefix() throws Exception {
    final Map<String, Object> fountain = obj(loadFixture(), "fountain_qr_v1");
    assertEquals("iroha:qr1:", string(fountain, "frame_prefix"), "fountain QR prefix");
  }

  private static void paymentTokenCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2PaymentToken token =
        new OfflineNoteV2PaymentToken(
            string(derivation, "chain_id"),
            string(payment, "invoice_id"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(payment, "token_id")),
            audit(fixture),
            longValue(payment, "created_at_ms"));

    final OfflineNoteV2PaymentToken noritoDecoded =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(OfflineNoteV2PaymentTokenCodec.encodeNorito(token));
    assertEquals(token.tokenIdHex(), noritoDecoded.tokenIdHex(), "norito token id");
    assertEquals(token.paymentRequestId(), noritoDecoded.paymentRequestId(), "norito payment request id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(noritoDecoded.audit().noritoEncoded()),
        "norito audit");

    final String text = OfflineNoteV2PaymentTokenCodec.encodeText(token);
    assertTrue(
        text.startsWith(OfflineNoteV2PaymentTokenCodec.TEXT_PREFIX),
        "payment token text prefix");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2PaymentTokenCodec.decodeText(text).tokenIdHex(),
        "text token id");

    final List<byte[]> frames =
        OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(
            token, new OfflineQrStream.Options(180, 2));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    byte[] payload = null;
    for (final byte[] frame : frames) {
      final OfflineQrStream.DecodeResult result = decoder.ingest(frame);
      if (result.payload() != null) {
        payload = result.payload();
      }
    }
    assertTrue(payload != null, "payment token QR payload");
    final OfflineNoteV2PaymentToken qrDecoded =
        OfflineNoteV2PaymentTokenCodec.decodeQrPayload(payload);
    assertEquals(token.tokenIdHex(), qrDecoded.tokenIdHex(), "qr token id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(qrDecoded.audit().noritoEncoded()),
        "qr audit norito");
  }

  private static void walletNoteJsonCodecRoundTripsFixtureNote() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteV2WalletNote note = sourceWalletNote(fixture, senderCertificate);

    final OfflineNoteV2WalletNote decoded =
        OfflineNoteV2WalletNoteJsonCodec.decode(OfflineNoteV2WalletNoteJsonCodec.encode(note));

    assertEquals(note.chainId(), decoded.chainId(), "note chain id");
    assertEquals(note.accountId(), decoded.accountId(), "note account id");
    assertEquals(note.assetId(), decoded.assetId(), "note asset id");
    assertEquals(note.canonicalAmount(), decoded.canonicalAmount(), "note amount");
    assertEquals(note.noteCommitmentHex(), decoded.noteCommitmentHex(), "note commitment");
    assertTrue(Arrays.equals(note.noteSecret(), decoded.noteSecret()), "note secret");
    assertEquals(note.state().name(), decoded.state().name(), "note state");
    assertEquals(note.createdAtMs(), decoded.createdAtMs(), "note created_at_ms");
    assertEquals(note.updatedAtMs(), decoded.updatedAtMs(), "note updated_at_ms");
    assertEquals(
        base64(note.keyCertificate().noritoEncoded()),
        base64(decoded.keyCertificate().noritoEncoded()),
        "note key certificate");
    assertTrue(
        decoded.origin() instanceof OfflineNoteV2.CommitmentOriginV2.IssuerLoad,
        "note origin type");
    final OfflineNoteV2.CommitmentOriginV2.IssuerLoad origin =
        (OfflineNoteV2.CommitmentOriginV2.IssuerLoad) decoded.origin();
    final OfflineNoteV2.CommitmentOriginV2.IssuerLoad expectedOrigin =
        (OfflineNoteV2.CommitmentOriginV2.IssuerLoad) note.origin();
    assertEquals(expectedOrigin.operationId(), origin.operationId(), "origin operation id");
    assertEquals(expectedOrigin.lineageId(), origin.lineageId(), "origin lineage id");
    assertEquals(expectedOrigin.localRevision(), origin.localRevision(), "origin local revision");
  }

  private static void walletLoadDerivesCommitmentBeforeIssuerSubmission() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteV2LoadContext loadContext =
        new OfflineNoteV2LoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final RecordingIssuerClient issuerClient = new RecordingIssuerClient(loadContext);
    final OfflineNoteV2Wallet wallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(issue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            new InMemoryOfflineNoteV2Store(),
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "source_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_000L);

    final OfflineNoteV2WalletNote note =
        wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount")).get();

    assertEquals(
        string(derivation, "source_note_commitment"),
        note.noteCommitmentHex(),
        "wallet load note commitment");
    assertEquals(
        string(derivation, "source_note_commitment"),
        issuerClient.lastIssueRequest.noteCommitmentHex(),
        "issuer request note commitment");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        note.state().name(),
        "loaded note state");
  }

  private static void toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> certificateJson =
        obj(obj(fixture, "payment_token"), "sender_key_certificate");
    final String accountId = string(certificateJson, "account_id");
    final String assetDefinitionId =
        assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"));
    final String offlinePublicKey = "a5".repeat(32);
    final Map<String, Object> bindingJson = new LinkedHashMap<>();
    bindingJson.put("device_id", "device-1");
    bindingJson.put("offline_public_key", offlinePublicKey);
    bindingJson.put("signature_base64", "nested-device-signature-is-not-body-auth");
    final OfflineNoteV2IssuerDeviceBinding binding =
        new OfflineNoteV2IssuerDeviceBinding("device-1", offlinePublicKey, bindingJson);
    final OfflineIssuerExecutor executor = new OfflineIssuerExecutor(certificateJson);
    final java.security.KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiOfflineNoteV2IssuerClient client =
        new ToriiOfflineNoteV2IssuerClient(
            new ToriiCanonicalRequestAuth(accountId, keyPair.getPrivate()),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            executor,
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            Map.of(),
            List.of(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-refill-1", "auth-refill-1", "auth-issue-1"));

    final OfflineNoteV2LoadContext context =
        client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").get();
    assertEquals("operation-refill-1", context.operationId(), "operation id");
    assertEquals("lineage-1", context.lineageId(), "lineage id");
    assertEquals(1L, context.localRevision(), "post-issue commitment revision");

    final byte[] commitment = new byte[32];
    for (int i = 0; i < commitment.length; i++) {
      commitment[i] = (byte) (i + 1);
    }
    final OfflineNoteV2IssueResponse response =
        client.issueNote(
                new OfflineNoteV2IssueRequest(
                    "chain-1",
                    accountId,
                    assetDefinitionId,
                    assetDefinitionId + "#" + accountId,
                    "5",
                    context,
                    commitment))
            .get();

    assertEquals(hex(commitment), hex(response.noteCommitment()), "issued commitment");
    assertEquals("settlement-entry-hash", response.settlementEntryHashHex(), "settlement hash");
    assertEquals(2L, executor.requests.size(), "issuer request count");
    assertEquals(
        "/v1/offline/v2/keys/refill", executor.requests.get(0).uri().getPath(), "refill path");
    assertEquals(
        "/v1/offline/v2/notes/issue", executor.requests.get(1).uri().getPath(), "issue path");
    for (final TransportRequest request : executor.requests) {
      assertTrue(
          request.headers().keySet().stream()
              .noneMatch(name -> name.regionMatches(true, 0, "X-Iroha-", 0, "X-Iroha-".length())),
          "offline issuer body auth must not use X-Iroha headers");
    }

    final Map<String, Object> refillBody = executor.requestBody(0);
    assertEquals(accountId, string(refillBody, "account_id"), "refill account id");
    assertEquals("operation-refill-1", string(refillBody, "operation_id"), "refill operation");
    assertEquals("auth-refill-1", string(refillBody, "nonce"), "refill nonce");
    assertTrue(!string(refillBody, "signature_base64").isBlank(), "refill body signature");
    assertEquals(
        "nested-device-signature-is-not-body-auth",
        string(obj(refillBody, "device_binding"), "signature_base64"),
        "nested device proof is preserved");

    final Map<String, Object> issueBody = executor.requestBody(1);
    assertEquals(hex(commitment), string(issueBody, "note_commitment"), "issue commitment");
    assertEquals(0L, longValue(issueBody, "local_revision"), "pre-issue revision");
    assertEquals("0", string(issueBody, "local_balance"), "pre-issue balance");
    assertEquals("auth-issue-1", string(issueBody, "nonce"), "issue nonce");
    obj(issueBody, "lineage_state");
  }

  private static void walletLifecycleBuildsAuditAcceptAndRedeemTransactions() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainAudit = obj(chain, "audit");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteV2Store senderStore = new InMemoryOfflineNoteV2Store();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet senderWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms"));
    final RecordingTransactionSubmitter recipientSubmitter = new RecordingTransactionSubmitter();
    final OfflineNoteV2Wallet recipientWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteV2Store(),
            null,
            recipientSubmitter,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_200L);

    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        receiveRequest.outputCommitmentHex(),
        "recipient output commitment");

    final OfflineNoteV2PaymentToken token = senderWallet.pay(receiveRequest);

    assertEquals(string(derivation, "payment_token_id"), token.tokenIdHex(), "payment token id");
    assertEquals(
        string(derivation, "payment_request_id"),
        token.paymentRequestId(),
        "payment request id");
    assertEquals(
        string(chainAudit, "public_inputs_hash"),
        hex(token.audit().publicInputsHash()),
        "audit public inputs hash");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "source note state");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "change note state");

    final OfflineNoteV2WalletNote accepted = recipientWallet.accept(token);

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertEquals(0L, recipientSubmitter.audits.size(), "audit submit count before publish");
    recipientWallet.publishAudit(token).get();
    assertEquals(1L, recipientSubmitter.audits.size(), "audit submit count");
    final OfflineNoteV2WalletNote redeeming = recipientWallet.redeem(accepted).get();
    assertEquals(
        OfflineNoteV2WalletNoteState.REDEEM_PENDING.name(),
        redeeming.state().name(),
        "redeem note state");
    assertEquals(1L, recipientSubmitter.redemptions.size(), "redeem submit count");
    assertEquals(
        string(chainRedeem, "public_inputs_hash"),
        hex(recipientSubmitter.redemptions.get(0).publicInputsHash()),
        "redeem public inputs hash");
  }

  private static void walletSyncReconcilesPendingSpendChangeAndRedeemStates() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteV2Store senderStore = new InMemoryOfflineNoteV2Store();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final Map<String, OfflineNoteV2WalletNoteState> resolutions = new LinkedHashMap<>();
    resolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteV2WalletNoteState.SPENT);
    resolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteV2WalletNoteState.SPENDABLE);
    final RecordingSyncResolver syncResolver = new RecordingSyncResolver(resolutions);
    final OfflineNoteV2Wallet senderWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            syncResolver,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_000L);
    final OfflineNoteV2Wallet recipientWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteV2Store(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_100L);

    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    senderWallet.pay(receiveRequest);
    senderWallet.sync().get();

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "synced source note state");
    final OfflineNoteV2WalletNote spendableChange =
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")));
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        spendableChange.state().name(),
        "synced change note state");
    assertEquals(0L, syncResolver.resolvedCommitments.size(), "sync resolver commitment count");

    resolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteV2WalletNoteState.REDEEMED);
    final OfflineNoteV2WalletNote redeeming = senderWallet.redeem(spendableChange).get();
    assertEquals(
        OfflineNoteV2WalletNoteState.REDEEM_PENDING.name(),
        redeeming.state().name(),
        "redeeming note state");

    senderWallet.sync().get();

    assertEquals(
        OfflineNoteV2WalletNoteState.REDEEMED.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "synced redeemed note state");
  }

  private static void walletRejectsDuplicateTokenAndAlreadyPendingInputs() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteV2Store senderStore = new InMemoryOfflineNoteV2Store();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet senderWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_200L);
    final OfflineNoteV2Wallet recipientWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteV2Store(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_300L);

    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    final OfflineNoteV2PaymentToken token = senderWallet.pay(receiveRequest);

    assertThrows(
        () -> senderWallet.pay(receiveRequest), "already pending input payment should throw");

    final OfflineNoteV2WalletNote accepted = recipientWallet.accept(token);
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertThrows(() -> recipientWallet.accept(token), "duplicate token replay should fail");
  }

  private static void walletSyncReconcilesFailedAuditAndRedeemOutcomes() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteV2Store senderStore = new InMemoryOfflineNoteV2Store();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final Map<String, OfflineNoteV2WalletNoteState> senderResolutions = new LinkedHashMap<>();
    senderResolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteV2WalletNoteState.SPENT);
    senderResolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteV2WalletNoteState.SPENDABLE);
    final OfflineNoteV2Wallet senderWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            new RecordingSyncResolver(senderResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_400L);
    final InMemoryOfflineNoteV2Store recipientStore = new InMemoryOfflineNoteV2Store();
    final Map<String, OfflineNoteV2WalletNoteState> recipientResolutions = new LinkedHashMap<>();
    recipientResolutions.put(
        string(derivation, "recipient_output_commitment"), OfflineNoteV2WalletNoteState.CANCELLED);
    final OfflineNoteV2Wallet recipientWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            recipientStore,
            null,
            new RejectingTransactionSubmitter(),
            new RecordingSyncResolver(recipientResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_500L);

    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    final OfflineNoteV2PaymentToken token = senderWallet.pay(receiveRequest);

    final OfflineNoteV2WalletNote accepted = recipientWallet.accept(token);
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertFutureFails(recipientWallet.publishAudit(token), "failed audit submit should fail publish");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment"))).state().name(),
        "failed audit leaves accepted note spendable");

    senderWallet.sync().get();
    recipientWallet.sync().get();

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "failed audit leaves input spent");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "failed audit leaves change spendable");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment"))).state().name(),
        "failed audit leaves recipient spendable");

    final InMemoryOfflineNoteV2Store redeemStore = new InMemoryOfflineNoteV2Store();
    final OfflineNoteV2WalletNote redeemNote = sourceWalletNote(fixture, senderCertificate);
    redeemStore.upsert(redeemNote);
    final Map<String, OfflineNoteV2WalletNoteState> redeemResolutions = new LinkedHashMap<>();
    redeemResolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteV2WalletNoteState.SPENDABLE);
    final OfflineNoteV2Wallet redeemWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            redeemStore,
            null,
            new RejectingTransactionSubmitter(),
            new RecordingSyncResolver(redeemResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_600L);

    assertFutureFails(redeemWallet.redeem(redeemNote), "failed redeem submit should fail redeem");
    assertEquals(
        OfflineNoteV2WalletNoteState.REDEEM_PENDING.name(),
        redeemStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "failed redeem leaves redeem pending before sync");

    redeemWallet.sync().get();

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        redeemStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "failed redeem sync restores spendable note");
  }

  private static void outcomeIndexResolvesCommittedAndRejectedExplorerInstructions() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issueVector = obj(chain, "issue");
    final Map<String, Object> redeemVector = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final OfflineNoteV2WalletNote redeemPending =
        new OfflineNoteV2WalletNote(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            string(issueVector, "asset_id"),
            string(redeemVector, "amount"),
            recipientCertificate,
            redeem.sourceNoteCommitment(),
            hexBytes(string(derivation, "recipient_note_secret_hex")),
            new OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                string(derivation, "payment_request_id"), 0),
            OfflineNoteV2WalletNoteState.REDEEM_PENDING,
            1_700_000_003_100L,
            1_700_000_003_100L);

    final OfflineNoteV2OutcomeIndex committed =
        OfflineNoteV2OutcomeIndex.fromExplorerOutcomes(
            List.of(
                new OfflineNoteV2ExplorerInstructionOutcome(
                    OfflineNoteV2OutcomeIndex.KIND_AUDIT,
                    "Committed",
                    "audit-tx",
                    rawInstructionPair(
                        OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.auditInstruction(audit)))),
                new OfflineNoteV2ExplorerInstructionOutcome(
                    OfflineNoteV2OutcomeIndex.KIND_REDEEM,
                    "Committed",
                    "redeem-tx",
                    rawInstructionPair(
                        OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem))))));
    assertTrue(
        committed.resolve(sourceWalletNote(fixture, certificate(obj(payment, "sender_key_certificate"))))
            == null,
        "audit outcomes do not mutate local-final notes");
    assertEquals(
        OfflineNoteV2WalletNoteState.REDEEMED.name(),
        committed.resolve(redeemPending).state().name(),
        "committed redeem");

    final OfflineNoteV2OutcomeIndex rejected =
        new OfflineNoteV2OutcomeIndex()
            .recordRejectedAudit(audit, "audit-rejected")
            .recordRejectedRedeem(redeem, "redeem-rejected");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        rejected.resolve(redeemPending).state().name(),
        "rejected redeem");
  }

  private static OfflineNoteV2.IssueV2 issue(final Map<String, Object> fixture) {
    final Map<String, Object> chainIssue = obj(obj(fixture, "chain_vectors"), "issue");
    return new OfflineNoteV2.IssueV2(
        hexBytes(string(chainIssue, "note_commitment")),
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
        string(chainIssue, "asset_id"),
        string(chainIssue, "amount"));
  }

  private static OfflineNoteV2.RedeemV2 redeem(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new OfflineNoteV2.RedeemV2(
        hexBytes(string(vector, "source_note_commitment")),
        hexList(vector, "input_nullifiers"),
        certificate(obj(payment, "recipient_key_certificate")),
        string(payment, "recipient_account_id"),
        string(vector, "asset_id"),
        string(vector, "amount"),
        new OfflineNoteV2.RecursiveProofV2(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNoteV2.AuditBundleV2 audit(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "audit");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final List<OfflineNoteV2.IssuedClaimV2> inputClaims = new ArrayList<>();
    for (final Object item : list(payment, "input_claims")) {
      inputClaims.add(issuedClaim(asMap(item, "input claim")));
    }
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims = new ArrayList<>();
    for (final Object item : list(payment, "output_claims")) {
      outputClaims.add(auditOutputClaim(asMap(item, "output claim")));
    }
    return new OfflineNoteV2.AuditBundleV2(
        hexBytes(string(vector, "token_id")),
        certificate(obj(payment, "sender_key_certificate")),
        hexList(vector, "input_nullifiers"),
        inputClaims,
        hexList(vector, "output_commitments"),
        outputClaims,
        new OfflineNoteV2.RecursiveProofV2(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-audit-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNoteV2.KeyCertificateV2 certificate(final Map<String, Object> json) {
    return new OfflineNoteV2.KeyCertificateV2(
        intValue(json, "version"),
        string(json, "platform"),
        string(json, "key_id"),
        string(json, "device_id"),
        string(json, "account_id"),
        base64Bytes(string(json, "public_key")),
        string(json, "assertion_scheme"),
        string(json, "assertion_key_algorithm"),
        base64Bytes(string(json, "assertion_public_key")),
        nullableInt(json, "assertion_usage_count_limit"),
        bool(json, "one_use"),
        base64Bytes(string(json, "issuer_signature_base64")));
  }

  private static OfflineNoteV2.IssuedClaimV2 issuedClaim(final Map<String, Object> json) {
    return new OfflineNoteV2.IssuedClaimV2(
        string(json, "domain"),
        hexBytes(string(json, "note_commitment")),
        hexBytes(string(json, "key_certificate_payload_hash")),
        string(json, "asset_id"),
        string(json, "amount"));
  }

  private static OfflineNoteV2.AuditOutputClaimV2 auditOutputClaim(
      final Map<String, Object> json) {
    return new OfflineNoteV2.AuditOutputClaimV2(
        hexBytes(string(json, "note_commitment")),
        certificate(obj(json, "key_certificate")),
        string(json, "asset_definition_id") + "#" + string(json, "account_id"),
        string(json, "amount"));
  }

  private static OfflineNoteV2WalletNote sourceWalletNote(
      final Map<String, Object> fixture, final OfflineNoteV2.KeyCertificateV2 certificate) {
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    return new OfflineNoteV2WalletNote(
        string(derivation, "chain_id"),
        accountFromAssetId(string(issue, "asset_id")),
        string(issue, "asset_id"),
        string(issue, "amount"),
        certificate,
        hexBytes(string(derivation, "source_note_commitment")),
        hexBytes(string(derivation, "source_note_secret_hex")),
        new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision")),
        OfflineNoteV2WalletNoteState.SPENDABLE,
        1_700_000_000_000L,
        1_700_000_000_000L);
  }

  private static final class StaticAttestationProvider implements OfflineNoteV2AttestationProvider {
    private final OfflineNoteV2.KeyCertificateV2 certificate;

    private StaticAttestationProvider(final OfflineNoteV2.KeyCertificateV2 certificate) {
      this.certificate = certificate;
    }

    @Override
    public OfflineNoteV2.KeyCertificateV2 currentKeyCertificate() {
      return certificate;
    }
  }

  private static final class QueueRandomSource implements OfflineNoteV2RandomSource {
    private final List<byte[]> values;
    private int index;

    private QueueRandomSource(final List<byte[]> values) {
      this.values = values;
    }

    @Override
    public byte[] nextBytes(final int length) {
      if (index >= values.size()) {
        throw new AssertionError("test random source exhausted");
      }
      final byte[] value = values.get(index++);
      if (value.length != length) {
        throw new AssertionError("test random source returned " + value.length + " bytes");
      }
      return Arrays.copyOf(value, value.length);
    }
  }

  private static final class FixedIdGenerator implements OfflineNoteV2IdGenerator {
    private final String id;

    private FixedIdGenerator(final String id) {
      this.id = id;
    }

    @Override
    public String nextId(final String prefix) {
      return id;
    }
  }

  private static final class SequenceIdGenerator implements OfflineNoteV2IdGenerator {
    private final String[] ids;
    private int index;

    private SequenceIdGenerator(final String... ids) {
      this.ids = Arrays.copyOf(ids, ids.length);
    }

    @Override
    public String nextId(final String prefix) {
      if (index >= ids.length) {
        throw new AssertionError("test id generator exhausted");
      }
      return ids[index++];
    }
  }

  private static final class OfflineIssuerExecutor implements HttpTransportExecutor {
    private final Map<String, Object> certificateJson;
    private final List<TransportRequest> requests = new ArrayList<>();

    private OfflineIssuerExecutor(final Map<String, Object> certificateJson) {
      this.certificateJson = certificateJson;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      final Map<String, Object> body = requestBody(request);
      final Map<String, Object> response = new LinkedHashMap<>();
      switch (request.uri().getPath()) {
        case "/v1/offline/v2/keys/refill" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("lineage_state", lineageState(0, "0"));
          response.put("key_certificate", certificateWithExpiry());
          response.put("key_certificates", List.of(certificateWithExpiry()));
        }
        case "/v1/offline/v2/notes/issue" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("settlement", Map.of("entry_hash", "settlement-entry-hash"));
          response.put("lineage_state", lineageState(1, "5"));
          response.put("local_balance", "5");
          response.put("locked_balance", "0");
          response.put("local_revision", 1L);
          response.put("local_state_hash", "lineage-state-hash");
          response.put("issued_note_commitment", string(body, "note_commitment"));
          response.put("key_certificate", certificateWithExpiry());
          response.put("key_certificates", List.of(certificateWithExpiry()));
        }
        default -> throw new IllegalStateException("unexpected path " + request.uri().getPath());
      }
      return CompletableFuture.completedFuture(
          TransportResponse.builder()
              .setStatusCode(200)
              .setBody(JsonEncoder.encode(response).getBytes(StandardCharsets.UTF_8))
              .build());
    }

    private Map<String, Object> requestBody(final int index) {
      return requestBody(requests.get(index));
    }

    @SuppressWarnings("unchecked")
    private Map<String, Object> requestBody(final TransportRequest request) {
      return (Map<String, Object>)
          JsonParser.parse(new String(request.body(), StandardCharsets.UTF_8));
    }

    private Map<String, Object> certificateWithExpiry() {
      final Map<String, Object> copy = new LinkedHashMap<>(certificateJson);
      copy.put("expires_at_ms", 1_700_000_060_000L);
      return copy;
    }

    private Map<String, Object> lineageState(final long revision, final String balance) {
      final Map<String, Object> authorization = new LinkedHashMap<>();
      authorization.put("expires_at_ms", 1_700_000_060_000L);
      final Map<String, Object> state = new LinkedHashMap<>();
      state.put("lineage_id", "lineage-1");
      state.put("server_revision", revision);
      state.put("pending_local_revision", revision);
      state.put("balance", balance);
      state.put("locked_balance", "0");
      state.put("authorization", authorization);
      return state;
    }
  }

  private enum BindingProofProvider implements OfflineNoteV2ProofProvider {
    INSTANCE;

    @Override
    public OfflineNoteV2.RecursiveProofV2 proveAudit(final OfflineNoteV2.AuditBundleV2 audit) {
      return new OfflineNoteV2.RecursiveProofV2(
          audit.publicInputsHash(),
          new OfflineNoteV2.ProofBox(
              OfflineNoteV2.RECURSIVE_BACKEND,
              "wallet-audit-proof".getBytes(StandardCharsets.UTF_8)));
    }

    @Override
    public OfflineNoteV2.RecursiveProofV2 proveRedeem(final OfflineNoteV2.RedeemV2 redemption) {
      return new OfflineNoteV2.RecursiveProofV2(
          redemption.publicInputsHash(),
          new OfflineNoteV2.ProofBox(
              OfflineNoteV2.RECURSIVE_BACKEND,
              "wallet-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    }
  }

  private enum BindingProofVerifier implements OfflineNoteV2ProofVerifier {
    INSTANCE;

    @Override
    public boolean verifyAudit(final OfflineNoteV2.AuditBundleV2 audit) {
      return Arrays.equals(audit.recursiveProof().publicInputsHash(), audit.publicInputsHash());
    }

    @Override
    public boolean verifyRedeem(final OfflineNoteV2.RedeemV2 redemption) {
      return Arrays.equals(
          redemption.recursiveProof().publicInputsHash(), redemption.publicInputsHash());
    }
  }

  private static final class RecordingIssuerClient implements OfflineNoteV2IssuerClient {
    private final OfflineNoteV2LoadContext loadContext;
    private OfflineNoteV2IssueRequest lastIssueRequest;

    private RecordingIssuerClient(final OfflineNoteV2LoadContext loadContext) {
      this.loadContext = loadContext;
    }

    @Override
    public CompletableFuture<OfflineNoteV2LoadContext> prepareLoad(
        final String chainId,
        final String accountId,
        final String assetDefinitionId,
        final String amount) {
      return CompletableFuture.completedFuture(loadContext);
    }

    @Override
    public CompletableFuture<OfflineNoteV2IssueResponse> issueNote(
        final OfflineNoteV2IssueRequest request) {
      lastIssueRequest = request;
      return CompletableFuture.completedFuture(
          new OfflineNoteV2IssueResponse(
              request.noteCommitment(),
              request.loadContext().operationId(),
              request.loadContext().lineageId(),
              request.loadContext().localRevision(),
              request.loadContext().keyCertificate(),
              "settlement-entry-hash"));
    }
  }

  private static final class RecordingTransactionSubmitter implements OfflineNoteV2TransactionSubmitter {
    private final List<OfflineNoteV2.AuditBundleV2> audits = new ArrayList<>();
    private final List<OfflineNoteV2.RedeemV2> redemptions = new ArrayList<>();

    @Override
    public CompletableFuture<ClientResponse> submitAudit(final OfflineNoteV2.AuditBundleV2 audit) {
      audits.add(audit);
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitRedeem(final OfflineNoteV2.RedeemV2 redemption) {
      redemptions.add(redemption);
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }
  }

  private static final class RejectingTransactionSubmitter implements OfflineNoteV2TransactionSubmitter {
    @Override
    public CompletableFuture<ClientResponse> submitAudit(final OfflineNoteV2.AuditBundleV2 audit) {
      return CompletableFuture.completedFuture(new ClientResponse(409, new byte[0], "rejected"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitRedeem(final OfflineNoteV2.RedeemV2 redemption) {
      return CompletableFuture.completedFuture(new ClientResponse(409, new byte[0], "rejected"));
    }
  }

  private static final class RecordingSyncResolver implements OfflineNoteV2SyncResolver {
    private final Map<String, OfflineNoteV2WalletNoteState> resolutions;
    private final List<String> resolvedCommitments = new ArrayList<>();

    private RecordingSyncResolver(final Map<String, OfflineNoteV2WalletNoteState> resolutions) {
      this.resolutions = resolutions;
    }

    @Override
    public CompletableFuture<OfflineNoteV2SyncResolution> resolvePendingNote(
        final OfflineNoteV2WalletNote note) {
      final String commitment = note.noteCommitmentHex();
      resolvedCommitments.add(commitment);
      final OfflineNoteV2WalletNoteState state = resolutions.get(commitment);
      return CompletableFuture.completedFuture(
          state == null ? null : new OfflineNoteV2SyncResolution(state, "tx-" + commitment));
    }
  }

  private static byte[] wirePayloadBytes(final InstructionBox instruction) {
    return ((InstructionBox.WirePayload) instruction.payload()).payloadBytes();
  }

  private static byte[] rawInstructionPair(final String wireName, final byte[] wirePayload) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeField(out, encodeString(wireName, true), true);
    writeField(out, encodeBytesVec(wirePayload), true);
    return out.toByteArray();
  }

  private static byte[] encodeString(final String value, final boolean compact) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeLength(out, bytes.length, compact);
    out.writeBytes(bytes);
    return out.toByteArray();
  }

  private static byte[] encodeBytesVec(final byte[] value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeUInt64(out, value.length);
    out.writeBytes(value);
    return out.toByteArray();
  }

  private static void writeField(
      final ByteArrayOutputStream out, final byte[] payload, final boolean compact) {
    writeLength(out, payload.length, compact);
    out.writeBytes(payload);
  }

  private static void writeLength(
      final ByteArrayOutputStream out, final long value, final boolean compact) {
    if (!compact) {
      writeUInt64(out, value);
      return;
    }
    long remaining = value;
    while (remaining >= 0x80) {
      out.write((int) ((remaining & 0x7F) | 0x80));
      remaining >>>= 7;
    }
    out.write((int) remaining);
  }

  private static void writeUInt64(final ByteArrayOutputStream out, final long value) {
    long remaining = value;
    for (int index = 0; index < 8; index++) {
      out.write((int) (remaining & 0xFF));
      remaining >>>= 8;
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadFixture() throws Exception {
    Path cursor = Paths.get("").toAbsolutePath();
    while (cursor != null) {
      final Path candidate = cursor.resolve("fixtures/offline/interop_contract_v2.json");
      if (Files.exists(candidate)) {
        final String json = Files.readString(candidate);
        return (Map<String, Object>) JsonParser.parse(json);
      }
      cursor = cursor.getParent();
    }
    throw new AssertionError("fixtures/offline/interop_contract_v2.json was not found");
  }

  private static Map<String, Object> obj(final Map<String, Object> map, final String key) {
    return asMap(map.get(key), key);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asMap(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new AssertionError(label + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof List)) {
      throw new AssertionError(key + " must be an array");
    }
    return (List<Object>) value;
  }

  private static List<byte[]> hexList(final Map<String, Object> map, final String key) {
    final List<byte[]> values = new ArrayList<>();
    for (final Object item : list(map, key)) {
      values.add(hexBytes((String) item));
    }
    return values;
  }

  private static String string(final Map<String, Object> map, final String key) {
    return (String) map.get(key);
  }

  private static boolean bool(final Map<String, Object> map, final String key) {
    return (Boolean) map.get(key);
  }

  private static int intValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).intValue();
  }

  private static long longValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).longValue();
  }

  private static Integer nullableInt(final Map<String, Object> map, final String key) {
    final Number value = (Number) map.get(key);
    return value == null ? null : value.intValue();
  }

  private static String base64(final byte[] bytes) {
    return Base64.getEncoder().encodeToString(bytes);
  }

  private static byte[] base64Bytes(final String value) {
    return Base64.getDecoder().decode(value);
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xFF));
    }
    return builder.toString();
  }

  private static String assetDefinitionFromAssetId(final String assetId) {
    return assetId.split("#", 2)[0];
  }

  private static String accountFromAssetId(final String assetId) {
    return assetId.split("#", 2)[1].split("#dataspace:", 2)[0];
  }

  private static byte[] hashFromPublicValues(final long[] values) {
    final byte[] out = new byte[32];
    for (int idx = 0; idx < 4; idx++) {
      long word = values[idx];
      for (int offset = 0; offset < 8; offset++) {
        out[idx * 8 + offset] = (byte) (word & 0xFFL);
        word >>>= 8;
      }
    }
    return out;
  }

  private static double[] benchmarkSeconds(
      final int iterations, final ThrowingRunnable action) throws Exception {
    final double[] durations = new double[iterations];
    for (int idx = 0; idx < iterations; idx++) {
      final long start = System.nanoTime();
      action.run();
      durations[idx] = (System.nanoTime() - start) / 1_000_000_000.0;
    }
    return durations;
  }

  private static String summary(final double[] values) {
    final double[] sorted = Arrays.copyOf(values, values.length);
    Arrays.sort(sorted);
    if (sorted.length == 0) {
      return "empty";
    }
    final double median =
        (sorted.length & 1) == 0
            ? (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2.0
            : sorted[sorted.length / 2];
    final int p95Index =
        Math.min(sorted.length - 1, Math.max(0, (int) Math.ceil(sorted.length * 0.95) - 1));
    return String.format(
        Locale.ROOT,
        "median=%.3fs p95=%.3fs max=%.3fs n=%d",
        median,
        sorted[p95Index],
        sorted[sorted.length - 1],
        sorted.length);
  }

  private static byte[] hexBytes(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int offset = 0; offset < value.length(); offset += 2) {
      out[offset / 2] = (byte) Integer.parseInt(value.substring(offset, offset + 2), 16);
    }
    return out;
  }

  private static void assertEquals(
      final String expected, final String actual, final String message) {
    if (!expected.equals(actual)) {
      throw new AssertionError(message + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertEquals(final long expected, final long actual, final String message) {
    if (expected != actual) {
      throw new AssertionError(message + ": expected " + expected + " but got " + actual);
    }
  }

  private static void assertTrue(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private static void assertThrows(final Runnable action, final String message) {
    try {
      action.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertFutureFails(final CompletableFuture<?> future, final String message) {
    try {
      future.get();
    } catch (final InterruptedException ex) {
      Thread.currentThread().interrupt();
      throw new AssertionError(message, ex);
    } catch (final Exception ex) {
      return;
    }
    throw new AssertionError(message);
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
