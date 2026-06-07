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
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineNoteV2Test {

  private OfflineNoteV2Test() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    offlineNoteV2ModelsMatchRustNoritoVectors();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    proofVerifierAndHashValidationRejectsMalformedValues();
    certificateValidationRejectsMalformedValues();
    auditBundleRejectsInvalidShapesAndUncommittedOutputs();
    issueRedeemPublicInputsAndInstancesRejectMalformedValues();
    instanceValuesMatchRustVectors();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
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

  private static void proofVerifierAndHashValidationRejectsMalformedValues() throws Exception {
    final byte[] publicInputsHash = audit(loadFixture()).publicInputsHash();
    final OfflineNoteV2.ProofBox trimmedProof =
        new OfflineNoteV2.ProofBox(
            "  " + OfflineNoteV2.RECURSIVE_BACKEND + "  ", new byte[] {1});
    assertEquals(OfflineNoteV2.RECURSIVE_BACKEND, trimmedProof.backend(), "trimmed proof backend");

    assertThrows(
        () -> new OfflineNoteV2.ProofBox(" \n ", new byte[] {1}),
        "blank proof backend should throw");
    assertThrows(
        () -> new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, new byte[0]),
        "empty proof bytes should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.RecursiveProofV2(
                new byte[31],
                new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, new byte[] {1})),
        "short public input hash should throw");
    final byte[] nonCanonicalHash = Arrays.copyOf(publicInputsHash, publicInputsHash.length);
    nonCanonicalHash[31] = (byte) (nonCanonicalHash[31] & 0xFE);
    assertThrows(
        () ->
            new OfflineNoteV2.RecursiveProofV2(
                nonCanonicalHash,
                new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, new byte[] {1})),
        "noncanonical public input hash should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("", "vk"),
        "blank verifier backend should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("halo2:ipa", "vk"),
        "colon verifier backend should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("halo2/ipa", "bad:vk"),
        "colon verifier name should throw");
  }

  private static void certificateValidationRejectsMalformedValues() throws Exception {
    final Map<String, Object> cert = obj(obj(loadFixture(), "payment_token"), "sender_key_certificate");
    final byte[] publicKey = base64Bytes(string(cert, "public_key"));
    final byte[] assertionPublicKey = base64Bytes(string(cert, "assertion_public_key"));
    final byte[] issuerSignature = base64Bytes(string(cert, "issuer_signature_base64"));

    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificateV2(
                OfflineNoteV2.KEY_CERTIFICATE_VERSION + 1,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                issuerSignature),
        "bad certificate version should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificateV2(
                OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                false,
                issuerSignature),
        "non-one-use certificate should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificateV2(
                OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                Arrays.copyOf(publicKey, 31),
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                issuerSignature),
        "short note public key should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificateV2(
                OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                -1,
                true,
                issuerSignature),
        "negative assertion usage limit should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificateV2(
                OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                Arrays.copyOf(issuerSignature, 63)),
        "short issuer signature should throw");
  }

  private static void auditBundleRejectsInvalidShapesAndUncommittedOutputs() throws Exception {
    final OfflineNoteV2.AuditBundleV2 audit = audit(loadFixture());
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                Collections.emptyList(),
                audit.inputClaims(),
                audit.outputCommitments(),
                audit.outputClaims(),
                audit.recursiveProof()),
        "empty audit input nullifiers should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                Collections.emptyList(),
                audit.outputCommitments(),
                audit.outputClaims(),
                audit.recursiveProof()),
        "empty audit input claims should throw");
    final List<byte[]> tooManyNullifiers = new ArrayList<>(audit.inputNullifiers());
    tooManyNullifiers.add(audit.inputNullifiers().get(0));
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                tooManyNullifiers,
                audit.inputClaims(),
                audit.outputCommitments(),
                audit.outputClaims(),
                audit.recursiveProof()),
        "audit input count mismatch should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                Collections.emptyList(),
                audit.outputClaims(),
                audit.recursiveProof()),
        "empty audit output commitments should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                audit.outputCommitments(),
                Collections.emptyList(),
                audit.recursiveProof()),
        "empty audit output claims should throw");
    final OfflineNoteV2.AuditOutputClaimV2 uncommittedOutput =
        new OfflineNoteV2.AuditOutputClaimV2(
            OfflineNoteV2.hash("uncommitted-output".getBytes(StandardCharsets.UTF_8)),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            audit.outputClaims().get(0).amount());
    assertThrows(
        () ->
            new OfflineNoteV2.AuditBundleV2(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                audit.outputCommitments(),
                Arrays.asList(uncommittedOutput),
                audit.recursiveProof()),
        "uncommitted audit output claim should throw");
  }

  private static void issueRedeemPublicInputsAndInstancesRejectMalformedValues() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 cert =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);

    assertThrows(
        () -> new OfflineNoteV2.IssueV2(new byte[31], cert, redeem.assetId(), "5"),
        "short issue commitment should throw");
    assertThrows(
        () -> new OfflineNoteV2.IssueV2(redeem.sourceNoteCommitment(), cert, "cash#branch.sbp", "5"),
        "bad issue asset id should throw");
    assertThrows(
        () -> new OfflineNoteV2.IssueV2(redeem.sourceNoteCommitment(), cert, redeem.assetId(), "not-a-number"),
        "bad issue amount should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.RedeemV2(
                redeem.sourceNoteCommitment(),
                Collections.emptyList(),
                redeem.senderKeyCertificate(),
                redeem.recipient(),
                redeem.assetId(),
                redeem.amount(),
                redeem.recursiveProof()),
        "empty redeem nullifiers should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.RedeemPublicInputsV2(
                new byte[31],
                redeem.inputNullifiers(),
                redeem.senderKeyCertificate().payloadHash(),
                redeem.recipient(),
                redeem.assetId(),
                redeem.amount()),
        "short redeem source commitment should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.RedeemPublicInputsV2(
                redeem.sourceNoteCommitment(),
                redeem.inputNullifiers(),
                new byte[31],
                redeem.recipient(),
                redeem.assetId(),
                redeem.amount()),
        "short redeem key-certificate hash should throw");
    assertThrows(
        () ->
            new OfflineNoteV2.RedeemPublicInputsV2(
                redeem.sourceNoteCommitment(),
                redeem.inputNullifiers(),
                redeem.senderKeyCertificate().payloadHash(),
                redeem.recipient() + "@bad",
                redeem.assetId(),
                redeem.amount()),
        "bad redeem recipient should throw");

    final OfflineNoteV2.AuditOutputClaimV2 overLimitOutput =
        new OfflineNoteV2.AuditOutputClaimV2(
            OfflineNoteV2.hash("third-output".getBytes(StandardCharsets.UTF_8)),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            "0");
    final List<byte[]> tooManyCommitments = new ArrayList<>(audit.outputCommitments());
    tooManyCommitments.add(overLimitOutput.noteCommitment());
    final List<OfflineNoteV2.AuditOutputClaimV2> tooManyClaims =
        new ArrayList<>(audit.outputClaims());
    tooManyClaims.add(overLimitOutput);
    final OfflineNoteV2.AuditBundleV2 tooManyOutputs =
        new OfflineNoteV2.AuditBundleV2(
            audit.tokenId(),
            audit.senderKeyCertificate(),
            audit.inputNullifiers(),
            audit.inputClaims(),
            tooManyCommitments,
            tooManyClaims,
            audit.recursiveProof());
    assertThrows(
        () -> OfflineNoteV2.InstanceBuilder.auditInstanceValues(tooManyOutputs),
        "too many audit outputs should throw");

    final OfflineNoteV2.AuditOutputClaimV2 unconservedOutput =
        new OfflineNoteV2.AuditOutputClaimV2(
            audit.outputClaims().get(0).noteCommitment(),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            "6");
    final OfflineNoteV2.AuditBundleV2 unconservedAudit =
        new OfflineNoteV2.AuditBundleV2(
            audit.tokenId(),
            audit.senderKeyCertificate(),
            audit.inputNullifiers(),
            audit.inputClaims(),
            audit.outputCommitments(),
            Arrays.asList(unconservedOutput, audit.outputClaims().get(1)),
            audit.recursiveProof());
    assertThrows(
        () -> OfflineNoteV2.InstanceBuilder.auditInstanceValues(unconservedAudit),
        "unconserved audit amounts should throw");
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
