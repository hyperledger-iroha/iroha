// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.io.File
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class OfflineCashWireV1Test {
    @Test
    fun `one exact positive request amount round trips canonically`() {
        val request = OfflineCashV1TestSupport.request(BigInteger.TEN)
        val raw = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        assertEquals(BigInteger.TEN, OfflineCashNoritoV1.decodePaymentRequestShapeExact(raw).amount)
    }

    @Test
    fun `proof authorization precedes ticket and complete current exchange round trips`() {
        val request = OfflineCashV1TestSupport.request()
        val authorization = OfflineCashV1TestSupport.authorization(request)
        val ticket = OfflineCashV1TestSupport.ticket(request, authorization)
        val payment = OfflineCashV1TestSupport.payment(request, authorization, ticket)
        val acknowledgement = OfflineCashV1TestSupport.acknowledgement(request, payment)

        val requestRaw = OfflineCashNoritoV1.encodePaymentRequestShape(request)
        val authorizationRaw = OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(authorization, request)
        val ticketRaw = OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket, request, authorization)
        val paymentRaw = OfflineCashNoritoV1.encodePaymentShape(payment, request)
        val acknowledgementRaw = OfflineCashNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment)

        assertContentEquals(requestRaw, OfflineCashNoritoV1.encodePaymentRequestShape(OfflineCashNoritoV1.decodePaymentRequestShapeExact(requestRaw)))
        assertContentEquals(
            authorizationRaw,
            OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(
                OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact(authorizationRaw, request),
                request,
            ),
        )
        assertContentEquals(
            ticketRaw,
            OfflineCashNoritoV1.encodeAcceptanceTicketShape(
                OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact(ticketRaw, request, authorization),
                request,
                authorization,
            ),
        )
        assertContentEquals(paymentRaw, OfflineCashNoritoV1.encodePaymentShape(OfflineCashNoritoV1.decodePaymentShapeExact(paymentRaw, request), request))
        assertContentEquals(
            acknowledgementRaw,
            OfflineCashNoritoV1.encodeAcknowledgementShape(
                OfflineCashNoritoV1.decodeAcknowledgementShapeExact(acknowledgementRaw, request, payment),
                request,
                payment,
            ),
        )
        assertEquals(
            requestRaw.size + authorizationRaw.size + ticketRaw.size,
            OfflineCashNoritoV1.validatePreTicketExchangeShape(request, authorization, ticket),
        )
        assertEquals(
            requestRaw.size + authorizationRaw.size + ticketRaw.size + paymentRaw.size + acknowledgementRaw.size,
            OfflineCashNoritoV1.validateCompleteExchangeShape(
                request,
                authorization,
                ticket,
                payment,
                acknowledgement,
            ),
        )
    }

    @Test
    fun `no commit closure round trips with exact authorization and ticket bindings`() {
        val closure = OfflineCashV1TestSupport.noCommitClosure()
        val raw = OfflineCashNoritoV1.encodeNoCommitClosureShape(closure)
        val decoded = OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(raw)

        assertTrue(raw.size <= OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES)
        assertContentEquals(raw, OfflineCashNoritoV1.encodeNoCommitClosureShape(decoded))
        assertEquals(32, OfflineCashNoritoV1.noCommitClosureDigestShape(decoded).size)
        val publicNames = OfflineCashNoCommitClosureStatementV1::class.java.declaredFields
            .map { it.name.lowercase() }
        listOf("predecessor", "successor", "statecommitment", "beforesequence", "aftersequence")
            .forEach { forbidden -> assertFalse(publicNames.any { it.contains(forbidden) }) }

        val statement = closure.statement
        val substitutedStatement = OfflineCashNoCommitClosureStatementV1(
            statement.version,
            statement.releaseId(),
            statement.suiteId(),
            statement.vkDigest(),
            statement.artifactManifestDigest(),
            statement.senderHardwareBindingCommitment(),
            OfflineCashV1TestSupport.bytes(0xee),
            statement.requestDigest(),
            statement.acceptanceTicketId(),
            statement.ticketDigest(),
            statement.intentAuthorizationDigest(),
            statement.intentDigest(),
            statement.exactAmount,
            statement.senderOneTimeCommitment(),
            statement.recoveryId(),
            statement.cancellationNullifier(),
            statement.equivalentDeliverySlotCommitment(),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineCashNoritoV1.encodeNoCommitClosureShape(
                OfflineCashNoCommitClosureV1(
                    1,
                    substitutedStatement,
                    closure.request,
                    closure.intentAuthorization,
                    closure.acceptanceTicket,
                    closure.proof,
                ),
            )
        }
    }

    @Test
    fun `generated no commit closure fixture is canonical when present`() {
        val fixture = sharedFixtureText()
        if (!fixture.contains("\"no_commit_closure\"")) return
        val raw = fixtureValue(fixture, "no_commit_closure", "norito_hex").hexBytes()
        assertContentEquals(
            raw,
            OfflineCashNoritoV1.encodeNoCommitClosureShape(
                OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(raw),
            ),
        )
    }

    @Test
    fun `native generated V1 fixture round trips every transported value`() {
        val fixture = sharedFixtureText()
        assertTrue(Regex("\\\"fixture_version\\\"\\s*:\\s*1").containsMatchIn(fixture))
        val raw = REQUIRED_CANONICAL_FIXTURE_VALUES.associateWith { name ->
            fixtureValue(fixture, name, "norito_hex").hexBytes()
        }

        val request = OfflineCashNoritoV1.decodePaymentRequestShapeExact(raw.getValue("payment_request"))
        val authorization = OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact(
            raw.getValue("acceptance_intent_authorization"),
            request,
        )
        val ticket = OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact(
            raw.getValue("acceptance_ticket"),
            request,
            authorization,
        )
        val closure = OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(raw.getValue("no_commit_closure"))
        val payment = OfflineCashNoritoV1.decodePaymentShapeExact(raw.getValue("payment"), request)
        val acknowledgement = OfflineCashNoritoV1.decodeAcknowledgementShapeExact(
            raw.getValue("acknowledgement"),
            request,
            payment,
        )
        val mintAuthorization = OfflineCashNoritoV1.decodeMintAuthorizationShapeExact(
            raw.getValue("mint_authorization"),
        )
        val mintCredit = OfflineCashNoritoV1.decodeMintCreditShapeExact(
            raw.getValue("mint_credit"),
            mintAuthorization,
        )
        val redemption = OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(
            raw.getValue("redemption_voucher"),
        )
        val envelope = OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(
            raw.getValue("encrypted_credit_envelope"),
        )
        val aad = OfflineCashNoritoV1.decodeEncryptedCreditAadShapeExact(
            raw.getValue("encrypted_credit_aad"),
        )
        val opening = OfflineCashNoritoV1.decodeCreditOpeningShapeExact(
            raw.getValue("credit_opening"),
        )

        val reencoded = mapOf(
            "payment_request" to OfflineCashNoritoV1.encodePaymentRequestShape(request),
            "acceptance_intent_authorization" to
                OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape(authorization, request),
            "acceptance_ticket" to
                OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket, request, authorization),
            "no_commit_closure" to OfflineCashNoritoV1.encodeNoCommitClosureShape(closure),
            "payment" to OfflineCashNoritoV1.encodePaymentShape(payment, request),
            "acknowledgement" to
                OfflineCashNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment),
            "mint_authorization" to OfflineCashNoritoV1.encodeMintAuthorizationShape(mintAuthorization),
            "mint_credit" to OfflineCashNoritoV1.encodeMintCreditShape(mintCredit, mintAuthorization),
            "redemption_voucher" to OfflineCashNoritoV1.encodeRedemptionVoucherShape(redemption),
            "encrypted_credit_envelope" to OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope),
            "encrypted_credit_aad" to OfflineCashNoritoV1.encodeEncryptedCreditAadShape(aad),
            "credit_opening" to OfflineCashNoritoV1.encodeCreditOpeningShape(opening),
        )

        REQUIRED_CANONICAL_FIXTURE_VALUES.forEach { name ->
            assertContentEquals(raw.getValue(name), reencoded.getValue(name), name)
        }
    }

    @Test
    fun `circuit bound digests use fixed semantic transcripts instead of Norito frames`() {
        val request = OfflineCashV1TestSupport.request()
        val authorization = OfflineCashV1TestSupport.authorization(request)
        val intent = authorization.statement.intent
        val intentTranscript = exactBytes(
            114,
            le16(intent.version),
            intent.requestDigest(),
            intent.intentId(),
            le128(intent.exactAmount),
            intent.senderOneTimeCommitment(),
        )
        assertContentEquals(
            circuitDigest("iroha:offline-cash:v1:acceptance-intent", intentTranscript),
            OfflineCashNoritoV1.acceptanceIntentDigest(intent, request),
        )

        val authorizationStatement = authorization.statement
        val authorizationTranscript = exactBytes(
            244,
            le16(authorizationStatement.version),
            intentTranscript,
            authorizationStatement.releaseId(),
            authorizationStatement.suiteId(),
            authorizationStatement.vkDigest(),
            authorizationStatement.artifactManifestDigest(),
        )
        assertContentEquals(
            circuitDigest(
                "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
                authorizationTranscript,
            ),
            OfflineCashNoritoV1.acceptanceIntentAuthorizationStatementDigestShape(
                authorizationStatement,
                request,
            ),
        )

        val closureStatement = OfflineCashV1TestSupport.noCommitClosure().statement
        val closureTranscript = exactBytes(
            498,
            le16(closureStatement.version),
            closureStatement.releaseId(),
            closureStatement.suiteId(),
            closureStatement.vkDigest(),
            closureStatement.artifactManifestDigest(),
            closureStatement.senderHardwareBindingCommitment(),
            closureStatement.requestId(),
            closureStatement.requestDigest(),
            closureStatement.acceptanceTicketId(),
            closureStatement.ticketDigest(),
            closureStatement.intentAuthorizationDigest(),
            closureStatement.intentDigest(),
            le128(closureStatement.exactAmount),
            closureStatement.senderOneTimeCommitment(),
            closureStatement.recoveryId(),
            closureStatement.cancellationNullifier(),
            closureStatement.equivalentDeliverySlotCommitment(),
        )
        assertContentEquals(
            circuitDigest("iroha:offline-cash:v1:no-commit-closure-statement", closureTranscript),
            OfflineCashNoritoV1.noCommitClosureStatementDigestShape(closureStatement),
        )

        val reservation = OfflineCashOutboxReservationV1(
            OfflineCashV1TestSupport.bytes(0xd1),
            OfflineCashOperationKindV1.SEND_SPLIT,
            OfflineCashWireV1.PAYMENT_OUTBOX_MIN_BYTES,
            7,
            11,
        )
        val reservationTranscript = exactBytes(
            56,
            reservation.reservationId(),
            le32(2),
            le32(reservation.reservedOutboxBytes),
            le64(reservation.issuedAtMs),
            le64(reservation.expiresAtMs),
        )
        assertContentEquals(
            circuitDigest("iroha:offline-cash:v1:outbox-reservation", reservationTranscript),
            OfflineCashNoritoV1.outboxReservationCommitmentShape(reservation),
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineCashNoritoV1.outboxReservationCommitmentShape(
                OfflineCashOutboxReservationV1(
                    OfflineCashV1TestSupport.bytes(0xd2),
                    OfflineCashOperationKindV1.SEND_SPLIT,
                    OfflineCashWireV1.PAYMENT_OUTBOX_MIN_BYTES - 1,
                    7,
                    11,
                ),
            )
        }

        val ticket = OfflineCashV1TestSupport.ticket(request, authorization)
        val certificate = OfflineCashV1TestSupport.payment(request, authorization, ticket).commitCertificate
        val evidenceTranscript = when (val evidence = certificate.commitEvidence) {
            is OfflineCashCommitEvidenceV1.TrustedTime ->
                exactBytes(36, le32(0), evidence.timeEvidenceCommitment())
            is OfflineCashCommitEvidenceV1.MonotonicLease ->
                exactBytes(36, le32(1), evidence.leaseEvidenceCommitment())
        }
        val certificateIdTranscript = exactBytes(
            238,
            le16(certificate.version),
            certificate.candidateEnvelopeDigest(),
            certificate.lifecycleBindingDigest(),
            certificate.transitionNullifier(),
            certificate.outboxReservationCommitment(),
            evidenceTranscript,
            certificate.hardwareProfileId(),
            le64(certificate.policyEpoch),
            certificate.hardwareTerminalCommitment(),
        )
        assertContentEquals(
            circuitDigest("iroha:offline-cash:v1:commit-certificate-id", certificateIdTranscript),
            OfflineCashNoritoV1.expectedCommitCertificateIdShape(certificate),
        )
        val certificateTranscript = exactBytes(
            270,
            le16(certificate.version),
            certificate.certificateId(),
            certificate.candidateEnvelopeDigest(),
            certificate.lifecycleBindingDigest(),
            certificate.transitionNullifier(),
            certificate.outboxReservationCommitment(),
            evidenceTranscript,
            certificate.hardwareProfileId(),
            le64(certificate.policyEpoch),
            certificate.hardwareTerminalCommitment(),
        )
        assertContentEquals(
            circuitDigest("iroha:offline-cash:v1:commit-certificate", certificateTranscript),
            OfflineCashNoritoV1.commitCertificateDigestShape(certificate),
        )

        val substitutedCertificate = OfflineCashCommitCertificateV1(
            certificate.version,
            certificate.certificateId(),
            OfflineCashV1TestSupport.bytes(0xee),
            certificate.lifecycleBindingDigest(),
            certificate.transitionNullifier(),
            certificate.outboxReservationCommitment(),
            certificate.commitEvidence,
            certificate.hardwareProfileId(),
            certificate.policyEpoch,
            certificate.hardwareTerminalCommitment(),
        )
        assertFalse(
            substitutedCertificate.certificateId().contentEquals(
                OfflineCashNoritoV1.expectedCommitCertificateIdShape(substitutedCertificate),
            ),
            "candidate substitution must invalidate the fixed-transcript certificate identity",
        )
    }

    @Test
    fun `signed JVM carriers preserve the full Rust unsigned transcript domain`() {
        val profile = OfflineCashV1TestSupport.profile
        val boundaryProfile = OfflineCashHardwareProfileV1(
            version = profile.version,
            protocolVersion = profile.protocolVersion,
            hardwareProfileId = profile.hardwareProfileId(),
            providerId = profile.providerId(),
            platformClass = profile.platformClass,
            productClassDigest = profile.productClassDigest(),
            firmwarePolicyDigest = profile.firmwarePolicyDigest(),
            enrollmentAttestationVerifierDigest = profile.enrollmentAttestationVerifierDigest(),
            attestationTrustRootsDigest = profile.attestationTrustRootsDigest(),
            allowedSuiteCommitment = profile.allowedSuiteCommitment(),
            policyEpoch = Long.MIN_VALUE,
            governanceCredentialPublicKey = profile.governanceCredentialPublicKey,
            capabilityMask = profile.capabilityMask,
            qualificationReportDigest = profile.qualificationReportDigest(),
            validFromMs = Long.MAX_VALUE,
            expiresAtMs = Long.MIN_VALUE,
        )
        val boundaryProfileRaw = OfflineCashNoritoV1.encodeHardwareProfileShape(boundaryProfile)
        val decodedProfile = OfflineCashNoritoV1.decodeHardwareProfileShapeExact(boundaryProfileRaw)
        assertEquals(Long.MIN_VALUE, decodedProfile.policyEpoch)
        assertEquals(Long.MAX_VALUE, decodedProfile.validFromMs)
        assertEquals(Long.MIN_VALUE, decodedProfile.expiresAtMs)
        assertContentEquals(boundaryProfileRaw, OfflineCashNoritoV1.encodeHardwareProfileShape(decodedProfile))

        val boundaryReservation = OfflineCashOutboxReservationV1(
            OfflineCashV1TestSupport.bytes(0xd1),
            OfflineCashOperationKindV1.SEND_SPLIT,
            -1,
            Long.MAX_VALUE,
            Long.MIN_VALUE,
        )
        assertContentEquals(
            "fc904c99266ca1728181789f606b6e421b90a04fe99edb1c8bc236f73b063b0e".hexBytes(),
            OfflineCashNoritoV1.outboxReservationCommitmentShape(boundaryReservation),
        )

        val boundaryCertificate = OfflineCashCommitCertificateV1(
            1,
            OfflineCashV1TestSupport.bytes(0xc1),
            OfflineCashV1TestSupport.bytes(0xc2),
            OfflineCashV1TestSupport.bytes(0xc3),
            OfflineCashV1TestSupport.bytes(0xc4),
            OfflineCashV1TestSupport.bytes(0xc5),
            OfflineCashCommitEvidenceV1.TrustedTime(OfflineCashV1TestSupport.bytes(0xc6)),
            OfflineCashV1TestSupport.bytes(0xc7),
            Long.MIN_VALUE,
            OfflineCashV1TestSupport.bytes(0xc8),
        )
        assertContentEquals(
            "b1fe2841e59c24eda16d2509e124bcf786199e9238cb5c168d0559aebe32cdc3".hexBytes(),
            OfflineCashNoritoV1.expectedCommitCertificateIdShape(boundaryCertificate),
        )
        assertContentEquals(
            "d8f7d13446aa7c0704894c4563c93cb468829d9e17aa7e17dcb70eb939ecc275".hexBytes(),
            OfflineCashNoritoV1.commitCertificateDigestShape(boundaryCertificate),
        )
    }

    @Test
    fun `typed opening AAD envelope and X25519 shape guards stay codec only`() {
        val opening = OfflineCashCreditOpeningV1(
            1,
            OfflineCashV1TestSupport.bytes(0xc1),
            BigInteger.valueOf(25),
            OfflineCashV1TestSupport.bytes(0xc2),
            OfflineCashV1TestSupport.bytes(0xc3),
            OfflineCashV1TestSupport.bytes(0xc4),
        )
        val openingRaw = OfflineCashNoritoV1.encodeCreditOpeningShape(opening)
        assertEquals(OfflineCashWireV1.CREDIT_OPENING_CANONICAL_BYTES, openingRaw.size)
        assertContentEquals(
            openingRaw,
            OfflineCashNoritoV1.encodeCreditOpeningShape(
                OfflineCashNoritoV1.decodeCreditOpeningShapeExactAgainst(
                    openingRaw,
                    opening.creditId(),
                    opening.amount,
                ),
            ),
        )

        val (authorization, credit) = OfflineCashV1TestSupport.mintAuthorizationAndCredit()
        val aad = OfflineCashNoritoV1.encryptedCreditAadForMintShape(authorization.statement)
        assertEquals(OfflineCashEncryptedCreditPurposeV1.MINT, aad.purpose)
        assertContentEquals(authorization.statement.creditId(), aad.creditId())
        val envelopeRaw = credit.encryptedCredit()
        val envelope = OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(envelopeRaw)
        assertEquals(
            OfflineCashWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES,
            envelope.ciphertextAndTag().size,
        )
        assertEquals(32, OfflineCashNoritoV1.encryptedCreditKdfSalt(OfflineCashV1TestSupport.x25519, envelope.ephemeralX25519PublicKey).size)
        assertTrue(OfflineCashNoritoV1.encryptedCreditKdfInfo(aad).size > 32)
        assertFailsWith<IllegalArgumentException> { OfflineCashX25519PublicKeyV1(ByteArray(32)) }
        assertFailsWith<IllegalArgumentException> { OfflineCashX25519PublicKeyV1(ByteArray(31) { 1 }) }
        assertFailsWith<IllegalArgumentException> { OfflineCashX25519PublicKeyV1(ByteArray(33) { 1 }) }
        val nonzeroLowOrderWireShape = ByteArray(32).also { it[0] = 1 }
        assertContentEquals(
            nonzeroLowOrderWireShape,
            OfflineCashX25519PublicKeyV1(nonzeroLowOrderWireShape).bytes(),
            "managed codecs must not probe X25519 elements or provide a software-crypto fallback",
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineCashEncryptedCreditEnvelopeV1(
                1,
                OfflineCashV1TestSupport.x25519,
                ByteArray(24) { 1 },
                ByteArray(OfflineCashWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES - 1) { 2 },
            )
        }
    }

    @Test
    fun `mint authorization credit and redemption use current standalone shapes`() {
        val (authorization, credit) = OfflineCashV1TestSupport.mintAuthorizationAndCredit()
        val authorizationRaw = OfflineCashNoritoV1.encodeMintAuthorizationShape(authorization)
        val creditRaw = OfflineCashNoritoV1.encodeMintCreditShape(credit, authorization)
        val redemption = OfflineCashV1TestSupport.redemption()
        val redemptionRaw = OfflineCashNoritoV1.encodeRedemptionVoucherShape(redemption)

        assertContentEquals(authorizationRaw, OfflineCashNoritoV1.encodeMintAuthorizationShape(OfflineCashNoritoV1.decodeMintAuthorizationShapeExact(authorizationRaw)))
        assertContentEquals(creditRaw, OfflineCashNoritoV1.encodeMintCreditShape(OfflineCashNoritoV1.decodeMintCreditShapeExact(creditRaw)))
        assertContentEquals(redemptionRaw, OfflineCashNoritoV1.encodeRedemptionVoucherShape(OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(redemptionRaw)))
    }

    @Test
    fun `mint credit rejects authorization artifact substitution`() {
        val (authorization, credit) = OfflineCashV1TestSupport.mintAuthorizationAndCredit()
        val substituted = OfflineCashMintCreditV1(
            credit.version,
            credit.statement,
            credit.proof,
            credit.finalityCertificateBinding(),
            credit.finalityAuthorityHead(),
            credit.finalityGenesisRosterId(),
            credit.finalityProofBindingDigest(),
            credit.encryptedCredit(),
            OfflineCashV1TestSupport.bytes(0xee),
        )

        assertFailsWith<IllegalArgumentException> {
            OfflineCashNoritoV1.encodeMintCreditShape(substituted, authorization)
        }
    }

    @Test
    fun `public current records expose no predecessor successor lane epoch or acknowledgement time`() {
        val forbidden = listOf(
            "predecessor",
            "successor",
            "senderlane",
            "senderepoch",
            "beforesequence",
            "aftersequence",
            "acknowledgedat",
            "inboxsequence",
        )
        listOf(
            OfflineCashTransferStatementV1::class.java,
            OfflineCashPaymentV1::class.java,
            OfflineCashAcknowledgementV1::class.java,
        ).forEach { type ->
            val publicNames = type.declaredFields.map { it.name } + type.declaredMethods.map { it.name }
            forbidden.forEach { fragment ->
                assertFalse(publicNames.any { it.lowercase().contains(fragment) }, "${type.simpleName} leaked $fragment")
            }
        }
    }

    @Test
    fun `generated fixture honors the first release hard cut`() {
        val fixture = sharedFixtureText()
        val raw = fixtureValue(fixture, "payment_request", "norito_hex").hexBytes()
        assertTrue(Regex("\\\"fixture_version\\\"\\s*:\\s*1").containsMatchIn(fixture))
        assertContentEquals(
            raw,
            OfflineCashNoritoV1.encodePaymentRequestShape(
                OfflineCashNoritoV1.decodePaymentRequestShapeExact(raw),
            ),
        )
    }

    @Test
    fun `wire constants and exact unpadded base64url bounds match Rust V1`() {
        assertEquals(1_024, OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES)
        assertEquals(1_370, OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES)
        assertEquals(9_984, OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES)
        assertEquals(13_326, OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES)
        assertEquals(6_528, OfflineCashWireV1.MAXIMUM_PAIRED_PROOF_BYTES)
        assertEquals(16_384, OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES)
        OfflineCashWirePayloadKindV1.values().forEach { kind ->
            val exact = ByteArray(kind.maximumRawBytes) { 0xa5.toByte() }
            val text = OfflineCashWireV1.encodeText(kind, exact)
            assertEquals(kind.maximumTextBytes, text.length)
            assertContentEquals(exact, OfflineCashWireV1.decodeText(kind, text))
            assertFailsWith<IllegalArgumentException> {
                OfflineCashWireV1.encodeText(kind, ByteArray(kind.maximumRawBytes + 1))
            }
        }
    }

    private fun circuitDigest(domain: String, transcript: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").run {
            update(domain.toByteArray(StandardCharsets.US_ASCII))
            update(byteArrayOf(0))
            update(le64(transcript.size.toLong()))
            digest(transcript)
        }

    private fun exactBytes(expectedSize: Int, vararg parts: ByteArray): ByteArray {
        val bytes = ByteArray(expectedSize)
        var offset = 0
        parts.forEach { part ->
            require(offset + part.size <= expectedSize)
            part.copyInto(bytes, offset)
            offset += part.size
        }
        require(offset == expectedSize)
        return bytes
    }

    private fun le16(value: Int): ByteArray =
        byteArrayOf(value.toByte(), (value ushr 8).toByte())

    private fun le32(value: Int): ByteArray =
        ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()

    private fun le64(value: Long): ByteArray =
        ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()

    private fun le128(value: BigInteger): ByteArray {
        val bigEndian = value.toByteArray()
        return ByteArray(16).also { littleEndian ->
            val width = minOf(littleEndian.size, bigEndian.size)
            repeat(width) { index -> littleEndian[index] = bigEndian[bigEndian.lastIndex - index] }
        }
    }

    private fun sharedFixtureText(): String {
        var directory: File? = File(System.getProperty("user.dir"))
        while (directory != null) {
            val candidate = File(directory, "fixtures/offline/offline_cash_v1.json")
            if (candidate.isFile) return candidate.readText()
            directory = directory.parentFile
        }
        error("fixtures/offline/offline_cash_v1.json not found")
    }

    private fun fixtureValue(fixture: String, section: String, name: String): String {
        val sectionOffset = fixture.indexOf("\"$section\"")
        require(sectionOffset >= 0)
        return requireNotNull(Regex("\\\"$name\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"").find(fixture, sectionOffset)).groupValues[1]
    }

    private fun String.hexBytes(): ByteArray = ByteArray(length / 2) { index ->
        substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }

    private companion object {
        val REQUIRED_CANONICAL_FIXTURE_VALUES = listOf(
            "payment_request",
            "acceptance_intent_authorization",
            "acceptance_ticket",
            "no_commit_closure",
            "payment",
            "acknowledgement",
            "mint_authorization",
            "mint_credit",
            "redemption_voucher",
            "encrypted_credit_envelope",
            "encrypted_credit_aad",
            "credit_opening",
        )
    }
}
