package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineNoteV2Test {

  private OfflineNoteV2Test() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    offlineNoteV2ModelsMatchRustNoritoVectors();
    walletDerivationsMatchRustVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    instanceValuesMatchRustVectors();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
    walletLoadDerivesCommitmentBeforeIssuerSubmission();
    walletLifecycleBuildsAuditAcceptAndRedeemTransactions();
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
            new QueueRandomSource(
                Arrays.asList(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")))),
            new FixedIdGenerator(string(derivation, "payment_request_id")),
            () -> 1_700_000_001_100L);
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
        string(chainAudit, "public_inputs_hash"),
        hex(token.audit().publicInputsHash()),
        "audit public inputs hash");
    assertEquals(
        OfflineNoteV2WalletNoteState.SPEND_PENDING.name(),
        senderStore.findNote(hexBytes(string(derivation, "source_note_commitment"))).state().name(),
        "source note state");
    assertEquals(
        OfflineNoteV2WalletNoteState.CHANGE_PENDING.name(),
        senderStore.findNote(hexBytes(string(derivation, "change_output_commitment"))).state().name(),
        "change note state");

    final OfflineNoteV2WalletNote accepted = recipientWallet.accept(token).get();

    assertEquals(
        OfflineNoteV2WalletNoteState.SPENDABLE.name(),
        accepted.state().name(),
        "accepted note state");
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
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
