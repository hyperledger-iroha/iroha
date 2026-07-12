package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
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

public final class AttestedOfflineNoteTest {
  private static final String RETIRED_ISSUE_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::IssueOfflineNoteV2";
  private static final String RETIRED_REDEEM_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::RedeemOfflineNoteV2";
  private static final String RETIRED_AUDIT_INSTRUCTION_ALIAS_SCHEMA =
      "iroha_data_model::isi::offline::AuditOfflineNoteV2";

  private AttestedOfflineNoteTest() {}

  public static void main(final String[] args) throws Exception {
    certificateSigningBytesMatchRustVector();
    attestedOfflineNoteModelsMatchRustNoritoVectors();
    attestedOfflineNoteDecodersRoundTripRustNoritoVectors();
    attestedOfflineNoteDecodersRejectMalformedPayloads();
    attestedOfflineNoteInstructionWrappersProduceSchemaBoundPayloads();
    attestedOfflineNoteInstructionWrappersRejectProofMismatches();
    attestedOfflineNoteInstructionDecodersReadExplorerEnvelopeBytes();
    attestedOfflineNoteInstructionDecodersRejectRetiredAliasEnvelopeBytes();
    attestedOfflineNoteInstructionDecodersRejectWrongEnvelopeShapes();
    publicInputHashesMatchRustVectors();
    proofBindingRejectsMismatch();
    proofVerifierAndHashValidationRejectsMalformedValues();
    openVerifyEnvelopeRejectsNonExactPublicInputHashBeforeDecoding();
    openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields();
    certificateValidationRejectsMalformedValues();
    offlineDeviceAttestationRegistrationMatchesRustVectors();
    offlineDeviceAttestationRegistrationDraftBuildsChallengeBeforeEvidence();
    androidKeyMintChallengeBuildsBeforeKeyGeneration();
    offlineDeviceAttestationRegistrationValidationRejectsMalformedValues();
    offlineDeviceAttestationRegistrationDefensivelyCopiesMutableByteArrays();
    auditBundleRejectsInvalidShapesAndUncommittedOutputs();
    issueRedeemPublicInputsAndInstancesRejectMalformedValues();
    attestedOfflineNoteDomainsRejectSubstitutionAndPadding();
    attestedOfflineNoteAssetScopeDataspaceIdsRejectNonCanonicalForms();
    instanceValuesMatchRustVectors();
    nativeHalo2ProverProducesVerifyingPayloadWhenRequested();
    nativeHalo2ProverPerformanceWhenRequested();
    qrFixtureUsesSdkTextPrefix();
    System.out.println("[IrohaAndroid] AttestedOfflineNoteTest passed.");
  }

  private static void certificateSigningBytesMatchRustVector() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.KeyCertificate sender =
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

  private static void attestedOfflineNoteModelsMatchRustNoritoVectors() throws Exception {
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

  private static void attestedOfflineNoteDecodersRoundTripRustNoritoVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final AttestedOfflineNote.KeyCertificate sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final AttestedOfflineNote.Issue issue = issue(fixture);
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
        attestationRegistration(fixture);

    final byte[] certificatePayloadBytes = sender.signingPayload().noritoEncoded();
    final byte[] certificateBytes = sender.noritoEncoded();
    final byte[] issuedClaimBytes = issue.issuedClaim().noritoEncoded();
    final byte[] auditOutputClaimBytes = AttestedOfflineNote.encodeAuditOutputClaim(audit.outputClaims().get(0));
    final byte[] recursiveProofBytes = AttestedOfflineNote.encodeRecursiveProof(audit.recursiveProof());
    final byte[] redeemPublicInputsBytes = redeem.publicInputs().noritoEncoded();
    final byte[] auditPublicInputsBytes = audit.publicInputs().noritoEncoded();
    final byte[] issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"));
    final byte[] auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"));
    final byte[] redeemBytes = base64Bytes(string(obj(chain, "redeem"), "norito_base64"));
    final byte[] registrationBytes =
        base64Bytes(string(obj(chain, "attestation_registration"), "norito_base64"));

    assertEquals(
        base64(certificatePayloadBytes),
        base64(AttestedOfflineNote.decodeCertificatePayload(certificatePayloadBytes).noritoEncoded()),
        "decoded certificate payload");
    assertEquals(
        base64(certificateBytes),
        base64(AttestedOfflineNote.decodeCertificate(certificateBytes).noritoEncoded()),
        "decoded certificate");
    assertEquals(
        base64(issuedClaimBytes),
        base64(AttestedOfflineNote.decodeIssuedClaim(issuedClaimBytes).noritoEncoded()),
        "decoded issued claim");
    assertEquals(
        base64(auditOutputClaimBytes),
        base64(
            AttestedOfflineNote.encodeAuditOutputClaim(
                AttestedOfflineNote.decodeAuditOutputClaim(auditOutputClaimBytes))),
        "decoded audit output claim");
    assertEquals(
        base64(recursiveProofBytes),
        base64(
            AttestedOfflineNote.encodeRecursiveProof(
                AttestedOfflineNote.decodeRecursiveProof(recursiveProofBytes))),
        "decoded recursive proof");
    assertEquals(
        base64(redeemPublicInputsBytes),
        base64(AttestedOfflineNote.decodeRedeemPublicInputs(redeemPublicInputsBytes).noritoEncoded()),
        "decoded redeem public inputs");
    assertEquals(
        base64(auditPublicInputsBytes),
        base64(AttestedOfflineNote.decodeAuditPublicInputs(auditPublicInputsBytes).noritoEncoded()),
        "decoded audit public inputs");
    assertEquals(
        base64(issueBytes),
        base64(AttestedOfflineNote.decodeIssue(issueBytes).noritoEncoded()),
        "decoded issue");

    final AttestedOfflineNote.AuditBundle decodedAudit = AttestedOfflineNote.decodeAudit(auditBytes);
    decodedAudit.validateProofBinding();
    assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()), "decoded audit");

    final AttestedOfflineNote.Redeem decodedRedeem = AttestedOfflineNote.decodeRedeem(redeemBytes);
    decodedRedeem.validateProofBinding();
    assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()), "decoded redeem");

    assertEquals(
        base64(registrationBytes),
        base64(
            AttestedOfflineNote.decodeDeviceAttestationRegistration(registrationBytes)
                .noritoEncoded()),
        "decoded device attestation registration");
  }

  private static void attestedOfflineNoteDecodersRejectMalformedPayloads() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final byte[] issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"));
    final AttestedOfflineNote.KeyCertificate sender =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final byte[] certificatePayloadBytes = sender.signingPayload().noritoEncoded();

    assertThrows(
        () -> AttestedOfflineNote.decodeIssue(Arrays.copyOf(issueBytes, issueBytes.length - 1)),
        "truncated issue decode should fail");
    assertThrows(
        () -> AttestedOfflineNote.decodeRedeem(issueBytes),
        "schema-mismatched issue decode should fail");
    final byte[] corruptedIssue = Arrays.copyOf(issueBytes, issueBytes.length);
    corruptedIssue[corruptedIssue.length - 1] ^= 0x01;
    assertThrows(
        () -> AttestedOfflineNote.decodeIssue(corruptedIssue),
        "checksum-corrupted issue decode should fail");
    assertThrows(
        () -> AttestedOfflineNote.decodeCertificate(certificatePayloadBytes),
        "certificate payload cannot decode as full certificate");
  }

  private static void attestedOfflineNoteInstructionWrappersProduceSchemaBoundPayloads()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.Issue issue = issue(fixture);
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
        attestationRegistration(fixture);

    assertEquals(
        "iroha_data_model::isi::offline::IssueOfflineNote",
        AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA,
        "canonical issue instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::RedeemOfflineNote",
        AttestedOfflineNote.REDEEM_INSTRUCTION_SCHEMA,
        "canonical redeem instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::AuditOfflineNote",
        AttestedOfflineNote.AUDIT_INSTRUCTION_SCHEMA,
        "canonical audit instruction wire name");
    assertEquals(
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation",
        AttestedOfflineNote.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
        "canonical device attestation registration instruction wire name");
    assertTrue(
        !AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA.endsWith("V2"),
        "issue instruction wire name must be chain-canonical");
    assertTrue(
        !AttestedOfflineNote.REDEEM_INSTRUCTION_SCHEMA.endsWith("V2"),
        "redeem instruction wire name must be chain-canonical");
    assertTrue(
        !AttestedOfflineNote.AUDIT_INSTRUCTION_SCHEMA.endsWith("V2"),
        "audit instruction wire name must be chain-canonical");
    assertTrue(
        !AttestedOfflineNote.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA.endsWith("V2"),
        "device attestation instruction wire name must be chain-canonical");

    assertInstructionWrapper(
        AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA,
        AttestedOfflineNote.encodeIssue(issue),
        AttestedOfflineNote.issueInstruction(issue));
    assertInstructionWrapper(
        AttestedOfflineNote.AUDIT_INSTRUCTION_SCHEMA,
        AttestedOfflineNote.encodeAudit(audit),
        AttestedOfflineNote.auditInstruction(audit));
    assertInstructionWrapper(
        AttestedOfflineNote.REDEEM_INSTRUCTION_SCHEMA,
        AttestedOfflineNote.encodeRedeem(redeem),
        AttestedOfflineNote.redeemInstruction(redeem));
    assertEquals(
        string(obj(obj(fixture, "chain_vectors"), "attestation_registration"), "instruction_norito_base64"),
        base64(wirePayloadBytes(AttestedOfflineNote.registerDeviceAttestationInstruction(registration))),
        "register device attestation instruction norito");
  }

  private static void attestedOfflineNoteInstructionWrappersRejectProofMismatches()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.RecursiveProof badProof =
        new AttestedOfflineNote.RecursiveProof(
            AttestedOfflineNote.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new AttestedOfflineNote.ProofBox(
                AttestedOfflineNote.RECURSIVE_BACKEND,
                "offline-v2-forged-proof".getBytes(StandardCharsets.UTF_8)));

    assertThrows(
        () -> AttestedOfflineNote.redeemInstruction(redeem.replacingRecursiveProof(badProof)),
        "forged redeem instruction should throw");
    assertThrows(
        () -> AttestedOfflineNote.auditInstruction(audit.replacingRecursiveProof(badProof)),
        "forged audit instruction should throw");
  }

  private static void attestedOfflineNoteInstructionDecodersReadExplorerEnvelopeBytes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.Issue issue = issue(fixture);
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
        attestationRegistration(fixture);
    final byte[] issueWirePayload = wirePayloadBytes(AttestedOfflineNote.issueInstruction(issue));
    final byte[] auditWirePayload = wirePayloadBytes(AttestedOfflineNote.auditInstruction(audit));
    final byte[] redeemWirePayload = wirePayloadBytes(AttestedOfflineNote.redeemInstruction(redeem));
    final byte[] registrationWirePayload =
        wirePayloadBytes(AttestedOfflineNote.registerDeviceAttestationInstruction(registration));

    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            AttestedOfflineNote.decodeIssueInstruction(
                    rawInstructionPair(AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload))
                .noritoEncoded()),
        "decoded issue instruction");
    assertEquals(
        base64(issue.noritoEncoded()),
        base64(
            AttestedOfflineNote.decodeIssueInstruction(
                    rawInstructionPair(AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload, false))
                .noritoEncoded()),
        "decoded non-compact issue instruction");
    assertEquals(
        base64(issue.noritoEncoded()),
        base64(AttestedOfflineNote.decodeIssueInstruction(issueWirePayload).noritoEncoded()),
        "decoded direct issue instruction payload");
    assertEquals(
        base64(registration.noritoEncoded()),
        base64(
            AttestedOfflineNote.decodeRegisterDeviceAttestationInstruction(
                    rawInstructionPair(
                        AttestedOfflineNote.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
                        registrationWirePayload))
                .noritoEncoded()),
        "decoded register device attestation instruction");
    assertEquals(
        base64(registration.noritoEncoded()),
        base64(
            AttestedOfflineNote.decodeRegisterDeviceAttestationInstruction(registrationWirePayload)
                .noritoEncoded()),
        "decoded direct register device attestation instruction payload");

    final AttestedOfflineNote.AuditBundle decodedAudit =
        AttestedOfflineNote.decodeAuditInstruction(
            rawInstructionPair(AttestedOfflineNote.AUDIT_INSTRUCTION_SCHEMA, auditWirePayload));
    decodedAudit.validateProofBinding();
    assertEquals(
        base64(audit.noritoEncoded()),
        base64(decodedAudit.noritoEncoded()),
        "decoded audit instruction");

    final AttestedOfflineNote.Redeem decodedRedeem =
        AttestedOfflineNote.decodeRedeemInstruction(
            rawInstructionPair(AttestedOfflineNote.REDEEM_INSTRUCTION_SCHEMA, redeemWirePayload));
    decodedRedeem.validateProofBinding();
    assertEquals(
        base64(redeem.noritoEncoded()),
        base64(decodedRedeem.noritoEncoded()),
        "decoded redeem instruction");
  }

  private static void attestedOfflineNoteInstructionDecodersRejectRetiredAliasEnvelopeBytes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.Issue issue = issue(fixture);
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final byte[] issueAliasWirePayload =
        encodeInstructionWrapper(RETIRED_ISSUE_INSTRUCTION_ALIAS_SCHEMA, AttestedOfflineNote.encodeIssue(issue));
    final byte[] auditAliasWirePayload =
        encodeInstructionWrapper(RETIRED_AUDIT_INSTRUCTION_ALIAS_SCHEMA, AttestedOfflineNote.encodeAudit(audit));
    final byte[] redeemAliasWirePayload =
        encodeInstructionWrapper(RETIRED_REDEEM_INSTRUCTION_ALIAS_SCHEMA, AttestedOfflineNote.encodeRedeem(redeem));

    assertThrows(
        () -> AttestedOfflineNote.decodeIssueInstruction(issueAliasWirePayload),
        "retired issue instruction alias should throw");
    assertThrows(
        () -> AttestedOfflineNote.decodeAuditInstruction(auditAliasWirePayload),
        "retired audit instruction alias should throw");
    assertThrows(
        () -> AttestedOfflineNote.decodeRedeemInstruction(redeemAliasWirePayload),
        "retired redeem instruction alias should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeIssueInstruction(
                rawInstructionPair(RETIRED_ISSUE_INSTRUCTION_ALIAS_SCHEMA, issueAliasWirePayload)),
        "retired issue instruction alias envelope should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeAuditInstruction(
                rawInstructionPair(RETIRED_AUDIT_INSTRUCTION_ALIAS_SCHEMA, auditAliasWirePayload)),
        "retired audit instruction alias envelope should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeRedeemInstruction(
                rawInstructionPair(RETIRED_REDEEM_INSTRUCTION_ALIAS_SCHEMA, redeemAliasWirePayload)),
        "retired redeem instruction alias envelope should throw");
  }

  private static void attestedOfflineNoteInstructionDecodersRejectWrongEnvelopeShapes()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.Issue issue = issue(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
        attestationRegistration(fixture);
    final byte[] issueWirePayload = wirePayloadBytes(AttestedOfflineNote.issueInstruction(issue));
    final byte[] redeemWirePayload = wirePayloadBytes(AttestedOfflineNote.redeemInstruction(redeem));
    final byte[] retiredRegisterWrapperPayload =
        encodeInstructionWrapper(
            AttestedOfflineNote.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
            AttestedOfflineNote.encodeDeviceAttestationRegistration(registration));
    final byte[] issuePair =
        rawInstructionPair(AttestedOfflineNote.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload);

    assertThrows(
        () ->
            AttestedOfflineNote.decodeIssueInstruction(
                rawInstructionPair(AttestedOfflineNote.REDEEM_INSTRUCTION_SCHEMA, issueWirePayload)),
        "wrong issue wire name should throw");
    assertThrows(
        () -> AttestedOfflineNote.decodeRedeemInstruction(issuePair),
        "wrong instruction model schema should throw");
    assertThrows(
        () -> AttestedOfflineNote.decodeIssueInstruction(issue.noritoEncoded()),
        "direct model frame should not decode as instruction wrapper");
    assertThrows(
        () -> AttestedOfflineNote.decodeIssueInstruction(Arrays.copyOf(issuePair, issuePair.length - 1)),
        "truncated instruction envelope should throw");
    final byte[] corruptedWirePayload = Arrays.copyOf(issueWirePayload, issueWirePayload.length);
    corruptedWirePayload[corruptedWirePayload.length - 1] ^= 0x01;
    assertThrows(
        () -> AttestedOfflineNote.decodeIssueInstruction(corruptedWirePayload),
        "checksum-corrupted instruction payload should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeAuditInstruction(
                rawInstructionPair(AttestedOfflineNote.AUDIT_INSTRUCTION_SCHEMA, redeemWirePayload)),
        "wrong audit instruction model schema should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeRegisterDeviceAttestationInstruction(retiredRegisterWrapperPayload),
        "retired register device attestation generic wrapper should throw");
    assertThrows(
        () ->
            AttestedOfflineNote.decodeRegisterDeviceAttestationInstruction(
                rawInstructionPair(
                    AttestedOfflineNote.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
                    retiredRegisterWrapperPayload)),
        "retired register device attestation generic wrapper envelope should throw");
  }

  private static void publicInputHashesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);

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
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.RecursiveProof badProof =
        new AttestedOfflineNote.RecursiveProof(
            AttestedOfflineNote.hash("wrong-public-inputs".getBytes(StandardCharsets.UTF_8)),
            new AttestedOfflineNote.ProofBox(
                AttestedOfflineNote.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8)));
    final AttestedOfflineNote.Redeem forged =
        new AttestedOfflineNote.Redeem(
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
    final AttestedOfflineNote.ProofBox proof =
        new AttestedOfflineNote.ProofBox(AttestedOfflineNote.RECURSIVE_BACKEND, new byte[] {1});
    assertEquals(AttestedOfflineNote.RECURSIVE_BACKEND, proof.backend(), "exact proof backend");

    assertThrows(
        () -> new AttestedOfflineNote.ProofBox("  " + AttestedOfflineNote.RECURSIVE_BACKEND + "  ", new byte[] {1}),
        "padded proof backend should throw");
    assertThrows(
        () -> new AttestedOfflineNote.ProofBox(" \n ", new byte[] {1}),
        "blank proof backend should throw");
    assertThrows(
        () -> new AttestedOfflineNote.ProofBox(AttestedOfflineNote.RECURSIVE_BACKEND, new byte[0]),
        "empty proof bytes should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.RecursiveProof(
                new byte[31],
                new AttestedOfflineNote.ProofBox(AttestedOfflineNote.RECURSIVE_BACKEND, new byte[] {1})),
        "short public input hash should throw");
    final byte[] nonCanonicalHash = Arrays.copyOf(publicInputsHash, publicInputsHash.length);
    nonCanonicalHash[31] = (byte) (nonCanonicalHash[31] & 0xFE);
    assertThrows(
        () ->
            new AttestedOfflineNote.RecursiveProof(
                nonCanonicalHash,
                new AttestedOfflineNote.ProofBox(AttestedOfflineNote.RECURSIVE_BACKEND, new byte[] {1})),
        "noncanonical public input hash should throw");
    assertThrows(
        () -> new AttestedOfflineNote.VerifyingKeyIdReference("", "vk"),
        "blank verifier backend should throw");
    assertThrows(
        () -> new AttestedOfflineNote.VerifyingKeyIdReference(" halo2/ipa ", "vk"),
        "padded verifier backend should throw");
    assertThrows(
        () -> new AttestedOfflineNote.VerifyingKeyIdReference("halo2/ipa", " vk "),
        "padded verifier name should throw");
    assertThrows(
        () -> new AttestedOfflineNote.VerifyingKeyIdReference("halo2:ipa", "vk"),
        "colon verifier backend should throw");
    assertThrows(
        () -> new AttestedOfflineNote.VerifyingKeyIdReference("halo2/ipa", "bad:vk"),
        "colon verifier name should throw");
  }

  private static void openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields()
      throws Exception {
    final long[] values =
        AttestedOfflineNote.InstanceBuilder.auditInstanceValues(audit(loadFixture())).publicValues();
    final byte[] payload = fakeZk1ProofPayload(new byte[] {1, 2, 3}, values);
    final byte[] envelope = AttestedOfflineNoteHalo2Prover.openVerifyEnvelope(payload);

    assertTrue(
        !AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(envelope, repeat("00", 32)),
        "mismatched public input hash should decode the envelope and return false");

    assertIllegalArgumentContains(
        () ->
            AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
                AttestedOfflineNoteHalo2Prover.openVerifyEnvelope(new byte[0]),
                repeat("00", 32)),
        "OpenVerifyEnvelope proof payload is empty");

    assertIllegalArgumentContains(
        () ->
            AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
                rawOpenVerifyEnvelopeWithCircuitPayload(
                    concat(openEnvelopeStringPayload(AttestedOfflineNoteHalo2Prover.CIRCUIT_ID),
                        new byte[] {0})),
                values),
        "Trailing bytes after OpenVerifyEnvelope field decode");
  }

  private static void openVerifyEnvelopeRejectsNonExactPublicInputHashBeforeDecoding() {
    final String canonicalHash = repeat("ab", 32);
    for (final String rejectedHash : nonExactPublicInputHashes(canonicalHash)) {
      assertTrue(
          !AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(new byte[0], rejectedHash),
          "Java Offline V2 Halo2 helper rejects non-exact public input hash before decoding");
    }
  }

  private static void certificateValidationRejectsMalformedValues() throws Exception {
    final Map<String, Object> cert = obj(obj(loadFixture(), "payment_token"), "sender_key_certificate");
    final byte[] publicKey = base64Bytes(string(cert, "public_key"));
    final byte[] assertionPublicKey = base64Bytes(string(cert, "assertion_public_key"));
    final byte[] issuerSignature = base64Bytes(string(cert, "issuer_signature_base64"));

    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION + 1,
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
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
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
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
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
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                "\u00A0\u2003",
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                issuerSignature),
        "blank certificate key_id should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificatePayload(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                "\u00A0\u2003",
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true),
        "blank certificate payload device_id should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
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
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
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
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                offCurveP256AssertionPublicKey(),
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                issuerSignature),
        "off-curve assertion public key should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                "apple-app-attest-v1",
                string(cert, "assertion_key_algorithm"),
                assertionPublicKey,
                nullableInt(cert, "assertion_usage_count_limit"),
                true,
                issuerSignature),
        "spliced iOS certificate profile should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificatePayload(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                "ios-app-attest",
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                "apple-app-attest-v1",
                "ecdsa-p256-sha256",
                assertionPublicKey,
                null,
                true),
        "retired iOS certificate payload profile should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificate(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                "ios-app-attest",
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                "apple-app-attest-v1",
                "ecdsa-p256-sha256",
                assertionPublicKey,
                null,
                true,
                issuerSignature),
        "retired iOS certificate profile should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificatePayload(
                AttestedOfflineNote.KEY_CERTIFICATE_VERSION,
                string(cert, "platform"),
                string(cert, "key_id"),
                string(cert, "device_id"),
                string(cert, "account_id"),
                publicKey,
                string(cert, "assertion_scheme"),
                string(cert, "assertion_key_algorithm"),
                offCurveP256AssertionPublicKey(),
                nullableInt(cert, "assertion_usage_count_limit"),
                true),
        "off-curve payload assertion public key should throw");
  }

  private static void offlineDeviceAttestationRegistrationMatchesRustVectors()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
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
    final AttestedOfflineNote.KeyCertificatePayload keyCertificatePayload =
        registration.keyCertificatePayload();
    assertEquals(
        string(vector, "key_certificate_payload_hash"),
        hex(keyCertificatePayload.payloadHash()),
        "registration key certificate payload helper hash");
    assertEquals(
        base64(keyCertificatePayload.noritoEncoded()),
        base64(
            AttestedOfflineNote.decodeCertificatePayload(keyCertificatePayload.noritoEncoded())
                .noritoEncoded()),
        "registration key certificate payload helper round-trip");
    assertEquals(
        string(vector, "norito_base64"),
        base64(registration.noritoEncoded()),
        "device attestation registration norito");
  }

  private static void offlineDeviceAttestationRegistrationDraftBuildsChallengeBeforeEvidence()
      throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final String androidSigningDigestHex =
        nullableString(vector, "android_signing_certificate_sha256");
    final byte[] preAttestationChallenge =
        AttestedOfflineNote.DeviceAttestationRegistration.preAttestationChallengeHash(
            intValue(vector, "version"),
            string(vector, "platform"),
            string(vector, "key_id"),
            string(vector, "device_id"),
            string(vector, "account_id"),
            nullableString(vector, "asset_definition_id"),
            nullableString(vector, "ios_team_id"),
            nullableString(vector, "ios_bundle_id"),
            nullableString(vector, "ios_environment"),
            nullableString(vector, "android_package_name"),
            androidSigningDigestHex == null ? null : hexBytes(androidSigningDigestHex),
            base64Bytes(string(vector, "public_key")),
            string(vector, "assertion_scheme"),
            string(vector, "assertion_key_algorithm"),
            nullableInt(vector, "assertion_usage_count_limit"),
            bool(vector, "one_use"),
            longValue(vector, "recent_block_height"),
            hexBytes(string(vector, "recent_block_hash")),
            longValue(vector, "expires_at_ms"));
    final AttestedOfflineNote.DeviceAttestationRegistration draft =
        new AttestedOfflineNote.DeviceAttestationRegistration(
            intValue(vector, "version"),
            string(vector, "platform"),
            string(vector, "key_id"),
            string(vector, "device_id"),
            string(vector, "account_id"),
            nullableString(vector, "asset_definition_id"),
            nullableString(vector, "ios_team_id"),
            nullableString(vector, "ios_bundle_id"),
            nullableString(vector, "ios_environment"),
            nullableString(vector, "android_package_name"),
            androidSigningDigestHex == null ? null : hexBytes(androidSigningDigestHex),
            base64Bytes(string(vector, "public_key")),
            string(vector, "assertion_scheme"),
            string(vector, "assertion_key_algorithm"),
            base64Bytes(string(vector, "assertion_public_key")),
            nullableInt(vector, "assertion_usage_count_limit"),
            bool(vector, "one_use"),
            null,
            null,
            null,
            null,
            null,
            longValue(vector, "recent_block_height"),
            hexBytes(string(vector, "recent_block_hash")),
            longValue(vector, "expires_at_ms"));
    final byte[] emptyReportHash = AttestedOfflineNote.hash(new byte[0]);
    final byte[] expectedEvidence = attestationEvidence(emptyReportHash);

    assertEquals(
        string(vector, "challenge_hash"),
        hex(draft.canonicalChallengeHash()),
        "draft device attestation canonical challenge hash");
    assertArrayEquals(
        preAttestationChallenge,
        draft.challengeHash(),
        "pre-attestation challenge helper parity");
    assertEquals(
        string(vector, "challenge_hash"),
        hex(draft.challengeHash()),
        "draft device attestation challenge hash");
    assertArrayEquals(emptyReportHash, draft.attestationReportHash(), "draft report hash");
    assertArrayEquals(new byte[0], draft.attestationReport(), "draft report");
    assertArrayEquals(expectedEvidence, draft.evidence(), "draft evidence");
    assertArrayEquals(
        AttestedOfflineNote.hash(expectedEvidence), draft.evidenceHash(), "draft evidence hash");
  }

  private static void androidKeyMintChallengeBuildsBeforeKeyGeneration() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "android_keymint_challenge");
    final byte[] signingDigest =
        hexBytes(string(vector, "android_signing_certificate_sha256"));
    final byte[] challenge =
        AttestedOfflineNote.DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash(
            intValue(vector, "version"),
            string(vector, "device_id"),
            string(vector, "account_id"),
            nullableString(vector, "asset_definition_id"),
            nullableString(vector, "ios_team_id"),
            nullableString(vector, "ios_bundle_id"),
            nullableString(vector, "ios_environment"),
            string(vector, "android_package_name"),
            signingDigest,
            base64Bytes(string(vector, "public_key")),
            string(vector, "assertion_scheme"),
            string(vector, "assertion_key_algorithm"),
            nullableInt(vector, "assertion_usage_count_limit"),
            bool(vector, "one_use"),
            longValue(vector, "recent_block_height"),
            hexBytes(string(vector, "recent_block_hash")),
            longValue(vector, "expires_at_ms"));
    assertEquals(
        string(vector, "challenge_hash"),
        hex(challenge),
        "Android pre-key-generation challenge must match the Rust vector");

    final byte[] changedDigest = Arrays.copyOf(signingDigest, signingDigest.length);
    changedDigest[0] ^= 0x01;
    final byte[] changedChallenge =
        AttestedOfflineNote.DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash(
            intValue(vector, "version"),
            string(vector, "device_id"),
            string(vector, "account_id"),
            nullableString(vector, "asset_definition_id"),
            nullableString(vector, "ios_team_id"),
            nullableString(vector, "ios_bundle_id"),
            nullableString(vector, "ios_environment"),
            string(vector, "android_package_name"),
            changedDigest,
            base64Bytes(string(vector, "public_key")),
            string(vector, "assertion_scheme"),
            string(vector, "assertion_key_algorithm"),
            nullableInt(vector, "assertion_usage_count_limit"),
            bool(vector, "one_use"),
            longValue(vector, "recent_block_height"),
            hexBytes(string(vector, "recent_block_hash")),
            longValue(vector, "expires_at_ms"));
    assertTrue(
        !Arrays.equals(challenge, changedChallenge),
        "Android signing identity substitution must change the challenge");
    assertThrows(
        () ->
            AttestedOfflineNote.DeviceAttestationRegistration
                .androidPreKeyGenerationChallengeHash(
                    intValue(vector, "version"),
                    string(vector, "device_id"),
                    string(vector, "account_id"),
                    nullableString(vector, "asset_definition_id"),
                    nullableString(vector, "ios_team_id"),
                    nullableString(vector, "ios_bundle_id"),
                    nullableString(vector, "ios_environment"),
                    string(vector, "android_package_name"),
                    signingDigest,
                    base64Bytes(string(vector, "public_key")),
                    "android-keymint-ecdsa-p256-usage-limit",
                    string(vector, "assertion_key_algorithm"),
                    nullableInt(vector, "assertion_usage_count_limit"),
                    bool(vector, "one_use"),
                    longValue(vector, "recent_block_height"),
                    hexBytes(string(vector, "recent_block_hash")),
                    longValue(vector, "expires_at_ms")),
        "retired Android assertion profile must not produce a challenge");
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
    final byte[] forgedEvidenceReportHash = new byte[32];
    Arrays.fill(forgedEvidenceReportHash, (byte) 0xA5);
    final byte[] forgedEvidence = attestationEvidence(forgedEvidenceReportHash);
    assertThrows(
        () ->
            attestationRegistration(
                fixture,
                null,
                null,
                AttestedOfflineNote.hash(forgedEvidence),
                null,
                null,
                null,
                null,
                forgedEvidence,
                null,
                null,
                null),
        "forged device attestation evidence envelope should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture, null, null, null, null, new byte[31], null, null, null, null, null, null),
        "short device attestation note public key should throw");
    assertThrows(
        () -> attestationRegistrationWithProfileAndKeyId(
            fixture,
            null,
            "not standard base64!",
            null,
            null,
            null),
        "malformed iOS App Attest key_id should throw");
    assertThrows(
        () -> attestationRegistrationWithProfileAndKeyId(
            fixture,
            null,
            "AB==",
            null,
            null,
            null),
        "non-canonical iOS App Attest key_id should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfileAndKeyId(
                fixture,
                null,
                " " + string(vector, "key_id") + " ",
                null,
                null,
                null),
        "padded device attestation key_id should throw");
    assertThrows(
        () -> attestationRegistrationWithDeviceId(fixture, " " + string(vector, "device_id") + " "),
        "padded device attestation device_id should throw");
    assertThrows(
        () -> attestationRegistrationWithDeviceId(fixture, "   "),
        "blank device attestation device_id should throw");
    assertThrows(
        () -> attestationRegistrationWithDeviceId(fixture, "\u00A0\u2003"),
        "Unicode-blank device attestation device_id should throw");
    assertThrows(
        () ->
            attestationRegistrationWithMetadata(
                fixture, " " + nullableString(vector, "ios_team_id") + " ", null, null, null),
        "padded iOS team id should throw");
    assertThrows(
        () ->
            attestationRegistrationWithMetadata(
                fixture, null, nullableString(vector, "ios_bundle_id") + "\n", null, null),
        "padded iOS bundle id should throw");
    assertThrows(
        () ->
            attestationRegistrationWithMetadata(
                fixture, null, null, "\t" + nullableString(vector, "ios_environment"), null),
        "padded iOS environment should throw");
    assertThrows(
        () ->
            attestationRegistrationWithMetadata(
                fixture, null, null, null, " jp.co.soramitsu.iroha.offline "),
        "padded Android package name should throw");
    assertThrows(
        () ->
            attestationRegistration(
                fixture,
                null,
                null,
                null,
                null,
                null,
                offCurveP256AssertionPublicKey(),
                null,
                null,
                null,
                null,
                null),
        "off-curve device attestation assertion public key should throw");
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
    assertThrows(
        () ->
            attestationRegistrationWithProfile(
                fixture,
                null,
                null,
                null,
                Integer.valueOf(1)),
        "iOS device attestation usage limit should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfile(
                fixture,
                AttestedOfflineNote.ANDROID_KEYMINT_PLATFORM,
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME,
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                null),
        "Android device attestation without usage limit should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfile(
                fixture,
                AttestedOfflineNote.ANDROID_KEYMINT_PLATFORM,
                "android-keymint-ecdsa-p256-usage-limit",
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                Integer.valueOf(1)),
        "Android device attestation retired scheme should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfileAndKeyId(
                fixture,
                AttestedOfflineNote.ANDROID_KEYMINT_PLATFORM,
                "0000000000000000000000000000000000000000000000000000000000000000",
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME,
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                Integer.valueOf(1)),
        "Android device attestation key_id mismatch should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfileAndKeyId(
                fixture,
                AttestedOfflineNote.ANDROID_KEYMINT_PLATFORM,
                hex(sha256(base64Bytes(string(vector, "assertion_public_key"))))
                    .toUpperCase(Locale.ROOT),
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME,
                AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                Integer.valueOf(1)),
        "Android device attestation uppercase key_id should throw");
    assertThrows(
        () ->
            attestationRegistrationWithProfile(
                fixture,
                "ios-app-attest",
                null,
                null,
                null),
        "unsupported device attestation platform should throw");
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
    final AttestedOfflineNote.DeviceAttestationRegistration registration =
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
    final AttestedOfflineNote.AuditBundle audit = audit(loadFixture());
    assertThrows(
        () ->
            new AttestedOfflineNote.AuditBundle(
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
            new AttestedOfflineNote.AuditBundle(
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
            new AttestedOfflineNote.AuditBundle(
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
            new AttestedOfflineNote.AuditBundle(
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
            new AttestedOfflineNote.AuditBundle(
                audit.tokenId(),
                audit.senderKeyCertificate(),
                audit.inputNullifiers(),
                audit.inputClaims(),
                audit.outputCommitments(),
                Collections.emptyList(),
                audit.recursiveProof()),
        "empty audit output claims should throw");
    final AttestedOfflineNote.AuditOutputClaim uncommittedOutput =
        new AttestedOfflineNote.AuditOutputClaim(
            AttestedOfflineNote.hash("uncommitted-output".getBytes(StandardCharsets.UTF_8)),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            audit.outputClaims().get(0).amount());
    assertThrows(
        () ->
            new AttestedOfflineNote.AuditBundle(
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
    final AttestedOfflineNote.KeyCertificate cert =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);

    assertThrows(
        () -> new AttestedOfflineNote.Issue(new byte[31], cert, redeem.assetId(), "5"),
        "short issue commitment should throw");
    assertThrows(
        () -> new AttestedOfflineNote.Issue(redeem.sourceNoteCommitment(), cert, "cash#branch.sbp", "5"),
        "bad issue asset id should throw");
    assertThrows(
        () -> new AttestedOfflineNote.Issue(redeem.sourceNoteCommitment(), cert, redeem.assetId(), "not-a-number"),
        "bad issue amount should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.Redeem(
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
            new AttestedOfflineNote.RedeemPublicInputs(
                new byte[31],
                redeem.inputNullifiers(),
                redeem.senderKeyCertificate().payloadHash(),
                redeem.recipient(),
                redeem.assetId(),
                redeem.amount()),
        "short redeem source commitment should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.RedeemPublicInputs(
                redeem.sourceNoteCommitment(),
                redeem.inputNullifiers(),
                new byte[31],
                redeem.recipient(),
                redeem.assetId(),
                redeem.amount()),
        "short redeem key-certificate hash should throw");
    assertThrows(
        () ->
            new AttestedOfflineNote.RedeemPublicInputs(
                redeem.sourceNoteCommitment(),
                redeem.inputNullifiers(),
                redeem.senderKeyCertificate().payloadHash(),
                redeem.recipient() + "@bad",
                redeem.assetId(),
                redeem.amount()),
        "bad redeem recipient should throw");

    final AttestedOfflineNote.AuditOutputClaim overLimitOutput =
        new AttestedOfflineNote.AuditOutputClaim(
            AttestedOfflineNote.hash("third-output".getBytes(StandardCharsets.UTF_8)),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            "0");
    final List<byte[]> tooManyCommitments = new ArrayList<>(audit.outputCommitments());
    tooManyCommitments.add(overLimitOutput.noteCommitment());
    final List<AttestedOfflineNote.AuditOutputClaim> tooManyClaims =
        new ArrayList<>(audit.outputClaims());
    tooManyClaims.add(overLimitOutput);
    final AttestedOfflineNote.AuditBundle tooManyOutputs =
        new AttestedOfflineNote.AuditBundle(
            audit.tokenId(),
            audit.senderKeyCertificate(),
            audit.inputNullifiers(),
            audit.inputClaims(),
            tooManyCommitments,
            tooManyClaims,
            audit.recursiveProof());
    assertThrows(
        () -> AttestedOfflineNote.InstanceBuilder.auditInstanceValues(tooManyOutputs),
        "too many audit outputs should throw");

    final AttestedOfflineNote.AuditOutputClaim unconservedOutput =
        new AttestedOfflineNote.AuditOutputClaim(
            audit.outputClaims().get(0).noteCommitment(),
            audit.outputClaims().get(0).keyCertificate(),
            audit.outputClaims().get(0).assetId(),
            "6");
    final AttestedOfflineNote.AuditBundle unconservedAudit =
        new AttestedOfflineNote.AuditBundle(
            audit.tokenId(),
            audit.senderKeyCertificate(),
            audit.inputNullifiers(),
            audit.inputClaims(),
            audit.outputCommitments(),
            Arrays.asList(unconservedOutput, audit.outputClaims().get(1)),
            audit.recursiveProof());
    assertThrows(
        () -> AttestedOfflineNote.InstanceBuilder.auditInstanceValues(unconservedAudit),
        "unconserved audit amounts should throw");
  }

  private static void attestedOfflineNoteDomainsRejectSubstitutionAndPadding() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.KeyCertificate certificate =
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"));
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    final AttestedOfflineNote.IssuedClaim claim = audit.inputClaims().get(0);
    final AttestedOfflineNote.AuditPublicInputs auditPublic = audit.publicInputs();
    final AttestedOfflineNote.RedeemPublicInputs redeemPublic = redeem.publicInputs();

    assertThrows(
        () ->
            new AttestedOfflineNote.KeyCertificatePayload(
                AttestedOfflineNote.KEY_CERTIFICATE_PAYLOAD_DOMAIN + " ",
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
            new AttestedOfflineNote.IssuedClaim(
                AttestedOfflineNote.ISSUED_CLAIM_DOMAIN + "\n",
                claim.noteCommitment(),
                claim.keyCertificatePayloadHash(),
                claim.assetId(),
                claim.amount()),
        "padded issued-claim domain must be rejected");
    assertThrows(
        () ->
            new AttestedOfflineNote.RedeemPublicInputs(
                "forged:" + AttestedOfflineNote.REDEEM_PUBLIC_INPUTS_DOMAIN,
                redeemPublic.sourceNoteCommitment(),
                redeemPublic.inputNullifiers(),
                redeemPublic.keyCertificatePayloadHash(),
                redeemPublic.recipient(),
                redeemPublic.assetId(),
                redeemPublic.amount()),
        "forged redeem-public-inputs domain must be rejected");
    assertThrows(
        () ->
            new AttestedOfflineNote.AuditPublicInputs(
                " " + AttestedOfflineNote.AUDIT_PUBLIC_INPUTS_DOMAIN,
                auditPublic.tokenId(),
                auditPublic.keyCertificatePayloadHash(),
                auditPublic.inputNullifiers(),
                auditPublic.inputClaims(),
                auditPublic.outputCommitments(),
                auditPublic.outputClaims()),
        "padded audit-public-inputs domain must be rejected");
  }

  private static void attestedOfflineNoteAssetScopeDataspaceIdsRejectNonCanonicalForms()
      throws Exception {
    final AttestedOfflineNote.Issue issue = issue(loadFixture());

    new AttestedOfflineNote.Issue(
        issue.noteCommitment(),
        issue.keyCertificate(),
        issue.assetId() + "#dataspace:0",
        issue.amount());
    new AttestedOfflineNote.Issue(
        issue.noteCommitment(),
        issue.keyCertificate(),
        issue.assetId() + "#dataspace:1",
        issue.amount());

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
              new AttestedOfflineNote.Issue(
                  issue.noteCommitment(),
                  issue.keyCertificate(),
                  issue.assetId() + "#" + rejected,
                  issue.amount()),
          "non-canonical V2 dataspace scope should reject: " + rejected);
    }
  }

  private static void instanceValuesMatchRustVectors() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final AttestedOfflineNote.InstanceValues auditValues =
        AttestedOfflineNote.InstanceBuilder.auditInstanceValues(audit(fixture));
    final AttestedOfflineNote.InstanceValues redeemValues =
        AttestedOfflineNote.InstanceBuilder.redeemInstanceValues(redeem(fixture));
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
        hex(AttestedOfflineNote.instanceScalarBytes(auditPublic[0])),
        hex(auditValues.publicInstanceColumns().get(0)),
        "audit first instance scalar");
  }

  private static void nativeHalo2ProverProducesVerifyingPayloadWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_ATTESTED_OFFLINE_NOTE_PROVER_TEST"))) {
      return;
    }
    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.InstanceValues values =
        AttestedOfflineNote.InstanceBuilder.auditInstanceValues(audit);
    AttestedOfflineNoteHalo2Prover.prewarm();
    final byte[] payload = AttestedOfflineNoteHalo2Prover.proveZk1Payload(values);

    assertTrue(
        AttestedOfflineNoteHalo2Prover.verifyZk1Payload(payload, values.publicValues()),
        "Java Offline V2 Halo2 payload verifies");
    final AttestedOfflineNote.RecursiveProof proof = AttestedOfflineNoteHalo2Prover.proveAudit(audit);
    audit.replacingRecursiveProof(proof).validateProofBinding();
    assertTrue(
        proof.proof().bytes().length <= AttestedOfflineNoteHalo2Prover.MAX_ENVELOPE_BYTES,
        "Java Offline V2 Halo2 envelope fits QR budget");
    final String publicInputsHashHex = hex(proof.publicInputsHash());
    assertTrue(
        AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(
            proof.proof().bytes(), publicInputsHashHex),
        "Java Offline V2 Halo2 SDK envelope helper verifies public input hash");
    for (final String rejectedHash : nonExactPublicInputHashes(publicInputsHashHex)) {
      assertTrue(
          !AttestedOfflineNoteHalo2Prover.verifyOpenVerifyEnvelope(proof.proof().bytes(), rejectedHash),
          "Java Offline V2 Halo2 SDK envelope helper rejects non-exact public input hash");
    }
  }

  private static void nativeHalo2ProverPerformanceWhenRequested() throws Exception {
    if (!"1".equals(System.getenv("IROHA_JAVA_ATTESTED_OFFLINE_NOTE_BENCH"))) {
      return;
    }
    final String configuredIterations =
        System.getenv("IROHA_JAVA_ATTESTED_OFFLINE_NOTE_BENCH_ITERATIONS");
    final int iterations =
        configuredIterations == null ? 20 : Integer.parseInt(configuredIterations);
    assertTrue(iterations > 0, "Java Offline V2 benchmark iterations must be positive");

    final Map<String, Object> fixture = loadFixture();
    final AttestedOfflineNote.AuditBundle audit = audit(fixture);
    final AttestedOfflineNote.Redeem redeem = redeem(fixture);
    AttestedOfflineNoteHalo2Prover.prewarm();
    AttestedOfflineNoteHalo2Prover.proveAudit(audit);
    AttestedOfflineNoteHalo2Prover.proveRedeem(redeem);

    final double[] auditSeconds =
        benchmarkSeconds(iterations, () -> AttestedOfflineNoteHalo2Prover.proveAudit(audit));
    final double[] redeemSeconds =
        benchmarkSeconds(iterations, () -> AttestedOfflineNoteHalo2Prover.proveRedeem(redeem));
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

  private static AttestedOfflineNote.Issue issue(final Map<String, Object> fixture) {
    final Map<String, Object> chainIssue = obj(obj(fixture, "chain_vectors"), "issue");
    return new AttestedOfflineNote.Issue(
        hexBytes(string(chainIssue, "note_commitment")),
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
        string(chainIssue, "asset_id"),
        string(chainIssue, "amount"));
  }

  private static AttestedOfflineNote.Redeem redeem(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "redeem");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    return new AttestedOfflineNote.Redeem(
        hexBytes(string(vector, "source_note_commitment")),
        hexList(vector, "input_nullifiers"),
        certificate(obj(payment, "recipient_key_certificate")),
        string(payment, "recipient_account_id"),
        string(vector, "asset_id"),
        string(vector, "amount"),
        new AttestedOfflineNote.RecursiveProof(
            hexBytes(string(vector, "public_inputs_hash")),
            new AttestedOfflineNote.ProofBox(
                AttestedOfflineNote.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static AttestedOfflineNote.AuditBundle audit(final Map<String, Object> fixture) {
    final Map<String, Object> vector = obj(obj(fixture, "chain_vectors"), "audit");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final List<AttestedOfflineNote.IssuedClaim> inputClaims = new ArrayList<>();
    for (final Object item : list(payment, "input_claims")) {
      inputClaims.add(issuedClaim(asMap(item, "input claim")));
    }
    final List<AttestedOfflineNote.AuditOutputClaim> outputClaims = new ArrayList<>();
    for (final Object item : list(payment, "output_claims")) {
      outputClaims.add(auditOutputClaim(asMap(item, "output claim")));
    }
    return new AttestedOfflineNote.AuditBundle(
        hexBytes(string(vector, "token_id")),
        certificate(obj(payment, "sender_key_certificate")),
        hexList(vector, "input_nullifiers"),
        inputClaims,
        hexList(vector, "output_commitments"),
        outputClaims,
        new AttestedOfflineNote.RecursiveProof(
            hexBytes(string(vector, "public_inputs_hash")),
            new AttestedOfflineNote.ProofBox(
                AttestedOfflineNote.RECURSIVE_BACKEND,
                "offline-v2-vector-audit-proof".getBytes(StandardCharsets.UTF_8))));
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
      final Map<String, Object> fixture) {
    return attestationRegistration(fixture, null, null, null);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
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

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
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
    return attestationRegistration(
        fixture,
        challengeHash,
        attestationReportHash,
        evidenceHash,
        androidSigningCertificateSha256,
        publicKey,
        assertionPublicKey,
        attestationReport,
        evidence,
        recentBlockHash,
        oneUse,
        assetDefinitionId,
        null,
        null,
        null,
        null);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistrationWithProfile(
      final Map<String, Object> fixture,
      final String platform,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final Integer assertionUsageCountLimit) {
    return attestationRegistration(
        fixture,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        platform,
        assertionScheme,
        assertionKeyAlgorithm,
        assertionUsageCountLimit);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration
      attestationRegistrationWithProfileAndKeyId(
          final Map<String, Object> fixture,
          final String platform,
          final String keyId,
          final String assertionScheme,
          final String assertionKeyAlgorithm,
          final Integer assertionUsageCountLimit) {
    return attestationRegistration(
        fixture,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        platform,
        assertionScheme,
        assertionKeyAlgorithm,
        assertionUsageCountLimit,
        keyId,
        null);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration
      attestationRegistrationWithDeviceId(
          final Map<String, Object> fixture,
          final String deviceId) {
    return attestationRegistration(
        fixture,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        deviceId);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration
      attestationRegistrationWithMetadata(
          final Map<String, Object> fixture,
          final String iosTeamId,
          final String iosBundleId,
          final String iosEnvironment,
          final String androidPackageName) {
    return attestationRegistration(
        fixture,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        iosTeamId,
        iosBundleId,
        iosEnvironment,
        androidPackageName);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
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
      final String assetDefinitionId,
      final String platform,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final Integer assertionUsageCountLimit) {
    return attestationRegistration(
        fixture,
        challengeHash,
        attestationReportHash,
        evidenceHash,
        androidSigningCertificateSha256,
        publicKey,
        assertionPublicKey,
        attestationReport,
        evidence,
        recentBlockHash,
        oneUse,
        assetDefinitionId,
        platform,
        assertionScheme,
        assertionKeyAlgorithm,
        assertionUsageCountLimit,
        null,
        null);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
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
      final String assetDefinitionId,
      final String platform,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final Integer assertionUsageCountLimit,
      final String keyId,
      final String deviceId) {
    return attestationRegistration(
        fixture,
        challengeHash,
        attestationReportHash,
        evidenceHash,
        androidSigningCertificateSha256,
        publicKey,
        assertionPublicKey,
        attestationReport,
        evidence,
        recentBlockHash,
        oneUse,
        assetDefinitionId,
        platform,
        assertionScheme,
        assertionKeyAlgorithm,
        assertionUsageCountLimit,
        keyId,
        deviceId,
        null,
        null,
        null,
        null);
  }

  private static AttestedOfflineNote.DeviceAttestationRegistration attestationRegistration(
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
      final String assetDefinitionId,
      final String platform,
      final String assertionScheme,
      final String assertionKeyAlgorithm,
      final Integer assertionUsageCountLimit,
      final String keyId,
      final String deviceId,
      final String iosTeamId,
      final String iosBundleId,
      final String iosEnvironment,
      final String androidPackageName) {
    final Map<String, Object> vector =
        obj(obj(fixture, "chain_vectors"), "attestation_registration");
    final String androidSigningDigestHex =
        nullableString(vector, "android_signing_certificate_sha256");
    return new AttestedOfflineNote.DeviceAttestationRegistration(
        intValue(vector, "version"),
        platform == null ? string(vector, "platform") : platform,
        keyId == null ? string(vector, "key_id") : keyId,
        deviceId == null ? string(vector, "device_id") : deviceId,
        string(vector, "account_id"),
        assetDefinitionId == null ? nullableString(vector, "asset_definition_id") : assetDefinitionId,
        iosTeamId == null ? nullableString(vector, "ios_team_id") : iosTeamId,
        iosBundleId == null ? nullableString(vector, "ios_bundle_id") : iosBundleId,
        iosEnvironment == null ? nullableString(vector, "ios_environment") : iosEnvironment,
        androidPackageName == null ? nullableString(vector, "android_package_name") : androidPackageName,
        androidSigningCertificateSha256 != null
            ? androidSigningCertificateSha256
            : androidSigningDigestHex == null ? null : hexBytes(androidSigningDigestHex),
        publicKey == null ? base64Bytes(string(vector, "public_key")) : publicKey,
        assertionScheme == null ? string(vector, "assertion_scheme") : assertionScheme,
        assertionKeyAlgorithm == null
            ? string(vector, "assertion_key_algorithm")
            : assertionKeyAlgorithm,
        assertionPublicKey == null
            ? base64Bytes(string(vector, "assertion_public_key"))
            : assertionPublicKey,
        assertionUsageCountLimit == null
            ? nullableInt(vector, "assertion_usage_count_limit")
            : assertionUsageCountLimit,
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

  private static AttestedOfflineNote.KeyCertificate certificate(final Map<String, Object> json) {
    return new AttestedOfflineNote.KeyCertificate(
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

  private static AttestedOfflineNote.IssuedClaim issuedClaim(final Map<String, Object> json) {
    return new AttestedOfflineNote.IssuedClaim(
        string(json, "domain"),
        hexBytes(string(json, "note_commitment")),
        hexBytes(string(json, "key_certificate_payload_hash")),
        string(json, "asset_id"),
        string(json, "amount"));
  }

  private static AttestedOfflineNote.AuditOutputClaim auditOutputClaim(
      final Map<String, Object> json) {
    return new AttestedOfflineNote.AuditOutputClaim(
        hexBytes(string(json, "note_commitment")),
        certificate(obj(json, "key_certificate")),
        string(json, "asset_definition_id") + "#" + string(json, "account_id"),
        string(json, "amount"));
  }

  private static void assertInstructionWrapper(
      final String schema, final byte[] modelPayload, final InstructionBox instruction) {
    assertEquals(schema, instruction.name(), "instruction wire name");
    if (!(instruction.payload() instanceof InstructionBox.WirePayload wire)) {
      throw new AssertionError("Attested Offline Note instruction must use a wire payload");
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
      throw new AssertionError("Attested Offline Note instruction must use a wire payload");
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
      instances.writeBytes(AttestedOfflineNote.instanceScalarBytes(value));
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
                child -> child.writeUInt(AttestedOfflineNoteHalo2Prover.BACKEND_TAG, 32));
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
        final String json = new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
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

  private static byte[] sha256(final byte[] bytes) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(bytes);
    } catch (final NoSuchAlgorithmException ex) {
      throw new AssertionError("SHA-256 must be available", ex);
    }
  }

  private static byte[] attestationEvidence(final byte[] attestationReportHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.writeBytes(
        AttestedOfflineNote.DEVICE_ATTESTATION_EVIDENCE_PREFIX.getBytes(StandardCharsets.UTF_8));
    out.writeBytes(attestationReportHash);
    return out.toByteArray();
  }

  private static byte[] offCurveP256AssertionPublicKey() {
    final byte[] key = new byte[65];
    key[0] = 0x04;
    return key;
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

  private static void assertArrayEquals(
      final byte[] expected, final byte[] actual, final String message) {
    if (!Arrays.equals(expected, actual)) {
      throw new AssertionError(
          message + ": expected " + hex(expected) + " but got " + hex(actual));
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
