package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
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
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class OfflineNoteV2Test {
  private static final String ISSUE_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::IssueOfflineNoteV2";
  private static final String REDEEM_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::RedeemOfflineNoteV2";
  private static final String AUDIT_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::AuditOfflineNoteV2";

  private OfflineNoteV2Test() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    offlineNoteV2ModelsMatchRustNoritoVectors();
    offlineNoteV2DecodersRoundTripRustNoritoVectors();
    offlineNoteV2DecodersRejectMalformedPayloads();
    offlineNoteV2InstructionWrappersProduceSchemaBoundPayloads();
    offlineNoteV2InstructionWrappersRejectProofMismatches();
    offlineNoteV2InstructionDecodersReadExplorerEnvelopeBytes();
    offlineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes();
    offlineNoteV2InstructionDecodersRejectWrongEnvelopeShapes();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    proofVerifierAndHashValidationRejectsMalformedValues();
    openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields();
    certificateValidationRejectsMalformedValues();
    offlineDeviceAttestationRegistrationMatchesRustVectors();
    offlineDeviceAttestationRegistrationValidationRejectsMalformedValues();
    offlineDeviceAttestationRegistrationDefensivelyCopiesMutableByteArrays();
    auditBundleRejectsInvalidShapesAndUncommittedOutputs();
    issueRedeemPublicInputsAndInstancesRejectMalformedValues();
    offlineNoteV2DomainsRejectSubstitutionAndPadding();
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
    assertEquals(
        string(obj(chain, "attestation_registration"), "norito_base64"),
        base64(attestationRegistration(fixture).noritoEncoded()),
        "device attestation registration norito");
  }

  private static void offlineNoteV2DecodersRoundTripRustNoritoVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final OfflineNoteV2.KeyCertificateV2 sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.DeviceAttestationRegistrationV2 registration =
        attestationRegistration(fixture);

    final byte[] certificatePayloadBytes = sender.signingPayload().noritoEncoded();
    final byte[] certificateBytes = sender.noritoEncoded();
    final byte[] issuedClaimBytes = issue.issuedClaim().noritoEncoded();
    final byte[] auditOutputClaimBytes = OfflineNoteV2.encodeAuditOutputClaim(audit.outputClaims().get(0));
    final byte[] recursiveProofBytes = OfflineNoteV2.encodeRecursiveProof(audit.recursiveProof());
    final byte[] redeemPublicInputsBytes = redeem.publicInputs().noritoEncoded();
    final byte[] auditPublicInputsBytes = audit.publicInputs().noritoEncoded();
    final byte[] issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"));
    final byte[] auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"));
    final byte[] redeemBytes = base64Bytes(string(obj(chain, "redeem"), "norito_base64"));
    final byte[] registrationBytes =
        base64Bytes(string(obj(chain, "attestation_registration"), "norito_base64"));

    assertEquals(
        base64(certificatePayloadBytes),
        base64(OfflineNoteV2.decodeCertificatePayload(certificatePayloadBytes).noritoEncoded()),
        "decoded certificate payload");
    assertEquals(
        base64(certificateBytes),
        base64(OfflineNoteV2.decodeCertificate(certificateBytes).noritoEncoded()),
        "decoded certificate");
    assertEquals(
        base64(issuedClaimBytes),
        base64(OfflineNoteV2.decodeIssuedClaim(issuedClaimBytes).noritoEncoded()),
        "decoded issued claim");
    assertEquals(
        base64(auditOutputClaimBytes),
        base64(
            OfflineNoteV2.encodeAuditOutputClaim(
                OfflineNoteV2.decodeAuditOutputClaim(auditOutputClaimBytes))),
        "decoded audit output claim");
    assertEquals(
        base64(recursiveProofBytes),
        base64(
            OfflineNoteV2.encodeRecursiveProof(
                OfflineNoteV2.decodeRecursiveProof(recursiveProofBytes))),
        "decoded recursive proof");
    assertEquals(
        base64(redeemPublicInputsBytes),
        base64(OfflineNoteV2.decodeRedeemPublicInputs(redeemPublicInputsBytes).noritoEncoded()),
        "decoded redeem public inputs");
    assertEquals(
        base64(auditPublicInputsBytes),
        base64(OfflineNoteV2.decodeAuditPublicInputs(auditPublicInputsBytes).noritoEncoded()),
        "decoded audit public inputs");
    assertEquals(
        base64(issueBytes),
        base64(OfflineNoteV2.decodeIssue(issueBytes).noritoEncoded()),
        "decoded issue");

    final OfflineNoteV2.AuditBundleV2 decodedAudit = OfflineNoteV2.decodeAudit(auditBytes);
    decodedAudit.validateProofBinding();
    assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()), "decoded audit");

    final OfflineNoteV2.RedeemV2 decodedRedeem = OfflineNoteV2.decodeRedeem(redeemBytes);
    decodedRedeem.validateProofBinding();
    assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()), "decoded redeem");

    assertEquals(
        base64(registrationBytes),
        base64(
            OfflineNoteV2.decodeDeviceAttestationRegistration(registrationBytes)
                .noritoEncoded()),
        "decoded device attestation registration");
  }

  private static void offlineNoteV2DecodersRejectMalformedPayloads() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final byte[] issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"));
    final OfflineNoteV2.KeyCertificateV2 sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final byte[] certificatePayloadBytes = sender.signingPayload().noritoEncoded();

    assertThrows(
        () -> OfflineNoteV2.decodeIssue(Arrays.copyOf(issueBytes, issueBytes.length - 1)),
        "truncated issue decode should fail");
    assertThrows(
        () -> OfflineNoteV2.decodeRedeem(issueBytes),
        "schema-mismatched issue decode should fail");
    final byte[] corruptedIssue = Arrays.copyOf(issueBytes, issueBytes.length);
    corruptedIssue[corruptedIssue.length - 1] ^= 0x01;
    assertThrows(
        () -> OfflineNoteV2.decodeIssue(corruptedIssue),
        "checksum-corrupted issue decode should fail");
    assertThrows(
        () -> OfflineNoteV2.decodeCertificate(certificatePayloadBytes),
        "certificate payload cannot decode as full certificate");
  }

  private static void offlineNoteV2InstructionWrappersProduceSchemaBoundPayloads()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.DeviceAttestationRegistrationV2 registration =
        attestationRegistration(fixture);

    assertEquals(
        "iroha_data_model::isi::offline::IssueOfflineNote",
        OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
        "canonical issue instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::RedeemOfflineNote",
        OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
        "canonical redeem instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::AuditOfflineNote",
        OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
        "canonical audit instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation",
        OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
        "canonical device attestation registration instruction wire name");
    assertTrue(
        !OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA.endsWith("V2"),
        "issue instruction wire name must be chain-canonical");
    assertTrue(
        !OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA.endsWith("V2"),
        "redeem instruction wire name must be chain-canonical");
    assertTrue(
        !OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA.endsWith("V2"),
        "audit instruction wire name must be chain-canonical");
    assertTrue(
        !OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA.endsWith("V2"),
        "device attestation instruction wire name must be chain-canonical");

    assertInstructionWrapper(
        OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
        OfflineNoteV2.encodeIssue(issue),
        OfflineNoteV2.issueInstruction(issue));
    assertInstructionWrapper(
        OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
        OfflineNoteV2.encodeAudit(audit),
        OfflineNoteV2.auditInstruction(audit));
    assertInstructionWrapper(
        OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
        OfflineNoteV2.encodeRedeem(redeem),
        OfflineNoteV2.redeemInstruction(redeem));
    assertInstructionWrapper(
        OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
        OfflineNoteV2.encodeDeviceAttestationRegistration(registration),
        OfflineNoteV2.registerDeviceAttestationInstruction(registration));
    assertEquals(
        string(obj(obj(fixture, "chain_vectors"), "attestation_registration"), "instruction_norito_base64"),
        base64(wirePayloadBytes(OfflineNoteV2.registerDeviceAttestationInstruction(registration))),
        "register device attestation instruction norito");
  }

  private static void offlineNoteV2InstructionWrappersRejectProofMismatches()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.RecursiveProofV2 badProof =
        new OfflineNoteV2.RecursiveProofV2(
            OfflineNoteV2.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-forged-proof".getBytes(StandardCharsets.UTF_8)));

    assertThrows(
        () -> OfflineNoteV2.redeemInstruction(redeem.replacingRecursiveProof(badProof)),
        "forged redeem instruction should throw");
    assertThrows(
        () -> OfflineNoteV2.auditInstruction(audit.replacingRecursiveProof(badProof)),
        "forged audit instruction should throw");
  }

  private static void offlineNoteV2InstructionDecodersReadExplorerEnvelopeBytes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.DeviceAttestationRegistrationV2 registration =
        attestationRegistration(fixture);
    final byte[] issueWirePayload = wirePayloadBytes(OfflineNoteV2.issueInstruction(issue));
    final byte[] auditWirePayload = wirePayloadBytes(OfflineNoteV2.auditInstruction(audit));
    final byte[] redeemWirePayload = wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem));
    final byte[] registrationWirePayload =
        wirePayloadBytes(OfflineNoteV2.registerDeviceAttestationInstruction(registration));

    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeIssueInstruction(
                    rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload))
                .noritoEncoded()),
        "decoded issue instruction");
    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeIssueInstruction(
                    rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload, false))
                .noritoEncoded()),
        "decoded non-compact issue instruction");
    assertEquals(
        base64(issue.noritoEncoded()),
        base64(OfflineNoteV2.decodeIssueInstruction(issueWirePayload).noritoEncoded()),
        "decoded direct issue instruction payload");
    assertEquals(
        base64(registration.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeRegisterDeviceAttestationInstruction(
                    rawInstructionPair(
                        OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
                        registrationWirePayload))
                .noritoEncoded()),
        "decoded register device attestation instruction");
    assertEquals(
        base64(registration.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeRegisterDeviceAttestationInstruction(registrationWirePayload)
                .noritoEncoded()),
        "decoded direct register device attestation instruction payload");

    final OfflineNoteV2.AuditBundleV2 decodedAudit =
        OfflineNoteV2.decodeAuditInstruction(
            rawInstructionPair(OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA, auditWirePayload));
    decodedAudit.validateProofBinding();
    assertEquals(
        base64(audit.noritoEncoded()),
        base64(decodedAudit.noritoEncoded()),
        "decoded audit instruction");

    final OfflineNoteV2.RedeemV2 decodedRedeem =
        OfflineNoteV2.decodeRedeemInstruction(
            rawInstructionPair(OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA, redeemWirePayload));
    decodedRedeem.validateProofBinding();
    assertEquals(
        base64(redeem.noritoEncoded()),
        base64(decodedRedeem.noritoEncoded()),
        "decoded redeem instruction");
  }

  private static void offlineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final byte[] issueAliasWirePayload =
        encodeInstructionWrapper(ISSUE_INSTRUCTION_ALIAS_SCHEMA, OfflineNoteV2.encodeIssue(issue));
    final byte[] auditAliasWirePayload =
        encodeInstructionWrapper(AUDIT_INSTRUCTION_ALIAS_SCHEMA, OfflineNoteV2.encodeAudit(audit));
    final byte[] redeemAliasWirePayload =
        encodeInstructionWrapper(REDEEM_INSTRUCTION_ALIAS_SCHEMA, OfflineNoteV2.encodeRedeem(redeem));

    assertEquals(
        base64(issue.noritoEncoded()),
        base64(OfflineNoteV2.decodeIssueInstruction(issueAliasWirePayload).noritoEncoded()),
        "decoded direct legacy alias issue instruction payload");
    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeIssueInstruction(
                    rawInstructionPair(ISSUE_INSTRUCTION_ALIAS_SCHEMA, issueAliasWirePayload))
                .noritoEncoded()),
        "decoded legacy alias issue instruction envelope");
    assertEquals(
        base64(audit.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeAuditInstruction(
                    rawInstructionPair(AUDIT_INSTRUCTION_ALIAS_SCHEMA, auditAliasWirePayload))
                .noritoEncoded()),
        "decoded legacy alias audit instruction envelope");
    assertEquals(
        base64(redeem.noritoEncoded()),
        base64(
            OfflineNoteV2.decodeRedeemInstruction(
                    rawInstructionPair(REDEEM_INSTRUCTION_ALIAS_SCHEMA, redeemAliasWirePayload))
                .noritoEncoded()),
        "decoded legacy alias redeem instruction envelope");
  }

  private static void offlineNoteV2InstructionDecodersRejectWrongEnvelopeShapes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.IssueV2 issue = issue(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final byte[] issueWirePayload = wirePayloadBytes(OfflineNoteV2.issueInstruction(issue));
    final byte[] redeemWirePayload = wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem));
    final byte[] issuePair =
        rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload);

    assertThrows(
        () ->
            OfflineNoteV2.decodeIssueInstruction(
                rawInstructionPair(OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA, issueWirePayload)),
        "wrong issue wire name should throw");
    assertThrows(
        () -> OfflineNoteV2.decodeRedeemInstruction(issuePair),
        "wrong instruction model schema should throw");
    assertThrows(
        () -> OfflineNoteV2.decodeIssueInstruction(issue.noritoEncoded()),
        "direct model frame should not decode as instruction wrapper");
    assertThrows(
        () -> OfflineNoteV2.decodeIssueInstruction(Arrays.copyOf(issuePair, issuePair.length - 1)),
        "truncated instruction envelope should throw");
    final byte[] corruptedWirePayload = Arrays.copyOf(issueWirePayload, issueWirePayload.length);
    corruptedWirePayload[corruptedWirePayload.length - 1] ^= 0x01;
    assertThrows(
        () -> OfflineNoteV2.decodeIssueInstruction(corruptedWirePayload),
        "checksum-corrupted instruction payload should throw");
    assertThrows(
        () ->
            OfflineNoteV2.decodeAuditInstruction(
                rawInstructionPair(OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA, redeemWirePayload)),
        "wrong audit instruction model schema should throw");
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
    final OfflineNoteV2.ProofBox proof =
        new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, new byte[] {1});
    assertEquals(OfflineNoteV2.RECURSIVE_BACKEND, proof.backend(), "exact proof backend");

    assertThrows(
        () -> new OfflineNoteV2.ProofBox("  " + OfflineNoteV2.RECURSIVE_BACKEND + "  ", new byte[] {1}),
        "padded proof backend should throw");
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
        () -> new OfflineNoteV2.VerifyingKeyIdReference(" halo2/ipa ", "vk"),
        "padded verifier backend should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("halo2/ipa", " vk "),
        "padded verifier name should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("halo2:ipa", "vk"),
        "colon verifier backend should throw");
    assertThrows(
        () -> new OfflineNoteV2.VerifyingKeyIdReference("halo2/ipa", "bad:vk"),
        "colon verifier name should throw");
  }

  private static void openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields()
      throws Exception {
    final long[] values =
        OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit(loadFixture())).publicValues();
    final byte[] payload = fakeZk1ProofPayload(new byte[] {1, 2, 3}, values);
    final byte[] envelope = OfflineNoteV2Halo2Prover.openVerifyEnvelope(payload);

    assertTrue(
        !OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(envelope, repeat("00", 32)),
        "mismatched public input hash should decode the envelope and return false");

    assertIllegalArgumentContains(
        () ->
            OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(
                OfflineNoteV2Halo2Prover.openVerifyEnvelope(new byte[0]),
                repeat("00", 32)),
        "OpenVerifyEnvelope proof payload is empty");

    assertIllegalArgumentContains(
        () ->
            OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(
                rawOpenVerifyEnvelopeWithCircuitPayload(
                    concat(openEnvelopeStringPayload(OfflineNoteV2Halo2Prover.CIRCUIT_ID),
                        new byte[] {0})),
                values),
        "Trailing bytes after OpenVerifyEnvelope field decode");
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

  private static void offlineDeviceAttestationRegistrationMatchesRustVectors()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final OfflineNoteV2.DeviceAttestationRegistrationV2 registration =
        attestationRegistration(fixture);

    assertEquals(
        string(vector, "challenge_hash"),
        hex(registration.canonicalChallengeHash()),
        "device attestation canonical challenge hash");
    assertEquals(
        string(vector, "attestation_report_hash"),
        hex(registration.attestationReportHash()),
        "device attestation report hash");
    assertEquals(
        string(vector, "evidence_hash"),
        hex(registration.evidenceHash()),
        "device attestation evidence hash");
    assertEquals(
        string(vector, "key_certificate_payload_hash"),
        hex(registration.keyCertificatePayloadHash()),
        "device attestation key certificate payload hash");
    assertEquals(
        string(vector, "norito_base64"),
        base64(registration.noritoEncoded()),
        "device attestation registration norito");
  }

  private static void offlineDeviceAttestationRegistrationValidationRejectsMalformedValues()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final byte[] badChallenge = hexBytes(string(vector, "challenge_hash"));
    badChallenge[0] ^= 0x01;
    final byte[] badReportHash = hexBytes(string(vector, "attestation_report_hash"));
    badReportHash[0] ^= 0x01;
    final byte[] badEvidenceHash = hexBytes(string(vector, "evidence_hash"));
    badEvidenceHash[0] ^= 0x01;

    assertThrows(
        () -> attestationRegistration(fixture, badChallenge, null, null),
        "bad device attestation challenge hash should throw");
    assertThrows(
        () -> attestationRegistration(fixture, null, badReportHash, null),
        "bad device attestation report hash should throw");
    assertThrows(
        () -> attestationRegistration(fixture, null, null, new byte[31]),
        "short Android signing certificate digest should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, badEvidenceHash, null, null, null, null, null, null, null, null),
        "bad device attestation evidence hash should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, null, null, new byte[31], null, null, null, null, null, null),
        "short device attestation note public key should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, null, null, null, null, null, null, new byte[31], null, null),
        "short device attestation recent block hash should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, null, null, null, null, null, null, null, false, null),
        "non-one-use device attestation registration should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, null, null, null, null, null, null, null, null, "cash#bad"),
        "bad device attestation asset definition id should throw");
  }

  private static void offlineDeviceAttestationRegistrationDefensivelyCopiesMutableByteArrays()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final byte[] publicKey = base64Bytes(string(vector, "public_key"));
    final byte[] assertionPublicKey = base64Bytes(string(vector, "assertion_public_key"));
    final byte[] attestationReport = base64Bytes(string(vector, "attestation_report_base64"));
    final byte[] evidence = base64Bytes(string(vector, "evidence_base64"));
    final byte[] recentBlockHash = hexBytes(string(vector, "recent_block_hash"));
    final OfflineNoteV2.DeviceAttestationRegistrationV2 registration =
        attestationRegistration(
            fixture,
            null,
            null,
            null,
            null,
            publicKey,
            assertionPublicKey,
            attestationReport,
            evidence,
            recentBlockHash,
            null,
            null);
    final byte[] encoded = registration.noritoEncoded();

    publicKey[0] ^= 0x01;
    assertionPublicKey[0] ^= 0x01;
    attestationReport[0] ^= 0x01;
    evidence[0] ^= 0x01;
    recentBlockHash[0] ^= 0x01;
    assertEquals(string(vector, "norito_base64"), base64(encoded), "original registration bytes");
    assertEquals(base64(encoded), base64(registration.noritoEncoded()), "constructor copies arrays");

    final byte[] returnedPublicKey = registration.publicKey();
    returnedPublicKey[0] ^= 0x01;
    final byte[] returnedReport = registration.attestationReport();
    returnedReport[0] ^= 0x01;
    final byte[] returnedEvidence = registration.evidence();
    returnedEvidence[0] ^= 0x01;
    assertEquals(base64(encoded), base64(registration.noritoEncoded()), "getter copies arrays");
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

  private static void offlineNoteV2DomainsRejectSubstitutionAndPadding() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final OfflineNoteV2.KeyCertificateV2 certificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final OfflineNoteV2.AuditBundleV2 audit = audit(fixture);
    final OfflineNoteV2.RedeemV2 redeem = redeem(fixture);
    final OfflineNoteV2.IssuedClaimV2 claim = audit.inputClaims().get(0);
    final OfflineNoteV2.AuditPublicInputsV2 auditPublic = audit.publicInputs();
    final OfflineNoteV2.RedeemPublicInputsV2 redeemPublic = redeem.publicInputs();

    assertThrows(
        () ->
            new OfflineNoteV2.KeyCertificatePayloadV2(
                OfflineNoteV2.KEY_CERTIFICATE_PAYLOAD_DOMAIN + " ",
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
            new OfflineNoteV2.IssuedClaimV2(
                OfflineNoteV2.ISSUED_CLAIM_DOMAIN + "\n",
                claim.noteCommitment(),
                claim.keyCertificatePayloadHash(),
                claim.assetId(),
                claim.amount()),
        "padded issued-claim domain must be rejected");
    assertThrows(
        () ->
            new OfflineNoteV2.RedeemPublicInputsV2(
                "forged:" + OfflineNoteV2.REDEEM_PUBLIC_INPUTS_DOMAIN,
                redeemPublic.sourceNoteCommitment(),
                redeemPublic.inputNullifiers(),
                redeemPublic.keyCertificatePayloadHash(),
                redeemPublic.recipient(),
                redeemPublic.assetId(),
                redeemPublic.amount()),
        "forged redeem-public-inputs domain must be rejected");
    assertThrows(
        () ->
            new OfflineNoteV2.AuditPublicInputsV2(
                " " + OfflineNoteV2.AUDIT_PUBLIC_INPUTS_DOMAIN,
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

  private static OfflineNoteV2.DeviceAttestationRegistrationV2 attestationRegistration(
      final Map<String, Object> fixture) {
    return attestationRegistration(fixture, null, null, null);
  }

  private static OfflineNoteV2.DeviceAttestationRegistrationV2 attestationRegistration(
      final Map<String, Object> fixture,
      final byte[] challengeHash,
      final byte[] attestationReportHash,
      final byte[] androidSigningCertificateSha256) {
    return attestationRegistration(
        fixture,
        challengeHash,
        attestationReportHash,
        null,
        androidSigningCertificateSha256,
        null,
        null,
        null,
        null,
        null,
        null,
        null);
  }

  private static OfflineNoteV2.DeviceAttestationRegistrationV2 attestationRegistration(
      final Map<String, Object> fixture,
      final byte[] challengeHash,
      final byte[] attestationReportHash,
      final byte[] evidenceHash,
      final byte[] androidSigningCertificateSha256,
      final byte[] publicKey,
      final byte[] assertionPublicKey,
      final byte[] attestationReport,
      final byte[] evidence,
      final byte[] recentBlockHash,
      final Boolean oneUse,
      final String assetDefinitionId) {
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final String androidSigningDigestHex =
        nullableString(vector, "android_signing_certificate_sha256");
    return new OfflineNoteV2.DeviceAttestationRegistrationV2(
        intValue(vector, "version"),
        string(vector, "platform"),
        string(vector, "key_id"),
        string(vector, "device_id"),
        string(vector, "account_id"),
        assetDefinitionId == null ? nullableString(vector, "asset_definition_id") : assetDefinitionId,
        nullableString(vector, "ios_team_id"),
        nullableString(vector, "ios_bundle_id"),
        nullableString(vector, "ios_environment"),
        nullableString(vector, "android_package_name"),
        androidSigningCertificateSha256 != null
            ? androidSigningCertificateSha256
            : androidSigningDigestHex == null ? null : hexBytes(androidSigningDigestHex),
        publicKey == null ? base64Bytes(string(vector, "public_key")) : publicKey,
        string(vector, "assertion_scheme"),
        string(vector, "assertion_key_algorithm"),
        assertionPublicKey == null
            ? base64Bytes(string(vector, "assertion_public_key"))
            : assertionPublicKey,
        nullableInt(vector, "assertion_usage_count_limit"),
        oneUse == null ? bool(vector, "one_use") : oneUse,
        challengeHash == null ? hexBytes(string(vector, "challenge_hash")) : challengeHash,
        attestationReportHash == null
            ? hexBytes(string(vector, "attestation_report_hash"))
            : attestationReportHash,
        attestationReport == null
            ? base64Bytes(string(vector, "attestation_report_base64"))
            : attestationReport,
        evidenceHash == null ? hexBytes(string(vector, "evidence_hash")) : evidenceHash,
        evidence == null ? base64Bytes(string(vector, "evidence_base64")) : evidence,
        longValue(vector, "recent_block_height"),
        recentBlockHash == null ? hexBytes(string(vector, "recent_block_hash")) : recentBlockHash,
        longValue(vector, "expires_at_ms"));
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

  private static void assertInstructionWrapper(
      final String schema, final byte[] modelPayload, final InstructionBox instruction) {
    assertEquals(schema, instruction.name(), "instruction wire name");
    if (!(instruction.payload() instanceof InstructionBox.WirePayload wire)) {
      throw new AssertionError("Offline Note V2 instruction must use a wire payload");
    }
    assertEquals(schema, wire.wireName(), "instruction payload wire name");
    final NoritoHeader.DecodeResult outerFrame = NoritoHeader.decode(wire.payloadBytes(), null);
    assertEquals(
        0,
        outerFrame.header().flags(),
        "instruction wrapper frame flags");
    assertTrue(isNoritoFrame(modelPayload), "public model encoder still returns a framed archive");
    final byte[] wrapperPayload = decodeInstructionWrapper(schema, wire.payloadBytes());
    assertTrue(!isNoritoFrame(wrapperPayload), "instruction wrapper must contain a bare model payload");
  }

  private static byte[] wirePayloadBytes(final InstructionBox instruction) {
    if (!(instruction.payload() instanceof InstructionBox.WirePayload wire)) {
      throw new AssertionError("Offline Note V2 instruction must use a wire payload");
    }
    return wire.payloadBytes();
  }

  private static byte[] fakeZk1ProofPayload(final byte[] proofTranscript, final long[] publicValues) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.writeBytes(new byte[] {0x5A, 0x4B, 0x31, 0x00});
    appendTlv(out, "PROF", proofTranscript);
    final ByteArrayOutputStream instances = new ByteArrayOutputStream();
    writeUInt32Le(instances, 16);
    writeUInt32Le(instances, 1);
    for (final long value : publicValues) {
      instances.writeBytes(OfflineNoteV2.instanceScalarBytes(value));
    }
    appendTlv(out, "I10P", instances.toByteArray());
    return out.toByteArray();
  }

  private static void appendTlv(
      final ByteArrayOutputStream out, final String tag, final byte[] value) {
    out.writeBytes(tag.getBytes(StandardCharsets.UTF_8));
    writeUInt32Le(out, value.length);
    out.writeBytes(value);
  }

  private static void writeUInt32Le(final ByteArrayOutputStream out, final int value) {
    int remaining = value;
    for (int i = 0; i < 4; i++) {
      out.write(remaining & 0xFF);
      remaining >>>= 8;
    }
  }

  private static byte[] rawOpenVerifyEnvelopeWithCircuitPayload(
      final byte[] circuitFieldPayload) {
    return NoritoCodec.encode(
        "raw-open-envelope",
        "iroha_data_model::zk::OpenVerifyEnvelope",
        new TypeAdapter<>() {
          @Override
          public void encode(final NoritoEncoder encoder, final String value) {
            writeOpenEnvelopeField(
                encoder,
                child -> child.writeUInt(OfflineNoteV2Halo2Prover.BACKEND_TAG, 32));
            writeOpenEnvelopeRawField(encoder, circuitFieldPayload);
          }

          @Override
          public String decode(final NoritoDecoder decoder) {
            throw new AssertionError("raw OpenVerifyEnvelope test adapter is encode-only");
          }
        },
        NoritoHeader.COMPACT_LEN);
  }

  private static byte[] openEnvelopeStringPayload(final String value) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    writeInstructionString(encoder, value);
    return encoder.toByteArray();
  }

  private static void writeOpenEnvelopeField(
      final NoritoEncoder encoder, final InstructionFieldWriter writePayload) {
    final NoritoEncoder child = encoder.childEncoder();
    writePayload.write(child);
    writeOpenEnvelopeRawField(encoder, child.toByteArray());
  }

  private static void writeOpenEnvelopeRawField(
      final NoritoEncoder encoder, final byte[] payload) {
    encoder.writeLength(payload.length, compact(encoder));
    encoder.writeBytes(payload);
  }

  private static byte[] rawInstructionPair(final String wireName, final byte[] wirePayload) {
    return rawInstructionPair(wireName, wirePayload, true);
  }

  private static byte[] rawInstructionPair(
      final String wireName, final byte[] wirePayload, final boolean compact) {
    final NoritoEncoder encoder = new NoritoEncoder(compact ? NoritoHeader.COMPACT_LEN : 0);
    writeInstructionField(encoder, child -> writeInstructionString(child, wireName));
    writeInstructionField(encoder, child -> writeInstructionBytesVec(child, wirePayload));
    return encoder.toByteArray();
  }

  private static void writeInstructionField(
      final NoritoEncoder encoder, final InstructionFieldWriter writePayload) {
    final NoritoEncoder child = encoder.childEncoder();
    writePayload.write(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, compact(encoder));
    encoder.writeBytes(payload);
  }

  private static void writeInstructionString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, compact(encoder));
    encoder.writeBytes(bytes);
  }

  private static void writeInstructionBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static byte[] encodeInstructionWrapper(
      final String schema, final byte[] modelPayload) {
    return NoritoCodec.encode(modelPayload, schema, INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER, 0);
  }

  private static byte[] decodeInstructionWrapper(
      final String schema, final byte[] wirePayload) {
    return NoritoCodec.decode(wirePayload, INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER, schema);
  }

  private static final TypeAdapter<byte[]> INSTRUCTION_WRAPPER_PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final byte[] value) {
          final NoritoEncoder child = encoder.childEncoder();
          child.writeBytes(value);
          final byte[] payload = child.toByteArray();
          encoder.writeLength(payload.length, compact(encoder));
          encoder.writeBytes(payload);
        }

        @Override
        public byte[] decode(final NoritoDecoder decoder) {
          final int length = (int) decoder.readLength(compact(decoder));
          final NoritoDecoder child =
              new NoritoDecoder(decoder.readBytes(length), decoder.flags(), decoder.flagsHint());
          final byte[] payload = child.readBytes(child.remaining());
          if (child.remaining() != 0) {
            throw new IllegalArgumentException("trailing bytes in instruction wrapper payload");
          }
          return payload;
        }
      };

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean compact(final NoritoDecoder decoder) {
    return (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static boolean isNoritoFrame(final byte[] bytes) {
    return bytes != null
        && bytes.length >= NoritoHeader.HEADER_LENGTH
        && bytes[0] == (byte) 'N'
        && bytes[1] == (byte) 'R'
        && bytes[2] == (byte) 'T'
        && bytes[3] == (byte) '0';
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

  private static String nullableString(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    return value == null ? null : (String) value;
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

  private static String repeat(final String value, final int count) {
    final StringBuilder builder = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      builder.append(value);
    }
    return builder.toString();
  }

  private static byte[] concat(final byte[] first, final byte[] second) {
    final byte[] out = Arrays.copyOf(first, first.length + second.length);
    System.arraycopy(second, 0, out, first.length, second.length);
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

  @FunctionalInterface
  private interface InstructionFieldWriter {
    void write(NoritoEncoder encoder);
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
