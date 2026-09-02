// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.core.model.NetworkId

internal object OfflineCashV1TestSupport {
    val network: NetworkId = NetworkId.fromBytes(bytes(0x11))
    val asset: OfflineCashAssetDefinitionIdV1 =
        OfflineCashAssetDefinitionIdV1.parse("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    val incarnation = OfflineCashAssetIncarnationV1(bytes(0x21))
    val account: OfflineCashAccountIdV1 = OfflineCashAccountIdV1.parse(
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
    )
    val devicePublicKey: OfflineCashDevicePublicKeyV1 = run {
        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val key = generator.generateKeyPair().public as ECPublicKey
        OfflineCashDevicePublicKeyV1(byteArrayOf(4) + fixed(key.w.affineX) + fixed(key.w.affineY))
    }
    val signature = OfflineCashDeviceSignatureV1(ByteArray(64).also { it[31] = 1; it[63] = 1 })
    val x25519 = OfflineCashX25519PublicKeyV1(ByteArray(32).also { it[0] = 9 })
    val liabilityPool: ByteArray = OfflineCashNoritoV1.liabilityPoolId(network, asset, incarnation)

    val profile = OfflineCashHardwareProfileV1(
        version = 1,
        protocolVersion = 1,
        hardwareProfileId = bytes(0x31),
        providerId = bytes(0x32),
        platformClass = OfflineCashHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
        productClassDigest = bytes(0x33),
        firmwarePolicyDigest = bytes(0x34),
        enrollmentAttestationVerifierDigest = bytes(0x35),
        attestationTrustRootsDigest = bytes(0x36),
        allowedSuiteCommitment = bytes(0x37),
        policyEpoch = 1,
        governanceCredentialPublicKey = devicePublicKey,
        capabilityMask = 0xffff,
        qualificationReportDigest = bytes(0x38),
        validFromMs = 0,
        expiresAtMs = 100_000,
    )

    val credential = OfflineCashHardwareCredentialV1(
        version = 1,
        credentialId = bytes(0x41),
        networkId = network,
        hardwareProfileId = profile.hardwareProfileId(),
        suiteId = bytes(0x42),
        firmwarePolicyDigest = profile.firmwarePolicyDigest(),
        policyEpoch = profile.policyEpoch,
        laneCommitment = bytes(0x43),
        hardwareEpochId = bytes(0x44),
        hardwareEpochGeneration = 1,
        devicePublicKey = devicePublicKey,
        deviceKeyReference = OfflineCashNoritoV1.deviceKeyReference(devicePublicKey),
        issuedAtMs = 10,
        expiresAtMs = 90_000,
        governanceSignature = signature,
    )

    val qualification = OfflineCashHardwareQualificationV1(
        1,
        profile,
        credential,
        bytes(0x45),
        OfflineCashHardwareCapabilityV1.values().toSet(),
    )

    fun state(sequence: Long = 0, stateTag: Int = 0x51): OfflineCashAggregateStateCommitmentV1 =
        OfflineCashAggregateStateCommitmentV1(
            version = 1,
            releaseId = qualification.releaseId(),
            networkId = network,
            asset = asset,
            assetIncarnation = incarnation,
            scale = 4,
            liabilityPoolId = liabilityPool,
            laneId = bytes(0x52),
            hardwareEpochId = credential.hardwareEpochId(),
            keyReference = credential.deviceKeyReference(),
            hardwarePolicyId = profile.hardwareProfileId(),
            sequence = BigInteger.valueOf(sequence),
            stateCommitment = bytes(stateTag),
        )

    fun request(
        mode: OfflineCashPaymentRequestModeV1 = OfflineCashPaymentRequestModeV1.SingleExact(BigInteger.valueOf(25)),
    ): OfflineCashPaymentRequestV1 = OfflineCashPaymentRequestV1(
        version = 1,
        releaseId = qualification.releaseId(),
        networkId = network,
        asset = asset,
        assetIncarnation = incarnation,
        scale = 4,
        liabilityPoolId = liabilityPool,
        recipient = account,
        requestMode = mode,
        hardwareCredential = credential,
        requestId = bytes(0x53),
        issuedAtMs = 1_000,
        expiresAtMs = 2_000,
        signature = signature,
    )

    fun authorization(
        request: OfflineCashPaymentRequestV1,
        amount: BigInteger = BigInteger.valueOf(25),
    ): OfflineCashAcceptanceIntentAuthorizationV1 {
        val intent = OfflineCashAcceptanceIntentV1(
            1,
            OfflineCashNoritoV1.paymentRequestDigest(request),
            bytes(0x54),
            amount,
            bytes(0x55),
        )
        val statement = OfflineCashAcceptanceIntentAuthorizationStatementV1(
            1,
            intent,
            request.releaseId(),
            request.hardwareCredential.suiteId(),
            bytes(0x56),
            bytes(0x57),
        )
        return OfflineCashAcceptanceIntentAuthorizationV1(
            1,
            statement,
            pairedProof(OfflineCashNoritoV1.acceptanceIntentAuthorizationStatementDigestShape(statement, request), 0x60),
        )
    }

    fun ticket(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): OfflineCashAcceptanceTicketV1 = OfflineCashAcceptanceTicketV1(
        version = 1,
        networkId = request.networkId,
        requestId = request.requestId(),
        requestDigest = OfflineCashNoritoV1.paymentRequestDigest(request),
        acceptanceTicketId = bytes(0x58),
        asset = request.asset,
        assetIncarnation = request.assetIncarnation,
        scale = request.scale,
        requestMode = request.requestMode,
        intentDigest = OfflineCashNoritoV1.acceptanceIntentDigest(authorization.statement.intent, request),
        exactAmount = authorization.statement.intent.exactAmount,
        reservedInboxBytes = OfflineCashWireV1.ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES,
        recipientOneTimeKey = x25519,
        hardwareProfileId = request.hardwareCredential.hardwareProfileId(),
        policyEpoch = request.hardwareCredential.policyEpoch,
        issuedAtMs = 1_100,
        expiresAtMs = 1_900,
        signature = signature,
    )

    fun noCommitClosure(): OfflineCashNoCommitClosureV1 {
        val request = request()
        val authorization = authorization(request)
        val ticket = ticket(request, authorization)
        val intent = authorization.statement.intent
        val statement = OfflineCashNoCommitClosureStatementV1(
            version = 1,
            releaseId = authorization.statement.releaseId(),
            suiteId = authorization.statement.suiteId(),
            vkDigest = authorization.statement.vkDigest(),
            artifactManifestDigest = authorization.statement.artifactManifestDigest(),
            senderHardwareBindingCommitment = bytes(0x59),
            requestId = request.requestId(),
            requestDigest = OfflineCashNoritoV1.paymentRequestDigest(request),
            acceptanceTicketId = ticket.acceptanceTicketId(),
            ticketDigest = OfflineCashNoritoV1.acceptanceTicketDigest(ticket, request, authorization),
            intentAuthorizationDigest =
                OfflineCashNoritoV1.acceptanceIntentAuthorizationDigestShape(authorization, request),
            intentDigest = OfflineCashNoritoV1.acceptanceIntentDigest(intent, request),
            exactAmount = intent.exactAmount,
            senderOneTimeCommitment = intent.senderOneTimeCommitment(),
            recoveryId = bytes(0x5a),
            cancellationNullifier = bytes(0x5b),
            equivalentDeliverySlotCommitment = bytes(0x5c),
        )
        return OfflineCashNoCommitClosureV1(
            1,
            statement,
            request,
            authorization,
            ticket,
            pairedProof(OfflineCashNoritoV1.noCommitClosureStatementDigestShape(statement), 0x5d),
        )
    }

    fun envelope(tag: Int = 0x61): OfflineCashEncryptedCreditEnvelopeV1 =
        OfflineCashEncryptedCreditEnvelopeV1(
            1,
            x25519,
            ByteArray(OfflineCashWireV1.XCHACHA20_POLY1305_NONCE_BYTES) { tag.toByte() },
            ByteArray(OfflineCashWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES) { (tag + 1).toByte() },
        )

    fun payment(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ): OfflineCashPaymentV1 {
        val encrypted = OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope())
        val evidence = OfflineCashCommitEvidenceV1.TrustedTime(bytes(0x62))
        val nullifier = bytes(0x63)
        val ticketDigest = OfflineCashNoritoV1.acceptanceTicketDigest(ticket, request, authorization)
        fun lifecycle(creditId: ByteArray) = OfflineCashLifecycleBindingV1(
            version = 1,
            networkId = request.networkId,
            protocolVersion = 1,
            suiteId = request.hardwareCredential.suiteId(),
            vkDigest = authorization.statement.vkDigest(),
            releaseId = request.releaseId(),
            asset = request.asset,
            assetIncarnation = request.assetIncarnation,
            scale = request.scale,
            liabilityPoolId = request.liabilityPoolId(),
            hardwareProfileId = profile.hardwareProfileId(),
            policyEpoch = profile.policyEpoch,
            operationKind = OfflineCashOperationKindV1.SEND_SPLIT,
            requestId = request.requestId(),
            acceptanceTicketId = ticket.acceptanceTicketId(),
            creditId = creditId,
            ciphertextDigest = OfflineCashNoritoV1.ciphertextDigestShape(encrypted),
        )
        fun statement(lifecycle: OfflineCashLifecycleBindingV1) = OfflineCashTransferStatementV1(
            1,
            lifecycle,
            ticket.exactAmount,
            nullifier,
            OfflineCashNoritoV1.paymentRequestDigest(request),
            ticketDigest,
            ticket.recipientOneTimeKey,
            bytes(0x64),
            evidence,
        )
        val provisional = statement(lifecycle(bytes(0x65)))
        val finalStatement = statement(lifecycle(OfflineCashNoritoV1.expectedPeerCreditIdShape(provisional)))
        val certificate = certificate(finalStatement.lifecycle, evidence, nullifier, 0x66)
        val wrapper = wrapperProof(
            OfflineCashNoritoV1.transferStatementDigestShape(finalStatement),
            certificate,
            0x70,
        )
        return OfflineCashPaymentV1(
            1,
            finalStatement,
            authorization.statement.intent,
            ticket,
            certificate,
            wrapper,
            encrypted,
            authorization.statement.artifactManifestDigest(),
        )
    }

    fun acknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): OfflineCashAcknowledgementV1 = OfflineCashAcknowledgementV1(
        1,
        OfflineCashNoritoV1.paymentRequestDigest(request),
        OfflineCashNoritoV1.paymentDigestShape(payment, request),
        OfflineCashInboxReceiptV1(1, payment.statement.lifecycle.creditId(), bytes(0x76)),
        signature,
    )

    fun mintAuthorizationAndCredit(): Pair<OfflineCashMintAuthorizationV1, OfflineCashMintCreditV1> {
        val context = OfflineCashMintAuthorizationContextV1(
            version = 1,
            operationId = bytes(0x77),
            releaseId = qualification.releaseId(),
            suiteId = credential.suiteId(),
            vkDigest = bytes(0x78),
            artifactManifestDigest = bytes(0x79),
            networkId = network,
            asset = asset,
            assetIncarnation = incarnation,
            scale = 4,
            liabilityPoolId = liabilityPool,
            amount = BigInteger.valueOf(40),
            payer = account,
            recipient = account,
            hardwareCredentialId = credential.credentialId(),
            hardwareProfileId = profile.hardwareProfileId(),
            policyEpoch = profile.policyEpoch,
            recipientCredentialCommitment = bytes(0x7a),
            creditCommitment = bytes(0x7b),
            recipientOneTimeKey = x25519,
        )
        val encrypted = OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope(0x7c))
        val ciphertextDigest = OfflineCashNoritoV1.ciphertextDigestShape(encrypted)
        val issuanceCommitment = bytes(0x7d)
        fun lifecycle(creditId: ByteArray) = OfflineCashLifecycleBindingV1(
            1, network, 1, context.suiteId(), context.vkDigest(), context.releaseId(), asset,
            incarnation, 4, liabilityPool, context.hardwareProfileId(), context.policyEpoch,
            OfflineCashOperationKindV1.MINT_FOLD, ByteArray(32), ByteArray(32), creditId,
            ciphertextDigest,
        )
        fun mintStatement(lifecycle: OfflineCashLifecycleBindingV1, authorizationDigest: ByteArray) =
            OfflineCashMintCreditStatementV1(
                1,
                lifecycle,
                context.recipientCredentialCommitment(),
                OfflineCashNoritoV1.mintAuthorizationContextDigestShape(context),
                authorizationDigest,
                context.amount,
                issuanceCommitment,
                context.recipient,
                context.creditCommitment(),
                1_500,
            )
        val provisional = mintStatement(lifecycle(bytes(0x7e)), bytes(0x7f))
        val creditId = OfflineCashNoritoV1.expectedMintCreditIdShape(provisional)
        val authorizationStatement = OfflineCashMintAuthorizationStatementV1(
            1,
            context,
            issuanceCommitment,
            creditId,
            ciphertextDigest,
        )
        val authorization = OfflineCashMintAuthorizationV1(
            1,
            authorizationStatement,
            pairedProof(OfflineCashNoritoV1.mintAuthorizationStatementDigestShape(authorizationStatement), 0x80),
        )
        val finalStatement = mintStatement(
            lifecycle(creditId),
            OfflineCashNoritoV1.mintAuthorizationDigestShape(authorization),
        )
        val credit = OfflineCashMintCreditV1(
            1,
            finalStatement,
            pairedProof(OfflineCashNoritoV1.mintCreditStatementDigestShape(finalStatement), 0x90),
            bytes(0x91),
            bytes(0x92),
            bytes(0x93),
            bytes(0x94),
            encrypted,
            context.artifactManifestDigest(),
        )
        return authorization to credit
    }

    fun redemption(): OfflineCashRedemptionVoucherV1 {
        val evidence = OfflineCashCommitEvidenceV1.MonotonicLease(bytes(0xa1))
        val nullifier = bytes(0xa2)
        val lifecycle = OfflineCashLifecycleBindingV1(
            1, network, 1, credential.suiteId(), bytes(0xa3), qualification.releaseId(), asset,
            incarnation, 4, liabilityPool, profile.hardwareProfileId(), profile.policyEpoch,
            OfflineCashOperationKindV1.REDEEM_SPLIT, ByteArray(32), ByteArray(32), ByteArray(32),
            ByteArray(32),
        )
        fun statement(redemptionId: ByteArray) = OfflineCashRedemptionStatementV1(
            1,
            lifecycle,
            BigInteger.valueOf(12),
            account,
            nullifier,
            bytes(0xa4),
            redemptionId,
            evidence,
        )
        val provisional = statement(bytes(0xa5))
        val finalStatement = statement(OfflineCashNoritoV1.expectedRedemptionIdShape(provisional))
        val certificate = certificate(lifecycle, evidence, nullifier, 0xa6)
        return OfflineCashRedemptionVoucherV1(
            1,
            finalStatement,
            certificate,
            wrapperProof(OfflineCashNoritoV1.redemptionStatementDigestShape(finalStatement), certificate, 0xb0),
            bytes(0xb6),
        )
    }

    private fun certificate(
        lifecycle: OfflineCashLifecycleBindingV1,
        evidence: OfflineCashCommitEvidenceV1,
        nullifier: ByteArray,
        tag: Int,
    ): OfflineCashCommitCertificateV1 {
        fun create(id: ByteArray) = OfflineCashCommitCertificateV1(
            1,
            id,
            bytes(tag),
            OfflineCashNoritoV1.lifecycleDigestShape(lifecycle),
            nullifier,
            bytes(tag + 1),
            evidence,
            lifecycle.hardwareProfileId(),
            lifecycle.policyEpoch,
            bytes(tag + 2),
        )
        val provisional = create(bytes(tag + 3))
        return create(OfflineCashNoritoV1.expectedCommitCertificateIdShape(provisional))
    }

    private fun wrapperProof(
        semanticDigest: ByteArray,
        certificate: OfflineCashCommitCertificateV1,
        tag: Int,
    ): OfflineCashCommitWrapperProofV1 = OfflineCashCommitWrapperProofV1(
        1,
        bytes(tag),
        bytes(tag + 1),
        semanticDigest,
        certificate.candidateEnvelopeDigest(),
        OfflineCashNoritoV1.commitCertificateDigestShape(certificate),
        bytes(tag + 2),
        bytes(tag + 3),
        byteArrayOf(tag.toByte()),
        byteArrayOf((tag + 1).toByte()),
        ByteArray(OfflineCashWireV1.HISTORY_ACCUMULATOR_BYTES) { tag.toByte() },
        ByteArray(OfflineCashWireV1.HISTORY_ACCUMULATOR_BYTES) { (tag + 1).toByte() },
    )

    fun pairedProof(semanticDigest: ByteArray, tag: Int): OfflineCashPairedProofV1 =
        OfflineCashPairedProofV1(
            1,
            bytes(tag),
            bytes(tag + 1),
            semanticDigest,
            bytes(tag + 2),
            bytes(tag + 3),
            bytes(tag + 4),
            bytes(tag + 5),
            byteArrayOf(tag.toByte()),
            byteArrayOf((tag + 1).toByte()),
            ByteArray(OfflineCashWireV1.HISTORY_ACCUMULATOR_BYTES) { tag.toByte() },
            ByteArray(OfflineCashWireV1.HISTORY_ACCUMULATOR_BYTES) { (tag + 1).toByte() },
        )

    fun bytes(tag: Int): ByteArray = ByteArray(32) { tag.toByte() }

    private fun fixed(value: BigInteger): ByteArray {
        val signed = value.toByteArray()
        val raw = if (signed.size == 33 && signed[0].toInt() == 0) signed.copyOfRange(1, 33) else signed
        return ByteArray(32).also { raw.copyInto(it, 32 - raw.size) }
    }
}
