// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import org.hyperledger.iroha.sdk.core.model.NetworkId

internal object KagemushaV1TestSupport {
    val network: NetworkId = NetworkId.fromBytes(bytes(0x11))
    val asset: KagemushaAssetDefinitionIdV1 =
        KagemushaAssetDefinitionIdV1.parse("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    val incarnation = KagemushaAssetIncarnationV1(bytes(0x21))
    val account: KagemushaAccountIdV1 = KagemushaAccountIdV1.parse(
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
    )
    val devicePublicKey: KagemushaDevicePublicKeyV1 = run {
        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val key = generator.generateKeyPair().public as ECPublicKey
        KagemushaDevicePublicKeyV1(byteArrayOf(4) + fixed(key.w.affineX) + fixed(key.w.affineY))
    }
    val signature = KagemushaDeviceSignatureV1(ByteArray(64).also { it[31] = 1; it[63] = 1 })
    val x25519 = KagemushaX25519PublicKeyV1(ByteArray(32).also { it[0] = 9 })
    val liabilityPool: ByteArray = KagemushaNoritoV1.liabilityPoolId(network, asset, incarnation)

    val profile = KagemushaHardwareProfileV1(
        version = 1,
        protocolVersion = 1,
        hardwareProfileId = bytes(0x31),
        providerId = bytes(0x32),
        platformClass = KagemushaHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
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

    val credential = KagemushaHardwareCredentialV1(
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
        deviceKeyReference = KagemushaNoritoV1.deviceKeyReference(devicePublicKey),
        issuedAtMs = 10,
        expiresAtMs = 90_000,
        governanceSignature = signature,
    )

    val qualification = KagemushaHardwareQualificationV1(
        1,
        profile,
        credential,
        bytes(0x45),
        KagemushaHardwareCapabilityV1.values().toSet(),
    )

    fun state(sequence: Long = 0, stateTag: Int = 0x51): KagemushaAggregateStateCommitmentV1 =
        KagemushaAggregateStateCommitmentV1(
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
        amount: BigInteger = BigInteger.valueOf(25),
    ): KagemushaPaymentRequestV1 = KagemushaPaymentRequestV1(
        version = 1,
        releaseId = qualification.releaseId(),
        networkId = network,
        asset = asset,
        assetIncarnation = incarnation,
        scale = 4,
        liabilityPoolId = liabilityPool,
        recipient = account,
        recipientLaneId = credential.laneCommitment(),
        recipientEncryptionKey = x25519,
        amount = amount,
        hardwareCredential = credential,
        requestId = bytes(0x53),
        issuedAtMs = 1_000,
        expiresAtMs = 2_000,
        signature = signature,
    )

    fun envelope(tag: Int = 0x61): KagemushaEncryptedCreditEnvelopeV1 =
        KagemushaEncryptedCreditEnvelopeV1(
            1,
            x25519,
            ByteArray(KagemushaWireV1.XCHACHA20_POLY1305_NONCE_BYTES) { tag.toByte() },
            ByteArray(KagemushaWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES) { (tag + 1).toByte() },
        )

    fun payment(request: KagemushaPaymentRequestV1): KagemushaPaymentV1 {
        val encrypted = KagemushaNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope())
        val nullifier = bytes(0x63)
        fun lifecycle(creditId: ByteArray) = KagemushaLifecycleBindingV1(
            version = 1,
            networkId = request.networkId,
            protocolVersion = 1,
            suiteId = request.hardwareCredential.suiteId(),
            vkDigest = bytes(0x56),
            releaseId = request.releaseId(),
            asset = request.asset,
            assetIncarnation = request.assetIncarnation,
            scale = request.scale,
            liabilityPoolId = request.liabilityPoolId(),
            hardwareProfileId = profile.hardwareProfileId(),
            policyEpoch = profile.policyEpoch,
            operationKind = KagemushaOperationKindV1.SEND_SPLIT,
            requestId = request.requestId(),
            creditId = creditId,
            ciphertextDigest = KagemushaNoritoV1.ciphertextDigestShape(encrypted),
        )
        fun statement(lifecycle: KagemushaLifecycleBindingV1) = KagemushaTransferStatementV1(
            1,
            lifecycle,
            request.amount,
            nullifier,
            KagemushaNoritoV1.paymentRequestDigest(request),
            pasta(0x64),
            pasta(0x66),
            request.recipientLaneId(),
            request.recipientEncryptionKey,
            1_500,
            bytes(0x66),
            bytes(0x64),
        )
        val provisional = statement(lifecycle(bytes(0x67)))
        val finalStatement = statement(lifecycle(KagemushaNoritoV1.expectedPeerCreditIdShape(provisional)))
        val proof = pairedProof(
            KagemushaNoritoV1.transferStatementDigestShape(finalStatement),
            0x70,
        )
        return KagemushaPaymentV1(
            1,
            finalStatement,
            proof,
            encrypted,
        )
    }

    fun acknowledgement(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): KagemushaAcknowledgementV1 = KagemushaAcknowledgementV1(
        1,
        KagemushaNoritoV1.paymentRequestDigest(request),
        KagemushaNoritoV1.paymentDigestShape(payment, request),
        KagemushaInboxReceiptV1(1, payment.statement.lifecycle.creditId(), bytes(0x76)),
        signature,
    )

    fun mintAuthorizationAndCredit(): Pair<KagemushaMintAuthorizationV1, KagemushaMintCreditV1> {
        val context = KagemushaMintAuthorizationContextV1(
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
        val encrypted = KagemushaNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope(0x7c))
        val ciphertextDigest = KagemushaNoritoV1.ciphertextDigestShape(encrypted)
        val issuanceCommitment = bytes(0x7d)
        fun lifecycle(creditId: ByteArray) = KagemushaLifecycleBindingV1(
            1, network, 1, context.suiteId(), context.vkDigest(), context.releaseId(), asset,
            incarnation, 4, liabilityPool, context.hardwareProfileId(), context.policyEpoch,
            KagemushaOperationKindV1.MINT_FOLD, ByteArray(32), creditId,
            ciphertextDigest,
        )
        fun mintStatement(lifecycle: KagemushaLifecycleBindingV1, authorizationDigest: ByteArray) =
            KagemushaMintCreditStatementV1(
                1,
                lifecycle,
                context.recipientCredentialCommitment(),
                KagemushaNoritoV1.mintAuthorizationContextDigestShape(context),
                authorizationDigest,
                context.amount,
                issuanceCommitment,
                context.recipient,
                context.creditCommitment(),
                1_500,
            )
        val provisional = mintStatement(lifecycle(bytes(0x7e)), bytes(0x7f))
        val creditId = KagemushaNoritoV1.expectedMintCreditIdShape(provisional)
        val authorizationStatement = KagemushaMintAuthorizationStatementV1(
            1,
            context,
            issuanceCommitment,
            creditId,
            ciphertextDigest,
        )
        val authorization = KagemushaMintAuthorizationV1(
            1,
            authorizationStatement,
            pairedProof(KagemushaNoritoV1.mintAuthorizationStatementDigestShape(authorizationStatement), 0x80),
        )
        val finalStatement = mintStatement(
            lifecycle(creditId),
            KagemushaNoritoV1.mintAuthorizationDigestShape(authorization),
        )
        val credit = KagemushaMintCreditV1(
            1,
            finalStatement,
            pairedProof(KagemushaNoritoV1.mintCreditStatementDigestShape(finalStatement), 0x90),
            bytes(0x91),
            bytes(0x92),
            bytes(0x93),
            bytes(0x94),
            encrypted,
            context.artifactManifestDigest(),
        )
        return authorization to credit
    }

    fun redemption(): KagemushaRedemptionVoucherV1 {
        val nullifier = bytes(0xa2)
        val lifecycle = KagemushaLifecycleBindingV1(
            1, network, 1, credential.suiteId(), bytes(0xa3), qualification.releaseId(), asset,
            incarnation, 4, liabilityPool, profile.hardwareProfileId(), profile.policyEpoch,
            KagemushaOperationKindV1.REDEEM_SPLIT, ByteArray(32), ByteArray(32),
            ByteArray(32),
        )
        fun statement(redemptionId: ByteArray) = KagemushaRedemptionStatementV1(
            1,
            lifecycle,
            BigInteger.valueOf(12),
            account,
            nullifier,
            pasta(0xa4),
            pasta(0xa6),
            1_500,
            bytes(0xa4),
            redemptionId,
            bytes(0xa6),
        )
        val provisional = statement(bytes(0xa7))
        val finalStatement = statement(KagemushaNoritoV1.expectedRedemptionIdShape(provisional))
        return KagemushaRedemptionVoucherV1(
            1,
            finalStatement,
            pairedProof(KagemushaNoritoV1.redemptionStatementDigestShape(finalStatement), 0xb0),
        )
    }

    fun pairedProof(semanticDigest: ByteArray, tag: Int): KagemushaPairedProofV1 =
        KagemushaPairedProofV1(
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
            ByteArray(KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES) { tag.toByte() },
            ByteArray(KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES) { (tag + 1).toByte() },
        )

    fun bytes(tag: Int): ByteArray = ByteArray(32) { tag.toByte() }

    fun pasta(tag: Int): KagemushaPastaStateCommitmentV1 =
        KagemushaPastaStateCommitmentV1(bytes(tag), bytes(tag + 1))

    private fun fixed(value: BigInteger): ByteArray {
        val signed = value.toByteArray()
        val raw = if (signed.size == 33 && signed[0].toInt() == 0) signed.copyOfRange(1, 33) else signed
        return ByteArray(32).also { raw.copyInto(it, 32 - raw.size) }
    }
}
