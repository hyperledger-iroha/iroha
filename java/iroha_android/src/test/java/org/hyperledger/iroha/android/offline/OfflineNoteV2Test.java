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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Function;
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
    receiveRequestCodecRoundTripsNoritoTextAndQrFrames();
    receiptAckCodecRoundTripsNoritoTextAndQrFrames();
    qrStreamRejectsAdversarialEnvelopesAndChunkShapes();
    transferHandoffSupportsQrNfcAndNearbyPayloads();
    transferHandoffRejectsAdversarialStreamsAndMetadata();
    nfcApduProtocolSupportsAndroidSafeAndIosFastChunks();
    transportWireFormatMatchesSharedFixture();
    nfcApduProtocolRejectsAdversarialPayloadsBeforeCommit();
    nfcApduProtocolRejectsMalformedCommandsAndBounds();
    nearbyEnvelopeRoundTripsPairingPaymentAndAck();
    nearbyEnvelopeRejectsAdversarialMessages();
    walletAcceptsCanonicalSdkInteropPaymentToken();
    walletNoteJsonCodecRoundTripsFixtureNote();
    walletLoadDerivesCommitmentBeforeIssuerSubmission();
    walletLoadDoesNotBlockIssuerCompletionThread();
    toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment();
    walletLifecycleBuildsAuditAcceptAndRedeemTransactions();
    walletSyncReconcilesPendingSpendChangeAndRedeemStates();
    walletRejectsDuplicateTokenAndAlreadyPendingInputs();
    walletRejectsAdversarialCertificateBindings();
    walletSyncReconcilesFailedAuditAndRedeemOutcomes();
    outcomeIndexResolvesCommittedAndRejectedExplorerInstructions();
    System.out.println("[IrohaAndroid] OfflineNoteV2Test passed.");
  }

  private static void certificateSigningBytesMatchRustVector() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final Map<String, Object> certificates = obj(obj(fixture, "chain_vectors"), "certificates");
    final OfflineNoteV2CertificateVerifier verifier = certificateVerifier(fixture);

    assertEquals(
        string(certificates, "sender_payload_base64"),
        base64(sender.signingBytes()),
        "sender certificate payload");
    assertEquals(
        string(certificates, "sender_payload_hash"),
        hex(sender.payloadHash()),
        "sender certificate payload hash");
    assertTrue(verifier.verifyCertificate(sender), "fixture sender certificate is trusted");

    final byte[] tamperedSignature = sender.issuerSignature();
    tamperedSignature[0] = (byte) (tamperedSignature[0] ^ 0x01);
    final OfflineNoteV2.KeyCertificateV2 tampered =
        new OfflineNoteV2.KeyCertificateV2(
            sender.version(),
            sender.platform(),
            sender.keyId(),
            sender.deviceId(),
            sender.accountId(),
            sender.publicKey(),
            sender.assertionScheme(),
            sender.assertionKeyAlgorithm(),
            sender.assertionPublicKey(),
            sender.assertionUsageCountLimit(),
            sender.oneUse(),
            tamperedSignature);
    assertTrue(!verifier.verifyCertificate(tampered), "tampered certificate signature is rejected");
    assertTrue(
        !new RejectingOfflineNoteV2CertificateVerifier().verifyCertificate(sender),
        "default certificate verifier rejects");
    assertTrue(
        !new Ed25519OfflineNoteV2CertificateVerifier(
                Collections.singletonList(filledBytes(32, 0x42)))
            .verifyCertificate(sender),
        "wrong issuer root rejects");
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
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final OfflineNoteV2PaymentToken token =
        new OfflineNoteV2PaymentToken(
            string(derivation, "chain_id"),
            string(payment, "invoice_id"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(payment, "token_id")),
            audit(fixture),
            longValue(payment, "created_at_ms"));
    final byte[] canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"));
    assertTrue(
        Arrays.equals(canonicalPayload, OfflineNoteV2PaymentTokenCodec.encodeNorito(token)),
        "canonical payment token Norito");

    final OfflineNoteV2PaymentToken noritoDecoded =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(OfflineNoteV2PaymentTokenCodec.encodeNorito(token));
    assertEquals(token.tokenIdHex(), noritoDecoded.tokenIdHex(), "norito token id");
    assertEquals(token.paymentRequestId(), noritoDecoded.paymentRequestId(), "norito payment request id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(noritoDecoded.audit().noritoEncoded()),
        "norito audit");
    final OfflineNoteV2PaymentToken canonicalDecoded =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(canonicalPayload);
    assertEquals(token.tokenIdHex(), canonicalDecoded.tokenIdHex(), "canonical token id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(canonicalDecoded.audit().noritoEncoded()),
        "canonical audit");

    final String text = OfflineNoteV2PaymentTokenCodec.encodeText(token);
    assertEquals(string(sdkInterop, "payment_token_text"), text, "canonical payment token text");
    assertTrue(
        text.startsWith(OfflineNoteV2PaymentTokenCodec.TEXT_PREFIX),
        "payment token text prefix");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2PaymentTokenCodec.decodeText(text).tokenIdHex(),
        "text token id");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2PaymentTokenCodec.decodeText(string(sdkInterop, "payment_token_text"))
            .tokenIdHex(),
        "canonical text token id");

    final List<byte[]> frames =
        OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(
            token, new OfflineQrStream.Options(180, 2));
    final List<Object> expectedFrameObjects = list(obj(sdkInterop, "payment_token_qr_v1"), "frames");
    final List<String> expectedFrames = new ArrayList<>();
    for (final Object frameObject : expectedFrameObjects) {
      expectedFrames.add(string(asMap(frameObject, "payment_token_qr_v1.frames[]"), "bytes_hex"));
    }
    final List<String> actualFrames = new ArrayList<>();
    for (final byte[] frame : frames) {
      actualFrames.add(hex(frame));
    }
    assertTrue(expectedFrames.equals(actualFrames), "canonical QR frames");
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

    final OfflineQrStream.Decoder canonicalDecoder = new OfflineQrStream.Decoder();
    byte[] canonicalQrPayload = null;
    for (final String frame : expectedFrames) {
      final OfflineQrStream.DecodeResult result = canonicalDecoder.ingest(hexBytes(frame));
      if (result.payload() != null) {
        canonicalQrPayload = result.payload();
      }
    }
    assertTrue(canonicalQrPayload != null, "canonical QR payload");
    assertTrue(Arrays.equals(canonicalPayload, canonicalQrPayload), "canonical QR payload bytes");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2PaymentTokenCodec.decodeQrPayload(canonicalQrPayload).tokenIdHex(),
        "canonical QR token id");
  }

  private static void receiveRequestCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2ReceiveRequest request = receiveRequestFixture(fixture);

    final byte[] norito = OfflineNoteV2ReceiveRequestCodec.encodeNorito(request);
    final OfflineNoteV2ReceiveRequest noritoDecoded =
        OfflineNoteV2ReceiveRequestCodec.decodeNorito(norito);
    assertEquals(request.paymentRequestId(), noritoDecoded.paymentRequestId(), "receive request id");
    assertEquals(request.accountId(), noritoDecoded.accountId(), "receive request account");
    assertEquals(request.assetId(), noritoDecoded.assetId(), "receive request asset");
    assertEquals(request.canonicalAmount(), noritoDecoded.canonicalAmount(), "receive request amount");
    assertEquals(
        request.outputCommitmentHex(),
        noritoDecoded.outputCommitmentHex(),
        "receive request output commitment");
    assertEquals(
        hex(request.keyCertificate().payloadHash()),
        hex(noritoDecoded.keyCertificate().payloadHash()),
        "receive request certificate");

    final String text = OfflineNoteV2ReceiveRequestCodec.encodeText(request);
    assertTrue(
        text.startsWith(OfflineNoteV2ReceiveRequestCodec.TEXT_PREFIX),
        "receive request text prefix");
    assertEquals(
        request.outputCommitmentHex(),
        OfflineNoteV2ReceiveRequestCodec.decodeText(text).outputCommitmentHex(),
        "receive request text output commitment");

    final List<byte[]> frames =
        OfflineNoteV2ReceiveRequestCodec.encodeQrFrameBytes(
            request, new OfflineQrStream.Options(180, 2));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    byte[] payload = null;
    for (final byte[] frame : frames) {
      final OfflineQrStream.DecodeResult result = decoder.ingest(frame);
      assertTrue(
          OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST_V2 == result.payloadKind(),
          "receive request QR kind");
      if (result.payload() != null) {
        payload = result.payload();
      }
    }
    assertTrue(payload != null, "receive request QR payload");
    assertEquals(
        request.outputCommitmentHex(),
        OfflineNoteV2ReceiveRequestCodec.decodeQrPayload(payload).outputCommitmentHex(),
        "receive request QR output commitment");
  }

  private static void receiptAckCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteV2ReceiptAck ack =
        OfflineNoteV2ReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));

    final byte[] norito = OfflineNoteV2ReceiptAckCodec.encodeNorito(ack);
    final OfflineNoteV2ReceiptAck noritoDecoded =
        OfflineNoteV2ReceiptAckCodec.decodeNorito(norito);
    assertEquals(ack.chainId(), noritoDecoded.chainId(), "receipt ACK chain id");
    assertEquals(
        ack.paymentRequestId(),
        noritoDecoded.paymentRequestId(),
        "receipt ACK payment request id");
    assertEquals(ack.tokenIdHex(), noritoDecoded.tokenIdHex(), "receipt ACK token id");
    assertEquals(
        ack.recipientAccountId(),
        noritoDecoded.recipientAccountId(),
        "receipt ACK recipient account");
    assertTrue(noritoDecoded.matchesPaymentToken(token), "receipt ACK matches token");

    final String text = OfflineNoteV2ReceiptAckCodec.encodeText(ack);
    assertTrue(
        text.startsWith(OfflineNoteV2ReceiptAckCodec.TEXT_PREFIX),
        "receipt ACK text prefix");
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteV2ReceiptAckCodec.decodeText(text).tokenIdHex(),
        "receipt ACK text token id");

    final List<byte[]> frames =
        OfflineNoteV2ReceiptAckCodec.encodeQrFrameBytes(ack, new OfflineQrStream.Options(180, 2));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    byte[] payload = null;
    for (final byte[] frame : frames) {
      final OfflineQrStream.DecodeResult result = decoder.ingest(frame);
      assertTrue(
          OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK_V2 == result.payloadKind(),
          "receipt ACK QR kind");
      if (result.payload() != null) {
        payload = result.payload();
      }
    }
    assertTrue(payload != null, "receipt ACK QR payload");
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteV2ReceiptAckCodec.decodeQrPayload(payload).tokenIdHex(),
        "receipt ACK QR token id");
  }

  private static OfflineNoteV2ReceiveRequest receiveRequestFixture(
      final Map<String, Object> fixture) {
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new OfflineNoteV2ReceiveRequest(
        string(derivation, "chain_id"),
        string(derivation, "payment_request_id"),
        string(payment, "recipient_account_id"),
        string(payment, "asset_definition_id"),
        string(payment, "asset_definition_id") + "#" + string(payment, "recipient_account_id"),
        string(payment, "amount"),
        certificate(obj(payment, "recipient_key_certificate")),
        hexBytes(string(derivation, "recipient_output_commitment")));
  }

  private static void qrStreamRejectsAdversarialEnvelopesAndChunkShapes() {
    final byte[] payload = new byte[300];
    for (int index = 0; index < payload.length; index++) {
      payload[index] = (byte) ((index * 31 + 7) & 0xFF);
    }
    final List<OfflineQrStream.Frame> frames =
        OfflineQrStream.Encoder.encodeFrames(
            payload,
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2,
            new OfflineQrStream.Options(100, 2));
    OfflineQrStream.Frame header = null;
    final List<OfflineQrStream.Frame> dataFrames = new ArrayList<>();
    final List<OfflineQrStream.Frame> parityFrames = new ArrayList<>();
    for (final OfflineQrStream.Frame frame : frames) {
      if (frame.kind() == OfflineQrStream.FrameKind.HEADER) {
        header = frame;
      } else if (frame.kind() == OfflineQrStream.FrameKind.DATA) {
        dataFrames.add(frame);
      } else if (frame.kind() == OfflineQrStream.FrameKind.PARITY) {
        parityFrames.add(frame);
      }
    }
    assertTrue(header != null, "QR stream header exists");
    final OfflineQrStream.Frame finalHeader = header;
    final OfflineQrStream.Frame firstData = dataFrames.get(0);
    final OfflineQrStream.Frame firstParity = parityFrames.get(0);

    assertThrows(
        () ->
            new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA, finalHeader.streamId(), 0, 1, new byte[0x1_0000]),
        "oversized frame payload should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA, finalHeader.streamId(), -1, 1, new byte[0]),
        "negative frame index should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA, finalHeader.streamId(), 0x1_0000, 1, new byte[0]),
        "oversized frame index should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA, finalHeader.streamId(), 0, -1, new byte[0]),
        "negative frame total should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA, finalHeader.streamId(), 0, 0x1_0000, new byte[0]),
        "oversized frame total should fail");
    assertThrows(
        () -> new OfflineQrStream.Envelope(0, 0, 0, 1, 1, 0, -1, 1, new byte[32]),
        "negative payload kind should fail");
    assertThrows(
        () -> new OfflineQrStream.Envelope(0, 0, 0, 1, 1, 0, 0x1_0000, 1, new byte[32]),
        "oversized payload kind should fail");

    final byte[] encodedHeader = finalHeader.encode();
    final byte[] trailingFrame = Arrays.copyOf(encodedHeader, encodedHeader.length + 1);
    assertThrows(
        () -> OfflineQrStream.Frame.decode(trailingFrame),
        "trailing frame bytes after CRC should fail");

    final byte[] unsupportedFrameVersion = finalHeader.encode();
    unsupportedFrameVersion[2] = 0x7F;
    assertThrows(
        () -> OfflineQrStream.Frame.decode(unsupportedFrameVersion),
        "unsupported frame version should fail");

    final byte[] unknownFrameKind = finalHeader.encode();
    unknownFrameKind[3] = 0x7F;
    assertThrows(
        () -> OfflineQrStream.Frame.decode(unknownFrameKind),
        "unknown frame kind should fail");

    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    new OfflineQrStream.Frame(
                            OfflineQrStream.FrameKind.HEADER,
                            filledBytes(16, (byte) 0xA5),
                            0,
                            1,
                            finalHeader.payload())
                        .encode()),
        "header stream id mismatch should fail");

    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    new OfflineQrStream.Frame(
                            OfflineQrStream.FrameKind.HEADER,
                            finalHeader.streamId(),
                            1,
                            1,
                            finalHeader.payload())
                        .encode()),
        "noncanonical header counters should fail");

    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(mutatedHeaderFrame(finalHeader, envelope -> Arrays.copyOf(envelope, envelope.length + 1))),
        "extra envelope bytes should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          envelope[2] = 0x7F;
                          return envelope;
                        })),
        "unsupported envelope encoding should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          writeUInt16LE(envelope, 4, 0);
                          return envelope;
                        })),
        "zero envelope chunk size should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          writeUInt16LE(envelope, 6, 1);
                          return envelope;
                        })),
        "envelope data chunk count mismatch should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          writeUInt16LE(envelope, 8, 0);
                          return envelope;
                        })),
        "envelope parity chunk count mismatch should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          envelope[1] = 0x01;
                          return envelope;
                        })),
        "unsupported envelope flags should fail");
    assertThrows(
        () ->
            new OfflineQrStream.Decoder()
                .ingest(
                    mutatedHeaderFrame(
                        finalHeader,
                        envelope -> {
                          envelope[0] = 0x7F;
                          return envelope;
                        })),
        "unsupported envelope version should fail");

    final OfflineQrStream.Decoder repeatedHeaderDecoder = new OfflineQrStream.Decoder();
    repeatedHeaderDecoder.ingest(finalHeader.encode());
    repeatedHeaderDecoder.ingest(finalHeader.encode());
    assertThrows(
        () ->
            repeatedHeaderDecoder.ingest(
                mutatedHeaderFrame(
                    finalHeader,
                    envelope -> {
                      writeUInt16LE(
                          envelope,
                          10,
                          OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST_V2.value());
                      return envelope;
                    })),
        "conflicting repeated header should fail");

    final OfflineQrStream.Decoder shortDataDecoder = new OfflineQrStream.Decoder();
    shortDataDecoder.ingest(finalHeader.encode());
    assertThrows(
        () ->
            shortDataDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.DATA,
                        firstData.streamId(),
                        firstData.index(),
                        firstData.total(),
                        Arrays.copyOf(firstData.payload(), firstData.payload().length - 1))
                    .encode()),
        "short data chunk should fail");

    final OfflineQrStream.Decoder longDataDecoder = new OfflineQrStream.Decoder();
    longDataDecoder.ingest(finalHeader.encode());
    final byte[] longDataPayload = Arrays.copyOf(firstData.payload(), firstData.payload().length + 1);
    assertThrows(
        () ->
            longDataDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.DATA,
                        firstData.streamId(),
                        firstData.index(),
                        firstData.total(),
                        longDataPayload)
                    .encode()),
        "long data chunk should fail");

    final OfflineQrStream.Decoder wrongTotalDecoder = new OfflineQrStream.Decoder();
    wrongTotalDecoder.ingest(finalHeader.encode());
    assertThrows(
        () ->
            wrongTotalDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.DATA,
                        firstData.streamId(),
                        firstData.index(),
                        firstData.total() + 1,
                        firstData.payload())
                    .encode()),
        "wrong data frame total should fail");

    final OfflineQrStream.Decoder pendingBadDataDecoder = new OfflineQrStream.Decoder();
    pendingBadDataDecoder.ingest(
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                firstData.streamId(),
                firstData.index(),
                firstData.total() + 1,
                firstData.payload())
            .encode());
    assertThrows(
        () -> pendingBadDataDecoder.ingest(finalHeader.encode()),
        "pending bad data frame should fail after header");

    final OfflineQrStream.Decoder conflictingDataDecoder = new OfflineQrStream.Decoder();
    conflictingDataDecoder.ingest(finalHeader.encode());
    conflictingDataDecoder.ingest(firstData.encode());
    final byte[] conflictingDataPayload = firstData.payload();
    conflictingDataPayload[0] ^= (byte) 0xFF;
    assertThrows(
        () ->
            conflictingDataDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.DATA,
                        firstData.streamId(),
                        firstData.index(),
                        firstData.total(),
                        conflictingDataPayload)
                    .encode()),
        "conflicting data chunk should fail");

    final OfflineQrStream.Decoder poisonedParityDecoder = new OfflineQrStream.Decoder();
    poisonedParityDecoder.ingest(finalHeader.encode());
    poisonedParityDecoder.ingest(firstData.encode());
    final byte[] poisonedParityPayload = firstParity.payload();
    poisonedParityPayload[0] ^= (byte) 0xFF;
    poisonedParityDecoder.ingest(
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.PARITY,
                firstParity.streamId(),
                firstParity.index(),
                firstParity.total(),
                poisonedParityPayload)
            .encode());
    assertThrows(
        () -> poisonedParityDecoder.ingest(dataFrames.get(1).encode()),
        "poisoned parity recovery should conflict with later real data");

    final OfflineQrStream.Decoder hashMismatchDecoder = new OfflineQrStream.Decoder();
    hashMismatchDecoder.ingest(finalHeader.encode());
    final byte[] mutatedFirstDataPayload = firstData.payload();
    mutatedFirstDataPayload[0] ^= (byte) 0xFF;
    hashMismatchDecoder.ingest(
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                firstData.streamId(),
                firstData.index(),
                firstData.total(),
                mutatedFirstDataPayload)
            .encode());
    hashMismatchDecoder.ingest(dataFrames.get(1).encode());
    assertThrows(
        () -> hashMismatchDecoder.ingest(dataFrames.get(2).encode()),
        "coherent mutated payload should fail hash validation");

    final OfflineQrStream.Decoder shortParityDecoder = new OfflineQrStream.Decoder();
    shortParityDecoder.ingest(finalHeader.encode());
    assertThrows(
        () ->
            shortParityDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.PARITY,
                        firstParity.streamId(),
                        firstParity.index(),
                        firstParity.total(),
                        Arrays.copyOf(firstParity.payload(), firstParity.payload().length - 1))
                    .encode()),
        "short parity chunk should fail");

    final OfflineQrStream.Decoder conflictingParityDecoder = new OfflineQrStream.Decoder();
    conflictingParityDecoder.ingest(finalHeader.encode());
    conflictingParityDecoder.ingest(firstParity.encode());
    final byte[] conflictingParityPayload = firstParity.payload();
    conflictingParityPayload[0] ^= (byte) 0xFF;
    assertThrows(
        () ->
            conflictingParityDecoder.ingest(
                new OfflineQrStream.Frame(
                        OfflineQrStream.FrameKind.PARITY,
                        firstParity.streamId(),
                        firstParity.index(),
                        firstParity.total(),
                        conflictingParityPayload)
                    .encode()),
        "conflicting parity chunk should fail");
  }

  private static void transferHandoffSupportsQrNfcAndNearbyPayloads() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final OfflineNoteV2PaymentToken token =
        new OfflineNoteV2PaymentToken(
            string(derivation, "chain_id"),
            string(payment, "invoice_id"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(payment, "token_id")),
            audit(fixture),
            longValue(payment, "created_at_ms"));
    final OfflineNoteV2ReceiveRequest receiveRequest = receiveRequestFixture(fixture);
    final OfflineNoteV2ReceiptAck receiptAck =
        OfflineNoteV2ReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));
    final byte[] canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"));

    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferCapabilities capabilities =
        OfflineNoteV2TransferHandoff.OfflineNoteV2TransferCapabilities.current(false, true);
    assertTrue(
        capabilities
            .supportedModalities()
            .contains(OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.QR_STREAMING),
        "QR transfer capability");
    assertTrue(
        capabilities
            .supportedModalities()
            .contains(OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.NEARBY),
        "nearby transfer capability");
    assertTrue(
        !capabilities
            .supportedModalities()
            .contains(OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.NFC),
        "NFC should require explicit HCE capability");

    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferPayload nearby =
        OfflineNoteV2TransferHandoff.nearbyPayload(token);
    assertEquals(
        OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.NEARBY.name(),
        nearby.modality().name(),
        "nearby modality");
    assertEquals(
        OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
        nearby.contentType(),
        "nearby content type");
    assertTrue(Arrays.equals(canonicalPayload, nearby.payload()), "nearby payload");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2TransferHandoff.decodePaymentToken(nearby).tokenIdHex(),
        "nearby token id");
    final OfflineNoteV2NearbyEnvelope textChallenge =
        new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.CHALLENGE,
            OfflineNoteV2ReceiveRequestCodec.encodeText(receiveRequest).getBytes(StandardCharsets.UTF_8),
            OfflineNoteV2TransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE,
            OfflineNoteV2NearbyEnvelope.PairingChallenge.random());
    final OfflineNoteV2NearbyEnvelope decodedTextChallenge =
        OfflineNoteV2NearbyEnvelope.decode(textChallenge.encoded());
    assertEquals(
        receiveRequest.outputCommitmentHex(),
        decodedTextChallenge.receiveRequest().outputCommitmentHex(),
        "nearby text challenge payload");
    final OfflineNoteV2NearbyEnvelope textPayment =
        new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.PAYMENT,
            OfflineNoteV2PaymentTokenCodec.encodeText(token).getBytes(StandardCharsets.UTF_8),
            OfflineNoteV2TransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE);
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2NearbyEnvelope.decode(textPayment.encoded()).paymentToken().tokenIdHex(),
        "nearby text payment payload");
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferPayload ackPayload =
        OfflineNoteV2TransferHandoff.receiptAckPayload(
            receiptAck, OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.NEARBY);
    assertEquals(
        receiptAck.tokenIdHex(),
        OfflineNoteV2TransferHandoff.decodeReceiptAck(ackPayload).tokenIdHex(),
        "nearby receipt ACK payload");
    final byte[] nearbyAckBytes = OfflineNoteV2TransferHandoff.nearbyReceiptAckEnvelopeBytes(receiptAck);
    assertEquals(
        receiptAck.tokenIdHex(),
        OfflineNoteV2TransferHandoff.decodeNearbyReceiptAck(nearbyAckBytes).tokenIdHex(),
        "nearby receipt ACK envelope");

    final List<Object> expectedFrameObjects = list(obj(sdkInterop, "payment_token_qr_v1"), "frames");
    final List<String> expectedFrames = new ArrayList<>();
    for (final Object frameObject : expectedFrameObjects) {
      expectedFrames.add(string(asMap(frameObject, "payment_token_qr_v1.frames[]"), "bytes_hex"));
    }
    final List<byte[]> qrFrames = OfflineNoteV2TransferHandoff.qrStreamingFrameBytes(token);
    final List<String> actualFrames = new ArrayList<>();
    for (final byte[] frame : qrFrames) {
      actualFrames.add(hex(frame));
    }
    assertTrue(expectedFrames.equals(actualFrames), "handoff QR frames");
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver qrReceiver =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver();
    OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamResult qrResult = null;
    for (final byte[] frame : qrFrames) {
      qrResult = qrReceiver.ingestFrame(frame);
    }
    assertTrue(qrResult != null && qrResult.token() != null, "handoff QR token");
    assertEquals(token.tokenIdHex(), qrResult.token().tokenIdHex(), "handoff QR token id");

    final List<byte[]> nfcFrames = OfflineNoteV2TransferHandoff.nfcFrameBytes(token);
    for (final byte[] frame : nfcFrames) {
      assertTrue(frame.length <= 250, "NFC frame fits short APDU payload budget");
    }
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver nfcReceiver =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver();
    OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamResult nfcResult = null;
    for (final byte[] frame : nfcFrames) {
      nfcResult = nfcReceiver.ingestFrame(frame);
    }
    assertTrue(nfcResult != null && nfcResult.token() != null, "handoff NFC token");
    assertEquals(token.tokenIdHex(), nfcResult.token().tokenIdHex(), "handoff NFC token id");
  }

  private static void transferHandoffRejectsAdversarialStreamsAndMetadata() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] rawPayload = OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(token);
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferPayload payload =
        OfflineNoteV2TransferHandoff.paymentTokenPayload(
            token, OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.QR_STREAMING);
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferPayload wrongContentType =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferPayload(
            OfflineNoteV2TransferHandoff.OfflineNoteV2TransferModality.NEARBY,
            OfflineNoteV2TransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
            payload.payload());
    assertThrows(
        () -> OfflineNoteV2TransferHandoff.decodePaymentToken(wrongContentType),
        "wrong handoff content type should fail");

    final List<byte[]> frames =
        OfflineNoteV2TransferHandoff.qrStreamingFrameBytes(
            token, new OfflineQrStream.Options(128, 0));
    assertTrue(frames.size() > 2, "adversarial stream test needs multiple data frames");

    final byte[] badMagic = Arrays.copyOf(frames.get(0), frames.get(0).length);
    badMagic[0] = 0x00;
    assertThrows(
        () -> new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver().ingestFrame(badMagic),
        "bad QR stream magic should fail");

    final byte[] badVersion = Arrays.copyOf(frames.get(0), frames.get(0).length);
    badVersion[2] = 0x7f;
    assertThrows(
        () -> new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver().ingestFrame(badVersion),
        "bad QR stream version should fail");

    final byte[] badChecksum = Arrays.copyOf(frames.get(1), frames.get(1).length);
    badChecksum[badChecksum.length - 1] ^= 0x01;
    assertThrows(
        () -> new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver().ingestFrame(badChecksum),
        "bad QR stream checksum should fail");

    final byte[] truncated = Arrays.copyOfRange(frames.get(0), 0, 8);
    assertThrows(
        () -> new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver().ingestFrame(truncated),
        "truncated QR stream frame should fail");

    final OfflineQrStream.Frame header = OfflineQrStream.Frame.decode(frames.get(0));
    final byte[] mismatchedHeaderStreamId = header.streamId();
    mismatchedHeaderStreamId[0] ^= 0x01;
    final byte[] mismatchedHeader =
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.HEADER,
                mismatchedHeaderStreamId,
                header.index(),
                header.total(),
                header.payload())
            .encode();
    assertThrows(
        () -> new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver().ingestFrame(mismatchedHeader),
        "header stream id mismatch should fail");

    final OfflineQrStream.Frame firstData = OfflineQrStream.Frame.decode(frames.get(1));
    final byte[] wrongStreamId = firstData.streamId();
    wrongStreamId[0] ^= 0x7f;
    final byte[] wrongStreamFrame =
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                wrongStreamId,
                firstData.index(),
                firstData.total(),
                firstData.payload())
            .encode();
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver ignoreWrongStreamReceiver =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver();
    assertTrue(!ignoreWrongStreamReceiver.ingestFrame(frames.get(0)).isComplete(), "header only is incomplete");
    assertTrue(!ignoreWrongStreamReceiver.ingestFrame(wrongStreamFrame).isComplete(), "wrong stream data is ignored");
    OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamResult completed = null;
    for (int index = 1; index < frames.size(); index++) {
      completed = ignoreWrongStreamReceiver.ingestFrame(frames.get(index));
    }
    assertTrue(completed != null && completed.token() != null, "valid stream should still complete");
    assertEquals(token.tokenIdHex(), completed.token().tokenIdHex(), "valid stream token after wrong-stream data");

    final byte[] poisonedPayload = firstData.payload();
    poisonedPayload[0] ^= 0x01;
    final byte[] poisonedFrame =
        new OfflineQrStream.Frame(
                OfflineQrStream.FrameKind.DATA,
                firstData.streamId(),
                firstData.index(),
                firstData.total(),
                poisonedPayload)
            .encode();
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver poisonedReceiver =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver();
    poisonedReceiver.ingestFrame(frames.get(0));
    poisonedReceiver.ingestFrame(poisonedFrame);
    assertThrows(
        () -> {
          for (int index = 2; index < frames.size(); index++) {
            poisonedReceiver.ingestFrame(frames.get(index));
          }
        },
        "valid-CRC poisoned data chunk should fail final payload hash");

    final List<byte[]> wrongKindFrames =
        OfflineQrStream.Encoder.encodeFrameBytes(
            rawPayload,
            OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK_V2,
            new OfflineQrStream.Options(512, 0));
    final OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver wrongKindReceiver =
        new OfflineNoteV2TransferHandoff.OfflineNoteV2TransferStreamReceiver();
    assertThrows(
        () -> {
          for (final byte[] frame : wrongKindFrames) {
            wrongKindReceiver.ingestFrame(frame);
          }
        },
        "non-payment stream payload kind should fail");
  }

  private static void nfcApduProtocolSupportsAndroidSafeAndIosFastChunks() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] payload = OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(token);

    assertEquals(
        OfflineNoteV2TransferHandoff.DEFAULT_NFC_AID_HEX,
        OfflineNoteV2NfcApduProtocol.AID_HEX,
        "NFC AID");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.select()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(OfflineNoteV2NfcApduProtocol.selectAidApdu())),
        "select APDU");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.getInfo()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(OfflineNoteV2NfcApduProtocol.getInfoApdu())),
        "get-info APDU");

    final byte[] infoBytes =
        OfflineNoteV2NfcApduProtocol.encodeInfo(
            OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload);
    final OfflineNoteV2NfcApduProtocol.PayloadInfo info =
        OfflineNoteV2NfcApduProtocol.decodeInfo(infoBytes);
    assertTrue(info != null, "NFC info decodes");
    assertTrue(info.kind() == OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, "info kind");
    assertEquals(payload.length, info.payloadLength(), "info payload length");
    assertEquals(
        OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES,
        info.maxChunkLength(),
        "info max chunk length");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.payloadDigestMatches(payload, info.sha256()),
        "info digest");

    final List<byte[]> androidApdus = OfflineNoteV2TransferHandoff.nfcPaymentTokenWriteApdus(token);
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.writeMeta(
                OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
                payload.length,
                info.sha256())
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(androidApdus.get(0))),
        "payment write meta");
    for (int index = 1; index < androidApdus.size() - 1; index++) {
      final OfflineNoteV2NfcApduProtocol.Command command =
          OfflineNoteV2NfcApduProtocol.parseCommand(androidApdus.get(index));
      assertTrue(
          command.type() == OfflineNoteV2NfcApduProtocol.Type.WRITE_CHUNK,
          "payment write chunk");
      assertTrue(
          command.bytes().length <= OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES,
          "payment write chunk fits Android APDU budget");
    }
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.commit()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(androidApdus.get(androidApdus.size() - 1))),
        "payment commit");

    final byte[] fastPayload = new byte[512];
    Arrays.fill(fastPayload, (byte) 0x5A);
    final byte[] fastApdu = OfflineNoteV2NfcApduProtocol.writeChunkApdu(1024, fastPayload);
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0x80, 0x21, 0x04, 0x00, 0x00, 0x02, 0x00},
            Arrays.copyOfRange(fastApdu, 0, 7)),
        "iOS fast extended write header");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.writeChunk(1024, fastPayload)
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(fastApdu)),
        "iOS fast extended write parse");
    final byte[] fastRead =
        OfflineNoteV2NfcApduProtocol.readChunkApdu(
            256, OfflineNoteV2NfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES);
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.readChunk(
                256, OfflineNoteV2NfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES)
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(fastRead)),
        "iOS fast extended read parse");
  }

  private static void transportWireFormatMatchesSharedFixture() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] payload = OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(token);
    final List<byte[]> writeApdus = OfflineNoteV2TransferHandoff.nfcPaymentTokenWriteApdus(token);
    final List<byte[]> readApdus = OfflineNoteV2NfcApduProtocol.readPayloadApdus(payload.length);
    final byte[] nearbyBytes = OfflineNoteV2TransferHandoff.nearbyPaymentEnvelopeBytes(token);

    assertEquals(2416, payload.length, "transport fixture payload length");
    assertEquals(
        "00a4040007f049524f48413200",
        hex(OfflineNoteV2NfcApduProtocol.selectAidApdu()),
        "select APDU fixture");
    assertEquals(
        "8010000000", hex(OfflineNoteV2NfcApduProtocol.getInfoApdu()), "get-info APDU fixture");
    assertEquals(
        "01020000097000f044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c",
        hex(
            OfflineNoteV2NfcApduProtocol.encodeInfo(
                OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload)),
        "NFC info fixture");
    assertEquals(
        "802000002601020000097044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c",
        hex(
            OfflineNoteV2NfcApduProtocol.writeMetaApdu(
                OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload)),
        "NFC write-meta fixture");
    assertEquals(13, writeApdus.size(), "NFC write APDU count");
    assertEquals(
        "802000002601020000097044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c",
        hex(writeApdus.get(0)),
        "NFC first write APDU fixture");
    assertEquals(
        "4037d861f58cb4820507bd2fe905e395dfc326e93613eb2dd885ba0235cfd053",
        hex(OfflineNoteV2NfcApduProtocol.sha256(writeApdus.get(1))),
        "NFC first chunk fixture digest");
    assertEquals(
        "802109601063746f722d61756469742d70726f6f66",
        hex(writeApdus.get(writeApdus.size() - 2)),
        "NFC final chunk fixture");
    assertEquals("8022000000", hex(writeApdus.get(writeApdus.size() - 1)), "NFC commit fixture");
    assertEquals(11, readApdus.size(), "NFC read APDU count");
    assertEquals("80110000f0", hex(readApdus.get(0)), "NFC first read APDU fixture");
    assertEquals(3335, nearbyBytes.length, "Nearby payment envelope fixture length");
    assertEquals(
        "ce3207d3c55c3d89fc91012bb96546ea7ed71617545bc90b266a3c7bd67aec5c",
        hex(OfflineNoteV2NfcApduProtocol.sha256(nearbyBytes)),
        "Nearby payment envelope fixture digest");
  }

  private static void nfcApduProtocolRejectsAdversarialPayloadsBeforeCommit() {
    final byte[] payload = "offline-payment".getBytes(StandardCharsets.UTF_8);
    final OfflineNoteV2NfcApduProtocol.PayloadInfo info =
        OfflineNoteV2NfcApduProtocol.decodeInfo(
            OfflineNoteV2NfcApduProtocol.encodeInfo(
                OfflineNoteV2NfcApduProtocol.PayloadKind.RECEIPT_ACK, payload));
    assertTrue(info != null, "receipt ACK info decodes");
    final OfflineNoteV2NfcApduProtocol.PayloadAssembler assembler =
        new OfflineNoteV2NfcApduProtocol.PayloadAssembler(info);

    assertTrue(
        !assembler.write(payload.length - 2, new byte[] {1, 1, 1, 1}),
        "reject chunk past declared length");
    assertTrue(assembler.write(0, Arrays.copyOfRange(payload, 0, 6)), "accept first chunk");
    assertTrue(assembler.write(0, Arrays.copyOfRange(payload, 0, 6)), "accept identical duplicate");
    assertTrue(
        !assembler.write(0, "OFFLIN".getBytes(StandardCharsets.UTF_8)),
        "reject conflicting duplicate");
    assertThrows(assembler::commit, "incomplete payload commit should fail");
    assertTrue(
        assembler.write(6, Arrays.copyOfRange(payload, 6, payload.length)),
        "accept final chunk");
    assertTrue(Arrays.equals(payload, assembler.commit()), "assembled payload");

    final byte[] oversizedInfo =
        OfflineNoteV2NfcApduProtocol.encodeInfo(
            OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload);
    final int oversized = OfflineNoteV2NfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1;
    oversizedInfo[2] = (byte) ((oversized >>> 24) & 0xFF);
    oversizedInfo[3] = (byte) ((oversized >>> 16) & 0xFF);
    oversizedInfo[4] = (byte) ((oversized >>> 8) & 0xFF);
    oversizedInfo[5] = (byte) (oversized & 0xFF);
    assertTrue(OfflineNoteV2NfcApduProtocol.decodeInfo(oversizedInfo) == null, "reject oversized info");

    final OfflineNoteV2NfcApduProtocol.PayloadAssembler badAssembler =
        new OfflineNoteV2NfcApduProtocol.PayloadAssembler(
            OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload.length, new byte[32]);
    assertTrue(badAssembler.write(0, payload), "bad checksum payload written");
    assertThrows(badAssembler::commit, "checksum mismatch commit should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NfcApduProtocol.PayloadAssembler(
                OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
                OfflineNoteV2NfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1,
                new byte[32]),
        "oversized assembler should fail before allocation");
  }

  private static void nfcApduProtocolRejectsMalformedCommandsAndBounds() {
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(null)),
        "null APDU should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(new byte[] {0x00})),
        "short APDU should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.unsupported()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {0x00, (byte) 0xA4, 0x04, 0x00, 0x01, (byte) 0xFF, 0x00})),
        "wrong AID should be unsupported");
    final byte[] selectWithNonZeroLe = OfflineNoteV2NfcApduProtocol.selectAidApdu();
    selectWithNonZeroLe[selectWithNonZeroLe.length - 1] = 0x01;
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.unsupported()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(selectWithNonZeroLe)),
        "select AID with nonzero Le should be unsupported");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.unsupported()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x81, 0x10, 0x00, 0x00, 0x00})),
        "wrong CLA should be unsupported");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x01, 0x00})),
        "get-info with nonzero P1/P2 should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x00, 0x01})),
        "get-info with nonzero Le should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x00, 0x01, 0x00})),
        "get-info with data should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x11, 0x00, 0x00, 0x00})),
        "zero short read length should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00})),
        "zero-length extended read should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x20, 0x00, 0x00, 0x01, 0x01})),
        "short write-meta should be invalid");
    final byte[] writeMetaWithOffset =
        OfflineNoteV2NfcApduProtocol.writeMetaApdu(
            OfflineNoteV2NfcApduProtocol.PayloadKind.RECEIPT_ACK, new byte[] {0x01});
    writeMetaWithOffset[3] = 0x01;
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(writeMetaWithOffset)),
        "write-meta with nonzero P1/P2 should be invalid");
    final byte[] zeroLengthMeta = new byte[38];
    zeroLengthMeta[0] = 0x01;
    zeroLengthMeta[1] = (byte) OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN.code();
    final byte[] zeroLengthMetaApdu = new byte[43];
    zeroLengthMetaApdu[0] = (byte) 0x80;
    zeroLengthMetaApdu[1] = 0x20;
    zeroLengthMetaApdu[4] = (byte) zeroLengthMeta.length;
    System.arraycopy(zeroLengthMeta, 0, zeroLengthMetaApdu, 5, zeroLengthMeta.length);
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(OfflineNoteV2NfcApduProtocol.parseCommand(zeroLengthMetaApdu)),
        "zero-length write-meta should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x21, 0x00, 0x00, 0x00})),
        "empty write chunk should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x21, 0x00, 0x00, 0x02, 0x01})),
        "truncated write chunk should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x00, 0x00, 0x01, 0x00})),
        "commit with data should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x01, 0x00, 0x00})),
        "commit with nonzero P1/P2 should be invalid");
    assertTrue(
        OfflineNoteV2NfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteV2NfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x00, 0x00, 0x01})),
        "commit with nonzero Le should be invalid");

    assertThrows(
        () -> OfflineNoteV2NfcApduProtocol.writeChunkApdu(0x1_0000, new byte[] {0x01}),
        "oversized offset should fail");
    assertThrows(
        () -> OfflineNoteV2NfcApduProtocol.writeChunkApdu(0, new byte[0]),
        "empty write chunk should fail");
    assertThrows(
        () -> OfflineNoteV2NfcApduProtocol.readChunkApdu(0, 0),
        "zero read chunk length should fail");
    assertThrows(
        () ->
            OfflineNoteV2NfcApduProtocol.readChunkApdu(
                0, OfflineNoteV2NfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1),
        "oversized direct read chunk length should fail");
    assertThrows(
        () ->
            OfflineNoteV2NfcApduProtocol.writePayloadApdus(
                OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN, new byte[] {0x01}, 0),
        "zero max chunk length should fail");
    assertThrows(
        () -> OfflineNoteV2NfcApduProtocol.readPayloadApdus(0),
        "zero read payload length should fail");
    assertThrows(
        () ->
            OfflineNoteV2NfcApduProtocol.readPayloadApdus(
                1, OfflineNoteV2NfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1),
        "oversized read chunk length should fail");

    final byte[] response = OfflineNoteV2NfcApduProtocol.response(new byte[] {(byte) 0xAA, (byte) 0xBB});
    assertTrue(
        Arrays.equals(new byte[] {(byte) 0xAA, (byte) 0xBB, (byte) 0x90, 0x00}, response),
        "response should append success status");
    assertEquals(0x9000, OfflineNoteV2NfcApduProtocol.responseStatus(response), "response status");
    assertEquals(
        -1,
        OfflineNoteV2NfcApduProtocol.responseStatus(new byte[] {(byte) 0x90}),
        "short response status");
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0xAA, (byte) 0xBB},
            OfflineNoteV2NfcApduProtocol.responseData(response)),
        "response data");
    assertTrue(
        Arrays.equals(new byte[0], OfflineNoteV2NfcApduProtocol.responseData(new byte[] {(byte) 0x90})),
        "short response data");

    final OfflineNoteV2NfcApduProtocol.PayloadAssembler assembler =
        new OfflineNoteV2NfcApduProtocol.PayloadAssembler(
            OfflineNoteV2NfcApduProtocol.PayloadKind.RECEIPT_ACK,
            4,
            OfflineNoteV2NfcApduProtocol.sha256(new byte[] {0x01, 0x02, 0x03, 0x04}));
    assertTrue(!assembler.write(Integer.MAX_VALUE, new byte[] {0x01}), "huge offset rejected");
    assertTrue(!assembler.write(4, new byte[] {0x01}), "end offset rejected");
    assertTrue(!assembler.write(-1, new byte[] {0x01}), "negative offset rejected");
    assertTrue(!assembler.write(0, new byte[0]), "empty assembler chunk rejected");
    assertTrue(assembler.write(0, new byte[] {0x01, 0x02}), "first partial overlap chunk");
    assertTrue(assembler.write(1, new byte[] {0x02, 0x03}), "identical overlap accepted");
    assertTrue(!assembler.write(1, new byte[] {0x09, 0x09}), "conflicting overlap rejected");
  }

  private static void nearbyEnvelopeRoundTripsPairingPaymentAndAck() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteV2ReceiveRequest receiveRequest = receiveRequestFixture(fixture);
    final OfflineNoteV2ReceiptAck receiptAck =
        OfflineNoteV2ReceiptAck.fromPaymentToken(
            token,
            string(obj(fixture, "payment_token"), "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));
    final OfflineNoteV2NearbyEnvelope.PairingChallenge challenge =
        new OfflineNoteV2NearbyEnvelope.PairingChallenge(" nearby_pairing_bird ");
    final OfflineNoteV2NearbyEnvelope challengeEnvelope =
        new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.CHALLENGE,
            OfflineNoteV2TransferHandoff.rawReceiveRequestBytes(receiveRequest),
            OfflineNoteV2TransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            challenge);
    final byte[] paymentBytes = OfflineNoteV2TransferHandoff.nearbyPaymentEnvelopeBytes(token);
    final OfflineNoteV2NearbyEnvelope paymentEnvelope =
        OfflineNoteV2NearbyEnvelope.decode(paymentBytes);
    final OfflineNoteV2NearbyEnvelope ackEnvelope =
        new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.RECEIPT_ACK,
            OfflineNoteV2TransferHandoff.rawReceiptAckBytes(receiptAck),
            OfflineNoteV2TransferHandoff.RECEIPT_ACK_CONTENT_TYPE);

    assertTrue(
        challenge.equals(
            OfflineNoteV2NearbyEnvelope.decode(challengeEnvelope.encoded()).pairingChallenge()),
        "challenge pairing roundtrip");
    assertTrue(paymentEnvelope.kind() == OfflineNoteV2NearbyEnvelope.Kind.PAYMENT, "payment kind");
    assertEquals(
        receiveRequest.outputCommitmentHex(),
        OfflineNoteV2NearbyEnvelope.decode(challengeEnvelope.encoded()).receiveRequest().outputCommitmentHex(),
        "receive request");
    assertEquals(token.tokenIdHex(), paymentEnvelope.paymentToken().tokenIdHex(), "payment token");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteV2TransferHandoff.decodeNearbyPaymentToken(paymentBytes).tokenIdHex(),
        "payment token handoff decode");
    assertTrue(
        OfflineNoteV2NearbyEnvelope.decode(ackEnvelope.encoded()).receiptAck().matchesPaymentToken(token),
        "ACK payload");
  }

  private static void nearbyEnvelopeRejectsAdversarialMessages() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final byte[] tokenPayload = base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64"));
    final OfflineNoteV2NearbyEnvelope.PairingChallenge pairing =
        new OfflineNoteV2NearbyEnvelope.PairingChallenge("nearby_pairing_mask");

    assertThrows(
        () -> new OfflineNoteV2NearbyEnvelope.PairingChallenge("nearby_pairing_mask<script>"),
        "invalid pairing asset should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.CHALLENGE,
                "challenge".getBytes(StandardCharsets.UTF_8),
                OfflineNoteV2TransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE),
        "challenge without pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.CHALLENGE,
                "challenge".getBytes(StandardCharsets.UTF_8),
                OfflineNoteV2TransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
                pairing),
        "challenge content type downgrade should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.PAYMENT,
                tokenPayload,
                OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
                pairing),
        "payment with pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.PAYMENT,
                new byte[OfflineNoteV2NfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1],
                OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE),
        "oversized nearby payment should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.RECEIPT_ACK,
                "ok".getBytes(StandardCharsets.UTF_8),
                OfflineNoteV2TransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE),
        "receipt ACK content type downgrade should fail");

    final byte[] unsupportedVersion =
        ("{\"version\":2,\"kind\":\"payment\",\"payload\":\"AQID\","
                + "\"contentType\":\"application/vnd.iroha.offline.payment-token-v2+norito\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] fractionalVersion =
        ("{\"version\":1.5,\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] unknownField =
        ("{\"version\":1,\"kind\":\"payment\",\"payload\":\"AQID\","
                + "\"contentType\":\"application/vnd.iroha.offline.payment-token-v2+norito\","
                + "\"extra\":true}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] challengeContentTypeDowngrade =
        ("{\"version\":1,\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receipt-ack-v2+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] ackContentTypeDowngrade =
        ("{\"version\":1,\"kind\":\"receipt_ack\",\"payload\":\"b2s\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedPayload =
        ("{\"version\":1,\"kind\":\"challenge\",\"payload\":\"YQ==\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(unsupportedVersion),
        "unsupported nearby envelope version should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(fractionalVersion),
        "fractional nearby envelope version should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(unknownField),
        "unknown nearby envelope field should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(challengeContentTypeDowngrade),
        "challenge content type downgrade should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(ackContentTypeDowngrade),
        "receipt ACK content type downgrade should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(paddedPayload),
        "padded nearby envelope payload should fail");
    final byte[] topLevelArray = "[]".getBytes(StandardCharsets.UTF_8);
    final byte[] invalidBase64Payload =
        ("{\"version\":1,\"kind\":\"challenge\",\"payload\":\"!!!!\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] badPairingObject =
        ("{\"version\":1,\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\","
                + "\"pairingChallenge\":{\"assetName\":1}}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] smuggledPairingObject =
        ("{\"version\":1,\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request-v2+norito\","
                + "\"pairingChallenge\":{\"assetName\":\"nearby_pairing_bird\",\"extra\":true}}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] ackWithPairing =
        ("{\"version\":1,\"kind\":\"receipt_ack\",\"payload\":\"b2s\","
                + "\"contentType\":\"application/vnd.iroha.offline.receipt-ack-v2+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(topLevelArray),
        "top-level array nearby envelope should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(invalidBase64Payload),
        "invalid nearby envelope base64url should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(badPairingObject),
        "bad nearby pairing object should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(smuggledPairingObject),
        "nearby pairing object with unknown field should fail");
    assertThrows(
        () -> OfflineNoteV2NearbyEnvelope.decode(ackWithPairing),
        "ACK with pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.PAYMENT,
                new byte[] {0x01, 0x02, 0x03},
                OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE),
        "invalid payment token payload should fail");
    assertThrows(
        () ->
            new OfflineNoteV2NearbyEnvelope(
                OfflineNoteV2NearbyEnvelope.Kind.RECEIPT_ACK,
                new byte[0],
                OfflineNoteV2TransferHandoff.RECEIPT_ACK_CONTENT_TYPE),
        "empty ACK payload should fail");
  }

  private static void walletAcceptsCanonicalSdkInteropPaymentToken() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNoteV2.KeyCertificateV2 recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteV2Store recipientStore = new InMemoryOfflineNoteV2Store();
    final OfflineNoteV2Wallet recipientWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            recipientStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_200L);
    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(obj(chain, "issue"), "asset_id")),
            string(obj(chain, "redeem"), "amount"));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        receiveRequest.outputCommitmentHex(),
        "canonical receive output commitment");

    final OfflineNoteV2PaymentToken token =
        OfflineNoteV2PaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteV2WalletNote accepted = recipientWallet.accept(token);

    assertEquals(
        string(derivation, "recipient_output_commitment"),
        accepted.noteCommitmentHex(),
        "accepted canonical note commitment");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted canonical note state");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        recipientStore
            .findNote(hexBytes(string(derivation, "recipient_output_commitment")))
            .state()
            .name(),
        "stored canonical note state");
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

    final String encoded =
        new String(OfflineNoteV2WalletNoteJsonCodec.encode(note), StandardCharsets.UTF_8);
    final OfflineNoteV2WalletNote migratedSpent =
        OfflineNoteV2WalletNoteJsonCodec.decode(
            encoded.replace("\"state\":\"SPENDABLE\"", "\"state\":\"SPEND_PENDING\"")
                .getBytes(StandardCharsets.UTF_8));
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENT.name(),
        migratedSpent.state().name(),
        "migrated spend pending state");
    final OfflineNoteV2WalletNote migratedChange =
        OfflineNoteV2WalletNoteJsonCodec.decode(
            encoded.replace("\"state\":\"SPENDABLE\"", "\"state\":\"CHANGE_PENDING\"")
                .getBytes(StandardCharsets.UTF_8));
    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        migratedChange.state().name(),
        "migrated change pending state");
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
            certificateVerifier(fixture),
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
    bindingJson.put("attestation_key_id", "attestation-key-1");
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
    assertEquals(0L, longValue(refillBody, "local_revision"), "refill local revision");
    assertEquals("", string(refillBody, "local_state_hash"), "refill local state hash");
    assertEquals("attestation-key-1", string(refillBody, "attestation_key_id"), "refill attestation key");
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

  private static void walletLoadDoesNotBlockIssuerCompletionThread() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> token = obj(fixture, "payment_token");
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> issue = obj(obj(fixture, "chain_vectors"), "issue");
    final OfflineNoteV2.KeyCertificateV2 senderCertificate =
        certificate(obj(token, "sender_key_certificate"));
    final String accountId = accountFromAssetId(string(issue, "asset_id"));
    final OfflineNoteV2LoadContext loadContext =
        new OfflineNoteV2LoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final CompletionControlledIssuerClient issuerClient =
        new CompletionControlledIssuerClient(loadContext);
    final BlockingOfflineNoteV2Store store = new BlockingOfflineNoteV2Store();
    final OfflineNoteV2Wallet wallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            accountId,
            new StaticAttestationProvider(senderCertificate),
            store,
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "source_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_000L);

    final CompletableFuture<OfflineNoteV2WalletNote> load =
        wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount"));
    assertTrue(
        issuerClient.issueRequested.await(5, TimeUnit.SECONDS),
        "wallet load did not submit issue request");

    final OfflineNoteV2IssueRequest request = issuerClient.lastIssueRequest;
    final OfflineNoteV2IssueResponse response =
        new OfflineNoteV2IssueResponse(
            request.noteCommitment(),
            request.loadContext().operationId(),
            request.loadContext().lineageId(),
            request.loadContext().localRevision(),
            request.loadContext().keyCertificate(),
            "settlement-entry-hash");
    final AtomicBoolean completeReturned = new AtomicBoolean(false);
    final ExecutorService issuerCompleter =
        Executors.newSingleThreadExecutor(r -> new Thread(r, "offline-note-v2-issuer-completer"));
    try {
      issuerCompleter.submit(
          () -> {
            issuerClient.issueFuture.complete(response);
            completeReturned.set(true);
          });
      assertTrue(
          store.entered.await(5, TimeUnit.SECONDS),
          "wallet load did not enter note persistence after issuer response");
      assertTrue(
          completeReturned.get(),
          "wallet load must not block the issuer completion thread while persisting notes");
      store.release.countDown();
      assertEquals(
          string(derivation, "source_note_commitment"),
          load.get(5, TimeUnit.SECONDS).noteCommitmentHex(),
          "wallet load note commitment after asynchronous issue completion");
    } finally {
      store.release.countDown();
      issuerCompleter.shutdownNow();
    }
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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

  private static void walletRejectsAdversarialCertificateBindings() throws Exception {
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
    final String senderAccountId = accountFromAssetId(string(chainIssue, "asset_id"));
    final String assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"));

    final OfflineNoteV2Wallet defaultRejectingWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteV2Store(),
            null,
            null,
            BindingProofProvider.INSTANCE,
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_700L);
    assertThrows(
        () -> defaultRejectingWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount")),
        "default verifier should reject receive certificates");
    final OfflineNoteV2Wallet wrongAccountReceiveWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteV2Store(),
            null,
            null,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_710L);
    assertThrows(
        () -> wrongAccountReceiveWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount")),
        "valid receive certificate for the wrong account should fail");

    final InMemoryOfflineNoteV2Store senderStore = new InMemoryOfflineNoteV2Store();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet senderWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms"));
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
            certificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_800L);

    final OfflineNoteV2ReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount"));
    final OfflineNoteV2ReceiveRequest accountSubstitution =
        new OfflineNoteV2ReceiveRequest(
            receiveRequest.chainId(),
            receiveRequest.paymentRequestId(),
            senderAccountId,
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetId(),
            receiveRequest.amount(),
            receiveRequest.keyCertificate(),
            receiveRequest.outputCommitment());
    assertThrows(
        () -> senderWallet.pay(accountSubstitution),
        "receive request account substitution should fail certificate binding");
    final OfflineNoteV2ReceiveRequest chainSubstitution =
        new OfflineNoteV2ReceiveRequest(
            receiveRequest.chainId() + "-evil",
            receiveRequest.paymentRequestId(),
            receiveRequest.accountId(),
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetId(),
            receiveRequest.amount(),
            receiveRequest.keyCertificate(),
            receiveRequest.outputCommitment());
    assertThrows(
        () -> senderWallet.pay(chainSubstitution),
        "receive request chain substitution should fail");
    final OfflineNoteV2ReceiveRequest assetOwnerSubstitution =
        new OfflineNoteV2ReceiveRequest(
            receiveRequest.chainId(),
            receiveRequest.paymentRequestId(),
            receiveRequest.accountId(),
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetDefinitionId() + "#" + senderAccountId,
            receiveRequest.amount(),
            receiveRequest.keyCertificate(),
            receiveRequest.outputCommitment());
    final InMemoryOfflineNoteV2Store assetOwnerSubstitutionStore = new InMemoryOfflineNoteV2Store();
    assetOwnerSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet assetOwnerSubstitutionSender =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            assetOwnerSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x21), filledBytes(32, 0x22))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 3);
    assertThrows(
        () -> assetOwnerSubstitutionSender.pay(assetOwnerSubstitution),
        "receive request asset owner substitution should fail certificate binding");

    final InMemoryOfflineNoteV2Store forgedInputStore = new InMemoryOfflineNoteV2Store();
    forgedInputStore.upsert(sourceWalletNote(fixture, tamperedSignatureCertificate(senderCertificate)));
    final OfflineNoteV2Wallet forgedInputWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            forgedInputStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_900L);
    assertThrows(
        () -> forgedInputWallet.pay(receiveRequest),
        "stored input with tampered certificate should fail");
    final InMemoryOfflineNoteV2Store wrongAccountInputStore = new InMemoryOfflineNoteV2Store();
    wrongAccountInputStore.upsert(sourceWalletNote(fixture, recipientCertificate));
    final OfflineNoteV2Wallet wrongAccountInputWallet =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            wrongAccountInputStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_910L);
    assertThrows(
        () -> wrongAccountInputWallet.pay(receiveRequest),
        "valid stored input certificate for the wrong account should fail");
    final InMemoryOfflineNoteV2Store commitmentSubstitutionStore = new InMemoryOfflineNoteV2Store();
    commitmentSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet commitmentSubstitutionSender =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            commitmentSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x31), filledBytes(32, 0x32))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 1);
    final OfflineNoteV2ReceiveRequest commitmentSubstitution =
        new OfflineNoteV2ReceiveRequest(
            receiveRequest.chainId(),
            receiveRequest.paymentRequestId(),
            receiveRequest.accountId(),
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetId(),
            receiveRequest.amount(),
            receiveRequest.keyCertificate(),
            filledBytes(32, 0xA5));
    assertThrows(
        () -> recipientWallet.accept(commitmentSubstitutionSender.pay(commitmentSubstitution)),
        "receive request output commitment substitution should not match recipient pending note");
    final String forgedOutputAmount = receiveRequest.amount().equals("1") ? "2" : "1";
    final InMemoryOfflineNoteV2Store amountSubstitutionStore = new InMemoryOfflineNoteV2Store();
    amountSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteV2Wallet amountSubstitutionSender =
        new OfflineNoteV2Wallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            amountSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            certificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x41), filledBytes(32, 0x42))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 2);
    final OfflineNoteV2ReceiveRequest amountSubstitution =
        new OfflineNoteV2ReceiveRequest(
            receiveRequest.chainId(),
            receiveRequest.paymentRequestId(),
            receiveRequest.accountId(),
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetId(),
            forgedOutputAmount,
            receiveRequest.keyCertificate(),
            receiveRequest.outputCommitment());
    assertThrows(
        () -> recipientWallet.accept(amountSubstitutionSender.pay(amountSubstitution)),
        "receive request amount substitution should not match recipient pending note");

    final OfflineNoteV2PaymentToken token = senderWallet.pay(receiveRequest);
    assertThrows(
        () -> recipientWallet.accept(paymentTokenReplacingChainId(token, token.chainId() + "-evil")),
        "payment token chain substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingPaymentRequestId(token, token.paymentRequestId() + "-evil")),
        "payment token payment request substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(paymentTokenReplacingTopLevelTokenId(token)),
        "payment token top-level token id substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(paymentTokenReplacingAuditTokenId(token)),
        "payment token audit token id substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingFirstOutputAmountWithoutProofRebind(token, forgedOutputAmount)),
        "payment token stale proof public inputs should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingFirstOutputAmount(token, forgedOutputAmount)),
        "payment token output amount substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingFirstOutputAsset(
                token,
                receiveRequest.assetId() + "#dataspace:1")),
        "payment token output asset substitution should fail");
    if (token.audit().outputClaims().size() < 2) {
      throw new AssertionError("fixture should include recipient and change outputs");
    }
    assertThrows(
        () -> recipientWallet.accept(paymentTokenReversingOutputs(token)),
        "payment token output order substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(paymentTokenDroppingFirstOutput(token)),
        "payment token missing recipient output should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingFirstOutputCertificate(token, senderCertificate)),
        "payment token output certificate account substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingLastOutputCertificate(token, recipientCertificate)),
        "payment token change output certificate account substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingFirstInputClaimHash(
                token, recipientCertificate.payloadHash())),
        "payment token input claim certificate hash substitution should fail");
    assertThrows(
        () -> recipientWallet.accept(
            paymentTokenReplacingSenderCertificate(token, recipientCertificate)),
        "payment token sender certificate account substitution should fail");

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        recipientWallet.accept(token).state().name(),
        "valid token remains acceptable after rejected adversarial tokens");
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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
            certificateVerifier(fixture),
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

  private static OfflineNoteV2CertificateVerifier certificateVerifier(
      final Map<String, Object> fixture) {
    return new Ed25519OfflineNoteV2CertificateVerifier(
        Collections.singletonList(base64Bytes(string(fixture, "offline_fi_public_key_base64"))));
  }

  private static OfflineNoteV2.KeyCertificateV2 tamperedSignatureCertificate(
      final OfflineNoteV2.KeyCertificateV2 certificate) {
    final byte[] signature = certificate.issuerSignature();
    signature[0] = (byte) (signature[0] ^ 0x01);
    return new OfflineNoteV2.KeyCertificateV2(
        certificate.version(),
        certificate.platform(),
        certificate.keyId(),
        certificate.deviceId(),
        certificate.accountId(),
        certificate.publicKey(),
        certificate.assertionScheme(),
        certificate.assertionKeyAlgorithm(),
        certificate.assertionPublicKey(),
        certificate.assertionUsageCountLimit(),
        certificate.oneUse(),
        signature);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingFirstOutputCertificate(
      final OfflineNoteV2PaymentToken token,
      final OfflineNoteV2.KeyCertificateV2 certificate) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNoteV2.AuditOutputClaimV2 output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNoteV2.AuditOutputClaimV2(
            output.noteCommitment(), certificate, output.assetId(), output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingFirstOutputAmount(
      final OfflineNoteV2PaymentToken token,
      final String amount) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNoteV2.AuditOutputClaimV2 output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNoteV2.AuditOutputClaimV2(
            output.noteCommitment(), output.keyCertificate(), output.assetId(), amount));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingFirstOutputAmountWithoutProofRebind(
      final OfflineNoteV2PaymentToken token,
      final String amount) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNoteV2.AuditOutputClaimV2 output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNoteV2.AuditOutputClaimV2(
            output.noteCommitment(), output.keyCertificate(), output.assetId(), amount));
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        new OfflineNoteV2.AuditBundleV2(
            token.audit().tokenId(),
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            outputClaims,
            token.audit().recursiveProof()),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingFirstOutputAsset(
      final OfflineNoteV2PaymentToken token,
      final String assetId) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNoteV2.AuditOutputClaimV2 output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNoteV2.AuditOutputClaimV2(
            output.noteCommitment(), output.keyCertificate(), assetId, output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReversingOutputs(
      final OfflineNoteV2PaymentToken token) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final List<byte[]> outputCommitments = new ArrayList<>(token.audit().outputCommitments());
    Collections.reverse(outputClaims);
    Collections.reverse(outputCommitments);
    return paymentTokenReplacingOutputs(token, outputClaims, outputCommitments);
  }

  private static OfflineNoteV2PaymentToken paymentTokenDroppingFirstOutput(
      final OfflineNoteV2PaymentToken token) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(
            token.audit().outputClaims().subList(1, token.audit().outputClaims().size()));
    final List<byte[]> outputCommitments =
        new ArrayList<>(
            token.audit().outputCommitments().subList(1, token.audit().outputCommitments().size()));
    return paymentTokenReplacingOutputs(token, outputClaims, outputCommitments);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingChainId(
      final OfflineNoteV2PaymentToken token, final String chainId) {
    return new OfflineNoteV2PaymentToken(
        chainId,
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        token.audit(),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingLastOutputCertificate(
      final OfflineNoteV2PaymentToken token,
      final OfflineNoteV2.KeyCertificateV2 certificate) {
    final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final int index = outputClaims.size() - 1;
    final OfflineNoteV2.AuditOutputClaimV2 output = outputClaims.get(index);
    outputClaims.set(
        index,
        new OfflineNoteV2.AuditOutputClaimV2(
            output.noteCommitment(), certificate, output.assetId(), output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingFirstInputClaimHash(
      final OfflineNoteV2PaymentToken token, final byte[] keyCertificatePayloadHash) {
    final List<OfflineNoteV2.IssuedClaimV2> inputClaims =
        new ArrayList<>(token.audit().inputClaims());
    final OfflineNoteV2.IssuedClaimV2 input = inputClaims.get(0);
    inputClaims.set(
        0,
        new OfflineNoteV2.IssuedClaimV2(
            input.domain(),
            input.noteCommitment(),
            keyCertificatePayloadHash,
            input.assetId(),
            input.amount()));
    return paymentTokenReplacingAuditClaims(token, inputClaims, token.audit().outputClaims());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingSenderCertificate(
      final OfflineNoteV2PaymentToken token,
      final OfflineNoteV2.KeyCertificateV2 certificate) {
    final byte[] certificateHash = certificate.payloadHash();
    final List<OfflineNoteV2.IssuedClaimV2> inputClaims = new ArrayList<>();
    for (final OfflineNoteV2.IssuedClaimV2 input : token.audit().inputClaims()) {
      inputClaims.add(
          new OfflineNoteV2.IssuedClaimV2(
              input.domain(),
              input.noteCommitment(),
              certificateHash,
              input.assetId(),
              input.amount()));
    }
    final byte[] tokenId =
        OfflineNoteV2.derivePaymentTokenId(
            new OfflineNoteV2.PaymentTokenIdPreimageV2(
                token.chainId(),
                token.paymentRequestId(),
                token.createdAtMs(),
                token.tokenNonce(),
                certificateHash,
                token.audit().inputNullifiers(),
                token.audit().outputCommitments()));
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            tokenId,
            certificate,
            token.audit().inputNullifiers(),
            inputClaims,
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNoteV2.RecursiveProofV2 proof =
        new OfflineNoteV2.RecursiveProofV2(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        tokenId,
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingPaymentRequestId(
      final OfflineNoteV2PaymentToken token, final String paymentRequestId) {
    final byte[] tokenId =
        OfflineNoteV2.derivePaymentTokenId(
            new OfflineNoteV2.PaymentTokenIdPreimageV2(
                token.chainId(),
                paymentRequestId,
                token.createdAtMs(),
                token.tokenNonce(),
                token.audit().senderKeyCertificate().payloadHash(),
                token.audit().inputNullifiers(),
                token.audit().outputCommitments()));
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            tokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNoteV2.RecursiveProofV2 proof =
        new OfflineNoteV2.RecursiveProofV2(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        paymentRequestId,
        token.tokenNonce(),
        tokenId,
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingTopLevelTokenId(
      final OfflineNoteV2PaymentToken token) {
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        flippedHash(token.tokenId()),
        token.audit(),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingAuditTokenId(
      final OfflineNoteV2PaymentToken token) {
    final byte[] auditTokenId = flippedHash(token.audit().tokenId());
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            auditTokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNoteV2.RecursiveProofV2 proof =
        new OfflineNoteV2.RecursiveProofV2(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingOutputs(
      final OfflineNoteV2PaymentToken token,
      final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims,
      final List<byte[]> outputCommitments) {
    final byte[] tokenId =
        OfflineNoteV2.derivePaymentTokenId(
            new OfflineNoteV2.PaymentTokenIdPreimageV2(
                token.chainId(),
                token.paymentRequestId(),
                token.createdAtMs(),
                token.tokenNonce(),
                token.audit().senderKeyCertificate().payloadHash(),
                token.audit().inputNullifiers(),
                outputCommitments));
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            tokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            outputCommitments,
            outputClaims,
            token.audit().recursiveProof());
    final OfflineNoteV2.RecursiveProofV2 proof =
        new OfflineNoteV2.RecursiveProofV2(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        tokenId,
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static byte[] flippedHash(final byte[] hash) {
    final byte[] copy = Arrays.copyOf(hash, hash.length);
    copy[0] = (byte) (copy[0] ^ 0x01);
    return copy;
  }

  private static OfflineNoteV2PaymentToken paymentTokenReplacingAuditClaims(
      final OfflineNoteV2PaymentToken token,
      final List<OfflineNoteV2.IssuedClaimV2> inputClaims,
      final List<OfflineNoteV2.AuditOutputClaimV2> outputClaims) {
    final OfflineNoteV2.AuditBundleV2 draft =
        new OfflineNoteV2.AuditBundleV2(
            token.audit().tokenId(),
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            inputClaims,
            token.audit().outputCommitments(),
            outputClaims,
            token.audit().recursiveProof());
    final OfflineNoteV2.RecursiveProofV2 proof =
        new OfflineNoteV2.RecursiveProofV2(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNoteV2PaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static byte[] filledBytes(final int length, final int value) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
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

  private static final class CompletionControlledIssuerClient implements OfflineNoteV2IssuerClient {
    private final OfflineNoteV2LoadContext loadContext;
    private final CountDownLatch issueRequested = new CountDownLatch(1);
    private final CompletableFuture<OfflineNoteV2IssueResponse> issueFuture =
        new CompletableFuture<>();
    private volatile OfflineNoteV2IssueRequest lastIssueRequest;

    private CompletionControlledIssuerClient(final OfflineNoteV2LoadContext loadContext) {
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
      issueRequested.countDown();
      return issueFuture;
    }
  }

  private static final class BlockingOfflineNoteV2Store implements OfflineNoteV2Store {
    private final Map<String, OfflineNoteV2WalletNote> notes = new LinkedHashMap<>();
    private final CountDownLatch entered = new CountDownLatch(1);
    private final CountDownLatch release = new CountDownLatch(1);

    @Override
    public synchronized <T> T mutateNotes(final Mutation<T> mutation) {
      entered.countDown();
      try {
        if (!release.await(5, TimeUnit.SECONDS)) {
          throw new IllegalStateException("timed out waiting to release blocked note store");
        }
      } catch (final InterruptedException ex) {
        Thread.currentThread().interrupt();
        throw new IllegalStateException("interrupted while waiting to release blocked note store", ex);
      }
      return mutation.apply(notes);
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

  private static byte[] mutatedHeaderFrame(
      final OfflineQrStream.Frame header, final Function<byte[], byte[]> mutator) {
    final byte[] envelope = header.payload();
    final byte[] mutated = mutator.apply(envelope);
    return new OfflineQrStream.Frame(
            OfflineQrStream.FrameKind.HEADER,
            header.streamId(),
            header.index(),
            header.total(),
            mutated)
        .encode();
  }

  private static void writeUInt16LE(final byte[] bytes, final int offset, final int value) {
    bytes[offset] = (byte) (value & 0xFF);
    bytes[offset + 1] = (byte) ((value >>> 8) & 0xFF);
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
