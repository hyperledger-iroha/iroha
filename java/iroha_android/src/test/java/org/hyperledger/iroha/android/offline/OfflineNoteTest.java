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
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.Function;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.address.AccountAddress;
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
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class OfflineNoteTest {

  private OfflineNoteTest() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    keyCertificatesRequireOneUseHardwareLimitWhenPresent();
    offlineNoteModelsMatchRustNoritoVectors();
    publicNoritoDecodersRoundTripFixturePayloads();
    publicNoritoInstructionDecodersReadExplorerEnvelopeBytes();
    publicNoritoInstructionWrappersContainBareModelPayloads();
    walletDerivationsMatchRustVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    proofBindingRejectsRecursiveMetadataSubstitution();
    recursiveProofMetadataRejectsPaddedAndMalformedVerifierKeys();
    commitmentOriginIdsRejectSurroundingWhitespace();
    derivationPreimageIdsRejectSurroundingWhitespace();
    assetScopeDataspaceIdsRejectNonCanonicalForms();
    issuedClaimsRejectNonCanonicalAssetIds();
    issuedClaimsRejectNonCanonicalAmounts();
    offlineNoteDomainsRejectSubstitutionAndPadding();
    instanceValuesMatchRustVectors();
    auditInstanceValuesRejectUnanchoredClaimsAndHiddenOutputs();
    kagemushaRecordBackedNativeProverValidatesInput();
    kagemushaCompactNativeInputCopiesBeforeDispatch();
    kagemushaRecursiveAggregationNativeProverValidatesInput();
    kagemushaRecursiveSpendNativeProverValidatesInput();
    verifyingKeyBoxStandaloneCodecDecodesAndRejectsMalformedArchives();
    kagemushaNativeProversRejectMissingAndEmptyNativeOutputs();
    kagemushaNativeAvailabilityRequiresJniEntrypoint();
    kagemushaRecursiveAggregationNativeAvailabilityRequiresJniEntrypoint();
    chainVkOfflineNoteProofWrappersValidateInputs();
    openVerifyEnvelopeRejectsNonExactPublicInputHashBeforeDecoding();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
    decodeExternalPaymentTextWhenRequested();
    paymentTokenCodecRoundTripsNoritoTextAndQrFrames();
    offlineBearerCashPolicyAndPrefixesUseSingleAppSurface();
    offlineBearerCashPolicyRejectsNonPositiveAndInvertedLimits();
    receiveRequestCodecRoundTripsNoritoTextAndQrFrames();
    receiptAckCodecRoundTripsNoritoTextAndQrFrames();
    receiptAckCodecRejectsNonPositiveAcceptedAtDecode();
    receiptAckRejectsPaddedIdentifiers();
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
    walletNoteJsonCodecRejectsNonCanonicalAmount();
    walletNoteJsonCodecRejectsNonExactCommitmentHex();
    walletNoteJsonCodecRejectsNonExactIntegerFields();
    walletNoteScopeIdsRejectSurroundingWhitespace();
    walletLoadDerivesCommitmentBeforeIssuerSubmission();
    walletRejectsNonPositiveLoadAmounts();
    walletCanonicalizesLoadAndReceiveAmountsAndRejectsMalformedAmounts();
    walletLoadDoesNotBlockIssuerCompletionThread();
    walletLoadCompletesExceptionallyWhenIssuerThrowsSynchronously();
    toriiIssuerClientBodySignsRefillAndRetiresNoteIssue();
    toriiIssuerClientSubmitsKagemushaTopUpArchive();
    toriiIssuerDeviceBindingRejectsRetiredAssertionPublicKeyAliases();
    toriiIssuerDeviceBindingRejectsWhitespaceNormalizedFields();
    toriiIssuerClientRejectsMalformedDeviceProofProviderOutput();
    toriiIssuerClientRejectsMalformedCertificateUsageLimits();
    toriiIssuerClientRejectsMalformedLineageState();
    walletLifecycleBuildsAuditAcceptAndRedeemTransactions();
    walletRejectsAuditWhenRecursiveVerifierFails();
    walletRejectsPaymentsNeedingMoreThanFourInputs();
    walletRejectsNonPositiveReceiveAndPaymentAmounts();
    walletRejectsRedeemWhenRecursiveVerifierFails();
    offlineNoteTransactionSubmitterIsRetiredAndKeepsFeeMetadataHelper();
    walletSyncReconcilesPendingSpendChangeAndRedeemStates();
    walletRejectsDuplicateTokenAndAlreadyPendingInputs();
    walletRejectsExactAmountReceiveRequestReplayAfterRestart();
    walletRedeemReservesNoteBeforeSubmitCompletes();
    walletRejectsAdversarialCertificateBindings();
    walletSyncReconcilesFailedAuditAndRedeemOutcomes();
    outcomeIndexResolvesCommittedAndRejectedExplorerInstructions();
    outcomeProviderRequiresExactEncodedInstructionHex();
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

  private static void publicNoritoInstructionWrappersContainBareModelPayloads() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.Issue issue = issue(fixture);
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);

    assertBareInstructionWrapper(wirePayloadBytes(OfflineNote.issueInstruction(issue)));
    assertBareInstructionWrapper(wirePayloadBytes(OfflineNote.auditInstruction(audit)));
    assertBareInstructionWrapper(wirePayloadBytes(OfflineNote.redeemInstruction(redeem)));
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

  private static void recursiveProofMetadataRejectsPaddedAndMalformedVerifierKeys() {
    assertThrows(
        () ->
            new OfflineNote.VerifyingKeyIdReference(
                "  " + OfflineNote.RECURSIVE_BACKEND + "  ",
                OfflineNote.RECURSIVE_VERIFIER_NAME),
        "padded verifier backend must be rejected");
    assertThrows(
        () ->
            new OfflineNote.VerifyingKeyIdReference(
                OfflineNote.RECURSIVE_BACKEND,
                "  " + OfflineNote.RECURSIVE_VERIFIER_NAME + "  "),
        "padded verifier name must be rejected");
    final OfflineNote.VerifyingKeyIdReference verifier =
        new OfflineNote.VerifyingKeyIdReference(
            OfflineNote.RECURSIVE_BACKEND,
            OfflineNote.RECURSIVE_VERIFIER_NAME);
    assertEquals(
        OfflineNote.RECURSIVE_BACKEND,
        verifier.backend(),
        "verifier backend should be preserved exactly");
    assertEquals(
        OfflineNote.RECURSIVE_VERIFIER_NAME,
        verifier.name(),
        "verifier name should be preserved exactly");

    assertThrows(
        () ->
            new OfflineNote.ProofBox(
                "  " + OfflineNote.RECURSIVE_BACKEND + "  ",
                new byte[] {0x01}),
        "padded proof backend must be rejected");
    final OfflineNote.ProofBox proof =
        new OfflineNote.ProofBox(
            OfflineNote.RECURSIVE_BACKEND,
            new byte[] {0x01});
    assertEquals(
        OfflineNote.RECURSIVE_BACKEND,
        proof.backend(),
        "proof backend should be preserved exactly");

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

  private static void commitmentOriginIdsRejectSurroundingWhitespace() {
    final OfflineNote.CommitmentOrigin.IssuerLoad issuerLoad =
        new OfflineNote.CommitmentOrigin.IssuerLoad("operation-1", "lineage-1", 0L);
    assertEquals("operation-1", issuerLoad.operationId(), "issuer operation id");
    assertEquals("lineage-1", issuerLoad.lineageId(), "issuer lineage id");

    final OfflineNote.CommitmentOrigin.P2pOutput p2pOutput =
        new OfflineNote.CommitmentOrigin.P2pOutput("payment-1", 0);
    assertEquals("payment-1", p2pOutput.paymentRequestId(), "p2p payment request id");

    assertThrows(
        () -> new OfflineNote.CommitmentOrigin.IssuerLoad(" operation-1", "lineage-1", 0L),
        "padded operation id must be rejected");
    assertThrows(
        () -> new OfflineNote.CommitmentOrigin.IssuerLoad("operation-1\n", "lineage-1", 0L),
        "control-character operation id must be rejected");
    assertThrows(
        () -> new OfflineNote.CommitmentOrigin.IssuerLoad("operation-1", " lineage-1", 0L),
        "padded lineage id must be rejected");
    assertThrows(
        () -> new OfflineNote.CommitmentOrigin.P2pOutput("payment-1 ", 0),
        "padded payment request id must be rejected");
  }

  private static void derivationPreimageIdsRejectSurroundingWhitespace() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final String chainId = string(derivation, "chain_id");
    final String assetId = string(obj(chain, "issue"), "asset_id");
    final String paymentRequestId = string(derivation, "payment_request_id");
    final byte[] hash = filledBytes(32, 0x11);
    final OfflineNote.CommitmentOrigin.IssuerLoad origin =
        new OfflineNote.CommitmentOrigin.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            0L);

    new OfflineNote.NoteCommitmentPreimage(
        chainId, hash, assetId, "1", hash, origin);
    new OfflineNote.InputNullifierPreimage(chainId, hash, hash, hash);
    new OfflineNote.PaymentTokenIdPreimage(
        chainId, paymentRequestId, 1_700_000_000_000L, hash, hash, List.of(hash), List.of(hash));

    assertThrows(
        () ->
            new OfflineNote.NoteCommitmentPreimage(
                " " + chainId, hash, assetId, "1", hash, origin),
        "padded note commitment chain id must be rejected");
    assertThrows(
        () -> new OfflineNote.InputNullifierPreimage(chainId + "\n", hash, hash, hash),
        "control-character nullifier chain id must be rejected");
    assertThrows(
        () ->
            new OfflineNote.PaymentTokenIdPreimage(
                chainId,
                paymentRequestId + " ",
                1_700_000_000_000L,
                hash,
                hash,
                List.of(hash),
                List.of(hash)),
        "padded payment-token request id must be rejected");
  }

  private static void assetScopeDataspaceIdsRejectNonCanonicalForms() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final String chainId = string(derivation, "chain_id");
    final String assetId = string(obj(chain, "issue"), "asset_id");
    final byte[] hash = filledBytes(32, 0x11);
    final OfflineNote.CommitmentOrigin.IssuerLoad origin =
        new OfflineNote.CommitmentOrigin.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            0L);

    new OfflineNote.NoteCommitmentPreimage(
        chainId, hash, assetId + "#dataspace:0", "1", hash, origin);
    new OfflineNote.NoteCommitmentPreimage(
        chainId, hash, assetId + "#dataspace:1", "1", hash, origin);

    for (final String rejected :
        Arrays.asList(
            "dataspace:",
            "dataspace:+1",
            "dataspace:01",
            "dataspace:-1",
            "dataspace:1.0",
            "DATASPACE:1",
            "dataspace:9223372036854775808")) {
      assertThrows(
          () ->
              new OfflineNote.NoteCommitmentPreimage(
                  chainId, hash, assetId + "#" + rejected, "1", hash, origin),
          "non-canonical dataspace scope should reject: " + rejected);
    }
  }

  private static void issuedClaimsRejectNonCanonicalAssetIds() throws Exception {
    final OfflineNote.IssuedClaim claim = audit(loadFixture()).inputClaims().get(0);
    final String accountId = accountFromAssetId(claim.assetId());
    final String alternateAccountId =
        AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).address.toI105(1);
    final String nonCanonicalAssetId =
        assetDefinitionFromAssetId(claim.assetId()) + "#" + alternateAccountId;
    assertTrue(
        !claim.assetId().equals(nonCanonicalAssetId),
        "alternate account discriminant must produce a distinct asset id");
    assertEquals(
        claim.assetId(),
        OfflineNote.canonicalAssetId(nonCanonicalAssetId),
        "canonical issued-claim asset id");
    assertIllegalArgumentContains(
        () ->
            new OfflineNote.IssuedClaim(
                claim.noteCommitment(),
                claim.keyCertificatePayloadHash(),
                nonCanonicalAssetId,
                claim.amount()),
        "asset_id must be canonical");
  }

  private static void issuedClaimsRejectNonCanonicalAmounts() throws Exception {
    final OfflineNote.IssuedClaim claim = audit(loadFixture()).inputClaims().get(0);
    final String nonCanonicalAmount = "0" + claim.amount();
    assertTrue(
        !claim.amount().equals(nonCanonicalAmount),
        "padded amount must produce a distinct amount string");
    assertEquals(
        claim.amount(),
        OfflineNote.canonicalAmountString(nonCanonicalAmount),
        "canonical issued-claim amount");
    assertIllegalArgumentContains(
        () ->
            new OfflineNote.IssuedClaim(
                claim.noteCommitment(),
                claim.keyCertificatePayloadHash(),
                claim.assetId(),
                nonCanonicalAmount),
        "amount must be canonical");
  }

  private static void offlineNoteDomainsRejectSubstitutionAndPadding() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate certificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);
    final OfflineNote.IssuedClaim claim = audit.inputClaims().get(0);
    final OfflineNote.AuditPublicInputs auditPublic = audit.publicInputs();
    final OfflineNote.RedeemPublicInputs redeemPublic = redeem.publicInputs();

    assertThrows(
        () ->
            new OfflineNote.KeyCertificatePayload(
                OfflineNote.KEY_CERTIFICATE_PAYLOAD_DOMAIN + " ",
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
                certificate.oneUse()),
        "padded key-certificate payload domain must be rejected");
    assertThrows(
        () ->
            new OfflineNote.IssuedClaim(
                OfflineNote.ISSUED_CLAIM_DOMAIN + "\n",
                claim.noteCommitment(),
                claim.keyCertificatePayloadHash(),
                claim.assetId(),
                claim.amount()),
        "padded issued-claim domain must be rejected");
    assertThrows(
        () ->
            new OfflineNote.RedeemPublicInputs(
                "forged:" + OfflineNote.REDEEM_PUBLIC_INPUTS_DOMAIN,
                redeemPublic.sourceNoteCommitment(),
                redeemPublic.inputNullifiers(),
                redeemPublic.keyCertificatePayloadHash(),
                redeemPublic.recipient(),
                redeemPublic.assetId(),
                redeemPublic.amount()),
        "forged redeem-public-inputs domain must be rejected");
    assertThrows(
        () ->
            new OfflineNote.AuditPublicInputs(
                " " + OfflineNote.AUDIT_PUBLIC_INPUTS_DOMAIN,
                auditPublic.tokenId(),
                auditPublic.keyCertificatePayloadHash(),
                auditPublic.inputNullifiers(),
                auditPublic.inputClaims(),
                auditPublic.outputCommitments(),
                auditPublic.outputClaims()),
        "padded audit-public-inputs domain must be rejected");
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
    final String publicInputsHashHex = hex(proof.publicInputsHash());
    for (final String rejectedHash : nonExactPublicInputHashes(publicInputsHashHex)) {
      assertTrue(
          !OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(proof.proof().bytes(), rejectedHash),
          "Java Offline Halo2 SDK envelope helper rejects non-exact public input hash");
    }
    assertTrue(
        !OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
            proof.proof().bytes(), "0000000000000000000000000000000000000000000000000000000000000000"),
        "Java Offline Halo2 SDK envelope helper rejects a wrong public input hash");
  }

  private static void openVerifyEnvelopeRejectsNonExactPublicInputHashBeforeDecoding() {
    final String canonicalHash = repeat("ab", 32);
    for (final String rejectedHash : nonExactPublicInputHashes(canonicalHash)) {
      assertTrue(
          !OfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(new byte[0], rejectedHash),
          "Java Offline Halo2 helper rejects non-exact public input hash before decoding");
    }
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
            == KagemushaRecursiveSpendProver.Mode.RECURSIVE_COMPACT_V1,
        "recursive compact should be preferred when compact ABI is present");
    assertTrue(
        KagemushaRecursiveSpendProver.preferredMode(false, true)
            == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        "recursive Kagemusha spend should be preferred when available");
    assertTrue(
        KagemushaRecursiveSpendProver.preferredMode(false, false) == null,
        "no recursive Kagemusha bridge should expose no preferred production mode");
    for (final KagemushaRecursiveSpendProver.Mode mode : KagemushaRecursiveSpendProver.Mode.values()) {
      assertTrue(
          !"checked_prefold_v1".equals(mode.wireName()),
          "checked-prefold is not a first-release spend mode");
    }
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
        () -> VerifyingKeyBoxCodec.encodeNorito(" halo2/ipa ", new byte[] {1}),
        "padded verifying key backend should fail");
    assertThrows(
        () -> VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", new byte[0]),
        "empty verifying key bytes should fail");
    assertEquals(
        "recursive_spend_v1",
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName(),
        "recursive Kagemusha spend wire mode");
    assertEquals(
        6,
        KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
        "recursive Kagemusha spend requires native bridge ABI 6");
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

  private static void verifyingKeyBoxStandaloneCodecDecodesAndRejectsMalformedArchives() {
    final byte[] encoded = VerifyingKeyBoxCodec.encodeNorito("halo2/ipa", new byte[] {1, 2, 3});
    final VerifyingKeyBoxCodec.VerifyingKeyBox decoded =
        VerifyingKeyBoxCodec.decodeNorito(encoded);

    assertEquals("halo2/ipa", decoded.backend(), "decoded verifying-key backend");
    assertTrue(
        Arrays.equals(new byte[] {1, 2, 3}, decoded.bytes()),
        "decoded verifying-key bytes");
    final byte[] decodedBytes = decoded.bytes();
    decodedBytes[0] = 0x7f;
    assertTrue(
        Arrays.equals(new byte[] {1, 2, 3}, decoded.bytes()),
        "decoded verifying-key bytes must be defensive copies");
    assertTrue(
        Arrays.equals(encoded, VerifyingKeyBoxCodec.encodeNorito(decoded.backend(), decoded.bytes())),
        "decoded verifying-key box must re-encode canonically");

    assertIllegalArgumentContains(
        () -> VerifyingKeyBoxCodec.decodeNorito(
            rawVerifyingKeyBoxNorito(" halo2/ipa ", new byte[] {1})),
        "backend must not contain surrounding whitespace");
    assertIllegalArgumentContains(
        () -> VerifyingKeyBoxCodec.decodeNorito(
            rawVerifyingKeyBoxNorito("halo2/ipa", new byte[0])),
        "bytes must not be empty");
    assertIllegalArgumentContains(
        () ->
            VerifyingKeyBoxCodec.decodeNorito(
                rawVerifyingKeyBoxNoritoFields(
                    concat(encodeString("halo2/ipa", true), new byte[] {0}),
                    encodeBytesVec(new byte[] {1}))),
        "Trailing bytes after VerifyingKeyBox field decode");
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
            () -> {
              throw new IllegalArgumentException("bad library name");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "record-backed Offline Note prover fails closed on malformed library loading");
    assertTrue(
        !NativeOfflineNoteProver.detectNativeAvailability(
            () -> {
              throw new IllegalStateException("bad native loader");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "record-backed Offline Note prover fails closed on runtime loader errors");
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
    assertTrue(
        !NativeOfflineNoteProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new IllegalStateException("bad native probe");
            }),
        "record-backed Offline Note prover fails closed on runtime probe errors");

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
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {
              throw new IllegalStateException("bad native loader");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "native availability rejects runtime loader failures");
    assertTrue(
        !KagemushaCompactPaymentTokenProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new IllegalStateException("bad native probe");
            }),
        "native availability rejects runtime probe failures");
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
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {
              throw new IllegalStateException("bad native loader");
            },
            () -> {
              throw new AssertionError("probe must not run");
            }),
        "recursive aggregation availability rejects runtime loader failures");
    assertTrue(
        !KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
            () -> {},
            () -> {
              throw new IllegalStateException("bad native probe");
            }),
        "recursive aggregation availability rejects runtime probe failures");
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

  private static void decodeExternalPaymentTextWhenRequested() throws Exception {
    final String path = System.getenv("OFFLINE_NOTE_PAYMENT_TEXT_FILE");
    if (path == null || path.trim().isEmpty()) {
      return;
    }
    final String text = Files.readString(Paths.get(path.trim()), StandardCharsets.UTF_8).trim();
    final OfflineNotePaymentToken token = OfflineNotePaymentTokenCodec.decodeText(text);
    System.out.println(
        "offline_note_external_payment_text token_id="
            + token.tokenIdHex()
            + " bearer_audit_trail_count="
            + token.bearerAuditTrail().size());
    for (int index = 0; index < token.bearerAuditTrail().size(); index++) {
      final OfflineNote.AuditBundle audit = token.bearerAuditTrail().get(index);
      System.out.println(
          "offline_note_external_payment_text audit_index="
              + index
              + " token_id="
              + OfflineNoteWallet.hexLower(audit.tokenId())
              + " java_halo2_verified="
              + OfflineNoteHalo2Prover.verifyAudit(audit));
    }
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
    final String compactText =
        OfflineNotePaymentTokenCodec.TEXT_PREFIX
            + OfflineNotePaymentTokenCodec.COMPACT_TEXT_MARKER
            + text.substring(OfflineNotePaymentTokenCodec.TEXT_PREFIX.length());
    assertEquals(
        token.tokenIdHex(),
        OfflineNotePaymentTokenCodec.decodeText(compactText).tokenIdHex(),
        "compact marker payment token text token id");
    assertEquals(
        token.tokenIdHex(),
        OfflineBearerCashTextCodec.decodePaymentText(compactText).tokenIdHex(),
        "compact marker Bearer Cash text token id");
    final OfflineNotePaymentToken compactTokenWithoutTerminalAudit =
        new OfflineNotePaymentToken(
            token.chainId(),
            token.paymentRequestId(),
            token.tokenNonce(),
            token.tokenId(),
            token.audit(),
            Collections.emptyList(),
            token.createdAtMs());
    final String compactTextWithoutTerminalAudit =
        OfflineNotePaymentTokenCodec.TEXT_PREFIX
            + OfflineNotePaymentTokenCodec.COMPACT_TEXT_MARKER
            + Base64.getUrlEncoder()
                .withoutPadding()
                .encodeToString(
                    OfflineNotePaymentTokenCodec.encodeNorito(compactTokenWithoutTerminalAudit));
    final OfflineNotePaymentToken compactDecodedWithRestoredAudit =
        OfflineNotePaymentTokenCodec.decodeText(compactTextWithoutTerminalAudit);
    assertEquals(
        1L,
        compactDecodedWithRestoredAudit.bearerAuditTrail().size(),
        "compact marker payment token restores terminal bearer audit");
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteWallet.hexLower(
            compactDecodedWithRestoredAudit.bearerAuditTrail().get(0).tokenId()),
        "compact marker payment token restored audit token id");
    assertTrue(
        OfflineBearerCashPayloadKindV1.PAYMENT == OfflineBearerCashTextCodec.payloadKind(text),
        "payment text payload kind");
    assertTrue(
        OfflineBearerCashPayloadKindV1.PAYMENT == OfflineBearerCashTextCodec.payloadKind(compactText),
        "compact marker payment text payload kind");
    assertThrows(
        () -> OfflineNotePaymentTokenCodec.decodeText(" " + text),
        "leading-whitespace payment token text should reject");
    assertThrows(
        () -> OfflineNotePaymentTokenCodec.decodeText(text + "\n"),
        "trailing-whitespace payment token text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodePaymentText("\t" + text),
        "Bearer Cash payment text wrapper should reject leading whitespace");
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(" " + text) == null,
        "payment payload kind should reject leading whitespace");
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(" wallet-offline-bearer-cash-payment:AAAA")
            == null,
        "payment payload kind should reject whitespace-wrapped prefix");
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
    final String nonCanonicalPaymentText = OfflineNotePaymentTokenCodec.TEXT_PREFIX + "AB";
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(nonCanonicalPaymentText) == null,
        "payment payload kind should reject non-canonical base64url");
    assertThrows(
        () -> OfflineNotePaymentTokenCodec.decodeText(nonCanonicalPaymentText),
        "non-canonical payment token text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodePaymentText(nonCanonicalPaymentText),
        "Bearer Cash payment text wrapper should reject non-canonical base64url");

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

  private static void offlineBearerCashPolicyRejectsNonPositiveAndInvertedLimits() {
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(0, 32, 2048, 12288, 20, 8, 40),
        "maxCustodyHops must be positive");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(Integer.MIN_VALUE, 32, 2048, 12288, 20, 8, 40),
        "maxCustodyHops must be positive");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 0, 2048, 12288, 20, 8, 40),
        "maxLineageSteps must be positive");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 32, 0, 12288, 20, 8, 40),
        "maxSingleQrPayloadBytes must be positive");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 32, 2, 1, 20, 8, 40),
        "maxStreamPayloadBytes must cover maxSingleQrPayloadBytes");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 32, 2048, 12288, 20, 0, 40),
        "androidKeyPoolReplenishBelow must be positive");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 32, 2048, 12288, 7, 8, 40),
        "androidKeyPoolTarget must cover androidKeyPoolReplenishBelow");
    assertIllegalArgumentContains(
        () -> new OfflineBearerCashPolicyV1(5, 32, 2048, 12288, 20, 8, 19),
        "androidKeyPoolCap must cover androidKeyPoolTarget");

    final OfflineBearerCashPolicyV1 policy = OfflineBearerCashPolicyV1.DEFAULT;
    for (final int payloadByteCount : new int[] {0, -1, Integer.MIN_VALUE}) {
      assertIllegalArgumentContains(
          () -> policy.recommendedTransportForPayloadByteCount(payloadByteCount),
          "payloadByteCount must be positive");
    }
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
    assertTrue(
        OfflineBearerCashPayloadKindV1.RECEIVE_REQUEST
            == OfflineBearerCashTextCodec.payloadKind(text),
        "receive request text payload kind");
    assertThrows(
        () -> OfflineNoteReceiveRequestCodec.decodeText(" " + text),
        "leading-whitespace receive request text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodeReceiveRequestText(text + "\n"),
        "Bearer Cash receive request text wrapper should reject trailing whitespace");
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(text + " ") == null,
        "receive request payload kind should reject trailing whitespace");
    assertThrows(
        () -> OfflineNoteReceiveRequestCodec.decodeText(text + "="),
        "padded receive request text should reject");
    final String nonCanonicalReceiveRequestText =
        OfflineNoteReceiveRequestCodec.TEXT_PREFIX + "AB";
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(nonCanonicalReceiveRequestText) == null,
        "receive request payload kind should reject non-canonical base64url");
    assertThrows(
        () -> OfflineNoteReceiveRequestCodec.decodeText(nonCanonicalReceiveRequestText),
        "non-canonical receive request text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodeReceiveRequestText(nonCanonicalReceiveRequestText),
        "Bearer Cash receive request text wrapper should reject non-canonical base64url");

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
    assertTrue(
        OfflineBearerCashPayloadKindV1.ACK == OfflineBearerCashTextCodec.payloadKind(text),
        "receipt ACK text payload kind");
    assertThrows(
        () -> OfflineNoteReceiptAckCodec.decodeText(" " + text),
        "leading-whitespace receipt ACK text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodeAckText(text + "\t"),
        "Bearer Cash receipt ACK text wrapper should reject trailing whitespace");
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind("\n" + text) == null,
        "receipt ACK payload kind should reject leading newline");
    assertThrows(
        () -> OfflineNoteReceiptAckCodec.decodeText(text + "="),
        "padded receipt ACK text should reject");
    final String nonCanonicalAckText = OfflineNoteReceiptAckCodec.TEXT_PREFIX + "AB";
    assertTrue(
        OfflineBearerCashTextCodec.payloadKind(nonCanonicalAckText) == null,
        "receipt ACK payload kind should reject non-canonical base64url");
    assertThrows(
        () -> OfflineNoteReceiptAckCodec.decodeText(nonCanonicalAckText),
        "non-canonical receipt ACK text should reject");
    assertThrows(
        () -> OfflineBearerCashTextCodec.decodeAckText(nonCanonicalAckText),
        "Bearer Cash receipt ACK text wrapper should reject non-canonical base64url");

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

  private static void receiptAckRejectsPaddedIdentifiers() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(obj(fixture, "sdk_interop"), "payment_token_norito_base64")));
    final String recipientAccountId = string(payment, "recipient_account_id");
    final long acceptedAtMs = longValue(obj(fixture, "receipt_ack"), "accepted_at_ms");

    assertThrows(
        () ->
            new OfflineNoteReceiptAck(
                " " + token.chainId(),
                token.paymentRequestId(),
                token.tokenId(),
                recipientAccountId,
                acceptedAtMs),
        "padded receipt ACK chain id should fail");
    assertThrows(
        () ->
            new OfflineNoteReceiptAck(
                token.chainId(),
                token.paymentRequestId() + "\n",
                token.tokenId(),
                recipientAccountId,
                acceptedAtMs),
        "padded receipt ACK payment request id should fail");
    assertThrows(
        () ->
            new OfflineNoteReceiptAck(
                token.chainId(),
                token.paymentRequestId(),
                token.tokenId(),
                recipientAccountId + " ",
                acceptedAtMs),
        "padded receipt ACK recipient should fail");
    assertThrows(
        () -> OfflineNoteReceiptAck.fromPaymentToken(token, " " + recipientAccountId, acceptedAtMs),
        "padded receipt ACK recipient should fail before token matching");
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
        new OfflineNoteNearbyEnvelope.PairingChallenge("nearby_pairing_bird");
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
    final String challengeEnvelopeText =
        new String(challengeEnvelope.encoded(), StandardCharsets.UTF_8);
    assertTrue(
        challengeEnvelopeText.contains("\"pairingChallenge\":\"nearby_pairing_bird\""),
        "challenge pairing JSON");
    assertThrows(
        () ->
            OfflineNoteNearbyEnvelope.decode(
                challengeEnvelopeText
                    .replace(
                        "\"pairingChallenge\":\"nearby_pairing_bird\"",
                        "\"pairingChallenge\":\" nearby_pairing_bird\"")
                    .getBytes(StandardCharsets.UTF_8)),
        "padded nearby pairing string should fail");
    assertThrows(
        () ->
            OfflineNoteNearbyEnvelope.decode(
                challengeEnvelopeText
                    .replace(
                        "\"pairingChallenge\":\"nearby_pairing_bird\"",
                        "\"pairingChallenge\":{\"assetName\":\"nearby_pairing_bird \"}")
                    .getBytes(StandardCharsets.UTF_8)),
        "padded nearby pairing object should fail");
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
        () -> new OfflineNoteNearbyEnvelope.PairingChallenge(" nearby_pairing_mask"),
        "padded pairing asset should fail");
    assertThrows(
        () -> new OfflineNoteNearbyEnvelope.PairingChallenge("nearby_pairing_mask\n"),
        "newline-padded pairing asset should fail");
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
        ("{\"kind\":\"receive_request\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receipt-ack+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] ackContentTypeDowngrade =
        ("{\"kind\":\"receipt_ack\",\"payload\":\"b2s\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] paddedPayload =
        ("{\"kind\":\"receive_request\",\"payload\":\"YQ==\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] nonCanonicalPayload =
        ("{\"kind\":\"receive_request\",\"payload\":\"AB\","
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
    assertThrows(
        () -> OfflineNoteNearbyEnvelope.decode(nonCanonicalPayload),
        "non-canonical nearby envelope payload should fail");
    final byte[] topLevelArray = "[]".getBytes(StandardCharsets.UTF_8);
    final byte[] invalidBase64Payload =
        ("{\"kind\":\"receive_request\",\"payload\":\"!!!!\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":\"nearby_pairing_bird\"}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] badPairingObject =
        ("{\"kind\":\"receive_request\",\"payload\":\"YQ\","
                + "\"contentType\":\"application/vnd.iroha.offline.receive-request+norito\","
                + "\"pairingChallenge\":{\"assetName\":1}}")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] smuggledPairingObject =
        ("{\"kind\":\"receive_request\",\"payload\":\"YQ\","
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
    for (final String retiredState :
        Arrays.asList("spendPending", "SPEND_PENDING", "changePending", "CHANGE_PENDING")) {
      assertThrows(
          () ->
              OfflineNoteWalletNoteJsonCodec.decode(
                  encoded.replace("\"state\":\"SPENDABLE\"", "\"state\":\"" + retiredState + "\"")
                      .getBytes(StandardCharsets.UTF_8)),
          "retired " + retiredState + " wallet-note state should reject");
    }
  }

  private static void walletNoteJsonCodecRejectsNonCanonicalAmount() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    final String encoded =
        new String(OfflineNoteWalletNoteJsonCodec.encode(note), StandardCharsets.UTF_8);
    final String canonicalField = "\"amount\":\"" + note.canonicalAmount() + "\"";
    final String replacementField = "\"amount\":\"0" + note.canonicalAmount() + "\"";
    assertTrue(encoded.contains(canonicalField), "encoded wallet-note amount must be present");
    assertIllegalArgumentContains(
        () ->
            OfflineNoteWalletNoteJsonCodec.decode(
                encoded.replace(canonicalField, replacementField).getBytes(StandardCharsets.UTF_8)),
        "amount must be canonical");
  }

  private static void walletNoteJsonCodecRejectsNonExactCommitmentHex() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    final String encoded =
        new String(OfflineNoteWalletNoteJsonCodec.encode(note), StandardCharsets.UTF_8);
    final String canonicalCommitment = note.noteCommitmentHex();
    final String canonicalField = "\"note_commitment_hex\":\"" + canonicalCommitment + "\"";
    assertTrue(encoded.contains(canonicalField), "encoded note commitment field should be present");

    for (final String nonExactCommitment : nonExactLowerHex32(canonicalCommitment)) {
      assertThrows(
          () ->
              OfflineNoteWalletNoteJsonCodec.decode(
                  encoded
                      .replace(
                          canonicalField,
                          "\"note_commitment_hex\":\"" + nonExactCommitment + "\"")
                      .getBytes(StandardCharsets.UTF_8)),
          "non-exact wallet-note commitment should reject: " + nonExactCommitment);
    }
  }

  private static void walletNoteJsonCodecRejectsNonExactIntegerFields() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    final String encoded =
        new String(OfflineNoteWalletNoteJsonCodec.encode(note), StandardCharsets.UTF_8);
    final OfflineNote.CommitmentOrigin.IssuerLoad origin =
        (OfflineNote.CommitmentOrigin.IssuerLoad) note.origin();

    assertWalletNoteIntegerRejects(
        encoded, "\"version\":1", "\"version\":\"1\"", "quoted wallet-note version should reject");
    assertWalletNoteIntegerRejects(
        encoded, "\"version\":1", "\"version\":1.5", "fractional wallet-note version should reject");
    assertWalletNoteIntegerRejects(
        encoded,
        "\"created_at_ms\":" + note.createdAtMs(),
        "\"created_at_ms\":\"" + note.createdAtMs() + "\"",
        "quoted wallet-note created_at_ms should reject");
    assertWalletNoteIntegerRejects(
        encoded,
        "\"updated_at_ms\":" + note.updatedAtMs(),
        "\"updated_at_ms\":" + note.updatedAtMs() + ".5",
        "fractional wallet-note updated_at_ms should reject");
    assertWalletNoteIntegerRejects(
        encoded,
        "\"local_revision\":" + origin.localRevision(),
        "\"local_revision\":9223372036854775808",
        "overflow wallet-note origin local_revision should reject");
  }

  private static void assertWalletNoteIntegerRejects(
      final String encoded,
      final String canonicalField,
      final String replacementField,
      final String message) {
    assertTrue(
        encoded.contains(canonicalField),
        "encoded wallet-note integer field should be present: " + canonicalField);
    assertThrows(
        () ->
            OfflineNoteWalletNoteJsonCodec.decode(
                encoded.replace(canonicalField, replacementField).getBytes(StandardCharsets.UTF_8)),
        message);
  }

  private static void walletNoteScopeIdsRejectSurroundingWhitespace() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> derivation = obj(obj(fixture, "chain_vectors"), "derivation");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    final String spentPaymentRequestId = string(derivation, "payment_request_id");

    assertEquals(
        spentPaymentRequestId,
        walletNoteCopy(note, note.chainId(), note.accountId(), spentPaymentRequestId)
            .spentPaymentRequestId(),
        "canonical spent payment request id");
    assertThrows(
        () -> walletNoteCopy(note, " " + note.chainId(), note.accountId(), null),
        "padded wallet-note chain id should reject");
    assertThrows(
        () -> walletNoteCopy(note, note.chainId(), note.accountId() + "\n", null),
        "padded wallet-note account id should reject");
    assertThrows(
        () -> walletNoteCopy(note, note.chainId(), note.accountId(), spentPaymentRequestId + " "),
        "padded spent payment request id should reject");
    assertThrows(
        () -> walletNoteCopy(note, note.chainId(), note.accountId(), ""),
        "blank spent payment request id should reject");
  }

  private static OfflineNoteWalletNote walletNoteCopy(
      final OfflineNoteWalletNote note,
      final String chainId,
      final String accountId,
      final String spentPaymentRequestId) {
    return new OfflineNoteWalletNote(
        chainId,
        accountId,
        note.assetId(),
        note.canonicalAmount(),
        note.keyCertificate(),
        note.noteCommitment(),
        note.noteSecret(),
        note.origin(),
        note.bearerAuditTrail(),
        note.state(),
        note.createdAtMs(),
        note.updatedAtMs(),
        spentPaymentRequestId);
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

    final Throwable cause =
        assertFutureFailsWithin(
            wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount")),
            "retired wallet load should fail");
    assertEquals(
        ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
        cause.getMessage(),
        "retired wallet load message");
    assertEquals(0L, issuerClient.prepareLoadCount, "retired load issuer prepare count");
    assertTrue(issuerClient.lastIssueRequest == null, "retired load issue request");
  }

  private static void walletRejectsNonPositiveLoadAmounts() throws Exception {
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
    final InMemoryOfflineNoteStore store = new InMemoryOfflineNoteStore();
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(issue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            store,
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-positive-load"),
            () -> 1_700_000_012_180L);
    final String assetDefinitionId = assetDefinitionFromAssetId(string(issue, "asset_id"));

    for (final String invalidAmount : Arrays.asList("0", "-1")) {
      final Throwable cause =
          assertFutureFailsWithin(
              wallet.load(assetDefinitionId, invalidAmount), "nonpositive load should fail");
      assertTrue(
          cause instanceof IllegalStateException,
          "nonpositive retired load should fail with IllegalStateException");
      assertEquals(
          ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
          cause.getMessage(),
          "nonpositive load failure message");
      assertEquals(0L, issuerClient.prepareLoadCount, "nonpositive load issuer prepare count");
      assertTrue(issuerClient.lastIssueRequest == null, "nonpositive load issue request");
      assertEquals(0L, store.listNotes().size(), "nonpositive load note count");
    }
  }

  private static void walletCanonicalizesLoadAndReceiveAmountsAndRejectsMalformedAmounts()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final String chainId = string(derivation, "chain_id");
    final String assetDefinitionId = assetDefinitionFromAssetId(string(issue, "asset_id"));
    final OfflineNoteLoadContext loadContext =
        new OfflineNoteLoadContext(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision"),
            senderCertificate);
    final RecordingIssuerClient issuerClient = new RecordingIssuerClient(loadContext);
    final InMemoryOfflineNoteStore loadStore = new InMemoryOfflineNoteStore();
    final OfflineNoteWallet loadWallet =
        new OfflineNoteWallet(
            chainId,
            accountFromAssetId(string(issue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            loadStore,
            issuerClient,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.singletonList(filledBytes(32, 0x42))),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-canonical-load"),
            () -> 1_700_000_012_181L);

    final Throwable retiredLoad =
        assertFutureFailsWithin(
            loadWallet.load(assetDefinitionId, "001.2300"), "retired canonical load should fail");
    assertEquals(
        ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
        retiredLoad.getMessage(),
        "retired canonical load message");
    assertEquals(0L, issuerClient.prepareLoadCount, "retired load issuer prepare count");
    assertEquals(0L, loadStore.listNotes().size(), "retired load note count");

    final InMemoryOfflineNoteStore receiveStore = new InMemoryOfflineNoteStore();
    final OfflineNoteWallet receiveWallet =
        new OfflineNoteWallet(
            chainId,
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            receiveStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.singletonList(filledBytes(32, 0x43))),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-canonical-receive"),
            () -> 1_700_000_012_182L,
            new StaticOwnerCertificateSigner(recipientCertificate));
    final OfflineNoteReceiveRequest receiveRequest =
        receiveWallet.prepareReceive(assetDefinitionId, "+10");
    assertEquals("10", receiveRequest.amount(), "canonical receive request amount");
    assertEquals("10", receiveRequest.canonicalAmount(), "canonical receive request amount accessor");
    assertEquals("10", receiveStore.listNotes().get(0).amount(), "canonical pending receive amount");

    for (final String nonCanonicalAmount : Arrays.asList("010", "+10")) {
      assertIllegalArgumentContains(
          () ->
              new OfflineNoteReceiveRequest(
                  receiveRequest.chainId(),
                  receiveRequest.paymentRequestId() + "-" + nonCanonicalAmount,
                  receiveRequest.accountId(),
                  receiveRequest.assetDefinitionId(),
                  receiveRequest.assetId(),
                  nonCanonicalAmount,
                  receiveRequest.keyCertificate(),
                  receiveRequest.outputCommitment()),
          "amount must be canonical");
    }
    final String alternateAccountId =
        AccountAddress.parseEncodedIgnoringCurveSupport(receiveRequest.accountId(), null)
            .address
            .toI105(1);
    final String nonCanonicalAssetId =
        assetDefinitionFromAssetId(receiveRequest.assetId()) + "#" + alternateAccountId;
    assertTrue(
        !receiveRequest.assetId().equals(nonCanonicalAssetId),
        "receive request noncanonical asset id differs");
    assertEquals(
        receiveRequest.assetId(),
        OfflineNote.canonicalAssetId(nonCanonicalAssetId),
        "receive request noncanonical asset id canonicalizes");
    assertIllegalArgumentContains(
        () ->
            new OfflineNoteReceiveRequest(
                receiveRequest.chainId(),
                receiveRequest.paymentRequestId() + "-asset",
                receiveRequest.accountId(),
                receiveRequest.assetDefinitionId(),
                nonCanonicalAssetId,
                receiveRequest.amount(),
                receiveRequest.keyCertificate(),
                receiveRequest.outputCommitment()),
        "asset_id must be canonical");

    for (final String invalidAmount : Arrays.asList(" 1", "1\n", "1e3", ".", "")) {
      assertIllegalArgumentContains(
          () -> receiveWallet.prepareReceive(assetDefinitionId, invalidAmount),
          "Offline Note payment amount");
      assertEquals(1L, receiveStore.listNotes().size(), "malformed receive note count");
      assertIllegalArgumentContains(
          () ->
              new OfflineNoteReceiveRequest(
                  receiveRequest.chainId(),
                  receiveRequest.paymentRequestId() + "-bad",
                  receiveRequest.accountId(),
                  receiveRequest.assetDefinitionId(),
                  receiveRequest.assetId(),
                  invalidAmount,
                  receiveRequest.keyCertificate(),
                  receiveRequest.outputCommitment()),
          "Offline Note payment amount");
    }
  }

  private static void toriiIssuerClientBodySignsRefillAndRetiresNoteIssue()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> certificateJson =
        currentIssuerCertificateJson(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
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
    final OfflineIssuerExecutor executor =
        new OfflineIssuerExecutor(certificateJson, "00".repeat(31) + "04");
    final List<byte[]> signedMessages = new ArrayList<>();
    final long[] nowMs = new long[] {1_700_000_000_000L};
    final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    defaultHeaders.put("X-Iroha-Account", "retired-account");
    defaultHeaders.put("x-iroha-signature", "retired-signature");
    defaultHeaders.put("X-IROHA-TIMESTAMP-MS", "123");
    defaultHeaders.put("X-Iroha-Nonce", "retired-nonce");
    defaultHeaders.put("X-Iroha-Witness", "retired-witness");
    defaultHeaders.put("X-Client-Trace", "trace-1");
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
              return deviceProofJson();
            },
            executor,
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            defaultHeaders,
            List.of(),
            () -> nowMs[0],
            new SequenceIdGenerator(
                "operation-refill-1",
                "auth-refill-1",
                "operation-refill-2",
                "auth-refill-2"));

    final OfflineNoteLoadContext context =
        client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").get();
    assertEquals("operation-refill-1", context.operationId(), "operation id");
    assertEquals("lineage-1", context.lineageId(), "lineage id");
    assertEquals(1L, context.localRevision(), "post-issue commitment revision");

    final byte[] commitment = new byte[32];
    for (int i = 0; i < commitment.length; i++) {
      commitment[i] = (byte) (i + 1);
    }
    final Throwable issueFailure =
        assertFutureFailsWithin(
            client.issueNote(
                new OfflineNoteIssueRequest(
                    "chain-1",
                    accountId,
                    assetDefinitionId,
                    assetDefinitionId + "#" + accountId,
                    "5",
                    context,
                    commitment)),
            "classic Offline Note issue must be retired");
    assertTrue(
        issueFailure instanceof IllegalStateException,
        "retired issue should fail with IllegalStateException");
    assertEquals(
        ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
        issueFailure.getMessage(),
        "retired issue message");
    assertEquals(1L, executor.requests.size(), "issuer request count");
    assertEquals(
        "/v1/offline/v2/keys/refill", executor.requests.get(0).uri().getPath(), "refill path");
    for (final TransportRequest request : executor.requests) {
      assertTrue(
          request.headers().keySet().stream()
              .noneMatch(name -> name.regionMatches(true, 0, "X-Iroha-", 0, "X-Iroha-".length())),
          "offline issuer body auth must not use X-Iroha headers");
    }
    assertTrue(
        List.of("trace-1").equals(executor.requests.get(0).headers().get("X-Client-Trace")),
        "non-auth default header should survive");

    final Map<String, Object> refillBody = executor.requestBody(0);
    assertEquals(accountId, string(refillBody, "account_id"), "refill account id");
    assertEquals("operation-refill-1", string(refillBody, "operation_id"), "refill operation");
    assertEquals(0L, longValue(refillBody, "local_revision"), "refill local revision");
    assertEquals("", string(refillBody, "local_state_hash"), "refill local state hash");
    assertEquals("attestation-key-1", string(refillBody, "attestation_key_id"), "refill attestation key");
    assertEquals("auth-refill-1", string(refillBody, "nonce"), "refill nonce");
    assertEquals(1L, signedMessages.size(), "issuer body auth signing count");
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
    final Map<String, Object> deviceProof = obj(refillBody, "device_proof");
    assertEquals("android", string(deviceProof, "platform"), "refill device proof platform");
    assertEquals(
        "attestation-key-1",
        string(deviceProof, "attestation_key_id"),
        "refill device proof attestation key");
    assertEquals(
        "00".repeat(31) + "ab",
        string(deviceProof, "challenge_hash_hex"),
        "refill device proof challenge hash");
    assertEquals(
        base64("assertion".getBytes(StandardCharsets.UTF_8)),
        string(deviceProof, "assertion_base64"),
        "refill device proof assertion");
    assertTrue(!deviceProof.containsKey("operation"), "device proof must not preserve provider metadata");

    nowMs[0] = 1_700_000_060_001L;
    final OfflineNoteLoadContext refillContext =
        client.prepareLoad("chain-1", accountId, assetDefinitionId, "7").get();
    assertEquals("operation-refill-2", refillContext.operationId(), "second refill operation id");
    assertEquals(2L, executor.requests.size(), "issuer request count after second refill");
    final Map<String, Object> secondRefillBody = executor.requestBody(1);
    assertEquals("", string(secondRefillBody, "local_state_hash"), "second refill local state hash");
  }

  private static void toriiIssuerClientSubmitsKagemushaTopUpArchive() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> certificateJson =
        currentIssuerCertificateJson(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final String accountId = string(certificateJson, "account_id");
    final String assetDefinitionId =
        assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"));
    final String offlinePublicKey = "a5".repeat(32);
    final Map<String, Object> bindingJson = new LinkedHashMap<>();
    bindingJson.put("device_id", "device-1");
    bindingJson.put("attestation_key_id", "attestation-key-1");
    bindingJson.put("offline_public_key", offlinePublicKey);
    final OfflineNoteIssuerDeviceBinding binding =
        new OfflineNoteIssuerDeviceBinding("device-1", offlinePublicKey, bindingJson);
    final OfflineIssuerExecutor executor = new OfflineIssuerExecutor(certificateJson);
    final List<byte[]> signedMessages = new ArrayList<>();
    final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    defaultHeaders.put("X-Iroha-Account", "retired-account");
    defaultHeaders.put("x-iroha-signature", "retired-signature");
    defaultHeaders.put("X-Iroha-Nonce", "retired-nonce");
    defaultHeaders.put("X-Client-Trace", "trace-topup");
    final ToriiOfflineNoteIssuerClient client =
        new ToriiOfflineNoteIssuerClient(
            new ToriiCanonicalRequestAuth(
                accountId,
                message -> {
                  signedMessages.add(Arrays.copyOf(message, message.length));
                  return fakeIssuerSignature(message);
                }),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            executor,
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            defaultHeaders,
            Collections.emptyList(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-topup-1", "auth-topup-1"));
    final byte[] archive = new byte[] {0x4b, 0x54};

    final CompletableFuture<KagemushaTopUpResponse> future =
        client.submitKagemushaTopUp("chain-1", accountId, assetDefinitionId, archive);
    archive[0] = 0;
    final KagemushaTopUpResponse response = future.get();

    assertEquals("operation-topup-1", response.operationId(), "top-up operation id");
    assertEquals("topup-chain-tx-hash", response.chainTxHash(), "top-up transaction hash");
    assertEquals(assetDefinitionId, response.assetDefinitionId(), "top-up asset definition");
    assertEquals("5", response.amount(), "top-up amount");
    assertTrue(
        Arrays.asList("hash-1", "hash-2").equals(response.topupAnchorNullifiers()),
        "top-up anchor nullifiers");
    assertTrue(
        Collections.singletonList("commitment-1").equals(response.outputCommitments()),
        "top-up output commitments");
    assertEquals("root-hint", response.rootHint(), "top-up root hint");
    assertEquals(1L, executor.requests.size(), "top-up request count");
    assertEquals(
        "/v1/offline/v2/kagemusha/topup",
        executor.requests.get(0).uri().getPath(),
        "top-up path");
    assertTrue(
        executor.requests.get(0).headers().keySet().stream()
            .noneMatch(name -> name.regionMatches(true, 0, "X-Iroha-", 0, "X-Iroha-".length())),
        "top-up body auth must not use X-Iroha headers");
    assertTrue(
        Collections.singletonList("trace-topup")
            .equals(executor.requests.get(0).headers().get("X-Client-Trace")),
        "top-up non-auth default header should survive");

    final Map<String, Object> body = executor.requestBody(0);
    assertEquals(accountId, string(body, "account_id"), "top-up account id");
    assertEquals("operation-topup-1", string(body, "operation_id"), "top-up operation");
    assertEquals("device-1", string(body, "device_id"), "top-up device id");
    assertEquals(offlinePublicKey, string(body, "offline_public_key"), "top-up offline public key");
    assertEquals(assetDefinitionId, string(body, "asset_definition_id"), "top-up asset");
    assertEquals("auth-topup-1", string(body, "nonce"), "top-up nonce");
    assertEquals("S1Q=", string(body, "topup_request_norito_base64"), "top-up archive copy");
    assertEquals("device-1", string(obj(body, "device_binding"), "device_id"), "top-up binding");
    assertTrue(!body.containsKey("amount"), "top-up body must not carry amount");
    assertTrue(
        !body.containsKey("init_request_norito_base64"),
        "top-up body must not carry raw init request");
    assertTrue(
        !body.containsKey("topup_init_request_norito_base64"),
        "top-up body must not carry raw top-up init request");
    assertEquals(1L, signedMessages.size(), "top-up signing count");
    final byte[] topUpMessage =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "POST",
            executor.requests.get(0).uri(),
            body,
            1_700_000_000_000L,
            "auth-topup-1");
    assertTrue(Arrays.equals(topUpMessage, signedMessages.get(0)), "top-up body auth message");
    assertEquals(
        Base64.getEncoder().encodeToString(fakeIssuerSignature(topUpMessage)),
        string(body, "signature_base64"),
        "top-up body signature");

    assertIllegalArgumentContains(
        () ->
            client.submitKagemushaTopUp(
                "chain-1", accountId + "-other", assetDefinitionId, new byte[] {1}),
        "canonical auth accountId must match top-up accountId");
    assertIllegalArgumentContains(
        () -> client.submitKagemushaTopUp("chain-1", accountId, assetDefinitionId, new byte[0]),
        "topUpRequestArchive must not be empty");
    assertEquals(1L, executor.requests.size(), "invalid top-up calls must not submit");
  }

  private static void toriiIssuerDeviceBindingRejectsRetiredAssertionPublicKeyAliases() {
    final String offlinePublicKey = "a5".repeat(32);
    for (final String retiredKey :
        Arrays.asList("device_public_key", "app_attest_public_key_base64")) {
      final Map<String, Object> bindingJson = new LinkedHashMap<>();
      bindingJson.put("device_id", "device-1");
      bindingJson.put("attestation_key_id", "attestation-key-1");
      bindingJson.put("offline_public_key", offlinePublicKey);
      bindingJson.put(retiredKey, base64(filledBytes(65, 2)));
      assertThrows(
          () -> new OfflineNoteIssuerDeviceBinding("device-1", offlinePublicKey, bindingJson),
          "device_binding." + retiredKey + " is retired; use assertion_public_key");
    }
  }

  private static void toriiIssuerDeviceBindingRejectsWhitespaceNormalizedFields() {
    final String offlinePublicKey = "a5".repeat(32);
    final OfflineNoteIssuerDeviceBinding valid =
        new OfflineNoteIssuerDeviceBinding(
            "device-1",
            offlinePublicKey,
            issuerDeviceBindingJson("device-1", "attestation-key-1", offlinePublicKey));
    assertEquals("attestation-key-1", valid.attestationKeyId(), "exact attestation key");
    assertIllegalArgumentContains(
        () ->
            new OfflineNoteIssuerDeviceBinding(
                " device-1",
                offlinePublicKey,
                issuerDeviceBindingJson("device-1", "attestation-key-1", offlinePublicKey)),
        "deviceId must be exact non-empty text");
    assertIllegalArgumentContains(
        () ->
            new OfflineNoteIssuerDeviceBinding(
                "device-1",
                offlinePublicKey + " ",
                issuerDeviceBindingJson("device-1", "attestation-key-1", offlinePublicKey)),
        "offlinePublicKey must be exact non-empty text");
    assertIllegalArgumentContains(
        () ->
            new OfflineNoteIssuerDeviceBinding(
                "device-1",
                offlinePublicKey,
                issuerDeviceBindingJson("device-1 ", "attestation-key-1", offlinePublicKey)),
        "device_binding.device_id must match deviceId");
    assertIllegalArgumentContains(
        () ->
            new OfflineNoteIssuerDeviceBinding(
                "device-1",
                offlinePublicKey,
                issuerDeviceBindingJson("device-1", "attestation-key-1", " " + offlinePublicKey)),
        "device_binding.offline_public_key must match offlinePublicKey");

    final OfflineNoteIssuerDeviceBinding paddedAttestation =
        new OfflineNoteIssuerDeviceBinding(
            "device-1",
            offlinePublicKey,
            issuerDeviceBindingJson("device-1", " attestation-key-1", offlinePublicKey));
    assertIllegalStateContains(
        () -> paddedAttestation.attestationKeyId(),
        "device_binding.attestation_key_id must be exact non-empty text");
    final OfflineNoteIssuerDeviceBinding emptyAttestation =
        new OfflineNoteIssuerDeviceBinding(
            "device-1",
            offlinePublicKey,
            issuerDeviceBindingJson("device-1", "", offlinePublicKey));
    assertIllegalStateContains(
        () -> emptyAttestation.attestationKeyId(),
        "device_binding.attestation_key_id is required");
  }

  private static void toriiIssuerClientRejectsMalformedDeviceProofProviderOutput()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> certificateJson =
        currentIssuerCertificateJson(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final String accountId = string(certificateJson, "account_id");
    final String assetDefinitionId =
        assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"));
    final String offlinePublicKey = "a5".repeat(32);
    final OfflineNoteIssuerDeviceBinding binding =
        new OfflineNoteIssuerDeviceBinding(
            "device-1",
            offlinePublicKey,
            issuerDeviceBindingJson("device-1", "attestation-key-1", offlinePublicKey));

    assertToriiIssuerClientRejectsDeviceProof(
        null,
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "device_proof must be a JSON object");

    Map<String, Object> proof = deviceProofJson();
    proof.remove("platform");
    assertToriiIssuerClientRejectsDeviceProof(
        proof, accountId, assetDefinitionId, binding, certificateJson, "platform is required");

    proof = deviceProofJson();
    proof.put("operation", "setup");
    assertToriiIssuerClientRejectsDeviceProof(
        proof,
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "device_proof.operation is not supported");

    for (final String invalidPlatform :
        List.of("ios-appattest", "android-keymint", "android-keymint ", "Android")) {
      proof = deviceProofJson();
      proof.put("platform", invalidPlatform);
      assertToriiIssuerClientRejectsDeviceProof(
          proof,
          accountId,
          assetDefinitionId,
          binding,
          certificateJson,
          "platform must be a supported first-release value");
    }

    for (final String invalidKeyId : List.of("", " attestation-key-1", "attestation-key-1\n")) {
      proof = deviceProofJson();
      proof.put("attestation_key_id", invalidKeyId);
      assertToriiIssuerClientRejectsDeviceProof(
          proof,
          accountId,
          assetDefinitionId,
          binding,
          certificateJson,
          "attestation_key_id must be an exact non-empty string");
    }

    for (final String invalidHash :
        List.of(
            " " + ("00".repeat(31) + "ab"),
            ("00".repeat(31) + "ab").toUpperCase(Locale.ROOT),
            "0x" + ("00".repeat(31) + "ab"),
            "00".repeat(31) + "a",
            "g".repeat(64))) {
      proof = deviceProofJson();
      proof.put("challenge_hash_hex", invalidHash);
      assertToriiIssuerClientRejectsDeviceProof(
          proof,
          accountId,
          assetDefinitionId,
          binding,
          certificateJson,
          "challenge_hash_hex must be 32-byte lowercase hex");
    }

    for (final String invalidAssertion :
        List.of(
            "",
            " " + base64("assertion".getBytes(StandardCharsets.UTF_8)),
            base64("assertion".getBytes(StandardCharsets.UTF_8)) + "\n",
            base64(filledBytes(4, 0xff)).replace('/', '_'),
            base64(filledBytes(1, 0xff)).replace("=", ""))) {
      proof = deviceProofJson();
      proof.put("assertion_base64", invalidAssertion);
      assertToriiIssuerClientRejectsDeviceProof(
          proof,
          accountId,
          assetDefinitionId,
          binding,
          certificateJson,
          "assertion_base64 must be canonical base64");
    }

    proof = deviceProofJson();
    proof.put("counter", -1L);
    assertToriiIssuerClientRejectsDeviceProof(
        proof,
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "counter must be non-negative");

    proof = deviceProofJson();
    proof.put("counter", "1");
    assertToriiIssuerClientRejectsDeviceProof(
        proof, accountId, assetDefinitionId, binding, certificateJson, "counter must be an integer");
  }

  private static void assertToriiIssuerClientRejectsDeviceProof(
      final Map<String, Object> deviceProof,
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding,
      final Map<String, Object> certificateJson,
      final String expectedMessage) {
    final ToriiOfflineNoteIssuerClient client =
        new ToriiOfflineNoteIssuerClient(
            new ToriiCanonicalRequestAuth(accountId, OfflineNoteTest::fakeIssuerSignature),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            (chainId, requestAccountId, requestAssetDefinitionId, operation, lineageId, proofAmount) ->
                deviceProof,
            new OfflineIssuerExecutor(certificateJson),
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            Map.of(),
            List.of(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-refill-proof", "auth-refill-proof"));

    assertIllegalStateContains(
        () -> client.prepareLoad("chain-1", accountId, assetDefinitionId, "5"),
        expectedMessage);
  }

  private static void toriiIssuerClientRejectsMalformedLineageState() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> certificateJson =
        currentIssuerCertificateJson(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final String accountId = string(certificateJson, "account_id");
    final String assetDefinitionId =
        assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"));
    final String offlinePublicKey = "a5".repeat(32);
    final OfflineNoteIssuerDeviceBinding binding =
        new OfflineNoteIssuerDeviceBinding(
            "device-1",
            offlinePublicKey,
            issuerDeviceBindingJson("device-1", "attestation-key-1", offlinePublicKey));

    assertToriiIssuerClientRejectsLineageState(
        state -> {
          state.put("lineage_id", " lineage-1");
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "lineage_id must be an exact non-empty string");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          state.put("balance", "-1");
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "balance must be a non-negative amount");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          state.put("server_revision", -1L);
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "server_revision must be non-negative");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          state.put("server_state_hash", ("00".repeat(31) + "ab").toUpperCase(Locale.ROOT));
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "server_state_hash must be 32-byte lowercase hex");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          state.put("issuer_signature_base64", base64(filledBytes(63, 6)));
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "issuer_signature_base64 must be 64 bytes");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          obj(state, "authorization").put("max_tx_value", "-1");
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "max_tx_value must be a non-negative amount");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          obj(state, "authorization").put("refresh_at_ms", 1_699_999_999_999L);
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "refresh_at_ms must be at or after issued_at_ms");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          obj(state, "authorization").put("expires_at_ms", 1_700_000_000_000L);
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "expires_at_ms must be after issued_at_ms");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          obj(state, "authorization").put("issuer_signature_base64", " " + base64(filledBytes(64, 5)));
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "issuer_signature_base64 must be canonical base64");
    assertToriiIssuerClientRejectsLineageState(
        state -> {
          obj(obj(state, "authorization"), "device_binding").put("attestation_key_id", " attestation-key-1");
          return state;
        },
        accountId,
        assetDefinitionId,
        binding,
        certificateJson,
        "attestation_key_id must be an exact non-empty string");
  }

  private static void assertToriiIssuerClientRejectsLineageState(
      final Function<Map<String, Object>, Map<String, Object>> lineageStateMutator,
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding,
      final Map<String, Object> certificateJson,
      final String expectedMessage) {
    final ToriiOfflineNoteIssuerClient client =
        new ToriiOfflineNoteIssuerClient(
            new ToriiCanonicalRequestAuth(accountId, OfflineNoteTest::fakeIssuerSignature),
            (chainId, requestAccountId, requestAssetDefinitionId) -> binding,
            (chainId, requestAccountId, requestAssetDefinitionId, operation, lineageId, proofAmount) ->
                deviceProofJson(),
            new OfflineIssuerExecutor(certificateJson, null, lineageStateMutator),
            URI.create("https://torii.example"),
            java.time.Duration.ofSeconds(15),
            Map.of(),
            List.of(),
            () -> 1_700_000_000_000L,
            new SequenceIdGenerator("operation-refill-lineage", "auth-refill-lineage"));

    final Throwable failure =
        assertFutureFailsWithin(
            client.prepareLoad("chain-1", accountId, assetDefinitionId, "5"),
            "malformed lineage_state must reject");
    final Throwable root = rootCause(failure);
    assertTrue(
        root.getMessage() != null && root.getMessage().contains(expectedMessage),
        "expected lineage_state failure to contain: " + expectedMessage + ", got: " + root);
  }

  private static Throwable rootCause(final Throwable failure) {
    Throwable root = failure;
    while (root.getCause() != null) {
      root = root.getCause();
    }
    return root;
  }

  private static Map<String, Object> issuerDeviceBindingJson(
      final Object deviceId, final Object attestationKeyId, final Object offlinePublicKey) {
    final Map<String, Object> bindingJson = new LinkedHashMap<>();
    bindingJson.put("device_id", deviceId);
    bindingJson.put("attestation_key_id", attestationKeyId);
    bindingJson.put("offline_public_key", offlinePublicKey);
    return bindingJson;
  }

  private static byte[] fakeIssuerSignature(final byte[] message) {
    final byte[] signature = new byte[64];
    for (int index = 0; index < signature.length; index++) {
      signature[index] = (byte) (message[index % message.length] ^ (index + 17));
    }
    return signature;
  }

  private static Map<String, Object> deviceProofJson() {
    final Map<String, Object> proof = new LinkedHashMap<>();
    proof.put("platform", "android");
    proof.put("attestation_key_id", "attestation-key-1");
    proof.put("challenge_hash_hex", "00".repeat(31) + "ab");
    proof.put("assertion_base64", base64("assertion".getBytes(StandardCharsets.UTF_8)));
    proof.put("counter", 1L);
    return proof;
  }

  private static void toriiIssuerClientRejectsMalformedCertificateUsageLimits() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> baseCertificateJson =
        currentIssuerCertificateJson(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
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
    for (final String invalidScheme :
        List.of("apple-appattest-counter", "android-keymint-ecdsa-p256-usage-limit")) {
      final Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
      certificateJson.put("assertion_scheme", invalidScheme);
      assertToriiIssuerClientRejectsCertificateJson(
          certificateJson, accountId, assetDefinitionId, binding);
    }
    for (final String invalidAlgorithm : List.of("ecdsa-p256-sha256", "ed25519")) {
      final Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
      certificateJson.put("assertion_key_algorithm", invalidAlgorithm);
      assertToriiIssuerClientRejectsCertificateJson(
          certificateJson, accountId, assetDefinitionId, binding);
    }
    for (final String invalidPlatform :
        List.of("android", "android-keymint ", "Android-keymint", "ios-appattest-android")) {
      final Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
      certificateJson.put("platform", invalidPlatform);
      certificateJson.put("assertion_scheme", OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME);
      certificateJson.put(
          "assertion_key_algorithm", OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM);
      certificateJson.put("assertion_usage_count_limit", Integer.valueOf(1));
      assertToriiIssuerClientRejectsCertificateJson(
          certificateJson, accountId, assetDefinitionId, binding);
    }

    Map<String, Object> certificateJson = new LinkedHashMap<>(baseCertificateJson);
    certificateJson.put("public_key", hex(filledBytes(33, 1)));
    assertToriiIssuerClientRejectsCertificateJson(
        certificateJson, accountId, assetDefinitionId, binding);

    certificateJson = new LinkedHashMap<>(baseCertificateJson);
    certificateJson.put(
        "assertion_public_key", base64(filledBytes(65, 0xff)).replace('/', '_'));
    assertToriiIssuerClientRejectsCertificateJson(
        certificateJson, accountId, assetDefinitionId, binding);

    certificateJson = new LinkedHashMap<>(baseCertificateJson);
    certificateJson.put("issuer_signature_base64", " " + base64(filledBytes(64, 3)));
    assertToriiIssuerClientRejectsCertificateJson(
        certificateJson, accountId, assetDefinitionId, binding);

    certificateJson = new LinkedHashMap<>(baseCertificateJson);
    certificateJson.put("issuer_signature_base64", base64(filledBytes(64, 3)).replace("=", ""));
    assertToriiIssuerClientRejectsCertificateJson(
        certificateJson, accountId, assetDefinitionId, binding);
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
            (chainId, requestAccountId, requestAssetDefinitionId, operation, lineageId, proofAmount) ->
                deviceProofJson(),
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
    final Throwable cause = assertFutureFailsWithin(load, "retired wallet load should fail");
    assertEquals(
        ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
        cause.getMessage(),
        "retired wallet load message");
    assertTrue(
        !issuerClient.issueRequested.await(100, TimeUnit.MILLISECONDS),
        "retired wallet load must not submit issue request");
    store.release.countDown();
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
        "retired wallet load should fail before issuer dispatch");
    assertEquals(
        ToriiOfflineNoteIssuerClient.RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE,
        cause.getMessage(),
        "retired wallet load message");
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

  private static void walletRejectsAuditWhenRecursiveVerifierFails() throws Exception {
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
            new RejectingProofVerifier(true, false),
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
            () -> 1_700_000_001_200L,
            new StaticOwnerCertificateSigner(recipientCertificate));
    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(
            assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            string(chainRedeem, "amount"));

    assertIllegalArgumentContains(
        () -> senderWallet.pay(receiveRequest),
        "Offline Note recursive audit proof verification failed");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "source note state after rejected audit proof");
    assertEquals(1L, senderStore.listNotes().size(), "note count after rejected audit proof");
  }

  private static void walletRejectsPaymentsNeedingMoreThanFourInputs() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final String chainId = string(derivation, "chain_id");
    final String assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"));
    final String senderAccountId = accountFromAssetId(string(chainIssue, "asset_id"));
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    for (int index = 0; index < 5; index++) {
      senderStore.upsert(
          issuerSourceWalletNote(
              chainId,
              senderAccountId,
              assetDefinitionId,
              "1",
              senderCertificate,
              filledBytes(32, 0x40 + index),
              "input-cap-" + index,
              1_700_000_012_000L + index));
    }
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            chainId,
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-input-cap"),
            () -> 1_700_000_012_100L,
            new StaticOwnerCertificateSigner(senderCertificate));
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            chainId,
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            new InMemoryOfflineNoteStore(),
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.singletonList(filledBytes(32, 0x50))),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-input-cap"),
            () -> 1_700_000_012_050L,
            new StaticOwnerCertificateSigner(recipientCertificate));
    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(assetDefinitionId, "5");

    assertIllegalArgumentContains(
        () -> senderWallet.pay(receiveRequest),
        "Offline Note payments support at most 4 input notes");
    assertEquals(5L, senderStore.listNotes().size(), "input-cap note count");
    long spendableCount = 0;
    for (final OfflineNoteWalletNote note : senderStore.listNotes()) {
      if (note.state() == OfflineNoteWalletNoteState.SPENDABLE) {
        spendableCount++;
      }
    }
    assertEquals(5L, spendableCount, "input-cap spendable note count");
  }

  private static void walletRejectsNonPositiveReceiveAndPaymentAmounts() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final String chainId = string(derivation, "chain_id");
    final String assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id"));
    final String senderAccountId = accountFromAssetId(string(chainIssue, "asset_id"));
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final InMemoryOfflineNoteStore recipientStore = new InMemoryOfflineNoteStore();
    final OfflineNoteWallet recipientWallet =
        new OfflineNoteWallet(
            chainId,
            string(payment, "recipient_account_id"),
            new StaticAttestationProvider(recipientCertificate),
            recipientStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(
                Arrays.asList(filledBytes(32, 0x60), filledBytes(32, 0x61))),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-positive-amount"),
            () -> 1_700_000_012_150L,
            new StaticOwnerCertificateSigner(recipientCertificate));

    for (final String invalidAmount : Arrays.asList("0", "-1")) {
      assertIllegalArgumentContains(
          () -> recipientWallet.prepareReceive(assetDefinitionId, invalidAmount),
          "Offline Note payment amount must be positive");
      assertEquals(0L, recipientStore.listNotes().size(), "nonpositive receive note count");
    }

    final OfflineNoteReceiveRequest receiveRequest =
        recipientWallet.prepareReceive(assetDefinitionId, "1");
    final InMemoryOfflineNoteStore senderStore = new InMemoryOfflineNoteStore();
    senderStore.upsert(
        issuerSourceWalletNote(
            chainId,
            senderAccountId,
            assetDefinitionId,
            "2",
            senderCertificate,
            filledBytes(32, 0x70),
            "positive-amount",
            1_700_000_012_160L));
    final OfflineNoteWallet senderWallet =
        new OfflineNoteWallet(
            chainId,
            senderAccountId,
            new StaticAttestationProvider(senderCertificate),
            senderStore,
            null,
            new RecordingTransactionSubmitter(),
            BindingProofProvider.INSTANCE,
            BindingProofVerifier.INSTANCE,
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id") + "-positive-amount"),
            () -> 1_700_000_012_170L,
            new StaticOwnerCertificateSigner(senderCertificate));

    for (final String invalidAmount : Arrays.asList("0", "-1")) {
      assertIllegalArgumentContains(
          () ->
              new OfflineNoteReceiveRequest(
                  receiveRequest.chainId(),
                  receiveRequest.paymentRequestId() + "-" + invalidAmount,
                  receiveRequest.accountId(),
                  receiveRequest.assetDefinitionId(),
                  receiveRequest.assetId(),
                  invalidAmount,
                  receiveRequest.keyCertificate(),
                  receiveRequest.outputCommitment()),
          "Offline Note payment amount must be positive");
      assertEquals(1L, senderStore.listNotes().size(), "nonpositive pay note count");
      assertEquals(
          OfflineNoteWalletNoteState.SPENDABLE.name(),
          senderStore.listNotes().get(0).state().name(),
          "nonpositive pay note state");
    }
  }

  private static void walletRejectsRedeemWhenRecursiveVerifierFails() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> chainIssue = obj(chain, "issue");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNote.KeyCertificate senderCertificate =
        certificate(obj(payment, "sender_key_certificate"));
    final OfflineNoteWalletNote note = sourceWalletNote(fixture, senderCertificate);
    final InMemoryOfflineNoteStore store = new InMemoryOfflineNoteStore();
    store.upsert(note);
    final RecordingTransactionSubmitter submitter = new RecordingTransactionSubmitter();
    final OfflineNoteWallet wallet =
        new OfflineNoteWallet(
            string(derivation, "chain_id"),
            accountFromAssetId(string(chainIssue, "asset_id")),
            new StaticAttestationProvider(senderCertificate),
            store,
            null,
            submitter,
            BindingProofProvider.INSTANCE,
            new RejectingProofVerifier(false, true),
            fixtureOwnerCertificateVerifier(fixture),
            new QueueRandomSource(Collections.emptyList()),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_300L,
            new StaticOwnerCertificateSigner(senderCertificate));

    assertIllegalArgumentContains(
        () -> wallet.redeem(note),
        "Offline Note recursive redeem proof verification failed");
    assertEquals(0L, submitter.defunds.size(), "defund submit count after rejected redeem proof");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        store.findNote(note.noteCommitment()).state().name(),
        "note state after rejected redeem proof");
  }

  private static void offlineNoteTransactionSubmitterIsRetiredAndKeepsFeeMetadataHelper()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final CapturingIrohaClient client = new CapturingIrohaClient();
    final Map<String, String> metadata =
        IrohaOfflineNoteTransactionSubmitter.feeMetadata(
            "  xor#universal  ", "  " + string(payment, "recipient_account_id") + "  ");
    assertEquals(
        "xor#universal",
        metadata.get(IrohaOfflineNoteTransactionSubmitter.GAS_ASSET_ID_METADATA_KEY),
        "gas asset metadata");
    assertEquals(
        string(payment, "recipient_account_id"),
        metadata.get(IrohaOfflineNoteTransactionSubmitter.FEE_SPONSOR_METADATA_KEY),
        "fee sponsor metadata");
    final IrohaOfflineNoteTransactionSubmitter submitter =
        new IrohaOfflineNoteTransactionSubmitter(
            client,
            new FakeSigner(),
            string(derivation, "chain_id"),
            string(payment, "sender_account_id"),
            new NoritoJavaCodecAdapter(),
            () -> 1_736_000_000_000L,
            metadata);

    final OfflineNote.AuditBundle audit = audit(fixture);
    final OfflineNote.Redeem redemption = redeem(fixture);
    assertRetiredOfflineNoteSubmission(submitter.submitAudit(audit));
    assertRetiredOfflineNoteSubmission(submitter.submitRedeem(redemption));
    assertRetiredOfflineNoteSubmission(
        submitter.submitDefund(redemption, Collections.singletonList(audit)));
    assertTrue(client.submittedTransaction == null, "retired submitter must not submit a transaction");
  }

  private static void assertRetiredOfflineNoteSubmission(
      final CompletableFuture<ClientResponse> future) throws Exception {
    try {
      future.get(5, TimeUnit.SECONDS);
      throw new AssertionError("classic Offline Note submitter should be retired");
    } catch (final ExecutionException ex) {
      final Throwable cause = ex.getCause();
      assertTrue(
          cause instanceof IllegalStateException,
          "retired submitter should fail with IllegalStateException");
      assertEquals(
          IrohaOfflineNoteTransactionSubmitter.RETIRED_OFFLINE_NOTE_PAYMENT_MESSAGE,
          cause.getMessage(),
          "retired submitter failure message");
    }
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
    final OfflineNote.Issue issue = issue(fixture);
    final OfflineNote.Redeem redeem = redeem(fixture);
    final OfflineNote.KeyCertificate recipientCertificate =
        certificate(obj(payment, "recipient_key_certificate"));
    final OfflineNoteWalletNote issuePending =
        sourceWalletNote(fixture, certificate(obj(payment, "sender_key_certificate")))
            .withState(OfflineNoteWalletNoteState.ISSUE_PENDING, 1_700_000_003_000L);
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
                    OfflineNoteOutcomeIndex.KIND_ISSUE,
                    OfflineNoteOutcomeIndex.STATUS_COMMITTED,
                    "issue-tx",
                    rawInstructionPair(
                        OfflineNote.ISSUE_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.issueInstruction(issue)))),
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_AUDIT,
                    OfflineNoteOutcomeIndex.STATUS_COMMITTED,
                    "audit-tx",
                    rawInstructionPair(
                        OfflineNote.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.auditInstruction(audit)))),
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_REDEEM,
                    OfflineNoteOutcomeIndex.STATUS_COMMITTED,
                    "redeem-tx",
                    rawInstructionPair(
                        OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.redeemInstruction(redeem))))));
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        committed.resolve(issuePending).state().name(),
        "committed issue");
    assertTrue(
        committed.resolve(sourceWalletNote(fixture, certificate(obj(payment, "sender_key_certificate"))))
            == null,
        "audit outcomes do not mutate local-final notes");
    assertEquals(
        OfflineNoteWalletNoteState.REDEEMED.name(),
        committed.resolve(redeemPending).state().name(),
        "committed redeem");

    final OfflineNoteOutcomeIndex permissiveCaseDrift =
        OfflineNoteOutcomeIndex.fromExplorerOutcomes(
            List.of(
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_ISSUE,
                    "committed",
                    "issue-case-drift",
                    rawInstructionPair(
                        OfflineNote.ISSUE_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.issueInstruction(issue)))),
                new OfflineNoteExplorerInstructionOutcome(
                    OfflineNoteOutcomeIndex.KIND_REDEEM.toLowerCase(Locale.ROOT),
                    OfflineNoteOutcomeIndex.STATUS_COMMITTED,
                    "redeem-case-drift",
                    rawInstructionPair(
                        OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNote.redeemInstruction(redeem))))));
    assertTrue(permissiveCaseDrift.resolve(issuePending) == null, "lowercase status ignored");
    assertTrue(permissiveCaseDrift.resolve(redeemPending) == null, "lowercase kind ignored");
    assertThrows(
        () ->
            new OfflineNoteExplorerInstructionOutcome(
                " " + OfflineNoteOutcomeIndex.KIND_REDEEM,
                OfflineNoteOutcomeIndex.STATUS_COMMITTED,
                rawInstructionPair(
                    OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                    wirePayloadBytes(OfflineNote.redeemInstruction(redeem)))),
        "padded outcome kind should throw");
    assertThrows(
        () ->
            new OfflineNoteExplorerInstructionOutcome(
                OfflineNoteOutcomeIndex.KIND_REDEEM,
                OfflineNoteOutcomeIndex.STATUS_COMMITTED + " ",
                rawInstructionPair(
                    OfflineNote.REDEEM_INSTRUCTION_SCHEMA,
                    wirePayloadBytes(OfflineNote.redeemInstruction(redeem)))),
        "padded outcome status should throw");

    final OfflineNoteOutcomeIndex rejected =
        new OfflineNoteOutcomeIndex()
            .recordRejectedIssue(issue, "issue-rejected")
            .recordRejectedAudit(audit, "audit-rejected")
            .recordRejectedRedeem(redeem, "redeem-rejected");
    assertEquals(
        OfflineNoteWalletNoteState.CANCELLED.name(),
        rejected.resolve(issuePending).state().name(),
        "rejected issue");
    assertEquals(
        OfflineNoteWalletNoteState.SPENDABLE.name(),
        rejected.resolve(redeemPending).state().name(),
        "rejected redeem");
  }

  private static void outcomeProviderRequiresExactEncodedInstructionHex() {
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0xbe, (byte) 0xef},
            outcomeProviderEncodedInstruction("beef", false)),
        "top-level encoded hex");
    assertTrue(
        Arrays.equals(
            new byte[] {(byte) 0xca, (byte) 0xfe},
            outcomeProviderEncodedInstruction("cafe", true)),
        "nested encoded hex");

    final Object[][] malformed = {
      {" beef", false},
      {"beef\n", false},
      {"0xbeef", false},
      {"BEEF", true},
      {"", true},
      {"abc", true},
      {"gg", true}
    };
    for (final Object[] payload : malformed) {
      final String encoded = (String) payload[0];
      final boolean nested = (Boolean) payload[1];
      assertThrows(
          () -> outcomeProviderEncodedInstruction(encoded, nested),
          "malformed encoded instruction hex should throw: " + encoded);
    }
  }

  private static byte[] outcomeProviderEncodedInstruction(
      final String encoded, final boolean nested) {
    try {
      final List<OfflineNoteExplorerInstructionOutcome> outcomes =
          new ToriiOfflineNoteOutcomeProvider(
                  new ExplorerOutcomeExecutor(encoded, nested),
                  URI.create("https://example.test"),
                  null,
                  Collections.emptyMap(),
                  Collections.emptyList(),
                  25)
              .listOutcomes()
              .get(5, TimeUnit.SECONDS);
      assertEquals(3L, outcomes.size(), "outcome count");
      return outcomes.get(0).encodedInstruction();
    } catch (final InterruptedException error) {
      Thread.currentThread().interrupt();
      throw new RuntimeException(error);
    } catch (final ExecutionException | TimeoutException error) {
      throw new RuntimeException(error);
    }
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

  private static Map<String, Object> currentIssuerCertificateJson(
      final Map<String, Object> json) {
    final Map<String, Object> copy = new LinkedHashMap<>(json);
    final String platform = string(copy, "platform");
    if (OfflineNoteV2.ANDROID_KEYMINT_PLATFORM.equals(platform)) {
      copy.put("assertion_scheme", OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME);
      copy.put(
          "assertion_key_algorithm", OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM);
    } else if (OfflineNoteV2.IOS_APP_ATTEST_PLATFORM.equals(platform)) {
      copy.put("assertion_scheme", OfflineNoteV2.IOS_APP_ATTEST_ASSERTION_SCHEME);
      copy.put("assertion_key_algorithm", OfflineNoteV2.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM);
    } else {
      copy.put("assertion_scheme", "unsupported-platform");
      copy.put("assertion_key_algorithm", "unsupported-platform");
    }
    return copy;
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

  private static OfflineNoteWalletNote issuerSourceWalletNote(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final String amount,
      final OfflineNote.KeyCertificate certificate,
      final byte[] noteSecret,
      final String operationSuffix,
      final long createdAtMs) {
    final String assetId = assetDefinitionId + "#" + accountId;
    final OfflineNote.CommitmentOrigin.IssuerLoad origin =
        new OfflineNote.CommitmentOrigin.IssuerLoad(
            "operation-" + operationSuffix, "lineage-" + operationSuffix, 1L);
    final byte[] noteCommitment =
        OfflineNote.deriveNoteCommitment(
            new OfflineNote.NoteCommitmentPreimage(
                chainId, certificate.payloadHash(), assetId, amount, noteSecret, origin));
    return new OfflineNoteWalletNote(
        chainId,
        accountId,
        assetId,
        amount,
        certificate,
        noteCommitment,
        noteSecret,
        origin,
        OfflineNoteWalletNoteState.SPENDABLE,
        createdAtMs,
        createdAtMs);
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

  private static final class ExplorerOutcomeExecutor implements HttpTransportExecutor {
    private final String encoded;
    private final boolean nested;

    private ExplorerOutcomeExecutor(final String encoded, final boolean nested) {
      this.encoded = encoded;
      this.nested = nested;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final Map<String, Object> json = new LinkedHashMap<>();
      if (nested) {
        json.put("encoded", encoded);
      }
      final Map<String, Object> box = new LinkedHashMap<>();
      box.put("json", json);
      if (!nested) {
        box.put("encoded", encoded);
      }
      final Map<String, Object> item = new LinkedHashMap<>();
      item.put("authority", "authority");
      item.put("created_at", "2025-01-01T00:00:00Z");
      item.put("kind", OfflineNoteOutcomeIndex.KIND_ISSUE);
      item.put("r#box", box);
      item.put("transaction_hash", "hash");
      item.put("transaction_status", OfflineNoteOutcomeIndex.STATUS_COMMITTED);
      item.put("block", 1L);
      item.put("index", 0L);
      final Map<String, Object> page = new LinkedHashMap<>();
      page.put("items", Collections.singletonList(item));
      return CompletableFuture.completedFuture(
          TransportResponse.builder()
              .setStatusCode(200)
              .setBody(JsonEncoder.encode(page).getBytes(StandardCharsets.UTF_8))
              .build());
    }
  }

  private static final class OfflineIssuerExecutor implements HttpTransportExecutor {
    private final Map<String, Object> certificateJson;
    private final String serverStateHash;
    private final Function<Map<String, Object>, Map<String, Object>> lineageStateMutator;
    private final List<TransportRequest> requests = new ArrayList<>();

    private OfflineIssuerExecutor(final Map<String, Object> certificateJson) {
      this(certificateJson, null);
    }

    private OfflineIssuerExecutor(
        final Map<String, Object> certificateJson, final String serverStateHash) {
      this(certificateJson, serverStateHash, Function.identity());
    }

    private OfflineIssuerExecutor(
        final Map<String, Object> certificateJson,
        final String serverStateHash,
        final Function<Map<String, Object>, Map<String, Object>> lineageStateMutator) {
      this.certificateJson = certificateJson;
      this.serverStateHash = serverStateHash;
      this.lineageStateMutator = lineageStateMutator;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      final Map<String, Object> body = requestBody(request);
      final Map<String, Object> response = new LinkedHashMap<>();
      switch (request.uri().getPath()) {
        case "/v1/offline/v2/keys/refill" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("lineage_state", lineageState(body, 0, "0"));
          response.put("key_certificate", certificateWithExpiry());
          response.put("key_certificates", List.of(certificateWithExpiry()));
        }
        case "/v1/offline/v2/notes/issue" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("settlement", Map.of("entry_hash", "settlement-entry-hash"));
          response.put("lineage_state", lineageState(body, 1, "5"));
          response.put("local_balance", "5");
          response.put("locked_balance", "0");
          response.put("local_revision", 1L);
          response.put("local_state_hash", "lineage-state-hash");
          response.put("issued_note_commitment", string(body, "note_commitment"));
          response.put("key_certificate", certificateWithExpiry());
          response.put("key_certificates", List.of(certificateWithExpiry()));
        }
        case "/v1/offline/v2/kagemusha/topup" -> {
          response.put("operation_id", string(body, "operation_id"));
          response.put("chain_tx_hash", "topup-chain-tx-hash");
          response.put("asset_definition_id", string(body, "asset_definition_id"));
          response.put("amount", "5");
          response.put("topup_anchor_nullifiers", Arrays.asList("hash-1", "hash-2"));
          response.put("output_commitments", Collections.singletonList("commitment-1"));
          response.put("root_hint", "root-hint");
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

    private Map<String, Object> lineageState(
        final Map<String, Object> requestBody, final long revision, final String balance) {
      final Map<String, Object> authorization = new LinkedHashMap<>();
      authorization.put("authorization_id", "authorization-1");
      authorization.put("lineage_id", "lineage-1");
      authorization.put("account_id", string(requestBody, "account_id"));
      authorization.put("device_id", string(requestBody, "device_id"));
      authorization.put("offline_public_key", string(requestBody, "offline_public_key"));
      authorization.put("verdict_id", "verdict-1");
      authorization.put("max_balance", "1000");
      authorization.put("max_tx_value", "250");
      authorization.put("issued_at_ms", 1_700_000_000_000L);
      authorization.put("refresh_at_ms", 1_700_000_030_000L);
      authorization.put("expires_at_ms", 1_700_000_060_000L);
      authorization.put("device_binding", obj(requestBody, "device_binding"));
      authorization.put("issuer_signature_base64", base64(filledBytes(64, 5)));
      final Map<String, Object> state = new LinkedHashMap<>();
      state.put("lineage_id", "lineage-1");
      state.put("account_id", string(requestBody, "account_id"));
      state.put("device_id", string(requestBody, "device_id"));
      state.put("offline_public_key", string(requestBody, "offline_public_key"));
      state.put("asset_definition_id", string(requestBody, "asset_definition_id"));
      state.put("server_revision", revision);
      state.put("pending_local_revision", revision);
      state.put("balance", balance);
      state.put("locked_balance", "0");
      state.put("authorization", authorization);
      state.put(
          "server_state_hash",
          serverStateHash == null ? "00".repeat(31) + "04" : serverStateHash);
      state.put("issuer_signature_base64", base64(filledBytes(64, 6)));
      return lineageStateMutator.apply(state);
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

  private static final class RejectingProofVerifier implements OfflineNoteProofVerifier {
    private final boolean rejectAudit;
    private final boolean rejectRedeem;

    private RejectingProofVerifier(final boolean rejectAudit, final boolean rejectRedeem) {
      this.rejectAudit = rejectAudit;
      this.rejectRedeem = rejectRedeem;
    }

    @Override
    public boolean verifyAudit(final OfflineNote.AuditBundle audit) {
      return !rejectAudit && BindingProofVerifier.INSTANCE.verifyAudit(audit);
    }

    @Override
    public boolean verifyRedeem(final OfflineNote.Redeem redemption) {
      return !rejectRedeem && BindingProofVerifier.INSTANCE.verifyRedeem(redemption);
    }
  }

  private static final class RecordingIssuerClient implements OfflineNoteIssuerClient {
    private final OfflineNoteLoadContext loadContext;
    private long prepareLoadCount;
    private String lastPrepareAmount;
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
      prepareLoadCount++;
      lastPrepareAmount = amount;
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

  private static void assertBareInstructionWrapper(final byte[] wirePayload) {
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(wirePayload, null);
    assertEquals(NoritoHeader.COMPACT_LEN, decoded.header().flags(), "instruction wrapper flags");
    final NoritoDecoder decoder =
        new NoritoDecoder(decoded.payload(), decoded.header().flags(), decoded.header().minor());
    final int length =
        (int) decoder.readLength((decoded.header().flags() & NoritoHeader.COMPACT_LEN) != 0);
    final byte[] modelPayload = decoder.readBytes(length);
    assertEquals(0, decoder.remaining(), "trailing instruction wrapper bytes");
    assertTrue(!isNoritoFrame(modelPayload), "instruction wrapper must contain a bare model payload");
  }

  private static boolean isNoritoFrame(final byte[] bytes) {
    return bytes != null
        && bytes.length >= NoritoHeader.HEADER_LENGTH
        && bytes[0] == (byte) 'N'
        && bytes[1] == (byte) 'R'
        && bytes[2] == (byte) 'T'
        && bytes[3] == (byte) '0';
  }

  private static byte[] rawInstructionPair(final String wireName, final byte[] wirePayload) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeField(out, encodeString(wireName, true), true);
    writeField(out, encodeBytesVec(wirePayload), true);
    return out.toByteArray();
  }

  private static byte[] rawVerifyingKeyBoxNorito(final String backend, final byte[] bytes) {
    return rawVerifyingKeyBoxNoritoFields(encodeString(backend, true), encodeBytesVec(bytes));
  }

  private static byte[] rawVerifyingKeyBoxNoritoFields(
      final byte[] backendFieldPayload, final byte[] bytesFieldPayload) {
    return NoritoCodec.encode(
        "raw-vk-box",
        "iroha_data_model::proof::VerifyingKeyBox",
        new TypeAdapter<>() {
          @Override
          public void encode(final NoritoEncoder encoder, final String value) {
            writeRawVerifyingKeyBoxField(encoder, backendFieldPayload);
            writeRawVerifyingKeyBoxField(encoder, bytesFieldPayload);
          }

          @Override
          public String decode(final NoritoDecoder decoder) {
            throw new AssertionError("raw VerifyingKeyBox test adapter is encode-only");
          }
        },
        NoritoHeader.COMPACT_LEN);
  }

  private static void writeRawVerifyingKeyBoxField(
      final NoritoEncoder encoder, final byte[] payload) {
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
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

  private static byte[] concat(final byte[] first, final byte[] second) {
    final byte[] out = Arrays.copyOf(first, first.length + second.length);
    System.arraycopy(second, 0, out, first.length, second.length);
    return out;
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

  private static List<String> nonExactPublicInputHashes(final String canonicalHash) {
    return Arrays.asList(
        " " + canonicalHash,
        canonicalHash + "\n",
        canonicalHash.toUpperCase(Locale.ROOT),
        "0x" + canonicalHash,
        canonicalHash.substring(0, canonicalHash.length() - 1),
        canonicalHash.substring(0, canonicalHash.length() - 2) + "zz",
        "");
  }

  private static List<String> nonExactLowerHex32(final String canonicalHex) {
    return Arrays.asList(
        " " + canonicalHex,
        canonicalHex + "\n",
        canonicalHex.toUpperCase(Locale.ROOT),
        "0x" + canonicalHex,
        canonicalHex.substring(0, canonicalHex.length() - 1),
        canonicalHex.substring(0, canonicalHex.length() - 2) + "zz",
        "");
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder builder = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      builder.append(value);
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

  private static void assertIllegalStateContains(
      final Runnable action, final String expectedMessage) {
    try {
      action.run();
    } catch (final IllegalStateException expected) {
      assertTrue(
          expected.getMessage().contains(expectedMessage),
          "expected IllegalStateException to contain: " + expectedMessage);
      return;
    }
    throw new AssertionError("expected IllegalStateException: " + expectedMessage);
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
