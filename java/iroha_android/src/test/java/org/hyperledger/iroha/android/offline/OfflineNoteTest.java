package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
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
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Function;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class OfflineNoteTest {

  private OfflineNoteTest() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    keyCertificatesRequireOneUseHardwareLimitWhenPresent();
    offlineNoteModelsMatchRustNoritoVectors();
    publicNoritoDecodersRoundTripFixturePayloads();
    publicNoritoInstructionDecodersReadExplorerEnvelopeBytes();
    walletDerivationsMatchRustVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    proofBindingRejectsRecursiveMetadataSubstitution();
    recursiveProofMetadataNormalizesAndRejectsMalformedVerifierKeys();
    instanceValuesMatchRustVectors();
    auditInstanceValuesRejectUnanchoredClaimsAndHiddenOutputs();
    kagemushaRecordBackedNativeProverValidatesInput();
    kagemushaCompactNativeInputCopiesBeforeDispatch();
    kagemushaRecursiveAggregationNativeProverValidatesInput();
    kagemushaRecursiveSpendNativeProverValidatesInput();
    kagemushaNativeProversRejectMissingAndEmptyNativeOutputs();
    kagemushaNativeAvailabilityRequiresJniEntrypoint();
    kagemushaRecursiveAggregationNativeAvailabilityRequiresJniEntrypoint();
    chainVkOfflineNoteProofWrappersValidateInputs();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
    paymentTokenCodecRoundTripsNoritoTextAndQrFrames();
    offlineBearerCashPolicyAndPrefixesUseSingleAppSurface();
    receiveRequestCodecRoundTripsNoritoTextAndQrFrames();
    receiptAckCodecRoundTripsNoritoTextAndQrFrames();
    receiptAckCodecRejectsNonPositiveAcceptedAtDecode();
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
    walletRejectsBearerCashCustodyPolicyOverflow();
    walletNoteJsonCodecRoundTripsFixtureNote();
    walletLoadDerivesCommitmentBeforeIssuerSubmission();
    walletLoadDoesNotBlockIssuerCompletionThread();
    walletLoadCompletesExceptionallyWhenIssuerThrowsSynchronously();
    toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment();
    toriiIssuerClientRejectsMalformedCertificateUsageLimits();
    walletLifecycleBuildsAuditAcceptAndRedeemTransactions();
    offlineNoteTransactionSubmitterIncludesFeeMetadata();
    walletSyncReconcilesPendingSpendChangeAndRedeemStates();
    walletRejectsDuplicateTokenAndAlreadyPendingInputs();
    walletRejectsExactAmountReceiveRequestReplayAfterRestart();
    walletRedeemReservesNoteBeforeSubmitCompletes();
    walletRejectsAdversarialCertificateBindings();
    walletSyncReconcilesFailedAuditAndRedeemOutcomes();
    outcomeIndexResolvesCommittedAndRejectedExplorerInstructions();
    System.out.println("[IrohaAndroid] OfflineNoteTest passed.");
  }

  private static void certificateSigningBytesMatchRustVector() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final Map<String, Object> certificates = obj(obj(fixture, "chain_vectors"), "certificates");
    final OfflineNoteCertificateVerifier verifier = certificateVerifier(fixture);

    assertEquals(
        string(certificates, "sender_payload_base64"),
        base64(sender.signingBytes()),
        "sender certificate payload");
    assertEquals(
        string(certificates, "sender_payload_hash"),
        hex(sender.payloadHash()),
        "sender certificate payload hash");
    assertTrue(verifier.verifyIssuerCertificate(sender), "fixture sender certificate is trusted");

    final byte[] tamperedSignature = sender.issuerSignature();
    tamperedSignature[0] = (byte) (tamperedSignature[0] ^ 0x01);
    final OfflineNote.KeyCertificate tampered =
        new OfflineNote.KeyCertificate(
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
    assertTrue(
        !verifier.verifyIssuerCertificate(tampered), "tampered certificate signature is rejected");
    assertTrue(
        !new RejectingOfflineNoteCertificateVerifier().verifyIssuerCertificate(sender),
        "default certificate verifier rejects");
    assertTrue(
        !new Ed25519OfflineNoteCertificateVerifier(
                Collections.singletonList(filledBytes(32, 0x42)))
            .verifyIssuerCertificate(sender),
        "wrong issuer root rejects");
  }

  private static void keyCertificatesRequireOneUseHardwareLimitWhenPresent() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));

    assertThrows(
        () ->
            new OfflineNote.KeyCertificatePayload(
                sender.version(),
                sender.platform(),
                sender.keyId(),
                sender.deviceId(),
                sender.accountId(),
                sender.publicKey(),
                sender.assertionScheme(),
                sender.assertionKeyAlgorithm(),
                sender.assertionPublicKey(),
                Integer.valueOf(2),
                true),
        "certificate payload must reject multi-use hardware limit");
    assertThrows(
        () ->
            new OfflineNote.KeyCertificate(
                sender.version(),
                sender.platform(),
                sender.keyId(),
                sender.deviceId(),
                sender.accountId(),
                sender.publicKey(),
                sender.assertionScheme(),
                sender.assertionKeyAlgorithm(),
                sender.assertionPublicKey(),
                Integer.valueOf(0),
                true,
                sender.issuerSignature()),
        "certificate must reject zero hardware usage limit");
    new OfflineNote.KeyCertificate(
        sender.version(),
        sender.platform(),
        sender.keyId(),
        sender.deviceId(),
        sender.accountId(),
        sender.publicKey(),
        sender.assertionScheme(),
        sender.assertionKeyAlgorithm(),
        sender.assertionPublicKey(),
        Integer.valueOf(1),
        true,
        sender.issuerSignature());
  }

  private static void offlineNoteModelsMatchRustNoritoVectors() throws Exception {
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
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final byte[] senderPayloadBytes = base64Bytes(string(certificates, "sender_payload_base64"));
    final byte[] issueBytes = base64Bytes(string(issueVector, "norito_base64"));
    final byte[] auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"));
    final byte[] redeemBytes = base64Bytes(string(redeemVector, "norito_base64"));

    assertEquals(
        base64(senderPayloadBytes),
        base64(OfflineNote.decodeCertificatePayload(senderPayloadBytes).noritoEncoded()),
        "decoded certificate payload");
    assertEquals(
        base64(senderCertificate.noritoEncoded()),
        base64(OfflineNote.decodeCertificate(senderCertificate.noritoEncoded()).noritoEncoded()),
        "decoded certificate");
    assertEquals(
        base64(issueBytes),
        base64(OfflineNote.decodeIssue(issueBytes).noritoEncoded()),
        "decoded issue");

    final OfflineNote.AuditBundle decodedAudit = OfflineNote.decodeAudit(auditBytes);
    assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()), "decoded audit");
    assertEquals(
        base64(decodedAudit.inputClaims().get(0).noritoEncoded()),
        base64(
            OfflineNote.decodeIssuedClaim(decodedAudit.inputClaims().get(0).noritoEncoded())
                .noritoEncoded()),
        "decoded issued claim");
    assertEquals(
        base64(decodedAudit.publicInputs().noritoEncoded()),
        base64(
            OfflineNote.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded())
                .noritoEncoded()),
        "decoded audit public inputs");

    final OfflineNote.Redeem decodedRedeem = OfflineNote.decodeRedeem(redeemBytes);
    assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()), "decoded redeem");
    assertEquals(
        base64(decodedRedeem.publicInputs().noritoEncoded()),
        base64(
            OfflineNote.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded())
                .noritoEncoded()),
        "decoded redeem public inputs");

    final OfflineNote.NoteCommitmentPreimage commitmentPreimage =
        new OfflineNote.NoteCommitmentPreimage(
            string(derivation, "chain_id"),
            hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            string(issueVector, "asset_id"),
            string(redeemVector, "amount"),
            hexBytes(string(derivation, "source_note_secret_hex")),
            new OfflineNote.CommitmentOrigin.IssuerLoad(
                string(derivation, "issuer_load_operation_id"),
                string(derivation, "issuer_load_lineage_id"),
                longValue(derivation, "issuer_load_local_revision")));
    assertEquals(
        base64(commitmentPreimage.noritoEncoded()),
        base64(
            OfflineNote.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded note commitment preimage");

    final OfflineNote.InputNullifierPreimage nullifierPreimage =
        new OfflineNote.InputNullifierPreimage(
            string(derivation, "chain_id"),
            hexBytes(string(derivation, "source_note_commitment")),
            hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            hexBytes(string(derivation, "source_note_secret_hex")));
    assertEquals(
        base64(nullifierPreimage.noritoEncoded()),
        base64(
            OfflineNote.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded input nullifier preimage");

    final OfflineNote.PaymentTokenIdPreimage tokenPreimage =
        new OfflineNote.PaymentTokenIdPreimage(
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
            OfflineNote.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded())
                .noritoEncoded()),
        "decoded payment token id preimage");
  }

  private static void publicNoritoInstructionDecodersReadExplorerEnvelopeBytes() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.Issue issue = issue(fixture);
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);

    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            OfflineNote.decodeIssueInstruction(
                    rawInstructionPair(
                        OfflineNote.ISSUE_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.issueInstruction(issue))))
                .noritoEncoded()),
        "decoded issue instruction");
    assertEquals(
        base64(audit.noritoEncoded()),
        base64(
            OfflineNote.decodeAuditInstruction(
                    rawInstructionPair(
                        OfflineNote.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.auditInstruction(audit))))
                .noritoEncoded()),
        "decoded audit instruction");
    assertEquals(
        base64(redeem.noritoEncoded()),
        base64(
            OfflineNote.decodeRedeemInstruction(
                    rawInstructionPair(
                        OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.redeemInstruction(redeem))))
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
        OfflineNote.deriveNoteCommitment(
            new OfflineNote.NoteCommitmentPreimage(
                chainId,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                string(issueVector, "asset_id"),
                string(issueVector, "amount"),
                hexBytes(string(derivation, "source_note_secret_hex")),
                new OfflineNote.CommitmentOrigin.IssuerLoad(
                    string(derivation, "issuer_load_operation_id"),
                    string(derivation, "issuer_load_lineage_id"),
                    longValue(derivation, "issuer_load_local_revision"))));
    assertEquals(
        string(derivation, "source_note_commitment"),
        hex(sourceCommitment),
        "source note commitment");

    final byte[] inputNullifier =
        OfflineNote.deriveInputNullifier(
            new OfflineNote.InputNullifierPreimage(
                chainId,
                sourceCommitment,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                hexBytes(string(derivation, "source_note_secret_hex"))));
    assertEquals(string(derivation, "input_nullifier"), hex(inputNullifier), "input nullifier");

    final byte[] recipientCommitment =
        OfflineNote.deriveNoteCommitment(
            new OfflineNote.NoteCommitmentPreimage(
                chainId,
                hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                string(recipientOutput, "asset_definition_id")
                    + "#"
                    + string(recipientOutput, "account_id"),
                string(recipientOutput, "amount"),
                hexBytes(string(derivation, "recipient_note_secret_hex")),
                new OfflineNote.CommitmentOrigin.P2pOutput(
                    string(derivation, "payment_request_id"), 0)));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        hex(recipientCommitment),
        "recipient output commitment");

    final byte[] changeCommitment =
        OfflineNote.deriveNoteCommitment(
            new OfflineNote.NoteCommitmentPreimage(
                chainId,
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                string(changeOutput, "asset_definition_id")
                    + "#"
                    + string(changeOutput, "account_id"),
                string(changeOutput, "amount"),
                hexBytes(string(derivation, "change_note_secret_hex")),
                new OfflineNote.CommitmentOrigin.P2pOutput(
                    string(derivation, "payment_request_id"), 1)));
    assertEquals(
        string(derivation, "change_output_commitment"),
        hex(changeCommitment),
        "change output commitment");

    final byte[] tokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                chainId,
                string(derivation, "payment_request_id"),
                longValue(payment, "created_at_ms"),
                hexBytes(string(derivation, "token_nonce_hex")),
                hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                Collections.singletonList(inputNullifier),
                Arrays.asList(recipientCommitment, changeCommitment)));
    assertEquals(string(derivation, "payment_token_id"), hex(tokenId), "payment token id");

    final byte[] redeemNullifier =
        OfflineNote.deriveInputNullifier(
            new OfflineNote.InputNullifierPreimage(
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
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);

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
    final OfflineNote.Redeem redeem = redeem(fixture);
    final OfflineNote.RecursiveProof badProof =
        new OfflineNote.RecursiveProof(
            OfflineNote.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new OfflineNote.ProofBox(
                OfflineNote.RECURSIVE_BACKEND,
                "offline-vector-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    final OfflineNote.Redeem forged =
        new OfflineNote.Redeem(
            redeem.sourceNoteCommitment(),
            redeem.inputNullifiers(),
            redeem.senderKeyCertificate(),
            redeem.recipient(),
            redeem.assetId(),
            redeem.amount(),
            badProof);

    assertThrows(forged::validateProofBinding, "proof binding mismatch should throw");
  }

  private static void proofBindingRejectsRecursiveMetadataSubstitution() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.RecursiveProof wrongVerifier =
        new OfflineNote.RecursiveProof(
            new OfflineNote.VerifyingKeyIdReference(
                "halo2/kzg", OfflineNote.RECURSIVE_VERIFIER_NAME),
            audit.publicInputsHash(),
            audit.recursiveProof().proof());
    assertThrows(
        () -> audit.replacingRecursiveProof(wrongVerifier).validateProofBinding(),
        "proof binding should reject verifier-key metadata substitution");

    final OfflineNote.Redeem redeem = redeem(fixture);
    final OfflineNote.RecursiveProof wrongProofBackend =
        new OfflineNote.RecursiveProof(
            redeem.publicInputsHash(),
            new OfflineNote.ProofBox(
                "groth16",
                redeem.recursiveProof().proof().bytes()));
    assertThrows(
        () -> redeem.replacingRecursiveProof(wrongProofBackend).validateProofBinding(),
        "proof binding should reject proof-backend metadata substitution");

    final OfflineNote.RecursiveProof draftPlaceholder =
        new OfflineNote.RecursiveProof(
            redeem.publicInputsHash(),
            new OfflineNote.ProofBox(
                "offline-note/draft-placeholder",
                new byte[] {0}));
    assertThrows(
        () -> redeem.replacingRecursiveProof(draftPlaceholder).validateProofBinding(),
        "proof binding should reject draft-placeholder proofs");

    for (final String backend :
        new String[] {"halo2/ipa:KZG", "halo2/ipa: KZG", "halo2/ipa:Mock-Proof"}) {
      final OfflineNote.RecursiveProof nonProductionBackend =
          new OfflineNote.RecursiveProof(
              redeem.publicInputsHash(),
              new OfflineNote.ProofBox(
                  backend,
                  redeem.recursiveProof().proof().bytes()));
      assertThrows(
          () -> redeem.replacingRecursiveProof(nonProductionBackend).validateProofBinding(),
          "proof binding should reject non-production backend " + backend);
    }
  }

  private static void recursiveProofMetadataNormalizesAndRejectsMalformedVerifierKeys() {
    final OfflineNote.VerifyingKeyIdReference verifier =
        new OfflineNote.VerifyingKeyIdReference(
            "  " + OfflineNote.RECURSIVE_BACKEND + "  ",
            "  " + OfflineNote.RECURSIVE_VERIFIER_NAME + "  ");
    assertEquals(
        OfflineNote.RECURSIVE_BACKEND,
        verifier.backend(),
        "verifier backend should be trimmed");
    assertEquals(
        OfflineNote.RECURSIVE_VERIFIER_NAME,
        verifier.name(),
        "verifier name should be trimmed");

    final OfflineNote.ProofBox proof =
        new OfflineNote.ProofBox(
            "  " + OfflineNote.RECURSIVE_BACKEND + "  ",
            new byte[] {0x01});
    assertEquals(
        OfflineNote.RECURSIVE_BACKEND,
        proof.backend(),
        "proof backend should be trimmed");

    assertThrows(
        () ->
            new OfflineNote.VerifyingKeyIdReference(
                "halo2/ipa:KZG", OfflineNote.RECURSIVE_VERIFIER_NAME),
        "verifier backend must reject colon separators");
    assertThrows(
        () ->
            new OfflineNote.VerifyingKeyIdReference(
                OfflineNote.RECURSIVE_BACKEND, "offline:note"),
        "verifier name must reject colon separators");
  }

  private static void instanceValuesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final OfflineNote.InstanceValues auditValues =
        OfflineNote.InstanceBuilder.auditInstanceValues(audit(fixture));
    final OfflineNote.InstanceValues redeemValues =
        OfflineNote.InstanceBuilder.redeemInstanceValues(redeem(fixture));
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
        hex(OfflineNote.instanceScalarBytes(auditPublic[0])),
        hex(auditValues.publicInstanceColumns().get(0)),
        "audit first instance scalar");
  }

  private static void auditInstanceValuesRejectUnanchoredClaimsAndHiddenOutputs() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.AuditBundle audit = audit(fixture);
    final List<byte[]> hiddenOutputCommitments = new ArrayList<>(audit.outputCommitments());
    hiddenOutputCommitments.add(flippedHash(audit.outputCommitments().get(0)));

    assertThrows(
        () ->
            new OfflineNote.AuditBundle(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                hiddenOutputCommitments,
                audit.outputClaims(),
                audit.recursiveProof()),
        "hidden audit output commitments must be rejected");

    assertTrue(audit.outputCommitments().size() > 1, "fixture has multiple audit outputs");
    final List<byte[]> reversedOutputCommitments = new ArrayList<>(audit.outputCommitments());
    Collections.reverse(reversedOutputCommitments);
    assertThrows(
        () ->
            new OfflineNote.AuditBundle(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                reversedOutputCommitments,
                audit.outputClaims(),
                audit.recursiveProof()),
        "reordered audit output commitments must be rejected");

    final OfflineNote.IssuedClaim claim = audit.inputClaims().get(0);
    final OfflineNote.IssuedClaim forgedClaim =
        new OfflineNote.IssuedClaim(
            claim.domain(),
            claim.noteCommitment(),
            flippedHash(claim.keyCertificatePayloadHash()),
            claim.assetId(),
            claim.amount());
    final OfflineNote.AuditBundle forgedAudit =
        new OfflineNote.AuditBundle(
            audit.tokenId(),
            audit.senderKeyCertificate(),
            audit.inputNullifiers(),
            Collections.singletonList(forgedClaim),
            audit.outputCommitments(),
            audit.outputClaims(),
            audit.recursiveProof());

    assertThrows(
        () -> OfflineNote.InstanceBuilder.auditInstanceValues(forgedAudit),
        "audit input claims must be anchored to sender certificate");
  }

  private static void nativeHalo2ProverProducesVerifyingPayloadWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_OFFLINE_PROVER_TEST"))) {
      return;
    }
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.InstanceValues values =
        OfflineNote.InstanceBuilder.auditInstanceValues(audit);
    OfflineNoteHalo2Prover.prewarm();
    final byte[] payload = OfflineNoteHalo2Prover.proveZk1Payload(values);

    assertTrue(
        OfflineNoteHalo2Prover.verifyZk1Payload(payload, values.publicValues()),
        "Java Offline Halo2 payload verifies");
    final OfflineNote.RecursiveProof proof = OfflineNoteHalo2Prover.proveAudit(audit);
    audit.replacingRecursiveProof(proof).validateProofBinding();
    assertTrue(
        proof.proof().bytes().length <= OfflineNoteHalo2Prover.MAX_ENVELOPE_BYTES,
        "Java Offline Halo2 envelope fits QR budget");
    final byte[] envelope = OfflineNoteHalo2Prover.proveOpenVerifyEnvelope(values);
    assertTrue(envelope.length > 0, "Java Offline Halo2 SDK envelope helper returns bytes");
    assertTrue(
        OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
            proof.proof().bytes(), values.publicValues()),
        "Java Offline Halo2 SDK envelope helper verifies public values");
    assertTrue(
        OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
            proof.proof().bytes(), hex(proof.publicInputsHash())),
        "Java Offline Halo2 SDK envelope helper verifies public input hash");
    assertTrue(
        !OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
            proof.proof().bytes(), "0000000000000000000000000000000000000000000000000000000000000000"),
        "Java Offline Halo2 SDK envelope helper rejects a wrong public input hash");
  }

  private static void kagemushaRecordBackedNativeProverValidatesInput() {
    final byte[] oversizedArchive =
        new byte[KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1];
    assertIllegalArgumentContains(
        () -> KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(
            new byte[0]),
        "recordBundleArchive must not be empty");
    assertIllegalArgumentContains(
        () -> KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(
            oversizedArchive),
        "recordBundleArchive must not exceed");
    assertIllegalArgumentContains(
        () ->
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(
                new byte[] {0x01, 0x02}),
        "recordBundleArchive must be a valid Norito archive");
    assertIllegalArgumentContains(
        () ->
            KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords(
                kagemushaNoritoFrame(0x4b)),
        "recordBundleArchive must contain a non-empty Norito payload");
  }

  private static void kagemushaCompactNativeInputCopiesBeforeDispatch() {
    final byte[] archive = kagemushaNoritoFrameWithPayload(0x4c);
    final byte[] expected = Arrays.copyOf(archive, archive.length);
    final byte[] ownedArchive =
        KagemushaCompactPaymentTokenProver.ownedNativeInput(archive, "recordBundleArchive");

    archive[6] = (byte) 0x7F;

    assertTrue(ownedArchive != archive, "Kagemusha compact native input must be copied");
    assertTrue(
        Arrays.equals(expected, ownedArchive),
        "Kagemusha compact native input copy must preserve original bytes");
  }

  private static void kagemushaRecursiveAggregationNativeProverValidatesInput() {
    final byte[] validArchive = kagemushaNoritoFrameWithPayload(0x4b);
    final byte[] oversizedArchive =
        new byte[KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1];
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    new byte[0], validArchive),
        "recordBundleArchive must not be empty");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive, new byte[0]),
        "pallasOpenEnvelopesArchive must not be empty");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive, validArchive),
        "recordBundleArchive must not exceed");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive, oversizedArchive),
        "pallasOpenEnvelopesArchive must not exceed");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    new byte[] {0x01, 0x02}, validArchive),
        "recordBundleArchive must be a valid Norito archive");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive, new byte[] {0x01, 0x02}),
        "pallasOpenEnvelopesArchive must be a valid Norito archive");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    kagemushaNoritoFrame(0x4b), validArchive),
        "recordBundleArchive must contain a non-empty Norito payload");
    assertIllegalArgumentContains(
        () ->
            KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive, kagemushaNoritoFrame(0x4b)),
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload");
  }

  private static void kagemushaRecursiveSpendNativeProverValidatesInput() {
    assertTrue(
        KagemushaRecursiveSpendProver.preferredMode(true, true)
            == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        "recursive spend should remain the production default when compact ABI is present");
    assertTrue(
        KagemushaRecursiveSpendProver.preferredMode(true)
            == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        "recursive Kagemusha spend should be preferred when available");
    assertTrue(
        KagemushaRecursiveSpendProver.preferredMode(false)
            == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1,
        "checked prefold should remain the compatibility fallback");
    assertEquals(
        "checked_prefold_v1",
        KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName(),
        "checked prefold Kagemusha wire mode");
    assertEquals(
        "recursive_compact_v1",
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_COMPACT_V1.wireName(),
        "recursive compact Kagemusha wire mode");
    assertTrue(
        VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", new byte[] {1, 2, 3}).length > 0,
        "verifying key box codec should encode non-empty records");
    assertThrows(
        () -> VerifyingKeyBoxCodec.encodeNorito(" ", new byte[] {1}),
        "blank verifying key backend should fail");
    assertThrows(
        () -> VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", new byte[0]),
        "empty verifying key bytes should fail");
    assertEquals(
        "recursive_spend_v1",
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName(),
        "recursive Kagemusha spend wire mode");
    assertEquals(
        6,
        KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION,
        "recursive Kagemusha spend requires bridge ABI 6");
    assertEquals(
        "kagemusha-recursive-aggregation-v1",
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        "semantic recursive aggregation proof circuit id");
    assertEquals(
        "kagemusha-recursive-spend-lineage-v1",
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        "reserved recursive spend lineage proof circuit id");
    assertTrue(
        KagemushaRecursiveSpendProver.detectNativeAvailability(
            () -> {},
            () -> 6,
            () ->
                KagemushaRecursiveSpendProver.expectIllegalArgumentProbe(
                    () -> {
                      throw new IllegalArgumentException("empty archive probe");
                    })),
        "recursive Kagemusha spend accepts ABI 6 with expected empty-probe rejection");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(() -> {}, () -> 6, () -> false),
        "recursive Kagemusha spend availability rejects probes that accept empty archives");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(() -> {}, () -> 5, () -> true),
        "recursive Kagemusha spend must reject native bridges older than ABI 6");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new IllegalArgumentException("broken ABI probe");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "recursive Kagemusha spend availability must fail closed when ABI probing is malformed");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(
            () -> {
              throw new UnsatisfiedLinkError("missing library");
            },
            () -> 6,
            () -> true),
        "recursive Kagemusha spend availability must fail closed when JNI is missing");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(
            () -> {},
            () -> 6,
            () -> {
              throw new UnsatisfiedLinkError("missing recursive spend symbol");
            }),
        "recursive Kagemusha spend availability must fail closed when a required JNI symbol is missing");
    assertTrue(
        !KagemushaRecursiveSpendProver.detectNativeAvailability(
            () -> {},
            () -> 6,
            () -> {
              throw new SecurityException("native bridge denied");
            }),
        "recursive Kagemusha spend availability must fail closed when symbol probing is denied");

    assertThrows(
        () -> KagemushaRecursiveSpendProver.initSpend(new byte[0]),
        "Kagemusha recursive spend init must reject empty archives before JNI");
    assertThrows(
        () -> KagemushaRecursiveSpendProver.appendSpend(new byte[0]),
        "Kagemusha recursive spend append must reject empty archives before JNI");
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                new byte[0], new byte[] {0x01}),
        "Kagemusha recursive spend init witness helper must reject empty request archives before JNI");
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                new byte[] {0x01}, new byte[0]),
        "Kagemusha recursive spend init witness helper must reject empty bundle archives before JNI");
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[0], new byte[] {0x01}, new byte[] {0x02}),
        "Kagemusha recursive spend append witness helper must reject empty witness archives before JNI");
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[] {0x01}, new byte[0], new byte[] {0x02}),
        "Kagemusha recursive spend append witness helper must reject empty request archives before JNI");
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[] {0x01}, new byte[] {0x02}, new byte[0]),
        "Kagemusha recursive spend append witness helper must reject empty bundle archives before JNI");
    assertThrows(
        () -> KagemushaRecursiveSpendProver.verifySpend(new byte[0]),
        "Kagemusha recursive spend verify must reject empty archives before JNI");
    assertThrows(
        () -> KagemushaRecursiveSpendProver.redeemSpend(new byte[0]),
        "Kagemusha recursive spend redeem must reject empty archives before JNI");
    if (KagemushaRecursiveSpendProver.isNativeAvailable()) {
      assertThrows(
          () -> KagemushaRecursiveSpendProver.initSpend(new byte[] {0x01, 0x02}),
          "Kagemusha recursive spend init must reject malformed archives");
      assertThrows(
          () -> KagemushaRecursiveSpendProver.appendSpend(new byte[] {0x01, 0x02}),
          "Kagemusha recursive spend append must reject malformed archives");
      assertThrows(
          () ->
              KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                  new byte[] {0x01, 0x02}, new byte[] {0x03, 0x04}),
          "Kagemusha recursive spend init witness helper must reject malformed archives");
      assertThrows(
          () ->
              KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                  new byte[] {0x01, 0x02}, new byte[] {0x03, 0x04}, new byte[] {0x05, 0x06}),
          "Kagemusha recursive spend append witness helper must reject malformed archives");
      assertThrows(
          () -> KagemushaRecursiveSpendProver.verifySpend(new byte[] {0x01, 0x02}),
          "Kagemusha recursive spend verify must reject malformed archives");
      assertThrows(
          () -> KagemushaRecursiveSpendProver.redeemSpend(new byte[] {0x01, 0x02}),
          "Kagemusha recursive spend redeem must reject malformed archives");
    }
  }

  private static void chainVkOfflineNoteProofWrappersValidateInputs() {
    assertThrows(
        () -> new ChainVkOfflineNoteProofProvider(null),
        "chain VK proof provider must reject null verifier key boxes");
    assertThrows(
        () -> new ChainVkOfflineNoteProofProvider(new byte[0]),
        "chain VK proof provider must reject empty verifier key boxes");
    assertThrows(
        () -> new ChainVkOfflineNoteProofVerifier(null),
        "chain VK proof verifier must reject null verifier key boxes");
    assertThrows(
        () -> new ChainVkOfflineNoteProofVerifier(new byte[0]),
        "chain VK proof verifier must reject empty verifier key boxes");
    new ChainVkOfflineNoteProofProvider(new byte[] {0x01});
    new ChainVkOfflineNoteProofVerifier(new byte[] {0x01});

    assertTrue(
        NativeOfflineNoteProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new IllegalArgumentException("empty native probe");
            }),
        "record-backed Offline Note prover accepts expected empty-probe rejection");
    assertTrue(
        !NativeOfflineNoteProver.detectNativeAvailability(
            () -> {
              throw new UnsatisfiedLinkError("missing library");
            },
            () -> {}),
        "record-backed Offline Note prover fails closed when JNI is missing");
    assertTrue(
        !NativeOfflineNoteProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new UnsatisfiedLinkError("missing symbol");
            }),
        "record-backed Offline Note prover fails closed when required JNI symbols are missing");
    assertTrue(
        !NativeOfflineNoteProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new SecurityException("native bridge denied");
            }),
        "record-backed Offline Note prover fails closed when symbol probing is denied");

    assertThrows(
        () -> NativeOfflineNoteProver.proveRedeem(null, new byte[] {0x01}),
        "record-backed redeem prover must reject null redemption payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.proveRedeem(new byte[0], new byte[] {0x01}),
        "record-backed redeem prover must reject empty redemption payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.proveRedeem(new byte[] {0x01}, null),
        "record-backed redeem prover must reject null verifier key boxes before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.proveRedeem(new byte[] {0x01}, new byte[0]),
        "record-backed redeem prover must reject empty verifier key boxes before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.proveAudit(null, new byte[] {0x01}),
        "record-backed audit prover must reject null audit payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.proveAudit(new byte[0], new byte[] {0x01}),
        "record-backed audit prover must reject empty audit payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.verifyRedeem(null, new byte[] {0x01}),
        "record-backed redeem verifier must reject null redemption payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.verifyRedeem(new byte[] {0x01}, new byte[0]),
        "record-backed redeem verifier must reject empty verifier key boxes before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.verifyAudit(null, new byte[] {0x01}),
        "record-backed audit verifier must reject null audit payloads before JNI");
    assertThrows(
        () -> NativeOfflineNoteProver.verifyAudit(new byte[] {0x01}, new byte[0]),
        "record-backed audit verifier must reject empty verifier key boxes before JNI");
  }

  private static void kagemushaNativeProversRejectMissingAndEmptyNativeOutputs() {
    try {
      KagemushaCompactPaymentTokenProver.requireNativeOutput(null, "native test");
      throw new AssertionError("native output guard must reject null output");
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains("returned no output"),
          "native output guard reports missing output");
    }

    try {
      KagemushaCompactPaymentTokenProver.requireNativeOutput(new byte[0], "native test");
      throw new AssertionError("native output guard must reject empty output");
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains("returned empty output"),
          "native output guard reports empty output");
    }

    try {
      KagemushaCompactPaymentTokenProver.requireNativeOutput(
          new byte[KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1],
          "native test");
      throw new AssertionError("native output guard must reject oversized output");
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains("returned oversized output"),
          "native output guard reports oversized output");
    }

    try {
      KagemushaCompactPaymentTokenProver.requireNativeOutput(
          new byte[] {0x01, 0x02}, "native test");
      throw new AssertionError("native output guard must reject malformed Norito output");
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains("returned invalid Norito archive"),
          "native output guard reports malformed Norito output");
    }

    try {
      KagemushaCompactPaymentTokenProver.requireNativeOutput(
          kagemushaNoritoFrame(0x4b), "native test");
      throw new AssertionError("native output guard must reject empty Norito payloads");
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains("returned empty Norito payload"),
          "native output guard reports empty Norito payload");
    }

    final byte[] output = kagemushaNoritoFrameWithPayload(0x4b);
    assertTrue(
        Arrays.equals(
            output, KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native test")),
        "native output guard returns valid Norito proof archives unchanged");
  }

  private static void kagemushaNativeAvailabilityRequiresJniEntrypoint() {
    assertTrue(
        KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {},
            () -> {
              return KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () -> {
                    throw new IllegalArgumentException("invalid archive");
                  });
            }),
        "native availability accepts the malformed-input error from a present JNI symbol");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(() -> {}, () -> false),
        "native availability rejects probes that accept empty archives");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new UnsatisfiedLinkError("missing symbol");
            }),
        "native availability rejects a loaded library without the Kagemusha JNI symbol");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {
              throw new UnsatisfiedLinkError("missing library");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "native availability rejects a missing library before probing symbols");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {
              throw new IllegalArgumentException("bad library name");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "native availability rejects malformed library loading before probing symbols");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {
              throw new SecurityException("denied");
            },
            () -> true),
        "native availability rejects a library blocked by the security manager");
  }

  private static void kagemushaRecursiveAggregationNativeAvailabilityRequiresJniEntrypoint() {
    assertTrue(
        KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {},
            () -> {
              return KagemushaRecursiveAggregationProofBundleProver.expectIllegalArgumentProbe(
                  () -> {
                    throw new IllegalArgumentException("invalid archive");
                  });
            }),
        "recursive aggregation availability accepts the malformed-input error from a present JNI symbol");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {}, () -> false),
        "recursive aggregation availability rejects probes that accept empty archives");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new UnsatisfiedLinkError("missing symbol");
            }),
        "recursive aggregation availability rejects a loaded library without the JNI symbol");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {
              throw new UnsatisfiedLinkError("missing library");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "recursive aggregation availability rejects a missing library before probing symbols");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {
              throw new IllegalArgumentException("bad library name");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "recursive aggregation availability rejects malformed library loading before probing symbols");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {
              throw new SecurityException("denied");
            },
            () -> true),
        "recursive aggregation availability rejects a library blocked by the security manager");
  }

  private static void nativeHalo2ProverPerformanceWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_OFFLINE_BENCH"))) {
      return;
    }
    final String configuredIterations = System.getenv("IROHA_JAVA_OFFLINE_BENCH_ITERATIONS");
    final int iterations =
        configuredIterations == null ? 20 : Integer.parseInt(configuredIterations);
    assertTrue(iterations > 0, "Java Offline benchmark iterations must be positive");

    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);
    OfflineNoteHalo2Prover.prewarm();
    OfflineNoteHalo2Prover.proveAudit(audit);
    OfflineNoteHalo2Prover.proveRedeem(redeem);

    final double[] auditSeconds =
        benchmarkSeconds(iterations, () -> OfflineNoteHalo2Prover.proveAudit(audit));
    final double[] redeemSeconds =
        benchmarkSeconds(iterations, () -> OfflineNoteHalo2Prover.proveRedeem(redeem));
    System.out.println(
        "offline_note_java_bench audit="
            + summary(auditSeconds)
            + " redeem="
            + summary(redeemSeconds));
  }

  private static void qrFixtureUsesSdkTextPrefix() throws Exception {
    final Map<String, Object> fountain = obj(loadFixture(), "fountain_qr");
    assertEquals("iroha:qr:", string(fountain, "frame_prefix"), "fountain QR prefix");
  }

  private static void paymentTokenCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final OfflineNotePaymentToken token =
        new OfflineNotePaymentToken(
            string(derivation, "chain_id"),
            string(payment, "invoice_id"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(payment, "token_id")),
            audit(fixture),
            longValue(payment, "created_at_ms"));
    final byte[] canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"));
    assertTrue(
        Arrays.equals(canonicalPayload, OfflineNotePaymentTokenCodec.encodeNorito(token)),
        "canonical payment token Norito");

    final OfflineNotePaymentToken noritoDecoded =
        OfflineNotePaymentTokenCodec.decodeNorito(OfflineNotePaymentTokenCodec.encodeNorito(token));
    assertEquals(token.tokenIdHex(), noritoDecoded.tokenIdHex(), "norito token id");
    assertEquals(token.paymentRequestId(), noritoDecoded.paymentRequestId(), "norito payment request id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(noritoDecoded.audit().noritoEncoded()),
        "norito audit");
    final OfflineNotePaymentToken canonicalDecoded =
        OfflineNotePaymentTokenCodec.decodeNorito(canonicalPayload);
    assertEquals(token.tokenIdHex(), canonicalDecoded.tokenIdHex(), "canonical token id");
    assertEquals(
        base64(token.audit().noritoEncoded()),
        base64(canonicalDecoded.audit().noritoEncoded()),
        "canonical audit");

    final String text = OfflineNotePaymentTokenCodec.encodeText(token);
    assertEquals(string(sdkInterop, "payment_token_text"), text, "canonical payment token text");
    assertTrue(
        text.startsWith(OfflineNotePaymentTokenCodec.TEXT_PREFIX),
        "payment token text prefix");
    assertEquals(
        OfflineBearerCashTextCodec.PAYMENT_TEXT_PREFIX,
        OfflineNotePaymentTokenCodec.TEXT_PREFIX,
        "Bearer Cash payment prefix");
    assertEquals(
        token.tokenIdHex(),
        OfflineNotePaymentTokenCodec.decodeText(text).tokenIdHex(),
        "text token id");
    assertEquals(
        token.tokenIdHex(),
        OfflineBearerCashTextCodec.decodePaymentText(text).tokenIdHex(),
        "Bearer Cash text token id");
    assertEquals(
        token.tokenIdHex(),
        OfflineNotePaymentTokenCodec.decodeText(string(sdkInterop, "payment_token_text"))
            .tokenIdHex(),
        "canonical text token id");
    assertThrows(
        () ->
            OfflineNotePaymentTokenCodec.decodeText(
                "wallet-offline-bearer-cash-payment-invalid:"
                    + text.substring(text.indexOf(':') + 1)),
        "unknown payment prefix should reject");
    assertThrows(
        () -> OfflineNotePaymentTokenCodec.decodeText(text + "="),
        "padded payment token text should reject");

    final List<byte[]> frames =
        OfflineNotePaymentTokenCodec.encodeQrFrameBytes(
            token, new OfflineQrStream.Options(180, 2));
    final List<Object> expectedFrameObjects = list(obj(sdkInterop, "payment_token_qr"), "frames");
    final List<String> expectedFrames = new ArrayList<>();
    for (final Object frameObject : expectedFrameObjects) {
      expectedFrames.add(string(asMap(frameObject, "payment_token_qr.frames[]"), "bytes_hex"));
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
    final OfflineNotePaymentToken qrDecoded =
        OfflineNotePaymentTokenCodec.decodeQrPayload(payload);
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
        OfflineNotePaymentTokenCodec.decodeQrPayload(canonicalQrPayload).tokenIdHex(),
        "canonical QR token id");
  }

  private static void offlineBearerCashPolicyAndPrefixesUseSingleAppSurface() throws Exception {
    final OfflineBearerCashPolicyV1 policy = OfflineBearerCashPolicyV1.DEFAULT;
    assertEquals(5, policy.maxCustodyHops(), "max custody hops");
    assertEquals(32, policy.maxLineageSteps(), "max lineage steps");
    assertEquals(2048, policy.maxSingleQrPayloadBytes(), "static QR max");
    assertEquals(12288, policy.maxStreamPayloadBytes(), "stream max");
    assertEquals(20, policy.androidKeyPoolTarget(), "android key pool target");
    assertEquals(8, policy.androidKeyPoolReplenishBelow(), "android key pool replenish");
    assertEquals(40, policy.androidKeyPoolCap(), "android key pool cap");
    assertTrue(
        OfflineBearerCashTransport.STATIC_QR
            == policy.recommendedTransportForPayloadByteCount(2048),
        "static QR threshold");
    assertTrue(
        OfflineBearerCashTransport.STREAMING_QR
            == policy.recommendedTransportForPayloadByteCount(2049),
        "streaming QR threshold");
    assertTrue(
        OfflineBearerCashTransport.FRAMED_BYTE_TRANSPORT
            == policy.recommendedTransportForPayloadByteCount(12289),
        "framed byte threshold");
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.AuditBundle ancestor = ancestorAuditProducingFirstInput(audit, 0xB0);
    final OfflineBearerCashPolicyV1.AuditTrailMetrics metrics =
        policy.auditTrailMetrics(Arrays.asList(ancestor, audit), audit);
    assertEquals(2L, metrics.custodyHops(), "custody hop metric");
    assertEquals(2L, metrics.lineageSteps(), "lineage step metric");
    assertThrows(
        () ->
            new OfflineBearerCashPolicyV1(1, 32, 2048, 12288, 20, 8, 40)
                .validateAuditTrail(Arrays.asList(ancestor, audit), audit),
        "custody limit should reject over-depth audit trails");
    assertThrows(
        () ->
            new OfflineBearerCashPolicyV1(5, 1, 2048, 12288, 20, 8, 40)
                .validateAuditTrail(Arrays.asList(ancestor, audit), audit),
        "lineage limit should reject overlong audit trails");
    assertEquals(
        "wallet-offline-bearer-cash-receive:",
        OfflineNoteReceiveRequestCodec.TEXT_PREFIX,
        "receive prefix");
    assertEquals(
        "wallet-offline-bearer-cash-payment:",
        OfflineNotePaymentTokenCodec.TEXT_PREFIX,
        "payment prefix");
    assertEquals(
        "wallet-offline-bearer-cash-ack:",
        OfflineNoteReceiptAckCodec.TEXT_PREFIX,
        "ack prefix");
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind("wallet-offline-bearer-cash-unknown:AAAA")
            == null,
        "unknown prefix");
  }

  private static void receiveRequestCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteReceiveRequest request = receiveRequestFixture(fixture);

    final byte[] norito = OfflineNoteReceiveRequestCodec.encodeNorito(request);
    final OfflineNoteReceiveRequest noritoDecoded =
        OfflineNoteReceiveRequestCodec.decodeNorito(norito);
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

    final String text = OfflineNoteReceiveRequestCodec.encodeText(request);
    assertTrue(
        text.startsWith(OfflineNoteReceiveRequestCodec.TEXT_PREFIX),
        "receive request text prefix");
    assertEquals(
        request.outputCommitmentHex(),
        OfflineNoteReceiveRequestCodec.decodeText(text).outputCommitmentHex(),
        "receive request text output commitment");
    assertThrows(
        () -> OfflineNoteReceiveRequestCodec.decodeText(text + "="),
        "padded receive request text should reject");

    final List<byte[]> frames =
        OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(
            request, new OfflineQrStream.Options(180, 2));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    byte[] payload = null;
    for (final byte[] frame : frames) {
      final OfflineQrStream.DecodeResult result = decoder.ingest(frame);
      assertTrue(
          OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST == result.payloadKind(),
          "receive request QR kind");
      if (result.payload() != null) {
        payload = result.payload();
      }
    }
    assertTrue(payload != null, "receive request QR payload");
    assertEquals(
        request.outputCommitmentHex(),
        OfflineNoteReceiveRequestCodec.decodeQrPayload(payload).outputCommitmentHex(),
        "receive request QR output commitment");
  }

  private static void receiptAckCodecRoundTripsNoritoTextAndQrFrames() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteReceiptAck ack =
        OfflineNoteReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));

    final byte[] norito = OfflineNoteReceiptAckCodec.encodeNorito(ack);
    final OfflineNoteReceiptAck noritoDecoded =
        OfflineNoteReceiptAckCodec.decodeNorito(norito);
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

    final String text = OfflineNoteReceiptAckCodec.encodeText(ack);
    assertTrue(
        text.startsWith(OfflineNoteReceiptAckCodec.TEXT_PREFIX),
        "receipt ACK text prefix");
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteReceiptAckCodec.decodeText(text).tokenIdHex(),
        "receipt ACK text token id");
    assertThrows(
        () -> OfflineNoteReceiptAckCodec.decodeText(text + "="),
        "padded receipt ACK text should reject");

    final List<byte[]> frames =
        OfflineNoteReceiptAckCodec.encodeQrFrameBytes(ack, new OfflineQrStream.Options(180, 2));
    final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();
    byte[] payload = null;
    for (final byte[] frame : frames) {
      final OfflineQrStream.DecodeResult result = decoder.ingest(frame);
      assertTrue(
          OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK == result.payloadKind(),
          "receipt ACK QR kind");
      if (result.payload() != null) {
        payload = result.payload();
      }
    }
    assertTrue(payload != null, "receipt ACK QR payload");
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteReceiptAckCodec.decodeQrPayload(payload).tokenIdHex(),
        "receipt ACK QR token id");
  }

  private static void receiptAckCodecRejectsNonPositiveAcceptedAtDecode() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    assertThrows(
        () ->
            OfflineNoteReceiptAck.fromPaymentToken(
                token, string(payment, "recipient_account_id"), 0L),
        "zero receipt ACK acceptance time should fail");

    final OfflineNoteReceiptAck ack =
        OfflineNoteReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));
    final byte[] malformed = OfflineNoteReceiptAckCodec.encodeNorito(ack);
    Arrays.fill(malformed, malformed.length - Long.BYTES, malformed.length, (byte) 0);
    assertThrows(
        () -> OfflineNoteReceiptAckCodec.decodeNorito(malformed),
        "zero decoded receipt ACK acceptance time should fail");
    assertThrows(
        () ->
            OfflineNoteReceiptAckCodec.decodeText(
                OfflineNoteReceiptAckCodec.TEXT_PREFIX
                    + Base64.getUrlEncoder().withoutPadding().encodeToString(malformed)),
        "zero text receipt ACK acceptance time should fail");
  }

  private static OfflineNoteReceiveRequest receiveRequestFixture(
      final Map<String, Object> fixture) {
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new OfflineNoteReceiveRequest(
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
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN,
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

    final byte[] unknownFrameKind = finalHeader.encode();
    unknownFrameKind[2] = 0x7F;
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
                          envelope[1] = 0x7F;
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
                          writeUInt16LE(envelope, 3, 0);
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
                          writeUInt16LE(envelope, 5, 1);
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
                          writeUInt16LE(envelope, 7, 0);
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
                          envelope[0] = 0x01;
                          return envelope;
                        })),
        "unsupported envelope flags should fail");

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
                              9,
                              OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST.value());
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
    final OfflineNotePaymentToken token =
        new OfflineNotePaymentToken(
            string(derivation, "chain_id"),
            string(payment, "invoice_id"),
            hexBytes(string(derivation, "token_nonce_hex")),
            hexBytes(string(payment, "token_id")),
            audit(fixture),
            longValue(payment, "created_at_ms"));
    final OfflineNoteReceiveRequest receiveRequest = receiveRequestFixture(fixture);
    final OfflineNoteReceiptAck receiptAck =
        OfflineNoteReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));
    final byte[] canonicalPayload = base64Bytes(string(sdkInterop, "payment_token_norito_base64"));

    final OfflineNoteTransferHandoff.OfflineNoteTransferCapabilities capabilities =
        OfflineNoteTransferHandoff.OfflineNoteTransferCapabilities.current(false, true);
    assertTrue(
        capabilities
            .supportedModalities()
            .contains(OfflineNoteTransferHandoff.OfflineNoteTransferModality.QR_STREAMING),
        "QR transfer capability");
    assertTrue(
        capabilities
            .supportedModalities()
            .contains(OfflineNoteTransferHandoff.OfflineNoteTransferModality.NEARBY),
        "nearby transfer capability");
    assertTrue(
        !capabilities
            .supportedModalities()
            .contains(OfflineNoteTransferHandoff.OfflineNoteTransferModality.NFC),
        "NFC should require explicit HCE capability");

    final OfflineNoteTransferHandoff.OfflineNoteTransferPayload nearby =
        OfflineNoteTransferHandoff.nearbyPayload(token);
    assertEquals(
        OfflineNoteTransferHandoff.OfflineNoteTransferModality.NEARBY.name(),
        nearby.modality().name(),
        "nearby modality");
    assertEquals(
        OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
        nearby.contentType(),
        "nearby content type");
    assertTrue(Arrays.equals(canonicalPayload, nearby.payload()), "nearby payload");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteTransferHandoff.decodePaymentToken(nearby).tokenIdHex(),
        "nearby token id");
    final OfflineNoteNearbyEnvelope textChallenge =
        new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.RECEIVE_REQUEST,
            OfflineNoteReceiveRequestCodec.encodeText(receiveRequest).getBytes(StandardCharsets.UTF_8),
            OfflineNoteTransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE,
            OfflineNoteNearbyEnvelope.PairingChallenge.random());
    final OfflineNoteNearbyEnvelope decodedTextChallenge =
        OfflineNoteNearbyEnvelope.decode(textChallenge.encoded());
    assertEquals(
        receiveRequest.outputCommitmentHex(),
        decodedTextChallenge.receiveRequest().outputCommitmentHex(),
        "nearby text challenge payload");
    final OfflineNoteNearbyEnvelope textPayment =
        new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.PAYMENT,
            OfflineNotePaymentTokenCodec.encodeText(token).getBytes(StandardCharsets.UTF_8),
            OfflineNoteTransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE);
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteNearbyEnvelope.decode(textPayment.encoded()).paymentToken().tokenIdHex(),
        "nearby text payment payload");
    final OfflineNoteTransferHandoff.OfflineNoteTransferPayload ackPayload =
        OfflineNoteTransferHandoff.receiptAckPayload(
            receiptAck, OfflineNoteTransferHandoff.OfflineNoteTransferModality.NEARBY);
    assertEquals(
        receiptAck.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeReceiptAck(ackPayload).tokenIdHex(),
        "nearby receipt ACK payload");
    final byte[] nearbyAckBytes = OfflineNoteTransferHandoff.nearbyReceiptAckEnvelopeBytes(receiptAck);
    assertEquals(
        receiptAck.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeNearbyReceiptAck(nearbyAckBytes).tokenIdHex(),
        "nearby receipt ACK envelope");

    final List<Object> expectedFrameObjects = list(obj(sdkInterop, "payment_token_qr"), "frames");
    final List<String> expectedFrames = new ArrayList<>();
    for (final Object frameObject : expectedFrameObjects) {
      expectedFrames.add(string(asMap(frameObject, "payment_token_qr.frames[]"), "bytes_hex"));
    }
    final List<byte[]> qrFrames = OfflineNoteTransferHandoff.qrStreamingFrameBytes(token);
    final List<String> actualFrames = new ArrayList<>();
    for (final byte[] frame : qrFrames) {
      actualFrames.add(hex(frame));
    }
    assertTrue(expectedFrames.equals(actualFrames), "handoff QR frames");
    final OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver qrReceiver =
        new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver();
    OfflineNoteTransferHandoff.OfflineNoteTransferStreamResult qrResult = null;
    for (final byte[] frame : qrFrames) {
      qrResult = qrReceiver.ingestFrame(frame);
    }
    assertTrue(qrResult != null && qrResult.token() != null, "handoff QR token");
    assertEquals(token.tokenIdHex(), qrResult.token().tokenIdHex(), "handoff QR token id");

    final List<byte[]> nfcFrames = OfflineNoteTransferHandoff.nfcFrameBytes(token);
    for (final byte[] frame : nfcFrames) {
      assertTrue(frame.length <= 250, "NFC frame fits short APDU payload budget");
    }
    final OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver nfcReceiver =
        new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver();
    OfflineNoteTransferHandoff.OfflineNoteTransferStreamResult nfcResult = null;
    for (final byte[] frame : nfcFrames) {
      nfcResult = nfcReceiver.ingestFrame(frame);
    }
    assertTrue(nfcResult != null && nfcResult.token() != null, "handoff NFC token");
    assertEquals(token.tokenIdHex(), nfcResult.token().tokenIdHex(), "handoff NFC token id");
  }

  private static void transferHandoffRejectsAdversarialStreamsAndMetadata() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] rawPayload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token);
    final OfflineNoteTransferHandoff.OfflineNoteTransferPayload payload =
        OfflineNoteTransferHandoff.paymentTokenPayload(
            token, OfflineNoteTransferHandoff.OfflineNoteTransferModality.QR_STREAMING);
    final OfflineNoteTransferHandoff.OfflineNoteTransferPayload wrongContentType =
        new OfflineNoteTransferHandoff.OfflineNoteTransferPayload(
            OfflineNoteTransferHandoff.OfflineNoteTransferModality.NEARBY,
            OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
            payload.payload());
    assertThrows(
        () -> OfflineNoteTransferHandoff.decodePaymentToken(wrongContentType),
        "wrong handoff content type should fail");

    final List<byte[]> frames =
        OfflineNoteTransferHandoff.qrStreamingFrameBytes(
            token, new OfflineQrStream.Options(128, 0));
    assertTrue(frames.size() > 2, "adversarial stream test needs multiple data frames");

    final byte[] badMagic = Arrays.copyOf(frames.get(0), frames.get(0).length);
    badMagic[0] = 0x00;
    assertThrows(
        () -> new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver().ingestFrame(badMagic),
        "bad QR stream magic should fail");

    final byte[] badKind = Arrays.copyOf(frames.get(0), frames.get(0).length);
    badKind[2] = 0x7f;
    assertThrows(
        () -> new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver().ingestFrame(badKind),
        "bad QR stream kind should fail");

    final byte[] badChecksum = Arrays.copyOf(frames.get(1), frames.get(1).length);
    badChecksum[badChecksum.length - 1] ^= 0x01;
    assertThrows(
        () -> new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver().ingestFrame(badChecksum),
        "bad QR stream checksum should fail");

    final byte[] truncated = Arrays.copyOfRange(frames.get(0), 0, 8);
    assertThrows(
        () -> new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver().ingestFrame(truncated),
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
        () -> new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver().ingestFrame(mismatchedHeader),
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
    final OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver ignoreWrongStreamReceiver =
        new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver();
    assertTrue(!ignoreWrongStreamReceiver.ingestFrame(frames.get(0)).isComplete(), "header only is incomplete");
    assertTrue(!ignoreWrongStreamReceiver.ingestFrame(wrongStreamFrame).isComplete(), "wrong stream data is ignored");
    OfflineNoteTransferHandoff.OfflineNoteTransferStreamResult completed = null;
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
    final OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver poisonedReceiver =
        new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver();
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
            OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK,
            new OfflineQrStream.Options(512, 0));
    final OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver wrongKindReceiver =
        new OfflineNoteTransferHandoff.OfflineNoteTransferStreamReceiver();
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
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] payload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token);

    assertEquals(
        OfflineNoteTransferHandoff.DEFAULT_NFC_AID_HEX,
        OfflineNoteNfcApduProtocol.AID_HEX,
        "NFC AID");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.select()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.selectAidApdu())),
        "select APDU");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.getInfo()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.getInfoApdu())),
        "get-info APDU");

    final byte[] infoBytes =
        OfflineNoteNfcApduProtocol.encodeInfo(
            OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload);
    final OfflineNoteNfcApduProtocol.PayloadInfo info =
        OfflineNoteNfcApduProtocol.decodeInfo(infoBytes);
    assertTrue(info != null, "NFC info decodes");
    assertTrue(info.kind() == OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, "info kind");
    assertEquals(payload.length, info.payloadLength(), "info payload length");
    assertEquals(
        OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES,
        info.maxChunkLength(),
        "info max chunk length");
    assertTrue(
        OfflineNoteNfcApduProtocol.payloadDigestMatches(payload, info.sha256()),
        "info digest");

    final List<byte[]> androidApdus = OfflineNoteTransferHandoff.nfcPaymentTokenWriteApdus(token);
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.writeMeta(
                OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
                payload.length,
                info.sha256())
            .equals(OfflineNoteNfcApduProtocol.parseCommand(androidApdus.get(0))),
        "payment write meta");
    for (int index = 1; index < androidApdus.size() - 1; index++) {
      final OfflineNoteNfcApduProtocol.Command command =
          OfflineNoteNfcApduProtocol.parseCommand(androidApdus.get(index));
      assertTrue(
          command.type() == OfflineNoteNfcApduProtocol.Type.WRITE_CHUNK,
          "payment write chunk");
      assertTrue(
          command.bytes().length <= OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES,
          "payment write chunk fits Android APDU budget");
    }
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.commit()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(androidApdus.get(androidApdus.size() - 1))),
        "payment commit");

    final byte[] fastPayload = new byte[512];
    Arrays.fill(fastPayload, (byte) 0x5A);
    final byte[] fastApdu = OfflineNoteNfcApduProtocol.writeChunkApdu(1024, fastPayload);
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0x80, 0x21, 0x04, 0x00, 0x00, 0x02, 0x00},
            Arrays.copyOfRange(fastApdu, 0, 7)),
        "iOS fast extended write header");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.writeChunk(1024, fastPayload)
            .equals(OfflineNoteNfcApduProtocol.parseCommand(fastApdu)),
        "iOS fast extended write parse");
    final byte[] fastRead =
        OfflineNoteNfcApduProtocol.readChunkApdu(
            256, OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES);
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.readChunk(
                256, OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES)
            .equals(OfflineNoteNfcApduProtocol.parseCommand(fastRead)),
        "iOS fast extended read parse");
  }

  private static void transportWireFormatMatchesSharedFixture() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final byte[] payload = OfflineNoteTransferHandoff.rawPaymentTokenBytes(token);
    final List<byte[]> writeApdus = OfflineNoteTransferHandoff.nfcPaymentTokenWriteApdus(token);
    final List<byte[]> readApdus = OfflineNoteNfcApduProtocol.readPayloadApdus(payload.length);
    final byte[] nearbyBytes = OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(token);

    assertEquals(4674, payload.length, "transport fixture payload length");
    assertEquals(
        "00a4040007f049524f48413200",
        hex(OfflineNoteNfcApduProtocol.selectAidApdu()),
        "select APDU fixture");
    assertEquals(
        "8010000000", hex(OfflineNoteNfcApduProtocol.getInfoApdu()), "get-info APDU fixture");
    assertEquals(
        "020000124200f068ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
        hex(
            OfflineNoteNfcApduProtocol.encodeInfo(
                OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload)),
        "NFC info fixture");
    assertEquals(
        "8020000025020000124268ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
        hex(
            OfflineNoteNfcApduProtocol.writeMetaApdu(
                OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload)),
        "NFC write-meta fixture");
    assertEquals(22, writeApdus.size(), "NFC write APDU count");
    assertEquals(
        "8020000025020000124268ca7bc10b9a8c2d2da698c943d94f84eccc0fb795ede09337399075fb330d3c",
        hex(writeApdus.get(0)),
        "NFC first write APDU fixture");
    assertEquals(
        "53d4d61b3f22e432a5a309c4813f55f5562d74b96b59c657bc230f6d5a0031d4",
        hex(OfflineNoteNfcApduProtocol.sha256(writeApdus.get(1))),
        "NFC first chunk fixture digest");
    assertEquals(
        "802111d0720968616c6f322f69706117166f66666c696e652d6e6f74652d72656375727369766520699b945eaef37b763f70ce18b173caed4fe4fec9bb8110fc5231feb9f868d7a52e0a0968616c6f322f697061221a000000000000006f66666c696e652d766563746f722d61756469742d70726f6f66",
        hex(writeApdus.get(writeApdus.size() - 2)),
        "NFC final chunk fixture");
    assertEquals("8022000000", hex(writeApdus.get(writeApdus.size() - 1)), "NFC commit fixture");
    assertEquals(20, readApdus.size(), "NFC read APDU count");
    assertEquals("80110000f0", hex(readApdus.get(0)), "NFC first read APDU fixture");
    assertEquals(6330, nearbyBytes.length, "Nearby payment envelope fixture length");
    assertEquals(
        "fa386f2157f8d9be82828eb1e79b6b57e05b9d4777d5e46b0c0684de11892184",
        hex(OfflineNoteNfcApduProtocol.sha256(nearbyBytes)),
        "Nearby payment envelope fixture digest");
  }

  private static void nfcApduProtocolRejectsAdversarialPayloadsBeforeCommit() {
    final byte[] payload = "offline-payment".getBytes(StandardCharsets.UTF_8);
    final OfflineNoteNfcApduProtocol.PayloadInfo info =
        OfflineNoteNfcApduProtocol.decodeInfo(
            OfflineNoteNfcApduProtocol.encodeInfo(
                OfflineNoteNfcApduProtocol.PayloadKind.RECEIPT_ACK, payload));
    assertTrue(info != null, "receipt ACK info decodes");
    final OfflineNoteNfcApduProtocol.PayloadAssembler assembler =
        new OfflineNoteNfcApduProtocol.PayloadAssembler(info);

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
        OfflineNoteNfcApduProtocol.encodeInfo(
            OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload);
    final int oversized = OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1;
    oversizedInfo[1] = (byte) ((oversized >>> 24) & 0xFF);
    oversizedInfo[2] = (byte) ((oversized >>> 16) & 0xFF);
    oversizedInfo[3] = (byte) ((oversized >>> 8) & 0xFF);
    oversizedInfo[4] = (byte) (oversized & 0xFF);
    assertTrue(OfflineNoteNfcApduProtocol.decodeInfo(oversizedInfo) == null, "reject oversized info");

    final OfflineNoteNfcApduProtocol.PayloadAssembler badAssembler =
        new OfflineNoteNfcApduProtocol.PayloadAssembler(
            OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, payload.length, new byte[32]);
    assertTrue(badAssembler.write(0, payload), "bad checksum payload written");
    assertThrows(badAssembler::commit, "checksum mismatch commit should fail");
    assertThrows(
        () ->
            new OfflineNoteNfcApduProtocol.PayloadAssembler(
                OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
                OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1,
                new byte[32]),
        "oversized assembler should fail before allocation");
  }

  private static void nfcApduProtocolRejectsMalformedCommandsAndBounds() {
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(null)),
        "null APDU should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(new byte[] {0x00})),
        "short APDU should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.unsupported()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {0x00, (byte) 0xA4, 0x04, 0x00, 0x01, (byte) 0xFF, 0x00})),
        "wrong AID should be unsupported");
    final byte[] selectWithNonZeroLe = OfflineNoteNfcApduProtocol.selectAidApdu();
    selectWithNonZeroLe[selectWithNonZeroLe.length - 1] = 0x01;
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.unsupported()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(selectWithNonZeroLe)),
        "select AID with nonzero Le should be unsupported");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.unsupported()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x81, 0x10, 0x00, 0x00, 0x00})),
        "wrong CLA should be unsupported");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x01, 0x00})),
        "get-info with nonzero P1/P2 should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x00, 0x01})),
        "get-info with nonzero Le should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x10, 0x00, 0x00, 0x01, 0x00})),
        "get-info with data should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x11, 0x00, 0x00, 0x00})),
        "zero short read length should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00})),
        "zero-length extended read should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x20, 0x00, 0x00, 0x01, 0x01})),
        "short write-meta should be invalid");
    final byte[] writeMetaWithOffset =
        OfflineNoteNfcApduProtocol.writeMetaApdu(
            OfflineNoteNfcApduProtocol.PayloadKind.RECEIPT_ACK, new byte[] {0x01});
    writeMetaWithOffset[3] = 0x01;
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(writeMetaWithOffset)),
        "write-meta with nonzero P1/P2 should be invalid");
    final byte[] zeroLengthMeta = new byte[37];
    zeroLengthMeta[0] = (byte) OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN.code();
    final byte[] zeroLengthMetaApdu = new byte[42];
    zeroLengthMetaApdu[0] = (byte) 0x80;
    zeroLengthMetaApdu[1] = 0x20;
    zeroLengthMetaApdu[4] = (byte) zeroLengthMeta.length;
    System.arraycopy(zeroLengthMeta, 0, zeroLengthMetaApdu, 5, zeroLengthMeta.length);
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(OfflineNoteNfcApduProtocol.parseCommand(zeroLengthMetaApdu)),
        "zero-length write-meta should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x21, 0x00, 0x00, 0x00})),
        "empty write chunk should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x21, 0x00, 0x00, 0x02, 0x01})),
        "truncated write chunk should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x00, 0x00, 0x01, 0x00})),
        "commit with data should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x01, 0x00, 0x00})),
        "commit with nonzero P1/P2 should be invalid");
    assertTrue(
        OfflineNoteNfcApduProtocol.Command.invalid()
            .equals(
                OfflineNoteNfcApduProtocol.parseCommand(
                    new byte[] {(byte) 0x80, 0x22, 0x00, 0x00, 0x01})),
        "commit with nonzero Le should be invalid");

    assertThrows(
        () -> OfflineNoteNfcApduProtocol.writeChunkApdu(0x1_0000, new byte[] {0x01}),
        "oversized offset should fail");
    assertThrows(
        () -> OfflineNoteNfcApduProtocol.writeChunkApdu(0, new byte[0]),
        "empty write chunk should fail");
    assertThrows(
        () -> OfflineNoteNfcApduProtocol.readChunkApdu(0, 0),
        "zero read chunk length should fail");
    assertThrows(
        () ->
            OfflineNoteNfcApduProtocol.readChunkApdu(
                0, OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1),
        "oversized direct read chunk length should fail");
    assertThrows(
        () ->
            OfflineNoteNfcApduProtocol.writePayloadApdus(
                OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN, new byte[] {0x01}, 0),
        "zero max chunk length should fail");
    assertThrows(
        () -> OfflineNoteNfcApduProtocol.readPayloadApdus(0),
        "zero read payload length should fail");
    assertThrows(
        () ->
            OfflineNoteNfcApduProtocol.readPayloadApdus(
                1, OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES + 1),
        "oversized read chunk length should fail");

    final byte[] response = OfflineNoteNfcApduProtocol.response(new byte[] {(byte) 0xAA, (byte) 0xBB});
    assertTrue(
        Arrays.equals(new byte[] {(byte) 0xAA, (byte) 0xBB, (byte) 0x90, 0x00}, response),
        "response should append success status");
    assertEquals(0x9000, OfflineNoteNfcApduProtocol.responseStatus(response), "response status");
    assertEquals(
        -1,
        OfflineNoteNfcApduProtocol.responseStatus(new byte[] {(byte) 0x90}),
        "short response status");
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0xAA, (byte) 0xBB},
            OfflineNoteNfcApduProtocol.responseData(response)),
        "response data");
    assertTrue(
        Arrays.equals(new byte[0], OfflineNoteNfcApduProtocol.responseData(new byte[] {(byte) 0x90})),
        "short response data");

    final OfflineNoteNfcApduProtocol.PayloadAssembler assembler =
        new OfflineNoteNfcApduProtocol.PayloadAssembler(
            OfflineNoteNfcApduProtocol.PayloadKind.RECEIPT_ACK,
            4,
            OfflineNoteNfcApduProtocol.sha256(new byte[] {0x01, 0x02, 0x03, 0x04}));
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
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteReceiveRequest receiveRequest = receiveRequestFixture(fixture);
    final OfflineNoteReceiptAck receiptAck =
        OfflineNoteReceiptAck.fromPaymentToken(
            token,
            string(obj(fixture, "payment_token"), "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));
    final OfflineNoteNearbyEnvelope.PairingChallenge challenge =
        new OfflineNoteNearbyEnvelope.PairingChallenge(" nearby_pairing_bird ");
    final OfflineNoteNearbyEnvelope challengeEnvelope =
        new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.RECEIVE_REQUEST,
            OfflineNoteTransferHandoff.rawReceiveRequestBytes(receiveRequest),
            OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            challenge);
    final byte[] paymentBytes = OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(token);
    final OfflineNoteNearbyEnvelope paymentEnvelope =
        OfflineNoteNearbyEnvelope.decode(paymentBytes);
    final OfflineNoteNearbyEnvelope ackEnvelope =
        new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.RECEIPT_ACK,
            OfflineNoteTransferHandoff.rawReceiptAckBytes(receiptAck),
            OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE);

    assertTrue(
        challenge.equals(
            OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).pairingChallenge()),
        "challenge pairing roundtrip");
    assertTrue(paymentEnvelope.kind() == OfflineNoteNearbyEnvelope.Kind.PAYMENT, "payment kind");
    assertEquals(
        receiveRequest.outputCommitmentHex(),
        OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).receiveRequest().outputCommitmentHex(),
        "receive request");
    assertEquals(token.tokenIdHex(), paymentEnvelope.paymentToken().tokenIdHex(), "payment token");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeNearbyPaymentToken(paymentBytes).tokenIdHex(),
        "payment token handoff decode");
    assertTrue(
        OfflineNoteNearbyEnvelope.decode(ackEnvelope.encoded()).receiptAck().matchesPaymentToken(token),
        "ACK payload");
  }

  private static void nearbyEnvelopeRejectsAdversarialMessages() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final byte[] tokenPayload = base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64"));
    final OfflineNoteNearbyEnvelope.PairingChallenge pairing =
        new OfflineNoteNearbyEnvelope.PairingChallenge("nearby_pairing_mask");

    assertThrows(
        () -> new OfflineNoteNearbyEnvelope.PairingChallenge("nearby_pairing_mask<script>"),
        "invalid pairing asset should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.RECEIVE_REQUEST,
                "challenge".getBytes(StandardCharsets.UTF_8),
                OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE),
        "challenge without pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.RECEIVE_REQUEST,
                "challenge".getBytes(StandardCharsets.UTF_8),
                OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
                pairing),
        "challenge content type downgrade should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.PAYMENT,
                tokenPayload,
                OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
                pairing),
        "payment with pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.PAYMENT,
                new byte[OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES + 1],
                OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE),
        "oversized nearby payment should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.RECEIPT_ACK,
                "ok".getBytes(StandardCharsets.UTF_8),
                OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE),
        "receipt ACK content type downgrade should fail");

    final byte[] unknownField =
        ("{\"kind\":\"payment\",\"payload\":\"AQID\","
                + "\"contentType\":\"application/vnd.iroha.offline.payment-token+norito\","
                + "\"extra\":true}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] challengeContentTypeDowngrade =
        ("{\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receipt-ack+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] ackContentTypeDowngrade =
        ("{\"kind\":\"receipt_ack\",\"payload\":\"b2s\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedPayload =
        ("{\"kind\":\"challenge\",\"payload\":\"YQ==\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(unknownField),
        "unknown nearby envelope field should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(challengeContentTypeDowngrade),
        "challenge content type downgrade should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(ackContentTypeDowngrade),
        "receipt ACK content type downgrade should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(paddedPayload),
        "padded nearby envelope payload should fail");
    final byte[] topLevelArray = "[]".getBytes(StandardCharsets.UTF_8);
    final byte[] invalidBase64Payload =
        ("{\"kind\":\"challenge\",\"payload\":\"!!!!\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] badPairingObject =
        ("{\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":{\"assetName\":1}}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] smuggledPairingObject =
        ("{\"kind\":\"challenge\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":{\"assetName\":\"nearby_pairing_bird\",\"extra\":true}}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] ackWithPairing =
        ("{\"kind\":\"receipt_ack\",\"payload\":\"b2s\","
                + "\"contentType\":\"application/vnd.iroha.offline.receipt-ack+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(topLevelArray),
        "top-level array nearby envelope should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(invalidBase64Payload),
        "invalid nearby envelope base64url should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(badPairingObject),
        "bad nearby pairing object should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(smuggledPairingObject),
        "nearby pairing object with unknown field should fail");
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(ackWithPairing),
        "ACK with pairing should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.PAYMENT,
                new byte[] {0x01, 0x02, 0x03},
                OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE),
        "invalid payment token payload should fail");
    assertThrows(
        () ->
            new OfflineNoteNearbyEnvelope(
                OfflineNoteNearbyEnvelope.Kind.RECEIPT_ACK,
                new byte[0],
                OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE),
        "empty ACK payload should fail");
  }

  private static void walletAcceptsCanonicalSdkInteropPaymentToken() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore recipientStore = new InMemoryOfflineNoteStore();
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            recipientStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_200L,
            new StaticOwnerCertificateSigner(recipientCertificate));
    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(obj(chain, "issue"), "asset_id")),
            string(obj(chain, "redeem"), "amount"));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        receiveRequest.outputCommitmentHex(),
        "canonical receive output commitment");

    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNoteWalletNote accepted = recipientWallet.accept(token);

    assertEquals(
        string(derivation, "recipient_output_commitment"),
        accepted.noteCommitmentHex(),
        "accepted canonical note commitment");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted canonical note state");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        recipientStore
            .findNote(hexBytes(string(derivation, "recipient_output_commitment")))
            .state()
            .name(),
        "stored canonical note state");
  }

  private static void walletRejectsBearerCashCustodyPolicyOverflow() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            null,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_250L,
            new OfflineBearerCashPolicyV1(1, 32, 2048, 12288, 20, 8, 40),
            new StaticOwnerCertificateSigner(recipientCertificate));
    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(obj(chain, "issue"), "asset_id")),
            string(obj(chain, "redeem"), "amount"));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        receiveRequest.outputCommitmentHex(),
        "recipient output commitment");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final OfflineNote.AuditBundle ancestor = ancestorAuditProducingFirstInput(token.audit(), 0xC0);
    final OfflineNotePaymentToken overLimit =
        paymentTokenReplacingBearerAuditTrail(token, Arrays.asList(ancestor, token.audit()));

    assertThrows(
        () -> recipientWallet.accept(overLimit),
        "custody policy should reject over-depth bearer audit trail");
  }

  private static void walletNoteJsonCodecRoundTripsFixtureNote() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);

    final OfflineNoteWalletNote decoded =
        OfflineNoteWalletNoteJsonCodec.decode(OfflineNoteWalletNoteJsonCodec.encode(note));

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
        decoded.origin() instanceof OfflineNote.CommitmentOrigin.IssuerLoad,
        "note origin type");
    final OfflineNote.CommitmentOrigin.IssuerLoad origin =
        (OfflineNote.CommitmentOrigin.IssuerLoad) decoded.origin();
    final OfflineNote.CommitmentOrigin.IssuerLoad expectedOrigin =
        (OfflineNote.CommitmentOrigin.IssuerLoad) note.origin();
    assertEquals(expectedOrigin.operationId(), origin.operationId(), "origin operation id");
    assertEquals(expectedOrigin.lineageId(), origin.lineageId(), "origin lineage id");
    assertEquals(expectedOrigin.localRevision(), origin.localRevision(), "origin local revision");

    final String encoded =
        new String(OfflineNoteWalletNoteJsonCodec.encode(note), StandardCharsets.UTF_8);
    final OfflineNoteWalletNote migratedSpent =
        OfflineNoteWalletNoteJsonCodec.decode(
            encoded.replace("\"state\":\"SPENDABLE\"", "\"state\":\"SPEND_PENDING\"")
                .getBytes(StandardCharsets.UTF_8));
    assertEquals(
        OfflineNoteWalletNoteState.SPENT.name(),
        migratedSpent.state().name(),
        "migrated spend pending state");
    final OfflineNoteWalletNote migratedChange =
        OfflineNoteWalletNoteJsonCodec.decode(
            encoded.replace("\"state\":\"SPENDABLE\"", "\"state\":\"CHANGE_PENDING\"")
                .getBytes(StandardCharsets.UTF_8));
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        migratedChange.state().name(),
        "migrated change pending state");
  }

  private static void walletLoadDerivesCommitmentBeforeIssuerSubmission() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteLoadContext loadContext =
        new OfflineNoteLoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final RecordingIssuerClient issuerClient = new RecordingIssuerClient(loadContext);
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(issue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            new InMemoryOfflineNoteStore(),
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "source_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_000L);

    final OfflineNoteWalletNote note =
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
        OfflineNoteWalletNoteState.SPENDABLE.name(),
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
    final OfflineNoteIssuerDeviceBinding binding =
        new OfflineNoteIssuerDeviceBinding("device-1", offlinePublicKey, bindingJson);
    final OfflineIssuerExecutor executor = new OfflineIssuerExecutor(certificateJson);
    final List<byte[]> signedMessages = new ArrayList<>();
    final ToriiOfflineNoteIssuerClient client =
        new ToriiOfflineNoteIssuerClient(
            new ToriiCanonicalRequestAuth(
                accountId,
                message -> {
                  signedMessages.add(Arrays.copyOf(message, message.length));
                  return fakeIssuerSignature(message);
                }),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            (chainId, requestAccountId, requestAssetDefinitionId, operation, lineageId, proofAmount) -> {
              final Map<String, Object> proof = new LinkedHashMap<>();
              proof.put("operation", operation);
              proof.put("lineage_id", lineageId);
              if (proofAmount != null) {
                proof.put("amount", proofAmount);
              }
              return proof;
            },
            executor,
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            Map.of(),
            List.of(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-refill-1", "auth-refill-1", "auth-issue-1"));

    final OfflineNoteLoadContext context =
        client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").get();
    assertEquals("operation-refill-1", context.operationId(), "operation id");
    assertEquals("lineage-1", context.lineageId(), "lineage id");
    assertEquals(1L, context.localRevision(), "post-issue commitment revision");

    final byte[] commitment = new byte[32];
    for (int i = 0; i < commitment.length; i++) {
      commitment[i] = (byte) (i + 1);
    }
    final OfflineNoteIssueResponse response =
        client.issueNote(
                new OfflineNoteIssueRequest(
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
        "/v1/offline/keys/refill", executor.requests.get(0).uri().getPath(), "refill path");
    assertEquals(
        "/v1/offline/notes/issue", executor.requests.get(1).uri().getPath(), "issue path");
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
    assertEquals(2L, signedMessages.size(), "issuer body auth signing count");
    final byte[] refillMessage =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "POST",
            executor.requests.get(0).uri(),
            refillBody,
            1_700_000_000_000L,
            "auth-refill-1");
    assertTrue(Arrays.equals(refillMessage, signedMessages.get(0)), "refill body auth message");
    assertEquals(
        Base64.getEncoder().encodeToString(fakeIssuerSignature(refillMessage)),
        string(refillBody, "signature_base64"),
        "refill body signature");
    assertEquals(
        "nested-device-signature-is-not-body-auth",
        string(obj(refillBody, "device_binding"), "signature_base64"),
        "nested device proof is preserved");
    assertEquals(
        "setup", string(obj(refillBody, "device_proof"), "operation"), "refill device proof");

    final Map<String, Object> issueBody = executor.requestBody(1);
    assertEquals(hex(commitment), string(issueBody, "note_commitment"), "issue commitment");
    assertEquals(0L, longValue(issueBody, "local_revision"), "pre-issue revision");
    assertEquals("0", string(issueBody, "local_balance"), "pre-issue balance");
    final byte[] issueMessage =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "POST",
            executor.requests.get(1).uri(),
            issueBody,
            1_700_000_000_000L,
            "auth-issue-1");
    assertTrue(Arrays.equals(issueMessage, signedMessages.get(1)), "issue body auth message");
    assertEquals(
        Base64.getEncoder().encodeToString(fakeIssuerSignature(issueMessage)),
        string(issueBody, "signature_base64"),
        "issue body signature");
    assertEquals("auth-issue-1", string(issueBody, "nonce"), "issue nonce");
    assertEquals(
        "load", string(obj(issueBody, "device_proof"), "operation"), "issue device proof");
    assertEquals("5", string(obj(issueBody, "device_proof"), "amount"), "issue proof amount");
    obj(issueBody, "lineage_state");
  }

  private static byte[] fakeIssuerSignature(final byte[] message) {
    final byte[] signature = new byte[64];
    for (int index = 0; index < signature.length; index++) {
      signature[index] = (byte) (message[index % message.length] ^ (index + 17));
    }
    return signature;
  }

  private static void toriiIssuerClientRejectsMalformedCertificateUsageLimits() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> baseCertificateJson =
        obj(obj(fixture, "payment_token"), "sender_key_certificate");
    final String accountId = string(baseCertificateJson, "account_id");
    final String assetDefinitionId =
        assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"));
    final String offlinePublicKey = "a5".repeat(32);
    final Map<String, Object> bindingJson = new LinkedHashMap<>();
    bindingJson.put("device_id", "device-1");
    bindingJson.put("attestation_key_id", "attestation-key-1");
    bindingJson.put("offline_public_key", offlinePublicKey);
    final OfflineNoteIssuerDeviceBinding binding =
        new OfflineNoteIssuerDeviceBinding("device-1", offlinePublicKey, bindingJson);

    for (final Object invalidLimit : List.of(0L, 2L, 4_294_967_297L, "1")) {
      final Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
      certificateJson.put("assertion_usage_count_limit", invalidLimit);
      assertToriiIssuerClientRejectsCertificateJson(
          certificateJson, accountId, assetDefinitionId, binding);
    }
    for (final Object invalidVersion : List.of(0L, 2L, 4_294_967_297L, "1")) {
      final Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
      certificateJson.put("version", invalidVersion);
      assertToriiIssuerClientRejectsCertificateJson(
          certificateJson, accountId, assetDefinitionId, binding);
    }
  }

  private static void assertToriiIssuerClientRejectsCertificateJson(
      final Map<String, Object> certificateJson,
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding) {
    final ToriiOfflineNoteIssuerClient client =
        new ToriiOfflineNoteIssuerClient(
            new ToriiCanonicalRequestAuth(accountId, OfflineNoteTest::fakeIssuerSignature),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            (chainId, requestAccountId, requestAssetDefinitionId, operation, lineageId, proofAmount) -> Map.of(),
            new OfflineIssuerExecutor(certificateJson),
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            Map.of(),
            List.of(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-refill-malformed", "auth-refill-malformed"));

    final Throwable cause =
        assertFutureFailsWithin(
            client.prepareLoad("chain-1", accountId, assetDefinitionId, "5"),
            "malformed certificate metadata must reject");
    Throwable root = cause;
    while (root instanceof java.util.concurrent.CompletionException completionException
        && completionException.getCause() != null) {
      root = completionException.getCause();
    }
    assertTrue(
        root instanceof OfflineToriiException
            || root instanceof IllegalStateException
            || root instanceof IllegalArgumentException,
        "malformed certificate metadata failure type");
  }

  private static void walletLoadDoesNotBlockIssuerCompletionThread() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> token = obj(fixture, "payment_token");
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> issue = obj(obj(fixture, "chain_vectors"), "issue");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(token, "sender_key_certificate"));
    final String accountId = accountFromAssetId(string(issue, "asset_id"));
    final OfflineNoteLoadContext loadContext =
        new OfflineNoteLoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final CompletionControlledIssuerClient issuerClient =
        new CompletionControlledIssuerClient(loadContext);
    final BlockingOfflineNoteStore store = new BlockingOfflineNoteStore();
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountId,
            new StaticAttestationProvider(senderCertificate),
            store,
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "source_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_000L);

    final CompletableFuture<OfflineNoteWalletNote> load =
        wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount"));
    assertTrue(
        issuerClient.issueRequested.await(5, TimeUnit.SECONDS),
        "wallet load did not submit issue request");

    final OfflineNoteIssueRequest request = issuerClient.lastIssueRequest;
    final OfflineNoteIssueResponse response =
        new OfflineNoteIssueResponse(
            request.noteCommitment(),
            request.loadContext().operationId(),
            request.loadContext().lineageId(),
            request.loadContext().localRevision(),
            request.loadContext().keyCertificate(),
            "settlement-entry-hash");
    final AtomicBoolean completeReturned = new AtomicBoolean(false);
    final ExecutorService issuerCompleter =
        Executors.newSingleThreadExecutor(r -> new Thread(r, "offline-note-issuer-completer"));
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

  private static void walletLoadCompletesExceptionallyWhenIssuerThrowsSynchronously()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> token = obj(fixture, "payment_token");
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final Map<String, Object> issue = obj(obj(fixture, "chain_vectors"), "issue");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(token, "sender_key_certificate"));
    final String accountId = accountFromAssetId(string(issue, "asset_id"));
    final OfflineNoteLoadContext loadContext =
        new OfflineNoteLoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountId,
            new StaticAttestationProvider(senderCertificate),
            new InMemoryOfflineNoteStore(),
            new SynchronouslyThrowingIssuerClient(loadContext),
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "source_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_000L);

    final Throwable cause =
        assertFutureFailsWithin(
            wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount")),
            "synchronous issue failure should fail wallet load");
    assertTrue(
        cause instanceof IllegalStateException,
        "synchronous issue failure should propagate the issuer exception");
    assertEquals("issuer exploded", cause.getMessage(), "synchronous issue failure message");
  }

  private static void walletLifecycleBuildsAuditAcceptAndRedeemTransactions() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainAudit = obj(chain, "audit");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms"),
            new StaticOwnerCertificateSigner(senderCertificate));
    final RecordingTransactionSubmitter recipientSubmitter = new RecordingTransactionSubmitter();
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            recipientSubmitter,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_200L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    assertEquals(
        string(derivation, "recipient_output_commitment"),
        receiveRequest.outputCommitmentHex(),
        "recipient output commitment");

    final OfflineNotePaymentToken token = senderWallet.pay(receiveRequest);

    assertEquals(string(derivation, "payment_token_id"), token.tokenIdHex(), "payment token id");
    assertEquals(
        string(derivation, "payment_request_id"),
        token.paymentRequestId(),
        "payment request id");
    final OfflineBearerCashPolicyV1.AuditTrailMetrics auditMetrics =
        OfflineBearerCashPolicyV1.DEFAULT.auditTrailMetrics(
            token.bearerAuditTrail(), token.audit());
    assertEquals(1L, auditMetrics.custodyHops(), "audit trail custody hops");
    assertEquals(1L, auditMetrics.lineageSteps(), "audit trail lineage steps");
    assertEquals(
        string(chainAudit, "public_inputs_hash"),
        hex(token.audit().publicInputsHash()),
        "audit public inputs hash");
    assertEquals(
        OfflineNoteWalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "source note state");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "change note state");

    final OfflineNoteWalletNote accepted = recipientWallet.accept(token);

    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertEquals(0L, recipientSubmitter.audits.size(), "audit submit count before publish");
    recipientWallet.publishAudit(token).get();
    assertEquals(1L, recipientSubmitter.audits.size(), "audit submit count");
    final OfflineNoteWalletNote redeeming = recipientWallet.redeem(accepted).get();
    assertEquals(
        OfflineNoteWalletNoteState.REDEEM_PENDING.name(),
        redeeming.state().name(),
        "redeem note state");
    assertEquals(0L, recipientSubmitter.redemptions.size(), "direct redeem submit count");
    assertEquals(1L, recipientSubmitter.defunds.size(), "defund submit count");
    assertEquals(
        string(chainRedeem, "public_inputs_hash"),
        hex(recipientSubmitter.defunds.get(0).redemption.publicInputsHash()),
        "redeem public inputs hash");
    assertEquals(1L, recipientSubmitter.defunds.get(0).bearerAuditTrail.size(), "defund audit trail count");
    assertEquals(
        token.tokenIdHex(),
        hex(recipientSubmitter.defunds.get(0).bearerAuditTrail.get(0).tokenId()),
        "defund audit trail token id");
  }

  private static void offlineNoteTransactionSubmitterIncludesFeeMetadata() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final CapturingIrohaClient client = new CapturingIrohaClient();
    final Map<String, String> metadata =
        IrohaOfflineNoteTransactionSubmitter.feeMetadata(
            "xor#universal", string(payment, "recipient_account_id"));
    final IrohaOfflineNoteTransactionSubmitter submitter =
        new IrohaOfflineNoteTransactionSubmitter(
            client,
            new FakeSigner(),
            string(derivation, "chain_id"),
            string(payment, "sender_account_id"),
            codec,
            () -> 1_736_000_000_000L,
            metadata);

    submitter.submitAudit(audit(fixture)).get(5, TimeUnit.SECONDS);

    assertTrue(client.submittedTransaction != null, "submitter should submit a transaction");
    final TransactionPayload payload =
        codec.decodeTransaction(client.submittedTransaction.encodedPayload());
    final Map<String, JsonValue> expectedMetadata = new LinkedHashMap<>();
    metadata.forEach((key, value) -> expectedMetadata.put(key, JsonValue.string(value)));
    assertTrue(expectedMetadata.equals(payload.metadata()), "fee metadata should round-trip");
  }

  private static void walletSyncReconcilesPendingSpendChangeAndRedeemStates() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final Map<String, OfflineNoteWalletNoteState> resolutions = new LinkedHashMap<>();
    resolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteWalletNoteState.SPENT);
    resolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteWalletNoteState.SPENDABLE);
    final RecordingSyncResolver syncResolver = new RecordingSyncResolver(resolutions);
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            syncResolver,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_000L,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_100L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    senderWallet.pay(receiveRequest);
    senderWallet.sync().get();

    assertEquals(
        OfflineNoteWalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "synced source note state");
    final OfflineNoteWalletNote spendableChange =
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")));
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        spendableChange.state().name(),
        "synced change note state");
    assertEquals(0L, syncResolver.resolvedCommitments.size(), "sync resolver commitment count");

    resolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteWalletNoteState.REDEEMED);
    final OfflineNoteWalletNote redeeming = senderWallet.redeem(spendableChange).get();
    assertEquals(
        OfflineNoteWalletNoteState.REDEEM_PENDING.name(),
        redeeming.state().name(),
        "redeeming note state");

    senderWallet.sync().get();

    assertEquals(
        OfflineNoteWalletNoteState.REDEEMED.name(),
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
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_200L,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_300L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    final OfflineNotePaymentToken token = senderWallet.pay(receiveRequest);

    assertThrows(
        () -> senderWallet.pay(receiveRequest), "already pending input payment should throw");

    final OfflineNoteWalletNote accepted = recipientWallet.accept(token);
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertThrows(() -> recipientWallet.accept(token), "duplicate token replay should fail");
  }

  private static void walletRejectsExactAmountReceiveRequestReplayAfterRestart() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final String accountId = accountFromAssetId(string(chainIssue, "asset_id"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    senderStore.upsert(
        new OfflineNoteWalletNote(
            string(derivation, "chain_id"),
            accountId,
            string(chainIssue, "asset_id"),
            string(chainIssue, "amount"),
            senderCertificate,
            filledBytes(32, (byte) 0x71),
            filledBytes(32, (byte) 0x72),
            new OfflineNote.CommitmentOrigin.IssuerLoad(
                "operation-extra-exact", "lineage-extra-exact", 2L),
            OfflineNoteWalletNoteState.SPENDABLE,
            1_700_000_000_100L,
            1_700_000_000_100L));
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountId,
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "token_nonce_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_400L,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_500L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainIssue, "amount"));
    senderWallet.pay(receiveRequest);
    int spentCount = 0;
    String spentPaymentRequestId = null;
    for (final OfflineNoteWalletNote note : senderStore.listNotes()) {
      if (note.state() == OfflineNoteWalletNoteState.SPENT) {
        spentCount++;
        spentPaymentRequestId = note.spentPaymentRequestId();
      }
    }
    assertEquals(1L, spentCount, "spent exact input count");
    assertEquals(
        string(derivation, "payment_request_id"),
        spentPaymentRequestId,
        "spent payment request id");

    final InMemoryOfflineNoteStore restoredStore = new InMemoryOfflineNoteStore();
    for (final OfflineNoteWalletNote note : senderStore.listNotes()) {
      restoredStore.upsert(
          OfflineNoteWalletNoteJsonCodec.decode(OfflineNoteWalletNoteJsonCodec.encode(note)));
    }
    final OfflineNoteWallet restoredWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountId,
            new StaticAttestationProvider(senderCertificate),
            restoredStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_600L,
            new StaticOwnerCertificateSigner(senderCertificate));

    assertThrows(
        () -> restoredWallet.pay(receiveRequest),
        "restored exact receive request replay should fail");
    int spendableCount = 0;
    for (final OfflineNoteWalletNote note : restoredStore.listNotes()) {
      if (note.state() == OfflineNoteWalletNoteState.SPENDABLE) {
        spendableCount++;
      }
    }
    assertEquals(1L, spendableCount, "remaining spendable note count");
  }

  private static void walletRedeemReservesNoteBeforeSubmitCompletes() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final InMemoryOfflineNoteStore store = new InMemoryOfflineNoteStore();
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    store.upsert(note);
    final PendingDefundTransactionSubmitter submitter =
        new PendingDefundTransactionSubmitter();
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            store,
            null,
            submitter,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_004_000L);

    final CompletableFuture<OfflineNoteWalletNote> redeeming = wallet.redeem(note);
    assertTrue(!redeeming.isDone(), "redeem future should wait for submit completion");
    assertEquals(1L, submitter.defunds.size(), "defund submission count");
    assertEquals(
        OfflineNoteWalletNoteState.REDEEM_PENDING.name(),
        store.findNote(note.noteCommitment()).state().name(),
        "redeem reserves note before submit completes");
    assertThrows(() -> wallet.redeem(note), "second redeem should reject pending note");
    assertEquals(1L, submitter.defunds.size(), "duplicate redeem should not submit");

    submitter.completeAccepted();
    assertEquals(
        OfflineNoteWalletNoteState.REDEEM_PENDING.name(),
        redeeming.get(1, TimeUnit.SECONDS).state().name(),
        "redeem returns pending note after submit");
  }

  private static void walletRejectsAdversarialCertificateBindings() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> chainRedeem = obj(chain, "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final String senderAccountId = accountFromAssetId(string(chainIssue, "asset_id"));
    final String assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"));

    final OfflineNoteWallet defaultRejectingWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            null,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_700L);
    assertThrows(
        () -> defaultRejectingWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount")),
        "default verifier should reject receive certificates");
    final OfflineNoteWallet wrongAccountReceiveWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            null,
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_710L,
            new StaticOwnerCertificateSigner(recipientCertificate));
    assertThrows(
        () -> wrongAccountReceiveWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount")),
        "valid receive certificate for the wrong account should fail");

    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms"),
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_800L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(assetDefinitionId, string(chainRedeem, "amount"));
    final OfflineNoteReceiveRequest accountSubstitution =
        new OfflineNoteReceiveRequest(
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
    final OfflineNoteReceiveRequest chainSubstitution =
        new OfflineNoteReceiveRequest(
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
    final OfflineNoteReceiveRequest assetOwnerSubstitution =
        new OfflineNoteReceiveRequest(
            receiveRequest.chainId(),
            receiveRequest.paymentRequestId(),
            receiveRequest.accountId(),
            receiveRequest.assetDefinitionId(),
            receiveRequest.assetDefinitionId() + "#" + senderAccountId,
            receiveRequest.amount(),
            receiveRequest.keyCertificate(),
            receiveRequest.outputCommitment());
    final InMemoryOfflineNoteStore assetOwnerSubstitutionStore = new InMemoryOfflineNoteStore();
    assetOwnerSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet assetOwnerSubstitutionSender =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            assetOwnerSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x21), filledBytes(32, 0x22))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 3,
            new StaticOwnerCertificateSigner(senderCertificate));
    assertThrows(
        () -> assetOwnerSubstitutionSender.pay(assetOwnerSubstitution),
        "receive request asset owner substitution should fail certificate binding");

    final InMemoryOfflineNoteStore forgedInputStore = new InMemoryOfflineNoteStore();
    forgedInputStore.upsert(sourceWalletNote(fixture, tamperedSignatureCertificate(senderCertificate)));
    final OfflineNoteWallet forgedInputWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            forgedInputStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_900L);
    assertThrows(
        () -> forgedInputWallet.pay(receiveRequest),
        "stored input with tampered certificate should fail");
    final InMemoryOfflineNoteStore wrongAccountInputStore = new InMemoryOfflineNoteStore();
    wrongAccountInputStore.upsert(sourceWalletNote(fixture, recipientCertificate));
    final OfflineNoteWallet wrongAccountInputWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            wrongAccountInputStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_910L);
    assertThrows(
        () -> wrongAccountInputWallet.pay(receiveRequest),
        "valid stored input certificate for the wrong account should fail");
    final InMemoryOfflineNoteStore commitmentSubstitutionStore = new InMemoryOfflineNoteStore();
    commitmentSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet commitmentSubstitutionSender =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            commitmentSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x31), filledBytes(32, 0x32))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 1,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteReceiveRequest commitmentSubstitution =
        new OfflineNoteReceiveRequest(
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
    final InMemoryOfflineNoteStore amountSubstitutionStore = new InMemoryOfflineNoteStore();
    amountSubstitutionStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final OfflineNoteWallet amountSubstitutionSender =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            amountSubstitutionStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x41), filledBytes(32, 0x42))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> longValue(payment, "created_at_ms") + 2,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteReceiveRequest amountSubstitution =
        new OfflineNoteReceiveRequest(
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

    final OfflineNotePaymentToken token = senderWallet.pay(receiveRequest);
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
        OfflineNoteWalletNoteState.SPENDABLE.name(),
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
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(sourceWalletNote(fixture, senderCertificate));
    final Map<String, OfflineNoteWalletNoteState> senderResolutions = new LinkedHashMap<>();
    senderResolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteWalletNoteState.SPENT);
    senderResolutions.put(
        string(derivation, "change_output_commitment"), OfflineNoteWalletNoteState.SPENDABLE);
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            new RecordingSyncResolver(senderResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_400L,
            new StaticOwnerCertificateSigner(senderCertificate));
    final InMemoryOfflineNoteStore recipientStore = new InMemoryOfflineNoteStore();
    final Map<String, OfflineNoteWalletNoteState> recipientResolutions = new LinkedHashMap<>();
    recipientResolutions.put(
        string(derivation, "recipient_output_commitment"), OfflineNoteWalletNoteState.CANCELLED);
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            recipientStore,
            null,
            new RejectingTransactionSubmitter(),
            new RecordingSyncResolver(recipientResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Collections.singletonList(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_500L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));
    final OfflineNotePaymentToken token = senderWallet.pay(receiveRequest);

    final OfflineNoteWalletNote accepted = recipientWallet.accept(token);
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
    assertFutureFails(recipientWallet.publishAudit(token), "failed audit submit should fail publish");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment"))).state().name(),
        "failed audit leaves accepted note spendable");

    senderWallet.sync().get();
    recipientWallet.sync().get();

    assertEquals(
        OfflineNoteWalletNoteState.SPENT.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "failed audit leaves input spent");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "failed audit leaves change spendable");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment"))).state().name(),
        "failed audit leaves recipient spendable");

    final InMemoryOfflineNoteStore redeemStore = new InMemoryOfflineNoteStore();
    final OfflineNoteWalletNote redeemNote = sourceWalletNote(fixture, senderCertificate);
    redeemStore.upsert(redeemNote);
    final Map<String, OfflineNoteWalletNoteState> redeemResolutions = new LinkedHashMap<>();
    redeemResolutions.put(
        string(derivation, "source_note_commitment"), OfflineNoteWalletNoteState.SPENDABLE);
    final OfflineNoteWallet redeemWallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            redeemStore,
            null,
            new RejectingTransactionSubmitter(),
            new RecordingSyncResolver(redeemResolutions),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_002_600L);

    assertFutureFails(redeemWallet.redeem(redeemNote), "failed redeem submit should fail redeem");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        redeemStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "failed redeem leaves note spendable");

    redeemWallet.sync().get();

    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
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
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final OfflineNoteWalletNote redeemPending =
        new OfflineNoteWalletNote(
            string(derivation, "chain_id"),
            string(payment, "recipient_account_id"),
            string(issueVector, "asset_id"),
            string(redeemVector, "amount"),
            recipientCertificate,
            redeem.sourceNoteCommitment(),
            hexBytes(string(derivation, "recipient_note_secret_hex")),
            new OfflineNote.CommitmentOrigin.P2pOutput(
                string(derivation, "payment_request_id"), 0),
            OfflineNoteWalletNoteState.REDEEM_PENDING,
            1_700_000_003_100L,
            1_700_000_003_100L);

    final OfflineNoteOutcomeIndex committed =
        OfflineNoteOutcomeIndex.fromExplorerOutcomes(
            List.of(
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_AUDIT,
                    "Committed",
                    "audit-tx",
                    rawInstructionPair(
                        OfflineNote.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.auditInstruction(audit)))),
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_REDEEM,
                    "Committed",
                    "redeem-tx",
                    rawInstructionPair(
                        OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.redeemInstruction(redeem))))));
    assertTrue(
        committed.resolve(sourceWalletNote(fixture, certificate(obj(payment, "sender_key_certificate"))))
            == null,
        "audit outcomes do not mutate local-final notes");
    assertEquals(
        OfflineNoteWalletNoteState.REDEEMED.name(),
        committed.resolve(redeemPending).state().name(),
        "committed redeem");

    final OfflineNoteOutcomeIndex rejected =
        new OfflineNoteOutcomeIndex()
            .recordRejectedAudit(audit, "audit-rejected")
            .recordRejectedRedeem(redeem, "redeem-rejected");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        rejected.resolve(redeemPending).state().name(),
        "rejected redeem");
  }

  private static OfflineNote.Issue issue(final Map<String, Object> fixture) {
    final Map<String, Object> chainIssue = obj(obj(fixture, "chain_vectors"), "issue");
    return new OfflineNote.Issue(
        hexBytes(string(chainIssue, "note_commitment")),
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
        string(chainIssue, "asset_id"),
        string(chainIssue, "amount"));
  }

  private static OfflineNote.Redeem redeem(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new OfflineNote.Redeem(
        hexBytes(string(vector, "source_note_commitment")),
        hexList(vector, "input_nullifiers"),
        certificate(obj(payment, "recipient_key_certificate")),
        string(payment, "recipient_account_id"),
        string(vector, "asset_id"),
        string(vector, "amount"),
        new OfflineNote.RecursiveProof(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNote.ProofBox(
                OfflineNote.RECURSIVE_BACKEND,
                "offline-vector-redeem-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNote.AuditBundle audit(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "audit");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final List<OfflineNote.IssuedClaim> inputClaims = new ArrayList<>();
    for (final Object item : list(payment, "input_claims")) {
      inputClaims.add(issuedClaim(asMap(item, "input claim")));
    }
    final List<OfflineNote.AuditOutputClaim> outputClaims = new ArrayList<>();
    for (final Object item : list(payment, "output_claims")) {
      outputClaims.add(auditOutputClaim(asMap(item, "output claim")));
    }
    return new OfflineNote.AuditBundle(
        hexBytes(string(vector, "token_id")),
        certificate(obj(payment, "sender_key_certificate")),
        hexList(vector, "input_nullifiers"),
        inputClaims,
        hexList(vector, "output_commitments"),
        outputClaims,
        new OfflineNote.RecursiveProof(
            hexBytes(string(vector, "public_inputs_hash")),
            new OfflineNote.ProofBox(
                OfflineNote.RECURSIVE_BACKEND,
                "offline-vector-audit-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNote.AuditBundle ancestorAuditProducingFirstInput(
      final OfflineNote.AuditBundle child, final int seed) {
    final OfflineNote.IssuedClaim childInput = child.inputClaims().get(0);
    final OfflineNote.IssuedClaim parentInput =
        new OfflineNote.IssuedClaim(
            filledBytes(32, seed | 1),
            child.senderKeyCertificate().payloadHash(),
            childInput.assetId(),
            childInput.amount());
    final OfflineNote.AuditOutputClaim output =
        new OfflineNote.AuditOutputClaim(
            childInput.noteCommitment(),
            child.senderKeyCertificate(),
            childInput.assetId(),
            childInput.amount());
    final byte[] tokenId = filledBytes(32, (seed + 2) | 1);
    final List<byte[]> inputNullifiers =
        Collections.singletonList(filledBytes(32, (seed + 4) | 1));
    final List<byte[]> outputCommitments =
        Collections.singletonList(childInput.noteCommitment());
    final OfflineNote.AuditPublicInputs auditPublicInputs =
        new OfflineNote.AuditPublicInputs(
            tokenId,
            child.senderKeyCertificate().payloadHash(),
            inputNullifiers,
            Collections.singletonList(parentInput),
            outputCommitments,
            Collections.singletonList(output.issuedClaim()));
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            tokenId,
            child.senderKeyCertificate(),
            inputNullifiers,
            Collections.singletonList(parentInput),
            outputCommitments,
            Collections.singletonList(output),
            new OfflineNote.RecursiveProof(
                auditPublicInputs.publicInputsHash(),
                new OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "ancestor-audit-provisional".getBytes(StandardCharsets.UTF_8))));
    return draft.replacingRecursiveProof(
        new OfflineNote.RecursiveProof(
            draft.publicInputsHash(),
            new OfflineNote.ProofBox(
                OfflineNote.RECURSIVE_BACKEND,
                "ancestor-audit-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static OfflineNote.KeyCertificate certificate(final Map<String, Object> json) {
    return new OfflineNote.KeyCertificate(
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

  private static OfflineNoteCertificateVerifier certificateVerifier(
      final Map<String, Object> fixture) {
    return new Ed25519OfflineNoteCertificateVerifier(
        Collections.singletonList(base64Bytes(string(fixture, "offline_fi_public_key_base64"))));
  }

  private static OfflineNoteCertificateVerifier fixtureOwnerCertificateVerifier(
      final Map<String, Object> fixture) {
    final OfflineNoteCertificateVerifier delegate = certificateVerifier(fixture);
    return new OfflineNoteCertificateVerifier() {
      @Override
      public boolean verifyIssuerCertificate(final OfflineNote.KeyCertificate certificate) {
        return delegate.verifyIssuerCertificate(certificate);
      }

      @Override
      public boolean verifyOwnerCertificate(final OfflineNote.KeyCertificate certificate) {
        return delegate.verifyOwnerCertificate(certificate)
            || delegate.verifyIssuerCertificate(certificate);
      }
    };
  }

  private static OfflineNote.KeyCertificate tamperedSignatureCertificate(
      final OfflineNote.KeyCertificate certificate) {
    final byte[] signature = certificate.issuerSignature();
    signature[0] = (byte) (signature[0] ^ 0x01);
    return new OfflineNote.KeyCertificate(
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

  private static OfflineNotePaymentToken paymentTokenReplacingFirstOutputCertificate(
      final OfflineNotePaymentToken token,
      final OfflineNote.KeyCertificate certificate) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNote.AuditOutputClaim output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNote.AuditOutputClaim(
            output.noteCommitment(), certificate, output.assetId(), output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNotePaymentToken paymentTokenReplacingFirstOutputAmount(
      final OfflineNotePaymentToken token,
      final String amount) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNote.AuditOutputClaim output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNote.AuditOutputClaim(
            output.noteCommitment(), output.keyCertificate(), output.assetId(), amount));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNotePaymentToken paymentTokenReplacingFirstOutputAmountWithoutProofRebind(
      final OfflineNotePaymentToken token,
      final String amount) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNote.AuditOutputClaim output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNote.AuditOutputClaim(
            output.noteCommitment(), output.keyCertificate(), output.assetId(), amount));
    return new OfflineNotePaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        new OfflineNote.AuditBundle(
            token.audit().tokenId(),
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            outputClaims,
            token.audit().recursiveProof()),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingFirstOutputAsset(
      final OfflineNotePaymentToken token,
      final String assetId) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final OfflineNote.AuditOutputClaim output = outputClaims.get(0);
    outputClaims.set(
        0,
        new OfflineNote.AuditOutputClaim(
            output.noteCommitment(), output.keyCertificate(), assetId, output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNotePaymentToken paymentTokenReversingOutputs(
      final OfflineNotePaymentToken token) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final List<byte[]> outputCommitments = new ArrayList<>(token.audit().outputCommitments());
    Collections.reverse(outputClaims);
    Collections.reverse(outputCommitments);
    return paymentTokenReplacingOutputs(token, outputClaims, outputCommitments);
  }

  private static OfflineNotePaymentToken paymentTokenDroppingFirstOutput(
      final OfflineNotePaymentToken token) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(
            token.audit().outputClaims().subList(1, token.audit().outputClaims().size()));
    final List<byte[]> outputCommitments =
        new ArrayList<>(
            token.audit().outputCommitments().subList(1, token.audit().outputCommitments().size()));
    return paymentTokenReplacingOutputs(token, outputClaims, outputCommitments);
  }

  private static OfflineNotePaymentToken paymentTokenReplacingChainId(
      final OfflineNotePaymentToken token, final String chainId) {
    return new OfflineNotePaymentToken(
        chainId,
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        token.audit(),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingLastOutputCertificate(
      final OfflineNotePaymentToken token,
      final OfflineNote.KeyCertificate certificate) {
    final List<OfflineNote.AuditOutputClaim> outputClaims =
        new ArrayList<>(token.audit().outputClaims());
    final int index = outputClaims.size() - 1;
    final OfflineNote.AuditOutputClaim output = outputClaims.get(index);
    outputClaims.set(
        index,
        new OfflineNote.AuditOutputClaim(
            output.noteCommitment(), certificate, output.assetId(), output.amount()));
    return paymentTokenReplacingAuditClaims(token, token.audit().inputClaims(), outputClaims);
  }

  private static OfflineNotePaymentToken paymentTokenReplacingFirstInputClaimHash(
      final OfflineNotePaymentToken token, final byte[] keyCertificatePayloadHash) {
    final List<OfflineNote.IssuedClaim> inputClaims =
        new ArrayList<>(token.audit().inputClaims());
    final OfflineNote.IssuedClaim input = inputClaims.get(0);
    inputClaims.set(
        0,
        new OfflineNote.IssuedClaim(
            input.domain(),
            input.noteCommitment(),
            keyCertificatePayloadHash,
            input.assetId(),
            input.amount()));
    return paymentTokenReplacingAuditClaims(token, inputClaims, token.audit().outputClaims());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingSenderCertificate(
      final OfflineNotePaymentToken token,
      final OfflineNote.KeyCertificate certificate) {
    final byte[] certificateHash = certificate.payloadHash();
    final List<OfflineNote.IssuedClaim> inputClaims = new ArrayList<>();
    for (final OfflineNote.IssuedClaim input : token.audit().inputClaims()) {
      inputClaims.add(
          new OfflineNote.IssuedClaim(
              input.domain(),
              input.noteCommitment(),
              certificateHash,
              input.assetId(),
              input.amount()));
    }
    final byte[] tokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                token.chainId(),
                token.paymentRequestId(),
                token.createdAtMs(),
                token.tokenNonce(),
                certificateHash,
                token.audit().inputNullifiers(),
                token.audit().outputCommitments()));
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            tokenId,
            certificate,
            token.audit().inputNullifiers(),
            inputClaims,
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNote.RecursiveProof proof =
        new OfflineNote.RecursiveProof(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNotePaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        tokenId,
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingPaymentRequestId(
      final OfflineNotePaymentToken token, final String paymentRequestId) {
    final byte[] tokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                token.chainId(),
                paymentRequestId,
                token.createdAtMs(),
                token.tokenNonce(),
                token.audit().senderKeyCertificate().payloadHash(),
                token.audit().inputNullifiers(),
                token.audit().outputCommitments()));
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            tokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNote.RecursiveProof proof =
        new OfflineNote.RecursiveProof(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNotePaymentToken(
        token.chainId(),
        paymentRequestId,
        token.tokenNonce(),
        tokenId,
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingTopLevelTokenId(
      final OfflineNotePaymentToken token) {
    return new OfflineNotePaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        flippedHash(token.tokenId()),
        token.audit(),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingBearerAuditTrail(
      final OfflineNotePaymentToken token, final List<OfflineNote.AuditBundle> bearerAuditTrail) {
    return new OfflineNotePaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        token.audit(),
        bearerAuditTrail,
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingAuditTokenId(
      final OfflineNotePaymentToken token) {
    final byte[] auditTokenId = flippedHash(token.audit().tokenId());
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            auditTokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            token.audit().outputCommitments(),
            token.audit().outputClaims(),
            token.audit().recursiveProof());
    final OfflineNote.RecursiveProof proof =
        new OfflineNote.RecursiveProof(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNotePaymentToken(
        token.chainId(),
        token.paymentRequestId(),
        token.tokenNonce(),
        token.tokenId(),
        draft.replacingRecursiveProof(proof),
        token.createdAtMs());
  }

  private static OfflineNotePaymentToken paymentTokenReplacingOutputs(
      final OfflineNotePaymentToken token,
      final List<OfflineNote.AuditOutputClaim> outputClaims,
      final List<byte[]> outputCommitments) {
    final byte[] tokenId =
        OfflineNote.derivePaymentTokenId(
            new OfflineNote.PaymentTokenIdPreimage(
                token.chainId(),
                token.paymentRequestId(),
                token.createdAtMs(),
                token.tokenNonce(),
                token.audit().senderKeyCertificate().payloadHash(),
                token.audit().inputNullifiers(),
                outputCommitments));
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            tokenId,
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            token.audit().inputClaims(),
            outputCommitments,
            outputClaims,
            token.audit().recursiveProof());
    final OfflineNote.RecursiveProof proof =
        new OfflineNote.RecursiveProof(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNotePaymentToken(
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

  private static OfflineNotePaymentToken paymentTokenReplacingAuditClaims(
      final OfflineNotePaymentToken token,
      final List<OfflineNote.IssuedClaim> inputClaims,
      final List<OfflineNote.AuditOutputClaim> outputClaims) {
    final OfflineNote.AuditBundle draft =
        new OfflineNote.AuditBundle(
            token.audit().tokenId(),
            token.audit().senderKeyCertificate(),
            token.audit().inputNullifiers(),
            inputClaims,
            token.audit().outputCommitments(),
            outputClaims,
            token.audit().recursiveProof());
    final OfflineNote.RecursiveProof proof =
        new OfflineNote.RecursiveProof(
            token.audit().recursiveProof().verifierKeyId(),
            draft.publicInputsHash(),
            token.audit().recursiveProof().proof());
    return new OfflineNotePaymentToken(
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

  private static OfflineNote.IssuedClaim issuedClaim(final Map<String, Object> json) {
    return new OfflineNote.IssuedClaim(
        string(json, "domain"),
        hexBytes(string(json, "note_commitment")),
        hexBytes(string(json, "key_certificate_payload_hash")),
        string(json, "asset_id"),
        string(json, "amount"));
  }

  private static OfflineNote.AuditOutputClaim auditOutputClaim(
      final Map<String, Object> json) {
    return new OfflineNote.AuditOutputClaim(
        hexBytes(string(json, "note_commitment")),
        certificate(obj(json, "key_certificate")),
        string(json, "asset_definition_id") + "#" + string(json, "account_id"),
        string(json, "amount"));
  }

  private static OfflineNoteWalletNote sourceWalletNote(
      final Map<String, Object> fixture, final OfflineNote.KeyCertificate certificate) {
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    return new OfflineNoteWalletNote(
        string(derivation, "chain_id"),
        accountFromAssetId(string(issue, "asset_id")),
        string(issue, "asset_id"),
        string(issue, "amount"),
        certificate,
        hexBytes(string(derivation, "source_note_commitment")),
        hexBytes(string(derivation, "source_note_secret_hex")),
        new OfflineNote.CommitmentOrigin.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision")),
        OfflineNoteWalletNoteState.SPENDABLE,
        1_700_000_000_000L,
        1_700_000_000_000L);
  }

  private static final class StaticAttestationProvider implements OfflineNoteAttestationProvider {
    private final OfflineNote.KeyCertificate certificate;

    private StaticAttestationProvider(final OfflineNote.KeyCertificate certificate) {
      this.certificate = certificate;
    }

    @Override
    public OfflineNote.KeyCertificate currentKeyCertificate() {
      return certificate;
    }
  }

  private static final class StaticOwnerCertificateSigner
      implements OfflineNoteOwnerCertificateSigner {
    private final OfflineNote.KeyCertificate certificate;

    private StaticOwnerCertificateSigner(final OfflineNote.KeyCertificate certificate) {
      this.certificate = certificate;
    }

    @Override
    public OfflineNote.KeyCertificate freshOwnerCertificate(final String accountId) {
      return certificate;
    }
  }

  private static final class QueueRandomSource implements OfflineNoteRandomSource {
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

  private static final class FixedIdGenerator implements OfflineNoteIdGenerator {
    private final String id;

    private FixedIdGenerator(final String id) {
      this.id = id;
    }

    @Override
    public String nextId(final String prefix) {
      return id;
    }
  }

  private static final class SequenceIdGenerator implements OfflineNoteIdGenerator {
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
        case "/v1/offline/keys/refill" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("lineage_state", lineageState(0, "0"));
          response.put("key_certificate", certificateWithExpiry());
          response.put("key_certificates", List.of(certificateWithExpiry()));
        }
        case "/v1/offline/notes/issue" -> {
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

  private enum BindingProofProvider implements OfflineNoteProofProvider {
    INSTANCE;

    @Override
    public OfflineNote.RecursiveProof proveAudit(final OfflineNote.AuditBundle audit) {
      return new OfflineNote.RecursiveProof(
          audit.publicInputsHash(),
          new OfflineNote.ProofBox(
              OfflineNote.RECURSIVE_BACKEND,
              "wallet-audit-proof".getBytes(StandardCharsets.UTF_8)));
    }

    @Override
    public OfflineNote.RecursiveProof proveRedeem(final OfflineNote.Redeem redemption) {
      return new OfflineNote.RecursiveProof(
          redemption.publicInputsHash(),
          new OfflineNote.ProofBox(
              OfflineNote.RECURSIVE_BACKEND,
              "wallet-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    }
  }

  private enum BindingProofVerifier implements OfflineNoteProofVerifier {
    INSTANCE;

    @Override
    public boolean verifyAudit(final OfflineNote.AuditBundle audit) {
      return Arrays.equals(audit.recursiveProof().publicInputsHash(), audit.publicInputsHash());
    }

    @Override
    public boolean verifyRedeem(final OfflineNote.Redeem redemption) {
      return Arrays.equals(
          redemption.recursiveProof().publicInputsHash(), redemption.publicInputsHash());
    }
  }

  private static final class RecordingIssuerClient implements OfflineNoteIssuerClient {
    private final OfflineNoteLoadContext loadContext;
    private OfflineNoteIssueRequest lastIssueRequest;

    private RecordingIssuerClient(final OfflineNoteLoadContext loadContext) {
      this.loadContext = loadContext;
    }

    @Override
    public CompletableFuture<OfflineNoteLoadContext> prepareLoad(
        final String chainId,
        final String accountId,
        final String assetDefinitionId,
        final String amount) {
      return CompletableFuture.completedFuture(loadContext);
    }

    @Override
    public CompletableFuture<OfflineNoteIssueResponse> issueNote(
        final OfflineNoteIssueRequest request) {
      lastIssueRequest = request;
      return CompletableFuture.completedFuture(
          new OfflineNoteIssueResponse(
              request.noteCommitment(),
              request.loadContext().operationId(),
              request.loadContext().lineageId(),
              request.loadContext().localRevision(),
              request.loadContext().keyCertificate(),
              "settlement-entry-hash"));
    }
  }

  private static final class CompletionControlledIssuerClient implements OfflineNoteIssuerClient {
    private final OfflineNoteLoadContext loadContext;
    private final CountDownLatch issueRequested = new CountDownLatch(1);
    private final CompletableFuture<OfflineNoteIssueResponse> issueFuture =
        new CompletableFuture<>();
    private volatile OfflineNoteIssueRequest lastIssueRequest;

    private CompletionControlledIssuerClient(final OfflineNoteLoadContext loadContext) {
      this.loadContext = loadContext;
    }

    @Override
    public CompletableFuture<OfflineNoteLoadContext> prepareLoad(
        final String chainId,
        final String accountId,
        final String assetDefinitionId,
        final String amount) {
      return CompletableFuture.completedFuture(loadContext);
    }

    @Override
    public CompletableFuture<OfflineNoteIssueResponse> issueNote(
        final OfflineNoteIssueRequest request) {
      lastIssueRequest = request;
      issueRequested.countDown();
      return issueFuture;
    }
  }

  private static final class SynchronouslyThrowingIssuerClient implements OfflineNoteIssuerClient {
    private final OfflineNoteLoadContext loadContext;

    private SynchronouslyThrowingIssuerClient(final OfflineNoteLoadContext loadContext) {
      this.loadContext = loadContext;
    }

    @Override
    public CompletableFuture<OfflineNoteLoadContext> prepareLoad(
        final String chainId,
        final String accountId,
        final String assetDefinitionId,
        final String amount) {
      return CompletableFuture.completedFuture(loadContext);
    }

    @Override
    public CompletableFuture<OfflineNoteIssueResponse> issueNote(
        final OfflineNoteIssueRequest request) {
      throw new IllegalStateException("issuer exploded");
    }
  }

  private static final class BlockingOfflineNoteStore implements OfflineNoteStore {
    private final Map<String, OfflineNoteWalletNote> notes = new LinkedHashMap<>();
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

  private static final class RecordingTransactionSubmitter implements OfflineNoteTransactionSubmitter {
    private final List<OfflineNote.AuditBundle> audits = new ArrayList<>();
    private final List<OfflineNote.Redeem> redemptions = new ArrayList<>();
    private final List<DefundSubmission> defunds = new ArrayList<>();

    @Override
    public CompletableFuture<ClientResponse> submitAudit(final OfflineNote.AuditBundle audit) {
      audits.add(audit);
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitRedeem(final OfflineNote.Redeem redemption) {
      redemptions.add(redemption);
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitDefund(
        final OfflineNote.Redeem redemption,
        final List<OfflineNote.AuditBundle> bearerAuditTrail) {
      defunds.add(new DefundSubmission(redemption, bearerAuditTrail));
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }
  }

  private static final class CapturingIrohaClient implements IrohaClient {
    private SignedTransaction submittedTransaction;

    @Override
    public CompletableFuture<ClientResponse> submitTransaction(final SignedTransaction transaction) {
      submittedTransaction = transaction;
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }
  }

  private static final class FakeSigner implements Signer {
    @Override
    public byte[] sign(final byte[] message) throws SigningException {
      final byte[] suffix = "-signature".getBytes(StandardCharsets.UTF_8);
      final byte[] combined = new byte[message.length + suffix.length];
      System.arraycopy(message, 0, combined, 0, message.length);
      System.arraycopy(suffix, 0, combined, message.length, suffix.length);
      return combined;
    }

    @Override
    public byte[] publicKey() {
      return "fake-public-key".getBytes(StandardCharsets.UTF_8);
    }

    @Override
    public String algorithm() {
      return "Ed25519";
    }
  }

  private static final class RejectingTransactionSubmitter implements OfflineNoteTransactionSubmitter {
    @Override
    public CompletableFuture<ClientResponse> submitAudit(final OfflineNote.AuditBundle audit) {
      return CompletableFuture.completedFuture(new ClientResponse(409, new byte[0], "rejected"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitRedeem(final OfflineNote.Redeem redemption) {
      return CompletableFuture.completedFuture(new ClientResponse(409, new byte[0], "rejected"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitDefund(
        final OfflineNote.Redeem redemption,
        final List<OfflineNote.AuditBundle> bearerAuditTrail) {
      return CompletableFuture.completedFuture(new ClientResponse(409, new byte[0], "rejected"));
    }
  }

  private static final class PendingDefundTransactionSubmitter implements OfflineNoteTransactionSubmitter {
    private final List<DefundSubmission> defunds = new ArrayList<>();
    private final CompletableFuture<ClientResponse> pending = new CompletableFuture<>();

    @Override
    public CompletableFuture<ClientResponse> submitAudit(final OfflineNote.AuditBundle audit) {
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitRedeem(final OfflineNote.Redeem redemption) {
      return CompletableFuture.completedFuture(new ClientResponse(202, new byte[0], "accepted"));
    }

    @Override
    public CompletableFuture<ClientResponse> submitDefund(
        final OfflineNote.Redeem redemption,
        final List<OfflineNote.AuditBundle> bearerAuditTrail) {
      defunds.add(new DefundSubmission(redemption, bearerAuditTrail));
      return pending;
    }

    private void completeAccepted() {
      pending.complete(new ClientResponse(202, new byte[0], "accepted"));
    }
  }

  private static final class DefundSubmission {
    private final OfflineNote.Redeem redemption;
    private final List<OfflineNote.AuditBundle> bearerAuditTrail;

    private DefundSubmission(
        final OfflineNote.Redeem redemption,
        final List<OfflineNote.AuditBundle> bearerAuditTrail) {
      this.redemption = redemption;
      this.bearerAuditTrail = new ArrayList<>(bearerAuditTrail);
    }
  }

  private static final class RecordingSyncResolver implements OfflineNoteSyncResolver {
    private final Map<String, OfflineNoteWalletNoteState> resolutions;
    private final List<String> resolvedCommitments = new ArrayList<>();

    private RecordingSyncResolver(final Map<String, OfflineNoteWalletNoteState> resolutions) {
      this.resolutions = resolutions;
    }

    @Override
    public CompletableFuture<OfflineNoteSyncResolution> resolvePendingNote(
        final OfflineNoteWalletNote note) {
      final String commitment = note.noteCommitmentHex();
      resolvedCommitments.add(commitment);
      final OfflineNoteWalletNoteState state = resolutions.get(commitment);
      return CompletableFuture.completedFuture(
          state == null ? null : new OfflineNoteSyncResolution(state, "tx-" + commitment));
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
      final Path candidate = cursor.resolve("fixtures/offline/interop_contract.json");
      if (Files.exists(candidate)) {
        final String json = Files.readString(candidate);
        return (Map<String, Object>) JsonParser.parse(json);
      }
      cursor = cursor.getParent();
    }
    throw new AssertionError("fixtures/offline/interop_contract.json was not found");
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

  private static byte[] kagemushaNoritoFrame(final int schemaByte) {
    final byte[] frame = new byte[40];
    frame[0] = (byte) 'N';
    frame[1] = (byte) 'R';
    frame[2] = (byte) 'T';
    frame[3] = (byte) '0';
    Arrays.fill(frame, 6, 22, (byte) schemaByte);
    return frame;
  }

  private static byte[] kagemushaNoritoFrameWithPayload(final int schemaByte) {
    final byte[] frame = new byte[45];
    System.arraycopy(kagemushaNoritoFrame(schemaByte), 0, frame, 0, 40);
    frame[23] = 3;
    final byte[] crc = new byte[] {
      (byte) 0xb9,
      (byte) 0xd3,
      (byte) 0xa8,
      0x0c,
      (byte) 0xcd,
      0x5d,
      0x13,
      0x24
    };
    System.arraycopy(crc, 0, frame, 31, crc.length);
    frame[42] = (byte) 0xa5;
    frame[43] = 0x5a;
    frame[44] = 0x11;
    return frame;
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

  private static void assertIllegalArgumentContains(
      final Runnable action, final String expectedMessage) {
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      assertTrue(
          expected.getMessage().contains(expectedMessage),
          "expected IllegalArgumentException to contain: " + expectedMessage);
      return;
    }
    throw new AssertionError("expected IllegalArgumentException: " + expectedMessage);
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

  private static Throwable assertFutureFailsWithin(
      final CompletableFuture<?> future, final String message) {
    try {
      future.get(5, TimeUnit.SECONDS);
    } catch (final InterruptedException ex) {
      Thread.currentThread().interrupt();
      throw new AssertionError(message, ex);
    } catch (final TimeoutException ex) {
      throw new AssertionError(message + ": future timed out", ex);
    } catch (final ExecutionException ex) {
      return ex.getCause();
    }
    throw new AssertionError(message);
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
