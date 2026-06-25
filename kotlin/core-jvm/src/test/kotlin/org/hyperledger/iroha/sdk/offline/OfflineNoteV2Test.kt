package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.Base64
import java.util.Locale
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

private const val ISSUE_INSTRUCTION_ALIAS_SCHEMA =
    "iroha_data_model::isi::offline::IssueOfflineNoteV2"
private const val REDEEM_INSTRUCTION_ALIAS_SCHEMA =
    "iroha_data_model::isi::offline::RedeemOfflineNoteV2"
private const val AUDIT_INSTRUCTION_ALIAS_SCHEMA =
    "iroha_data_model::isi::offline::AuditOfflineNoteV2"

class OfflineNoteV2Test {
    @Test
    fun certificateSigningBytesMatchRustVector() {
        val fixture = loadFixture()
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val certificates = obj(obj(fixture, "chain_vectors"), "certificates")

        assertEquals(string(certificates, "sender_payload_base64"), base64(sender.signingBytes()))
        assertEquals(string(certificates, "sender_payload_hash"), hex(sender.payloadHash()))
    }

    @Test
    fun offlineNoteV2ModelsMatchRustNoritoVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")

        assertEquals(string(obj(chain, "issue"), "norito_base64"), base64(issue(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "audit"), "norito_base64"), base64(audit(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "redeem"), "norito_base64"), base64(redeem(fixture).noritoEncoded()))
        assertEquals(
            string(obj(chain, "attestation_registration"), "norito_base64"),
            base64(attestationRegistration(fixture).noritoEncoded()),
        )
    }

    @Test
    fun offlineNoteV2DecodersRoundTripRustNoritoVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val registration = attestationRegistration(fixture)

        val certificatePayloadBytes = sender.signingPayload().noritoEncoded()
        val certificateBytes = sender.noritoEncoded()
        val issuedClaimBytes = issue.issuedClaim().noritoEncoded()
        val auditOutputClaimBytes = OfflineNoteV2.encodeAuditOutputClaim(audit.outputClaims[0])
        val recursiveProofBytes = OfflineNoteV2.encodeRecursiveProof(audit.recursiveProof)
        val redeemPublicInputsBytes = redeem.publicInputs().noritoEncoded()
        val auditPublicInputsBytes = audit.publicInputs().noritoEncoded()
        val issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"))
        val auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"))
        val redeemBytes = base64Bytes(string(obj(chain, "redeem"), "norito_base64"))
        val registrationBytes = base64Bytes(string(obj(chain, "attestation_registration"), "norito_base64"))

        assertEquals(
            base64(certificatePayloadBytes),
            base64(OfflineNoteV2.decodeCertificatePayload(certificatePayloadBytes).noritoEncoded()),
        )
        assertEquals(base64(certificateBytes), base64(OfflineNoteV2.decodeCertificate(certificateBytes).noritoEncoded()))
        assertEquals(base64(issuedClaimBytes), base64(OfflineNoteV2.decodeIssuedClaim(issuedClaimBytes).noritoEncoded()))
        assertEquals(
            base64(auditOutputClaimBytes),
            base64(OfflineNoteV2.encodeAuditOutputClaim(OfflineNoteV2.decodeAuditOutputClaim(auditOutputClaimBytes))),
        )
        assertEquals(
            base64(recursiveProofBytes),
            base64(OfflineNoteV2.encodeRecursiveProof(OfflineNoteV2.decodeRecursiveProof(recursiveProofBytes))),
        )
        assertEquals(
            base64(redeemPublicInputsBytes),
            base64(OfflineNoteV2.decodeRedeemPublicInputs(redeemPublicInputsBytes).noritoEncoded()),
        )
        assertEquals(
            base64(auditPublicInputsBytes),
            base64(OfflineNoteV2.decodeAuditPublicInputs(auditPublicInputsBytes).noritoEncoded()),
        )
        assertEquals(base64(issueBytes), base64(OfflineNoteV2.decodeIssue(issueBytes).noritoEncoded()))

        val decodedAudit = OfflineNoteV2.decodeAudit(auditBytes)
        decodedAudit.validateProofBinding()
        assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()))

        val decodedRedeem = OfflineNoteV2.decodeRedeem(redeemBytes)
        decodedRedeem.validateProofBinding()
        assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()))
        assertEquals(
            base64(registration.noritoEncoded()),
            base64(OfflineNoteV2.decodeDeviceAttestationRegistration(registrationBytes).noritoEncoded()),
        )
    }

    @Test
    fun offlineNoteV2DecodersRejectMalformedPayloads() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val issueBytes = base64Bytes(string(obj(chain, "issue"), "norito_base64"))
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val certificatePayloadBytes = sender.signingPayload().noritoEncoded()

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssue(issueBytes.copyOf(issueBytes.size - 1))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeRedeem(issueBytes)
        }
        val corruptedIssue = issueBytes.copyOf()
        corruptedIssue[corruptedIssue.lastIndex] = (corruptedIssue.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssue(corruptedIssue)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeCertificate(certificatePayloadBytes)
        }
    }

    @Test
    fun offlineNoteV2InstructionWrappersProduceSchemaBoundPayloads() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val registration = attestationRegistration(fixture)

        assertEquals(
            "iroha_data_model::isi::offline::IssueOfflineNote",
            OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
            "canonical issue instruction wire name",
        )
        assertEquals(
            "iroha_data_model::isi::offline::RedeemOfflineNote",
            OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
            "canonical redeem instruction wire name",
        )
        assertEquals(
            "iroha_data_model::isi::offline::AuditOfflineNote",
            OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
            "canonical audit instruction wire name",
        )
        assertEquals(
            "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation",
            OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
            "canonical attestation registration instruction wire name",
        )
        assertTrue(!OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA.endsWith("V2"))
        assertTrue(!OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA.endsWith("V2"))
        assertTrue(!OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA.endsWith("V2"))

        assertInstructionWrapper(
            schema = OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
            modelPayload = OfflineNoteV2.encodeIssue(issue),
            instruction = OfflineNoteV2.issueInstruction(issue),
        )
        assertInstructionWrapper(
            schema = OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
            modelPayload = OfflineNoteV2.encodeAudit(audit),
            instruction = OfflineNoteV2.auditInstruction(audit),
        )
        assertInstructionWrapper(
            schema = OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
            modelPayload = OfflineNoteV2.encodeRedeem(redeem),
            instruction = OfflineNoteV2.redeemInstruction(redeem),
        )
        assertInstructionWrapper(
            schema = OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
            modelPayload = OfflineNoteV2.encodeDeviceAttestationRegistration(registration),
            instruction = OfflineNoteV2.registerDeviceAttestationInstruction(registration),
        )
    }

    @Test
    fun offlineDeviceAttestationRegistrationMatchesRustVectors() {
        val fixture = loadFixture()
        val registration = attestationRegistration(fixture)
        val vector = obj(obj(fixture, "chain_vectors"), "attestation_registration")

        assertEquals(string(vector, "challenge_hash"), hex(registration.canonicalChallengeHash()))
        assertEquals(string(vector, "challenge_hash"), hex(registration.challengeHash()))
        assertEquals(string(vector, "attestation_report_hash"), hex(registration.attestationReportHash()))
        assertEquals(string(vector, "evidence_hash"), hex(registration.evidenceHash()))
        assertEquals(string(vector, "key_certificate_payload_hash"), hex(registration.keyCertificatePayloadHash()))
        assertEquals(string(vector, "norito_base64"), base64(registration.noritoEncoded()))

        val changedReport = "other-report".toByteArray(Charsets.UTF_8)
        val changed = registration.replacingAttestationEvidence(
            attestationReport = changedReport,
            evidence = attestationEvidence(OfflineNoteV2.hash(changedReport)),
        )
        assertEquals(registration.canonicalChallengeHash().toList(), changed.canonicalChallengeHash().toList())
        assertTrue(!registration.attestationReportHash().contentEquals(changed.attestationReportHash()))
        assertTrue(!registration.evidenceHash().contentEquals(changed.evidenceHash()))
    }

    @Test
    fun offlineDeviceAttestationRegistrationDraftBuildsChallengeBeforeEvidence() {
        val fixture = loadFixture()
        val vector = obj(obj(fixture, "chain_vectors"), "attestation_registration")
        val draft = OfflineNoteV2.DeviceAttestationRegistrationV2(
            version = int(vector, "version"),
            platform = string(vector, "platform"),
            keyId = string(vector, "key_id"),
            deviceId = string(vector, "device_id"),
            accountId = string(vector, "account_id"),
            assetDefinitionId = nullableString(vector, "asset_definition_id"),
            iosTeamId = nullableString(vector, "ios_team_id"),
            iosBundleId = nullableString(vector, "ios_bundle_id"),
            iosEnvironment = nullableString(vector, "ios_environment"),
            androidPackageName = nullableString(vector, "android_package_name"),
            androidSigningCertificateSha256 =
                nullableString(vector, "android_signing_certificate_sha256")?.let(::hexBytes),
            publicKey = base64Bytes(string(vector, "public_key")),
            assertionScheme = string(vector, "assertion_scheme"),
            assertionKeyAlgorithm = string(vector, "assertion_key_algorithm"),
            assertionPublicKey = base64Bytes(string(vector, "assertion_public_key")),
            assertionUsageCountLimit = nullableInt(vector, "assertion_usage_count_limit"),
            oneUse = bool(vector, "one_use"),
            recentBlockHeight = long(vector, "recent_block_height"),
            recentBlockHash = hexBytes(string(vector, "recent_block_hash")),
            expiresAtMs = long(vector, "expires_at_ms"),
        )
        val emptyReportHash = OfflineNoteV2.hash(ByteArray(0))
        val expectedEvidence = attestationEvidence(emptyReportHash)

        assertEquals(string(vector, "challenge_hash"), hex(draft.canonicalChallengeHash()))
        assertEquals(string(vector, "challenge_hash"), hex(draft.challengeHash()))
        assertEquals(emptyReportHash.toList(), draft.attestationReportHash().toList())
        assertEquals(ByteArray(0).toList(), draft.attestationReport().toList())
        assertEquals(expectedEvidence.toList(), draft.evidence().toList())
        assertEquals(OfflineNoteV2.hash(expectedEvidence).toList(), draft.evidenceHash().toList())
    }

    @Test
    fun offlineNoteV2InstructionWrappersRejectProofMismatches() {
        val fixture = loadFixture()
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val badProof = OfflineNoteV2.RecursiveProofV2(
            publicInputsHash = OfflineNoteV2.hash("wrong-public-inputs".toByteArray()),
            proof = OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-forged-proof".toByteArray()
            )
        )

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.redeemInstruction(redeem.replacingRecursiveProof(badProof))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.auditInstruction(audit.replacingRecursiveProof(badProof))
        }
    }

    @Test
    fun offlineNoteV2InstructionDecodersReadExplorerEnvelopeBytes() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val registration = attestationRegistration(fixture)
        val issueWirePayload = wirePayloadBytes(OfflineNoteV2.issueInstruction(issue))
        val auditWirePayload = wirePayloadBytes(OfflineNoteV2.auditInstruction(audit))
        val redeemWirePayload = wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem))
        val registerWirePayload = wirePayloadBytes(OfflineNoteV2.registerDeviceAttestationInstruction(registration))
        assertEquals(
            string(obj(obj(fixture, "chain_vectors"), "attestation_registration"), "instruction_norito_base64"),
            base64(registerWirePayload),
        )

        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(
                rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload),
            ).noritoEncoded()),
        )
        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(
                rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload, compact = false),
            ).noritoEncoded()),
        )
        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(issueWirePayload).noritoEncoded()),
        )

        val decodedAudit = OfflineNoteV2.decodeAuditInstruction(
            rawInstructionPair(OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA, auditWirePayload),
        )
        decodedAudit.validateProofBinding()
        assertEquals(base64(audit.noritoEncoded()), base64(decodedAudit.noritoEncoded()))

        val decodedRedeem = OfflineNoteV2.decodeRedeemInstruction(
            rawInstructionPair(OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA, redeemWirePayload),
        )
        decodedRedeem.validateProofBinding()
        assertEquals(base64(redeem.noritoEncoded()), base64(decodedRedeem.noritoEncoded()))
        assertEquals(
            base64(registration.noritoEncoded()),
            base64(OfflineNoteV2.decodeRegisterDeviceAttestationInstruction(
                rawInstructionPair(
                    OfflineNoteV2.REGISTER_DEVICE_ATTESTATION_INSTRUCTION_SCHEMA,
                    registerWirePayload,
                ),
            ).noritoEncoded()),
        )
    }

    @Test
    fun offlineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val issueAliasWirePayload = encodeInstructionWrapper(
            ISSUE_INSTRUCTION_ALIAS_SCHEMA,
            OfflineNoteV2.encodeIssue(issue),
        )
        val auditAliasWirePayload = encodeInstructionWrapper(
            AUDIT_INSTRUCTION_ALIAS_SCHEMA,
            OfflineNoteV2.encodeAudit(audit),
        )
        val redeemAliasWirePayload = encodeInstructionWrapper(
            REDEEM_INSTRUCTION_ALIAS_SCHEMA,
            OfflineNoteV2.encodeRedeem(redeem),
        )

        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(issueAliasWirePayload).noritoEncoded()),
        )
        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(
                rawInstructionPair(ISSUE_INSTRUCTION_ALIAS_SCHEMA, issueAliasWirePayload),
            ).noritoEncoded()),
        )
        assertEquals(
            base64(audit.noritoEncoded()),
            base64(OfflineNoteV2.decodeAuditInstruction(
                rawInstructionPair(AUDIT_INSTRUCTION_ALIAS_SCHEMA, auditAliasWirePayload),
            ).noritoEncoded()),
        )
        assertEquals(
            base64(redeem.noritoEncoded()),
            base64(OfflineNoteV2.decodeRedeemInstruction(
                rawInstructionPair(REDEEM_INSTRUCTION_ALIAS_SCHEMA, redeemAliasWirePayload),
            ).noritoEncoded()),
        )
    }

    @Test
    fun offlineNoteV2InstructionDecodersRejectWrongEnvelopeShapes() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val redeem = redeem(fixture)
        val issueWirePayload = wirePayloadBytes(OfflineNoteV2.issueInstruction(issue))
        val redeemWirePayload = wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem))
        val issuePair = rawInstructionPair(OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA, issueWirePayload)

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssueInstruction(
                rawInstructionPair(OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA, issueWirePayload),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeRedeemInstruction(issuePair)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssueInstruction(issue.noritoEncoded())
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssueInstruction(issuePair.copyOf(issuePair.size - 1))
        }
        val corruptedWirePayload = issueWirePayload.copyOf()
        corruptedWirePayload[corruptedWirePayload.lastIndex] =
            (corruptedWirePayload.last().toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeIssueInstruction(corruptedWirePayload)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.decodeAuditInstruction(
                rawInstructionPair(OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA, redeemWirePayload),
            )
        }
    }

    @Test
    fun offlineNoteV2DomainsRejectSubstitutionAndPadding() {
        val fixture = loadFixture()
        val certificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val claim = audit.inputClaims.first()
        val auditPublic = audit.publicInputs()
        val redeemPublic = redeem.publicInputs()

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificatePayloadV2(
                domain = "${OfflineNoteV2.KEY_CERTIFICATE_PAYLOAD_DOMAIN} ",
                version = certificate.version,
                platform = certificate.platform,
                keyId = certificate.keyId,
                deviceId = certificate.deviceId,
                accountId = certificate.accountId,
                publicKey = certificate.publicKey(),
                assertionScheme = certificate.assertionScheme,
                assertionKeyAlgorithm = certificate.assertionKeyAlgorithm,
                assertionPublicKey = certificate.assertionPublicKey(),
                assertionUsageCountLimit = certificate.assertionUsageCountLimit,
                oneUse = certificate.oneUse,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssuedClaimV2(
                domain = "${OfflineNoteV2.ISSUED_CLAIM_DOMAIN}\n",
                noteCommitment = claim.noteCommitment(),
                keyCertificatePayloadHash = claim.keyCertificatePayloadHash(),
                assetId = claim.assetId,
                amount = claim.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                domain = "forged:${OfflineNoteV2.REDEEM_PUBLIC_INPUTS_DOMAIN}",
                sourceNoteCommitment = redeemPublic.sourceNoteCommitment(),
                inputNullifiers = redeemPublic.inputNullifiers(),
                keyCertificatePayloadHash = redeemPublic.keyCertificatePayloadHash(),
                recipient = redeemPublic.recipient,
                assetId = redeemPublic.assetId,
                amount = redeemPublic.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditPublicInputsV2(
                domain = " ${OfflineNoteV2.AUDIT_PUBLIC_INPUTS_DOMAIN}",
                tokenId = auditPublic.tokenId(),
                keyCertificatePayloadHash = auditPublic.keyCertificatePayloadHash(),
                inputNullifiers = auditPublic.inputNullifiers(),
                inputClaims = auditPublic.inputClaims,
                outputCommitments = auditPublic.outputCommitments(),
                outputClaims = auditPublic.outputClaims,
            )
        }
    }

    @Test
    fun publicInputHashesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertEquals(string(obj(chain, "audit"), "public_inputs_hash"), hex(audit.publicInputsHash()))
        assertEquals(string(obj(chain, "redeem"), "public_inputs_hash"), hex(redeem.publicInputsHash()))
        audit.validateProofBinding()
        redeem.validateProofBinding()
        audit.replacingRecursiveProof(audit.recursiveProof).validateProofBinding()
        redeem.replacingRecursiveProof(redeem.recursiveProof).validateProofBinding()
    }

    @Test
    fun proofBindingRejectsMismatch() {
        val fixture = loadFixture()
        val redeem = redeem(fixture)
        val badProof = OfflineNoteV2.RecursiveProofV2(
            publicInputsHash = OfflineNoteV2.hash("wrong-public-inputs".toByteArray()),
            proof = OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".toByteArray()
            )
        )
        val forged = OfflineNoteV2.RedeemV2(
            sourceNoteCommitment = redeem.sourceNoteCommitment(),
            inputNullifiers = redeem.inputNullifiers(),
            senderKeyCertificate = redeem.senderKeyCertificate,
            recipient = redeem.recipient,
            assetId = redeem.assetId,
            amount = redeem.amount,
            recursiveProof = badProof,
        )

        assertFailsWith<IllegalArgumentException> {
            forged.validateProofBinding()
        }
    }

    @Test
    fun proofVerifierAndHashValidationRejectsMalformedValues() {
        val publicInputsHash = audit(loadFixture()).publicInputsHash()
        val proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1))
        assertEquals(OfflineNoteV2.RECURSIVE_BACKEND, proof.backend)

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.ProofBox("  ${OfflineNoteV2.RECURSIVE_BACKEND}  ", byteArrayOf(1))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.ProofBox(" \n ", byteArrayOf(1))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf())
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = ByteArray(31) { 1 },
                proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1)),
            )
        }
        val nonCanonicalHash = publicInputsHash.copyOf()
        nonCanonicalHash[31] = (nonCanonicalHash[31].toInt() and 0xfe).toByte()
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = nonCanonicalHash,
                proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "", name = "vk")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = " halo2/ipa ", name = "vk")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2/ipa", name = " vk ")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2:ipa", name = "vk")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2/ipa", name = "bad:vk")
        }
    }

    @Test
    fun openVerifyEnvelopeDecoderRejectsMalformedV2EnvelopeFields() {
        val values = OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit(loadFixture())).publicValues()
        val payload = fakeZk1ProofPayload(byteArrayOf(1, 2, 3), values)
        val envelope = OfflineNoteV2Halo2Prover.openVerifyEnvelope(payload)

        assertFalse(OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(envelope, "00".repeat(32)))

        val emptyProofError = assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(
                OfflineNoteV2Halo2Prover.openVerifyEnvelope(ByteArray(0)),
                "00".repeat(32),
            )
        }
        assertTrue(emptyProofError.message.orEmpty().contains("OpenVerifyEnvelope proof payload is empty"))

        val trailingCircuitError = assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2Halo2Prover.verifyOpenVerifyEnvelope(
                rawOpenVerifyEnvelopeWithCircuitPayload(
                    openEnvelopeStringPayload(OfflineNoteV2Halo2Prover.CIRCUIT_ID) + byteArrayOf(0),
                ),
                values,
            )
        }
        assertTrue(
            trailingCircuitError.message.orEmpty()
                .contains("Trailing bytes after OpenVerifyEnvelope field decode"),
        )
    }

    @Test
    fun certificateValidationRejectsMalformedValues() {
        val certJson = obj(obj(loadFixture(), "payment_token"), "sender_key_certificate")
        val publicKey = base64Bytes(string(certJson, "public_key"))
        val assertionPublicKey = base64Bytes(string(certJson, "assertion_public_key"))
        val issuerSignature = base64Bytes(string(certJson, "issuer_signature_base64"))

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                version = OfflineNoteV2.KEY_CERTIFICATE_VERSION + 1,
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = false,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = "\u00A0\u2003",
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificatePayloadV2(
                version = OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = "\u00A0\u2003",
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey.copyOfRange(0, 31),
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = -1,
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature.copyOfRange(0, 63),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = offCurveP256AssertionPublicKey(),
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = "apple-app-attest-v1",
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificatePayloadV2(
                version = OfflineNoteV2.KEY_CERTIFICATE_VERSION,
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = offCurveP256AssertionPublicKey(),
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
            )
        }
    }

    @Test
    fun offlineDeviceAttestationRegistrationValidationRejectsMalformedValues() {
        val fixture = loadFixture()
        val vector = obj(obj(fixture, "chain_vectors"), "attestation_registration")

        val badChallenge = hexBytes(string(vector, "challenge_hash"))
        badChallenge[0] = (badChallenge[0].toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, challengeHash = badChallenge)
        }

        val badReportHash = hexBytes(string(vector, "attestation_report_hash"))
        badReportHash[0] = (badReportHash[0].toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, attestationReportHash = badReportHash)
        }

        val badEvidenceHash = hexBytes(string(vector, "evidence_hash"))
        badEvidenceHash[0] = (badEvidenceHash[0].toInt() xor 0x01).toByte()
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, evidenceHash = badEvidenceHash)
        }

        val forgedEvidence = attestationEvidence(ByteArray(32) { 0xA5.toByte() })
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(
                fixture,
                evidenceHash = OfflineNoteV2.hash(forgedEvidence),
                evidence = forgedEvidence,
            )
        }

        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, androidSigningCertificateSha256 = ByteArray(31) { 1 })
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, publicKey = ByteArray(31) { 1 })
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, keyId = "not standard base64!")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, keyId = "AB==")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, keyId = " ${string(vector, "key_id")} ")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, deviceId = " ${string(vector, "device_id")} ")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, deviceId = "\u00A0\u2003")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, iosTeamId = " ${nullableString(vector, "ios_team_id")} ")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, iosBundleId = "${nullableString(vector, "ios_bundle_id")}\n")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, iosEnvironment = "\t${nullableString(vector, "ios_environment")}")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, androidPackageName = " jp.co.soramitsu.iroha.offline ")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, assertionPublicKey = offCurveP256AssertionPublicKey())
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, recentBlockHash = ByteArray(31) { 1 })
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, oneUse = false)
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, assetDefinitionId = "cash#bad")
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, assertionUsageCountLimit = 1)
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(
                fixture,
                platform = OfflineNoteV2.ANDROID_KEYMINT_PLATFORM,
                assertionScheme = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME,
                assertionKeyAlgorithm = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(
                fixture,
                platform = OfflineNoteV2.ANDROID_KEYMINT_PLATFORM,
                assertionScheme = "android-keymint-ecdsa-p256-usage-limit",
                assertionKeyAlgorithm = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(
                fixture,
                keyId = "00".repeat(32),
                platform = OfflineNoteV2.ANDROID_KEYMINT_PLATFORM,
                assertionScheme = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME,
                assertionKeyAlgorithm = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = 1,
            )
        }
        val androidUppercaseKeyId = hex(
            sha256(base64Bytes(string(vector, "assertion_public_key")))
        ).uppercase(Locale.ROOT)
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(
                fixture,
                keyId = androidUppercaseKeyId,
                platform = OfflineNoteV2.ANDROID_KEYMINT_PLATFORM,
                assertionScheme = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME,
                assertionKeyAlgorithm = OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = 1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            attestationRegistration(fixture, platform = "ios-app-attest")
        }
    }

    @Test
    fun offlineDeviceAttestationRegistrationDefensivelyCopiesMutableByteArrays() {
        val fixture = loadFixture()
        val vector = obj(obj(fixture, "chain_vectors"), "attestation_registration")
        val publicKey = base64Bytes(string(vector, "public_key"))
        val assertionPublicKey = base64Bytes(string(vector, "assertion_public_key"))
        val attestationReport = base64Bytes(string(vector, "attestation_report_base64"))
        val evidence = base64Bytes(string(vector, "evidence_base64"))
        val recentBlockHash = hexBytes(string(vector, "recent_block_hash"))

        val registration = attestationRegistration(
            fixture,
            publicKey = publicKey,
            assertionPublicKey = assertionPublicKey,
            attestationReport = attestationReport,
            evidence = evidence,
            recentBlockHash = recentBlockHash,
        )
        val encoded = registration.noritoEncoded()
        publicKey[0] = (publicKey[0].toInt() xor 0x01).toByte()
        assertionPublicKey[0] = (assertionPublicKey[0].toInt() xor 0x01).toByte()
        attestationReport[0] = (attestationReport[0].toInt() xor 0x01).toByte()
        evidence[0] = (evidence[0].toInt() xor 0x01).toByte()
        recentBlockHash[0] = (recentBlockHash[0].toInt() xor 0x01).toByte()

        assertEquals(string(vector, "norito_base64"), base64(encoded))
        assertEquals(base64(encoded), base64(registration.noritoEncoded()))

        val returnedPublicKey = registration.publicKey()
        returnedPublicKey[0] = (returnedPublicKey[0].toInt() xor 0x01).toByte()
        val returnedReport = registration.attestationReport()
        returnedReport[0] = (returnedReport[0].toInt() xor 0x01).toByte()
        val returnedEvidence = registration.evidence()
        returnedEvidence[0] = (returnedEvidence[0].toInt() xor 0x01).toByte()
        assertEquals(base64(encoded), base64(registration.noritoEncoded()))
    }

    @Test
    fun auditBundleRejectsInvalidShapesAndUncommittedOutputs() {
        val audit = audit(loadFixture())
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = emptyList(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = emptyList(),
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers() + audit.inputNullifiers()[0],
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = emptyList(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = emptyList(),
                recursiveProof = audit.recursiveProof,
            )
        }
        val uncommittedOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = OfflineNoteV2.hash("uncommitted-output".toByteArray()),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = audit.outputClaims[0].amount,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = listOf(uncommittedOutput),
                recursiveProof = audit.recursiveProof,
            )
        }
    }

    @Test
    fun issueRedeemPublicInputsAndInstancesRejectMalformedValues() {
        val fixture = loadFixture()
        val cert = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = ByteArray(31) { 1 },
                keyCertificate = cert,
                assetId = string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"),
                amount = "5",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = redeem.sourceNoteCommitment(),
                keyCertificate = cert,
                assetId = "cash#branch.sbp",
                amount = "5",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = redeem.sourceNoteCommitment(),
                keyCertificate = cert,
                assetId = redeem.assetId,
                amount = "not-a-number",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = emptyList(),
                senderKeyCertificate = redeem.senderKeyCertificate,
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
                recursiveProof = redeem.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = ByteArray(31) { 1 },
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = redeem.senderKeyCertificate.payloadHash(),
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = ByteArray(31) { 1 },
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = redeem.senderKeyCertificate.payloadHash(),
                recipient = "${redeem.recipient}@bad",
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }

        val overLimitOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = OfflineNoteV2.hash("third-output".toByteArray()),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = "0",
        )
        val tooManyOutputs = OfflineNoteV2.AuditBundleV2(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments() + overLimitOutput.noteCommitment(),
            outputClaims = audit.outputClaims + overLimitOutput,
            recursiveProof = audit.recursiveProof,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.InstanceBuilder.auditInstanceValues(tooManyOutputs)
        }

        val unconservedOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = audit.outputClaims[0].noteCommitment(),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = "6",
        )
        val unconservedAudit = OfflineNoteV2.AuditBundleV2(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments(),
            outputClaims = listOf(unconservedOutput, audit.outputClaims[1]),
            recursiveProof = audit.recursiveProof,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.InstanceBuilder.auditInstanceValues(unconservedAudit)
        }
    }

    @Test
    fun instanceValuesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val auditValues = OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit(fixture))
        val redeemValues = OfflineNoteV2.InstanceBuilder.redeemInstanceValues(redeem(fixture))
        val auditPublic = auditValues.publicValues()
        val redeemPublic = redeemValues.publicValues()

        assertEquals(
            string(obj(chain, "audit"), "public_inputs_hash"),
            hex(hashFromPublicValues(auditPublic)),
        )
        assertEquals(
            string(obj(chain, "redeem"), "public_inputs_hash"),
            hex(hashFromPublicValues(redeemPublic)),
        )
        assertEquals(2L, auditPublic[4])
        assertEquals(1L, auditPublic[5])
        assertEquals(2L, auditPublic[6])
        assertEquals(52L, auditPublic[7])
        assertEquals(52L, auditPublic[8])
        assertEquals(1L, redeemPublic[4])
        assertEquals(1L, redeemPublic[5])
        assertEquals(1L, redeemPublic[6])
        assertEquals(5L, redeemPublic[7])
        assertEquals(5L, redeemPublic[8])
        assertEquals(52L, auditValues.inputAmounts()[0])
        assertEquals(5L, auditValues.outputAmounts()[0])
        assertEquals(47L, auditValues.outputAmounts()[1])
        assertEquals(5L, redeemValues.inputAmounts()[0])
        assertEquals(5L, redeemValues.outputAmounts()[0])
        assertEquals(
            OfflineNoteV2.instanceScalarBytes(auditPublic[0]).toList(),
            auditValues.publicInstanceColumns()[0].toList(),
        )
    }

    @Test
    fun nativeHalo2ProverProducesVerifyingPayloadWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_V2_PROVER_TEST") != "1") {
            return
        }
        val fixture = loadFixture()
        val audit = audit(fixture)
        val values = OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit)
        OfflineNoteV2Halo2Prover.prewarm()
        val payload = OfflineNoteV2Halo2Prover.proveZk1Payload(values)
        System.getenv("IROHA_JVM_OFFLINE_V2_PAYLOAD_OUT")?.let {
            Files.write(Paths.get(it), payload)
        }

        assertTrue(OfflineNoteV2Halo2Prover.verifyZk1Payload(payload, values.publicValues()))
        val proof = OfflineNoteV2Halo2Prover.proveAudit(audit)
        audit.replacingRecursiveProof(proof).validateProofBinding()
        assertTrue(proof.proof.bytes().size <= OfflineNoteV2Halo2Prover.MAX_ENVELOPE_BYTES)
    }

    @Test
    fun nativeHalo2ProverPerformanceWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_V2_BENCH") != "1") {
            return
        }
        val iterations = System.getenv("IROHA_JVM_OFFLINE_V2_BENCH_ITERATIONS")?.toInt() ?: 20
        assertTrue(iterations > 0)
        val fixture = loadFixture()
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        OfflineNoteV2Halo2Prover.prewarm()
        OfflineNoteV2Halo2Prover.proveAudit(audit)
        OfflineNoteV2Halo2Prover.proveRedeem(redeem)

        val auditSeconds = benchmarkSeconds(iterations) {
            OfflineNoteV2Halo2Prover.proveAudit(audit)
        }
        val redeemSeconds = benchmarkSeconds(iterations) {
            OfflineNoteV2Halo2Prover.proveRedeem(redeem)
        }
        println("offline_note_v2_jvm_bench audit=${summary(auditSeconds)} redeem=${summary(redeemSeconds)}")
    }

    @Test
    fun qrFixtureUsesSdkTextPrefix() {
        val fountain = obj(loadFixture(), "fountain_qr_v1")
        assertEquals("iroha:qr1:", string(fountain, "frame_prefix"))
    }

    private fun issue(fixture: Map<String, Any?>): OfflineNoteV2.IssueV2 {
        val chainIssue = obj(obj(fixture, "chain_vectors"), "issue")
        return OfflineNoteV2.IssueV2(
            noteCommitment = hexBytes(string(chainIssue, "note_commitment")),
            keyCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
            assetId = string(chainIssue, "asset_id"),
            amount = string(chainIssue, "amount"),
        )
    }

    private fun redeem(fixture: Map<String, Any?>): OfflineNoteV2.RedeemV2 {
        val vector = obj(obj(fixture, "chain_vectors"), "redeem")
        val payment = obj(fixture, "payment_token")
        return OfflineNoteV2.RedeemV2(
            sourceNoteCommitment = hexBytes(string(vector, "source_note_commitment")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            senderKeyCertificate = certificate(obj(payment, "recipient_key_certificate")),
            recipient = string(payment, "recipient_account_id"),
            assetId = string(vector, "asset_id"),
            amount = string(vector, "amount"),
            recursiveProof = OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "offline-v2-vector-redeem-proof".toByteArray()
                )
            )
        )
    }

    private fun audit(fixture: Map<String, Any?>): OfflineNoteV2.AuditBundleV2 {
        val vector = obj(obj(fixture, "chain_vectors"), "audit")
        val payment = obj(fixture, "payment_token")
        return OfflineNoteV2.AuditBundleV2(
            tokenId = hexBytes(string(vector, "token_id")),
            senderKeyCertificate = certificate(obj(payment, "sender_key_certificate")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            inputClaims = list(payment, "input_claims").map { issuedClaim(objValue(it, "input claim")) },
            outputCommitments = list(vector, "output_commitments").map { hexBytes(it as String) },
            outputClaims = list(payment, "output_claims").map { auditOutputClaim(objValue(it, "output claim")) },
            recursiveProof = OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "offline-v2-vector-audit-proof".toByteArray()
                )
            )
        )
    }

    private fun attestationRegistration(
        fixture: Map<String, Any?>,
        challengeHash: ByteArray? = null,
        attestationReportHash: ByteArray? = null,
        evidenceHash: ByteArray? = null,
        androidSigningCertificateSha256: ByteArray? = null,
        publicKey: ByteArray? = null,
        assertionPublicKey: ByteArray? = null,
        keyId: String? = null,
        deviceId: String? = null,
        platform: String? = null,
        assertionScheme: String? = null,
        assertionKeyAlgorithm: String? = null,
        assertionUsageCountLimit: Int? = null,
        iosTeamId: String? = null,
        iosBundleId: String? = null,
        iosEnvironment: String? = null,
        androidPackageName: String? = null,
        attestationReport: ByteArray? = null,
        evidence: ByteArray? = null,
        recentBlockHash: ByteArray? = null,
        oneUse: Boolean? = null,
        assetDefinitionId: String? = null,
    ): OfflineNoteV2.DeviceAttestationRegistrationV2 {
        val vector = obj(obj(fixture, "chain_vectors"), "attestation_registration")
        return OfflineNoteV2.DeviceAttestationRegistrationV2(
            version = int(vector, "version"),
            platform = platform ?: string(vector, "platform"),
            keyId = keyId ?: string(vector, "key_id"),
            deviceId = deviceId ?: string(vector, "device_id"),
            accountId = string(vector, "account_id"),
            assetDefinitionId = assetDefinitionId ?: nullableString(vector, "asset_definition_id"),
            iosTeamId = iosTeamId ?: nullableString(vector, "ios_team_id"),
            iosBundleId = iosBundleId ?: nullableString(vector, "ios_bundle_id"),
            iosEnvironment = iosEnvironment ?: nullableString(vector, "ios_environment"),
            androidPackageName = androidPackageName ?: nullableString(vector, "android_package_name"),
            androidSigningCertificateSha256 = androidSigningCertificateSha256
                ?: nullableString(vector, "android_signing_certificate_sha256")?.let(::hexBytes),
            publicKey = publicKey ?: base64Bytes(string(vector, "public_key")),
            assertionScheme = assertionScheme ?: string(vector, "assertion_scheme"),
            assertionKeyAlgorithm = assertionKeyAlgorithm ?: string(vector, "assertion_key_algorithm"),
            assertionPublicKey = assertionPublicKey ?: base64Bytes(string(vector, "assertion_public_key")),
            assertionUsageCountLimit = assertionUsageCountLimit ?: nullableInt(vector, "assertion_usage_count_limit"),
            oneUse = oneUse ?: bool(vector, "one_use"),
            challengeHash = challengeHash ?: hexBytes(string(vector, "challenge_hash")),
            attestationReportHash = attestationReportHash ?: hexBytes(string(vector, "attestation_report_hash")),
            attestationReport = attestationReport ?: base64Bytes(string(vector, "attestation_report_base64")),
            evidenceHash = evidenceHash ?: hexBytes(string(vector, "evidence_hash")),
            evidence = evidence ?: base64Bytes(string(vector, "evidence_base64")),
            recentBlockHeight = long(vector, "recent_block_height"),
            recentBlockHash = recentBlockHash ?: hexBytes(string(vector, "recent_block_hash")),
            expiresAtMs = long(vector, "expires_at_ms"),
        )
    }

    private fun certificate(json: Map<String, Any?>): OfflineNoteV2.KeyCertificateV2 =
        OfflineNoteV2.KeyCertificateV2(
            version = int(json, "version"),
            platform = string(json, "platform"),
            keyId = string(json, "key_id"),
            deviceId = string(json, "device_id"),
            accountId = string(json, "account_id"),
            publicKey = base64Bytes(string(json, "public_key")),
            assertionScheme = string(json, "assertion_scheme"),
            assertionKeyAlgorithm = string(json, "assertion_key_algorithm"),
            assertionPublicKey = base64Bytes(string(json, "assertion_public_key")),
            assertionUsageCountLimit = nullableInt(json, "assertion_usage_count_limit"),
            oneUse = bool(json, "one_use"),
            issuerSignature = base64Bytes(string(json, "issuer_signature_base64")),
        )

    private fun issuedClaim(json: Map<String, Any?>): OfflineNoteV2.IssuedClaimV2 =
        OfflineNoteV2.IssuedClaimV2(
            domain = string(json, "domain"),
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificatePayloadHash = hexBytes(string(json, "key_certificate_payload_hash")),
            assetId = string(json, "asset_id"),
            amount = string(json, "amount"),
        )

    private fun auditOutputClaim(json: Map<String, Any?>): OfflineNoteV2.AuditOutputClaimV2 =
        OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificate = certificate(obj(json, "key_certificate")),
            assetId = "${string(json, "asset_definition_id")}#${string(json, "account_id")}",
            amount = string(json, "amount"),
        )

    private fun assertInstructionWrapper(
        schema: String,
        modelPayload: ByteArray,
        instruction: InstructionBox,
    ) {
        assertEquals(schema, instruction.name)
        val payload = instruction.payload as? WirePayload
            ?: error("Offline Note V2 instruction must use a wire payload")
        assertEquals(schema, payload.wireName)
        val outerFrame = NoritoHeader.decode(payload.payloadBytes, null)
        assertEquals(
            0,
            outerFrame.header.flags,
            "instruction wrapper frame flags",
        )
        assertTrue(isNoritoFrame(modelPayload), "public model encoder still returns a framed archive")
        val wrapperPayload = decodeInstructionWrapper(schema, payload.payloadBytes)
        assertFalse(isNoritoFrame(wrapperPayload), "instruction wrapper must contain a bare model payload")
    }

    private fun wirePayloadBytes(instruction: InstructionBox): ByteArray =
        (instruction.payload as WirePayload).payloadBytes

    private fun fakeZk1ProofPayload(proofTranscript: ByteArray, publicValues: LongArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf(0x5A, 0x4B, 0x31, 0x00))
        appendTlv(out, "PROF", proofTranscript)
        val instances = ByteArrayOutputStream()
        writeUInt32Le(instances, 16)
        writeUInt32Le(instances, 1)
        for (value in publicValues) {
            instances.write(OfflineNoteV2.instanceScalarBytes(value))
        }
        appendTlv(out, "I10P", instances.toByteArray())
        return out.toByteArray()
    }

    private fun appendTlv(out: ByteArrayOutputStream, tag: String, value: ByteArray) {
        out.write(tag.toByteArray(Charsets.UTF_8))
        writeUInt32Le(out, value.size)
        out.write(value)
    }

    private fun writeUInt32Le(out: ByteArrayOutputStream, value: Int) {
        var remaining = value
        repeat(4) {
            out.write(remaining and 0xff)
            remaining = remaining ushr 8
        }
    }

    private fun rawOpenVerifyEnvelopeWithCircuitPayload(circuitFieldPayload: ByteArray): ByteArray {
        val adapter = object : TypeAdapter<Unit> {
            override fun encode(encoder: NoritoEncoder, value: Unit) {
                writeOpenEnvelopeField(encoder) {
                    it.writeUInt(OfflineNoteV2Halo2Prover.BACKEND_TAG.toLong(), 32)
                }
                writeOpenEnvelopeRawField(encoder, circuitFieldPayload)
            }

            override fun decode(decoder: NoritoDecoder): Unit =
                throw AssertionError("raw OpenVerifyEnvelope test adapter is encode-only")
        }
        return NoritoCodec.encode(
            Unit,
            "iroha_data_model::zk::OpenVerifyEnvelope",
            adapter,
            NoritoHeader.COMPACT_LEN,
        )
    }

    private fun openEnvelopeStringPayload(value: String): ByteArray {
        val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        writeInstructionString(encoder, value)
        return encoder.toByteArray()
    }

    private fun writeOpenEnvelopeField(encoder: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        writePayload(child)
        writeOpenEnvelopeRawField(encoder, child.toByteArray())
    }

    private fun writeOpenEnvelopeRawField(encoder: NoritoEncoder, payload: ByteArray) {
        encoder.writeLength(payload.size.toLong(), compact(encoder))
        encoder.writeBytes(payload)
    }

    private fun rawInstructionPair(wireName: String, wirePayload: ByteArray, compact: Boolean = true): ByteArray {
        val flags = if (compact) NoritoHeader.COMPACT_LEN else 0
        val encoder = NoritoEncoder(flags)
        writeInstructionField(encoder) { writeInstructionString(it, wireName) }
        writeInstructionField(encoder) { writeInstructionBytesVec(it, wirePayload) }
        return encoder.toByteArray()
    }

    private fun writeInstructionField(encoder: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        writePayload(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), compact(encoder))
        encoder.writeBytes(payload)
    }

    private fun writeInstructionString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), compact(encoder))
        encoder.writeBytes(bytes)
    }

    private fun writeInstructionBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun encodeInstructionWrapper(schema: String, modelPayload: ByteArray): ByteArray =
        NoritoCodec.encode(modelPayload, schema, InstructionWrapperPayloadAdapter, 0)

    private fun decodeInstructionWrapper(schema: String, wirePayload: ByteArray): ByteArray =
        NoritoCodec.decode(wirePayload, InstructionWrapperPayloadAdapter, schema)

    private object InstructionWrapperPayloadAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            val child = encoder.childEncoder()
            child.writeBytes(value)
            val payload = child.toByteArray()
            encoder.writeLength(payload.size.toLong(), compact(encoder))
            encoder.writeBytes(payload)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val length = decoder.readLength(compact(decoder)).toInt()
            val child = NoritoDecoder(decoder.readBytes(length), decoder.flags, decoder.flagsHint)
            val payload = child.readBytes(child.remaining())
            require(child.remaining() == 0) { "trailing bytes in instruction wrapper payload" }
            return payload
        }
    }

    private companion object {
        fun compact(encoder: NoritoEncoder): Boolean =
            (encoder.flags and NoritoHeader.COMPACT_LEN) != 0

        fun compact(decoder: NoritoDecoder): Boolean =
            (decoder.flags and NoritoHeader.COMPACT_LEN) != 0

        fun isNoritoFrame(bytes: ByteArray): Boolean =
            bytes.size >= NoritoHeader.HEADER_LENGTH &&
                bytes[0] == 'N'.code.toByte() &&
                bytes[1] == 'R'.code.toByte() &&
                bytes[2] == 'T'.code.toByte() &&
                bytes[3] == '0'.code.toByte()
    }

    private fun loadFixture(): Map<String, Any?> {
        val path = Paths.get("..", "..", "fixtures", "offline", "interop_contract_v2.json")
        val parsed = JsonParser.parse(String(Files.readAllBytes(path), Charsets.UTF_8))
        @Suppress("UNCHECKED_CAST")
        return parsed as Map<String, Any?>
    }

    private fun obj(map: Map<String, Any?>, key: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as Map<String, Any?>
    }

    private fun list(map: Map<String, Any?>, key: String): List<Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as List<Any?>
    }

    private fun objValue(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?> ?: error("$label must be an object")
    }

    private fun string(map: Map<String, Any?>, key: String): String = map[key] as String
    private fun nullableString(map: Map<String, Any?>, key: String): String? = map[key] as String?
    private fun bool(map: Map<String, Any?>, key: String): Boolean = map[key] as Boolean
    private fun int(map: Map<String, Any?>, key: String): Int = (map[key] as Number).toInt()
    private fun long(map: Map<String, Any?>, key: String): Long = (map[key] as Number).toLong()
    private fun nullableInt(map: Map<String, Any?>, key: String): Int? = (map[key] as Number?)?.toInt()

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
    private fun base64Bytes(value: String): ByteArray = Base64.getDecoder().decode(value)

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xFF) }

    private fun sha256(bytes: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun attestationEvidence(attestationReportHash: ByteArray): ByteArray =
        OfflineNoteV2.DEVICE_ATTESTATION_EVIDENCE_PREFIX.toByteArray(Charsets.UTF_8) + attestationReportHash

    private fun offCurveP256AssertionPublicKey(): ByteArray =
        ByteArray(65).also { it[0] = 0x04 }

    private fun hashFromPublicValues(values: LongArray): ByteArray {
        val out = ByteArray(32)
        for (idx in 0 until 4) {
            var word = values[idx]
            for (offset in 0 until 8) {
                out[idx * 8 + offset] = (word and 0xffL).toByte()
                word = word ushr 8
            }
        }
        return out
    }

    private fun benchmarkSeconds(iterations: Int, body: () -> Unit): DoubleArray {
        val durations = DoubleArray(iterations)
        for (idx in 0 until iterations) {
            val start = System.nanoTime()
            body()
            durations[idx] = (System.nanoTime() - start).toDouble() / 1_000_000_000.0
        }
        return durations
    }

    private fun summary(values: DoubleArray): String {
        val sorted = values.sorted()
        if (sorted.isEmpty()) {
            return "empty"
        }
        val median = if (sorted.size % 2 == 0) {
            (sorted[sorted.size / 2 - 1] + sorted[sorted.size / 2]) / 2.0
        } else {
            sorted[sorted.size / 2]
        }
        val p95Index = minOf(sorted.size - 1, maxOf(0, kotlin.math.ceil(sorted.size * 0.95).toInt() - 1))
        return "median=%.3fs p95=%.3fs max=%.3fs n=%d".format(
            Locale.ROOT,
            median,
            sorted[p95Index],
            sorted.last(),
            sorted.size,
        )
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex length must be even" }
        val out = ByteArray(value.length / 2)
        var offset = 0
        while (offset < value.length) {
            out[offset / 2] = value.substring(offset, offset + 2).toInt(16).toByte()
            offset += 2
        }
        return out
    }
}
